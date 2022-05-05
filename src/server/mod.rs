use std::net::SocketAddr;

use actix_web::{
    dev::{Server as ActixServer, Service},
    web, App, HttpRequest, HttpResponse, HttpServer,
};
use common::logging;
use futures::FutureExt;
use tracing::{debug, info, warn};
use tracing_futures::Instrument;

pub use connection::ws::WSError;
use state::ServerState;

use crate::{error::Result, kafka::Kafka, Config};

mod connection;
mod state;

pub struct Server {
    http_server: ActixServer,
    kafka: Kafka,
    state: web::Data<ServerState>,
    bound_addrs: Vec<SocketAddr>,
}

impl Server {
    pub fn start(config: Config) -> Result<Self> {
        let kafka = Kafka::start(&config)?;

        let state = web::Data::new(ServerState::new(kafka.clone()));
        let app_state = state.clone();

        let http_server = HttpServer::new(move || {
            let state = app_state.clone();

            App::new()
                .app_data(state)
                .wrap_fn(|req, srv| {
                    // initialize logging for this request
                    let span = logging::req_span(&req);
                    let _span_guard = span.enter();

                    debug!("Received new HTTP request");

                    srv.call(req)
                        .map(|res| {
                            debug!("Sending back response");
                            res
                        })
                        .in_current_span()
                })
                .service(
                    web::resource("/{schema_id}").route(web::post().to(connection::http::handle)),
                )
                .service(web::resource("/ws").route(web::get().to(connection::ws::handle)))
                .default_service(web::route().to(|| HttpResponse::NotFound()))
        })
        .disable_signals()
        .bind(&config.addr)?;

        // in case we bind to any available port
        let bound_addrs = http_server.addrs();

        Ok(Server {
            http_server: http_server.run(),
            kafka,
            bound_addrs,
            state,
        })
    }

    /// Will gracefully stop the server.
    pub async fn stop(self) {
        debug!("Closing all WebSocket connections");
        self.state.close_all_ws().await;

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

fn get_tenant_id(_req: &HttpRequest) -> i64 {
    // TODO: implement this function when authentication is ready
    1
}
