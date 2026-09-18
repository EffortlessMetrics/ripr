use match_arm_diagnostic_literal_no_promotion::route;

// Mentions the changed arm's pattern literal only as assertion message
// text: the call selects the sibling arm, so the message cannot observe
// the changed sensor arm.
#[test]
fn sibling_assertion_mentions_sensor_only_in_its_message() {
    assert_eq!(route("focused-test"), "proof", "sensor");
}
