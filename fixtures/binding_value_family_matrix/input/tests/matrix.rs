use bv::{body_of, bump, gate, head, tail, within};

#[test]
fn strip_prefix_boundary_is_observed() {
    assert_eq!(body_of("pre-fix"), "matched");
}

#[test]
fn strip_prefix_fallback_is_observed() {
    assert_eq!(body_of("raw"), "none");
}

#[test]
fn starts_with_boundary_is_observed() {
    assert!(gate("pre-fix", true));
}

#[test]
fn len_boundary_is_observed() {
    assert!(within("zz", "abc"));
}

#[test]
fn checked_add_boundary_is_observed() {
    assert!(bump(3));
}

#[test]
fn head_some_arm_boundary_is_observed() {
    assert!(head("xray"));
}

#[test]
fn head_input_arm_stays_false() {
    assert!(!head("fix"));
}

#[test]
fn head_fallback_is_observed() {
    assert!(head(""));
}

#[test]
fn tail_some_arm_boundary_is_observed() {
    assert!(tail("box"));
}

#[test]
fn tail_fallback_is_observed() {
    assert!(tail(""));
}
