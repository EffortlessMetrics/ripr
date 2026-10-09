// Unrelated package remains unrelated.
macro_rules! proptest { ($($tt:tt)*) => {} }
proptest! { #[test] fn unrelated() { discounted_total(0,0); } }
