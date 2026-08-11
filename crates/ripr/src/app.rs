pub(crate) mod agent_brief;
pub(crate) mod agent_gap_packet;
pub(crate) mod agent_receipt;
pub(crate) mod agent_review_summary;
pub(crate) mod agent_status;
pub(crate) mod agent_workflow;
pub(crate) mod analysis_outcome_artifact;
pub(crate) mod annotations;
pub(crate) mod causal_projection;
mod check;
pub(crate) mod check_artifact;
mod context;
mod explain;
pub(crate) mod impacted_evidence;
mod navigation;
pub(crate) mod pr_evidence;
/// Shared PR-evidence summary projection used by the `ripr` binary and the
/// compatibility `xtask` route.
pub mod pr_summary;
pub(crate) mod receipt;
pub(crate) mod ripr_plus;
mod selector;
pub(crate) mod temp_diff;
pub(crate) mod verification_execution;

pub use crate::output::format::OutputFormat;
pub use check::{check_workspace, check_workspace_repo, repo_seam_inventory_input};

/// The `ripr-perl-facts-v1` packet schema this ripr build consumes (Campaign 31
/// item 5). Canonical, always-compiled declaration; the lang-perl-gated perl
/// module (`app::check`, `analysis::language::perl`) and the doctor
/// (`cli::commands`) reference this single source of truth via
/// `crate::app::PERL_FACT_PACKET_SCHEMA`.
pub(crate) const PERL_FACT_PACKET_SCHEMA: &str = "ripr-perl-facts-v1";

pub(crate) use crate::analysis::repair_route::repair_route_readiness;
/// The versioned envelope consumed by the producer-owned agent verification
/// route and emitted by the agent seam packet renderer. Defined in the
/// central registry (`output::schemas`, #2973) and re-exported here.
pub(crate) use crate::output::schemas::AGENT_SEAM_PACKET_SCHEMA_VERSION;
pub(crate) use check::is_managed_perl_producer;
pub use check::{
    check_workspace_repo_with_config, check_workspace_with_config,
    check_workspace_worktree_with_config,
};
pub(crate) use context::collect_context_from_artifact;
pub use context::collect_context_with_config;
pub use context::{collect_context, collect_context_with_input};
#[cfg(test)]
pub(crate) use explain::explain_finding_from_artifact;
pub use explain::explain_finding_with_config;
pub use explain::{explain_finding, explain_finding_with_input};
pub(crate) use explain::{
    explain_finding_from_artifact_with_navigation_mode,
    explain_finding_with_config_and_navigation_mode,
};
pub(crate) use navigation::{FindingNavigation, finding_navigation};

use crate::analysis::{AnalysisMode, PreviewLanguageAdvisory};
use crate::config::RiprConfig;
use crate::domain::{Finding, Summary};
use crate::output;
use std::path::PathBuf;

pub(crate) const CHECK_OUTPUT_SCHEMA_VERSION: &str = "0.2";

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
    /// Path to a `ripr-perl-facts-v1` packet for the Perl adapter
    /// (Campaign 31, #1429). When `None`, the Perl adapter returns a named
    /// limitation (no analysis). When `Some`, the adapter reads the packet
    /// and produces Findings + limitations from it.
    pub perl_facts_path: Option<PathBuf>,
    /// Optional explicit suppression-policy file for this check run (#1441).
    /// Relative paths resolve against `root`. When `Some`, exposure-gap
    /// entries (by `finding_id` or `path` glob) mark matching findings as
    /// suppressed in the output; a missing or malformed file fails the run.
    /// When `None`, check output is unchanged.
    pub suppression_policy: Option<PathBuf>,
    /// Cooperative per-invocation git deadline for the diff-load path
    /// (#2303). When `None`, git invocations are unbounded. The CLI check
    /// adapter populates this from the `--git-timeout` flag or
    /// `RIPR_GIT_TIMEOUT` env var (default: 5 minutes); the LSP refresh path
    /// populates it from the `gitTimeoutMs` session option.
    pub git_timeout: Option<std::time::Duration>,
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
            perl_facts_path: None,
            suppression_policy: None,
            git_timeout: None,
        }
    }
}

