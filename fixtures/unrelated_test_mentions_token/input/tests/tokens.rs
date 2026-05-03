use unrelated_test_mentions_token_fixture::token_label;

#[test]
fn token_label_includes_token_text() {
    assert_eq!(token_label("discount_threshold"), "token:discount_threshold");
}
