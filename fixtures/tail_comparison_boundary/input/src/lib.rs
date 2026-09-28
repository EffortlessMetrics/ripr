/// True when an order ships free.
pub fn ships_free(items: u32) -> bool {
    items >= 10
}

/// Shipping in cents for an order.
pub fn shipping(items: u32) -> u32 {
    if ships_free(items) {
        0
    } else {
        800
    }
}

/// True when a basket earns a gift.
pub fn earns_gift(items: u32) -> bool {
    items >= 5
}
