use common::config::CommonConfig;
use tokio::signal;
use tracing::{debug, info};

use ingest::{error::Result, Config, Server};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load()?;
    common::logging::init(&config.logging)?;
    debug!("Config: {:?}", config);

    info!("Starting server...");
    let server = Server::start(config).await?;
    info!("Server started");

    close_signal().await?;
    info!("Shutting down...");
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
    use tokio_stream::wrappers::SignalStream;

    select_all(vec![
        SignalStream::new(unix::signal(SignalKind::interrupt())?),
        SignalStream::new(unix::signal(SignalKind::terminate())?),
        SignalStream::new(unix::signal(SignalKind::quit())?),
    ])
    .next()
    .await;

    Ok(())
}
