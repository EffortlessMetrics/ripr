use crate::analysis::{AnalysisMode, AnalysisOptions, run_analysis};
use crate::domain::{Finding, Summary};
use crate::output;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct CheckInput {
    pub root: PathBuf,
    pub base: Option<String>,
    pub diff_file: Option<PathBuf>,
    pub mode: Mode,
    pub format: OutputFormat,
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
    Instant,
    Draft,
    Fast,
    Deep,
    Ready,
}

impl Mode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Instant => "instant",
            Mode::Draft => "draft",
            Mode::Fast => "fast",
            Mode::Deep => "deep",
            Mode::Ready => "ready",
        }
    }

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
    Human,
    Json,
    Github,
}

#[derive(Clone, Debug)]
pub struct CheckOutput {
    pub schema_version: String,
    pub tool: String,
    pub mode: Mode,
    pub root: PathBuf,
    pub base: Option<String>,
    pub summary: Summary,
    pub findings: Vec<Finding>,
}

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

pub fn render_check(output: &CheckOutput, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Human => output::human::render(output),
        OutputFormat::Json => output::json::render(output),
        OutputFormat::Github => output::github::render(output),
    }
}

pub fn explain_finding(root: &Path, selector: &str) -> Result<String, String> {
    explain_finding_with_input(
        CheckInput {
            root: root.to_path_buf(),
            ..CheckInput::default()
        },
        selector,
    )
}

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
