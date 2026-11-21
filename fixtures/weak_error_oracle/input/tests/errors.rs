use weak_error_oracle_fixture::authenticate;

#[test]
fn empty_token_is_rejected() {
    assert!(authenticate("").is_err());
}
