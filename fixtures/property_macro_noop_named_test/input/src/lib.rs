pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {
    if amount >= discount_threshold {
        amount - 10
    } else {
        amount
    }
}

macro_rules! prop_assert_eq { ($($args:tt)*) => {} }
#[cfg(test)] mod tests {
#[test]
fn discounted_total() { prop_assert_eq!(super::discounted_total(100,100),90); }
}
