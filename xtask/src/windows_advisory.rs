//! Verdict for the advisory Windows lane (#2393).
//!
//! The lane runs `cargo test --workspace --no-fail-fast` twice and hands both
//! logs plus both captured exit statuses here.
//!
//! # Absence is not a pass
//!
//! The load-bearing rule is that a test missing from a log has **not** been
//! observed passing. `cargo test` can stop before later binaries, a run can die
//! mid-way, and a filter can exclude a name — so "failed in run 1, absent in
//! run 2" cannot be read as "passed in run 2, therefore flaky". Each test gets a
//! three-state observation per run ([`TestObservation`]) and the verdict is
//! derived from the pair, with an explicit `masked_unknown` outcome when one side
//! never observed the test at all.
//!
//! Verdicts are deliberately named for what two samples establish:
//! `repeated_failure` (reproduced in both samples) rather than "deterministic
//! defect", because a shared race can reproduce twice.
//!
//! # Missing evidence is a failure, not a pass
//!
//! Test failures are advisory: the lane does not gate merges on them. Failure to
//! *produce trustworthy evidence* is different, and this command exits non-zero
//! for it — a missing log, a missing exit status, an unreadable file, or a zero
//! exit status over a log that does not show a test run ([`RunState::
//! IncompleteEvidence`]). A lane that reported success while its own evidence was
//! absent would be the exact false-confidence condition it exists to prevent,
//! and a `0` in a status file is not on its own evidence that anything ran.
//!
//! # Release-seam controls must be observed
//!
//! A handful of tests are the only native Windows proof for a release seam
//! (#3922): Job Object process ownership, poisoned LSP initialize terminality,
//! the stat-only cache refusal, and the real-binary CLI, cache, PowerShell and
//! LSP journeys under spaced, non-ASCII and beyond-`MAX_PATH` roots. Their
//! names are listed in [`RELEASE_SEAM_CONTROLS`] and every run reports each
//! one's observation. A control that *fails* stays advisory like any other
//! test. A control that is *absent* from a usable run is an evidence failure:
//! the seam would otherwise read as covered by a green lane that never executed
//! its only native proof.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;

/// One named native-Windows control for a release seam (#3922).
pub(crate) struct SeamControl {
    /// The seam row this control proves.
    pub(crate) seam: &'static str,
    /// The issue that owns the seam's implementation.
    pub(crate) issue: &'static str,
    /// The test name exactly as libtest prints it.
    pub(crate) test: &'static str,
    /// Repository-relative source file that defines the test, so a rename is
    /// caught by an ordinary (non-Windows) test run rather than only by the
    /// Windows lane refusing its evidence.
    pub(crate) source: &'static str,
}

/// Tests that are the native Windows proof for a release seam. Most are
/// `#[cfg(windows)]` or `#[cfg(not(unix))]`, so no other lane executes them.
pub(crate) const RELEASE_SEAM_CONTROLS: &[SeamControl] = &[
    SeamControl {
        seam: "process",
        issue: "#3803",
        test: "process_owner::tests::owner_drop_terminates_a_still_running_child",
        source: "crates/ripr/src/process_owner.rs",
    },
    SeamControl {
        seam: "process",
        issue: "#3803",
        test: "process_owner::tests::terminate_tree_kills_pipe_inheriting_descendants",
        source: "crates/ripr/src/process_owner.rs",
    },
    SeamControl {
        seam: "process",
        issue: "#3803",
        test: "process_owner::tests::terminate_tree_leaves_unrelated_processes_alive",
        source: "crates/ripr/src/process_owner.rs",
    },
    SeamControl {
        seam: "process",
        issue: "#3803",
        test: "process_owner::tests::owner_drop_kills_descendants_after_the_primary_exits",
        source: "crates/ripr/src/process_owner.rs",
    },
    SeamControl {
        seam: "process",
        issue: "#3803",
        test: "process_owner::tests::terminate_tree_after_primary_exit_kills_descendants",
        source: "crates/ripr/src/process_owner.rs",
    },
    SeamControl {
        seam: "process",
        issue: "#3096",
        test: "run::tests::capture_output_with_timeout_terminates_pipe_inheriting_descendants",
        source: "xtask/src/run.rs",
    },
    SeamControl {
        seam: "lsp",
        issue: "#3802",
        test: "lsp::tests::initialize_surfaces_poisoned_client_features_store_as_a_session_failure",
        source: "crates/ripr/src/lsp/tests.rs",
    },
    SeamControl {
        seam: "lsp",
        issue: "#3802",
        test: "lsp::tests::poisoned_initialize_failure_commit_survives_a_wedged_client_channel",
        source: "crates/ripr/src/lsp/tests.rs",
    },
    SeamControl {
        seam: "cache",
        issue: "#3848",
        test: "analysis::seam_cache::tests::corpus_fingerprint_is_none_without_a_content_change_witness",
        source: "crates/ripr/src/analysis/seam_cache.rs",
    },
    SeamControl {
        seam: "paths",
        issue: "#3922",
        test: "spaced_root_check_and_file_fact_cache_round_trip",
        source: "crates/ripr/tests/native_path_roots.rs",
    },
    SeamControl {
        seam: "paths",
        issue: "#3922",
        test: "unicode_root_check_and_file_fact_cache_round_trip",
        source: "crates/ripr/tests/native_path_roots.rs",
    },
    SeamControl {
        seam: "paths",
        issue: "#3922",
        test: "long_root_check_round_trips_or_names_the_windows_path_limit",
        source: "crates/ripr/tests/native_path_roots.rs",
    },
    SeamControl {
        seam: "paths",
        issue: "#3922",
        test: "powershell_launch_passes_a_quoted_unicode_root_to_ripr",
        source: "crates/ripr/tests/native_path_roots.rs",
    },
    SeamControl {
        seam: "paths",
        issue: "#3922",
        test: "refresh_publishes_diagnostics_under_a_spaced_unicode_root",
        source: "crates/ripr/tests/lsp_lifecycle.rs",
    },
    SeamControl {
        seam: "paths",
        issue: "#3922",
        test: "refresh_under_a_root_beyond_max_path_publishes_or_names_the_windows_path_limit",
        source: "crates/ripr/tests/lsp_lifecycle.rs",
    },
];

/// What one run observed about one test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TestObservation {
    Failed,
    ObservedPass,
    /// The run never reported this test: masked by an earlier target, filtered,
    /// or the run ended first. Never treated as a pass.
    NotObserved,
}

/// How one run terminated, derived from its captured exit status rather than
/// inferred from log prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RunState {
    /// Exit status zero **and** the log demonstrates tests actually ran.
    CompletedClean,
    /// Non-zero exit with at least one parsed test failure. Deliberately not
    /// called "completed": tests were observed failing, but a later package
    /// could still have died in compile or harness, so normal completion of the
    /// whole workspace run is not claimed.
    NonZeroWithObservedTestFailures,
    /// Non-zero exit with no parsed test failure: compile, link, harness, or
    /// runner problem. Not a product verdict.
    CompileOrHarnessFailure,
    /// Status says success but the log does not show a test run — empty,
    /// truncated, or not a `cargo test` log. A zero status alone is not evidence
    /// that anything executed.
    IncompleteEvidence,
    LogMissing,
    StatusMissing,
}

impl RunState {
    fn label(self) -> &'static str {
        match self {
            Self::CompletedClean => "completed_clean",
            Self::NonZeroWithObservedTestFailures => "nonzero_with_observed_test_failures",
            Self::CompileOrHarnessFailure => "compile_or_harness_failure",
            Self::IncompleteEvidence => "incomplete_evidence",
            Self::LogMissing => "log_missing",
            Self::StatusMissing => "status_missing",
        }
    }

    /// Whether this run produced evidence that can be compared at all.
    ///
    /// `IncompleteEvidence` is unusable on purpose: treating a zero status over
    /// an unrecognised log as a clean run is the same false-confidence error as
    /// treating an absent test as a passing one.
    fn is_usable(self) -> bool {
        !matches!(
            self,
            Self::LogMissing | Self::StatusMissing | Self::IncompleteEvidence
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RunOutcome {
    pub(crate) state: RunState,
    pub(crate) exit_status: Option<i32>,
    pub(crate) failed: BTreeSet<String>,
    pub(crate) passed: BTreeSet<String>,
    pub(crate) targets: Vec<String>,
    pub(crate) results: Vec<String>,
    /// First explanatory line of each failed test's captured libtest block.
    pub(crate) reasons: BTreeMap<String, String>,
}

impl RunOutcome {
    fn missing(state: RunState) -> Self {
        Self {
            state,
            exit_status: None,
            failed: BTreeSet::new(),
            passed: BTreeSet::new(),
            targets: Vec::new(),
            results: Vec::new(),
            reasons: BTreeMap::new(),
        }
    }

    fn observe(&self, name: &str) -> TestObservation {
        if self.failed.contains(name) {
            TestObservation::Failed
        } else if self.passed.contains(name) {
            TestObservation::ObservedPass
        } else {
            TestObservation::NotObserved
        }
    }
}

/// What two samples establish about one test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    /// Failed in both samples. Reproduced twice — not yet demonstrated to be
    /// deterministic, since a shared race can also reproduce twice.
    RepeatedFailure,
    /// Failed in one sample and observed passing in the other.
    Unstable,
    /// Failed in one sample and never observed in the other. Cannot be
    /// classified without a run that actually reached it.
    MaskedUnknown,
}

impl Verdict {
    fn label(self) -> &'static str {
        match self {
            Self::RepeatedFailure => "repeated_failure",
            Self::Unstable => "unstable",
            Self::MaskedUnknown => "masked_unknown",
        }
    }
}

fn classify(first: TestObservation, second: TestObservation) -> Option<Verdict> {
    use TestObservation::{Failed, NotObserved, ObservedPass};
    match (first, second) {
        (Failed, Failed) => Some(Verdict::RepeatedFailure),
        (Failed, ObservedPass) | (ObservedPass, Failed) => Some(Verdict::Unstable),
        (Failed, NotObserved) | (NotObserved, Failed) => Some(Verdict::MaskedUnknown),
        // Nothing failed anywhere: not reported.
        _ => None,
    }
}

/// The four artifact roles this command consumes.
#[derive(Debug, PartialEq, Eq)]
struct Inputs {
    run1_log: String,
    run1_status: String,
    run2_log: String,
    run2_status: String,
}

