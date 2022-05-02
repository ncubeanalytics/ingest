use std::collections::HashMap;
use std::net::SocketAddr;

use common::config::CommonConfig;
use common::logging::LoggingConfig;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub addr: SocketAddr,
    pub topic_prefix: String,
    pub logging: LoggingConfig,
    pub librdkafka_config: HashMap<String, String>,
}

impl CommonConfig for Config {
    const ENV_CONFIG: &'static str = "PHAEDRA_INGEST_CONFIG";
}

impl Default for Config {
    fn default() -> Config {
        Config {
            addr: ([127, 0, 0, 1], 8088).into(),
            topic_prefix: "in_".to_string(),
            logging: LoggingConfig::default(),
            librdkafka_config: HashMap::new(),
        }
    }
}
