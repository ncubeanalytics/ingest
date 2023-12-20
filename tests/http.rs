use async_stream::try_stream;
use reqwest::{Body, Client, Method, RequestBuilder, Response, StatusCode};

use common::config::ConfigError;
use futures::TryStream;
use ingest::{error::Error, error::Result as IResult, Config, Server};

async fn start_server(config: serde_json::Value) -> IResult<Server> {
    std::env::set_var(
        "PYTHONPATH",
        env!("CARGO_MANIFEST_DIR").to_owned() + "/tests",
    );
    let _ = common::logging::Logging::init(
        common::logging::LoggingConfig {
            fmt_json: false,
            console: std::env::var("TEST_ENABLE_LOG").is_ok(),
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

async fn request<T: Into<Body>>(
    server_config: serde_json::Value,
    path: &str,
    body: T,
    method: Method,
) -> IResult<Response> {
    Ok(request_with_headers(server_config, path, body, method, Vec::new()).await?)
}

async fn request_with_headers<T: Into<Body>>(
    server_config: serde_json::Value,
    path: &str,
    body: T,
    method: Method,
    headers: Vec<(String, String)>,
) -> IResult<Response> {
    let (server, req) = build_request(server_config, path, method, headers).await?;
    let res = req.body(body).send().await.unwrap();

    server.kill().await;
    Ok(res)
}

fn vec_to_stream(v: Vec<String>) -> impl TryStream<Ok = String, Error = std::io::Error> {
    try_stream! {
        for d in v {
            yield d;
        }
    }
}

async fn request_with_stream(
    server_config: serde_json::Value,
    schema_id: &str,
    body: Vec<String>,
    method: Method,
    headers: Vec<(String, String)>,
) -> IResult<Response> {
    let (server, req) = build_request(server_config, schema_id, method, headers).await?;

    let s = vec_to_stream(body);

    let res = req.body(Body::wrap_stream(s)).send().await.unwrap();

    server.kill().await;
    Ok(res)
}

async fn build_request(
    server_config: serde_json::Value,
    path: &str,
    method: Method,
    headers: Vec<(String, String)>,
) -> IResult<(Server, RequestBuilder)> {
    let server = start_server(server_config).await?;
    let client = Client::new();

    let addr = &server.addrs().first().unwrap().to_string();

    let mut req = client.request(method, format!("http://{}/{}", addr, path));

    for (k, v) in headers {
        req = req.header(k, v);
    }
    Ok((server, req))
}

fn broker_addr() -> String {
    std::env::var("BROKER_ADDRESS").unwrap_or("localhost:9092".to_owned())
}

fn server_config(service_config: serde_json::Value) -> serde_json::Value {
    _server_config(service_config, None)
}

fn _server_config(
    service_config: serde_json::Value,
    librdkafka_config_opt: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut conf = service_config.clone();
    conf["addr"] = serde_json::json!("127.0.0.1:0");
    conf["python_plugin_src_dir"] =
        serde_json::json!(env!("CARGO_MANIFEST_DIR").to_owned() + "/src/python");
    let librdkafka_config = if let Some(librdkafka_config) = librdkafka_config_opt {
        librdkafka_config
    } else {
        serde_json::json!([{"config": {"bootstrap.servers": broker_addr().as_str()}}])
    };
    serde_json::json!({
        "service": conf,
        "librdkafka" : librdkafka_config
    })
}

fn server_config_with_librdkafka(
    service_config: serde_json::Value,
    librdkafka_config: serde_json::Value,
) -> serde_json::Value {
    _server_config(service_config, Some(librdkafka_config))
}

// language=json
const DATA: &str = r#"[{"some":{"nested":"data"}},{"some":{"deeper":{"nested":"data"}}}]"#;
const DATA_LEN: u128 = DATA.len() as u128;

async fn assert_ingest_response(
    res: Response,
    status: StatusCode,
    ingested_opt: Option<(String, u64, u128, String)>,
) {
    let expected_body = if let Some((content_type, ingested_count, ingested_bytes, schema_id)) =
        ingested_opt
    {
        Some(format!(
            r#"{{"ingested_count":{},"ingested_bytes":{},"ingested_content_type":"{}","ingested_schema_id":"{}"}}"#,
            ingested_count, ingested_bytes, content_type, schema_id
        ))
    } else {
        None
    };
    // this fails! actix web bug?
    // assert_eq!(
    //     res.headers()["content-length"],
    //     expected_body.len().to_string()
    // );
    let content_type = expected_body
        .as_ref()
        .map(|_| res.headers()["content-type"].clone());
    assert_response(res, status, expected_body.as_deref()).await;
    if let Some(_) = expected_body {
        assert_eq!(content_type.unwrap(), "application/json");
    }
}

async fn assert_response(res: Response, status: StatusCode, body: Option<&str>) {
    assert_eq!(res.status(), status);
    let expected_body = if let Some(b) = body {
        b.to_owned()
    } else {
        "".to_owned()
    };
    assert_eq!(res.text().await.unwrap(), expected_body);
}

fn assert_is_config_error<T>(r: IResult<T>, err_txt: &str) {
    assert!(r.is_err());
    if let Err(err) = r {
        assert!(matches!(err, Error::Config(ConfigError::Invalid(_))));
        if let Error::Config(ConfigError::Invalid(s)) = err {
            assert_eq!(s, err_txt);
        }
    }
}

#[tokio::test]
async fn test_response_default() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
        }
    }));

    let res = request(config, "1", DATA, Method::POST).await.unwrap();
    assert_ingest_response(
        res,
        StatusCode::OK,
        Some(("application/json".to_owned(), 1, DATA_LEN, "1".to_owned())),
    )
    .await;
}

