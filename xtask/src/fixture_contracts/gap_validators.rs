//! Gap-outcome fixture-corpus validators for `check-fixture-contracts`: the
//! swarm-plan packet, actionable-gap-outcomes, first-successful-pr, and
//! gap-decision-ledger corpora, with their const case tables and the shared
//! FIRST_PR boundary evidence strings.
//!
//! Extracted verbatim from `main.rs` as a behavior-preserving decomposition
//! slice of #2119. Items referenced outside this module are `pub(crate)` and
//! re-exported from `main.rs` so existing call sites (`dispatch.rs`,
//! `dogfood.rs`, and `tests.rs`) compile unchanged.

use super::*;

const SWARM_PLAN_PACKET_CORPUS: &str = "fixtures/swarm-plan-packet-corpus/corpus.json";

const SWARM_PLAN_PACKET_REQUIRED_CASES: &[(&str, &str)] = &[
    ("high_confidence_boundary_assertion_packet", "queued"),
    ("exact_error_variant_packet", "queued"),
    ("output_observer_packet", "queued"),
    (
        "public_projection_excluded_packet",
        "blocked_by_public_projection_exclusion",
    ),
    (
        "static_only_predicate_boundary_packet",
        "blocked_by_operator_judgment",
    ),
    (
        "blocked_static_limitation_packet",
        "blocked_by_static_limitation",
    ),
    (
        "activation_boundary_input_unresolved_packet",
        "blocked_by_static_limitation",
    ),
    (
        "missing_verify_command_packet",
        "blocked_by_missing_context",
    ),
    (
        "missing_receipt_command_packet",
        "blocked_by_missing_context",
    ),
    (
        "inconsistent_repair_route_packet",
        "blocked_by_missing_context",
    ),
    (
        "must_not_change_boundary_packet",
        "blocked_by_missing_context",
    ),
];

const ACTIONABLE_GAP_OUTCOMES_CORPUS: &str = "fixtures/actionable-gap-outcomes-corpus/corpus.json";

const ACTIONABLE_GAP_OUTCOMES_REQUIRED_CASES: &[(&str, &str, &str)] = &[
    (
        "not_attempted_packet",
        "not_attempted",
        RECEIPT_NOT_APPLICABLE,
    ),
    (
        "receipt_present_without_movement",
        "receipt_present",
        RECEIPT_FOUND,
    ),
    (
        "evidence_improved_from_receipt",
        "evidence_improved",
        RECEIPT_MOVEMENT_IMPROVED,
    ),
    (
        "attempted_no_receipt_from_unchanged_targeted_outcome",
        "attempted_no_receipt",
        RECEIPT_MISSING,
    ),
    (
        "attempted_no_receipt_from_regressed_targeted_outcome",
        "attempted_no_receipt",
        RECEIPT_MISSING,
    ),
    (
        "attempted_no_receipt_from_removed_targeted_outcome",
        "attempted_no_receipt",
        RECEIPT_MISSING,
    ),
    (
        "attempted_no_receipt_from_new_targeted_outcome",
        "attempted_no_receipt",
        RECEIPT_MISSING,
    ),
    (
        "orphaned_receipt_reported",
        "not_attempted",
        RECEIPT_GAP_MISMATCH,
    ),
];

const FIRST_SUCCESSFUL_PR_CORPUS: &str = "fixtures/first_successful_pr/corpus.json";
pub(crate) const FIRST_PR_BOUNDARY_CHANGED_BEHAVIOR: &str = "amount >= threshold";
pub(crate) const FIRST_PR_BOUNDARY_CURRENT_EVIDENCE_STRENGTH: &str = "Static evidence found related Rust test context, but the current proof is weak because the discriminator is missing.";
pub(crate) const FIRST_PR_BOUNDARY_MISSING_DISCRIMINATOR: &str =
    "Equality-boundary assertion for the changed behavior.";
pub(crate) const FIRST_PR_BOUNDARY_FOCUSED_PROOF_INTENT: &str =
    "Add a focused boundary assertion in tests/pricing.rs: assert_eq!(discount(100, 100), 90).";
pub(crate) const FIRST_PR_STATIC_EVIDENCE_BOUNDARY: &str = "static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval.";

const FIRST_SUCCESSFUL_PR_REQUIRED_CASES: &[(&str, &str, &str, &str)] = &[
    (
        "boundary-gap",
        "actionable",
        "top_gap",
        START_HERE_ACTIONABLE_GAP,
    ),
    (
        "output-contract-gap",
        "actionable",
        "top_gap",
        START_HERE_ACTIONABLE_GAP,
    ),
    (
        "typescript-preview-gap",
        "actionable",
        "top_gap",
        START_HERE_PREVIEW_LIMITED,
    ),
    ("empty-diff", "no_action", "empty_diff", START_HERE_CLEAN),
    (
        "blocked-ledger",
        "blocked",
        "blocked_artifact",
        START_HERE_MISSING_ARTIFACTS,
    ),
];

const GAP_DECISION_LEDGER_CORPUS: &str = "fixtures/gap-decision-ledger/corpus.json";

const GAP_DECISION_REQUIRED_KINDS: &[&str] = &[
    "MissingBoundaryAssertion",
    "MissingErrorDiscriminator",
    "MissingValueAssertion",
    "MissingSideEffectObserver",
    "MissingOutputContract",
    "StaticLimitation",
    "NoActionAlreadyObserved",
    "NoActionInternal",
    "Unknown",
];

const GAP_DECISION_REQUIRED_SCOPES: &[&str] = &[
    "pr_local",
    "repo_scoped",
    "baseline_debt",
    "artifact_missing",
];

const GAP_DECISION_REQUIRED_POLICY_STATES: &[&str] = &[
    "new",
    "baseline_known",
    "waived",
    "suppressed",
    "acknowledged",
    "resolved",
    "reintroduced",
    "blocked",
    "not_policy_targeted",
    "unknown",
];

const GAP_DECISION_REQUIRED_REPAIRABILITY: &[&str] = &[
    "repairable",
    "needs_human_design",
    "analyzer_limitation",
    "no_action",
    "unknown",
];

pub(crate) fn validate_swarm_plan_packet_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new("fixtures/swarm-plan-packet-corpus");
    for required in ["SPEC.md", "corpus.json"] {
        let path = root.join(required);
        if !path.exists() {
            violations.push(format!(
                "swarm plan packet fixture corpus is missing {}",
                normalize_path(&path)
            ));
        }
    }
    let spec = root.join("SPEC.md");
    if spec.exists() {
        let spec_text = read_text_lossy(&spec)?;
        if !spec_text
            .lines()
            .any(|line| line.starts_with("Spec: RIPR-SPEC-0057"))
        {
            violations.push(format!(
                "{} is missing `Spec: RIPR-SPEC-0057`",
                normalize_path(&spec)
            ));
        }
        for heading in ["## Given", "## When", "## Then", "## Must Not"] {
            if !has_markdown_heading(&spec_text, heading) {
                violations.push(format!("{} is missing `{heading}`", normalize_path(&spec)));
            }
        }
    }

    validate_swarm_plan_packet_fixture_corpus_at(Path::new(SWARM_PLAN_PACKET_CORPUS), violations)
}

fn validate_swarm_plan_packet_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        violations.push(format!(
            "swarm plan packet corpus is missing {}",
            normalize_path(path)
        ));
        return Ok(());
    }

    let corpus = match read_json_value(path) {
        Ok(value) => value,
        Err(err) => {
            violations.push(err);
            return Ok(());
        }
    };
    if json_string_field(&corpus, "kind").as_deref() != Some("swarm_plan_packet_corpus") {
        violations.push(format!(
            "{} kind must be swarm_plan_packet_corpus",
            normalize_path(path)
        ));
    }
    if json_string_field(&corpus, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "{} schema_version must be 0.1",
            normalize_path(path)
        ));
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0057") {
        violations.push(format!(
            "{} spec must be RIPR-SPEC-0057",
            normalize_path(path)
        ));
    }

    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        violations.push(format!("{} is missing cases array", normalize_path(path)));
        return Ok(());
    };
    let mut seen = BTreeMap::new();
    for case in cases {
        let case_id = json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string());
        if seen
            .insert(
                case_id.clone(),
                json_string_field(case.get("expected").unwrap_or(&Value::Null), "swarm_state")
                    .unwrap_or_default(),
            )
            .is_some()
        {
            violations.push(format!("swarm plan packet case {case_id} is duplicated"));
        }
        validate_swarm_plan_packet_fixture_case(case, &case_id, violations);
    }

    for (case_id, expected_state) in SWARM_PLAN_PACKET_REQUIRED_CASES {
        match seen.get(*case_id) {
            Some(actual) if actual == expected_state => {}
            Some(actual) => violations.push(format!(
                "swarm plan packet case {case_id} must have state {expected_state}, got {actual}"
            )),
            None => violations.push(format!(
                "swarm plan packet corpus is missing case {case_id}"
            )),
        }
    }

    Ok(())
}

