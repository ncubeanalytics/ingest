use actix_web::{dev::Server as ActixServer, web, App, HttpServer};
use log::info;
use rdkafka::producer::FutureProducer;

use crate::{error::Result, kafka, Config};

mod connection;

pub struct Server {
    http_server: ActixServer,
    kafka_producer: FutureProducer,
}

pub struct ServerState {
    kafka_producer: FutureProducer,
    config: Config,
}

impl Server {
    pub fn start(config: Config) -> Result<Self> {
        let kafka_producer = kafka::new_producer(&config.kafka)?;

        let state_kafka_producer = kafka_producer.clone();
        let state_config = config.clone();

        let http_server = HttpServer::new(move || {
            let kafka_producer = state_kafka_producer.clone();
            let config = state_config.clone();

            App::new()
                .data(ServerState {
                    kafka_producer,
                    config,
                })
                .route("/http", web::get().to(connection::http::handle))
                .route("/ws", web::get().to(connection::ws::handle))
        })
        .disable_signals()
        .bind(&config.addr)?
        .run();

        Ok(Server {
            http_server,
            kafka_producer,
        })
    }

    pub async fn stop(self) {
        info!("Stopping http/ws server");
        // true means gracefully
        self.http_server.stop(true).await;

        info!("Stopping kafka producer");
        kafka::stop_producer(&self.kafka_producer);
    }
}
