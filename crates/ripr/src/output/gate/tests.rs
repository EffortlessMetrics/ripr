use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn gate_visible_only_records_pr_guidance_without_blocking() -> Result<(), String> {
    let input = fixture_input(GateMode::VisibleOnly);
    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "advisory");
    assert_eq!(report.summary.evaluated, 1);
    assert_eq!(report.summary.advisory, 1);
    assert!(!gate_decision_should_fail(&report));
    let json_text = render_gate_decision_json(&report)?;
    let value: Value = serde_json::from_str(&json_text)
        .map_err(|err| format!("gate decision JSON should parse: {err}"))?;
    assert_eq!(value["schema_version"], SCHEMA_VERSION);
    assert_eq!(value["status"], "advisory");
    assert_eq!(value["decisions"][0]["decision"], "advisory");
    assert_eq!(
        value["decisions"][0]["evidence"]["recommended_test"],
        "tests/pricing.rs::above_threshold_gets_discount"
    );
    let markdown = render_gate_decision_markdown(&report);
    assert!(markdown.contains("# RIPR Gate Decision"));
    assert!(markdown.contains("Decision: advisory"));
    assert!(markdown.contains("visible-only mode records evidence without blocking"));
    Ok(())
}

#[test]
fn gate_acknowledgeable_blocks_policy_candidate_without_label() -> Result<(), String> {
    let input = fixture_input(GateMode::Acknowledgeable);
    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "blocked");
    assert_eq!(report.summary.blocking, 1);
    assert!(gate_decision_should_fail(&report));
    assert_eq!(report.decisions[0].decision, "blocking");
    Ok(())
}

#[test]
fn gate_acknowledgeable_keeps_waived_candidate_visible() -> Result<(), String> {
    let mut input = fixture_input(GateMode::Acknowledgeable);
    input.labels.push("ripr-waive".to_string());
    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "acknowledged");
    assert_eq!(report.summary.acknowledged, 1);
    assert!(!gate_decision_should_fail(&report));
    assert_eq!(
        report.decisions[0].policy.acknowledgement_label,
        Some("ripr-waive".to_string())
    );
    Ok(())
}

#[test]
fn gate_calibrated_mode_requires_explicit_baseline() -> Result<(), String> {
    let input = fixture_input(GateMode::CalibratedGate);
    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "config_error");
    assert_eq!(report.summary.evaluated, 0);
    assert!(gate_decision_should_fail(&report));
    assert!(
        report
            .config_errors
            .iter()
            .any(|error| error.contains("requires an explicit --baseline"))
    );
    Ok(())
}