pub(crate) fn validate_swarm_plan_packet_fixture_case(
    case: &Value,
    case_id: &str,
    violations: &mut Vec<String>,
) {
    if json_string_field(case, "description").is_none() {
        violations.push(format!(
            "swarm plan packet case {case_id} is missing description"
        ));
    }
    require_non_empty_string_array_at(case, "must_not_claim", case_id, violations);

    let Some(packet) = case.get("packet").filter(|value| value.is_object()) else {
        violations.push(format!(
            "swarm plan packet case {case_id} is missing packet object"
        ));
        return;
    };
    if !matches!(packet.get("raw_findings"), Some(Value::Array(values)) if !values.is_empty()) {
        violations.push(format!(
            "swarm plan packet case {case_id} must keep raw_findings as supporting evidence"
        ));
    }
    if let Some(receipt_command) = audit_non_empty_string(packet, &["receipt_command"])
        && !ripr_swarm_plan_fixture_receipt_command_supported(&receipt_command)
    {
        violations.push(format!(
            "swarm plan packet case {case_id} uses unsupported receipt_command `{receipt_command}`"
        ));
    }

    let Some(expected) = case.get("expected").filter(|value| value.is_object()) else {
        violations.push(format!(
            "swarm plan packet case {case_id} is missing expected object"
        ));
        return;
    };

    let actionable_gaps = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "report": "actionable-gaps",
        "summary": {"actionable_gaps": 1},
        "packets": [packet.clone()]
    });
    let report = ripr_swarm_plan_from_actionable_gaps_value(
        10,
        Path::new("target/ripr/reports/actionable-gaps.json"),
        &actionable_gaps,
    );
    if report.packets.len() != 1 {
        violations.push(format!(
            "swarm plan packet case {case_id} produced {} packets, expected 1",
            report.packets.len()
        ));
        return;
    }
    let planned = &report.packets[0];

    if let Some(expected_state) = json_string_field(expected, "swarm_state") {
        if planned.swarm_state != expected_state {
            violations.push(format!(
                "swarm plan packet case {case_id} state must be {expected_state}, got {}",
                planned.swarm_state
            ));
        }
    } else {
        violations.push(format!(
            "swarm plan packet case {case_id} expected.swarm_state is missing"
        ));
    }

    if let Some(expected_ready) = json_bool_field(expected, "swarm_ready") {
        let actual_ready = planned.swarm_state == "queued";
        if actual_ready != expected_ready {
            violations.push(format!(
                "swarm plan packet case {case_id} ready must be {expected_ready}, got {actual_ready}"
            ));
        }
        if expected_ready
            && audit_string_array(packet, &["allowed_edit_surface"])
                .unwrap_or_default()
                .is_empty()
        {
            violations.push(format!(
                "swarm plan packet case {case_id} must carry explicit allowed_edit_surface when swarm-ready"
            ));
        }
    }
    if let Some(expected_high_confidence) = json_bool_field(expected, "high_confidence") {
        let actual_high_confidence = ripr_swarm_plan_packet_is_high_confidence(planned);
        if actual_high_confidence != expected_high_confidence {
            violations.push(format!(
                "swarm plan packet case {case_id} high_confidence must be {expected_high_confidence}, got {actual_high_confidence}"
            ));
        }
    }

    let expected_missing_context = json_string_array_field(expected, "missing_context");
    if planned.missing_context != expected_missing_context {
        violations.push(format!(
            "swarm plan packet case {case_id} missing_context must be [{}], got [{}]",
            expected_missing_context.join(", "),
            planned.missing_context.join(", ")
        ));
    }
    let expected_blocked_reasons = json_string_array_field(expected, "blocked_reasons");
    if planned.blocked_reasons != expected_blocked_reasons {
        violations.push(format!(
            "swarm plan packet case {case_id} blocked_reasons must be [{}], got [{}]",
            expected_blocked_reasons.join(", "),
            planned.blocked_reasons.join(", ")
        ));
    }

    let output_packet = ripr_swarm_plan_packets_json(std::slice::from_ref(planned))
        .into_iter()
        .next()
        .unwrap_or(Value::Null);
    if output_packet
        .get("raw_findings_supporting_only")
        .and_then(Value::as_bool)
        != Some(true)
    {
        violations.push(format!(
            "swarm plan packet case {case_id} must emit raw_findings_supporting_only"
        ));
    }

    let Some(summary) = expected.get("summary").and_then(Value::as_object) else {
        violations.push(format!(
            "swarm plan packet case {case_id} expected.summary is missing"
        ));
        return;
    };
    let actual_summary = ripr_swarm_plan_summary_json(&report);
    for (key, expected_value) in summary {
        let Some(expected_count) = expected_value.as_u64() else {
            violations.push(format!(
                "swarm plan packet case {case_id} expected.summary.{key} must be numeric"
            ));
            continue;
        };
        let actual_count = actual_summary.get(key).and_then(Value::as_u64);
        if actual_count != Some(expected_count) {
            violations.push(format!(
                "swarm plan packet case {case_id} summary.{key} must be {expected_count}, got {}",
                actual_count
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "missing".to_string())
            ));
        }
    }
}

pub(crate) fn validate_actionable_gap_outcomes_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new("fixtures/actionable-gap-outcomes-corpus");
    for required in ["SPEC.md", "corpus.json"] {
        let path = root.join(required);
        if !path.exists() {
            violations.push(format!(
                "actionable gap outcomes fixture corpus is missing {}",
                normalize_path(&path)
            ));
        }
    }
    let spec = root.join("SPEC.md");
    if spec.exists() {
        let spec_text = read_text_lossy(&spec)?;
        if !spec_text
            .lines()
            .any(|line| line.starts_with("Spec: RIPR-SPEC-0031"))
        {
            violations.push(format!(
                "{} is missing `Spec: RIPR-SPEC-0031`",
                normalize_path(&spec)
            ));
        }
        if !spec_text
            .lines()
            .any(|line| line.starts_with("Related: RIPR-SPEC-0057"))
        {
            violations.push(format!(
                "{} is missing `Related: RIPR-SPEC-0057`",
                normalize_path(&spec)
            ));
        }
        for heading in ["## Given", "## When", "## Then", "## Must Not"] {
            if !has_markdown_heading(&spec_text, heading) {
                violations.push(format!("{} is missing `{heading}`", normalize_path(&spec)));
            }
        }
    }

    validate_actionable_gap_outcomes_fixture_corpus_at(
        Path::new(ACTIONABLE_GAP_OUTCOMES_CORPUS),
        violations,
    )
}

pub(crate) fn validate_actionable_gap_outcomes_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        violations.push(format!(
            "actionable gap outcomes corpus is missing {}",
            normalize_path(path)
        ));
        return Ok(());
    }

    let corpus = match read_json_value(path) {
        Ok(value) => value,
        Err(err) => {
            violations.push(err);
            return Ok(());
        }
    };
    if json_string_field(&corpus, "kind").as_deref() != Some("actionable_gap_outcomes_corpus") {
        violations.push(format!(
            "{} kind must be actionable_gap_outcomes_corpus",
            normalize_path(path)
        ));
    }
    if json_string_field(&corpus, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "{} schema_version must be 0.1",
            normalize_path(path)
        ));
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0031") {
        violations.push(format!(
            "{} spec must be RIPR-SPEC-0031",
            normalize_path(path)
        ));
    }
    if json_string_field(&corpus, "related_spec").as_deref() != Some("RIPR-SPEC-0057") {
        violations.push(format!(
            "{} related_spec must be RIPR-SPEC-0057",
            normalize_path(path)
        ));
    }

    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        violations.push(format!("{} is missing cases array", normalize_path(path)));
        return Ok(());
    };
    let mut seen = BTreeMap::new();
    for case in cases {
        let case_id = json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string());
        if seen
            .insert(
                case_id.clone(),
                actionable_gap_outcomes_expected_case_state(case).unwrap_or_default(),
            )
            .is_some()
        {
            violations.push(format!(
                "actionable gap outcomes case {case_id} is duplicated"
            ));
        }
        validate_actionable_gap_outcomes_fixture_case(case, &case_id, violations)?;
    }

    for (case_id, expected_state, expected_receipt_state) in ACTIONABLE_GAP_OUTCOMES_REQUIRED_CASES
    {
        match seen.get(*case_id) {
            Some((actual_state, actual_receipt_state))
                if actual_state == expected_state && actual_receipt_state == expected_receipt_state => {}
            Some((actual_state, actual_receipt_state)) => violations.push(format!(
                "actionable gap outcomes case {case_id} must have state {expected_state}/{expected_receipt_state}, got {actual_state}/{actual_receipt_state}"
            )),
            None => violations.push(format!(
                "actionable gap outcomes corpus is missing case {case_id}"
            )),
        }
    }

    Ok(())
}

