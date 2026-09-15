pub fn price(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}

fn retired_helper(value: i32) -> i32 {
    if value > 100 { price(value, 50) } else { price(value, 0) }
}
