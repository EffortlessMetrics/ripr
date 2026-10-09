#[test]
fn source_routed_rust_ready_types_admit_exact_indented_event() {
    let workflow = include_str!("../../.github/workflows/routed-rust.yml").replace("\r\n", "\n");
    let expected = Some(vec!["ready_for_review".to_string()]);
    assert_eq!(crate::routed_rust_pull_request_types(&workflow), expected);
    assert!(crate::routed_rust_ready_event_contract_violations(&workflow).is_empty());
    assert_eq!(
        crate::routed_rust_event_route(&workflow, "pull_request", Some("ready_for_review"), None),
        crate::RoutedRustEventRoute::LaunchFullGate,
    );
    for action in [
        "opened",
        "synchronize",
        "reopened",
        "labeled",
        "unlabeled",
        "edited",
    ] {
        assert_eq!(
            crate::routed_rust_event_route(
                &workflow,
                "pull_request",
                Some(action),
                Some("full-ci")
            ),
            crate::RoutedRustEventRoute::WorkflowNotTriggered,
            "actual Ready-only workflow must withhold {action}",
        );
    }
    let spaced = workflow.replacen("  pull_request:\n", "  pull_request: \t\n", 1);
    assert_ne!(spaced, workflow);
    assert_eq!(crate::routed_rust_pull_request_types(&spaced), expected);
    assert!(crate::routed_rust_ready_event_contract_violations(&spaced).is_empty());
}

#[test]
fn source_routed_rust_ready_types_reject_indent_and_event_substitutes() {
    let workflow = include_str!("../../.github/workflows/routed-rust.yml").replace("\r\n", "\n");
    for header in [
        " pull_request:",
        "   pull_request:",
        "    pull_request:",
        "\tpull_request:",
        "  \tpull_request:",
    ] {
        let wrong = workflow.replacen("  pull_request:\n", &format!("{header}\n"), 1);
        assert_ne!(wrong, workflow);
        assert_eq!(
            crate::routed_rust_pull_request_types(&wrong),
            None,
            "{header:?}"
        );
        assert!(
            crate::routed_rust_ready_event_contract_violations(&wrong)
                .iter()
                .any(|violation| violation.contains("inline pull_request types array")),
            "{header:?}"
        );
    }
    for types in [
        " types: [ready_for_review]",
        "  types: [ready_for_review]",
        "   types: [ready_for_review]",
        "     types: [ready_for_review]",
        "      types: [ready_for_review]",
        "    \ttypes: [ready_for_review]",
    ] {
        let wrong = workflow.replacen("    types: [ready_for_review]\n", &format!("{types}\n"), 1);
        assert_ne!(wrong, workflow);
        assert_eq!(
            crate::routed_rust_pull_request_types(&wrong),
            None,
            "{types:?}"
        );
        assert!(
            crate::routed_rust_ready_event_contract_violations(&wrong)
                .iter()
                .any(|violation| violation.contains("inline pull_request types array")),
            "{types:?}"
        );
    }
    for (case, wrong) in [
        (
            "other event",
            "on:\n  pull_request:\n  repository_dispatch:\n    types: [ready_for_review]\n",
        ),
        (
            "wrong section",
            "env:\n  pull_request:\n    types: [ready_for_review]\non:\n  push:\n",
        ),
        (
            "comment",
            "on:\n  pull_request:\n    # types: [ready_for_review]\n",
        ),
        (
            "nested field",
            "on:\n  pull_request:\n    details:\n      types: [ready_for_review]\n",
        ),
        (
            "block scalar",
            "on:\n  pull_request:\n    types: |\n      [ready_for_review]\n",
        ),
    ] {
        assert_eq!(crate::routed_rust_pull_request_types(wrong), None, "{case}");
        assert!(
            crate::routed_rust_ready_event_contract_violations(wrong)
                .iter()
                .any(|violation| violation.contains("inline pull_request types array")),
            "{case}"
        );
    }
}
