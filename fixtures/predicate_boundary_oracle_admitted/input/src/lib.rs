pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {
    if amount >= discount_threshold {
        amount - 10
    } else {
        amount
    }
}

#[cfg(test)]
mod tests {
use super::*;
#[test]
fn boundary() { assert_eq!(discounted_total(100, 100), 90); }
#[test]
fn far() { assert_eq!(discounted_total(101, 100), 91); }
}
