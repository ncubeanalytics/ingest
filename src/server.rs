use anyhow::Result;
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response, StatusCode,
};
use tokio::{
    sync::{mpsc, oneshot},
    task::{self, JoinHandle},
};

use crate::event;

mod config;
mod connection;
mod ws;

pub(crate) mod kafka;

pub use config::*;

use kafka::KafkaEvent;

/// Arbitrary channel buffer size.
const CHAN_BUF: usize = 1024;

pub struct Server {
    _config: Config,
    kafka_handle: JoinHandle<()>,
    kafka_close: oneshot::Sender<()>,
    hyper_handle: JoinHandle<hyper::Result<()>>,
    hyper_close: oneshot::Sender<()>,
}

impl Server {
    pub fn start(config: Config) -> Result<Self> {
        // initialize kafka
        let producer = kafka::new_producer(&config.kafka)?;
        let (kafka_close, kafka_close_rx) = oneshot::channel();
        let (kafka_send, kafka_send_rx) = mpsc::channel(CHAN_BUF);

        let kafka_handle = task::spawn(kafka::start(
            producer,
            config.kafka.clone(),
            kafka_send_rx,
            kafka_close_rx,
        ));

        // initialize hyper
        let service = make_service_fn(move |_| {
            let k_send = kafka_send.clone();

            async { Ok::<_, hyper::Error>(service_fn(move |req| handler(req, k_send.clone()))) }
        });

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
            kafka_handle,
            kafka_close,
            hyper_handle,
            hyper_close,
        })
    }

    pub async fn stop(self) {
        let _ = self.kafka_close.send(());
        let _ = self.kafka_handle.await;

        let _ = self.hyper_close.send(());
        let _ = self.hyper_handle.await;
    }
}

async fn handler(
    req: Request<Body>,
    kafka_send: mpsc::Sender<KafkaEvent>,
) -> Result<Response<Body>> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/http") => match event::forward(req, kafka_send).await {
            Ok(()) => Ok(Response::new(Body::empty())),

            Err(e) if e.is::<json::Error>() => Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(format!(
                    "Invalid JSON: {}",
                    e.downcast::<json::Error>().unwrap()
                )))
                .map_err(|e| e.into()),

            Err(e) => {
                eprintln!("Failed to forward messages to kafka: {}", e);

                internal_server_error()
            }
        },

        (&Method::GET, "/ws") => match ws::handshake(&req).await {
            // this means that the handshake failed
            Ok(res) if res.status() != StatusCode::SWITCHING_PROTOCOLS => Ok(res),

            Ok(res) => {
                task::spawn(connection::handle(req));
                Ok(res)
            }

            Err(e) => {
                eprintln!("Error accepting websocket connection: {}", e);

                internal_server_error()
            }
        },

        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body("Not found".into())?),
    }
}

fn internal_server_error() -> Result<Response<Body>> {
    Ok(Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::empty())?)
}
