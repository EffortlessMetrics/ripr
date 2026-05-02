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

/// Renders a previously computed [`CheckOutput`] in the requested format.
pub fn render_check(output: &CheckOutput, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Human => output::human::render(output),
        OutputFormat::Json => output::json::render(output),
        OutputFormat::Github => output::github::render(output),
    }
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
    use super::{Mode, selector_matches_location};
    use crate::analysis::AnalysisMode;
    use crate::domain::{
        Confidence, ExposureClass, Finding, OracleStrength, Probe, ProbeFamily, ProbeId,
        RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState,
        StopReason, Summary,
    };

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
            stop_reasons: vec![StopReason::NoChangedRustLine],
            related_tests: vec![RelatedTest {
                name: "sample_test".to_string(),
                file: "tests/sample.rs".into(),
                line: 10,
                oracle: None,
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
}
