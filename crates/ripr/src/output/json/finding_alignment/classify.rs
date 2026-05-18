use crate::domain::{Finding, OracleKind, OracleStrength, RelatedTest};

use super::model::*;

pub(super) fn classify_presentation_text(
    constant_name: &str,
    raw_findings: &[&Finding],
) -> PresentationTextClassification {
    let source_file = raw_findings
        .first()
        .map(|finding| finding.probe.location.file.display().to_string())
        .unwrap_or_default();

    if is_internal_only_text(constant_name, &source_file) {
        return internal_only_classification();
    }

    if let Some(sink) = visible_sink_for(constant_name, &source_file) {
        if let Some((observer, related_test)) = observer_for_findings(raw_findings) {
            return observed_classification(sink, observer, related_test);
        }

        return actionable_output_classification(sink, constant_name);
    }

    visibility_unknown_classification()
}

fn visibility_unknown_classification() -> PresentationTextClassification {
    PresentationTextClassification {
        canonical_item_kind: "limitation".to_string(),
        gap_state: "static_limitation".to_string(),
        actionability: "inspect_visibility".to_string(),
        why: "Changed presentation text could not be traced to or away from a user-visible output sink.".to_string(),
        recommended_repair:
            "Trace the string constant to a rendered output path or confirm it is internal-only."
                .to_string(),
        related_test: None,
        static_limitations: vec![FindingAlignmentStaticLimitation {
            category: VISIBILITY_UNKNOWN_CATEGORY.to_string(),
            repair_route: VISIBILITY_UNKNOWN_REPAIR_ROUTE.to_string(),
            user_actionability: "unknown_until_visibility_known".to_string(),
        }],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Visibility-unknown presentation text is benchmark-pinned; no user test debt is claimed without an output sink.".to_string(),
            ],
        },
        visibility: "unknown".to_string(),
        observer: "unknown".to_string(),
        presentation_actionability: "static_limitation_visibility_unknown".to_string(),
        recommended_observer: "unknown".to_string(),
        repair_kind: "inspect_visibility".to_string(),
        target_test_type: "unknown".to_string(),
        suggested_assertion:
            "Trace the constant to a supported output sink before adding or updating tests."
                .to_string(),
    }
}

fn internal_only_classification() -> PresentationTextClassification {
    PresentationTextClassification {
        canonical_item_kind: "no_action".to_string(),
        gap_state: "internal_only".to_string(),
        actionability: "no_action".to_string(),
        why: "Changed label is confined to an internal proof, policy, or config-only path in fixture-backed scope.".to_string(),
        recommended_repair: "No user test action.".to_string(),
        related_test: None,
        static_limitations: vec![],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Internal-only presentation labels are benchmark-pinned as no-action evidence, not user-visible output debt.".to_string(),
            ],
        },
        visibility: "internal_only".to_string(),
        observer: "none".to_string(),
        presentation_actionability: "no_action_internal".to_string(),
        recommended_observer: "none".to_string(),
        repair_kind: "no_action".to_string(),
        target_test_type: "none".to_string(),
        suggested_assertion: "No user-facing assertion is recommended for this internal label."
            .to_string(),
    }
}

fn actionable_output_classification(
    sink: PresentationTextSink,
    constant_name: &str,
) -> PresentationTextClassification {
    let recommended_repair = format!(
        "Add or update a {} for {constant_name}.",
        sink.repair_target,
    );
    let suggested_assertion = format!(
        "Assert {} includes the {constant_name} text.",
        sink.assertion_subject,
    );

    PresentationTextClassification {
        canonical_item_kind: "gap".to_string(),
        gap_state: "actionable".to_string(),
        actionability: "add_output_observer".to_string(),
        why: format!(
            "Changed text flows to {} and no supported output observer is found.",
            sink.description
        ),
        recommended_repair,
        related_test: None,
        static_limitations: vec![],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Visible unobserved presentation text is actionable only for supported sink patterns.".to_string(),
            ],
        },
        visibility: "user_visible".to_string(),
        observer: "none".to_string(),
        presentation_actionability: "add_output_observer".to_string(),
        recommended_observer: sink.recommended_observer.to_string(),
        repair_kind: "output_observer".to_string(),
        target_test_type: sink.target_test_type.to_string(),
        suggested_assertion,
    }
}

