pub(crate) mod agent_brief;
pub(crate) mod agent_review_summary;
pub(crate) mod agent_status;
pub(crate) mod agent_workflow;
mod check;
mod context;
mod explain;
mod selector;

pub use crate::output::format::OutputFormat;
pub use check::{check_workspace, check_workspace_repo, repo_seam_inventory_input};
pub(crate) use check::{check_workspace_repo_with_config, check_workspace_with_config};
pub(crate) use context::collect_context_with_config;
pub use context::{collect_context, collect_context_with_input};
pub(crate) use explain::explain_finding_with_config;
pub use explain::{explain_finding, explain_finding_with_input};

use crate::analysis::AnalysisMode;
use crate::config::RiprConfig;
use crate::domain::{Finding, Summary};
use crate::output;
use std::path::PathBuf;

/// Input contract for [`check_workspace`].
///
/// This structure mirrors the user-facing CLI switches but is exposed for
/// library consumers that embed `ripr` checks in their own tooling.
#[derive(Clone, Debug)]
pub struct CheckInput {
    /// Workspace root used for discovery and analysis.
    pub root: PathBuf,
    /// Git base revision used when collecting a diff automatically.
    pub base: Option<String>,
    /// Optional path to a unified diff file. When set, `base` is ignored.
    pub diff_file: Option<PathBuf>,
    /// Analysis effort profile.
    pub mode: Mode,
    /// Preferred renderer for programmatic wrappers.
    pub format: OutputFormat,
    /// Whether unchanged tests may still be used as static evidence.
    pub include_unchanged_tests: bool,
}

impl Default for CheckInput {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            base: Some("origin/main".to_string()),
            diff_file: None,
            mode: Mode::Draft,
            format: OutputFormat::Human,
            include_unchanged_tests: true,
        }
    }
}

/// Public analysis effort profile used by both CLI flags and library
/// integrations.
///
/// Modes tune static evidence collection cost versus depth, while keeping
/// result language in terms of exposure estimates (`exposed`,
/// `weakly_exposed`, unknown classes) rather than runtime mutation outcomes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Minimal-latency local feedback.
    Instant,
    /// Default developer draft mode.
    Draft,
    /// Faster-than-deep with broader evidence than draft.
    Fast,
    /// Higher-effort local review mode.
    Deep,
    /// Review-ready mode used before sharing results.
    Ready,
}

impl Mode {
    /// Returns the stable CLI/programmatic label for this mode.
    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Instant => "instant",
            Mode::Draft => "draft",
            Mode::Fast => "fast",
            Mode::Deep => "deep",
            Mode::Ready => "ready",
        }
    }

    /// Maps a public mode to the internal analysis profile.
    pub fn analysis_mode(&self) -> AnalysisMode {
        match self {
            Mode::Instant => AnalysisMode::Instant,
            Mode::Draft => AnalysisMode::Draft,
            Mode::Fast => AnalysisMode::Fast,
            Mode::Deep => AnalysisMode::Deep,
            Mode::Ready => AnalysisMode::Ready,
        }
    }
}

/// Result payload produced by [`check_workspace`].
#[derive(Clone, Debug)]
pub struct CheckOutput {
    /// Output schema version for machine consumers.
    pub schema_version: String,
    /// Tool identifier.
    pub tool: String,
    /// Mode used for this analysis.
    pub mode: Mode,
    /// Analyzed workspace root.
    pub root: PathBuf,
    /// Base revision used to build the diff when applicable.
    pub base: Option<String>,
    /// Summary counts and high-level evidence status.
    pub summary: Summary,
    /// Probe-level findings.
    pub findings: Vec<Finding>,
}

/// Renders a previously computed [`CheckOutput`] in the requested format.
///
/// Returns `Err` when the requested format requires auxiliary inputs that
/// are not present — currently only the `BadgePlus*` formats, which read
/// the test-efficiency report. The other formats are infallible and
/// always return `Ok`.
pub fn render_check(output: &CheckOutput, format: &OutputFormat) -> Result<String, String> {
    render_check_with_config(output, format, &RiprConfig::default())
}

pub(crate) fn render_check_with_config(
    output: &CheckOutput,
    format: &OutputFormat,
    config: &RiprConfig,
) -> Result<String, String> {
    output::render::render_check_with_config(output, format, config)
}

#[cfg(test)]
mod tests {
    use super::{
        CheckOutput, Mode, OutputFormat, render_check, render_check_with_config,
        selector::selector_matches_location,
    };
    use crate::analysis::AnalysisMode;
    use crate::domain::{
        ActivationEvidence, Confidence, ExposureClass, Finding, OracleStrength, Probe, ProbeFamily,
        ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence,
        StageState, StopReason, Summary,
    };
    use std::path::PathBuf;

