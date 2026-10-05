//! `verdict-corpus relabel`: re-derive a sample of the corpus's runtime truth.
//!
//! Every label in the corpus rests on mutants someone ran once, by hand. This
//! command replays them: for each selected case it builds a run-owned copy of
//! the subject, confirms the unedited tests pass, applies the case edit, then
//! writes each mutant's `mutated_line` over the anchor and runs the case's own
//! test command again. It fails when an observed outcome, failing test, or
//! derived truth drifts from the label, when a mutant does not compile or
//! leaves the anchor unchanged, and when repeated runs disagree.
//!
//! Authored subjects are stored whole and replay offline. Upstream subjects
//! are excerpts, so they replay only from a full checkout at the pinned commit
//! passed with `--checkouts`; this command never clones or fetches, and runs
//! cargo offline, so fetch a checkout's dependencies (`cargo fetch`) first.
//! It does not sandbox the subject's own build scripts or tests.

use super::verdict_corpus::{
    CORPUS_DIR, Case, Corpus, EditKind, MutantOutcome, Subject, SubjectOrigin, TruthState,
    copy_tree, materialize_edit, validated_corpus,
};
use crate::normalize_path;
use crate::run::{capture_output_measured, run_output_owned};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const RELABEL_SCHEMA: &str = "ripr_verdict_corpus_relabel.v1";
const DEFAULT_OUT: &str = "target/ripr/reports/verdict-corpus";
const DEFAULT_SEED: &str = "ripr-verdict-corpus";
/// libtest options after `--` that list instead of running, or change the
/// output format so failing tests can no longer be named.
const NON_RUNNING_TEST_FLAGS: &[&str] = &["--list", "--format", "-q", "--quiet"];
const UNREPLAYABLE_CARGO_FLAGS: &[&str] =
    &["--no-run", "--manifest-path", "--target-dir", "--config"];
const DEFAULT_REPEAT: usize = 2;
const DEFAULT_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RelabelArgs {
    pub(crate) sample: Option<usize>,
    pub(crate) seed: String,
    pub(crate) cases: Vec<String>,
    pub(crate) checkouts: Option<PathBuf>,
    pub(crate) repeat: usize,
    pub(crate) timeout: Duration,
    pub(crate) out: PathBuf,
    /// Where run-owned subject trees and their build caches live.
    pub(crate) work_dir: PathBuf,
}

pub(crate) fn parse_args(args: &[String]) -> Result<RelabelArgs, String> {
    let mut parsed = RelabelArgs {
        sample: None,
        seed: DEFAULT_SEED.to_string(),
        cases: Vec::new(),
        checkouts: None,
        repeat: DEFAULT_REPEAT,
        timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        out: PathBuf::from(DEFAULT_OUT),
        work_dir: std::env::temp_dir().join("ripr-verdict-relabel"),
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("verdict-corpus relabel: {name} needs a value"))
        };
        let positive = |name: &str, text: String| -> Result<u64, String> {
            match text.parse::<u64>() {
                Ok(n) if n > 0 => Ok(n),
                _ => Err(format!(
                    "verdict-corpus relabel: {name} must be a positive integer, got `{text}`"
                )),
            }
        };
        match arg.as_str() {
            "--sample" => {
                let n = positive("--sample", value("--sample")?)?;
                parsed.sample = Some(usize::try_from(n).map_err(|err| err.to_string())?);
            }
            "--seed" => parsed.seed = value("--seed")?,
            "--case" => {
                let case = value("--case")?;
                if !parsed.cases.contains(&case) {
                    parsed.cases.push(case);
                }
            }
            "--checkouts" => parsed.checkouts = Some(PathBuf::from(value("--checkouts")?)),
            "--repeat" => {
                let n = positive("--repeat", value("--repeat")?)?;
                parsed.repeat = usize::try_from(n).map_err(|err| err.to_string())?;
            }
            "--timeout-secs" => {
                parsed.timeout =
                    Duration::from_secs(positive("--timeout-secs", value("--timeout-secs")?)?);
            }
            "--out" => parsed.out = PathBuf::from(value("--out")?),
            "--work-dir" => parsed.work_dir = PathBuf::from(value("--work-dir")?),
            other => {
                return Err(format!(
                    "verdict-corpus relabel: unknown argument `{other}`"
                ));
            }
        }
    }
    if parsed.sample.is_some() && !parsed.cases.is_empty() {
        return Err("verdict-corpus relabel: pass --sample or --case, not both".to_string());
    }
    Ok(parsed)
}

/// Deterministic sample: order cases by sha256(seed, case id) and take the
/// first `n`. The same seed always picks the same cases, and a different seed
/// (a scheduled run might pass the date) rotates through the corpus.
pub(crate) fn sample_order<'a>(cases: &[&'a Case], seed: &str) -> Vec<&'a Case> {
    let mut keyed: Vec<(String, &Case)> = cases
        .iter()
        .map(|case| {
            let mut hasher = Sha256::new();
            hasher.update(seed.as_bytes());
            hasher.update([0]);
            hasher.update(case.case_id.as_bytes());
            (format!("{:x}", hasher.finalize()), *case)
        })
        .collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    keyed.into_iter().map(|(_, case)| case).collect()
}

