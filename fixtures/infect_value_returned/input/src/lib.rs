pub const MULTIPLIER: i32 = 9;

pub fn score(amount: i32) -> i32 {
    let result = helper(amount * MULTIPLIER);
    result + 1
}

pub fn helper(x: i32) -> i32 {
    x / 10
}
