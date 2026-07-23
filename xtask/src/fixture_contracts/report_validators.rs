//! Report-surface fixture-corpus validators for `check-fixture-contracts`:
//! the assistant-loop-health, pr-review-front-panel, report-packet-index,
//! and pr-inline-comment-publisher corpora.
//!
//! Extracted verbatim from `main.rs` as a behavior-preserving decomposition
//! slice of #2119. Items referenced outside this module are `pub(crate)` and
//! re-exported from `main.rs` so existing call sites (`dispatch.rs`,
//! `dogfood.rs`, and `tests.rs`) compile unchanged.

use super::*;

pub(crate) fn validate_assistant_loop_health_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let base = Path::new("fixtures/boundary_gap/expected/assistant-loop-health");
    validate_assistant_loop_health_fixture_corpus_at(base, violations)
}

pub(crate) fn validate_assistant_loop_health_fixture_corpus_at(
    base: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !base.exists() {
        return Ok(());
    }

    for required in ["README.md", "corpus.json"] {
        let path = base.join(required);
        if !path.exists() {
            violations.push(format!(
                "assistant-loop-health corpus is missing {}",
                normalize_path(&path)
            ));
        }
    }

    let corpus_path = base.join("corpus.json");
    if !corpus_path.exists() {
        return Ok(());
    }

    let corpus = match read_json_value(&corpus_path) {
        Ok(value) => value,
        Err(err) => {
            violations.push(err);
            return Ok(());
        }
    };
    if json_string_field(&corpus, "kind").as_deref() != Some("assistant_loop_health_corpus") {
        violations.push(
            "assistant-loop-health corpus kind must be assistant_loop_health_corpus".to_string(),
        );
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0022") {
        violations.push("assistant-loop-health corpus spec must be RIPR-SPEC-0022".to_string());
    }

    let cases = match corpus.get("cases").and_then(Value::as_array) {
        Some(cases) => cases,
        None => {
            violations.push("assistant-loop-health corpus is missing cases array".to_string());
            return Ok(());
        }
    };

    let required_cases = [
        "complete_improved",
        "partial_missing_optional",
        "missing_required_input",
        "unchanged_after_attempt",
        "regressed_after_attempt",
        "warning_heavy",
        "multi_proof",
    ];
    let mut seen_cases = BTreeSet::new();

    for case in cases {
        let case_id = json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string());
        seen_cases.insert(case_id.clone());

        let expected = match case.get("expected") {
            Some(value) => value,
            None => {
                violations.push(format!(
                    "assistant-loop-health case {case_id} is missing expected"
                ));
                continue;
            }
        };
        let expected_status =
            json_string_field(expected, "status").unwrap_or_else(|| "missing".to_string());

        let report_path = match json_string_field(case, "expected_report") {
            Some(path) => path,
            None => {
                violations.push(format!(
                    "assistant-loop-health case {case_id} is missing expected_report"
                ));
                continue;
            }
        };
        let markdown_path = match json_string_field(case, "expected_markdown") {
            Some(path) => path,
            None => {
                violations.push(format!(
                    "assistant-loop-health case {case_id} is missing expected_markdown"
                ));
                continue;
            }
        };

        if let Some(proofs) = case.get("proofs").and_then(Value::as_array) {
            if proofs.is_empty() {
                violations.push(format!(
                    "assistant-loop-health case {case_id} must name at least one proof input"
                ));
            }
            for proof in proofs {
                match proof.as_str() {
                    Some(path) if Path::new(path).exists() => {}
                    Some(path) => violations.push(format!(
                        "assistant-loop-health case {case_id} proof input is missing {path}"
                    )),
                    None => violations.push(format!(
                        "assistant-loop-health case {case_id} has a non-string proof path"
                    )),
                }
            }
        } else {
            violations.push(format!(
                "assistant-loop-health case {case_id} is missing proofs array"
            ));
        }

        let report = match read_json_value(Path::new(&report_path)) {
            Ok(value) => value,
            Err(err) => {
                violations.push(format!("assistant-loop-health case {case_id}: {err}"));
                continue;
            }
        };
        if json_string_field(&report, "kind").as_deref() != Some("assistant_loop_health") {
            violations.push(format!(
                "assistant-loop-health case {case_id} report kind must be assistant_loop_health"
            ));
        }
        if json_string_field(&report, "status").as_deref() != Some(expected_status.as_str()) {
            violations.push(format!(
                "assistant-loop-health case {case_id} expected status {expected_status}"
            ));
        }
        if serde_json::to_string(&report)
            .map(|text| text.contains("\"static_class\""))
            .unwrap_or(false)
        {
            violations.push(format!(
                "assistant-loop-health case {case_id} report must use grip_class, not static_class"
            ));
        }
        validate_assistant_loop_health_count(violations, &case_id, expected, &report, "proofs");
        for key in [
            "complete",
            "partial",
            "missing_required_input",
            "missing_optional_input",
            "improved",
            "unchanged",
            "regressed",
            "unknown_movement",
            "warnings",
            "repair_queue",
        ] {
            validate_assistant_loop_health_count(violations, &case_id, expected, &report, key);
        }
        if json_usize_field(expected, "repair_queue").unwrap_or(0) > 0
            && !report
                .get("repair_queue")
                .and_then(Value::as_array)
                .is_some_and(|items| {
                    items
                        .iter()
                        .all(|item| json_string_field(item, "repair_kind").is_some())
                })
        {
            violations.push(format!(
                "assistant-loop-health case {case_id} repair_queue entries must include repair_kind"
            ));
        }
        if !report
            .get("limits")
            .and_then(Value::as_array)
            .is_some_and(|limits| {
                limits
                    .iter()
                    .any(|limit| limit.as_str() == Some("Static RIPR evidence only."))
            })
        {
            violations.push(format!(
                "assistant-loop-health case {case_id} report is missing static evidence limit"
            ));
        }

        let markdown = match fs::read_to_string(&markdown_path) {
            Ok(markdown) => markdown,
            Err(err) => {
                violations.push(format!(
                    "assistant-loop-health case {case_id} Markdown missing {}: {err}",
                    markdown_path
                ));
                continue;
            }
        };
        if !markdown.contains(&format!("Status: {expected_status}")) {
            violations.push(format!(
                "assistant-loop-health case {case_id} Markdown must pin status {expected_status}"
            ));
        }
        if json_usize_field(expected, "repair_queue").unwrap_or(0) > 0
            && ![
                "regenerate_proof",
                "regenerate_missing_artifact",
                "rerun_verify_and_receipt",
                "refresh_before_after_evidence",
                "inspect_unchanged_attempt",
                "inspect_regression",
                "inspect_summary_only_guidance",
                "attach_receipt",
                "no_repair",
            ]
            .iter()
            .any(|repair_kind| markdown.contains(repair_kind))
        {
            violations.push(format!(
                "assistant-loop-health case {case_id} Markdown repair queue must include repair_kind"
            ));
        }
    }

    for required in required_cases {
        if !seen_cases.contains(required) {
            violations.push(format!(
                "assistant-loop-health corpus is missing required case {required}"
            ));
        }
    }

    Ok(())
}