fn actionable_gap_outcomes_expected_case_state(case: &Value) -> Option<(String, String)> {
    let outcome = audit_array(case, &["expected", "outcomes"]).first()?;
    Some((
        json_string_field(outcome, "outcome_state")?,
        json_string_field(outcome, "receipt_state")?,
    ))
}

pub(crate) fn validate_actionable_gap_outcomes_fixture_case(
    case: &Value,
    case_id: &str,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if json_string_field(case, "description").is_none() {
        violations.push(format!(
            "actionable gap outcomes case {case_id} is missing description"
        ));
    }
    require_actionable_gap_outcomes_string_array_at(case, "must_not_claim", case_id, violations);

    let Some(actionable_gaps) = case
        .get("actionable_gaps")
        .filter(|value| value.is_object())
    else {
        violations.push(format!(
            "actionable gap outcomes case {case_id} is missing actionable_gaps object"
        ));
        return Ok(());
    };
    if audit_array(actionable_gaps, &["packets"]).is_empty() {
        violations.push(format!(
            "actionable gap outcomes case {case_id} actionable_gaps.packets must not be empty"
        ));
        return Ok(());
    }
    for packet in audit_array(actionable_gaps, &["packets"]) {
        let packet_id =
            json_string_field(packet, "canonical_gap_id").unwrap_or_else(|| "unknown".to_string());
        if let Some(receipt_command) = audit_non_empty_string(packet, &["receipt_command_or_path"])
            .or_else(|| audit_non_empty_string(packet, &["receipt_command"]))
            && !ripr_swarm_plan_fixture_receipt_command_supported(&receipt_command)
        {
            violations.push(format!(
                "actionable gap outcomes case {case_id} packet {packet_id} uses unsupported receipt command `{receipt_command}`"
            ));
        }
    }

    let Some(expected) = case.get("expected").filter(|value| value.is_object()) else {
        violations.push(format!(
            "actionable gap outcomes case {case_id} is missing expected object"
        ));
        return Ok(());
    };
    let expected_outcomes = audit_array(expected, &["outcomes"]);
    if expected_outcomes.is_empty() {
        violations.push(format!(
            "actionable gap outcomes case {case_id} expected.outcomes must not be empty"
        ));
        return Ok(());
    }

    let agent_receipt = actionable_gap_outcomes_optional_fixture_input(case, "agent_receipt");
    let targeted_test_outcome =
        actionable_gap_outcomes_optional_fixture_input(case, "targeted_test_outcome");
    if let Some(targeted_test_outcome) = targeted_test_outcome {
        validate_actionable_gap_outcomes_targeted_shape(case_id, targeted_test_outcome, violations);
    }
    let report = match actionable_gap_outcomes_report_from_values(
        actionable_gaps,
        agent_receipt,
        targeted_test_outcome,
        format!(
            "{}#{case_id}:actionable_gaps",
            ACTIONABLE_GAP_OUTCOMES_CORPUS
        ),
        agent_receipt
            .map(|_| format!("{}#{case_id}:agent_receipt", ACTIONABLE_GAP_OUTCOMES_CORPUS)),
        targeted_test_outcome.map(|_| {
            format!(
                "{}#{case_id}:targeted_test_outcome",
                ACTIONABLE_GAP_OUTCOMES_CORPUS
            )
        }),
    ) {
        Ok(report) => report,
        Err(err) => {
            violations.push(format!(
                "actionable gap outcomes case {case_id} failed to build report: {err}"
            ));
            return Ok(());
        }
    };

    for expected_outcome in expected_outcomes {
        validate_actionable_gap_outcomes_expected_outcome(
            case_id,
            expected_outcome,
            &report,
            violations,
        );
    }

    let rendered = match actionable_gap_outcomes_json(&report) {
        Ok(json) => json,
        Err(err) => {
            violations.push(format!(
                "actionable gap outcomes case {case_id} failed to render JSON: {err}"
            ));
            return Ok(());
        }
    };
    let rendered: Value = match serde_json::from_str(&rendered) {
        Ok(value) => value,
        Err(err) => {
            violations.push(format!(
                "actionable gap outcomes case {case_id} rendered invalid JSON: {err}"
            ));
            return Ok(());
        }
    };
    validate_actionable_gap_outcomes_expected_summary(case_id, expected, &rendered, violations);
    validate_actionable_gap_outcomes_expected_orphaned_receipts(
        case_id, expected, &rendered, violations,
    );
    let markdown = actionable_gap_outcomes_markdown(&report);
    if !markdown.contains("# Actionable Gap Outcomes") {
        violations.push(format!(
            "actionable gap outcomes case {case_id} markdown must include report heading"
        ));
    }

    Ok(())
}

fn actionable_gap_outcomes_optional_fixture_input<'a>(
    case: &'a Value,
    field: &str,
) -> Option<&'a Value> {
    case.get(field).filter(|value| !value.is_null())
}

fn validate_actionable_gap_outcomes_targeted_shape(
    case_id: &str,
    targeted_test_outcome: &Value,
    violations: &mut Vec<String>,
) {
    for bucket in ["moved", "unchanged", "regressed"] {
        for item in audit_array(targeted_test_outcome, &[bucket]) {
            for field in ["before", "after", "direction"] {
                if audit_non_empty_string(item, &[field]).is_none() {
                    violations.push(format!(
                        "actionable gap outcomes case {case_id} targeted_test_outcome.{bucket} item must include {field}"
                    ));
                }
            }
        }
    }
    for bucket in ["removed", "new"] {
        for item in audit_array(targeted_test_outcome, &[bucket]) {
            if audit_non_empty_string(item, &["grip_class"]).is_none() {
                violations.push(format!(
                    "actionable gap outcomes case {case_id} targeted_test_outcome.{bucket} item must include grip_class"
                ));
            }
            for field in ["before", "after", "direction"] {
                if item.get(field).is_some_and(|value| !value.is_null()) {
                    violations.push(format!(
                        "actionable gap outcomes case {case_id} targeted_test_outcome.{bucket} item must use one-sided grip_class instead of {field}"
                    ));
                }
            }
        }
    }
}

fn validate_actionable_gap_outcomes_expected_outcome(
    case_id: &str,
    expected: &Value,
    report: &ActionableGapOutcomesReport,
    violations: &mut Vec<String>,
) {
    let Some(canonical_gap_id) = json_string_field(expected, "canonical_gap_id") else {
        violations.push(format!(
            "actionable gap outcomes case {case_id} expected outcome is missing canonical_gap_id"
        ));
        return;
    };
    let Some(actual) = report
        .outcomes
        .iter()
        .find(|outcome| outcome.canonical_gap_id == canonical_gap_id)
    else {
        violations.push(format!(
            "actionable gap outcomes case {case_id} did not produce outcome {canonical_gap_id}"
        ));
        return;
    };

    for (field, actual_value) in [
        ("outcome_state", actual.outcome_state.as_str()),
        ("receipt_state", actual.receipt_state.as_str()),
    ] {
        let Some(expected_value) = json_string_field(expected, field) else {
            violations.push(format!(
                "actionable gap outcomes case {case_id} expected outcome {canonical_gap_id} is missing {field}"
            ));
            continue;
        };
        if actual_value != expected_value.as_str() {
            violations.push(format!(
                "actionable gap outcomes case {case_id} outcome {canonical_gap_id} {field} must be {expected_value}, got {actual_value}"
            ));
        }
    }

    for (field, actual_value) in [
        (
            "movement_source",
            actual.movement_source.as_deref().map(Value::from),
        ),
        (
            "movement_direction",
            actual.movement_direction.as_deref().map(Value::from),
        ),
        ("before", actual.before.as_deref().map(Value::from)),
        ("after", actual.after.as_deref().map(Value::from)),
    ] {
        if let Some(expected_value) = expected.get(field) {
            let actual_value = actual_value.unwrap_or(Value::Null);
            if actual_value != *expected_value {
                violations.push(format!(
                    "actionable gap outcomes case {case_id} outcome {canonical_gap_id} {field} must be {expected_value}, got {actual_value}"
                ));
            }
        }
    }
}

