//! Typed doctor/preflight report for machine-readable output (`ripr doctor --json`).
//!
//! See #1771 / #1614 / #1862. The report captures the core checks (root,
//! Cargo.toml, configuration, and tool availability) as typed `DoctorCheck`
//! values and leaves deeper sub-checks (languages, cache, Perl, and test
//! surfaces) for a follow-up projection. The structure here proves the dual
//! human/JSON projection without a massive one-shot refactor.
//!
//! `cli::commands::doctor` is an argv adapter only: it parses `--root` /
//! `--json`, calls into this module to evaluate the core checks and probe
//! tool availability, and prints either the JSON report or the human-prose
//! projection.

use crate::config::{CONFIG_FILE_NAME, RiprConfig, load_for_root};
use crate::domain::LanguageId;
use crate::output::path::human_path;
use crate::process_owner::OwnedProcess;
use serde::Serialize;
use std::path::Path;
use std::time::{Duration, Instant};

/// The closing line of a failing `ripr doctor` run.
///
/// It used to end `run `ripr doctor --help` for usage`. Help is not the
/// remedy for anything doctor reports: every failing check already prints its
/// own fix on its own `!` line (`rustup update stable`, an install command, a
/// config repair), so the closing line sent the reader away from the answer
/// they had just been given. The CLI path prints the same line for the same
/// state, so both read it from here rather than keeping two copies of the
/// text.
pub(crate) const DOCTOR_FAILED_LINE: &str =
    "! doctor checks failed; each `!` line above names the check and its fix\n";

/// First command doctor prints after the checks. Git-backed routes are only
/// recommended when the `tool_git` check actually passed (#4735).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DoctorFirstCommand {
    SavedDiff,
    Worktree,
    DefaultCheck,
}

impl DoctorFirstCommand {
    pub(crate) const SAVED_DIFF_LINE: &'static str = "ripr check --diff PATH";
    pub(crate) const WORKTREE_LINE: &'static str = "ripr check --base HEAD --worktree";
    pub(crate) const DEFAULT_LINE: &'static str = "ripr check";

    /// `dirty_worktree` is only evaluated when git can run, so a gitless
    /// environment is not probed (and not told to run `--worktree`).
    pub(crate) fn resolve(git_can_run: bool, dirty_worktree: impl FnOnce() -> bool) -> Self {
        if !git_can_run {
            Self::SavedDiff
        } else if dirty_worktree() {
            Self::Worktree
        } else {
            Self::DefaultCheck
        }
    }

    pub(crate) fn command_line(self) -> &'static str {
        match self {
            Self::SavedDiff => Self::SAVED_DIFF_LINE,
            Self::Worktree => Self::WORKTREE_LINE,
            Self::DefaultCheck => Self::DEFAULT_LINE,
        }
    }
}

/// Fail closed: only an explicit passing `tool_git` check means git can run.
pub(crate) fn git_tool_can_run(report: &DoctorReport) -> bool {
    report
        .checks
        .iter()
        .any(|check| check.name == "tool_git" && check.status == DoctorCheckStatus::Pass)
}

/// The single source of truth for which tools doctor probes for availability.
/// Both the evaluation (which actually spawns each tool to check it) and the
/// human-readable projection (which reads the resulting checks back out of
/// the report) iterate this list, so there is exactly one place that names
/// the probed tools.
pub(crate) const DOCTOR_TOOLS: [&str; 3] = ["git", "cargo", "rustc"];

/// The subset of [`DOCTOR_TOOLS`] that only a Rust root needs. `git` is
/// required for every root because diff scoping reads Git history.
const RUST_TOOLCHAIN_TOOLS: [&str; 2] = ["cargo", "rustc"];

const MINIMUM_RUSTC_VERSION: &str = env!("CARGO_PKG_RUST_VERSION");

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RustcVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

impl std::fmt::Display for RustcVersion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

fn parse_rustc_version(output: &str) -> Option<RustcVersion> {
    let version_token = output
        .trim_start()
        .strip_prefix("rustc ")?
        .split_whitespace()
        .next()?;
    let (core, suffix) = match version_token.find(['-', '+']) {
        Some(index) => (&version_token[..index], &version_token[index..]),
        None => (version_token, ""),
    };
    if !suffix.is_empty() && !valid_rustc_version_suffix(suffix) {
        return None;
    }
    let mut components = core.split('.');
    let major = components.next()?.parse().ok()?;
    let minor = components.next()?.parse().ok()?;
    let patch = components.next()?.parse().ok()?;
    if components.next().is_some() {
        return None;
    }
    Some(RustcVersion {
        major,
        minor,
        patch,
    })
}

fn valid_rustc_version_suffix(suffix: &str) -> bool {
    if suffix.is_empty() {
        return false;
    }
    let (prerelease, build, has_prerelease) = if let Some(remainder) = suffix.strip_prefix('-') {
        match remainder.split_once('+') {
            Some((prerelease, build)) => (prerelease, Some(build), true),
            None => (remainder, None, true),
        }
    } else if let Some(build) = suffix.strip_prefix('+') {
        ("", Some(build), false)
    } else {
        return false;
    };
    if prerelease.is_empty() && (has_prerelease || build.is_none())
        || (!prerelease.is_empty() && !prerelease.split('.').all(valid_prerelease_identifier))
    {
        return false;
    }
    build.is_none_or(|build| {
        !build.is_empty()
            && build
                .split('.')
                .all(|identifier| valid_semver_identifier(identifier, false))
    })
}

fn valid_prerelease_identifier(identifier: &str) -> bool {
    valid_semver_identifier(identifier, true)
        && !identifier.starts_with('-')
        && !identifier.ends_with('-')
}

fn valid_semver_identifier(identifier: &str, reject_numeric_leading_zero: bool) -> bool {
    !identifier.is_empty()
        && identifier
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
        && !(reject_numeric_leading_zero
            && identifier.len() > 1
            && identifier.starts_with('0')
            && identifier
                .chars()
                .all(|character| character.is_ascii_digit()))
}

fn minimum_rustc_version() -> Option<RustcVersion> {
    let mut components = MINIMUM_RUSTC_VERSION.split('.');
    let major = components.next()?.parse().ok()?;
    let minor = components.next()?.parse().ok()?;
    let patch = components
        .next()
        .unwrap_or("0")
        .split(['-', '+'])
        .next()?
        .parse()
        .ok()?;
    Some(RustcVersion {
        major,
        minor,
        patch,
    })
}

/// Why the local `rustc` is worth a word, split from whether the check passed.
///
/// `MINIMUM_RUSTC_VERSION` is ripr's own `rust-version`: what it takes to
/// **build** or install ripr from source. The already-running ripr binary's
/// built-in static analysis does not directly run `rustc`. Configured external
/// producers have their own prerequisites; this advisory does not establish
/// their compatibility.
///
/// A version below the minimum fails the source-build prerequisite; the
/// analysis profile projects that failure as advisory. An unreadable version
/// is also not evidence that source builds work.
enum RustcVersionVerdict {
    /// Parsed and at or above ripr's build minimum.
    Current,
    /// Parsed and below ripr's build minimum, with the line to disclose.
    BelowBuildMinimum(String),
    /// Not parseable, with the failure to report.
    Unreadable(String),
}

/// The below-minimum rustc note already states what that means for the
/// running binary's analysis; the analysis-profile advisory must not say it a
/// second time (clean-install walk, 0.11).
const RUSTC_ANALYSIS_SCOPE: &str = "The already-running ripr binary's built-in static analysis does not directly run rustc; configured external producers have their own prerequisites.";

fn validate_rustc_version(output: &str) -> RustcVersionVerdict {
    let Some(minimum) = minimum_rustc_version() else {
        return RustcVersionVerdict::Unreadable(format!(
            "declared package rust-version `{MINIMUM_RUSTC_VERSION}` could not be parsed; update Cargo.toml"
        ));
    };
    let Some(version) = parse_rustc_version(output) else {
        return RustcVersionVerdict::Unreadable(format!(
            "rustc version could not be parsed from `{}`; install Rust {minimum}+",
            output.trim()
        ));
    };
    if version < minimum {
        return RustcVersionVerdict::BelowBuildMinimum(format!(
            "{}; below ripr's build minimum {minimum}. That minimum is what building or installing ripr from source requires. {RUSTC_ANALYSIS_SCOPE} Run `rustup update stable` before building ripr from source.",
            output.trim()
        ));
    }
    RustcVersionVerdict::Current
}

/// How long a tool probe may run before it is terminated (#2183 review): a
/// broken or malicious shim must not hang `ripr doctor` forever.
///
/// Residual, documented (#2183 review): the deadline terminates the spawned
/// process itself, not a whole process tree. A shim that *detaches*
/// (double-fork/setsid) work can leave that work running after the probe
/// returns. Process-group termination needs either `unsafe` (forbidden in
/// this crate) or a new dependency, and doctor returns bounded regardless —
/// so the bounded-probe contract is honored while the detached-descendant
/// case is accepted here rather than hidden.
const DOCTOR_TOOL_TIMEOUT: Duration = Duration::from_secs(5);

/// The top-level doctor status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DoctorStatus {
    /// All top-level checks passed.
    Pass,
    /// One or more top-level checks failed.
    Fail,
}

/// The status of one top-level doctor check.
///
/// `Skipped` marks a check that does not apply to the selected root (for
/// example the Cargo/Rust toolchain checks on a Python- or TypeScript-only
/// root). It never fails the report, and its evidence says why the check was
/// not run, so a skipped check never reads as a verified pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DoctorCheckStatus {
    /// The check ran and passed.
    Pass,
    /// The check ran and failed; the report fails in the selected profile.
    Fail,
    /// The observed capability is unavailable, but is not required by this profile.
    Advisory,
    /// The check does not apply to this root and was not run.
    Skipped,
}

/// The requested doctor capability. An installed binary can analyze a Rust
/// workspace without compiling RIPR or running the project's verification.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DoctorProfile {
    #[default]
    Analysis,
    SourceBuild,
}

impl From<DoctorStatus> for DoctorCheckStatus {
    fn from(status: DoctorStatus) -> Self {
        match status {
            DoctorStatus::Pass => Self::Pass,
            DoctorStatus::Fail => Self::Fail,
        }
    }
}

/// A single typed doctor check (root, Cargo.toml, tool availability).
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct DoctorCheck {
    /// The check name (e.g. "root_directory", "cargo_toml", "tool_git").
    pub(crate) name: String,
    /// The check status.
    pub(crate) status: DoctorCheckStatus,
    /// Human-readable evidence (e.g. "Cargo.toml found at /workspace").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) evidence: Option<String>,
}

/// A text-based section from a deeper check (languages, cache, etc.).
/// Typed checks replace these incrementally.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct DoctorSection {
    /// The section name (e.g. "detected_languages", "cache_status").
    pub(crate) name: String,
    /// The captured text output.
    pub(crate) lines: Vec<String>,
}

/// The result of a language runtime probe. Primary runtimes for enabled
/// languages are required; detected preview runtimes that are not enabled,
/// and optional test/package runners, remain visible but advisory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct DoctorRuntimeProbe {
    pub(crate) language: String,
    pub(crate) tool: String,
    pub(crate) status: DoctorStatus,
    pub(crate) evidence: String,
    pub(crate) required: bool,
    pub(crate) hint: String,
}

/// The full doctor report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct DoctorReport {
    pub(crate) schema_version: &'static str,
    pub(crate) tool: &'static str,
    pub(crate) ripr_version: &'static str,
    pub(crate) ripr_build_msrv: &'static str,
    pub(crate) root: String,
    /// Which capability the top-level status evaluates.
    pub(crate) profile: DoctorProfile,
    pub(crate) status: DoctorStatus,
    pub(crate) checks: Vec<DoctorCheck>,
    pub(crate) sections: Vec<DoctorSection>,
    pub(crate) runtime_probes: Vec<DoctorRuntimeProbe>,
    /// Enabled language wire strings from the effective config (#2072):
    /// the typed surface the generated CI consumes instead of parsing the
    /// human "Enabled languages:" line.
    pub(crate) languages: Vec<String>,
    /// The running binary and the `ripr` on PATH (additive in schema `0.2`).
    /// The command adapter fills it; core evaluation leaves it unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) binary: Option<super::doctor_binary::DoctorBinaryIdentity>,
}

impl DoctorReport {
    pub(crate) const SCHEMA_VERSION: &'static str = "0.3";

