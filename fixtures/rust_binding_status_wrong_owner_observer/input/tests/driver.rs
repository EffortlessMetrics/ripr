use rust_binding_status_wrong_owner_observer_fixture::{
    AttemptStatus, AttemptVerdict, receipt_binding_matches,
};

#[test]
fn binding_inputs_reach_the_changed_comparison() {
    // Establishes reach and boundary-shaped inputs only: matching verdicts
    // must bind, a status flip must refuse. No oracle observes the decision
    // beyond executing it (smoke execution, not discrimination).
    let clean = AttemptVerdict {
        status: AttemptStatus::Ready,
        violations: vec![],
        display_paths: vec![],
    };
    let drifted = AttemptVerdict {
        status: AttemptStatus::Tampered,
        violations: vec![],
        display_paths: vec![],
    };
    let _ = receipt_binding_matches(&clean, &clean);
    let _ = receipt_binding_matches(&clean, &drifted);
}
