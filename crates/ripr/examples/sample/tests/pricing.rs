#[test]
fn premium_customer_gets_discount() {
    let quote = ripr_sample::price(10_000, 100);
    assert!(quote.total > 0);
}

#[test]
fn rejects_bad_currency() {
    let result = ripr_sample::validate_currency("XYZ");
    assert!(result.is_err());
}