fn validate_actionable_gap_outcomes_expected_summary(
    case_id: &str,
    expected: &Value,
    rendered: &Value,
    violations: &mut Vec<String>,
) {
    let Some(summary) = expected.get("summary").and_then(Value::as_object) else {
        violations.push(format!(
            "actionable gap outcomes case {case_id} expected.summary is missing"
        ));
        return;
    };
    let null_summary = Value::Null;
    let actual_summary = audit_get(rendered, &["summary"]).unwrap_or(&null_summary);
    for (key, expected_value) in summary {
        let Some(expected_count) = expected_value.as_u64() else {
            violations.push(format!(
                "actionable gap outcomes case {case_id} expected.summary.{key} must be numeric"
            ));
            continue;
        };
        let actual_count = actual_summary.get(key).and_then(Value::as_u64);
        if actual_count != Some(expected_count) {
            violations.push(format!(
                "actionable gap outcomes case {case_id} summary.{key} must be {expected_count}, got {}",
                actual_count
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "missing".to_string())
            ));
        }
    }
}

fn validate_actionable_gap_outcomes_expected_orphaned_receipts(
    case_id: &str,
    expected: &Value,
    rendered: &Value,
    violations: &mut Vec<String>,
) {
    let Some(expected_receipts) = expected.get("orphaned_receipts").and_then(Value::as_array)
    else {
        return;
    };
    let actual_receipts = audit_get(rendered, &["orphaned_receipts"])
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for expected_receipt in expected_receipts {
        let Some(receipt_id) = json_string_field(expected_receipt, "receipt_id") else {
            violations.push(format!(
                "actionable gap outcomes case {case_id} expected orphaned receipt is missing receipt_id"
            ));
            continue;
        };
        let Some(actual) = actual_receipts.iter().find(|actual| {
            json_string_field(actual, "receipt_id").as_deref() == Some(receipt_id.as_str())
        }) else {
            violations.push(format!(
                "actionable gap outcomes case {case_id} did not produce orphaned receipt {receipt_id}"
            ));
            continue;
        };
        for field in ["seam_id", "movement_direction"] {
            if let Some(expected_value) = expected_receipt.get(field) {
                let actual_value = actual.get(field).cloned().unwrap_or(Value::Null);
                if actual_value != *expected_value {
                    violations.push(format!(
                        "actionable gap outcomes case {case_id} orphaned receipt {receipt_id} {field} must be {expected_value}, got {actual_value}"
                    ));
                }
            }
        }
    }
}

fn require_actionable_gap_outcomes_string_array_at(
    value: &Value,
    field: &str,
    case_id: &str,
    violations: &mut Vec<String>,
) {
    match value.get(field) {
        Some(Value::Array(items))
            if !items.is_empty() && items.iter().all(|item| item.as_str().is_some()) => {}
        _ => violations.push(format!(
            "actionable gap outcomes case {case_id} {field} must be a non-empty string array"
        )),
    }
}

fn ripr_swarm_plan_fixture_receipt_command_supported(command: &str) -> bool {
    let command = command.trim();
    ripr_swarm_plan_field_missing(command)
        || command == "cargo xtask receipts"
        || command == "cargo xtask receipts check"
        || command
            .strip_prefix("ripr agent receipt ")
            .is_some_and(ripr_swarm_plan_fixture_agent_receipt_args_supported)
        || command
            .strip_prefix("cargo run -p ripr -- agent receipt ")
            .is_some_and(ripr_swarm_plan_fixture_agent_receipt_args_supported)
}

fn ripr_swarm_plan_fixture_agent_receipt_args_supported(args: &str) -> bool {
    let mut has_json = false;
    let mut has_verify_json = false;
    let mut has_seam_id = false;
    let mut tokens = args.split_whitespace();
    let mut saw_any = false;

    while let Some(token) = tokens.next() {
        saw_any = true;
        match token {
            "--json" => has_json = true,
            "--root" | "--verify-json" | "--seam-id" | "--test" | "--command" | "--out" => {
                let Some(value) = tokens.next() else {
                    return false;
                };
                if value.trim().is_empty() || value.starts_with("--") {
                    return false;
                }
                match token {
                    "--verify-json" => has_verify_json = true,
                    "--seam-id" => has_seam_id = true,
                    _ => {}
                }
            }
            _ => return false,
        }
    }

    saw_any && has_json && has_verify_json && has_seam_id
}

pub(crate) fn validate_first_successful_pr_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new("fixtures/first_successful_pr");
    for required in ["README.md", "corpus.json"] {
        let path = root.join(required);
        if !path.exists() {
            violations.push(format!(
                "first successful PR fixture corpus is missing {}",
                normalize_path(&path)
            ));
        }
    }
    validate_first_successful_pr_fixture_corpus_at(
        Path::new(FIRST_SUCCESSFUL_PR_CORPUS),
        violations,
    )
}

pub(crate) fn validate_first_successful_pr_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let Some(root) = path.parent() else {
        violations.push(format!(
            "first successful PR corpus path has no parent: {}",
            normalize_path(path)
        ));
        return Ok(());
    };
    if !path.exists() {
        violations.push(format!(
            "first successful PR corpus is missing {}",
            normalize_path(path)
        ));
        return Ok(());
    }

    let corpus = match read_json_value(path) {
        Ok(value) => value,
        Err(err) => {
            violations.push(err);
            return Ok(());
        }
    };
    if json_string_field(&corpus, "kind").as_deref() != Some("first_successful_pr_corpus") {
        violations.push(format!(
            "{} kind must be first_successful_pr_corpus",
            normalize_path(path)
        ));
    }
    if json_string_field(&corpus, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "{} schema_version must be 0.1",
            normalize_path(path)
        ));
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0051") {
        violations.push(format!(
            "{} spec must be RIPR-SPEC-0051",
            normalize_path(path)
        ));
    }

    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        violations.push(format!("{} is missing cases array", normalize_path(path)));
        return Ok(());
    };
    let mut seen = BTreeMap::new();
    for case in cases {
        let case_id = json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string());
        if seen
            .insert(
                case_id.clone(),
                (
                    json_string_field(case, "expected_status").unwrap_or_default(),
                    json_string_field(case, "expected_state").unwrap_or_default(),
                    json_string_field(case, "expected_output_state").unwrap_or_default(),
                ),
            )
            .is_some()
        {
            violations.push(format!("first successful PR case {case_id} is duplicated"));
        }
        validate_first_successful_pr_case(root, case, &case_id, violations)?;
    }
    for (case_id, expected_status, expected_state, expected_output_state) in
        FIRST_SUCCESSFUL_PR_REQUIRED_CASES
    {
        match seen.get(*case_id) {
            Some((status, state, output_state))
                if status == expected_status
                    && state == expected_state
                    && output_state == expected_output_state => {}
            Some((status, state, output_state)) => violations.push(format!(
                "first successful PR case {case_id} must be {expected_status}/{expected_state}/{expected_output_state}, got {status}/{state}/{output_state}"
            )),
            None => violations.push(format!(
                "first successful PR corpus is missing case {case_id}"
            )),
        }
    }
    Ok(())
}

