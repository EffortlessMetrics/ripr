pub mod internal;

#[macro_export]
macro_rules! call_inner {
    ($a:expr, $b:expr) => {
        $crate::internal::inner($a, $b)
    };
}
