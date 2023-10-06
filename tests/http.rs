use reqwest::{Client, Method, Response, StatusCode};

use ingest::{error::Result as IResult, Config, Server};

async fn start_server(config: serde_json::Value) -> IResult<Server> {
    let _ = common::logging::Logging::init(
        common::logging::LoggingConfig {
            fmt_json: false,
            console: true,
            otel_metrics: false,
            otel_tracing: false,
        },
        "test",
        "test",
        false,
    );
    let config: Config = serde_json::from_value(config).unwrap();
    Server::start(config).await
}

async fn get_response(
    server_config: serde_json::Value,
    schema_id: &str,
    body: &'static str,
    method: Method,
) -> IResult<Response> {
    let server = start_server(server_config).await?;
    let client = Client::new();

    let addr = &server.addrs().first().unwrap().to_string();

    let res = client
        .request(method, format!("http://{}/{}", addr, schema_id))
        .body(body)
        .send()
        .await
        .expect("failed to send good http request to server");
    server.kill().await;
    Ok(res)
}

fn broker_addr() -> String {
    std::env::var("BROKER_ADDRESS").unwrap_or("localhost:9092".to_owned())
}

fn server_config(service_config: serde_json::Value) -> serde_json::Value {
    let mut conf = service_config.clone();
    conf["addr"] = serde_json::json!("127.0.0.1:0");
    serde_json::json!({
        "service": conf,
        "librdkafka_config": {"bootstrap.servers": broker_addr().as_str()}
    })
}

// language=json
const DATA: &str = r#"[{"some":{"nested":"data"}},{"some":{"deeper":{"nested":"data"}}}]"#;

// language=jsonlines
const DATALINES: &str = r#"
{"line1": "1"}
{"line2": "2"}
"#;

async fn assert_status_and_empty(res: Response, status: StatusCode) {
    assert_eq!(res.status(), status);
    assert_eq!(res.headers()["content-length"], "0");
    assert_eq!(res.text().await.unwrap(), "");
}

#[tokio::test]
async fn test_response_default() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
        }
    }));

    let res = get_response(config, "1", DATA, Method::POST).await.unwrap();
    assert_status_and_empty(res, StatusCode::OK).await;
}

#[tokio::test]
async fn test_response_customized() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "response_status": 201,
            "forward_request_url": true,
            "forward_request_http_headers": true,
            "forward_request_method": true
        }
    }));

    let res = get_response(config, "1", DATA, Method::POST).await.unwrap();
    assert_status_and_empty(res, StatusCode::CREATED).await;
}

#[tokio::test]
async fn test_response_custom_method() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "allowed_methods": ["POST", "PUT"],
            "forward_request_method": true
        }
    }));

    let res = get_response(config, "1", DATA, Method::PUT).await.unwrap();
    assert_status_and_empty(res, StatusCode::OK).await;
}

#[tokio::test]
async fn test_response_not_allowed_method() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test"
        }
    }));

    let res = get_response(config, "1", DATA, Method::PUT).await.unwrap();
    assert_status_and_empty(res, StatusCode::METHOD_NOT_ALLOWED).await;
}

#[tokio::test]
async fn test_response_ndjson() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "content_type": "application/jsonlines"
        }
    }));

    let res = get_response(config, "1", DATALINES, Method::POST)
        .await
        .unwrap();
    assert_status_and_empty(res, StatusCode::OK).await;
}

#[tokio::test]
async fn test_response_schema_specific_config() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "response_status": 200,
        },
        "schema_config": [
            {"schema_id": "2",
                "response_status": 201,
            }
        ]
    }));

    let server = start_server(config).await.unwrap();
    let client = Client::new();
    let addr = &server.addrs().first().unwrap().to_string();

    let res = client
        .request(Method::POST, format!("http://{}/{}", addr, "2"))
        .body(DATA)
        .send()
        .await
        .unwrap();
    assert_status_and_empty(res, StatusCode::CREATED).await;

    let res = client
        .request(Method::POST, format!("http://{}/{}", addr, "1"))
        .body(DATA)
        .send()
        .await
        .unwrap();
    assert_status_and_empty(res, StatusCode::OK).await;
}

#[tokio::test]
async fn test_response_python_fail() {
    std::env::set_var(
        "PYTHONPATH",
        env!("CARGO_MANIFEST_DIR").to_owned() + "/tests",
    );

    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "python_request_processor": "python_processors:FailingProcessor"
        },
        "python_plugin_src_dir": env!("CARGO_MANIFEST_DIR").to_owned() + "/src/python"
    }));

    let res = get_response(config, "1", DATA, Method::POST).await.unwrap();
    assert_status_and_empty(res, StatusCode::INTERNAL_SERVER_ERROR).await;
}

#[tokio::test]
async fn test_response_python_response() {
    std::env::set_var(
        "PYTHONPATH",
        env!("CARGO_MANIFEST_DIR").to_owned() + "/tests",
    );

    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "python_request_processor": "python_processors:StaticProcessor"
        },
        "python_plugin_src_dir": env!("CARGO_MANIFEST_DIR").to_owned() + "/src/python"
    }));

    let res = get_response(config, "1", DATA, Method::POST).await.unwrap();
    assert_eq!(res.status(), StatusCode::IM_A_TEAPOT);
    assert_eq!(res.headers()["a"], "b");
    assert_eq!(res.headers()["c"], "d");
    assert_eq!(res.text().await.unwrap(), "body");
}

#[tokio::test]
async fn test_response_python_conditional_response() {
    std::env::set_var(
        "PYTHONPATH",
        env!("CARGO_MANIFEST_DIR").to_owned() + "/tests",
    );

    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "python_request_processor": "python_processors:MirrorGetProcessor"
        },
        "python_plugin_src_dir": env!("CARGO_MANIFEST_DIR").to_owned() + "/src/python"
    }));

    let server = start_server(config).await.unwrap();
    let client = Client::new();
    let addr = &server.addrs().first().unwrap().to_string();

    let res = client
        .request(Method::POST, format!("http://{}/{}", addr, "1"))
        .body("no")
        .send()
        .await
        .unwrap();
    assert_status_and_empty(res, StatusCode::OK).await;

    let res = client
        .request(Method::GET, format!("http://{}/{}", addr, "1"))
        .header("a", "b")
        .header("c", "d")
        .body("yes")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::IM_A_TEAPOT);
    assert_eq!(res.headers()["a"], "b");
    assert_eq!(res.headers()["c"], "d");
    assert_eq!(res.text().await.unwrap(), "yes");

    server.kill().await;
}
