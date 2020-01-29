use anyhow::Result;
use tokio::signal;

use ingest::server::{Config, Server};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::default();

    let server = Server::start(config).expect("Error starting server");

    signal::ctrl_c().await?;

    println!("\nClosing server");

    server.stop().await;

    Ok(())
}
