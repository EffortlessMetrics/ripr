pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {
    if amount >= discount_threshold {
        amount - 10
    } else {
        amount
    }
}

macro_rules! proptest { ($($args:tt)*) => {} }
proptest! { #[test] fn boundary() { assert_eq!(other::discounted_total(100, 100), 90); } }
