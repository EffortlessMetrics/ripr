use rust_binding_status_wrong_owner_observer_fixture::{
    AttemptStatus, AttemptVerdict, ArtifactRecord, artifact_reaches_terminal,
};

#[test]
fn artifact_terminal_status_is_observed() {
    // Never reaches the changed sink. Strong oracle on the unrelated owner
    // plus nearby-value tokens the sink also names.
    let record = ArtifactRecord {
        status: "terminal".to_string(),
        verdict_note: "blocked".to_string(),
    };
    assert!(artifact_reaches_terminal(&record));
    assert_eq!(record.verdict_note, "blocked");
    assert_eq!(format!("{:?}", AttemptStatus::Ready), "Ready");
    let verdict = AttemptVerdict {
        status: AttemptStatus::Ready,
        violations: vec![],
        display_paths: vec!["out/receipt.json".to_string()],
    };
    assert!(format!("{:?}", verdict.status).contains("Ready"));
}
