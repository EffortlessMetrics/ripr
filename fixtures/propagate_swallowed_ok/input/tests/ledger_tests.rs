use propagate_swallowed_ok_fixture::Ledger;

#[test]
fn apply_changes_balance() {
    let mut ledger = Ledger::new(100);
    ledger.apply(5);
    assert_eq!(ledger.balance(), 145);
}
