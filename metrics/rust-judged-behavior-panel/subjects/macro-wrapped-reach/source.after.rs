pub fn normalize_score(value: i32) -> i32 {
    value.max(1)
}

#[macro_export]
macro_rules! assert_normalized {
    ($value:expr, $expected:expr) => {
        assert_eq!($crate::normalize_score($value), $expected)
    };
}
