use super::markdown::{push_wrapped_paragraph, str_or, with_period};
use super::*;
use crate::output::test_support::{read_file, repo_root};
use std::path::Path;

#[test]
fn first_useful_action_matches_actionable_fixture() -> Result<(), String> {
    let repo_root = repo_root()?;
    let base = repo_root.join("fixtures/boundary_gap/expected/first-useful-action/actionable");
    let proof = repo_root
            .join("fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/test-oracle-assistant-proof.json");
    let pr_guidance = repo_root.join(
        "fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-guidance.json",
    );
    let ledger =
            repo_root.join("fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-evidence-ledger.json");
    let report = build_first_useful_action_report(FirstUsefulActionInput {
        root: "fixtures/boundary_gap/input".to_string(),
        generated_at: "2026-05-09T12:00:00Z".to_string(),
        pr_guidance_path: Some(fixture_path(&repo_root, &pr_guidance)),
        assistant_proof_path: Some(fixture_path(&repo_root, &proof)),
        gap_ledger_path: None,
        ledger_path: Some(fixture_path(&repo_root, &ledger)),
        baseline_delta_path: None,
        receipt_path: None,
        gate_decision_path: None,
        coverage_frontier_path: None,
        editor_context_path: None,
        pr_guidance_json: Some(Ok(read_file(&pr_guidance)?)),
        assistant_proof_json: Some(Ok(read_file(&proof)?)),
        gap_ledger_json: None,
        ledger_json: Some(Ok(read_file(&ledger)?)),
        baseline_delta_json: None,
        receipt_json: None,
        gate_decision_json: None,
        coverage_frontier_json: None,
        editor_context_json: None,
    });

    assert_eq!(
        render_first_useful_action_json(&report)?,
        read_file(&base.join("first-useful-action.json"))?.trim_end()
    );
    assert_eq!(
        render_first_useful_action_markdown(&report),
        read_file(&base.join("first-useful-action.md"))?
    );
    Ok(())
}

#[test]
fn first_useful_action_matches_unchanged_after_attempt_fixture() -> Result<(), String> {
    let repo_root = repo_root()?;
    let base = repo_root
        .join("fixtures/boundary_gap/expected/first-useful-action/unchanged-after-attempt");
    let proof = repo_root
            .join("fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/test-oracle-assistant-proof.json");
    let pr_guidance = repo_root.join(
        "fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-guidance.json",
    );
    let ledger =
            repo_root.join("fixtures/boundary_gap/expected/test-oracle-assistant-loop/canonical/pr-evidence-ledger.json");
    let receipt =
        repo_root.join("fixtures/boundary_gap/expected/editor-agent-loop/agent-receipt.json");
    let report = build_first_useful_action_report(FirstUsefulActionInput {
        root: "fixtures/boundary_gap/input".to_string(),
        generated_at: "2026-05-09T12:00:00Z".to_string(),
        pr_guidance_path: Some(fixture_path(&repo_root, &pr_guidance)),
        assistant_proof_path: Some(fixture_path(&repo_root, &proof)),
        gap_ledger_path: None,
        ledger_path: Some(fixture_path(&repo_root, &ledger)),
        baseline_delta_path: None,
        receipt_path: Some(fixture_path(&repo_root, &receipt)),
        gate_decision_path: None,
        coverage_frontier_path: None,
        editor_context_path: None,
        pr_guidance_json: Some(Ok(read_file(&pr_guidance)?)),
        assistant_proof_json: Some(Ok(read_file(&proof)?)),
        gap_ledger_json: None,
        ledger_json: Some(Ok(read_file(&ledger)?)),
        baseline_delta_json: None,
        receipt_json: Some(Ok(read_file(&receipt)?)),
        gate_decision_json: None,
        coverage_frontier_json: None,
        editor_context_json: None,
    });

    assert_eq!(
        render_first_useful_action_json(&report)?,
        read_file(&base.join("first-useful-action.json"))?.trim_end()
    );
    assert_eq!(
        render_first_useful_action_markdown(&report),
        read_file(&base.join("first-useful-action.md"))?
    );
    Ok(())
}

