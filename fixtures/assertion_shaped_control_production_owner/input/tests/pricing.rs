use assertion_shaped_control_production_owner::discounted_total;

fn boundary_amount() -> i32 {
    100
}

#[test]
fn discounted_total_at_boundary() {
    assert_eq!(discounted_total(boundary_amount()), 90);
}
