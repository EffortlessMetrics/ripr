pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {
    if amount >= discount_threshold {
        amount - 10
    } else {
        amount
    }
}

#[cfg(test)]
mod tests {
    use super::discounted_total;

    #[test]
    fn below_threshold_has_no_discount() {
        assert_eq!(discounted_total(50, 100), 50);
    }

    #[test]
    fn far_above_threshold_discounts() {
        assert_eq!(discounted_total(10_000, 100), 9_990);
    }
}