    pub(crate) fn new(root: &str) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            tool: "ripr",
            ripr_version: env!("CARGO_PKG_VERSION"),
            ripr_build_msrv: MINIMUM_RUSTC_VERSION,
            root: root.to_string(),
            profile: DoctorProfile::Analysis,
            status: DoctorStatus::Pass,
            checks: Vec::new(),
            sections: Vec::new(),
            runtime_probes: Vec::new(),
            languages: Vec::new(),
            binary: None,
        }
    }

    /// Add a typed check and update the overall status.
    pub(crate) fn add_check(&mut self, name: &str, status: DoctorStatus, evidence: Option<String>) {
        if status == DoctorStatus::Fail {
            self.status = DoctorStatus::Fail;
        }
        self.checks.push(DoctorCheck {
            name: name.to_string(),
            status: status.into(),
            evidence,
        });
    }

    /// Record a check that does not apply to this root. It never changes the
    /// overall status; `evidence` must say why the check was skipped.
    pub(crate) fn add_skipped_check(&mut self, name: &str, evidence: String) {
        self.checks.push(DoctorCheck {
            name: name.to_string(),
            status: DoctorCheckStatus::Skipped,
            evidence: Some(evidence),
        });
    }

    pub(crate) fn add_advisory_check(&mut self, name: &str, evidence: String) {
        self.checks.push(DoctorCheck {
            name: name.to_string(),
            status: DoctorCheckStatus::Advisory,
            evidence: Some(evidence),
        });
    }

    /// Add a runtime probe and fail the report only when a required probe
    /// fails. Optional preview/tooling probes remain visible in JSON without
    /// turning a Rust-only or otherwise advisory setup into a false failure.
    pub(crate) fn add_runtime_probe(
        &mut self,
        language: &str,
        tool: &str,
        status: DoctorStatus,
        evidence: &str,
        required: bool,
        hint: &str,
    ) {
        if required && status == DoctorStatus::Fail {
            self.status = DoctorStatus::Fail;
        }
        self.runtime_probes.push(DoctorRuntimeProbe {
            language: language.to_string(),
            tool: tool.to_string(),
            status,
            evidence: evidence.to_string(),
            required,
            hint: hint.to_string(),
        });
    }

    /// Add a text-based section.
    #[cfg(test)]
    pub(crate) fn add_section(&mut self, name: &str, lines: Vec<String>) {
        self.sections.push(DoctorSection {
            name: name.to_string(),
            lines,
        });
    }

    /// Render the report as human-readable text (mirrors the existing prose output).
    #[cfg(test)]
    pub(crate) fn render_text(&self) -> String {
        let mut out = String::new();
        out.push_str("ripr doctor\n");
        out.push_str(&format!("- root: {}\n", self.root));
        out.push_str(&format!(
            "- RIPR {} (source build requires Rust {})\n",
            self.ripr_version, self.ripr_build_msrv
        ));
        for check in &self.checks {
            let icon = match check.status {
                DoctorCheckStatus::Pass => "✓",
                DoctorCheckStatus::Fail => "!",
                DoctorCheckStatus::Advisory => "~",
                DoctorCheckStatus::Skipped => "-",
            };
            if let Some(evidence) = &check.evidence {
                out.push_str(&format!("{icon} {evidence}\n"));
            } else {
                out.push_str(&format!("{icon} {}\n", check.name));
            }
        }
        for section in &self.sections {
            for line in &section.lines {
                out.push_str(line);
                out.push('\n');
            }
        }
        match self.status {
            DoctorStatus::Pass => out.push_str("✓ doctor checks passed\n"),
            DoctorStatus::Fail => out.push_str(DOCTOR_FAILED_LINE),
        }
        out
    }

    /// Render the report as JSON.
    pub(crate) fn render_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self)
            .map_err(|error| format!("failed to serialize doctor report: {error}"))
    }
}

/// Strip a `toml` parse error down to its first line.
///
/// `toml::de::Error`'s `Display` embeds the offending source excerpt and a
/// caret pointing at the failing column (e.g. `"...\n  |\n1 | [invalid\n  |
/// ^...\n"`). Echoing that excerpt into `ripr doctor --json` would leak
/// `ripr.toml` source text — which may contain repository-specific paths or
/// other content the caller did not intend to publish — into machine-readable
/// output that gets captured in CI logs, PR comments, and agent context
/// (RIPR-SPEC-0007, P2). The first line alone (path, "invalid ripr.toml",
/// parse location) is enough to act on and contains no source text, so
/// doctor's JSON evidence keeps only that line.
fn redact_config_parse_error(error: &str) -> String {
    error.lines().next().unwrap_or(error).trim().to_string()
}

/// Result of evaluating the doctor core checks, plus the raw config load
/// result for the human-readable projection (which prints the full local
/// error to the user's own terminal — not a machine-readable output surface,
/// so it is not subject to the RIPR-SPEC-0007 redaction above).
pub(crate) struct DoctorCoreEvaluation {
    pub(crate) report: DoctorReport,
    pub(crate) config: Result<RiprConfig, String>,
}

/// Evaluate the doctor core checks (root, Cargo.toml, config, tool
/// availability) and return just the typed report.
#[cfg(test)]
pub(crate) fn evaluate_doctor_core(root: &Path, detected: &[LanguageId]) -> DoctorReport {
    evaluate_doctor_core_with_config(root, detected).report
}

/// Whether the Rust toolchain checks (`Cargo.toml`, `cargo`, `rustc`) apply
/// to a doctor root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RustToolchainScope {
    /// Rust is in scope: a missing `Cargo.toml` or toolchain fails doctor.
    Required,
    /// Rust is not in scope for this root; the checks are reported as
    /// skipped with this reason instead of failing.
    NotInScope(String),
}

/// Decide whether the Rust toolchain checks apply to a root.
///
/// `detected` is doctor's marker scan of the root (`Cargo.toml` or `.rs`
/// files mark Rust) and `config` is the effective configuration, whose
/// enabled set already includes Python auto-enablement from
/// `config::load_for_root`. The rule stays fail-closed for Rust:
///
/// - an unloadable config keeps the checks required (the config check
///   already fails the report, and nothing proves Rust is out of scope);
/// - Rust absent from the effective enabled set puts it out of scope;
/// - detected Rust markers keep the checks required, so a Rust root without
///   `Cargo.toml` still fails;
/// - otherwise Rust is enabled only by default or by an explicit list that
///   also names another language. When another language is detected or
///   enabled, the root is that language's project and Rust is out of scope;
///   with no other language at all (an empty or wrong root) the checks stay
///   required, so the missing `Cargo.toml` is still reported as a failure.
///
/// `enabled = ["rust", "<language>"]` is what doctor's own enablement tip
/// and the preview docs tell users to write, so an explicit `rust` entry
/// alone cannot mean the root is a Rust project.
pub(crate) fn rust_toolchain_scope(
    config: &Result<RiprConfig, String>,
    detected: &[LanguageId],
) -> RustToolchainScope {
    let Ok(config) = config else {
        return RustToolchainScope::Required;
    };
    let enabled = config.languages().enabled();
    if !enabled.contains(&LanguageId::Rust) {
        return RustToolchainScope::NotInScope("Rust is not enabled in [languages]".to_string());
    }
    if detected.contains(&LanguageId::Rust) {
        return RustToolchainScope::Required;
    }
    let mut others: Vec<&'static str> = Vec::new();
    for language in detected.iter().chain(enabled) {
        if *language != LanguageId::Rust && !others.contains(&language.as_str()) {
            others.push(language.as_str());
        }
    }
    if others.is_empty() {
        return RustToolchainScope::Required;
    }
    RustToolchainScope::NotInScope(format!(
        "Rust not detected at this root (no Cargo.toml or .rs files); in scope: {}",
        others.join(", ")
    ))
}

/// Whether `root` is inside a Git work tree, or `None` when the probe could
/// not run at all.
///
/// A probe that never ran may not assert that a directory is not a
/// repository: git missing from `PATH`, or a spawn that times out, is a
/// different state from git running and reporting no work tree, and only the
/// second one has a repair the user can act on. `rev-parse` exiting nonzero
/// is git answering, so that arm reports `false` rather than the unknown.
fn is_inside_work_tree(root: &Path) -> Option<bool> {
    let output = crate::git::run_git_output_with_deadline(
        root,
        &["rev-parse", "--is-inside-work-tree"],
        Some(DOCTOR_TOOL_TIMEOUT),
    )
    .ok()?;
    if !output.status.success() {
        return Some(false);
    }
    Some(String::from_utf8_lossy(&output.stdout).trim() == "true")
}

/// Evaluate the doctor core checks and also return the raw config load
/// result, so the human-readable projection can print full local detail
/// without going through the redacted JSON evidence. `detected` is the
/// caller's marker scan of the root, used only to decide whether the Rust
/// toolchain checks apply (see [`rust_toolchain_scope`]).
#[cfg(test)]
pub(crate) fn evaluate_doctor_core_with_config(
    root: &Path,
    detected: &[LanguageId],
) -> DoctorCoreEvaluation {
    evaluate_doctor_core_with_config_for_profile(root, detected, DoctorProfile::Analysis)
}

pub(crate) fn evaluate_doctor_core_with_config_for_profile(
    root: &Path,
    detected: &[LanguageId],
    profile: DoctorProfile,
) -> DoctorCoreEvaluation {
    evaluate_doctor_core_with_probe_for_profile(root, detected, profile, doctor_tool_check_for_root)
}

#[cfg(test)]
fn evaluate_doctor_core_with_probe(
    root: &Path,
    detected: &[LanguageId],
    probe_tool: impl FnMut(&str, &Path) -> (DoctorStatus, String),
) -> DoctorCoreEvaluation {
    evaluate_doctor_core_with_probe_for_profile(root, detected, DoctorProfile::Analysis, probe_tool)
}

fn evaluate_doctor_core_with_probe_for_profile(
    root: &Path,
    detected: &[LanguageId],
    profile: DoctorProfile,
    mut probe_tool: impl FnMut(&str, &Path) -> (DoctorStatus, String),
) -> DoctorCoreEvaluation {
    let mut report = DoctorReport::new(&root.display().to_string());
    report.profile = profile;
    let config = load_for_root(root);
    let rust_scope = rust_toolchain_scope(&config, detected);
    if root.is_dir() {
        report.add_check(
            "root_directory",
            DoctorStatus::Pass,
            Some(format!("root directory exists at {}", human_path(root))),
        );
    } else {
        report.add_check(
            "root_directory",
            DoctorStatus::Fail,
            Some(format!(
                "root directory does not exist at {}",
                human_path(root)
            )),
        );
    }
    if let RustToolchainScope::NotInScope(reason) = &rust_scope {
        report.add_skipped_check("cargo_toml", format!("Cargo.toml check skipped: {reason}"));
    } else if root.join("Cargo.toml").exists() {
        report.add_check(
            "cargo_toml",
            DoctorStatus::Pass,
            Some(format!(
                "Cargo.toml found at {}",
                human_path(&root.join("Cargo.toml"))
            )),
        );
    } else {
        report.add_check(
            "cargo_toml",
            DoctorStatus::Fail,
            Some(format!("no Cargo.toml found at {}", human_path(root))),
        );
    }
    match is_inside_work_tree(root) {
        Some(true) => report.add_check(
            "git_repository",
            DoctorStatus::Pass,
            Some(format!("inside a Git work tree at {}", human_path(root))),
        ),
        Some(false) => report.add_check(
            "git_repository",
            DoctorStatus::Fail,
            Some(format!(
                "not inside a Git work tree at {}; the diff-scoped commands read committed \
                 history and cannot run here. For a repository-free scan, run `ripr check --root \
                 {} --format repo-exposure-md`",
                human_path(root),
                root.display()
            )),
        ),
        None => report.add_check(
            "git_repository",
            DoctorStatus::Fail,
            Some(format!(
                "could not determine whether {} is inside a Git work tree; the git tool check \
                 below carries the reason",
                human_path(root)
            )),
        ),
    }
    match &config {
        Ok(config) => report.add_check(
            "config",
            DoctorStatus::Pass,
            Some(match config.source_path() {
                Some(path) => format!("loaded {} at {}", CONFIG_FILE_NAME, human_path(path)),
                None => format!("{CONFIG_FILE_NAME} not found; using built-in defaults"),
            }),
        ),
        Err(error) => report.add_check(
            "config",
            DoctorStatus::Fail,
            Some(redact_config_parse_error(error)),
        ),
    }
    for tool in DOCTOR_TOOLS {
        let name = format!("tool_{tool}");
        match &rust_scope {
            RustToolchainScope::NotInScope(reason)
                if RUST_TOOLCHAIN_TOOLS.contains(&tool) && profile == DoctorProfile::Analysis =>
            {
                report.add_skipped_check(&name, format!("{tool} check skipped: {reason}"));
            }
            _ => {
                let (status, evidence) = probe_tool(tool, root);
                if RUST_TOOLCHAIN_TOOLS.contains(&tool)
                    && profile == DoctorProfile::Analysis
                    && status == DoctorStatus::Fail
                {
                    report.add_advisory_check(
                        &name,
                        analysis_advisory_toolchain_evidence(tool, &evidence),
                    );
                } else {
                    report.add_check(&name, status, Some(evidence));
                }
            }
        }
    }
    // Typed language surface for generated CI (#2072): mirror exactly the
    // effective enabled set the human projection prints.
    if let Ok(config) = &config {
        report.languages = config
            .languages()
            .enabled()
            .iter()
            .map(|language| language.as_str().to_string())
            .collect();
    }
    DoctorCoreEvaluation { report, config }
}