    #[test]
    fn mode_labels_match_public_contract() {
        assert_eq!(Mode::Instant.as_str(), "instant");
        assert_eq!(Mode::Draft.as_str(), "draft");
        assert_eq!(Mode::Fast.as_str(), "fast");
        assert_eq!(Mode::Deep.as_str(), "deep");
        assert_eq!(Mode::Ready.as_str(), "ready");
    }

    #[test]
    fn mode_maps_to_internal_profiles() {
        assert_eq!(Mode::Instant.analysis_mode(), AnalysisMode::Instant);
        assert_eq!(Mode::Draft.analysis_mode(), AnalysisMode::Draft);
        assert_eq!(Mode::Fast.analysis_mode(), AnalysisMode::Fast);
        assert_eq!(Mode::Deep.analysis_mode(), AnalysisMode::Deep);
        assert_eq!(Mode::Ready.analysis_mode(), AnalysisMode::Ready);
    }

    #[test]
    fn selector_matches_exact_and_suffix_file_locations() {
        let finding = sample_finding("src/lib.rs", 42);

        assert!(selector_matches_location("src/lib.rs:42", &finding));
        assert!(selector_matches_location(
            "crates/ripr/src/lib.rs:42",
            &finding
        ));
        assert!(!selector_matches_location("src/lib.rs:41", &finding));
        assert!(!selector_matches_location("src/main.rs:42", &finding));
    }

    #[test]
    fn selector_matches_absolute_finding_location_exactly() {
        let finding = sample_finding("/workspace/ripr/src/lib.rs", 42);

        assert!(selector_matches_location(
            "/workspace/ripr/src/lib.rs:42",
            &finding
        ));
        assert!(!selector_matches_location("src/lib.rs:42", &finding));
    }