fn observed_classification(
    sink: PresentationTextSink,
    observer: &'static str,
    related_test: FindingAlignmentRelatedTest,
) -> PresentationTextClassification {
    PresentationTextClassification {
        canonical_item_kind: "observed".to_string(),
        gap_state: "already_observed".to_string(),
        actionability: "already_observed".to_string(),
        why: format!(
            "Changed text flows to {} and a supported {observer} observer covers it.",
            sink.description
        ),
        recommended_repair: "No new RIPR action.".to_string(),
        related_test: Some(related_test),
        static_limitations: vec![],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Observed presentation text stays visible as evidence without becoming a user repair.".to_string(),
            ],
        },
        visibility: "user_visible".to_string(),
        observer: observer.to_string(),
        presentation_actionability: "already_observed".to_string(),
        recommended_observer: observer.to_string(),
        repair_kind: "no_action".to_string(),
        target_test_type: observer.to_string(),
        suggested_assertion: format!(
            "Existing {observer} observer already covers the {}.",
            sink.description
        ),
    }
}

pub(super) fn classify_config_policy_constant(
    constant_name: &str,
    raw_findings: &[&Finding],
) -> ConfigPolicyClassification {
    let source_file = raw_findings
        .first()
        .map(|finding| finding.probe.location.file.display().to_string())
        .unwrap_or_default();

    if is_internal_only_config_policy(constant_name, &source_file) {
        return internal_config_policy_classification();
    }

    if is_opaque_config_policy_lookup(constant_name, &source_file) {
        return config_policy_limitation_classification(
            OPAQUE_CONFIG_LOOKUP_CATEGORY,
            OPAQUE_CONFIG_LOOKUP_REPAIR_ROUTE,
            "Changed config or policy constant is routed through an unsupported lookup helper.",
            "Add fixture-backed support for this lookup shape before claiming visibility or observer debt.",
            "unknown_until_lookup_supported",
        );
    }

    if let Some(sink) = config_policy_visible_sink_for(constant_name, &source_file) {
        if let Some((observer, related_test)) = observer_for_findings(raw_findings) {
            return observed_config_policy_classification(sink, observer, related_test);
        }

        return actionable_config_policy_classification(sink, constant_name);
    }

    config_policy_limitation_classification(
        CONFIG_POLICY_FLOW_UNKNOWN_CATEGORY,
        CONFIG_POLICY_FLOW_UNKNOWN_REPAIR_ROUTE,
        "Changed config or policy constant could not be traced to or away from a supported output, schema, validation, or behavior sink.",
        "Trace the constant to a supported output, schema, validation, or behavior sink before claiming user test debt.",
        "unknown_until_config_flow_known",
    )
}

fn internal_config_policy_classification() -> ConfigPolicyClassification {
    ConfigPolicyClassification {
        canonical_item_kind: "no_action".to_string(),
        gap_state: "internal_only".to_string(),
        actionability: "no_action_internal".to_string(),
        why: "Changed constant is confined to internal allowlist, proof, or policy metadata in fixture-backed scope.".to_string(),
        recommended_repair: "No user test action.".to_string(),
        related_test: None,
        static_limitations: vec![],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Internal config or policy metadata is benchmark-pinned as no-action evidence, not user test debt.".to_string(),
            ],
        },
        role: "internal_policy_metadata".to_string(),
        visibility: "internal_only".to_string(),
        observer: "none".to_string(),
        config_actionability: "no_action_internal".to_string(),
        repair_kind: "no_action".to_string(),
        target_test_type: "none".to_string(),
        suggested_assertion:
            "No user-facing assertion is recommended for this internal policy constant."
                .to_string(),
    }
}

fn actionable_config_policy_classification(
    sink: ConfigPolicySink,
    constant_name: &str,
) -> ConfigPolicyClassification {
    let recommended_repair = format!(
        "Add or update a {} for {constant_name}.",
        sink.repair_target
    );
    let suggested_assertion = format!(
        "Assert {} includes the {constant_name} value or selected behavior.",
        sink.assertion_subject
    );

    ConfigPolicyClassification {
        canonical_item_kind: "gap".to_string(),
        gap_state: "actionable".to_string(),
        actionability: sink.actionability.to_string(),
        why: format!(
            "Changed config or policy constant flows to {} and no supported observer or discriminator is found.",
            sink.description
        ),
        recommended_repair,
        related_test: None,
        static_limitations: vec![],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Visible unobserved config or policy constants are actionable only for supported sink patterns.".to_string(),
            ],
        },
        role: sink.role.to_string(),
        visibility: "user_visible".to_string(),
        observer: "none".to_string(),
        config_actionability: sink.actionability.to_string(),
        repair_kind: sink.repair_kind.to_string(),
        target_test_type: sink.target_test_type.to_string(),
        suggested_assertion,
    }
}

