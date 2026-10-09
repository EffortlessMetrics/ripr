#[test]
fn routed_implementation_condition_checks_inline_and_folded_operators() {
    for condition in [
        "    if: always() && true\n",
        "    if: >-\n      Always () &&\n      true\n",
    ] {
        let workflow = format!("jobs:\n  rust-github:\n{condition}    runs-on: ubuntu-latest\n");
        assert!(crate::routed_rust_job_condition_has_always(
            &workflow,
            "rust-github"
        ));
    }
}

#[test]
fn routed_implementation_condition_ignores_foreign_fields_and_cleanup() {
    for workflow in [
        "env:\n  rust-github:\n    if: always()\njobs:\n  rust-github:\n    if: !cancelled()\n",
        "jobs:\n  rust-github:\n    if: >-\n      !cancelled()\n    # always() is a comment outside the condition\n    with:\n      message: always()\n",
        "jobs:\n  rust-github:\n    if: !cancelled()\n    steps:\n      - name: cleanup\n        if: always()\n        run: true\n",
        "jobs:\n  other:\n    if: always()\n  rust-github:\n    if: !cancelled()\n",
    ] {
        assert!(!crate::routed_rust_job_condition_has_always(
            workflow,
            "rust-github"
        ));
    }
}
