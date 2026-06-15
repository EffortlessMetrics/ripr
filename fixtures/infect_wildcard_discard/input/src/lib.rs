pub fn process(amount: i32) -> i32 {
    let _ = compute_fee(amount * 9);
    amount
}

fn compute_fee(x: i32) -> i32 {
    x / 10
}