/// Parse the argument list exhaustively.
///
/// Written as a single typed pass rather than repeated positional scanning so
/// that an unrecognised flag or a stray positional is refused rather than
/// ignored. Silently tolerating an extra argument is how a caller ends up
/// believing it supplied four independent artifacts when it did not.
fn parse_inputs(args: &[String]) -> Result<Inputs, String> {
    let mut run1_log: Option<String> = None;
    let mut run1_status: Option<String> = None;
    let mut run2_log: Option<String> = None;
    let mut run2_status: Option<String> = None;

    let mut index = 0usize;
    while index < args.len() {
        let flag = args[index].as_str();
        let slot = match flag {
            "--run1" => &mut run1_log,
            "--run1-status" => &mut run1_status,
            "--run2" => &mut run2_log,
            "--run2-status" => &mut run2_status,
            other => {
                return Err(format!(
                    "windows-advisory-summary got unexpected argument {other:?}; expected only --run1, --run1-status, --run2, --run2-status"
                ));
            }
        };
        if slot.is_some() {
            return Err(format!(
                "windows-advisory-summary got {flag} more than once; pass it once"
            ));
        }
        index += 1;
        let value = args
            .get(index)
            .ok_or_else(|| format!("windows-advisory-summary requires a value for {flag}"))?;
        if value.starts_with('-') {
            return Err(format!(
                "windows-advisory-summary got {flag} followed by {value}, which looks like a flag rather than a path"
            ));
        }
        *slot = Some(value.clone());
        index += 1;
    }

    let missing = [
        ("--run1", &run1_log),
        ("--run1-status", &run1_status),
        ("--run2", &run2_log),
        ("--run2-status", &run2_status),
    ]
    .into_iter()
    .filter_map(|(flag, slot)| slot.is_none().then_some(flag))
    .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "windows-advisory-summary is missing {}",
            missing
                .iter()
                .map(|flag| format!("{flag} <path>"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let inputs = Inputs {
        run1_log: run1_log.unwrap_or_default(),
        run1_status: run1_status.unwrap_or_default(),
        run2_log: run2_log.unwrap_or_default(),
        run2_status: run2_status.unwrap_or_default(),
    };

    // All four roles must be distinct artifacts. Two runs sharing a log would
    // report every failure as reproduced; two runs sharing a *status* would give
    // one run the other's exit code, so a clean run could be labelled a harness
    // failure — both are wrong verdicts rather than errors. A log reused as a
    // status file is equally incoherent.
    let paths = [
        &inputs.run1_log,
        &inputs.run1_status,
        &inputs.run2_log,
        &inputs.run2_status,
    ];
    let distinct: BTreeSet<&&String> = paths.iter().collect();
    if distinct.len() != paths.len() {
        return Err(format!(
            "windows-advisory-summary requires four distinct paths; got --run1 {}, --run1-status {}, --run2 {}, --run2-status {}",
            inputs.run1_log, inputs.run1_status, inputs.run2_log, inputs.run2_status
        ));
    }
    Ok(inputs)
}

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let Inputs {
        run1_log,
        run1_status,
        run2_log,
        run2_status,
    } = parse_inputs(args)?;

    let first = load_run(Path::new(&run1_log), Path::new(&run1_status));
    let second = load_run(Path::new(&run2_log), Path::new(&run2_status));
    print!("{}", render(&first, &second));

    // Advisory applies to test outcomes, not to evidence. A run whose log or
    // status is missing means this lane cannot be trusted, so fail loudly.
    let mut unusable = Vec::new();
    for (label, outcome) in [("run 1", &first), ("run 2", &second)] {
        if !outcome.state.is_usable() {
            unusable.push(format!("{label} is {}", outcome.state.label()));
        }
    }
    unusable.extend(unobserved_controls(&first, &second, RELEASE_SEAM_CONTROLS));
    if unusable.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "windows-advisory-summary could not produce trustworthy evidence: {}",
            unusable.join("; ")
        ))
    }
}

const NESTED_ALIAS_TEST: &str = "analysis::test_grip_evidence::tests::nested_test_module_alias_ancestry_resolves_through_production_path";

#[derive(Debug, PartialEq, Eq)]
enum IsolatedObservation {
    Pass { status: i32, result: String },
    TestFailure { status: i32, result: String },
    EvidenceFailure(String),
}

fn exact_isolated_result(result: &str, passed: bool) -> bool {
    let prefix = if passed {
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
    } else {
        "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; "
    };
    let Some(rest) = result.strip_prefix(prefix) else {
        return false;
    };
    let Some((filtered, duration)) = rest.split_once(" filtered out; finished in ") else {
        return false;
    };
    if filtered.is_empty() || !filtered.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let Some(seconds) = duration.strip_suffix('s') else {
        return false;
    };
    filtered.parse::<u64>().is_ok()
        && seconds
            .parse::<f64>()
            .is_ok_and(|value| value.is_finite() && value >= 0.0)
}

fn classify_isolated(log: Option<&str>, raw_status: Option<&str>) -> IsolatedObservation {
    let Some(log) = log else {
        return IsolatedObservation::EvidenceFailure("log missing or unreadable".to_string());
    };
    let Some(raw_status) = raw_status else {
        return IsolatedObservation::EvidenceFailure("status missing or unreadable".to_string());
    };
    let Ok(status) = raw_status.trim().parse::<i32>() else {
        return IsolatedObservation::EvidenceFailure(format!(
            "invalid cargo exit status {:?}",
            raw_status.trim()
        ));
    };
    let rows = log
        .lines()
        .filter_map(|line| test_result_line(strip_ansi(line).trim()))
        .collect::<Vec<_>>();
    let outcome = parse_log(log);
    if rows.len() != 1
        || rows
            .first()
            .is_none_or(|(name, _)| name.as_str() != NESTED_ALIAS_TEST)
    {
        return IsolatedObservation::EvidenceFailure(format!(
            "expected exactly one named test row, found {} total test row(s)",
            rows.len()
        ));
    }
    if outcome.targets.len() != 1 || outcome.results.len() != 1 {
        return IsolatedObservation::EvidenceFailure(format!(
            "expected one test target and one result total, found {} target(s) and {} result(s)",
            outcome.targets.len(),
            outcome.results.len()
        ));
    }
    let Some((_, row_failed)) = rows.first() else {
        return IsolatedObservation::EvidenceFailure("named test row missing".to_string());
    };
    let Some(result) = outcome.results.first().cloned() else {
        return IsolatedObservation::EvidenceFailure("result total missing".to_string());
    };
    if status == 0 && !*row_failed && exact_isolated_result(&result, true) {
        IsolatedObservation::Pass { status, result }
    } else if status != 0 && *row_failed && exact_isolated_result(&result, false) {
        IsolatedObservation::TestFailure { status, result }
    } else {
        IsolatedObservation::EvidenceFailure(format!(
            "cargo exit {status}, named row failed={}, result {result:?} disagree",
            row_failed
        ))
    }
}

/// Summarize three explicitly requested native Windows named-test repetitions.
/// Unlike the broad advisory verdict, any failed named repetition fails this
/// opt-in proof. Every run is reported before the command returns its status.
pub(crate) fn run_isolated(args: &[String]) -> Result<(), String> {
    let [flag, directory] = args else {
        return Err("windows-advisory-isolated-summary requires --dir <path>".to_string());
    };
    if flag != "--dir" || directory.is_empty() || directory.starts_with('-') {
        return Err("windows-advisory-isolated-summary requires --dir <path>".to_string());
    }
    let directory = Path::new(directory);
    println!("### Nested-alias isolated native Windows repeats (#4377)\n");
    let mut failed = false;
    for run in 1..=3 {
        let log_path = directory.join(format!("nested-alias-isolated-{run}.log"));
        let status_path = directory.join(format!("nested-alias-isolated-{run}.status"));
        let log = std::fs::read_to_string(&log_path);
        let status = std::fs::read_to_string(&status_path);
        let verdict = match (&log, &status) {
            (Ok(log), Ok(status)) => classify_isolated(Some(log), Some(status)),
            (Err(error), _) => IsolatedObservation::EvidenceFailure(format!(
                "read {}: {error}",
                log_path.display()
            )),
            (_, Err(error)) => IsolatedObservation::EvidenceFailure(format!(
                "read {}: {error}",
                status_path.display()
            )),
        };
        match verdict {
            IsolatedObservation::Pass { status, result } => {
                println!("- run {run}: PASS (cargo exit {status}; {result})");
            }
            IsolatedObservation::TestFailure { status, result } => {
                failed = true;
                println!("- run {run}: TEST_FAIL (cargo exit {status}; {result})");
            }
            IsolatedObservation::EvidenceFailure(reason) => {
                failed = true;
                println!("- run {run}: EVIDENCE_FAILURE ({reason})");
            }
        }
    }
    std::io::stdout()
        .flush()
        .map_err(|error| format!("flush isolated Windows verdict: {error}"))?;
    if failed {
        Err(
            "nested-alias isolated repetitions did not all pass; see the verdict and raw logs"
                .to_string(),
        )
    } else {
        Ok(())
    }
}

/// Every usable run must have observed every release-seam control, passing or
/// failing. An unusable run is already refused on its own, so it is not
/// reported a second time per control.
fn unobserved_controls(
    first: &RunOutcome,
    second: &RunOutcome,
    controls: &[SeamControl],
) -> Vec<String> {
    let mut missing = Vec::new();
    for (label, outcome) in [("run 1", first), ("run 2", second)] {
        if !outcome.state.is_usable() {
            continue;
        }
        for control in controls {
            if outcome.observe(control.test) == TestObservation::NotObserved {
                missing.push(format!(
                    "{label} did not observe {} control `{}` ({}, defined in {})",
                    control.seam, control.test, control.issue, control.source
                ));
            }
        }
    }
    missing
}

