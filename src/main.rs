use tokio::signal;

use ingest::{error::Result, Config, Server};

#[actix_rt::main]
async fn main() -> Result<()> {
    let config = Config::default();

    let server = Server::start(config).expect("Error starting server");
    println!("Server started");

    close_signal().await?;

    println!("\nServer shutting down...");
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

    Ok(select_all(vec![
        unix::signal(SignalKind::interrupt())?,
        unix::signal(SignalKind::terminate())?,
        unix::signal(SignalKind::quit())?,
    ])
    .next()
    .await
    .expect("Got None OS signal!"))
}
