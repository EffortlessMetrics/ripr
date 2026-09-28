use parcels::{bulk_rate, discounted_total, shipping};

#[test]
fn discount_boundary() {
    assert_eq!(discounted_total(5_000), 5_000);
    assert_eq!(discounted_total(10_000), 9_000);
    assert_eq!(discounted_total(20_000), 18_000);
}

#[test]
fn bulk_rate_boundary() {
    assert_eq!(bulk_rate(3), 100);
    assert_eq!(bulk_rate(parcels::BULK_ITEMS), 90);
}

#[test]
fn shipping_boundary() {
    assert_eq!(shipping(1_000), 500);
    assert_eq!(shipping(5_000), 0);
}
