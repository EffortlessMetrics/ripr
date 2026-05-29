use super::*;
use crate::analysis::ClassifiedSeam;
use crate::analysis::seams::SeamGripClass;
use crate::analysis::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamKind};
use crate::analysis::test_grip_evidence::{
    RelatedTestGrip, RelationConfidence, RelationReason, TestGripEvidence,
};
use crate::app::Mode;
use crate::domain::{
    Confidence, MissingDiscriminatorFact, OracleKind, OracleStrength, StageEvidence, StageState,
    ValueFact,
};
use crate::output::path::display_path;
use crate::output::pilot::ranking::top_actionable_seams;
use crate::output::python_repair_card::PythonRepairCard;
use std::path::{Path, PathBuf};

fn seam(file: &str, line: usize, expression: &str) -> RepoSeam {
    RepoSeam::new(
        file,
        "pricing::discounted_total",
        SeamKind::PredicateBoundary,
        line * 10,
        line,
        expression,
        RequiredDiscriminator::BoundaryValue {
            description: expression.to_string(),
        },
        ExpectedSink::ReturnValue,
    )
}

fn stage(state: StageState) -> StageEvidence {
    StageEvidence::new(state, Confidence::Medium, "stage summary")
}

fn missing() -> MissingDiscriminatorFact {
    MissingDiscriminatorFact {
        value: "input that hits the boundary: amount >= discount_threshold".to_string(),
        reason: "observed values do not include the equality-boundary case".to_string(),
        flow_sink: None,
    }
}

fn related_test() -> RelatedTestGrip {
    RelatedTestGrip {
        test_name: "below_threshold_has_no_discount".to_string(),
        file: PathBuf::from("tests/pricing.rs"),
        line: 12,
        oracle_kind: OracleKind::ExactValue,
        oracle_strength: OracleStrength::Strong,
        evidence_summary: "exact value assertion".to_string(),
        relation_reason: RelationReason::DirectOwnerCall,
        relation_confidence: RelationConfidence::High,
    }
}

fn pilot_artifacts() -> PilotArtifacts {
    PilotArtifacts {
        repo_exposure_json: PathBuf::from("target/ripr/pilot/repo-exposure.json"),
        repo_exposure_md: PathBuf::from("target/ripr/pilot/repo-exposure.md"),
        agent_seam_packets_json: PathBuf::from("target/ripr/pilot/agent-seam-packets.json"),
        pilot_summary_json: PathBuf::from("target/ripr/pilot/pilot-summary.json"),
        pilot_summary_md: PathBuf::from("target/ripr/pilot/pilot-summary.md"),
    }
}

fn pilot_context(artifacts: &PilotArtifacts) -> PilotSummaryContext<'_> {
    PilotSummaryContext {
        root: Path::new("."),
        mode: &Mode::Draft,
        config_path: Some(Path::new("ripr.toml")),
        max_seams: 5,
        timeout_ms: 30_000,
        artifacts,
        python_first_use: None,
    }
}

fn python_repair_card() -> PythonRepairCard {
    PythonRepairCard {
        card_version: "python_repair_card.v1".to_string(),
        source: "check_python_preview".to_string(),
        canonical_gap_id:
            "gap:python:src/pricing.py:calculate_discount:predicate_boundary:predicate:amount>=threshold"
                .to_string(),
        language: "python".to_string(),
        language_status: "preview".to_string(),
        authority_boundary: "preview_advisory_only".to_string(),
        changed_owner: "calculate_discount".to_string(),
        changed_behavior: "predicate_boundary changed at src/pricing.py:2: `amount >= threshold`"
            .to_string(),
        current_test_evidence:
            "tests/test_pricing.py:6 test_calculate_discount_above_threshold currently has oracle_strength=weak, oracle_kind=broad_assertion: assert result"
                .to_string(),
        missing_discriminator: "amount == threshold".to_string(),
        recommended_test_shape:
            "Add or strengthen a pytest boundary assertion for `amount == threshold`."
                .to_string(),
        suggested_assertion: "Assert the owner result or effect at the boundary `amount == threshold`."
            .to_string(),
        suggested_test_file: "tests/test_pricing.py".to_string(),
        suggested_test_name: "test_calculate_discount_threshold_boundary".to_string(),
        suggested_test_node_id: Some(
            "tests/test_pricing.py::test_calculate_discount_threshold_boundary".to_string(),
        ),
        verify_command:
            "pytest tests/test_pricing.py::test_calculate_discount_threshold_boundary".to_string(),
        verify_command_confidence: "high".to_string(),
        receipt_command: None,
        receipt_status: "unavailable_until_python_gap_ledger".to_string(),
        stop_conditions: vec![
            "Stop if imports, fixtures, or test setup cannot call the changed owner.".to_string(),
            "Stop if the expected value for the missing discriminator is ambiguous.".to_string(),
            "Stop if adding the test appears to require a production-code edit.".to_string(),
        ],
        limits: vec![
            "Syntax-first Python preview evidence only.".to_string(),
            "No source edits, generated tests, mutation execution, provider calls, or gate authority."
                .to_string(),
            "Verify success alone is not a gap-closure receipt.".to_string(),
        ],
    }
}

