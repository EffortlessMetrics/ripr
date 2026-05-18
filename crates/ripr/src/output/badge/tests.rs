use super::{
    BADGE_REASON_KEYS, BadgePolicy, BadgeScope, BadgeStatus, TestEfficiencyBadgeSummary,
    badge_status_color, parse_test_efficiency_badge_summary, render_native_json,
    render_shields_json, repo_gap_ledger_badge_summary_from_json, ripr_badge_summary,
    ripr_plus_badge_summary, ripr_seam_badge_summary,
};
use crate::analysis::ClassifiedSeam;
use crate::analysis::seams::{
    ExpectedSink, RepoSeam, RequiredDiscriminator, SeamGripClass, SeamKind,
};
use crate::analysis::test_grip_evidence::TestGripEvidence;
use crate::app::{CheckInput, CheckOutput, Mode};
use crate::config::RiprConfig;
use crate::domain::{
    ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, OracleKind, OracleStrength,
    Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation,
    StageEvidence, StageState, Summary,
};
use std::path::PathBuf;

fn finding(class: ExposureClass, related: Vec<RelatedTest>) -> Finding {
    Finding {
        id: "probe:src_lib_rs:1:predicate".to_string(),
        probe: Probe {
            id: ProbeId("probe:src_lib_rs:1:predicate".to_string()),
            family: ProbeFamily::Predicate,
            location: SourceLocation::new("src/lib.rs", 1, 1),
            owner: None,
            delta: DeltaKind::Control,
            before: None,
            after: None,
            expression: "expr".to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        },
        class,
        ripr: RiprEvidence {
            reach: StageEvidence::new(StageState::Yes, Confidence::Medium, "reached"),
            infect: StageEvidence::new(StageState::Weak, Confidence::Low, "infected"),
            propagate: StageEvidence::new(StageState::No, Confidence::Medium, "not propagated"),
            reveal: RevealEvidence {
                observe: StageEvidence::new(StageState::Weak, Confidence::Low, "observed"),
                discriminate: StageEvidence::new(
                    StageState::No,
                    Confidence::Medium,
                    "no discriminator",
                ),
            },
        },
        confidence: 0.5,
        evidence: Vec::new(),
        missing: Vec::new(),
        flow_sinks: Vec::new(),
        activation: ActivationEvidence::default(),
        stop_reasons: Vec::new(),
        related_tests: related,
        recommended_next_step: None,
        language: None,
        language_status: None,
        owner_kind: None,
        static_limit_kind: None,
    }
}

fn related_test(name: &str, file: &str, line: usize) -> RelatedTest {
    RelatedTest {
        name: name.to_string(),
        file: PathBuf::from(file),
        line,
        oracle: None,
        oracle_kind: OracleKind::Unknown,
        oracle_strength: OracleStrength::Weak,
    }
}

fn check_output(findings: Vec<Finding>) -> CheckOutput {
    let defaults = CheckInput::default();
    CheckOutput {
        schema_version: "0.1".to_string(),
        tool: "ripr".to_string(),
        mode: Mode::Draft,
        root: defaults.root,
        base: defaults.base,
        summary: Summary::default(),
        findings,
    }
}

fn stage(state: StageState) -> StageEvidence {
    StageEvidence::new(state, Confidence::Medium, "stage")
}

fn classified_seam(class: SeamGripClass) -> ClassifiedSeam {
    let seam = RepoSeam::new(
        "src/lib.rs",
        "crate::discounted_total",
        SeamKind::PredicateBoundary,
        10,
        2,
        "amount >= threshold",
        RequiredDiscriminator::BoundaryValue {
            description: "amount == threshold".to_string(),
        },
        ExpectedSink::ReturnValue,
    );
    ClassifiedSeam {
        evidence: TestGripEvidence {
            seam_id: seam.id().clone(),
            related_tests: Vec::new(),
            reach: stage(StageState::Yes),
            activate: stage(StageState::Yes),
            propagate: stage(StageState::Yes),
            observe: stage(StageState::Yes),
            discriminate: stage(StageState::Weak),
            observed_values: Vec::new(),
            missing_discriminators: Vec::new(),
        },
        seam,
        class,
    }
}

#[test]
fn badge_summary_counts_weakly_exposed_reachable_unrevealed_and_no_static_path() {
    let output = check_output(vec![
        finding(ExposureClass::WeaklyExposed, vec![]),
        finding(ExposureClass::ReachableUnrevealed, vec![]),
        finding(ExposureClass::NoStaticPath, vec![]),
    ]);

    let summary = ripr_badge_summary(&output, BadgePolicy::default());

    assert_eq!(summary.counts.unsuppressed_exposure_gaps, 3);
    assert_eq!(summary.message, "3");
}

#[test]
fn badge_summary_does_not_count_exposed_findings() {
    let output = check_output(vec![
        finding(ExposureClass::Exposed, vec![]),
        finding(ExposureClass::Exposed, vec![]),
    ]);

    let summary = ripr_badge_summary(&output, BadgePolicy::default());

    assert_eq!(summary.counts.unsuppressed_exposure_gaps, 0);
    assert_eq!(summary.counts.analyzed_findings, 2);
    assert_eq!(summary.message, "0");
    assert_eq!(summary.status, BadgeStatus::Pass);
    assert_eq!(summary.color, "brightgreen");
}

#[test]
fn badge_summary_reports_unknowns_separately_from_headline() {
    let output = check_output(vec![
        finding(ExposureClass::InfectionUnknown, vec![]),
        finding(ExposureClass::PropagationUnknown, vec![]),
        finding(ExposureClass::StaticUnknown, vec![]),
        finding(ExposureClass::WeaklyExposed, vec![]),
    ]);

    let summary = ripr_badge_summary(&output, BadgePolicy::default());

    assert_eq!(summary.counts.unsuppressed_exposure_gaps, 1);
    assert_eq!(summary.counts.unknowns, 3);
    // Headline excludes unknowns by default.
    assert_eq!(summary.message, "1");
}

