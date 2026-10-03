pub(crate) mod agent_brief;
pub(crate) mod agent_gap_packet;
pub(crate) mod agent_receipt;
pub(crate) mod agent_review_summary;
pub(crate) mod agent_status;
pub(crate) mod agent_workflow;
pub(crate) mod analysis_outcome_artifact;
pub(crate) mod analysis_subject;
pub(crate) mod annotations;
pub(crate) mod causal_projection;
mod check;
pub(crate) mod check_artifact;
mod context;
mod explain;
pub(crate) mod impacted_evidence;
mod navigation;
pub mod pr_evidence;
mod progress;
pub use pr_evidence::reject_pr_evidence_error_packet;
pub(crate) mod feedback;
/// Shared PR-evidence summary projection used by the `ripr` binary and the
/// compatibility `xtask` route.
pub mod pr_summary;
pub(crate) mod python_repair_binding;
pub(crate) mod python_repair_verification;
pub(crate) mod receipt;
pub(crate) mod repair_attempt;
pub(crate) mod repair_card;
/// Production handoff producer that assembles a `RepairCardV1` for one seam
/// entry (#4667: card-first bounded agent handoff; canonical packet stays
/// behind the explicit `ripr agent packet` route).
pub(crate) mod repair_card_handoff;
/// Measurement and ratification producer for the RepairCard default budget
/// (#4669; RIPR-SPEC-0196): synthetic wire-size measurement and governed
/// real-opportunity accounting that back the versioned decision receipt.
pub mod repair_card_usability;
pub(crate) mod ripr_plus;

/// Shared final qualification boundary for legacy RIPR+ receipt composition.
/// Exposure summaries and gap ledgers preserve useful observed counts, but
/// cannot establish complete test-quality evidence bound to the current candidate.
/// The compatibility xtask uses this same boundary before writing a receipt.
pub fn qualify_legacy_ripr_plus_receipt(
    receipt: serde_json::Value,
) -> Result<serde_json::Value, String> {
    ripr_plus::qualify_legacy_receipt(receipt)
}
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

/// The versioned envelope consumed by the producer-owned agent verification
/// route and emitted by the agent seam packet renderer.
///
/// `0.5` adds the seam packet's edit-cage fields (`allowed_edit_surface`,
/// `forbidden_files`, `must_not_change`, #4330): the packet now states the
/// cage its repair will enforce, derived from the same recommended target the
/// cage authority consumes, so the disclosure cannot drift from enforcement.
/// `0.5` also adds the optional envelope-level `repair_attempt` continuation
/// block carried by the `ripr agent repair --phase before --json` success
/// stdout (#4329); every other projection keeps the `0.4` shape and only the
/// version string moves.
pub(crate) const AGENT_SEAM_PACKET_SCHEMA_VERSION: &str = "0.5";
pub(crate) use crate::analysis::repair_route::repair_route_readiness;
pub(crate) use check::check_with_progress;
#[cfg(test)]
pub(crate) use check::check_workspace_repo_with_origins;
#[cfg(test)]
pub(crate) use check::check_workspace_worktree_with_origins;
pub(crate) use check::check_workspace_worktree_with_sources_open_rust_paths_and_progress;
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
pub(crate) use navigation::{FindingDrillIn, FindingNavigation, finding_navigation};
pub(crate) use progress::{
    AnalysisProgressEvent, AnalysisProgressScope, AnalysisProgressSink, AnalysisProgressStage,
    repo_inventory_with_progress,
};

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
    /// `None` resolves the repository's real default branch
    /// (RIPR-SPEC-0084); the library default is `None` so every consumer
    /// inherits resolution instead of a hardcoded branch.
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
    /// Immutable Git candidate subject (#3237 / #3276 R1). When `Some`, the
    /// run must consume base and candidate bytes from the named Git tree
    /// objects — never the worktree or live index. Mutually exclusive with
    /// `diff_file` and `base`. The object producer (#3277) executes the
    /// subject in diff mode; worktree and repo modes fail closed with a
    /// named error rather than falling back to worktree analysis.
    pub git_candidate: Option<crate::domain::GitCandidateSubject>,
}

impl Default for CheckInput {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            // #3952 / RIPR-SPEC-0084: no hardcoded branch. `None` sends
            // every consumer through default-branch resolution instead of
            // `origin/main`, which need not exist in the analyzed repo.
            base: None,
            diff_file: None,
            mode: Mode::Draft,
            format: OutputFormat::Human,
            include_unchanged_tests: true,
            perl_facts_path: None,
            suppression_policy: None,
            git_timeout: None,
            git_candidate: None,
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
    /// Test-harness registry projections (#3532): what each exact
    /// registration established for this run. Empty when the repository
    /// has no registrations, so every existing consumer output is
    /// unchanged.
    pub harness_projections: Vec<crate::analysis::harness_projection::TestHarnessProjection>,
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
    /// When `true`, the run analyzed committed history (an explicit `--base`
    /// or the resolved default base) while source or test files had
    /// uncommitted changes. The run read those files as committed at `HEAD`,
    /// so the changes were NOT analyzed, neither in the diff nor as test
    /// evidence. An empty result in this state does NOT mean they are
    /// covered. See RIPR-SPEC-0112.
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

/// Renders with navigation while reporting repo-scope progress boundaries to
/// `progress` for the full-repo audit-path formats (#4945).
pub(crate) fn render_check_with_config_and_navigation_and_progress(
    output: &CheckOutput,
    format: &OutputFormat,
    config: &RiprConfig,
    drill_in: Option<&FindingDrillIn>,
    progress: Option<&dyn AnalysisProgressSink>,
) -> Result<String, String> {
    output::render::render_check_with_config_and_navigation_and_progress(
        output, format, config, drill_in, progress,
    )
}

#[cfg(test)]
mod tests;
