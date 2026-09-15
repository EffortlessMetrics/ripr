pub fn apply_fee(amount: i32, fee: i32) -> i32 {
    if amount > fee {
        amount - fee
    } else {
        amount
    }
}

pub fn unrelated(x: i32) -> i32 {
    x + 1
}