/// Probe a single tool's availability via `<tool> --version`.
/// Probe a tool that must NOT load project configuration (#2183 review,
/// CWE-829): `yarn --version` run in a repository checkout executes
/// repo-controlled code when the repo pins `.yarnrc.yml`/`yarnPath`, so
/// merely running `ripr doctor` in a hostile checkout would execute it.
/// The probe runs from the OS temp dir with config resolution disabled.
pub(crate) fn doctor_tool_check_isolated(tool: &str) -> (DoctorStatus, String) {
    let mut command = doctor_tool_command(tool);
    command
        .current_dir(std::env::temp_dir())
        .env("YARN_IGNORE_PATH", "1");
    doctor_tool_check_with_command(tool, command, DOCTOR_TOOL_TIMEOUT, None).into_public()
}

fn doctor_tool_command(tool: &str) -> std::process::Command {
    // Windows: std's program lookup for a bare `pnpm` only resolves
    // `pnpm.exe` on PATH, so a tool installed as a batch shim (npm/corepack
    // install `pnpm.cmd` and `yarn.cmd`) would be misreported as not
    // installed. When no `.exe` exists, run the resolved shim by full path;
    // std launches `.cmd`/`.bat` through cmd.exe with its batch-argument
    // escaping. Other platforms keep the plain tool name unchanged.
    let mut program = std::ffi::OsString::from(tool);
    if cfg!(windows) {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let pathext = std::env::var("PATHEXT").ok();
        let dirs: Vec<std::path::PathBuf> = std::env::split_paths(&path).collect();
        if let Some(shim) =
            resolve_windows_batch_shim(tool, &dirs, pathext.as_deref(), &|p| p.is_file())
        {
            // A relative PATH entry was checked against this process's
            // directory; pin that before a probe moves the child's cwd.
            program = std::path::absolute(&shim).unwrap_or(shim).into_os_string();
        }
    }
    std::process::Command::new(program)
}

/// Resolve a Windows batch shim (`<tool>.cmd`/`<tool>.bat`) on PATH for a bare
/// tool name that has no `<tool>.exe` anywhere on PATH. Returns `None` when the
/// native lookup should be used: the name already carries a path or
/// extension, a `.exe` exists (it wins, matching `Command`'s own lookup), or no
/// shim exists. Pure over its inputs so the policy is testable on any host.
fn resolve_windows_batch_shim(
    tool: &str,
    path_dirs: &[std::path::PathBuf],
    pathext: Option<&str>,
    is_file: &dyn Fn(&Path) -> bool,
) -> Option<std::path::PathBuf> {
    if tool.is_empty() || tool.contains(['/', '\\', '.']) {
        return None;
    }
    if path_dirs
        .iter()
        .any(|dir| is_file(&dir.join(format!("{tool}.exe"))))
    {
        return None;
    }
    // Only batch extensions, in PATHEXT order; PATHEXT's other entries
    // (`.com`, `.vbs`, `.js`, ...) are not run by the doctor.
    let batch_exts: Vec<String> = pathext
        .unwrap_or(".COM;.EXE;.BAT;.CMD")
        .split(';')
        .map(str::to_ascii_lowercase)
        .filter(|ext| ext == ".cmd" || ext == ".bat")
        .collect();
    path_dirs.iter().find_map(|dir| {
        batch_exts
            .iter()
            .map(|ext| dir.join(format!("{tool}{ext}")))
            .find(|candidate| is_file(candidate))
    })
}

#[cfg(test)]
pub(crate) fn doctor_tool_check(tool: &str) -> (DoctorStatus, String) {
    doctor_tool_check_with_timeout(tool, DOCTOR_TOOL_TIMEOUT)
}

fn doctor_tool_check_for_root(tool: &str, root: &Path) -> (DoctorStatus, String) {
    if RUST_TOOLCHAIN_TOOLS.contains(&tool)
        && let Some(file) = crate::config::repository_toolchain_path_pin(root)
    {
        return (
            DoctorStatus::Fail,
            crate::config::toolchain_path_pin_refusal(&file),
        );
    }
    doctor_tool_check_with_timeout_result_at(
        tool,
        DOCTOR_TOOL_TIMEOUT,
        doctor_tool_probe_dir(tool, root),
    )
    .into_public()
}

/// The directory a core tool probe runs in. `cargo` and `rustc` resolve
/// through rustup's per-directory toolchain selection (`rust-toolchain.toml`,
/// overrides), so both are probed in the selected root: that is the toolchain
/// a source build there uses and the `cargo` the analyzer's `cargo metadata`
/// probe runs. Other tools keep the caller's directory.
fn doctor_tool_probe_dir<'a>(tool: &str, root: &'a Path) -> Option<&'a Path> {
    RUST_TOOLCHAIN_TOOLS.contains(&tool).then_some(root)
}

/// Evidence for an unavailable Cargo/rustc capability under the analysis
/// profile. The installed binary's static analysis does not compile the
/// workspace, but it does read `cargo metadata` for the custom test-harness
/// target inventory, so a missing `cargo` withholds that evidence (the
/// harness verdict fails closed as `manifest_unavailable`). Doctor names that
/// degradation instead of implying analysis is unaffected.
fn analysis_advisory_toolchain_evidence(tool: &str, evidence: &str) -> String {
    if evidence.contains(RUSTC_ANALYSIS_SCOPE) {
        return evidence.to_string();
    }
    let analysis_effect = if tool == "cargo" {
        "static analysis continues, but evidence that reads `cargo metadata` in the selected root (custom test-harness target inventory) is withheld"
    } else {
        "the installed binary's static analysis does not run rustc"
    };
    format!(
        "{evidence}; {analysis_effect}; project verification and source builds require their own toolchain"
    )
}

#[cfg(test)]
fn doctor_tool_check_with_timeout(tool: &str, timeout: Duration) -> (DoctorStatus, String) {
    doctor_tool_check_with_timeout_result(tool, timeout).into_public()
}

#[cfg(test)]
fn doctor_tool_check_with_timeout_result(tool: &str, timeout: Duration) -> DoctorToolCheckResult {
    doctor_tool_check_with_timeout_result_at(tool, timeout, None)
}

fn doctor_tool_check_with_timeout_result_at(
    tool: &str,
    timeout: Duration,
    root: Option<&Path>,
) -> DoctorToolCheckResult {
    doctor_tool_check_with_command(tool, doctor_tool_command(tool), timeout, root)
}

fn doctor_tool_check_with_command(
    tool: &str,
    mut command: std::process::Command,
    timeout: Duration,
    root: Option<&Path>,
) -> DoctorToolCheckResult {
    command.arg("--version");
    crate::process_owner::forbid_rustup_auto_install(&mut command);
    if let Some(root) = root {
        command.current_dir(root);
    }
    doctor_tool_run_result(tool, timeout, run_doctor_tool(command, timeout))
}

fn doctor_tool_run_result(
    tool: &str,
    timeout: Duration,
    run: Result<std::process::Output, DoctorToolRunError>,
) -> DoctorToolCheckResult {
    match run {
        Ok(output) if output.status.success() => doctor_tool_check_success(tool, &output.stdout),
        Ok(output) => DoctorToolCheckResult::failure(doctor_exit_failure_evidence(tool, &output)),
        Err(DoctorToolRunError::TimedOut) => {
            DoctorToolCheckResult::failure(doctor_timeout_evidence(tool, timeout))
        }
        Err(DoctorToolRunError::CleanupFailed(end, cleanup)) => {
            let event = match end {
                DoctorProbeEnd::Exited => format!("{tool} exited"),
                DoctorProbeEnd::TimedOut => doctor_timeout_evidence(tool, timeout),
                DoctorProbeEnd::WaitFailed => format!("{tool} could not be waited on"),
            };
            DoctorToolCheckResult::failure(format!(
                "{event}; ripr could not confirm the probe's processes stopped and some may still be running: {cleanup}"
            ))
        }
        Err(DoctorToolRunError::Spawn(kind)) => doctor_spawn_failure(tool, kind),
        _ => DoctorToolCheckResult::failure(format!("{tool} not available")),
    }
}

/// Evidence for a probe that ran and exited non-zero. The tool exists, so
/// "not available" would be false; its stderr carries the real cause, such
/// as rustup's "toolchain ... is not installed" (#4734). The first `error:`
/// line wins, because rustup can print a `warn:` line first (duplicate
/// toolchain files); otherwise the first nonempty line.
fn doctor_exit_failure_evidence(tool: &str, output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut lines = stderr
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let first = lines.clone().next();
    match lines.find(|line| line.starts_with("error:")).or(first) {
        Some(line) => format!("{tool} --version failed ({}): {line}", output.status),
        None => format!("{tool} --version failed ({})", output.status),
    }
}

