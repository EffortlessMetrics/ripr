use infect_wildcard_discard_fixture::process;

#[test]
fn process_returns_amount_unchanged() {
    assert_eq!(process(42), 42);
}
