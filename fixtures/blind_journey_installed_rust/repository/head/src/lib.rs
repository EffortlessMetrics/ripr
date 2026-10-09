//! Loyalty tier for one order quantity.
//!
//! The boundary is inclusive: an order of exactly 20 now qualifies for gold.

/// Return the loyalty tier for one order quantity.
pub fn tier(quantity: u64) -> &'static str {
    if quantity >= 20 {
        "gold"
    } else {
        "standard"
    }
}
