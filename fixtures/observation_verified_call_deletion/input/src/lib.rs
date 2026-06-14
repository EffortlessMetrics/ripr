pub struct Cache {
    pub inserted: Vec<String>,
}

impl Cache {
    pub fn new() -> Self {
        Cache { inserted: Vec::new() }
    }
    pub fn insert(&mut self, key: &str, value: i32) {
        self.inserted.push(format!("{key}={value}"));
    }
}

pub fn store_result(cache: &mut Cache, result: i32) {
    cache.insert("result_key", result);
}
