use std::ops::Index;

use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};
use bytes::Bytes;

use split_newlines::split_newlines;

use crate::config::ContentType;
use crate::error::Result;
use crate::event::forward_to_kafka;
use crate::python::call_processor_process;
use crate::server::{get_tenant_id, ServerState};

mod split_newlines;

pub async fn handle(
    req: HttpRequest,
    body: Bytes,
    path: web::Path<String>,
    state: web::Data<ServerState>,
) -> Result<HttpResponse> {
    let _tenant_id = get_tenant_id(&req);
    let schema_id = path.into_inner();

    let schema_config = state
        .schema_configs
        .get(&schema_id)
        .unwrap_or(&state.default_schema_config);

    let mut response_status = schema_config.response_status;
    let mut response_headers: Vec<(String, String)> = Vec::new();
    let mut response_body: Vec<u8> = b"".to_vec();
    let mut forward = true;
    if let Some(python_processor) = state
        .python_processors
        .get(&schema_id)
        .or(state.default_python_processor.as_ref())
    {
        if let Some((
            py_forward,
            py_response_status_opt,
            py_response_headers_opt,
            py_response_body_opt,
        )) = call_processor_process(
            python_processor,
            req.uri().to_string().as_str(),
            &req.method().to_string(),
            req.headers()
                .iter()
                .map(|(k, v)| (k.as_str(), v.to_str().unwrap()))
                .collect::<Vec<(&str, &str)>>()
                .as_slice(),
            Vec::from(body.clone()).as_slice(),
        )? {
            forward = py_forward;
            if let Some(py_response_status) = py_response_status_opt {
                response_status = py_response_status;
            }

            if let Some(py_response_headers) = py_response_headers_opt {
                response_headers = py_response_headers;
            }

            if let Some(py_response_body) = py_response_body_opt {
                response_body = py_response_body;
            }
        }
    } else if !schema_config
        .allowed_methods
        .contains(&req.method().to_string())
    {
        return Ok(HttpResponse::MethodNotAllowed().finish());
    }

    if forward {
        let conn_info = req.connection_info();
        let mut headers: Vec<(&str, &[u8])> = vec![
            (&state.header_names.schema_id, (&schema_id).as_bytes()),
            (
                &state.header_names.ip,
                conn_info.realip_remote_addr().unwrap_or("").as_bytes(), // .to_owned()
                                                                         // .into(),
            ),
            // TODO:
            //("ncube-ingest-schema-revision".to_owned(), revision_number),
            // ("ncube-ingest-tenant-id".to_owned(), tenant_id.to_string()),
        ];

        let url: String;
        if schema_config.forward_request_url {
            url = req.uri().to_string();
            headers.push((&state.header_names.http_url, &(url).as_bytes()))
        }

        let method: String;
        if schema_config.forward_request_method {
            method = req.method().to_string();
            headers.push((&state.header_names.http_method, &(method).as_bytes()))
        }

        let mut header_keys: Vec<String>;
        if schema_config.forward_request_http_headers {
            header_keys = Vec::with_capacity(req.headers().len());
            for (k, _) in req.headers() {
                header_keys.push(state.header_names.http_header_prefix.clone() + k.as_str());
            }
            for (i, (_, v)) in req.headers().iter().enumerate() {
                // header_keys.push(state.header_names.http_header_prefix.clone() + k.as_str());
                let name = header_keys.index(i);
                headers.push((name, v.as_bytes()))
            }
        }

        let content_type = if schema_config.content_type_from_header {
            if let Some(req_content_type) = req
                .headers()
                .get("content-type")
                .map(|h| h.to_str().ok())
                .flatten()
            {
                match req_content_type {
                    "application/x-ndjson"
                    | "application/jsonlines"
                    | "application/x-jsonlines" => ContentType::Jsonlines,
                    _ => ContentType::Json,
                }
            } else {
                schema_config
                    .content_type
                    .clone()
                    .unwrap_or(ContentType::Json)
            }
        } else {
            schema_config
                .content_type
                .clone()
                .unwrap_or(ContentType::Json)
        };

        let data = match content_type {
            ContentType::Jsonlines => split_newlines(body),
            ContentType::Json => vec![body],
        };

        forward_to_kafka(
            data,
            headers,
            state.kafka.clone(),
            &schema_config.destination_topic,
        )
        .await?;
    }
    let mut response_builder = HttpResponse::build(StatusCode::from_u16(response_status).unwrap());
    for h in response_headers {
        response_builder.append_header(h);
    }
    Ok(response_builder.body(response_body))
}