fn validate_assistant_loop_health_count(
    violations: &mut Vec<String>,
    case_id: &str,
    expected: &Value,
    report: &Value,
    key: &str,
) {
    let Some(expected_count) = json_usize_field(expected, key) else {
        violations.push(format!(
            "assistant-loop-health case {case_id} expected is missing {key}"
        ));
        return;
    };
    let actual_count = json_summary_count(report, key);
    if actual_count != expected_count {
        violations.push(format!(
            "assistant-loop-health case {case_id} expected {key}={expected_count}, got {actual_count}"
        ));
    }
}

pub(crate) fn validate_pr_review_front_panel_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let base = Path::new("fixtures/boundary_gap/expected/pr-review-front-panel");
    validate_pr_review_front_panel_fixture_corpus_at(base, violations)
}

pub(crate) fn validate_pr_review_front_panel_fixture_corpus_at(
    base: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !base.exists() {
        violations.push(format!(
            "pr-review-front-panel corpus is missing {}",
            normalize_path(base)
        ));
        return Ok(());
    }

    for required in ["README.md", "corpus.json"] {
        let path = base.join(required);
        if !path.exists() {
            violations.push(format!(
                "pr-review-front-panel corpus is missing {}",
                normalize_path(&path)
            ));
        }
    }

    let corpus_path = base.join("corpus.json");
    if !corpus_path.exists() {
        return Ok(());
    }

    let corpus = match read_json_value(&corpus_path) {
        Ok(value) => value,
        Err(err) => {
            violations.push(err);
            return Ok(());
        }
    };
    if json_string_field(&corpus, "kind").as_deref() != Some("pr_review_front_panel_corpus") {
        violations.push(
            "pr-review-front-panel corpus kind must be pr_review_front_panel_corpus".to_string(),
        );
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0023") {
        violations.push("pr-review-front-panel corpus spec must be RIPR-SPEC-0023".to_string());
    }

    let cases = match corpus.get("cases").and_then(Value::as_array) {
        Some(cases) => cases,
        None => {
            violations.push("pr-review-front-panel corpus is missing cases array".to_string());
            return Ok(());
        }
    };

    let required_cases = [
        "advisory_only",
        "actionable",
        "summary_only",
        "acknowledged",
        "suppressed",
        "baseline_resolved",
        "blocked",
        "missing_proof",
        "coverage_flat_grip_improved",
    ];
    let mut seen_cases = BTreeSet::new();

    for case in cases {
        let case_id = json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string());
        seen_cases.insert(case_id.clone());

        let expected = match case.get("expected") {
            Some(value) => value,
            None => {
                violations.push(format!(
                    "pr-review-front-panel case {case_id} is missing expected"
                ));
                continue;
            }
        };
        let expected_status =
            json_string_field(expected, "status").unwrap_or_else(|| "missing".to_string());

        let report_path = match json_string_field(case, "expected_report") {
            Some(path) => path,
            None => {
                violations.push(format!(
                    "pr-review-front-panel case {case_id} is missing expected_report"
                ));
                continue;
            }
        };
        let markdown_path = match json_string_field(case, "expected_markdown") {
            Some(path) => path,
            None => {
                violations.push(format!(
                    "pr-review-front-panel case {case_id} is missing expected_markdown"
                ));
                continue;
            }
        };

        let report = match read_json_value(Path::new(&report_path)) {
            Ok(value) => value,
            Err(err) => {
                violations.push(format!("pr-review-front-panel case {case_id}: {err}"));
                continue;
            }
        };
        if json_string_field(&report, "kind").as_deref() != Some("pr_review_front_panel") {
            violations.push(format!(
                "pr-review-front-panel case {case_id} report kind must be pr_review_front_panel"
            ));
        }
        if json_string_field(&report, "status").as_deref() != Some(expected_status.as_str()) {
            violations.push(format!(
                "pr-review-front-panel case {case_id} expected status {expected_status}"
            ));
        }

        for key in [
            "top_issue_state",
            "policy_state",
            "placement",
            "movement_state",
            "coverage_grip_state",
        ] {
            validate_pr_review_front_panel_summary_string(
                violations, &case_id, expected, &report, key,
            );
        }
        for key in [
            "new_policy_eligible",
            "baseline_resolved",
            "blocking_candidates",
            "warnings",
        ] {
            validate_pr_review_front_panel_summary_count(
                violations, &case_id, expected, &report, key,
            );
        }

        let artifact_groups_are_valid = report
            .get("artifacts")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                !items.is_empty()
                    && items.iter().all(|item| {
                        matches!(
                            json_string_field(item, "group").as_deref(),
                            Some(
                                "start_here"
                                    | "repair"
                                    | "evidence"
                                    | "policy"
                                    | "calibration"
                                    | "generated_ci"
                            )
                        )
                    })
            });
        if !artifact_groups_are_valid {
            violations.push(format!(
                "pr-review-front-panel case {case_id} artifacts must use known groups"
            ));
        }

        if !report
            .get("limits")
            .and_then(Value::as_array)
            .is_some_and(|limits| {
                limits
                    .iter()
                    .any(|limit| limit.as_str() == Some("Static RIPR evidence only."))
            })
        {
            violations.push(format!(
                "pr-review-front-panel case {case_id} report is missing static evidence limit"
            ));
        }
        if let Some(top_issue) = report.get("top_issue").filter(|value| !value.is_null()) {
            if json_string_field(top_issue, "static_evidence_boundary").as_deref()
                != Some(FIRST_PR_STATIC_EVIDENCE_BOUNDARY)
            {
                violations.push(format!(
                    "pr-review-front-panel case {case_id} top_issue must mirror first-pr static evidence boundary"
                ));
            }
            if json_string_field(top_issue, "current_evidence_strength").is_none() {
                violations.push(format!(
                    "pr-review-front-panel case {case_id} top_issue is missing current_evidence_strength"
                ));
            }
        }

        let markdown = match fs::read_to_string(&markdown_path) {
            Ok(markdown) => markdown,
            Err(err) => {
                violations.push(format!(
                    "pr-review-front-panel case {case_id} Markdown missing {}: {err}",
                    markdown_path
                ));
                continue;
            }
        };
        if !markdown.contains("# RIPR PR Review") {
            violations.push(format!(
                "pr-review-front-panel case {case_id} Markdown must use the PR review heading"
            ));
        }
        if !markdown.contains(&format!("Status: {expected_status}")) {
            violations.push(format!(
                "pr-review-front-panel case {case_id} Markdown must pin status {expected_status}"
            ));
        }
        if json_usize_field(expected, "blocking_candidates").unwrap_or(0) > 0
            && !markdown.contains("Gate authority:")
        {
            violations.push(format!(
                "pr-review-front-panel case {case_id} blocked Markdown must name gate authority"
            ));
        }
    }

    for required in required_cases {
        if !seen_cases.contains(required) {
            violations.push(format!(
                "pr-review-front-panel corpus is missing required case {required}"
            ));
        }
    }

    Ok(())
}