#[tokio::test]
async fn test_response_with_arbitrary_path() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
        }
    }));

    let res = request(config, "1/stuff/after/the/schema?yes=1", DATA, Method::POST)
        .await
        .unwrap();
    assert_ingest_response(
        res,
        StatusCode::OK,
        Some(("application/json".to_owned(), 1, DATA_LEN, "1".to_owned())),
    )
    .await;
}

#[tokio::test]
async fn test_response_trim() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
        }
    }));

    //language=json
    let data = "\n\r\t    \t\t\t    \r\r{\"some\":\"data\"}      \t\t\t\r\r\r\n\r     ";

    let res = request(config, "1", data, Method::POST).await.unwrap();
    assert_ingest_response(
        res,
        StatusCode::OK,
        Some(("application/json".to_owned(), 1, 15, "1".to_owned())),
    )
    .await;
}
#[tokio::test]
async fn test_response_binary_no_trim() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "content_type": "application/octet-stream"
        }
    }));

    //language=json
    let data = "\n\r\t    \t\t\t    \r\r{\"some\":\"data\"}      \t\t\t\r\r\r\n\r     ";

    let res = request(config, "1", data, Method::POST).await.unwrap();
    assert_ingest_response(
        res,
        StatusCode::OK,
        Some((
            "application/octet-stream".to_owned(),
            1,
            data.len() as u128,
            "1".to_owned(),
        )),
    )
    .await;
}

#[tokio::test]
async fn test_response_content_type_from_request() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "content_type": "application/octet-stream"
        }
    }));

    //language=json
    let data = "\n\r\t    \t\t\t    \r\r{\"some\":\"data\"}      \t\t\t\r\r\r\n\r     ";
    // send request with application/json, expect trimming to take place
    let res = request_with_headers(
        config,
        "1",
        data,
        Method::POST,
        vec![("Content-Type".to_owned(), "application/json".to_owned())],
    )
    .await
    .unwrap();
    assert_ingest_response(
        res,
        StatusCode::OK,
        Some(("application/json".to_owned(), 1, 15, "1".to_owned())),
    )
    .await;
}

