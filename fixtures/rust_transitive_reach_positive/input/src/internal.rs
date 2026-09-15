pub(crate) fn inner(a: i32, b: i32) -> i32 {
    if a > b {
        a - b
    } else {
        b
    }
}