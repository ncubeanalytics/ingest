use actix_web::{web, HttpRequest, HttpResponse, Responder};

use crate::event::forward_to_kafka;
use crate::server::{get_tenant_id, ServerState};

pub async fn handle(
    req: HttpRequest,
    body: bytes::Bytes,
    state: web::Data<ServerState>,
) -> impl Responder {
    forward_to_kafka(body, state.kafka.clone(), get_tenant_id(&req))
        .await
        .map(|_| HttpResponse::Created())
}
