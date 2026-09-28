pub struct Config {
    pub timeout_secs: u32,
    pub retries: u32,
}

pub fn default_config() -> Config {
    let retries = 3;
    Config {
        timeout_secs: 30,
        retries,
    }
}

pub struct Fallback {
    pub retries: u32,
}

pub fn fallback() -> Fallback {
    Fallback { retries: 3 }
}