fn doctor_tool_check_success(tool: &str, stdout: &[u8]) -> DoctorToolCheckResult {
    let evidence = String::from_utf8_lossy(stdout).trim().to_string();
    if tool != "rustc" {
        return DoctorToolCheckResult::pass(evidence);
    }
    match validate_rustc_version(&evidence) {
        RustcVersionVerdict::Current => DoctorToolCheckResult::pass(evidence),
        RustcVersionVerdict::BelowBuildMinimum(note) => DoctorToolCheckResult::failure(note),
        RustcVersionVerdict::Unreadable(error) => DoctorToolCheckResult::failure(error),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct DoctorToolCheckResult {
    status: DoctorStatus,
    evidence: String,
    retryable_launch_failure: bool,
}

impl DoctorToolCheckResult {
    fn pass(evidence: String) -> Self {
        Self {
            status: DoctorStatus::Pass,
            evidence,
            retryable_launch_failure: false,
        }
    }

    fn failure(evidence: String) -> Self {
        Self {
            status: DoctorStatus::Fail,
            evidence,
            retryable_launch_failure: false,
        }
    }

    fn into_public(self) -> (DoctorStatus, String) {
        (self.status, self.evidence)
    }
}

fn doctor_spawn_failure(tool: &str, kind: std::io::ErrorKind) -> DoctorToolCheckResult {
    DoctorToolCheckResult {
        status: DoctorStatus::Fail,
        evidence: if kind == std::io::ErrorKind::NotFound && tool == "git" {
            crate::git::GIT_NOT_FOUND_ON_PATH_MESSAGE.to_string()
        } else if kind == std::io::ErrorKind::NotFound {
            format!("{tool} not available")
        } else {
            format!("{tool} could not be launched: {kind:?}")
        },
        retryable_launch_failure: doctor_spawn_failure_is_retryable(kind),
    }
}

fn doctor_spawn_failure_is_retryable(kind: std::io::ErrorKind) -> bool {
    // The transient launch-failure class (#2242): resource exhaustion or
    // exec races under load. Observed in the wild as both WouldBlock and
    // ExecutableFileBusy under full-suite parallelism. NotFound (missing
    // tool) and PermissionDenied (not executable) are persistent and must
    // fail immediately.
    matches!(
        kind,
        std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::ExecutableFileBusy
            | std::io::ErrorKind::OutOfMemory
    )
}

fn doctor_timeout_evidence(tool: &str, timeout: Duration) -> String {
    let milliseconds = timeout.as_millis();
    if milliseconds < 1_000 || !milliseconds.is_multiple_of(1_000) {
        format!("{tool} timed out after {milliseconds}ms")
    } else {
        format!("{tool} timed out after {}s", timeout.as_secs())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DoctorToolRunError {
    Spawn(std::io::ErrorKind),
    Wait,
    TimedOut,
    /// The owner could not confirm the probe tree was stopped after the
    /// named event; part of it may still be running.
    CleanupFailed(DoctorProbeEnd, String),
}

/// What ended a probe before its tree cleanup, so a cleanup failure names
/// the real event instead of always reading as a timeout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DoctorProbeEnd {
    Exited,
    TimedOut,
    WaitFailed,
}

/// Run one doctor probe under the shared owned-subprocess authority
/// (#3803) so a timeout ends the whole tree, not only the direct child. On
/// Windows a `.cmd`/`.bat` shim (pnpm, yarn) runs as `cmd.exe /c`, and
/// killing `cmd.exe` alone left a hung node grandchild running after doctor
/// reported the timeout; the owner's Job Object takes the grandchild with
/// it. Other platforms keep the direct-child kill.
///
/// Both pipes drain on reader threads while the probe runs, so a verbose
/// tool cannot fill the pipe buffer and read as a false timeout.
fn run_doctor_tool(
    mut command: std::process::Command,
    timeout: Duration,
) -> Result<std::process::Output, DoctorToolRunError> {
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child =
        OwnedProcess::spawn(command).map_err(|err| DoctorToolRunError::Spawn(err.kind()))?;
    let stdout = child.stdout_pipe().take().map(spawn_doctor_pipe_reader);
    let stderr = child.stderr_pipe().take().map(spawn_doctor_pipe_reader);
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Ending the tree before the joins takes down any
                // descendant still holding a pipe on Windows, so the readers
                // reach EOF instead of waiting on it. A failed termination
                // returns here: joining behind a live descendant could block
                // doctor indefinitely.
                if let Err(cleanup) = child.terminate_tree() {
                    return Err(DoctorToolRunError::CleanupFailed(
                        DoctorProbeEnd::Exited,
                        cleanup,
                    ));
                }
                drop(child);
                return Ok(std::process::Output {
                    status,
                    stdout: join_doctor_pipe_reader(stdout)?,
                    stderr: join_doctor_pipe_reader(stderr)?,
                });
            }
            Ok(None) if started.elapsed() >= timeout => {
                // Readers are detached: terminating the tree closes every
                // write end, so they finish on their own.
                return Err(match child.terminate_tree() {
                    Ok(()) => DoctorToolRunError::TimedOut,
                    Err(cleanup) => {
                        DoctorToolRunError::CleanupFailed(DoctorProbeEnd::TimedOut, cleanup)
                    }
                });
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => {
                return Err(match child.terminate_tree() {
                    Ok(()) => DoctorToolRunError::Wait,
                    Err(cleanup) => {
                        DoctorToolRunError::CleanupFailed(DoctorProbeEnd::WaitFailed, cleanup)
                    }
                });
            }
        }
    }
}

type DoctorPipeReader = std::thread::JoinHandle<std::io::Result<Vec<u8>>>;

/// Bytes kept per probe stream. A `--version` line is far shorter; the rest
/// is drained and dropped so a tool that floods its pipe cannot grow
/// doctor's memory until the deadline.
const DOCTOR_PIPE_RETAIN_BYTES: usize = 64 * 1024;

fn spawn_doctor_pipe_reader(pipe: impl std::io::Read + Send + 'static) -> DoctorPipeReader {
    std::thread::spawn(move || drain_doctor_pipe(pipe, DOCTOR_PIPE_RETAIN_BYTES))
}

/// Read `pipe` to EOF, keeping at most `retain` bytes.
fn drain_doctor_pipe(mut pipe: impl std::io::Read, retain: usize) -> std::io::Result<Vec<u8>> {
    let mut kept = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let read = match pipe.read(&mut chunk) {
            Ok(0) => return Ok(kept),
            Ok(read) => read,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err),
        };
        let room = retain.saturating_sub(kept.len());
        kept.extend_from_slice(chunk.get(..read.min(room)).unwrap_or_default());
    }
}

fn join_doctor_pipe_reader(
    reader: Option<DoctorPipeReader>,
) -> Result<Vec<u8>, DoctorToolRunError> {
    match reader {
        None => Ok(Vec::new()),
        Some(handle) => match handle.join() {
            Ok(Ok(buffer)) => Ok(buffer),
            Ok(Err(_)) | Err(_) => Err(DoctorToolRunError::Wait),
        },
    }
}

