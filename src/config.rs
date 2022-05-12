use std::collections::HashMap;
use std::net::SocketAddr;

use common::config::CommonConfig;
use common::logging::LoggingConfig;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub addr: SocketAddr,
    pub keepalive_seconds: usize,
    pub topic: String,
    pub logging: LoggingConfig,
    pub librdkafka_config: HashMap<String, String>,
}

impl CommonConfig for Config {
    const ENV_CONFIG: &'static str = "NCUBE_INGEST_CONFIG";
}

impl Default for Config {
    fn default() -> Config {
        Config {
            addr: ([127, 0, 0, 1], 8088).into(),
            keepalive_seconds: 60,
            topic: "events_raw".to_owned(),
            logging: LoggingConfig::default(),
            librdkafka_config: HashMap::new(),
        }
    }
}