/// The case's test command as cargo arguments. Labels name a plain
/// `cargo test ...` line; anything else (a shell pipeline, another program)
/// is refused rather than interpreted.
pub(crate) fn test_command_args(command: &str) -> Result<Vec<String>, String> {
    let mut words = command.split_whitespace();
    if words.next() != Some("cargo") {
        return Err(format!("test command `{command}` is not a cargo command"));
    }
    let rest: Vec<String> = words.map(str::to_string).collect();
    if rest.first().map(String::as_str) != Some("test") {
        return Err(format!("test command `{command}` is not `cargo test ...`"));
    }
    if rest
        .iter()
        .any(|word| word.contains(['|', ';', '&', '>', '<', '`', '$']))
    {
        return Err(format!("test command `{command}` contains shell syntax"));
    }
    // Cargo options that move the build out of the run-owned tree or change
    // how it is driven would let a replay test something else and still pass.
    for word in rest.iter().take_while(|word| word.as_str() != "--") {
        let flag = word.split('=').next().unwrap_or(word);
        // Short flags bundle (`-vZx` is `-v -Zx`), so check every letter.
        let short_bundle = !flag.starts_with("--")
            && flag
                .strip_prefix('-')
                .is_some_and(|letters| letters.contains(['Z', 'C']));
        if UNREPLAYABLE_CARGO_FLAGS.contains(&flag) || short_bundle {
            return Err(format!(
                "test command `{command}` passes `{flag}`, which the replay does not allow"
            ));
        }
    }
    if let Some(flag) = rest
        .iter()
        .skip_while(|word| word.as_str() != "--")
        .find(|word| {
            let flag = word.split('=').next().unwrap_or(word);
            NON_RUNNING_TEST_FLAGS.contains(&flag)
        })
    {
        return Err(format!(
            "test command `{command}` passes `{flag}` to the test binary, which hides the per-test results the replay reads"
        ));
    }
    Ok(rest)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub(crate) enum RunOutcome {
    TestsPassed,
    TestsFailed {
        failing_tests: BTreeSet<String>,
    },
    /// The command failed before any test ran: a mutant that does not
    /// compile has no runtime outcome, so it can carry no label.
    BuildFailed {
        error: String,
    },
    TimedOut,
    /// The command succeeded but no test executed (`--no-run`, a filter
    /// matching nothing): a pass with no test is not runtime evidence.
    NoTestsRan,
}

impl RunOutcome {
    fn label(&self) -> &'static str {
        match self {
            Self::TestsPassed => "tests_passed",
            Self::TestsFailed { .. } => "tests_failed",
            Self::BuildFailed { .. } => "build_failed",
            Self::TimedOut => "timed_out",
            Self::NoTestsRan => "no_tests_ran",
        }
    }

    /// The label plus the failing tests, so runs that fail in different
    /// tests read as different.
    fn describe(&self) -> String {
        match self {
            Self::TestsFailed { failing_tests } if !failing_tests.is_empty() => format!(
                "tests_failed [{}]",
                failing_tests.iter().cloned().collect::<Vec<_>>().join(", ")
            ),
            other => other.label().to_string(),
        }
    }
}

/// Read one `cargo test` run. libtest prints `test <name> ... FAILED` for
/// each failing test and `test result:` once per test binary that ran; a
/// failure that no test explains means the build failed, and cargo's first
/// `error` line on stderr says why.
pub(crate) fn classify_run(
    success: bool,
    timed_out: bool,
    stdout: &str,
    stderr: &str,
) -> RunOutcome {
    if timed_out {
        return RunOutcome::TimedOut;
    }
    if success {
        return if executed_tests(stdout) == 0 {
            RunOutcome::NoTestsRan
        } else {
            RunOutcome::TestsPassed
        };
    }
    let failing_tests: BTreeSet<String> = stdout
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("test ")?;
            let name = rest.strip_suffix(" ... FAILED")?;
            // libtest marks `#[should_panic]` tests as `name - should panic`.
            let name = name.strip_suffix(" - should panic").unwrap_or(name);
            Some(name.trim().to_string())
        })
        .collect();
    // With no named failure, the run still failed in a test when a binary
    // reported a failed count, or started (`running N tests`) and never
    // printed its result, as one that aborts on a stack overflow does.
    // Otherwise every binary that ran passed, and the failure came after
    // them: rustdoc failing before any doctest ran, say. That is a build
    // failure, even though earlier binaries printed passing results.
    // Only libtest's exact `running N test(s)` header counts, so a test
    // that prints a line beginning `running ` cannot fake a started binary.
    let started = stdout
        .lines()
        .filter(|line| is_running_header(line))
        .count();
    let finished = stdout
        .lines()
        .filter(|line| line.starts_with("test result:"))
        .count();
    // cargo names a test binary that exited nonzero after its results (a
    // panic in a destructor, say) this way; a rustdoc failure reads
    // `error: doctest failed` or a compile error instead.
    let binary_failed = stderr
        .lines()
        .any(|line| line.starts_with("error: test failed, to rerun pass"));
    let failed_in_a_test = started > finished
        || failed_tests(stdout) > 0
        || binary_failed
        || stdout
            .lines()
            .any(|line| line.starts_with("test result: FAILED"));
    if failing_tests.is_empty() && !failed_in_a_test {
        let error = stderr
            .lines()
            .find(|line| line.starts_with("error"))
            .unwrap_or("no error line on stderr")
            .trim()
            .to_string();
        RunOutcome::BuildFailed { error }
    } else {
        RunOutcome::TestsFailed { failing_tests }
    }
}