fn load_run(log: &Path, status: &Path) -> RunOutcome {
    let text = match std::fs::read_to_string(log) {
        Ok(text) => text,
        Err(error) => {
            eprintln!(
                "windows-advisory-summary: read {} failed: {error}",
                log.display()
            );
            return RunOutcome::missing(RunState::LogMissing);
        }
    };
    let exit_status = match std::fs::read_to_string(status) {
        Ok(raw) => match raw.trim().parse::<i32>() {
            Ok(code) => Some(code),
            Err(error) => {
                eprintln!(
                    "windows-advisory-summary: {} is not an integer exit status: {error}",
                    status.display()
                );
                None
            }
        },
        Err(error) => {
            eprintln!(
                "windows-advisory-summary: read {} failed: {error}",
                status.display()
            );
            None
        }
    };
    let Some(exit_status) = exit_status else {
        let mut outcome = parse_log(&text);
        outcome.state = RunState::StatusMissing;
        return outcome;
    };
    let mut outcome = parse_log(&text);
    outcome.exit_status = Some(exit_status);
    // A zero status is not, by itself, evidence that tests ran. Require the log
    // to show at least one test target and at least one `test result:` summary
    // before calling a run clean; otherwise an empty, truncated, or non-test log
    // beside a `0` status would be reported as a clean workspace run.
    let demonstrates_a_test_run = !outcome.targets.is_empty() && !outcome.results.is_empty();
    outcome.state = if exit_status == 0 {
        if demonstrates_a_test_run {
            RunState::CompletedClean
        } else {
            RunState::IncompleteEvidence
        }
    } else if outcome.failed.is_empty() {
        RunState::CompileOrHarnessFailure
    } else {
        RunState::NonZeroWithObservedTestFailures
    };
    outcome
}

/// Remove ANSI SGR escape sequences from one line.
///
/// CI sets `CARGO_TERM_COLOR: always`, so cargo's own progress lines arrive as
/// `\x1b[1m\x1b[92m     Running\x1b[0m unittests src\lib.rs (...)`. Matching a
/// prefix like `Running ` against that fails, which is how the first real lane
/// run reported no targets while parsing every failure correctly.
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }
        // Peek rather than consume: an ESC that does not introduce a CSI
        // sequence must not swallow the character after it. Consuming
        // unconditionally silently deleted real text — an ESC followed by `X`
        // lost both.
        if chars.peek() != Some(&'[') {
            continue;
        }
        let _ = chars.next();
        for next in chars.by_ref() {
            if next.is_ascii_alphabetic() {
                break;
            }
        }
    }
    out
}

pub(crate) fn parse_log(text: &str) -> RunOutcome {
    let mut outcome = RunOutcome::missing(RunState::StatusMissing);
    let mut block: Option<FailureBlock> = None;
    for raw_line in text.lines() {
        let line = strip_ansi(raw_line);
        let trimmed = line.trim();
        if let Some(name) = failure_block_header(trimmed) {
            close_failure_block(&mut outcome, block.take());
            block = Some(FailureBlock::new(name));
            continue;
        }
        // A block ends at the next libtest section or cargo target line, so an
        // aborted harness cannot swallow the targets that follow it.
        if trimmed == "failures:"
            || trimmed.starts_with("test result:")
            || running_target(trimmed).is_some()
        {
            close_failure_block(&mut outcome, block.take());
        }
        if let Some(open) = block.as_mut() {
            open.push(trimmed);
            continue;
        }
        if let Some((name, failed)) = test_result_line(trimmed) {
            if failed {
                outcome.failed.insert(name);
            } else {
                outcome.passed.insert(name);
            }
        }
        if let Some(target) = running_target(trimmed) {
            outcome.targets.push(target);
        }
        if trimmed.starts_with("test result:") {
            outcome.results.push(trimmed.to_string());
        }
    }
    close_failure_block(&mut outcome, block.take());
    outcome
}

/// Longest failure reason carried into the verdict. The reason exists so a
/// truncated job log still says *why* a test failed; a full panic payload
/// (some tests print whole JSON documents) would bury the verdict instead.
const MAX_REASON_CHARS: usize = 240;

/// One libtest `---- name stdout ----` block from the `failures:` section.
struct FailureBlock {
    name: String,
    error: Option<String>,
    panic: Option<String>,
    awaiting_panic_message: bool,
    first_line: Option<String>,
}

impl FailureBlock {
    fn new(name: String) -> Self {
        Self {
            name,
            error: None,
            panic: None,
            awaiting_panic_message: false,
            first_line: None,
        }
    }

    /// Keep the most explanatory line: a returned `Error:` first, then a
    /// panic location with its message, then whatever the test printed first.
    fn push(&mut self, line: &str) {
        if line.is_empty() {
            return;
        }
        if self.awaiting_panic_message {
            self.awaiting_panic_message = false;
            if let Some(location) = self.panic.as_mut() {
                location.push(' ');
                location.push_str(line);
            }
            return;
        }
        if self.error.is_none() && line.starts_with("Error: ") {
            self.error = Some(line.to_string());
        } else if self.panic.is_none()
            && let Some((_, location)) = line.split_once(" panicked at ")
        {
            self.panic = Some(format!("panicked at {location}"));
            self.awaiting_panic_message = true;
        } else if self.first_line.is_none() {
            self.first_line = Some(line.to_string());
        }
    }

    fn reason(self) -> Option<String> {
        let reason = self.error.or(self.panic).or(self.first_line)?;
        if reason.chars().count() <= MAX_REASON_CHARS {
            return Some(reason);
        }
        let mut cut: String = reason.chars().take(MAX_REASON_CHARS).collect();
        cut.push('…');
        Some(cut)
    }
}

/// `---- some::test stdout ----` -> the test name.
fn failure_block_header(line: &str) -> Option<String> {
    let name = line
        .strip_prefix("---- ")?
        .strip_suffix(" stdout ----")?
        .trim();
    (!name.is_empty()).then(|| name.to_string())
}

fn close_failure_block(outcome: &mut RunOutcome, block: Option<FailureBlock>) {
    let Some(block) = block else {
        return;
    };
    let name = block.name.clone();
    if let Some(reason) = block.reason() {
        outcome.reasons.entry(name).or_insert(reason);
    }
}

fn observation_label(outcome: &RunOutcome, name: &str) -> &'static str {
    if !outcome.state.is_usable() {
        return "no_evidence";
    }
    match outcome.observe(name) {
        TestObservation::Failed => "FAILED",
        TestObservation::ObservedPass => "pass",
        TestObservation::NotObserved => "not_observed",
    }
}

/// `test some::path ... FAILED` / `... ok` -> (name, failed).
///
/// Both outcomes are collected: knowing a test was observed *passing* is what
/// separates a flake from a test that was never reached.
///
/// Names containing spaces are accepted. A doctest is reported as
/// `test src/lib.rs - foo::bar (line 12) ... ok`, so rejecting spaces would have
/// silently dropped every doctest from the observation model — a doctest could
/// fail in one run and simply be absent from the verdict. The surrounding
/// `test ` / ` ... ok` shape is specific enough on its own.
fn test_result_line(line: &str) -> Option<(String, bool)> {
    let rest = line.strip_prefix("test ")?;
    let (name, failed) = if let Some(name) = rest.strip_suffix(" ... FAILED") {
        (name, true)
    } else if let Some(name) = rest.strip_suffix(" ... ok") {
        (name, false)
    } else {
        return None;
    };
    let name = name.trim();
    (!name.is_empty()).then(|| (name.to_string(), failed))
}

/// `Running unittests src\lib.rs (target\debug\deps\ripr-abc.exe)` -> the
/// source path, which identifies the target more stably than the hashed binary.
///
/// The ` (` requirement matters: other lines can begin with `Running ` (build
/// scripts, custom commands), and only a cargo test-target line carries the
/// binary in parentheses.
fn running_target(line: &str) -> Option<String> {
    let rest = line
        .strip_prefix("Running unittests ")
        .or_else(|| line.strip_prefix("Running "))?;
    let (path, _binary) = rest.split_once(" (")?;
    let path = path.trim();
    (!path.is_empty()).then(|| path.replace('\\', "/"))
}

/// Why each reported test failed, per run, so the verdict stays readable
/// when the job log is truncated or its artifacts are unreachable. A failed
/// test with no captured block is said to have none, never left blank.
fn render_failure_reasons(
    out: &mut String,
    first: &RunOutcome,
    second: &RunOutcome,
    verdicts: &BTreeMap<&'static str, Vec<String>>,
) {
    let reported: BTreeSet<&String> = verdicts.values().flatten().collect();
    if reported.is_empty() {
        return;
    }
    out.push_str("### Failure reasons\n\n");
    for name in reported {
        out.push_str(&format!("- `{name}`\n"));
        for (label, outcome) in [("Run 1", first), ("Run 2", second)] {
            if !outcome.failed.contains(name) {
                continue;
            }
            let reason = outcome
                .reasons
                .get(name)
                .map_or("no failure block captured", String::as_str);
            out.push_str(&format!("  - {label}: {}\n", reason.replace('`', "'")));
        }
    }
    out.push('\n');
}

fn render(first: &RunOutcome, second: &RunOutcome) -> String {
    let mut out = String::from("### Run states\n\n");
    for (label, outcome) in [("Run 1", first), ("Run 2", second)] {
        let status = match outcome.exit_status {
            Some(code) => format!("cargo exit {code}"),
            None => "cargo exit unknown".to_string(),
        };
        out.push_str(&format!(
            "- {label}: `{}` ({status})\n",
            outcome.state.label()
        ));
    }
    out.push('\n');

    if !first.state.is_usable() || !second.state.is_usable() {
        out.push_str("**Evidence failure.** At least one run did not produce a usable log and exit status, so no verdict can be derived. This is reported as a workflow failure, not as a pass — a lane that goes green without evidence is worse than no lane. A zero exit status over a log that does not show a test run counts as `incomplete_evidence`, not as clean.\n\n");
    }
    if first.state == RunState::CompileOrHarnessFailure
        || second.state == RunState::CompileOrHarnessFailure
    {
        out.push_str("**Infrastructure failure.** A run exited non-zero with no parsed test failure, which indicates a compile, link, harness, or runner problem rather than a product regression.\n\n");
    }

    out.push_str("### Verdicts\n\n");
    let mut verdicts: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    let candidates: BTreeSet<&String> = first.failed.iter().chain(second.failed.iter()).collect();
    for name in candidates {
        if let Some(verdict) = classify(first.observe(name), second.observe(name)) {
            verdicts
                .entry(verdict.label())
                .or_default()
                .push(name.clone());
        }
    }

    if verdicts.is_empty() {
        if first.state.is_usable() && second.state.is_usable() {
            out.push_str("No test failed in either run.\n\n");
        } else {
            out.push_str("No verdict: see the evidence failure above.\n\n");
        }
    }

    for (label, explanation) in [
        (
            "repeated_failure",
            "Failed in both runs. Reproduced twice; a shared race can also reproduce twice, so this is not yet demonstrated to be deterministic.",
        ),
        (
            "unstable",
            "Failed in one run and observed passing in the other. Confirm in isolation before filing as a defect.",
        ),
        (
            "masked_unknown",
            "Failed in one run and never observed in the other, so it cannot be classified. A test absent from a log was not observed passing.",
        ),
    ] {
        let Some(names) = verdicts.get(label) else {
            continue;
        };
        out.push_str(&format!(
            "**{label} ({})** — {explanation}\n\n",
            names.len()
        ));
        for name in names {
            out.push_str(&format!("- `{name}`\n"));
        }
        out.push('\n');
    }

    render_failure_reasons(&mut out, first, second, &verdicts);

    out.push_str("### Release-seam controls (#3922)\n\n");
    out.push_str("Native Windows proof for release seams. A failure here is advisory like any test; an unobserved control fails this lane.\n\n");
    out.push_str("| Seam | Issue | Control | Run 1 | Run 2 |\n|---|---|---|---|---|\n");
    for control in RELEASE_SEAM_CONTROLS {
        out.push_str(&format!(
            "| {} | {} | `{}` | {} | {} |\n",
            control.seam,
            control.issue,
            control.test,
            observation_label(first, control.test),
            observation_label(second, control.test)
        ));
    }
    out.push('\n');

    out.push_str("### Targets reached\n\n");
    out.push_str("Recorded because a compile or harness failure can still stop a run before later targets. With `--no-fail-fast` an ordinary test failure no longer hides them.\n\n");
    for (label, outcome) in [("Run 1", first), ("Run 2", second)] {
        let targets = if outcome.targets.is_empty() {
            "none reported".to_string()
        } else {
            outcome.targets.join(", ")
        };
        out.push_str(&format!("- {label}: {targets}\n"));
    }
    out.push('\n');

    out.push_str("### Totals\n\n");
    for (label, outcome) in [("Run 1", first), ("Run 2", second)] {
        if outcome.results.is_empty() {
            out.push_str(&format!("- {label}: no `test result:` line\n"));
            continue;
        }
        out.push_str(&format!(
            "- {label}: observed {} pass, {} fail across {} target result line(s)\n",
            outcome.passed.len(),
            outcome.failed.len(),
            outcome.results.len()
        ));
    }
    out.push_str("\nTest outcomes here are advisory and do not gate merges (#2393). Missing evidence is not advisory and fails this lane.\n");
    out
}

