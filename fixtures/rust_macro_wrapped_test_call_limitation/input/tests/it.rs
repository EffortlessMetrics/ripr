#[test]
fn test_macro_returns_difference() {
    let result = rust_macro_wrapped_test_call_limitation_fixture::call_inner!(10, 3);
    assert_eq!(result, 7);
}
