#[derive(Debug)]
pub struct Config {
    pub addr: String,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            addr: "127.0.0.1:8088".to_string(),
        }
    }
}
