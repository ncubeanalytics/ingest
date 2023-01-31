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
    let config = Config {
        addr: ([127, 0, 0, 1], 0).into(), // bind to any available port
        destination_topic: "".to_string(),
        keepalive_seconds: Default::default(),
        headers: Default::default(),
        logging: Default::default(),
        librdkafka_config: Default::default(),
    };

    Server::start(config).await
}
