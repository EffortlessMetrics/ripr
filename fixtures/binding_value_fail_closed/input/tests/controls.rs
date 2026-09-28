use fc::{via_dynamic_needle, via_map_or_else, via_shifted_closure};

#[test]
fn map_or_else_boundary_stays_unresolved() {
    assert_eq!(via_map_or_else("ab", 'x'), "a");
}

#[test]
fn shifted_closure_boundary_stays_unresolved() {
    assert_eq!(via_shifted_closure("ax", 'x'), "ax");
}

#[test]
fn dynamic_needle_boundary_stays_unresolved() {
    assert!(via_dynamic_needle("ax", 'x'));
}
