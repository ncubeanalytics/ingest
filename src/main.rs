use ingestion::server;

#[tokio::main]
async fn main() {
    let config = server::Config::default();

    server::start(config).await.expect("Error starting server");
}