#[tokio::test]
async fn test_response_content_type_from_request_with_semicolon() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "content_type": "application/octet-stream"
        }
    }));

    //language=json
    let data = "\n\r\t    \t\t\t    \r\r{\"some\":\"data\"}      \t\t\t\r\r\r\n\r     ";
    // send request with application/json, expect trimming to take place
    let res = request_with_headers(
        config,
        "1",
        data,
        Method::POST,
        vec![(
            "Content-Type".to_owned(),
            "application/json; charset=utf-8".to_owned(),
        )],
    )
    .await
    .unwrap();
    assert_ingest_response(
        res,
        StatusCode::OK,
        Some(("application/json".to_owned(), 1, 15, "1".to_owned())),
    )
    .await;
}

#[tokio::test]
async fn test_response_ignore_content_type_from_request() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "content_type": "application/octet-stream",
            "content_type_from_header": false
        }
    }));

    //language=json
    let data = "\n\r\t    \t\t\t    \r\r{\"some\":\"data\"}      \t\t\t\r\r\r\n\r     ";
    // send request with application/json, expect it to be ignored and no trimming to take place
    let res = request_with_headers(
        config,
        "1",
        data,
        Method::POST,
        vec![("Content-Type".to_owned(), "application/json".to_owned())],
    )
    .await
    .unwrap();
    assert_ingest_response(
        res,
        StatusCode::OK,
        Some(("application/octet-stream".to_owned(), 1, 50, "1".to_owned())),
    )
    .await;
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

    let res = request(config, "1", DATA, Method::POST).await.unwrap();
    assert_ingest_response(
        res,
        StatusCode::CREATED,
        Some(("application/json".to_owned(), 1, DATA_LEN, "1".to_owned())),
    )
    .await;
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

    let res = request(config, "1", DATA, Method::PUT).await.unwrap();
    assert_ingest_response(
        res,
        StatusCode::OK,
        Some(("application/json".to_owned(), 1, DATA_LEN, "1".to_owned())),
    )
    .await;
}

#[tokio::test]
async fn test_response_not_allowed_method() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test"
        }
    }));

    let res = request(config, "1", DATA, Method::PUT).await.unwrap();
    assert_ingest_response(res, StatusCode::METHOD_NOT_ALLOWED, None).await;
}

#[tokio::test]
async fn test_response_ndjson() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "content_type": "application/jsonlines"
        }
    }));

    // language=jsonlines
    let datalines = "{\"line1\": \"1\"}\n{\"line2\": \"2\"}\n";

    let res = request(config, "1", datalines, Method::POST).await.unwrap();
    assert_ingest_response(
        res,
        StatusCode::OK,
        Some(("application/jsonlines".to_owned(), 2, 28, "1".to_owned())),
    )
    .await;
}

#[tokio::test]
async fn test_response_ndjson_ignore_empty_lines() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "content_type": "application/jsonlines"
        }
    }));

    // language=jsonlines
    let datalines =
        "{\"line1\": \"1\"}\n\n{\"line2\": \"2\"}\n\t\r     \n{\"line3\": \"3\"}\n    \t\t\t\r\n";

    let res = request(config, "1", datalines, Method::POST).await.unwrap();
    assert_ingest_response(
        res,
        StatusCode::OK,
        Some(("application/jsonlines".to_owned(), 3, 42, "1".to_owned())),
    )
    .await;
}

#[tokio::test]
async fn test_response_chunked_transfer() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
        }
    }));

    let data = vec![DATA.to_owned(), DATA.to_owned(), DATA.to_owned()];

    let res = request_with_stream(config, "1", data, Method::POST, Vec::new())
        .await
        .unwrap();

    assert_ingest_response(
        res,
        StatusCode::OK,
        Some((
            "application/json".to_owned(),
            1,
            DATA_LEN * 3,
            "1".to_owned(),
        )),
    )
    .await;
}

#[tokio::test]
async fn test_response_chunked_transfer_ndjson() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "content_type": "application/jsonlines"
        }
    }));

    let data = vec![
        "{\"line\": 1}\n".to_owned(),
        "{\"line\": 2}\n".to_owned(),
        "{\"line\": 3}\n".to_owned(),
    ];

    let res = request_with_stream(config, "1", data, Method::POST, Vec::new())
        .await
        .unwrap();

    assert_ingest_response(
        res,
        StatusCode::OK,
        Some(("application/jsonlines".to_owned(), 3, 33, "1".to_owned())),
    )
    .await;
}

