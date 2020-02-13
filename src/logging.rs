use std::env;

use actix_web::{dev::ServiceRequest, http::header::HeaderMap};
use tracing::{trace_span, Span};
use tracing_subscriber::{filter::LevelFilter, EnvFilter, FmtSubscriber};
use uuid::Uuid;

use crate::error::Result;
use crate::Config;

const LOG_ENV_VAR: &str = "RUST_LOG";
const REQ_ID_HEADER: &str = "x-request-id";

pub fn init(config: &Config) -> Result<()> {
    // by default, use info level
    let mut filter = EnvFilter::new(LevelFilter::INFO.to_string());

    // overwrite with env variable
    if let Ok(s) = env::var(LOG_ENV_VAR) {
        filter = filter.add_directive(s.parse()?);
    }

    let subscriber = FmtSubscriber::builder()
        .with_max_level(LevelFilter::INFO)
        .with_env_filter(filter);

    if config.log_json {
        subscriber.json().init();
    } else {
        subscriber.init();
    }

    Ok(())
}

pub fn req_span(req: &ServiceRequest) -> Span {
    let id = req_id(req.headers());
    let path = req.path();

    trace_span!("http_request", %id, %path)
}

fn req_id(headers: &HeaderMap) -> String {
    let id_header = headers.get(REQ_ID_HEADER).map(|h| h.to_str());

    match id_header {
        Some(Ok(h)) => h.to_string(),
        _ => Uuid::new_v4().to_string(),
    }
}

pub fn ws_span() -> Span {
    let id = Uuid::new_v4().to_string();

    trace_span!("websocket_connection", %id)
}

pub fn ws_msg_span() -> Span {
    let id = Uuid::new_v4().to_string();

    trace_span!("websocket_request", %id)
}
