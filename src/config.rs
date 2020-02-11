use std::env;
use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::Result;

const ENV_CONFIG: &str = "PHAEDRA_INGEST_CONFIG";

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

impl Config {
    /// Looks for a TOML config file at a path defined by the
    /// PHAEDRA_INGEST_CONFIG env var and parses it.
    /// If the env var is not set, default config will be returned.
    pub fn load() -> Result<Config> {
        if let Some(path) = Config::env_file_path() {
            Config::load_from_toml_file(path)
        } else {
            Ok(Config::default())
        }
    }

    fn env_file_path() -> Option<PathBuf> {
        env::var(ENV_CONFIG).map(|p| p.into()).ok()
    }

    fn load_from_toml_file<P>(path: P) -> Result<Config>
    where
        P: AsRef<Path>,
    {
        let mut file = File::open(&path)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;

        Ok(toml::from_slice(&buf)?)
    }
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
            servers: "127.0.0.1:9092".to_string(),
            topic: "events".to_string(),
            timeout_ms: "5000".to_string(),
            acks: "all".to_string(),
        }
    }
}