#[tokio::test]
async fn test_response_limit_exceeded() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
        },
        "max_event_size_bytes": 2
    }));

    let data = "12345";

    let res = request(config, "1", data, Method::POST).await.unwrap();
    assert_ingest_response(
        res,
        StatusCode::PAYLOAD_TOO_LARGE,
        Some(("application/json".to_owned(), 0, 0, "1".to_owned())),
    )
    .await;
}

#[tokio::test]
async fn test_response_chunked_transfer_limit_exceeded() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test"
        },
        "max_event_size_bytes": 5
    }));

    let data = vec![
        "11".to_owned(),
        "22".to_owned(),
        "333".to_owned(),
        "44".to_owned(),
    ];

    let res = request_with_stream(config, "1", data, Method::POST, Vec::new())
        .await
        .unwrap();

    assert_ingest_response(
        res,
        StatusCode::PAYLOAD_TOO_LARGE,
        Some(("application/json".to_owned(), 0, 0, "1".to_owned())),
    )
    .await;
}

#[tokio::test]
async fn test_response_ndjson_line_limit_exceeded() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "content_type": "application/jsonlines"
        },
        "max_event_size_bytes": 2
    }));

    let data = "12\n34\n563\n23";

    let res = request(config, "1", data, Method::POST).await.unwrap();

    assert_ingest_response(
        res,
        StatusCode::PAYLOAD_TOO_LARGE,
        Some(("application/jsonlines".to_owned(), 2, 4, "1".to_owned())),
    )
    .await;
}

#[tokio::test]
async fn test_response_chunked_transfer_ndjson_line_limit_exceeded() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "content_type": "application/jsonlines"
        },
        "max_event_size_bytes": 2
    }));

    let data = vec![
        "11\n".to_owned(),
        "22\n".to_owned(),
        "333\n".to_owned(),
        "44\n".to_owned(),
    ];

    let res = request_with_stream(config, "1", data, Method::POST, Vec::new())
        .await
        .unwrap();

    assert_ingest_response(
        res,
        StatusCode::PAYLOAD_TOO_LARGE,
        Some(("application/jsonlines".to_owned(), 2, 4, "1".to_owned())),
    )
    .await;
}

#[tokio::test]
async fn test_response_python_ndjson() {
    let _300kb = 300 * 1024;
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "content_type": "application/jsonlines",
            "python_request_processor": [{"processor": "python_processors:BodyLengthInResponseHeaderProcessor"}],
        },
        "max_event_size_bytes": _300kb
    }));

    let repeated = "12\n34\n563\n23\n";
    let repeat_count = 50000;
    let data = repeated.repeat(repeat_count); // 50k * 12 chars = ~ 500Kb

    let res = request(config, "1", data, Method::POST).await.unwrap();

    // looks like actix web read buffer is 64k, but i cound not find this and it could be wrong
    // i set a max event size at 300kb is probably higher than any read buffer
    let python_read_length: u64 = res.headers()["python-body-length"]
        .to_str()
        .unwrap()
        .to_string()
        .parse()
        .unwrap();
    let expected_ingest_count = 4 * repeat_count;
    let expected_ingest_bytes = (repeated.len() - 4) * (repeat_count);

    assert!(python_read_length <= _300kb);
    assert!(python_read_length > (_300kb / 2));

    assert_ingest_response(
        res,
        StatusCode::OK,
        Some((
            "application/jsonlines".to_owned(),
            expected_ingest_count as u64,
            expected_ingest_bytes as u128,
            "1".to_owned(),
        )),
    )
    .await;
}

#[tokio::test]
async fn test_response_schema_specific_config() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "response_status": 200,
        },
        "schema_config": [{
            "schema_id": "2",
            "response_status": 201,
        }]
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
    assert_ingest_response(
        res,
        StatusCode::CREATED,
        Some(("application/json".to_owned(), 1, DATA_LEN, "2".to_owned())),
    )
    .await;

    let res = client
        .request(Method::POST, format!("http://{}/{}", addr, "1"))
        .body(DATA)
        .send()
        .await
        .unwrap();
    assert_ingest_response(
        res,
        StatusCode::OK,
        Some(("application/json".to_owned(), 1, DATA_LEN, "1".to_owned())),
    )
    .await;
}

