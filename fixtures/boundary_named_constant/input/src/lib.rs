mod config;
use config::FREE_SHIPPING;

/// Orders at or above this amount (in cents) get a discount.
pub const DISCOUNT_THRESHOLD: u64 = 10_000;

/// Orders with at least this many items get the bulk rate.
pub const BULK_ITEMS: u32 = 5 * 2;

/// Returns the total after applying the volume discount.
pub fn discounted_total(amount: u64) -> u64 {
    if amount >= DISCOUNT_THRESHOLD {
        amount - amount / 10
    } else {
        amount
    }
}

/// Per-item price in cents.
pub fn bulk_rate(items: u32) -> u32 {
    if items >= BULK_ITEMS {
        90
    } else {
        100
    }
}

/// Shipping cost in cents.
pub fn shipping(amount: u64) -> u64 {
    if amount >= FREE_SHIPPING {
        0
    } else {
        500
    }
}
