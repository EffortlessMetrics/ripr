use tier_fixture::tier;

#[test]
fn below_boundary_stays_standard() {
    assert_eq!(tier(19), "standard");
}

#[test]
fn above_boundary_is_gold() {
    assert_eq!(tier(21), "gold");
}