    fn sample_finding(file: &str, line: usize) -> Finding {
        Finding {
            id: "probe:src_lib_rs:42:error_path".to_string(),
            probe: Probe {
                id: ProbeId("probe:src_lib_rs:42:error_path".to_string()),
                family: ProbeFamily::ErrorPath,
                location: SourceLocation::new(file, line, 1),
                owner: None,
                delta: crate::domain::DeltaKind::Control,
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
                oracle_kind: crate::domain::OracleKind::Unknown,
                oracle_strength: OracleStrength::Weak,
            }],
            recommended_next_step: Some("add stronger assertion".to_string()),
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
        }
    }

    #[test]
    fn summary_default_is_empty() {
        let summary = Summary::default();
        assert_eq!(summary.findings, 0);
        assert_eq!(summary.exposed, 0);
        assert_eq!(summary.weakly_exposed, 0);
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

    fn check_output_with_temp_seam_workspace(
        findings: Vec<Finding>,
    ) -> Result<CheckOutput, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("ripr-app-repo-badge-{stamp}"));
        std::fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp src dir: {err}"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname=\"ripr-app-repo-badge\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
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

    #[test]
    fn configured_finding_severity_applies_to_human_json_and_github() -> Result<(), String> {
        let output = check_output_with(vec![sample_finding("src/lib.rs", 1)]);
        let config =
            crate::config::tests_only_parse("[severity.findings]\nweakly_exposed = \"info\"\n")?;

        let human = render_check_with_config(&output, &OutputFormat::Human, &config)?;
        let json = render_check_with_config(&output, &OutputFormat::Json, &config)?;
        let github = render_check_with_config(&output, &OutputFormat::Github, &config)?;

        if !human.contains("INFO src/lib.rs:1") {
            return Err(format!("human severity was not configured: {human}"));
        }
        if !json.contains("\"severity\": \"info\"") {
            return Err(format!("json severity was not configured: {json}"));
        }
        if !github.starts_with("::notice ") {
            return Err(format!("github severity was not configured: {github}"));
        }
        Ok(())
    }

    #[test]
    fn render_check_dispatches_badge_json_format() -> Result<(), String> {
        let output = check_output_with(vec![sample_finding("src/lib.rs", 1)]);
        let rendered = render_check(&output, &OutputFormat::BadgeJson)?;

        // Native snake_case wire shape with all required top-level keys.
        assert!(rendered.contains("\"schema_version\": \"0.4\""));
        assert!(rendered.contains("\"kind\": \"ripr\""));
        assert!(rendered.contains("\"scope\": \"diff\""));
        assert!(rendered.contains("\"basis\": \"finding_exposure\""));
        assert!(rendered.contains("\"counts\":"));
        assert!(rendered.contains("\"reason_counts\":"));
        assert!(rendered.contains("\"policy\":"));
        // Specifically includes the new vocabulary from #187/#188 with zero default.
        assert!(rendered.contains("\"duplicate_activation_and_oracle_shape\": 0"));
        Ok(())
    }

    #[test]
    fn render_check_dispatches_badge_shields_format() -> Result<(), String> {
        let output = check_output_with(vec![sample_finding("src/lib.rs", 1)]);
        let rendered = render_check(&output, &OutputFormat::BadgeShields)?;

        assert!(rendered.contains("\"schemaVersion\": 1"));
        assert!(rendered.contains("\"label\":"));
        assert!(rendered.contains("\"message\":"));
        assert!(rendered.contains("\"color\":"));
        // Native-only fields must not leak into the Shields shape.
        for forbidden in [
            "\"counts\"",
            "\"reason_counts\"",
            "\"policy\"",
            "\"kind\"",
            "\"status\"",
            "\"scope\"",
            "\"basis\"",
        ] {
            assert!(
                !rendered.contains(forbidden),
                "Shields projection must not contain `{forbidden}`"
            );
        }
        Ok(())
    }

    #[test]
    fn badge_render_message_has_no_denominator_or_coverage_framing() -> Result<(), String> {
        let output = check_output_with(vec![
            sample_finding("src/a.rs", 1),
            sample_finding("src/b.rs", 2),
        ]);
        for format in [OutputFormat::BadgeJson, OutputFormat::BadgeShields] {
            let rendered = render_check(&output, &format)?;
            let lower = rendered.to_ascii_lowercase();
            // Confirm no "X/Y" denominator pattern in the message field; the
            // message itself is just a count string.
            assert!(
                !rendered.contains("\"message\": \"") || {
                    let after = rendered.split("\"message\": \"").nth(1).unwrap_or("");
                    let value_end = after.find('"').unwrap_or(after.len());
                    let value = &after[..value_end];
                    !value.contains('/')
                },
                "badge message must not contain a denominator: {rendered}"
            );
            assert!(!lower.contains("coverage"));
            assert!(!lower.contains("uncovered"));
        }
        Ok(())
    }

    #[test]
    fn render_check_repo_badge_json_paints_scope_repo() -> Result<(), String> {
        let output = check_output_with_temp_seam_workspace(vec![sample_finding("src/lib.rs", 1)])?;
        let rendered = render_check(&output, &OutputFormat::RepoBadgeJson)?;

        assert!(rendered.contains("\"schema_version\": \"0.4\""));
        assert!(rendered.contains("\"scope\": \"repo\""));
        assert!(rendered.contains("\"basis\": \"seam_native\""));
        assert!(!rendered.contains("\"scope\": \"diff\""));
        assert!(rendered.contains("\"kind\": \"ripr\""));
        assert!(rendered.contains("\"analyzed_findings\": 0"));
        assert!(!rendered.contains("\"analyzed_seams\": 0"));
        let _ = std::fs::remove_dir_all(&output.root);
        Ok(())
    }

    #[test]
    fn render_check_repo_badge_shields_stays_four_fields_without_scope_leak() -> Result<(), String>
    {
        let output = check_output_with_temp_seam_workspace(vec![sample_finding("src/lib.rs", 1)])?;
        let rendered = render_check(&output, &OutputFormat::RepoBadgeShields)?;

        // Scope is native-only metadata; Shields stays a four-field projection.
        assert!(!rendered.contains("\"scope\""));
        assert!(!rendered.contains("\"basis\""));
        let top_level_keys = rendered
            .lines()
            .filter(|line| line.starts_with("  \""))
            .count();
        assert_eq!(top_level_keys, 4);
        for forbidden in [
            "\"counts\"",
            "\"reason_counts\"",
            "\"policy\"",
            "\"kind\"",
            "\"status\"",
            "\"schema_version\"",
        ] {
            assert!(
                !rendered.contains(forbidden),
                "Shields projection must not contain `{forbidden}`"
            );
        }
        let _ = std::fs::remove_dir_all(&output.root);
        Ok(())
    }

    #[test]
    fn render_check_badge_plus_fails_when_test_efficiency_report_missing() -> Result<(), String> {
        // CheckOutput.root points at a temporary directory that does NOT
        // contain target/ripr/reports/test-efficiency.json.
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp = std::env::temp_dir().join(format!("ripr-badge-plus-missing-{stamp}"));
        std::fs::create_dir_all(&tmp).map_err(|e| format!("create temp dir: {e}"))?;

        let mut output = check_output_with(vec![sample_finding("src/lib.rs", 1)]);
        output.root = tmp.clone();

        let result = render_check(&output, &OutputFormat::BadgePlusJson);
        assert!(result.is_err(), "badge-plus must fail when report missing");
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains("test-efficiency.json"),
            "error must name the missing report: {err}"
        );
        assert!(
            err.contains("cargo xtask test-efficiency-report"),
            "error must direct the user to the regenerator command: {err}"
        );

        let _ = std::fs::remove_dir_all(&tmp);
        Ok(())
    }
}
