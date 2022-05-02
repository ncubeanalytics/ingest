use std::net::SocketAddr;

use serde::Deserialize;

use common::config::CommonConfig;
use common::logging::LoggingConfig;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub addr: SocketAddr,
    pub logging: LoggingConfig,
    pub kafka: Kafka,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Kafka {
    pub servers: String,
    pub topic_prefix: String,
    pub timeout_ms: String,
    pub acks: String,
}

impl CommonConfig for Config {
    const ENV_CONFIG: &'static str = "PHAEDRA_INGEST_CONFIG";
}

impl Default for Config {
    fn default() -> Config {
        Config {
            addr: ([127, 0, 0, 1], 8088).into(),
            logging: LoggingConfig::default(),
            kafka: Kafka::default(),
        }
    }
}

impl Default for Kafka {
    fn default() -> Kafka {
        Kafka {
            servers: "127.0.0.1:19092".to_string(),
            topic_prefix: "in_".to_string(),
            timeout_ms: "5000".to_string(),
            acks: "all".to_string(),
        }
    }
}
