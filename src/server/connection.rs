use futures::StreamExt;
use hyper::{Body, Request};
use tokio::select;

use super::ws::upgrade;

/// Handles a websocket connection AFTER A SUCCESSFUL HANDSHAKE.
pub async fn handle(req: Request<Body>) {
    let ws_conn = match upgrade(req).await {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Failed to upgrade connection: {}", e);
            return;
        }
    };

    let (mut _outgoing, mut incoming) = ws_conn.split();

    loop {
        select! {
            _msg = incoming.next() => {
                unimplemented!();
            }
        }
    }
}
