pub fn triage(input: &str) -> bool {
    !input.is_empty()
}

#[cfg(test)]
mod tests {
    const FIXTURE_JSON: &str = r#"{
        "producer_id": "abd",
        "admission": 1
    }"#;

    #[test]
    fn parses_fixture() {
        assert!(super::triage(FIXTURE_JSON));
    }
}
