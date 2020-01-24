use std::net::SocketAddr;

use anyhow::Result;
use async_tungstenite as ws;
use futures::{future, StreamExt};
use tokio::{
    net::{TcpListener, TcpStream},
    task,
};

mod config;
pub use config::*;

pub async fn start(config: Config) -> Result<()> {
    let mut socket = TcpListener::bind(&config.addr).await?;

    println!("Listening for websocket connections on {}", config.addr);

    while let Ok((stream, addr)) = socket.accept().await {
        task::spawn(handle_connection(stream, addr));
    }

    Ok(())
}

async fn handle_connection(stream: TcpStream, addr: SocketAddr) {
    let adapter = ws::tokio::TokioAdapter(stream);
    let ws_stream = ws::accept_async(adapter)
        .await
        .expect("Error accepting ws connection");

    println!("new connection from {}", addr);

    ws_stream
        .for_each(|msg| {
            println!("got message from {}: {:#?}", addr, msg);

            future::ready(())
        })
        .await;
}