#[test]
fn seam_badge_summary_counts_visible_headline_eligible_seams() {
    let classified = vec![
        classified_seam(SeamGripClass::WeaklyGripped),
        classified_seam(SeamGripClass::Ungripped),
        classified_seam(SeamGripClass::StronglyGripped),
        classified_seam(SeamGripClass::Opaque),
    ];

    let summary =
        ripr_seam_badge_summary(&classified, &RiprConfig::default(), BadgePolicy::default());

    assert_eq!(summary.scope, BadgeScope::Repo);
    assert_eq!(summary.basis.as_str(), "seam_native");
    assert_eq!(summary.counts.unsuppressed_exposure_gaps, 2);
    assert_eq!(summary.counts.unknowns, 1);
    assert_eq!(summary.counts.analyzed_findings, 0);
    assert_eq!(summary.counts.analyzed_seams, 4);
    assert_eq!(summary.message, "2");
}

#[test]
fn seam_badge_summary_respects_configured_off_severity() -> Result<(), String> {
    let config = crate::config::tests_only_parse(
        r#"
[severity.seams]
weakly_gripped = "off"
"#,
    )?;
    let classified = vec![
        classified_seam(SeamGripClass::WeaklyGripped),
        classified_seam(SeamGripClass::Ungripped),
    ];

    let summary = ripr_seam_badge_summary(&classified, &config, BadgePolicy::default());

    assert_eq!(summary.counts.unsuppressed_exposure_gaps, 1);
    assert_eq!(summary.message, "1");
    Ok(())
}

#[test]
fn gap_ledger_badge_summary_counts_projection_targets() -> Result<(), String> {
    let ledger = r#"{
          "gap_records": [
            {
              "gap_id": "gap:repo:pricing:reintroduced-boundary",
              "kind": "MissingBoundaryAssertion",
              "language": "rust",
              "language_status": "stable",
              "scope": "repo_scoped",
              "gap_state": "reintroduced",
              "policy_state": "reintroduced",
              "repairability": "repairable",
              "projection_eligibility": {
                "ripr_zero_count": {"eligible": true, "reason": "repo_policy_targeted_unresolved_gap"},
                "ripr_plus_count": {"eligible": true, "reason": "broader_repo_advisory_gap"}
              }
            },
            {
              "gap_id": "gap:repo:waived",
              "kind": "MissingValueAssertion",
              "language": "rust",
              "language_status": "stable",
              "scope": "repo_scoped",
              "gap_state": "waived",
              "policy_state": "waived",
              "repairability": "no_action",
              "projection_eligibility": {
                "ripr_zero_count": {"eligible": false, "reason": "waived"},
                "ripr_plus_count": {"eligible": false, "reason": "waived"}
              }
            }
          ]
        }"#;

    let summary = repo_gap_ledger_badge_summary_from_json(
        ledger,
        super::BadgeKind::Ripr,
        BadgePolicy::default(),
    )?;

    assert_eq!(summary.scope, BadgeScope::Repo);
    assert_eq!(summary.basis.as_str(), "gap_decision_ledger");
    assert_eq!(summary.counts.unsuppressed_exposure_gaps, 1);
    assert_eq!(summary.counts.analyzed_gap_records, 2);
    assert_eq!(summary.message, "1");
    Ok(())
}

#[test]
fn badge_summary_message_never_contains_a_denominator() {
    let output = check_output(vec![
        finding(ExposureClass::WeaklyExposed, vec![]),
        finding(ExposureClass::Exposed, vec![]),
        finding(ExposureClass::Exposed, vec![]),
    ]);

    let summary = ripr_badge_summary(&output, BadgePolicy::default());

    assert!(!summary.message.contains('/'), "no denominator");
    assert!(!summary.message.to_ascii_lowercase().contains("coverage"));
    assert!(!summary.message.to_ascii_lowercase().contains("uncovered"));
    assert_eq!(summary.message, "1");
}

#[test]
fn badge_status_color_zero_is_pass_brightgreen() {
    assert_eq!(
        badge_status_color(0, false),
        (BadgeStatus::Pass, "brightgreen")
    );
}

#[test]
fn badge_status_color_one_to_three_is_warn_yellow() {
    for count in 1..=3 {
        assert_eq!(
            badge_status_color(count, false),
            (BadgeStatus::Warn, "yellow"),
            "count {count}",
        );
    }
}

#[test]
fn badge_status_color_four_or_more_is_warn_orange() {
    for count in [4, 5, 12, 100] {
        assert_eq!(
            badge_status_color(count, false),
            (BadgeStatus::Warn, "orange"),
            "count {count}",
        );
    }
}

#[test]
fn badge_status_color_fail_on_nonzero_promotes_warn_to_fail_red() {
    assert_eq!(
        badge_status_color(1, true),
        (BadgeStatus::Fail, "red"),
        "fail_on_nonzero with count 1"
    );
    assert_eq!(
        badge_status_color(7, true),
        (BadgeStatus::Fail, "red"),
        "fail_on_nonzero with count 7"
    );
    // Zero remains pass even with fail_on_nonzero.
    assert_eq!(
        badge_status_color(0, true),
        (BadgeStatus::Pass, "brightgreen"),
        "zero stays pass even with fail_on_nonzero"
    );
}

#[test]
fn badge_native_json_uses_snake_case_schema_version_and_all_required_fields() {
    let output = check_output(vec![finding(ExposureClass::WeaklyExposed, vec![])]);
    let summary = ripr_badge_summary(&output, BadgePolicy::default());
    let json = render_native_json(&summary);

    assert!(json.contains("\"schema_version\": \"0.4\""));
    assert!(!json.contains("\"schemaVersion\""));
    assert!(json.contains("\"kind\": \"ripr\""));
    assert!(json.contains("\"scope\": \"diff\""));
    assert!(json.contains("\"basis\": \"finding_exposure\""));
    assert!(json.contains("\"label\": \"ripr\""));
    assert!(json.contains("\"message\": \"1\""));
    assert!(json.contains("\"status\": \"warn\""));
    assert!(json.contains("\"color\": \"yellow\""));
    for key in [
        "unsuppressed_exposure_gaps",
        "unsuppressed_test_efficiency_findings",
        "intentional_test_efficiency_findings",
        "suppressed_exposure_gaps",
        "suppressed_test_efficiency_findings",
        "unknowns",
        "unknowns_test_efficiency",
        "analyzed_findings",
        "analyzed_seams",
        "analyzed_gap_records",
        "analyzed_tests",
    ] {
        assert!(
            json.contains(&format!("\"{key}\":")),
            "native JSON missing count key `{key}`"
        );
    }
    for key in [
        "include_unknowns",
        "fail_on_nonzero",
        "test_intent_path",
        "suppressions_path",
    ] {
        assert!(
            json.contains(&format!("\"{key}\":")),
            "native JSON missing policy key `{key}`"
        );
    }
}

