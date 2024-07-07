use std::ops::Index;

use actix_web::error::{ErrorBadRequest, ErrorPayloadTooLarge, PayloadError};
use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse, ResponseError};
use async_stream::stream;
use bytes::Bytes;
use futures::{pin_mut, Stream};
use tracing::{instrument, trace};

use futures::stream::StreamExt;
use serde::Serialize;
use tokio::sync::mpsc;
use tokio_util::codec::{FramedRead, LinesCodec, LinesCodecError};
use tokio_util::io::StreamReader;

use crate::config::{ContentType, HeaderNames, SchemaConfig};
use crate::error::{Error, Result};
use crate::event::forward_message_to_kafka;
use crate::kafka::Kafka;
use crate::python::{call_processor_process, call_processor_process_head, ProcessorResponse};
use crate::server::{PythonProcessor, ServerState};

pub async fn handle_with_trailing_path(
    req: HttpRequest,
    body_stream: web::Payload,
    path: web::Path<(String, String)>,
    state: web::Data<ServerState>,
) -> Result<HttpResponse> {
    _handle(req, body_stream, path.into_inner().0, state).await
}

pub async fn handle(
    req: HttpRequest,
    body_stream: web::Payload,
    path: web::Path<String>,
    state: web::Data<ServerState>,
) -> Result<HttpResponse> {
    _handle(req, body_stream, path.into_inner(), state).await
}

async fn _handle(
    req: HttpRequest,
    mut body_stream: web::Payload,
    schema_id: String,
    state: web::Data<ServerState>,
) -> Result<HttpResponse> {
    // do something with tenant from authentication or config if/when multitenant
    //let _tenant_id = get_tenant_id(&req);

    let schema_config = state
        .schema_configs
        .get(&schema_id)
        .unwrap_or(&state.default_schema_config);

    if !schema_config
        .allowed_methods
        .contains(&req.method().to_string())
    {
        return Ok(HttpResponse::MethodNotAllowed().finish());
    }

    let mut response_status = schema_config.response_status;
    let mut response_headers: Vec<(String, String)> = Vec::new();
    let mut response_body_opt: Option<Vec<u8>> = None;

    let mut should_forward = true;

    let body_read;
    // TODO: consider replacing with a custom struct that exposes .read() to python, and at the same
    // time stores the read bytes and can be later passed as a stream to forward()
    if let Some(python_processor) = state
        .python_processor_resolver
        .get(&schema_id, &req.method().to_string())
    {
        let (r, body_read_) = process_python(
            &req,
            python_processor,
            &mut body_stream,
            state.max_event_size_bytes as usize,
        )
        .await?;
        body_read = body_read_;

        if let Some(ProcessorResponse {
            forward: py_forward,
            response_status: py_response_status_opt,
            response_headers: py_response_headers_opt,
            response_body: py_response_body_opt,
        }) = r
        {
            should_forward = py_forward;
            if let Some(py_response_status) = py_response_status_opt {
                response_status = py_response_status;
            }

            if let Some(py_response_headers) = py_response_headers_opt {
                response_headers = py_response_headers;
            }

            if py_response_body_opt.is_some() {
                response_body_opt = py_response_body_opt;
            }
        }
    } else {
        body_read = Bytes::new();
    }

    let ingest_response = if should_forward {
        let s = stream! {
            yield Ok(body_read);
            while let Some(chunk) = body_stream.next().await {
                yield chunk;
            }
        };
        pin_mut!(s);
        forward(
            req,
            s,
            &schema_id,
            schema_config,
            &state.header_names,
            state.kafka.clone(),
            state.max_event_size_bytes as usize,
        )
        .await
    } else {
        IngestResponse {
            ingested_count: 0,
            ingested_bytes: 0,
            ingested_content_type: ContentType::Binary,
            ingested_schema_id: schema_id,
            error: None,
        }
    };

    if let Some(e) = &ingest_response.error {
        response_status = e.status_code().as_u16();
    }

    let mut response_builder = HttpResponse::build(StatusCode::from_u16(response_status).unwrap());
    // if we have a body, send it, otherwise build a json one. if we build a json one, also set the
    // content-type header if not set
    let mut content_type_seen = false;
    for h in response_headers {
        if h.0.to_lowercase() == "content-type" {
            content_type_seen = true;
        }
        response_builder.append_header(h);
    }
    let response_body = if let Some(response_body) = response_body_opt {
        response_body
    } else {
        if !content_type_seen {
            response_builder.append_header(("Content-Type", "application/json"));
        }
        serde_json::to_vec(&ingest_response).unwrap()
    };

    Ok(response_builder.body(response_body))
}

