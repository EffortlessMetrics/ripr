//! Reference adapter for Rust.
//!
//! See `docs/specs/RIPR-SPEC-0026-language-adapter-contract.md`.
//!
//! This adapter hosts the existing Rust analysis pipeline behind the
//! `LanguageAdapter` seam. The bodies of `analyze_diff` and `analyze_repo`
//! are relocated from `analysis::pipeline` without behavior change; the
//! pipeline module is now a language-neutral orchestrator that loads the
//! diff, dispatches to this adapter, and applies sort + summary on the
//! returned findings.

use super::super::{
    AnalysisMode, AnalysisOptions, classifier, classify, diff::ChangedFile, probes, rust_index,
    workspace,
};
use super::{LanguageAdapter, LanguageDiffResult, LanguageId, LanguageRepoResult, route};
use crate::analysis::cancellation;
use crate::analysis::facts::{FunctionSummary, RustIndex};
use crate::config::OraclePolicy;
use crate::domain::{
    ExposureClass, Finding, Probe, SourceCurrentness, StaticLimitKind, StopReason,
};
use std::collections::BTreeSet;
use std::path::Path;

/// Default ceiling on the number of Rust files a diff-scoped analysis will
/// load into the index. A large multi-crate diff expands the index far beyond
/// the changed files (`select_rust_files_for_mode` pulls in whole touched
/// packages), and building that working set can exhaust a constrained runner
/// (issue #1023). Above this many files the analysis fails closed with a named
/// `diff_scope_oversized` error rather than exhausting host memory and aborting.
const DIFF_INDEX_FILE_LIMIT: usize = 800;

/// Hard analysis-cost guard for the repo-scoped path (#2109): the diff path
/// caps its working set at [`DIFF_INDEX_FILE_LIMIT`], and the repo path now
/// has the same guard so `ripr check --mode deep|ready` on a large monorepo
/// fails closed with a named `repo_scope_oversized` error instead of loading
/// and indexing the entire workspace unbounded.
const REPO_INDEX_FILE_LIMIT: usize = 800;

/// Env override for [`REPO_INDEX_FILE_LIMIT`].
const REPO_INDEX_FILE_LIMIT_ENV: &str = "RIPR_MAX_REPO_INDEX_FILES";

/// Env override for [`DIFF_INDEX_FILE_LIMIT`]. Operators on larger, well-resourced
/// runners raise it; CI can lower it to exercise the guard.
const DIFF_INDEX_FILE_LIMIT_ENV: &str = "RIPR_MAX_DIFF_INDEX_FILES";

/// Default ceiling on the number of added/removed Rust diff lines that may be
/// expanded into probes. Large code-motion PRs can touch only one indexed file
/// but still create thousands of probe/classifier records, exhausting
/// constrained runners before an artifact is written (#1324).
const DIFF_CHANGED_RUST_LINE_LIMIT: usize = 2_000;

/// Env override for [`DIFF_CHANGED_RUST_LINE_LIMIT`]. Operators can raise it
/// for larger runners or lower it to exercise the guard.
const DIFF_CHANGED_RUST_LINE_LIMIT_ENV: &str = "RIPR_MAX_DIFF_CHANGED_RUST_LINES";

/// Named, matchable prefix for the diff-scope guard errors (#1023, #1324).
/// The LSP refresh path matches this prefix to convert the fail-closed guard
/// stop into a committed limited snapshot with one workspace-scoped warning
/// diagnostic (#2299); the CLI path keeps the non-zero exit and the unchanged
/// error text. The distinct `repo_scope_oversized` guard (#2109) does NOT
/// share this prefix and never converts on the LSP path.
pub(crate) const DIFF_SCOPE_OVERSIZED_PREFIX: &str = "diff_scope_oversized";

/// True when `error` is the named diff-scope guard error (#2299). Matchable
/// in the style of `git::is_git_invocation_timeout`: only the raw,
/// unwrapped guard error matches — a wrapped error (for example
/// `workspace analysis failed: ...`) does not.
pub(crate) fn is_diff_scope_oversized(error: &str) -> bool {
    error.starts_with(DIFF_SCOPE_OVERSIZED_PREFIX)
}
const NO_TESTS_INFECTION_SUMMARY: &str =
    "No tests were found, so activation/infection cannot be estimated";
const NO_STATICALLY_REACHABLE_TEST_PATH_INFECTION_SUMMARY: &str =
    "No statically reachable test path was found, so activation/infection cannot be estimated";

fn diff_index_file_limit() -> Result<usize, String> {
    diff_index_file_limit_from_env(std::env::var(DIFF_INDEX_FILE_LIMIT_ENV))
}

fn diff_index_file_limit_from_env(
    value: Result<String, std::env::VarError>,
) -> Result<usize, String> {
    positive_limit_from_env(DIFF_INDEX_FILE_LIMIT_ENV, DIFF_INDEX_FILE_LIMIT, value)
}

fn repo_index_file_limit_from_env(
    value: Result<String, std::env::VarError>,
) -> Result<usize, String> {
    positive_limit_from_env(REPO_INDEX_FILE_LIMIT_ENV, REPO_INDEX_FILE_LIMIT, value)
}

/// Fail closed when a repo-scoped working set exceeds the guard (#2109).
/// The repair route names only effective continuations: a diff-based run
/// (`--base`/`--diff`) or raising the limit. A "narrower mode" is NOT
/// offered — repo-scoped analysis does not select files by mode, so that
/// retry would hit the same guard.
fn enforce_repo_index_file_limit(file_count: usize, scope_limit: usize) -> Result<(), String> {
    if file_count <= scope_limit {
        return Ok(());
    }
    Err(format!(
        "repo_scope_oversized: {file_count} indexed Rust files exceed the \
         {REPO_INDEX_FILE_LIMIT_ENV} limit ({scope_limit}); analysis was not run to protect \
         runner memory. Repair route: narrow the scope with a diff-based run (--base/--diff), \
         or raise the limit via {REPO_INDEX_FILE_LIMIT_ENV}=<number>."
    ))
}

fn diff_changed_rust_line_limit() -> Result<usize, String> {
    diff_changed_rust_line_limit_from_env(std::env::var(DIFF_CHANGED_RUST_LINE_LIMIT_ENV))
}

fn diff_changed_rust_line_limit_from_env(
    value: Result<String, std::env::VarError>,
) -> Result<usize, String> {
    positive_limit_from_env(
        DIFF_CHANGED_RUST_LINE_LIMIT_ENV,
        DIFF_CHANGED_RUST_LINE_LIMIT,
        value,
    )
}

fn positive_limit_from_env(
    env_name: &str,
    default: usize,
    value: Result<String, std::env::VarError>,
) -> Result<usize, String> {
    match value {
        Ok(raw) => {
            let parsed = raw
                .trim()
                .parse::<usize>()
                .map_err(|err| format!("{env_name} must be a positive integer: {err}"))?;
            if parsed == 0 {
                return Err(format!("{env_name} must be a positive integer"));
            }
            Ok(parsed)
        }
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!("{env_name} must be valid UTF-8")),
    }
}

/// Default partial-selection budget on the number of changed-line files a
/// diff-scoped Rust analysis will inspect before returning a bounded
/// `limited_partial_scope` result (RIPR-PROP-0019, #1999). Deliberately
/// smaller than [`DIFF_INDEX_FILE_LIMIT`]: the hard guard protects runner
/// memory and still fails closed with `diff_scope_oversized`; this budget is
/// the lower, interactive-cost bound that yields a disclosed partial result
/// instead of an all-or-nothing error.
const PARTIAL_DIFF_FILE_BUDGET_DEFAULT: usize = 200;

/// Env override for [`PARTIAL_DIFF_FILE_BUDGET_DEFAULT`]. This is the only
/// continuation route for a partial run (RIPR-PROP-0019 decision 6); named
/// partition continuation is a deliberate non-goal.
pub(crate) const PARTIAL_DIFF_FILE_BUDGET_ENV: &str = "RIPR_PARTIAL_DIFF_FILE_BUDGET";

/// Default partial-selection budget on added/removed changed lines across the
/// selected partition. Deliberately smaller than
/// [`DIFF_CHANGED_RUST_LINE_LIMIT`] for the same reason as the file budget.
const PARTIAL_DIFF_LINE_BUDGET_DEFAULT: usize = 1_000;

/// Env override for [`PARTIAL_DIFF_LINE_BUDGET_DEFAULT`].
pub(crate) const PARTIAL_DIFF_LINE_BUDGET_ENV: &str = "RIPR_PARTIAL_DIFF_LINE_BUDGET";

/// Selection-algorithm version stamped into the partition identity
/// (RIPR-PROP-0019 decision 7). Bumps only on a contract revision of the
/// selection algorithm.
pub const PARTIAL_DIFF_SELECTION_VERSION: &str = "partial-diff-v1";

/// Language-tier ordering version stamped into the partition identity
/// (RIPR-PROP-0019 decision 7). Bumps if language tiers are added or
/// reordered. `lang-tier-v1`: supported language (Rust) first, then
/// preview-language files carrying changed lines.
pub const PARTIAL_DIFF_LANGUAGE_TIER_VERSION: &str = "lang-tier-v1";

/// Which partial-selection budget bound stopped selection (RIPR-PROP-0019
/// decision 3). Recorded on every `limited_partial_scope` result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartialDiffStopReason {
    /// The file budget bound selection. Also reported when the same file hits
    /// both budgets (the simultaneous-hit rule), with the line count recorded
    /// alongside on the scope record.
    FileBudget,
    /// A later whole file would have exceeded the remaining line budget and
    /// was excluded; selection never overshoots the line budget after the
    /// first selected file.
    LineBudget,
    /// The first selected file alone exceeded the line budget; that single
    /// file was analyzed anyway so the partition is never empty. Always wins
    /// over the simultaneous-hit rule.
    LineBudgetExceededOnFirstFile,
}

impl PartialDiffStopReason {
    /// Stable wire string for JSON / human output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FileBudget => "file_budget",
            Self::LineBudget => "line_budget",
            Self::LineBudgetExceededOnFirstFile => "line_budget_exceeded_on_first_file",
        }
    }
}

/// The typed run state of a `limited_partial_scope` diff analysis
/// (RIPR-PROP-0019 decision 4). Carries the exact selected paths in selection
/// order, lower-bound uninspected accounting derived from the diff (never
/// estimates), the named stop reason, and the run-comparable partition
/// identity (decision 7).
///
/// A partial result is advisory only: it is never a gate, baseline, badge, or
/// RIPR Zero input (decision 5), and its identity marks it
/// `gate_eligibility: ineligible` so a downstream consumer fails closed
/// rather than treating a partial denominator as complete.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartialDiffScope {
    /// Stable run-state wire string for this result.
    pub run_status: String,
    /// Content identity of the parsed diff, computed the same way for
    /// full-scope and partial runs (`sha256:`-prefixed lowercase hex).
    pub diff_identity: String,
    /// Effective (post-clamp) changed-line file budget.
    pub file_budget: usize,
    /// Effective (post-clamp) changed-line budget.
    pub line_budget: usize,
    /// Clamp disclosures emitted when an override exceeded its hard guard.
    /// Empty when no clamp occurred.
    pub budget_disclosures: Vec<String>,
    /// Exact selected file paths (normalized, forward-slash, repo-relative),
    /// in deterministic selection order.
    pub selected_files: Vec<String>,
    /// Changed-line count across the selected partition.
    pub selected_changed_lines: usize,
    /// Lower-bound count of changed-line files that were NOT inspected.
    pub uninspected_files_lower_bound: usize,
    /// Lower-bound changed-line count that was NOT inspected.
    pub uninspected_changed_lines_lower_bound: usize,
    /// Which budget bound stopped selection.
    pub stop_reason: PartialDiffStopReason,
    /// Lowercase hex sha256 of the canonical partition form (decision 7).
    pub partition_identity: String,
}

impl PartialDiffScope {
    /// The run-state wire string for every partial result.
    pub const RUN_STATUS: &'static str = "limited_partial_scope";
    /// Gate eligibility marker for every partial result (decision 5): a
    /// downstream consumer must fail closed on this state.
    pub const GATE_ELIGIBILITY: &'static str = "ineligible";
    /// Disclosure naming the only continuation route (decision 6): raise the
    /// explicit budget overrides. Named partition continuation is not
    /// available in this contract revision.
    pub const CONTINUATION_DISCLOSURE: &'static str = "partial result: raise RIPR_PARTIAL_DIFF_FILE_BUDGET and/or \
         RIPR_PARTIAL_DIFF_LINE_BUDGET to widen the analyzed partition; named \
         partition continuation is not available";

    /// Whether `path` (any spelling) names a selected file.
    pub(crate) fn selects(&self, path: &Path) -> bool {
        let normalized = normalize_changed_path(path);
        self.selected_files.contains(&normalized)
    }
}

/// Effective partial-selection budgets after env parsing and hard-guard
/// clamping, plus the clamp disclosures (RIPR-PROP-0019 decision 3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PartialDiffBudgets {
    pub(crate) file_budget: usize,
    pub(crate) line_budget: usize,
    pub(crate) disclosures: Vec<String>,
}

pub(crate) fn partial_diff_budgets() -> Result<PartialDiffBudgets, String> {
    partial_diff_budgets_from_env(
        std::env::var(PARTIAL_DIFF_FILE_BUDGET_ENV),
        std::env::var(PARTIAL_DIFF_LINE_BUDGET_ENV),
        diff_index_file_limit()?,
        diff_changed_rust_line_limit()?,
    )
}

/// `effective_file_limit` / `effective_line_limit` are the resolved
/// analysis-cost limits (env override or built-in default) the same run's
/// hard guards enforce. Clamping against the *effective* limit — not the
/// built-in default — keeps the budgets coherent when an operator raises a
/// limit: a run that authorizes `RIPR_MAX_DIFF_CHANGED_RUST_LINES=2500` must
/// accept a 2,501+ line-budget override up to that same ceiling instead of
/// silently truncating it back to the default (#3595 review).
fn partial_diff_budgets_from_env(
    file_value: Result<String, std::env::VarError>,
    line_value: Result<String, std::env::VarError>,
    effective_file_limit: usize,
    effective_line_limit: usize,
) -> Result<PartialDiffBudgets, String> {
    let (file_budget, file_disclosure) = partial_budget_from_env(
        PARTIAL_DIFF_FILE_BUDGET_ENV,
        PARTIAL_DIFF_FILE_BUDGET_DEFAULT,
        effective_file_limit,
        file_value,
    )?;
    let (line_budget, line_disclosure) = partial_budget_from_env(
        PARTIAL_DIFF_LINE_BUDGET_ENV,
        PARTIAL_DIFF_LINE_BUDGET_DEFAULT,
        effective_line_limit,
        line_value,
    )?;
    let mut disclosures = Vec::new();
    disclosures.extend(file_disclosure);
    disclosures.extend(line_disclosure);
    Ok(PartialDiffBudgets {
        file_budget,
        line_budget,
        disclosures,
    })
}

/// Resolve one partial-budget override. Mirrors the env-parse contract of the
/// hard guards (`positive_limit_from_env`): an empty, non-numeric, or
/// overflowing value is a parse failure, and zero is rejected; every failure
/// fails closed as a named `partial_budget_invalid` error — never a silent
/// unlimited or hidden fallback. A valid override above the corresponding
/// effective analysis-cost limit is clamped to that limit and the clamp is
/// disclosed (RIPR-PROP-0019 decision 3).
fn partial_budget_from_env(
    env_name: &str,
    default: usize,
    effective_limit: usize,
    value: Result<String, std::env::VarError>,
) -> Result<(usize, Option<String>), String> {
    let parsed = positive_limit_from_env(env_name, default, value)
        .map_err(|err| format!("partial_budget_invalid: {err}"))?;
    if parsed > effective_limit {
        Ok((
            effective_limit,
            Some(format!(
                "{env_name}={parsed} exceeds the effective analysis-cost limit \
                 ({effective_limit}); clamped to {effective_limit}"
            )),
        ))
    } else {
        Ok((parsed, None))
    }
}

/// One changed-line file eligible for partition selection. Context-only
/// files (no changed lines) are never candidates: they play their existing
/// read-only context role and never consume the partial budget
/// (RIPR-PROP-0019 decision 2).
#[derive(Clone, Debug)]
struct PartitionCandidate {
    normalized_path: String,
    package: String,
    language_tier: usize,
    changed_lines: usize,
    /// Whether the language adapter for this file is enabled for the run.
    /// Disabled-language files are never selected, but still count toward the
    /// uninspected lower bounds so the scope record never hides them
    /// (#2142 review).
    enabled: bool,
}

fn normalize_changed_path(path: &Path) -> String {
    crate::analysis::stable_path_text(path)
        .trim_start_matches("./")
        .to_string()
}

/// Content identity of the parsed diff, computed identically for full-scope
/// and partial runs (RIPR-PROP-0019 decision 7). Canonical rendering: files
/// sorted by normalized path, one `file=` line each, then one line per
/// changed line (`+<new-side line>:<text>` / `-<old-side line>:<text>`) in
/// parser order; LF-separated; `sha256:`-prefixed lowercase hex.
fn diff_identity_from_changed_files(changed_files: &[ChangedFile]) -> String {
    let mut files: Vec<&ChangedFile> = changed_files.iter().collect();
    files.sort_by_key(|file| normalize_changed_path(&file.path));
    let mut lines = Vec::new();
    for file in files {
        lines.push(format!("file={}", normalize_changed_path(&file.path)));
        for added in &file.added_lines {
            lines.push(format!("+{}:{}", added.new_side_line, added.text));
        }
        for removed in &file.removed_lines {
            lines.push(format!("-{}:{}", removed.line, removed.text));
        }
    }
    format!("sha256:{}", sha256_hex(lines.join("\n").as_bytes()))
}

/// Canonical partition form (RIPR-PROP-0019 decision 7): one field per line,
/// LF-separated, UTF-8. Never a generic map serialization — the field order
/// is fixed by construction here.
fn partition_canonical_form(
    diff_identity: &str,
    file_budget: usize,
    line_budget: usize,
    selected_sorted: &[String],
) -> String {
    let mut lines = vec![
        format!("selection_version={PARTIAL_DIFF_SELECTION_VERSION}"),
        format!("language_tier_version={PARTIAL_DIFF_LANGUAGE_TIER_VERSION}"),
        format!("diff_identity={diff_identity}"),
        format!("file_budget={file_budget}"),
        format!("line_budget={line_budget}"),
    ];
    for path in selected_sorted {
        lines.push(format!("selected={path}"));
    }
    lines.join("\n")
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let mut rendered = String::with_capacity(digest.len() * 2);
    for byte in digest {
        rendered.push_str(&format!("{byte:02x}"));
    }
    rendered
}

/// Select the deterministic bounded partition for a diff that exceeds the
/// partial-selection budget (RIPR-PROP-0019 decisions 1-3). Returns `None`
/// when the diff fits within both budgets (a full-scope run).
///
/// Selection unit: the changed file, whole — files are never split.
/// Selection order is fully deterministic and content-independent:
/// supported-language (Rust) changed-line files first, then preview-language
/// changed-line files; within a tier, package path ascending, then file path
/// ascending. The order does not depend on diff ordering, filesystem
/// enumeration order, mtimes, sizes, or content hashes.
///
/// Stop rules: the first selected file is analyzed even when it alone
/// exceeds the line budget (`line_budget_exceeded_on_first_file` — never an
/// empty partition); a later whole file that would exceed the remaining line
/// budget is excluded with stop reason `line_budget` (never an overshoot);
/// when the same file hits both budgets the stop reason is `file_budget`
/// with the line count recorded on the scope record; the first-file
/// exception always wins over the simultaneous-hit rule.
#[cfg(test)]
fn select_partial_diff_partition(
    changed_files: &[ChangedFile],
    budgets: &PartialDiffBudgets,
    enabled_languages: &[LanguageId],
) -> Option<PartialDiffScope> {
    select_partial_diff_partition_with_identity(
        changed_files,
        changed_files,
        budgets,
        enabled_languages,
    )
}

fn select_partial_diff_partition_with_identity(
    changed_files: &[ChangedFile],
    identity_files: &[ChangedFile],
    budgets: &PartialDiffBudgets,
    enabled_languages: &[LanguageId],
) -> Option<PartialDiffScope> {
    let mut candidates: Vec<PartitionCandidate> = changed_files
        .iter()
        .filter_map(|file| {
            let changed_lines = file
                .added_lines
                .len()
                .saturating_add(file.removed_lines.len());
            if changed_lines == 0 {
                return None;
            }
            let language = route(&file.path)?;
            let language_tier = if language == LanguageId::Rust { 0 } else { 1 };
            let normalized_path = normalize_changed_path(&file.path);
            Some(PartitionCandidate {
                package: workspace::package_root(&file.path).unwrap_or_default(),
                normalized_path,
                language_tier,
                changed_lines,
                enabled: enabled_languages.contains(&language),
            })
        })
        .collect();
    let total_files = candidates.len();
    let total_lines = candidates.iter().fold(0usize, |sum, candidate| {
        sum.saturating_add(candidate.changed_lines)
    });
    if total_files <= budgets.file_budget && total_lines <= budgets.line_budget {
        return None;
    }
    candidates.sort_by(|left, right| {
        left.language_tier
            .cmp(&right.language_tier)
            .then_with(|| left.package.cmp(&right.package))
            .then_with(|| left.normalized_path.cmp(&right.normalized_path))
    });

    let mut selected: Vec<&PartitionCandidate> = Vec::new();
    let mut selected_lines = 0usize;
    let mut stop_reason = None;
    for candidate in &candidates {
        // A file whose language adapter is not enabled for this run is never
        // selected: selecting it would advertise an inspected path no adapter
        // will inspect (#2142 review). It stays counted in the totals, so the
        // uninspected lower bounds remain honest.
        if !candidate.enabled {
            continue;
        }
        if selected.is_empty()
            && stop_reason.is_none()
            && candidate.changed_lines > budgets.line_budget
        {
            // First-file exception: analyze that single file anyway so the
            // partition is never empty; always wins over simultaneous-hit.
            selected.push(candidate);
            selected_lines = candidate.changed_lines;
            stop_reason = Some(PartialDiffStopReason::LineBudgetExceededOnFirstFile);
            break;
        }
        let would_exceed_file_budget = selected.len().saturating_add(1) > budgets.file_budget;
        let would_exceed_line_budget =
            selected_lines.saturating_add(candidate.changed_lines) > budgets.line_budget;
        if would_exceed_file_budget {
            // Simultaneous-hit included: when the same file hits both budgets
            // the stop reason is the file budget (line count recorded on the
            // scope record via selected_changed_lines).
            stop_reason = Some(PartialDiffStopReason::FileBudget);
            break;
        }
        if would_exceed_line_budget {
            // A later whole file is excluded; never included with overshoot.
            stop_reason = Some(PartialDiffStopReason::LineBudget);
            break;
        }
        selected.push(candidate);
        selected_lines = selected_lines.saturating_add(candidate.changed_lines);
    }
    // A budget-exceeding diff always reaches a stop rule: over the file
    // budget the file rule fires, over the line budget some file must cross
    // the remaining budget (or the first-file exception fired).
    let stop_reason = stop_reason?;

    let selected_files: Vec<String> = selected
        .iter()
        .map(|candidate| candidate.normalized_path.clone())
        .collect();
    let mut selected_sorted = selected_files.clone();
    selected_sorted.sort();
    let diff_identity = diff_identity_from_changed_files(identity_files);
    let canonical = partition_canonical_form(
        &diff_identity,
        budgets.file_budget,
        budgets.line_budget,
        &selected_sorted,
    );
    Some(PartialDiffScope {
        run_status: PartialDiffScope::RUN_STATUS.to_string(),
        diff_identity,
        file_budget: budgets.file_budget,
        line_budget: budgets.line_budget,
        budget_disclosures: budgets.disclosures.clone(),
        selected_files,
        selected_changed_lines: selected_lines,
        uninspected_files_lower_bound: total_files.saturating_sub(selected.len()),
        uninspected_changed_lines_lower_bound: total_lines.saturating_sub(selected_lines),
        stop_reason,
        partition_identity: sha256_hex(canonical.as_bytes()),
    })
}

