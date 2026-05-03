use crate::analysis::{AnalysisMode, AnalysisOptions, run_analysis};
use crate::domain::{Finding, Summary};
use crate::output;
use std::path::{Path, PathBuf};

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable plain text report.
    Human,
    /// Versioned JSON report for automation.
    Json,
    /// GitHub annotation output suitable for CI logs.
    Github,
    /// Native `ripr` badge JSON (snake_case wire shape with full counts,
    /// reason counts, and policy). Consumed by tools and CI artifacts.
    BadgeJson,
    /// Shields-compatible projection for the `ripr` badge: exactly four
    /// top-level fields (`schemaVersion`, `label`, `message`, `color`).
    BadgeShields,
    /// Native `ripr+` badge JSON. Sums unsuppressed exposure gaps and
    /// unsuppressed actionable test-efficiency findings, excluding
    /// declared intent. Requires `target/ripr/reports/test-efficiency.json`
    /// produced by `cargo xtask test-efficiency-report`.
    BadgePlusJson,
    /// Shields-compatible projection for the `ripr+` badge.
    BadgePlusShields,
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

/// Runs the end-to-end static exposure analysis for a workspace.
pub fn check_workspace(input: CheckInput) -> Result<CheckOutput, String> {
    let options = AnalysisOptions {
        root: input.root.clone(),
        base: input.base.clone(),
        diff_file: input.diff_file.clone(),
        mode: input.mode.analysis_mode(),
        include_unchanged_tests: input.include_unchanged_tests,
    };
    let analysis = run_analysis(&options)?;
    Ok(CheckOutput {
        schema_version: "0.1".to_string(),
        tool: "ripr".to_string(),
        mode: input.mode,
        root: input.root,
        base: input.base,
        summary: analysis.summary,
        findings: analysis.findings,
    })
}

/// Path (relative to the analyzed workspace root) where the
/// test-efficiency report is expected when rendering `ripr+` badge formats.
pub(crate) const TEST_EFFICIENCY_REPORT_RELATIVE: &str = "target/ripr/reports/test-efficiency.json";

/// Renders a previously computed [`CheckOutput`] in the requested format.
///
/// Returns `Err` when the requested format requires auxiliary inputs that
/// are not present — currently only the `BadgePlus*` formats, which read
/// the test-efficiency report. The other formats are infallible and
/// always return `Ok`.
pub fn render_check(output: &CheckOutput, format: &OutputFormat) -> Result<String, String> {
    match format {
        OutputFormat::Human => Ok(output::human::render(output)),
        OutputFormat::Json => Ok(output::json::render(output)),
        OutputFormat::Github => Ok(output::github::render(output)),
        OutputFormat::BadgeJson => {
            let summary = ripr_summary_with_suppressions(output)?;
            Ok(output::badge::render_native_json(&summary))
        }
        OutputFormat::BadgeShields => {
            let summary = ripr_summary_with_suppressions(output)?;
            Ok(output::badge::render_shields_json(&summary))
        }
        OutputFormat::BadgePlusJson => {
            let summary = ripr_plus_summary_from_disk(output)?;
            Ok(output::badge::render_native_json(&summary))
        }
        OutputFormat::BadgePlusShields => {
            let summary = ripr_plus_summary_from_disk(output)?;
            Ok(output::badge::render_shields_json(&summary))
        }
    }
}

fn load_suppressions(
    output: &CheckOutput,
) -> Result<Vec<output::suppressions::SuppressionEntry>, String> {
    output::suppressions::load_suppressions_for_root(&output.root).map_err(|violations| {
        format!(
            ".ripr/suppressions.toml validation failed:\n{}",
            violations.join("\n")
        )
    })
}

fn ripr_summary_with_suppressions(
    output: &CheckOutput,
) -> Result<output::badge::BadgeSummary, String> {
    let suppressions = load_suppressions(output)?;
    let today = output::suppressions::current_iso_date();
    Ok(output::badge::ripr_badge_summary_with_suppressions(
        output,
        &suppressions,
        &today,
        output::badge::BadgePolicy::default(),
    ))
}

