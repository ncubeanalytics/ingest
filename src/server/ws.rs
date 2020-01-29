use async_tungstenite::{tokio::TokioAdapter, WebSocketStream};
use hyper::{upgrade::Upgraded, Body, Request};
use tungstenite::protocol::Role;

mod handshake;

pub use handshake::handshake;

/// Use AFTER SUCCESSFUL HANDSHAKE to upgrade Request to WebSocket.
pub async fn upgrade(
    req: Request<Body>,
) -> Result<WebSocketStream<TokioAdapter<Upgraded>>, hyper::Error> {
    match req.into_body().on_upgrade().await {
        Ok(upgraded) => {
            Ok(WebSocketStream::from_raw_socket(TokioAdapter(upgraded), Role::Server, None).await)
        }
        Err(e) => Err(e),
    }
}