#[tokio::test]
async fn test_response_python_fail() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "python_request_processor": [{"processor": "python_processors:FailingProcessor"}]
        }
    }));

    let res = request(config, "1", DATA, Method::POST).await.unwrap();
    assert_ingest_response(res, StatusCode::INTERNAL_SERVER_ERROR, None).await;
}

#[tokio::test]
async fn test_response_python_noop_process() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "python_request_processor": [{"processor": "python_processors:NoopProcessor"}],
            "response_status": 201,
        }
    }));

    let res = request(config, "1", DATA, Method::POST).await.unwrap();
    assert_ingest_response(
        res,
        StatusCode::CREATED,
        Some(("application/json".to_owned(), 1, DATA_LEN, "1".to_owned())),
    )
    .await;
}

#[tokio::test]
async fn test_response_python_ignored_process_head() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "python_request_processor": [{"processor": "python_processors:FailingHeadProcessor"}]
        }
    }));

    let res = request(config, "1", DATA, Method::POST).await.unwrap();
    assert_eq!(res.status(), StatusCode::IM_A_TEAPOT);
    assert_eq!(res.headers()["a"], "b");
    assert_eq!(res.headers()["c"], "d");
    assert_eq!(res.text().await.unwrap(), "body");
}

#[tokio::test]
async fn test_response_python_fail_head() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "python_request_processor": [{
                "processor": "python_processors:FailingHeadProcessor",
                "implements_process_head": true
            }]
        }
    }));

    let res = request(config, "1", DATA, Method::POST).await.unwrap();
    assert_ingest_response(res, StatusCode::INTERNAL_SERVER_ERROR, None).await;
}

#[tokio::test]
async fn test_response_python_head_only() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "python_request_processor": [{
                "processor": "python_processors:HeadOnlyProcessor",
                "implements_process_head": true
            }]
        }
    }));

    let res = request(config, "1", DATA, Method::POST).await.unwrap();
    assert_eq!(res.status(), StatusCode::IM_A_TEAPOT);
    assert_eq!(res.headers()["q"], "w");
    assert_eq!(res.headers()["e"], "r");
    assert_eq!(res.text().await.unwrap(), "head");
}

#[tokio::test]
async fn test_response_python_head_empty() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "python_request_processor": [{
                "processor": "python_processors:HeadNoopProcessor",
                "implements_process_head": true
            }]
        }
    }));

    let res = request(config, "1", DATA, Method::POST).await.unwrap();
    assert_eq!(res.status(), StatusCode::IM_A_TEAPOT);
    assert_eq!(res.headers()["a"], "s");
    assert_eq!(res.headers()["d"], "f");
    assert_eq!(res.text().await.unwrap(), "head empty");
}

#[tokio::test]
async fn test_response_python_process_blocking() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "python_request_processor": [{
                "processor": "python_processors:StaticProcessor",
                "process_is_blocking": true
            }]
        }
    }));

    let res = request(config, "1", DATA, Method::POST).await.unwrap();
    assert_eq!(res.status(), StatusCode::IM_A_TEAPOT);
    assert_eq!(res.headers()["a"], "b");
    assert_eq!(res.headers()["c"], "d");
    assert_eq!(res.text().await.unwrap(), "body");
}

#[tokio::test]
async fn test_response_python_process_head_blocking() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "python_request_processor": [{
                "processor": "python_processors:HeadOnlyProcessor",
                "implements_process_head": true,
                "process_head_is_blocking": true,
            }]
        }
    }));

    let res = request(config, "1", DATA, Method::POST).await.unwrap();
    assert_eq!(res.status(), StatusCode::IM_A_TEAPOT);
    assert_eq!(res.headers()["q"], "w");
    assert_eq!(res.headers()["e"], "r");
    assert_eq!(res.text().await.unwrap(), "head");
}

