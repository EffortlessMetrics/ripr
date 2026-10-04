//! Loyalty tier for one order quantity.
//!
//! The boundary is exclusive: an order of exactly 20 stays standard.

/// Return the loyalty tier for one order quantity.
pub fn tier(quantity: u64) -> &'static str {
    if quantity > 20 {
        "gold"
    } else {
        "standard"
    }
}