fn validate_pr_review_front_panel_summary_string(
    violations: &mut Vec<String>,
    case_id: &str,
    expected: &Value,
    report: &Value,
    key: &str,
) {
    let Some(expected_value) = json_string_field(expected, key) else {
        violations.push(format!(
            "pr-review-front-panel case {case_id} expected is missing {key}"
        ));
        return;
    };
    let actual_value = report
        .get("summary")
        .and_then(|summary| json_string_field(summary, key))
        .unwrap_or_else(|| "missing".to_string());
    if actual_value != expected_value {
        violations.push(format!(
            "pr-review-front-panel case {case_id} expected {key}={expected_value}, got {actual_value}"
        ));
    }
}

fn validate_pr_review_front_panel_summary_count(
    violations: &mut Vec<String>,
    case_id: &str,
    expected: &Value,
    report: &Value,
    key: &str,
) {
    let Some(expected_count) = json_usize_field(expected, key) else {
        violations.push(format!(
            "pr-review-front-panel case {case_id} expected is missing {key}"
        ));
        return;
    };
    let actual_count = json_summary_count(report, key);
    if actual_count != expected_count {
        violations.push(format!(
            "pr-review-front-panel case {case_id} expected {key}={expected_count}, got {actual_count}"
        ));
    }
}

pub(crate) fn validate_report_packet_index_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let base = Path::new("fixtures/boundary_gap/expected/report-packet-index");
    validate_report_packet_index_fixture_corpus_at(base, violations)
}