fn changed_rust_line_count(changed_files: &[ChangedFile]) -> usize {
    changed_files
        .iter()
        .filter(|file| route(&file.path) == Some(LanguageId::Rust))
        .map(|file| {
            file.added_lines
                .len()
                .saturating_add(file.removed_lines.len())
        })
        .sum()
}

fn enforce_changed_rust_line_limit(
    changed_files: &[ChangedFile],
    line_limit: usize,
) -> Result<(), String> {
    let changed_line_count = changed_rust_line_count(changed_files);
    if changed_line_count <= line_limit {
        return Ok(());
    }
    let changed_file_count = changed_files
        .iter()
        .filter(|file| route(&file.path) == Some(LanguageId::Rust))
        .count();
    Err(format!(
        "diff_scope_oversized: {changed_line_count} changed Rust lines across \
         {changed_file_count} Rust files exceed the {DIFF_CHANGED_RUST_LINE_LIMIT_ENV} \
         limit ({line_limit}); analysis was not run to protect runner memory before \
         probe expansion. Repair route: reduce the diff scope, split the extraction \
         PR, run a narrower diff, or raise the limit via \
         {DIFF_CHANGED_RUST_LINE_LIMIT_ENV}=<number>."
    ))
}

/// Returns `true` when the owner function carries an FFI or language-binding
/// attribute that indicates its surface may be exercised by an external-language
/// test oracle rather than a Rust test.
///
/// The markers checked are the standard attribute substrings used by the major
/// Rust FFI and binding crates. `extern "C"` is intentionally excluded: it is
/// an ABI qualifier on the `fn` keyword and is not captured in
/// `FunctionFact.attrs`.
fn owner_has_ffi_attr(owner_fn: &FunctionSummary) -> bool {
    const FFI_MARKERS: &[&str] = &[
        "no_mangle",
        "export_name",
        "wasm_bindgen",
        "napi",
        "pyo3",
        "uniffi",
        "cxx",
    ];
    owner_fn.attrs.iter().any(|attr| {
        let lowered = attr.to_lowercase();
        FFI_MARKERS.iter().any(|marker| lowered.contains(marker))
    })
}

/// Resolve the probe's owner function from the index and check for FFI attrs.
/// Returns `Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved)` when
/// the probe owner is FFI/binding-exposed and the finding class is an
/// unrevealed gap; `None` otherwise. Pure-Rust owners (no FFI attrs) return
/// `None` unconditionally.
fn cross_language_limit_kind(
    probe: &crate::domain::Probe,
    index: &rust_index::RustIndex,
    class: &ExposureClass,
) -> Option<StaticLimitKind> {
    let is_gap_class = matches!(
        class,
        ExposureClass::WeaklyExposed
            | ExposureClass::ReachableUnrevealed
            | ExposureClass::InfectionUnknown
    );
    if !is_gap_class {
        return None;
    }
    let owner_id = probe.owner.as_ref()?;
    let owner_fn = index
        .functions
        .iter()
        .find(|function| &function.id == owner_id)?;
    if owner_has_ffi_attr(owner_fn) {
        Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved)
    } else {
        None
    }
}

/// Extract the bare function name from a probe's owner SymbolId for the
/// transitive-reach walk. The SymbolId format is "path::fn_name" or
/// "path::module::fn_name"; we return the last segment.
/// Returns None when the owner id is absent or the name is empty.
fn owner_name_from_id(
    owner: &Option<crate::domain::SymbolId>,
    _file: &std::path::Path,
) -> Option<String> {
    let id = owner.as_ref()?;
    // SymbolId format: "crates/ripr/src/lib.rs::pricing::score" or similar.
    // Take the last "::"-delimited segment.
    let name = id.0.split("::").last().unwrap_or("");
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn apply_rust_no_static_path_limit(finding: &mut Finding, probe: &Probe, index: &RustIndex) {
    if !(finding.class == ExposureClass::NoStaticPath
        && finding.related_tests.is_empty()
        && finding.static_limit_kind.is_none())
    {
        return;
    }

    let Some(owner_name) = owner_name_from_id(&probe.owner, &probe.location.file) else {
        return;
    };

    if let Some(witness) = classify::find_transitive_witness(&owner_name, index) {
        replace_witnessed_no_path_infection_summary(finding);
        finding.static_limit_kind = Some(transitive_reach_limit_kind(&witness.test_file));
        finding
            .stop_reasons
            .push(StopReason::TransitiveReachUnresolved);
        finding
            .evidence
            .push(classify::RUST_TRANSITIVE_REACH_MESSAGE.to_string());
        finding
            .evidence
            .push(classify::transitive_reach_witness_pointer(&witness));
        finding
            .evidence
            .extend(classify::transitive_reach_limitation_detail_lines(
                &witness,
                &owner_name,
            ));
    } else if let Some(witness) = classify::find_macro_reach_witness(&owner_name, index) {
        replace_witnessed_no_path_infection_summary(finding);
        finding.static_limit_kind = Some(macro_reach_limit_kind(&witness.macro_host));
        finding.stop_reasons.push(StopReason::MacroReachUnresolved);
        finding
            .evidence
            .push(classify::RUST_MACRO_REACH_MESSAGE.to_string());
        finding
            .evidence
            .push(classify::macro_reach_witness_pointer(&witness));
        finding
            .evidence
            .extend(classify::macro_reach_limitation_detail_lines(
                &witness,
                &owner_name,
            ));
    } else if let Some(test) = find_subprocess_binary_test(index, &probe.location.file) {
        finding.static_limit_kind = Some(StaticLimitKind::RustSubprocessBinaryReachUnresolved);
        finding.evidence.push(
            "An integration test invokes a Cargo-built binary, but ripr cannot yet map that binary back to the changed owner; no subprocess reach or receipt claim is made.".to_string(),
        );
        finding.evidence.push(format!(
            "Where to inspect: {}:{} ({})",
            test.file.display(),
            test.start_line,
            test.name
        ));
    }
}

fn find_subprocess_binary_test<'a>(
    index: &'a RustIndex,
    owner_file: &Path,
) -> Option<&'a crate::analysis::facts::TestFact> {
    if !is_binary_source_path(owner_file) {
        return None;
    }
    index
        .tests
        .iter()
        .filter(|test| rust_index::is_test_file(&test.file))
        .filter(|test| is_cargo_binary_invocation(&test.body))
        .min_by(|left, right| {
            left.file
                .cmp(&right.file)
                .then(left.start_line.cmp(&right.start_line))
                .then(left.name.cmp(&right.name))
        })
}

fn is_binary_source_path(path: &Path) -> bool {
    let components: Vec<_> = path.components().collect();
    components
        .windows(2)
        .any(|window| window[0].as_os_str() == "src" && window[1].as_os_str() == "main.rs")
        || components
            .windows(2)
            .any(|window| window[0].as_os_str() == "src" && window[1].as_os_str() == "bin")
}

fn is_cargo_binary_invocation(body: &str) -> bool {
    let compact: String = body
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let has_cargo_bin_env = compact.contains("Command::new(env!(\"CARGO_BIN_EXE_")
        && (compact.contains(".output(") || compact.contains(".status("));
    let has_assert_cmd_binary = compact.contains("cargo_bin(\"")
        && (compact.contains(".assert(")
            || compact.contains(".output(")
            || compact.contains(".status("));
    has_cargo_bin_env || has_assert_cmd_binary
}

fn apply_rust_macro_wrapped_assertion_limit(finding: &mut Finding, index: &RustIndex) {
    if !(finding.class == ExposureClass::ReachableUnrevealed
        && !finding.related_tests.is_empty()
        && finding.static_limit_kind.is_none()
        && finding.ripr.reveal.observe.state == crate::domain::StageState::No
        && finding
            .related_tests
            .iter()
            .all(|related| related.oracle.is_none()))
    {
        return;
    }

    let Some(witness) = find_unresolved_assertion_macro_witness(finding, index) else {
        return;
    };

    finding.static_limit_kind = Some(StaticLimitKind::RustMacroWrappedAssertionUnresolved);
    finding.evidence.push(
        "A related Rust test uses an assertion-like macro that ripr does not classify as an oracle."
            .to_string(),
    );
    finding
        .evidence
        .push(rust_macro_assertion_witness_pointer(&witness));
    finding
        .evidence
        .extend(rust_macro_assertion_limitation_detail_lines(&witness));
}

/// Name the bounded value-propagation limitation from #3215 without
/// pretending that syntax-first analysis proved the equality boundary.
///
/// This deliberately recognizes only a changed `let` binding whose value is
/// produced by `find`/`rfind` or `len_utf8`, normalized through `map_or`, and
/// whose same-owner body later compares that binding. Other helper, loop,
/// coercion, and data-flow shapes remain unchanged and fail closed as before.
fn apply_rust_value_propagation_limit(finding: &mut Finding, probe: &Probe, index: &RustIndex) {
    if finding.class != ExposureClass::StaticUnknown
        || finding.static_limit_kind.is_some()
        || finding.related_tests.is_empty()
    {
        return;
    }
    let Some((binding, rhs)) = changed_let_binding(&probe.expression) else {
        return;
    };
    let masked_rhs = mask_rust_comments_and_strings(rhs);
    if !masked_rhs.contains(".map_or(")
        || !(masked_rhs.contains(".find(")
            || masked_rhs.contains(".rfind(")
            || masked_rhs.contains(".len_utf8("))
    {
        return;
    }
    let Some(owner_id) = probe.owner.as_ref() else {
        return;
    };
    let Some(owner) = index
        .functions
        .iter()
        .find(|function| &function.id == owner_id)
    else {
        return;
    };
    let Some(predicate) = find_value_propagation_predicate(&owner.body, binding) else {
        return;
    };

    finding.static_limit_kind = Some(StaticLimitKind::RustValuePropagationUnresolved);
    finding
        .stop_reasons
        .push(StopReason::PropagationEvidenceUnknown);
    finding.evidence.push(format!(
        "limitation_last_established_edge: changed binding `{binding}` uses `{rhs}`"
    ));
    finding.evidence.push(format!(
        "limitation_first_unresolved_edge: `{binding}` value propagation into equality predicate `{}`",
        predicate.trim()
    ));
    finding
        .evidence
        .push("limitation_analyzer_route: analysis/rust-value-propagation".to_string());
    finding.evidence.push(
        "limitation_non_claim: named analyzer limitation only; ripr does not confirm coverage or prescribe a repair test"
            .to_string(),
    );
}

/// #3700 (final consolidation): a wrapper error seam — a `map_err`
/// conversion whose changed expression carries no parseable error variant —
/// carries the typed `wrapper_error_binding_unresolved` limitation. Whether
/// the boxed conversion faithfully carries the converted callee's error
/// variant (`Into`/`From` through `Box<dyn Error>`) is not statically
/// establishable, so the seam stays below `exposed` and the limitation names
/// what ripr could not resolve instead of prescribing an assertion the suite
/// may already contain. Classification (already `weakly_exposed` via the
/// unconfirmed-observation rule in reveal) is unchanged.
fn apply_wrapper_error_binding_limit(finding: &mut Finding, probe: &Probe) {
    if finding.class != ExposureClass::WeaklyExposed || finding.static_limit_kind.is_some() {
        return;
    }
    if !matches!(
        probe.family,
        crate::domain::ProbeFamily::ErrorPath | crate::domain::ProbeFamily::ReturnValue
    ) {
        return;
    }
    if !crate::analysis::classify::wrapper_error_seam_expression(&[probe.expression.as_str()]) {
        return;
    }

    finding.static_limit_kind = Some(StaticLimitKind::WrapperErrorBindingUnresolved);
    finding.evidence.push(
        "limitation_last_established_edge: changed wrapper conversion maps the converted callee's error into the boxed error channel".to_string(),
    );
    finding.evidence.push(
        "limitation_first_unresolved_edge: whether `Into`/`From` through `Box<dyn Error>` carries the callee's error variant to the wrapper's callers".to_string(),
    );
    finding
        .evidence
        .push("limitation_analyzer_route: analysis/wrapper-error-binding".to_string());
    finding.evidence.push(
        "limitation_non_claim: named analyzer limitation only; ripr does not confirm coverage or prescribe a repair test"
            .to_string(),
    );
}

/// probe's finding. The probe is predicate-shaped and classifies through
/// the normal predicate path; this only discloses the causal link (which
/// binding and initializer fed the predicate) and the operand-value
/// limitation. It never changes the class, adds a stop reason, or
/// prescribes a repair — the operand values stay unresolved until a
/// later slice evaluates them.
fn attach_changed_binding_predicate_evidence(
    finding: &mut Finding,
    relation: &Option<probes::ChangedBindingPredicateUse>,
) {
    let Some(relation) = relation else {
        return;
    };
    // The relation names both causal values when the diff carries the
    // old initializer; the probe's before/after already hold them, and
    // the relation line states them together.
    let initializer_range = match finding.probe.before.as_deref() {
        Some(before) if before != relation.initializer => {
            format!("`{before}` -> `{}`", relation.initializer)
        }
        _ => format!("`{}`", relation.initializer),
    };
    finding.evidence.push(format!(
        "binding_predicate_relation: changed binding `{}` initializer {initializer_range} flows into predicate operand at line {}",
        relation.binding, relation.predicate_line
    ));
    if let probes::BindingValueResolution::Unresolved { earliest_operation } =
        &relation.value_resolution
    {
        // Neutral prefixes: this is a value disclosure on a
        // predicate-shaped finding, not a `static_limit_kind` record,
        // so it deliberately stays outside the structured
        // `limitation_*` evidence contract (#3294 review).
        finding.evidence.push(format!(
            "binding_predicate_value_unresolved: operand value of `{}` unresolved at earliest initializer operation `{}`",
            relation.binding, earliest_operation
        ));
        finding.evidence.push(
            "binding_predicate_non_claim: named analyzer limitation only; ripr does not confirm coverage or prescribe a repair test"
                .to_string(),
        );
    }
}

/// Find an equality predicate that refers to the established binding after
/// its declaration. Comments, strings, member names, and later shadowing are
/// intentionally excluded so this limitation remains fail-closed.
fn find_value_propagation_predicate<'a>(body: &'a str, binding: &str) -> Option<&'a str> {
    let masked = mask_rust_comments_and_strings(body);
    let mut established = false;
    let mut shadowed = false;
    for (line, masked_line) in body.lines().zip(masked.lines()) {
        let trimmed = masked_line.trim();
        if trimmed.starts_with("let ") {
            let is_binding_declaration = trimmed
                .split_once('=')
                .is_some_and(|(lhs, _)| contains_identifier_token(lhs, binding));
            if is_binding_declaration && established {
                shadowed = true;
            }
            established = is_binding_declaration || established;
            continue;
        }
        if shadowed || !established || !binding_equality_predicate(trimmed, binding) {
            continue;
        }
        let mut search = 0;
        while let Some(offset) = trimmed[search..].find(binding) {
            let start = search + offset;
            let before = trimmed[..start].chars().next_back();
            let after = trimmed[start + binding.len()..].chars().next();
            if !before.is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric())
                && !after.is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric())
                && !trimmed[..start].trim_end().ends_with('.')
            {
                return Some(line);
            }
            search = start.saturating_add(binding.len());
        }
    }
    None
}

fn binding_equality_predicate(line: &str, binding: &str) -> bool {
    if line.contains("!=") {
        return false;
    }
    let Some((left, right)) = line.split_once("==") else {
        return false;
    };
    let left = left
        .rsplit_once("&&")
        .or_else(|| left.rsplit_once("||"))
        .map_or(left, |(_, operand)| operand);
    let right = right
        .split_once("&&")
        .or_else(|| right.split_once("||"))
        .map_or(right, |(operand, _)| operand);
    [left, right].into_iter().any(|side| {
        let Some(start) = side.find(binding) else {
            return false;
        };
        let before = side[..start].chars().next_back();
        let after = side[start + binding.len()..].chars().next();
        !before.is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric() || ch == '.')
            && !after.is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    })
}

/// A simple `let <ident> = <rhs>;` line. Shared by the #3271 limitation
/// and the #3294 binding-predicate relation so both agree on what a
/// changed binding is.
pub(crate) fn changed_let_binding(expression: &str) -> Option<(&str, &str)> {
    let text = expression.trim().trim_end_matches(';').trim();
    let rest = text.strip_prefix("let ")?;
    let (lhs, rhs) = rest.split_once('=')?;
    let binding = lhs.trim().strip_prefix("mut ").unwrap_or(lhs.trim());
    if binding.is_empty()
        || !binding
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    Some((binding, rhs.trim()))
}

fn contains_identifier_token(text: &str, ident: &str) -> bool {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|token| token == ident)
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RustMacroAssertionWitness {
    test_name: String,
    test_file: std::path::PathBuf,
    test_line: usize,
    macro_name: String,
    macro_line: usize,
}

