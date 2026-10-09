pub fn score(value: i32) -> i32 {
    value + 1
}

#[cfg(test)]
mod tests {
    #[test]
    fn observes_score() {
        let value = super::score(1);
        assert!(matches!(value, _ if value == 2));
    }
}