#[instrument(level = "debug", skip_all, fields(body_read_size))]
pub async fn process_python(
    req: &HttpRequest,
    python_processor: &PythonProcessor,
    body_stream: &mut web::Payload,
    read_max_body_bytes: usize,
) -> Result<(Option<ProcessorResponse>, Bytes)> {
    let url = req.uri().to_string();
    let method = req.method().to_string();

    if python_processor.implements_process_head {
        let res;
        // use spawn_blocking if blocking
        // https://docs.rs/actix-rt/latest/actix_rt/task/fn.spawn_blocking.html
        if python_processor.process_head_is_blocking {
            res = {
                let url = url.clone();
                let method = method.clone();
                let headers = req.headers().clone();
                let processor = python_processor.processor.clone();

                tokio::task::spawn_blocking(move || {
                    call_processor_process_head(&processor, url.as_str(), &method, headers)
                })
            }
            .await
            .unwrap()?;
        } else {
            res = call_processor_process_head(
                &python_processor.processor,
                url.as_str(),
                &method,
                req.headers().clone(),
            )?;
        }
        if let Some(r) = res {
            return Ok((Some(r), Bytes::new()));
        }
    }

    let mut read_body = web::BytesMut::new();
    let mut remaining = web::BytesMut::new();
    while let Some(item) = body_stream.next().await {
        let chunk = item.map_err(|e| Error::from(actix_web::Error::from(e)))?;
        let read_limit_exceeded = (read_body.len() + chunk.len()) > read_max_body_bytes;
        if read_limit_exceeded && read_body.len() > 0 {
            remaining.extend_from_slice(&chunk);
        } else {
            read_body.extend_from_slice(&chunk);
        }
        if read_limit_exceeded {
            break;
        }
    }

    let res;
    // use spawn_blocking if blocking
    // https://docs.rs/actix-rt/latest/actix_rt/task/fn.spawn_blocking.html
    if python_processor.process_is_blocking {
        res = {
            let url = url.clone();
            let method = method.clone();
            let headers = req.headers().clone();
            let processor = python_processor.processor.clone();
            let body = read_body.clone();

            tokio::task::spawn_blocking(move || {
                call_processor_process(
                    &processor,
                    url.as_str(),
                    &method,
                    headers,
                    Vec::from(body).as_slice(),
                )
            })
        }
        .await
        .unwrap()?;
    } else {
        res = call_processor_process(
            &python_processor.processor,
            url.as_str(),
            &method,
            req.headers().clone(),
            Vec::from(read_body.clone()).as_slice(),
        )?
    }
    read_body.extend_from_slice(&remaining);
    tracing::Span::current().record("body_read_size", read_body.len());
    Ok((res, read_body.freeze()))
}

#[derive(Serialize)]
pub struct IngestResponse {
    pub ingested_count: u64,
    pub ingested_bytes: u128,
    pub ingested_content_type: ContentType,
    pub ingested_schema_id: String,
    #[serde(skip)]
    pub error: Option<Error>,
}