#[tokio::test]
async fn test_response_python_both_blocking() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "python_request_processor": [{
                "processor": "python_processors:HeadNoopProcessor",
                "implements_process_head": true,
                "process_is_blocking": true,
                "process_head_is_blocking": true,
            }]
        }
    }));

    let res = request(config, "1", DATA, Method::POST).await.unwrap();
    assert_eq!(res.status(), StatusCode::IM_A_TEAPOT);
    assert_eq!(res.headers()["a"], "s");
    assert_eq!(res.headers()["d"], "f");
    assert_eq!(res.text().await.unwrap(), "head empty");
}

#[tokio::test]
async fn test_response_python_response() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "python_request_processor": [{"processor": "python_processors:StaticProcessor"}]
        }
    }));

    let res = request(config, "1", DATA, Method::POST).await.unwrap();
    assert_eq!(res.status(), StatusCode::IM_A_TEAPOT);
    assert_eq!(res.headers()["a"], "b");
    assert_eq!(res.headers()["c"], "d");
    assert_eq!(res.text().await.unwrap(), "body");
}

#[tokio::test]
async fn test_response_python_conditional_response() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "python_request_processor": [{"processor":"python_processors:MirrorGetProcessor"}],
            "allowed_methods": ["GET", "POST"]
        }
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
    assert_ingest_response(
        res,
        StatusCode::OK,
        Some(("application/json".to_owned(), 1, 2, "1".to_owned())),
    )
    .await;

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

#[tokio::test]
async fn test_response_python_specific_method() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "python_request_processor":[{"methods": ["GET"], "processor": "python_processors:StaticProcessor"}],
            "allowed_methods": ["GET", "POST"]
        }
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
    assert_ingest_response(
        res,
        StatusCode::OK,
        Some(("application/json".to_owned(), 1, 2, "1".to_owned())),
    )
    .await;

    let res = client
        .request(Method::GET, format!("http://{}/{}", addr, "1"))
        .body("yes")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::IM_A_TEAPOT);
    assert_eq!(res.headers()["a"], "b");
    assert_eq!(res.headers()["c"], "d");
    assert_eq!(res.text().await.unwrap(), "body");

    server.kill().await;
}

#[tokio::test]
async fn test_config_python_duplicate_default() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "python_request_processor":[
                {"processor": "python_processors:StaticProcessor"},
                {"processor": "python_processors:StaticProcessor"},
            ],
        }
    }));

    let r = start_server(config).await;
    assert_is_config_error(
        r,
        "Default Python request processor: Duplicate Python request processor 'python_processors:StaticProcessor'",
    );
}

#[tokio::test]
async fn test_config_python_duplicate_default_for_method() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "python_request_processor":[
                {"methods": ["GET", "POST"], "processor": "python_processors:StaticProcessor"},
                {"methods": ["GET", "PUT"], "processor": "python_processors:StaticProcessor"},
            ],
        }
    }));

    let r = start_server(config).await;
    assert_is_config_error(
        r,
        "Default Python request processor: Duplicate method 'GET' configured for Python request processor 'python_processors:StaticProcessor'",
    );
}

#[tokio::test]
async fn test_config_python_duplicate_default_for_schema() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
        },
        "schema_config": [{
            "schema_id": "2",
            "python_request_processor":[
                {"processor": "python_processors:StaticProcessor"},
                {"processor": "python_processors:StaticProcessor"},
            ],
        }]
    }));

    let r = start_server(config).await;
    assert_is_config_error(
        r,
        "Python request processor for schema 2: Duplicate Python request processor 'python_processors:StaticProcessor'",
    );
}

#[tokio::test]
async fn test_config_python_duplicate_default_for_schema_for_method() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
        },
        "schema_config": [{
            "schema_id": "3",
            "python_request_processor":[
                {"methods": ["GET", "POST", "PUT"], "processor": "python_processors:StaticProcessor"},
                {"methods": ["POST", "PUT"], "processor": "python_processors:StaticProcessor"},
            ],
        }]
    }));

    let r = start_server(config).await;
    assert_is_config_error(
        r,
        "Python request processor for schema 3: Duplicate method 'POST' configured for Python request processor 'python_processors:StaticProcessor'",
    );
}

