// Keep discounts strict at the configured threshold.
pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {
    if amount > discount_threshold {
        amount - 10
    } else {
        amount
    }
}