pub(crate) fn validate_report_packet_index_fixture_corpus_at(
    base: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !base.exists() {
        violations.push(format!(
            "report-packet-index corpus is missing {}",
            normalize_path(base)
        ));
        return Ok(());
    }

    for required in ["README.md", "corpus.json"] {
        let path = base.join(required);
        if !path.exists() {
            violations.push(format!(
                "report-packet-index corpus is missing {}",
                normalize_path(&path)
            ));
        }
    }

    let corpus_path = base.join("corpus.json");
    if !corpus_path.exists() {
        return Ok(());
    }

    let corpus = match read_json_value(&corpus_path) {
        Ok(value) => value,
        Err(err) => {
            violations.push(err);
            return Ok(());
        }
    };
    if json_string_field(&corpus, "kind").as_deref() != Some("report_packet_index_corpus") {
        violations
            .push("report-packet-index corpus kind must be report_packet_index_corpus".to_string());
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0024") {
        violations.push("report-packet-index corpus spec must be RIPR-SPEC-0024".to_string());
    }

    let cases = match corpus.get("cases").and_then(Value::as_array) {
        Some(cases) => cases,
        None => {
            violations.push("report-packet-index corpus is missing cases array".to_string());
            return Ok(());
        }
    };

    let required_cases = [
        "complete_packet",
        "sparse_advisory",
        "missing_front_panel",
        "blocked_gate",
        "missing_assistant_proof",
        "missing_receipts",
        "coverage_grip_present",
    ];
    let mut seen_cases = BTreeSet::new();

    for case in cases {
        let case_id = json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string());
        seen_cases.insert(case_id.clone());

        let expected = match case.get("expected") {
            Some(value) => value,
            None => {
                violations.push(format!(
                    "report-packet-index case {case_id} is missing expected"
                ));
                continue;
            }
        };
        let expected_status =
            json_string_field(expected, "status").unwrap_or_else(|| "missing".to_string());

        let report_path = match json_string_field(case, "expected_report") {
            Some(path) => path,
            None => {
                violations.push(format!(
                    "report-packet-index case {case_id} is missing expected_report"
                ));
                continue;
            }
        };
        let markdown_path = match json_string_field(case, "expected_markdown") {
            Some(path) => path,
            None => {
                violations.push(format!(
                    "report-packet-index case {case_id} is missing expected_markdown"
                ));
                continue;
            }
        };

        let report = match read_json_value(Path::new(&report_path)) {
            Ok(value) => value,
            Err(err) => {
                violations.push(format!("report-packet-index case {case_id}: {err}"));
                continue;
            }
        };
        if json_string_field(&report, "schema_version").as_deref() != Some("0.1") {
            violations.push(format!(
                "report-packet-index case {case_id} schema_version must be 0.1"
            ));
        }
        if json_string_field(&report, "kind").as_deref() != Some("report_packet_index") {
            violations.push(format!(
                "report-packet-index case {case_id} report kind must be report_packet_index"
            ));
        }
        if json_string_field(&report, "status").as_deref() != Some(expected_status.as_str()) {
            violations.push(format!(
                "report-packet-index case {case_id} expected status {expected_status}"
            ));
        }
        if !json_bool_summary_field(&report, "advisory").unwrap_or(false) {
            violations.push(format!(
                "report-packet-index case {case_id} summary.advisory must be true"
            ));
        }

        for key in ["missing_expected", "failures", "warnings"] {
            validate_report_packet_index_summary_count(
                violations, &case_id, expected, &report, key,
            );
        }

        let expected_start_here =
            json_bool_field(expected, "start_here_available").unwrap_or(false);
        let actual_start_here = report
            .get("summary")
            .and_then(|summary| summary.get("start_here"))
            .is_some_and(|value| !value.is_null());
        if actual_start_here != expected_start_here {
            violations.push(format!(
                "report-packet-index case {case_id} expected start_here_available={expected_start_here}, got {actual_start_here}"
            ));
        }

        let expected_gate_authority =
            json_bool_field(expected, "gate_authority_present").unwrap_or(false);
        let actual_gate_authority = report
            .get("summary")
            .and_then(|summary| summary.get("gate_authority"))
            .is_some_and(|value| !value.is_null());
        if actual_gate_authority != expected_gate_authority {
            violations.push(format!(
                "report-packet-index case {case_id} expected gate_authority_present={expected_gate_authority}, got {actual_gate_authority}"
            ));
        }

        validate_report_packet_index_groups(violations, &case_id, expected, &report);
        validate_report_packet_index_missing_reasons(violations, &case_id, &report);

        if !report
            .get("limits")
            .and_then(Value::as_array)
            .is_some_and(|limits| {
                limits
                    .iter()
                    .any(|limit| limit.as_str() == Some("Advisory report-packet index only."))
            })
        {
            violations.push(format!(
                "report-packet-index case {case_id} report is missing advisory index limit"
            ));
        }

        let markdown = match fs::read_to_string(&markdown_path) {
            Ok(markdown) => markdown,
            Err(err) => {
                violations.push(format!(
                    "report-packet-index case {case_id} Markdown missing {}: {err}",
                    markdown_path
                ));
                continue;
            }
        };
        if !markdown.contains("# RIPR Report Packet Index") {
            violations.push(format!(
                "report-packet-index case {case_id} Markdown must use the report-packet index heading"
            ));
        }
        if !markdown.contains(&format!("Status: {expected_status}")) {
            violations.push(format!(
                "report-packet-index case {case_id} Markdown must pin status {expected_status}"
            ));
        }
        if expected_start_here && !markdown.contains("Start here:") {
            violations.push(format!(
                "report-packet-index case {case_id} Markdown must name start-here artifacts"
            ));
        }
        if expected_gate_authority && !markdown.contains("Gate authority:") {
            violations.push(format!(
                "report-packet-index case {case_id} Markdown must name gate authority"
            ));
        }
        if json_usize_field(expected, "missing_expected").unwrap_or(0) > 0
            && !markdown.contains("Missing expected")
        {
            violations.push(format!(
                "report-packet-index case {case_id} Markdown must list missing expected artifacts"
            ));
        }
    }

    for required in required_cases {
        if !seen_cases.contains(required) {
            violations.push(format!(
                "report-packet-index corpus is missing required case {required}"
            ));
        }
    }

    Ok(())
}

