pub struct Config {
    pub timeout_secs: u32,
    pub retries: u32,
}

pub fn default_config() -> Config {
    Config {
        timeout_secs: 30,
        retries: 1,
    }
}