/// libtest's per-binary header: `running 1 test` or `running N tests`.
fn is_running_header(line: &str) -> bool {
    line.strip_prefix("running ")
        .and_then(|rest| {
            rest.strip_suffix(" tests")
                .or_else(|| rest.strip_suffix(" test"))
        })
        .is_some_and(|count| !count.is_empty() && count.bytes().all(|b| b.is_ascii_digit()))
}

/// Tests executed across every `test result:` line (passed plus failed;
/// ignored and filtered-out tests did not run).
pub(crate) fn executed_tests(stdout: &str) -> u64 {
    result_counts(stdout, &["passed", "failed"])
}

/// Failed tests across every `test result:` line.
fn failed_tests(stdout: &str) -> u64 {
    result_counts(stdout, &["failed"])
}

fn result_counts(stdout: &str, kinds: &[&str]) -> u64 {
    stdout
        .lines()
        .filter_map(|line| line.strip_prefix("test result: "))
        .flat_map(|rest| rest.split(';'))
        .filter_map(|part| {
            let mut words = part.split_whitespace().rev();
            let kind = words.next()?;
            let count = words.next()?;
            kinds.contains(&kind).then(|| count.parse::<u64>().ok())?
        })
        .sum()
}

/// A labeled failing test names the observed one when they are equal or one
/// is the other's module-qualified form (`tests::x` and `x`, or a crate
/// path).
pub(crate) fn names_failing_test(label: &str, observed: &BTreeSet<String>) -> bool {
    observed.iter().any(|name| {
        name == label
            || name.ends_with(&format!("::{label}"))
            || label.ends_with(&format!("::{name}"))
    })
}

/// Replace the 1-based `line` of `text` with `mutated` (already trimmed),
/// keeping the original indentation. An empty `mutated` blanks the line, which
/// removes the statement without shifting any other line.
pub(crate) fn apply_mutated_line(text: &str, line: usize, mutated: &str) -> Result<String, String> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    let index = line
        .checked_sub(1)
        .filter(|index| *index < lines.len())
        .ok_or_else(|| format!("anchor line {line} is past the end of the file"))?;
    let (original, carriage) = match lines[index].strip_suffix('\r') {
        Some(body) => (body, "\r"),
        None => (lines[index], ""),
    };
    let indent = &original[..original.len() - original.trim_start().len()];
    let replaced = if mutated.is_empty() {
        carriage.to_string()
    } else {
        format!("{indent}{mutated}{carriage}")
    };
    lines[index] = &replaced;
    Ok(lines.join("\n"))
}

