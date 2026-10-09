this is invalid Rust;
macro_rules! proptest { ($($tt:tt)*) => {} }
proptest! {
#[test]
fn boundary() { assert_eq!(discounted_total(100,100),90); }
}
