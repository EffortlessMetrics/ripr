macro_rules! assert_result {
    ($actual:expr, $expected:expr) => {
        assert_eq!($actual, $expected);
    };
}

#[test]
fn test_inner_with_custom_assertion_macro() {
    let result = rust_macro_wrapped_assertion_limitation_fixture::inner(10, 3);
    assert_result!(result, 7);
}
