use env_logger::Builder;
use log::{info, LevelFilter};
use tokio::signal;

use ingest::{error::Result, Config, Server};

const ENV_LOG_CONF: &str = "RUST_LOG";

#[actix_rt::main]
async fn main() -> Result<()> {
    init_logging();

    let config = Config::default();

    let server = Server::start(config).expect("Error starting server");
    info!("Server started");

    close_signal().await?;

    info!("Server shutting down...");
    server.stop().await;

    Ok(())
}

fn init_logging() {
    let env_conf = std::env::var(ENV_LOG_CONF).unwrap_or_default();

    Builder::from_default_env()
        // Use `info` level by default
        .filter_level(LevelFilter::Info)
        // overwrite with env config
        .parse_filters(&env_conf)
        .init();
}

#[cfg(windows)]
async fn close_signal() -> Result<()> {
    signal::ctrl_c().await?
}

#[cfg(unix)]
async fn close_signal() -> Result<()> {
    use futures::stream::{select_all, StreamExt};
    use signal::unix::{self, SignalKind};

    Ok(select_all(vec![
        unix::signal(SignalKind::interrupt())?,
        unix::signal(SignalKind::terminate())?,
        unix::signal(SignalKind::quit())?,
    ])
    .next()
    .await
    .expect("Got None OS signal!"))
}
