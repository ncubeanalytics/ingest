use tokio::signal;
use tracing::{debug, info};

use common::config::CommonConfig;

use ingest::{error::Result, logging, Config, Server};

#[actix_rt::main]
async fn main() -> Result<()> {
    let config = Config::load()?;

    logging::init(&config)?;

    let server = Server::start(config)?;
    info!("Server started");

    close_signal().await?;
    debug!("Received close signal");

    info!("Server shutting down...");
    server.stop().await;

    Ok(())
}

#[cfg(windows)]
async fn close_signal() -> Result<()> {
    signal::ctrl_c().await?
}

#[cfg(unix)]
async fn close_signal() -> Result<()> {
    use futures::stream::{select_all, StreamExt};
    use signal::unix::{self, SignalKind};

    select_all(vec![
        unix::signal(SignalKind::interrupt())?,
        unix::signal(SignalKind::terminate())?,
        unix::signal(SignalKind::quit())?,
    ])
    .next()
    .await;

    Ok(())
}