#[derive(Debug, Serialize)]
pub(crate) struct MutantResult {
    pub(crate) replacement: String,
    pub(crate) mutated_line: Option<String>,
    pub(crate) labeled: String,
    pub(crate) observed: Vec<RunOutcome>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CaseResult {
    pub(crate) case_id: String,
    pub(crate) subject_id: String,
    pub(crate) toolchain: String,
    pub(crate) labeled_truth: String,
    pub(crate) observed_truth: Option<String>,
    pub(crate) baseline: Vec<RunOutcome>,
    pub(crate) edited: Vec<RunOutcome>,
    pub(crate) mutants: Vec<MutantResult>,
    pub(crate) drift: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Receipt {
    schema_version: &'static str,
    corpus_version: String,
    seed: String,
    sample: Option<usize>,
    repeat: usize,
    selected: usize,
    not_replayed: Vec<String>,
    drifted_cases: usize,
    cases: Vec<CaseResult>,
}

/// Compare one mutant's repeated runs with its label and return each drift.
pub(crate) fn mutant_drift(
    case_id: &str,
    what: &str,
    labeled: MutantOutcome,
    failing_test: Option<&str>,
    observed: &[RunOutcome],
) -> Vec<String> {
    let mut drift = Vec::new();
    let Some(first) = observed.first() else {
        return vec![format!("case `{case_id}` {what}: no run was observed")];
    };
    // Runs that fail in different tests disagree as much as a pass and a fail.
    if observed.iter().any(|run| run != first) {
        let seen: Vec<String> = observed.iter().map(RunOutcome::describe).collect();
        drift.push(format!(
            "case `{case_id}` {what}: repeated runs disagree ({}); a flaky or timing-dependent test cannot carry a label",
            seen.join(", ")
        ));
    }
    match (first, labeled) {
        (RunOutcome::BuildFailed { error }, _) => drift.push(format!(
            "case `{case_id}` {what}: does not compile ({error}), so it has no runtime outcome; fix its mutated_line or drop it"
        )),
        (RunOutcome::TimedOut, _) => drift.push(format!(
            "case `{case_id}` {what}: timed out; raise --timeout-secs or label it by hand"
        )),
        (RunOutcome::NoTestsRan, _) => drift.push(format!(
            "case `{case_id}` {what}: the test command ran no test, so it has no runtime outcome; fix the case's test_command"
        )),
        (RunOutcome::TestsPassed, MutantOutcome::TestsFailed) => drift.push(format!(
            "case `{case_id}` {what}: labeled tests_failed but the tests pass"
        )),
        (RunOutcome::TestsFailed { .. }, MutantOutcome::TestsPassed) => drift.push(format!(
            "case `{case_id}` {what}: labeled tests_passed but the tests fail"
        )),
        (RunOutcome::TestsFailed { failing_tests }, MutantOutcome::TestsFailed) => {
            // An aborted test binary names no failing test, so there is no
            // name to hold the label to.
            if let Some(label) = failing_test
                && !failing_tests.is_empty()
                && !names_failing_test(label, failing_tests)
            {
                drift.push(format!(
                    "case `{case_id}` {what}: labeled failing test `{label}` did not fail; observed {}",
                    failing_tests.iter().cloned().collect::<Vec<_>>().join(", ")
                ));
            }
        }
        (RunOutcome::TestsPassed, MutantOutcome::TestsPassed) => {}
    }
    drift
}

/// Truth from observed outcomes, by the corpus's own rule: every mutant
/// failing the tests is discriminated, none is not_discriminated. `None` when
/// any mutant has no runtime outcome.
pub(crate) fn observed_truth(firsts: &[&RunOutcome]) -> Option<TruthState> {
    let mut failed = 0;
    for run in firsts {
        match run {
            RunOutcome::TestsFailed { .. } => failed += 1,
            RunOutcome::TestsPassed => {}
            RunOutcome::BuildFailed { .. } | RunOutcome::TimedOut | RunOutcome::NoTestsRan => {
                return None;
            }
        }
    }
    Some(match failed {
        0 => TruthState::NotDiscriminated,
        n if n == firsts.len() => TruthState::Discriminated,
        _ => TruthState::PartiallyDiscriminated,
    })
}

struct Runner<'a> {
    args: &'a RelabelArgs,
    work_root: PathBuf,
    /// Per-process, so two runs sharing a work dir never clear each other's
    /// trees; the cargo target dirs stay shared (cargo locks them).
    trees_root: PathBuf,
}

impl Runner<'_> {
    fn run_tests(
        &self,
        tree: &Path,
        subject_id: &str,
        toolchain: &str,
        command: &[String],
    ) -> Result<Vec<RunOutcome>, String> {
        let target = self
            .work_root
            .join("target")
            .join(format!("{subject_id}-{toolchain}"));
        let target = std::path::absolute(&target)
            .map_err(|err| format!("resolve {}: {err}", normalize_path(&target)))?;
        let target = target.to_string_lossy().into_owned();
        let envs = [
            ("CARGO_TARGET_DIR", target.as_str()),
            ("CARGO_TERM_COLOR", "never"),
            ("RUSTUP_TOOLCHAIN", toolchain),
            ("RUSTUP_AUTO_INSTALL", "0"),
            // Replays never reach the network: an upstream checkout whose
            // dependencies are not in the cargo cache fails to build rather
            // than downloading them.
            ("CARGO_NET_OFFLINE", "true"),
            // The caller's compiler settings must not reach the subject: a
            // `-D warnings` turns a deleted statement into a build failure,
            // a `--cfg` enables disabled tests, and `RUSTC` or `RUSTDOC`
            // would bypass the labeled toolchain (rustdoc builds and runs the
            // doctests). An empty wrapper disables it; an empty
            // RUSTFLAGS-family value overrides every config-file rustflags,
            // so a subject's own `.cargo/config.toml` rustflags are ignored
            // too. An empty RUSTC_BOOTSTRAP keeps the toolchain stable.
            ("RUSTC", "rustc"),
            ("RUSTDOC", "rustdoc"),
            ("CARGO_BUILD_RUSTDOC", "rustdoc"),
            ("RUSTC_WRAPPER", ""),
            ("RUSTC_WORKSPACE_WRAPPER", ""),
            ("RUSTFLAGS", ""),
            ("CARGO_ENCODED_RUSTFLAGS", ""),
            ("CARGO_BUILD_RUSTFLAGS", ""),
            ("RUSTDOCFLAGS", ""),
            ("CARGO_ENCODED_RUSTDOCFLAGS", ""),
            ("RUSTC_BOOTSTRAP", ""),
        ];
        let mut runs = Vec::with_capacity(self.args.repeat);
        for _ in 0..self.args.repeat {
            let output = capture_output_measured(
                "cargo",
                command,
                Some(tree),
                &envs,
                self.args.timeout,
                "verdict-corpus relabel test run",
            )?
            .output;
            let success = output.status.is_some_and(|status| status.success());
            runs.push(classify_run(
                success,
                output.timed_out,
                &output.stdout,
                &output.stderr,
            ));
        }
        Ok(runs)
    }
}

