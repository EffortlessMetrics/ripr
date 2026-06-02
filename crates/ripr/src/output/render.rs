use super::{
    agent_seam_packets, badge, format::OutputFormat, github, human, json, repo_exposure,
    repo_seams, sarif, suppressions,
};
use crate::analysis;
use crate::app::CheckOutput;
use crate::config::RiprConfig;
use std::collections::BTreeMap;

/// Path (relative to the analyzed workspace root) where the
/// test-efficiency report is expected when rendering `ripr+` badge formats.
const TEST_EFFICIENCY_REPORT_RELATIVE: &str = "target/ripr/reports/test-efficiency.json";

pub(crate) fn render_check_with_config(
    output: &CheckOutput,
    format: &OutputFormat,
    config: &RiprConfig,
) -> Result<String, String> {
    match format {
        OutputFormat::Human => Ok(human::render_with_config(output, config)),
        OutputFormat::Json => Ok(json::render_with_config(output, config)),
        OutputFormat::Github => Ok(github::render_with_config(output, config)),
        OutputFormat::Sarif => {
            let suppressions = load_suppressions(output, config)?;
            Ok(sarif::render_findings_sarif(output, config, &suppressions))
        }
        OutputFormat::BadgeJson => {
            let summary = ripr_summary_with_suppressions(output, config)?;
            Ok(badge::render_native_json(&summary))
        }
        OutputFormat::RepoBadgeJson => {
            let summary = ripr_repo_canonical_actionable_summary(output, config)?;
            Ok(badge::render_native_json(&summary))
        }
        OutputFormat::BadgeShields => {
            let summary = ripr_summary_with_suppressions(output, config)?;
            Ok(badge::render_shields_json(&summary))
        }
        OutputFormat::RepoBadgeShields => {
            let summary = ripr_repo_canonical_actionable_summary(output, config)?;
            Ok(badge::render_shields_json(&summary))
        }
        OutputFormat::BadgePlusJson | OutputFormat::RepoBadgePlusJson => {
            let summary = ripr_plus_summary_from_disk(output, format.is_repo_scope(), config)?;
            Ok(badge::render_native_json(&summary))
        }
        OutputFormat::BadgePlusShields | OutputFormat::RepoBadgePlusShields => {
            let summary = ripr_plus_summary_from_disk(output, format.is_repo_scope(), config)?;
            Ok(badge::render_shields_json(&summary))
        }
        OutputFormat::RepoSeamsJson => {
            let seams = analysis::inventory_seams_at(&output.root)?;
            Ok(repo_seams::render_repo_seams_json(&seams))
        }
        OutputFormat::RepoSeamsMd => {
            let seams = analysis::inventory_seams_at(&output.root)?;
            Ok(repo_seams::render_repo_seams_md(&seams))
        }
        OutputFormat::RepoExposureJson => {
            let classified =
                analysis::inventory_classified_seams_at_with_config(&output.root, config)?;
            Ok(repo_exposure::render_repo_exposure_json(&classified))
        }
        OutputFormat::RepoExposureSummaryJson => {
            let classified =
                analysis::inventory_compact_classified_seams_at_with_config(&output.root, config)?;
            Ok(repo_exposure::render_repo_exposure_summary_json(
                &classified,
                &output.root,
                output.base.as_deref(),
                output.mode.as_str(),
            ))
        }
        OutputFormat::RepoExposureMd => {
            let classified =
                analysis::inventory_classified_seams_at_with_config(&output.root, config)?;
            Ok(repo_exposure::render_repo_exposure_md(&classified))
        }
        OutputFormat::RepoSarif => {
            let classified =
                analysis::inventory_classified_seams_at_with_config(&output.root, config)?;
            Ok(sarif::render_repo_seams_sarif(&classified, config))
        }
        OutputFormat::AgentSeamPacketsJson => {
            let classified =
                analysis::inventory_classified_seams_at_with_config(&output.root, config)?;
            Ok(agent_seam_packets::render_agent_seam_packets_json(
                &classified,
            ))
        }
    }
}

fn load_suppressions(
    output: &CheckOutput,
    config: &RiprConfig,
) -> Result<Vec<suppressions::SuppressionEntry>, String> {
    suppressions::load_suppressions_for_root_at(&output.root, config.suppressions().path()).map_err(
        |violations| {
            format!(
                "{} validation failed:\n{}",
                config.suppressions().display_path(),
                violations.join("\n")
            )
        },
    )
}