fn validate_first_successful_pr_case(
    root: &Path,
    case: &Value,
    case_id: &str,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    for field in [
        "description",
        "expected_status",
        "expected_state",
        "expected_output_state",
    ] {
        if json_string_field(case, field).is_none() {
            violations.push(format!(
                "first successful PR case {case_id} is missing {field}"
            ));
        }
    }
    let case_dir = root.join(case_id);
    let input_ledger = case_dir.join("inputs/reports/gap-decision-ledger.json");
    let expected_json = case_dir.join("expected/start-here.json");
    let expected_md = case_dir.join("expected/start-here.md");
    let expected_status = json_string_field(case, "expected_status").unwrap_or_default();
    let expected_state = json_string_field(case, "expected_state").unwrap_or_default();
    let expected_output_state =
        json_string_field(case, "expected_output_state").unwrap_or_default();
    for required in [&input_ledger, &expected_json, &expected_md] {
        if !required.exists() {
            violations.push(format!(
                "first successful PR case {case_id} is missing {}",
                normalize_path(required)
            ));
        }
    }
    if case_id == "boundary-gap" {
        validate_first_successful_pr_boundary_demo(root, case, case_id, violations)?;
    }
    if input_ledger.exists() {
        let ledger = read_json_value(&input_ledger)?;
        if json_string_field(&ledger, "kind").as_deref() != Some("gap_decision_ledger") {
            violations.push(format!(
                "first successful PR case {case_id} input ledger kind must be gap_decision_ledger"
            ));
        }
    }
    if expected_json.exists() {
        let packet = read_json_value(&expected_json)?;
        if json_string_field(&packet, "schema_version").as_deref() != Some("0.1") {
            violations.push(format!(
                "first successful PR case {case_id} start-here schema_version must be 0.1"
            ));
        }
        if json_string_field(&packet, "kind").as_deref() != Some("first_pr_start_here") {
            violations.push(format!(
                "first successful PR case {case_id} start-here kind must be first_pr_start_here"
            ));
        }
        if json_string_field(&packet, "status").as_deref() != Some(expected_status.as_str()) {
            violations.push(format!(
                "first successful PR case {case_id} status must be {expected_status}"
            ));
        }
        if json_string_field(&packet, "posture").as_deref() != Some("advisory") {
            violations.push(format!(
                "first successful PR case {case_id} posture must be advisory"
            ));
        }
        if audit_string(&packet, &["selected", "state"]).as_deref() != Some(expected_state.as_str())
        {
            violations.push(format!(
                "first successful PR case {case_id} selected.state must be {expected_state}"
            ));
        }
        if audit_string(&packet, &["selected", "output_state"]).as_deref()
            != Some(expected_output_state.as_str())
        {
            violations.push(format!(
                "first successful PR case {case_id} selected.output_state must be {expected_output_state}"
            ));
        }
        if expected_status == "actionable"
            && audit_string(&packet, &["selected", "verify_command"]).is_none()
        {
            violations.push(format!(
                "first successful PR case {case_id} actionable packet must name verify_command"
            ));
        }
        if expected_status == "actionable" {
            validate_first_successful_pr_actionable_json(case_id, &packet, violations);
        }
    }
    if expected_md.exists() {
        let markdown = read_text_lossy(&expected_md)?;
        for required in [
            "# RIPR First PR Start Here",
            "Status: advisory",
            "## Start Here",
            "## Artifacts",
            "## Authority",
            "## Limits",
        ] {
            if !markdown.contains(required) {
                violations.push(format!(
                    "first successful PR case {case_id} Markdown is missing `{required}`"
                ));
            }
        }
        match expected_status.as_str() {
            "actionable" if !markdown.contains("## Start Here") => {
                violations.push(format!(
                    "first successful PR case {case_id} Markdown must show Start Here"
                ));
                violations.push(format!(
                    "first successful PR case {case_id} Markdown must show state `{expected_state}`"
                ));
            }
            "actionable" if !markdown.contains(&format!("- State: `{expected_state}`")) => {
                violations.push(format!(
                    "first successful PR case {case_id} Markdown must show state `{expected_state}`"
                ));
            }
            "no_action" if !markdown.contains(&format!("- State: `{expected_state}`")) => {
                violations.push(format!(
                    "first successful PR case {case_id} Markdown must show no-action state `{expected_state}`"
                ));
            }
            "blocked" if !markdown.contains(&format!("- State: `{expected_state}`")) => {
                violations.push(format!(
                    "first successful PR case {case_id} Markdown must show blocked state `{expected_state}`"
                ));
            }
            _ => {}
        }
        if !markdown.contains(&format!("- Output state: `{expected_output_state}`")) {
            violations.push(format!(
                "first successful PR case {case_id} Markdown must show output state `{expected_output_state}`"
            ));
        }
        if expected_status == "actionable" {
            validate_first_successful_pr_actionable_markdown(case_id, &markdown, violations);
        }
    }
    validate_first_successful_pr_outcome_receipts(root, case, case_id, violations)?;
    Ok(())
}

fn validate_first_successful_pr_outcome_receipts(
    root: &Path,
    case: &Value,
    case_id: &str,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if case.get("expected_outcome").is_some() {
        let receipt = serde_json::json!({
            "id": "default",
            "before": json_string_field(case, "outcome_before"),
            "after": json_string_field(case, "outcome_after"),
            "expected": json_string_field(case, "expected_outcome"),
            "expected_markdown": json_string_field(case, "expected_outcome_markdown"),
            "expected_gap_movement": json_string_field(case, "expected_gap_movement")
        });
        validate_first_successful_pr_outcome_receipt(root, case_id, &receipt, violations)?;
    }

    let Some(receipts) = case.get("outcome_receipts") else {
        return Ok(());
    };
    let Some(receipts) = receipts.as_array() else {
        violations.push(format!(
            "first successful PR case {case_id} outcome_receipts must be an array"
        ));
        return Ok(());
    };

    let mut seen = BTreeSet::new();
    for receipt in receipts {
        let id = json_string_field(receipt, "id").unwrap_or_else(|| "unknown".to_string());
        if !seen.insert(id.clone()) {
            violations.push(format!(
                "first successful PR case {case_id} outcome receipt {id} is duplicated"
            ));
        }
        validate_first_successful_pr_outcome_receipt(root, case_id, receipt, violations)?;
    }
    Ok(())
}

pub(crate) fn validate_first_successful_pr_outcome_receipt(
    root: &Path,
    case_id: &str,
    receipt: &Value,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let receipt_id = json_string_field(receipt, "id").unwrap_or_else(|| "unknown".to_string());
    let required_fields = [
        "before",
        "after",
        "expected",
        "expected_markdown",
        "expected_gap_movement",
    ];
    for field in required_fields {
        if json_string_field(receipt, field).is_none() {
            violations.push(format!(
                "first successful PR case {case_id} outcome receipt {receipt_id} is missing {field}"
            ));
        }
    }

    let Some(before) = json_string_field(receipt, "before") else {
        return Ok(());
    };
    let Some(after) = json_string_field(receipt, "after") else {
        return Ok(());
    };
    let Some(expected) = json_string_field(receipt, "expected") else {
        return Ok(());
    };
    let Some(expected_markdown) = json_string_field(receipt, "expected_markdown") else {
        return Ok(());
    };
    let Some(expected_gap_movement) = json_string_field(receipt, "expected_gap_movement") else {
        return Ok(());
    };

    for movement in [
        "closed",
        "opened",
        "strengthened",
        "weakened",
        "unchanged",
        "new",
        "removed",
        "changed",
    ] {
        if movement == expected_gap_movement {
            break;
        }
        if movement == "changed" {
            violations.push(format!(
                "first successful PR case {case_id} outcome receipt {receipt_id} has unknown expected_gap_movement {expected_gap_movement}"
            ));
        }
    }

    for rel in [&before, &after, &expected, &expected_markdown] {
        let path = root.join(rel);
        if !path.exists() {
            violations.push(format!(
                "first successful PR case {case_id} outcome receipt {receipt_id} is missing {}",
                normalize_path(&path)
            ));
        }
    }

    let expected_path = root.join(&expected);
    if expected_path.exists() {
        let outcome = read_json_value(&expected_path)?;
        if json_string_field(&outcome, "schema_version").as_deref() != Some("0.1") {
            violations.push(format!(
                "first successful PR case {case_id} outcome receipt {receipt_id} schema_version must be 0.1"
            ));
        }
        if json_string_field(&outcome, "tool").as_deref() != Some("ripr") {
            violations.push(format!(
                "first successful PR case {case_id} outcome receipt {receipt_id} tool must be ripr"
            ));
        }
        if json_string_field(&outcome, "status").as_deref() != Some("advisory") {
            violations.push(format!(
                "first successful PR case {case_id} outcome receipt {receipt_id} status must be advisory"
            ));
        }
        let expected_before_input = first_successful_pr_outcome_input_path(root, &before);
        let expected_after_input = first_successful_pr_outcome_input_path(root, &after);
        if audit_string(&outcome, &["inputs", "before"]).as_deref()
            != Some(expected_before_input.as_str())
        {
            violations.push(format!(
                "first successful PR case {case_id} outcome receipt {receipt_id} before input must be {expected_before_input}"
            ));
        }
        if audit_string(&outcome, &["inputs", "after"]).as_deref()
            != Some(expected_after_input.as_str())
        {
            violations.push(format!(
                "first successful PR case {case_id} outcome receipt {receipt_id} after input must be {expected_after_input}"
            ));
        }
        let movement_count = outcome
            .get("summary")
            .and_then(|summary| summary.get("gap_movement"))
            .and_then(|movement| movement.get(expected_gap_movement.as_str()))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if movement_count == 0 {
            violations.push(format!(
                "first successful PR case {case_id} outcome receipt {receipt_id} must report expected gap movement {expected_gap_movement}"
            ));
        }
        if !outcome.get("review_receipt").is_some_and(Value::is_object) {
            violations.push(format!(
                "first successful PR case {case_id} outcome receipt {receipt_id} must include review_receipt"
            ));
        }
    }

    let expected_markdown_path = root.join(&expected_markdown);
    if expected_markdown_path.exists() {
        let markdown = read_text_lossy(&expected_markdown_path)?;
        for required in [
            "# ripr targeted-test outcome report",
            "Status: advisory",
            "## Gap Movement",
            "## Review Receipt",
        ] {
            if !markdown.contains(required) {
                violations.push(format!(
                    "first successful PR case {case_id} outcome receipt {receipt_id} Markdown is missing `{required}`"
                ));
            }
        }
        let expected_row = format!("| {expected_gap_movement} | 1 |");
        if !markdown.contains(&expected_row) {
            violations.push(format!(
                "first successful PR case {case_id} outcome receipt {receipt_id} Markdown must show `{expected_row}`"
            ));
        }
    }

    Ok(())
}