#[instrument(
    level = "debug",
    skip_all,
    fields(schema_id, ip_address, content_type, message_count)
)]
pub async fn forward(
    req: HttpRequest,
    body_stream: impl Stream<Item = std::result::Result<Bytes, PayloadError>>,
    schema_id: &str,
    schema_config: &SchemaConfig,
    header_names: &HeaderNames,
    kafka: Kafka,
    max_event_size_bytes: usize,
) -> IngestResponse {
    let conn_info = req.connection_info();
    let ip_address = conn_info.realip_remote_addr().unwrap_or("");
    tracing::Span::current().record("ip_address", ip_address);

    let mut headers: Vec<(String, Bytes)> = vec![
        (
            header_names.schema_id.clone(),
            Bytes::from(schema_id.as_bytes().to_vec()),
        ),
        (
            header_names.ip.clone(),
            Bytes::from(ip_address.as_bytes().to_vec()),
        ),
        //("ncube-ingest-schema-revision".to_owned(), revision_number),
        // ("ncube-ingest-tenant-id".to_owned(), tenant_id.to_string()),
    ];

    if schema_config.forward_ingest_version {
        headers.push((
            header_names.ingest_version.clone(),
            Bytes::from(crate::PKG_VERSION),
        ))
    }

    let url: String;
    if schema_config.forward_request_url {
        url = req.uri().to_string();
        headers.push((
            header_names.http_url.clone(),
            Bytes::from(url.as_bytes().to_vec()),
        ))
    }

    let method: String;
    if schema_config.forward_request_method {
        method = req.method().to_string();
        headers.push((
            header_names.http_method.clone(),
            Bytes::from(method.as_bytes().to_vec()),
        ))
    }

    let mut header_keys: Vec<String>;
    if schema_config.forward_request_http_headers {
        header_keys = Vec::with_capacity(req.headers().len());
        for (k, _) in req.headers() {
            header_keys.push(header_names.http_header_prefix.clone() + k.as_str());
        }
        for (i, (_, v)) in req.headers().iter().enumerate() {
            // header_keys.push(header_names.http_header_prefix.clone() + k.as_str());
            let name = header_keys.index(i);
            headers.push((name.clone(), Bytes::from(v.as_bytes().to_vec())))
        }
    }

    let content_type = if schema_config.content_type_from_header {
        if let Some(req_content_type) = req
            .headers()
            .get("content-type")
            .map(|h| h.to_str().ok())
            .flatten()
        {
            // Content-Type contents defined at https://www.rfc-editor.org/rfc/rfc9110#media.type
            match req_content_type.split(";").next().unwrap_or("") {
                "application/x-ndjson" | "application/jsonlines" | "application/x-jsonlines" => {
                    ContentType::Jsonlines
                }
                "application/json" => ContentType::Json,
                _ => ContentType::Binary,
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

    tracing::Span::current().record("content_type", tracing::field::display(&content_type));
    let mut messages_received: u64 = 0;
    let mut messages_delivered: u64 = 0;
    let mut bytes_count: u128 = 0;
    let mut error = None;
    match content_type {
        ContentType::Json | ContentType::Binary => {
            messages_received = 1;
            tracing::Span::current().record("message_count", messages_received);

            // read the body until done or until it exceeds max_event_size_bytes
            let mut body = web::BytesMut::new();
            pin_mut!(body_stream); // needed for iteration
            while let Some(item) = body_stream.next().await {
                let chunk = match item {
                    Err(e) => {
                        return IngestResponse {
                            ingested_count: messages_delivered,
                            ingested_bytes: bytes_count,
                            ingested_content_type: content_type,
                            ingested_schema_id: schema_id.to_owned(),
                            error: Some(Error::from(actix_web::Error::from(e))),
                        }
                    }
                    Ok(item) => item,
                };
                body.extend_from_slice(&chunk);
                if body.len() > max_event_size_bytes {
                    return IngestResponse {
                        ingested_count: messages_delivered,
                        ingested_bytes: bytes_count,
                        ingested_content_type: content_type,
                        ingested_schema_id: schema_id.to_owned(),
                        error: Some(Error::from(ErrorPayloadTooLarge(PayloadError::Overflow))),
                    };
                }
            }

            let body = if let ContentType::Json = content_type {
                let s = String::from_utf8(body.as_ref().to_vec());
                match s {
                    Err(e) => {
                        return IngestResponse {
                            ingested_count: messages_delivered,
                            ingested_bytes: bytes_count,
                            ingested_content_type: content_type,
                            ingested_schema_id: schema_id.to_owned(),
                            error: Some(Error::from(ErrorBadRequest(e))),
                        };
                    }
                    Ok(s) => Bytes::from(s.trim().as_bytes().to_vec()),
                }
            } else {
                body.freeze()
            };
            bytes_count = body.len() as u128;
            match forward_message_to_kafka(
                body,
                headers,
                kafka,
                &schema_config.destination_topic,
                &schema_config.librdkafka_config,
            )
            .await
            {
                Err(e) => {
                    return IngestResponse {
                        ingested_count: messages_delivered,
                        ingested_bytes: bytes_count,
                        ingested_content_type: content_type,
                        ingested_schema_id: schema_id.to_owned(),
                        error: Some(Error::from(e)),
                    }
                }
                Ok(_) => {
                    messages_delivered += 1;
                }
            };
        }
        ContentType::Jsonlines => {
            let (delivered_tx, mut delivered_rx) = mpsc::channel(512);

            pin_mut!(body_stream);
            // convert the bytes stream into an async read and use line codec framing to turn that
            // into a lines stream
            // https://docs.rs/tokio-util/latest/tokio_util/io/struct.StreamReader.html
            // https://docs.rs/tokio/1.32.0/tokio/io/trait.AsyncBufReadExt.html#method.read_until

            let stream_reader = StreamReader::new(
                body_stream
                    // StreamReader needs errors to be std::io::Error, so convert them
                    .map(|result| {
                        result.map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))
                    }),
            );

            let mut newline_stream = FramedRead::new(
                stream_reader,
                LinesCodec::new_with_max_length(max_event_size_bytes),
            );

            let mut newline_stream_done = false;

            loop {
                tokio::select! {
                    line_opt = newline_stream.next(), if !newline_stream_done => {
                        if let Some(line) = line_opt {
                            match line.map_err(|e| {
                                return match e {
                                    LinesCodecError::MaxLineLengthExceeded => {
                                        Error::from(ErrorPayloadTooLarge(PayloadError::Overflow))
                                    }
                                    LinesCodecError::Io(io_error) => Error::from(io_error),
                                };
                            }) {
                                Err(e) => {
                                    // on error set the current error and stop reading the request
                                    // stream. don't exit early to give a chance to in-flight
                                    // produces to finish. exiting is handled in the delivery
                                    // listener
                                    error = Some(e);
                                    newline_stream_done = true;
                                },
                                Ok(data) => {
                                    let data = data.trim(); // XXX: create new codec that returns bytes, not string
                                    if data.len() > 0 {
                                        messages_received += 1;
                                        trace!(messages_received, messages_delivered, "JSON received");
                                        tracing::Span::current().record("message_count", messages_received);

                                        {
                                            // XXX: should not have to do all this copying
                                            // kafka produce should happen in the same task, we should
                                            // only listen for deliveries possibly somewhere else
                                            // should probably create a producer version that
                                            // returns a stream of deliveries
                                            // and listen to that stream of deliveries
                                            // only problem is how to deal with queue full
                                            // if we make send async just to deal with it?
                                            // should the queue waiting be behind a global lock so
                                            // that re-enqueues happen in order?
                                            // should each thread receive a position in the waiting
                                            // queue so that when the queue is freed, re-sends
                                            // happen in order
                                            // also on the delivery stream. the same producer may
                                            // have several delivery streams? or just one at any time?
                                            // API could be you turn it on, then all delivery callbacks
                                            // put results in some queue?
                                            // and a stream reads that queue
                                            // does a channel work for this?
                                            // then you turn it off?
                                            // what if producer is used by many concurrent tasks,
                                            // each task wants its own delivery stream
                                            // so we need to pass some state to the producer send
                                            // so that it knows where to put the delivery
                                            let headers = headers.clone();
                                            let kafka = kafka.clone();
                                            let data = Bytes::from(data.as_bytes().to_vec());
                                            let destination_topic = schema_config.destination_topic.to_owned();
                                            let librdkafka_config = schema_config.librdkafka_config.to_owned();
                                            let delivered_tx = delivered_tx.clone();
                                            tokio::spawn(async move {
                                                // XXX: limit spawns up to a number
                                                // https://users.rust-lang.org/t/tokio-number-of-concurrent-tasks/40450
                                                // https://github.com/tokio-rs/tokio/discussions/2648
                                                // https://users.rust-lang.org/t/limited-concurrency-for-future-execution-tokio/87171
                                                let res =
                                                    forward_message_to_kafka(data, headers, kafka, &destination_topic, &librdkafka_config)
                                                        .await;
                                                let _ = delivered_tx.send(res).await;
                                            });
                                        }
                                    }
                                }
                            }
                        } else {
                            trace!(messages_received, messages_delivered, "end of JSON lines received");
                            newline_stream_done = true;
                            // disables this select branch
                        }
                    }
                    Some(res) = delivered_rx.recv() => {
                        match res {
                            Err(e) => {
                                // on delivery error, set the error (but don't overwrite an error
                                // already set by a request stream error), and stop waiting for any
                                // further deliveries
                                if error.is_none() {
                                    error = Some(Error::from(e))
                                }
                                delivered_rx.close();
                            },
                            Ok(data_len) => {
                                bytes_count += data_len as u128;
                                messages_delivered += 1;
                                trace!(messages_received, messages_delivered, "JSON line delivered");
                                if messages_delivered == messages_received && newline_stream_done {
                                    delivered_rx.close();
                                    // close the receiver so that recv() returns None, disabling this select branch
                                }
                            }
                        }
                    }
                    else => break,
                };
            }
        }
    };
    IngestResponse {
        ingested_count: messages_delivered,
        ingested_bytes: bytes_count,
        ingested_content_type: content_type,
        ingested_schema_id: schema_id.to_owned(),
        error,
    }
}
