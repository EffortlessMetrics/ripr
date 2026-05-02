use boundary_gap::discounted_total;

#[test]
fn below_threshold_has_no_discount() {
    assert_eq!(discounted_total(90, 100), 90);
}

#[test]
fn at_threshold_discounts() {
    assert_eq!(discounted_total(100, 100), 90);
}

#[test]
fn far_above_threshold_discounts() {
    assert_eq!(discounted_total(200, 100), 180);
}
