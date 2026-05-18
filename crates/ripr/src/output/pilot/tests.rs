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
        value: "discount_threshold equality boundary".to_string(),
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
        "- Why it matters: missing discriminator: discount_threshold equality boundary",
        "- Focused test: add `discounted_total_boundary_discriminator` in `tests/pricing.rs`",
        "- Candidate value: `discount_threshold equality boundary`",
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
        "why it matters: missing discriminator: discount_threshold equality boundary",
        "focused test: add discounted_total_boundary_discriminator in tests/pricing.rs",
        "candidate value: discount_threshold equality boundary",
        "assertion: assert_eq!(discounted_total(/* discount_threshold equality boundary */), /* expected */)",
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
    };

    let json = render_pilot_timeout_summary_json(context);
    assert!(json.contains(r#""schema_version": "0.2""#));
    assert!(json.contains(r#""status": "partial""#));
    assert!(json.contains(r#""reason": "timeout""#));
    assert!(json.contains(r#""actionable_seams_total": null"#));
    assert!(json.contains("ripr pilot --root . --out target/ripr/pilot --mode draft"));
    assert!(json.contains("--timeout-ms 120000"));
}