#[cfg(test)]
mod tests {
    fn packaged_temp_wiring_is_complete(workflow: &str) -> bool {
        let lines = workflow.lines().map(str::trim).collect::<Vec<_>>();
        [
            "$env:CARGO_TEMP_CONFIG = Join-Path $temp 'cargo-package-config.toml'",
            "$env:CARGO_TARGET_DIR = Join-Path $temp 'cargo-target'",
            "$env:TEMP = $env:CARGO_TEMP_DIR",
            "$env:TMP = $env:CARGO_TEMP_DIR",
            "$env:TMPDIR = $env:CARGO_TEMP_DIR",
            "\"CARGO_TEMP_CONFIG=$env:CARGO_TEMP_CONFIG\" >> $env:GITHUB_ENV",
            "\"CARGO_TARGET_DIR=$env:CARGO_TARGET_DIR\" >> $env:GITHUB_ENV",
            "\"TEMP=$env:TEMP\" >> $env:GITHUB_ENV",
            "\"TMP=$env:TMP\" >> $env:GITHUB_ENV",
            "\"TMPDIR=$env:TMPDIR\" >> $env:GITHUB_ENV",
        ]
        .iter()
        .all(|required| lines.iter().any(|line| line.contains(required)))
    }

    use super::*;

    fn isolated_log(row: &str, result: &str) -> String {
        format!(
            "Running unittests src/lib.rs (target/debug/deps/ripr-test.exe)\ntest {NESTED_ALIAS_TEST} ... {row}\n{result}\n"
        )
    }

    fn expect_isolated_evidence_failure(
        observed: IsolatedObservation,
        label: &str,
    ) -> Result<(), String> {
        if matches!(&observed, IsolatedObservation::EvidenceFailure(_)) {
            Ok(())
        } else {
            Err(format!(
                "{label}: expected evidence failure, got {observed:?}"
            ))
        }
    }

    #[test]
    fn isolated_summary_accepts_one_exact_named_pass_and_reports_named_failure()
    -> Result<(), String> {
        let pass = isolated_log(
            "ok",
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6032 filtered out; finished in 0.01s",
        );
        let expected_pass = IsolatedObservation::Pass {
            status: 0,
            result: "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6032 filtered out; finished in 0.01s"
                .to_string(),
        };
        let actual_pass = classify_isolated(Some(&pass), Some("0\n"));
        if actual_pass != expected_pass {
            return Err(format!(
                "exact named pass: expected {expected_pass:?}, got {actual_pass:?}"
            ));
        }
        let fail = isolated_log(
            "FAILED",
            "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 6032 filtered out; finished in 0.01s",
        );
        let actual_fail = classify_isolated(Some(&fail), Some("101\n"));
        if !matches!(
            &actual_fail,
            IsolatedObservation::TestFailure { status: 101, .. }
        ) {
            return Err(format!(
                "named test failure should be distinct: {actual_fail:?}"
            ));
        }
        expect_isolated_evidence_failure(
            classify_isolated(Some(&fail), Some("0")),
            "failed row with zero exit",
        )?;
        Ok(())
    }

    #[test]
    fn isolated_summary_refuses_missing_or_fake_observations() -> Result<(), String> {
        let pass = isolated_log(
            "ok",
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6032 filtered out; finished in 0.01s",
        );
        for (label, log, status) in [
            ("missing log", None, Some("0")),
            ("missing status", Some(pass.as_str()), None),
            ("invalid status", Some(pass.as_str()), Some("not-an-exit")),
            (
                "pass row with nonzero exit",
                Some(pass.as_str()),
                Some("101"),
            ),
        ] {
            expect_isolated_evidence_failure(classify_isolated(log, status), label)?;
        }
        let wrong_total = isolated_log(
            "ok",
            "test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 6032 filtered out; finished in 0.01s",
        );
        let duplicate_result = format!(
            "{pass}test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6032 filtered out; finished in 0.01s\n"
        );
        let duplicate_row = pass.replace(
            "test result:",
            &format!("test {NESTED_ALIAS_TEST} ... ok\ntest result:"),
        );
        let extra_test = pass.replace("test result:", "test unrelated::test ... ok\ntest result:");
        let no_target = pass.replacen(
            "Running unittests src/lib.rs (target/debug/deps/ripr-test.exe)\n",
            "",
            1,
        );
        let truncated_total = isolated_log("ok", "test result: ok. 1 passed; 0 failed;");
        let missing_counts = isolated_log(
            "ok",
            "test result: ok. 1 passed; 0 failed; 6032 filtered out; finished in 0.01s",
        );
        let nonnumeric_filtered = isolated_log(
            "ok",
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; many filtered out; finished in 0.01s",
        );
        let missing_duration = isolated_log(
            "ok",
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6032 filtered out",
        );
        let nonnumeric_duration = isolated_log(
            "ok",
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6032 filtered out; finished in later",
        );
        for (label, log) in [
            ("wrong total", wrong_total),
            ("duplicate result", duplicate_result),
            ("duplicate named row", duplicate_row),
            ("extra test", extra_test),
            ("missing target", no_target),
            ("truncated total", truncated_total),
            ("missing counts", missing_counts),
            ("nonnumeric filtered", nonnumeric_filtered),
            ("missing duration", missing_duration),
            ("nonnumeric duration", nonnumeric_duration),
        ] {
            expect_isolated_evidence_failure(classify_isolated(Some(&log), Some("0")), label)?;
        }
        Ok(())
    }

    /// Real bytes from a Windows lane run (#2393): cargo's `Running` lines are
    /// ANSI-coloured because CI sets `CARGO_TERM_COLOR: always`, libtest's are
    /// not. Copied verbatim, because synthetic uncoloured fixtures hid a parser
    /// gap that only real runner output exposed.
    const REAL_RUNNER_LOG: &str = concat!(
        "\u{1b}[1m\u{1b}[92m     Running\u{1b}[0m unittests src\\lib.rs (target\\debug\\deps\\ripr-b675962642118180.exe)\n",
        "test some::alpha ... ok\n",
        "test result: ok. 3777 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.15s\n",
        "\u{1b}[1m\u{1b}[92m     Running\u{1b}[0m tests\\lsp_lifecycle.rs (target\\debug\\deps\\lsp_lifecycle-d7f865dad16cc0c7.exe)\n",
        "test compat_journey_collect_workspace_status_over_real_wire ... FAILED\n",
        "test result: FAILED. 25 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.30s\n",
    );

    fn outcome(state: RunState, failed: &[&str], passed: &[&str]) -> RunOutcome {
        RunOutcome {
            state,
            exit_status: Some(if failed.is_empty() { 0 } else { 101 }),
            failed: failed.iter().map(|name| (*name).to_string()).collect(),
            passed: passed.iter().map(|name| (*name).to_string()).collect(),
            targets: vec!["src/lib.rs".to_string()],
            results: vec!["test result: FAILED. 1 passed; 1 failed".to_string()],
            reasons: BTreeMap::new(),
        }
    }

    /// Real failure-section shapes from a Windows lane run: a returned
    /// `Error:`, a panic with its message on the next line, and a block whose
    /// only content is printed output.
    const FAILURE_SECTION_LOG: &str = concat!(
        "\u{1b}[1m\u{1b}[92m     Running\u{1b}[0m unittests src\\lib.rs (target\\debug\\deps\\ripr-1.exe)\n",
        "test a::returns_error ... FAILED\n",
        "test b::panics ... FAILED\n",
        "test c::prints_only ... FAILED\n",
        "test d::passes ... ok\n",
        "\n",
        "failures:\n",
        "\n",
        "---- a::returns_error stdout ----\n",
        "some progress output\n",
        "Error: \"descendant PID marker: marker not written\"\n",
        "---- b::panics stdout ----\n",
        "\n",
        "thread 'b::panics' (5684) panicked at crates\\ripr\\src\\b.rs:263:5:\n",
        "expected command to succeed\n",
        "stdout:\n",
        "{\n",
        "---- c::prints_only stdout ----\n",
        "only this line\n",
        "\n",
        "failures:\n",
        "    a::returns_error\n",
        "\n",
        "test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.00s\n",
    );

