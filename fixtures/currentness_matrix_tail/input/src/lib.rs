pub fn price(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}

pub fn ledger(amount: i32, threshold: i32) -> Option<i32> {
    if amount >= threshold {
        Some(amount - 10)
    } else {
        None
    }
}

fn retired_helper() -> i32 {
    let cost = 5;
    if cost > 0 { cost } else { 0 }
}
