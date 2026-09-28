use match_arm_shared_result_no_promotion::route;

// Selects only the sibling arm: the shared result literal never observes
// the changed sensor arm, so this oracle cannot discriminate the sensor
// result change from `sensor-v1` to `sensor-v2`.
#[test]
fn sibling_proof_arm_is_observed() {
    assert_eq!(route("focused-test"), "sensor-v2");
}
