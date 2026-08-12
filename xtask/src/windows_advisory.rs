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

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

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
    if unusable.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "windows-advisory-summary could not produce trustworthy evidence: {}",
            unusable.join("; ")
        ))
    }
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
    for raw_line in text.lines() {
        let line = strip_ansi(raw_line);
        let trimmed = line.trim();
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
    outcome
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
        }
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
