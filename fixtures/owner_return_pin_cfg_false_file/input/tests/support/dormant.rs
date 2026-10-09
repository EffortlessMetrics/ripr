#![cfg(any())]
use owner_pin_control::weight;
#[test] fn dormant() { assert_eq!(weight(4),12); }