    #[test]
    fn failure_blocks_yield_the_most_explanatory_reason() {
        let outcome = parse_log(FAILURE_SECTION_LOG);
        assert_eq!(
            outcome.reasons.get("a::returns_error").map(String::as_str),
            Some("Error: \"descendant PID marker: marker not written\""),
        );
        assert_eq!(
            outcome.reasons.get("b::panics").map(String::as_str),
            Some("panicked at crates\\ripr\\src\\b.rs:263:5: expected command to succeed"),
        );
        assert_eq!(
            outcome.reasons.get("c::prints_only").map(String::as_str),
            Some("only this line"),
        );
        assert!(!outcome.reasons.contains_key("d::passes"));
        assert_eq!(outcome.failed.len(), 3);
        assert_eq!(outcome.results.len(), 1);
    }

    #[test]
    fn an_unterminated_failure_block_does_not_swallow_later_targets() {
        let log = concat!(
            "     Running unittests src\\lib.rs (target\\debug\\deps\\ripr-1.exe)\n",
            "test a::aborts ... FAILED\n",
            "failures:\n",
            "---- a::aborts stdout ----\n",
            "Error: \"harness aborted\"\n",
            "     Running tests\\later.rs (target\\debug\\deps\\later-2.exe)\n",
            "test later::ok ... ok\n",
            "test result: ok. 1 passed; 0 failed\n",
        );
        let outcome = parse_log(log);
        assert_eq!(outcome.targets, vec!["src/lib.rs", "tests/later.rs"]);
        assert!(outcome.passed.contains("later::ok"));
        assert_eq!(
            outcome.reasons.get("a::aborts").map(String::as_str),
            Some("Error: \"harness aborted\""),
        );
    }

    #[test]
    fn a_long_reason_is_truncated_to_the_bound() {
        let long = format!("Error: \"{}\"", "x".repeat(MAX_REASON_CHARS * 2));
        let log = format!("test a::long ... FAILED\nfailures:\n---- a::long stdout ----\n{long}\n");
        let reason = parse_log(&log)
            .reasons
            .remove("a::long")
            .unwrap_or_default();
        assert_eq!(reason.chars().count(), MAX_REASON_CHARS + 1);
        assert!(reason.ends_with('…'), "{reason}");
    }

    #[test]
    fn the_verdict_names_each_failure_reason_per_run() {
        let mut first = outcome(
            RunState::NonZeroWithObservedTestFailures,
            &["seam::flaky", "seam::silent"],
            &[],
        );
        first.reasons.insert(
            "seam::flaky".to_string(),
            "Error: \"marker `x` missing\"".to_string(),
        );
        let second = outcome(
            RunState::NonZeroWithObservedTestFailures,
            &["seam::silent"],
            &["seam::flaky"],
        );
        let rendered = render(&first, &second);
        assert!(rendered.contains("### Failure reasons"), "{rendered}");
        assert!(
            rendered.contains("- `seam::flaky`\n  - Run 1: Error: \"marker 'x' missing\"\n"),
            "{rendered}"
        );
        assert!(
            rendered.contains(
                "- `seam::silent`\n  - Run 1: no failure block captured\n  - Run 2: no failure block captured\n"
            ),
            "{rendered}"
        );
        let clean = outcome(RunState::CompletedClean, &[], &["seam::flaky"]);
        assert!(!render(&clean, &clean).contains("### Failure reasons"));
    }

    #[test]
    fn parses_ansi_coloured_runner_output_including_passes() {
        let parsed = parse_log(REAL_RUNNER_LOG);
        assert_eq!(
            parsed.targets,
            vec![
                "src/lib.rs".to_string(),
                "tests/lsp_lifecycle.rs".to_string()
            ]
        );
        assert!(
            parsed
                .failed
                .contains("compat_journey_collect_workspace_status_over_real_wire")
        );
        assert!(
            parsed.passed.contains("some::alpha"),
            "observed passes must be collected too: {:?}",
            parsed.passed
        );
        assert_eq!(parsed.results.len(), 2);
    }

    /// The rule this module exists for: a test absent from one run has not been
    /// observed passing, so it cannot be called a flake.
    #[test]
    fn absence_is_masked_unknown_not_unstable() {
        let first = outcome(RunState::NonZeroWithObservedTestFailures, &["x::y"], &[]);
        let second = outcome(RunState::CompletedClean, &[], &[]); // never reported x::y
        assert_eq!(
            classify(first.observe("x::y"), second.observe("x::y")),
            Some(Verdict::MaskedUnknown)
        );
        let rendered = render(&first, &second);
        assert!(rendered.contains("masked_unknown (1)"), "{rendered}");
        assert!(!rendered.contains("unstable ("), "{rendered}");
    }

    /// Only an explicitly observed pass on the other side makes a failure a flake.
    #[test]
    fn observed_pass_on_the_other_side_is_unstable() {
        let first = outcome(RunState::NonZeroWithObservedTestFailures, &["x::y"], &[]);
        let second = outcome(RunState::CompletedClean, &[], &["x::y"]);
        assert_eq!(
            classify(first.observe("x::y"), second.observe("x::y")),
            Some(Verdict::Unstable)
        );
        let rendered = render(&first, &second);
        assert!(rendered.contains("unstable (1)"), "{rendered}");
        assert!(!rendered.contains("masked_unknown ("), "{rendered}");
    }

    #[test]
    fn failing_twice_is_reported_as_repeated_not_deterministic() {
        let both = outcome(RunState::NonZeroWithObservedTestFailures, &["x::y"], &[]);
        assert_eq!(
            classify(both.observe("x::y"), both.observe("x::y")),
            Some(Verdict::RepeatedFailure)
        );
        let rendered = render(&both, &both);
        assert!(rendered.contains("repeated_failure (1)"), "{rendered}");
        assert!(
            !rendered.contains("deterministic platform defect"),
            "two samples do not establish determinism: {rendered}"
        );
    }

    #[test]
    fn non_zero_exit_without_parsed_failures_is_infrastructure() {
        let mut broken = outcome(RunState::CompileOrHarnessFailure, &[], &[]);
        broken.exit_status = Some(101);
        let clean = outcome(RunState::CompletedClean, &[], &["x::y"]);
        let rendered = render(&broken, &clean);
        assert!(rendered.contains("Infrastructure failure"), "{rendered}");
    }

    #[test]
    fn missing_evidence_renders_an_evidence_failure_and_no_pass_claim() {
        let missing = RunOutcome::missing(RunState::LogMissing);
        let clean = outcome(RunState::CompletedClean, &[], &["x::y"]);
        let rendered = render(&missing, &clean);
        assert!(rendered.contains("**Evidence failure.**"), "{rendered}");
        assert!(
            !rendered.contains("No test failed in either run."),
            "{rendered}"
        );
    }

    /// The hardest missing-evidence shape to read correctly: the logs parsed
    /// cleanly, so `### Totals` reports observed passes, but the captured exit
    /// status never arrived. Every number on the page looks like a clean run.
    /// The refusal has to survive that, or a reader skimming the totals would
    /// take an unverifiable run for a green one.
    #[test]
    fn pass_shaped_totals_still_refuse_a_verdict_when_the_status_is_missing() {
        let mut unverified = outcome(RunState::StatusMissing, &[], &["x::y"]);
        unverified.exit_status = None;
        let rendered = render(&unverified, &unverified);
        assert!(rendered.contains("**Evidence failure.**"), "{rendered}");
        assert!(
            rendered.contains("No verdict: see the evidence failure above."),
            "{rendered}"
        );
        assert!(
            !rendered.contains("No test failed in either run."),
            "pass-shaped totals must not be promoted to a clean verdict: {rendered}"
        );
        assert!(
            rendered.contains("observed 1 pass, 0 fail"),
            "the observed counts stay on the page; the banner is what refuses: {rendered}"
        );
    }

    /// The summarizer's refusal only protects the lane if a reader can see it.
    ///
    /// `run()` prints the rendered verdict and *then* returns an error for
    /// unusable evidence, so both halves travel the same pipe: the step's
    /// standard output carries the `**Evidence failure.**` banner and its exit
    /// status carries the failure. This binds the workflow side of that. A
    /// redirection operator on the command would consume `tee`'s standard
    /// output and leave the job log with a command echo and nothing else —
    /// which is the state issue #2393's lane was in until this step was
    /// changed — and a `|| true` or a `continue-on-error` would turn the
    /// refusal back into a green job.
    #[test]
    fn the_summarizer_verdict_reaches_the_job_log_and_its_refusal_is_not_suppressed() {
        let workflow = include_str!("../../.github/workflows/windows-advisory.yml");
        let mut checked = false;
        for command in workflow
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("cargo xtask windows-advisory-summary"))
        {
            checked = true;
            assert!(
                !command.contains('>'),
                "no redirection may divert the summarizer's stdout away from the job log: {command}"
            );
            assert!(
                command.contains("| tee target/windows-verdict.md"),
                "the verdict must still reach the artifact file: {command}"
            );
            assert!(
                command.contains(r#"| tee -a "$GITHUB_STEP_SUMMARY""#),
                "the verdict must still reach the step summary, appended not truncated: {command}"
            );
            assert!(
                !command.contains("|| true"),
                "an evidence failure must fail the step: {command}"
            );
        }
        assert!(checked, "the lane must invoke the summarizer");

        assert!(
            !workflow.contains("continue-on-error"),
            "test outcomes are advisory because the run steps exit 0, never because \
             a failure is swallowed; a continue-on-error would also swallow the \
             evidence refusal"
        );
    }

    fn control(test: &'static str) -> SeamControl {
        SeamControl {
            seam: "process",
            issue: "#3803",
            test,
            source: "crates/ripr/src/process_owner.rs",
        }
    }

    /// #3922: an absent release-seam control is an evidence failure in each
    /// usable run, while a failing control stays advisory.
    #[test]
    fn an_unobserved_release_seam_control_is_an_evidence_failure() {
        let controls = [control("seam::observed"), control("seam::absent")];
        let first = outcome(
            RunState::NonZeroWithObservedTestFailures,
            &["seam::observed"],
            &["seam::absent"],
        );
        let second = outcome(RunState::CompletedClean, &[], &["seam::observed"]);

        let missing = unobserved_controls(&first, &second, &controls);
        assert_eq!(
            missing,
            vec![
                "run 2 did not observe process control `seam::absent` \
                 (#3803, defined in crates/ripr/src/process_owner.rs)"
                    .to_string()
            ],
            "a failed control is observed; only the run that never reported one is refused"
        );

        let both = outcome(
            RunState::CompletedClean,
            &[],
            &["seam::observed", "seam::absent"],
        );
        assert!(unobserved_controls(&both, &both, &controls).is_empty());
    }