fn validate_report_packet_index_groups(
    violations: &mut Vec<String>,
    case_id: &str,
    expected: &Value,
    report: &Value,
) {
    let Some(groups) = report.get("groups").and_then(Value::as_array) else {
        violations.push(format!(
            "report-packet-index case {case_id} is missing groups array"
        ));
        return;
    };
    let mut seen_groups = BTreeSet::new();
    for group in groups {
        let group_name = json_string_field(group, "group").unwrap_or_else(|| "missing".to_string());
        if !matches!(
            group_name.as_str(),
            "start_here"
                | "pr_review_story"
                | "repair_agent_handoff"
                | "evidence_movement"
                | "policy_gates"
                | "calibration"
                | "validation_receipts"
                | "sarif_badges"
                | "local_context"
        ) {
            violations.push(format!(
                "report-packet-index case {case_id} groups must use known group vocabulary"
            ));
        }
        seen_groups.insert(group_name);
        let Some(entries) = group.get("entries").and_then(Value::as_array) else {
            violations.push(format!(
                "report-packet-index case {case_id} group entries must be arrays"
            ));
            continue;
        };
        if entries.is_empty() {
            violations.push(format!(
                "report-packet-index case {case_id} group entries must not be empty"
            ));
        }
        for entry in entries {
            if !matches!(
                json_string_field(entry, "status").as_deref(),
                Some(
                    "available"
                        | "missing"
                        | "pass"
                        | "warn"
                        | "fail"
                        | "blocked"
                        | "acknowledged"
                        | "suppressed"
                        | "stale"
                        | "incomplete"
                        | "unreadable"
                        | "not_applicable"
                )
            ) {
                violations.push(format!(
                    "report-packet-index case {case_id} entries must use known status vocabulary"
                ));
            }
        }
    }

    if let Some(required_groups) = expected.get("required_groups").and_then(Value::as_array) {
        for required_group in required_groups {
            let Some(required_group) = required_group.as_str() else {
                violations.push(format!(
                    "report-packet-index case {case_id} expected required_groups must be strings"
                ));
                continue;
            };
            if !seen_groups.contains(required_group) {
                violations.push(format!(
                    "report-packet-index case {case_id} is missing required group {required_group}"
                ));
            }
        }
    } else {
        violations.push(format!(
            "report-packet-index case {case_id} expected is missing required_groups"
        ));
    }
}