/// Translate the report's overall status into the doctor command's exit
/// result.
pub(crate) fn doctor_report_result(report: &DoctorReport) -> Result<(), String> {
    match report.status {
        DoctorStatus::Pass => Ok(()),
        DoctorStatus::Fail => Err("doctor found issues".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn below_minimum_rustc_output() -> Result<String, String> {
        let minimum = minimum_rustc_version()
            .ok_or_else(|| "minimum rustc version should parse".to_string())?;
        let below = if minimum.patch > 0 {
            format!("{}.{}.{}", minimum.major, minimum.minor, minimum.patch - 1)
        } else if minimum.minor > 0 {
            format!("{}.{}.0", minimum.major, minimum.minor - 1)
        } else if minimum.major > 0 {
            format!("{}.99.0", minimum.major - 1)
        } else {
            return Err("cannot construct a version below 0.0.0".to_string());
        };
        Ok(format!("rustc {below} (abc 2024-01-01)"))
    }

    fn unique_test_dir(label: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "ripr-output-doctor-{label}-{}-{stamp}-{}",
            std::process::id(),
            TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
        ))
    }

    /// Run `git` in `dir` and return its trimmed stdout.
    fn git_in(dir: &std::path::Path, args: &[&str]) -> Result<String, String> {
        let output = crate::git::run_git_output_with_deadline(dir, args, Some(DOCTOR_TOOL_TIMEOUT))
            .map_err(|error| format!("git {args:?} in {}: {error}", dir.display()))?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    #[test]
    fn a_root_outside_a_git_work_tree_is_named_as_such() -> Result<(), String> {
        // A bare repository is not a work tree wherever the fixture lands.
        // `std::env::temp_dir()` resolves inside this checkout in some
        // environments, so a plain empty directory would be a work tree and
        // this test would prove nothing. The construction check below is what
        // catches that, and it is the reason the fixture is a bare repository.
        let mut failures = Vec::new();

        // Two fixtures, because git answers this question two different ways
        // and the reported case is the second. A bare repository prints
        // `false` and exits 0; a directory git cannot read as a repository
        // exits nonzero, which is the arm a plain directory outside any
        // checkout takes. A plain directory is not usable as a fixture here:
        // the temp root resolves inside this checkout in some environments,
        // where it would be a work tree.
        let bare = unique_test_dir("outside-work-tree-bare");
        std::fs::create_dir_all(&bare).map_err(|error| format!("create fixture: {error}"))?;
        git_in(&bare, &["init", "--bare", "."])?;

        let gitfile = unique_test_dir("outside-work-tree-gitfile");
        std::fs::create_dir_all(&gitfile).map_err(|error| format!("create fixture: {error}"))?;
        std::fs::write(gitfile.join(".git"), "not a gitfile\n")
            .map_err(|error| format!("write fixture gitfile: {error}"))?;

        for fixture in [&bare, &gitfile] {
            let inside =
                git_in(fixture, &["rev-parse", "--is-inside-work-tree"]).unwrap_or_default();
            if inside == "true" {
                failures.push(format!(
                    "fixture {} is inside a work tree, so it proves nothing",
                    fixture.display()
                ));
                continue;
            }
            let report = evaluate_doctor_core_with_config(fixture, &[]).report;
            match report
                .checks
                .iter()
                .find(|check| check.name == "git_repository")
            {
                None => failures.push(format!(
                    "{}: no git_repository check was reported",
                    fixture.display()
                )),
                Some(check) => {
                    if check.status != DoctorStatus::Fail.into() {
                        failures.push(format!(
                            "{}: a root outside a work tree reported {:?}",
                            fixture.display(),
                            check.status
                        ));
                    }
                    let evidence = check.evidence.as_deref().unwrap_or_default();
                    // The line has to carry the state, the consequence, and a
                    // command that works where the user is standing. Advice
                    // that cannot run there is what this check replaces.
                    for expected in [
                        "not inside a Git work tree",
                        "cannot run here",
                        "--format repo-exposure-md",
                    ] {
                        if !evidence.contains(expected) {
                            failures.push(format!("`{evidence}` does not say `{expected}`"));
                        }
                    }
                }
            }
        }

        // Positive control: this checkout is a work tree, so the same check
        // must pass here. Without it the test would also pass against a check
        // that always fails.
        let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        match evaluate_doctor_core_with_config(checkout, &[])
            .report
            .checks
            .iter()
            .find(|check| check.name == "git_repository")
        {
            Some(check) if check.status == DoctorStatus::Pass.into() => {}
            other => failures.push(format!(
                "this checkout should report a work tree, reported {other:?}"
            )),
        }

        let _ = std::fs::remove_dir_all(&bare);
        let _ = std::fs::remove_dir_all(&gitfile);
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("\n"))
        }
    }

    fn shim_dirs() -> Vec<std::path::PathBuf> {
        vec![
            std::path::PathBuf::from("first-bin"),
            std::path::PathBuf::from("npm-global"),
        ]
    }

    #[test]
    fn windows_shim_resolves_cmd_when_no_exe_exists() {
        let dirs = shim_dirs();
        let shim = dirs[1].join("pnpm.cmd");
        let resolved =
            resolve_windows_batch_shim("pnpm", &dirs, Some(".COM;.EXE;.BAT;.CMD"), &|p| {
                p == shim.as_path()
            });
        assert_eq!(resolved, Some(shim.clone()));
        // PATHEXT absent falls back to the Windows default list.
        assert_eq!(
            resolve_windows_batch_shim("pnpm", &dirs, None, &|p| p == shim.as_path()),
            Some(shim)
        );
    }

    #[test]
    fn windows_shim_defers_to_exe_anywhere_on_path() {
        let dirs = shim_dirs();
        let cmd = dirs[0].join("yarn.cmd");
        let exe = dirs[1].join("yarn.exe");
        let resolved = resolve_windows_batch_shim("yarn", &dirs, Some(".EXE;.CMD"), &|p| {
            p == cmd.as_path() || p == exe.as_path()
        });
        assert_eq!(resolved, None);
    }

    #[test]
    fn windows_shim_absent_stays_unresolved() {
        let dirs = shim_dirs();
        assert_eq!(
            resolve_windows_batch_shim("pnpm", &dirs, Some(".EXE;.CMD"), &|_| false),
            None
        );
        // A non-batch PATHEXT match is not run as a shim.
        let js = dirs[0].join("pnpm.js");
        assert_eq!(
            resolve_windows_batch_shim("pnpm", &dirs, Some(".JS;.EXE"), &|p| p == js.as_path()),
            None
        );
        // Names that already carry a path or extension use native lookup.
        assert_eq!(
            resolve_windows_batch_shim(r"npm-global\pnpm", &dirs, None, &|_| true),
            None
        );
    }

    #[test]
    fn windows_shim_honours_pathext_order_within_a_directory() {
        let dirs = shim_dirs();
        let bat = dirs[0].join("yarn.bat");
        let cmd = dirs[0].join("yarn.cmd");
        let exists = |p: &Path| p == bat.as_path() || p == cmd.as_path();
        assert_eq!(
            resolve_windows_batch_shim("yarn", &dirs, Some(".EXE;.CMD;.BAT"), &exists),
            Some(cmd.clone())
        );
        assert_eq!(
            resolve_windows_batch_shim("yarn", &dirs, Some(".EXE;.BAT;.CMD"), &exists),
            Some(bat)
        );
    }

    #[test]
    fn empty_report_is_pass() {
        let report = DoctorReport::new("/workspace");
        assert_eq!(report.status, DoctorStatus::Pass);
        assert!(report.checks.is_empty());
    }

    #[test]
    fn failed_check_flips_overall_status() {
        let mut report = DoctorReport::new("/workspace");
        report.add_check(
            "root_directory",
            DoctorStatus::Pass,
            Some("root exists".to_string()),
        );
        assert_eq!(report.status, DoctorStatus::Pass);
        report.add_check(
            "cargo_toml",
            DoctorStatus::Fail,
            Some("no Cargo.toml".to_string()),
        );
        assert_eq!(report.status, DoctorStatus::Fail);
    }

    /// A toolchain below ripr's own `rust-version` is disclosed as advisory
    /// by the analysis profile, but fails the source-build capability:
    /// that minimum is what building or installing ripr from source takes,
    /// while the already-running binary's built-in static analysis does not
    /// directly run `rustc`. The malformed-output and parser controls below
    /// ensure that this advisory cannot hide an unknown version.
    #[test]
    fn rustc_below_build_minimum_is_disclosed_and_supported_versions_pass() -> Result<(), String> {
        let minimum = minimum_rustc_version()
            .ok_or_else(|| {
                "minimum rustc version should parse for the disclosure test".to_string()
            })?
            .to_string();
        let below = below_minimum_rustc_output()?;
        let cases = [
            (
                below.clone(),
                DoctorStatus::Fail,
                "below ripr's build minimum",
            ),
            (
                format!("rustc {minimum} (abc 2026-04-14)"),
                DoctorStatus::Pass,
                "",
            ),
            (
                format!("rustc {minimum}-nightly (abc 2026-05-01)"),
                DoctorStatus::Pass,
                "",
            ),
            (
                format!("rustc {minimum}+build.1 (abc 2026-05-01)"),
                DoctorStatus::Pass,
                "",
            ),
        ];
        for (evidence, expected_status, expected_fragment) in cases {
            let result = doctor_tool_check_success("rustc", evidence.as_bytes());
            if result.status != expected_status {
                return Err(format!(
                    "unexpected status for {evidence:?}: {:?}",
                    result.status
                ));
            }
            if !result.evidence.contains(expected_fragment) {
                return Err(format!(
                    "missing expected evidence for {evidence:?}: {:?}",
                    result.evidence
                ));
            }
        }

        // Discriminator 1: the below-minimum case must keep the actual and
        // minimum versions, the build/install and built-in-analysis scopes,
        // external-producer limitation, and an action, or the note is not
        // usable.
        let old_toolchain = doctor_tool_check_success("rustc", below.as_bytes());
        for expected in [
            "below ripr's build minimum",
            &minimum,
            "building or installing ripr from source",
            "built-in static analysis does not directly run rustc",
            "configured external producers have their own prerequisites",
            "rustup update stable",
        ] {
            if !old_toolchain.evidence.contains(expected) {
                return Err(format!(
                    "the disclosure must contain {expected:?}: {:?}",
                    old_toolchain.evidence
                ));
            }
        }
        if !old_toolchain.evidence.contains("rustc ") {
            return Err(format!(
                "the disclosure must name the actual rustc version: {:?}",
                old_toolchain.evidence
            ));
        }

        // Discriminator 2: a current toolchain must not carry the note, so
        // the disclosure cannot be unconditional text.
        let current = format!("rustc {minimum} (abc 2026-04-14)");
        let current = doctor_tool_check_success("rustc", current.as_bytes());
        if current.evidence.contains("below ripr's build minimum") {
            return Err(format!(
                "a supported toolchain must not be disclosed as below the minimum: {:?}",
                current.evidence
            ));
        }
        Ok(())
    }

    #[test]
    fn rustc_version_check_fails_closed_for_malformed_output() -> Result<(), String> {
        for output in [
            "rustc unavailable",
            "rustc 1.80.0-",
            "rustc 1.80.0+",
            "rustc 1.80.0-+",
            "rustc 1.80.0-+build",
            "rustc 1.80.0--",
            "rustc 1.80.0+build+extra",
            "rustc 1.80.0-nightly..1",
            "rustc 1.80.0+build..1",
            "rustc 1.80.0-nightly+build.",
        ] {
            let result = doctor_tool_check_success("rustc", output.as_bytes());
            if result.status != DoctorStatus::Fail {
                return Err(format!(
                    "malformed rustc output unexpectedly passed for {output:?}: {result:?}"
                ));
            }
            if !result.evidence.contains("could not be parsed") {
                return Err(format!(
                    "unexpected malformed-output evidence for {output:?}: {:?}",
                    result.evidence
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn analysis_advisory_states_the_rustc_analysis_scope_once() -> Result<(), String> {
        let below = below_minimum_rustc_output()?;
        let rustc = doctor_tool_check_success("rustc", below.as_bytes());
        let advisory = analysis_advisory_toolchain_evidence("rustc", &rustc.evidence);
        if advisory.matches("run rustc").count() != 1 {
            return Err(format!(
                "the rustc analysis scope must appear once: {advisory:?}"
            ));
        }
        let missing = analysis_advisory_toolchain_evidence("rustc", "rustc not found on PATH");
        if !missing.contains("the installed binary's static analysis does not run rustc") {
            return Err(format!(
                "a missing rustc still needs the analysis scope: {missing:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn below_minimum_rustc_advises_through_public_doctor_projection() -> Result<(), String> {
        let below = below_minimum_rustc_output()?;
        let rustc = doctor_tool_check_success("rustc", below.as_bytes());
        let mut report = DoctorReport::new("/workspace");
        report.add_advisory_check("tool_rustc", rustc.evidence.clone());

        if report.status != DoctorStatus::Pass {
            return Err(format!("below-minimum advisory must pass: {report:?}"));
        }
        if let Err(error) = doctor_report_result(&report) {
            return Err(format!("doctor result must pass: {error}"));
        }
        let text = report.render_text();
        for expected in [
            "doctor checks passed",
            &below,
            "below ripr's build minimum",
            "building or installing ripr from source",
            "built-in static analysis does not directly run rustc",
            "configured external producers have their own prerequisites",
            "rustup update stable",
        ] {
            if !text.contains(expected) {
                return Err(format!("rendered text is missing {expected:?}: {text}"));
            }
        }
        let json = report.render_json()?;
        let parsed: serde_json::Value =
            serde_json::from_str(&json).map_err(|error| format!("invalid JSON: {error}"))?;
        if parsed["status"] != "pass" || parsed["checks"][0]["name"] != "tool_rustc" {
            return Err(format!(
                "unexpected doctor JSON status projection: {parsed}"
            ));
        }
        if parsed["checks"][0]["status"] != "advisory" {
            return Err(format!("rustc check must render as advisory: {parsed}"));
        }
        let evidence = parsed["checks"][0]["evidence"]
            .as_str()
            .ok_or_else(|| format!("rustc evidence must be rendered: {parsed}"))?;
        for expected in [
            &below,
            "building or installing ripr from source",
            "built-in static analysis does not directly run rustc",
            "configured external producers have their own prerequisites",
            "rustup update stable",
        ] {
            if !evidence.contains(expected) {
                return Err(format!(
                    "rendered JSON evidence is missing {expected:?}: {parsed}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn rustc_version_parser_covers_invalid_prefix_and_components() {
        for output in [
            "",
            "cargo 1.95.0",
            "rustc",
            "rustc ",
            "rustc x.95.0",
            "rustc 1.x.0",
            "rustc 1.95",
            "rustc 1.95.x",
            "rustc 1.95.-nightly",
            "rustc 1.95.0-",
            "rustc 1.95.0+",
            "rustc 1.95.0-+",
            "rustc 1.95.0-+build",
            "rustc 1.95.0--",
            "rustc 1.95.0+build+extra",
            "rustc 1.95.0-nightly..1",
            "rustc 1.95.0+build..1",
            "rustc 1.95.0-nightly+build.",
        ] {
            assert!(
                parse_rustc_version(output).is_none(),
                "malformed rustc output unexpectedly parsed: {output:?}"
            );
        }
        for output in [
            "rustc 1.95.0-nightly",
            "rustc 1.95.0-beta.1",
            "rustc 1.95.0+build.1",
            "rustc 1.95.0-nightly+build.01",
        ] {
            assert!(
                parse_rustc_version(output).is_some(),
                "valid rustc suffix unexpectedly rejected: {output:?}"
            );
        }
        assert_eq!(
            doctor_tool_check_success("cargo", b"cargo 1.95.0").status,
            DoctorStatus::Pass
        );
    }

    #[test]
    fn rustc_doctor_probes_apply_the_version_gate() {
        let (status, evidence) = doctor_tool_check("rustc");
        assert_eq!(status, DoctorStatus::Pass, "{evidence}");
        assert!(evidence.starts_with("rustc "), "{evidence}");

        let (isolated_status, isolated_evidence) = doctor_tool_check_isolated("rustc");
        assert_eq!(isolated_status, DoctorStatus::Pass, "{isolated_evidence}");
        assert!(
            isolated_evidence.starts_with("rustc "),
            "{isolated_evidence}"
        );
    }

    #[test]
    fn rustc_doctor_probe_from_selected_root_applies_the_version_gate() {
        let (status, evidence) = doctor_tool_check_for_root("rustc", &std::env::temp_dir());
        assert_eq!(status, DoctorStatus::Pass, "{evidence}");
        assert!(evidence.starts_with("rustc "), "{evidence}");
    }

    #[cfg(unix)]
    #[test]
    fn doctor_rustc_probe_uses_selected_root() -> Result<(), String> {
        let dir = unique_test_dir("selected-root-rustc");
        let selected_root = dir.join("selected-root");
        std::fs::create_dir_all(&selected_root).map_err(|err| format!("create root: {err}"))?;
        let shim = publish_doctor_test_tool(
            &dir,
            "rustc-root-probe",
            "#!/bin/sh\ncase \"$PWD\" in\n  *selected-root) printf 'rustc 1.94.0 (target-root)\\n' ;;\n  *) printf 'rustc 1.96.0 (caller-root)\\n' ;;\nesac\n",
        )?;
        let shim_str: &str = shim
            .to_str()
            .ok_or_else(|| "shim path is not UTF-8".to_string())?;

        // #3073: publishing a shim and immediately execing it is exposed to
        // the #2242/#2378 exec-contention family under full-suite parallelism
        // (a retryable launch failure flips the verdict without proving
        // anything about root selection), and the 5s production timeout can
        // elapse on a loaded host before /bin/sh even starts. Both produce
        // the same observable — evidence without the selected root's marker —
        // so the oracle is made load-independent: the spawn goes through the
        // shared bounded retry (only a retryable launch failure is retried,
        // never a real verdict) under a generous test ceiling instead of the
        // production constant.
        let result = probe_published_tool_with_command(
            "rustc",
            || doctor_tool_command(shim_str),
            SHIM_PROBE_TEST_CEILING,
            Some(&selected_root),
        );
        let _ = std::fs::remove_dir_all(&dir);

        // The subject here is which directory the probe ran in, so the oracle
        // is the shim's own per-directory marker rather than the verdict: the
        // selected root prints `target-root`, the caller root `caller-root`.
        // The probe reports the source-build prerequisite as failed; the
        // analysis profile projects that observation as advisory. The root
        // discriminator remains the shim's own per-directory marker.
        assert!(
            result.evidence.contains("target-root"),
            "probe must run in the selected root; evidence: {}",
            result.evidence
        );
        assert!(
            !result.evidence.contains("caller-root"),
            "probe must not run in the caller root; evidence: {}",
            result.evidence
        );
        assert_eq!(
            result.status,
            DoctorStatus::Fail,
            "evidence: {}",
            result.evidence
        );
        assert!(
            result.evidence.contains("below ripr's build minimum"),
            "the selected root's 1.94.0 must still be disclosed; evidence: {}",
            result.evidence
        );
        Ok(())
    }

    #[test]
    fn render_text_shows_pass_and_fail_checks() {
        let mut report = DoctorReport::new("/workspace");
        report.add_check(
            "root_directory",
            DoctorStatus::Pass,
            Some("root exists".to_string()),
        );
        report.add_check(
            "cargo_toml",
            DoctorStatus::Fail,
            Some("no Cargo.toml".to_string()),
        );
        report.add_section("guidance", vec!["run ripr doctor --help".to_string()]);
        let text = report.render_text();
        assert!(text.contains("✓ root exists"));
        assert!(text.contains("! no Cargo.toml"));
        assert!(text.contains("run ripr doctor --help"));
        assert!(text.contains("! doctor checks failed"));
        // The remedy is on the failing check's own line, so the closing line
        // must not send the reader to help text instead.
        assert!(
            !text.contains("for usage"),
            "the closing line must not point at help text: {text}"
        );
    }

    #[test]
    fn required_runtime_probe_failure_fails_report_but_optional_does_not() -> Result<(), String> {
        let mut report = DoctorReport::new("/workspace");
        report.add_runtime_probe(
            "python",
            "python3",
            DoctorStatus::Fail,
            "python3 not available",
            false,
            "install python3",
        );
        assert_eq!(report.status, DoctorStatus::Pass);
        report.add_runtime_probe(
            "python",
            "python3",
            DoctorStatus::Fail,
            "python3 not available",
            true,
            "install python3",
        );
        assert_eq!(report.status, DoctorStatus::Fail);
        let parsed: serde_json::Value = serde_json::from_str(&report.render_json()?)
            .map_err(|error| format!("invalid JSON: {error}"))?;
        assert_eq!(parsed["runtime_probes"][0]["required"], false);
        assert_eq!(parsed["runtime_probes"][1]["required"], true);
        assert_eq!(parsed["runtime_probes"][1]["status"], "fail");
        Ok(())
    }

    /// Build a doctor root holding `files` (relative path, contents).
    fn doctor_scope_root(
        label: &str,
        files: &[(&str, &str)],
    ) -> Result<std::path::PathBuf, String> {
        let root = unique_test_dir(label);
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        for (relative, contents) in files {
            let path = root.join(relative);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|err| format!("create parent: {err}"))?;
            }
            std::fs::write(&path, contents).map_err(|err| format!("write {relative}: {err}"))?;
        }
        Ok(root)
    }

    /// Evaluate a root with a probe where every tool except `git` is
    /// missing, recording which tools doctor actually probed.
    fn evaluate_without_rust_toolchain(
        root: &Path,
        detected: &[LanguageId],
    ) -> (DoctorReport, Vec<String>) {
        let mut probed = Vec::new();
        let report = evaluate_doctor_core_with_probe(root, detected, |tool, _root| {
            probed.push(tool.to_string());
            if tool == "git" {
                (DoctorStatus::Pass, "git version 2.43.0".to_string())
            } else {
                (DoctorStatus::Fail, format!("{tool} not available"))
            }
        })
        .report;
        (report, probed)
    }

    fn check<'a>(report: &'a DoctorReport, name: &str) -> Result<&'a DoctorCheck, String> {
        report
            .checks
            .iter()
            .find(|check| check.name == name)
            .ok_or_else(|| format!("missing {name} check: {:?}", report.checks))
    }

    /// The three Rust toolchain checks are skipped (with a reason naming
    /// `reason_fragment`), never probed, and the report passes although
    /// neither cargo nor rustc is available.
    fn assert_rust_toolchain_skipped(
        report: &DoctorReport,
        probed: &[String],
        reason_fragment: &str,
    ) -> Result<(), String> {
        for name in ["cargo_toml", "tool_cargo", "tool_rustc"] {
            let skipped = check(report, name)?;
            if skipped.status != DoctorCheckStatus::Skipped {
                return Err(format!("{name} was not skipped: {skipped:?}"));
            }
            let evidence = skipped.evidence.as_deref().unwrap_or_default();
            if !evidence.contains("skipped: ") || !evidence.contains(reason_fragment) {
                return Err(format!("{name} evidence does not say why: {evidence:?}"));
            }
        }
        if probed != ["git"] {
            return Err(format!("only git may be probed, probed {probed:?}"));
        }
        if report.status != DoctorStatus::Pass {
            return Err(format!("report must pass: {:?}", report.checks));
        }
        let json: serde_json::Value = serde_json::from_str(&report.render_json()?)
            .map_err(|err| format!("invalid JSON: {err}"))?;
        if json["status"] != "pass" || json["checks"][1]["status"] != "skipped" {
            return Err(format!("unexpected JSON projection: {json}"));
        }
        Ok(())
    }

    #[test]
    #[cfg(feature = "lang-python")]
    fn python_root_skips_rust_toolchain_checks_without_cargo_or_rustc() -> Result<(), String> {
        let root = doctor_scope_root(
            "scope-python",
            &[
                ("pyproject.toml", "[project]\nname = \"textfmt\"\n"),
                ("src/textfmt/__init__.py", "def f():\n    return 1\n"),
            ],
        )?;
        let (report, probed) = evaluate_without_rust_toolchain(&root, &[LanguageId::Python]);
        let _ = std::fs::remove_dir_all(&root);
        // Python auto-enablement keeps the default `rust` entry, so the skip
        // must come from the absent Rust markers, not a missing `rust` entry.
        if report.languages != ["rust", "python"] {
            return Err(format!("unexpected enabled set: {:?}", report.languages));
        }
        assert_rust_toolchain_skipped(&report, &probed, "in scope: python")
    }

    #[test]
    #[cfg(feature = "lang-typescript")]
    fn typescript_enabled_root_skips_rust_toolchain_checks() -> Result<(), String> {
        let root = doctor_scope_root(
            "scope-typescript",
            &[
                (
                    CONFIG_FILE_NAME,
                    "[languages]\nenabled = [\"typescript\"]\n",
                ),
                ("package.json", "{}\n"),
            ],
        )?;
        let (report, probed) = evaluate_without_rust_toolchain(&root, &[LanguageId::TypeScript]);
        let _ = std::fs::remove_dir_all(&root);
        assert_rust_toolchain_skipped(&report, &probed, "Rust is not enabled in [languages]")
    }

    /// `enabled = ["rust", "typescript"]` is the list doctor's own tip tells
    /// a TypeScript user to write; with no Rust markers the root is still a
    /// TypeScript project.
    #[test]
    #[cfg(feature = "lang-typescript")]
    fn rust_listed_beside_typescript_without_rust_markers_skips_rust_toolchain()
    -> Result<(), String> {
        let root = doctor_scope_root(
            "scope-rust-and-typescript",
            &[
                (
                    CONFIG_FILE_NAME,
                    "[languages]\nenabled = [\"rust\", \"typescript\"]\n",
                ),
                ("package.json", "{}\n"),
            ],
        )?;
        let (report, probed) = evaluate_without_rust_toolchain(&root, &[LanguageId::TypeScript]);
        let _ = std::fs::remove_dir_all(&root);
        assert_rust_toolchain_skipped(&report, &probed, "in scope: typescript")
    }

    #[test]
    fn rust_sources_without_cargo_toml_still_fail_the_cargo_toml_check() -> Result<(), String> {
        let root = doctor_scope_root(
            "scope-rust-no-manifest",
            &[("src/lib.rs", "pub fn f() {}\n")],
        )?;
        let (report, probed) = evaluate_without_rust_toolchain(&root, &[LanguageId::Rust]);
        let _ = std::fs::remove_dir_all(&root);
        let cargo_toml = check(&report, "cargo_toml")?;
        if cargo_toml.status != DoctorCheckStatus::Fail
            || !cargo_toml
                .evidence
                .as_deref()
                .unwrap_or_default()
                .starts_with("no Cargo.toml found")
        {
            return Err(format!(
                "Rust root must fail the Cargo.toml check: {cargo_toml:?}"
            ));
        }
        if probed != ["git", "cargo", "rustc"] || report.status != DoctorStatus::Fail {
            return Err(format!(
                "Rust root must probe and fail: {probed:?} {:?}",
                report.checks
            ));
        }
        Ok(())
    }

    #[test]
    fn rust_root_with_missing_cargo_discloses_verification_limitation() -> Result<(), String> {
        let root = doctor_scope_root(
            "scope-rust-no-cargo",
            &[
                ("Cargo.toml", "[package]\nname = \"probe\"\n"),
                ("src/lib.rs", "pub fn f() {}\n"),
            ],
        )?;
        let (report, _probed) = evaluate_without_rust_toolchain(&root, &[LanguageId::Rust]);
        let _ = std::fs::remove_dir_all(&root);
        if check(&report, "cargo_toml")?.status != DoctorCheckStatus::Pass {
            return Err(format!("Cargo.toml must pass: {:?}", report.checks));
        }
        for tool in ["tool_cargo", "tool_rustc"] {
            let advisory = check(&report, tool)?;
            if advisory.status != DoctorCheckStatus::Advisory {
                return Err(format!(
                    "{tool} must be advisory for installed analysis: {advisory:?}"
                ));
            }
        }
        if report.status != DoctorStatus::Pass {
            return Err("missing cargo must not fail installed analysis".to_string());
        }
        // PR #4196 review: the analyzer reads `cargo metadata` for the custom
        // harness inventory, so a missing cargo is a disclosed analysis
        // degradation, while a missing rustc is not claimed as one.
        let cargo = check(&report, "tool_cargo")?
            .evidence
            .clone()
            .unwrap_or_default();
        let rustc = check(&report, "tool_rustc")?
            .evidence
            .clone()
            .unwrap_or_default();
        if !cargo.contains("`cargo metadata`")
            || !cargo.contains("is withheld")
            || rustc.contains("cargo metadata")
            || !rustc.contains("does not run rustc")
        {
            return Err(format!(
                "cargo advisory must disclose the cargo-metadata limitation: {cargo:?} / {rustc:?}"
            ));
        }
        Ok(())
    }

    /// PR #4196 review: `cargo` resolves through rustup's per-directory
    /// toolchain just like `rustc`, so the source-build profile must probe it
    /// in the selected root, not the caller's directory.
    #[cfg(unix)]
    #[test]
    fn doctor_cargo_probe_uses_selected_root() -> Result<(), String> {
        let dir = unique_test_dir("selected-root-cargo");
        let selected_root = dir.join("selected-root");
        std::fs::create_dir_all(&selected_root).map_err(|err| format!("create root: {err}"))?;
        let shim = publish_doctor_test_tool(
            &dir,
            "cargo-root-probe",
            "#!/bin/sh\ncase \"$PWD\" in\n  *selected-root) printf 'cargo 1.95.0 (target-root)\\n' ;;\n  *) exit 1 ;;\nesac\n",
        )?;
        let shim_str: &str = shim
            .to_str()
            .ok_or_else(|| "shim path is not UTF-8".to_string())?;
        let result = probe_published_tool_with_command(
            "cargo",
            || doctor_tool_command(shim_str),
            SHIM_PROBE_TEST_CEILING,
            doctor_tool_probe_dir("cargo", &selected_root),
        );
        let git_dir = doctor_tool_probe_dir("git", &selected_root);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            (result.status, result.evidence.as_str()),
            (DoctorStatus::Pass, "cargo 1.95.0 (target-root)"),
        );
        assert_eq!(git_dir, None, "git keeps the caller directory");
        Ok(())
    }

    #[test]
    fn old_workspace_compiler_is_advisory_for_analysis_and_fails_source_build() -> Result<(), String>
    {
        let root = doctor_scope_root(
            "scope-old-compiler",
            &[
                (
                    "Cargo.toml",
                    "[package]\nname = \"probe\"\nversion = \"0.1.0\"\n",
                ),
                ("src/lib.rs", "pub fn f() {}\n"),
            ],
        )?;
        git_in(&root, &["init", "."])?;
        let old = below_minimum_rustc_output()?;
        let probe = |tool: &str, _root: &Path| {
            if tool == "rustc" {
                doctor_tool_check_success(tool, old.as_bytes()).into_public()
            } else {
                (DoctorStatus::Pass, format!("{tool} available"))
            }
        };
        let analysis = evaluate_doctor_core_with_probe_for_profile(
            &root,
            &[LanguageId::Rust],
            DoctorProfile::Analysis,
            &probe,
        )
        .report;
        let build = evaluate_doctor_core_with_probe_for_profile(
            &root,
            &[LanguageId::Rust],
            DoctorProfile::SourceBuild,
            &probe,
        )
        .report;
        let _ = std::fs::remove_dir_all(&root);
        if check(&analysis, "tool_rustc")?.status != DoctorCheckStatus::Advisory
            || check(&build, "tool_rustc")?.status != DoctorCheckStatus::Fail
            || analysis.profile != DoctorProfile::Analysis
            || build.profile != DoctorProfile::SourceBuild
        {
            return Err(format!(
                "wrong capability projection: {analysis:?} {build:?}"
            ));
        }
        let analysis_json = analysis.render_json()?;
        let build_json = build.render_json()?;
        if !analysis_json.contains("\"advisory\"")
            || !build_json.contains("\"source-build\"")
            || doctor_report_result(&analysis).is_err()
            || doctor_report_result(&build).is_ok()
        {
            return Err(format!(
                "wrong rendered/exit projection: {analysis_json} {build_json}"
            ));
        }
        Ok(())
    }

    /// A mixed root still reports unavailable Rust verification tools.
    #[test]
    #[cfg(feature = "lang-python")]
    fn mixed_rust_and_python_root_reports_rust_toolchain_advisory() -> Result<(), String> {
        let root = doctor_scope_root(
            "scope-mixed",
            &[
                ("Cargo.toml", "[package]\nname = \"probe\"\n"),
                ("pyproject.toml", "[project]\nname = \"probe\"\n"),
            ],
        )?;
        let (report, _probed) =
            evaluate_without_rust_toolchain(&root, &[LanguageId::Rust, LanguageId::Python]);
        let _ = std::fs::remove_dir_all(&root);
        if check(&report, "tool_cargo")?.status != DoctorCheckStatus::Advisory
            || report.status != DoctorStatus::Pass
        {
            return Err(format!(
                "mixed root must disclose missing cargo as advisory: {:?}",
                report.checks
            ));
        }
        Ok(())
    }

    /// An empty (likely wrong) root under the Rust-only default keeps the
    /// missing-Cargo.toml failure instead of silently passing.
    #[test]
    fn empty_root_with_default_config_keeps_the_cargo_toml_failure() -> Result<(), String> {
        let root = doctor_scope_root("scope-empty", &[])?;
        let (report, _probed) = evaluate_without_rust_toolchain(&root, &[]);
        let _ = std::fs::remove_dir_all(&root);
        if check(&report, "cargo_toml")?.status != DoctorCheckStatus::Fail
            || report.status != DoctorStatus::Fail
        {
            return Err(format!(
                "empty root must fail Cargo.toml: {:?}",
                report.checks
            ));
        }
        Ok(())
    }

    #[test]
    fn unloadable_config_keeps_rust_toolchain_required() {
        let config: Result<RiprConfig, String> = Err("invalid ripr.toml".to_string());
        assert_eq!(
            rust_toolchain_scope(&config, &[LanguageId::TypeScript]),
            RustToolchainScope::Required
        );
    }

    #[test]
    fn render_text_names_checks_without_evidence_and_passes() {
        let mut report = DoctorReport::new("/workspace");
        report.add_check("config", DoctorStatus::Pass, None);
        let text = report.render_text();
        assert!(text.contains("✓ config"));
        assert!(text.contains("✓ doctor checks passed"));
    }

    #[test]
    fn doctor_timeout_evidence_preserves_fractional_durations() {
        assert_eq!(
            doctor_timeout_evidence("probe", std::time::Duration::from_millis(250)),
            "probe timed out after 250ms"
        );
        assert_eq!(
            doctor_timeout_evidence("probe", std::time::Duration::from_millis(1_500)),
            "probe timed out after 1500ms"
        );
        assert_eq!(
            doctor_timeout_evidence("probe", std::time::Duration::from_secs(5)),
            "probe timed out after 5s"
        );
    }

    #[test]
    #[cfg(feature = "lang-python")]
    fn doctor_json_carries_enabled_languages() -> Result<(), String> {
        // #2072: generated CI consumes the typed languages surface.
        // Hermetic: a temp root with a configured enabled list, so the
        // test proves config propagation, not just built-in defaults
        // (#2182 review).
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root =
            std::env::temp_dir().join(format!("ripr-doctor-lang-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"probe\"\n")
            .map_err(|err| format!("write Cargo.toml: {err}"))?;
        std::fs::write(
            root.join(crate::config::CONFIG_FILE_NAME),
            "[languages]\nenabled = [\"rust\", \"python\"]\n",
        )
        .map_err(|err| format!("write config: {err}"))?;

        let report = evaluate_doctor_core(&root, &[LanguageId::Rust]);
        let json = report.render_json()?;
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        let parsed: serde_json::Value =
            serde_json::from_str(&json).map_err(|e| format!("invalid JSON: {e}"))?;
        let languages = parsed["languages"]
            .as_array()
            .ok_or_else(|| "languages must be an array".to_string())?;
        assert!(
            languages.iter().any(|language| language == "rust"),
            "configured set must include rust: {languages:?}"
        );
        assert!(
            languages.iter().any(|language| language == "python"),
            "configured set must include python: {languages:?}"
        );
        Ok(())
    }

    #[test]
    fn render_json_produces_valid_json() -> Result<(), String> {
        let mut report = DoctorReport::new("/workspace");
        report.add_check(
            "root_directory",
            DoctorStatus::Pass,
            Some("root exists".to_string()),
        );
        report.add_section("cache", vec!["cache: target/ripr/cache".to_string()]);
        let json = report.render_json()?;
        let parsed: serde_json::Value =
            serde_json::from_str(&json).map_err(|e| format!("invalid JSON: {e}"))?;
        assert_eq!(parsed["schema_version"], "0.3");
        assert_eq!(parsed["tool"], "ripr");
        assert_eq!(parsed["status"], "pass");
        assert_eq!(parsed["checks"][0]["name"], "root_directory");
        assert_eq!(parsed["checks"][0]["status"], "pass");
        assert_eq!(parsed["sections"][0]["name"], "cache");
        Ok(())
    }

    #[test]
    fn sections_are_optional() -> Result<(), String> {
        let report = DoctorReport::new("/workspace");
        let json = report.render_json()?;
        let parsed: serde_json::Value =
            serde_json::from_str(&json).map_err(|e| format!("invalid JSON: {e}"))?;
        assert!(parsed["sections"].is_array());
        assert_eq!(parsed["sections"].as_array().map(Vec::len), Some(0));
        Ok(())
    }

    /// RIPR-SPEC-0007 (P2): a malformed `ripr.toml` must never echo its
    /// source excerpt into the doctor report's evidence. The evidence should
    /// still be actionable (it must name the parse failure and its
    /// location), just not reproduce the offending source line.
    #[test]
    fn config_check_redacts_source_excerpt_from_malformed_toml() -> Result<(), String> {
        let dir = unique_test_dir("redact-config");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        std::fs::write(dir.join(CONFIG_FILE_NAME), "[invalid\n")
            .map_err(|err| format!("write invalid config: {err}"))?;

        let report = evaluate_doctor_core(&dir, &[]);
        let _ = std::fs::remove_dir_all(&dir);

        let config_check = report
            .checks
            .iter()
            .find(|check| check.name == "config")
            .ok_or_else(|| "missing config check".to_string())?;
        assert_eq!(config_check.status, DoctorCheckStatus::Fail);
        let evidence = config_check
            .evidence
            .as_deref()
            .ok_or_else(|| "expected evidence for invalid config".to_string())?;

        assert!(
            !evidence.contains("[invalid"),
            "evidence must not echo the config source line: {evidence:?}"
        );
        assert!(
            !evidence.contains('\n'),
            "evidence must be a single line (no source excerpt/caret): {evidence:?}"
        );
        assert!(
            evidence.contains("invalid ripr.toml"),
            "evidence must still name the failure: {evidence:?}"
        );
        assert!(
            evidence.to_lowercase().contains("line"),
            "evidence must still point at a parse location: {evidence:?}"
        );
        Ok(())
    }

    /// Publish a test executable only after its writable file descriptor is
    /// closed, so the pathname handed to `exec` can never be concurrently open
    /// for writing by this fixture.
    #[cfg(unix)]
    fn publish_doctor_test_tool(
        directory: &Path,
        name: &str,
        script: &str,
    ) -> Result<std::path::PathBuf, String> {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;

        let published = directory.join(name);
        let staged = directory.join(format!(".{name}.staged"));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged)
            .map_err(|err| format!("create staged tool: {err}"))?;
        file.write_all(script.as_bytes())
            .map_err(|err| format!("write staged tool: {err}"))?;
        file.set_permissions(std::fs::Permissions::from_mode(0o755))
            .map_err(|err| format!("chmod staged tool: {err}"))?;
        file.sync_all()
            .map_err(|err| format!("sync staged tool: {err}"))?;
        drop(file);
        std::fs::rename(&staged, &published)
            .map_err(|err| format!("publish staged tool: {err}"))?;
        Ok(published)
    }

    /// Probe a just-published shim, retrying only a retryable launch failure.
    ///
    /// Atomic publication (#2242/#2378) removes the writer this process holds,
    /// but it cannot remove host-level exec contention. `ETXTBSY`
    /// (`ExecutableFileBusy`) is raised when *any* process holds the file open
    /// for writing: under full-suite parallelism another thread can `fork`
    /// while some writable descriptor is open, and that descriptor keeps the
    /// file un-executable until the child reaches its own `exec`. `FD_CLOEXEC`
    /// closes the descriptor *at* exec — it does not close the fork/exec
    /// window.
    ///
    /// Every test that publishes a tool and immediately executes it is exposed
    /// to this, so the retry lives here rather than at one call site. The
    /// bound is deliberate: only `retryable_launch_failure` is retried, so a
    /// real verdict — a timeout, a non-zero exit, a tool that genuinely did not
    /// execute — is never retried into a pass.
    #[cfg(unix)]
    fn probe_published_tool(tool: &str, timeout: Duration) -> (DoctorStatus, String) {
        probe_published_tool_with_command(tool, || doctor_tool_command(tool), timeout, None)
            .into_public()
    }

    /// #3073: test-only ceiling for shim probes. The production 5s constant
    /// (`DOCTOR_TOOL_TIMEOUT`) is what `ripr doctor` uses; a loaded host
    /// running the full suite in parallel can exceed it before a shim even
    /// prints, and that harness condition must not flip a test verdict. The
    /// oracle under test (which shim branch ran) stays deterministic.
    #[cfg(unix)]
    const SHIM_PROBE_TEST_CEILING: Duration = Duration::from_mins(1);

    /// `probe_published_tool` for a caller-supplied command shape and probe
    /// root: `doctor_rustc_probe_uses_selected_root` (#3073) publishes a shim
    /// and immediately execs it through `doctor_tool_check_with_command`, so
    /// it is exposed to the same exec-contention family and must share the
    /// same bounded launch retry rather than forking a parallel loop.
    #[cfg(unix)]
    fn probe_published_tool_with_command(
        tool: &str,
        build_command: impl Fn() -> std::process::Command,
        timeout: Duration,
        root: Option<&Path>,
    ) -> DoctorToolCheckResult {
        let mut launch_attempt = 0usize;
        loop {
            launch_attempt += 1;
            let outcome = doctor_tool_check_with_command(tool, build_command(), timeout, root);
            if launch_attempt >= 3 || !outcome.retryable_launch_failure {
                break outcome;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    #[cfg(unix)]
    #[test]
    fn doctor_test_tool_is_closed_before_its_executable_path_is_published() -> Result<(), String> {
        let dir = unique_test_dir("atomic-publish");
        std::fs::create_dir(&dir).map_err(|err| format!("create dir: {err}"))?;
        let shim = publish_doctor_test_tool(&dir, "ripr-atomic-probe-tool", "#!/bin/sh\nexit 0\n")?;
        let staged = dir.join(".ripr-atomic-probe-tool.staged");
        if staged.exists() {
            return Err("staged tool remained after atomic publication".to_string());
        }
        let shim_text = shim.to_str().ok_or("shim path is not utf-8")?;
        // #2441: this test publishes and immediately execs, exactly like its
        // sibling below, so it needs the same bounded launch retry. Without it
        // the test failed intermittently on CI with `ExecutableFileBusy` while
        // proving nothing about the publication contract it exists to check.
        let (status, evidence) = probe_published_tool(shim_text, Duration::from_secs(1));
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove dir: {err}"))?;
        if status != DoctorStatus::Pass {
            return Err(format!(
                "atomically published tool did not execute: {evidence}"
            ));
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn doctor_tool_check_times_out_a_hanging_tool_100_times() -> Result<(), String> {
        // #2183 review: a shim that blocks forever must not hang doctor;
        // the probe terminates it at the deadline and names the timeout. Run
        // the materialization and probe repeatedly to catch visibility and
        // cleanup races under full-suite parallelism.
        for attempt in 0..100 {
            let dir = unique_test_dir("timeout");
            std::fs::create_dir(&dir).map_err(|err| format!("create dir: {err}"))?;
            let sleep = ["/usr/bin/sleep", "/bin/sleep"]
                .into_iter()
                .find(|candidate| std::path::Path::new(candidate).is_file())
                .ok_or("no portable Unix sleep utility found")?;
            let script = format!("#!/bin/sh\nexec {sleep} 60\n");
            let shim = publish_doctor_test_tool(&dir, "ripr-hanging-probe-tool", &script)?;

            let start = std::time::Instant::now();
            let shim_text = shim.to_str().ok_or("shim path is not utf-8")?;
            // #2242/#2378: publish the executable pathname only after the
            // writer is closed. The bounded retry for independent host-level
            // exec contention now lives in `probe_published_tool`, shared with
            // the atomic-publication test (#2441); a real timeout result is
            // still never retried.
            let (status, evidence) = probe_published_tool(shim_text, Duration::from_millis(250));
            let elapsed = start.elapsed();

            std::fs::remove_dir_all(&dir).map_err(|err| format!("remove dir: {err}"))?;
            if status != DoctorStatus::Fail {
                return Err(format!(
                    "attempt {attempt}: hanging tool unexpectedly passed"
                ));
            }
            if evidence != format!("{shim_text} timed out after 250ms") {
                return Err(format!(
                    "attempt {attempt}: expected a 250ms timeout, got: {evidence}"
                ));
            }
            if elapsed >= std::time::Duration::from_secs(30) {
                return Err(format!(
                    "attempt {attempt}: hanging tool was not terminated: {elapsed:?}"
                ));
            }
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn doctor_tool_check_names_non_missing_launch_failure() -> Result<(), String> {
        let dir = unique_test_dir("launch-failure");
        std::fs::create_dir(&dir).map_err(|err| format!("create directory tool: {err}"))?;
        let tool = dir.to_str().ok_or("directory path is not utf-8")?;
        let (status, evidence) = doctor_tool_check(tool);
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove directory tool: {err}"))?;
        if status != DoctorStatus::Fail {
            return Err("directory tool unexpectedly passed".to_string());
        }
        if !evidence.contains("could not be launched") {
            return Err(format!("launch failure was misclassified: {evidence}"));
        }
        Ok(())
    }

    #[test]
    fn doctor_spawn_failure_retry_policy_covers_transient_kinds() -> Result<(), String> {
        for (kind, expected) in [
            (std::io::ErrorKind::WouldBlock, true),
            (std::io::ErrorKind::ExecutableFileBusy, true),
            (std::io::ErrorKind::OutOfMemory, true),
            (std::io::ErrorKind::PermissionDenied, false),
            (std::io::ErrorKind::NotFound, false),
        ] {
            if doctor_spawn_failure_is_retryable(kind) != expected {
                return Err(format!("unexpected retry policy for {kind:?}"));
            }
        }
        Ok(())
    }

    #[test]
    fn doctor_spawn_failure_names_the_shared_git_path_fix() {
        let git_missing = doctor_spawn_failure("git", std::io::ErrorKind::NotFound);
        assert_eq!(git_missing.status, DoctorStatus::Fail);
        assert_eq!(
            git_missing.evidence,
            crate::git::GIT_NOT_FOUND_ON_PATH_MESSAGE
        );
        assert!(git_missing.evidence.contains("`--diff PATH` / `--diff -`"));
        assert!(
            !git_missing.evidence.contains('['),
            "git argv must not appear on the doctor ! line: {}",
            git_missing.evidence
        );

        let cargo_missing = doctor_spawn_failure("cargo", std::io::ErrorKind::NotFound);
        assert_eq!(cargo_missing.evidence, "cargo not available");
        assert!(
            !cargo_missing.evidence.contains("--diff"),
            "cargo must not inherit git's saved-diff repair"
        );

        let denied = doctor_spawn_failure("git", std::io::ErrorKind::PermissionDenied);
        assert!(
            denied.evidence.contains("could not be launched"),
            "permission denied is not a missing-PATH diagnosis: {}",
            denied.evidence
        );
        assert!(!denied.evidence.contains("--diff"));
    }

    #[test]
    fn doctor_first_command_prefers_saved_diff_when_git_cannot_run() {
        let mut probed = false;
        assert_eq!(
            DoctorFirstCommand::resolve(false, || {
                probed = true;
                false
            }),
            DoctorFirstCommand::SavedDiff
        );
        assert!(!probed, "a gitless doctor must not probe the worktree");
        assert_eq!(
            DoctorFirstCommand::resolve(true, || true),
            DoctorFirstCommand::Worktree
        );
        assert_eq!(
            DoctorFirstCommand::resolve(true, || false),
            DoctorFirstCommand::DefaultCheck
        );
        assert_eq!(
            DoctorFirstCommand::SavedDiff.command_line(),
            DoctorFirstCommand::SAVED_DIFF_LINE
        );

        let mut missing_git = DoctorReport::new(".");
        missing_git.add_check(
            "tool_git",
            DoctorStatus::Fail,
            Some(crate::git::GIT_NOT_FOUND_ON_PATH_MESSAGE.to_string()),
        );
        assert!(!git_tool_can_run(&missing_git));
        assert_eq!(
            DoctorFirstCommand::resolve(git_tool_can_run(&missing_git), || true),
            DoctorFirstCommand::SavedDiff,
            "a dirty tree cannot win over a missing git binary"
        );

        let mut git_ok = DoctorReport::new(".");
        git_ok.add_check(
            "tool_git",
            DoctorStatus::Pass,
            Some("git version 2.43.0".to_string()),
        );
        assert!(git_tool_can_run(&git_ok));
        assert!(!git_tool_can_run(&DoctorReport::new(".")));
    }
    /// absolute path must fail closed with actionable evidence, independent
    /// of what happens to be (or not be) on the host's PATH.
    #[test]
    fn doctor_tool_check_fails_closed_for_guaranteed_missing_tool() {
        let missing = std::env::temp_dir().join(format!(
            "ripr-doctor-missing-tool-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        let missing_str = missing.to_string_lossy().into_owned();
        let (status, evidence) = doctor_tool_check(&missing_str);
        assert_eq!(status, DoctorStatus::Fail);
        assert!(
            evidence.ends_with("not available"),
            "unexpected evidence for missing tool: {evidence:?}"
        );
    }

    /// A doctor probe runs with rustup auto-install off (#4734), even when
    /// the caller's environment turned it on: `cargo --version` in a
    /// checkout pinning a missing toolchain must not download it.
    #[cfg(unix)]
    #[test]
    fn doctor_tool_probe_forbids_rustup_auto_install() {
        let mut command = doctor_tool_command("sh");
        command
            .args(["-c", "echo \"auto=${RUSTUP_AUTO_INSTALL-unset}\""])
            .env("RUSTUP_AUTO_INSTALL", "1");
        let result = doctor_tool_check_with_command("cargo", command, DOCTOR_TOOL_TIMEOUT, None);
        assert_eq!(result.status, DoctorStatus::Pass);
        assert_eq!(result.evidence, "auto=0");
    }

    /// A tool that runs and exits non-zero is present, so doctor names the
    /// exit and the tool's own `error:` line, skipping a leading `warn:`,
    /// instead of "not available" (#4734).
    #[cfg(unix)]
    #[test]
    fn doctor_tool_nonzero_exit_names_the_tool_error() {
        let mut command = doctor_tool_command("sh");
        command.args([
            "-c",
            "printf '\\nwarn: both rust-toolchain and rust-toolchain.toml exist\\nerror: toolchain 1.81.0 is not installed\\nhelp: run rustup\\n' >&2; exit 1",
        ]);
        let result = doctor_tool_check_with_command("rustc", command, DOCTOR_TOOL_TIMEOUT, None);
        assert_eq!(result.status, DoctorStatus::Fail);
        assert_eq!(
            result.evidence,
            "rustc --version failed (exit status: 1): error: toolchain 1.81.0 is not installed"
        );
    }

    #[test]
    fn doctor_tool_runner_times_out_and_reaps_child() -> Result<(), String> {
        let mut command = doctor_tool_command(if cfg!(windows) { "powershell" } else { "sh" });
        #[cfg(windows)]
        command.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 5"]);
        #[cfg(not(windows))]
        command.args(["-c", "sleep 5"]);

        match run_doctor_tool(command, Duration::from_millis(20)) {
            Err(DoctorToolRunError::TimedOut) => Ok(()),
            Err(error) => Err(format!("expected timeout, got {error:?}")),
            Ok(_) => Err("timed-out tool unexpectedly completed".into()),
        }
    }

    #[test]
    fn doctor_pipe_drain_keeps_a_bounded_prefix_and_reads_to_eof() -> Result<(), String> {
        let flood = vec![b'x'; 300_000];
        let mut reader = std::io::Cursor::new(flood);
        let kept = drain_doctor_pipe(&mut reader, 1_000).map_err(|err| err.to_string())?;
        if kept.len() != 1_000 {
            return Err(format!(
                "kept {} bytes, expected the 1000-byte cap",
                kept.len()
            ));
        }
        if reader.position() != 300_000 {
            return Err(format!("pipe not drained to EOF: at {}", reader.position()));
        }
        let short = drain_doctor_pipe(std::io::Cursor::new(b"pnpm 9.1.0\n".to_vec()), 1_000)
            .map_err(|err| err.to_string())?;
        if short != b"pnpm 9.1.0\n" {
            return Err("a short version line must be kept whole".to_string());
        }
        Ok(())
    }

    #[test]
    fn doctor_timeout_with_incomplete_cleanup_names_the_leftover_processes() {
        let result = doctor_tool_run_result(
            "pnpm",
            Duration::from_secs(5),
            Err(DoctorToolRunError::CleanupFailed(
                DoctorProbeEnd::TimedOut,
                "job termination failed".to_string(),
            )),
        );
        assert_eq!(result.status, DoctorStatus::Fail);
        assert_eq!(
            result.evidence,
            "pnpm timed out after 5s; ripr could not confirm the probe's processes stopped and some may still be running: job termination failed"
        );
        let plain = doctor_tool_run_result(
            "pnpm",
            Duration::from_secs(5),
            Err(DoctorToolRunError::TimedOut),
        );
        assert_eq!(plain.evidence, "pnpm timed out after 5s");
        // A probe that exited or could not be waited on must not read as a
        // timeout when its cleanup fails.
        let exited = doctor_tool_run_result(
            "pnpm",
            Duration::from_secs(5),
            Err(DoctorToolRunError::CleanupFailed(
                DoctorProbeEnd::Exited,
                "job termination failed".to_string(),
            )),
        );
        assert_eq!(exited.status, DoctorStatus::Fail);
        assert_eq!(
            exited.evidence,
            "pnpm exited; ripr could not confirm the probe's processes stopped and some may still be running: job termination failed"
        );
        let wait_failed = doctor_tool_run_result(
            "pnpm",
            Duration::from_secs(5),
            Err(DoctorToolRunError::CleanupFailed(
                DoctorProbeEnd::WaitFailed,
                "job termination failed".to_string(),
            )),
        );
        assert_eq!(
            wait_failed.evidence,
            "pnpm could not be waited on; ripr could not confirm the probe's processes stopped and some may still be running: job termination failed"
        );
    }

    /// A doctor probe that times out takes its descendants with it. A
    /// Windows `.cmd` shim runs as `cmd.exe /c node ...`; the probe used to
    /// kill only the direct child and left the grandchild running. The
    /// descendant here holds the inherited pipes the way a shim's node does.
    #[cfg(windows)]
    #[test]
    fn doctor_tool_timeout_terminates_pipe_inheriting_descendants() -> Result<(), String> {
        let marker =
            std::env::temp_dir().join(format!("ripr-doctor-descendant-{}.pid", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let marker_text = marker.display().to_string().replace('\'', "''");
        let mut command = doctor_tool_command("powershell");
        command.args([
            "-NoProfile",
            "-Command",
            &format!(
                "$p = Start-Process -FilePath powershell -ArgumentList @('-NoProfile','-Command','Start-Sleep -Seconds 120') -NoNewWindow -PassThru; Set-Content -LiteralPath '{marker_text}' -Value $p.Id; Wait-Process -Id $p.Id"
            ),
        ]);
        // Same setup budget as the process-owner descendant test.
        let outcome = run_doctor_tool(command, Duration::from_secs(30));
        let written = std::fs::read_to_string(&marker);
        let _ = std::fs::remove_file(&marker);
        if !matches!(outcome, Err(DoctorToolRunError::TimedOut)) {
            return Err(format!("expected a timeout, got {outcome:?}"));
        }
        // A missing marker is a setup failure, not proof of containment.
        let pid: u32 = written
            .map_err(|err| format!("descendant marker was not written: {err}"))?
            .trim()
            .parse()
            .map_err(|err| format!("descendant marker is not a PID: {err}"))?;
        let mut probe = doctor_tool_command("powershell");
        probe.args([
            "-NoProfile",
            "-Command",
            &format!("if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ exit 0 }} else {{ exit 1 }}"),
        ]);
        let alive = run_doctor_tool(probe, Duration::from_secs(30))
            .map_err(|err| format!("liveness probe failed: {err:?}"))?
            .status
            .success();
        if alive {
            return Err(format!(
                "descendant {pid} outlived the doctor probe timeout"
            ));
        }
        Ok(())
    }
}