pub(crate) fn first_successful_pr_outcome_input_path(root: &Path, relative: &str) -> String {
    let path = root.join(relative);
    if let Some(stable) = first_successful_pr_fixture_relative_path(&path) {
        return stable;
    }
    if let Ok(current_dir) = std::env::current_dir() {
        if let Ok(stripped) = path.strip_prefix(&current_dir) {
            return normalize_path(stripped);
        }
        if let Some(parent) = current_dir.parent()
            && let Ok(stripped) = path.strip_prefix(parent)
        {
            return normalize_path(stripped);
        }
    }
    normalize_path(&path)
}

fn first_successful_pr_fixture_relative_path(path: &Path) -> Option<String> {
    let parts = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();

    for index in 0..parts.len().saturating_sub(1) {
        if parts[index] == "fixtures" && parts[index + 1] == "first_successful_pr" {
            let mut stable = PathBuf::new();
            for part in &parts[index..] {
                stable.push(part);
            }
            return Some(normalize_path(&stable));
        }
    }

    None
}

pub(crate) fn validate_first_successful_pr_actionable_json(
    case_id: &str,
    packet: &Value,
    violations: &mut Vec<String>,
) {
    for (path, label) in [
        (&["selected", "kind"][..], "top actionable gap"),
        (&["selected", "changed_behavior"][..], "changed behavior"),
        (&["selected", "why"][..], "why this matters"),
        (
            &["selected", "current_evidence_strength"][..],
            "current evidence strength",
        ),
        (
            &["selected", "missing_discriminator"][..],
            "missing discriminator",
        ),
        (
            &["selected", "focused_proof_intent"][..],
            "focused proof intent",
        ),
        (&["selected", "verify_command"][..], "verify command"),
    ] {
        if audit_non_empty_string(packet, path).is_none() {
            violations.push(format!(
                "first successful PR case {case_id} actionable packet must name {label}"
            ));
        }
    }

    let receipt_command = audit_non_empty_string(packet, &["selected", "receipt_command"]);
    let receipt_path = audit_non_empty_string(packet, &["selected", "receipt_path"]);
    if receipt_command.is_none() && receipt_path.is_none() {
        violations.push(format!(
            "first successful PR case {case_id} actionable packet must name receipt command or path"
        ));
    }

    match audit_string(packet, &["selected", "static_evidence_boundary"]) {
        Some(boundary) if boundary == FIRST_PR_STATIC_EVIDENCE_BOUNDARY => {}
        Some(boundary) => violations.push(format!(
            "first successful PR case {case_id} actionable packet static_evidence_boundary must be {FIRST_PR_STATIC_EVIDENCE_BOUNDARY:?}, got {boundary:?}"
        )),
        None => violations.push(format!(
            "first successful PR case {case_id} actionable packet must name static_evidence_boundary"
        )),
    }
}

pub(crate) fn validate_first_successful_pr_actionable_markdown(
    case_id: &str,
    markdown: &str,
    violations: &mut Vec<String>,
) {
    for required in [
        "- Top actionable gap:",
        "- Changed behavior:",
        "- Why this matters:",
        "- Current evidence strength:",
        "- Missing discriminator:",
        "- Focused proof intent:",
        "- Verify command:",
        "- Boundary: static advisory evidence only;",
    ] {
        if !markdown.contains(required) {
            violations.push(format!(
                "first successful PR case {case_id} actionable Markdown must include `{required}`"
            ));
        }
    }
    if !(markdown.contains("- Receipt command:") || markdown.contains("- Receipt path:")) {
        violations.push(format!(
            "first successful PR case {case_id} actionable Markdown must include receipt command or path"
        ));
    }
}

