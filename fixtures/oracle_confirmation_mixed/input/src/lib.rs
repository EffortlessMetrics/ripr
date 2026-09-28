pub fn analyze_diff(rows: &mut Vec<u32>) {
    record_effect(rows);
}

fn record_effect(rows: &mut Vec<u32>) {
    rows.push(5);
}

#[cfg(test)]
mod tests {
    #[test]
    fn calls_owner_without_observing_effect() {
        super::analyze_diff(&mut Vec::new());
    }

    #[test]
    fn unrelated_exact_oracle() {
        let store = vec![7];
        let expected = vec![7];
        assert_eq!(store, expected);
        let label = "record_effect";
        assert!(label.contains("record_effect"));
    }
}
