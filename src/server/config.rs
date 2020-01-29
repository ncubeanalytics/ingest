use std::net::SocketAddr;

#[derive(Debug)]
pub struct Config {
    pub addr: SocketAddr,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            addr: ([127, 0, 0, 1], 8088).into(),
        }
    }
}
