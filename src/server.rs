use std::net::SocketAddr;

use actix_web::{
    dev::{Server as ActixServer, Service},
    web, App, HttpServer,
};
use futures::FutureExt;
use tracing::{debug, info, warn};
use tracing_futures::Instrument;

use crate::{error::Result, kafka::Kafka, logging, Config};

mod connection;

pub struct Server {
    http_server: ActixServer,
    kafka: Kafka,
    bound_addrs: Vec<SocketAddr>,
}

pub struct ServerState {
    kafka: Kafka,
}

impl Server {
    pub fn start(config: Config) -> Result<Self> {
        let kafka = Kafka::start(config.kafka.clone())?;

        let state_kafka = kafka.clone();

        let http_server = HttpServer::new(move || {
            App::new()
                .data(ServerState {
                    kafka: state_kafka.clone(),
                })
                .wrap_fn(|req, srv| {
                    // initialize logging for this request
                    let span = logging::req_span(&req);

                    span.in_scope(|| {
                        debug!("Received new HTTP request");
                    });

                    srv.call(req)
                        .map(|res| {
                            debug!("Sending back response");
                            res
                        })
                        .instrument(span)
                })
                .route("/http", web::post().to(connection::http::handle))
                .route("/ws", web::get().to(connection::ws::handle))
        })
        .disable_signals()
        .bind(&config.addr)?;

        // in case we bind to any available port
        let bound_addrs = http_server.addrs();

        Ok(Server {
            http_server: http_server.run(),
            kafka,
            bound_addrs,
        })
    }

    /// Will gracefully stop the server.
    pub async fn stop(self) {
        info!("Stopping web server");
        // true means gracefully
        self.http_server.stop(true).await;

        info!("Stopping kafka producer");
        self.kafka.stop();
    }

    /// Will ungracefully shut the server down.
    /// Use `stop` for graceful shutdown.
    pub async fn kill(self) {
        warn!("Killing server");
        self.http_server.stop(false).await;
    }

    pub fn addrs(&self) -> &[SocketAddr] {
        &self.bound_addrs
    }
}
