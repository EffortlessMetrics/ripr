use propagate_value_returned_fixture::Ledger;

#[test]
fn apply_returns_ok_on_success() {
    let mut ledger = Ledger::new(100);
    assert!(ledger.apply(5).is_ok());
}
