use std::error::Error;

use futures::{SinkExt, StreamExt};
use reqwest::{Client, StatusCode};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async as ws_connect,
    tungstenite::{
        protocol::{frame::coding::CloseCode, CloseFrame},
        Message as WSMessage, Result as WSResult,
    },
    WebSocketStream,
};
use url::Url;

use ingest::{error::Result as IResult, Config, Server};

const HTTP_PATH: &str = "/http";
const WS_PATH: &str = "/ws";

const INVALID_DATA: &[&str] = &["not json", "null", "1", r#" { "an": "object" } "#];
const VALID_DATA: &[&str] = &[
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

#[actix_rt::test]
async fn http_bad_request() -> IResult<()> {
    let server = start_server()?;
    let client = Client::new();

    for case in INVALID_DATA {
        let res = client.post(http_path(&server)).body(*case).send().await;

        assert!(res.is_ok());

        let res = res.unwrap();

        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    server.kill().await;

    Ok(())
}

#[actix_rt::test]
async fn http_ok() -> IResult<()> {
    let server = start_server()?;
    let client = Client::new();

    for case in VALID_DATA {
        let res = client.post(http_path(&server)).body(*case).send().await;

        assert!(res.is_ok());

        let res = res.unwrap();

        assert_eq!(res.status(), StatusCode::OK);
    }

    server.kill().await;

    Ok(())
}

#[actix_rt::test]
async fn ws_unsuccessful() -> Result<(), Box<dyn Error>> {
    let server = start_server()?;

    let mut client = ws_client(&server).await?;

    for case in INVALID_DATA {
        let text_msg = WSMessage::text(*case);
        let byte_msg = WSMessage::binary(case.as_bytes());

        for msg in vec![text_msg, byte_msg] {
            client
                .send(msg.clone())
                .await
                .expect("failed to send ws text message");

            let res = next_json(&mut client).await?;

            if res["success"] == true {
                panic!(
                    r#"Websocket request "{}" expected unsuccessful response, but got "{}""#,
                    msg, res
                );
            }
        }
    }

    server.kill().await;

    Ok(())
}

#[actix_rt::test]
async fn ws_successful() -> Result<(), Box<dyn Error>> {
    let server = start_server()?;

    let mut client = ws_client(&server).await?;

    for case in VALID_DATA {
        let text_msg = WSMessage::text(*case);
        let byte_msg = WSMessage::binary(case.as_bytes());

        for msg in vec![text_msg, byte_msg] {
            client
                .send(msg.clone())
                .await
                .expect("failed to send ws text message");

            let res = next_json(&mut client).await?;

            if res["success"] == false {
                panic!(
                    r#"Websocket request "{}" expected successful response, but got "{}""#,
                    msg, res
                );
            }
        }
    }

    server.kill().await;

    Ok(())
}

#[actix_rt::test]
async fn ws_close() -> Result<(), Box<dyn Error>> {
    let server = start_server()?;

    let mut client = ws_client(&server).await?;

    server.stop().await;

    let ws_close = client
        .next()
        .await
        .expect("failed to receive ws close message");

    match ws_close {
        Ok(WSMessage::Close(Some(CloseFrame {
            code: CloseCode::Restart,
            ..
        }))) => {}

        _ => panic!(
            "Expected websocket client to get close frame with Restart code, but got {:?}",
            ws_close
        ),
    }

    Ok(())
}

fn start_server() -> IResult<Server> {
    let mut config = Config::load()?;

    // bind to any available port
    config.addr = ([127, 0, 0, 1], 0).into();
    config.kafka.timeout_ms = "5000".to_string();

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

fn ws_path(server: &Server) -> Url {
    let mut url = Url::parse(&format! {
        "ws://{}/",
        &server.addrs().first().unwrap().to_string()
    })
    .unwrap();

    url.set_path(WS_PATH);

    url
}

async fn ws_client(server: &Server) -> WSResult<WebSocketStream<TcpStream>> {
    ws_connect(ws_path(server)).await.map(|(stream, _)| stream)
}

fn extract_json(ws_result: Option<WSResult<WSMessage>>) -> Result<json::JsonValue, Box<dyn Error>> {
    match ws_result {
        Some(Ok(WSMessage::Text(t))) => Ok(json::parse(&t)?),
        Some(Ok(WSMessage::Binary(b))) => Ok(json::parse(String::from_utf8(b)?.as_str())?),
        _ => Err(format!("Expected JSON ws response data; got {:#?}", ws_result).into()),
    }
}

async fn next_json(
    client: &mut WebSocketStream<TcpStream>,
) -> Result<json::JsonValue, Box<dyn Error>> {
    extract_json(client.next().await)
}