fn validate_report_packet_index_missing_reasons(
    violations: &mut Vec<String>,
    case_id: &str,
    report: &Value,
) {
    let Some(missing_expected) = report.get("missing_expected").and_then(Value::as_array) else {
        violations.push(format!(
            "report-packet-index case {case_id} is missing missing_expected array"
        ));
        return;
    };
    for missing in missing_expected {
        if !matches!(
            json_string_field(missing, "reason").as_deref(),
            Some(
                "not_generated"
                    | "input_not_available"
                    | "configured_off"
                    | "missing_required_input"
                    | "stale_upstream"
                    | "unknown"
            )
        ) {
            violations.push(format!(
                "report-packet-index case {case_id} missing_expected entries must use known reasons"
            ));
        }
    }
}

fn validate_report_packet_index_summary_count(
    violations: &mut Vec<String>,
    case_id: &str,
    expected: &Value,
    report: &Value,
    key: &str,
) {
    let Some(expected_count) = json_usize_field(expected, key) else {
        violations.push(format!(
            "report-packet-index case {case_id} expected is missing {key}"
        ));
        return;
    };
    let actual_count = json_summary_count(report, key);
    if actual_count != expected_count {
        violations.push(format!(
            "report-packet-index case {case_id} expected {key}={expected_count}, got {actual_count}"
        ));
    }
}

pub(crate) fn validate_pr_inline_comment_publisher_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let base = Path::new("fixtures/boundary_gap/expected/pr-inline-comment-publisher");
    validate_pr_inline_comment_publisher_fixture_corpus_at(base, violations)
}