/// A run-owned copy of the subject with the case edit applied: the stored
/// crate for an authored subject, the pinned full checkout for an upstream
/// one. Returns the tree and the edited anchor file's text.
fn prepare_tree(
    dir: &Path,
    case: &Case,
    subject: &Subject,
    checkouts: Option<&Path>,
    trees_root: &Path,
) -> Result<PathBuf, String> {
    let tree = trees_root.join(&case.subject_id);
    match fs::remove_dir_all(&tree) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(format!("clear {}: {err}", normalize_path(&tree))),
    }
    match subject.origin {
        SubjectOrigin::Authored => {
            copy_tree(&dir.join("subjects").join(&subject.subject_id), &tree)?
        }
        SubjectOrigin::Upstream => {
            let checkout = checkout_for(subject, checkouts)?.ok_or_else(|| {
                format!(
                    "no full checkout for upstream subject `{}`",
                    subject.subject_id
                )
            })?;
            copy_checkout(&checkout, &tree, &tracked_files(&checkout)?)?;
        }
    }
    detach_from_enclosing_workspace(&tree)?;
    std::path::absolute(&tree).map_err(|err| format!("resolve {}: {err}", normalize_path(&tree)))
}

/// Give the copied subject's root manifest an empty `[workspace]` table when
/// it has none, so cargo builds it standalone wherever the run-owned tree
/// sits. Without it, a tree under this repository (where the default work dir
/// lands, since `.cargo/config.toml` points TMPDIR at `target`) is refused as
/// a package that "believes it's in a workspace". An empty table on a root
/// package changes nothing else about its build.
pub(crate) fn detach_from_enclosing_workspace(tree: &Path) -> Result<(), String> {
    let manifest = tree.join("Cargo.toml");
    let text = fs::read_to_string(&manifest)
        .map_err(|err| format!("read {}: {err}", normalize_path(&manifest)))?;
    if declares_workspace(&text) {
        return Ok(());
    }
    let separator = if text.ends_with('\n') { "" } else { "\n" };
    fs::write(&manifest, format!("{text}{separator}\n[workspace]\n"))
        .map_err(|err| format!("write {}: {err}", normalize_path(&manifest)))
}

pub(crate) fn declares_workspace(manifest: &str) -> bool {
    manifest
        .lines()
        .map(str::trim)
        .any(|line| line == "[workspace]" || line.starts_with("[workspace."))
}

/// The full checkout of an upstream subject under `checkouts`, verified to
/// sit at the pinned commit; `None` when none was supplied.
fn checkout_for(subject: &Subject, checkouts: Option<&Path>) -> Result<Option<PathBuf>, String> {
    let Some(root) = checkouts else {
        return Ok(None);
    };
    let checkout = root.join(&subject.subject_id);
    if !checkout.is_dir() {
        return Ok(None);
    }
    let pinned = subject
        .commit
        .as_deref()
        .ok_or_else(|| format!("upstream subject `{}` names no commit", subject.subject_id))?;
    let args = vec![
        "-C".to_string(),
        checkout.to_string_lossy().into_owned(),
        "rev-parse".to_string(),
        "HEAD".to_string(),
    ];
    let head = run_output_owned("git", &args)?;
    if head.trim() != pinned {
        return Err(format!(
            "{} is at {}, not the pinned commit {pinned} of `{}`",
            normalize_path(&checkout),
            head.trim(),
            subject.subject_id
        ));
    }
    // Local edits or stray untracked files would replay a different subject;
    // gitignored build output does not show without `--ignored`.
    let args = vec![
        "-C".to_string(),
        checkout.to_string_lossy().into_owned(),
        "status".to_string(),
        "--porcelain".to_string(),
        // Explicit, so a user's status.showUntrackedFiles cannot hide them.
        "--untracked-files=all".to_string(),
    ];
    let dirty = run_output_owned("git", &args)?;
    if !dirty.trim().is_empty() {
        return Err(format!(
            "{} has local changes or untracked files; reset it to the pinned commit {pinned}",
            normalize_path(&checkout)
        ));
    }
    Ok(Some(checkout))
}

/// The checkout's tracked paths, as git lists them. Only these are copied,
/// so ignored local files (a `.cargo/config.toml`, a stray lockfile) cannot
/// change the replay.
fn tracked_files(checkout: &Path) -> Result<Vec<PathBuf>, String> {
    let args = vec![
        "-C".to_string(),
        checkout.to_string_lossy().into_owned(),
        "ls-files".to_string(),
        "-z".to_string(),
    ];
    let listed = run_output_owned("git", &args)?;
    Ok(listed
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect())
}

