use std::collections::HashMap;

use actix_web::{web, HttpRequest, HttpResponse, Responder};
use bytes::Bytes;

use split_newlines::split_newlines;

use crate::event::forward_to_kafka;
use crate::server::{get_tenant_id, ServerState};

mod split_newlines;

pub async fn handle(
    req: HttpRequest,
    body: Bytes,
    path: web::Path<String>,
    state: web::Data<ServerState>,
) -> impl Responder {
    let schema_id = path.into_inner();
    let data = split_newlines(body);

    forward_to_kafka(
        data,
        HashMap::from([
            (state.header_names.schema_id.clone(), schema_id),
            (
                state.header_names.ip.clone(),
                req.connection_info()
                    .realip_remote_addr()
                    .unwrap_or("")
                    .to_owned(),
            ),
        ]),
        state.kafka.clone(),
        get_tenant_id(&req),
    )
    .await
    .map(|_| HttpResponse::Created())
}