fn observed_config_policy_classification(
    sink: ConfigPolicySink,
    observer: &'static str,
    related_test: FindingAlignmentRelatedTest,
) -> ConfigPolicyClassification {
    ConfigPolicyClassification {
        canonical_item_kind: "observed".to_string(),
        gap_state: "already_observed".to_string(),
        actionability: "already_observed".to_string(),
        why: format!(
            "Changed config or policy constant flows to {} and a supported {observer} observer covers it.",
            sink.description
        ),
        recommended_repair: "No new RIPR action.".to_string(),
        related_test: Some(related_test),
        static_limitations: vec![],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Observed config or policy constants stay visible as evidence without becoming user repair work.".to_string(),
            ],
        },
        role: sink.role.to_string(),
        visibility: "user_visible".to_string(),
        observer: observer.to_string(),
        config_actionability: "already_observed".to_string(),
        repair_kind: "no_action".to_string(),
        target_test_type: observer.to_string(),
        suggested_assertion: format!(
            "Existing {observer} observer already covers the {}.",
            sink.description
        ),
    }
}

fn config_policy_limitation_classification(
    category: &str,
    repair_route: &str,
    why: &str,
    recommended_repair: &str,
    user_actionability: &str,
) -> ConfigPolicyClassification {
    ConfigPolicyClassification {
        canonical_item_kind: "limitation".to_string(),
        gap_state: "static_limitation".to_string(),
        actionability: "inspect_config_flow".to_string(),
        why: why.to_string(),
        recommended_repair: recommended_repair.to_string(),
        related_test: None,
        static_limitations: vec![FindingAlignmentStaticLimitation {
            category: category.to_string(),
            repair_route: repair_route.to_string(),
            user_actionability: user_actionability.to_string(),
        }],
        confidence: FindingAlignmentConfidence {
            basis: "fixture_backed".to_string(),
            notes: vec![
                "Config/policy unknowns are benchmark-pinned as named limitations; no user test debt is claimed without supported sink evidence.".to_string(),
            ],
        },
        role: "unknown".to_string(),
        visibility: "unknown".to_string(),
        observer: "unknown".to_string(),
        config_actionability: "inspect_config_flow".to_string(),
        repair_kind: "inspect_config_flow".to_string(),
        target_test_type: "unknown".to_string(),
        suggested_assertion:
            "Trace the constant to a supported output or behavior sink before adding tests."
                .to_string(),
    }
}

fn visible_sink_for(constant_name: &str, file: &str) -> Option<PresentationTextSink> {
    let file = normalize_token_text(file);

    if name_has_token(constant_name, "HELP")
        && (file.contains("help") || file.contains("cli") || file.contains("command"))
    {
        return Some(PresentationTextSink {
            recommended_observer: "cli_help_output",
            repair_target: "help-output snapshot assertion",
            description: "CLI help output",
            target_test_type: "help_output_snapshot",
            assertion_subject: "CLI help output",
        });
    }

    if name_has_token(constant_name, "REPORT") && file.contains("report") {
        return Some(PresentationTextSink {
            recommended_observer: "report_render",
            repair_target: "report-render or golden-output test",
            description: "rendered report output",
            target_test_type: "report_render_or_golden",
            assertion_subject: "the rendered report output",
        });
    }

    if (name_has_token(constant_name, "TABLE") || name_has_token(constant_name, "DISPLAY"))
        && (file.contains("table") || file.contains("display") || file.contains("render"))
    {
        return Some(PresentationTextSink {
            recommended_observer: "table_render",
            repair_target: "table-render or golden-output test",
            description: "rendered table output",
            target_test_type: "table_render_or_golden",
            assertion_subject: "the rendered table output",
        });
    }

    None
}

