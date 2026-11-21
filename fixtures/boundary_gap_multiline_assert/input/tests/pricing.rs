use boundary_gap_multiline_assert_fixture::discounted_total;

#[test]
fn below_threshold_has_no_discount() {
    assert_eq!(
        discounted_total(50, 100),
        50,
    );
}

#[test]
fn far_above_threshold_discounts() {
    assert_eq!(
        discounted_total(10_000, 100),
        9_990,
    );
}
