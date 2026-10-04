use proximity_name_oracle_not_credited_fixture::ParseError;

#[test]
fn try_parse_variant_is_distinct() {
    let variant = ParseError::MalformedSource;
    assert_eq!(variant, ParseError::MalformedSource);
}