    /// An unusable run is refused once for itself, not again per control.
    #[test]
    fn an_unusable_run_is_not_double_reported_per_control() {
        let controls = [control("seam::absent")];
        let missing = RunOutcome::missing(RunState::LogMissing);
        let clean = outcome(RunState::CompletedClean, &[], &["seam::absent"]);
        assert!(unobserved_controls(&missing, &clean, &controls).is_empty());
    }

    #[test]
    fn the_verdict_reports_each_release_seam_control_per_run() -> Result<(), String> {
        let [first_control, second_control, ..] = RELEASE_SEAM_CONTROLS else {
            return Err("the lane must watch at least two release-seam controls".to_string());
        };
        let first = outcome(
            RunState::NonZeroWithObservedTestFailures,
            &[first_control.test],
            &[],
        );
        let second = outcome(RunState::CompletedClean, &[], &[first_control.test]);
        let rendered = render(&first, &second);
        assert!(
            rendered.contains(&format!(
                "| {} | {} | `{}` | FAILED | pass |",
                first_control.seam, first_control.issue, first_control.test
            )),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!(
                "`{}` | not_observed | not_observed |",
                second_control.test
            )),
            "{rendered}"
        );
        let unusable = RunOutcome::missing(RunState::StatusMissing);
        assert!(
            render(&unusable, &second)
                .contains(&format!("`{}` | no_evidence | pass |", first_control.test))
        );
        Ok(())
    }

    /// Every control names a test that exists where it says, so a rename breaks
    /// here on any platform instead of only on the Windows lane.
    #[test]
    fn every_release_seam_control_names_a_defined_test() -> Result<(), String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut names = BTreeSet::new();
        for control in RELEASE_SEAM_CONTROLS {
            assert!(
                names.insert(control.test),
                "duplicate control {}",
                control.test
            );
            let source = std::fs::read_to_string(root.join(control.source))
                .map_err(|err| format!("read {}: {err}", control.source))?;
            let leaf = control.test.rsplit("::").next().unwrap_or(control.test);
            assert!(
                source.contains(&format!("fn {leaf}(")),
                "{} does not define `{leaf}` for control {}",
                control.source,
                control.test
            );
        }
        Ok(())
    }

    #[test]
    fn run_state_labels_are_stable_wire_strings() {
        assert_eq!(RunState::CompletedClean.label(), "completed_clean");
        assert_eq!(
            RunState::NonZeroWithObservedTestFailures.label(),
            "nonzero_with_observed_test_failures"
        );
        assert_eq!(
            RunState::CompileOrHarnessFailure.label(),
            "compile_or_harness_failure"
        );
        assert_eq!(RunState::IncompleteEvidence.label(), "incomplete_evidence");
        assert_eq!(RunState::LogMissing.label(), "log_missing");
        assert_eq!(RunState::StatusMissing.label(), "status_missing");
        assert!(RunState::NonZeroWithObservedTestFailures.is_usable());
        assert!(RunState::CompletedClean.is_usable());
        assert!(!RunState::LogMissing.is_usable());
        assert!(!RunState::StatusMissing.is_usable());
        assert!(
            !RunState::IncompleteEvidence.is_usable(),
            "a zero status over an unrecognised log is not comparable evidence"
        );
    }

    /// A zero exit status is not evidence that tests ran. An empty, truncated, or
    /// non-`cargo test` log beside a `0` status must not be reported as clean —
    /// that is the same false-confidence error as treating an absent test as a
    /// passing one, in a different place.
    #[test]
    fn zero_status_over_an_empty_log_is_incomplete_evidence_not_clean() {
        let dir = std::env::temp_dir().join(format!(
            "ripr-winadv-incomplete-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0)
        ));
        let cleanup = |dir: &std::path::Path| {
            let _ = std::fs::remove_dir_all(dir);
        };
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let log = dir.join("empty.log");
        let status = dir.join("empty.status");
        if std::fs::write(&log, "").is_err() || std::fs::write(&status, "0\n").is_err() {
            cleanup(&dir);
            return;
        }
        let outcome = load_run(&log, &status);
        assert_eq!(outcome.state, RunState::IncompleteEvidence);
        assert_eq!(outcome.exit_status, Some(0));

        // Prose that is not a cargo test log is equally unusable.
        let noise = dir.join("noise.log");
        if std::fs::write(&noise, "some unrelated output\nnothing to see\n").is_ok() {
            assert_eq!(
                load_run(&noise, &status).state,
                RunState::IncompleteEvidence
            );
        }

        // A real log with targets and result lines is clean.
        let real = dir.join("real.log");
        if std::fs::write(&real, REAL_RUNNER_LOG).is_ok() {
            assert_eq!(load_run(&real, &status).state, RunState::CompletedClean);
        }

        let rendered = render(
            &RunOutcome {
                state: RunState::IncompleteEvidence,
                exit_status: Some(0),
                failed: BTreeSet::new(),
                passed: BTreeSet::new(),
                targets: Vec::new(),
                results: Vec::new(),
                reasons: BTreeMap::new(),
            },
            &outcome,
        );
        assert!(rendered.contains("**Evidence failure.**"), "{rendered}");
        assert!(
            !rendered.contains("No test failed in either run."),
            "an unusable pair must not claim a clean result: {rendered}"
        );
        cleanup(&dir);
    }

    /// Sharing a status file between runs would hand one run the other's exit
    /// code, so a clean run could be labelled a harness failure — a wrong
    /// verdict, not an error. All four paths must be distinct.
    #[test]
    fn duplicate_paths_are_rejected_including_status_paths() {
        let args = |values: &[&str]| -> Vec<String> {
            values.iter().map(|value| (*value).to_string()).collect()
        };
        let shared_status = args(&[
            "--run1",
            "a.log",
            "--run1-status",
            "x.status",
            "--run2",
            "b.log",
            "--run2-status",
            "x.status",
        ]);
        let error = run(&shared_status).err().unwrap_or_default();
        assert!(
            error.contains("four distinct paths"),
            "shared status paths must be refused: {error}"
        );

        let shared_log = args(&[
            "--run1",
            "a.log",
            "--run1-status",
            "1.status",
            "--run2",
            "a.log",
            "--run2-status",
            "2.status",
        ]);
        let error = run(&shared_log).err().unwrap_or_default();
        assert!(
            error.contains("four distinct paths"),
            "shared logs must be refused: {error}"
        );
    }

    /// Doctest names contain spaces. Rejecting them would drop every doctest
    /// from the observation model, so a doctest could fail in one run and be
    /// silently absent from the verdict.
    #[test]
    fn doctest_names_containing_spaces_are_observed() {
        let parsed = parse_log(
            "test src/lib.rs - foo::bar (line 12) ... ok\ntest src/lib.rs - baz::qux (line 30) ... FAILED\n",
        );
        assert!(
            parsed.passed.contains("src/lib.rs - foo::bar (line 12)"),
            "{:?}",
            parsed.passed
        );
        assert!(
            parsed.failed.contains("src/lib.rs - baz::qux (line 30)"),
            "{:?}",
            parsed.failed
        );
    }

    #[test]
    fn strip_ansi_removes_sgr_sequences_and_keeps_text() {
        assert_eq!(
            strip_ansi("\u{1b}[1m\u{1b}[92m     Running\u{1b}[0m unittests src/lib.rs"),
            "     Running unittests src/lib.rs"
        );
        assert_eq!(strip_ansi("plain text"), "plain text");
    }

    /// An ESC that does not introduce a CSI sequence must drop only the ESC.
    /// Consuming the next character unconditionally deleted real text, which
    /// could silently truncate a test name.
    #[test]
    fn strip_ansi_keeps_the_character_after_a_lone_escape() {
        assert_eq!(strip_ansi("a\u{1b}Xb"), "aXb");
        assert_eq!(strip_ansi("\u{1b}"), "");
        assert_eq!(strip_ansi("test some::name\u{1b}"), "test some::name");
        // The pathological case this protects: a stray ESC before the shape the
        // parser matches on.
        assert_eq!(
            strip_ansi("test \u{1b}some::name ... FAILED"),
            "test some::name ... FAILED"
        );
    }

    /// A line beginning `Running ` without a parenthesised binary is not a test
    /// target — build scripts and custom commands can produce one.
    #[test]
    fn running_target_requires_a_parenthesised_binary() {
        assert_eq!(
            running_target("Running unittests src/lib.rs (target/debug/deps/a-1.exe)"),
            Some("src/lib.rs".to_string())
        );
        assert_eq!(running_target("Running a custom build command"), None);
        assert_eq!(running_target("Running"), None);
    }

    #[test]
    fn ignores_lines_that_only_resemble_a_test_result() {
        let parsed = parse_log("failures:\n    some::name\ntest result: FAILED. 1 failed\n");
        assert!(parsed.failed.is_empty(), "{:?}", parsed.failed);
        assert!(parsed.passed.is_empty(), "{:?}", parsed.passed);
    }

    fn argv(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn complete_argv() -> Vec<String> {
        argv(&[
            "--run1",
            "1.log",
            "--run1-status",
            "1.status",
            "--run2",
            "2.log",
            "--run2-status",
            "2.status",
        ])
    }

    /// The whole argument list is validated, asserted on error values so the
    /// operator-facing message is pinned and no panic-family call is needed.
    #[test]
    fn parse_inputs_accepts_exactly_four_distinct_roles() {
        assert_eq!(
            parse_inputs(&complete_argv()),
            Ok(Inputs {
                run1_log: "1.log".to_string(),
                run1_status: "1.status".to_string(),
                run2_log: "2.log".to_string(),
                run2_status: "2.status".to_string(),
            })
        );
    }

    #[test]
    fn parse_inputs_rejects_every_malformed_argument_shape() {
        // Missing entirely: every absent role is named with its expected value.
        assert_eq!(
            parse_inputs(&[]),
            Err("windows-advisory-summary is missing --run1 <path>, --run1-status <path>, --run2 <path>, --run2-status <path>".to_string())
        );
        // A partial invocation names only what is actually absent.
        assert_eq!(
            parse_inputs(&argv(&["--run1", "1.log", "--run2", "2.log"])),
            Err(
                "windows-advisory-summary is missing --run1-status <path>, --run2-status <path>"
                    .to_string()
            )
        );
        // Missing final value.
        assert_eq!(
            parse_inputs(&argv(&["--run1"])),
            Err("windows-advisory-summary requires a value for --run1".to_string())
        );
        // A flag consumed as a path (the Gemini finding).
        assert_eq!(
            parse_inputs(&argv(&["--run1", "--run2", "b.log"])),
            Err("windows-advisory-summary got --run1 followed by --run2, which looks like a flag rather than a path".to_string())
        );
        // Duplicated flag.
        assert_eq!(
            parse_inputs(&argv(&["--run1", "a.log", "--run1", "b.log"])),
            Err("windows-advisory-summary got --run1 more than once; pass it once".to_string())
        );
        // An unknown flag must be refused, not ignored.
        let mut unknown = complete_argv();
        unknown.push("--verbose".to_string());
        assert!(
            parse_inputs(&unknown)
                .err()
                .unwrap_or_default()
                .contains("unexpected argument \"--verbose\""),
            "an unknown flag must be refused"
        );
        // A stray positional must be refused too: silently ignoring it is how a
        // caller believes it supplied something the command never read.
        let mut positional = complete_argv();
        positional.push("extra.log".to_string());
        assert!(
            parse_inputs(&positional)
                .err()
                .unwrap_or_default()
                .contains("unexpected argument \"extra.log\""),
            "a stray positional must be refused"
        );
    }

    #[test]
    fn packaged_qualification_workflow_is_immutable_and_non_publishing() {
        let workflow = include_str!("../../.github/workflows/windows-packaged-qualification.yml");
        let lines = workflow
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect::<Vec<_>>();
        let scalar = |key: &str| {
            lines
                .iter()
                .find_map(|line| {
                    line.strip_prefix(key)
                        .and_then(|value| value.strip_prefix(':'))
                })
                .map(str::trim)
                .map(|value| value.trim_matches('"').trim_matches('\''))
        };
        assert_eq!(scalar("contents"), Some("read"));
        assert_eq!(scalar("persist-credentials"), Some("false"));
        assert_eq!(scalar("ref"), Some("${{ inputs.candidate_sha }}"));
        assert!(lines.iter().any(|line| line.starts_with("candidate_sha:")));
        assert!(lines.iter().any(|line| line.starts_with("candidate_ref:")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("refs/tags/ripr-release-0\\.11\\.0-"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("$remoteLine = git ls-remote --exit-code origin"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("$remoteSha -ne $head"))
        );
        assert!(lines.iter().any(|line| line.contains("$refSha -ne $head")));
        assert!(lines.iter().any(|line| {
            line.contains("$candidateTagSha -ne $env:CANDIDATE_SHA.ToLowerInvariant()")
        }));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("RIPR_TEST_SERVER_PATH = $env:RIPR_PACKAGED"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("if ($LASTEXITCODE -ne 0)"))
        );
        assert!(lines.iter().any(|line| line.starts_with("if (-not $binaryPath.StartsWith($env:QUAL_TEMP_ROOT")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("RIPR_TEST_SERVER_PATH"))
        );
        assert!(lines.iter().any(|line| {
            line.contains("actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a")
        }));
        for forbidden in ["gh release", "vsce publish", "ovsx publish", "secrets."] {
            assert!(
                !lines.iter().any(|line| line.contains(forbidden)),
                "workflow must not publish or use secrets: {forbidden}"
            );
        }
    }

    #[test]
    fn packaged_qualification_cli_helper_preserves_arguments() -> Result<(), String> {
        let workflow = include_str!("../../.github/workflows/windows-packaged-qualification.yml");
        let helper = workflow
            .lines()
            .find(|line| line.trim_start().starts_with("function Invoke-CLI"))
            .ok_or_else(|| "workflow must define the packaged CLI helper".to_string())?;
        if !helper.contains("[string[]]$cliArgs") {
            return Err("packaged CLI helper must use a named argument parameter".to_string());
        }
        if helper.contains("$args") {
            return Err(
                "packaged CLI helper must not shadow PowerShell's automatic $args".to_string(),
            );
        }
        for required in [
            "if ($null -eq $cliArgs -or $cliArgs.Count -eq 0)",
            "throw \"packaged CLI $name requires arguments\"",
            "& $env:RIPR_PACKAGED @cliArgs",
        ] {
            if !helper.contains(required) {
                return Err(format!("Invoke-CLI must contain {required:?}"));
            }
        }
        let guard = helper
            .find("if ($null -eq $cliArgs -or $cliArgs.Count -eq 0)")
            .ok_or_else(|| "Invoke-CLI must guard empty arguments".to_string())?;
        let invocation = helper
            .find("& $env:RIPR_PACKAGED @cliArgs")
            .ok_or_else(|| "Invoke-CLI must forward arguments".to_string())?;
        if guard > invocation {
            return Err("Invoke-CLI must validate arguments before invocation".to_string());
        }
        for required in [
            "Invoke-CLI 'version' @('--version')",
            "packaged --version did not report the package version",
        ] {
            if !workflow.contains(required) {
                return Err(format!(
                    "packaged CLI qualification must contain {required:?}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn packaged_qualification_journey_matches_cli_contract() -> Result<(), String> {
        let workflow = include_str!("../../.github/workflows/windows-packaged-qualification.yml");
        let explain = workflow
            .lines()
            .find(|line| line.contains("Invoke-CLI 'explain'"))
            .ok_or_else(|| "workflow must run the packaged explain journey".to_string())?;
        let context = workflow
            .lines()
            .find(|line| line.contains("Invoke-CLI 'context'"))
            .ok_or_else(|| "workflow must run the packaged context journey".to_string())?;
        let pilot = workflow
            .lines()
            .find(|line| line.contains("Invoke-CLI 'pilot'"))
            .ok_or_else(|| "workflow must run the packaged pilot journey".to_string())?;
        let outcome = workflow
            .lines()
            .find(|line| line.contains("Invoke-CLI 'outcome'"))
            .ok_or_else(|| "workflow must run the packaged outcome journey".to_string())?;

        // `explain` has a positional selector and intentionally has no JSON
        // switch. A prior workflow passed --finding and --json, so the
        // packaged binary stopped before the remaining journeys ran.
        if !explain.contains("'--from', $artifact, $finding.id") {
            return Err(
                "explain must pass the artifact followed by the positional finding selector"
                    .to_string(),
            );
        }
        if explain.contains("'--finding'") || explain.contains("'--json'") {
            return Err("explain must not use context-only selector/output flags".to_string());
        }
        if !context.contains("'--from', $artifact, '--at', $finding.id, '--json'") {
            return Err("context must use --at and --json with the check artifact".to_string());
        }
        if explain.contains("Invoke-CLI 'context'") || !context.contains("Invoke-CLI 'context'") {
            return Err(
                "explain and context journeys must remain separate command lines".to_string(),
            );
        }
        if context.contains("Invoke-CLI 'explain'") || !explain.contains("Invoke-CLI 'explain'") {
            return Err("explain and context journeys must not be coalesced".to_string());
        }

        let cli_commands = include_str!("../../crates/ripr/src/cli/commands.rs");
        let cli_context = include_str!("../../crates/ripr/src/cli/commands/context.rs");
        let cli_pilot = include_str!("../../crates/ripr/src/cli/commands/pilot.rs");
        let cli_help = include_str!("../../crates/ripr/src/cli/help/core.rs");
        for source in [cli_commands, cli_context, cli_pilot] {
            if !source.contains("--root") {
                return Err("packaged journey contract sources must retain --root".to_string());
            }
        }
        for flag in ["--from", "--base"] {
            if !cli_commands.contains(&format!("\"{flag}\"")) {
                return Err(format!("explain/context parser must support {flag}"));
            }
        }
        for flag in ["--at", "--json"] {
            if !cli_context.contains(&format!("\"{flag}\"")) {
                return Err(format!("context parser must support {flag}"));
            }
        }
        for flag in ["--out", "--max-seams", "--timeout-ms"] {
            if !cli_pilot.contains(&format!("\"{flag}\"")) {
                return Err(format!("pilot parser must support {flag}"));
            }
        }
        for flag in ["--before", "--after", "--format"] {
            if !cli_commands.contains(&format!("\"{flag}\"")) {
                return Err(format!("outcome parser must support {flag}"));
            }
        }
        for usage in [
            "Usage: ripr explain",
            "<finding-id|file:line>",
            "Usage: ripr context",
            "--at <finding-id|file:line>",
            "Usage: ripr pilot",
            "Usage: ripr outcome",
        ] {
            if !cli_help.contains(usage) {
                return Err(format!("CLI help must document {usage:?}"));
            }
        }

        // Keep the remaining commands visible in the same journey line so a
        // future edit cannot silently stop after repairing explain/context.
        for command in ["'pilot'", "'outcome'"] {
            if !workflow.contains(&format!("Invoke-CLI {command}")) {
                return Err(format!("workflow must retain the {command} journey"));
            }
        }
        if !pilot.contains("'--root', $fixture")
            || !pilot.contains("'--out'")
            || !pilot.contains("'--max-seams'")
            || !pilot.contains("'--timeout-ms'")
        {
            return Err("pilot journey flags drifted from its public contract".to_string());
        }
        if !outcome.contains("'--before', $before")
            || !outcome.contains("'--after', $after")
            || !outcome.contains("'--format', 'json'")
        {
            return Err("outcome journey flags drifted from its public contract".to_string());
        }
        Ok(())
    }

    #[test]
    fn packaged_qualification_receipts_survive_checkout_cleanup() -> Result<(), String> {
        let workflow = include_str!("../../.github/workflows/windows-packaged-qualification.yml");
        let lines = workflow.lines().map(str::trim).collect::<Vec<_>>();
        let checkout = lines
            .iter()
            .position(|line| line.starts_with("- name: Checkout immutable candidate"))
            .ok_or_else(|| "workflow must retain the immutable checkout step".to_string())?;
        let before = lines
            .iter()
            .position(|line| line.starts_with("- name: Initialize receipt root before checkout"))
            .ok_or_else(|| "workflow must initialize the early-failure receipt root".to_string())?;
        let after = lines
            .iter()
            .position(|line| line.starts_with("- name: Initialize receipt root after checkout"))
            .ok_or_else(|| "workflow must reinitialize the root after checkout".to_string())?;
        if !(before < checkout && checkout < after) {
            return Err("receipt-root initialization must bracket checkout".to_string());
        }
        if !lines
            .iter()
            .any(|line| line.contains("$receipts = Join-Path $base 'receipts'"))
        {
            return Err("receipt root must use the dedicated receipts child".to_string());
        }
        if !lines
            .iter()
            .any(|line| line.contains("$work = Join-Path $base 'work'"))
        {
            return Err("qualification work must use the dedicated work child".to_string());
        }
        if !lines
            .iter()
            .any(|line| line.contains("QUAL_ROOT=$receipts"))
            || !lines
                .iter()
                .any(|line| line.contains("QUAL_TEMP_ROOT=$work"))
        {
            return Err("receipt and work roots must be exported separately".to_string());
        }
        let upload_path = lines
            .iter()
            .find(|line| line.starts_with("path:"))
            .ok_or_else(|| "artifact upload must declare a bounded path".to_string())?;
        if upload_path != &"path: ${{ runner.temp }}\\ripr-windows-packaged-qualification\\receipts"
        {
            return Err("artifact upload must target only the receipts root".to_string());
        }
        if upload_path.contains("\\work") || upload_path.contains("qualification\\receipts\\work") {
            return Err("artifact upload must not include qualification work files".to_string());
        }

        if !lines.iter().any(|line| {
            line.contains("path: ${{ runner.temp }}\\ripr-windows-packaged-qualification\\receipts")
        }) {
            return Err("artifact upload must use the durable receipt root".to_string());
        }

        // Model checkout cleaning the workspace while the receipt root lives
        // in RUNNER_TEMP. A failure before identity verification must still
        // leave a file for the always-run upload step to collect.
        let sandbox = std::env::temp_dir().join(format!(
            "ripr-windows-receipt-contract-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0)
        ));
        let workspace = sandbox.join("workspace");
        let receipt_root = sandbox.join("runner-temp").join("receipts");
        let work_root = sandbox.join("runner-temp").join("work");
        if receipt_root == work_root
            || receipt_root.starts_with(&work_root)
            || work_root.starts_with(&receipt_root)
        {
            return Err("receipt and work roots must be distinct non-nested paths".to_string());
        }
        if std::fs::create_dir_all(workspace.join("target"))
            .and(std::fs::create_dir_all(&receipt_root))
            .and(std::fs::create_dir_all(&work_root))
            .is_err()
        {
            return Err("unable to create checkout-cleanup test directories".to_string());
        }
        let failure_receipt = receipt_root.join("failure-receipt.txt");
        let work_file = work_root.join(r"build\install\vsix.bin");
        let result = (|| {
            std::fs::remove_dir_all(&workspace)?;
            std::fs::write(&failure_receipt, "identity verification failed")?;
            std::fs::create_dir_all(work_file.parent().ok_or_else(|| {
                std::io::Error::other("work-file parent directory was not available")
            })?)?;
            std::fs::write(&work_file, "not a receipt")?;
            if !failure_receipt.is_file() {
                return Err(std::io::Error::other("failure receipt was not retained"));
            }
            let uploaded = std::fs::read_dir(&receipt_root)?
                .map(|entry| entry.map(|item| item.path()))
                .collect::<Result<Vec<_>, _>>()?;
            if uploaded.iter().any(|path| path == &work_file) {
                return Err(std::io::Error::other(
                    "artifact receipt root included a qualification work file",
                ));
            }
            Ok::<(), std::io::Error>(())
        })();
        let _ = std::fs::remove_dir_all(&sandbox);
        result.map_err(|error| format!("failure receipts must survive checkout cleanup: {error}"))
    }

    #[test]
    fn packaged_qualification_overrides_workspace_temp_for_package_builds() -> Result<(), String> {
        let workflow = include_str!("../../.github/workflows/windows-packaged-qualification.yml");
        let lines = workflow.lines().map(str::trim).collect::<Vec<_>>();
        let isolate = lines
            .iter()
            .position(|line| line.starts_with("- name: Isolate package installation"))
            .ok_or_else(|| "workflow must isolate package installation".to_string())?;
        let package_line = lines
            .iter()
            .position(|line| line.contains(" package --target-dir"))
            .ok_or_else(|| "workflow must package the exact crate".to_string())?;
        let install_line = lines
            .iter()
            .position(|line| line.contains(" install --target-dir"))
            .ok_or_else(|| "workflow must install the extracted crate".to_string())?;
        if !(isolate < package_line && package_line < install_line) {
            return Err("Cargo temp isolation must precede both package builds".to_string());
        }
        let package = lines[package_line].split_whitespace().collect::<Vec<_>>();
        let install = lines[install_line].split_whitespace().collect::<Vec<_>>();
        let position = |tokens: &[&str], token: &str| {
            tokens
                .iter()
                .position(|candidate| *candidate == token)
                .ok_or_else(|| format!("command must contain {token:?}"))
        };
        let package_command = position(&package, "package")?;
        let package_target = position(&package, "--target-dir")?;
        let package_name = position(&package, "-p")?;
        let package_locked = position(&package, "--locked")?;
        if !(package_command < package_target
            && package_target < package_name
            && package_name < package_locked
            && package.get(package_name + 1) == Some(&"ripr"))
        {
            return Err(
                "package command must order package, target-dir, -p ripr, and locked".to_string(),
            );
        }
        let install_command = position(&install, "install")?;
        let install_target = position(&install, "--target-dir")?;
        let install_path = position(&install, "--path")?;
        if !(install_command < install_target
            && install_target < install_path
            && install.get(install_path + 1) == Some(&"$packageDir.FullName"))
        {
            return Err(
                "install command must order install, target-dir, and --path package dir"
                    .to_string(),
            );
        }
        if !packaged_temp_wiring_is_complete(workflow) {
            return Err(
                "Cargo temp process assignments and cross-step exports must be complete"
                    .to_string(),
            );
        }
        for required in [
            "$env:CARGO_TEMP_DIR = Join-Path $temp 'cargo-temp'",
            "$env:CARGO_TEMP_CONFIG = Join-Path $temp 'cargo-package-config.toml'",
            "$env:CARGO_TARGET_DIR = Join-Path $temp 'cargo-target'",
            "$env:TEMP = $env:CARGO_TEMP_DIR",
            "$env:TMP = $env:CARGO_TEMP_DIR",
            "$env:TMPDIR = $env:CARGO_TEMP_DIR",
            "TEMP = { value = '$tomlTemp', force = true, relative = false }",
            "TMP = { value = '$tomlTemp', force = true, relative = false }",
            "TMPDIR = { value = '$tomlTemp', force = true, relative = false }",
            "New-Item -ItemType Directory -Force -Path $env:CARGO_HOME, $env:CARGO_TARGET_DIR, $env:CARGO_TEMP_DIR",
            "\"CARGO_TEMP_DIR=$env:CARGO_TEMP_DIR\" >> $env:GITHUB_ENV",
            "\"CARGO_TEMP_CONFIG=$env:CARGO_TEMP_CONFIG\" >> $env:GITHUB_ENV",
            "\"CARGO_TARGET_DIR=$env:CARGO_TARGET_DIR\" >> $env:GITHUB_ENV",
            "\"TEMP=$env:TEMP\" >> $env:GITHUB_ENV",
            "\"TMP=$env:TMP\" >> $env:GITHUB_ENV",
            "\"TMPDIR=$env:TMPDIR\" >> $env:GITHUB_ENV",
            "cargo_temp_config = $env:CARGO_TEMP_CONFIG",
            "temp = $env:TEMP",
            "tmp = $env:TMP",
            "tmpdir = $env:TMPDIR",
        ] {
            if !lines.iter().any(|line| line.contains(required)) {
                return Err(format!("workflow must contain {required:?}"));
            }
        }
        Ok(())
    }

    #[test]
    fn packaged_qualification_vsix_compile_uses_external_cargo_config() -> Result<(), String> {
        let workflow = include_str!("../../.github/workflows/windows-packaged-qualification.yml");
        let lines = workflow.lines().map(str::trim).collect::<Vec<_>>();
        let isolate = lines
            .iter()
            .position(|line| line.starts_with("- name: Isolate package installation"))
            .ok_or_else(|| "workflow must isolate package installation".to_string())?;
        let compile = lines
            .iter()
            .position(|line| line.contains("xtask vscode-compile"))
            .ok_or_else(|| "workflow must compile the VSIX through xtask".to_string())?;
        if compile <= isolate {
            return Err("VSIX compile must run after Cargo isolation".to_string());
        }
        let command = lines[compile];
        let cargo_invocation = command
            .split(';')
            .map(str::trim)
            .find(|segment| segment.contains("xtask vscode-compile"))
            .ok_or_else(|| "VSIX compile command must contain a Cargo invocation".to_string())?;
        for required in [
            "cargo --config $env:CARGO_TEMP_CONFIG",
            "xtask vscode-compile",
        ] {
            if !cargo_invocation.contains(required) {
                return Err(format!(
                    "VSIX compile must carry the external Cargo setting {required:?}"
                ));
            }
        }
        let unisolated = cargo_invocation.replace(
            "cargo --config $env:CARGO_TEMP_CONFIG xtask vscode-compile",
            "npm run compile",
        );
        if unisolated != "npm run compile" {
            return Err(
                "test fixture did not model the unisolated npm compile command".to_string(),
            );
        }
        if unisolated.contains("CARGO_TEMP_CONFIG") || unisolated.contains("CARGO_TARGET_DIR") {
            return Err(
                "unisolated compile mutation unexpectedly retained Cargo isolation".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn packaged_qualification_temp_contract_rejects_each_missing_export() {
        let workflow = include_str!("../../.github/workflows/windows-packaged-qualification.yml");
        let required = [
            "$env:CARGO_TEMP_CONFIG = Join-Path $temp 'cargo-package-config.toml'",
            "$env:CARGO_TARGET_DIR = Join-Path $temp 'cargo-target'",
            "$env:TEMP = $env:CARGO_TEMP_DIR",
            "$env:TMP = $env:CARGO_TEMP_DIR",
            "$env:TMPDIR = $env:CARGO_TEMP_DIR",
            "\"CARGO_TEMP_CONFIG=$env:CARGO_TEMP_CONFIG\" >> $env:GITHUB_ENV",
            "\"CARGO_TARGET_DIR=$env:CARGO_TARGET_DIR\" >> $env:GITHUB_ENV",
            "\"TEMP=$env:TEMP\" >> $env:GITHUB_ENV",
            "\"TMP=$env:TMP\" >> $env:GITHUB_ENV",
            "\"TMPDIR=$env:TMPDIR\" >> $env:GITHUB_ENV",
        ];
        for missing in required {
            let mutated = workflow.replacen(missing, "", 1);
            assert!(
                !mutated.contains(missing),
                "negative fixture retained {missing:?}"
            );
            assert!(
                !packaged_temp_wiring_is_complete(&mutated),
                "contract must reject missing {missing:?}"
            );
        }
    }
}
