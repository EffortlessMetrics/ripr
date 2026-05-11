use super::{
    agent_seam_packets, badge, format::OutputFormat, github, human, json, repo_exposure,
    repo_seams, sarif, suppressions,
};
use crate::analysis;
use crate::app::CheckOutput;
use crate::config::RiprConfig;

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
            let summary = ripr_repo_seam_summary(output, config)?;
            Ok(badge::render_native_json(&summary))
        }
        OutputFormat::BadgeShields => {
            let summary = ripr_summary_with_suppressions(output, config)?;
            Ok(badge::render_shields_json(&summary))
        }
        OutputFormat::RepoBadgeShields => {
            let summary = ripr_repo_seam_summary(output, config)?;
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

fn ripr_repo_seam_summary(
    output: &CheckOutput,
    config: &RiprConfig,
) -> Result<badge::BadgeSummary, String> {
    let class_counts =
        analysis::inventory_seam_grip_class_counts_at_with_config(&output.root, config)?;
    let policy = badge::BadgePolicy {
        suppressions_path: config.suppressions().display_path(),
        ..badge::BadgePolicy::default()
    };
    Ok(badge::ripr_seam_badge_summary_from_counts(
        &class_counts,
        config,
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
        return Err(format!(
            "missing {}; run `cargo xtask test-efficiency-report` before requesting badge-plus formats",
            report_path.display()
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
        let class_counts =
            analysis::inventory_seam_grip_class_counts_at_with_config(&output.root, config)?;
        Ok(
            badge::ripr_plus_seam_badge_summary_from_counts_with_suppressions(
                &class_counts,
                config,
                test_efficiency,
                &suppressions,
                &today,
                policy,
                scope,
            ),
        )
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
        assert!(native.contains("\"basis\": \"seam_native\""));
        assert!(shields.contains("\"schemaVersion\": 1"));
        assert!(!shields.contains("\"scope\""));

        remove_temp_root(&output.root)?;
        Ok(())
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
        }
    }
}
