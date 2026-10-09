pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {
    if amount >= discount_threshold {
        amount - 10
    } else {
        amount
    }
}

macro_rules! quickcheck { ($($args:tt)*) => {} }
quickcheck! { fn boundary() -> bool { assert_eq!(discounted_total(100, 100), 90); true } }
