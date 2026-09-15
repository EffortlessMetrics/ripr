mod internal;

macro_rules! call_inner {
    ($a:expr, $b:expr) => {
        internal::inner($a, $b)
    };
}

pub fn outer(a: i32, b: i32) -> i32 {
    call_inner!(a, b)
}