/// Copy the listed paths of a checkout. A listed directory is a submodule,
/// whose content git does not pin here, so it is refused.
pub(crate) fn copy_checkout(from: &Path, to: &Path, files: &[PathBuf]) -> Result<(), String> {
    let root =
        fs::canonicalize(from).map_err(|err| format!("resolve {}: {err}", normalize_path(from)))?;
    // Every listed path and each of its directories exists in the copy; a
    // link may only resolve to one of them.
    let copied: BTreeSet<&Path> = files.iter().flat_map(|rel| rel.ancestors()).collect();
    for rel in files {
        let source = from.join(rel);
        let target = to.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("create {}: {err}", normalize_path(parent)))?;
        }
        let kind = fs::symlink_metadata(&source)
            .map_err(|err| format!("stat {}: {err}", normalize_path(&source)))?
            .file_type();
        if kind.is_symlink() {
            let link = fs::read_link(&source)
                .map_err(|err| format!("read link {}: {err}", normalize_path(&source)))?;
            // A link out of the checkout would let edits and mutants write
            // through the copy into files the run does not own. Resolve it
            // for real, since a chain of links can each look contained. A
            // link that does not resolve (dangling, a loop, unreadable)
            // cannot be shown to stay inside, so it is refused. So is one
            // whose target is not copied (an ignored or untracked path): in
            // the copy it would dangle, and whatever later appeared there
            // would decide where writes through it land.
            let resolved_inside = fs::canonicalize(&source).is_ok_and(|resolved| {
                resolved
                    .strip_prefix(&root)
                    .is_ok_and(|inside| copied.contains(inside))
            });
            let dir = rel.parent().unwrap_or(Path::new(""));
            if !resolved_inside || !link_stays_inside(dir, &link) {
                return Err(format!(
                    "{} links outside the checkout's tracked files or does not resolve ({}); refusing to replay it",
                    normalize_path(&source),
                    normalize_path(&link)
                ));
            }
            symlink(&link, &target)?;
        } else if kind.is_dir() {
            return Err(format!(
                "{} is a submodule; its content is not pinned by the subject commit",
                normalize_path(&source)
            ));
        } else {
            fs::copy(&source, &target)
                .map_err(|err| format!("copy {}: {err}", normalize_path(&target)))?;
        }
    }
    Ok(())
}