pub(crate) fn validate_pr_inline_comment_publisher_fixture_corpus_at(
    base: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !base.exists() {
        violations.push(format!(
            "pr-inline-comment-publisher corpus is missing {}",
            normalize_path(base)
        ));
        return Ok(());
    }

    for required in ["README.md", "corpus.json"] {
        let path = base.join(required);
        if !path.exists() {
            violations.push(format!(
                "pr-inline-comment-publisher corpus is missing {}",
                normalize_path(&path)
            ));
        }
    }

    let corpus_path = base.join("corpus.json");
    if !corpus_path.exists() {
        return Ok(());
    }

    let corpus = match read_json_value(&corpus_path) {
        Ok(value) => value,
        Err(err) => {
            violations.push(err);
            return Ok(());
        }
    };
    if json_string_field(&corpus, "kind").as_deref() != Some("pr_inline_comment_publisher_corpus") {
        violations.push(
            "pr-inline-comment-publisher corpus kind must be pr_inline_comment_publisher_corpus"
                .to_string(),
        );
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0025") {
        violations
            .push("pr-inline-comment-publisher corpus spec must be RIPR-SPEC-0025".to_string());
    }

    let cases = match corpus.get("cases").and_then(Value::as_array) {
        Some(cases) => cases,
        None => {
            violations
                .push("pr-inline-comment-publisher corpus is missing cases array".to_string());
            return Ok(());
        }
    };

    let required_cases = [
        "publishable_changed_line",
        "summary_only_excluded",
        "cap_overflow",
        "dedupe_upsert",
        "stale_existing",
        "fork_or_no_token",
        "missing_input",
    ];
    let mut seen_cases = BTreeSet::new();

    for case in cases {
        let case_id = json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string());
        seen_cases.insert(case_id.clone());

        let expected = match case.get("expected") {
            Some(value) => value,
            None => {
                violations.push(format!(
                    "pr-inline-comment-publisher case {case_id} is missing expected"
                ));
                continue;
            }
        };
        let expected_status =
            json_string_field(expected, "status").unwrap_or_else(|| "missing".to_string());
        let expected_mode =
            json_string_field(expected, "mode").unwrap_or_else(|| "missing".to_string());

        let report_path = match json_string_field(case, "expected_report") {
            Some(path) => path,
            None => {
                violations.push(format!(
                    "pr-inline-comment-publisher case {case_id} is missing expected_report"
                ));
                continue;
            }
        };
        let markdown_path = match json_string_field(case, "expected_markdown") {
            Some(path) => path,
            None => {
                violations.push(format!(
                    "pr-inline-comment-publisher case {case_id} is missing expected_markdown"
                ));
                continue;
            }
        };

        let report = match read_json_value(Path::new(&report_path)) {
            Ok(value) => value,
            Err(err) => {
                violations.push(format!("pr-inline-comment-publisher case {case_id}: {err}"));
                continue;
            }
        };
        if json_string_field(&report, "schema_version").as_deref() != Some("0.1") {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} schema_version must be 0.1"
            ));
        }
        if json_string_field(&report, "kind").as_deref() != Some("pr_inline_comment_publish_plan") {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} report kind must be pr_inline_comment_publish_plan"
            ));
        }
        if json_string_field(&report, "status").as_deref() != Some(expected_status.as_str()) {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} expected status {expected_status}"
            ));
        }
        if json_string_field(&report, "mode").as_deref() != Some(expected_mode.as_str()) {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} expected mode {expected_mode}"
            ));
        }
        if report
            .get("limits")
            .and_then(|limits| json_string_field(limits, "comments_default"))
            .as_deref()
            != Some("off")
        {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} limits.comments_default must be off"
            ));
        }

        for key in ["publishable", "skipped", "blocked"] {
            validate_pr_inline_comment_publisher_summary_count(
                violations, &case_id, expected, &report, key,
            );
        }

        let expected_safe = json_bool_field(expected, "safe_to_publish").unwrap_or(false);
        let actual_safe = report
            .get("summary")
            .and_then(|summary| json_bool_field(summary, "safe_to_publish"))
            .unwrap_or(false);
        if actual_safe != expected_safe {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} expected safe_to_publish={expected_safe}, got {actual_safe}"
            ));
        }

        validate_pr_inline_comment_publisher_operations(violations, &case_id, expected, &report);
        validate_pr_inline_comment_publisher_reasons(
            violations,
            &case_id,
            expected,
            &report,
            "skipped",
            "skip_reason",
            &known_pr_inline_comment_skip_reasons(),
        );
        validate_pr_inline_comment_publisher_reasons(
            violations,
            &case_id,
            expected,
            &report,
            "blocked",
            "blocked_reason",
            &known_pr_inline_comment_blocked_reasons(),
        );

        if !json_string_field(&report, "limits_note")
            .is_some_and(|note| note.contains("Advisory inline-comment publish plan only"))
        {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} report is missing advisory publish-plan limit"
            ));
        }

        let markdown = match fs::read_to_string(&markdown_path) {
            Ok(markdown) => markdown,
            Err(err) => {
                violations.push(format!(
                    "pr-inline-comment-publisher case {case_id} Markdown missing {}: {err}",
                    markdown_path
                ));
                continue;
            }
        };
        if !markdown.contains("# RIPR Inline Comment Publish Plan") {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} Markdown must use the inline comment publish-plan heading"
            ));
        }
        if !markdown.contains(&format!("Mode: {expected_mode}")) {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} Markdown must pin mode {expected_mode}"
            ));
        }
        if !markdown.contains(&format!("Status: {expected_status}")) {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} Markdown must pin status {expected_status}"
            ));
        }
        if json_usize_field(expected, "publishable").unwrap_or(0) > 0
            && !markdown.contains("Planned operations:")
        {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} Markdown must list planned operations"
            ));
        }
        if json_usize_field(expected, "skipped").unwrap_or(0) > 0 && !markdown.contains("Skipped:")
        {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} Markdown must list skipped guidance"
            ));
        }
        if json_usize_field(expected, "blocked").unwrap_or(0) > 0 && !markdown.contains("Blocked:")
        {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} Markdown must list blocked operations"
            ));
        }
        if !markdown.contains("Advisory inline-comment publish plan") {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} Markdown must name advisory publish-plan limit"
            ));
        }
    }

    for required in required_cases {
        if !seen_cases.contains(required) {
            violations.push(format!(
                "pr-inline-comment-publisher corpus is missing required case {required}"
            ));
        }
    }

    Ok(())
}