fn python_first_use() -> PilotPythonFirstUse {
    PilotPythonFirstUse {
        status: super::types::PilotPythonFirstUseStatus::Ready,
        findings_total: 1,
        repair_cards_total: 1,
        limitation_count: 0,
        analysis_error: None,
        top_repair_card: Some(python_repair_card()),
    }
}

fn pilot_context_with_python<'a>(
    artifacts: &'a PilotArtifacts,
    python_first_use: &'a PilotPythonFirstUse,
) -> PilotSummaryContext<'a> {
    PilotSummaryContext {
        root: Path::new("."),
        mode: &Mode::Draft,
        config_path: None,
        max_seams: 5,
        timeout_ms: 30_000,
        artifacts,
        python_first_use: Some(python_first_use),
    }
}

fn classified_with(
    class: SeamGripClass,
    file: &str,
    line: usize,
    missing_discriminators: Vec<MissingDiscriminatorFact>,
    related_tests: Vec<RelatedTestGrip>,
) -> ClassifiedSeam {
    let seam = seam(file, line, "amount >= discount_threshold");
    ClassifiedSeam {
        evidence: TestGripEvidence {
            seam_id: seam.id().clone(),
            related_tests,
            reach: stage(StageState::Yes),
            activate: stage(StageState::Yes),
            propagate: stage(StageState::Yes),
            observe: stage(StageState::Yes),
            discriminate: stage(StageState::Weak),
            observed_values: Vec::<ValueFact>::new(),
            missing_discriminators,
        },
        seam,
        class,
    }
}

#[test]
fn pilot_ranking_prefers_actionable_class_order_before_tie_breakers() {
    let ungripped = classified_with(
        SeamGripClass::Ungripped,
        "src/a.rs",
        10,
        vec![missing()],
        vec![related_test()],
    );
    let weak = classified_with(
        SeamGripClass::WeaklyGripped,
        "src/z.rs",
        99,
        Vec::new(),
        Vec::new(),
    );

    let entries = [ungripped, weak];
    let ranked = top_actionable_seams(&entries, 5);
    assert_eq!(ranked[0].class, SeamGripClass::WeaklyGripped);
    assert_eq!(ranked[1].class, SeamGripClass::Ungripped);
}

#[test]
fn pilot_ranking_uses_evidence_tie_breakers_then_stable_location() {
    let no_missing = classified_with(
        SeamGripClass::WeaklyGripped,
        "src/a.rs",
        10,
        Vec::new(),
        vec![related_test()],
    );
    let with_missing = classified_with(
        SeamGripClass::WeaklyGripped,
        "src/b.rs",
        10,
        vec![missing()],
        Vec::new(),
    );
    let stable_first = classified_with(
        SeamGripClass::WeaklyGripped,
        "src/c.rs",
        10,
        Vec::new(),
        Vec::new(),
    );
    let stable_second = classified_with(
        SeamGripClass::WeaklyGripped,
        "src/d.rs",
        10,
        Vec::new(),
        Vec::new(),
    );

    let entries = [stable_second, stable_first, no_missing, with_missing];
    let ranked = top_actionable_seams(&entries, 5);
    assert_eq!(display_path(ranked[0].seam.file()), "src/b.rs");
    assert_eq!(display_path(ranked[1].seam.file()), "src/a.rs");
    assert_eq!(display_path(ranked[2].seam.file()), "src/c.rs");
    assert_eq!(display_path(ranked[3].seam.file()), "src/d.rs");
}

