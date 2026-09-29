use super::*;
use crate::agent::loop_commands::check_repo_exposure_command;
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
use crate::output::markdown::powershell_command;
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
        test_target: Some(
            crate::analysis::test_grip_evidence::TestTargetEvidence::fixture(
                "below_threshold_has_no_discount",
                std::path::Path::new("tests/pricing.rs"),
                12,
            ),
        ),
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
        language_routes: None,
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
        repair_action: "strengthen_existing_test".to_string(),
        changed_owner: "calculate_discount".to_string(),
        changed_behavior: "predicate_boundary changed at src/pricing.py:2: `amount >= threshold`"
            .to_string(),
        current_test_evidence:
            "tests/test_pricing.py:6 test_calculate_discount_above_threshold currently has oracle_strength=weak, oracle_kind=broad_assertion: assert result"
                .to_string(),
        missing_discriminator: "amount == threshold".to_string(),
        recommended_test_shape:
            "Strengthen the existing pytest boundary assertion for `amount == threshold`."
                .to_string(),
        suggested_assertion: "Assert the owner result or effect at the boundary `amount == threshold`."
            .to_string(),
        suggested_test_file: "tests/test_pricing.py".to_string(),
        suggested_test_name: "test_calculate_discount_above_threshold".to_string(),
        suggested_test_node_id: Some(
            "tests/test_pricing.py::test_calculate_discount_above_threshold".to_string(),
        ),
        verify_command:
            "pytest tests/test_pricing.py::test_calculate_discount_above_threshold".to_string(),
        verify_command_confidence: "high".to_string(),
        receipt_command: None,
        receipt_status: "unavailable_until_python_gap_ledger".to_string(),
        receipt_guidance:
            "Save this `ripr check --format json` report, then run `ripr first-pr --check-output <check.json>` or `ripr reports gap-ledger --check-output <check.json>` to materialize a gap ledger with a concrete receipt command."
                .to_string(),
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
        language_routes: None,
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
        language_routes: None,
    };

    let json = crate::testing::cwd_placeholder::project_cwd_text(&render_pilot_summary_json(
        &[entry],
        context,
    ));
    assert!(json.contains(r#""schema_version": "0.2""#));
    assert!(json.contains(r#""status": "complete""#));
    assert!(json.contains(r#""state": "loaded""#));
    assert!(json.contains(r#""top_actionable_seams""#));
    assert!(json.contains(r#""missing_discriminator""#));
    assert!(json.contains("ripr outcome --before <cwd>/target/ripr/pilot/repo-exposure.json"));
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
    let md = crate::testing::cwd_placeholder::project_cwd_text(&render_pilot_summary_md(
        &[entry],
        pilot_context(&artifacts),
    ));

    for needle in [
        "## What Was Inspected",
        "## Top Recommendation",
        "- Inspected seam:",
        "(weak, weakly_gripped)",
        " weak (`weakly_gripped`) src/pricing.rs:88 ",
        "- weak, weakly_gripped\n",
        "- Why it matters: missing discriminator: input that hits the boundary: amount >= discount_threshold",
        "- Focused test: none: this seam has no test target that `ripr agent repair` can use (for example, the only tests are in another crate, or static evidence names no exact discriminator), so it will not start a repair attempt here",
        "Target seam:",
        "Target placement blocked:",
        "## Ranked Seams\n\nNone of these seams can start a repair attempt (`ripr agent repair`); they are ranked for inspection by hand.",
        "## Next Commands",
        "No repair attempt is available for the top seam. Next, add a test for `pricing::discounted_total` in the crate that owns src/pricing.rs, then rerun repo exposure and compare the snapshots:",
        "ripr outcome --before <cwd>/target/ripr/pilot/repo-exposure.json",
    ] {
        assert!(md.contains(needle), "missing markdown needle: {needle}");
    }
    // #4216 row 3: a list with no repair start is not headed "actionable",
    // and the producer's route-readiness jargon stays in the JSON packets.
    assert!(!md.contains("## Ranked Actionable Seams"), "{md}");
    assert!(
        !md.contains("- Focused test: not applicable (route limited"),
        "{md}"
    );
}

/// The bash fence content is pinned byte-for-byte: adding the PowerShell
/// variant must never reshape the form existing consumers copy today (#2628).
#[test]
fn pilot_summary_md_pairs_bash_next_commands_with_powershell_variants() -> Result<(), String> {
    let entry = classified_with(
        SeamGripClass::WeaklyGripped,
        "src/pricing.rs",
        88,
        vec![missing()],
        vec![related_test()],
    );
    let artifacts = pilot_artifacts();
    let md = render_pilot_summary_md(&[entry], pilot_context(&artifacts));
    let cwd = crate::agent::loop_commands::bound_root(".");

    // Issue #3872: the after-snapshot redirect anchors at the resolved --root,
    // so both presented forms build from the same builder output the pilot
    // renderer uses (the anchor math itself is pinned in loop_commands tests).
    let after_snapshot = check_repo_exposure_command(
        &crate::agent::loop_commands::bound_root("."),
        "draft",
        "target/ripr/pilot/after.repo-exposure.json",
    );
    let bash_block = format!(
        "```bash\n{after_snapshot}\nripr outcome --before {cwd}/target/ripr/pilot/repo-exposure.json --after {cwd}/target/ripr/pilot/after.repo-exposure.json\n```"
    );
    assert!(
        md.contains(bash_block.as_str()),
        "bash next-commands block drifted:\n{md}"
    );
    // The default pilot path is unquoted in the bash form; PowerShell parses
    // method-call arguments in expression mode, so the WriteAllText target must
    // arrive as a quoted literal (PR #3617 review), and the write is guarded by
    // $LASTEXITCODE with the status propagated so a failed run cannot publish
    // the artifact (PR #3625 review, codex P1).
    let powershell_snapshot = powershell_command(&after_snapshot)
        .ok_or_else(|| "redirect commands gain a powershell variant".to_string())?;
    assert!(
        md.contains(powershell_snapshot.as_str()),
        "powershell after-snapshot translation missing:\n{md}"
    );
    // Disclosure precedes the first copyable command, mirroring the landed
    // agent_workflow ordering, and states the cmd.exe boundary.
    let disclosure = md
        .find("cmd.exe is not supported")
        .ok_or_else(|| format!("pilot markdown must state the cmd.exe boundary: {md}"))?;
    let first_fence = md
        .find("```bash")
        .ok_or_else(|| format!("pilot markdown must fence the bash commands: {md}"))?;
    assert!(
        disclosure < first_fence,
        "shell disclosure at {disclosure} must precede the first command fence at {first_fence}"
    );
    // The redirect-free outcome command translates to itself in PowerShell, so
    // the variant fence still carries a runnable second command.
    let powershell_block = md
        .find("```powershell\n")
        .map(|start| &md[start..])
        .ok_or_else(|| format!("pilot markdown must fence the powershell commands: {md}"))?;
    assert!(
        powershell_block.contains(
            &format!("ripr outcome --before {cwd}/target/ripr/pilot/repo-exposure.json --after {cwd}/target/ripr/pilot/after.repo-exposure.json")
        ),
        "powershell outcome command missing:\n{powershell_block}"
    );
    Ok(())
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
    let seam_id = entry.seam.id().as_str().to_string();
    let artifacts = pilot_artifacts();
    let terminal = crate::testing::cwd_placeholder::project_cwd_text(&render_pilot_terminal(
        &[entry],
        pilot_context(&artifacts),
    ));

    for needle in [
        "Inspected:",
        "root: .",
        "mode: draft",
        "config: loaded ripr.toml",
        "Top recommendation:",
        "inspected seam:",
        "why it matters: missing discriminator: input that hits the boundary: amount >= discount_threshold",
        "focused test: none: this seam has no test target that `ripr agent repair` can use (for example, the only tests are in another crate, or static evidence names no exact discriminator), so it will not start a repair attempt here",
        "assertion: not_applicable",
        "Detailed brief:",
        "target/ripr/pilot/pilot-summary.md",
        "Structured packet:",
        "target/ripr/pilot/agent-seam-packets.json",
        "Next, by hand: add a test for `pricing::discounted_total` in the crate that owns src/pricing.rs, then compare against this run:",
        "ripr outcome --before <cwd>/target/ripr/pilot/repo-exposure.json",
    ] {
        assert!(
            terminal.contains(needle),
            "missing terminal needle: {needle}"
        );
    }
    // Issue #3872: the after-snapshot redirect anchors at the resolved --root.
    let after_snapshot = check_repo_exposure_command(
        &crate::agent::loop_commands::bound_root("."),
        "draft",
        "target/ripr/pilot/after.repo-exposure.json",
    );
    assert!(
        terminal
            .contains(crate::testing::cwd_placeholder::project_cwd_text(&after_snapshot).as_str()),
        "missing anchored after-snapshot needle:\n{terminal}"
    );
    // A route-limited seam keeps the snapshot comparison: the repair
    // transaction has no target here (#3906).
    assert!(!terminal.contains("Next, in order:"), "{terminal}");
    // #4216 row 3: no producer jargon and no wait on "producer evidence" the
    // user cannot supply; the next step is a test the user writes.
    assert!(!terminal.contains("route limited"), "{terminal}");
    assert!(!terminal.contains("producer"), "{terminal}");
    assert!(!terminal.contains("ripr pilot"), "{terminal}");

    // The id leads the line, so the next documented step
    // (`ripr agent repair --seam-id <id>`) is reachable from the screen alone.
    assert!(
        terminal.contains(&format!(
            "inspected seam: {seam_id} src/pricing.rs:88 predicate_boundary in pricing::discounted_total (weak, weakly_gripped)"
        )),
        "the terminal seam line must lead with the seam id:\n{terminal}"
    );

    // This seam's repair route is limited, so the paste-ready repair command
    // must not appear: the id is what the user needs here, not a transaction
    // against a target the route does not have. The applicable-route half of
    // this contract is proved end to end by
    // `pilot_writes_default_packet_outputs_for_boundary_gap_fixture`.
    assert!(
        !terminal.contains("repair this seam:"),
        "a route-limited seam must not be offered a repair transaction:\n{terminal}"
    );
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
        language_routes: None,
    };

    let json = render_pilot_timeout_summary_json(context);
    assert!(json.contains(r#""schema_version": "0.2""#));
    assert!(json.contains(r#""status": "partial""#));
    assert!(json.contains(r#""reason": "timeout""#));
    assert!(json.contains(r#""actionable_seams_total": null"#));
    assert!(json.contains(&format!(
        "ripr pilot --root {0} --out {0}/target/ripr/pilot --mode draft",
        crate::agent::loop_commands::shell_arg(&crate::agent::loop_commands::bound_root("."))
    )));
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
        language_routes: None,
    }
}

#[test]
fn timeout_summary_md_explains_partial_status_and_retry_command() {
    let artifacts = pilot_artifacts();
    let md = crate::testing::cwd_placeholder::project_cwd_text(&render_pilot_timeout_summary_md(
        pilot_context(&artifacts),
    ));

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
        "ripr pilot --root <cwd> --out <cwd>/target/ripr/pilot --mode draft",
        "--timeout-ms 120000",
    ] {
        assert!(md.contains(needle), "missing timeout-md needle: {needle}");
    }
}

/// The retry command carries no redirect and no quoting, so it runs unchanged
/// in PowerShell: the bash form stays byte-identical and no identical
/// PowerShell block repeats it (#2628, F60-12).
#[test]
fn timeout_summary_md_pairs_bash_retry_with_powershell_variant() -> Result<(), String> {
    let artifacts = pilot_artifacts();
    let md = render_pilot_timeout_summary_md(pilot_context(&artifacts));

    let retry = format!(
        "ripr pilot --root {0} --out {0}/target/ripr/pilot --mode draft --max-seams 5 --timeout-ms 120000",
        crate::agent::loop_commands::shell_arg(&crate::agent::loop_commands::bound_root("."))
    );
    assert!(
        md.contains(&format!("```bash\n{retry}\n```")),
        "bash retry block drifted:\n{md}"
    );
    assert!(
        !md.contains("```powershell"),
        "an unchanged retry must not repeat as a PowerShell block:\n{md}"
    );
    let disclosure = md
        .find("cmd.exe is not supported")
        .ok_or_else(|| format!("pilot timeout markdown must state the cmd.exe boundary: {md}"))?;
    let first_fence = md
        .find("```bash")
        .ok_or_else(|| format!("pilot timeout markdown must fence the bash command: {md}"))?;
    assert!(
        disclosure < first_fence,
        "shell disclosure at {disclosure} must precede the first command fence at {first_fence}"
    );
    Ok(())
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
    let terminal = crate::testing::cwd_placeholder::project_cwd_text(
        &render_pilot_timeout_terminal(pilot_context(&artifacts)),
    );

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
        "ripr pilot --root <cwd> --out <cwd>/target/ripr/pilot --mode draft",
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
        r#""repair_action": "strengthen_existing_test""#,
        r#""changed_owner": "calculate_discount""#,
        r#""missing_discriminator": "amount == threshold""#,
        r#""suggested_test_file": "tests/test_pricing.py""#,
        r#""suggested_test_name": "test_calculate_discount_above_threshold""#,
        r#""verify_command": "pytest tests/test_pricing.py::test_calculate_discount_above_threshold""#,
        r#""receipt_status": "unavailable_until_python_gap_ledger""#,
        r#""receipt_guidance": "Save this `ripr check --format json` report, then run `ripr first-pr --check-output <check.json>` or `ripr reports gap-ledger --check-output <check.json>` to materialize a gap ledger with a concrete receipt command.""#,
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
        "Repair action: `strengthen_existing_test`",
        "Changed owner: `calculate_discount`",
        "Missing discriminator: `amount == threshold`",
        "Suggested test target: `test_calculate_discount_above_threshold` in `tests/test_pricing.py`",
        "Verify: `pytest tests/test_pricing.py::test_calculate_discount_above_threshold`",
        "Receipt status: `unavailable_until_python_gap_ledger`",
        "Receipt guidance: Save this `ripr check --format json` report, then run `ripr first-pr --check-output <check.json>` or `ripr reports gap-ledger --check-output <check.json>` to materialize a gap ledger with a concrete receipt command.",
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
        "repair action: strengthen_existing_test",
        "changed owner: calculate_discount",
        "missing discriminator: amount == threshold",
        "recommended repair: strengthen test_calculate_discount_above_threshold in tests/test_pricing.py",
        "verify: pytest tests/test_pricing.py::test_calculate_discount_above_threshold",
        "receipt status: unavailable_until_python_gap_ledger",
        "receipt guidance: Save this `ripr check --format json` report, then run `ripr first-pr --check-output <check.json>` or `ripr reports gap-ledger --check-output <check.json>` to materialize a gap ledger with a concrete receipt command.",
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

/// A route-ready seam related to one test per file in `test_files` (#3906).
/// The missing-discriminator shape and a fully observed `discriminate` stage
/// are what `repair_route_readiness` needs to select a target.
fn route_ready_entry(test_files: &[&str]) -> ClassifiedSeam {
    let related = test_files
        .iter()
        .map(|file| {
            let mut test = related_test();
            test.file = PathBuf::from(file);
            test
        })
        .collect();
    let mut entry = classified_with(
        SeamGripClass::WeaklyGripped,
        "src/pricing.rs",
        88,
        vec![MissingDiscriminatorFact {
            value: "discount_threshold (equality boundary)".to_string(),
            reason: "observed values do not include the equality-boundary case".to_string(),
            flow_sink: None,
        }],
        related,
    );
    entry.evidence.discriminate = stage(StageState::Yes);
    entry
}

/// Every pilot surface offers `agent repair` only when the fail-closed
/// repair-packet flip holds, not when route readiness alone does (#3906). The
/// second entry adds a TypeScript observer beside the same Rust test: the Rust
/// test still resolves a target, so the route stays ready and the focused-test
/// outline stays applicable, but the oracle path is unresolved from Rust
/// evidence. A renderer gated on readiness or on the outline passes the first
/// half and fails the second. The third is eligible but its recommended test
/// sits in the production file, which `agent repair` refuses to edit.
#[test]
fn pilot_offers_agent_repair_only_past_the_repair_packet_flip() -> Result<(), String> {
    use crate::analysis::repair_route::repair_packet_eligibility;
    use crate::output::agent_seam_packets::targeted_test_brief_outline_for_classified_seam;

    let artifacts = pilot_artifacts();
    for (test_files, eligible, offered_expected) in [
        (&["tests/pricing.rs"][..], true, true),
        (
            &["tests/pricing.rs", "tests/pricing.test.ts"][..],
            false,
            false,
        ),
        // Eligible, but the recommended test is an inline module in a
        // production file, which `agent repair` refuses as an edit target.
        (&["src/pricing.rs"][..], true, false),
    ] {
        let test_file = test_files.join(" + ");
        let entry = route_ready_entry(test_files);
        // Fixture preconditions: both are route ready with an applicable
        // focused-test outline; only the Rust-only one is eligible.
        let eligibility = repair_packet_eligibility(&entry);
        if !eligibility.readiness.is_repair_ready() {
            return Err(format!("{test_file}: fixture must be route ready"));
        }
        if eligibility.eligible() != eligible {
            return Err(format!("{test_file}: eligibility must be {eligible}"));
        }
        if targeted_test_brief_outline_for_classified_seam(&entry).is_not_applicable() {
            return Err(format!(
                "{test_file}: the focused-test outline must stay applicable"
            ));
        }
        // The inline-test case must reach the test-surface check with its
        // production file as the recommended target, or its row is vacuous.
        let recommended = crate::output::agent_seam_packets::recommended_test_for(&entry).file;
        if eligible && crate::analysis::is_test_surface_path(&recommended) != offered_expected {
            return Err(format!(
                "{test_file}: recommended test `{recommended}` test-surface must be {offered_expected}"
            ));
        }
        let command = format!(
            "ripr agent repair --root {} --seam-id {} --phase before",
            crate::agent::loop_commands::shell_arg(&crate::agent::loop_commands::bound_root(".")),
            entry.seam.id().as_str()
        );
        let entries = [entry];

        let terminal = render_pilot_terminal(&entries, pilot_context(&artifacts));
        let json = render_pilot_summary_json(&entries, pilot_context(&artifacts));
        let summary: serde_json::Value =
            serde_json::from_str(&json).map_err(|e| format!("parse pilot JSON: {e}\n{json}"))?;
        let md = render_pilot_summary_md(&entries, pilot_context(&artifacts));

        // Both must be the ranked top seam, or the negative half is vacuous.
        if !terminal.contains(&format!(
            "inspected seam: {} ",
            entries[0].seam.id().as_str()
        )) {
            return Err(format!("{test_file}: must be the top seam\n{terminal}"));
        }
        let offered = [
            terminal.contains(&format!("  repair this seam: {command}\n")),
            terminal.contains(&format!("  1. {command}\n")),
            summary
                .pointer("/next/repair_command")
                .and_then(serde_json::Value::as_str)
                == Some(command.as_str()),
            md.contains(&command),
        ];
        if offered != [offered_expected; 4] {
            return Err(format!(
                "{test_file}: terminal line, closing step, JSON, Markdown = {offered:?}, want all {offered_expected}\n{terminal}\n{json}\n{md}"
            ));
        }
        if !offered_expected {
            if !summary
                .pointer("/next/repair_command")
                .is_some_and(serde_json::Value::is_null)
            {
                return Err(format!(
                    "{test_file}: JSON repair_command must be null\n{json}"
                ));
            }
            if terminal.contains("agent repair") || terminal.contains("Next, in order:") {
                return Err(format!(
                    "{test_file}: no repair route on screen\n{terminal}"
                ));
            }
        }
    }
    Ok(())
}

fn discovered_files(
    entries: &[(crate::domain::LanguageId, &str)],
) -> Vec<(crate::domain::LanguageId, PathBuf)> {
    entries
        .iter()
        .map(|(language, path)| (*language, PathBuf::from(path)))
        .collect()
}

#[test]
fn pilot_language_routes_state_follows_rust_seams_and_discovered_languages() {
    use super::language_routes::PilotLanguageRoutesState;
    use crate::domain::LanguageId;
    use crate::output::repo_exposure::{PythonRepoExposureGuidance, TsFullRepoGuidance};

    let root = Path::new(".");
    let rust_only = PilotLanguageRoutes::from_discovered(root, false, &[LanguageId::Rust], &[]);
    assert_eq!(rust_only.state, PilotLanguageRoutesState::NotDetected);
    assert!(rust_only.routes.is_empty());
    assert!(rust_only.required().is_none());

    let files = discovered_files(&[
        (LanguageId::Perl, "lib/App.pm"),
        (LanguageId::Python, "src/app.py"),
        (LanguageId::JavaScript, "web/b.js"),
        (LanguageId::TypeScript, "web/c.ts"),
        (LanguageId::TypeScript, "web/d.ts"),
    ]);
    let enabled = [LanguageId::Rust, LanguageId::TypeScript];
    let required = PilotLanguageRoutes::from_discovered(root, false, &enabled, &files);
    assert_eq!(required.state, PilotLanguageRoutesState::Required);
    assert_eq!(required.state.as_str(), "required");
    // A language this binary cannot analyze (#4252: TypeScript and Python
    // in a Rust-only build, Perl without `lang-perl`) gets the unavailable
    // route whatever ripr.toml enables: no command, the adapter notice.
    let assert_unavailable = |route: &super::language_routes::PilotLanguageRoute| {
        assert_eq!(route.language_status(), "unavailable");
        assert_eq!(route.route(), "unavailable_in_this_binary");
        assert_eq!(route.command, None);
        assert_eq!(route.guidance, route.language.unavailable_adapter_notice());
    };
    let typescript_available = LanguageId::TypeScript.is_available();
    let summary = required
        .routes
        .iter()
        .map(|route| (route.language, route.file_count, route.enabled))
        .collect::<Vec<_>>();
    assert_eq!(
        summary,
        vec![
            (LanguageId::TypeScript, 2, typescript_available),
            // JavaScript runs through the TypeScript-family adapter.
            (LanguageId::JavaScript, 1, typescript_available),
            (LanguageId::Python, 1, false),
            (LanguageId::Perl, 1, false),
        ]
    );
    for route in &required.routes[..2] {
        if !typescript_available {
            assert_unavailable(route);
            continue;
        }
        assert_eq!(route.command.as_deref(), Some("ripr check --root ."));
        assert_eq!(route.guidance_category, Some(TsFullRepoGuidance::CATEGORY));
        assert_eq!(
            route.guidance.as_deref(),
            Some(TsFullRepoGuidance::REPAIR_ROUTE)
        );
    }
    let python = &required.routes[2];
    if LanguageId::Python.is_available() {
        assert_eq!(python.command.as_deref(), Some("ripr check --root ."));
        assert_eq!(
            python.guidance_category,
            Some(PythonRepoExposureGuidance::CATEGORY)
        );
        assert_eq!(
            python.guidance.as_deref(),
            Some(PythonRepoExposureGuidance::REPAIR_ROUTE)
        );
    } else {
        assert_unavailable(python);
    }
    let perl = &required.routes[3];
    if LanguageId::Perl.is_available() {
        assert_eq!(perl.language_status(), "preview");
        assert_eq!(perl.command.as_deref(), Some("ripr check --root ."));
    } else {
        assert_unavailable(perl);
    }
    let expected_commands: Vec<&str> = if typescript_available || LanguageId::Python.is_available()
    {
        vec!["ripr check --root ."]
    } else {
        Vec::new()
    };
    assert_eq!(
        PilotLanguageRoutes::commands(&required.routes),
        expected_commands
    );

    let supplementary = PilotLanguageRoutes::from_discovered(root, true, &enabled, &files);
    assert_eq!(supplementary.state, PilotLanguageRoutesState::Supplementary);
    assert_eq!(supplementary.routes, required.routes);
    assert!(supplementary.required().is_none());
}

#[test]
// Compares the route label across runnable TypeScript and Python routes;
// a build lacking either has no such pair (#4252).
#[cfg(all(feature = "lang-typescript", feature = "lang-python"))]
fn pilot_terminal_route_label_has_one_shape_for_every_language() {
    use crate::domain::LanguageId;
    use crate::output::repo_exposure::{PythonRepoExposureGuidance, TsFullRepoGuidance};

    // Re-walk N10: the route label is `route:` for every language. The
    // guidance category stays in JSON and Markdown, so TypeScript
    // (`typescript_diff_first`) and Python (`python_diff_first`) print the
    // same label.
    let artifacts = pilot_artifacts();
    let files = discovered_files(&[
        (LanguageId::TypeScript, "web/c.ts"),
        (LanguageId::JavaScript, "web/d.js"),
        (LanguageId::Python, "src/pricing.py"),
    ]);
    let enabled = [LanguageId::Rust, LanguageId::TypeScript, LanguageId::Python];
    let routes = PilotLanguageRoutes::from_discovered(Path::new("."), false, &enabled, &files);
    let runnable: Vec<_> = routes
        .routes
        .iter()
        .filter(|route| route.command.is_some())
        .collect();
    assert!(
        runnable.iter().any(|route| {
            route.language == LanguageId::TypeScript
                && route.guidance_category == Some(TsFullRepoGuidance::CATEGORY)
        }),
        "{routes:?}"
    );
    assert!(
        runnable.iter().any(|route| {
            route.language == LanguageId::Python
                && route.guidance_category == Some(PythonRepoExposureGuidance::CATEGORY)
        }),
        "{routes:?}"
    );

    let context = PilotSummaryContext {
        language_routes: Some(&routes),
        ..pilot_context(&artifacts)
    };
    let terminal = render_pilot_terminal(&[], context);
    for route in &runnable {
        let header = format!("  {}: ", route.language.as_str());
        let route_line = terminal
            .lines()
            .skip_while(|line| !line.starts_with(&header))
            .nth(1)
            .unwrap_or_default();
        assert_eq!(
            route_line,
            "    route: ripr check --root .",
            "{} route label differs:\n{terminal}",
            route.language.as_str()
        );
    }
}

#[test]
fn pilot_renderers_show_language_routes_only_without_rust_seams() -> Result<(), String> {
    use crate::domain::LanguageId;

    let artifacts = pilot_artifacts();
    let files = discovered_files(&[
        (LanguageId::TypeScript, "web/c.ts"),
        (LanguageId::Perl, "lib/App.pm"),
    ]);
    let root = Path::new(".");

    // Rust seams exist: terminal and Markdown are byte-identical to a run
    // without language routes, and JSON lists the routes as supplementary.
    let entries = [classified_with(
        SeamGripClass::WeaklyGripped,
        "src/pricing.rs",
        88,
        vec![missing()],
        vec![related_test()],
    )];
    let supplementary =
        PilotLanguageRoutes::from_discovered(root, true, &[LanguageId::Rust], &files);
    let with_routes = PilotSummaryContext {
        language_routes: Some(&supplementary),
        ..pilot_context(&artifacts)
    };
    assert_eq!(
        render_pilot_terminal(&entries, with_routes),
        render_pilot_terminal(&entries, pilot_context(&artifacts))
    );
    assert_eq!(
        render_pilot_summary_md(&entries, with_routes),
        render_pilot_summary_md(&entries, pilot_context(&artifacts))
    );
    let json = render_pilot_summary_json(&entries, with_routes);
    assert!(json.contains("\"state\": \"supplementary\""), "{json}");
    assert!(json.contains("\"language\": \"typescript\""), "{json}");
    let parsed: serde_json::Value = serde_json::from_str(&json)
        .map_err(|err| format!("pilot summary JSON must parse: {err}\n{json}"))?;
    assert_eq!(parsed["language_routes"]["routes"][1]["language"], "perl");

    // No Rust seams: every surface names the routes and none reads as clean.
    let required = PilotLanguageRoutes::from_discovered(root, false, &[LanguageId::Rust], &files);
    let context = PilotSummaryContext {
        language_routes: Some(&required),
        ..pilot_context(&artifacts)
    };
    let terminal = render_pilot_terminal(&[], context);
    assert!(
        !terminal.contains("none ranked by the default pilot policy"),
        "{terminal}"
    );
    let md = render_pilot_summary_md(&[], context);
    assert!(
        md.contains("## Languages Outside The Rust Seam Scan"),
        "{md}"
    );
    if let Some(notice) = LanguageId::TypeScript.unavailable_adapter_notice() {
        // #4252: a Rust-only build names the rebuild and invents no route.
        assert!(
            terminal.contains(&format!(
                "typescript: 1 file (not available in this build)\n    {notice}\n"
            )),
            "{terminal}"
        );
        assert!(md.contains(&notice), "{md}");
        if !LanguageId::Perl.is_available() {
            // Neither discovered language is analyzable: no command at all.
            assert!(!terminal.contains("route: ripr check"), "{terminal}");
            assert!(
                terminal.ends_with("No follow-up command applies: this ripr binary cannot analyze the languages listed above.\n"),
                "{terminal}"
            );
            assert!(!md.contains("```bash"), "{md}");
        }
    } else {
        assert!(
            terminal.contains("typescript: 1 file (preview, diff-first; not enabled in ripr.toml [languages])\n    route: ripr check --root .\n"),
            "{terminal}"
        );
        assert!(
            terminal.ends_with(
                "Next, analyze the changed code in these languages:\n  ripr check --root .\n"
            ),
            "{terminal}"
        );
        assert!(md.contains("```bash\nripr check --root .\n```"), "{md}");
    }
    assert!(!terminal.contains("ripr outcome --before"), "{terminal}");
    assert!(!md.contains("ripr outcome --before"), "{md}");
    let json = render_pilot_summary_json(&[], context);
    assert!(json.contains("\"state\": \"required\""), "{json}");
    if let Some(notice) = LanguageId::Perl.unavailable_adapter_notice() {
        assert!(
            terminal.contains(&format!(
                "perl: 1 file (not available in this build)\n    {notice}\n"
            )),
            "{terminal}"
        );
        assert!(md.contains(&notice), "{md}");
    }

    // Only unavailable languages: no runnable command is invented.
    if !LanguageId::Perl.is_available() {
        let perl_only = PilotLanguageRoutes::from_discovered(
            root,
            false,
            &[LanguageId::Rust],
            &discovered_files(&[(LanguageId::Perl, "lib/App.pm")]),
        );
        let context = PilotSummaryContext {
            language_routes: Some(&perl_only),
            ..pilot_context(&artifacts)
        };
        let terminal = render_pilot_terminal(&[], context);
        assert!(
            terminal.ends_with("No follow-up command applies: this ripr binary cannot analyze the languages listed above.\n"),
            "{terminal}"
        );
        let md = render_pilot_summary_md(&[], context);
        assert!(md.ends_with("No follow-up command applies: this ripr binary cannot analyze the languages listed above.\n"), "{md}");
        assert!(!md.contains("```bash"), "{md}");
    }
    Ok(())
}

/// rc rehearsal (py-pricing): with no Rust seam, a Python repair card was
/// the top recommendation but the closing block said `ripr check`, which only
/// leads back to pilot. The closing block now names the card's route:
/// `ripr first-pr` before the edit (it names the receipt command, and stops
/// selecting the gap once the edit closes it), the test edit, the card's
/// verify command, and the receipt command. A card that carries its own
/// receipt command skips first-pr.
#[test]
fn pilot_terminal_next_follows_the_python_repair_card_route() -> Result<(), String> {
    use crate::domain::LanguageId;

    let artifacts = pilot_artifacts();
    let python = python_first_use();
    let files = discovered_files(&[(LanguageId::Python, "pricing/__init__.py")]);
    let routes =
        PilotLanguageRoutes::from_discovered(Path::new("."), false, &[LanguageId::Python], &files);
    let first_pr = format!(
        "ripr first-pr --root {}",
        crate::agent::loop_commands::shell_arg(&crate::agent::loop_commands::bound_root("."))
    );
    for language_routes in [None, Some(&routes)] {
        let context = PilotSummaryContext {
            language_routes,
            ..pilot_context_with_python(&artifacts, &python)
        };
        let terminal = render_pilot_terminal(&[], context);
        let expected = format!(
            "Next, in order:\n  1. {first_pr} (names this gap's receipt command; run any regeneration command it prints first)\n  2. strengthen test_calculate_discount_above_threshold in tests/test_pricing.py (test files only): Assert the owner result or effect at the boundary `amount == threshold`.\n  3. pytest tests/test_pricing.py::test_calculate_discount_above_threshold\n  4. run the receipt command step 1 printed\n"
        );
        assert!(terminal.ends_with(&expected), "{terminal}");
        assert!(
            !terminal.contains("Next, analyze the changed code"),
            "{terminal}"
        );
        assert!(!terminal.contains("ripr outcome --before"), "{terminal}");
        let md = render_pilot_summary_md(&[], context);
        let next = md
            .split("## Next Commands")
            .nth(1)
            .ok_or_else(|| format!("missing Next Commands: {md}"))?;
        assert!(
            next.contains(&format!(
                "```bash\n{first_pr}\npytest tests/test_pricing.py::test_calculate_discount_above_threshold\n```"
            )),
            "{next}"
        );
        assert!(!next.contains("ripr check --root"), "{next}");
    }

    // A card that carries its own receipt command uses it directly.
    let mut carded = python_first_use();
    if let Some(card) = carded.top_repair_card.as_mut() {
        card.receipt_command = Some("ripr receipt write --gap g --status not_run".to_string());
    }
    let context = pilot_context_with_python(&artifacts, &carded);
    let terminal = render_pilot_terminal(&[], context);
    assert!(
        terminal.ends_with("  2. pytest tests/test_pricing.py::test_calculate_discount_above_threshold\n  3. ripr receipt write --gap g --status not_run\n"),
        "{terminal}"
    );
    assert!(!terminal.contains("ripr first-pr --root"), "{terminal}");
    let md = render_pilot_summary_md(&[], context);
    assert!(
        md.contains("```bash\npytest tests/test_pricing.py::test_calculate_discount_above_threshold\nripr receipt write --gap g --status not_run\n```"),
        "{md}"
    );
    Ok(())
}
