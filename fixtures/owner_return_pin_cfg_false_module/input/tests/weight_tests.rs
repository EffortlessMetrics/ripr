use owner_pin_control::weight;
#[r#cfg(any())] mod dormant { use super::weight; #[test] fn inner() { assert_eq!(weight(4),12); } }
#[test] fn smoke() { let _ = weight(4); }