fn ripr_summary_with_suppressions(
    output: &CheckOutput,
    config: &RiprConfig,
) -> Result<badge::BadgeSummary, String> {
    let suppressions = load_suppressions(output, config)?;
    let today = suppressions::current_iso_date();
    let policy = badge::BadgePolicy {
        suppressions_path: config.suppressions().display_path(),
        ..badge::BadgePolicy::default()
    };
    Ok(badge::ripr_badge_summary_with_suppressions(
        output,
        &suppressions,
        &today,
        policy,
    ))
}

fn ripr_repo_canonical_actionable_summary(
    output: &CheckOutput,
    config: &RiprConfig,
) -> Result<badge::BadgeSummary, String> {
    let classified =
        analysis::inventory_compact_classified_seams_at_with_config(&output.root, config)?;
    let policy = badge::BadgePolicy {
        suppressions_path: config.suppressions().display_path(),
        ..badge::BadgePolicy::default()
    };
    Ok(badge::ripr_canonical_actionable_gap_badge_summary(
        &classified,
        policy,
    ))
}

fn ripr_plus_summary_from_disk(
    output: &CheckOutput,
    repo_scope: bool,
    config: &RiprConfig,
) -> Result<badge::BadgeSummary, String> {
    let report_path = output.root.join(TEST_EFFICIENCY_REPORT_RELATIVE);
    if !report_path.exists() {
        let warning = format!(
            "missing {}; provide test-efficiency JSON before requesting a measured ripr+ badge; see docs/BADGE_ADOPTION.md",
            report_path.display()
        );
        eprintln!("ripr: {warning}; rendering neutral ripr+ badge");
        return Ok(missing_test_efficiency_badge_summary(
            repo_scope, config, warning,
        ));
    }
    let text = std::fs::read_to_string(&report_path)
        .map_err(|err| format!("failed to read {}: {err}", report_path.display()))?;
    let test_efficiency = badge::parse_test_efficiency_badge_summary(&text)?;
    let suppressions = load_suppressions(output, config)?;
    let today = suppressions::current_iso_date();
    // `cargo xtask test-efficiency-report` is repo-wide as a fact source.
    // Diff-scoped `ripr+` filters that ledger to entries related to the
    // changed code (via `Finding.related_tests` names + `Finding.probe.owner`
    // intersected with each entry's `reached_owners`); repo-scoped
    // `ripr+` aggregates the repo-wide ledger directly.
    let diff_filter = if repo_scope {
        None
    } else {
        Some(badge::DiffRelatedTests::from_check_output(output))
    };
    let scope = match &diff_filter {
        Some(filter) => badge::TestEfficiencyAggregationScope::Diff(filter),
        None => badge::TestEfficiencyAggregationScope::Repo,
    };
    let policy = badge::BadgePolicy {
        suppressions_path: config.suppressions().display_path(),
        ..badge::BadgePolicy::default()
    };
    if repo_scope {
        let exposure = ripr_repo_canonical_actionable_summary(output, config)?;
        Ok(badge::ripr_plus_canonical_actionable_gap_badge_summary(
            exposure,
            test_efficiency,
            policy,
        ))
    } else {
        Ok(badge::ripr_plus_badge_summary_with_suppressions(
            output,
            test_efficiency,
            &suppressions,
            &today,
            policy,
            scope,
        ))
    }
}

fn missing_test_efficiency_badge_summary(
    repo_scope: bool,
    config: &RiprConfig,
    warning: String,
) -> badge::BadgeSummary {
    badge::BadgeSummary {
        kind: badge::BadgeKind::RiprPlus,
        scope: if repo_scope {
            badge::BadgeScope::Repo
        } else {
            badge::BadgeScope::Diff
        },
        basis: if repo_scope {
            badge::BadgeBasis::CanonicalActionableGap
        } else {
            badge::BadgeBasis::FindingExposure
        },
        message: "needs test-efficiency".to_string(),
        status: badge::BadgeStatus::Warn,
        color: "lightgrey",
        counts: badge::BadgeCounts {
            unsuppressed_exposure_gaps: 0,
            unsuppressed_test_efficiency_findings: 0,
            intentional_test_efficiency_findings: 0,
            suppressed_exposure_gaps: 0,
            suppressed_test_efficiency_findings: 0,
            unknowns: 0,
            unknowns_test_efficiency: 0,
            analyzed_findings: 0,
            analyzed_seams: 0,
            analyzed_gap_records: 0,
            analyzed_tests: 0,
        },
        reason_counts: BTreeMap::new(),
        policy: badge::BadgePolicy {
            suppressions_path: config.suppressions().display_path(),
            ..badge::BadgePolicy::default()
        },
        warnings: vec![warning],
    }
}