fn ripr_plus_summary_from_disk(
    output: &CheckOutput,
) -> Result<output::badge::BadgeSummary, String> {
    let report_path = output.root.join(TEST_EFFICIENCY_REPORT_RELATIVE);
    if !report_path.exists() {
        return Err(format!(
            "missing {}; run `cargo xtask test-efficiency-report` before requesting badge-plus formats",
            report_path.display()
        ));
    }
    let text = std::fs::read_to_string(&report_path)
        .map_err(|err| format!("failed to read {}: {err}", report_path.display()))?;
    let test_efficiency = output::badge::parse_test_efficiency_badge_summary(&text)?;
    let suppressions = load_suppressions(output)?;
    let today = output::suppressions::current_iso_date();
    Ok(output::badge::ripr_plus_badge_summary_with_suppressions(
        output,
        test_efficiency,
        &suppressions,
        &today,
        output::badge::BadgePolicy::default(),
    ))
}

/// Computes findings and renders a single selected finding in human format.
///
/// The selector can be either a finding identifier (for example
/// `probe:path_to_file.rs:42:family`) or a `file:line` location.
pub fn explain_finding(root: &Path, selector: &str) -> Result<String, String> {
    explain_finding_with_input(
        CheckInput {
            root: root.to_path_buf(),
            ..CheckInput::default()
        },
        selector,
    )
}

/// Like [`explain_finding`] but allows overriding the full check input.
pub fn explain_finding_with_input(input: CheckInput, selector: &str) -> Result<String, String> {
    let output = check_workspace(input)?;
    let selected = output
        .findings
        .iter()
        .find(|finding| finding.id == selector || selector_matches_location(selector, finding));

    match selected {
        Some(finding) => Ok(output::human::render_finding(finding)),
        None => Err(format!("no finding matched {selector:?}")),
    }
}

/// Produces a compact JSON context packet for one selected finding.
pub fn collect_context(
    root: &Path,
    selector: &str,
    max_related_tests: usize,
) -> Result<String, String> {
    collect_context_with_input(
        CheckInput {
            root: root.to_path_buf(),
            format: OutputFormat::Json,
            ..CheckInput::default()
        },
        selector,
        max_related_tests,
    )
}

/// Like [`collect_context`] but allows overriding the full check input.
pub fn collect_context_with_input(
    input: CheckInput,
    selector: &str,
    max_related_tests: usize,
) -> Result<String, String> {
    let input = CheckInput {
        format: OutputFormat::Json,
        ..input
    };
    let output = check_workspace(input)?;
    let selected = output
        .findings
        .iter()
        .find(|finding| finding.id == selector || selector_matches_location(selector, finding));

    match selected {
        Some(finding) => Ok(output::json::render_context_packet(
            finding,
            max_related_tests,
        )),
        None => Err(format!("no finding matched {selector:?}")),
    }
}

fn selector_matches_location(selector: &str, finding: &Finding) -> bool {
    let file = finding.probe.location.file.to_string_lossy();
    let line = finding.probe.location.line;
    selector == format!("{file}:{line}")
        || selector.ends_with(&format!(":{line}")) && selector.contains(file.as_ref())
}

#[cfg(test)]
mod tests {
    use super::{CheckOutput, Mode, OutputFormat, render_check, selector_matches_location};
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

    #[test]
    fn render_check_dispatches_badge_json_format() -> Result<(), String> {
        let output = check_output_with(vec![sample_finding("src/lib.rs", 1)]);
        let rendered = render_check(&output, &OutputFormat::BadgeJson)?;

        // Native snake_case wire shape with all required top-level keys.
        assert!(rendered.contains("\"schema_version\": \"0.2\""));
        assert!(rendered.contains("\"kind\": \"ripr\""));
        assert!(rendered.contains("\"scope\": \"diff\""));
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
