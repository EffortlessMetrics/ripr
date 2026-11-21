pub fn price(amount: i32) -> i32 {
    if amount > 100 { amount - 10 } else { amount }
}
