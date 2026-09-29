use proximity_name_oracle_not_credited_fixture::try_parse;

#[test]
fn rejects_at_sign() {
    assert!(try_parse("@bad").is_err());
}