/// Default cooperative git deadline for the CLI path (#2613).
///
/// 5 minutes — long enough for large repos, short enough to surface a hang
/// that would otherwise block indefinitely. Override with `--git-timeout`
/// or `RIPR_GIT_TIMEOUT` (seconds; `0` disables the deadline).
pub(crate) const DEFAULT_CLI_GIT_TIMEOUT_SECS: u64 = 300;

pub(crate) fn default_cli_git_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_CLI_GIT_TIMEOUT_SECS)
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
    /// Typed producer-owned completeness and limitation facts for diff and
    /// worktree analysis. `None` is reserved for repo-scope projections that
    /// have no diff denominator.
    pub(crate) analysis_outcome: Option<crate::analysis_outcome::AnalysisOutcome>,
    /// Summary counts and high-level evidence status.
    pub summary: Summary,
    /// Probe-level findings.
    pub findings: Vec<Finding>,
    /// Advisory records for preview-language files in the analyzed scope.
    ///
    /// Non-empty only when TypeScript, JavaScript, or Python files were
    /// present in the diff or repo. An empty result with non-empty advisories
    /// is NOT a clean Rust-grade result — it means the preview adapter
    /// analyzed the scope but found nothing actionable at this time.
    /// See RIPR-SPEC-0082.
    pub preview_language_advisories: Vec<PreviewLanguageAdvisory>,
    /// Per-language run-status records for languages that did NOT complete
    /// successfully. Empty when every enabled language ran to completion.
    /// Non-abort contract (Campaign 31 PR 10, #1403): a failure here does not
    /// abort the report.
    pub language_runs: Vec<crate::analysis::LanguageRun>,
    /// When `true`, no analysis scope was provided by the caller (no `--diff`,
    /// `--base`, `--files`, or full-repo mode flag). An empty result in this
    /// state does NOT mean the changed behavior is covered — it means nothing
    /// was analyzed. See RIPR-SPEC-0083.
    pub no_scope_provided: bool,
    /// When `true`, `--base` was used to analyze committed history AND the
    /// working tree has uncommitted changes to tracked source files that were
    /// NOT part of the analyzed diff. An empty result in this state does NOT
    /// mean the working-tree changes are covered — they were silently excluded
    /// from the analysis. See RIPR-SPEC-0112.
    pub unanalyzed_working_tree: bool,
    /// Suppression-policy application outcome (#1441). `Some` only when the
    /// caller passed `--suppression-policy`; findings named here stay in
    /// `findings` (visible, marked suppressed by renderers) while the
    /// per-class `summary` buckets count unsuppressed findings only.
    pub suppression: Option<crate::output::suppressions::CheckSuppressionOutcome>,
    /// Partial diff-scope run state (RIPR-PROP-0019, #1999). `Some` only when
    /// the diff exceeded the partial-selection budget and the run analyzed a
    /// deterministic bounded partition instead of the full diff
    /// (`limited_partial_scope`). In that state `summary`/`findings` cover the
    /// selected partition only, the uninspected accounting on the record is a
    /// lower bound, and the result is never a gate, baseline, badge, or RIPR
    /// Zero input (`gate_eligibility: ineligible`).
    pub partial_scope: Option<crate::analysis::PartialDiffScope>,
}

/// Renders a previously computed [`CheckOutput`] in the requested format.
///
/// Missing auxiliary inputs for badge rendering produce neutral badge output
/// when that is safe for badge generators. Malformed auxiliary inputs still
/// return `Err` so callers do not publish misleading measured badges.
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

pub(crate) fn render_check_with_config_and_navigation(
    output: &CheckOutput,
    format: &OutputFormat,
    config: &RiprConfig,
    navigation: Option<&FindingNavigation>,
) -> Result<String, String> {
    output::render::render_check_with_config_and_navigation(output, format, config, navigation)
}

#[cfg(test)]
mod tests;
