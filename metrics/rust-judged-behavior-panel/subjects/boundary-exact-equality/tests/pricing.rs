use rust_judged_boundary_exact::discounted_total;

#[test]
fn equality_boundary_discounts() {
    assert_eq!(discounted_total(100, 100), 90);
}