/// Whether a symlink at `<dir>/<name>` with target `link` resolves, lexically,
/// to a path inside the copied root.
pub(crate) fn link_stays_inside(dir: &Path, link: &Path) -> bool {
    use std::path::Component;
    let mut depth = dir.components().count();
    for component in link.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

#[cfg(unix)]
fn symlink(link: &Path, at: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(link, at)
        .map_err(|err| format!("link {}: {err}", normalize_path(at)))
}

#[cfg(not(unix))]
fn symlink(_link: &Path, at: &Path) -> Result<(), String> {
    Err(format!(
        "{} is a symlink; replay upstream checkouts with symlinks on a Unix host",
        normalize_path(at)
    ))
}

fn relabel_case(
    runner: &Runner<'_>,
    dir: &Path,
    case: &Case,
    subject: &Subject,
) -> Result<CaseResult, String> {
    let id = &case.case_id;
    let command =
        test_command_args(&case.truth.test_command).map_err(|err| format!("case `{id}`: {err}"))?;
    let tree = prepare_tree(
        dir,
        case,
        subject,
        runner.args.checkouts.as_deref(),
        &runner.trees_root,
    )?;
    let mut drift = Vec::new();
    let toolchain = labeled_toolchain(&case.truth.toolchain).ok_or_else(|| {
        format!(
            "case `{id}`: toolchain `{}` names no rustc release",
            case.truth.toolchain
        )
    })?;
    let observed = installed_release(&tree, toolchain);
    if observed.as_deref() != Some(toolchain_release(&case.truth.toolchain)) {
        drift.push(format!(
            "case `{id}`: labeled on `{}` but that toolchain is not installed here ({}); run `rustup toolchain install {toolchain}` and replay",
            case.truth.toolchain,
            observed.unwrap_or_else(|| "rustc did not start".to_string())
        ));
        return Ok(CaseResult {
            case_id: id.clone(),
            subject_id: case.subject_id.clone(),
            toolchain: toolchain.to_string(),
            labeled_truth: truth_label(case.truth.state).to_string(),
            observed_truth: None,
            baseline: Vec::new(),
            edited: Vec::new(),
            mutants: Vec::new(),
            drift,
        });
    }

    let baseline = runner.run_tests(&tree, &case.subject_id, toolchain, &command)?;
    if baseline.iter().any(|run| *run != RunOutcome::TestsPassed) {
        let seen: Vec<String> = baseline.iter().map(describe).collect();
        drift.push(format!(
            "case `{id}`: the unedited subject does not pass `{}` ({}); no mutant outcome can be trusted",
            case.truth.test_command,
            seen.join(", ")
        ));
    }

    let anchored = materialize_edit(dir, case, &tree)?;
    let edited_text = fs::read_to_string(&anchored)
        .map_err(|err| format!("read {}: {err}", normalize_path(&anchored)))?;
    let edited_line = edited_text
        .split('\n')
        .nth(case.anchor.line.saturating_sub(1))
        .map(|line| line.trim().to_string())
        .unwrap_or_default();
    let edited = runner.run_tests(&tree, &case.subject_id, toolchain, &command)?;

    let mut mutants = Vec::new();
    match case.edit_kind {
        EditKind::BehaviorChange => {
            // The edit itself is the one mutant.
            let mutant = case
                .truth
                .mutants
                .first()
                .ok_or_else(|| format!("case `{id}` has no mutant"))?;
            drift.extend(mutant_drift(
                id,
                "edit",
                mutant.outcome,
                mutant.failing_test.as_deref(),
                &edited,
            ));
            mutants.push(MutantResult {
                replacement: mutant.replacement.clone(),
                mutated_line: None,
                labeled: outcome_label(mutant.outcome).to_string(),
                observed: edited.clone(),
            });
        }
        EditKind::BehaviorPreservingRewrite => {
            if edited.iter().any(|run| *run != RunOutcome::TestsPassed) {
                let seen: Vec<String> = edited.iter().map(describe).collect();
                drift.push(format!(
                    "case `{id}`: the rewrite is labeled behavior-preserving but its tests do not pass ({})",
                    seen.join(", ")
                ));
            }
            for mutant in &case.truth.mutants {
                let what = format!("mutant `{}`", mutant.replacement);
                let Some(line) = mutant.mutated_line.as_deref() else {
                    drift.push(format!("case `{id}` {what}: has no mutated_line to replay"));
                    continue;
                };
                if line == edited_line {
                    drift.push(format!(
                        "case `{id}` {what}: mutated_line equals the edited anchor line, so it changes nothing"
                    ));
                    continue;
                }
                let mutated = apply_mutated_line(&edited_text, case.anchor.line, line)?;
                fs::write(&anchored, mutated)
                    .map_err(|err| format!("write {}: {err}", normalize_path(&anchored)))?;
                let observed = runner.run_tests(&tree, &case.subject_id, toolchain, &command);
                fs::write(&anchored, &edited_text)
                    .map_err(|err| format!("restore {}: {err}", normalize_path(&anchored)))?;
                let observed = observed?;
                drift.extend(mutant_drift(
                    id,
                    &what,
                    mutant.outcome,
                    mutant.failing_test.as_deref(),
                    &observed,
                ));
                mutants.push(MutantResult {
                    replacement: mutant.replacement.clone(),
                    mutated_line: Some(line.to_string()),
                    labeled: outcome_label(mutant.outcome).to_string(),
                    observed,
                });
            }
        }
    }

    let firsts: Vec<&RunOutcome> = mutants.iter().filter_map(|m| m.observed.first()).collect();
    let truth = if firsts.len() == case.truth.mutants.len() {
        observed_truth(&firsts)
    } else {
        None
    };
    if let Some(truth) = truth
        && truth != case.truth.state
    {
        drift.push(format!(
            "case `{id}`: labeled {} but the replayed mutants give {}",
            truth_label(case.truth.state),
            truth_label(truth)
        ));
    }
    Ok(CaseResult {
        case_id: id.clone(),
        subject_id: case.subject_id.clone(),
        toolchain: toolchain.to_string(),
        labeled_truth: truth_label(case.truth.state).to_string(),
        observed_truth: truth.map(|t| truth_label(t).to_string()),
        baseline,
        edited,
        mutants,
        drift,
    })
}

fn describe(run: &RunOutcome) -> String {
    match run {
        RunOutcome::BuildFailed { error } => format!("build_failed: {error}"),
        other => other.label().to_string(),
    }
}

fn outcome_label(outcome: MutantOutcome) -> &'static str {
    match outcome {
        MutantOutcome::TestsFailed => "tests_failed",
        MutantOutcome::TestsPassed => "tests_passed",
    }
}

fn truth_label(truth: TruthState) -> &'static str {
    match truth {
        TruthState::Discriminated => "discriminated",
        TruthState::PartiallyDiscriminated => "partially_discriminated",
        TruthState::NotDiscriminated => "not_discriminated",
    }
}

/// `rustc 1.95.0 (59807616e 2026-04-14)` from a label's toolchain or from
/// `rustc --version`; the host triple after the comma is not compared.
pub(crate) fn toolchain_release(text: &str) -> &str {
    text.split(',').next().unwrap_or(text).trim()
}

/// The rustup toolchain name for a label: `1.95.0` from
/// `rustc 1.95.0 (59807616e 2026-04-14), x86_64-unknown-linux-gnu`. Each case
/// replays on the toolchain it was labeled on, not on whatever the caller's
/// directory pins: a label that only holds on one compiler is still a label.
pub(crate) fn labeled_toolchain(text: &str) -> Option<&str> {
    let version = toolchain_release(text)
        .strip_prefix("rustc ")?
        .split(' ')
        .next()?;
    let valid = !version.is_empty()
        && version
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
    valid.then_some(version)
}