#[test]
fn gate_fails_closed_on_limited_partial_scope_pr_guidance() -> Result<(), String> {
    // RIPR-PROP-0019 decision 5: a structurally valid guidance document that
    // discloses a `limited_partial_scope` producer run is a partial
    // denominator, never a gate input — fail closed like a malformed document.
    let dir = temp_dir("gate-partial-pr-guidance")?;
    let guidance = write_temp_json(
        &dir,
        "comments.json",
        r#"{
          "schema_version": "0.1",
          "status": "advisory",
          "comments": [],
          "analysis_scope": {
            "run_status": "limited_partial_scope",
            "gate_eligibility": "ineligible"
          }
        }"#,
    )?;
    let mut input = fixture_input(GateMode::VisibleOnly);
    input.pr_guidance = Some(guidance);

    let report = build_gate_decision_report(&input)?;

    assert_eq!(report.status, "config_error");
    assert!(gate_decision_should_fail(&report));
    assert_eq!(
        report.summary.evaluated, 0,
        "a partial denominator must not be evaluated as a complete input"
    );
    assert!(
        report
            .config_errors
            .iter()
            .any(|error| error.contains("limited_partial_scope")),
        "config error must name the partial run state: {:?}",
        report.config_errors
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_fails_closed_on_limited_partial_scope_gap_ledger() -> Result<(), String> {
    let dir = temp_dir("gate-partial-gap-ledger")?;
    let ledger = write_temp_json(
        &dir,
        "ledger.json",
        r#"{
          "schema_version": "0.1",
          "records": [],
          "run_limitations": [
            {"category": "limited_partial_scope", "run_status": "limited_partial_scope"}
          ]
        }"#,
    )?;
    let mut input = fixture_input(GateMode::VisibleOnly);
    input.pr_guidance = None;
    input.gap_ledger = Some(ledger);

    let report = build_gate_decision_report(&input)?;

    assert_eq!(report.status, "config_error");
    assert!(gate_decision_should_fail(&report));
    assert!(
        report
            .config_errors
            .iter()
            .any(|error| error.contains("limited_partial_scope")),
        "config error must name the partial run state: {:?}",
        report.config_errors
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_fails_closed_on_limited_partial_scope_baseline() -> Result<(), String> {
    let dir = temp_dir("gate-partial-baseline")?;
    let baseline = write_temp_json(
        &dir,
        "baseline.json",
        r#"{
          "schema_version": "0.1",
          "decisions": [],
          "analysis_scope": {"run_status": "limited_partial_scope"}
        }"#,
    )?;
    let mut input = fixture_input(GateMode::CalibratedGate);
    input.baseline = Some(baseline);

    let report = build_gate_decision_report(&input)?;

    assert_eq!(report.status, "config_error");
    assert!(gate_decision_should_fail(&report));
    assert!(
        report
            .config_errors
            .iter()
            .any(|error| error.contains("limited_partial_scope")),
        "config error must name the partial run state: {:?}",
        report.config_errors
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn limited_partial_scope_detection_covers_run_state_vocabulary() {
    let run_status = crate::analysis::PartialDiffScope::RUN_STATUS;
    assert!(discloses_limited_partial_scope(&json!({
        "run_status": run_status
    })));
    assert!(discloses_limited_partial_scope(&json!({
        "analysis_scope": {"run_status": run_status}
    })));
    assert!(discloses_limited_partial_scope(&json!({
        "run_limitations": [{"category": run_status}]
    })));
    assert!(discloses_limited_partial_scope(&json!({
        "run_limitations": [{"run_status": run_status}]
    })));
    assert!(!discloses_limited_partial_scope(&json!({
        "schema_version": "0.1",
        "status": "advisory",
        "comments": []
    })));
    assert!(!discloses_limited_partial_scope(&json!({
        "analysis_scope": {"run_status": "diff_scope_oversized"}
    })));
}

#[test]
fn gate_calibrated_mode_blocks_new_supported_candidate() -> Result<(), String> {
    let dir = temp_dir("gate-calibrated")?;
    let baseline = dir.join("baseline.json");
    fs::write(&baseline, r#"{"schema_version":"0.1","decisions":[]}"#)
        .map_err(|err| format!("write baseline failed: {err}"))?;
    let mut input = fixture_input(GateMode::CalibratedGate);
    input.baseline = Some(baseline);
    input.recommendation_calibration = Some(PathBuf::from(
        "fixtures/boundary_gap/expected/recommendation-calibration/recommendation-calibration.json",
    ));
    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "blocked");
    assert_eq!(report.summary.blocking, 1);
    assert_eq!(
        report.decisions[0]
            .evidence
            .recommendation_calibration
            .confidence_effect,
        "supports_static_gap"
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_calibrated_mode_uses_imported_mutation_support() -> Result<(), String> {
    let dir = temp_dir("gate-mutation-calibrated")?;
    let baseline = dir.join("baseline.json");
    let mutation = dir.join("mutation-calibration.json");
    fs::write(&baseline, r#"{"schema_version":"0.1","decisions":[]}"#)
        .map_err(|err| format!("write baseline failed: {err}"))?;
    fs::write(
        &mutation,
        r#"{
              "schema_version": "0.1",
              "matches": [
                {
                  "join_method": "seam_id",
                  "runtime": {
                    "seam_id": "8f7fa8644fd12280",
                    "runtime_outcome": "missed"
                  },
                  "static": {
                    "seam_id": "8f7fa8644fd12280"
                  }
                }
              ],
              "ambiguous_file_line_matches": []
            }"#,
    )
    .map_err(|err| format!("write mutation calibration failed: {err}"))?;
    let mut input = fixture_input(GateMode::CalibratedGate);
    input.baseline = Some(baseline);
    input.mutation_calibration = Some(mutation);
    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "blocked");
    assert_eq!(report.summary.blocking, 1);
    assert_eq!(
        report.decisions[0]
            .evidence
            .mutation_calibration
            .confidence_effect,
        "supports_static_gap"
    );
    assert!(
        report.decisions[0]
            .gate_reason
            .contains("imported mutation calibration")
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_labels_json_acknowledges_candidate() -> Result<(), String> {
    let dir = temp_dir("gate-labels-json")?;
    let labels = dir.join("labels.json");
    fs::write(&labels, r#"{"labels":["ripr-waive"]}"#)
        .map_err(|err| format!("write labels failed: {err}"))?;
    let mut input = fixture_input(GateMode::Acknowledgeable);
    input.labels_json = Some(labels);
    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "acknowledged");
    assert_eq!(report.inputs.labels, vec!["ripr-waive".to_string()]);
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_baseline_check_keeps_existing_candidate_advisory() -> Result<(), String> {
    let dir = temp_dir("gate-baseline-existing")?;
    let baseline = dir.join("baseline.json");
    fs::write(
        &baseline,
        r#"{
              "schema_version": "0.1",
              "decisions": [
                {"seam_id": "8f7fa8644fd12280", "source_id": "ripr-review-8f7fa8644fd12280"}
              ]
            }"#,
    )
    .map_err(|err| format!("write baseline failed: {err}"))?;
    let mut input = fixture_input(GateMode::BaselineCheck);
    input.baseline = Some(baseline);
    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "advisory");
    assert_eq!(report.summary.blocking, 0);
    assert_eq!(report.summary.advisory, 1);
    assert!(
        report.decisions[0]
            .gate_reason
            .contains("explicit baseline")
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_baseline_check_reads_baseline_ledger_entries() -> Result<(), String> {
    let dir = temp_dir("gate-baseline-ledger-entry")?;
    let baseline = dir.join("baseline.json");
    fs::write(
        &baseline,
        r#"{
              "schema_version": "0.1",
              "kind": "gate_baseline",
              "entries": [
                {
                  "identity": {
                    "seam_id": "8f7fa8644fd12280",
                    "source_id": "ripr-review-8f7fa8644fd12280",
                    "id": "ripr-gate-8f7fa8644fd12280",
                    "dedupe_key": null,
                    "fallback": "src/pricing.rs:88:weakly_gripped"
                  }
                }
              ]
            }"#,
    )
    .map_err(|err| format!("write baseline failed: {err}"))?;
    let mut input = fixture_input(GateMode::BaselineCheck);
    input.baseline = Some(baseline);
    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "advisory");
    assert_eq!(report.summary.blocking, 0);
    assert_eq!(report.summary.advisory, 1);
    assert!(
        report.decisions[0]
            .gate_reason
            .contains("explicit baseline")
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_baseline_check_matches_canonical_gap_id_from_evidence_record() -> Result<(), String> {
    let dir = temp_dir("gate-baseline-canonical")?;
    let baseline = write_temp_json(
        &dir,
        "baseline.json",
        r#"{
              "schema_version": "0.1",
              "kind": "gate_baseline",
              "entries": [
                {
                  "identity": {
                    "canonical_gap_id": "pricing::discount::threshold_equality",
                    "seam_id": "old-seam",
                    "source_id": "old-review-id",
                    "fallback": "src/pricing.rs:88:weakly_gripped"
                  }
                }
              ]
            }"#,
    )?;
    let guidance = write_temp_json(
        &dir,
        "comments.json",
        r#"{
              "schema_version": "0.1",
              "summary": {"unchanged_tests": true},
              "comments": [
                {
                  "id": "ripr-review-new-line",
                  "seam_id": "new-seam",
                  "gap_state": "actionable",
                  "grip_class": "weakly_gripped",
                  "severity": "warning",
                  "owner": "pricing::discounted_total",
                  "seam": {
                    "expression": "amount >= discount_threshold",
                    "file": "src/pricing.rs",
                    "line": 144
                  },
                  "missing_discriminator": "amount == discount_threshold",
                  "placement": {"path": "src/pricing.rs", "line": 144},
                  "suggested_test": {
                    "candidate_values": ["amount == discount_threshold"],
                    "near_test": "above_threshold_gets_discount",
                    "related_test": {
                      "name": "above_threshold_gets_discount",
                      "file": "tests/pricing.rs",
                      "line": 12
                    }
                  },
                  "llm_guidance": {
                    "command": "ripr agent brief --root . --seam-id new-seam --json",
                    "prompt": "Add the equality-boundary discriminator next to the related test.",
                    "verify_command": "cargo test -p pricing above_threshold_gets_discount"
                  },
                  "receipt_command": "ripr receipt write --gap pricing::discount::threshold_equality",
                  "evidence_record": {
                    "canonical_gap_id": "pricing::discount::threshold_equality"
                  }
                }
              ],
              "summary_only": [],
              "suppressed": []
            }"#,
    )?;
    let mut input = fixture_input(GateMode::BaselineCheck);
    input.pr_guidance = Some(guidance);
    input.baseline = Some(baseline);

    let report = build_gate_decision_report(&input)?;
    let rendered = render_gate_decision_json(&report)?;
    let value: Value = serde_json::from_str(&rendered)
        .map_err(|err| format!("gate decision JSON should parse: {err}"))?;

    assert_eq!(report.status, "advisory");
    assert_eq!(report.summary.blocking, 0);
    assert_eq!(report.summary.advisory, 1);
    assert_eq!(
        report.decisions[0].policy.baseline_identity.as_deref(),
        Some("pricing::discount::threshold_equality")
    );
    assert_eq!(
        value["decisions"][0]["canonical_gap_id"],
        "pricing::discount::threshold_equality"
    );
    assert_eq!(
        value["decisions"][0]["policy"]["baseline_identity"],
        "pricing::discount::threshold_equality"
    );
    assert!(
        report.decisions[0]
            .gate_reason
            .contains("explicit baseline")
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_baseline_fallback_only_match_discloses_warning_and_match_kind() -> Result<(), String> {
    let dir = temp_dir("gate-baseline-fallback-only")?;
    let baseline = write_temp_json(
        &dir,
        "baseline.json",
        r#"{
              "schema_version": "0.1",
              "kind": "gate_baseline",
              "entries": [
                {
                  "identity": {
                    "canonical_gap_id": "gap:stale-reviewed-debt",
                    "fallback": "src/pricing.rs:88:weakly_gripped"
                  }
                }
              ]
            }"#,
    )?;
    let mut input = fixture_input(GateMode::BaselineCheck);
    input.baseline = Some(baseline);

    let report = build_gate_decision_report(&input)?;
    let rendered = render_gate_decision_json(&report)?;
    let value: Value = serde_json::from_str(&rendered)
        .map_err(|err| format!("gate decision JSON should parse: {err}"))?;
    let markdown = render_gate_decision_markdown(&report);

    // Disclose-first (issue #1934): the fallback-only match still suppresses
    // blocking during the compatibility window, but it cannot stay silent.
    assert_eq!(report.status, "advisory");
    assert_eq!(report.summary.blocking, 0);
    assert!(!report.decisions[0].is_baseline_new);
    assert_eq!(
        report.decisions[0].baseline_match_kind.as_deref(),
        Some("legacy_path_line_class")
    );
    assert_eq!(
        value["decisions"][0]["baseline_match_kind"],
        "legacy_path_line_class"
    );
    let warning = report
        .warnings
        .iter()
        .find(|warning| {
            warning.contains("matched baseline evidence by fallback path/line/static_class")
        })
        .ok_or_else(|| {
            format!(
                "expected a fallback-match disclosure warning, got {:?}",
                report.warnings
            )
        })?;
    assert!(
        warning.contains("ripr-review-8f7fa8644fd12280"),
        "warning must name the candidate, got {warning:?}"
    );
    assert!(
        warning.contains("src/pricing.rs:88:weakly_gripped"),
        "warning must name the matched legacy identity, got {warning:?}"
    );
    assert!(
        markdown.contains("matched baseline evidence by fallback path/line/static_class"),
        "fallback disclosure must be visible in human output"
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_baseline_canonical_match_has_no_fallback_disclosure() -> Result<(), String> {
    let dir = temp_dir("gate-baseline-canonical-precedence")?;
    // The baseline entry carries both the canonical identity and the legacy
    // fallback selector; the canonical hit must win and stay silent.
    let baseline = write_temp_json(
        &dir,
        "baseline.json",
        r#"{
              "schema_version": "0.1",
              "kind": "gate_baseline",
              "entries": [
                {
                  "identity": {
                    "canonical_gap_id": "gap:dedf923a13a00573",
                    "fallback": "src/pricing.rs:88:weakly_gripped"
                  }
                }
              ]
            }"#,
    )?;
    let mut input = fixture_input(GateMode::BaselineCheck);
    input.baseline = Some(baseline);

    let report = build_gate_decision_report(&input)?;
    let rendered = render_gate_decision_json(&report)?;

    assert_eq!(report.status, "advisory");
    assert!(!report.decisions[0].is_baseline_new);
    assert_eq!(report.decisions[0].baseline_match_kind, None);
    assert!(
        report.warnings.is_empty(),
        "canonical match must not warn, got {:?}",
        report.warnings
    );
    assert!(
        !rendered.contains("\"baseline_match_kind\""),
        "canonical-match decisions must render byte-identical (no baseline_match_kind key)"
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_baseline_new_candidate_has_no_fallback_disclosure() -> Result<(), String> {
    let dir = temp_dir("gate-baseline-new-no-disclosure")?;
    let baseline = write_temp_json(
        &dir,
        "baseline.json",
        r#"{"schema_version": "0.1", "kind": "gate_baseline", "entries": []}"#,
    )?;
    let mut input = fixture_input(GateMode::BaselineCheck);
    input.baseline = Some(baseline);

    let report = build_gate_decision_report(&input)?;
    let rendered = render_gate_decision_json(&report)?;

    assert_eq!(report.status, "blocked");
    assert!(report.decisions[0].is_baseline_new);
    assert_eq!(report.decisions[0].baseline_match_kind, None);
    assert!(
        report.warnings.is_empty(),
        "baseline-new candidate must not warn, got {:?}",
        report.warnings
    );
    assert!(
        !rendered.contains("\"baseline_match_kind\""),
        "baseline-new decisions must render byte-identical (no baseline_match_kind key)"
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_baseline_index_reads_all_canonical_gap_identity_shapes() {
    let value = json!({
      "entries": [
        {"canonical_gap_id": "gap:direct"},
        {"identity": {"canonical_gap_id": "gap:identity"}},
        {"evidence_record": {"canonical_gap_id": "gap:record"}}
      ],
      "decisions": [
        {"canonical_gap_id": "gap:decision-direct"},
        {"identity": {"canonical_gap_id": "gap:decision-identity"}},
        {"evidence_record": {"canonical_gap_id": "gap:decision-record"}}
      ],
      "comments": [
        {"canonical_gap_id": "gap:comment-direct"},
        {"identity": {"canonical_gap_id": "gap:comment-identity"}},
        {"evidence_record": {"canonical_gap_id": "gap:comment-record"}}
      ],
      "summary_only": [
        {"canonical_gap_id": "gap:summary-direct"}
      ],
      "suppressed": [
        {"canonical_gap_id": "gap:suppressed-direct"}
      ]
    });
    let index = baseline_index_from_value(&value);

    for expected in [
        "gap:direct",
        "gap:identity",
        "gap:record",
        "gap:decision-direct",
        "gap:decision-identity",
        "gap:decision-record",
        "gap:comment-direct",
        "gap:comment-identity",
        "gap:comment-record",
        "gap:summary-direct",
        "gap:suppressed-direct",
    ] {
        assert!(
            index.identities.contains(expected),
            "expected baseline identity {expected}"
        );
    }
}

#[test]
fn gate_candidate_reads_canonical_gap_id_from_supported_shapes() {
    for (value, expected) in [
        (json!({"canonical_gap_id": "gap:direct"}), "gap:direct"),
        (
            json!({"identity": {"canonical_gap_id": "gap:identity"}}),
            "gap:identity",
        ),
        (
            json!({"evidence_record": {"canonical_gap_id": "gap:record"}}),
            "gap:record",
        ),
    ] {
        assert_eq!(
            canonical_gap_id_from_value(&value).as_deref(),
            Some(expected)
        );
    }

    assert_eq!(canonical_gap_id_from_value(&json!({})), None);
}

#[test]
fn gate_mode_parse_covers_all_values_and_unknowns() {
    assert_eq!(GateMode::parse("visible-only"), Ok(GateMode::VisibleOnly));
    assert_eq!(
        GateMode::parse("acknowledgeable"),
        Ok(GateMode::Acknowledgeable)
    );
    assert_eq!(
        GateMode::parse("baseline-check"),
        Ok(GateMode::BaselineCheck)
    );
    assert_eq!(
        GateMode::parse("calibrated-gate"),
        Ok(GateMode::CalibratedGate)
    );
    assert_eq!(
        GateMode::parse("hard"),
        Err("unknown gate mode `hard`; expected `visible-only`, `acknowledgeable`, `baseline-check`, or `calibrated-gate`".to_string())
    );
}

#[test]
fn gate_optional_inputs_emit_warnings_and_markdown_sections() -> Result<(), String> {
    let dir = temp_dir("gate-optional-warnings")?;
    let invalid = write_temp_json(&dir, "invalid.json", "{")?;
    let mut input = fixture_input(GateMode::VisibleOnly);
    input.root = dir.clone();
    input.pr_guidance = Some(write_temp_json(&dir, "comments.json", PR_GUIDANCE_JSON)?);
    input.repo_exposure = Some(PathBuf::from("missing-repo.json"));
    input.sarif_policy = Some(
        invalid
            .strip_prefix(&dir)
            .map_err(|err| err.to_string())?
            .to_path_buf(),
    );
    input.labels_json = Some(input.sarif_policy.clone().unwrap_or_default());
    input.agent_verify = Some(PathBuf::from("missing-verify.json"));
    input.agent_receipt = Some(input.sarif_policy.clone().unwrap_or_default());
    input.recommendation_calibration = Some(PathBuf::from("missing-recommendation.json"));
    input.mutation_calibration = Some(input.sarif_policy.clone().unwrap_or_default());
    input.baseline = Some(input.sarif_policy.clone().unwrap_or_default());

    let report = build_gate_decision_report(&input)?;
    let mut warning_report = report.clone();
    warning_report
        .warnings
        .push("manual | warning\nwith newline".to_string());
    let markdown = render_gate_decision_markdown(&warning_report);

    assert_eq!(report.status, "advisory");
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.contains("optional repo_exposure"))
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.contains("optional labels_json"))
    );
    assert!(markdown.contains("## Warnings"));
    assert!(markdown.contains("manual \\| warning with newline"));
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

// ── `--exception-policy` ledger integration (#1442) ──

fn exception_ledger_toml(review_after: &str, expires: &str, due_review: &str) -> String {
    format!(
        "schema_version = 1\npolicy = \"quality-gate-exceptions\"\nstatus = \"active\"\ndue_review = \"{due_review}\"\n\n[[exception]]\nid = \"total-burndown\"\nkind = \"temporary_burndown\"\nscope = \"ripr_plus_total\"\nowner = \"proof-lane\"\nreason = \"Pre-existing gaps predate the gate.\"\nfinal_target = \"unresolved total = 0\"\nevidence = \"target/receipts/quality/ripr-plus.json\"\nremoval_criteria = \"final mode requires zero\"\ncreated = \"2026-01-01\"\nreview_after = \"{review_after}\"\nexpires = \"{expires}\"\n"
    )
}

fn exception_input(dir: &Path, ledger: &str) -> Result<GateEvaluateInput, String> {
    let guidance = write_temp_json(dir, "comments.json", PR_GUIDANCE_JSON)?;
    let policy = dir.join("quality-gate-exceptions.toml");
    fs::write(&policy, ledger).map_err(|err| format!("write ledger failed: {err}"))?;
    Ok(GateEvaluateInput {
        root: PathBuf::from("."),
        repo_exposure: None,
        pr_guidance: Some(guidance),
        gap_ledger: None,
        sarif_policy: None,
        labels_json: None,
        labels: Vec::new(),
        agent_verify: None,
        agent_receipt: None,
        recommendation_calibration: None,
        mutation_calibration: None,
        baseline: None,
        mode: GateMode::VisibleOnly,
        acknowledgement_labels: Vec::new(),
        exception_policy: Some(policy),
    })
}

#[test]
fn gate_exception_policy_active_ledger_reports_section_without_blocking() -> Result<(), String> {
    let dir = temp_dir("gate-exception-active")?;
    // Far-future dates keep this test independent of the wall clock.
    let input = exception_input(
        &dir,
        &exception_ledger_toml("9999-01-01", "9999-12-31", "fail"),
    )?;

    let report = build_gate_decision_report(&input)?;
    let json = render_gate_decision_json(&report)?;
    let markdown = render_gate_decision_markdown(&report);

    assert_ne!(report.status, "blocked");
    assert_ne!(report.status, "config_error");
    let value: Value =
        serde_json::from_str(&json).map_err(|err| format!("gate JSON should parse: {err}"))?;
    assert_eq!(value["exception_policy"]["active_count"], 1);
    assert_eq!(
        value["exception_policy"]["violations"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert!(value["inputs"]["exception_policy"].as_str().is_some());
    assert!(markdown.contains("## Exception Policy"));
    assert!(markdown.contains("total-burndown"));
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_exception_policy_expired_entry_blocks() -> Result<(), String> {
    let dir = temp_dir("gate-exception-expired")?;
    let input = exception_input(
        &dir,
        &exception_ledger_toml("2000-01-01", "2000-06-01", "fail"),
    )?;

    let report = build_gate_decision_report(&input)?;

    assert_eq!(report.status, "blocked");
    assert!(gate_decision_should_fail(&report));
    let json = render_gate_decision_json(&report)?;
    let value: Value =
        serde_json::from_str(&json).map_err(|err| format!("gate JSON should parse: {err}"))?;
    assert!(
        value["exception_policy"]["violations"]
            .as_array()
            .is_some_and(|violations| violations.iter().any(|violation| {
                violation["kind"] == "quality_exception_expired" && violation["blocking"] == true
            })),
        "violations: {}",
        value["exception_policy"]["violations"]
    );
    let markdown = render_gate_decision_markdown(&report);
    assert!(
        markdown.contains("quality\\_exception\\_expired")
            || markdown.contains("quality_exception_expired"),
        "markdown missing expired violation: {markdown}"
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_exception_policy_review_due_warn_surfaces_warning_not_block() -> Result<(), String> {
    let dir = temp_dir("gate-exception-review-warn")?;
    // review_after in the past, expires far in the future, due_review=warn.
    let input = exception_input(
        &dir,
        &exception_ledger_toml("2000-01-01", "9999-12-31", "warn"),
    )?;

    let report = build_gate_decision_report(&input)?;

    assert_ne!(report.status, "blocked");
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.contains("quality_exception_review_due")),
        "warnings: {:?}",
        report.warnings
    );
    // Under due_review=fail the same ledger blocks.
    let input = exception_input(
        &dir,
        &exception_ledger_toml("2000-01-01", "9999-12-31", "fail"),
    )?;
    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "blocked");
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_exception_policy_missing_or_malformed_ledger_is_config_error() -> Result<(), String> {
    let dir = temp_dir("gate-exception-config-error")?;
    let guidance = write_temp_json(&dir, "comments.json", PR_GUIDANCE_JSON)?;
    let mut input = GateEvaluateInput {
        root: PathBuf::from("."),
        repo_exposure: None,
        pr_guidance: Some(guidance),
        gap_ledger: None,
        sarif_policy: None,
        labels_json: None,
        labels: Vec::new(),
        agent_verify: None,
        agent_receipt: None,
        recommendation_calibration: None,
        mutation_calibration: None,
        baseline: None,
        mode: GateMode::VisibleOnly,
        acknowledgement_labels: Vec::new(),
        exception_policy: Some(dir.join("missing-ledger.toml")),
    };

    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "config_error");
    assert!(gate_decision_should_fail(&report));
    assert!(
        report
            .config_errors
            .iter()
            .any(|error| error.contains("failed to read exception policy")),
        "config_errors: {:?}",
        report.config_errors
    );

    let malformed = dir.join("malformed-ledger.toml");
    fs::write(&malformed, "schema_version = 2\npolicy = \"other\"\n")
        .map_err(|err| format!("write malformed ledger failed: {err}"))?;
    input.exception_policy = Some(malformed);
    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "config_error");

    // Without the flag, the report and JSON carry no exception fields —
    // existing gate-decision consumers and goldens see identical output.
    input.exception_policy = None;
    let report = build_gate_decision_report(&input)?;
    assert!(report.exception_policy.is_none());
    let json = render_gate_decision_json(&report)?;
    assert!(!json.contains("exception_policy"));
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_config_errors_render_markdown_and_fail_status() -> Result<(), String> {
    let input = GateEvaluateInput {
        root: repo_root(),
        repo_exposure: None,
        pr_guidance: Some(PathBuf::from("missing-comments.json")),
        gap_ledger: None,
        sarif_policy: None,
        labels_json: None,
        labels: Vec::new(),
        agent_verify: None,
        agent_receipt: None,
        recommendation_calibration: None,
        mutation_calibration: None,
        baseline: None,
        mode: GateMode::BaselineCheck,
        acknowledgement_labels: Vec::new(),
        exception_policy: None,
    };

    let report = build_gate_decision_report(&input)?;
    let markdown = render_gate_decision_markdown(&report);

    assert_eq!(report.status, "config_error");
    assert!(gate_decision_should_fail(&report));
    assert!(markdown.contains("## Config Errors"));
    assert!(markdown.contains("requires an explicit --baseline"));
    Ok(())
}

#[test]
fn gate_summary_only_and_suppressed_candidates_remain_visible() -> Result<(), String> {
    let dir = temp_dir("gate-summary-suppressed")?;
    let guidance = write_temp_json(&dir, "comments.json", SUMMARY_AND_SUPPRESSED_JSON)?;
    let mut input = fixture_input(GateMode::Acknowledgeable);
    input.root = dir.clone();
    input.pr_guidance = Some(
        guidance
            .strip_prefix(&dir)
            .map_err(|err| err.to_string())?
            .to_path_buf(),
    );

    let report = build_gate_decision_report(&input)?;

    assert_eq!(report.status, "advisory");
    assert_eq!(report.summary.suppressed, 1);
    assert_eq!(report.summary.advisory, 1);
    assert!(
        report
            .decisions
            .iter()
            .any(|decision| decision.gate_reason.contains("summary-only"))
    );
    assert!(
        report
            .decisions
            .iter()
            .any(|decision| decision.gate_reason.contains("configured-hidden"))
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_changed_test_and_missing_guidance_candidates_stay_advisory() -> Result<(), String> {
    let dir = temp_dir("gate-ineligible")?;
    let guidance = write_temp_json(&dir, "comments.json", INELIGIBLE_GUIDANCE_JSON)?;
    let mut input = fixture_input(GateMode::Acknowledgeable);
    input.root = dir.clone();
    input.pr_guidance = Some(
        guidance
            .strip_prefix(&dir)
            .map_err(|err| err.to_string())?
            .to_path_buf(),
    );

    let report = build_gate_decision_report(&input)?;

    assert_eq!(report.status, "advisory");
    assert_eq!(report.summary.blocking, 0);
    assert!(
        report
            .decisions
            .iter()
            .any(|decision| decision.gate_reason.contains("nearby focused test changed"))
    );
    let missing_guidance = write_temp_json(&dir, "missing.json", MISSING_GUIDANCE_JSON)?;
    input.pr_guidance = Some(
        missing_guidance
            .strip_prefix(&dir)
            .map_err(|err| err.to_string())?
            .to_path_buf(),
    );
    let report = build_gate_decision_report(&input)?;
    assert!(
        report
            .decisions
            .iter()
            .any(|decision| decision.gate_reason.contains("missing concrete"))
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_baseline_check_blocks_new_candidate() -> Result<(), String> {
    let dir = temp_dir("gate-baseline-new")?;
    let baseline = write_temp_json(&dir, "baseline.json", r#"{"decisions":[]}"#)?;
    let mut input = fixture_input(GateMode::BaselineCheck);
    input.baseline = Some(baseline);

    let report = build_gate_decision_report(&input)?;

    assert_eq!(report.status, "blocked");
    assert_eq!(report.summary.blocking, 1);
    assert!(report.decisions[0].gate_reason.contains("baseline-check"));
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_acknowledgeable_blocks_complete_gap_ledger_route_with_typed_seam_identity()
-> Result<(), String> {
    let dir = temp_dir("gate-gap-ledger-block")?;
    let gap_ledger = write_temp_json(&dir, "gap-ledger.json", GAP_LEDGER_BLOCKING_JSON)?;
    let input = GateEvaluateInput {
        root: dir.clone(),
        repo_exposure: None,
        pr_guidance: None,
        gap_ledger: Some(
            gap_ledger
                .strip_prefix(&dir)
                .map_err(|err| err.to_string())?
                .to_path_buf(),
        ),
        sarif_policy: None,
        labels_json: None,
        labels: Vec::new(),
        agent_verify: None,
        agent_receipt: None,
        recommendation_calibration: None,
        mutation_calibration: None,
        baseline: None,
        mode: GateMode::Acknowledgeable,
        acknowledgement_labels: Vec::new(),
        exception_policy: None,
    };

    let report = build_gate_decision_report(&input)?;
    let rendered = render_gate_decision_json(&report)?;
    let value: Value = serde_json::from_str(&rendered)
        .map_err(|err| format!("gate decision JSON should parse: {err}"))?;

    assert_eq!(report.status, "blocked");
    assert_eq!(report.summary.blocking, 1);
    assert_eq!(report.summary.advisory, 0);
    assert_eq!(report.decisions[0].decision, "blocking");
    assert!(gate_decision_should_fail(&report));
    assert_eq!(report.decisions[0].source, "gap_decision_ledger");
    assert_eq!(report.decisions[0].gap_id.as_deref(), Some("gap:pricing"));
    assert_eq!(
        value["inputs"]["gap_ledger"], "gap-ledger.json",
        "gate report should name the explicit gap ledger input"
    );
    assert_eq!(
        value["decisions"][0]["gap_kind"],
        "MissingBoundaryAssertion"
    );
    assert_eq!(
        value["decisions"][0]["evidence"]["repair_route"]["route_kind"],
        "AddBoundaryAssertion"
    );
    assert_eq!(
        value["decisions"][0]["repair_route"]["seam_id"],
        "seam-pricing-threshold"
    );
    assert_eq!(
        value["decisions"][0]["repair_route"]["inspection_command"],
        "ripr agent brief --root . --seam-id seam-pricing-threshold --json"
    );
    assert_eq!(
        value["decisions"][0]["evidence"]["verification_commands"][0],
        "cargo xtask fixtures boundary_gap"
    );
    assert_eq!(
        value["decisions"][0]["evidence"]["candidate_values"],
        Value::Array(Vec::new()),
        "gap ledger records do not carry test input variants"
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn conflicting_gap_ledger_seam_identities_fail_closed_as_config_error() -> Result<(), String> {
    let dir = temp_dir("gate-gap-ledger-conflicting-seams")?;
    let mut value: Value = serde_json::from_str(GAP_LEDGER_BLOCKING_JSON)
        .map_err(|err| format!("complete fixture should parse: {err}"))?;
    let record = value["gap_records"][0].clone();
    let mut conflicting = record;
    conflicting["seam_id"] = Value::String("seam-other".to_string());
    value["gap_records"] = Value::Array(vec![value["gap_records"][0].clone(), conflicting]);
    let gap_ledger = write_temp_json(
        &dir,
        "gap-ledger.json",
        &serde_json::to_string(&value).map_err(|err| err.to_string())?,
    )?;
    let input = GateEvaluateInput {
        root: dir.clone(),
        repo_exposure: None,
        pr_guidance: None,
        gap_ledger: Some(
            gap_ledger
                .strip_prefix(&dir)
                .map_err(|err| err.to_string())?
                .to_path_buf(),
        ),
        sarif_policy: None,
        labels_json: None,
        labels: Vec::new(),
        agent_verify: None,
        agent_receipt: None,
        recommendation_calibration: None,
        mutation_calibration: None,
        baseline: None,
        mode: GateMode::Acknowledgeable,
        acknowledgement_labels: Vec::new(),
        exception_policy: None,
    };

    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "config_error");
    assert_eq!(report.summary.evaluated, 0);
    assert!(gate_decision_should_fail(&report));
    assert!(report.config_errors.iter().any(|error| {
        error.contains("conflicting seam identities for canonical gap pricing::discount::threshold")
    }));
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn multiple_legacy_gap_ledger_records_remain_route_limited() -> Result<(), String> {
    let dir = temp_dir("gate-gap-ledger-legacy-duplicates")?;
    let mut value: Value = serde_json::from_str(&legacy_gap_ledger_json()?)
        .map_err(|err| format!("legacy fixture should parse: {err}"))?;
    let record = value["gap_records"][0].clone();
    value["gap_records"] = Value::Array(vec![record.clone(), record]);
    let gap_ledger = write_temp_json(
        &dir,
        "gap-ledger.json",
        &serde_json::to_string(&value).map_err(|err| err.to_string())?,
    )?;
    let input = GateEvaluateInput {
        root: dir.clone(),
        repo_exposure: None,
        pr_guidance: None,
        gap_ledger: Some(
            gap_ledger
                .strip_prefix(&dir)
                .map_err(|err| err.to_string())?
                .to_path_buf(),
        ),
        sarif_policy: None,
        labels_json: None,
        labels: Vec::new(),
        agent_verify: None,
        agent_receipt: None,
        recommendation_calibration: None,
        mutation_calibration: None,
        baseline: None,
        mode: GateMode::Acknowledgeable,
        acknowledgement_labels: Vec::new(),
        exception_policy: None,
    };

    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "advisory");
    assert!(report.config_errors.is_empty());
    assert_eq!(report.summary.evaluated, 2);
    assert!(report.decisions.iter().all(|decision| {
        decision.decision == "advisory" && decision.gate_reason.contains("incomplete_repair_route")
    }));
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_gap_ledger_static_unknown_only_stays_report_only() -> Result<(), String> {
    let dir = temp_dir("gate-gap-ledger-report-only")?;
    let gap_ledger = write_temp_json(&dir, "gap-ledger.json", GAP_LEDGER_REPORT_ONLY_JSON)?;
    let input = GateEvaluateInput {
        root: dir.clone(),
        repo_exposure: None,
        pr_guidance: None,
        gap_ledger: Some(
            gap_ledger
                .strip_prefix(&dir)
                .map_err(|err| err.to_string())?
                .to_path_buf(),
        ),
        sarif_policy: None,
        labels_json: None,
        labels: Vec::new(),
        agent_verify: None,
        agent_receipt: None,
        recommendation_calibration: None,
        mutation_calibration: None,
        baseline: None,
        mode: GateMode::Acknowledgeable,
        acknowledgement_labels: Vec::new(),
        exception_policy: None,
    };

    let report = build_gate_decision_report(&input)?;

    assert_eq!(report.status, "pass");
    assert_eq!(report.summary.blocking, 0);
    assert_eq!(report.summary.not_applicable, 1);
    assert!(
        report.decisions[0]
            .gate_reason
            .contains("not gate-candidate eligible")
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_labels_array_supports_custom_acknowledgement_label() -> Result<(), String> {
    let dir = temp_dir("gate-label-array")?;
    let labels = write_temp_json(&dir, "labels.json", r#"["accepted-risk"]"#)?;
    let mut input = fixture_input(GateMode::Acknowledgeable);
    input.labels_json = Some(labels);
    input.acknowledgement_labels = vec!["accepted-risk".to_string()];

    let report = build_gate_decision_report(&input)?;

    assert_eq!(report.status, "acknowledged");
    assert_eq!(
        report.decisions[0].policy.acknowledgement_label.as_deref(),
        Some("accepted-risk")
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_calibration_can_keep_candidates_advisory() -> Result<(), String> {
    let dir = temp_dir("gate-calibration-advisory")?;
    let baseline = write_temp_json(&dir, "baseline.json", r#"{"decisions":[]}"#)?;
    let recommendation = write_temp_json(
        &dir,
        "recommendation.json",
        r#"{"recommendations":[{"id":"ripr-review-8f7fa8644fd12280","calibration":{"outcome":"wrong_target"}}]}"#,
    )?;
    let mutation = write_temp_json(
        &dir,
        "mutation.json",
        r#"{
              "matches": [
                {
                  "static": {"seam_id": "other-seam"},
                  "runtime": {"runtime_outcome": "caught"}
                }
              ],
              "static_only_findings": [
                {"static": {"seam_id": "8f7fa8644fd12280"}}
              ],
              "ambiguous_file_line_matches": [{"file":"src/lib.rs","line":7}]
            }"#,
    )?;
    let mut input = fixture_input(GateMode::CalibratedGate);
    input.baseline = Some(baseline);
    input.recommendation_calibration = Some(recommendation);
    input.mutation_calibration = Some(mutation);

    let report = build_gate_decision_report(&input)?;

    assert_eq!(report.status, "advisory");
    assert_eq!(
        report.decisions[0]
            .evidence
            .recommendation_calibration
            .confidence_effect,
        "keeps_advisory"
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.contains("ambiguous file/line"))
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn gate_markdown_projects_complete_repair_route_for_ci_summary() -> Result<(), String> {
    let input = fixture_input(GateMode::Acknowledgeable);
    let report = build_gate_decision_report(&input)?;
    let rendered = render_gate_decision_markdown(&report);

    for (label, expected) in [
        ("gap identity", "  - Gap: `gap:dedf923a13a00573`"),
        ("seam identity", "  - Seam: `8f7fa8644fd12280`"),
        ("gap state", "  - Gap state: `actionable`"),
        ("classification", "  - Classification: `weakly_gripped`"),
        (
            "changed owner",
            "  - Changed owner: `pricing::discounted_total`",
        ),
        (
            "changed behavior",
            "  - Changed behavior: amount >= discount_threshold",
        ),
        (
            "missing discriminator",
            "  - Why it remains open: amount == discount_threshold",
        ),
        (
            "related test",
            "  - Near test: `above_threshold_gets_discount` at `tests/pricing.rs:12`",
        ),
        (
            "inspection command",
            "  - Inspect: `ripr agent brief --root . --seam-id 8f7fa8644fd12280 --json > target/ripr/workflow/agent-brief.json`",
        ),
        (
            "authority boundary",
            "  - Boundary: `static_ripr_evidence_only`",
        ),
    ] {
        require_contains(&rendered, expected, label)?;
    }
    require_contains(
        &rendered,
        "  - Add: Write one focused Rust test",
        "test intent",
    )?;
    require_contains(
        &rendered,
        "  - Verify: `ripr agent verify",
        "verify command",
    )?;
    require_contains(
        &rendered,
        "  - Receipt: `ripr agent receipt",
        "receipt command",
    )
}

#[test]
fn gate_markdown_projects_incomplete_route_limitation_without_fabricated_command()
-> Result<(), String> {
    let mut input = fixture_input(GateMode::Acknowledgeable);
    input.pr_guidance = Some(PathBuf::from(
        "fixtures/boundary_gap/expected/calibrated-gate/summary-and-suppressed/pr-guidance.json",
    ));
    let report = build_gate_decision_report(&input)?;
    let rendered = render_gate_decision_markdown(&report);

    require_contains(
        &rendered,
        "  - Repair route limitation: `incomplete_repair_route`",
        "limitation kind",
    )?;
    require_contains(
        &rendered,
        "  - Missing route fields: `canonical_gap_id, gap_state, changed_owner, changed_behavior, repair_target, test_intent, verify_command, receipt_command, inspection_command`",
        "limitation missing fields",
    )?;
    require_not_contains(
        &rendered,
        "  - Inspect:",
        "incomplete route inspection command",
    )
}

#[test]
fn calibrated_gate_fixture_matrix_matches_checked_outputs() -> Result<(), String> {
    let cases = [
        GateFixtureCase {
            name: "visible-only-advisory",
            mode: GateMode::VisibleOnly,
            pr_guidance: "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
            labels_json: None,
            labels: &[],
            recommendation_calibration: None,
            mutation_calibration: None,
            baseline: None,
        },
        GateFixtureCase {
            name: "acknowledged-waiver",
            mode: GateMode::Acknowledgeable,
            pr_guidance: "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
            labels_json: Some(
                "fixtures/boundary_gap/expected/calibrated-gate/acknowledged-waiver/labels.json",
            ),
            labels: &["ripr-waive"],
            recommendation_calibration: None,
            mutation_calibration: None,
            baseline: None,
        },
        GateFixtureCase {
            name: "baseline-check-existing",
            mode: GateMode::BaselineCheck,
            pr_guidance: "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
            labels_json: None,
            labels: &[],
            recommendation_calibration: None,
            mutation_calibration: None,
            baseline: Some(
                "fixtures/boundary_gap/expected/calibrated-gate/baseline-check-existing/baseline.json",
            ),
        },
        GateFixtureCase {
            name: "calibrated-high-confidence-new-gap",
            mode: GateMode::CalibratedGate,
            pr_guidance: "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
            labels_json: None,
            labels: &[],
            recommendation_calibration: Some(
                "fixtures/boundary_gap/expected/recommendation-calibration/recommendation-calibration.json",
            ),
            mutation_calibration: None,
            baseline: Some(
                "fixtures/boundary_gap/expected/calibrated-gate/calibrated-high-confidence-new-gap/baseline.json",
            ),
        },
        GateFixtureCase {
            name: "summary-and-suppressed",
            mode: GateMode::Acknowledgeable,
            pr_guidance: "fixtures/boundary_gap/expected/calibrated-gate/summary-and-suppressed/pr-guidance.json",
            labels_json: None,
            labels: &[],
            recommendation_calibration: None,
            mutation_calibration: None,
            baseline: None,
        },
        GateFixtureCase {
            name: "missing-input",
            mode: GateMode::BaselineCheck,
            pr_guidance: "fixtures/boundary_gap/expected/calibrated-gate/missing-input/missing-comments.json",
            labels_json: None,
            labels: &[],
            recommendation_calibration: None,
            mutation_calibration: None,
            baseline: Some(
                "fixtures/boundary_gap/expected/calibrated-gate/baseline-check-existing/baseline.json",
            ),
        },
        GateFixtureCase {
            name: "calibration-disagreement",
            mode: GateMode::CalibratedGate,
            pr_guidance: "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
            labels_json: None,
            labels: &[],
            recommendation_calibration: Some(
                "fixtures/boundary_gap/expected/calibrated-gate/calibration-disagreement/recommendation-calibration.json",
            ),
            mutation_calibration: Some(
                "fixtures/boundary_gap/expected/calibrated-gate/calibration-disagreement/mutation-calibration.json",
            ),
            baseline: Some(
                "fixtures/boundary_gap/expected/calibrated-gate/calibration-disagreement/baseline.json",
            ),
        },
    ];

    for case in cases {
        let input = case.input();
        let mut report = build_gate_decision_report(&input)?;
        report.root = ".".to_string();
        let rendered_json = render_gate_decision_json(&report)?;
        let rendered_md = render_gate_decision_markdown(&report);
        let expected_dir =
            PathBuf::from("fixtures/boundary_gap/expected/calibrated-gate").join(case.name);
        assert_repo_fixture(
            &expected_dir.join("gate-decision.json"),
            &rendered_json,
            &format!("{} JSON", case.name),
        )?;
        assert_repo_fixture(
            &expected_dir.join("gate-decision.md"),
            &rendered_md,
            &format!("{} Markdown", case.name),
        )?;
    }

    Ok(())
}

/// Adversarial baseline-fallback corpus (issue #1934, RIPR-SPEC-0014 §
/// Baseline Comparison): each scenario pins whether a legacy
/// `path:line:static_class` fallback-only match is disclosed (warning +
/// `baseline_match_kind`) or a canonical match stays silent. See
/// `fixtures/gate_baseline_fallback_disclosure/expected/gate-baseline/README.md`.
#[test]
fn baseline_fallback_disclosure_fixture_matrix_matches_checked_outputs() -> Result<(), String> {
    let corpus = PathBuf::from("fixtures/gate_baseline_fallback_disclosure/expected/gate-baseline");
    for scenario in [
        "fallback-only-new-canonical",
        "line-moved-canonical-match",
        "two-gaps-one-line-same-class",
        "missing-canonical-identity",
        "mixed-canonical-and-legacy-entries",
    ] {
        let dir = corpus.join(scenario);
        let input = GateEvaluateInput {
            root: repo_root(),
            repo_exposure: None,
            pr_guidance: Some(dir.join("pr-guidance.json")),
            gap_ledger: None,
            sarif_policy: None,
            labels_json: None,
            labels: Vec::new(),
            agent_verify: None,
            agent_receipt: None,
            recommendation_calibration: None,
            mutation_calibration: None,
            baseline: Some(dir.join("baseline.json")),
            mode: GateMode::BaselineCheck,
            acknowledgement_labels: Vec::new(),
            exception_policy: None,
        };
        let mut report = build_gate_decision_report(&input)?;
        report.root = ".".to_string();
        let rendered_json = render_gate_decision_json(&report)?;
        let rendered_md = render_gate_decision_markdown(&report);
        assert_repo_fixture(
            &dir.join("gate-decision.json"),
            &rendered_json,
            &format!("{scenario} JSON"),
        )?;
        assert_repo_fixture(
            &dir.join("gate-decision.md"),
            &rendered_md,
            &format!("{scenario} Markdown"),
        )?;
    }

    Ok(())
}

#[test]
fn display_path_normalizes_empty_and_dot_prefixed_paths() {
    assert_eq!(display_path(Path::new("")), ".");
    assert_eq!(
        display_path(Path::new("./target/out.json")),
        "target/out.json"
    );
}

// -- coverage-gap tests --

#[test]
fn given_both_pr_guidance_and_gap_ledger_missing_when_evaluated_then_config_error()
-> Result<(), String> {
    let input = GateEvaluateInput {
        root: repo_root(),
        repo_exposure: None,
        pr_guidance: None,
        gap_ledger: None,
        sarif_policy: None,
        labels_json: None,
        labels: Vec::new(),
        agent_verify: None,
        agent_receipt: None,
        recommendation_calibration: None,
        mutation_calibration: None,
        baseline: None,
        mode: GateMode::VisibleOnly,
        acknowledgement_labels: Vec::new(),
        exception_policy: None,
    };

    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "config_error");
    assert!(gate_decision_should_fail(&report));
    assert!(
        report
            .config_errors
            .iter()
            .any(|error| error.contains("--pr-guidance") && error.contains("--gap-ledger")),
        "expected combined input requirement message, got {:?}",
        report.config_errors,
    );
    Ok(())
}

#[test]
fn given_invalid_gap_ledger_json_when_evaluated_then_config_error_includes_parse_failure()
-> Result<(), String> {
    let dir = temp_dir("gate-gap-ledger-invalid-json")?;
    let gap_ledger = write_temp_json(&dir, "gap-ledger.json", "{not valid json")?;
    let input = GateEvaluateInput {
        root: dir.clone(),
        repo_exposure: None,
        pr_guidance: None,
        gap_ledger: Some(
            gap_ledger
                .strip_prefix(&dir)
                .map_err(|err| err.to_string())?
                .to_path_buf(),
        ),
        sarif_policy: None,
        labels_json: None,
        labels: Vec::new(),
        agent_verify: None,
        agent_receipt: None,
        recommendation_calibration: None,
        mutation_calibration: None,
        baseline: None,
        mode: GateMode::Acknowledgeable,
        acknowledgement_labels: Vec::new(),
        exception_policy: None,
    };

    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "config_error");
    assert!(
        report
            .config_errors
            .iter()
            .any(|error| error.contains("gap decision ledger")
                && error.contains("is invalid")
                && !error.contains("read failed")),
        "expected parse-failure config error, got {:?}",
        report.config_errors,
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn given_unreadable_gap_ledger_when_evaluated_then_config_error_includes_read_failure()
-> Result<(), String> {
    let dir = temp_dir("gate-gap-ledger-unreadable")?;
    let gap_ledger_dir = dir.join("gap-ledger.json");
    fs::create_dir_all(&gap_ledger_dir)
        .map_err(|err| format!("create gap-ledger dir failed: {err}"))?;
    let input = GateEvaluateInput {
        root: dir.clone(),
        repo_exposure: None,
        pr_guidance: None,
        gap_ledger: Some(
            gap_ledger_dir
                .strip_prefix(&dir)
                .map_err(|err| err.to_string())?
                .to_path_buf(),
        ),
        sarif_policy: None,
        labels_json: None,
        labels: Vec::new(),
        agent_verify: None,
        agent_receipt: None,
        recommendation_calibration: None,
        mutation_calibration: None,
        baseline: None,
        mode: GateMode::Acknowledgeable,
        acknowledgement_labels: Vec::new(),
        exception_policy: None,
    };

    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "config_error");
    assert!(
        report
            .config_errors
            .iter()
            .any(|error| error.contains("gap decision ledger") && error.contains("read failed")),
        "expected read-failure config error, got {:?}",
        report.config_errors,
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn given_unreadable_baseline_in_baseline_mode_then_config_error_includes_invalid_baseline()
-> Result<(), String> {
    let dir = temp_dir("gate-baseline-unreadable")?;
    let baseline_dir = dir.join("baseline.json");
    fs::create_dir_all(&baseline_dir)
        .map_err(|err| format!("create baseline dir failed: {err}"))?;
    let mut input = fixture_input(GateMode::BaselineCheck);
    input.baseline = Some(baseline_dir);

    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "config_error");
    assert!(
        report
            .config_errors
            .iter()
            .any(|error| error.contains("required baseline") && error.contains("is invalid")),
        "expected required-baseline-invalid config error, got {:?}",
        report.config_errors,
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn given_recommendation_calibration_with_unknown_outcome_then_confidence_effect_is_unknown()
-> Result<(), String> {
    let dir = temp_dir("gate-recommendation-unknown-outcome")?;
    let baseline = write_temp_json(&dir, "baseline.json", r#"{"decisions":[]}"#)?;
    let recommendation = write_temp_json(
        &dir,
        "recommendation.json",
        r#"{
              "recommendations": [
                {
                  "id": "ripr-review-8f7fa8644fd12280",
                  "calibration": {"outcome": "novel-outcome"}
                }
              ]
            }"#,
    )?;
    let mut input = fixture_input(GateMode::CalibratedGate);
    input.baseline = Some(baseline);
    input.recommendation_calibration = Some(recommendation);

    let report = build_gate_decision_report(&input)?;
    assert_eq!(
        report.decisions[0]
            .evidence
            .recommendation_calibration
            .confidence_effect,
        "unknown"
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn given_mutation_calibration_with_unknown_outcome_then_confidence_effect_is_unknown()
-> Result<(), String> {
    let dir = temp_dir("gate-mutation-unknown-outcome")?;
    let baseline = write_temp_json(&dir, "baseline.json", r#"{"decisions":[]}"#)?;
    let mutation = write_temp_json(
        &dir,
        "mutation.json",
        r#"{
              "matches": [
                {
                  "static": {"seam_id": "8f7fa8644fd12280"},
                  "runtime": {"runtime_outcome": "novel-mutation-outcome"}
                }
              ]
            }"#,
    )?;
    let mut input = fixture_input(GateMode::CalibratedGate);
    input.baseline = Some(baseline);
    input.mutation_calibration = Some(mutation);

    let report = build_gate_decision_report(&input)?;
    assert_eq!(
        report.decisions[0]
            .evidence
            .mutation_calibration
            .confidence_effect,
        "unknown"
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn given_mutation_calibration_match_without_outcome_then_confidence_effect_is_not_used()
-> Result<(), String> {
    let dir = temp_dir("gate-mutation-missing-outcome")?;
    let baseline = write_temp_json(&dir, "baseline.json", r#"{"decisions":[]}"#)?;
    let mutation = write_temp_json(
        &dir,
        "mutation.json",
        r#"{
              "matches": [
                {
                  "static": {"seam_id": "8f7fa8644fd12280"},
                  "runtime": {}
                }
              ]
            }"#,
    )?;
    let mut input = fixture_input(GateMode::CalibratedGate);
    input.baseline = Some(baseline);
    input.mutation_calibration = Some(mutation);

    let report = build_gate_decision_report(&input)?;
    assert_eq!(
        report.decisions[0]
            .evidence
            .mutation_calibration
            .confidence_effect,
        "not_used"
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn given_mutation_calibration_match_without_seam_id_then_match_is_skipped() -> Result<(), String> {
    let dir = temp_dir("gate-mutation-no-seam-id")?;
    let baseline = write_temp_json(&dir, "baseline.json", r#"{"decisions":[]}"#)?;
    let mutation = write_temp_json(
        &dir,
        "mutation.json",
        r#"{
              "matches": [
                {
                  "static": {},
                  "runtime": {"runtime_outcome": "missed"}
                }
              ]
            }"#,
    )?;
    let mut input = fixture_input(GateMode::CalibratedGate);
    input.baseline = Some(baseline);
    input.mutation_calibration = Some(mutation);

    let report = build_gate_decision_report(&input)?;
    assert_eq!(
        report.decisions[0]
            .evidence
            .mutation_calibration
            .confidence_effect,
        "not_used",
        "match without seam_id must not populate the mutation calibration index",
    );
    assert_eq!(report.status, "advisory");
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn given_guidance_with_recommended_file_only_then_recommended_test_is_file_path()
-> Result<(), String> {
    let dir = temp_dir("gate-recommended-file-only")?;
    let guidance = write_temp_json(
        &dir,
        "comments.json",
        r#"{
              "schema_version": "0.1",
              "summary": {"unchanged_tests": true},
              "comments": [
                {
                  "id": "ripr-review-file-only",
                  "seam_id": "file-only-seam",
                  "grip_class": "weakly_gripped",
                  "severity": "warning",
                  "missing_discriminator": "amount == discount_threshold",
                  "placement": {"path": "src/pricing.rs", "line": 88},
                  "suggested_test": {
                    "recommended_file": "tests/pricing.rs",
                    "candidate_values": ["amount == discount_threshold"]
                  }
                }
              ],
              "summary_only": [],
              "suppressed": []
            }"#,
    )?;
    let mut input = fixture_input(GateMode::VisibleOnly);
    input.root = dir.clone();
    input.pr_guidance = Some(
        guidance
            .strip_prefix(&dir)
            .map_err(|err| err.to_string())?
            .to_path_buf(),
    );

    let report = build_gate_decision_report(&input)?;
    assert_eq!(
        report.decisions[0].evidence.recommended_test.as_deref(),
        Some("tests/pricing.rs"),
        "with no near_test the recommended file alone becomes the recommended test path",
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn given_candidate_without_any_identity_then_baseline_identity_uses_path_line_class_fallback() {
    let candidate = GateCandidate {
        source: "pr_guidance".to_string(),
        source_id: String::new(),
        gap_id: None,
        gap_kind: None,
        canonical_gap_id: None,
        seam_id: None,
        gap_state: None,
        static_class: Some("weakly_gripped".to_string()),
        severity: Some("warning".to_string()),
        placement: GatePlacement {
            path: Some("src/pricing.rs".to_string()),
            line: Some(88),
        },
        missing_discriminator: None,
        route_facts: GateRouteFacts::default(),
        assertion_shape: None,
        candidate_values: Vec::new(),
        recommended_test: None,
        repair_route: None,
        verification_commands: Vec::new(),
        nearby_test_changed: false,
        suppressed: false,
        configured_off: false,
        suppression_reason: None,
        summary_reason: None,
        gap_ledger_gate_candidate: false,
        gap_ledger_gate_reason: None,
        gap_ledger_safe_gate_predicate: false,
    };

    assert_eq!(
        baseline_identity(&candidate).as_deref(),
        Some("src/pricing.rs:88:weakly_gripped"),
        "fallback identity must encode file:line:class when no stable id exists",
    );

    let mut without_class = candidate.clone();
    without_class.static_class = None;
    assert_eq!(
        baseline_identity(&without_class).as_deref(),
        Some("src/pricing.rs:88:unknown"),
        "fallback identity tags missing class as `unknown`",
    );

    let mut without_placement = candidate;
    without_placement.placement = GatePlacement {
        path: None,
        line: None,
    };
    assert!(
        baseline_identity(&without_placement).is_none(),
        "without placement or id the fallback cannot synthesize an identity",
    );
}

#[test]
fn given_calibrated_gate_with_mutation_keeps_advisory_then_gate_reason_cites_mutation_calibration()
-> Result<(), String> {
    let dir = temp_dir("gate-mutation-keeps-advisory-reason")?;
    let baseline = write_temp_json(&dir, "baseline.json", r#"{"decisions":[]}"#)?;
    let mutation = write_temp_json(
        &dir,
        "mutation.json",
        r#"{
              "matches": [
                {
                  "static": {"seam_id": "8f7fa8644fd12280"},
                  "runtime": {"runtime_outcome": "caught"}
                }
              ]
            }"#,
    )?;
    let mut input = fixture_input(GateMode::CalibratedGate);
    input.baseline = Some(baseline);
    input.mutation_calibration = Some(mutation);

    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "advisory");
    assert_eq!(report.decisions[0].decision, "advisory");
    assert!(
        report.decisions[0]
            .gate_reason
            .contains("imported mutation calibration keeps this candidate advisory"),
        "expected mutation-calibration advisory reason, got {:?}",
        report.decisions[0].gate_reason,
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn given_calibrated_gate_without_any_calibration_then_gate_reason_falls_through_to_default_advisory()
-> Result<(), String> {
    let dir = temp_dir("gate-calibrated-no-calibration-default")?;
    let baseline = write_temp_json(&dir, "baseline.json", r#"{"decisions":[]}"#)?;
    let mut input = fixture_input(GateMode::CalibratedGate);
    input.baseline = Some(baseline);

    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "advisory");
    assert_eq!(report.decisions[0].decision, "advisory");
    assert_eq!(
        report.decisions[0].gate_reason, "candidate remains advisory under current policy inputs",
        "with neither calibration available the default advisory reason applies",
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn given_gap_ledger_record_with_eligible_projection_but_unsafe_predicate_then_reason_cites_predicate()
-> Result<(), String> {
    let dir = temp_dir("gate-gap-ledger-unsafe-predicate")?;
    let gap_ledger = write_temp_json(
        &dir,
        "gap-ledger.json",
        r#"{
              "gap_records": [
                {
                  "gap_id": "gap:pricing",
                  "canonical_gap_id": "pricing::discount::unsafe",
                  "kind": "MissingBoundaryAssertion",
                  "language": "rust",
                  "language_status": "stable",
                  "scope": "pr_local",
                  "evidence_class": "weakly_exposed",
                  "gap_state": "actionable",
                  "policy_state": "new",
                  "repairability": "repairable",
                  "repair_route": {
                    "route_kind": "AddBoundaryAssertion",
                    "target_file": "tests/pricing.rs",
                    "related_test": "tests/pricing.rs::above_threshold_gets_discount",
                    "assertion_shape": "assert_eq!(price(threshold), discounted)",
                    "changed_behavior": "amount == discount_threshold"
                  },
                  "anchor": {
                    "file": "src/pricing.rs",
                    "line": 88,
                    "owner": "price",
                    "dedupe_fingerprint": "gap:pricing"
                  },
                  "projection_eligibility": {
                    "gate_candidate": {
                      "eligible": true,
                      "reason": "new_repairable_pr_local_gap"
                    }
                  },
                  "verification_commands": ["cargo xtask fixtures boundary_gap"],
                  "safe_gate_predicate": {
                    "policy_target_enabled": false,
                    "suppressed": false,
                    "waived": false,
                    "acknowledged_only": false,
                    "baseline_known": false,
                    "preview_language": false,
                    "static_unknown_only": false
                  }
                }
              ]
            }"#,
    )?;
    let input = GateEvaluateInput {
        root: dir.clone(),
        repo_exposure: None,
        pr_guidance: None,
        gap_ledger: Some(
            gap_ledger
                .strip_prefix(&dir)
                .map_err(|err| err.to_string())?
                .to_path_buf(),
        ),
        sarif_policy: None,
        labels_json: None,
        labels: Vec::new(),
        agent_verify: None,
        agent_receipt: None,
        recommendation_calibration: None,
        mutation_calibration: None,
        baseline: None,
        mode: GateMode::Acknowledgeable,
        acknowledgement_labels: Vec::new(),
        exception_policy: None,
    };

    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.decisions[0].decision, "not_applicable");
    assert!(
        report.decisions[0]
            .gate_reason
            .contains("safe gate predicate"),
        "expected safe-gate-predicate reason, got {:?}",
        report.decisions[0].gate_reason,
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn given_gap_ledger_record_with_safe_predicate_but_missing_anchor_then_reason_cites_anchor()
-> Result<(), String> {
    let dir = temp_dir("gate-gap-ledger-missing-anchor")?;
    let gap_ledger = write_temp_json(
        &dir,
        "gap-ledger.json",
        r#"{
              "gap_records": [
                {
                  "gap_id": "gap:pricing",
                  "canonical_gap_id": "pricing::discount::no_anchor",
                  "kind": "MissingBoundaryAssertion",
                  "language": "rust",
                  "language_status": "stable",
                  "scope": "pr_local",
                  "evidence_class": "weakly_exposed",
                  "gap_state": "actionable",
                  "policy_state": "new",
                  "repairability": "repairable",
                  "repair_route": {
                    "route_kind": "AddBoundaryAssertion",
                    "target_file": "tests/pricing.rs",
                    "related_test": "tests/pricing.rs::above_threshold_gets_discount",
                    "assertion_shape": "assert_eq!(price(threshold), discounted)",
                    "changed_behavior": "amount == discount_threshold"
                  },
                  "projection_eligibility": {
                    "gate_candidate": {
                      "eligible": true,
                      "reason": "new_repairable_pr_local_gap"
                    }
                  },
                  "verification_commands": ["cargo xtask fixtures boundary_gap"],
                  "safe_gate_predicate": {
                    "policy_target_enabled": true,
                    "suppressed": false,
                    "waived": false,
                    "acknowledged_only": false,
                    "baseline_known": false,
                    "preview_language": false,
                    "static_unknown_only": false
                  }
                }
              ]
            }"#,
    )?;
    let input = GateEvaluateInput {
        root: dir.clone(),
        repo_exposure: None,
        pr_guidance: None,
        gap_ledger: Some(
            gap_ledger
                .strip_prefix(&dir)
                .map_err(|err| err.to_string())?
                .to_path_buf(),
        ),
        sarif_policy: None,
        labels_json: None,
        labels: Vec::new(),
        agent_verify: None,
        agent_receipt: None,
        recommendation_calibration: None,
        mutation_calibration: None,
        baseline: None,
        mode: GateMode::Acknowledgeable,
        acknowledgement_labels: Vec::new(),
        exception_policy: None,
    };

    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.decisions[0].decision, "not_applicable");
    assert!(
        report.decisions[0]
            .gate_reason
            .contains("stable file and line anchor"),
        "expected missing-anchor reason, got {:?}",
        report.decisions[0].gate_reason,
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn given_gap_ledger_record_without_typed_seam_identity_then_baseline_check_fails_closed()
-> Result<(), String> {
    let dir = temp_dir("gate-gap-ledger-baseline-check-new")?;
    let legacy = legacy_gap_ledger_json()?;
    let gap_ledger = write_temp_json(&dir, "gap-ledger.json", &legacy)?;
    let baseline = write_temp_json(&dir, "baseline.json", r#"{"decisions":[]}"#)?;
    let input = GateEvaluateInput {
        root: dir.clone(),
        repo_exposure: None,
        pr_guidance: None,
        gap_ledger: Some(
            gap_ledger
                .strip_prefix(&dir)
                .map_err(|err| err.to_string())?
                .to_path_buf(),
        ),
        sarif_policy: None,
        labels_json: None,
        labels: Vec::new(),
        agent_verify: None,
        agent_receipt: None,
        recommendation_calibration: None,
        mutation_calibration: None,
        baseline: Some(
            baseline
                .strip_prefix(&dir)
                .map_err(|err| err.to_string())?
                .to_path_buf(),
        ),
        mode: GateMode::BaselineCheck,
        acknowledgement_labels: Vec::new(),
        exception_policy: None,
    };

    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.status, "advisory");
    assert_eq!(report.decisions[0].decision, "advisory");
    assert_eq!(report.decisions[0].source, "gap_decision_ledger");
    assert!(
        report.decisions[0]
            .gate_reason
            .contains("incomplete_repair_route"),
        "expected fail-closed route reason, got {:?}",
        report.decisions[0].gate_reason,
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn given_class_not_policy_eligible_with_concrete_guidance_then_reason_cites_class_or_placement_scope()
-> Result<(), String> {
    let dir = temp_dir("gate-class-not-policy-eligible")?;
    let guidance = write_temp_json(
        &dir,
        "comments.json",
        r#"{
              "schema_version": "0.1",
              "summary": {"unchanged_tests": true},
              "comments": [
                {
                  "id": "ripr-review-ungrippable",
                  "seam_id": "ungrippable-seam",
                  "grip_class": "off_seam",
                  "severity": "warning",
                  "missing_discriminator": "amount == discount_threshold",
                  "placement": {"path": "src/pricing.rs", "line": 88},
                  "suggested_test": {
                    "candidate_values": ["amount == discount_threshold"],
                    "near_test": "above_threshold_gets_discount"
                  }
                }
              ],
              "summary_only": [],
              "suppressed": []
            }"#,
    )?;
    let mut input = fixture_input(GateMode::Acknowledgeable);
    input.root = dir.clone();
    input.pr_guidance = Some(
        guidance
            .strip_prefix(&dir)
            .map_err(|err| err.to_string())?
            .to_path_buf(),
    );

    let report = build_gate_decision_report(&input)?;
    assert_eq!(report.decisions[0].decision, "advisory");
    assert!(
        report.decisions[0]
            .gate_reason
            .contains("policy-eligible class or placement scope"),
        "expected policy-eligible-class fallthrough reason, got {:?}",
        report.decisions[0].gate_reason,
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn given_read_json_value_pointed_at_directory_then_error_describes_non_not_found_failure()
-> Result<(), String> {
    let dir = temp_dir("gate-read-json-dir")?;
    let target_dir = dir.join("not-a-file.json");
    fs::create_dir_all(&target_dir).map_err(|err| format!("create dir failed: {err}"))?;
    let display = PathBuf::from("not-a-file.json");

    let result = read_json_value_with_display(&target_dir, &display);

    let error = match result {
        Ok(_) => return Err("reading a directory must fail".to_string()),
        Err(error) => error,
    };
    assert!(
        error.starts_with("read not-a-file.json failed:") && !error.contains("not found"),
        "expected non-not-found read error, got {error}",
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

// -- #1037 regression: structurally-invalid pr-guidance must be config_error --

#[test]
fn given_non_guidance_json_object_when_gate_evaluated_then_config_error_not_advisory()
-> Result<(), String> {
    let dir = temp_dir("gate-invalid-guidance-schema")?;
    let bad = write_temp_json(&dir, "bad.json", r#"{"not":"guidance"}"#)?;
    let input = GateEvaluateInput {
        root: dir.clone(),
        repo_exposure: None,
        pr_guidance: Some(
            bad.strip_prefix(&dir)
                .map_err(|err| err.to_string())?
                .to_path_buf(),
        ),
        gap_ledger: None,
        sarif_policy: None,
        labels_json: None,
        labels: Vec::new(),
        agent_verify: None,
        agent_receipt: None,
        recommendation_calibration: None,
        mutation_calibration: None,
        baseline: None,
        mode: GateMode::VisibleOnly,
        acknowledgement_labels: Vec::new(),
        exception_policy: None,
    };

    let report = build_gate_decision_report(&input)?;

    assert_eq!(
        report.status, "config_error",
        "a non-guidance JSON object must produce config_error, not advisory; got {:?}",
        report.status,
    );
    assert!(
        gate_decision_should_fail(&report),
        "gate_decision_should_fail must be true for config_error",
    );
    assert!(
        report.config_errors.iter().any(|error| {
            error.contains("bad.json")
                && error.contains("not a recognized review-comments guidance document")
        }),
        "config_errors must name the defect and the file path, got {:?}",
        report.config_errors,
    );
    assert!(
        report.decisions.is_empty(),
        "no decisions must be emitted for a config_error guidance doc",
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn given_valid_guidance_doc_with_zero_findings_when_gate_evaluated_then_advisory_not_config_error()
-> Result<(), String> {
    let dir = temp_dir("gate-empty-guidance")?;
    let guidance = write_temp_json(
        &dir,
        "comments.json",
        r#"{
              "schema_version": "0.1",
              "summary": {"unchanged_tests": true},
              "comments": [],
              "summary_only": [],
              "suppressed": []
            }"#,
    )?;
    let input = GateEvaluateInput {
        root: dir.clone(),
        repo_exposure: None,
        pr_guidance: Some(
            guidance
                .strip_prefix(&dir)
                .map_err(|err| err.to_string())?
                .to_path_buf(),
        ),
        gap_ledger: None,
        sarif_policy: None,
        labels_json: None,
        labels: Vec::new(),
        agent_verify: None,
        agent_receipt: None,
        recommendation_calibration: None,
        mutation_calibration: None,
        baseline: None,
        mode: GateMode::VisibleOnly,
        acknowledgement_labels: Vec::new(),
        exception_policy: None,
    };

    let report = build_gate_decision_report(&input)?;

    assert!(
        report.config_errors.is_empty(),
        "a valid empty guidance doc must produce no config_errors, got {:?}",
        report.config_errors,
    );
    assert!(
        report.status == "pass" || report.status == "advisory",
        "a valid empty guidance doc must yield pass or advisory, got {:?}",
        report.status,
    );
    assert!(
        !gate_decision_should_fail(&report),
        "gate_decision_should_fail must be false for a genuinely clean guidance doc",
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

// -- #1217 GAP 1 regression: well-formed error packet must fail-closed --

#[test]
fn given_well_formed_error_status_packet_when_gate_evaluated_then_config_error_not_pass()
-> Result<(), String> {
    // Repro: `ripr review-comments` crashes → xtask writes a structurally-valid
    // packet with `status:"error"` and `warnings:[{kind:"tool_error"}]`.
    // Before the fix, gate evaluate read the empty `comments` array, found
    // zero candidates, and returned exit 0 / status=pass — a fake-clean.
    let dir = temp_dir("gate-error-packet")?;
    let packet = write_temp_json(
        &dir,
        "error-packet.json",
        r#"{
              "schema_version": "0.1",
              "tool": "ripr",
              "status": "error",
              "root": ".",
              "base": "origin/main",
              "head": "HEAD",
              "mode": "fast",
              "rendering_limits": {"max_inline_comments": 0, "max_summary_items": 0},
              "summary": {"comments": 0, "summary_only": 0, "suppressed": 0, "unchanged_tests": true},
              "comments": [],
              "summary_only": [],
              "suppressed": [],
              "warnings": [{"kind": "tool_error", "message": "ripr exited with status 1", "path": null}],
              "limits_note": "Review guidance generation is advisory. The producer did not complete, so no comments are emitted."
            }"#,
    )?;
    let input = GateEvaluateInput {
        root: dir.clone(),
        repo_exposure: None,
        pr_guidance: Some(
            packet
                .strip_prefix(&dir)
                .map_err(|err| err.to_string())?
                .to_path_buf(),
        ),
        gap_ledger: None,
        sarif_policy: None,
        labels_json: None,
        labels: Vec::new(),
        agent_verify: None,
        agent_receipt: None,
        recommendation_calibration: None,
        mutation_calibration: None,
        baseline: None,
        mode: GateMode::Acknowledgeable,
        acknowledgement_labels: Vec::new(),
        exception_policy: None,
    };

    let report = build_gate_decision_report(&input)?;

    assert_eq!(
        report.status, "config_error",
        "a well-formed error-status packet must produce config_error, not pass; got {:?}",
        report.status,
    );
    assert!(
        gate_decision_should_fail(&report),
        "gate_decision_should_fail must be true for a crashed-producer error packet",
    );
    assert!(
        report.config_errors.iter().any(|e| {
            e.contains("error-packet.json") && e.contains("producer did not complete")
        }),
        "config_errors must name the file and the producer-incomplete reason, got {:?}",
        report.config_errors,
    );
    assert!(
        report.decisions.is_empty(),
        "no decisions must be emitted for a crashed-producer config_error",
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

// -- #1217 GAP 2 regression: cap-demoted weakly_exposed/warning stays blocking --

#[test]
fn given_cap_demoted_summary_only_gap_when_gate_evaluated_then_blocking_not_advisory()
-> Result<(), String> {
    // Repro: 3 exposed/info items fill the inline-comment cap; a genuine
    // weakly_exposed/warning gap lands in summary_only with
    // summary_reason=inline_comment_cap_reached.  Before the fix the gate
    // excluded ALL summary_only items via `source != "summary_only"`, so the
    // fourth gap was advisory (exit 0) even though the identical item in an
    // inline slot would have been blocking (exit 2).
    let dir = temp_dir("gate-cap-demoted")?;
    let guidance = write_temp_json(
        &dir,
        "cap-demoted.json",
        r#"{
              "schema_version": "0.1",
              "summary": {"unchanged_tests": true},
              "comments": [
                {
                  "id": "inline-1",
                  "seam_id": "inline-seam-1",
                  "grip_class": "weakly_exposed",
                  "severity": "info",
                  "missing_discriminator": "boundary_a",
                  "placement": {"path": "src/a.rs", "line": 10}
                },
                {
                  "id": "inline-2",
                  "seam_id": "inline-seam-2",
                  "grip_class": "weakly_exposed",
                  "severity": "info",
                  "missing_discriminator": "boundary_b",
                  "placement": {"path": "src/b.rs", "line": 20}
                },
                {
                  "id": "inline-3",
                  "seam_id": "inline-seam-3",
                  "grip_class": "weakly_exposed",
                  "severity": "info",
                  "missing_discriminator": "boundary_c",
                  "placement": {"path": "src/c.rs", "line": 30}
                }
              ],
              "summary_only": [
                {
                  "id": "cap-demoted-gap",
                  "seam_id": "cap-demoted-seam",
                  "canonical_gap_id": "gap:cap-demoted",
                  "gap_state": "actionable",
                  "grip_class": "weakly_exposed",
                  "severity": "warning",
                  "owner": "d::changed_caller",
                  "seam": {
                    "expression": "changed_caller emits real_blocker_value",
                    "file": "src/d.rs",
                    "line": 40
                  },
                  "missing_discriminator": "real_blocker_value",
                  "placement": {"path": "src/d.rs", "line": 40},
                  "suggested_test": {
                    "intent": "Add one focused discriminator test.",
                    "related_test": {
                      "name": "changed_caller_observes_value",
                      "file": "tests/d.rs",
                      "line": 12
                    }
                  },
                  "llm_guidance": {
                    "command": "ripr agent brief --root . --seam-id cap-demoted-seam --json",
                    "prompt": "Exercise d::changed_caller and assert real_blocker_value.",
                    "verify_command": "cargo test -p d changed_caller_observes_value"
                  },
                  "receipt_command": "ripr receipt write --gap gap:cap-demoted",
                  "summary_reason": "inline_comment_cap_reached"
                }
              ],
              "suppressed": []
            }"#,
    )?;
    let input = GateEvaluateInput {
        root: dir.clone(),
        repo_exposure: None,
        pr_guidance: Some(
            guidance
                .strip_prefix(&dir)
                .map_err(|err| err.to_string())?
                .to_path_buf(),
        ),
        gap_ledger: None,
        sarif_policy: None,
        labels_json: None,
        labels: Vec::new(),
        agent_verify: None,
        agent_receipt: None,
        recommendation_calibration: None,
        mutation_calibration: None,
        baseline: None,
        mode: GateMode::Acknowledgeable,
        acknowledgement_labels: Vec::new(),
        exception_policy: None,
    };

    let report = build_gate_decision_report(&input)?;

    assert!(
        report.summary.blocking >= 1,
        "cap-demoted weakly_exposed/warning gap must count as blocking (>=1), got summary {:?}",
        report.summary,
    );
    assert_eq!(
        report.status, "blocked",
        "gate status must be 'blocked' when a cap-demoted block-eligible gap is present; got {:?}",
        report.status,
    );
    assert!(
        gate_decision_should_fail(&report),
        "gate_decision_should_fail must be true when a cap-demoted gap is block-eligible",
    );
    // The cap-demoted item must be the blocking decision.
    let cap_decision = report
        .decisions
        .iter()
        .find(|d| d.source_id == "cap-demoted-gap")
        .ok_or_else(|| "cap-demoted-gap decision not found".to_string())?;
    assert_eq!(
        cap_decision.decision, "blocking",
        "the cap-demoted gap must resolve to 'blocking', got {:?}",
        cap_decision.decision,
    );
    let _ = fs::remove_dir_all(dir);
    Ok(())
}

// -- RIPR-SPEC-0111: new_unsuppressed receipt field --

/// Advisory candidates ARE counted in `new_unsuppressed.count` even when
/// `summary.blocking == 0`. In visible-only mode every policy-eligible
/// candidate is advisory (never blocking), so if the count equalled
/// `summary.blocking` it would always be 0 — a broken invariant.
#[test]
fn new_unsuppressed_counts_advisory_policy_eligible_candidates_not_just_blocking()
-> Result<(), String> {
    // visible-only: the standard fixture has 1 policy-eligible candidate
    // that will become "advisory" (not "blocking").
    let input = fixture_input(GateMode::VisibleOnly);
    let report = build_gate_decision_report(&input)?;
    // Baseline assertion: blocking is 0 (visible-only never blocks).
    assert_eq!(
        report.summary.blocking, 0,
        "visible-only must have blocking=0; got {:?}",
        report.summary,
    );
    // The honesty check: count > blocking because advisory items are included.
    assert!(
        report.new_unsuppressed.count > report.summary.blocking as u64,
        "new_unsuppressed.count ({}) must be > summary.blocking ({}) in visible-only because advisory items are included",
        report.new_unsuppressed.count,
        report.summary.blocking,
    );
    assert_eq!(
        report.new_unsuppressed.basis.as_deref(),
        Some("diff"),
        "visible-only mode must use basis=diff",
    );
    assert!(
        report.new_unsuppressed.reason.is_none(),
        "no reason expected for clean diff run",
    );
    Ok(())
}

#[test]
fn new_unsuppressed_excludes_advisory_candidates_with_incomplete_repair_routes()
-> Result<(), String> {
    let dir = temp_dir("gate-incomplete-route-count")?;
    let source = read_repo_fixture(Path::new(
        "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
    ))?;
    let mut guidance: Value = serde_json::from_str(&source)
        .map_err(|err| format!("parse current PR-guidance fixture: {err}"))?;
    let verify = guidance
        .pointer_mut("/comments/0/llm_guidance/verify_command")
        .ok_or_else(|| "current PR-guidance fixture lacks verify command".to_string())?;
    *verify = Value::Null;
    let guidance_text = serde_json::to_string_pretty(&guidance)
        .map_err(|err| format!("render incomplete PR guidance: {err}"))?;
    let guidance_path = write_temp_json(&dir, "comments.json", &guidance_text)?;
    let mut input = fixture_input(GateMode::Acknowledgeable);
    input.root = dir.clone();
    input.pr_guidance = Some(
        guidance_path
            .strip_prefix(&dir)
            .map_err(|err| err.to_string())?
            .to_path_buf(),
    );

    let report = build_gate_decision_report(&input)?;
    let decision = report
        .decisions
        .first()
        .ok_or_else(|| "expected one incomplete gate decision".to_string())?;

    if decision.decision != "advisory" {
        return Err(format!(
            "incomplete route must stay advisory, got {}",
            decision.decision
        ));
    }
    if decision
        .repair_route
        .limitation
        .as_ref()
        .map(|item| item.kind)
        != Some("incomplete_repair_route")
    {
        return Err("incomplete route must name its limitation".to_string());
    }
    if report.new_unsuppressed.count != 0 {
        return Err(format!(
            "incomplete route must not count as new policy-eligible debt, got {}",
            report.new_unsuppressed.count
        ));
    }

    let _ = fs::remove_dir_all(dir);
    Ok(())
}

/// `config_error` MUST produce `basis=null, count=0, reason=<disclosure>`.
/// This is the fail-closed sentinel: count=0 on analysis failure must NOT
/// look like a clean pass to a downstream thresholder.
#[test]
fn new_unsuppressed_config_error_produces_null_basis_and_zero_count_with_reason()
-> Result<(), String> {
    // Use calibrated-gate mode without a baseline: guaranteed config_error.
    let input = fixture_input(GateMode::CalibratedGate);
    let report = build_gate_decision_report(&input)?;
    assert_eq!(
        report.status, "config_error",
        "expected config_error status, got {:?}",
        report.status,
    );
    // Fail-closed check.
    assert!(
        report.new_unsuppressed.basis.is_none(),
        "config_error must produce basis=null (fail-closed), got {:?}",
        report.new_unsuppressed.basis,
    );
    assert_eq!(
        report.new_unsuppressed.count, 0,
        "config_error must produce count=0 (fail-closed), got {}",
        report.new_unsuppressed.count,
    );
    assert!(
        report.new_unsuppressed.reason.is_some(),
        "config_error must disclose reason (not a fake-zero clean), got None",
    );
    let reason = report.new_unsuppressed.reason.as_deref().unwrap_or("");
    assert!(
        reason.contains("analysis did not run"),
        "reason must start with 'analysis did not run', got {:?}",
        reason,
    );
    Ok(())
}

fn fixture_input(mode: GateMode) -> GateEvaluateInput {
    GateEvaluateInput {
        root: repo_root(),
        repo_exposure: None,
        pr_guidance: Some(PathBuf::from(
            "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
        )),
        gap_ledger: None,
        sarif_policy: None,
        labels_json: None,
        labels: Vec::new(),
        agent_verify: None,
        agent_receipt: None,
        recommendation_calibration: None,
        mutation_calibration: None,
        baseline: None,
        mode,
        acknowledgement_labels: Vec::new(),
        exception_policy: None,
    }
}

struct GateFixtureCase {
    name: &'static str,
    mode: GateMode,
    pr_guidance: &'static str,
    labels_json: Option<&'static str>,
    labels: &'static [&'static str],
    recommendation_calibration: Option<&'static str>,
    mutation_calibration: Option<&'static str>,
    baseline: Option<&'static str>,
}

impl GateFixtureCase {
    fn input(&self) -> GateEvaluateInput {
        GateEvaluateInput {
            root: repo_root(),
            repo_exposure: None,
            pr_guidance: Some(PathBuf::from(self.pr_guidance)),
            gap_ledger: None,
            sarif_policy: None,
            labels_json: self.labels_json.map(PathBuf::from),
            labels: self
                .labels
                .iter()
                .map(|label| (*label).to_string())
                .collect(),
            agent_verify: None,
            agent_receipt: None,
            recommendation_calibration: self.recommendation_calibration.map(PathBuf::from),
            mutation_calibration: self.mutation_calibration.map(PathBuf::from),
            baseline: self.baseline.map(PathBuf::from),
            mode: self.mode,
            acknowledgement_labels: Vec::new(),
            exception_policy: None,
        }
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn temp_dir(name: &str) -> Result<PathBuf, String> {
    let mut path = std::env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system time before unix epoch: {err}"))?
        .as_nanos();
    path.push(format!("ripr-{name}-{stamp}"));
    fs::create_dir_all(&path).map_err(|err| format!("create temp dir failed: {err}"))?;
    Ok(path)
}

fn write_temp_json(dir: &Path, name: &str, contents: &str) -> Result<PathBuf, String> {
    let path = dir.join(name);
    fs::write(&path, contents).map_err(|err| format!("write {name} failed: {err}"))?;
    Ok(path)
}

fn read_repo_fixture(path: &Path) -> Result<String, String> {
    let resolved = repo_root().join(path);
    fs::read_to_string(&resolved)
        .map_err(|err| format!("read {} failed: {err}", resolved.display()))
}

fn require_contains(actual: &str, expected: &str, label: &str) -> Result<(), String> {
    if actual.contains(expected) {
        Ok(())
    } else {
        Err(format!(
            "{label} missing from rendered gate Markdown: {expected}"
        ))
    }
}

fn require_not_contains(actual: &str, unexpected: &str, label: &str) -> Result<(), String> {
    if actual.contains(unexpected) {
        Err(format!(
            "{label} unexpectedly present in rendered gate Markdown: {unexpected}"
        ))
    } else {
        Ok(())
    }
}

fn assert_repo_fixture(path: &Path, rendered: &str, label: &str) -> Result<(), String> {
    let resolved = repo_root().join(path);
    if std::env::var("RIPR_UPDATE_FIXTURES").is_ok() {
        fs::write(&resolved, rendered)
            .map_err(|err| format!("write {} failed: {err}", resolved.display()))?;
        return Ok(());
    }
    let expected = read_repo_fixture(path)?;
    assert_eq!(rendered, expected, "{label} drifted");
    Ok(())
}

const PR_GUIDANCE_JSON: &str = r#"{
      "schema_version": "0.1",
      "summary": {"unchanged_tests": true},
      "comments": [
        {
          "id": "ripr-review-8f7fa8644fd12280",
          "seam_id": "8f7fa8644fd12280",
          "grip_class": "weakly_gripped",
          "severity": "warning",
          "missing_discriminator": "amount == discount_threshold",
          "placement": {"path": "src/pricing.rs", "line": 88},
          "suggested_test": {
            "candidate_values": ["amount == discount_threshold"],
            "near_test": "above_threshold_gets_discount"
          }
        }
      ],
      "summary_only": [],
      "suppressed": []
    }"#;

const SUMMARY_AND_SUPPRESSED_JSON: &str = r#"{
      "schema_version": "0.1",
      "summary": {"unchanged_tests": true},
      "comments": [],
      "summary_only": [
        {
          "id": "summary-1",
          "seam_id": "summary-seam",
          "grip_class": "weakly_gripped",
          "severity": "warning",
          "missing_discriminator": "amount == discount_threshold",
          "placement": {"path": "src/pricing.rs", "line": 88}
        }
      ],
      "suppressed": [
        {
          "id": "suppressed-1",
          "seam_id": "suppressed-seam",
          "grip_class": "weakly_gripped",
          "severity": "off",
          "reason": "severity_off",
          "missing_discriminator": "amount == discount_threshold",
          "placement": {"path": "src/pricing.rs", "line": 89}
        }
      ]
    }"#;

const INELIGIBLE_GUIDANCE_JSON: &str = r#"{
      "schema_version": "0.1",
      "summary": {"unchanged_tests": false},
      "comments": [
        {
          "id": "changed-test",
          "seam_id": "changed-test-seam",
          "grip_class": "weakly_gripped",
          "severity": "warning",
          "missing_discriminator": "amount == discount_threshold",
          "placement": {"path": "src/pricing.rs", "line": 88}
        },
        {
          "id": "missing-guidance",
          "seam_id": "missing-guidance-seam",
          "grip_class": "weakly_gripped",
          "severity": "warning",
          "placement": {"path": "src/pricing.rs", "line": 89}
        }
      ],
      "summary_only": [],
      "suppressed": []
    }"#;

const MISSING_GUIDANCE_JSON: &str = r#"{
      "schema_version": "0.1",
      "summary": {"unchanged_tests": true},
      "comments": [
        {
          "id": "missing-guidance",
          "seam_id": "missing-guidance-seam",
          "grip_class": "weakly_gripped",
          "severity": "warning",
          "placement": {"path": "src/pricing.rs", "line": 89}
        }
      ],
      "summary_only": [],
      "suppressed": []
    }"#;

const GAP_LEDGER_BLOCKING_JSON: &str = r#"{
      "gap_records": [
        {
          "gap_id": "gap:pricing",
          "canonical_gap_id": "pricing::discount::threshold",
          "seam_id": "seam-pricing-threshold",
          "kind": "MissingBoundaryAssertion",
          "language": "rust",
          "language_status": "stable",
          "scope": "pr_local",
          "evidence_class": "weakly_exposed",
          "gap_state": "actionable",
          "policy_state": "new",
          "repairability": "repairable",
          "repair_route": {
            "route_kind": "AddBoundaryAssertion",
            "target_file": "tests/pricing.rs",
            "target_line": 12,
            "related_test": "tests/pricing.rs::above_threshold_gets_discount",
            "assertion_shape": "assert_eq!(price(threshold), discounted)",
            "missing_discriminator": "amount == discount_threshold",
            "changed_behavior": "amount == discount_threshold",
            "inspection_command": "ripr agent brief --root . --seam-id seam-pricing-threshold --json"
          },
          "anchor": {
            "file": "src/pricing.rs",
            "line": 88,
            "owner": "price",
            "dedupe_fingerprint": "gap:pricing"
          },
          "evidence_ids": ["seam-pricing"],
          "projection_eligibility": {
            "gate_candidate": {
              "eligible": true,
              "reason": "new_repairable_pr_local_gap"
            }
          },
          "verification_commands": ["cargo xtask fixtures boundary_gap"],
          "receipt_command": "ripr receipt write --gap pricing::discount::threshold",
          "safe_gate_predicate": {
            "policy_target_enabled": true,
            "suppressed": false,
            "waived": false,
            "acknowledged_only": false,
            "baseline_known": false,
            "preview_language": false,
            "static_unknown_only": false
          }
        }
      ]
    }"#;

fn legacy_gap_ledger_json() -> Result<String, String> {
    let mut value: Value = serde_json::from_str(GAP_LEDGER_BLOCKING_JSON)
        .map_err(|err| format!("complete fixture should parse: {err}"))?;
    value["gap_records"][0]
        .as_object_mut()
        .ok_or_else(|| "complete fixture record should be an object".to_string())?
        .remove("seam_id");
    value["gap_records"][0]["repair_route"]
        .as_object_mut()
        .ok_or_else(|| "complete fixture route should be an object".to_string())?
        .remove("inspection_command");
    serde_json::to_string(&value).map_err(|err| format!("legacy fixture should serialize: {err}"))
}

const GAP_LEDGER_REPORT_ONLY_JSON: &str = r#"{
      "gap_records": [
        {
          "gap_id": "gap:unknown",
          "canonical_gap_id": "pricing::unknown",
          "kind": "Unknown",
          "language": "rust",
          "language_status": "stable",
          "scope": "pr_local",
          "evidence_class": "static_unknown",
          "gap_state": "unknown",
          "policy_state": "new",
          "repairability": "analyzer_limitation",
          "anchor": {
            "file": "src/pricing.rs",
            "line": 90,
            "dedupe_fingerprint": "gap:unknown"
          },
          "projection_eligibility": {
            "gate_candidate": {
              "eligible": false,
              "reason": "static_unknown_only"
            }
          },
          "safe_gate_predicate": {
            "policy_target_enabled": true,
            "static_unknown_only": true
          }
        }
      ]
    }"#;

#[test]
fn fallback_identity_for_normalizes_like_the_baseline_writers() -> Result<(), String> {
    // #2285 review: baseline writers normalize `\` -> `/` and strip `./`
    // (baseline_update.rs / baseline_delta.rs); the gate's fallback identity
    // must match that form or a legacy entry misses a compatibility match.
    let expected = Some("src/pricing.rs:88:weakly_exposed".to_string());
    for raw in [
        "src/pricing.rs",
        "./src/pricing.rs",
        r"src\pricing.rs",
        r".\src\pricing.rs",
    ] {
        let actual = fallback_identity_for(Some(raw), Some(88), Some("weakly_exposed"));
        if actual != expected {
            return Err(format!(
                "fallback identity for {raw:?} must normalize to {expected:?}, got {actual:?}"
            ));
        }
    }
    Ok(())
}
