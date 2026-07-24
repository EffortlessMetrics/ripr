mod internal;

pub fn outer(a: i32, b: i32) -> i32 {
    internal::inner(a, b)
}