#[test]
fn pilot_ranking_excludes_solved_governed_classes() {
    let strong = classified_with(
        SeamGripClass::StronglyGripped,
        "src/strong.rs",
        1,
        Vec::new(),
        Vec::new(),
    );
    let intentional = classified_with(
        SeamGripClass::Intentional,
        "src/intentional.rs",
        2,
        Vec::new(),
        Vec::new(),
    );
    let suppressed = classified_with(
        SeamGripClass::Suppressed,
        "src/suppressed.rs",
        3,
        Vec::new(),
        Vec::new(),
    );
    let opaque = classified_with(
        SeamGripClass::Opaque,
        "src/opaque.rs",
        4,
        Vec::new(),
        Vec::new(),
    );

    let entries = [strong, intentional, suppressed, opaque];
    let ranked = top_actionable_seams(&entries, 5);
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].class, SeamGripClass::Opaque);
}

#[test]
fn pilot_summary_json_contains_config_state_artifacts_and_next_commands() {
    let entry = classified_with(
        SeamGripClass::WeaklyGripped,
        "src/pricing.rs",
        88,
        vec![missing()],
        vec![related_test()],
    );
    let artifacts = PilotArtifacts {
        repo_exposure_json: PathBuf::from("target/ripr/pilot/repo-exposure.json"),
        repo_exposure_md: PathBuf::from("target/ripr/pilot/repo-exposure.md"),
        agent_seam_packets_json: PathBuf::from("target/ripr/pilot/agent-seam-packets.json"),
        pilot_summary_json: PathBuf::from("target/ripr/pilot/pilot-summary.json"),
        pilot_summary_md: PathBuf::from("target/ripr/pilot/pilot-summary.md"),
    };
    let context = PilotSummaryContext {
        root: Path::new("."),
        mode: &Mode::Draft,
        config_path: Some(Path::new("ripr.toml")),
        max_seams: 5,
        timeout_ms: 30_000,
        artifacts: &artifacts,
        python_first_use: None,
    };

    let json = render_pilot_summary_json(&[entry], context);
    assert!(json.contains(r#""schema_version": "0.2""#));
    assert!(json.contains(r#""status": "complete""#));
    assert!(json.contains(r#""state": "loaded""#));
    assert!(json.contains(r#""top_actionable_seams""#));
    assert!(json.contains(r#""missing_discriminator""#));
    assert!(json.contains("ripr outcome --before target/ripr/pilot/repo-exposure.json"));
}

#[test]
fn pilot_summary_md_spells_out_first_screen_recommendation() {
    let entry = classified_with(
        SeamGripClass::WeaklyGripped,
        "src/pricing.rs",
        88,
        vec![missing()],
        vec![related_test()],
    );
    let artifacts = pilot_artifacts();
    let md = render_pilot_summary_md(&[entry], pilot_context(&artifacts));

    for needle in [
        "## What Was Inspected",
        "## Top Recommendation",
        "- Inspected seam:",
        "- Why it matters: missing discriminator: input that hits the boundary: amount >= discount_threshold",
        "- Focused test: add `discounted_total_boundary_discriminator` in `tests/pricing.rs`",
        "- Candidate value: `input that hits the boundary: amount >= discount_threshold`",
        "Target seam:",
        "Add a targeted test:",
        "## Next Commands",
        "ripr outcome --before target/ripr/pilot/repo-exposure.json",
    ] {
        assert!(md.contains(needle), "missing markdown needle: {needle}");
    }
}

#[test]
fn pilot_terminal_prints_top_test_and_follow_up_commands() {
    let entry = classified_with(
        SeamGripClass::WeaklyGripped,
        "src/pricing.rs",
        88,
        vec![missing()],
        vec![related_test()],
    );
    let artifacts = pilot_artifacts();
    let terminal = render_pilot_terminal(&[entry], pilot_context(&artifacts));

    for needle in [
        "Inspected:",
        "root: .",
        "mode: draft",
        "config: loaded ripr.toml",
        "Top recommendation:",
        "inspected seam: src/pricing.rs:88 predicate_boundary in pricing::discounted_total (weakly_gripped)",
        "why it matters: missing discriminator: input that hits the boundary: amount >= discount_threshold",
        "focused test: add discounted_total_boundary_discriminator in tests/pricing.rs",
        "candidate value: input that hits the boundary: amount >= discount_threshold",
        "assertion: assert_eq!(discounted_total(/* boundary input where amount >= discount_threshold */), /* expected */)",
        "Detailed brief:",
        "target/ripr/pilot/pilot-summary.md",
        "Structured packet:",
        "target/ripr/pilot/agent-seam-packets.json",
        "Run after adding the focused test:",
        "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json",
        "ripr outcome --before target/ripr/pilot/repo-exposure.json",
    ] {
        assert!(
            terminal.contains(needle),
            "missing terminal needle: {needle}"
        );
    }
}

#[test]
fn timeout_summary_json_is_partial_and_points_to_retry() {
    let artifacts = PilotArtifacts {
        repo_exposure_json: PathBuf::from("target/ripr/pilot/repo-exposure.json"),
        repo_exposure_md: PathBuf::from("target/ripr/pilot/repo-exposure.md"),
        agent_seam_packets_json: PathBuf::from("target/ripr/pilot/agent-seam-packets.json"),
        pilot_summary_json: PathBuf::from("target/ripr/pilot/pilot-summary.json"),
        pilot_summary_md: PathBuf::from("target/ripr/pilot/pilot-summary.md"),
    };
    let context = PilotSummaryContext {
        root: Path::new("."),
        mode: &Mode::Draft,
        config_path: None,
        max_seams: 5,
        timeout_ms: 1,
        artifacts: &artifacts,
        python_first_use: None,
    };

    let json = render_pilot_timeout_summary_json(context);
    assert!(json.contains(r#""schema_version": "0.2""#));
    assert!(json.contains(r#""status": "partial""#));
    assert!(json.contains(r#""reason": "timeout""#));
    assert!(json.contains(r#""actionable_seams_total": null"#));
    assert!(json.contains("ripr pilot --root . --out target/ripr/pilot --mode draft"));
    assert!(json.contains("--timeout-ms 120000"));
}

#[test]
fn timeout_summary_json_records_loaded_config_path() {
    let artifacts = pilot_artifacts();
    let json = render_pilot_timeout_summary_json(pilot_context(&artifacts));
    assert!(
        json.contains(r#""config": {"state": "loaded", "path": "ripr.toml"}"#),
        "expected loaded-config path in timeout JSON, got:\n{json}"
    );
}

fn pilot_context_without_config<'a>(artifacts: &'a PilotArtifacts) -> PilotSummaryContext<'a> {
    PilotSummaryContext {
        root: Path::new("."),
        mode: &Mode::Draft,
        config_path: None,
        max_seams: 5,
        timeout_ms: 30_000,
        artifacts,
        python_first_use: None,
    }
}

#[test]
fn timeout_summary_md_explains_partial_status_and_retry_command() {
    let artifacts = pilot_artifacts();
    let md = render_pilot_timeout_summary_md(pilot_context(&artifacts));

    for needle in [
        "# RIPR Pilot Summary",
        "## Scope",
        "- Status: `partial`",
        "- Reason: analysis timed out after 30000 ms",
        "- Config: loaded `ripr.toml`",
        "## Outputs",
        "Analysis did not finish within the pilot budget",
        "- Pilot summary JSON: `target/ripr/pilot/pilot-summary.json`",
        "## Next Command",
        "ripr pilot --root . --out target/ripr/pilot --mode draft",
        "--timeout-ms 120000",
    ] {
        assert!(md.contains(needle), "missing timeout-md needle: {needle}");
    }
}

#[test]
fn timeout_summary_md_reports_missing_config_branch_when_no_config_loaded() {
    let artifacts = pilot_artifacts();
    let md = render_pilot_timeout_summary_md(pilot_context_without_config(&artifacts));
    assert!(
        md.contains("- Config: missing; using built-in defaults"),
        "expected missing-config line in timeout markdown, got:\n{md}"
    );
}

#[test]
fn timeout_terminal_lists_written_files_and_retry_command() {
    let artifacts = pilot_artifacts();
    let terminal = render_pilot_timeout_terminal(pilot_context(&artifacts));

    for needle in [
        "RIPR pilot partial.",
        "Reason:",
        "analysis timed out after 30000 ms",
        "Config:",
        "loaded: ripr.toml",
        "Written:",
        "target/ripr/pilot/pilot-summary.json",
        "target/ripr/pilot/pilot-summary.md",
        "Next:",
        "ripr pilot --root . --out target/ripr/pilot --mode draft",
    ] {
        assert!(
            terminal.contains(needle),
            "missing timeout-terminal needle: {needle}"
        );
    }
}

#[test]
fn timeout_terminal_reports_missing_config_branch_when_no_config_loaded() {
    let artifacts = pilot_artifacts();
    let terminal = render_pilot_timeout_terminal(pilot_context_without_config(&artifacts));
    assert!(
        terminal.contains("missing: using built-in defaults"),
        "expected missing-config line in timeout terminal, got:\n{terminal}"
    );
}

#[test]
fn pilot_summary_json_reports_missing_config_when_none_loaded() {
    let entry = classified_with(
        SeamGripClass::WeaklyGripped,
        "src/pricing.rs",
        88,
        vec![missing()],
        vec![related_test()],
    );
    let artifacts = pilot_artifacts();
    let json = render_pilot_summary_json(&[entry], pilot_context_without_config(&artifacts));
    assert!(
        json.contains(r#""state": "missing", "path": null"#),
        "expected missing-config JSON branch, got:\n{json}"
    );
}

#[test]
fn pilot_summary_md_reports_missing_config_when_none_loaded() {
    let entry = classified_with(
        SeamGripClass::WeaklyGripped,
        "src/pricing.rs",
        88,
        vec![missing()],
        vec![related_test()],
    );
    let artifacts = pilot_artifacts();
    let md = render_pilot_summary_md(&[entry], pilot_context_without_config(&artifacts));
    assert!(
        md.contains("- Config: missing; using built-in defaults"),
        "expected missing-config line in pilot markdown, got:\n{md}"
    );
}

#[test]
fn pilot_terminal_reports_missing_config_when_none_loaded() {
    let entry = classified_with(
        SeamGripClass::WeaklyGripped,
        "src/pricing.rs",
        88,
        vec![missing()],
        vec![related_test()],
    );
    let artifacts = pilot_artifacts();
    let terminal = render_pilot_terminal(&[entry], pilot_context_without_config(&artifacts));
    assert!(
        terminal.contains("config: missing, using built-in defaults"),
        "expected missing-config line in pilot terminal, got:\n{terminal}"
    );
}

#[test]
fn pilot_summary_renderers_omit_recommendation_when_no_actionable_seams() {
    let entries = [
        classified_with(
            SeamGripClass::StronglyGripped,
            "src/a.rs",
            10,
            Vec::new(),
            Vec::new(),
        ),
        classified_with(
            SeamGripClass::Intentional,
            "src/b.rs",
            20,
            Vec::new(),
            Vec::new(),
        ),
        classified_with(
            SeamGripClass::Suppressed,
            "src/c.rs",
            30,
            Vec::new(),
            Vec::new(),
        ),
    ];
    let artifacts = pilot_artifacts();

    let json = render_pilot_summary_json(&entries, pilot_context(&artifacts));
    assert!(
        json.contains(r#""actionable_seams_total": 0"#),
        "expected zero actionable seams in JSON, got:\n{json}"
    );
    assert!(
        json.contains(r#""top_actionable_seams": []"#),
        "expected empty top_actionable_seams array, got:\n{json}"
    );

    let md = render_pilot_summary_md(&entries, pilot_context(&artifacts));
    assert!(
        md.contains("No actionable seam was ranked by the default pilot policy."),
        "expected no-actionable-seam markdown line, got:\n{md}"
    );

    let terminal = render_pilot_terminal(&entries, pilot_context(&artifacts));
    assert!(
        terminal.contains("none ranked by the default pilot policy"),
        "expected no-recommendation terminal line, got:\n{terminal}"
    );
}

#[test]
fn pilot_summary_json_projects_python_first_use_repair_card() {
    let artifacts = pilot_artifacts();
    let python = python_first_use();
    let json = render_pilot_summary_json(&[], pilot_context_with_python(&artifacts, &python));

    for needle in [
        r#""python_first_use": {"#,
        r#""status": "ready""#,
        r#""language": "python""#,
        r#""language_status": "preview""#,
        r#""authority_boundary": "preview_advisory_only""#,
        r#""findings_total": 1"#,
        r#""repair_cards_total": 1"#,
        r#""top_repair_card": {"#,
        "gap:python:src/pricing.py:calculate_discount:predicate_boundary:predicate:amount>=threshold",
        r#""changed_owner": "calculate_discount""#,
        r#""missing_discriminator": "amount == threshold""#,
        r#""suggested_test_file": "tests/test_pricing.py""#,
        r#""suggested_test_name": "test_calculate_discount_threshold_boundary""#,
        r#""verify_command": "pytest tests/test_pricing.py::test_calculate_discount_threshold_boundary""#,
        r#""receipt_status": "unavailable_until_python_gap_ledger""#,
        r#""deferred_features": ["outcome_receipts", "runtime_mutation_execution", "gate_authority", "generated_tests"]"#,
    ] {
        assert!(
            json.contains(needle),
            "missing Python JSON needle: {needle}"
        );
    }
}

#[test]
fn pilot_markdown_and_terminal_use_python_repair_card_when_no_seam_ranked() {
    let artifacts = pilot_artifacts();
    let python = python_first_use();
    let context = pilot_context_with_python(&artifacts, &python);
    let md = render_pilot_summary_md(&[], context);
    let terminal = render_pilot_terminal(&[], context);

    for needle in [
        "## Top Recommendation",
        "Top Python repairable gap",
        "Changed owner: `calculate_discount`",
        "Missing discriminator: `amount == threshold`",
        "Suggested test: `test_calculate_discount_threshold_boundary` in `tests/test_pricing.py`",
        "Verify: `pytest tests/test_pricing.py::test_calculate_discount_threshold_boundary`",
        "Receipt status: `unavailable_until_python_gap_ledger`",
        "## Python Preview First Use",
    ] {
        assert!(
            md.contains(needle),
            "missing Python markdown needle: {needle}"
        );
    }

    for needle in [
        "Top recommendation:",
        "language: python (preview)",
        "changed owner: calculate_discount",
        "missing discriminator: amount == threshold",
        "recommended test: add test_calculate_discount_threshold_boundary in tests/test_pricing.py",
        "verify: pytest tests/test_pricing.py::test_calculate_discount_threshold_boundary",
        "receipt status: unavailable_until_python_gap_ledger",
        "Python preview:",
        "status: ready",
    ] {
        assert!(
            terminal.contains(needle),
            "missing Python terminal needle: {needle}"
        );
    }
    assert!(
        !terminal.contains("none ranked by the default pilot policy"),
        "Python repair card should replace the no-recommendation top line"
    );
}

#[test]
fn why_line_uses_static_discriminator_summary_when_no_missing_discriminator() {
    let seam = seam("src/pricing.rs", 88, "amount >= discount_threshold");
    let entry = ClassifiedSeam {
        evidence: TestGripEvidence {
            seam_id: seam.id().clone(),
            related_tests: vec![related_test()],
            reach: stage(StageState::Yes),
            activate: stage(StageState::Yes),
            propagate: stage(StageState::Yes),
            observe: stage(StageState::Yes),
            discriminate: StageEvidence::new(
                StageState::Weak,
                Confidence::Medium,
                "weak boundary oracle",
            ),
            observed_values: Vec::<ValueFact>::new(),
            missing_discriminators: Vec::new(),
        },
        seam,
        class: SeamGripClass::WeaklyGripped,
    };
    assert_eq!(
        super::render::why_line(&entry),
        "static discriminator summary: weak boundary oracle"
    );
}

#[test]
fn why_line_falls_back_to_class_label_when_no_summary_or_missing_discriminator() {
    let seam = seam("src/pricing.rs", 88, "amount >= discount_threshold");
    let entry = ClassifiedSeam {
        evidence: TestGripEvidence {
            seam_id: seam.id().clone(),
            related_tests: Vec::new(),
            reach: stage(StageState::Yes),
            activate: stage(StageState::Yes),
            propagate: stage(StageState::Yes),
            observe: stage(StageState::Yes),
            discriminate: StageEvidence::new(StageState::Weak, Confidence::Medium, "   "),
            observed_values: Vec::<ValueFact>::new(),
            missing_discriminators: Vec::new(),
        },
        seam,
        class: SeamGripClass::Ungripped,
    };
    assert_eq!(
        super::render::why_line(&entry),
        "ungripped static seam evidence"
    );
}
