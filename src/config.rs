use std::net::SocketAddr;

#[derive(Clone, Debug)]
pub struct Config {
    pub addr: SocketAddr,
    pub log_json: bool,
    pub kafka: Kafka,
}

#[derive(Clone, Debug)]
pub struct Kafka {
    pub servers: String,
    pub topic: String,
    pub timeout_ms: String,
    pub acks: String,
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