#[test]
fn first_useful_action_reports_stale_editor_context_first() -> Result<(), String> {
    let report = build_first_useful_action_report(FirstUsefulActionInput {
        root: "fixtures/boundary_gap/input".to_string(),
        generated_at: "2026-05-09T12:00:00Z".to_string(),
        pr_guidance_path: None,
        assistant_proof_path: None,
        gap_ledger_path: None,
        ledger_path: None,
        baseline_delta_path: None,
        receipt_path: None,
        gate_decision_path: None,
        coverage_frontier_path: None,
        editor_context_path: Some("target/ripr/workflow/evidence-context.json".to_string()),
        pr_guidance_json: None,
        assistant_proof_json: None,
        gap_ledger_json: None,
        ledger_json: None,
        baseline_delta_json: None,
        receipt_json: None,
        gate_decision_json: None,
        coverage_frontier_json: None,
        editor_context_json: Some(Ok(r#"{
  "freshness": "stale",
  "stale_reason": "diagnostic generation is older than the latest saved workspace refresh",
  "seam_id": "67fc764ba37d77bd",
  "seam_kind": "predicate_boundary",
  "path": "src/lib.rs",
  "line": 2,
  "classification": "weakly_exposed"
}"#
        .to_string())),
    });
    let rendered = render_first_useful_action_json(&report)?;
    assert!(rendered.contains(r#""status": "stale""#));
    assert!(rendered.contains(r#""action_kind": "refresh_evidence""#));
    assert!(rendered.contains("diagnostic generation is older"));
    Ok(())
}

#[test]
fn first_useful_action_routes_missing_assistant_proof() -> Result<(), String> {
    let report = build_first_useful_action_report(FirstUsefulActionInput {
        root: ".".to_string(),
        generated_at: "2026-05-09T12:00:00Z".to_string(),
        pr_guidance_path: Some("comments.json".to_string()),
        assistant_proof_path: None,
        gap_ledger_path: None,
        ledger_path: Some("ledger.json".to_string()),
        baseline_delta_path: None,
        receipt_path: None,
        gate_decision_path: None,
        coverage_frontier_path: None,
        editor_context_path: None,
        pr_guidance_json: Some(Ok(
            r#"{"comments":[{"seam_id":"seam-a","missing_discriminator":"x == 1"}]}"#.to_string(),
        )),
        assistant_proof_json: None,
        gap_ledger_json: None,
        ledger_json: Some(Ok(r#"{"kind":"pr_evidence_ledger"}"#.to_string())),
        baseline_delta_json: None,
        receipt_json: None,
        gate_decision_json: None,
        coverage_frontier_json: None,
        editor_context_json: None,
    });
    let rendered = render_first_useful_action_json(&report)?;
    assert!(rendered.contains(r#""status": "missing_required_artifact""#));
    assert!(rendered.contains(r#""action_kind": "generate_missing_artifact""#));
    assert!(rendered.contains(DEFAULT_TEST_ORACLE_ASSISTANT_PROOF_OUT));
    Ok(())
}

#[test]
fn first_useful_action_routes_gap_record_without_assistant_proof() -> Result<(), String> {
    let report = build_first_useful_action_report(FirstUsefulActionInput {
        root: ".".to_string(),
        generated_at: "2026-05-09T12:00:00Z".to_string(),
        pr_guidance_path: Some("comments.json".to_string()),
        assistant_proof_path: None,
        gap_ledger_path: Some("gap-decision-ledger.json".to_string()),
        ledger_path: None,
        baseline_delta_path: None,
        receipt_path: None,
        gate_decision_path: None,
        coverage_frontier_path: None,
        editor_context_path: None,
        pr_guidance_json: Some(Ok(
            r#"{"comments":[{"seam_id":"raw-a","classification":"static_unknown"}]}"#.to_string(),
        )),
        assistant_proof_json: None,
        gap_ledger_json: Some(Ok(r#"{
  "kind": "gap_decision_ledger",
  "records": [
    {
      "gap_id": "gap:pr:pricing:threshold-boundary",
      "canonical_gap_id": "gap:rust:pricing:discount:threshold-boundary",
      "kind": "MissingBoundaryAssertion",
      "language": "rust",
      "language_status": "stable",
      "scope": "pr_local",
      "evidence_class": "predicate_boundary",
      "gap_state": "actionable",
      "policy_state": "new",
      "repairability": "repairable",
      "anchor": {
        "file": "src/pricing.rs",
        "line": 42,
        "dedupe_fingerprint": "gap:rust:pricing:discount:threshold-boundary"
      },
      "repair_route": {
        "route_kind": "AddBoundaryAssertion",
        "target_file": "tests/pricing.rs",
        "related_test": "tests/pricing.rs::below_threshold_has_no_discount",
        "assertion_shape": "assert_eq!(discount(100, 100), 90)"
      },
      "verification_commands": [
        "cargo xtask fixtures boundary_gap"
      ]
    }
  ]
}"#
        .to_string())),
        ledger_json: None,
        baseline_delta_json: None,
        receipt_json: None,
        gate_decision_json: None,
        coverage_frontier_json: None,
        editor_context_json: None,
    });
    let rendered = render_first_useful_action_json(&report)?;
    assert!(rendered.contains(r#""source": "gap_ledger""#));
    assert!(rendered.contains(r#""gap_id": "gap:pr:pricing:threshold-boundary""#));
    assert!(rendered.contains(r#""repair_route": "AddBoundaryAssertion""#));
    assert!(rendered.contains(r#""verify": "cargo xtask fixtures boundary_gap""#));
    assert!(!rendered.contains(DEFAULT_TEST_ORACLE_ASSISTANT_PROOF_OUT));
    assert!(!rendered.contains("Confidence"));
    let markdown = render_first_useful_action_markdown(&report);
    assert!(markdown.contains("Repair MissingBoundaryAssertion via AddBoundaryAssertion"));
    assert!(markdown.contains("`cargo xtask fixtures boundary_gap`"));
    Ok(())
}

#[test]
fn first_useful_action_supports_gap_record_shapes_and_output_routes() -> Result<(), String> {
    let raw_array: Value = serde_json::from_str(&format!("[{}]", output_contract_gap_record()))
        .map_err(|err| format!("parse raw gap records: {err}"))?;
    assert_eq!(gap_records(&raw_array).len(), 1);

    let wrapped_gap_records: Value = serde_json::from_str(&format!(
        r#"{{"gap_records":[{}]}}"#,
        output_contract_gap_record()
    ))
    .map_err(|err| format!("parse wrapped gap_records: {err}"))?;
    assert!(first_actionable_gap_record(&wrapped_gap_records).is_some());

    let fixture_cases: Value = serde_json::from_str(&format!(
        r#"{{"cases":[{{"expected_gap_record":{}}}]}}"#,
        output_contract_gap_record()
    ))
    .map_err(|err| format!("parse fixture-style gap records: {err}"))?;
    assert!(first_actionable_gap_record(&fixture_cases).is_some());

    let report = build_first_useful_action_report(FirstUsefulActionInput {
        root: ".".to_string(),
        generated_at: "2026-05-09T12:00:00Z".to_string(),
        pr_guidance_path: None,
        assistant_proof_path: None,
        gap_ledger_path: Some("gap-decision-ledger.json".to_string()),
        ledger_path: None,
        baseline_delta_path: None,
        receipt_path: None,
        gate_decision_path: None,
        coverage_frontier_path: None,
        editor_context_path: None,
        pr_guidance_json: None,
        assistant_proof_json: None,
        gap_ledger_json: Some(Ok(format!(
            r#"{{"gap_records":[{}]}}"#,
            output_contract_gap_record()
        ))),
        ledger_json: None,
        baseline_delta_json: None,
        receipt_json: None,
        gate_decision_json: None,
        coverage_frontier_json: None,
        editor_context_json: None,
    });

    let rendered = render_first_useful_action_json(&report)?;
    assert!(rendered.contains(r#""action_kind": "generate_missing_artifact""#));
    assert!(rendered.contains(r#""repair_route": "AddOutputGolden""#));
    assert!(rendered.contains(r#""verify": "cargo xtask goldens check""#));
    assert!(rendered.contains(r#""file": "fixtures/boundary_gap/expected/output.json""#));
    Ok(())
}

fn output_contract_gap_record() -> &'static str {
    r#"{
  "gap_id": "gap:pr:output:first-action-golden",
  "canonical_gap_id": "gap:rust:output:first-action-golden",
  "kind": "MissingOutputContract",
  "language": "rust",
  "language_status": "stable",
  "scope": "pr_local",
  "evidence_class": "presentation_text",
  "gap_state": "actionable",
  "policy_state": "reintroduced",
  "repairability": "repairable",
  "anchor": {
    "file": "crates/ripr/src/output/human.rs",
    "line": 17,
    "dedupe_fingerprint": "gap:rust:output:first-action-golden"
  },
  "repair_route": {
    "route_kind": "AddOutputGolden",
    "target_file": "fixtures/boundary_gap/expected/output.json",
    "target_line": 1,
    "assertion_shape": "refresh output golden"
  },
  "verification_commands": [
    "cargo xtask goldens check"
  ]
}"#
}

fn fixture_path(repo_root: &Path, path: &Path) -> String {
    match path.strip_prefix(repo_root) {
        Ok(relative) => display_path(relative),
        Err(_) => display_path(path),
    }
}

// ── helper: build a bare-minimum FirstUsefulActionInput with all Nones ──

fn bare_input() -> FirstUsefulActionInput {
    FirstUsefulActionInput {
        root: ".".to_string(),
        generated_at: "2026-01-01T00:00:00Z".to_string(),
        pr_guidance_path: None,
        assistant_proof_path: None,
        gap_ledger_path: None,
        ledger_path: None,
        baseline_delta_path: None,
        receipt_path: None,
        gate_decision_path: None,
        coverage_frontier_path: None,
        editor_context_path: None,
        pr_guidance_json: None,
        assistant_proof_json: None,
        gap_ledger_json: None,
        ledger_json: None,
        baseline_delta_json: None,
        receipt_json: None,
        gate_decision_json: None,
        coverage_frontier_json: None,
        editor_context_json: None,
    }
}

// ── parse_optional_json branches ─────────────────────────────────────────

#[test]
fn parse_optional_json_no_path_returns_none() -> Result<(), String> {
    // When path is None the function immediately returns None without touching
    // ParsedSources – zero warnings, zero read_errors.
    let mut parsed = ParsedSources::default();
    let result =
        parsing::parse_optional_json("label", None, &Some(Ok("{}".to_string())), &mut parsed);
    assert!(result.is_none(), "expected None when path is None");
    assert!(
        parsed.warnings.is_empty(),
        "expected no warnings but got {:?}",
        parsed.warnings
    );
    Ok(())
}

#[test]
fn parse_optional_json_path_but_no_text_records_warning() -> Result<(), String> {
    // path is Some, but text (Option<Result>) is None → warning + read_error
    let mut parsed = ParsedSources::default();
    let result =
        parsing::parse_optional_json("my-label", Some("some/path.json"), &None, &mut parsed);
    assert!(result.is_none(), "expected None when text is absent");
    assert!(
        !(parsed.warnings.is_empty()),
        "expected a warning when text is absent"
    );
    assert!(
        parsed.warnings[0].contains("some/path.json") && parsed.warnings[0].contains("my-label"),
        "unexpected warning text: {}",
        parsed.warnings[0]
    );
    assert!(
        !(parsed.read_errors.is_empty()),
        "expected a read_error entry"
    );
    Ok(())
}

#[test]
fn parse_optional_json_io_error_records_warning() -> Result<(), String> {
    // text is Some(Err(...)) → warning + read_error
    let mut parsed = ParsedSources::default();
    let result = parsing::parse_optional_json(
        "my-label",
        Some("broken.json"),
        &Some(Err("permission denied".to_string())),
        &mut parsed,
    );
    assert!(result.is_none(), "expected None on Err text");
    let warning = parsed.warnings.first().ok_or("expected a warning")?.clone();
    assert!(
        warning.contains("permission denied"),
        "expected error text in warning, got: {warning}"
    );
    Ok(())
}

#[test]
fn parse_optional_json_invalid_json_records_warning() -> Result<(), String> {
    // text is Some(Ok(...)) but not valid JSON → warning + read_error
    let mut parsed = ParsedSources::default();
    let result = parsing::parse_optional_json(
        "my-label",
        Some("bad.json"),
        &Some(Ok("not json {{{".to_string())),
        &mut parsed,
    );
    assert!(result.is_none(), "expected None on invalid JSON");
    assert!(
        !(parsed.warnings.is_empty()),
        "expected warning on invalid JSON"
    );
    assert!(
        !(parsed.read_errors.is_empty()),
        "expected read_error on invalid JSON"
    );
    Ok(())
}

#[test]
fn parse_optional_json_valid_json_returns_value() -> Result<(), String> {
    let mut parsed = ParsedSources::default();
    let result = parsing::parse_optional_json(
        "my-label",
        Some("ok.json"),
        &Some(Ok(r#"{"key": "value"}"#.to_string())),
        &mut parsed,
    );
    let val = result.ok_or("expected Some(Value) on valid JSON")?;
    assert!(
        (val.get("key").and_then(|v| v.as_str()) == Some("value")),
        "unexpected parsed value"
    );
    assert!(
        parsed.warnings.is_empty(),
        "expected no warnings on valid JSON"
    );
    Ok(())
}

// ── generated_at empty / whitespace → DEFAULT_GENERATED_AT ─────────────

#[test]
fn empty_generated_at_uses_default() -> Result<(), String> {
    let mut input = bare_input();
    input.generated_at = "  ".to_string();
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(&format!(r#""generated_at": "{DEFAULT_GENERATED_AT}""#)),
        "expected generated_at to be '{DEFAULT_GENERATED_AT}' in: {rendered}"
    );
    Ok(())
}

// ── no_actionable_report – both warning and no-warning branches ──────────

#[test]
fn no_actionable_report_with_no_inputs_warns() -> Result<(), String> {
    // All inputs None → warning injected
    let report = build_first_useful_action_report(bare_input());
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "no_actionable_seam""#),
        "expected no_actionable_seam status but got: {rendered}"
    );
    assert!(
        rendered.contains("no explicit first-action artifact input was supplied"),
        "expected warning about no inputs"
    );
    Ok(())
}

#[test]
fn no_actionable_report_with_inputs_no_warning() -> Result<(), String> {
    // Some input paths provided but all JSON parses fine to non-actionable content
    let mut input = bare_input();
    input.pr_guidance_path = Some("guidance.json".to_string());
    input.pr_guidance_json = Some(Ok(r#"{"comments":[]}"#.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "no_actionable_seam""#),
        "expected no_actionable_seam"
    );
    assert!(
        !(rendered.contains("no explicit first-action artifact input was supplied")),
        "should not warn when inputs are present"
    );
    Ok(())
}

// ── read_error_report ────────────────────────────────────────────────────

#[test]
fn read_error_triggers_missing_required_report() -> Result<(), String> {
    // Providing a path with no JSON text creates a read_error
    let mut input = bare_input();
    input.pr_guidance_path = Some("guidance.json".to_string());
    input.pr_guidance_json = None; // path given but text not loaded
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "missing_required_artifact""#),
        "expected missing_required_artifact but got: {rendered}"
    );
    assert!(
        rendered.contains("guidance.json"),
        "expected missing path in report"
    );
    Ok(())
}

// ── receipt_report: improved/resolved ────────────────────────────────────

#[test]
fn receipt_improved_routes_already_improved() -> Result<(), String> {
    let receipt_json = r#"{
            "provenance": { "movement": "improved", "seam_id": "seam-abc" }
        }"#;
    let mut input = bare_input();
    input.receipt_path = Some("receipt.json".to_string());
    input.receipt_json = Some(Ok(receipt_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "already_improved""#),
        "expected already_improved status but got: {rendered}"
    );
    Ok(())
}

#[test]
fn receipt_resolved_routes_already_improved() -> Result<(), String> {
    let receipt_json = r#"{
            "provenance": { "movement": "resolved", "seam_id": "seam-xyz" }
        }"#;
    let mut input = bare_input();
    input.receipt_path = Some("receipt.json".to_string());
    input.receipt_json = Some(Ok(receipt_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "already_improved""#),
        "expected already_improved but got: {rendered}"
    );
    Ok(())
}

#[test]
fn receipt_unchanged_routes_unchanged_after_attempt() -> Result<(), String> {
    let receipt_json = r#"{
            "provenance": { "movement": "unchanged", "seam_id": "seam-u" }
        }"#;
    let mut input = bare_input();
    input.receipt_path = Some("receipt.json".to_string());
    input.receipt_json = Some(Ok(receipt_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "unchanged_after_attempt""#),
        "expected unchanged_after_attempt but got: {rendered}"
    );
    Ok(())
}

#[test]
fn receipt_unknown_movement_does_not_route_receipt() -> Result<(), String> {
    // movement = "other" → receipt_report returns None → falls through
    let receipt_json = r#"{"provenance": {"movement": "other"}}"#;
    let mut input = bare_input();
    input.receipt_path = Some("receipt.json".to_string());
    input.receipt_json = Some(Ok(receipt_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        !(rendered.contains(r#""status": "already_improved""#)
            || rendered.contains(r#""status": "unchanged_after_attempt""#)),
        "should not route as receipt for unknown movement: {rendered}"
    );
    Ok(())
}

#[test]
fn receipt_movement_from_seam_change_field() -> Result<(), String> {
    // receipt_movement also reads ["seam"]["change"]
    let receipt_json = r#"{"seam": {"change": "improved"}}"#;
    let mut input = bare_input();
    input.receipt_path = Some("receipt.json".to_string());
    input.receipt_json = Some(Ok(receipt_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "already_improved""#),
        "expected already_improved but got: {rendered}"
    );
    Ok(())
}

// ── suppressed_report ───────────────────────────────────────────────────

#[test]
fn suppressed_guidance_routes_suppressed() -> Result<(), String> {
    let pr_guidance_json = r#"{
            "suppressed": [{"seam_id": "seam-s", "kind": "predicate_boundary"}]
        }"#;
    let mut input = bare_input();
    input.pr_guidance_path = Some("guidance.json".to_string());
    input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "suppressed""#),
        "expected suppressed status but got: {rendered}"
    );
    assert!(
        rendered.contains(r#""action_kind": "no_action""#),
        "expected no_action for suppressed"
    );
    Ok(())
}

#[test]
fn suppressed_guidance_via_warning_text() -> Result<(), String> {
    // has_suppressed_guidance also checks warnings array containing "configured off"
    let pr_guidance_json = r#"{
            "warnings": ["seam configured off by policy"]
        }"#;
    let mut input = bare_input();
    input.pr_guidance_path = Some("guidance.json".to_string());
    input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "suppressed""#),
        "expected suppressed from warning text, got: {rendered}"
    );
    Ok(())
}

#[test]
fn suppressed_guidance_via_warning_text_suppressed_keyword() -> Result<(), String> {
    let pr_guidance_json = r#"{
            "warnings": ["seam is suppressed by ripr policy"]
        }"#;
    let mut input = bare_input();
    input.pr_guidance_path = Some("guidance.json".to_string());
    input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "suppressed""#),
        "expected suppressed from warning text, got: {rendered}"
    );
    Ok(())
}

// ── acknowledged_report via baseline_delta ───────────────────────────────

#[test]
fn acknowledged_bucket_in_baseline_delta_routes_acknowledged() -> Result<(), String> {
    let delta_json = r#"{
            "items": [
                {"bucket": "acknowledged", "path": "src/lib.rs", "line": 10, "kind": "predicate_boundary"}
            ]
        }"#;
    let mut input = bare_input();
    input.baseline_delta_path = Some("delta.json".to_string());
    input.baseline_delta_json = Some(Ok(delta_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "acknowledged""#),
        "expected acknowledged but got: {rendered}"
    );
    Ok(())
}

#[test]
fn acknowledged_via_ledger_movement_count() -> Result<(), String> {
    // No baseline_delta, but ledger has acknowledged count > 0
    let ledger_json = r#"{
            "movement": {"acknowledged": 1},
            "top_repair_route": {"seam_id": "seam-ack"}
        }"#;
    let mut input = bare_input();
    input.ledger_path = Some("ledger.json".to_string());
    input.ledger_json = Some(Ok(ledger_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "acknowledged""#),
        "expected acknowledged via ledger but got: {rendered}"
    );
    Ok(())
}

#[test]
fn zero_acknowledged_in_ledger_does_not_route_acknowledged() -> Result<(), String> {
    let ledger_json = r#"{"movement": {"acknowledged": 0}}"#;
    let mut input = bare_input();
    input.ledger_path = Some("ledger.json".to_string());
    input.ledger_json = Some(Ok(ledger_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        !(rendered.contains(r#""status": "acknowledged""#)),
        "should not route acknowledged when count is 0"
    );
    Ok(())
}

// ── waived_report ────────────────────────────────────────────────────────

#[test]
fn gate_decision_with_waiver_routes_waived() -> Result<(), String> {
    let gate_json = r#"{
            "waiver": {"state": "waived"},
            "seam_id": "seam-w",
            "path": "src/main.rs",
            "line": 5
        }"#;
    let mut input = bare_input();
    input.gate_decision_path = Some("gate.json".to_string());
    input.gate_decision_json = Some(Ok(gate_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "waived""#),
        "expected waived status but got: {rendered}"
    );
    Ok(())
}

#[test]
fn gate_decision_waivers_array_non_empty_routes_waived() -> Result<(), String> {
    let gate_json = r#"{
            "waivers": [{"id": "w1"}],
            "seam_id": "seam-ww"
        }"#;
    let mut input = bare_input();
    input.gate_decision_path = Some("gate.json".to_string());
    input.gate_decision_json = Some(Ok(gate_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "waived""#),
        "expected waived via waivers array but got: {rendered}"
    );
    Ok(())
}

#[test]
fn gate_decision_visible_waiver_routes_waived() -> Result<(), String> {
    let gate_json = r#"{"waiver": "visible"}"#;
    let mut input = bare_input();
    input.gate_decision_path = Some("gate.json".to_string());
    input.gate_decision_json = Some(Ok(gate_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "waived""#),
        "expected waived for visible waiver but got: {rendered}"
    );
    Ok(())
}

#[test]
fn gate_decision_visible_status_does_not_route_waived() -> Result<(), String> {
    let gate_json = r#"{"status": "visible"}"#;
    let mut input = bare_input();
    input.gate_decision_path = Some("gate.json".to_string());
    input.gate_decision_json = Some(Ok(gate_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        !(rendered.contains(r#""status": "waived""#)),
        "visible status should not route waived: {rendered}"
    );
    Ok(())
}

#[test]
fn gate_decision_without_waiver_does_not_route_waived() -> Result<(), String> {
    let gate_json = r#"{"status": "blocking"}"#;
    let mut input = bare_input();
    input.gate_decision_path = Some("gate.json".to_string());
    input.gate_decision_json = Some(Ok(gate_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        !(rendered.contains(r#""status": "waived""#)),
        "should not route waived for non-waived gate"
    );
    Ok(())
}

// ── baseline_only_report ─────────────────────────────────────────────────

#[test]
fn baseline_delta_still_present_routes_baseline_only() -> Result<(), String> {
    let delta_json = r#"{
            "items": [
                {"bucket": "still_present", "path": "src/lib.rs", "line": 20}
            ]
        }"#;
    let mut input = bare_input();
    input.baseline_delta_path = Some("delta.json".to_string());
    input.baseline_delta_json = Some(Ok(delta_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "baseline_only""#),
        "expected baseline_only status but got: {rendered}"
    );
    assert!(
        rendered.contains(r#""action_kind": "acknowledge_baseline""#),
        "expected acknowledge_baseline action"
    );
    Ok(())
}

#[test]
fn baseline_delta_only_bucket_routes_baseline_only() -> Result<(), String> {
    let delta_json = r#"{
            "items": [
                {"bucket": "baseline_only", "path": "src/other.rs", "line": 5}
            ]
        }"#;
    let mut input = bare_input();
    input.baseline_delta_path = Some("delta.json".to_string());
    input.baseline_delta_json = Some(Ok(delta_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "baseline_only""#),
        "expected baseline_only but got: {rendered}"
    );
    Ok(())
}

// ── is_stale variants ────────────────────────────────────────────────────

#[test]
fn is_stale_detects_analysis_stale_status() -> Result<(), String> {
    let ctx_json = r#"{"status": "analysis_stale", "seam_id": "seam-s"}"#;
    let mut input = bare_input();
    input.editor_context_path = Some("ctx.json".to_string());
    input.editor_context_json = Some(Ok(ctx_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "stale""#),
        "expected stale for analysis_stale status but got: {rendered}"
    );
    Ok(())
}

#[test]
fn is_stale_detects_stale_state_field() -> Result<(), String> {
    let ctx_json = r#"{"state": "stale", "seam_id": "seam-s"}"#;
    let mut input = bare_input();
    input.editor_context_path = Some("ctx.json".to_string());
    input.editor_context_json = Some(Ok(ctx_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "stale""#),
        "expected stale from state field but got: {rendered}"
    );
    Ok(())
}

#[test]
fn is_stale_detects_stale_evidence_state_field() -> Result<(), String> {
    let ctx_json = r#"{"evidence_state": "stale", "seam_id": "seam-s"}"#;
    let mut input = bare_input();
    input.editor_context_path = Some("ctx.json".to_string());
    input.editor_context_json = Some(Ok(ctx_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "stale""#),
        "expected stale from evidence_state but got: {rendered}"
    );
    Ok(())
}

#[test]
fn is_stale_detects_bool_stale_field() -> Result<(), String> {
    let ctx_json = r#"{"stale": true, "seam_id": "seam-s"}"#;
    let mut input = bare_input();
    input.editor_context_path = Some("ctx.json".to_string());
    input.editor_context_json = Some(Ok(ctx_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""status": "stale""#),
        "expected stale from bool stale field but got: {rendered}"
    );
    Ok(())
}

#[test]
fn is_stale_with_reason_included_in_warnings() -> Result<(), String> {
    // stale_warnings picks up "reason", "stale_reason", "freshness_reason"
    let ctx_json = r#"{"freshness": "stale", "reason": "outdated cache"}"#;
    let mut input = bare_input();
    input.editor_context_path = Some("ctx.json".to_string());
    input.editor_context_json = Some(Ok(ctx_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains("outdated cache"),
        "expected stale reason in warnings but got: {rendered}"
    );
    Ok(())
}

#[test]
fn is_stale_with_freshness_reason_included_in_warnings() -> Result<(), String> {
    let ctx_json = r#"{"freshness": "stale", "freshness_reason": "file changed"}"#;
    let mut input = bare_input();
    input.editor_context_path = Some("ctx.json".to_string());
    input.editor_context_json = Some(Ok(ctx_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains("file changed"),
        "expected freshness_reason in warnings: {rendered}"
    );
    Ok(())
}

#[test]
fn non_stale_editor_context_does_not_route_stale() -> Result<(), String> {
    let ctx_json = r#"{"freshness": "current", "seam_id": "seam-s"}"#;
    let mut input = bare_input();
    input.editor_context_path = Some("ctx.json".to_string());
    input.editor_context_json = Some(Ok(ctx_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        !(rendered.contains(r#""status": "stale""#)),
        "should not route stale for current freshness"
    );
    Ok(())
}

// ── render_first_useful_action_markdown branches ─────────────────────────

#[test]
fn markdown_includes_why_first_when_non_empty() -> Result<(), String> {
    // The existing actionable fixture exercises why_first – build a simple
    // actionable report from inline JSON so we don't need file I/O.
    let proof_json = r#"{
            "seam": {
                "seam_id": "seam-md",
                "seam_kind": "predicate_boundary",
                "path": "src/lib.rs",
                "line": 5,
                "grip_class": "weakly_gripped",
                "missing_discriminator": "assert boundary"
            },
            "recommendation": {
                "related_test": "tests/lib.rs::test_me",
                "suggested_test_name": "test_boundary"
            }
        }"#;
    let pr_guidance_json = r#"{"comments":[{"seam_id":"seam-md"}]}"#;
    let mut input = bare_input();
    input.pr_guidance_path = Some("g.json".to_string());
    input.assistant_proof_path = Some("proof.json".to_string());
    input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
    input.assistant_proof_json = Some(Ok(proof_json.to_string()));

    let report = build_first_useful_action_report(input);
    let md = render_first_useful_action_markdown(&report);

    // The actionable report has why_first bullets
    assert!(
        md.contains("## Why First"),
        "expected Why First section in markdown: {md}"
    );
    Ok(())
}

#[test]
fn markdown_includes_where_section_for_write_focused_test() -> Result<(), String> {
    let proof_json = r#"{
            "seam": {
                "seam_id": "seam-where",
                "seam_kind": "predicate_boundary",
                "path": "src/lib.rs",
                "line": 7,
                "grip_class": "weakly_gripped"
            },
            "recommendation": {
                "related_test": "tests/lib.rs::test_boundary",
                "recommended_file": "tests/lib.rs",
                "suggested_test_name": "test_boundary_at_seven"
            }
        }"#;
    let pr_guidance_json = r#"{"comments":[{"seam_id":"seam-where","suggested_test":{"recommended_file":"tests/lib.rs","near_test":"tests/lib.rs::test_boundary","recommended_name":"test_boundary_at_seven"}}]}"#;
    let mut input = bare_input();
    input.pr_guidance_path = Some("g.json".to_string());
    input.assistant_proof_path = Some("proof.json".to_string());
    input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
    input.assistant_proof_json = Some(Ok(proof_json.to_string()));

    let report = build_first_useful_action_report(input);
    let md = render_first_useful_action_markdown(&report);

    assert!(
        md.contains("## Where"),
        "expected Where section in markdown: {md}"
    );
    Ok(())
}

#[test]
fn markdown_fallback_with_missing_artifact() -> Result<(), String> {
    // missing_required_report includes fallback with missing field
    let mut input = bare_input();
    input.pr_guidance_path = Some("g.json".to_string());
    input.pr_guidance_json = None; // forces read_error → missing_required_report
    let report = build_first_useful_action_report(input);
    let md = render_first_useful_action_markdown(&report);
    assert!(
        md.contains("## Fallback"),
        "expected Fallback section: {md}"
    );
    assert!(
        md.contains("Missing required artifact"),
        "expected 'Missing required artifact' text: {md}"
    );
    Ok(())
}

#[test]
fn markdown_fallback_with_summary_text() -> Result<(), String> {
    // suppressed report uses fallback with summary (not missing)
    let pr_guidance_json = r#"{"suppressed":[{"seam_id":"seam-s"}]}"#;
    let mut input = bare_input();
    input.pr_guidance_path = Some("g.json".to_string());
    input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
    let report = build_first_useful_action_report(input);
    let md = render_first_useful_action_markdown(&report);
    assert!(
        md.contains("## Fallback"),
        "expected Fallback section in markdown: {md}"
    );
    Ok(())
}

#[test]
fn markdown_no_fallback_for_actionable_status() -> Result<(), String> {
    // actionable report has no fallback – markdown should not show Fallback section
    let proof_json = r#"{
            "seam": {"seam_id": "seam-a", "seam_kind": "predicate_boundary", "grip_class": "weakly_gripped"},
            "recommendation": {}
        }"#;
    let pr_guidance_json = r#"{"comments":[{"seam_id":"seam-a"}]}"#;
    let mut input = bare_input();
    input.pr_guidance_path = Some("g.json".to_string());
    input.assistant_proof_path = Some("proof.json".to_string());
    input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
    input.assistant_proof_json = Some(Ok(proof_json.to_string()));
    let report = build_first_useful_action_report(input);
    let md = render_first_useful_action_markdown(&report);
    // actionable status → fallback section suppressed
    assert!(
        !(md.contains("## Fallback")),
        "should not show Fallback for actionable status: {md}"
    );
    Ok(())
}

#[test]
fn markdown_limits_section_present() -> Result<(), String> {
    // All routing branches produce limits; verify they appear in markdown
    let report = build_first_useful_action_report(bare_input());
    let md = render_first_useful_action_markdown(&report);
    assert!(
        md.contains("## Limits"),
        "expected Limits section in markdown: {md}"
    );
    Ok(())
}

// ── with_period / trim_period / str_or ───────────────────────────────────

#[test]
fn with_period_appends_when_missing() -> Result<(), String> {
    let result = with_period("hello");
    assert!((result == "hello."), "expected 'hello.' but got '{result}'");
    Ok(())
}

#[test]
fn with_period_does_not_double_period() -> Result<(), String> {
    let result = with_period("done.");
    assert!((result == "done."), "expected 'done.' but got '{result}'");
    Ok(())
}

#[test]
fn trim_period_removes_trailing_dot() -> Result<(), String> {
    let result = trim_period("word.");
    assert!((result == "word"), "expected 'word' but got '{result}'");
    Ok(())
}

#[test]
fn trim_period_leaves_no_dot_unchanged() -> Result<(), String> {
    let result = trim_period("word");
    assert!((result == "word"), "expected 'word' but got '{result}'");
    Ok(())
}

#[test]
fn str_or_returns_value_when_some() -> Result<(), String> {
    let result = str_or(Some("actual"), "fallback");
    assert!((result == "actual"), "expected 'actual' but got '{result}'");
    Ok(())
}

#[test]
fn str_or_returns_fallback_when_none() -> Result<(), String> {
    let result = str_or(None, "fallback");
    assert!(
        (result == "fallback"),
        "expected 'fallback' but got '{result}'"
    );
    Ok(())
}

// ── normalize_suggested_assertion ────────────────────────────────────────

#[test]
fn normalize_suggested_assertion_reformats_add_focused_test_pattern() -> Result<(), String> {
    let input = "Add a focused test where x > 0 and assert the exact output is 1.";
    let result = normalize_suggested_assertion(input);
    assert!(
        result.starts_with("Assert the exact"),
        "expected reformatted assertion but got: {result}"
    );
    assert!(
        !(result.contains("Add a focused test where")),
        "should not contain original prefix after normalization: {result}"
    );
    Ok(())
}

#[test]
fn normalize_suggested_assertion_passes_through_unmatched() -> Result<(), String> {
    let input = "assert_eq!(f(x), 42)";
    let result = normalize_suggested_assertion(input);
    assert!((result == input), "expected pass-through but got: {result}");
    Ok(())
}

// ── classification_from_sources alias mapping ────────────────────────────

#[test]
fn classification_from_sources_maps_weakly_gripped() -> Result<(), String> {
    let value: Value =
        serde_json::from_str(r#"{"grip_class": "weakly_gripped"}"#).map_err(|e| e.to_string())?;
    let result = classification_from_sources(&[(Some(&value), &["grip_class"])]);
    assert!(
        (result.as_deref() == Some("weakly_exposed")),
        "expected weakly_exposed but got: {result:?}"
    );
    Ok(())
}

#[test]
fn classification_from_sources_maps_strongly_gripped() -> Result<(), String> {
    let value: Value =
        serde_json::from_str(r#"{"grip_class": "strongly_gripped"}"#).map_err(|e| e.to_string())?;
    let result = classification_from_sources(&[(Some(&value), &["grip_class"])]);
    assert!(
        (result.as_deref() == Some("exposed")),
        "expected exposed but got: {result:?}"
    );
    Ok(())
}

#[test]
fn classification_from_sources_passes_through_other() -> Result<(), String> {
    let value: Value = serde_json::from_str(r#"{"classification": "static_unknown"}"#)
        .map_err(|e| e.to_string())?;
    let result = classification_from_sources(&[(Some(&value), &["classification"])]);
    assert!(
        (result.as_deref() == Some("static_unknown")),
        "expected static_unknown but got: {result:?}"
    );
    Ok(())
}

// ── path_value numeric index ─────────────────────────────────────────────

#[test]
fn path_value_resolves_numeric_array_index() -> Result<(), String> {
    let value: Value =
        serde_json::from_str(r#"{"items": ["a", "b", "c"]}"#).map_err(|e| e.to_string())?;
    let result = path_value(&value, &["items", "1"]);
    assert!(
        (result.and_then(Value::as_str) == Some("b")),
        "expected 'b' at index 1 but got: {result:?}"
    );
    Ok(())
}

#[test]
fn path_value_returns_none_for_missing_key() -> Result<(), String> {
    let value: Value = serde_json::from_str(r#"{"a": 1}"#).map_err(|e| e.to_string())?;
    let result = path_value(&value, &["b", "c"]);
    assert!(result.is_none(), "expected None for missing key");
    Ok(())
}

// ── value_as_string numeric coercion ─────────────────────────────────────

#[test]
fn value_as_string_coerces_i64() -> Result<(), String> {
    let value: Value = serde_json::from_str(r#"-42"#).map_err(|e| e.to_string())?;
    let result = value_as_string(&value);
    assert!(
        (result.as_deref() == Some("-42")),
        "expected '-42' but got: {result:?}"
    );
    Ok(())
}

#[test]
fn value_as_string_coerces_u64() -> Result<(), String> {
    let value: Value =
        serde_json::from_str(r#"18446744073709551615"#).map_err(|e| e.to_string())?;
    let result = value_as_string(&value);
    assert!(result.is_some(), "expected Some for large u64");
    Ok(())
}

#[test]
fn value_as_string_returns_none_for_object() -> Result<(), String> {
    let value: Value = serde_json::from_str(r#"{"k": 1}"#).map_err(|e| e.to_string())?;
    let result = value_as_string(&value);
    assert!(result.is_none(), "expected None for object value");
    Ok(())
}

// ── bool_path ────────────────────────────────────────────────────────────

#[test]
fn bool_path_extracts_true_value() -> Result<(), String> {
    let value: Value = serde_json::from_str(r#"{"stale": true}"#).map_err(|e| e.to_string())?;
    let result = bool_path(&value, &["stale"]);
    assert!(
        (result == Some(true)),
        "expected Some(true) but got: {result:?}"
    );
    Ok(())
}

#[test]
fn bool_path_extracts_false_value() -> Result<(), String> {
    let value: Value = serde_json::from_str(r#"{"stale": false}"#).map_err(|e| e.to_string())?;
    let result = bool_path(&value, &["stale"]);
    assert!(
        (result == Some(false)),
        "expected Some(false) but got: {result:?}"
    );
    Ok(())
}

#[test]
fn bool_path_returns_none_for_missing_key() -> Result<(), String> {
    let value: Value = serde_json::from_str(r#"{}"#).map_err(|e| e.to_string())?;
    let result = bool_path(&value, &["stale"]);
    assert!(result.is_none(), "expected None for missing key");
    Ok(())
}

// ── has_any_input / has_any_parsed ───────────────────────────────────────

#[test]
fn has_any_input_false_when_all_none() -> Result<(), String> {
    let input = bare_input();
    assert!(
        !(has_any_input(&input)),
        "expected has_any_input to be false with all Nones"
    );
    Ok(())
}

#[test]
fn has_any_input_true_when_one_path_set() -> Result<(), String> {
    let mut input = bare_input();
    input.coverage_frontier_path = Some("frontier.json".to_string());
    assert!(has_any_input(&input), "expected has_any_input to be true");
    Ok(())
}

#[test]
fn has_any_parsed_false_when_all_none() -> Result<(), String> {
    let parsed = ParsedSources::default();
    assert!(
        !(has_any_parsed(&parsed)),
        "expected has_any_parsed to be false"
    );
    Ok(())
}

#[test]
fn has_any_parsed_true_when_coverage_frontier_set() -> Result<(), String> {
    let parsed = ParsedSources {
        coverage_frontier: Some(serde_json::Value::Bool(true)),
        ..ParsedSources::default()
    };
    assert!(
        has_any_parsed(&parsed),
        "expected has_any_parsed to be true"
    );
    Ok(())
}

// ── first_guidance_item / first_summary_only_item / first_suppressed_item ─

#[test]
fn first_guidance_item_returns_first_comment() -> Result<(), String> {
    let guidance: Value =
        serde_json::from_str(r#"{"comments":[{"seam_id":"s1"},{"seam_id":"s2"}]}"#)
            .map_err(|e| e.to_string())?;
    let item = first_guidance_item(Some(&guidance));
    let seam_id = item.and_then(|v| v.get("seam_id")).and_then(|v| v.as_str());
    assert!(
        (seam_id == Some("s1")),
        "expected 's1' but got: {seam_id:?}"
    );
    Ok(())
}

#[test]
fn first_guidance_item_returns_none_when_empty() -> Result<(), String> {
    let guidance: Value = serde_json::from_str(r#"{"comments":[]}"#).map_err(|e| e.to_string())?;
    assert!(
        first_guidance_item(Some(&guidance)).is_none(),
        "expected None for empty comments"
    );
    Ok(())
}

#[test]
fn first_summary_only_item_returns_first() -> Result<(), String> {
    let guidance: Value = serde_json::from_str(r#"{"summary_only":[{"seam_id":"so1"}]}"#)
        .map_err(|e| e.to_string())?;
    let item = first_summary_only_item(Some(&guidance));
    assert!(item.is_some(), "expected Some for summary_only item");
    Ok(())
}

#[test]
fn first_suppressed_item_returns_first() -> Result<(), String> {
    let guidance: Value = serde_json::from_str(r#"{"suppressed":[{"seam_id":"sup1"}]}"#)
        .map_err(|e| e.to_string())?;
    let item = first_suppressed_item(Some(&guidance));
    assert!(item.is_some(), "expected Some for suppressed item");
    Ok(())
}

// ── has_actionable_guidance ──────────────────────────────────────────────

#[test]
fn has_actionable_guidance_true_for_comments() -> Result<(), String> {
    let guidance: Value =
        serde_json::from_str(r#"{"comments":[{"seam_id":"s1"}]}"#).map_err(|e| e.to_string())?;
    assert!(
        has_actionable_guidance(Some(&guidance)),
        "expected actionable guidance for comments"
    );
    Ok(())
}

#[test]
fn has_actionable_guidance_true_for_summary_only() -> Result<(), String> {
    let guidance: Value = serde_json::from_str(r#"{"summary_only":[{"seam_id":"s2"}]}"#)
        .map_err(|e| e.to_string())?;
    assert!(
        has_actionable_guidance(Some(&guidance)),
        "expected actionable guidance for summary_only"
    );
    Ok(())
}

#[test]
fn has_actionable_guidance_false_when_none() -> Result<(), String> {
    assert!(
        !(has_actionable_guidance(None)),
        "expected false when guidance is None"
    );
    Ok(())
}

// ── has_suppressed_guidance ──────────────────────────────────────────────

#[test]
fn has_suppressed_guidance_false_when_none() -> Result<(), String> {
    assert!(
        !(has_suppressed_guidance(None)),
        "expected false when guidance is None"
    );
    Ok(())
}

#[test]
fn has_suppressed_guidance_true_for_suppressed_array() -> Result<(), String> {
    let guidance: Value =
        serde_json::from_str(r#"{"suppressed":[{"seam_id":"s"}]}"#).map_err(|e| e.to_string())?;
    assert!(
        has_suppressed_guidance(Some(&guidance)),
        "expected suppressed guidance"
    );
    Ok(())
}

// ── first_item_with_bucket ───────────────────────────────────────────────

#[test]
fn first_item_with_bucket_finds_matching_bucket() -> Result<(), String> {
    let report: Value = serde_json::from_str(
        r#"{
            "items": [
                {"bucket": "resolved", "path": "a.rs"},
                {"bucket": "still_present", "path": "b.rs"}
            ]
        }"#,
    )
    .map_err(|e| e.to_string())?;
    let item = first_item_with_bucket(&report, &["still_present"]);
    let path = item.and_then(|v| v.get("path")).and_then(|v| v.as_str());
    assert!((path == Some("b.rs")), "expected b.rs but got: {path:?}");
    Ok(())
}

#[test]
fn first_item_with_bucket_returns_none_when_no_match() -> Result<(), String> {
    let report: Value = serde_json::from_str(r#"{"items": [{"bucket": "resolved"}]}"#)
        .map_err(|e| e.to_string())?;
    assert!(
        first_item_with_bucket(&report, &["still_present"]).is_none(),
        "expected None when no bucket matches"
    );
    Ok(())
}

#[test]
fn first_item_with_bucket_returns_none_when_no_items() -> Result<(), String> {
    let report: Value = serde_json::from_str(r#"{}"#).map_err(|e| e.to_string())?;
    assert!(
        first_item_with_bucket(&report, &["still_present"]).is_none(),
        "expected None when no items key"
    );
    Ok(())
}

// ── gate_has_waiver variants ─────────────────────────────────────────────

#[test]
fn gate_has_waiver_detects_decision_waived() -> Result<(), String> {
    let gate: Value =
        serde_json::from_str(r#"{"decision": "waived"}"#).map_err(|e| e.to_string())?;
    assert!(
        gate_has_waiver(&gate),
        "expected waiver for decision=waived"
    );
    Ok(())
}

#[test]
fn gate_has_waiver_false_for_empty_waivers_array() -> Result<(), String> {
    let gate: Value = serde_json::from_str(r#"{"waivers": []}"#).map_err(|e| e.to_string())?;
    assert!(
        !(gate_has_waiver(&gate)),
        "expected no waiver for empty waivers array"
    );
    Ok(())
}

// ── first_gate_* helpers ─────────────────────────────────────────────────

#[test]
fn first_gate_seam_from_top_level() -> Result<(), String> {
    let gate: Value =
        serde_json::from_str(r#"{"seam_id": "top-seam"}"#).map_err(|e| e.to_string())?;
    let result = first_gate_seam(&gate);
    assert!(
        (result.as_deref() == Some("top-seam")),
        "expected top-seam but got: {result:?}"
    );
    Ok(())
}

#[test]
fn first_gate_seam_from_items_array() -> Result<(), String> {
    let gate: Value = serde_json::from_str(r#"{"items": [{"seam_id": "item-seam"}]}"#)
        .map_err(|e| e.to_string())?;
    let result = first_gate_seam(&gate);
    assert!(
        (result.as_deref() == Some("item-seam")),
        "expected item-seam but got: {result:?}"
    );
    Ok(())
}

#[test]
fn first_gate_path_from_top_level() -> Result<(), String> {
    let gate: Value =
        serde_json::from_str(r#"{"path": "src/gate.rs"}"#).map_err(|e| e.to_string())?;
    let result = first_gate_path(&gate);
    assert!(
        (result.as_deref() == Some("src/gate.rs")),
        "expected src/gate.rs but got: {result:?}"
    );
    Ok(())
}

#[test]
fn first_gate_line_from_top_level() -> Result<(), String> {
    let gate: Value = serde_json::from_str(r#"{"line": 42}"#).map_err(|e| e.to_string())?;
    let result = first_gate_line(&gate);
    assert!(
        (result == Some(42)),
        "expected Some(42) but got: {result:?}"
    );
    Ok(())
}

#[test]
fn first_gate_missing_discriminator_from_top_level() -> Result<(), String> {
    let gate: Value = serde_json::from_str(r#"{"missing_discriminator": "assert x == 1"}"#)
        .map_err(|e| e.to_string())?;
    let result = first_gate_missing_discriminator(&gate);
    assert!(
        (result.as_deref() == Some("assert x == 1")),
        "expected 'assert x == 1' but got: {result:?}"
    );
    Ok(())
}

// ── receipt_command / selected_seam_id ───────────────────────────────────

#[test]
fn receipt_command_with_seam_id_from_receipt_provenance() -> Result<(), String> {
    let receipt_json = r#"{"provenance": {"movement": "improved", "seam_id": "seam-rc"}}"#;
    let mut input = bare_input();
    input.receipt_path = Some("receipt.json".to_string());
    input.receipt_json = Some(Ok(receipt_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    // already_improved route → receipt command uses seam_id
    assert!(
        rendered.contains("seam-rc"),
        "expected seam-rc in rendered: {rendered}"
    );
    Ok(())
}

#[test]
fn selected_seam_id_from_ledger_top_repair_route() -> Result<(), String> {
    let ledger_json =
        r#"{"movement": {"acknowledged": 1}, "top_repair_route": {"seam_id": "seam-ledger"}}"#;
    let mut input = bare_input();
    input.ledger_path = Some("ledger.json".to_string());
    input.ledger_json = Some(Ok(ledger_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains("seam-ledger"),
        "expected seam-ledger in rendered: {rendered}"
    );
    Ok(())
}

// ── gap_records: "records" key shape ─────────────────────────────────────

#[test]
fn gap_records_from_records_key() -> Result<(), String> {
    let value: Value =
        serde_json::from_str(r#"{"records": [{"gap_id": "g1"}]}"#).map_err(|e| e.to_string())?;
    let records = gap_records(&value);
    assert!(
        (records.len() == 1),
        "expected 1 record but got {}",
        records.len()
    );
    Ok(())
}

#[test]
fn gap_records_from_cases_key() -> Result<(), String> {
    let value: Value = serde_json::from_str(
        r#"{"cases": [{"expected_gap_record": {"gap_id": "g2"}}, {"no_gap": true}]}"#,
    )
    .map_err(|e| e.to_string())?;
    let records = gap_records(&value);
    // Only the case with expected_gap_record is included
    assert!(
        (records.len() == 1),
        "expected 1 record from cases but got {}",
        records.len()
    );
    Ok(())
}

#[test]
fn gap_records_from_empty_object_returns_empty() -> Result<(), String> {
    let value: Value = serde_json::from_str(r#"{}"#).map_err(|e| e.to_string())?;
    let records = gap_records(&value);
    assert!(
        records.is_empty(),
        "expected empty records but got {}",
        records.len()
    );
    Ok(())
}

// ── first_actionable_gap_record: reintroduced policy_state ───────────────

#[test]
fn first_actionable_gap_record_accepts_reintroduced_policy_state() -> Result<(), String> {
    let record_json = r#"[{
            "gap_id": "g-r",
            "kind": "MissingAssertion",
            "language": "rust",
            "language_status": "stable",
            "scope": "pr_local",
            "gap_state": "actionable",
            "policy_state": "reintroduced",
            "repairability": "repairable",
            "repair_route": {"route_kind": "AddBoundaryAssertion"},
            "verification_commands": ["cargo test"]
        }]"#;
    let value: Value = serde_json::from_str(record_json).map_err(|e| e.to_string())?;
    assert!(
        first_actionable_gap_record(&value).is_some(),
        "expected reintroduced policy_state to be accepted"
    );
    Ok(())
}

#[test]
fn first_actionable_gap_record_rejects_missing_repair_route() -> Result<(), String> {
    let record_json = r#"[{
            "gap_id": "g-no-route",
            "language": "rust",
            "language_status": "stable",
            "scope": "pr_local",
            "gap_state": "actionable",
            "policy_state": "new",
            "repairability": "repairable",
            "verification_commands": ["cargo test"]
        }]"#;
    let value: Value = serde_json::from_str(record_json).map_err(|e| e.to_string())?;
    assert!(
        first_actionable_gap_record(&value).is_none(),
        "expected None when repair_route is absent"
    );
    Ok(())
}

#[test]
fn first_actionable_gap_record_rejects_empty_verification_commands() -> Result<(), String> {
    let record_json = r#"[{
            "gap_id": "g-empty-cmds",
            "language": "rust",
            "language_status": "stable",
            "scope": "pr_local",
            "gap_state": "actionable",
            "policy_state": "new",
            "repairability": "repairable",
            "repair_route": {"route_kind": "AddAssertion"},
            "verification_commands": ["   "]
        }]"#;
    let value: Value = serde_json::from_str(record_json).map_err(|e| e.to_string())?;
    assert!(
        first_actionable_gap_record(&value).is_none(),
        "expected None for whitespace-only verification commands"
    );
    Ok(())
}

// ── action_kind_for_gap_route ─────────────────────────────────────────────

#[test]
fn action_kind_for_add_output_golden() -> Result<(), String> {
    let result = action_kind_for_gap_route("AddOutputGolden");
    assert!(
        (result == "generate_missing_artifact"),
        "expected generate_missing_artifact but got: {result}"
    );
    Ok(())
}

#[test]
fn action_kind_for_regenerate_artifact() -> Result<(), String> {
    let result = action_kind_for_gap_route("RegenerateArtifact");
    assert!(
        (result == "generate_missing_artifact"),
        "expected generate_missing_artifact but got: {result}"
    );
    Ok(())
}

#[test]
fn action_kind_for_other_route() -> Result<(), String> {
    let result = action_kind_for_gap_route("AddBoundaryAssertion");
    assert!(
        (result == "write_focused_test"),
        "expected write_focused_test but got: {result}"
    );
    Ok(())
}

// ── target_from_gap_record filter ────────────────────────────────────────

#[test]
fn current_evidence_strength_prefers_explicit_typed_fields() -> Result<(), String> {
    let value: Value = serde_json::from_str(
        r#"{
                "classification": "weakly_exposed",
                "selected": {
                    "current_evidence_strength": "explicit selected strength"
                }
            }"#,
    )
    .map_err(|e| e.to_string())?;

    assert_eq!(
        current_evidence_strength_from_sources(&[Some(&value)]),
        Some("explicit selected strength".to_string())
    );
    Ok(())
}

#[test]
fn current_evidence_strength_describes_known_repair_routes() {
    assert_eq!(
            current_evidence_strength_for_selection(Some("AddOutputGolden"), None, None),
            Some(
                "Static evidence found changed user-facing output, but no checked output or golden proof is attached."
                    .to_string()
            )
        );
    assert_eq!(
            current_evidence_strength_for_selection(
                Some("MissingBoundaryAssertion"),
                Some("reachable_unrevealed"),
                None,
            ),
            Some(
                "Static evidence found related test context, but the current check is weak because the discriminator is missing."
                    .to_string()
            )
        );
}

#[test]
fn current_evidence_strength_describes_exposure_classes() {
    assert_eq!(
            current_evidence_strength_for_selection(None, Some("reachable_unrevealed"), None),
            Some(
                "Static evidence found reachable changed behavior, but no current check observes the changed result."
                    .to_string()
            )
        );
    assert_eq!(
        current_evidence_strength_for_selection(None, Some("no_static_path"), None),
        Some(
            "Static analysis did not find a current test path to the changed behavior.".to_string()
        )
    );
    assert_eq!(
        current_evidence_strength_for_selection(None, Some("exposed"), None),
        Some(
            "Static evidence found a current check that appears to observe the changed behavior."
                .to_string()
        )
    );
}

#[test]
fn current_evidence_strength_keeps_unknowns_conservative() {
    assert_eq!(
        current_evidence_strength_for_selection(None, Some("static_unknown"), None),
        Some("Static evidence is `static_unknown`; no runtime proof is claimed.".to_string())
    );
    assert_eq!(
        current_evidence_strength_for_selection(None, Some("custom_preview_state"), None),
        Some(
            "Static evidence reported `custom_preview_state`; no runtime proof is claimed."
                .to_string()
        )
    );
    assert_eq!(
        current_evidence_strength_for_selection(None, None, None),
        None
    );
}

#[test]
fn target_from_gap_record_returns_none_when_no_useful_fields() -> Result<(), String> {
    // repair_route present but none of file/related_test/assertion_shape set
    let record: Value = serde_json::from_str(r#"{"repair_route": {"route_kind": "AddAssertion"}}"#)
        .map_err(|e| e.to_string())?;
    let result = target_from_gap_record(&record);
    assert!(result.is_none(), "expected None when no target fields");
    Ok(())
}

#[test]
fn target_from_gap_record_returns_some_when_file_set() -> Result<(), String> {
    let record: Value = serde_json::from_str(
        r#"{"repair_route": {"route_kind": "AddAssertion", "target_file": "tests/foo.rs"}}"#,
    )
    .map_err(|e| e.to_string())?;
    let result = target_from_gap_record(&record);
    assert!(result.is_some(), "expected Some when target_file is set");
    Ok(())
}

#[test]
fn target_from_gap_record_none_when_no_repair_route() -> Result<(), String> {
    let record: Value = serde_json::from_str(r#"{"gap_id": "g1"}"#).map_err(|e| e.to_string())?;
    let result = target_from_gap_record(&record);
    assert!(result.is_none(), "expected None when repair_route missing");
    Ok(())
}

// ── push_wrapped_paragraph ───────────────────────────────────────────────

#[test]
fn push_wrapped_paragraph_formats_text() -> Result<(), String> {
    let mut out = String::new();
    push_wrapped_paragraph(&mut out, "short text");
    assert!(
        out.contains("short text"),
        "expected 'short text' in output: {out}"
    );
    assert!(out.ends_with('\n'), "expected trailing newline");
    Ok(())
}

#[test]
fn push_wrapped_paragraph_wraps_long_text() -> Result<(), String> {
    let long_text = "word ".repeat(20).trim().to_string();
    let mut out = String::new();
    push_wrapped_paragraph(&mut out, &long_text);
    let lines: Vec<&str> = out.lines().collect();
    assert!(
        (lines.len() >= 2),
        "expected wrapped text to have multiple lines but got: {out}"
    );
    Ok(())
}

// ── selected_from_receipt_or_sources seam_id fallback ────────────────────

#[test]
fn selected_from_receipt_uses_proof_seam_id_fallback() -> Result<(), String> {
    // receipt has no seam_id, but assistant_proof has seam.seam_id
    let receipt_json = r#"{"provenance": {"movement": "improved"}}"#;
    let proof_json = r#"{"seam": {"seam_id": "proof-seam-id"}}"#;
    let pr_guidance_json = r#"{"comments":[{"seam_id":"g1"}]}"#;
    let mut input = bare_input();
    input.receipt_path = Some("receipt.json".to_string());
    input.receipt_json = Some(Ok(receipt_json.to_string()));
    input.assistant_proof_path = Some("proof.json".to_string());
    input.assistant_proof_json = Some(Ok(proof_json.to_string()));
    input.pr_guidance_path = Some("g.json".to_string());
    input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains("proof-seam-id"),
        "expected proof-seam-id in receipt selected: {rendered}"
    );
    Ok(())
}

// ── selected_from_guidance: summary_only fallback ────────────────────────

#[test]
fn selected_from_guidance_uses_summary_only_item() -> Result<(), String> {
    // suppressed items are absent but summary_only is present, and suppressed string in warnings
    // forces suppressed route → guidance selected from summary_only fallback
    let pr_guidance_json = r#"{
            "summary_only": [{"seam_id": "so-seam", "kind": "predicate_boundary"}],
            "warnings": ["seam configured off by policy"]
        }"#;
    let mut input = bare_input();
    input.pr_guidance_path = Some("g.json".to_string());
    input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    // suppressed route triggered by warning text
    assert!(
        rendered.contains(r#""status": "suppressed""#),
        "expected suppressed but got: {rendered}"
    );
    Ok(())
}

// ── selected_acknowledged: ledger fallback seam_id ───────────────────────

#[test]
fn selected_acknowledged_uses_fallback_seam_id_when_no_top_repair_route() -> Result<(), String> {
    let ledger_json = r#"{"movement": {"acknowledged": 2}}"#;
    let mut input = bare_input();
    input.ledger_path = Some("ledger.json".to_string());
    input.ledger_json = Some(Ok(ledger_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains("acknowledged-boundary-0001"),
        "expected fallback seam_id in acknowledged: {rendered}"
    );
    Ok(())
}

// ── selected_waived: fallback seam_id when no seam_id present ────────────

#[test]
fn selected_waived_uses_fallback_seam_id_when_no_seam_id() -> Result<(), String> {
    let gate_json = r#"{"waivers": [{"id": "w1"}]}"#;
    let mut input = bare_input();
    input.gate_decision_path = Some("gate.json".to_string());
    input.gate_decision_json = Some(Ok(gate_json.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains("waived-boundary-0001"),
        "expected fallback seam_id for waived: {rendered}"
    );
    Ok(())
}

// ── display_path round-trip ───────────────────────────────────────────────

#[test]
fn display_path_converts_backslashes() -> Result<(), String> {
    let path = Path::new("some\\windows\\path.json");
    let result = display_path(path);
    assert!(
        !(result.contains('\\')),
        "expected forward slashes but got: {result}"
    );
    assert!(
        result.contains("some/windows/path.json"),
        "unexpected path: {result}"
    );
    Ok(())
}

// ── render_first_useful_action_json: verify schema fields present ─────────

#[test]
fn render_first_useful_action_json_includes_schema_version() -> Result<(), String> {
    let report = build_first_useful_action_report(bare_input());
    let rendered = render_first_useful_action_json(&report)?;
    assert!(
        rendered.contains(r#""schema_version": "0.1""#),
        "expected schema_version in JSON output: {rendered}"
    );
    assert!(
        rendered.contains(r#""kind": "first_useful_action""#),
        "expected kind field in JSON output: {rendered}"
    );
    Ok(())
}

// ── coverage_frontier and editor_context parsed but not actionable ────────

#[test]
fn coverage_frontier_does_not_block_no_actionable_routing() -> Result<(), String> {
    let mut input = bare_input();
    input.coverage_frontier_path = Some("frontier.json".to_string());
    input.coverage_frontier_json = Some(Ok(r#"{"kind": "coverage_frontier"}"#.to_string()));
    let report = build_first_useful_action_report(input);
    let rendered = render_first_useful_action_json(&report)?;
    // coverage frontier alone doesn't trigger actionable routing
    assert!(
        rendered.contains(r#""status": "no_actionable_seam""#),
        "expected no_actionable_seam with only frontier: {rendered}"
    );
    Ok(())
}

// ── markdown receipt section ─────────────────────────────────────────────

#[test]
fn markdown_verify_section_present_for_actionable() -> Result<(), String> {
    // The actionable report emits seam_commands which include a verify command
    let proof_json = r#"{
            "seam": {
                "seam_id": "seam-verify",
                "seam_kind": "predicate_boundary",
                "grip_class": "weakly_gripped"
            },
            "recommendation": {}
        }"#;
    let pr_guidance_json = r#"{"comments":[{"seam_id":"seam-verify"}]}"#;
    let mut input = bare_input();
    input.pr_guidance_path = Some("g.json".to_string());
    input.assistant_proof_path = Some("proof.json".to_string());
    input.pr_guidance_json = Some(Ok(pr_guidance_json.to_string()));
    input.assistant_proof_json = Some(Ok(proof_json.to_string()));
    let report = build_first_useful_action_report(input);
    let md = render_first_useful_action_markdown(&report);
    assert!(
        md.contains("## Verify"),
        "expected Verify section in actionable markdown: {md}"
    );
    Ok(())
}
