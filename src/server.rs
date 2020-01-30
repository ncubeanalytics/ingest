use anyhow::Result;
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response, StatusCode,
};
use tokio::{
    sync::oneshot,
    task::{self, JoinHandle},
};

mod config;
mod connection;
mod kafka;
mod ws;

pub use config::*;

pub struct Server {
    _config: Config,
    hyper_handle: JoinHandle<hyper::Result<()>>,
    hyper_close: oneshot::Sender<()>,
}

impl Server {
    pub fn start(config: Config) -> Result<Self> {
        let service = make_service_fn(|_| async { Ok::<_, hyper::Error>(service_fn(handler)) });

        let (hyper_close, hyper_close_rx) = oneshot::channel();

        let hyper_handle = tokio::spawn(
            hyper::Server::bind(&config.addr)
                .serve(service)
                .with_graceful_shutdown(async {
                    hyper_close_rx.await.ok();
                }),
        );

        println!("Server started and listening on {}", &config.addr);

        Ok(Server {
            _config: config,
            hyper_handle,
            hyper_close,
        })
    }

    pub async fn stop(self) {
        let _ = self.hyper_close.send(());
        let _ = self.hyper_handle.await;
    }
}

async fn handler(req: Request<Body>) -> Result<Response<Body>> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/http") => Ok(Response::new("we are going to use http".into())),

        (&Method::GET, "/ws") => match ws::handshake(&req).await {
            // this means that the handshake failed
            Ok(res) if res.status() != StatusCode::SWITCHING_PROTOCOLS => Ok(res),

            Ok(res) => {
                task::spawn(connection::handle(req));
                Ok(res)
            }

            Err(e) => {
                eprintln!("Error accepting websocket connection: {}", e);

                Ok(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::empty())?)
            }
        },

        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body("Not found".into())?),
    }
}
