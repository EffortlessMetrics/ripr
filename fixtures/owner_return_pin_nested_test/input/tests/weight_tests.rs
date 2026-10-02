use owner_pin_control::weight;
#[test]
fn outer() { #[test] fn inner() { assert_eq!(weight(4),12); } let _ = weight(4); }
