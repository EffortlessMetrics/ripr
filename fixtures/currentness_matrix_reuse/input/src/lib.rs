pub fn price(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}

fn retired_helper(flag: bool) -> bool {
    let margin = 1;
    if margin > 0 { flag } else { !flag }
}
