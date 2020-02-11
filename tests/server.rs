use reqwest::{Client, StatusCode};
use url::Url;

use ingest::{config::Kafka, error::Result, Config, Server};

const HTTP_PATH: &str = "/http";

#[actix_rt::test]
async fn http_bad_request() -> Result<()> {
    let server = start_server()?;
    let client = Client::new();

    let cases = &["not json", "null", "1", r#" { "an": "object" } "#];

    for case in cases {
        let res = client.post(http_path(&server)).body(*case).send().await;

        assert!(res.is_ok());

        let res = res.unwrap();

        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    server.kill().await;

    Ok(())
}

#[actix_rt::test]
async fn http_ok() -> Result<()> {
    let server = start_server()?;
    let client = Client::new();

    let cases = &[
        r#"
        [
            {
                "single": "object"
            }
        ]
            "#,
        r#"
        [
            {
                "some": "json"
            },
            {
                "some": "more"
            }
        ]
        "#,
        r#"
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
        "#,
    ];

    for case in cases {
        let res = client.post(http_path(&server)).body(*case).send().await;

        assert!(res.is_ok());

        let res = res.unwrap();

        assert_eq!(res.status(), StatusCode::OK);
    }

    server.kill().await;

    Ok(())
}

fn start_server() -> Result<Server> {
    let config = Config {
        addr: ([127, 0, 0, 1], 0).into(), // bind to any available port
        kafka: Kafka {
            timeout_ms: "1000".to_string(), // don't wait too long
            ..Kafka::default()
        },
    };

    Server::start(config)
}

fn http_path(server: &Server) -> Url {
    let mut url = Url::parse(&format!(
        "http://{}/",
        &server.addrs().first().unwrap().to_string()
    ))
    .unwrap();

    url.set_path(HTTP_PATH);

    url
}