#[cfg(test)]
mod tests {
    use super::render_check_with_config;
    use crate::app::{CheckOutput, Mode};
    use crate::config::RiprConfig;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, OracleKind,
        OracleStrength, Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence,
        SourceLocation, StageEvidence, StageState, StopReason, Summary, SymbolId,
    };
    use crate::output::format::OutputFormat;
    use std::path::{Path, PathBuf};

    #[test]
    fn render_dispatch_renders_diff_sarif() -> Result<(), String> {
        let output = check_output_with(vec![sample_finding("src/lib.rs", 1)]);
        let rendered =
            render_check_with_config(&output, &OutputFormat::Sarif, &RiprConfig::default())?;

        assert!(rendered.contains("\"version\": \"2.1.0\""));
        assert!(rendered.contains("ripr.finding.weakly_exposed"));
        Ok(())
    }

    #[test]
    fn render_dispatch_renders_repo_seam_formats() -> Result<(), String> {
        let output = check_output_with_temp_seam_workspace(Vec::new())?;

        let seams_json = render_check_with_config(
            &output,
            &OutputFormat::RepoSeamsJson,
            &RiprConfig::default(),
        )?;
        let seams_md =
            render_check_with_config(&output, &OutputFormat::RepoSeamsMd, &RiprConfig::default())?;

        assert!(seams_json.contains("\"schema_version\": \"0.1\""));
        assert!(seams_json.contains("over_threshold"));
        assert!(seams_md.contains("over_threshold"));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_renders_repo_exposure_and_sarif_formats() -> Result<(), String> {
        let output = check_output_with_temp_seam_workspace(Vec::new())?;

        let exposure_json = render_check_with_config(
            &output,
            &OutputFormat::RepoExposureJson,
            &RiprConfig::default(),
        )?;
        let exposure_md = render_check_with_config(
            &output,
            &OutputFormat::RepoExposureMd,
            &RiprConfig::default(),
        )?;
        let sarif =
            render_check_with_config(&output, &OutputFormat::RepoSarif, &RiprConfig::default())?;

        assert!(exposure_json.contains("\"schema_version\": \"0.3\""));
        assert!(exposure_json.contains("over_threshold"));
        let exposure_summary = render_check_with_config(
            &output,
            &OutputFormat::RepoExposureSummaryJson,
            &RiprConfig::default(),
        )?;
        assert!(exposure_summary.contains("\"format\": \"repo-exposure-summary-json\""));
        assert!(exposure_summary.contains("\"basis\": \"canonical_actionable_gap\""));
        assert!(!exposure_summary.contains("\"evidence_record\""));
        assert!(exposure_md.contains("over_threshold"));
        assert!(sarif.contains("\"version\": \"2.1.0\""));
        assert!(sarif.contains("ripr.seam."));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_renders_agent_seam_packets() -> Result<(), String> {
        let output = check_output_with_temp_seam_workspace(Vec::new())?;

        let rendered = render_check_with_config(
            &output,
            &OutputFormat::AgentSeamPacketsJson,
            &RiprConfig::default(),
        )?;

        assert!(rendered.contains("\"schema_version\": \"0.3\""));
        assert!(rendered.contains("\"packets\""));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_reads_diff_badge_plus_report() -> Result<(), String> {
        let output =
            check_output_with_temp_report_workspace(vec![sample_finding("src/lib.rs", 1)])?;

        let native = render_check_with_config(
            &output,
            &OutputFormat::BadgePlusJson,
            &RiprConfig::default(),
        )?;
        let shields = render_check_with_config(
            &output,
            &OutputFormat::BadgePlusShields,
            &RiprConfig::default(),
        )?;

        assert!(native.contains("\"kind\": \"ripr_plus\""));
        assert!(native.contains("\"scope\": \"diff\""));
        assert!(shields.contains("\"schemaVersion\": 1"));
        assert!(!shields.contains("\"scope\""));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_reads_repo_badge_plus_report() -> Result<(), String> {
        let output = check_output_with_temp_seam_workspace(Vec::new())?;
        write_test_efficiency_report(&output.root)?;

        let native = render_check_with_config(
            &output,
            &OutputFormat::RepoBadgePlusJson,
            &RiprConfig::default(),
        )?;
        let shields = render_check_with_config(
            &output,
            &OutputFormat::RepoBadgePlusShields,
            &RiprConfig::default(),
        )?;

        assert!(native.contains("\"kind\": \"ripr_plus\""));
        assert!(native.contains("\"scope\": \"repo\""));
        assert!(native.contains("\"basis\": \"canonical_actionable_gap\""));
        assert!(shields.contains("\"schemaVersion\": 1"));
        assert!(!shields.contains("\"scope\""));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_renders_human_json_github_with_default_config() -> Result<(), String> {
        let output = check_output_with(vec![sample_finding("src/lib.rs", 1)]);
        let config = RiprConfig::default();

        let human = render_check_with_config(&output, &OutputFormat::Human, &config)?;
        let json = render_check_with_config(&output, &OutputFormat::Json, &config)?;
        let github = render_check_with_config(&output, &OutputFormat::Github, &config)?;

        assert!(!human.is_empty());
        assert!(json.contains("\"schema_version\""));
        assert!(github.contains("ripr"));
        Ok(())
    }

    #[test]
    fn render_dispatch_renders_diff_badge_formats() -> Result<(), String> {
        let output = check_output_with(vec![sample_finding("src/lib.rs", 1)]);
        let config = RiprConfig::default();

        let native = render_check_with_config(&output, &OutputFormat::BadgeJson, &config)?;
        let shields = render_check_with_config(&output, &OutputFormat::BadgeShields, &config)?;

        assert!(native.contains("\"kind\""));
        assert!(shields.contains("\"schemaVersion\": 1"));
        Ok(())
    }

    #[test]
    fn render_dispatch_renders_repo_badge_formats_with_seam_workspace() -> Result<(), String> {
        let output = check_output_with_temp_seam_workspace(Vec::new())?;
        let config = RiprConfig::default();

        let native = render_check_with_config(&output, &OutputFormat::RepoBadgeJson, &config)?;
        let shields = render_check_with_config(&output, &OutputFormat::RepoBadgeShields, &config)?;

        assert!(native.contains("\"scope\": \"repo\""));
        assert!(native.contains("\"basis\": \"canonical_actionable_gap\""));
        assert!(shields.contains("\"schemaVersion\": 1"));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_sarif_surfaces_malformed_suppressions_as_error() -> Result<(), String> {
        let output = check_output_with_temp_malformed_suppressions()?;
        let result =
            render_check_with_config(&output, &OutputFormat::Sarif, &RiprConfig::default());

        let err = expect_err(result)?;
        assert!(
            err.contains("validation failed"),
            "expected suppressions validation failure, got: {err}"
        );
        assert!(err.contains(".ripr/suppressions.toml"));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_badge_json_surfaces_malformed_suppressions_as_error() -> Result<(), String> {
        let output = check_output_with_temp_malformed_suppressions()?;
        let result =
            render_check_with_config(&output, &OutputFormat::BadgeJson, &RiprConfig::default());

        let err = expect_err(result)?;
        assert!(err.contains("validation failed"));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_badge_shields_surfaces_malformed_suppressions_as_error() -> Result<(), String>
    {
        let output = check_output_with_temp_malformed_suppressions()?;
        let result =
            render_check_with_config(&output, &OutputFormat::BadgeShields, &RiprConfig::default());

        let err = expect_err(result)?;
        assert!(err.contains("validation failed"));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_repo_badge_json_surfaces_missing_workspace_as_error() -> Result<(), String> {
        let output = check_output_with_nonexistent_root();
        let result = render_check_with_config(
            &output,
            &OutputFormat::RepoBadgeJson,
            &RiprConfig::default(),
        );

        let err = expect_err(result)?;
        assert!(!err.is_empty());
        Ok(())
    }

    #[test]
    fn render_dispatch_repo_badge_shields_surfaces_missing_workspace_as_error() -> Result<(), String>
    {
        let output = check_output_with_nonexistent_root();
        let result = render_check_with_config(
            &output,
            &OutputFormat::RepoBadgeShields,
            &RiprConfig::default(),
        );

        let err = expect_err(result)?;
        assert!(!err.is_empty());
        Ok(())
    }

    #[test]
    fn render_dispatch_repo_seams_json_surfaces_missing_workspace_as_error() -> Result<(), String> {
        let output = check_output_with_nonexistent_root();
        let result = render_check_with_config(
            &output,
            &OutputFormat::RepoSeamsJson,
            &RiprConfig::default(),
        );

        let err = expect_err(result)?;
        assert!(!err.is_empty());
        Ok(())
    }

    #[test]
    fn render_dispatch_repo_seams_md_surfaces_missing_workspace_as_error() -> Result<(), String> {
        let output = check_output_with_nonexistent_root();
        let result =
            render_check_with_config(&output, &OutputFormat::RepoSeamsMd, &RiprConfig::default());

        let err = expect_err(result)?;
        assert!(!err.is_empty());
        Ok(())
    }

    #[test]
    fn render_dispatch_repo_exposure_json_surfaces_missing_workspace_as_error() -> Result<(), String>
    {
        let output = check_output_with_nonexistent_root();
        let result = render_check_with_config(
            &output,
            &OutputFormat::RepoExposureJson,
            &RiprConfig::default(),
        );

        let err = expect_err(result)?;
        assert!(!err.is_empty());
        Ok(())
    }

    #[test]
    fn render_dispatch_repo_exposure_summary_json_surfaces_missing_workspace_as_error()
    -> Result<(), String> {
        let output = check_output_with_nonexistent_root();
        let result = render_check_with_config(
            &output,
            &OutputFormat::RepoExposureSummaryJson,
            &RiprConfig::default(),
        );

        let err = expect_err(result)?;
        assert!(!err.is_empty());
        Ok(())
    }

    #[test]
    fn render_dispatch_repo_exposure_md_surfaces_missing_workspace_as_error() -> Result<(), String>
    {
        let output = check_output_with_nonexistent_root();
        let result = render_check_with_config(
            &output,
            &OutputFormat::RepoExposureMd,
            &RiprConfig::default(),
        );

        let err = expect_err(result)?;
        assert!(!err.is_empty());
        Ok(())
    }

    #[test]
    fn render_dispatch_repo_sarif_surfaces_missing_workspace_as_error() -> Result<(), String> {
        let output = check_output_with_nonexistent_root();
        let result =
            render_check_with_config(&output, &OutputFormat::RepoSarif, &RiprConfig::default());

        let err = expect_err(result)?;
        assert!(!err.is_empty());
        Ok(())
    }

    #[test]
    fn render_dispatch_agent_seam_packets_surfaces_missing_workspace_as_error() -> Result<(), String>
    {
        let output = check_output_with_nonexistent_root();
        let result = render_check_with_config(
            &output,
            &OutputFormat::AgentSeamPacketsJson,
            &RiprConfig::default(),
        );

        let err = expect_err(result)?;
        assert!(!err.is_empty());
        Ok(())
    }

    #[test]
    fn render_dispatch_badge_plus_shields_missing_report_is_neutral() -> Result<(), String> {
        let root = temp_root("ripr-render-badge-plus-shields-missing")?;
        let mut output = check_output_with(Vec::new());
        output.root = root;

        let rendered = render_check_with_config(
            &output,
            &OutputFormat::BadgePlusShields,
            &RiprConfig::default(),
        )?;

        assert!(rendered.contains(r#""schemaVersion": 1"#));
        assert!(rendered.contains(r#""label": "ripr+""#));
        assert!(rendered.contains(r#""message": "needs test-efficiency""#));
        assert!(rendered.contains(r#""color": "lightgrey""#));
        assert!(!rendered.contains("cargo xtask test-efficiency-report"));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_repo_badge_plus_json_missing_report_is_neutral() -> Result<(), String> {
        let root = temp_root("ripr-render-repo-badge-plus-shields-missing")?;
        let mut output = check_output_with(Vec::new());
        output.root = root;

        let rendered = render_check_with_config(
            &output,
            &OutputFormat::RepoBadgePlusJson,
            &RiprConfig::default(),
        )?;

        assert!(rendered.contains(r#""kind": "ripr_plus""#));
        assert!(rendered.contains(r#""scope": "repo""#));
        assert!(rendered.contains(r#""basis": "canonical_actionable_gap""#));
        assert!(rendered.contains(r#""message": "needs test-efficiency""#));
        assert!(rendered.contains(r#""color": "lightgrey""#));
        assert!(rendered.contains("test-efficiency.json"));
        assert!(rendered.contains("docs/BADGE_ADOPTION.md"));
        assert!(!rendered.contains("cargo xtask test-efficiency-report"));

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_badge_plus_surfaces_invalid_test_efficiency_json_as_error()
    -> Result<(), String> {
        let root = temp_root("ripr-render-badge-plus-invalid-json")?;
        let report_dir = root.join("target/ripr/reports");
        std::fs::create_dir_all(&report_dir)
            .map_err(|err| format!("create test-efficiency report dir: {err}"))?;
        std::fs::write(report_dir.join("test-efficiency.json"), "this is not json")
            .map_err(|err| format!("write malformed test-efficiency: {err}"))?;
        let mut output = check_output_with(Vec::new());
        output.root = root;

        let result = render_check_with_config(
            &output,
            &OutputFormat::BadgePlusJson,
            &RiprConfig::default(),
        );

        let err = expect_err(result)?;
        assert!(
            err.contains("test-efficiency.json is not valid JSON"),
            "expected JSON parse failure, got: {err}"
        );

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn render_dispatch_badge_plus_surfaces_malformed_suppressions_as_error() -> Result<(), String> {
        let root = temp_root("ripr-render-badge-plus-bad-suppressions")?;
        // Valid efficiency report so parse succeeds and execution reaches the
        // suppressions load.
        write_test_efficiency_report(&root)?;
        write_malformed_suppressions(&root)?;
        let mut output = check_output_with(Vec::new());
        output.root = root;

        let result = render_check_with_config(
            &output,
            &OutputFormat::BadgePlusJson,
            &RiprConfig::default(),
        );

        let err = expect_err(result)?;
        assert!(
            err.contains("validation failed"),
            "expected suppressions validation failure, got: {err}"
        );

        remove_temp_root(&output.root)?;
        Ok(())
    }

    #[test]
    fn remove_temp_root_treats_missing_dir_as_success() -> Result<(), String> {
        let path = std::env::temp_dir().join(format!(
            "ripr-render-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        // Path does not exist; helper should return Ok without error.
        remove_temp_root(&path)?;
        Ok(())
    }

    #[test]
    fn remove_temp_root_returns_error_for_other_io_failures() -> Result<(), String> {
        // Passing a regular file (not a directory) makes `remove_dir_all`
        // fail with an error kind other than NotFound, exercising the
        // catch-all `Err(err) => Err(...)` arm.
        let root = temp_root("ripr-render-remove-error")?;
        let file_path = root.join("not-a-dir");
        std::fs::write(&file_path, b"").map_err(|err| format!("write sentinel file: {err}"))?;

        let result = remove_temp_root(&file_path);
        match result {
            Ok(()) => {
                // Some platforms allow `remove_dir_all` on a regular file
                // (it just unlinks it). In that case we cannot exercise
                // the error arm here, so accept the success path. Clean up
                // and return.
                remove_temp_root(&root)?;
                Ok(())
            }
            Err(message) => {
                assert!(message.contains("remove temp root"));
                let _ = std::fs::remove_file(&file_path);
                remove_temp_root(&root)?;
                Ok(())
            }
        }
    }

    fn check_output_with_nonexistent_root() -> CheckOutput {
        let mut output = check_output_with(Vec::new());
        output.root = PathBuf::from("/this/path/does/not/exist/ripr-render-test");
        output
    }

    fn check_output_with_temp_malformed_suppressions() -> Result<CheckOutput, String> {
        let root = temp_root("ripr-render-bad-suppressions")?;
        write_malformed_suppressions(&root)?;
        let mut output = check_output_with(Vec::new());
        output.root = root;
        Ok(output)
    }

    fn write_malformed_suppressions(root: &Path) -> Result<(), String> {
        let dir = root.join(".ripr");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create .ripr dir: {err}"))?;
        // Missing `schema_version = 1` and an unsupported top-level field
        // both produce validation violations from `parse_suppressions_manifest`.
        std::fs::write(
            dir.join("suppressions.toml"),
            "unsupported_field = \"value\"\n",
        )
        .map_err(|err| format!("write malformed suppressions: {err}"))?;
        Ok(())
    }

    fn expect_err<T: std::fmt::Debug>(result: Result<T, String>) -> Result<String, String> {
        match result {
            Ok(value) => Err(format!("expected error, got Ok({value:?})")),
            Err(err) => Ok(err),
        }
    }

    fn check_output_with(findings: Vec<Finding>) -> CheckOutput {
        CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("."),
            base: Some("origin/main".to_string()),
            summary: Summary::default(),
            findings,
        }
    }

    fn check_output_with_temp_report_workspace(
        findings: Vec<Finding>,
    ) -> Result<CheckOutput, String> {
        let root = temp_root("ripr-render-report")?;
        write_test_efficiency_report(&root)?;
        let mut output = check_output_with(findings);
        output.root = root;
        Ok(output)
    }

    fn check_output_with_temp_seam_workspace(
        findings: Vec<Finding>,
    ) -> Result<CheckOutput, String> {
        let root = temp_root("ripr-render-seams")?;
        std::fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src dir: {err}"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname=\"ripr-render-seams\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
        )
        .map_err(|err| format!("write temp Cargo.toml: {err}"))?;
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn over_threshold(amount: i32, threshold: i32) -> bool {\n    amount >= threshold\n}\n",
        )
        .map_err(|err| format!("write temp src/lib.rs: {err}"))?;

        let mut output = check_output_with(findings);
        output.root = root;
        Ok(output)
    }

    fn temp_root(prefix: &str) -> Result<PathBuf, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("{prefix}-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&root).map_err(|err| format!("create temp root: {err}"))?;
        Ok(root)
    }

    fn write_test_efficiency_report(root: &Path) -> Result<(), String> {
        let report_dir = root.join("target/ripr/reports");
        std::fs::create_dir_all(&report_dir)
            .map_err(|err| format!("create test-efficiency report dir: {err}"))?;
        std::fs::write(
            report_dir.join("test-efficiency.json"),
            r#"{
  "schema_version": "0.1",
  "tests": [
    {
      "class": "smoke_only",
      "name": "sample_test",
      "reached_owners": ["sample_owner"]
    }
  ],
  "metrics": {
    "tests_scanned": 1,
    "reason_counts": {
      "smoke_oracle_only": 1
    }
  }
}"#,
        )
        .map_err(|err| format!("write test-efficiency report: {err}"))?;
        Ok(())
    }

    fn remove_temp_root(root: &Path) -> Result<(), String> {
        match std::fs::remove_dir_all(root) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(format!("remove temp root: {err}")),
        }
    }

    fn sample_finding(file: &str, line: usize) -> Finding {
        Finding {
            id: "probe:src_lib_rs:42:error_path".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:src_lib_rs:42:error_path".to_string()),
                family: ProbeFamily::ErrorPath,
                location: SourceLocation::new(file, line, 1),
                owner: Some(SymbolId("sample_owner".to_string())),
                delta: DeltaKind::Control,
                before: None,
                after: None,
                expression: "sample_expr".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class: ExposureClass::WeaklyExposed,
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
            evidence: vec!["changed test".to_string()],
            missing: vec!["strong oracle".to_string()],
            flow_sinks: Vec::new(),
            activation: ActivationEvidence::default(),
            stop_reasons: vec![StopReason::NoChangedRustLine],
            related_tests: vec![RelatedTest {
                name: "sample_test".to_string(),
                file: "tests/sample.rs".into(),
                line: 10,
                oracle: None,
                oracle_kind: OracleKind::Unknown,
                oracle_strength: OracleStrength::Weak,
            }],
            recommended_next_step: Some("add stronger assertion".to_string()),
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
        }
    }
}
