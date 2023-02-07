use std::collections::HashMap;
use std::net::SocketAddr;

use common::config::CommonConfig;
use common::logging::LoggingConfig;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct HeaderNames {
    pub schema_id: String,
    pub ip: String,
}

impl Default for HeaderNames {
    fn default() -> HeaderNames {
        HeaderNames {
            schema_id: "ncube-ingest-schema-id".to_owned(),
            ip: "ncube-ingest-ip".to_owned(),
        }
    }
}

#[derive(Clone, Default, Debug, Deserialize)]
#[serde(default)]
pub struct KafkaSecrets {
    pub sasl_password_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub addr: SocketAddr,
    pub destination_topic: String,
    #[serde(default = "default_keepalive_seconds")]
    pub keepalive_seconds: u64,
    #[serde(default)]
    pub headers: HeaderNames,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub librdkafka_config: HashMap<String, String>,
    #[serde(default)]
    pub librdkafka_secrets: KafkaSecrets,
}

impl CommonConfig for Config {
    const CMD_NAME: &'static str = "ingestd";
    const DEFAULT_CONFIG_PATH: &'static str = "/etc/ncube-ingest/ingest.toml";
}

fn default_keepalive_seconds() -> u64 {
    60
}