#[test]
fn badge_native_json_emits_repo_scope_when_summary_carries_repo_scope() {
    let output = check_output(vec![finding(ExposureClass::WeaklyExposed, vec![])]);
    let mut summary = ripr_badge_summary(&output, BadgePolicy::default());
    summary.scope = BadgeScope::Repo;
    let json = render_native_json(&summary);

    assert!(json.contains("\"scope\": \"repo\""));
    assert!(!json.contains("\"scope\": \"diff\""));
}

#[test]
fn badge_shields_projection_omits_scope_field() {
    // Shields stays exactly four fields after native schema bumps:
    // schemaVersion, label, message, color. `scope` and `basis` are native-only.
    let output = check_output(vec![finding(ExposureClass::WeaklyExposed, vec![])]);
    let mut summary = ripr_badge_summary(&output, BadgePolicy::default());
    summary.scope = BadgeScope::Repo;
    let shields = render_shields_json(&summary);

    assert!(!shields.contains("\"scope\""));
    assert!(!shields.contains("\"basis\""));
    let top_level_keys = shields
        .lines()
        .filter_map(|line| {
            let stripped = line.trim().strip_prefix('"')?;
            let end = stripped.find('"')?;
            Some(stripped[..end].to_string())
        })
        .collect::<Vec<_>>();
    assert_eq!(top_level_keys.len(), 4);
}

#[test]
fn badge_native_json_contains_all_nine_reason_defaults() {
    let output = check_output(vec![]);
    let summary = ripr_badge_summary(&output, BadgePolicy::default());
    let json = render_native_json(&summary);

    for reason in BADGE_REASON_KEYS {
        assert!(
            json.contains(&format!("\"{reason}\": 0")),
            "native JSON missing reason key `{reason}` with default 0"
        );
    }
    // Specifically sanity-check the new reason from #187/#188.
    assert!(json.contains("\"duplicate_activation_and_oracle_shape\": 0"));
}

#[test]
fn badge_shields_projection_uses_camel_case_schema_version_key_and_exactly_four_fields() {
    let output = check_output(vec![finding(ExposureClass::WeaklyExposed, vec![])]);
    let summary = ripr_badge_summary(&output, BadgePolicy::default());
    let shields = render_shields_json(&summary);

    assert!(shields.contains("\"schemaVersion\": 1"));
    assert!(!shields.contains("\"schema_version\""));
    assert!(shields.contains("\"label\": \"ripr\""));
    assert!(shields.contains("\"message\": \"1\""));
    assert!(shields.contains("\"color\": \"yellow\""));

    // Exactly four top-level keys.
    let top_level_quoted_keys = shields
        .lines()
        .filter(|line| line.starts_with("  \""))
        .count();
    assert_eq!(
        top_level_quoted_keys, 4,
        "Shields projection must have exactly four top-level fields"
    );
    // No native-JSON-only fields leak in.
    for forbidden in [
        "counts",
        "reason_counts",
        "policy",
        "kind",
        "status",
        "scope",
        "basis",
    ] {
        assert!(
            !shields.contains(&format!("\"{forbidden}\":")),
            "Shields projection must not include `{forbidden}`"
        );
    }
}

#[test]
fn badge_summary_counts_unique_related_tests_by_file_name_line() {
    let test_a = related_test("test_one", "tests/a.rs", 10);
    let test_b = related_test("test_two", "tests/a.rs", 20);
    // Same identity — should dedupe across findings.
    let test_a_again = related_test("test_one", "tests/a.rs", 10);

    let output = check_output(vec![
        finding(ExposureClass::WeaklyExposed, vec![test_a, test_b]),
        finding(ExposureClass::WeaklyExposed, vec![test_a_again]),
    ]);

    let summary = ripr_badge_summary(&output, BadgePolicy::default());

    assert_eq!(
        summary.counts.analyzed_tests, 2,
        "analyzed_tests counts unique (file, name, line) identities"
    );
}

#[test]
fn badge_include_unknowns_policy_adds_unknowns_to_headline() {
    let output = check_output(vec![
        finding(ExposureClass::WeaklyExposed, vec![]),
        finding(ExposureClass::InfectionUnknown, vec![]),
        finding(ExposureClass::StaticUnknown, vec![]),
    ]);

    let policy = BadgePolicy {
        include_unknowns: true,
        ..BadgePolicy::default()
    };
    let summary = ripr_badge_summary(&output, policy);

    // 1 exposure gap + 2 unknowns = 3.
    assert_eq!(summary.message, "3");
    // Counts still report them separately.
    assert_eq!(summary.counts.unsuppressed_exposure_gaps, 1);
    assert_eq!(summary.counts.unknowns, 2);
}

#[test]
fn badge_test_efficiency_counts_are_zero_until_later_prs() {
    let output = check_output(vec![finding(ExposureClass::WeaklyExposed, vec![])]);
    let summary = ripr_badge_summary(&output, BadgePolicy::default());

    // This PR does not yet read the test-efficiency report. Future PRs
    // (`badge/ripr-plus-count-v1`, `test-intent/v1`, `suppressions/v1`)
    // will populate these.
    assert_eq!(summary.counts.unsuppressed_test_efficiency_findings, 0);
    assert_eq!(summary.counts.intentional_test_efficiency_findings, 0);
    assert_eq!(summary.counts.suppressed_test_efficiency_findings, 0);
    assert_eq!(summary.counts.suppressed_exposure_gaps, 0);
    assert_eq!(summary.counts.unknowns_test_efficiency, 0);
}

// -------- ripr+ test-efficiency parser --------

fn te_json(tests_json: &str, reason_counts: &str) -> String {
    format!(
        r#"{{
  "schema_version": "0.1",
  "tests": [{tests_json}],
  "metrics": {{
    "tests_scanned": 42,
    "reason_counts": {{{reason_counts}}}
  }}
}}"#
    )
}