fn validate_pr_inline_comment_publisher_summary_count(
    violations: &mut Vec<String>,
    case_id: &str,
    expected: &Value,
    report: &Value,
    key: &str,
) {
    let Some(expected_count) = json_usize_field(expected, key) else {
        violations.push(format!(
            "pr-inline-comment-publisher case {case_id} expected is missing {key}"
        ));
        return;
    };
    let actual_count = json_summary_count(report, key);
    if actual_count != expected_count {
        violations.push(format!(
            "pr-inline-comment-publisher case {case_id} expected {key}={expected_count}, got {actual_count}"
        ));
    }
}

fn validate_pr_inline_comment_publisher_operations(
    violations: &mut Vec<String>,
    case_id: &str,
    expected: &Value,
    report: &Value,
) {
    let Some(operations) = report.get("operations").and_then(Value::as_array) else {
        violations.push(format!(
            "pr-inline-comment-publisher case {case_id} is missing operations array"
        ));
        return;
    };
    let mut seen_operations = BTreeSet::new();
    for operation in operations {
        let operation_name =
            json_string_field(operation, "operation").unwrap_or_else(|| "missing".to_string());
        if !known_pr_inline_comment_operations().contains(&operation_name.as_str()) {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} operations must use known operation vocabulary"
            ));
        }
        seen_operations.insert(operation_name.clone());

        if json_string_field(operation, "source_collection").as_deref() == Some("summary_only") {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} operations must not publish summary_only guidance"
            ));
        }
        if matches!(operation_name.as_str(), "create" | "update" | "keep")
            && json_string_field(operation, "source_collection").as_deref() != Some("comments")
        {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} publishable operations must come from comments"
            ));
        }
        if matches!(
            operation_name.as_str(),
            "create" | "update" | "keep" | "delete"
        ) && json_string_field(operation, "dedupe_key").is_none()
        {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} dedupe operations must carry dedupe_key"
            ));
        }
        if matches!(operation_name.as_str(), "create" | "update" | "keep")
            && operation.get("placement").is_none()
        {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} publishable operations must carry placement"
            ));
        }
    }

    for expected_operation in json_string_array_field(expected, "operations") {
        if !seen_operations.contains(&expected_operation) {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} is missing expected operation {expected_operation}"
            ));
        }
    }
}

fn validate_pr_inline_comment_publisher_reasons(
    violations: &mut Vec<String>,
    case_id: &str,
    expected: &Value,
    report: &Value,
    collection: &str,
    reason_key: &str,
    known_reasons: &[&str],
) {
    let Some(items) = report.get(collection).and_then(Value::as_array) else {
        violations.push(format!(
            "pr-inline-comment-publisher case {case_id} is missing {collection} array"
        ));
        return;
    };

    let mut seen_reasons = BTreeSet::new();
    for item in items {
        let reason = json_string_field(item, reason_key).unwrap_or_else(|| "missing".to_string());
        if !known_reasons.contains(&reason.as_str()) {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} {collection} entries must use known {reason_key} vocabulary"
            ));
        }
        seen_reasons.insert(reason);
    }

    let expected_key = if collection == "skipped" {
        "skip_reasons"
    } else {
        "blocked_reasons"
    };
    for expected_reason in json_string_array_field(expected, expected_key) {
        if !seen_reasons.contains(&expected_reason) {
            violations.push(format!(
                "pr-inline-comment-publisher case {case_id} is missing expected {reason_key} {expected_reason}"
            ));
        }
    }
}

fn known_pr_inline_comment_operations() -> [&'static str; 6] {
    ["create", "update", "keep", "delete", "skip", "blocked"]
}

fn known_pr_inline_comment_skip_reasons() -> [&'static str; 7] {
    [
        "mode_off",
        "summary_only",
        "suppressed",
        "inline_comment_cap_reached",
        "unchanged_tests",
        "not_publishable",
        "already_current",
    ]
}

fn known_pr_inline_comment_blocked_reasons() -> [&'static str; 11] {
    [
        "missing_pr_guidance",
        "malformed_pr_guidance",
        "missing_pull_request",
        "missing_token",
        "missing_write_permission",
        "fork_untrusted",
        "unsafe_event",
        "missing_dedupe_key",
        "missing_changed_line_placement",
        "unsupported_mode",
        "unknown",
    ]
}