fn find_unresolved_assertion_macro_witness(
    finding: &Finding,
    index: &RustIndex,
) -> Option<RustMacroAssertionWitness> {
    let mut candidates = Vec::new();
    for test in index
        .tests
        .iter()
        .chain(index.files.values().flat_map(|file| file.tests.iter()))
    {
        if !finding
            .related_tests
            .iter()
            .any(|related| related.name == test.name && related.file == test.file)
        {
            continue;
        }
        for (macro_name, macro_line) in
            unresolved_assertion_macro_invocations(&test.body, test.start_line)
        {
            candidates.push(RustMacroAssertionWitness {
                test_name: test.name.clone(),
                test_file: test.file.clone(),
                test_line: test.start_line,
                macro_name,
                macro_line,
            });
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates.into_iter().next()
}

fn unresolved_assertion_macro_invocations(body: &str, start_line: usize) -> Vec<(String, usize)> {
    let mut invocations = Vec::new();
    let masked_body = mask_rust_comments_and_strings(body);
    for (offset, line) in masked_body.lines().enumerate() {
        let mut search_start = 0usize;
        while let Some(relative_bang) = line[search_start..].find('!') {
            let bang = search_start + relative_bang;
            search_start = bang.saturating_add(1);
            if line[bang + 1..].starts_with('=') {
                continue;
            }
            if !line[bang + 1..]
                .trim_start()
                .chars()
                .next()
                .is_some_and(|ch| matches!(ch, '(' | '[' | '{'))
            {
                continue;
            }
            let Some(macro_name) = macro_name_before_bang(line, bang) else {
                continue;
            };
            if !is_unresolved_assertion_like_macro(&macro_name) {
                continue;
            }
            invocations.push((macro_name, start_line + offset));
        }
    }
    invocations.sort();
    invocations.dedup();
    invocations
}

pub(crate) fn mask_rust_comments_and_strings(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut masked = bytes.to_vec();
    let mut index = 0usize;
    let mut block_depth = 0usize;

    while index < bytes.len() {
        if block_depth > 0 {
            if starts_with_bytes(bytes, index, b"/*") {
                mask_non_newline_bytes(&mut masked, index, index.saturating_add(2));
                block_depth = block_depth.saturating_add(1);
                index = index.saturating_add(2);
            } else if starts_with_bytes(bytes, index, b"*/") {
                mask_non_newline_bytes(&mut masked, index, index.saturating_add(2));
                block_depth = block_depth.saturating_sub(1);
                index = index.saturating_add(2);
            } else {
                mask_non_newline_bytes(&mut masked, index, index.saturating_add(1));
                index = index.saturating_add(1);
            }
            continue;
        }

        if starts_with_bytes(bytes, index, b"//") {
            let end = bytes[index..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(bytes.len(), |offset| index + offset);
            mask_non_newline_bytes(&mut masked, index, end);
            index = end;
            continue;
        }

        if starts_with_bytes(bytes, index, b"/*") {
            mask_non_newline_bytes(&mut masked, index, index.saturating_add(2));
            block_depth = 1;
            index = index.saturating_add(2);
            continue;
        }

        if let Some(end) = rust_raw_string_literal_end(bytes, index) {
            mask_non_newline_bytes(&mut masked, index, end);
            index = end;
            continue;
        }

        if bytes[index] == b'"' {
            let end = rust_string_literal_end(bytes, index);
            mask_non_newline_bytes(&mut masked, index, end);
            index = end;
            continue;
        }

        index = index.saturating_add(1);
    }

    match String::from_utf8(masked) {
        Ok(value) => value,
        Err(_) => text.to_string(),
    }
}

fn starts_with_bytes(bytes: &[u8], index: usize, needle: &[u8]) -> bool {
    bytes
        .get(index..index.saturating_add(needle.len()))
        .is_some_and(|candidate| candidate == needle)
}

fn mask_non_newline_bytes(bytes: &mut [u8], start: usize, end: usize) {
    let bounded_end = end.min(bytes.len());
    for byte in bytes.iter_mut().take(bounded_end).skip(start) {
        if *byte != b'\n' {
            *byte = b' ';
        }
    }
}

fn rust_string_literal_end(bytes: &[u8], start: usize) -> usize {
    let mut index = start.saturating_add(1);
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            return index.saturating_add(1);
        }
        index = index.saturating_add(1);
    }
    bytes.len()
}

fn rust_raw_string_literal_end(bytes: &[u8], start: usize) -> Option<usize> {
    let prefix_len = if bytes.get(start) == Some(&b'r') {
        1
    } else if bytes.get(start) == Some(&b'b') && bytes.get(start.saturating_add(1)) == Some(&b'r') {
        2
    } else {
        return None;
    };

    let mut delimiter = start.saturating_add(prefix_len);
    let mut hashes = 0usize;
    while bytes.get(delimiter) == Some(&b'#') {
        hashes = hashes.saturating_add(1);
        delimiter = delimiter.saturating_add(1);
    }
    if bytes.get(delimiter) != Some(&b'"') {
        return None;
    }

    let mut index = delimiter.saturating_add(1);
    while index < bytes.len() {
        if bytes[index] == b'"' {
            let suffix_start = index.saturating_add(1);
            let suffix_end = suffix_start.saturating_add(hashes);
            if suffix_end <= bytes.len()
                && bytes[suffix_start..suffix_end]
                    .iter()
                    .all(|byte| *byte == b'#')
            {
                return Some(suffix_end);
            }
        }
        index = index.saturating_add(1);
    }

    Some(bytes.len())
}

fn macro_name_before_bang(line: &str, bang: usize) -> Option<String> {
    let prefix = line[..bang].trim_end();
    let end = prefix.len();
    if end == 0 {
        return None;
    }
    let mut start = end;
    for (idx, ch) in prefix.char_indices().rev() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == ':' {
            start = idx;
        } else {
            break;
        }
    }
    let name = prefix[start..end].trim_matches(':');
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn is_unresolved_assertion_like_macro(macro_name: &str) -> bool {
    if is_known_rust_assertion_macro(macro_name) {
        return false;
    }
    let base = macro_name.rsplit("::").next().unwrap_or(macro_name);
    base == "assert" || base.starts_with("assert_")
}

fn is_known_rust_assertion_macro(macro_name: &str) -> bool {
    let compact = macro_name.replace(' ', "");
    let base = compact.rsplit("::").next().unwrap_or(compact.as_str());
    matches!(
        base,
        "assert" | "assert_eq" | "assert_ne" | "assert_matches" | "matches"
    ) || compact.starts_with("insta::assert")
        || compact.contains("snapshot")
}

fn rust_macro_assertion_witness_pointer(witness: &RustMacroAssertionWitness) -> String {
    let test_location = format!(
        "{}:{}",
        witness.test_file.display().to_string().replace('\\', "/"),
        witness.test_line
    );
    let macro_location = format!(
        "{}:{}",
        witness.test_file.display().to_string().replace('\\', "/"),
        witness.macro_line
    );
    format!(
        "{}`{}` ({}) reaches the changed owner, then invokes assertion-like macro `{}!` at {}. ripr does not classify that macro as an oracle.",
        crate::domain::TRANSITIVE_REACH_WITNESS_PREFIX,
        witness.test_name,
        test_location,
        witness.macro_name,
        macro_location
    )
}

fn rust_macro_assertion_limitation_detail_lines(
    witness: &RustMacroAssertionWitness,
) -> [String; 4] {
    let test_location = format!(
        "{}:{}",
        witness.test_file.display().to_string().replace('\\', "/"),
        witness.test_line
    );
    let macro_location = format!(
        "{}:{}",
        witness.test_file.display().to_string().replace('\\', "/"),
        witness.macro_line
    );
    [
        format!(
            "{}test `{}` ({}) -> assertion macro `{}!` at {}",
            crate::domain::LIMITATION_LAST_ESTABLISHED_EDGE_PREFIX,
            witness.test_name,
            test_location,
            witness.macro_name,
            macro_location
        ),
        format!(
            "{}assertion macro `{}!` semantics toward the changed owner",
            crate::domain::LIMITATION_FIRST_UNRESOLVED_EDGE_PREFIX,
            witness.macro_name
        ),
        format!(
            "{}analysis/rust-macro-assertion-oracle",
            crate::domain::LIMITATION_ANALYZER_ROUTE_PREFIX
        ),
        format!(
            "{}named limitation only; ripr cannot confirm or deny that the macro assertion discriminates the change",
            crate::domain::LIMITATION_NON_CLAIM_PREFIX
        ),
    ]
}

fn transitive_reach_limit_kind(test_file: &Path) -> StaticLimitKind {
    if rust_index::is_test_file(test_file) {
        StaticLimitKind::RustIntegrationPublicApiPathUnresolved
    } else {
        StaticLimitKind::RustTransitiveReachUnresolved
    }
}

fn macro_reach_limit_kind(macro_host: &str) -> StaticLimitKind {
    if macro_host == classify::MACRO_WITNESS_TEST_BODY_HOST {
        StaticLimitKind::RustMacroWrappedTestCallUnresolved
    } else {
        StaticLimitKind::RustMacroReachUnresolved
    }
}

fn replace_witnessed_no_path_infection_summary(finding: &mut Finding) {
    if finding.ripr.infect.summary == NO_TESTS_INFECTION_SUMMARY {
        finding.ripr.infect.summary =
            NO_STATICALLY_REACHABLE_TEST_PATH_INFECTION_SUMMARY.to_string();
    }
    for evidence in &mut finding.evidence {
        if evidence == NO_TESTS_INFECTION_SUMMARY {
            *evidence = NO_STATICALLY_REACHABLE_TEST_PATH_INFECTION_SUMMARY.to_string();
        }
    }
}

/// Reference adapter for Rust.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RustAdapter;

/// Return whether a Rust path is a conventional generated-source surface.
///
/// This is deliberately conservative and always-on. Optional additive
/// patterns from `[languages.rust].generated_file_patterns` are applied separately;
/// the default still avoids analyzing common generated names without requiring
/// a config value.
pub(crate) fn is_generated_rust_file(path: &Path) -> bool {
    if route(path) != Some(LanguageId::Rust) {
        return false;
    }

    let name = path
        .file_name()
        .map(|value| value.to_string_lossy())
        .unwrap_or_default();
    let conventional_name = name == "bindings.rs"
        || name == "generated.rs"
        || name == "schema.rs"
        || name.ends_with(".gen.rs")
        || name.ends_with("_generated.rs")
        || name.starts_with("generated_");
    let generated_directory = path.components().any(|component| {
        let std::path::Component::Normal(value) = component else {
            return false;
        };
        matches!(
            value.to_string_lossy().as_ref(),
            "gen" | "generated" | "out"
        )
    });
    conventional_name || generated_directory
}

pub(crate) fn is_generated_rust_file_with_patterns(
    path: &Path,
    generated_file_patterns: &[String],
) -> bool {
    is_generated_rust_file(path)
        || generated_file_patterns
            .iter()
            .any(|pattern| generated_pattern_matches(pattern, path))
}

fn generated_pattern_matches(pattern: &str, path: &Path) -> bool {
    if route(path) != Some(LanguageId::Rust) {
        return false;
    }

    let normalized_path = path.to_string_lossy().replace('\\', "/");
    if pattern.contains('/') {
        path_glob_matches(pattern, &normalized_path)
    } else {
        path.file_name().is_some_and(|name| {
            let pattern_chars = pattern.chars().collect::<Vec<_>>();
            let name_chars = name.to_string_lossy().chars().collect::<Vec<_>>();
            let mut memo = vec![vec![None; name_chars.len() + 1]; pattern_chars.len() + 1];
            glob_segment_chars_match(&pattern_chars, &name_chars, 0, 0, &mut memo)
        })
    }
}

fn path_glob_matches(pattern: &str, path: &str) -> bool {
    let pattern_segments = pattern
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .map(|segment| segment.chars().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let path_segments = path
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .map(|segment| segment.chars().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let mut memo = vec![vec![None; path_segments.len() + 1]; pattern_segments.len() + 1];
    glob_segments_match(&pattern_segments, &path_segments, 0, 0, &mut memo)
}

fn glob_segments_match(
    pattern: &[Vec<char>],
    path: &[Vec<char>],
    pattern_index: usize,
    path_index: usize,
    memo: &mut [Vec<Option<bool>>],
) -> bool {
    if let Some(result) = memo[pattern_index][path_index] {
        return result;
    }

    let result = if pattern_index == pattern.len() {
        path_index == path.len()
    } else if pattern[pattern_index] == ['*', '*'] {
        glob_segments_match(pattern, path, pattern_index + 1, path_index, memo)
            || (path_index < path.len()
                && glob_segments_match(pattern, path, pattern_index, path_index + 1, memo))
    } else if path_index == path.len() {
        false
    } else {
        let segment_pattern = &pattern[pattern_index];
        let segment = &path[path_index];
        let mut segment_memo = vec![vec![None; segment.len() + 1]; segment_pattern.len() + 1];
        glob_segment_chars_match(segment_pattern, segment, 0, 0, &mut segment_memo)
            && glob_segments_match(pattern, path, pattern_index + 1, path_index + 1, memo)
    };

    memo[pattern_index][path_index] = Some(result);
    result
}

fn glob_segment_chars_match(
    pattern: &[char],
    segment: &[char],
    pattern_index: usize,
    segment_index: usize,
    memo: &mut [Vec<Option<bool>>],
) -> bool {
    if let Some(result) = memo[pattern_index][segment_index] {
        return result;
    }

    let result = if pattern_index == pattern.len() {
        segment_index == segment.len()
    } else {
        match pattern[pattern_index] {
            '*' => {
                glob_segment_chars_match(pattern, segment, pattern_index + 1, segment_index, memo)
                    || (segment_index < segment.len()
                        && glob_segment_chars_match(
                            pattern,
                            segment,
                            pattern_index,
                            segment_index + 1,
                            memo,
                        ))
            }
            '?' => {
                segment_index < segment.len()
                    && glob_segment_chars_match(
                        pattern,
                        segment,
                        pattern_index + 1,
                        segment_index + 1,
                        memo,
                    )
            }
            expected => {
                segment_index < segment.len()
                    && segment[segment_index] == expected
                    && glob_segment_chars_match(
                        pattern,
                        segment,
                        pattern_index + 1,
                        segment_index + 1,
                        memo,
                    )
            }
        }
    };

    memo[pattern_index][segment_index] = Some(result);
    result
}

impl RustAdapter {
    /// Diff analysis with the enabled-language set the pipeline will
    /// dispatch, so the partial-diff partition (RIPR-PROP-0019) never selects
    /// a file no enabled adapter will inspect (#2142 review).
    pub(crate) fn analyze_diff_for_languages(
        &self,
        options: &AnalysisOptions,
        oracle_policy: &OraclePolicy,
        changed_files: &[ChangedFile],
        enabled_languages: &[LanguageId],
    ) -> Result<LanguageDiffResult, String> {
        self.analyze_diff_for_languages_with_generated_file_patterns(
            options,
            oracle_policy,
            changed_files,
            enabled_languages,
            &[],
        )
    }

    pub(crate) fn analyze_diff_for_languages_with_generated_file_patterns(
        &self,
        options: &AnalysisOptions,
        oracle_policy: &OraclePolicy,
        changed_files: &[ChangedFile],
        enabled_languages: &[LanguageId],
        generated_file_patterns: &[String],
    ) -> Result<LanguageDiffResult, String> {
        // Exclude conventional generated surfaces before hard line limits and
        // partial-diff budgeting so machine output cannot consume the budget
        // that protects actionable source analysis.
        let analyzable_changed_files = changed_files
            .iter()
            .filter(|file| {
                !is_generated_rust_file_with_patterns(&file.path, generated_file_patterns)
            })
            .cloned()
            .collect::<Vec<_>>();
        enforce_changed_rust_line_limit(
            &analyzable_changed_files,
            diff_changed_rust_line_limit()?,
        )?;
        // RIPR-PROP-0019 (#1999): within the hard guards, a diff that exceeds
        // the smaller partial-selection budget is analyzed as a deterministic
        // bounded partition and reported as `limited_partial_scope` instead of
        // failing closed with zero findings. A malformed override fails closed
        // as `partial_budget_invalid`.
        let partial_budgets = partial_diff_budgets()?;
        let partial_scope = select_partial_diff_partition_with_identity(
            &analyzable_changed_files,
            changed_files,
            &partial_budgets,
            enabled_languages,
        );
        let changed_rust_paths = analyzable_changed_files
            .iter()
            .filter(|file| self.accepts_path(&file.path))
            .filter(|file| {
                partial_scope
                    .as_ref()
                    .is_none_or(|scope| scope.selects(&file.path))
            })
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        let rust_files = workspace::discover_rust_files(&options.root)?;
        let analyzable_rust_files = rust_files
            .into_iter()
            .filter(|path| !is_generated_rust_file_with_patterns(path, generated_file_patterns))
            .collect::<Vec<_>>();
        // #2970 slice C: in the modes whose selection narrows to changed
        // packages (Draft/Fast with unchanged tests), a behavior change in a
        // path dependency can surface in every crate that depends on it, so
        // the reverse path-dependency adjacency contributes the dependent
        // package roots to the scope decision. Expansion only ever adds
        // packages; every other selection contract (Instant changed-files-
        // only, Deep/Ready whole-workspace, include_unchanged_tests=false)
        // ignores the roots. The manifest scan runs only when package
        // narrowing can apply, so non-narrowing paths pay nothing. A
        // `limited`/`unavailable` graph is disclosed, never silently treated
        // as a complete reach.
        let (dependent_package_roots, manifest_dir_prefixes) =
            if matches!(options.mode, AnalysisMode::Draft | AnalysisMode::Fast)
                && options.include_unchanged_tests
            {
                let changed_package_roots = changed_rust_paths
                    .iter()
                    .filter_map(|path| workspace::package_root(path))
                    .collect::<std::collections::BTreeSet<_>>();
                // Files the layout heuristics cannot place (custom Cargo
                // target paths, #3616 review) would previously drop out of
                // the seed set entirely; they go to the expansion for
                // attribution against the discovered manifest inventory.
                let unattributed_changed_files = changed_rust_paths
                    .iter()
                    .filter(|path| workspace::package_root(path).is_none())
                    .map(|path| workspace::normalize_path(path))
                    .collect::<std::collections::BTreeSet<_>>();
                if changed_package_roots.is_empty() && unattributed_changed_files.is_empty() {
                    (std::collections::BTreeSet::new(), Vec::new())
                } else {
                    let expansion = workspace::reverse_dependent_scope_expansion(
                        &options.root,
                        &changed_package_roots,
                        &unattributed_changed_files,
                    );
                    if let Some(disclosure) = expansion.scope_disclosure() {
                        eprintln!("{disclosure}");
                    }
                    let manifest_dir_prefixes = expansion.manifest_dir_prefixes().to_vec();
                    let dependent_package_roots = expansion.into_dependent_package_roots();
                    (dependent_package_roots, manifest_dir_prefixes)
                }
            } else {
                (std::collections::BTreeSet::new(), Vec::new())
            };
        let index_files = workspace::select_rust_files_for_mode_with_dependent_packages(
            &analyzable_rust_files,
            &changed_rust_paths,
            options.mode,
            options.include_unchanged_tests,
            &dependent_package_roots,
            &manifest_dir_prefixes,
        );
        // Fail closed before the working-set build that can exhaust a
        // constrained runner's memory (#1023): a too-large index is a named
        // limited state with a repair route, not an analysis result.
        let scope_limit = diff_index_file_limit()?;
        if index_files.len() > scope_limit {
            return Err(format!(
                "diff_scope_oversized: {} indexed Rust files exceed the \
                 {DIFF_INDEX_FILE_LIMIT_ENV} limit ({scope_limit}); analysis was not run to \
                 protect runner memory. Repair route: reduce the diff scope, run a narrower \
                 mode, or raise the limit via {DIFF_INDEX_FILE_LIMIT_ENV}=<number>.",
                index_files.len()
            ));
        }
        // Load files into memory and use the content-addressed per-file fact
        // cache. This avoids re-parsing unchanged files with ra_ap_syntax on
        // every ripr check / LSP save (#1912). The cache is keyed on a
        // content hash; unchanged files hit the cache and skip the parse.
        let loaded_files = index_files
            .iter()
            .map(|file| {
                // Cooperative cancellation (#1972): a superseded or
                // deadline-expired LSP refresh stops the load loop instead
                // of reading the whole working set. No-op without a token
                // (CLI path).
                cancellation::checkpoint()?;
                let full = options.root.join(file);
                let bytes = std::fs::read(&full)
                    .map_err(|err| format!("failed to read {}: {err}", full.display()))?;
                Ok((file.clone(), bytes))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let cached = rust_index::build_index_from_loaded_files_with_cache_and_test_harnesses(
            &options.root,
            &loaded_files,
            &options.test_harnesses,
        )?;
        let mut index = cached.index;
        if let Some(disclosure) = rust_index::include_resolution_disclosure(&index) {
            eprintln!("{disclosure}");
        }
        if let Some(disclosure) = rust_index::module_composition_disclosure(&index) {
            eprintln!("{disclosure}");
        }
        rust_index::apply_oracle_policy(&mut index, oracle_policy);

        let mut findings = Vec::new();
        let mut changed_rust_files = 0usize;
        let mut candidate_lines = BTreeSet::new();

        // Authoritative source-role context (#3283): declared Cargo
        // test/bench targets confirm evidence role outside the default
        // layouts, and the repository opt-in restores production-like
        // analysis for selected targets.
        let mut source_role_context = workspace::context_for_files(
            &options.root,
            analyzable_rust_files.iter().map(|path| path.as_path()),
        );
        source_role_context.production_like_targets = options.production_like_targets.clone();
        // Cargo-validated file-wide harness evidence (#3608): only
        // registrations whose manifest declares `harness = false` keep
        // the grant.
        source_role_context.harness_targets =
            rust_index::validated_file_wide_harness_targets(&options.root, &options.test_harnesses);

        // #2971: The cross-crate calls_owner bypass in find_related_tests
        // requires a workspace-complete function index. In Instant/Draft/Fast
        // mode, index.functions is scoped to changed files or changed packages,
        // so a same-named function in an unchanged file would be absent from
        // the uniqueness count — fail-closed by treating the index as
        // incomplete for those modes.
        //
        // The mode alone does not decide this. `select_rust_files_for_mode`
        // returns the changed files only whenever `include_unchanged_tests` is
        // false, including under Deep and Ready, so keying on the mode would
        // still mark a changed-files-only index complete. Ask the selection
        // that actually built the index instead: it is a deduplicated subset
        // of `analyzable_rust_files`, so an equal length means nothing was
        // dropped. Any narrower selection leaves the index partial.
        //
        // #2970 slice C: path-dependency scope expansion folds into the same
        // derivation without special-casing. When the dependent roots happen
        // to cover every analyzable file, the equality genuinely holds and
        // the index really does span the workspace; when an unrelated crate
        // stays out, the index stays partial and the bypass stays off.
        let workspace_index_complete = index_files.len() == analyzable_rust_files.len();

        // #2972: one path-dependency edge context per classification pass,
        // consumed by the cross-crate owner-call admit in
        // `find_related_tests`. Only a whole-workspace index can admit (the
        // same precondition as the #2971 uniqueness bypass), so narrower
        // passes skip the manifest scan entirely.
        let (manifest_dir_prefixes, dependency_adjacency) = if workspace_index_complete {
            let manifest_dir_prefixes =
                crate::analysis::seam_cache::workspace_manifest_dir_prefixes(&options.root);
            let dependency_adjacency = workspace::PathDependencyAdjacency::build(
                &crate::analysis::seam_cache::workspace_graph_provenance(&options.root),
            );
            (manifest_dir_prefixes, Some(dependency_adjacency))
        } else {
            (Vec::new(), None)
        };
        let dependency_edges =
            dependency_adjacency
                .as_ref()
                .map(|adjacency| classify::DependencyEdgeContext {
                    adjacency,
                    manifest_dir_prefixes: &manifest_dir_prefixes,
                    index: &index,
                });

        for changed in analyzable_changed_files
            .iter()
            .filter(|file| self.accepts_path(&file.path))
            .filter(|file| {
                partial_scope
                    .as_ref()
                    .is_none_or(|scope| scope.selects(&file.path))
            })
        {
            changed_rust_files += 1;
            // Producer-owned source role (#3283): only production
            // subjects and explicitly opted-in production-like targets
            // seed diff probes; Cargo benches, examples, integration
            // tests, and confirmed test-target files stay indexed
            // evidence without harness-plumbing obligations. Changed
            // files under the repository's own automation directory
            // still seed probes: automation changes are reviewed
            // behavior (pre-#3283 diff behavior, pinned by the
            // `source_promotion_verification` fixture), while the
            // evidence role keeps governing repo-mode indexing.
            let role = workspace::classify_with(&changed.path, &source_role_context);
            if !role.seeds_production_findings()
                && !workspace::is_repo_automation_path(&changed.path)
            {
                continue;
            }
            // Cooperative cancellation (#1972): check once per changed file
            // and once per probe so a superseded or deadline-expired refresh
            // exits the classify loop promptly.
            cancellation::checkpoint()?;
            let probes = probes::probes_for_file_with_relations(&options.root, changed, &index);
            for (probe, binding_relation) in probes {
                candidate_lines.insert((probe.location.file.clone(), probe.location.line));
                cancellation::checkpoint()?;
                let mut finding = classifier::classify_probe(
                    &probe,
                    &index,
                    workspace_index_complete,
                    dependency_edges.as_ref(),
                );
                finding.language = Some(LanguageId::Rust);
                // Producer-owned source currentness (#3280): resolved from the diff
                // evidence that seeded the probe, before any limitation shaping.
                finding.source_currentness =
                    probes::resolve_probe_source_currentness(changed, &probe);
                // `language_status` is omitted for Rust per RIPR-SPEC-0026.
                // RIPR-SPEC-0114: when the direct-call classifier finds no related
                // test (no_static_path + empty related_tests), run the bounded
                // transitive-reach walk. If a candidate path is found, name the
                // limitation. Classification NEVER changes (fail-closed).
                // RIPR-SPEC-0115: the walk returns the witnessing test so the
                // limitation can name something concrete to open (file:line +
                // entry symbol). The witness is NOT added to related_tests.
                // RIPR-SPEC-0117: when no lexical transitive path is available,
                // name a macro-reach limitation only when a same-repo macro
                // definition lexically mentions the changed owner.
                apply_rust_no_static_path_limit(&mut finding, &probe, &index);
                // Name unresolved custom assertion macros only after reach has
                // already been established and no recognized oracle observes
                // the seam. This is an oracle limitation, not macro expansion
                // or promotion.
                apply_rust_macro_wrapped_assertion_limit(&mut finding, &index);
                apply_rust_value_propagation_limit(&mut finding, &probe, &index);
                apply_wrapper_error_binding_limit(&mut finding, &probe);
                // #3294: a retargeted changed-binding probe keeps its
                // predicate-shaped classification, but the finding still
                // discloses the operand-value limitation it inherited from the
                // changed initializer.
                attach_changed_binding_predicate_evidence(&mut finding, &binding_relation);
                // Fail closed on cross-language seams: when the probe owner
                // carries an FFI/binding attribute, replace any Rust-gap
                // static_limit_kind with the cross-language limitation so
                // downstream consumers know to verify the external oracle
                // rather than acting on a Rust repair packet. (#910)
                if let Some(limit) = cross_language_limit_kind(&probe, &index, &finding.class) {
                    finding.static_limit_kind = Some(limit);
                }
                findings.push(finding);
            }
        }

        Ok(LanguageDiffResult {
            findings,
            harness_projections: super::super::harness_projection::projections_from_index(
                &index,
                &options.test_harnesses,
            ),
            changed_files: changed_rust_files,
            candidate_line_count: candidate_lines.len(),
            changed_files_by_language: Vec::new(),
            partial_scope,
            skipped_files: changed_files
                .iter()
                .filter(|file| self.accepts_path(&file.path))
                .filter(|file| {
                    is_generated_rust_file_with_patterns(&file.path, generated_file_patterns)
                })
                .count(),
            limitations: Vec::new(),
        })
    }
}

impl LanguageAdapter for RustAdapter {
    fn accepts_path(&self, path: &Path) -> bool {
        matches!(route(path), Some(LanguageId::Rust))
    }

    /// Direct adapter calls (tests, non-pipeline callers) analyze with every
    /// language selectable; the pipeline uses
    /// [`RustAdapter::analyze_diff_for_languages`] with the real enabled set
    /// so the partial partition never selects an uninspected file.
    fn analyze_diff(
        &self,
        options: &AnalysisOptions,
        oracle_policy: &OraclePolicy,
        changed_files: &[ChangedFile],
    ) -> Result<LanguageDiffResult, String> {
        self.analyze_diff_for_languages(
            options,
            oracle_policy,
            changed_files,
            &[
                LanguageId::Rust,
                LanguageId::TypeScript,
                LanguageId::JavaScript,
                LanguageId::Python,
                LanguageId::Perl,
            ],
        )
    }

    fn analyze_repo(
        &self,
        options: &AnalysisOptions,
        oracle_policy: &OraclePolicy,
    ) -> Result<LanguageRepoResult, String> {
        self.analyze_repo_with_generated_file_patterns(options, oracle_policy, &[])
    }
}

impl RustAdapter {
    pub(crate) fn analyze_repo_with_generated_file_patterns(
        &self,
        options: &AnalysisOptions,
        oracle_policy: &OraclePolicy,
        generated_file_patterns: &[String],
    ) -> Result<LanguageRepoResult, String> {
        let rust_files = workspace::discover_rust_files(&options.root)?;
        let skipped_files = rust_files
            .iter()
            .filter(|path| is_generated_rust_file_with_patterns(path, generated_file_patterns))
            .count();
        let analyzable_rust_files = rust_files
            .iter()
            .filter(|path| !is_generated_rust_file_with_patterns(path, generated_file_patterns))
            .cloned()
            .collect::<Vec<_>>();
        // Fail closed before the whole-workspace load (#2109): an
        // over-limit repo analysis is a named error with a repair route,
        // not an unbounded read+index that can exhaust host memory.
        let scope_limit = repo_index_file_limit_from_env(std::env::var(REPO_INDEX_FILE_LIMIT_ENV))?;
        enforce_repo_index_file_limit(analyzable_rust_files.len(), scope_limit)?;
        // Producer-owned source role (#3283): the repo production set
        // routes through the same role as diff seeding — layout plus
        // declared Cargo targets plus the production-like opt-in.
        let repo_source_role_context = {
            let mut context = workspace::context_for_files(
                &options.root,
                analyzable_rust_files.iter().map(|path| path.as_path()),
            );
            context.production_like_targets = options.production_like_targets.clone();
            // Cargo-validated file-wide harness evidence (#3608).
            context.harness_targets = rust_index::validated_file_wide_harness_targets(
                &options.root,
                &options.test_harnesses,
            );
            context
        };
        let production_files = analyzable_rust_files
            .iter()
            .filter(|path| {
                workspace::classify_with(path, &repo_source_role_context)
                    .seeds_production_findings()
            })
            .cloned()
            .collect::<Vec<_>>();

        // Index all discovered Rust files (production + tests + benches +
        // examples). The classifier's `find_related_tests` looks up tests
        // in the index; without test files the repo headline silently
        // inflates `no_static_path` for owners that *are* exercised by
        // integration tests under `tests/` or `examples/`. Probe seeding
        // stays production-only so test bodies do not generate findings.
        // Use the content-addressed per-file fact cache (#1912).
        let loaded_rust_files = analyzable_rust_files
            .iter()
            .map(|file| {
                let full = options.root.join(file);
                let bytes = std::fs::read(&full)
                    .map_err(|err| format!("failed to read {}: {err}", full.display()))?;
                Ok((file.clone(), bytes))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let cached = rust_index::build_index_from_loaded_files_with_cache_and_test_harnesses(
            &options.root,
            &loaded_rust_files,
            &options.test_harnesses,
        )?;
        let mut index = cached.index;
        if let Some(disclosure) = rust_index::lexical_fallback_disclosure(&index) {
            eprintln!("{disclosure}");
        }
        if let Some(disclosure) = rust_index::include_resolution_disclosure(&index) {
            eprintln!("{disclosure}");
        }
        if let Some(disclosure) = rust_index::module_composition_disclosure(&index) {
            eprintln!("{disclosure}");
        }
        rust_index::apply_oracle_policy(&mut index, oracle_policy);

        let mut findings = Vec::new();

        // #2972: one path-dependency edge context per repo pass. Repo mode
        // indexes the whole workspace, so the admit precondition holds by
        // construction here.
        let manifest_dir_prefixes =
            crate::analysis::seam_cache::workspace_manifest_dir_prefixes(&options.root);
        let dependency_adjacency = workspace::PathDependencyAdjacency::build(
            &crate::analysis::seam_cache::workspace_graph_provenance(&options.root),
        );
        let dependency_edges = classify::DependencyEdgeContext {
            adjacency: &dependency_adjacency,
            manifest_dir_prefixes: &manifest_dir_prefixes,
            index: &index,
        };

        for path in &production_files {
            let probes = probes::probes_for_repo_file(&options.root, path, &index);
            for probe in probes {
                let mut finding =
                    classifier::classify_probe(&probe, &index, true, Some(&dependency_edges));
                finding.language = Some(LanguageId::Rust);
                // Repo mode seeds probes from the current tree, so every
                // finding's source is candidate-side by construction
                // (#3280).
                finding.source_currentness = SourceCurrentness::CandidateCurrent;
                // `language_status` is omitted for Rust per RIPR-SPEC-0026.
                // RIPR-SPEC-0114 + 0115 + 0117: no_static_path limitation
                // disclosure for repo-mode (same logic as diff-mode).
                apply_rust_no_static_path_limit(&mut finding, &probe, &index);
                apply_rust_macro_wrapped_assertion_limit(&mut finding, &index);
                apply_rust_value_propagation_limit(&mut finding, &probe, &index);
                apply_wrapper_error_binding_limit(&mut finding, &probe);
                // Fail closed on cross-language seams (#910).
                if let Some(limit) = cross_language_limit_kind(&probe, &index, &finding.class) {
                    finding.static_limit_kind = Some(limit);
                }
                findings.push(finding);
            }
        }

        Ok(LanguageRepoResult {
            findings,
            harness_projections: super::super::harness_projection::projections_from_index(
                &index,
                &options.test_harnesses,
            ),
            production_files: production_files.len(),
            skipped_files,
            partial_reason: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DIFF_CHANGED_RUST_LINE_LIMIT, DIFF_INDEX_FILE_LIMIT, PARTIAL_DIFF_FILE_BUDGET_DEFAULT,
        PARTIAL_DIFF_FILE_BUDGET_ENV, PARTIAL_DIFF_LANGUAGE_TIER_VERSION,
        PARTIAL_DIFF_LINE_BUDGET_DEFAULT, PARTIAL_DIFF_LINE_BUDGET_ENV,
        PARTIAL_DIFF_SELECTION_VERSION, PartialDiffBudgets, PartialDiffScope,
        PartialDiffStopReason, REPO_INDEX_FILE_LIMIT_ENV, RustAdapter,
        apply_rust_macro_wrapped_assertion_limit, changed_rust_line_count,
        cross_language_limit_kind, diff_changed_rust_line_limit_from_env,
        diff_identity_from_changed_files, diff_index_file_limit_from_env,
        enforce_changed_rust_line_limit, enforce_repo_index_file_limit, is_binary_source_path,
        is_cargo_binary_invocation, is_generated_rust_file, is_generated_rust_file_with_patterns,
        macro_reach_limit_kind, owner_has_ffi_attr, partial_diff_budgets_from_env,
        partition_canonical_form, replace_witnessed_no_path_infection_summary,
        repo_index_file_limit_from_env, select_partial_diff_partition,
        select_partial_diff_partition_with_identity, sha256_hex, transitive_reach_limit_kind,
    };
    use crate::analysis::cancellation;
    use crate::analysis::diff::{ChangedFile, ChangedLine};
    use crate::analysis::facts::FunctionSourceRole;
    use crate::analysis::facts::{
        CallFact, FunctionSummary, LiteralFact, RustIndex, TestFact, TestSummary,
    };
    use crate::analysis::language::{LanguageAdapter, LanguageId};
    use crate::analysis::{AnalysisMode, AnalysisOptions, diff};
    use crate::config::OraclePolicy;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, Probe, ProbeFamily,
        ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence,
        StageState, StaticLimitKind, SymbolId,
    };
    use std::env::VarError;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("ripr-rust-adapter-{name}-{stamp}"));
        fs::create_dir_all(&root).map_err(|err| format!("create temp root failed: {err}"))?;
        Ok(root)
    }

    fn write(path: &Path, text: &str) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("create parent failed: {err}"))?;
        }
        fs::write(path, text).map_err(|err| format!("write {} failed: {err}", path.display()))
    }

    #[test]
    fn diff_analysis_indexes_changed_tests_without_probing_them() -> Result<(), String> {
        assert!(!crate::analysis::rust_index::is_test_file(Path::new(
            "src/test_helper.rs"
        )));

        let root = temp_root("changed-tests-are-evidence")?;
        write(
            &root.join("Cargo.toml"),
            "[package]\nname='probe-authority'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        write(
            &root.join("src/lib.rs"),
            "pub fn gate_state(flag: bool) -> bool {\n    if flag { true } else { false }\n}\n",
        )?;
        write(
            &root.join("src/tests/gate_state_tests.rs"),
            "#[test]\nfn exact_gate_state() {\n    assert_eq!(gate_state(true), true);\n}\n",
        )?;
        let changed_files = diff::parse_unified_diff(
            "diff --git a/src/lib.rs b/src/lib.rs\n\
             new file mode 100644\n\
             --- /dev/null\n\
             +++ b/src/lib.rs\n\
             @@ -0,0 +1,3 @@\n\
             +pub fn gate_state(flag: bool) -> bool {\n\
             +    if flag { true } else { false }\n\
             +}\n\
             diff --git a/src/tests/gate_state_tests.rs b/src/tests/gate_state_tests.rs\n\
             new file mode 100644\n\
             --- /dev/null\n\
             +++ b/src/tests/gate_state_tests.rs\n\
             @@ -0,0 +1,4 @@\n\
             +#[test]\n\
             +fn exact_gate_state() {\n\
             +    assert_eq!(gate_state(true), true);\n\
             +}\n",
        );

        let result = RustAdapter.analyze_diff(
            &AnalysisOptions {
                root,
                base: None,
                diff_file: None,
                mode: AnalysisMode::Ready,
                resolved_subject_identity: None,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &changed_files,
        )?;

        assert_eq!(
            result.changed_files, 2,
            "changed-file accounting must retain the test file"
        );
        assert!(
            result.findings.iter().all(|finding| {
                !finding
                    .probe
                    .location
                    .file
                    .to_string_lossy()
                    .replace('\\', "/")
                    .contains("/tests/")
            }),
            "test code must not become a production probe: {:?}",
            result.findings
        );
        assert!(
            result.findings.iter().any(|finding| {
                finding.related_tests.iter().any(|test| {
                    test.file
                        .to_string_lossy()
                        .replace('\\', "/")
                        .ends_with("src/tests/gate_state_tests.rs")
                })
            }),
            "changed test must remain indexed as related evidence: {:?}",
            result.findings
        );
        Ok(())
    }

    // --- #2970 slice C: path-dependency diff-scope expansion ---

    fn diff_options(root: PathBuf, mode: AnalysisMode) -> AnalysisOptions {
        AnalysisOptions {
            root,
            base: None,
            diff_file: None,
            mode,
            resolved_subject_identity: None,
            include_unchanged_tests: true,
            resolve_tsconfig_paths: false,
            perl_facts_path: None,
            git_timeout: None,
            git_candidate: None,
            production_like_targets: Default::default(),
            test_harnesses: Vec::new(),
        }
    }

    /// Writes the a <- b <- c path-dep workspace: `b` declares a path
    /// dependency on `a`, `c` declares one on `b`, and `b`'s integration test
    /// calls `a`'s changed owner. `with_edge = false` removes `b`'s edge and
    /// is the over-reach discriminator: scope must then stay `a`-only.
    fn write_path_dep_workspace(root: &Path, with_edge: bool) -> Result<(), String> {
        write(
            &root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"a\", \"b\", \"c\"]\nresolver = \"2\"\n",
        )?;
        write(
            &root.join("a/Cargo.toml"),
            "[package]\nname = \"scope_a\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )?;
        let b_dependencies = if with_edge {
            "\n[dependencies]\nscope_a = { path = \"../a\" }\n"
        } else {
            ""
        };
        write(
            &root.join("b/Cargo.toml"),
            &format!(
                "[package]\nname = \"scope_b\"\nversion = \"0.1.0\"\nedition = \"2024\"\n{b_dependencies}"
            ),
        )?;
        write(
            &root.join("c/Cargo.toml"),
            "[package]\nname = \"scope_c\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
             [dependencies]\nscope_b = { path = \"../b\" }\n",
        )?;
        write(
            &root.join("a/src/lib.rs"),
            "pub fn quarble_gauge(flag: bool) -> bool {\n    if flag { true } else { false }\n}\n",
        )?;
        write(
            &root.join("b/src/lib.rs"),
            "pub fn relay(flag: bool) -> bool {\n    scope_a::quarble_gauge(flag)\n}\n",
        )?;
        write(
            &root.join("b/tests/quarble_gauge_tests.rs"),
            "#[test]\nfn quarble_gauge_holds() {\n    assert!(scope_a::quarble_gauge(true));\n}\n",
        )?;
        write(
            &root.join("c/src/lib.rs"),
            "pub fn forward(flag: bool) -> bool {\n    scope_b::relay(flag)\n}\n",
        )
    }

    fn changed_a_lib_diff() -> Vec<ChangedFile> {
        diff::parse_unified_diff(
            "diff --git a/a/src/lib.rs b/a/src/lib.rs\n\
             new file mode 100644\n\
             --- /dev/null\n\
             +++ b/a/src/lib.rs\n\
             @@ -0,0 +1,4 @@\n\
             +pub fn quarble_gauge(flag: bool) -> bool {\n\
             +    if flag { true } else { false }\n\
             +}\n",
        )
    }

    fn has_related_test(findings: &[Finding], test_file_suffix: &str) -> bool {
        findings.iter().any(|finding| {
            finding.related_tests.iter().any(|test| {
                test.file
                    .to_string_lossy()
                    .replace('\\', "/")
                    .ends_with(test_file_suffix)
            })
        })
    }

    fn has_related_test_in_b(findings: &[Finding]) -> bool {
        has_related_test(findings, "b/tests/quarble_gauge_tests.rs")
    }

    /// The slice C contract end to end: a Draft-mode diff that touches only
    /// crate `a` brings the tests of path-dependents `b` and `c` into scope,
    /// so `b`'s integration test that calls the changed owner is credited as
    /// related evidence. The reach must come from the dependency edge: a
    /// swapped forward/reverse adjacency fails here, because `a` declares no
    /// dependencies, so a forward walk from `a` reaches nothing and `b`'s
    /// test never enters the index.
    #[test]
    fn draft_diff_scope_reaches_path_dependent_tests_through_the_dependency_edge()
    -> Result<(), String> {
        let root = temp_root("path-dep-scope-reach")?;
        write_path_dep_workspace(&root, true)?;
        let changed_files = changed_a_lib_diff();

        let result = RustAdapter.analyze_diff(
            &diff_options(root, AnalysisMode::Draft),
            &OraclePolicy::default(),
            &changed_files,
        )?;

        assert!(
            !result.findings.is_empty(),
            "the changed owner must seed probes: {:?}",
            result.findings
        );
        assert!(
            has_related_test_in_b(&result.findings),
            "the dependent crate's test must become reachable related evidence: {:?}",
            result.findings
        );
        Ok(())
    }

    /// Over-reach discriminator: the same workspace without `b`'s dependency
    /// edge must not reach `b`. The index stays `a`-only, is therefore not
    /// workspace-complete, and the cross-crate test stays out fail-closed.
    #[test]
    fn draft_diff_scope_stays_narrow_without_the_path_dependency_edge() -> Result<(), String> {
        let root = temp_root("path-dep-scope-no-edge")?;
        write_path_dep_workspace(&root, false)?;
        let changed_files = changed_a_lib_diff();

        let result = RustAdapter.analyze_diff(
            &diff_options(root, AnalysisMode::Draft),
            &OraclePolicy::default(),
            &changed_files,
        )?;

        assert!(
            !result.findings.is_empty(),
            "the changed owner must seed probes: {:?}",
            result.findings
        );
        assert!(
            !has_related_test_in_b(&result.findings),
            "without the dependency edge the dependent test stays out of scope: {:?}",
            result.findings
        );
        Ok(())
    }

    /// Instant stays changed-files-only even with the dependency edge: the
    /// expansion participates only in the package-narrowing selections.
    #[test]
    fn instant_mode_does_not_expand_scope_through_path_dependencies() -> Result<(), String> {
        let root = temp_root("path-dep-scope-instant")?;
        write_path_dep_workspace(&root, true)?;
        let changed_files = changed_a_lib_diff();

        let result = RustAdapter.analyze_diff(
            &diff_options(root, AnalysisMode::Instant),
            &OraclePolicy::default(),
            &changed_files,
        )?;

        assert!(
            !result.findings.is_empty(),
            "the changed owner must seed probes: {:?}",
            result.findings
        );
        assert!(
            !has_related_test_in_b(&result.findings),
            "Instant stays changed-files-only; the dependent test stays out: {:?}",
            result.findings
        );
        Ok(())
    }

    /// #3616 review fix 1 end to end: a Draft diff that touches only a
    /// custom-target file (`[lib] path = "lib/core.rs"`, no heuristic
    /// package root) still expands its crate's path dependents. The
    /// manifest-inventory attribution seeds the owning package, the index
    /// spans the whole two-crate workspace, and the dependent's integration
    /// test is credited as related evidence through the #2971 uniqueness
    /// bypass. Without the attribution the index stays at the changed file
    /// alone and the dependent test stays out fail-closed. The supported
    /// production-like opt-in (#3283) is what makes the custom-target file
    /// seed probes at all.
    #[test]
    fn draft_diff_scope_expands_custom_target_files_to_their_path_dependents() -> Result<(), String>
    {
        let root = temp_root("path-dep-scope-custom-target")?;
        write(
            &root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"t\", \"u\"]\nresolver = \"2\"\n",
        )?;
        write(
            &root.join("t/Cargo.toml"),
            "[package]\nname = \"scope_t\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
             [lib]\npath = \"lib/core.rs\"\n",
        )?;
        write(
            &root.join("u/Cargo.toml"),
            "[package]\nname = \"scope_u\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
             [dependencies]\nscope_t = { path = \"../t\" }\n",
        )?;
        write(
            &root.join("t/lib/core.rs"),
            "pub fn quarble_gauge(flag: bool) -> bool {\n    if flag { true } else { false }\n}\n",
        )?;
        write(
            &root.join("u/src/lib.rs"),
            "pub fn relay(flag: bool) -> bool {\n    scope_t::quarble_gauge(flag)\n}\n",
        )?;
        write(
            &root.join("u/tests/quarble_gauge_tests.rs"),
            "#[test]\nfn quarble_gauge_holds() {\n    assert!(scope_t::quarble_gauge(true));\n}\n",
        )?;
        let changed_files = diff::parse_unified_diff(
            "diff --git a/t/lib/core.rs b/t/lib/core.rs\n\
             new file mode 100644\n\
             --- /dev/null\n\
             +++ b/t/lib/core.rs\n\
             @@ -0,0 +1,4 @@\n\
             +pub fn quarble_gauge(flag: bool) -> bool {\n\
             +    if flag { true } else { false }\n\
             +}\n",
        );
        let mut production_like_targets = std::collections::BTreeSet::new();
        production_like_targets.insert(PathBuf::from("t/lib/core.rs"));
        let options = AnalysisOptions {
            production_like_targets,
            ..diff_options(root, AnalysisMode::Draft)
        };

        let result =
            RustAdapter.analyze_diff(&options, &OraclePolicy::default(), &changed_files)?;

        assert!(
            !result.findings.is_empty(),
            "the opted-in custom-target owner must seed probes: {:?}",
            result.findings
        );
        assert!(
            has_related_test(&result.findings, "u/tests/quarble_gauge_tests.rs"),
            "the dependent crate's test must become reachable through the attributed scope: {:?}",
            result.findings
        );
        Ok(())
    }

    #[test]
    fn diff_analysis_skips_inline_cfg_test_helpers_but_keeps_production_controls()
    -> Result<(), String> {
        let root = temp_root("inline-cfg-test-second-role")?;
        write(
            &root.join("Cargo.toml"),
            "[package]\nname='inline-cfg-test-role'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        write(
            &root.join("src/lib.rs"),
            "pub fn production_control(value: i32) -> Result<i32, String> {\n    if value < 0 { return Err(\"negative\".to_string()); }\n    Ok(value)\n}\n\n#[cfg(all(feature = \"slow\", test))]\nmod tests {\n    fn helper_returns_result(value: i32) -> Result<(), String> {\n        if value < 0 { return Err(\"negative\".to_string()); }\n        Ok(())\n    }\n\n    #[test]\n    fn equivalent_assertion() {\n        if helper_returns_result(1).is_err() { return; }\n    }\n}\n",
        )?;
        write(
            &root.join("src/test_helper.rs"),
            "pub fn production_helper(value: i32) -> i32 {\n    if value < 0 { 0 } else { value }\n}\n",
        )?;
        let changed_files = diff::parse_unified_diff(
            "diff --git a/src/lib.rs b/src/lib.rs\nnew file mode 100644\n--- /dev/null\n+++ b/src/lib.rs\n@@ -0,0 +1,18 @@\n+pub fn production_control(value: i32) -> Result<i32, String> {\n+    if value < 0 { return Err(\"negative\".to_string()); }\n+    Ok(value)\n+}\n+\n+#[cfg(all(feature = \"slow\", test))]\n+mod tests {\n+    fn helper_returns_result(value: i32) -> Result<(), String> {\n+        if value < 0 { return Err(\"negative\".to_string()); }\n+        Ok(())\n+    }\n+\n+    #[test]\n+    fn equivalent_assertion() {\n+        if helper_returns_result(1).is_err() { return; }\n+    }\n+}\n\n diff --git a/src/test_helper.rs b/src/test_helper.rs\nnew file mode 100644\n--- /dev/null\n+++ b/src/test_helper.rs\n@@ -0,0 +1,3 @@\n+pub fn production_helper(value: i32) -> i32 {\n+    if value < 0 { 0 } else { value }\n+}\n",
        );

        let result = RustAdapter.analyze_diff(
            &AnalysisOptions {
                root,
                base: None,
                diff_file: None,
                mode: AnalysisMode::Ready,
                resolved_subject_identity: None,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &changed_files,
        )?;

        assert!(
            result.findings.iter().any(|finding| {
                finding
                    .probe
                    .owner
                    .as_ref()
                    .is_some_and(|owner| owner.0.contains("production_control"))
            }),
            "production control must remain a finding: {:?}",
            result.findings
        );
        assert!(
            result.findings.iter().any(|finding| {
                finding
                    .probe
                    .owner
                    .as_ref()
                    .is_some_and(|owner| owner.0.contains("production_helper"))
            }),
            "src/test_helper.rs must remain production by semantic role: {:?}",
            result.findings
        );
        assert!(
            result.findings.iter().all(|finding| {
                !finding
                    .probe
                    .owner
                    .as_ref()
                    .is_some_and(|owner| owner.0.contains("tests::helper_returns_result"))
            }),
            "test-second cfg(all(...)) helper must not become a production finding: {:?}",
            result.findings
        );
        Ok(())
    }

    // RIPR-SPEC-0153 / #3695: exercise the composer and the production diff
    // adapter together, with an eligible field in every input diff.
    #[test]
    fn diff_analysis_ownerless_fields_follow_resolved_context() -> Result<(), String> {
        for (name, declarations, child_path, test_only, extra_source) in [
            (
                "module",
                "#[cfg(test)] mod support;\n",
                "src/support.rs",
                true,
                None,
            ),
            (
                "literal-path",
                "#[cfg(test)] #[path = \"support/shared.rs\"] mod support;\n",
                "src/support/shared.rs",
                true,
                None,
            ),
            (
                "transitive",
                "#[cfg(test)] mod outer;\n",
                "src/outer/support.rs",
                true,
                Some(("src/outer.rs", "mod support;\n")),
            ),
            (
                "production",
                "mod support;\n",
                "src/support.rs",
                false,
                None,
            ),
            (
                "mixed",
                "#[cfg(test)] mod support;\ninclude!(\"support.rs\");\n",
                "src/support.rs",
                false,
                None,
            ),
            ("missing-parent", "", "src/support.rs", false, None),
            (
                "lexical-fallback",
                "#[cfg(test)] mod support;\n",
                "src/support.rs",
                false,
                None,
            ),
            (
                "unresolved-ancestor",
                "mod outer;\n#[path = \"outer.rs\"] mod other;\n",
                "src/outer/support.rs",
                false,
                Some(("src/outer.rs", "#[cfg(test)] mod support;\n")),
            ),
        ] {
            let root = temp_root(&format!("ownerless-fields-{name}"))?;
            write(
                &root.join("Cargo.toml"),
                "[package]\nname='ownerless-fields'\nversion='0.1.0'\nedition='2024'\n",
            )?;
            write(
                &root.join("src/lib.rs"),
                &format!("{declarations}mod production;\n"),
            )?;
            if let Some((path, source)) = extra_source {
                write(&root.join(path), source)?;
            }
            let mut diff_text = String::new();
            for path in [child_path, "src/production.rs"] {
                let source = if name == "lexical-fallback" && path == child_path {
                    "struct Counter {\n    allowed: usize,\n}\nfn incomplete(\n"
                } else {
                    "struct Counter {\n    allowed: usize,\n}\n"
                };
                if name == "lexical-fallback"
                    && path == child_path
                    && crate::analysis::rust_index::RustSyntaxAdapter::summarize_file(
                        &crate::analysis::rust_index::RaRustSyntaxAdapter,
                        &root.join(path),
                        source,
                    )
                    .is_ok()
                {
                    return Err("fallback fixture unexpectedly parsed".to_string());
                }
                write(&root.join(path), source)?;
                diff_text.push_str(&format!(
                    "diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1,2 +1,3 @@\n struct Counter {{\n+    allowed: usize,\n }}\n"
                ));
            }
            let changed_files = diff::parse_unified_diff(&diff_text);
            if changed_files.len() != 2
                || changed_files.iter().any(|file| file.added_lines.len() != 1)
            {
                return Err(format!("{name}: expected two changed field inputs"));
            }
            let result = RustAdapter.analyze_diff(
                &AnalysisOptions {
                    root: root.clone(),
                    base: None,
                    diff_file: None,
                    mode: AnalysisMode::Ready,
                    resolved_subject_identity: None,
                    include_unchanged_tests: true,
                    resolve_tsconfig_paths: false,
                    perl_facts_path: None,
                    git_timeout: None,
                    git_candidate: None,
                    production_like_targets: Default::default(),
                    test_harnesses: Vec::new(),
                },
                &OraclePolicy::default(),
                &changed_files,
            )?;
            let field_count = |path: &str| {
                let expected_path = root.join(path);
                result
                    .findings
                    .iter()
                    .filter(|finding| {
                        finding.probe.location.file == expected_path
                            && finding.probe.family == ProbeFamily::FieldConstruction
                    })
                    .count()
            };
            if field_count("src/production.rs") != 1 {
                return Err(format!(
                    "{name}: production field missing: {:?}",
                    result.findings
                ));
            }
            let expected_child_fields = usize::from(!test_only);
            if field_count(child_path) != expected_child_fields {
                return Err(format!(
                    "{name}: expected {expected_child_fields} child field findings: {:?}",
                    result.findings
                ));
            }
        }
        Ok(())
    }

    // Shared end-to-end shape for the #3271/#3294 binding-value family:
    // a small crate whose changed `let` initializer feeds an equality
    // predicate in the same function, with exact-value tests touching
    // the boundary from both sides.
    fn binding_value_crate(
        name: &str,
        changed_line_number: usize,
        old_line: &str,
        changed_line: &str,
        predicate_line: &str,
    ) -> Result<
        (
            std::path::PathBuf,
            super::super::adapter::LanguageDiffResult,
        ),
        String,
    > {
        let root = temp_root(name)?;
        write(
            &root.join("Cargo.toml"),
            "[package]\nname='value-propagation'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        let start_line = if changed_line_number == 2 {
            changed_line
        } else {
            "    let start = delim.chars().next().map_or(0, |ch| ch.len_utf8());"
        };
        let end_line = if changed_line_number == 3 {
            changed_line
        } else {
            "    let end = input.rfind(delim).map_or(0, |idx| idx);"
        };
        let source = format!(
            "pub fn split(input: &str, delim: &str) -> usize {{\n{start_line}\n{end_line}\n{predicate_line}\n    0\n}}\n"
        );
        write(&root.join("src/lib.rs"), &source)?;
        write(
            &root.join("tests/split.rs"),
            "#[test]\nfn empty_delimiter_splits_at_start() {\n    assert_eq!(split(\"abc\", \"\"), 1);\n}\n#[test]\nfn nonempty_delimiter_splits() {\n    assert_eq!(split(\"abc\", \"b\"), 0);\n}\n",
        )?;
        let diff_text = format!(
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -{changed_line_number},1 +{changed_line_number},1 @@\n-{old_line}\n+{changed_line}\n"
        );
        let changed_files = diff::parse_unified_diff(&diff_text);
        let result = RustAdapter.analyze_diff(
            &AnalysisOptions {
                root: root.clone(),
                base: None,
                diff_file: None,
                mode: AnalysisMode::Ready,
                resolved_subject_identity: None,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &changed_files,
        )?;
        Ok((root, result))
    }

    // #3294: a changed binding that reaches its same-function equality
    // predicate directly retargets the probe to the predicate. The
    // finding is predicate-shaped, the generic changed-syntax limitation
    // is gone, and the operand-value limitation names the earliest
    // initializer operation.
    #[test]
    fn diff_analysis_retargets_changed_binding_to_predicate_use() -> Result<(), String> {
        let (root, result) = binding_value_crate(
            "rfind-binding-predicate",
            3,
            "    let end = input.rfind(delim).map_or(0, |idx| idx);",
            "    let end = input.rfind(delim).map_or(1, |idx| idx);",
            "    if end == start { return 1; }",
        )?;
        let finding = result
            .findings
            .iter()
            .find(|finding| {
                finding.probe.family == ProbeFamily::Predicate
                    && finding.probe.expression.contains("end == start")
            })
            .ok_or_else(|| format!("missing retargeted finding: {:?}", result.findings))?;
        assert_eq!(finding.probe.location.line, 4, "probe sits on the use");
        assert_ne!(finding.class, ExposureClass::StaticUnknown);
        // #1429: the operand value is admitted-unresolved (see the
        // `binding_predicate_value_unresolved` evidence below), so the
        // missing discriminator is unconfirmed: the finding carries
        // the named limitation and withholds the boundary-test
        // prescription instead of asserting no limit.
        assert_eq!(
            finding.static_limit_kind,
            Some(StaticLimitKind::RustValuePropagationUnresolved),
            "unresolved operand must carry the named limitation: {:?}",
            finding.static_limit_kind
        );
        assert!(
            finding
                .recommended_next_step
                .as_deref()
                .is_some_and(|step| step.contains("rust_value_propagation_unresolved")
                    && !step.contains("Add boundary tests")),
            "no specific boundary input may be prescribed: {:?}",
            finding.recommended_next_step
        );
        assert!(
            finding.evidence.iter().any(|line| line
                .contains("binding_predicate_relation: changed binding `end` initializer")),
            "relation evidence missing: {:?}",
            finding.evidence
        );
        assert!(
            finding.evidence.iter().any(|line| line.contains(
                "binding_predicate_value_unresolved: operand value of `end` unresolved at earliest initializer operation `.rfind(`"
            )),
            "earliest operation evidence missing: {:?}",
            finding.evidence
        );
        assert!(
            !result.findings.iter().any(|finding| finding
                .evidence
                .iter()
                .any(|line| line.contains("not mapped to a high-confidence probe family"))),
            "the generic changed-syntax limitation must be absent: {:?}",
            result.findings
        );
        fs::remove_dir_all(root).map_err(|error| format!("remove fixture: {error}"))?;
        Ok(())
    }

    // #3294: the same retarget for the `len_utf8` operand binding.
    #[test]
    fn diff_analysis_retargets_len_utf8_binding_to_predicate_use() -> Result<(), String> {
        let (root, result) = binding_value_crate(
            "len-utf8-binding-predicate",
            2,
            "    let start = delim.chars().next().map_or(0, |ch| ch.len_utf8());",
            "    let start = delim.chars().next().map_or(1, |ch| ch.len_utf8());",
            "    if end == start { return 1; }",
        )?;
        let finding = result
            .findings
            .iter()
            .find(|finding| {
                finding.probe.family == ProbeFamily::Predicate
                    && finding.probe.expression.contains("end == start")
            })
            .ok_or_else(|| format!("missing retargeted finding: {:?}", result.findings))?;
        assert!(
            finding.evidence.iter().any(|line| line.contains(
                "binding_predicate_value_unresolved: operand value of `start` unresolved at earliest initializer operation `.chars(`"
            )),
            "earliest operation evidence missing: {:?}",
            finding.evidence
        );
        fs::remove_dir_all(root).map_err(|error| format!("remove fixture: {error}"))?;
        Ok(())
    }

    // #3294: an initializer ripr cannot evaluate at all (`input.len()`)
    // still retargets — the predicate is named, the operand value stays
    // unresolved at the earliest call.
    #[test]
    fn diff_analysis_retargets_unbounded_initializer_naming_earliest_call() -> Result<(), String> {
        let (root, result) = binding_value_crate(
            "unbounded-binding-predicate",
            3,
            "    let end = input.rfind(delim).map_or(0, |idx| idx);",
            "    let end = input.len();",
            "    if end == start { return 1; }",
        )?;
        let finding = result
            .findings
            .iter()
            .find(|finding| {
                finding.probe.family == ProbeFamily::Predicate
                    && finding.probe.expression.contains("end == start")
            })
            .ok_or_else(|| format!("missing retargeted finding: {:?}", result.findings))?;
        assert!(
            finding.evidence.iter().any(|line| line.contains(
                "binding_predicate_value_unresolved: operand value of `end` unresolved at earliest initializer operation `input.len(`"
            )),
            "earliest call evidence missing: {:?}",
            finding.evidence
        );
        fs::remove_dir_all(root).map_err(|error| format!("remove fixture: {error}"))?;
        Ok(())
    }

    // #3295: the exact test inputs evaluate the changed and sibling
    // bindings, so the equality boundary is observed (infection yes at
    // the changed boundary) instead of `observed end values: unknown`.
    #[test]
    fn diff_analysis_evaluates_exact_boundary_from_test_inputs() -> Result<(), String> {
        let root = temp_root("exact-boundary-evaluation")?;
        write(
            &root.join("Cargo.toml"),
            "[package]
name='value-propagation'
version='0.1.0'
edition='2024'
",
        )?;
        write(
            &root.join("src/lib.rs"),
            "pub fn split_after(input: &str, delim: char) -> &str {
    let end = input.rfind(delim).map_or(1, |idx| idx);
    let start = delim.len_utf8();
    if end == start {
        &input[..end]
    } else {
        input
    }
}
",
        )?;
        write(
            &root.join("tests/split.rs"),
            "use value_propagation::split_after;
#[test]
fn absent_delimiter_boundary_returns_head() {
    assert_eq!(split_after(\"ab\", 'x'), \"a\");
}
",
        )?;
        let diff_text = "diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -2,1 +2,1 @@
-    let end = input.rfind(delim).map_or(0, |idx| idx);
+    let end = input.rfind(delim).map_or(1, |idx| idx);
";
        let changed_files = diff::parse_unified_diff(diff_text);
        let result = RustAdapter.analyze_diff(
            &AnalysisOptions {
                root: root.clone(),
                base: None,
                diff_file: None,
                mode: AnalysisMode::Ready,
                resolved_subject_identity: None,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &changed_files,
        )?;
        let finding = result
            .findings
            .iter()
            .find(|finding| {
                finding.probe.family == ProbeFamily::Predicate
                    && finding.probe.expression.contains("end == start")
            })
            .ok_or_else(|| format!("missing retargeted finding: {:?}", result.findings))?;
        assert_eq!(
            finding.ripr.infect.state,
            StageState::Yes,
            "the exact inputs end=1, start=1 observe the boundary: {finding:?}"
        );
        assert!(
            finding
                .activation
                .observed_values
                .iter()
                .any(|fact| fact.value == "end == start"),
            "the boundary equality must be an observed value: {:?}",
            finding.activation.observed_values
        );
        assert!(
            finding
                .activation
                .missing_discriminators
                .iter()
                .all(|fact| fact.value != "end == start"),
            "the boundary discriminator is no longer missing: {:?}",
            finding.activation.missing_discriminators
        );
        fs::remove_dir_all(root).map_err(|error| format!("remove fixture: {error}"))?;
        Ok(())
    }

    // #3271 stays the fallback for shapes #3294 cannot relate: a
    // predicate use behind a macro invocation is blocked by the
    // relation, so the generic static-unknown finding keeps the
    // value-propagation limitation.
    #[test]
    fn diff_analysis_keeps_value_propagation_limitation_for_macro_guarded_use() -> Result<(), String>
    {
        let (root, result) = binding_value_crate(
            "macro-guarded-value-limit",
            3,
            "    let end = input.rfind(delim).map_or(0, |idx| idx);",
            "    let end = input.rfind(delim).map_or(1, |idx| idx);",
            "    ensure!(end == start);",
        )?;
        let finding = result
            .findings
            .iter()
            .find(|finding| finding.probe.expression.contains("let end"))
            .ok_or_else(|| format!("missing changed binding finding: {:?}", result.findings))?;
        assert_eq!(finding.class, ExposureClass::StaticUnknown);
        assert_eq!(
            finding
                .static_limit_kind
                .as_ref()
                .map(StaticLimitKind::as_str),
            Some("rust_value_propagation_unresolved")
        );
        assert!(
            finding
                .evidence
                .iter()
                .any(|line| line.contains("analysis/rust-value-propagation"))
        );
        fs::remove_dir_all(root).map_err(|error| format!("remove fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn value_propagation_predicate_ignores_non_entity_text_and_shadowing() {
        let body = r#"
    let end = input.rfind(delim).map_or(0, |idx| idx);
    let copied = end;
    // end == start is only documentation.
    let text = "end == start";
    if other.end == start { return 0; }
    let end = 1;
    if end == start { return 1; }
"#;
        assert!(super::find_value_propagation_predicate(body, "end").is_none());
    }

    #[test]
    fn value_propagation_predicate_rejects_mixed_operator_line() {
        let body = "    let end = input.rfind(delim).map_or(0, |idx| idx);\n    if end != start && other == marker { return 0; }\n";
        assert!(super::find_value_propagation_predicate(body, "end").is_none());
    }

    #[test]
    fn value_propagation_predicate_rejects_binding_outside_equality_operand() {
        let body = "    let end = input.rfind(delim).map_or(0, |idx| idx);\n    if other == marker && end > 0 { return 0; }\n";
        assert!(super::find_value_propagation_predicate(body, "end").is_none());
    }

    #[test]
    fn value_propagation_predicate_rejects_map_or_else_shape() {
        let rhs =
            super::mask_rust_comments_and_strings("input.find(delim).map_or_else(|| 0, |idx| idx)");
        assert!(!rhs.contains(".map_or("));
    }

    #[test]
    fn diff_index_file_limit_defaults_when_unset() {
        assert_eq!(
            diff_index_file_limit_from_env(Err(VarError::NotPresent)),
            Ok(DIFF_INDEX_FILE_LIMIT)
        );
    }

    #[test]
    fn diff_index_file_limit_parses_positive_override() {
        assert_eq!(
            diff_index_file_limit_from_env(Ok("  50 ".to_string())),
            Ok(50)
        );
    }

    fn rejection_message(value: &str) -> String {
        match diff_index_file_limit_from_env(Ok(value.to_string())) {
            Ok(parsed) => format!("expected rejection of {value:?}, got Ok({parsed})"),
            Err(message) => message,
        }
    }

    #[test]
    fn diff_index_file_limit_rejects_zero() {
        let message = rejection_message("0");
        assert!(message.contains("positive integer"), "got: {message}");
    }

    #[test]
    fn diff_index_file_limit_rejects_non_numeric() {
        let message = rejection_message("lots");
        assert!(message.contains("positive integer"), "got: {message}");
    }

    #[test]
    fn diff_index_file_limit_rejects_non_unicode() {
        let result = diff_index_file_limit_from_env(Err(VarError::NotUnicode("x".into())));
        assert!(
            matches!(&result, Err(err) if err.contains("valid UTF-8")),
            "non-unicode must error with a UTF-8 message, got {result:?}"
        );
    }

    #[test]
    fn diff_changed_rust_line_limit_defaults_when_unset() -> Result<(), String> {
        let parsed = diff_changed_rust_line_limit_from_env(Err(VarError::NotPresent))?;
        if parsed != DIFF_CHANGED_RUST_LINE_LIMIT {
            return Err(format!(
                "expected default {DIFF_CHANGED_RUST_LINE_LIMIT}, got {parsed}"
            ));
        }
        Ok(())
    }

    #[test]
    fn diff_changed_rust_line_limit_parses_positive_override() -> Result<(), String> {
        let parsed = diff_changed_rust_line_limit_from_env(Ok("  1500 ".to_string()))?;
        if parsed != 1500 {
            return Err(format!("expected parsed limit 1500, got {parsed}"));
        }
        Ok(())
    }

    #[test]
    fn changed_rust_line_count_ignores_non_rust_paths() -> Result<(), String> {
        let files = vec![
            changed_file("src/lib.rs", 2, 1),
            changed_file("tests/example.test.ts", 30, 30),
        ];

        let count = changed_rust_line_count(&files);
        if count != 3 {
            return Err(format!(
                "expected only Rust changed lines to count, got {count}"
            ));
        }
        Ok(())
    }

    #[test]
    fn changed_rust_line_limit_rejects_oversized_diff_before_probe_expansion() -> Result<(), String>
    {
        let files = vec![changed_file("src/lib.rs", 2, 1)];

        let message = match enforce_changed_rust_line_limit(&files, 2) {
            Ok(()) => return Err("three changed Rust lines should exceed limit two".to_string()),
            Err(message) => message,
        };

        for needle in [
            "diff_scope_oversized",
            "3 changed Rust lines across 1 Rust files",
            "RIPR_MAX_DIFF_CHANGED_RUST_LINES",
            "split the extraction PR",
        ] {
            if !message.contains(needle) {
                return Err(format!("missing `{needle}` in message: {message}"));
            }
        }
        Ok(())
    }

    #[test]
    fn changed_rust_line_limit_accepts_at_limit() -> Result<(), String> {
        let files = vec![changed_file("src/lib.rs", 1, 1)];
        enforce_changed_rust_line_limit(&files, 2)
    }

    // --- Partial diff-scope partition tests (RIPR-PROP-0019, #1999) ---

    /// Every language selectable: the default for selection-unit tests that
    /// do not exercise the enabled-language filter.
    const ALL_LANGUAGES: &[LanguageId] = &[
        LanguageId::Rust,
        LanguageId::TypeScript,
        LanguageId::JavaScript,
        LanguageId::Python,
        LanguageId::Perl,
    ];

    fn budgets(file_budget: usize, line_budget: usize) -> PartialDiffBudgets {
        PartialDiffBudgets {
            file_budget,
            line_budget,
            disclosures: Vec::new(),
        }
    }

    fn require_partial(
        scope: Option<PartialDiffScope>,
        label: &str,
    ) -> Result<PartialDiffScope, String> {
        scope.ok_or_else(|| format!("expected a partial partition for {label}"))
    }

    #[test]
    fn partial_selection_is_deterministic_across_diff_orderings() -> Result<(), String> {
        // Same changed files, two different diff orderings: the partition and
        // its identity must not depend on diff ordering.
        let forward = vec![
            changed_file("crates/b/src/x.rs", 5, 0),
            changed_file("src/a.rs", 5, 0),
            changed_file("crates/a/src/y.rs", 5, 0),
        ];
        let reversed = vec![
            changed_file("crates/a/src/y.rs", 5, 0),
            changed_file("src/a.rs", 5, 0),
            changed_file("crates/b/src/x.rs", 5, 0),
        ];

        let first = require_partial(
            select_partial_diff_partition(&forward, &budgets(2, 100), ALL_LANGUAGES),
            "forward ordering",
        )?;
        let second = require_partial(
            select_partial_diff_partition(&reversed, &budgets(2, 100), ALL_LANGUAGES),
            "reversed ordering",
        )?;

        assert_eq!(first, second, "partition must be ordering-independent");
        assert_eq!(
            first.selected_files,
            vec!["src/a.rs".to_string(), "crates/a/src/y.rs".to_string()],
            "selection order: package path ascending, then file path ascending"
        );
        Ok(())
    }

    #[test]
    fn partial_selection_orders_rust_before_preview_then_package_then_path() -> Result<(), String> {
        let files = vec![
            changed_file("app/z.ts", 4, 0),
            changed_file("crates/b/src/x.rs", 4, 0),
            changed_file("app/a.ts", 4, 0),
            changed_file("crates/a/src/y.rs", 4, 0),
            changed_file("src/a.rs", 4, 0),
        ];

        let scope = require_partial(
            select_partial_diff_partition(&files, &budgets(3, 1_000), ALL_LANGUAGES),
            "tier ordering",
        )?;

        assert_eq!(
            scope.selected_files,
            vec![
                "src/a.rs".to_string(),
                "crates/a/src/y.rs".to_string(),
                "crates/b/src/x.rs".to_string(),
            ],
            "supported-language files first (package then path ascending); preview files after"
        );
        assert_eq!(scope.stop_reason, PartialDiffStopReason::FileBudget);
        assert_eq!(scope.uninspected_files_lower_bound, 2);
        assert_eq!(scope.uninspected_changed_lines_lower_bound, 8);
        Ok(())
    }

    #[test]
    fn partial_file_budget_stop_reports_exact_paths_and_lower_bounds() -> Result<(), String> {
        let files = vec![
            changed_file("src/d.rs", 10, 0),
            changed_file("src/a.rs", 10, 0),
            changed_file("src/c.rs", 10, 0),
            changed_file("src/b.rs", 10, 0),
        ];

        let scope = require_partial(
            select_partial_diff_partition(&files, &budgets(2, 1_000), ALL_LANGUAGES),
            "file budget stop",
        )?;

        assert_eq!(
            scope.selected_files,
            vec!["src/a.rs".to_string(), "src/b.rs".to_string()]
        );
        assert_eq!(scope.selected_changed_lines, 20);
        assert_eq!(scope.stop_reason, PartialDiffStopReason::FileBudget);
        assert_eq!(scope.uninspected_files_lower_bound, 2);
        assert_eq!(scope.uninspected_changed_lines_lower_bound, 20);
        assert_eq!(scope.file_budget, 2);
        assert_eq!(scope.line_budget, 1_000);
        assert_eq!(scope.run_status, PartialDiffScope::RUN_STATUS);
        Ok(())
    }

    #[test]
    fn partial_line_budget_stop_excludes_later_file_without_overshoot() -> Result<(), String> {
        let files = vec![
            changed_file("src/a.rs", 60, 0),
            changed_file("src/b.rs", 50, 0),
            changed_file("src/c.rs", 10, 0),
        ];

        let scope = require_partial(
            select_partial_diff_partition(&files, &budgets(10, 100), ALL_LANGUAGES),
            "line budget stop",
        )?;

        assert_eq!(scope.selected_files, vec!["src/a.rs".to_string()]);
        assert_eq!(scope.selected_changed_lines, 60);
        assert!(
            scope.selected_changed_lines <= scope.line_budget,
            "a later whole file must be excluded, never included with overshoot"
        );
        assert_eq!(scope.stop_reason, PartialDiffStopReason::LineBudget);
        assert_eq!(scope.uninspected_files_lower_bound, 2);
        assert_eq!(scope.uninspected_changed_lines_lower_bound, 60);
        Ok(())
    }

    #[test]
    fn partial_first_file_oversized_exception_analyzes_exactly_one_file() -> Result<(), String> {
        let files = vec![
            changed_file("src/a.rs", 150, 0),
            changed_file("src/b.rs", 10, 0),
        ];

        let scope = require_partial(
            select_partial_diff_partition(&files, &budgets(5, 100), ALL_LANGUAGES),
            "first-file exception",
        )?;

        assert_eq!(
            scope.selected_files,
            vec!["src/a.rs".to_string()],
            "the first oversized file is analyzed anyway — never an empty partition"
        );
        assert_eq!(scope.selected_changed_lines, 150);
        assert_eq!(
            scope.stop_reason,
            PartialDiffStopReason::LineBudgetExceededOnFirstFile
        );
        assert_eq!(scope.uninspected_files_lower_bound, 1);
        assert_eq!(scope.uninspected_changed_lines_lower_bound, 10);
        Ok(())
    }

    #[test]
    fn partial_simultaneous_hit_reports_file_budget_with_line_count() -> Result<(), String> {
        // Selecting the second file would both exceed the file budget and
        // overshoot the remaining line budget: the stop reason is the file
        // budget, with the line count recorded on the scope record.
        let files = vec![
            changed_file("src/a.rs", 60, 0),
            changed_file("src/b.rs", 60, 0),
        ];

        let scope = require_partial(
            select_partial_diff_partition(&files, &budgets(1, 100), ALL_LANGUAGES),
            "simultaneous hit",
        )?;

        assert_eq!(scope.selected_files, vec!["src/a.rs".to_string()]);
        assert_eq!(scope.stop_reason, PartialDiffStopReason::FileBudget);
        assert_eq!(scope.selected_changed_lines, 60);
        assert_eq!(scope.uninspected_files_lower_bound, 1);
        assert_eq!(scope.uninspected_changed_lines_lower_bound, 60);
        Ok(())
    }

    #[test]
    fn partial_first_file_exception_wins_over_simultaneous_hit() -> Result<(), String> {
        // file_budget=1 means the second file would hit both budgets, but the
        // FIRST file alone exceeds the line budget: the exception always wins.
        let files = vec![
            changed_file("src/a.rs", 60, 0),
            changed_file("src/b.rs", 10, 0),
        ];

        let scope = require_partial(
            select_partial_diff_partition(&files, &budgets(1, 50), ALL_LANGUAGES),
            "first-file exception precedence",
        )?;

        assert_eq!(scope.selected_files, vec!["src/a.rs".to_string()]);
        assert_eq!(
            scope.stop_reason,
            PartialDiffStopReason::LineBudgetExceededOnFirstFile,
            "first-file overshoot wins regardless of the file-budget state"
        );
        Ok(())
    }

    #[test]
    fn partial_context_only_files_are_never_selected_or_budgeted() -> Result<(), String> {
        let files = vec![
            changed_file("src/context.rs", 0, 0),
            changed_file("src/a.rs", 10, 0),
            changed_file("src/b.rs", 10, 0),
        ];

        let scope = require_partial(
            select_partial_diff_partition(&files, &budgets(1, 1_000), ALL_LANGUAGES),
            "context-only exclusion",
        )?;

        assert_eq!(scope.selected_files, vec!["src/a.rs".to_string()]);
        assert_eq!(
            scope.uninspected_files_lower_bound, 1,
            "the context-only file is not counted as uninspected changed-line scope"
        );
        assert_eq!(scope.uninspected_changed_lines_lower_bound, 10);

        // A diff with only context-only files fits every budget: no partial.
        let context_only = vec![changed_file("src/context.rs", 0, 0)];
        assert!(
            select_partial_diff_partition(&context_only, &budgets(1, 1), ALL_LANGUAGES).is_none()
        );
        Ok(())
    }

    #[test]
    fn partial_selection_never_selects_disabled_preview_files() -> Result<(), String> {
        // Enabled set is Rust-only: a preview-only over-budget diff must not
        // fabricate a limited_partial_scope run advertising inspected paths
        // no enabled adapter will inspect (#2142 review).
        let files = vec![
            changed_file("app/a.ts", 4, 0),
            changed_file("app/b.ts", 4, 0),
            changed_file("app/c.ts", 4, 0),
        ];
        assert!(
            select_partial_diff_partition(&files, &budgets(2, 1_000), &[LanguageId::Rust])
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn partial_selection_counts_disabled_preview_files_as_uninspected() -> Result<(), String> {
        // Rust enabled, TypeScript disabled: the partition selects only Rust
        // files, but the disabled preview files stay counted in the
        // uninspected lower bounds so the scope record never hides them.
        let files = vec![
            changed_file("src/a.rs", 4, 0),
            changed_file("src/b.rs", 4, 0),
            changed_file("src/c.rs", 4, 0),
            changed_file("app/a.ts", 4, 0),
            changed_file("app/b.ts", 4, 0),
            changed_file("app/c.ts", 4, 0),
        ];
        let scope = require_partial(
            select_partial_diff_partition(&files, &budgets(2, 1_000), &[LanguageId::Rust]),
            "rust-only enabled set",
        )?;
        assert_eq!(scope.selected_files.len(), 2);
        assert!(
            scope
                .selected_files
                .iter()
                .all(|path| path.ends_with(".rs"))
        );
        assert_eq!(scope.uninspected_files_lower_bound, 4);
        Ok(())
    }

    #[test]
    fn partial_simultaneous_hit_on_later_file_reports_file_budget() -> Result<(), String> {
        // The first file fits and is selected; the second file would cross
        // BOTH budgets. The simultaneous-hit rule reports file_budget — the
        // first-file exception does not apply because a file was already
        // selected (#2142 review).
        let files = vec![
            changed_file("src/a.rs", 5, 0),
            changed_file("src/b.rs", 200, 0),
        ];
        let scope = require_partial(
            select_partial_diff_partition(&files, &budgets(1, 100), ALL_LANGUAGES),
            "simultaneous hit on second file",
        )?;
        assert_eq!(scope.selected_files, vec!["src/a.rs".to_string()]);
        assert_eq!(scope.stop_reason, PartialDiffStopReason::FileBudget);
        assert_eq!(scope.selected_changed_lines, 5);
        Ok(())
    }

    #[test]
    fn partial_selection_returns_none_when_diff_fits_budgets() {
        let files = vec![
            changed_file("src/a.rs", 10, 5),
            changed_file("src/b.rs", 4, 0),
        ];
        assert!(select_partial_diff_partition(&files, &budgets(2, 19), ALL_LANGUAGES).is_none());
        assert!(
            select_partial_diff_partition(&files, &budgets(200, 1_000), ALL_LANGUAGES).is_none()
        );
    }

    #[test]
    fn partial_budget_env_defaults_when_unset() -> Result<(), String> {
        let resolved = partial_diff_budgets_from_env(
            Err(VarError::NotPresent),
            Err(VarError::NotPresent),
            DIFF_INDEX_FILE_LIMIT,
            DIFF_CHANGED_RUST_LINE_LIMIT,
        )?;
        assert_eq!(resolved.file_budget, PARTIAL_DIFF_FILE_BUDGET_DEFAULT);
        assert_eq!(resolved.line_budget, PARTIAL_DIFF_LINE_BUDGET_DEFAULT);
        assert!(resolved.disclosures.is_empty());
        assert!(
            resolved.file_budget <= DIFF_INDEX_FILE_LIMIT
                && resolved.line_budget <= DIFF_CHANGED_RUST_LINE_LIMIT,
            "defaults must sit inside the hard analysis-cost guards"
        );
        Ok(())
    }

    fn invalid_budget_message(file: &str, line: &str) -> String {
        match partial_diff_budgets_from_env(
            Ok(file.to_string()),
            Ok(line.to_string()),
            DIFF_INDEX_FILE_LIMIT,
            DIFF_CHANGED_RUST_LINE_LIMIT,
        ) {
            Ok(resolved) => format!(
                "expected partial_budget_invalid for file={file:?} line={line:?}, got {resolved:?}"
            ),
            Err(message) => message,
        }
    }

    #[test]
    fn repo_index_file_limit_env_parsing() -> Result<(), String> {
        // Default applies when unset; valid override wins; invalid fails
        // closed (#2109).
        let unset = repo_index_file_limit_from_env(Err(std::env::VarError::NotPresent))
            .map_err(|err| format!("default should parse: {err}"))?;
        assert_eq!(
            unset, 800,
            "default must be the {REPO_INDEX_FILE_LIMIT_ENV} guard"
        );
        let raised = repo_index_file_limit_from_env(Ok("5000".to_string()))
            .map_err(|err| format!("valid override should parse: {err}"))?;
        assert_eq!(raised, 5000);
        for bad in ["", "  ", "lots", "1.5", "0", "-5"] {
            if repo_index_file_limit_from_env(Ok(bad.to_string())).is_ok() {
                return Err(format!("invalid override {bad:?} must fail closed"));
            }
        }
        Ok(())
    }

    #[test]
    fn enforce_repo_index_file_limit_is_fail_closed_exactly_over_the_guard() -> Result<(), String> {
        // Exactly at the limit passes; one file over fails with the named
        // error, the guard identity, and the repair route (#2109 review).
        enforce_repo_index_file_limit(800, 800)
            .map_err(|err| format!("exactly-at-limit must pass: {err}"))?;
        let err = match enforce_repo_index_file_limit(801, 800) {
            Err(err) => err,
            Ok(()) => return Err("one over the limit must fail".to_string()),
        };
        for needle in [
            "repo_scope_oversized",
            "801 indexed Rust files",
            "RIPR_MAX_REPO_INDEX_FILES",
            "--base/--diff",
        ] {
            assert!(err.contains(needle), "error missing `{needle}`: {err}");
        }
        Ok(())
    }

    #[test]
    fn partial_budget_env_rejects_invalid_overrides() -> Result<(), String> {
        for (file, line, label) in [
            ("", "100", "empty file budget"),
            ("100", "  ", "whitespace-only line budget"),
            ("lots", "100", "non-numeric file budget"),
            ("100", "1.5", "non-integer line budget"),
            ("0", "100", "zero file budget"),
            ("100", "0", "zero line budget"),
            ("-5", "100", "negative file budget"),
            ("100", "-1", "negative line budget"),
            (
                "99999999999999999999999999",
                "100",
                "overflowing file budget",
            ),
        ] {
            let message = invalid_budget_message(file, line);
            if !message.starts_with("partial_budget_invalid:") {
                return Err(format!(
                    "{label}: override must fail closed as partial_budget_invalid, got: {message}"
                ));
            }
            if !message.contains(PARTIAL_DIFF_FILE_BUDGET_ENV)
                && !message.contains(PARTIAL_DIFF_LINE_BUDGET_ENV)
            {
                return Err(format!(
                    "{label}: error must name the offending env var, got: {message}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn partial_budget_env_rejects_non_unicode() {
        let result = partial_diff_budgets_from_env(
            Err(VarError::NotUnicode("x".into())),
            Err(VarError::NotPresent),
            DIFF_INDEX_FILE_LIMIT,
            DIFF_CHANGED_RUST_LINE_LIMIT,
        );
        assert!(
            matches!(&result, Err(message) if message.starts_with("partial_budget_invalid:")),
            "non-unicode override must fail closed as partial_budget_invalid, got {result:?}"
        );
    }

    #[test]
    fn partial_budget_env_clamps_above_guard_with_disclosure() -> Result<(), String> {
        let resolved = partial_diff_budgets_from_env(
            Ok((DIFF_INDEX_FILE_LIMIT + 1).to_string()),
            Ok((DIFF_CHANGED_RUST_LINE_LIMIT + 1).to_string()),
            DIFF_INDEX_FILE_LIMIT,
            DIFF_CHANGED_RUST_LINE_LIMIT,
        )?;

        assert_eq!(resolved.file_budget, DIFF_INDEX_FILE_LIMIT);
        assert_eq!(resolved.line_budget, DIFF_CHANGED_RUST_LINE_LIMIT);
        assert_eq!(resolved.disclosures.len(), 2);
        for disclosure in &resolved.disclosures {
            assert!(
                disclosure.contains("clamped"),
                "clamp must be disclosed: {disclosure}"
            );
        }
        assert!(resolved.disclosures[0].contains(PARTIAL_DIFF_FILE_BUDGET_ENV));
        assert!(resolved.disclosures[1].contains(PARTIAL_DIFF_LINE_BUDGET_ENV));

        // A valid in-range override applies without disclosure.
        let resolved = partial_diff_budgets_from_env(
            Ok(" 50 ".to_string()),
            Ok("250".to_string()),
            DIFF_INDEX_FILE_LIMIT,
            DIFF_CHANGED_RUST_LINE_LIMIT,
        )?;
        assert_eq!(resolved.file_budget, 50);
        assert_eq!(resolved.line_budget, 250);
        assert!(resolved.disclosures.is_empty());
        Ok(())
    }

    #[test]
    fn partial_budget_clamp_bounds_against_effective_limit_not_default() -> Result<(), String> {
        // When RIPR_MAX_DIFF_CHANGED_RUST_LINES raises the analysis-cost
        // limit, the partial-budget clamp must follow the effective limit,
        // not the built-in 2000 default (#3595 review): otherwise a CI runner
        // that raises the limit and the budget together still truncates the
        // partition back to the default ceiling and loses full-scope
        // evidence.
        let effective_max = diff_changed_rust_line_limit_from_env(Ok("2500".to_string()))?;
        assert_eq!(effective_max, 2500);

        // 2001 exceeds the 2000 default but sits inside the raised limit, so
        // it applies verbatim with no clamp disclosure.
        let resolved = partial_diff_budgets_from_env(
            Err(VarError::NotPresent),
            Ok((DIFF_CHANGED_RUST_LINE_LIMIT + 1).to_string()),
            DIFF_INDEX_FILE_LIMIT,
            effective_max,
        )?;
        assert_eq!(
            resolved.line_budget,
            DIFF_CHANGED_RUST_LINE_LIMIT + 1,
            "an override inside the raised limit must not clamp to the 2000 default"
        );
        assert!(resolved.disclosures.is_empty());

        // An override above the raised limit still clamps, to the raised
        // limit — never to the default.
        let clamped = partial_diff_budgets_from_env(
            Err(VarError::NotPresent),
            Ok("3000".to_string()),
            DIFF_INDEX_FILE_LIMIT,
            effective_max,
        )?;
        assert_eq!(clamped.line_budget, 2500);
        assert_eq!(clamped.disclosures.len(), 1);
        assert!(
            clamped.disclosures[0].contains("2500"),
            "clamp must bound against the raised limit: {:?}",
            clamped.disclosures[0]
        );
        Ok(())
    }

    #[test]
    fn partition_identity_is_stable_for_same_inputs() -> Result<(), String> {
        let files = vec![
            changed_file("src/a.rs", 60, 0),
            changed_file("src/b.rs", 60, 0),
        ];
        let first = require_partial(
            select_partial_diff_partition(&files, &budgets(1, 100), ALL_LANGUAGES),
            "identity stability (first)",
        )?;
        let second = require_partial(
            select_partial_diff_partition(&files, &budgets(1, 100), ALL_LANGUAGES),
            "identity stability (second)",
        )?;

        assert_eq!(first.partition_identity, second.partition_identity);
        assert_eq!(first.diff_identity, second.diff_identity);
        assert!(
            first.partition_identity.len() == 64
                && first
                    .partition_identity
                    .chars()
                    .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase()),
            "partition identity must be lowercase hex sha256: {}",
            first.partition_identity
        );
        Ok(())
    }

    #[test]
    fn partition_identity_discriminates_budget_diff_and_version() -> Result<(), String> {
        let files = vec![
            changed_file("src/a.rs", 60, 0),
            changed_file("src/b.rs", 60, 0),
        ];
        let baseline = require_partial(
            select_partial_diff_partition(&files, &budgets(1, 100), ALL_LANGUAGES),
            "identity discrimination baseline",
        )?;

        let other_budget = require_partial(
            select_partial_diff_partition(&files, &budgets(1, 101), ALL_LANGUAGES),
            "different line budget",
        )?;
        assert_ne!(
            baseline.partition_identity, other_budget.partition_identity,
            "a different budget must produce a different identity"
        );

        let mut changed = files.clone();
        changed[0].added_lines[0].text = "let value = input + 2;".to_string();
        let other_diff = require_partial(
            select_partial_diff_partition(&changed, &budgets(1, 100), ALL_LANGUAGES),
            "different diff",
        )?;
        assert_ne!(
            baseline.partition_identity, other_diff.partition_identity,
            "a different diff must produce a different identity"
        );
        assert_ne!(
            baseline.diff_identity, other_diff.diff_identity,
            "diff identity must track diff content"
        );

        // A selection-version bump must produce a different identity even for
        // the same remaining inputs (canonical form, not a generic map
        // serialization whose key order is not guaranteed).
        let selected_sorted = baseline.selected_files.clone();
        let canonical = partition_canonical_form(
            &baseline.diff_identity,
            baseline.file_budget,
            baseline.line_budget,
            &selected_sorted,
        );
        assert_eq!(
            sha256_hex(canonical.as_bytes()),
            baseline.partition_identity,
            "the identity must be the sha256 of the canonical form"
        );
        let bumped = canonical.replacen(PARTIAL_DIFF_SELECTION_VERSION, "partial-diff-v2", 1);
        assert_ne!(
            sha256_hex(bumped.as_bytes()),
            baseline.partition_identity,
            "a selection-version bump must change the identity"
        );
        Ok(())
    }

    #[test]
    fn partition_canonical_form_is_field_per_line_not_map_json() -> Result<(), String> {
        let canonical = partition_canonical_form(
            "sha256:abc",
            2,
            100,
            &["src/a.rs".to_string(), "src/b.rs".to_string()],
        );

        let lines: Vec<&str> = canonical.lines().collect();
        let expected = vec![
            format!("selection_version={PARTIAL_DIFF_SELECTION_VERSION}"),
            format!("language_tier_version={PARTIAL_DIFF_LANGUAGE_TIER_VERSION}"),
            "diff_identity=sha256:abc".to_string(),
            "file_budget=2".to_string(),
            "line_budget=100".to_string(),
            "selected=src/a.rs".to_string(),
            "selected=src/b.rs".to_string(),
        ];
        assert_eq!(lines, expected, "canonical form must be field-per-line");
        assert!(!canonical.contains('{') && !canonical.contains('['));
        assert!(!canonical.contains('\r'), "canonical form is LF-separated");
        Ok(())
    }

    #[test]
    fn diff_identity_tracks_parsed_diff_content() -> Result<(), String> {
        let files = vec![changed_file("src/a.rs", 2, 1)];
        let identity = diff_identity_from_changed_files(&files);
        assert!(identity.starts_with("sha256:"), "got: {identity}");
        assert_eq!(identity, diff_identity_from_changed_files(&files));

        // File ordering in the diff does not change the identity.
        let multi_forward = vec![
            changed_file("src/a.rs", 1, 0),
            changed_file("src/b.rs", 1, 0),
        ];
        let multi_reversed = vec![
            changed_file("src/b.rs", 1, 0),
            changed_file("src/a.rs", 1, 0),
        ];
        assert_eq!(
            diff_identity_from_changed_files(&multi_forward),
            diff_identity_from_changed_files(&multi_reversed),
            "diff identity must not depend on diff file ordering"
        );
        Ok(())
    }

    #[test]
    fn partial_scope_selects_matches_normalized_paths() -> Result<(), String> {
        let files = vec![
            changed_file("src/a.rs", 60, 0),
            changed_file("src/b.rs", 60, 0),
        ];
        let scope = require_partial(
            select_partial_diff_partition(&files, &budgets(1, 100), ALL_LANGUAGES),
            "selects helper",
        )?;

        assert!(scope.selects(Path::new("src/a.rs")));
        assert!(scope.selects(Path::new("./src/a.rs")));
        assert!(scope.selects(Path::new("src\\a.rs")));
        assert!(!scope.selects(Path::new("src/b.rs")));
        Ok(())
    }

    /// End-to-end: a diff over the default partial line budget but inside the
    /// hard guards returns a `limited_partial_scope` result whose findings
    /// cover exactly the selected partition — never a silent subset.
    #[test]
    fn analyze_diff_returns_limited_partial_scope_for_over_budget_diff() -> Result<(), String> {
        // Default line budget is 1_000; two 600-changed-line files exceed it
        // (1_200) while staying under the 2_000 hard guard.
        fn source(lines: usize, name: &str) -> String {
            let mut out = format!("pub fn {name}(x: i32) -> i32 {{\n    if x > 0 {{\n");
            for index in 0..lines.saturating_sub(7) {
                out.push_str(&format!("        let v{index} = x + {index};\n"));
            }
            out.push_str("        1\n    } else {\n        0\n    }\n}\n");
            out
        }
        fn new_file_diff(path: &str, content: &str) -> String {
            let mut out = format!(
                "diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1,{} @@\n",
                content.lines().count()
            );
            for line in content.lines() {
                out.push_str(&format!("+{line}\n"));
            }
            out
        }

        let root = temp_root("partial-scope-end-to-end")?;
        write(
            &root.join("Cargo.toml"),
            "[package]\nname='partial-scope'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        let a_source = source(600, "alpha");
        let b_source = source(600, "beta");
        write(&root.join("src/a.rs"), &a_source)?;
        write(&root.join("src/b.rs"), &b_source)?;
        let diff_text = format!(
            "{}{}",
            new_file_diff("src/a.rs", &a_source),
            new_file_diff("src/b.rs", &b_source)
        );
        let changed_files = diff::parse_unified_diff(&diff_text);

        let result = RustAdapter.analyze_diff(
            &AnalysisOptions {
                root,
                base: None,
                diff_file: None,
                mode: AnalysisMode::Draft,
                resolved_subject_identity: None,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &changed_files,
        )?;

        let scope = result
            .partial_scope
            .ok_or("over-budget diff must return a partial partition")?;
        let per_file_lines = a_source.lines().count();
        assert_eq!(scope.run_status, PartialDiffScope::RUN_STATUS);
        assert_eq!(scope.selected_files, vec!["src/a.rs".to_string()]);
        assert_eq!(scope.stop_reason, PartialDiffStopReason::LineBudget);
        assert_eq!(scope.selected_changed_lines, per_file_lines);
        assert_eq!(scope.uninspected_files_lower_bound, 1);
        assert_eq!(scope.uninspected_changed_lines_lower_bound, per_file_lines);
        assert_eq!(result.changed_files, 1);
        assert!(
            result.findings.iter().all(|finding| {
                finding
                    .probe
                    .location
                    .file
                    .to_string_lossy()
                    .replace('\\', "/")
                    .ends_with("src/a.rs")
            }),
            "findings must cover the selected partition only: {:?}",
            result.findings
        );
        Ok(())
    }

    #[test]
    fn witnessed_no_path_limitation_does_not_claim_no_tests_found() {
        let mut finding = no_path_finding_with_infection_summary(
            super::NO_TESTS_INFECTION_SUMMARY,
            vec![
                "first evidence".to_string(),
                super::NO_TESTS_INFECTION_SUMMARY.to_string(),
            ],
        );

        replace_witnessed_no_path_infection_summary(&mut finding);

        assert_eq!(
            finding.ripr.infect.summary,
            super::NO_STATICALLY_REACHABLE_TEST_PATH_INFECTION_SUMMARY
        );
        assert!(
            finding
                .evidence
                .iter()
                .all(|line| line != super::NO_TESTS_INFECTION_SUMMARY),
            "witnessed limitations must not say no tests were found: {:?}",
            finding.evidence
        );
        assert!(
            finding
                .evidence
                .iter()
                .any(|line| line == super::NO_STATICALLY_REACHABLE_TEST_PATH_INFECTION_SUMMARY),
            "replacement evidence line should be preserved for renderers"
        );
    }

    #[test]
    fn witnessed_no_path_limitation_preserves_other_infection_summaries() {
        let summary = "No reachable tests were found, so infection cannot be established";
        let mut finding =
            no_path_finding_with_infection_summary(summary, vec![summary.to_string()]);

        replace_witnessed_no_path_infection_summary(&mut finding);

        assert_eq!(finding.ripr.infect.summary, summary);
        assert_eq!(finding.evidence, vec![summary.to_string()]);
    }

    #[test]
    fn transitive_reach_limit_kind_names_integration_test_path() {
        assert_eq!(
            transitive_reach_limit_kind(Path::new("tests/version_req.rs")),
            StaticLimitKind::RustIntegrationPublicApiPathUnresolved
        );
        assert_eq!(
            transitive_reach_limit_kind(Path::new("src/lib.rs")),
            StaticLimitKind::RustTransitiveReachUnresolved
        );
    }

    #[test]
    fn cargo_binary_invocation_shape_is_conservative_and_deterministic() {
        assert!(is_cargo_binary_invocation(
            r#"let output = Command::new(env!("CARGO_BIN_EXE_worker"))
                .output().expect("binary output");
            assert!(output.status.success());"#
        ));
        assert!(is_cargo_binary_invocation(
            r#"Command::cargo_bin("worker").unwrap().assert().success();"#
        ));
        assert!(!is_cargo_binary_invocation(
            r#"Command::new("sh").arg("-c").output().unwrap();"#
        ));
        assert!(!is_cargo_binary_invocation(
            r#"let _binary = env!("CARGO_BIN_EXE_worker");
            Command::new("sh").status().unwrap();"#
        ));
        assert!(!is_cargo_binary_invocation(
            r#"assert!(output.stdout.contains("receipt:"));"#
        ));
    }

    #[test]
    fn subprocess_limit_only_applies_to_binary_source_paths_and_integration_tests() {
        let mut index = RustIndex::default();
        index.tests.push(TestFact {
            name: "cli_receipt".to_string(),
            file: PathBuf::from("tests/cli.rs"),
            start_line: 12,
            end_line: 18,
            body: r#"Command::new(env!("CARGO_BIN_EXE_worker")).output().unwrap();"#.to_string(),
            calls: Vec::new(),
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        });
        assert!(is_binary_source_path(Path::new("src/main.rs")));
        assert!(super::find_subprocess_binary_test(&index, Path::new("src/main.rs")).is_some());
        assert!(super::find_subprocess_binary_test(&index, Path::new("src/lib.rs")).is_none());
        index.tests[0].file = PathBuf::from("src/lib.rs");
        assert!(super::find_subprocess_binary_test(&index, Path::new("src/main.rs")).is_none());
    }

    #[test]
    fn macro_reach_limit_kind_names_direct_test_body_macro_path() {
        assert_eq!(
            macro_reach_limit_kind(crate::analysis::classify::MACRO_WITNESS_TEST_BODY_HOST),
            StaticLimitKind::RustMacroWrappedTestCallUnresolved
        );
        assert_eq!(
            macro_reach_limit_kind("outer"),
            StaticLimitKind::RustMacroReachUnresolved
        );
    }

    #[test]
    fn macro_wrapped_assertion_limit_names_reachable_unobserved_assertion_macro() {
        let mut finding = reachable_unrevealed_finding_with_related_test(
            "test_inner_with_custom_assertion_macro",
            "tests/it.rs",
            4,
        );
        let index = RustIndex {
            tests: vec![test_summary(
                "test_inner_with_custom_assertion_macro",
                "tests/it.rs",
                4,
                "let result = inner(10, 3);\nassert_result!(result, 7);",
            )],
            ..RustIndex::default()
        };

        apply_rust_macro_wrapped_assertion_limit(&mut finding, &index);

        assert_eq!(
            finding.static_limit_kind,
            Some(StaticLimitKind::RustMacroWrappedAssertionUnresolved)
        );
        assert!(finding.evidence.iter().any(|line| {
            line.contains("assertion-like macro `assert_result!` at tests/it.rs:5")
        }));
        assert!(finding.evidence.iter().any(|line| {
            line == "limitation_last_established_edge: test `test_inner_with_custom_assertion_macro` (tests/it.rs:4) -> assertion macro `assert_result!` at tests/it.rs:5"
        }));
        assert!(finding.evidence.iter().any(|line| {
            line == "limitation_first_unresolved_edge: assertion macro `assert_result!` semantics toward the changed owner"
        }));
        assert!(finding.evidence.iter().any(|line| {
            line == "limitation_analyzer_route: analysis/rust-macro-assertion-oracle"
        }));
        assert!(finding.evidence.iter().any(|line| {
            line == "limitation_non_claim: named limitation only; ripr cannot confirm or deny that the macro assertion discriminates the change"
        }));
    }

    #[test]
    fn macro_wrapped_assertion_limit_ignores_known_assertion_macros() {
        let mut finding = reachable_unrevealed_finding_with_related_test(
            "test_inner_with_known_assertion_macro",
            "tests/it.rs",
            4,
        );
        let index = RustIndex {
            tests: vec![test_summary(
                "test_inner_with_known_assertion_macro",
                "tests/it.rs",
                4,
                "let result = inner(10, 3);\nassert_eq!(result, 7);",
            )],
            ..RustIndex::default()
        };

        apply_rust_macro_wrapped_assertion_limit(&mut finding, &index);

        assert_eq!(finding.static_limit_kind, None);
        assert!(finding.evidence.is_empty());
    }

    #[test]
    fn macro_wrapped_assertion_limit_ignores_comments_and_string_literals() {
        let mut finding = reachable_unrevealed_finding_with_related_test(
            "test_inner_with_commented_assertion_macro",
            "tests/it.rs",
            4,
        );
        let index = RustIndex {
            tests: vec![test_summary(
                "test_inner_with_commented_assertion_macro",
                "tests/it.rs",
                4,
                r##"let result = inner(10, 3);
// assert_result!(result, 7);
/* assert_block_result!(result, 7); */
let note = "assert_string_result!(result, 7)";
let raw = r#"assert_raw_result!(result, 7)"#;
let _ = (result, note, raw);"##,
            )],
            ..RustIndex::default()
        };

        apply_rust_macro_wrapped_assertion_limit(&mut finding, &index);

        assert_eq!(finding.static_limit_kind, None);
        assert!(finding.evidence.is_empty());
    }

    fn changed_file(path: &str, added: usize, removed: usize) -> ChangedFile {
        ChangedFile {
            path: PathBuf::from(path),
            added_lines: changed_lines(added),
            removed_lines: changed_lines(removed),
        }
    }

    #[test]
    fn generated_rust_paths_use_conservative_name_and_directory_rules() {
        for path in [
            "src/proto/generated.rs",
            "src/model.gen.rs",
            "src/model_generated.rs",
            "src/generated_model.rs",
            "src/bindings.rs",
            "src/schema.rs",
            "src/generated/model.rs",
            "src/gen/model.rs",
            "target/out/model.rs",
        ] {
            assert!(
                is_generated_rust_file(Path::new(path)),
                "expected generated Rust path: {path}"
            );
        }
        for path in [
            "src/lib.rs",
            "src/engine.rs",
            "tests/behavior.rs",
            "src/proto/generated/data.ts",
            "out/data.py",
        ] {
            assert!(
                !is_generated_rust_file(Path::new(path)),
                "unexpected generated Rust path: {path}"
            );
        }
    }

    #[test]
    fn custom_generated_rust_patterns_match_names_and_repository_paths() {
        let patterns = vec!["*.gen.rs".to_string(), "src/generated/**/*.rs".to_string()];
        for path in [
            "src/proto/messages.gen.rs",
            "src/generated/model.rs",
            "src/generated/nested/model.rs",
        ] {
            assert!(
                is_generated_rust_file_with_patterns(Path::new(path), &patterns),
                "expected custom generated Rust path: {path}"
            );
        }
        for path in ["src/model.rs", "generated/model.py", "src/not-generated.rs"] {
            assert!(
                !is_generated_rust_file_with_patterns(Path::new(path), &patterns),
                "unexpected custom generated Rust path: {path}"
            );
        }
    }

    #[test]
    fn custom_generated_rust_patterns_are_additive_to_builtin_rules() {
        let patterns = vec!["src/custom/**/*.rs".to_string()];
        assert!(is_generated_rust_file_with_patterns(
            Path::new("src/schema.rs"),
            &patterns
        ));
        assert!(is_generated_rust_file_with_patterns(
            Path::new("src/custom/model.rs"),
            &patterns
        ));
    }

    #[test]
    fn custom_generated_rust_patterns_bound_wildcard_backtracking() {
        let patterns = vec![
            "src/**/**/**/**/**/**/**/**/**/**/generated/*.gen.rs".to_string(),
            "*?*?*?*?*?*?*?*?*?*?*?*?*?*?*?*?*.rs".to_string(),
        ];
        assert!(is_generated_rust_file_with_patterns(
            Path::new("src/a/b/c/d/e/f/g/h/i/j/generated/messages.gen.rs"),
            &patterns
        ));
        assert!(is_generated_rust_file_with_patterns(
            Path::new("src/this_is_a_long_generated_file_name.rs"),
            &patterns
        ));
    }

    #[test]
    fn partial_scope_identity_includes_skipped_generated_files() -> Result<(), String> {
        let analyzable = vec![
            changed_file("src/a.rs", 1, 0),
            changed_file("src/b.rs", 1, 0),
        ];
        let mut identity_a = analyzable.clone();
        identity_a.push(changed_file("src/generated.rs", 1, 0));
        let mut identity_b = analyzable.clone();
        identity_b.push(changed_file("src/schema.rs", 1, 0));

        let first = select_partial_diff_partition_with_identity(
            &analyzable,
            &identity_a,
            &budgets(1, 1),
            ALL_LANGUAGES,
        )
        .ok_or_else(|| "two changed files exceed the partial budget".to_string())?;
        let second = select_partial_diff_partition_with_identity(
            &analyzable,
            &identity_b,
            &budgets(1, 1),
            ALL_LANGUAGES,
        )
        .ok_or_else(|| "two changed files exceed the partial budget".to_string())?;

        assert_ne!(first.diff_identity, second.diff_identity);
        assert_ne!(first.partition_identity, second.partition_identity);
        Ok(())
    }

    fn changed_lines(count: usize) -> Vec<ChangedLine> {
        (1..=count)
            .map(|line| ChangedLine {
                line,
                text: "let value = input + 1;".to_string(),
                new_side_line: line,
            })
            .collect()
    }

    fn reachable_unrevealed_finding_with_related_test(
        test_name: &str,
        test_file: &str,
        test_line: usize,
    ) -> Finding {
        let mut finding = no_path_finding_with_infection_summary("stage", Vec::new());
        let stage = |state| StageEvidence::new(state, Confidence::Medium, "stage");
        finding.class = ExposureClass::ReachableUnrevealed;
        finding.ripr.reach = stage(StageState::Yes);
        finding.ripr.infect = stage(StageState::Yes);
        finding.ripr.propagate = stage(StageState::Yes);
        finding.ripr.reveal.observe = stage(StageState::No);
        finding.ripr.reveal.discriminate = stage(StageState::No);
        finding.related_tests = vec![RelatedTest {
            name: test_name.to_string(),
            file: PathBuf::from(test_file),
            line: test_line,
            oracle: None,
            oracle_kind: crate::domain::OracleKind::Unknown,
            oracle_strength: crate::domain::OracleStrength::None,
            relation_reason: None,
            relation_confidence: None,
        }];
        finding
    }

    fn test_summary(name: &str, file: &str, start_line: usize, body: &str) -> TestSummary {
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line,
            end_line: start_line + body.lines().count(),
            body: body.to_string(),
            calls: vec![CallFact {
                line: start_line,
                name: "inner".to_string(),
                text: "inner(10, 3)".to_string(),
            }],
            assertions: Vec::new(),
            literals: vec![
                LiteralFact {
                    line: start_line,
                    value: "10".to_string(),
                },
                LiteralFact {
                    line: start_line,
                    value: "3".to_string(),
                },
                LiteralFact {
                    line: start_line + 1,
                    value: "7".to_string(),
                },
            ],
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    fn no_path_finding_with_infection_summary(summary: &str, evidence: Vec<String>) -> Finding {
        let stage = |state| StageEvidence::new(state, Confidence::Low, "stage");
        Finding {
            id: "probe:src_lib.rs:predicate:test".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:src_lib.rs:predicate:test".to_string()),
                location: SourceLocation::new("src/lib.rs", 2, 1),
                owner: Some(SymbolId("src/lib.rs::inner".to_string())),
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: None,
                after: Some("if a >= b {".to_string()),
                expression: "if a >= b {".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class: ExposureClass::NoStaticPath,
            ripr: RiprEvidence {
                reach: stage(StageState::No),
                infect: StageEvidence::new(StageState::Unknown, Confidence::Low, summary),
                propagate: stage(StageState::Yes),
                reveal: RevealEvidence {
                    observe: stage(StageState::No),
                    discriminate: stage(StageState::No),
                },
            },
            confidence: 0.48,
            evidence,
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence::default(),
            stop_reasons: Vec::new(),
            related_tests: Vec::new(),
            recommended_next_step: None,
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
            changed_sink: None,
            observed_sink: None,
            oracle_alignment: None,
            alignment_reason: None,
            source_currentness: crate::domain::SourceCurrentness::CandidateCurrent,
        }
    }

    // --- FFI / cross-language guard tests (#910) ---

    fn ffi_function(file: &str, name: &str, attrs: Vec<&str>) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId(format!("{file}::{name}")),
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 5,
            body: format!("pub fn {name}(x: i32) -> i32 {{ x }}"),
            calls: vec![],
            returns: vec![],
            literals: vec![],
            source_role: FunctionSourceRole::Production,
            attrs: attrs.into_iter().map(|s| s.to_string()).collect(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    fn probe_for_owner(file: &str, name: &str, family: ProbeFamily) -> Probe {
        Probe {
            id: ProbeId(format!("probe:{file}::{name}")),
            location: SourceLocation::new(file, 2, 1),
            owner: Some(SymbolId(format!("{file}::{name}"))),
            family,
            delta: DeltaKind::Control,
            before: None,
            after: Some("x > 0".to_string()),
            expression: "x > 0".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        }
    }

    #[test]
    fn owner_with_no_mangle_attr_is_ffi() {
        let owner = ffi_function("src/lib.rs", "ffi_fn", vec!["#[no_mangle]"]);
        assert!(owner_has_ffi_attr(&owner));
    }

    #[test]
    fn owner_with_wasm_bindgen_attr_is_ffi() {
        let owner = ffi_function("src/lib.rs", "wasm_fn", vec!["#[wasm_bindgen]"]);
        assert!(owner_has_ffi_attr(&owner));
    }

    #[test]
    fn owner_with_no_attrs_is_not_ffi() {
        let owner = ffi_function("src/lib.rs", "pure_fn", vec![]);
        assert!(!owner_has_ffi_attr(&owner));
    }

    #[test]
    fn owner_with_plain_test_attr_is_not_ffi() {
        let owner = ffi_function("src/lib.rs", "plain_fn", vec!["#[test]"]);
        assert!(!owner_has_ffi_attr(&owner));
    }

    #[test]
    fn cross_language_guard_fires_for_weakly_exposed_with_ffi_attr() {
        let owner = ffi_function("src/lib.rs", "exported_fn", vec!["#[no_mangle]"]);
        let probe = probe_for_owner("src/lib.rs", "exported_fn", ProbeFamily::Predicate);
        let index = RustIndex {
            functions: vec![owner],
            ..RustIndex::default()
        };
        let result = cross_language_limit_kind(&probe, &index, &ExposureClass::WeaklyExposed);
        assert_eq!(
            result,
            Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved),
            "FFI-marked owner with WeaklyExposed gap should set cross-language limit"
        );
    }

    #[test]
    fn cross_language_guard_fires_for_reachable_unrevealed_with_wasm_bindgen() {
        let owner = ffi_function("src/lib.rs", "wasm_fn", vec!["#[wasm_bindgen]"]);
        let probe = probe_for_owner("src/lib.rs", "wasm_fn", ProbeFamily::ReturnValue);
        let index = RustIndex {
            functions: vec![owner],
            ..RustIndex::default()
        };
        let result = cross_language_limit_kind(&probe, &index, &ExposureClass::ReachableUnrevealed);
        assert_eq!(
            result,
            Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved),
            "FFI-marked owner with ReachableUnrevealed gap should set cross-language limit"
        );
    }

    #[test]
    fn cross_language_guard_fires_for_infection_unknown_with_ffi_attr() {
        let owner = ffi_function("src/lib.rs", "exported_fn", vec!["#[no_mangle]"]);
        let probe = probe_for_owner("src/lib.rs", "exported_fn", ProbeFamily::Predicate);
        let index = RustIndex {
            functions: vec![owner],
            ..RustIndex::default()
        };
        let result = cross_language_limit_kind(&probe, &index, &ExposureClass::InfectionUnknown);
        assert_eq!(
            result,
            Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved),
            "FFI-marked owner with InfectionUnknown gap should set cross-language limit"
        );
    }

    #[test]
    fn cross_language_guard_does_not_fire_for_pure_rust_owner_weakly_exposed() {
        // Pure-Rust control: no FFI attr — guard must NOT fire even for a gap class.
        let owner = ffi_function("src/lib.rs", "pure_fn", vec![]);
        let probe = probe_for_owner("src/lib.rs", "pure_fn", ProbeFamily::Predicate);
        let index = RustIndex {
            functions: vec![owner],
            ..RustIndex::default()
        };
        let result = cross_language_limit_kind(&probe, &index, &ExposureClass::WeaklyExposed);
        assert_eq!(
            result, None,
            "Pure-Rust owner must NOT receive cross-language static_limit_kind"
        );
    }

    #[test]
    fn cross_language_guard_does_not_fire_for_exposed_class_even_with_ffi() {
        // Even an FFI-marked owner should not gain the limitation on Exposed
        // (no gap = nothing to reclassify).
        let owner = ffi_function("src/lib.rs", "exported_fn", vec!["#[no_mangle]"]);
        let probe = probe_for_owner("src/lib.rs", "exported_fn", ProbeFamily::ReturnValue);
        let index = RustIndex {
            functions: vec![owner],
            ..RustIndex::default()
        };
        let result = cross_language_limit_kind(&probe, &index, &ExposureClass::Exposed);
        assert_eq!(
            result, None,
            "Exposed class must not receive cross-language static_limit_kind regardless of FFI"
        );
    }

    fn changed_lib_rs_diff() -> Vec<ChangedFile> {
        diff::parse_unified_diff(
            "diff --git a/src/lib.rs b/src/lib.rs\n\
             --- a/src/lib.rs\n\
             +++ b/src/lib.rs\n\
             @@ -1,3 +1,3 @@\n\
             pub fn gate_state(flag: bool) -> bool {\n\
             -    if flag { true } else { false }\n\
             +    if flag { false } else { true }\n\
             }\n",
        )
    }

    fn analyze_diff_error_with_cancelled_token(root: PathBuf) -> Result<String, String> {
        // #1972: a token cancelled before analysis starts (e.g. an expired
        // physical refresh deadline) must surface as a prompt cooperative
        // cancellation error, not a completed or partial result.
        let token = cancellation::AnalysisCancellationToken::new();
        if !token.cancel(cancellation::AnalysisAbortKind::DeadlineExceeded) {
            return Err("test setup: deadline cancel must win on a fresh token".to_string());
        }
        let changed_files = changed_lib_rs_diff();
        let options = AnalysisOptions {
            root,
            base: None,
            diff_file: None,
            mode: AnalysisMode::Ready,
            resolved_subject_identity: None,
            include_unchanged_tests: true,
            resolve_tsconfig_paths: false,
            perl_facts_path: None,
            git_timeout: None,
            git_candidate: None,
            production_like_targets: Default::default(),
            test_harnesses: Vec::new(),
        };
        let policy = OraclePolicy::default();
        let result = cancellation::with_token(&token, || {
            RustAdapter.analyze_diff(&options, &policy, &changed_files)
        });
        match result {
            Err(error) => Ok(error),
            Ok(_) => Err("a pre-cancelled token must not produce a result".to_string()),
        }
    }

    #[test]
    fn pre_cancelled_token_stops_the_diff_file_load_loop() -> Result<(), String> {
        // The changed file exists on disk, so it is selected into the index
        // working set; the load loop's per-file checkpoint is the first
        // checkpoint in program order and must surface the cancellation.
        let root = temp_root("cancel-load-loop")?;
        write(
            &root.join("Cargo.toml"),
            "[package]\nname='cancel-load'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        write(
            &root.join("src/lib.rs"),
            "pub fn gate_state(flag: bool) -> bool {\n    if flag { true } else { false }\n}\n",
        )?;
        let error = analyze_diff_error_with_cancelled_token(root)?;
        if !error.contains("DeadlineExceeded") {
            return Err(format!(
                "expected a deadline-exceeded cancellation from the load loop, got: {error}"
            ));
        }
        Ok(())
    }

    #[test]
    fn pre_cancelled_token_stops_the_classify_loop() -> Result<(), String> {
        // The changed file is absent from the on-disk workspace, so the index
        // working set is empty and the load loop never iterates; the first
        // checkpoint hit is the classify loop's per-file checkpoint.
        let root = temp_root("cancel-classify-loop")?;
        write(
            &root.join("Cargo.toml"),
            "[package]\nname='cancel-classify'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        let error = analyze_diff_error_with_cancelled_token(root)?;
        if !error.contains("DeadlineExceeded") {
            return Err(format!(
                "expected a deadline-exceeded cancellation from the classify loop, got: {error}"
            ));
        }
        Ok(())
    }

    #[test]
    fn diff_analysis_treats_cargo_benches_as_evidence_not_production() -> Result<(), String> {
        // #3283: benches/** is excluded from the repo production set today,
        // but the diff path seeds production probes from changed bench files
        // — harness plumbing (`iter!`, black_box, Ok(()) returns) generates
        // recursive obligations. A changed bench must stay indexed evidence
        // without seeding production findings, exactly like tests/**.
        let root = temp_root("benches-are-evidence")?;
        write(
            &root.join("Cargo.toml"),
            "[package]\nname='bench-role'\nversion='0.1.0'\nedition='2024'\n\n[[bench]]\nname='exposure'\nharness = false\n",
        )?;
        write(
            &root.join("src/lib.rs"),
            "pub fn price(amount: i32) -> i32 {\n    if amount > 100 { amount - 10 } else { amount }\n}\n",
        )?;
        write(
            &root.join("benches/exposure.rs"),
            "use criterion::Criterion;\nfn bench_price(c: &mut Criterion) {\n    c.bench_function(\"price\", |b| b.iter(|| price(120)));\n}\ncriterion_group!(benches, bench_price);\ncriterion_main!(benches);\n",
        )?;
        let changed_files = diff::parse_unified_diff(
            "diff --git a/src/lib.rs b/src/lib.rs\n\
         new file mode 100644\n\
         --- /dev/null\n\
         +++ b/src/lib.rs\n\
         @@ -0,0 +1,3 @@\n\
         +pub fn price(amount: i32) -> i32 {\n\
         +    if amount > 100 { amount - 10 } else { amount }\n\
         +}\n\
         diff --git a/benches/exposure.rs b/benches/exposure.rs\n\
         new file mode 100644\n\
         --- /dev/null\n\
         +++ b/benches/exposure.rs\n\
         @@ -0,0 +1,6 @@\n\
         +use criterion::Criterion;\n\
         +fn bench_price(c: &mut Criterion) {\n\
         +    c.bench_function(\"price\", |b| b.iter(|| price(120)));\n\
         +}\n\
         +criterion_group!(benches, bench_price);\n\
         +criterion_main!(benches);\n",
        );

        let result = RustAdapter.analyze_diff(
            &AnalysisOptions {
                root,
                base: None,
                diff_file: None,
                mode: AnalysisMode::Ready,
                resolved_subject_identity: None,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &changed_files,
        )?;

        assert_eq!(
            result.changed_files, 2,
            "changed-file accounting must retain the bench file"
        );
        assert!(
            result.findings.iter().all(|finding| !finding
                .probe
                .location
                .file
                .to_string_lossy()
                .replace('\\', "/")
                .contains("benches/")),
            "bench harness plumbing must not become production probes: {:?}",
            result.findings
        );
        Ok(())
    }

    #[test]
    fn diff_analysis_confirms_declared_test_targets_but_not_unconfirmed_names() -> Result<(), String>
    {
        // #3283: a `[[test]]` target with an explicit path confirms
        // evidence role outside tests/; the same filename without a
        // declaration stays a production subject.
        let root = temp_root("declared-test-target")?;
        write(
            &root.join("Cargo.toml"),
            "[package]\nname='declared-target'\nversion='0.1.0'\nedition='2024'\n\n[[test]]\nname='contract'\npath='src/contract_test.rs'\n",
        )?;
        write(
            &root.join("src/lib.rs"),
            "pub fn price(amount: i32) -> i32 {\n    if amount > 100 { amount - 10 } else { amount }\n}\n",
        )?;
        write(
            &root.join("src/contract_test.rs"),
            "fn setup_price(amount: i32) -> i32 {\n    if amount < 0 { 0 } else { price(amount) }\n}\n\n#[test]\nfn price_at_boundary() {\n    assert_eq!(setup_price(100), 90);\n}\n",
        )?;
        write(
            &root.join("src/unconfirmed_test.rs"),
            "pub fn helper_path(amount: i32) -> i32 {\n    if amount < 0 { 0 } else { amount }\n}\n",
        )?;
        let changed_files = diff::parse_unified_diff(
            "diff --git a/src/lib.rs b/src/lib.rs\n\
             new file mode 100644\n\
             --- /dev/null\n\
             +++ b/src/lib.rs\n\
             @@ -0,0 +1,3 @@\n\
             +pub fn price(amount: i32) -> i32 {\n\
             +    if amount > 100 { amount - 10 } else { amount }\n\
             +}\n\
             diff --git a/src/contract_test.rs b/src/contract_test.rs\n\
             new file mode 100644\n\
             --- /dev/null\n\
             +++ b/src/contract_test.rs\n\
             @@ -0,0 +1,8 @@\n\
             +fn setup_price(amount: i32) -> i32 {\n\
             +    if amount < 0 { 0 } else { price(amount) }\n\
             +}\n\
             +\n\
             +#[test]\n\
             +fn price_at_boundary() {\n\
             +    assert_eq!(setup_price(100), 90);\n\
             +}\n\
             diff --git a/src/unconfirmed_test.rs b/src/unconfirmed_test.rs\n\
             new file mode 100644\n\
             --- /dev/null\n\
             +++ b/src/unconfirmed_test.rs\n\
             @@ -0,0 +1,3 @@\n\
             +pub fn helper_path(amount: i32) -> i32 {\n\
             +    if amount < 0 { 0 } else { amount }\n\
             +}\n",
        );
        let result = RustAdapter.analyze_diff(
            &AnalysisOptions {
                root,
                base: None,
                diff_file: None,
                mode: AnalysisMode::Ready,
                resolved_subject_identity: None,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &changed_files,
        )?;
        let path_text = |finding: &crate::domain::Finding| {
            finding
                .probe
                .location
                .file
                .to_string_lossy()
                .replace('\\', "/")
        };
        assert!(
            result
                .findings
                .iter()
                .all(|finding| !path_text(finding).ends_with("src/contract_test.rs")),
            "a declared [[test]] target must not seed production probes: {:?}",
            result.findings
        );
        assert!(
            result
                .findings
                .iter()
                .any(|finding| path_text(finding).ends_with("src/unconfirmed_test.rs")),
            "an unconfirmed *_test.rs filename stays a production subject: {:?}",
            result.findings
        );
        // The confirmed target's test still relates to the changed owner:
        // evidence stays indexed and usable.
        assert!(
            result
                .findings
                .iter()
                .any(|finding| path_text(finding).ends_with("src/lib.rs")
                    && finding
                        .related_tests
                        .iter()
                        .any(|test| test.name == "price_at_boundary")),
            "the declared target's test must remain usable evidence: {:?}",
            result.findings
        );
        Ok(())
    }

    #[test]
    fn diff_analysis_registered_harness_target_never_seeds_production_probes() -> Result<(), String>
    {
        // #3532: an exact `[analysis.test_harnesses]` registration makes a
        // `harness = false` custom target evidence role outside tests/ —
        // its inert `#[test]` attributes and helper plumbing seed no
        // production probes, while an unregistered sibling does.
        let root = temp_root("registered-harness-diff")?;
        write(
            &root.join("Cargo.toml"),
            "[package]\nname='registered-harness'\nversion='0.1.0'\nedition='2024'\n\n\
             [workspace]\n\n\
             [[test]]\nname='price_mimic'\npath='src/price_mimic.rs'\nharness=false\n",
        )?;
        write(
            &root.join("src/lib.rs"),
            "pub fn price(amount: i32) -> i32 {\n    if amount > 100 { amount - 10 } else { amount }\n}\n",
        )?;
        write(
            &root.join("src/price_mimic.rs"),
            "use libtest_mimic::Trial;\n\nfn setup_price(amount: i32) -> i32 {\n    if amount < 0 { 0 } else { price(amount) }\n}\n\n#[test]\nfn inert_without_the_harness() {\n    assert_eq!(setup_price(100), 90);\n}\n\nfn trials() -> Vec<Trial> {\n    vec![Trial::test(\"price_at_boundary\", || {\n        assert_eq!(setup_price(100), 90);\n    })]\n}\n",
        )?;
        write(
            &root.join("src/unregistered_helper.rs"),
            "pub fn helper_path(amount: i32) -> i32 {\n    if amount < 0 { 0 } else { amount }\n}\n",
        )?;
        let mut diff_text = String::new();
        let mut add_file = |path: &str, lines: &[&str]| {
            diff_text.push_str(&format!("diff --git a{path} b{path}\n"));
            diff_text.push_str("new file mode 100644\n");
            diff_text.push_str("--- /dev/null\n");
            diff_text.push_str(&format!("+++ b{path}\n"));
            diff_text.push_str(&format!("@@ -0,0 +1,{} @@\n", lines.len()));
            for line in lines {
                diff_text.push_str(&format!("+{line}\n"));
            }
        };
        add_file(
            "/src/lib.rs",
            &[
                "pub fn price(amount: i32) -> i32 {",
                "    if amount > 100 { amount - 10 } else { amount }",
                "}",
            ],
        );
        add_file(
            "/src/price_mimic.rs",
            &[
                "use libtest_mimic::Trial;",
                "",
                "fn setup_price(amount: i32) -> i32 {",
                "    if amount < 0 { 0 } else { price(amount) }",
                "}",
                "",
                "#[test]",
                "fn inert_without_the_harness() {",
                "    assert_eq!(setup_price(100), 90);",
                "}",
                "",
                "fn trials() -> Vec<Trial> {",
                "    vec![Trial::test(\"price_at_boundary\", || {",
                "        assert_eq!(setup_price(100), 90);",
                "    })]",
                "}",
            ],
        );
        add_file(
            "/src/unregistered_helper.rs",
            &[
                "pub fn helper_path(amount: i32) -> i32 {",
                "    if amount < 0 { 0 } else { amount }",
                "}",
            ],
        );
        let changed_files = diff::parse_unified_diff(&diff_text);
        let result = RustAdapter.analyze_diff(
            &AnalysisOptions {
                root,
                base: None,
                diff_file: None,
                mode: AnalysisMode::Ready,
                resolved_subject_identity: None,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: vec![crate::config::TestHarnessRegistration {
                    registration_id: "mimic-suite".to_string(),
                    target: std::path::PathBuf::from("src/price_mimic.rs"),
                    kind: crate::config::TestHarnessKind::CustomHarnessTarget,
                    adapter: crate::config::TestHarnessAdapter::LibtestMimicV1,
                    marker: "libtest_mimic".to_string(),
                }],
            },
            &OraclePolicy::default(),
            &changed_files,
        )?;
        let finding_files = result
            .findings
            .iter()
            .map(|finding| {
                finding
                    .probe
                    .location
                    .file
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect::<Vec<_>>();
        assert!(
            finding_files
                .iter()
                .all(|file| !file.ends_with("src/price_mimic.rs")),
            "a registered harness target must not seed production probes: {finding_files:?}"
        );
        assert!(
            finding_files
                .iter()
                .any(|file| file.ends_with("src/unregistered_helper.rs")),
            "an unregistered sibling stays a production subject: {finding_files:?}"
        );
        // The harness registry's subject facts ride on the diff result.
        assert_eq!(result.harness_projections.len(), 1);
        assert_eq!(result.harness_projections[0].registration_id, "mimic-suite");
        assert!(
            result.harness_projections[0]
                .subjects
                .iter()
                .any(|subject| subject.name == "price_at_boundary"),
            "the exact trial registration is a projected subject"
        );
        Ok(())
    }

    #[test]
    fn diff_analysis_misdeclared_harness_target_keeps_seeding_and_records_the_conflict()
    -> Result<(), String> {
        // #3608: a `custom_harness` registration whose target does not
        // match any Cargo `[[test]]` target keeps seeding production
        // seams (no file-wide evidence role on an unverified premise) and
        // records the conflict as a typed limitation.
        let root = temp_root("misdeclared-harness-diff")?;
        write(
            &root.join("Cargo.toml"),
            // The manifest carries a [workspace] table so the fixture is a
            // standalone workspace root, and deliberately declares nothing
            // for src/price_mimic.rs: the registration below is misdeclared.
            "[package]\nname='registered-harness'\nversion='0.1.0'\nedition='2024'\n\n[workspace]\n",
        )?;
        write(
            &root.join("src/lib.rs"),
            "pub fn price(amount: i32) -> i32 {\n    if amount > 100 { amount - 10 } else { amount }\n}\n",
        )?;
        write(
            &root.join("src/price_mimic.rs"),
            "use libtest_mimic::Trial;\n\nfn setup_price(amount: i32) -> i32 {\n    if amount < 0 { 0 } else { price(amount) }\n}\n\nfn trials() -> Vec<Trial> {\n    vec![Trial::test(\"price_at_boundary\", || {\n        assert_eq!(setup_price(100), 90);\n    })]\n}\n",
        )?;
        write(
            &root.join("src/unregistered_helper.rs"),
            "pub fn helper_path(amount: i32) -> i32 {\n    if amount < 0 { 0 } else { amount }\n}\n",
        )?;
        let mut diff_text = String::new();
        let mut add_file = |path: &str, lines: &[&str]| {
            diff_text.push_str(&format!("diff --git a{path} b{path}\n"));
            diff_text.push_str("new file mode 100644\n");
            diff_text.push_str("--- /dev/null\n");
            diff_text.push_str(&format!("+++ b{path}\n"));
            diff_text.push_str(&format!("@@ -0,0 +1,{} @@\n", lines.len()));
            for line in lines {
                diff_text.push_str(&format!("+{line}\n"));
            }
        };
        add_file(
            "/src/lib.rs",
            &[
                "pub fn price(amount: i32) -> i32 {",
                "    if amount > 100 { amount - 10 } else { amount }",
                "}",
            ],
        );
        add_file(
            "/src/price_mimic.rs",
            &[
                "use libtest_mimic::Trial;",
                "",
                "fn setup_price(amount: i32) -> i32 {",
                "    if amount < 0 { 0 } else { price(amount) }",
                "}",
                "",
                "fn trials() -> Vec<Trial> {",
                "    vec![Trial::test(\"price_at_boundary\", || {",
                "        assert_eq!(setup_price(100), 90);",
                "    })]",
                "}",
            ],
        );
        add_file(
            "/src/unregistered_helper.rs",
            &[
                "pub fn helper_path(amount: i32) -> i32 {",
                "    if amount < 0 { 0 } else { amount }",
                "}",
            ],
        );
        let changed_files = diff::parse_unified_diff(&diff_text);
        let result = RustAdapter.analyze_diff(
            &AnalysisOptions {
                root,
                base: None,
                diff_file: None,
                mode: AnalysisMode::Ready,
                resolved_subject_identity: None,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: vec![crate::config::TestHarnessRegistration {
                    registration_id: "mimic-suite".to_string(),
                    target: std::path::PathBuf::from("src/price_mimic.rs"),
                    kind: crate::config::TestHarnessKind::CustomHarnessTarget,
                    adapter: crate::config::TestHarnessAdapter::LibtestMimicV1,
                    marker: "libtest_mimic".to_string(),
                }],
            },
            &OraclePolicy::default(),
            &changed_files,
        )?;
        let finding_files = result
            .findings
            .iter()
            .map(|finding| {
                finding
                    .probe
                    .location
                    .file
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect::<Vec<_>>();
        assert!(
            finding_files
                .iter()
                .any(|file| file.ends_with("src/price_mimic.rs")),
            "the misdeclared target keeps seeding production seams: {finding_files:?}"
        );
        let projection = result
            .harness_projections
            .iter()
            .find(|projection| projection.registration_id == "mimic-suite")
            .ok_or("missing harness projection")?;
        assert!(
            projection.subjects.is_empty(),
            "a misdeclared target establishes no trial subjects: {:?}",
            projection.subjects
        );
        assert!(
            projection.limitations.iter().any(|limitation| {
                limitation.code == "target_not_declared"
                    && limitation.detail.contains("src/price_mimic.rs")
            }),
            "the conflict is recorded with the target named: {:?}",
            projection.limitations
        );
        Ok(())
    }

    #[test]
    fn diff_analysis_opt_in_restores_production_like_analysis() -> Result<(), String> {
        // #3283: `production_like_targets` restores ordinary production
        // analysis for the selected target only.
        let root = temp_root("production-like-opt-in")?;
        write(
            &root.join("Cargo.toml"),
            "[package]\nname='opt-in'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        write(
            &root.join("src/lib.rs"),
            "pub fn price(amount: i32) -> i32 {\n    if amount > 100 { amount - 10 } else { amount }\n}\n",
        )?;
        write(
            &root.join("tests/api_contract.rs"),
            "pub fn contract_helper(amount: i32) -> i32 {\n    if amount < 0 { 0 } else { amount }\n}\n",
        )?;
        write(
            &root.join("tests/other.rs"),
            "pub fn other_helper(amount: i32) -> i32 {\n    if amount < 0 { 0 } else { amount }\n}\n",
        )?;
        let changed_files = diff::parse_unified_diff(
            "diff --git a/src/lib.rs b/src/lib.rs\n\
             new file mode 100644\n\
             --- /dev/null\n\
             +++ b/src/lib.rs\n\
             @@ -0,0 +1,3 @@\n\
             +pub fn price(amount: i32) -> i32 {\n\
             +    if amount > 100 { amount - 10 } else { amount }\n\
             +}\n\
             diff --git a/tests/api_contract.rs b/tests/api_contract.rs\n\
             new file mode 100644\n\
             --- /dev/null\n\
             +++ b/tests/api_contract.rs\n\
             @@ -0,0 +1,3 @@\n\
             +pub fn contract_helper(amount: i32) -> i32 {\n\
             +    if amount < 0 { 0 } else { amount }\n\
             +}\n\
             diff --git a/tests/other.rs b/tests/other.rs\n\
             new file mode 100644\n\
             --- /dev/null\n\
             +++ b/tests/other.rs\n\
             @@ -0,0 +1,3 @@\n\
             +pub fn other_helper(amount: i32) -> i32 {\n\
             +    if amount < 0 { 0 } else { amount }\n\
             +}\n",
        );
        let mut production_like = std::collections::BTreeSet::new();
        production_like.insert(std::path::PathBuf::from("tests/api_contract.rs"));
        let result = RustAdapter.analyze_diff(
            &AnalysisOptions {
                root,
                base: None,
                diff_file: None,
                mode: AnalysisMode::Ready,
                resolved_subject_identity: None,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: production_like,
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &changed_files,
        )?;
        let path_text = |finding: &crate::domain::Finding| {
            finding
                .probe
                .location
                .file
                .to_string_lossy()
                .replace('\\', "/")
        };
        assert!(
            result
                .findings
                .iter()
                .any(|finding| path_text(finding).ends_with("tests/api_contract.rs")),
            "the opted-in target is analyzed as production-like: {:?}",
            result.findings
        );
        assert!(
            result
                .findings
                .iter()
                .all(|finding| !path_text(finding).ends_with("tests/other.rs")),
            "sibling test targets stay evidence-only: {:?}",
            result.findings
        );
        Ok(())
    }
}