fn config_policy_visible_sink_for(constant_name: &str, file: &str) -> Option<ConfigPolicySink> {
    let file = normalize_token_text(file);

    if (name_has_token(constant_name, "SCHEMA") || name_has_token(constant_name, "FIELD"))
        && file.contains("schema")
    {
        return Some(ConfigPolicySink {
            role: "schema_field_label",
            repair_target: "schema-render or golden-output test",
            description: "rendered schema output",
            actionability: "add_output_observer",
            repair_kind: "output_observer",
            target_test_type: "schema_render_or_golden",
            assertion_subject: "the rendered schema output",
        });
    }

    if (name_has_token(constant_name, "REPORT")
        || (name_has_token(constant_name, "POLICY") && name_has_token(constant_name, "LABEL")))
        && file.contains("report")
    {
        return Some(ConfigPolicySink {
            role: "rendered_policy_label",
            repair_target: "report-render or golden-output test",
            description: "rendered report output",
            actionability: "add_output_observer",
            repair_kind: "output_observer",
            target_test_type: "report_render_or_golden",
            assertion_subject: "the rendered report output",
        });
    }

    if (name_has_token(constant_name, "CONFIG") || name_has_token(constant_name, "SETTING"))
        && (file.contains("settings") || file.contains("output") || file.contains("render"))
    {
        return Some(ConfigPolicySink {
            role: "rendered_config_label",
            repair_target: "config-output, snapshot, or golden-output test",
            description: "rendered config output",
            actionability: "add_output_observer",
            repair_kind: "output_observer",
            target_test_type: "config_output_or_golden",
            assertion_subject: "the rendered config output",
        });
    }

    if (name_has_token(constant_name, "THRESHOLD")
        || name_has_token(constant_name, "SELECTOR")
        || name_has_token(constant_name, "VALIDATION"))
        && (file.contains("validation") || file.contains("routing") || file.contains("selector"))
    {
        return Some(ConfigPolicySink {
            role: "behavior_selector",
            repair_target: "behavior discriminator test",
            description: "observable validation or routing behavior",
            actionability: "add_behavior_discriminator",
            repair_kind: "behavior_discriminator",
            target_test_type: "validation_behavior",
            assertion_subject: "the selected behavior",
        });
    }

    None
}

fn is_internal_only_text(constant_name: &str, file: &str) -> bool {
    let file = normalize_token_text(file);
    name_has_token(constant_name, "INTERNAL")
        || name_has_token(constant_name, "PROOF")
        || name_has_token(constant_name, "POLICY")
        || name_has_token(constant_name, "CONFIG")
        || file.contains("proof")
        || file.contains("policy")
        || file.contains("config")
        || file.contains("internal")
}

fn is_internal_only_config_policy(constant_name: &str, file: &str) -> bool {
    let file = normalize_token_text(file);
    name_has_token(constant_name, "INTERNAL")
        || name_has_token(constant_name, "ALLOWLIST")
        || name_has_token(constant_name, "DENYLIST")
        || name_has_token(constant_name, "PROOF")
        || file.contains("internal")
        || file.contains("allowlist")
        || file.contains("denylist")
        || file.contains("proof")
}

fn is_opaque_config_policy_lookup(constant_name: &str, file: &str) -> bool {
    let file = normalize_token_text(file);
    name_has_token(constant_name, "OPAQUE")
        || file.contains("registry")
        || file.contains("lookup")
        || file.contains("dynamic")
}

fn observer_for_findings(
    raw_findings: &[&Finding],
) -> Option<(&'static str, FindingAlignmentRelatedTest)> {
    raw_findings
        .iter()
        .flat_map(|finding| finding.related_tests.iter())
        .filter_map(|test| {
            observer_for_related_test(test).map(|(rank, observer)| (rank, observer, test))
        })
        .min_by_key(|(rank, _, _)| *rank)
        .map(|(_, observer, test)| (observer, related_test_for(test)))
}
fn observer_for_related_test(test: &RelatedTest) -> Option<(u8, &'static str)> {
    let text = normalize_token_text(&format!("{} {}", test.name, test.file.display()));
    let strong_oracle = matches!(
        test.oracle_strength,
        OracleStrength::Strong | OracleStrength::Medium
    );

    if strong_oracle && text.contains("golden") {
        return Some((0, "golden"));
    }

    if test.oracle_kind == OracleKind::Snapshot || (strong_oracle && text.contains("snapshot")) {
        return Some((1, "snapshot"));
    }

    if strong_oracle && (text.contains("help_output") || text.contains("help")) {
        return Some((2, "cli_help_output"));
    }

    if strong_oracle && (text.contains("report") || text.contains("markdown")) {
        return Some((3, "report_render"));
    }

    if strong_oracle && text.contains("schema") {
        return Some((4, "schema_render"));
    }

    if strong_oracle && (text.contains("config") || text.contains("settings")) {
        return Some((5, "config_output"));
    }

    if strong_oracle && (text.contains("validation") || text.contains("routing")) {
        return Some((6, "validation_behavior"));
    }

    if strong_oracle && (text.contains("table") || text.contains("display")) {
        return Some((7, "table_render"));
    }

    None
}

fn related_test_for(test: &RelatedTest) -> FindingAlignmentRelatedTest {
    FindingAlignmentRelatedTest {
        name: test.name.clone(),
        file: test.file.display().to_string(),
        line: test.line,
    }
}

pub(super) fn name_has_token(name: &str, token: &str) -> bool {
    name.to_ascii_uppercase()
        .split('_')
        .any(|part| part == token)
}

pub(super) fn normalize_token_text(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}
