use shop::{earns_gift, shipping};

#[test]
fn big_order_ships_free() {
    assert_eq!(shipping(20), 0);
}

#[test]
fn small_order_pays_shipping() {
    assert_eq!(shipping(2), 800);
}

#[test]
fn five_items_earn_a_gift() {
    assert_eq!(earns_gift(5), true);
    assert_eq!(earns_gift(4), false);
}
