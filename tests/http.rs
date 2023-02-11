use reqwest::{Client, StatusCode};

use ingest::{error::Result as IResult, Config, Server};

const DATA: &str = r#"
    [
        {
            "some": {
                "nested": "data"
            }
        },
        {
            "some": {
                "deeper": {
                    "nested": "data"
                }
            }
        }
    ]
    "#;

#[tokio::test]
async fn http_ok() -> IResult<()> {
    let server = start_server().await?;
    let client = Client::new();

    let addr = &server.addrs().first().unwrap().to_string();

    let res = client
        .post(format!("http://{}/1", addr))
        .body(DATA)
        .send()
        .await
        .expect("failed to send good http request to server");

    assert_eq!(res.status(), StatusCode::CREATED);

    server.kill().await;

    Ok(())
}

async fn start_server() -> IResult<Server> {
    let mut config: Config = serde_json::from_str(
        r#"
        {"addr": "127.0.0.1:0",
        "destination_topic": "test",
        "librdkafka_config": {"bootstrap.servers":"localhost:9092" }
        }"#,
    )
    .unwrap();

    if let Ok(broker_address) = std::env::var("BROKER_ADDRESS") {
        config
            .librdkafka_config
            .insert("bootstrap.servers".to_owned(), broker_address);
    }

    Server::start(config).await
}