/// `rustc --version` for `toolchain` in `tree`, without letting rustup
/// install anything; `None` when it cannot start.
fn installed_release(tree: &Path, toolchain: &str) -> Option<String> {
    let output = capture_output_measured(
        "rustc",
        &["--version".to_string()],
        Some(tree),
        &[
            ("RUSTUP_TOOLCHAIN", toolchain),
            ("RUSTUP_AUTO_INSTALL", "0"),
        ],
        Duration::from_mins(1),
        "verdict-corpus relabel toolchain probe",
    )
    .ok()?
    .output;
    output
        .status
        .is_some_and(|status| status.success())
        .then(|| toolchain_release(&output.stdout).to_string())
}

pub(crate) fn relabel(args: &[String]) -> Result<(), String> {
    let args = parse_args(args)?;
    let dir = Path::new(CORPUS_DIR);
    let corpus: Corpus = validated_corpus(dir)?;
    let subject = |id: &str| corpus.subjects.iter().find(|s| s.subject_id == id);

    let mut not_replayed = Vec::new();
    let mut replayable = Vec::new();
    for case in &corpus.cases {
        // An explicit selection inspects only its own checkouts, so a stale
        // checkout of an unrelated subject cannot block it.
        if !args.cases.is_empty() && !args.cases.contains(&case.case_id) {
            continue;
        }
        let Some(owner) = subject(&case.subject_id) else {
            return Err(format!("case `{}` names an unknown subject", case.case_id));
        };
        let ready = match owner.origin {
            SubjectOrigin::Authored => true,
            SubjectOrigin::Upstream => checkout_for(owner, args.checkouts.as_deref())?.is_some(),
        };
        if ready {
            replayable.push(case);
        } else {
            not_replayed.push(case.case_id.clone());
        }
    }

    let selected: Vec<&Case> = if args.cases.is_empty() {
        let ordered = sample_order(&replayable, &args.seed);
        match args.sample {
            Some(n) => ordered.into_iter().take(n).collect(),
            None => ordered,
        }
    } else {
        let mut picked = Vec::new();
        for id in &args.cases {
            let case = corpus
                .cases
                .iter()
                .find(|case| &case.case_id == id)
                .ok_or_else(|| format!("verdict-corpus relabel: no case `{id}`"))?;
            if !replayable.iter().any(|ready| ready.case_id == case.case_id) {
                return Err(format!(
                    "verdict-corpus relabel: case `{id}` is an upstream excerpt; pass --checkouts <dir> holding a full checkout of `{}` at its pinned commit",
                    case.subject_id
                ));
            }
            picked.push(case);
        }
        picked
    };
    if selected.is_empty() {
        return Err("verdict-corpus relabel: no replayable case was selected".to_string());
    }

    let runner = Runner {
        args: &args,
        work_root: args.work_dir.clone(),
        trees_root: args
            .work_dir
            .join("trees")
            .join(std::process::id().to_string()),
    };
    let mut results = Vec::new();
    let replayed = (|| {
        for case in &selected {
            let owner = subject(&case.subject_id)
                .ok_or_else(|| format!("case `{}` names an unknown subject", case.case_id))?;
            eprintln!("verdict-corpus relabel: {}", case.case_id);
            let result = relabel_case(&runner, dir, case, owner)?;
            for line in &result.drift {
                eprintln!("  drift: {line}");
            }
            results.push(result);
        }
        Ok::<(), String>(())
    })();
    // Trees are scratch, removed on error too; a failed removal leaves only
    // disk use behind.
    if let Err(err) = fs::remove_dir_all(&runner.trees_root) {
        eprintln!(
            "verdict-corpus relabel: could not remove {}: {err}",
            normalize_path(&runner.trees_root)
        );
    }
    replayed?;

    let drifted_cases = results.iter().filter(|r| !r.drift.is_empty()).count();
    let receipt = Receipt {
        schema_version: RELABEL_SCHEMA,
        corpus_version: corpus.corpus_version.clone(),
        seed: args.seed.clone(),
        sample: args.sample,
        repeat: args.repeat,
        selected: results.len(),
        not_replayed,
        drifted_cases,
        cases: results,
    };
    fs::create_dir_all(&args.out)
        .map_err(|err| format!("create {}: {err}", normalize_path(&args.out)))?;
    let path = args.out.join("relabel.json");
    let json = serde_json::to_string_pretty(&receipt).map_err(|err| err.to_string())?;
    fs::write(&path, format!("{json}\n"))
        .map_err(|err| format!("write {}: {err}", normalize_path(&path)))?;
    println!(
        "verdict-corpus relabel: replayed {} case(s), {} drifted, {} upstream case(s) not replayed without --checkouts; wrote {}",
        receipt.selected,
        receipt.drifted_cases,
        receipt.not_replayed.len(),
        normalize_path(&path)
    );
    if drifted_cases > 0 {
        return Err(format!(
            "verdict-corpus relabel: {drifted_cases} case(s) drifted from their labels; see {}",
            normalize_path(&path)
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "verdict_corpus_relabel_tests.rs"]
mod tests;