fn entry_json(class: &str, with_intent: bool) -> String {
    let intent = if with_intent {
        r#","declared_intent":{"intent":"smoke","owner":"x","reason":"y","source":".ripr/test_intent.toml"}"#
    } else {
        ""
    };
    format!(r#"{{"class":"{class}"{intent}}}"#)
}

#[test]
fn badge_plus_parses_test_efficiency_metrics() -> Result<(), String> {
    let json = te_json(&entry_json("strong_discriminator", false), "");
    let summary = parse_test_efficiency_badge_summary(&json)?;

    assert_eq!(summary.analyzed_tests, 42);
    assert_eq!(summary.unsuppressed_test_efficiency_findings, 0);
    assert_eq!(summary.intentional_test_efficiency_findings, 0);
    assert_eq!(summary.unknowns_test_efficiency, 0);
    Ok(())
}

#[test]
fn badge_plus_counts_actionable_classes() -> Result<(), String> {
    for class in [
        "likely_vacuous",
        "possibly_circular",
        "smoke_only",
        "duplicative",
    ] {
        let json = te_json(&entry_json(class, false), "");
        let summary = parse_test_efficiency_badge_summary(&json)?;
        assert_eq!(
            summary.unsuppressed_test_efficiency_findings, 1,
            "class `{class}` must count as actionable"
        );
        assert_eq!(summary.intentional_test_efficiency_findings, 0);
    }
    Ok(())
}

#[test]
fn badge_plus_does_not_count_strong_discriminator_or_useful_but_broad() -> Result<(), String> {
    for class in ["strong_discriminator", "useful_but_broad"] {
        let json = te_json(&entry_json(class, false), "");
        let summary = parse_test_efficiency_badge_summary(&json)?;
        assert_eq!(
            summary.unsuppressed_test_efficiency_findings, 0,
            "class `{class}` must not count"
        );
        assert_eq!(summary.intentional_test_efficiency_findings, 0);
        assert_eq!(summary.unknowns_test_efficiency, 0);
    }
    Ok(())
}

#[test]
fn badge_plus_reports_opaque_as_unknowns_test_efficiency() -> Result<(), String> {
    let json = te_json(&entry_json("opaque", false), "");
    let summary = parse_test_efficiency_badge_summary(&json)?;

    assert_eq!(summary.unsuppressed_test_efficiency_findings, 0);
    assert_eq!(summary.unknowns_test_efficiency, 1);
    Ok(())
}

#[test]
fn badge_plus_declared_intent_excludes_actionable_finding() -> Result<(), String> {
    let json = te_json(&entry_json("smoke_only", true), "");
    let summary = parse_test_efficiency_badge_summary(&json)?;

    assert_eq!(
        summary.unsuppressed_test_efficiency_findings, 0,
        "declared intent must exclude the finding from unsuppressed"
    );
    assert_eq!(
        summary.intentional_test_efficiency_findings, 1,
        "declared intent must increment intentional count"
    );
    Ok(())
}

#[test]
fn badge_plus_reason_counts_default_missing_keys_to_zero() -> Result<(), String> {
    let json = te_json(&entry_json("strong_discriminator", false), "");
    let summary = parse_test_efficiency_badge_summary(&json)?;

    for key in BADGE_REASON_KEYS {
        assert_eq!(
            summary.reason_counts.get(*key).copied(),
            Some(0),
            "reason `{key}` should default to 0"
        );
    }
    Ok(())
}

#[test]
fn badge_plus_reason_counts_propagate_known_keys() -> Result<(), String> {
    let reasons =
        r#""smoke_oracle_only":4,"duplicate_activation_and_oracle_shape":2,"unrecognized":99"#;
    let json = te_json(&entry_json("strong_discriminator", false), reasons);
    let summary = parse_test_efficiency_badge_summary(&json)?;

    assert_eq!(
        summary.reason_counts.get("smoke_oracle_only").copied(),
        Some(4)
    );
    assert_eq!(
        summary
            .reason_counts
            .get("duplicate_activation_and_oracle_shape")
            .copied(),
        Some(2)
    );
    // Unknown reason names are silently dropped — they're not part of the
    // badge contract, only the nine allow-listed keys are.
    assert!(!summary.reason_counts.contains_key("unrecognized"));
    Ok(())
}

#[test]
fn badge_plus_rejects_unknown_class_string() {
    let json = te_json(r#"{"class":"vibe_only"}"#, "");
    let result = parse_test_efficiency_badge_summary(&json);

    assert!(result.is_err(), "unknown class must fail parse");
    let err = result.err().unwrap_or_default();
    assert!(err.contains("vibe_only"));
}

#[test]
fn badge_plus_rejects_unsupported_schema_version() {
    let json =
        r#"{"schema_version":"2.0","tests":[],"metrics":{"tests_scanned":0,"reason_counts":{}}}"#;
    let result = parse_test_efficiency_badge_summary(json);

    assert!(result.is_err());
    let err = result.err().unwrap_or_default();
    assert!(err.contains("schema_version"));
}

#[test]
fn badge_plus_rejects_missing_metrics_tests_scanned() {
    let json = r#"{"schema_version":"0.1","tests":[],"metrics":{}}"#;
    let result = parse_test_efficiency_badge_summary(json);

    assert!(result.is_err());
    let err = result.err().unwrap_or_default();
    assert!(err.contains("metrics.tests_scanned"));
}

// -------- ripr+ summary builder + renderers --------

#[test]
fn ripr_plus_native_json_has_kind_ripr_plus_and_label_ripr_plus() {
    let summary = ripr_plus_badge_summary(
        &check_output(Vec::new()),
        TestEfficiencyBadgeSummary {
            unsuppressed_test_efficiency_findings: 0,
            intentional_test_efficiency_findings: 0,
            unknowns_test_efficiency: 0,
            analyzed_tests: 12,
            reason_counts: {
                let mut m = std::collections::BTreeMap::new();
                for k in BADGE_REASON_KEYS {
                    m.insert(*k, 0);
                }
                m
            },
            actionable_entries: Vec::new(),
            entries: Vec::new(),
        },
        BadgePolicy::default(),
    );
    let json = render_native_json(&summary);

    assert!(json.contains("\"kind\": \"ripr_plus\""));
    assert!(json.contains("\"label\": \"ripr+\""));
    assert!(json.contains("\"analyzed_tests\": 12"));
    assert!(json.contains("\"message\": \"0\""));
}

#[test]
fn ripr_plus_message_sums_exposure_and_unsuppressed_test_efficiency() {
    // 1 weakly_exposed + 1 reachable_unrevealed = 2 exposure gaps.
    // 3 unsuppressed test-efficiency findings.
    // 2 declared intent (NOT in headline). Total: 2 + 3 = 5.
    let summary = ripr_plus_badge_summary(
        &check_output(vec![
            finding(ExposureClass::WeaklyExposed, vec![]),
            finding(ExposureClass::ReachableUnrevealed, vec![]),
            finding(ExposureClass::Exposed, vec![]),
        ]),
        TestEfficiencyBadgeSummary {
            unsuppressed_test_efficiency_findings: 3,
            intentional_test_efficiency_findings: 2,
            unknowns_test_efficiency: 1,
            analyzed_tests: 0,
            reason_counts: std::collections::BTreeMap::new(),
            actionable_entries: Vec::new(),
            entries: Vec::new(),
        },
        BadgePolicy::default(),
    );

    assert_eq!(summary.message, "5");
    assert_eq!(summary.counts.unsuppressed_exposure_gaps, 2);
    assert_eq!(summary.counts.unsuppressed_test_efficiency_findings, 3);
    assert_eq!(summary.counts.intentional_test_efficiency_findings, 2);
    assert_eq!(summary.counts.unknowns_test_efficiency, 1);
}

#[test]
fn ripr_plus_shields_projection_has_exactly_four_fields_with_ripr_plus_label() {
    let summary = ripr_plus_badge_summary(
        &check_output(vec![finding(ExposureClass::WeaklyExposed, vec![])]),
        TestEfficiencyBadgeSummary::default(),
        BadgePolicy::default(),
    );
    let shields = render_shields_json(&summary);

    assert!(shields.contains("\"schemaVersion\": 1"));
    assert!(shields.contains("\"label\": \"ripr+\""));
    assert!(shields.contains("\"message\": \"1\""));
    assert!(shields.contains("\"color\":"));

    let top_level_quoted_keys = shields
        .lines()
        .filter(|line| line.starts_with("  \""))
        .count();
    assert_eq!(top_level_quoted_keys, 4);
    for forbidden in [
        "counts",
        "reason_counts",
        "policy",
        "kind",
        "status",
        "scope",
        "basis",
    ] {
        assert!(
            !shields.contains(&format!("\"{forbidden}\":")),
            "ripr+ Shields projection must not contain `{forbidden}`"
        );
    }
}

#[test]
fn ripr_plus_message_has_no_denominator_or_coverage_framing() {
    let summary = ripr_plus_badge_summary(
        &check_output(vec![
            finding(ExposureClass::WeaklyExposed, vec![]),
            finding(ExposureClass::Exposed, vec![]),
        ]),
        TestEfficiencyBadgeSummary {
            unsuppressed_test_efficiency_findings: 4,
            ..TestEfficiencyBadgeSummary::default()
        },
        BadgePolicy::default(),
    );
    let json = render_native_json(&summary);
    let shields = render_shields_json(&summary);

    for body in [&json, &shields] {
        let lower = body.to_ascii_lowercase();
        assert!(!lower.contains("coverage"));
        assert!(!lower.contains("uncovered"));
    }
    assert_eq!(summary.message, "5");
    assert!(!summary.message.contains('/'));
}

#[test]
fn ripr_plus_include_unknowns_policy_adds_both_unknown_axes_to_headline() {
    let policy = BadgePolicy {
        include_unknowns: true,
        ..BadgePolicy::default()
    };
    let summary = ripr_plus_badge_summary(
        &check_output(vec![
            finding(ExposureClass::WeaklyExposed, vec![]), // 1 gap
            finding(ExposureClass::InfectionUnknown, vec![]), // 1 unknown
        ]),
        TestEfficiencyBadgeSummary {
            unsuppressed_test_efficiency_findings: 2,
            unknowns_test_efficiency: 3,
            ..TestEfficiencyBadgeSummary::default()
        },
        policy,
    );

    // 1 + 2 + 1 + 3 = 7
    assert_eq!(summary.message, "7");
}

// -------- suppressions wiring --------

use super::{
    TestEfficiencyAggregationScope, TestEfficiencyBadgeEntry, ripr_badge_summary_with_suppressions,
    ripr_plus_badge_summary_with_suppressions,
};
use crate::output::suppressions::{SuppressionEntry, SuppressionKind};

fn finding_at_id(id: &str, class: ExposureClass) -> Finding {
    let mut f = finding(class, vec![]);
    f.id = id.to_string();
    f
}

fn exposure_suppression(finding_id: &str, expires: Option<&str>) -> SuppressionEntry {
    SuppressionEntry {
        kind: SuppressionKind::ExposureGap,
        finding_id: Some(finding_id.to_string()),
        test: None,
        path: None,
        reason: "x".to_string(),
        owner: "y".to_string(),
        expires: expires.map(str::to_string),
        scope: None,
        created_at: None,
        last_seen: None,
        review_by: None,
        expected_visibility: None,
        static_class: None,
        language: None,
        language_status: None,
        block_line: 10,
    }
}

fn te_suppression(test: &str, path: Option<&str>, expires: Option<&str>) -> SuppressionEntry {
    SuppressionEntry {
        kind: SuppressionKind::TestEfficiency,
        finding_id: None,
        test: Some(test.to_string()),
        path: path.map(str::to_string),
        reason: "x".to_string(),
        owner: "y".to_string(),
        expires: expires.map(str::to_string),
        scope: None,
        created_at: None,
        last_seen: None,
        review_by: None,
        expected_visibility: None,
        static_class: None,
        language: None,
        language_status: None,
        block_line: 20,
    }
}

#[test]
fn ripr_badge_with_suppressions_moves_matched_findings_into_suppressed_bucket() {
    let output = check_output(vec![
        finding_at_id("probe:a", ExposureClass::WeaklyExposed),
        finding_at_id("probe:b", ExposureClass::ReachableUnrevealed),
        finding_at_id("probe:c", ExposureClass::NoStaticPath),
    ]);
    let suppressions = vec![exposure_suppression("probe:b", None)];

    let summary = ripr_badge_summary_with_suppressions(
        &output,
        &suppressions,
        "2026-05-03",
        BadgePolicy::default(),
    );

    assert_eq!(summary.counts.unsuppressed_exposure_gaps, 2);
    assert_eq!(summary.counts.suppressed_exposure_gaps, 1);
    assert_eq!(summary.message, "2");
    assert!(summary.warnings.is_empty());
}

#[test]
fn ripr_badge_with_expired_suppression_keeps_finding_in_headline_and_warns() {
    let output = check_output(vec![finding_at_id("probe:a", ExposureClass::WeaklyExposed)]);
    let suppressions = vec![exposure_suppression("probe:a", Some("2025-01-01"))];

    let summary = ripr_badge_summary_with_suppressions(
        &output,
        &suppressions,
        "2026-05-03",
        BadgePolicy::default(),
    );

    // Expired suppression must NOT apply.
    assert_eq!(summary.counts.unsuppressed_exposure_gaps, 1);
    assert_eq!(summary.counts.suppressed_exposure_gaps, 0);
    // Warning surfaces so debt is visible.
    assert_eq!(summary.warnings.len(), 1);
    assert!(summary.warnings[0].contains("expired"));
    assert!(summary.warnings[0].contains("probe:a"));
}

#[test]
fn ripr_plus_badge_with_test_efficiency_suppressions_moves_into_suppressed_bucket() {
    let te = TestEfficiencyBadgeSummary {
        unsuppressed_test_efficiency_findings: 2,
        actionable_entries: vec![
            TestEfficiencyBadgeEntry {
                test: "alpha".to_string(),
                path: "tests/a.rs".to_string(),
                has_intent: false,
                class: "smoke_only".to_string(),
                reached_owners: Vec::new(),
            },
            TestEfficiencyBadgeEntry {
                test: "beta".to_string(),
                path: "tests/b.rs".to_string(),
                has_intent: false,
                class: "smoke_only".to_string(),
                reached_owners: Vec::new(),
            },
        ],
        ..TestEfficiencyBadgeSummary::default()
    };
    let output = check_output(vec![]);
    let suppressions = vec![te_suppression("alpha", Some("tests/a.rs"), None)];

    let summary = ripr_plus_badge_summary_with_suppressions(
        &output,
        te,
        &suppressions,
        "2026-05-03",
        BadgePolicy::default(),
        TestEfficiencyAggregationScope::Repo,
    );

    assert_eq!(summary.counts.unsuppressed_test_efficiency_findings, 1);
    assert_eq!(summary.counts.suppressed_test_efficiency_findings, 1);
    assert_eq!(summary.message, "1");
    assert!(summary.warnings.is_empty());
}

#[test]
fn native_json_emits_warnings_array_always_even_when_empty() {
    let summary = ripr_badge_summary(&check_output(vec![]), BadgePolicy::default());
    let json = render_native_json(&summary);

    // Empty case still emits the field for stable shape.
    assert!(json.contains("\"warnings\": []"));
}

#[test]
fn native_json_emits_warnings_when_suppressions_have_warnings() {
    let output = check_output(vec![finding_at_id("probe:a", ExposureClass::WeaklyExposed)]);
    let suppressions = vec![exposure_suppression("probe:a", Some("2025-01-01"))];
    let summary = ripr_badge_summary_with_suppressions(
        &output,
        &suppressions,
        "2026-05-03",
        BadgePolicy::default(),
    );
    let json = render_native_json(&summary);

    assert!(json.contains("\"warnings\": ["));
    assert!(json.contains("expired"));
    assert!(json.contains("probe:a"));
}

#[test]
fn shields_projection_remains_four_fields_even_with_warnings_present() {
    let output = check_output(vec![finding_at_id("probe:a", ExposureClass::WeaklyExposed)]);
    let suppressions = vec![exposure_suppression("probe:does_not_match", None)];
    let summary = ripr_badge_summary_with_suppressions(
        &output,
        &suppressions,
        "2026-05-03",
        BadgePolicy::default(),
    );
    let shields = render_shields_json(&summary);

    // Warnings must NOT bleed into the Shields projection.
    assert!(!shields.contains("warnings"));
    assert!(!shields.contains("probe:does_not_match"));
    let top_level = shields.lines().filter(|l| l.starts_with("  \"")).count();
    assert_eq!(top_level, 4);
}

#[test]
fn declared_intent_remains_distinct_from_suppression_in_counts() {
    // 1 unsuppressed actionable, 2 intentional, 0 unknowns_te.
    let te = TestEfficiencyBadgeSummary {
        unsuppressed_test_efficiency_findings: 1,
        intentional_test_efficiency_findings: 2,
        actionable_entries: vec![TestEfficiencyBadgeEntry {
            test: "alpha".to_string(),
            path: "tests/a.rs".to_string(),
            has_intent: false,
            class: "smoke_only".to_string(),
            reached_owners: Vec::new(),
        }],
        ..TestEfficiencyBadgeSummary::default()
    };
    let output = check_output(vec![]);
    let suppressions = vec![te_suppression("alpha", Some("tests/a.rs"), None)];

    let summary = ripr_plus_badge_summary_with_suppressions(
        &output,
        te,
        &suppressions,
        "2026-05-03",
        BadgePolicy::default(),
        TestEfficiencyAggregationScope::Repo,
    );

    // The actionable becomes suppressed, leaving 0 unsuppressed.
    assert_eq!(summary.counts.unsuppressed_test_efficiency_findings, 0);
    assert_eq!(summary.counts.suppressed_test_efficiency_findings, 1);
    // Intentional count is unaffected — intent and suppression are distinct.
    assert_eq!(summary.counts.intentional_test_efficiency_findings, 2);
}

// -------- diff-scope `ripr+` aggregation --------
//
// `cargo xtask test-efficiency-report` is repo-wide as a fact source.
// Diff-scoped `ripr+` must filter that ledger to entries related to
// the changed code; repo-scoped `ripr+` aggregates the full ledger.
// The `DiffRelatedTests` filter uses `Finding.related_tests` names
// (rule 1) and `Finding.probe.owner` ∩ entry `reached_owners`
// (rule 2). These tests pin both rules and the bucket-by-class
// behavior under diff scope.

use super::{DiffRelatedTests, TestEfficiencyAggregationScope as Scope};
use crate::domain::SymbolId;
use std::collections::BTreeMap;

fn finding_with_owner(owner: &str, related: Vec<RelatedTest>) -> Finding {
    let mut f = finding(ExposureClass::WeaklyExposed, related);
    f.probe.owner = Some(SymbolId(owner.to_string()));
    f
}

fn te_entry(
    name: &str,
    path: &str,
    class: &str,
    has_intent: bool,
    reached_owners: &[&str],
) -> TestEfficiencyBadgeEntry {
    TestEfficiencyBadgeEntry {
        test: name.to_string(),
        path: path.to_string(),
        has_intent,
        class: class.to_string(),
        reached_owners: reached_owners.iter().map(|s| s.to_string()).collect(),
    }
}

fn te_summary(
    unsuppressed: usize,
    intentional: usize,
    unknowns: usize,
    actionable: Vec<TestEfficiencyBadgeEntry>,
    all: Vec<TestEfficiencyBadgeEntry>,
) -> TestEfficiencyBadgeSummary {
    TestEfficiencyBadgeSummary {
        unsuppressed_test_efficiency_findings: unsuppressed,
        intentional_test_efficiency_findings: intentional,
        unknowns_test_efficiency: unknowns,
        analyzed_tests: all.len(),
        reason_counts: BTreeMap::new(),
        actionable_entries: actionable,
        entries: all,
    }
}

#[test]
fn diff_related_tests_extracts_owners_and_test_keys_from_findings() {
    let output = check_output(vec![
        finding_with_owner(
            "pricing::quote",
            vec![related_test(
                "premium_customer_gets_discount",
                "tests/pricing.rs",
                12,
            )],
        ),
        finding_with_owner("billing::charge", vec![]),
    ]);
    let filter = DiffRelatedTests::from_check_output(&output);

    assert!(filter.changed_owners.contains("pricing::quote"));
    assert!(filter.changed_owners.contains("billing::charge"));
    // Both bare and qualified test keys are present so the filter
    // can match either shape from the test-efficiency report.
    assert!(
        filter
            .related_test_keys
            .contains("premium_customer_gets_discount")
    );
    assert!(
        filter
            .related_test_keys
            .contains("tests/pricing.rs::premium_customer_gets_discount")
    );
}

#[test]
fn diff_ripr_plus_counts_related_smoke_only_via_related_tests_match() {
    // Entry's `(name, path)` matches a Finding.related_tests entry.
    let related = related_test("premium_customer_gets_discount", "tests/pricing.rs", 12);
    let output = check_output(vec![finding_with_owner("pricing::quote", vec![related])]);
    let entry = te_entry(
        "premium_customer_gets_discount",
        "tests/pricing.rs",
        "smoke_only",
        false,
        &[], // no reached_owners — rule 1 still wins
    );
    let te = te_summary(1, 0, 0, vec![entry.clone()], vec![entry]);
    let filter = DiffRelatedTests::from_check_output(&output);

    let summary = ripr_plus_badge_summary_with_suppressions(
        &output,
        te,
        &[],
        "2026-05-04",
        BadgePolicy::default(),
        Scope::Diff(&filter),
    );

    assert_eq!(summary.counts.unsuppressed_test_efficiency_findings, 1);
}

#[test]
fn diff_ripr_plus_counts_related_smoke_only_via_owner_intersection() {
    // No related_tests match; rule 2 (reached_owners ∩ changed_owners) wins.
    let output = check_output(vec![finding_with_owner("pricing::quote", vec![])]);
    let entry = te_entry(
        "unrelated_name",
        "tests/elsewhere.rs",
        "smoke_only",
        false,
        &["pricing::quote", "billing::charge"],
    );
    let te = te_summary(1, 0, 0, vec![entry.clone()], vec![entry]);
    let filter = DiffRelatedTests::from_check_output(&output);

    let summary = ripr_plus_badge_summary_with_suppressions(
        &output,
        te,
        &[],
        "2026-05-04",
        BadgePolicy::default(),
        Scope::Diff(&filter),
    );

    assert_eq!(summary.counts.unsuppressed_test_efficiency_findings, 1);
}

#[test]
fn diff_ripr_plus_counts_related_duplicative_entry() {
    let output = check_output(vec![finding_with_owner("pricing::quote", vec![])]);
    let entry = te_entry(
        "premium_customer_gets_discount",
        "tests/pricing.rs",
        "duplicative",
        false,
        &["pricing::quote"],
    );
    let te = te_summary(1, 0, 0, vec![entry.clone()], vec![entry]);
    let filter = DiffRelatedTests::from_check_output(&output);

    let summary = ripr_plus_badge_summary_with_suppressions(
        &output,
        te,
        &[],
        "2026-05-04",
        BadgePolicy::default(),
        Scope::Diff(&filter),
    );

    assert_eq!(summary.counts.unsuppressed_test_efficiency_findings, 1);
}

#[test]
fn diff_ripr_plus_ignores_unrelated_likely_vacuous_test() {
    // Diff touches `pricing::quote`; the test-efficiency entry
    // reaches `unrelated::module` and has no related_tests match.
    let output = check_output(vec![finding_with_owner("pricing::quote", vec![])]);
    let unrelated = te_entry(
        "totally_unrelated_test",
        "tests/elsewhere.rs",
        "likely_vacuous",
        false,
        &["unrelated::module"],
    );
    let te = te_summary(1, 0, 0, vec![unrelated.clone()], vec![unrelated]);
    let filter = DiffRelatedTests::from_check_output(&output);

    let summary = ripr_plus_badge_summary_with_suppressions(
        &output,
        te,
        &[],
        "2026-05-04",
        BadgePolicy::default(),
        Scope::Diff(&filter),
    );

    assert_eq!(
        summary.counts.unsuppressed_test_efficiency_findings, 0,
        "unrelated repo-wide test-efficiency debt must NOT move the diff headline"
    );
}

#[test]
fn diff_ripr_plus_ignores_unrelated_duplicative_group() {
    let output = check_output(vec![finding_with_owner("pricing::quote", vec![])]);
    let entries = vec![
        te_entry(
            "dup_a",
            "tests/elsewhere.rs",
            "duplicative",
            false,
            &["unrelated::a"],
        ),
        te_entry(
            "dup_b",
            "tests/elsewhere.rs",
            "duplicative",
            false,
            &["unrelated::b"],
        ),
        te_entry(
            "dup_c",
            "tests/elsewhere.rs",
            "duplicative",
            false,
            &["unrelated::c"],
        ),
    ];
    let te = te_summary(3, 0, 0, entries.clone(), entries);
    let filter = DiffRelatedTests::from_check_output(&output);

    let summary = ripr_plus_badge_summary_with_suppressions(
        &output,
        te,
        &[],
        "2026-05-04",
        BadgePolicy::default(),
        Scope::Diff(&filter),
    );

    assert_eq!(summary.counts.unsuppressed_test_efficiency_findings, 0);
}

#[test]
fn repo_ripr_plus_still_counts_whole_repo_actionable_test_efficiency() {
    // Same fixture as the diff-ignores test above; under repo
    // scope the whole-repo unsuppressed total still counts.
    let output = check_output(vec![finding_with_owner("pricing::quote", vec![])]);
    let unrelated = te_entry(
        "totally_unrelated_test",
        "tests/elsewhere.rs",
        "likely_vacuous",
        false,
        &["unrelated::module"],
    );
    let te = te_summary(1, 0, 0, vec![unrelated.clone()], vec![unrelated]);

    let summary = ripr_plus_badge_summary_with_suppressions(
        &output,
        te,
        &[],
        "2026-05-04",
        BadgePolicy::default(),
        Scope::Repo,
    );

    assert_eq!(summary.counts.unsuppressed_test_efficiency_findings, 1);
}

#[test]
fn diff_ripr_plus_excludes_related_declared_intent_finding() {
    // A related entry with declared intent counts toward
    // `intentional_*`, never toward the unsuppressed headline.
    let output = check_output(vec![finding_with_owner("pricing::quote", vec![])]);
    let intent_entry = te_entry(
        "premium_customer_gets_discount",
        "tests/pricing.rs",
        "smoke_only",
        true,
        &["pricing::quote"],
    );
    // unsuppressed count from the parser is 0 (intent excluded);
    // intentional total is 1.
    let te = te_summary(0, 1, 0, Vec::new(), vec![intent_entry]);
    let filter = DiffRelatedTests::from_check_output(&output);

    let summary = ripr_plus_badge_summary_with_suppressions(
        &output,
        te,
        &[],
        "2026-05-04",
        BadgePolicy::default(),
        Scope::Diff(&filter),
    );

    assert_eq!(summary.counts.unsuppressed_test_efficiency_findings, 0);
    assert_eq!(summary.counts.intentional_test_efficiency_findings, 1);
}

#[test]
fn diff_ripr_plus_excludes_related_suppressed_finding() {
    let output = check_output(vec![finding_with_owner("pricing::quote", vec![])]);
    let entry = te_entry(
        "premium_customer_gets_discount",
        "tests/pricing.rs",
        "smoke_only",
        false,
        &["pricing::quote"],
    );
    let te = te_summary(1, 0, 0, vec![entry.clone()], vec![entry]);
    let suppressions = vec![te_suppression(
        "premium_customer_gets_discount",
        Some("tests/pricing.rs"),
        None,
    )];
    let filter = DiffRelatedTests::from_check_output(&output);

    let summary = ripr_plus_badge_summary_with_suppressions(
        &output,
        te,
        &suppressions,
        "2026-05-04",
        BadgePolicy::default(),
        Scope::Diff(&filter),
    );

    assert_eq!(summary.counts.unsuppressed_test_efficiency_findings, 0);
    assert_eq!(summary.counts.suppressed_test_efficiency_findings, 1);
}

#[test]
fn diff_ripr_plus_keeps_related_opaque_visible_but_not_headline() {
    // Use an exposure-side `Exposed` finding (which does not count
    // as an exposure gap) so the headline isolates the
    // test-efficiency contribution. The owner is still set so the
    // diff filter has something to intersect against.
    let mut f = finding(ExposureClass::Exposed, vec![]);
    f.probe.owner = Some(SymbolId("pricing::quote".to_string()));
    let output = check_output(vec![f]);
    let opaque_entry = te_entry(
        "opaque_oracle_test",
        "tests/pricing.rs",
        "opaque",
        false,
        &["pricing::quote"],
    );
    // Parser totals: 0 unsuppressed (opaque doesn't go there), 1 unknowns_te.
    let te = te_summary(0, 0, 1, Vec::new(), vec![opaque_entry]);
    let filter = DiffRelatedTests::from_check_output(&output);

    let summary = ripr_plus_badge_summary_with_suppressions(
        &output,
        te,
        &[],
        "2026-05-04",
        BadgePolicy::default(),
        Scope::Diff(&filter),
    );

    assert_eq!(summary.counts.unsuppressed_test_efficiency_findings, 0);
    assert_eq!(summary.counts.unknowns_test_efficiency, 1);
    // Headline excludes both unknowns and unsuppressed_te here:
    // 0 exposure gaps + 0 te + 0 (unknowns excluded by default policy).
    assert_eq!(summary.message, "0");
}

#[test]
fn diff_ripr_plus_count_unaffected_by_unrelated_repo_wide_te_debt() {
    // Mix one related actionable with many unrelated repo-wide
    // entries. The diff headline should reflect only the related
    // entry; unrelated debt stays in the repo-wide counts but does
    // NOT move the diff signal.
    let output = check_output(vec![finding_with_owner("pricing::quote", vec![])]);
    let related = te_entry(
        "premium_customer_gets_discount",
        "tests/pricing.rs",
        "smoke_only",
        false,
        &["pricing::quote"],
    );
    let unrelated = (0..5)
        .map(|i| {
            te_entry(
                &format!("unrelated_{i}"),
                "tests/elsewhere.rs",
                "duplicative",
                false,
                &["other::module"],
            )
        })
        .collect::<Vec<_>>();

    let mut all_entries = vec![related.clone()];
    all_entries.extend(unrelated.iter().cloned());
    let mut all_actionable = vec![related];
    all_actionable.extend(unrelated);

    let te = te_summary(6, 0, 0, all_actionable, all_entries);
    let filter = DiffRelatedTests::from_check_output(&output);

    let summary = ripr_plus_badge_summary_with_suppressions(
        &output,
        te,
        &[],
        "2026-05-04",
        BadgePolicy::default(),
        Scope::Diff(&filter),
    );

    assert_eq!(
        summary.counts.unsuppressed_test_efficiency_findings, 1,
        "diff headline should only count the related entry, not the 5 unrelated ones"
    );
}
