use anyhow::Result;
use futures::StreamExt;
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response, Server, StatusCode,
};
use tokio::task;
use tungstenite::Message;

mod config;
mod ws;

pub use config::*;

pub async fn start(config: Config) -> Result<()> {
    let service = make_service_fn(|_| async { Ok::<_, hyper::Error>(service_fn(handler)) });

    let server = Server::bind(&config.addr).serve(service);

    server.await?;

    Ok(())
}

async fn handler(req: Request<Body>) -> Result<Response<Body>> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/http") => Ok(Response::new("we are going to use http".into())),

        (&Method::GET, "/ws") => match ws::handshake(&req).await {
            // this means that the handshake failed
            Ok(res) if res.status() != StatusCode::SWITCHING_PROTOCOLS => Ok(res),

            Ok(res) => {
                task::spawn(handle_connection(req));
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

async fn handle_connection(req: Request<Body>) {
    let ws_conn = match ws::upgrade(req).await {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Failed to upgrade connection: {}", e);
            return;
        }
    };

    let (outgoing, incoming) = ws_conn.split();

    incoming
        .map(|msg| {
            Ok(Message::text(format!(
                "ola client. got your {:?} message",
                msg
            )))
        })
        .forward(outgoing)
        .await
        .unwrap();
}
