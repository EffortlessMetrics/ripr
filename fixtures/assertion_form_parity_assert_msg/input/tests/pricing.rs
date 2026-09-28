use parity_assert_msg::discounted_total;

#[test]
fn boundary_matches_expected() {
    let actual = discounted_total(100, 100);
    let expected = 90;
    assert!(actual == expected, "actual={actual:?}");
}