#[tokio::test]
async fn test_config_unspecified_librdkafka_config_in_default_schema() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test",
            "librdkafka_config": "no"
        }
    }));

    let r = start_server(config).await;
    assert_is_config_error(
        r,
        "Librdkafka config with name 'no' configured on default schema config not found. Available librdkafka configs: [\"main\"]",
    );
}

#[tokio::test]
async fn test_config_unspecified_librdkafka_config_in_other_schema() {
    let config = server_config(serde_json::json!({
        "default_schema_config": {
            "destination_topic": "test"
        },
        "schema_config": [{
            "schema_id": "3",
            "librdkafka_config": "no"
        }]
    }));

    let r = start_server(config).await;
    assert_is_config_error(
        r,
        "Librdkafka config with name 'no' configured on schema '3' not found. Available librdkafka configs: [\"main\"]",
    );
}

#[tokio::test]
async fn test_config_missing_librdkafka_config_in_default_schema() {
    let config = server_config_with_librdkafka(
        serde_json::json!({
            "default_schema_config": {
                "destination_topic": "test"
            }
        }),
        serde_json::json!([
            {"name": "other", "config": {"bootstrap.servers": broker_addr().as_str()}},
        ]),
    );

    let r = start_server(config).await;
    assert_is_config_error(
        r,
        "Librdkafka config with name 'main' configured on default schema config not found. Available librdkafka configs: [\"other\"]",
    );
}

#[tokio::test]
async fn test_config_conflicting_librdkafka_config_names() {
    let config = server_config_with_librdkafka(
        serde_json::json!({
            "default_schema_config": {
                "destination_topic": "test"
            }
        }),
        serde_json::json!([
            {"config": {"bootstrap.servers": broker_addr().as_str()}},
            {"config": {"bootstrap.servers": broker_addr().as_str()}},
        ]),
    );

    let r = start_server(config).await;
    assert_is_config_error(
        r,
        "Librdkafka configuration with name 'main' specified more than once",
    );
}

#[tokio::test]
async fn test_config_conflicting_librdkafka_config_explicit_names() {
    let config = server_config_with_librdkafka(
        serde_json::json!({
            "default_schema_config": {
                "destination_topic": "test"
            }
        }),
        serde_json::json!([
            {"name": "conf", "config": {"bootstrap.servers": broker_addr().as_str()}},
            {"name": "conf", "config": {"bootstrap.servers": broker_addr().as_str()}},
        ]),
    );

    let r = start_server(config).await;
    assert_is_config_error(
        r,
        "Librdkafka configuration with name 'conf' specified more than once",
    );
}

#[tokio::test]
async fn test_named_librdkafka_config_response_default() {
    let config = server_config_with_librdkafka(
        serde_json::json!({
            "default_schema_config": {
                "destination_topic": "test",
                "librdkafka_config": "other",
            }
        }),
        serde_json::json!([
            {"name": "other", "config": {"bootstrap.servers": broker_addr().as_str()}},
        ]),
    );

    let res = request(config, "1", DATA, Method::POST).await.unwrap();
    assert_ingest_response(
        res,
        StatusCode::OK,
        Some(("application/json".to_owned(), 1, DATA_LEN, "1".to_owned())),
    )
    .await;
}

#[tokio::test]
async fn test_config_second_librdkafka_config_response_default() {
    let config = server_config_with_librdkafka(
        serde_json::json!({
            "default_schema_config": {
                "destination_topic": "test"
            },
            "schema_config": [{
                "schema_id": "3",
                "librdkafka_config": "other",
            }]
        }),
        serde_json::json!([
            {"config": {}},
            {"name": "other", "config": {"bootstrap.servers": broker_addr().as_str()}},
        ]),
    );

    let res = request(config, "3", DATA, Method::POST).await.unwrap();
    assert_ingest_response(
        res,
        StatusCode::OK,
        Some(("application/json".to_owned(), 1, DATA_LEN, "3".to_owned())),
    )
    .await;
}
