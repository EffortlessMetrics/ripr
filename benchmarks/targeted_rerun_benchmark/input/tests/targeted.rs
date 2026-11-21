use targeted_rerun_benchmark::target;

#[test]
fn target_boundary_is_observed() {
    assert!(target(0));
}
