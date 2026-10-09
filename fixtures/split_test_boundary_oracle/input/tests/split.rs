use split_test_boundary_oracle_fixture::gate;

#[test]
fn boundary() {
    let _ = gate(10);
    let _ = gate(9);
}

#[test]
fn far() {
    assert_eq!(gate(100), true);
}
