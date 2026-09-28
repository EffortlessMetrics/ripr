use wrapper_seam_callee_attribution_fixture::try_parse_summary;

#[test]
fn observes_callee_outcome() {
    let parsed = try_parse_summary("abc");
    assert_eq!(parsed.map(|n| n), Ok(3));
}

// #3728: a same-named binding shadows only uses at or after its own line,
// so the captured call on the first line is a real seam-callee call and the
// `seam_callee_call` relation survives the positional defeat.
#[test]
fn calls_callee_before_shadow_binding() {
    let parsed = try_parse_summary("abc");
    assert_eq!(parsed.map(|n| n), Ok(3));
    let try_parse_summary = |raw: &str| Ok(raw.len());
    let ignored = try_parse_summary("later");
    assert_eq!(ignored, Ok(5));
}
