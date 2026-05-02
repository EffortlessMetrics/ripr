use crate::analysis::{AnalysisMode, AnalysisOptions, run_analysis};
use crate::domain::{Finding, Summary};
use crate::output;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
/// Input options for a `ripr` workspace check.
pub struct CheckInput {
    /// Repository root or workspace root to analyze.
    pub root: PathBuf,
    /// Base git reference used when `diff_file` is not provided.
    pub base: Option<String>,
    /// Optional path to a unified diff file.
    pub diff_file: Option<PathBuf>,
    /// Analysis depth and speed profile.
    pub mode: Mode,
    /// Output renderer to use for the final report.
    pub format: OutputFormat,
    /// Whether unchanged tests may be considered as supporting evidence.
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
/// User-facing analysis profile.
pub enum Mode {
    /// Prioritize immediate feedback over deeper evidence.
    Instant,
    /// Default draft-mode pass for static exposure evidence.
    Draft,
    /// Faster analysis profile with moderate evidence depth.
    Fast,
    /// Deeper static analysis intended for stronger review confidence.
    Deep,
    /// Most complete local static pass before review handoff.
    Ready,
}

impl Mode {
    /// Returns the canonical lowercase CLI/API spelling of this mode.
    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Instant => "instant",
            Mode::Draft => "draft",
            Mode::Fast => "fast",
            Mode::Deep => "deep",
            Mode::Ready => "ready",
        }
    }

    /// Maps the user-facing mode to the internal analysis mode.
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
/// Output renderer selection for `check` results.
pub enum OutputFormat {
    /// Human-readable text report.
    Human,
    /// Stable JSON output contract.
    Json,
    /// GitHub annotation-style text.
    Github,
}

#[derive(Clone, Debug)]
/// Full result packet produced by [`check_workspace`].
pub struct CheckOutput {
    /// Output schema version for machine consumers.
    pub schema_version: String,
    /// Tool identifier (currently `ripr`).
    pub tool: String,
    /// Effective mode used for analysis.
    pub mode: Mode,
    /// Workspace root that was analyzed.
    pub root: PathBuf,
    /// Base git reference used when diffing against VCS.
    pub base: Option<String>,
    /// Aggregated finding counts and class totals.
    pub summary: Summary,
    /// Per-probe findings with RIPR evidence.
    pub findings: Vec<Finding>,
}

/// Runs static analysis for the provided workspace input and returns findings.
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

/// Renders a [`CheckOutput`] using the requested [`OutputFormat`].
pub fn render_check(output: &CheckOutput, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Human => output::human::render(output),
        OutputFormat::Json => output::json::render(output),
        OutputFormat::Github => output::github::render(output),
    }
}

/// Explains a single finding selected by id or `path:line` selector.
pub fn explain_finding(root: &Path, selector: &str) -> Result<String, String> {
    explain_finding_with_input(
        CheckInput {
            root: root.to_path_buf(),
            ..CheckInput::default()
        },
        selector,
    )
}

/// Explains a single finding using fully customized [`CheckInput`].
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

/// Builds a JSON context packet for one finding from workspace defaults.
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

/// Builds a JSON context packet for one finding using custom input options.
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
