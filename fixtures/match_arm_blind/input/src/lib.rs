pub fn reason(x: Option<i32>) -> i32 {
    match x {
        Some(v) => v + 1,
        None => 0,
    }
}
