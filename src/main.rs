use common::config::CommonConfig;
use tokio::signal;
use tracing::{debug, info};

use ingest::{error::Result, Config, Server};

fn main() -> Result<()> {
    let config = Config::load()?;
    debug!("Config: {:?}", config);

    let _guard = common::logging::Logging::pre_runtime_init(
        &config.logging,
        ingest::PKG_NAME,
        ingest::PKG_VERSION,
    )?;

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(run(config))
}

async fn run(config: Config) -> Result<()> {
    let _logging = common::logging::Logging::init(
        config.logging.clone(),
        ingest::PKG_NAME,
        ingest::PKG_VERSION,
        common::logging::TokioRuntime::MultiThread,
    )?;

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
