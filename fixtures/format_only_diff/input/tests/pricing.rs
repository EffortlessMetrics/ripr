use format_only_diff_fixture::discounted_total;

#[test]
fn above_threshold_discounts() {
    assert_eq!(discounted_total(150, 100), 140);
}