fn validate_first_successful_pr_boundary_demo(
    root: &Path,
    case: &Value,
    case_id: &str,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let Some(demo_story) = json_string_field(case, "demo_story") else {
        violations.push(format!(
            "first successful PR case {case_id} is missing demo_story"
        ));
        return Ok(());
    };
    let story_path = Path::new(&demo_story);
    if story_path.is_absolute()
        || story_path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        violations.push(format!(
            "first successful PR case {case_id} demo_story must be workspace-local"
        ));
        return Ok(());
    }
    let story_path = root.join(story_path);
    if !story_path.exists() {
        violations.push(format!(
            "first successful PR case {case_id} is missing {}",
            normalize_path(&story_path)
        ));
        return Ok(());
    }
    let story = read_text_lossy(&story_path)?;
    for required in [
        "before evidence",
        "first-pr recommendation",
        "ripr first-pr",
        "ripr outcome",
        "focused external proof",
        "reviewer receipt",
        "fixture smoke path",
        "release demo",
        "agent training path",
        "cargo xtask fixtures boundary_gap",
        "fixtures/boundary_gap/calibration/before-targeted-test.repo-exposure.json",
        "fixtures/boundary_gap/calibration/after-targeted-test.repo-exposure.json",
        "fixtures/boundary_gap/calibration/targeted-test-outcome.md",
        "No runtime mutation proof",
        "No coverage adequacy",
        "No general correctness proof",
        "No merge approval",
        "No source edit or generated test from RIPR",
    ] {
        if !story.contains(required) {
            violations.push(format!(
                "{} is missing `{required}`",
                normalize_path(&story_path)
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_gap_decision_ledger_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    validate_gap_decision_ledger_fixture_corpus_at(
        Path::new(GAP_DECISION_LEDGER_CORPUS),
        violations,
    )
}

pub(crate) fn validate_gap_decision_ledger_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let Some(base) = path.parent() else {
        violations.push(format!(
            "gap-decision ledger corpus path has no parent: {}",
            normalize_path(path)
        ));
        return Ok(());
    };
    for required in ["README.md", "corpus.json"] {
        let required_path = base.join(required);
        if !required_path.exists() {
            violations.push(format!(
                "gap-decision ledger corpus is missing {}",
                normalize_path(&required_path)
            ));
        }
    }
    if !path.exists() {
        return Ok(());
    }

    let corpus = match read_json_value(path) {
        Ok(value) => value,
        Err(err) => {
            violations.push(err);
            return Ok(());
        }
    };
    validate_gap_decision_ledger_corpus_value(path, &corpus, violations);
    Ok(())
}

pub(crate) fn validate_gap_decision_ledger_corpus_value(
    path: &Path,
    corpus: &Value,
    violations: &mut Vec<String>,
) {
    let normalized = normalize_path(path);
    if json_string_field(corpus, "kind").as_deref() != Some("gap_decision_ledger_corpus") {
        violations.push(format!(
            "{normalized} kind must be gap_decision_ledger_corpus"
        ));
    }
    if json_string_field(corpus, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!("{normalized} schema_version must be 0.1"));
    }
    if json_string_field(corpus, "spec").as_deref() != Some("RIPR-SPEC-0046") {
        violations.push(format!("{normalized} spec must be RIPR-SPEC-0046"));
    }
    if json_string_field(corpus, "proposal").as_deref() != Some("RIPR-PROP-0006") {
        violations.push(format!("{normalized} proposal must be RIPR-PROP-0006"));
    }

    require_string_array_contains_all(
        corpus,
        "required_gap_kinds",
        GAP_DECISION_REQUIRED_KINDS,
        "Gap decision ledger",
        violations,
    );
    require_string_array_contains_all(
        corpus,
        "required_scopes",
        GAP_DECISION_REQUIRED_SCOPES,
        "Gap decision ledger",
        violations,
    );
    require_string_array_contains_all(
        corpus,
        "required_policy_states",
        GAP_DECISION_REQUIRED_POLICY_STATES,
        "Gap decision ledger",
        violations,
    );
    require_string_array_contains_all(
        corpus,
        "required_repairability",
        GAP_DECISION_REQUIRED_REPAIRABILITY,
        "Gap decision ledger",
        violations,
    );

    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        violations.push(format!("{normalized} is missing cases array"));
        return;
    };

    let mut seen_ids = BTreeSet::new();
    let mut seen_kinds = BTreeSet::new();
    let mut seen_scopes = BTreeSet::new();
    let mut seen_policy_states = BTreeSet::new();
    let mut seen_repairability = BTreeSet::new();
    let mut has_pr_comment_eligible = false;
    let mut has_gate_candidate = false;
    let mut has_ripr_zero_target = false;
    let mut has_output_contract_gap = false;
    let mut has_preview_ineligible = false;
    let mut has_missing_artifact = false;
    let mut has_receipt_improved = false;
    let mut has_receipt_unchanged = false;

    for case in cases {
        let case_id = json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string());
        if !seen_ids.insert(case_id.clone()) {
            violations.push(format!("gap-decision ledger case {case_id} is duplicated"));
        }
        validate_gap_decision_ledger_case(
            &case_id,
            case,
            &mut seen_kinds,
            &mut seen_scopes,
            &mut seen_policy_states,
            &mut seen_repairability,
            &mut has_pr_comment_eligible,
            &mut has_gate_candidate,
            &mut has_ripr_zero_target,
            &mut has_output_contract_gap,
            &mut has_preview_ineligible,
            &mut has_missing_artifact,
            &mut has_receipt_improved,
            &mut has_receipt_unchanged,
            violations,
        );
    }

    for required in GAP_DECISION_REQUIRED_KINDS {
        if !seen_kinds.contains(*required) {
            violations.push(format!(
                "gap-decision ledger is missing gap kind {required}"
            ));
        }
    }
    for required in GAP_DECISION_REQUIRED_SCOPES {
        if !seen_scopes.contains(*required) {
            violations.push(format!("gap-decision ledger is missing scope {required}"));
        }
    }
    for required in GAP_DECISION_REQUIRED_POLICY_STATES {
        if !seen_policy_states.contains(*required) {
            violations.push(format!(
                "gap-decision ledger is missing policy_state {required}"
            ));
        }
    }
    for required in GAP_DECISION_REQUIRED_REPAIRABILITY {
        if !seen_repairability.contains(*required) {
            violations.push(format!(
                "gap-decision ledger is missing repairability {required}"
            ));
        }
    }
    if !has_pr_comment_eligible {
        violations.push(
            "gap-decision ledger must include a PR-comment-eligible repair card case".to_string(),
        );
    }
    if !has_gate_candidate {
        violations.push("gap-decision ledger must include a safe gate-candidate case".to_string());
    }
    if !has_ripr_zero_target {
        violations.push("gap-decision ledger must include a RIPR Zero target case".to_string());
    }
    if !has_output_contract_gap {
        violations
            .push("gap-decision ledger must include a MissingOutputContract case".to_string());
    }
    if !has_preview_ineligible {
        violations.push("gap-decision ledger must include a preview-ineligible case".to_string());
    }
    if !has_missing_artifact {
        violations.push("gap-decision ledger must include a missing-artifact case".to_string());
    }
    if !has_receipt_improved {
        violations.push("gap-decision ledger must include an improved receipt case".to_string());
    }
    if !has_receipt_unchanged {
        violations.push(
            "gap-decision ledger must include an unchanged-after-attempt receipt case".to_string(),
        );
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "fixture corpus guard tracks independent coverage flags explicitly"
)]
fn validate_gap_decision_ledger_case(
    case_id: &str,
    case: &Value,
    seen_kinds: &mut BTreeSet<String>,
    seen_scopes: &mut BTreeSet<String>,
    seen_policy_states: &mut BTreeSet<String>,
    seen_repairability: &mut BTreeSet<String>,
    has_pr_comment_eligible: &mut bool,
    has_gate_candidate: &mut bool,
    has_ripr_zero_target: &mut bool,
    has_output_contract_gap: &mut bool,
    has_preview_ineligible: &mut bool,
    has_missing_artifact: &mut bool,
    has_receipt_improved: &mut bool,
    has_receipt_unchanged: &mut bool,
    violations: &mut Vec<String>,
) {
    for field in ["id", "description", "expected_claim"] {
        require_gap_decision_json_string_at(case, field, case_id, violations);
    }
    require_gap_decision_non_empty_string_array_at(case, "source_artifacts", case_id, violations);
    require_gap_decision_non_empty_string_array_at(case, "must_not_claim", case_id, violations);

    let Some(record @ Value::Object(_)) = case.get("expected_gap_record") else {
        violations.push(format!(
            "gap-decision ledger case {case_id} is missing expected_gap_record object"
        ));
        return;
    };

    for field in [
        "gap_id",
        "canonical_gap_id",
        "kind",
        "language",
        "language_status",
        "scope",
        "evidence_class",
        "gap_state",
        "policy_state",
        "repairability",
    ] {
        require_gap_decision_json_string_at(record, field, case_id, violations);
    }
    require_gap_decision_non_empty_string_array_at(record, "evidence_ids", case_id, violations);
    require_gap_decision_non_empty_string_array_at(
        record,
        "verification_commands",
        case_id,
        violations,
    );

    let kind = json_string_field(record, "kind");
    let language = json_string_field(record, "language");
    let language_status = json_string_field(record, "language_status");
    let scope = json_string_field(record, "scope");
    let evidence_class = json_string_field(record, "evidence_class");
    let policy_state = json_string_field(record, "policy_state");
    let repairability = json_string_field(record, "repairability");
    let route_kind = audit_string(record, &["repair_route", "route_kind"]);

    gap_decision_track_allowed(
        case_id,
        "kind",
        kind.as_deref(),
        GAP_DECISION_REQUIRED_KINDS,
        seen_kinds,
        violations,
    );
    gap_decision_track_allowed(
        case_id,
        "scope",
        scope.as_deref(),
        GAP_DECISION_REQUIRED_SCOPES,
        seen_scopes,
        violations,
    );
    gap_decision_track_allowed(
        case_id,
        "policy_state",
        policy_state.as_deref(),
        GAP_DECISION_REQUIRED_POLICY_STATES,
        seen_policy_states,
        violations,
    );
    gap_decision_track_allowed(
        case_id,
        "repairability",
        repairability.as_deref(),
        GAP_DECISION_REQUIRED_REPAIRABILITY,
        seen_repairability,
        violations,
    );

    if !matches!(record.get("projection_eligibility"), Some(Value::Object(_))) {
        violations.push(format!(
            "gap-decision ledger case {case_id} is missing projection_eligibility object"
        ));
    }
    for projection in [
        "ci_summary",
        "report_packet",
        "pr_comment",
        "lsp_diagnostic",
        "agent_packet",
        "gate_candidate",
        "ripr_zero_count",
        "ripr_plus_count",
    ] {
        validate_gap_projection(case_id, record, projection, violations);
    }

    if repairability.as_deref() == Some("repairable") {
        if !matches!(record.get("repair_route"), Some(Value::Object(_))) {
            violations.push(format!(
                "gap-decision ledger repairable case {case_id} is missing repair_route object"
            ));
        }
        if route_kind.is_none() {
            violations.push(format!(
                "gap-decision ledger repairable case {case_id} is missing repair_route.route_kind"
            ));
        }
    }

    if projection_eligible(record, "pr_comment") == Some(true) {
        *has_pr_comment_eligible = true;
        if language.as_deref() != Some("rust") || language_status.as_deref() != Some("stable") {
            violations.push(format!(
                "gap-decision ledger PR-comment case {case_id} must be stable Rust"
            ));
        }
        if scope.as_deref() != Some("pr_local") || repairability.as_deref() != Some("repairable") {
            violations.push(format!(
                "gap-decision ledger PR-comment case {case_id} must be PR-local and repairable"
            ));
        }
        if audit_string(record, &["anchor", "dedupe_fingerprint"]).is_none() {
            violations.push(format!(
                "gap-decision ledger PR-comment case {case_id} is missing anchor.dedupe_fingerprint"
            ));
        }
        if route_kind.is_none() {
            violations.push(format!(
                "gap-decision ledger PR-comment case {case_id} is missing repair route"
            ));
        }
    }

    if projection_eligible(record, "gate_candidate") == Some(true) {
        *has_gate_candidate = true;
        validate_gap_decision_safe_gate_candidate(
            case_id,
            record,
            language.as_deref(),
            language_status.as_deref(),
            scope.as_deref(),
            policy_state.as_deref(),
            repairability.as_deref(),
            route_kind.as_deref(),
            violations,
        );
    }

    if projection_eligible(record, "ripr_zero_count") == Some(true) {
        *has_ripr_zero_target = true;
        if language.as_deref() != Some("rust")
            || scope.as_deref() != Some("repo_scoped")
            || matches!(policy_state.as_deref(), Some("waived" | "suppressed"))
        {
            violations.push(format!(
                "gap-decision ledger RIPR Zero case {case_id} must be repo-scoped unresolved Rust policy debt"
            ));
        }
    }

    if kind.as_deref() == Some("MissingOutputContract") {
        *has_output_contract_gap = true;
        if evidence_class.as_deref() != Some("presentation_text") {
            violations.push(format!(
                "gap-decision ledger MissingOutputContract case {case_id} must use presentation_text evidence"
            ));
        }
        if !matches!(
            route_kind.as_deref(),
            Some("AddOutputGolden" | "AddHelpOutputSnapshot" | "AddReportRenderGolden")
        ) {
            violations.push(format!(
                "gap-decision ledger MissingOutputContract case {case_id} must route to output/golden repair"
            ));
        }
    }

    if language_status.as_deref() == Some("preview") {
        *has_preview_ineligible = true;
        for projection in ["gate_candidate", "ripr_zero_count", "ripr_plus_count"] {
            if projection_eligible(record, projection) == Some(true) {
                violations.push(format!(
                    "gap-decision ledger preview case {case_id} must not be eligible for {projection}"
                ));
            }
        }
    }

    if case.get("static_unknown_only").and_then(Value::as_bool) == Some(true)
        && (repairability.as_deref() == Some("repairable")
            || projection_eligible(record, "pr_comment") == Some(true)
            || projection_eligible(record, "gate_candidate") == Some(true))
    {
        violations.push(format!(
            "gap-decision ledger static-unknown-only case {case_id} must stay report-only unless a repair route exists"
        ));
    }

    if matches!(
        policy_state.as_deref(),
        Some("baseline_known" | "waived" | "suppressed" | "acknowledged")
    ) && projection_eligible(record, "gate_candidate") == Some(true)
    {
        violations.push(format!(
            "gap-decision ledger policy-overlay case {case_id} must not be gate-candidate eligible"
        ));
    }
    if matches!(policy_state.as_deref(), Some("waived" | "suppressed"))
        && projection_eligible(record, "ripr_zero_count") == Some(true)
    {
        violations.push(format!(
            "gap-decision ledger waived/suppressed case {case_id} must not count toward RIPR Zero"
        ));
    }

    if scope.as_deref() == Some("artifact_missing") {
        *has_missing_artifact = true;
        require_gap_decision_non_empty_string_array_at(
            record,
            "regeneration_commands",
            case_id,
            violations,
        );
        if projection_eligible(record, "pr_comment") == Some(true)
            || projection_eligible(record, "gate_candidate") == Some(true)
        {
            violations.push(format!(
                "gap-decision ledger missing-artifact case {case_id} must not be PR-comment or gate eligible"
            ));
        }
    }

    if let Some(movement) = audit_string(record, &["receipt", "movement"]) {
        match movement.as_str() {
            "improved" => *has_receipt_improved = true,
            "unchanged_after_attempt" => *has_receipt_unchanged = true,
            "resolved" | "worsened" | "missing_receipt" | "not_applicable" => {}
            other => violations.push(format!(
                "gap-decision ledger case {case_id} has unsupported receipt movement {other}"
            )),
        }
    }
}

