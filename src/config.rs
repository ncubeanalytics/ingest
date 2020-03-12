use std::net::SocketAddr;

use serde::Deserialize;

use common::config::CommonConfig;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub addr: SocketAddr,
    pub log_json: bool,
    pub kafka: Kafka,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Kafka {
    pub servers: String,
    pub topic: String,
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
            log_json: false,
            kafka: Kafka::default(),
        }
    }
}

impl Default for Kafka {
    fn default() -> Kafka {
        Kafka {
            servers: "127.0.0.1:19092".to_string(),
            topic: "events".to_string(),
            timeout_ms: "5000".to_string(),
            acks: "all".to_string(),
        }
    }
}
