use actix_web::{web, HttpResponse, Responder};

use crate::event::forward_to_kafka;
use crate::server::ServerState;

pub async fn handle(body: bytes::Bytes, state: web::Data<ServerState>) -> impl Responder {
    forward_to_kafka(body, state.kafka.clone())
        .await
        .map(|_| HttpResponse::Ok())
}
