pub struct Cache {
    built: bool,
}

impl Cache {
    pub fn new() -> Cache {
        Cache { built: false }
    }

    pub fn build(&mut self) {
        self.built = true;
    }

    pub fn is_built(&self) -> bool {
        self.built
    }
}

#[cfg(test)]
mod tests {
    use super::Cache;

    #[test]
    fn cache_builds() {
        let mut cache = Cache::new();
        cache.build();
        assert!(cache.is_built());
    }
}