fn gap_decision_track_allowed(
    case_id: &str,
    field: &str,
    value: Option<&str>,
    allowed: &[&str],
    seen: &mut BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    match value {
        Some(value) if allowed.contains(&value) => {
            seen.insert(value.to_string());
        }
        Some(value) => violations.push(format!(
            "gap-decision ledger case {case_id} has unsupported {field} {value}"
        )),
        None => violations.push(format!(
            "gap-decision ledger case {case_id} is missing {field}"
        )),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "safe gate predicate guard receives normalized case fields"
)]
fn validate_gap_decision_safe_gate_candidate(
    case_id: &str,
    record: &Value,
    language: Option<&str>,
    language_status: Option<&str>,
    scope: Option<&str>,
    policy_state: Option<&str>,
    repairability: Option<&str>,
    route_kind: Option<&str>,
    violations: &mut Vec<String>,
) {
    if language != Some("rust") || language_status != Some("stable") {
        violations.push(format!(
            "gap-decision ledger gate candidate {case_id} must be stable Rust"
        ));
    }
    if scope != Some("pr_local") {
        violations.push(format!(
            "gap-decision ledger gate candidate {case_id} must be PR-local"
        ));
    }
    if !matches!(policy_state, Some("new" | "blocked")) {
        violations.push(format!(
            "gap-decision ledger gate candidate {case_id} must be new or blocked policy state"
        ));
    }
    if repairability != Some("repairable") || route_kind.is_none() {
        violations.push(format!(
            "gap-decision ledger gate candidate {case_id} must have repairable route"
        ));
    }
    if audit_get(record, &["safe_gate_predicate", "policy_target_enabled"]).and_then(Value::as_bool)
        != Some(true)
    {
        violations.push(format!(
            "gap-decision ledger gate candidate {case_id} must set safe_gate_predicate.policy_target_enabled=true"
        ));
    }
    for forbidden in [
        "suppressed",
        "waived",
        "acknowledged_only",
        "baseline_known",
        "preview_language",
        "static_unknown_only",
    ] {
        if audit_get(record, &["safe_gate_predicate", forbidden]).and_then(Value::as_bool)
            != Some(false)
        {
            violations.push(format!(
                "gap-decision ledger gate candidate {case_id} must set safe_gate_predicate.{forbidden}=false"
            ));
        }
    }
}

fn validate_gap_projection(
    case_id: &str,
    record: &Value,
    projection: &str,
    violations: &mut Vec<String>,
) {
    let Some(value @ Value::Object(_)) = audit_get(record, &["projection_eligibility", projection])
    else {
        violations.push(format!(
            "gap-decision ledger case {case_id} is missing projection_eligibility.{projection}"
        ));
        return;
    };
    if value.get("eligible").and_then(Value::as_bool).is_none() {
        violations.push(format!(
            "gap-decision ledger case {case_id} projection {projection} is missing eligible boolean"
        ));
    }
    if json_string_field(value, "reason").is_none() {
        violations.push(format!(
            "gap-decision ledger case {case_id} projection {projection} is missing reason"
        ));
    }
}

fn projection_eligible(record: &Value, projection: &str) -> Option<bool> {
    audit_get(record, &["projection_eligibility", projection, "eligible"]).and_then(Value::as_bool)
}

fn require_gap_decision_json_string_at(
    value: &Value,
    field: &str,
    case_id: &str,
    violations: &mut Vec<String>,
) {
    if json_string_field(value, field).is_none() {
        violations.push(format!(
            "gap-decision ledger case {case_id} is missing string field {field}"
        ));
    }
}

fn require_gap_decision_non_empty_string_array_at(
    value: &Value,
    field: &str,
    case_id: &str,
    violations: &mut Vec<String>,
) {
    match value.get(field) {
        Some(Value::Array(items))
            if !items.is_empty() && items.iter().all(|item| item.as_str().is_some()) => {}
        _ => violations.push(format!(
            "gap-decision ledger case {case_id} {field} must be a non-empty string array"
        )),
    }
}
