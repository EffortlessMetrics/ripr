use qb::quote_body;

#[test]
fn empty_body_equality_case() {
    assert_eq!(quote_body("[]", '[', ']'), Some(""));
}

#[test]
fn nonempty_body() {
    assert_eq!(quote_body("[ab]", '[', ']'), Some("ab"));
}

#[test]
fn missing_close_returns_none() {
    assert_eq!(quote_body("[ab", '[', ']'), None);
}
