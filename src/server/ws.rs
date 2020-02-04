use hyper::{upgrade::Upgraded, Body, Request};
use tokio_tungstenite::WebSocketStream;
use tungstenite::protocol::Role;

mod handshake;

pub use handshake::handshake;

/// Use AFTER SUCCESSFUL HANDSHAKE to upgrade Request to WebSocket.
pub async fn upgrade(req: Request<Body>) -> Result<WebSocketStream<Upgraded>, hyper::Error> {
    match req.into_body().on_upgrade().await {
        Ok(upgraded) => Ok(WebSocketStream::from_raw_socket(upgraded, Role::Server, None).await),
        Err(e) => Err(e),
    }
}
