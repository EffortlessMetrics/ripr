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

pub(crate) const CHECK_OUTPUT_SCHEMA_VERSION: &str = "0.1";

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
mod tests;
