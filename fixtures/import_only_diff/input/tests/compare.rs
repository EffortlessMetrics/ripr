use import_only_diff_fixture::compare_total;
use std::cmp::Ordering;

#[test]
fn compare_total_orders_values() {
    assert_eq!(compare_total(1, 2), Ordering::Less);
}
