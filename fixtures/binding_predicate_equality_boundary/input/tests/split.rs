use bp::split_after;

#[test]
fn present_delimiter_returns_prefix() {
    assert_eq!(split_after("a.b", '.'), "a");
}

#[test]
fn absent_delimiter_boundary_returns_head() {
    assert_eq!(split_after("ab", 'x'), "a");
}
