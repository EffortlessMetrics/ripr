pub struct Cache;

impl Cache {
    pub fn insert(&self, key: &str, value: i32) {
        let _ = (key, value);
    }
}

pub fn store_result(cache: &Cache, result: i32) {
    cache.insert("result_key", result);
}
