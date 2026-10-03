//! Production-command discriminators for #5043 and #5107. The Cargo-shaped logs are
//! synthetic; they establish parser identity, not native Windows execution.
//! The control inventory is deliberately independent of the implementation.
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const CONTROLS: &[(&str, &str, &str)] = &[
    (
        "unittests src/lib.rs",
        "ripr",
        "process_owner::tests::owner_drop_terminates_a_still_running_child",
    ),
    (
        "unittests src/lib.rs",
        "ripr",
        "process_owner::tests::terminate_tree_kills_pipe_inheriting_descendants",
    ),
    (
        "unittests src/lib.rs",
        "ripr",
        "process_owner::tests::terminate_tree_leaves_unrelated_processes_alive",
    ),
    (
        "unittests src/lib.rs",
        "ripr",
        "process_owner::tests::owner_drop_kills_descendants_after_the_primary_exits",
    ),
    (
        "unittests src/lib.rs",
        "ripr",
        "process_owner::tests::terminate_tree_after_primary_exit_kills_descendants",
    ),
    (
        "unittests src/main.rs",
        "xtask",
        "run::tests::capture_output_with_timeout_terminates_pipe_inheriting_descendants",
    ),
    (
        "unittests src/lib.rs",
        "ripr",
        "lsp::tests::initialize_surfaces_poisoned_client_features_store_as_a_session_failure",
    ),
    (
        "unittests src/lib.rs",
        "ripr",
        "lsp::tests::poisoned_initialize_failure_commit_survives_a_wedged_client_channel",
    ),
    (
        "unittests src/lib.rs",
        "ripr",
        "analysis::seam_cache::tests::corpus_fingerprint_is_none_without_a_content_change_witness",
    ),
    (
        "tests/native_path_roots.rs",
        "native_path_roots",
        "spaced_root_check_and_file_fact_cache_round_trip",
    ),
    (
        "tests/native_path_roots.rs",
        "native_path_roots",
        "unicode_root_check_and_file_fact_cache_round_trip",
    ),
    (
        "tests/native_path_roots.rs",
        "native_path_roots",
        "long_root_check_round_trips_or_names_the_windows_path_limit",
    ),
    (
        "tests/native_path_roots.rs",
        "native_path_roots",
        "powershell_launch_passes_a_quoted_unicode_root_to_ripr",
    ),
    (
        "tests/lsp_lifecycle.rs",
        "lsp_lifecycle",
        "refresh_publishes_diagnostics_under_a_spaced_unicode_root",
    ),
    (
        "tests/lsp_lifecycle.rs",
        "lsp_lifecycle",
        "refresh_under_a_root_beyond_max_path_publishes_or_names_the_windows_path_limit",
    ),
];

const HASH: &str = "1111111111111111";
const NAME: &str = "same_name";

fn target(source: &str, stem: &str, rows: &[(&str, &str, &str)]) -> String {
    let mut log = format!(
        "Running {source} (target/debug/deps/{stem}-{HASH}.exe)\nrunning {} tests\n",
        rows.len()
    );
    for (name, result, _) in rows {
        log.push_str(&format!("test {name} ... {result}\n"));
    }
    let failures = rows
        .iter()
        .filter(|(_, result, _)| *result == "FAILED")
        .collect::<Vec<_>>();
    if !failures.is_empty() {
        log.push_str("failures:\n");
        for (name, _, reason) in &failures {
            log.push_str(&format!("---- {name} stdout ----\nError: {reason}\n\n"));
        }
        log.push_str("failures:\n");
        for (name, _, _) in &failures {
            log.push_str(&format!("    {name}\n"));
        }
    }
    log.push_str(&format!("test result: {}. {} passed; {} failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n", if failures.is_empty() { "ok" } else { "FAILED" }, rows.len()-failures.len(), failures.len()));
    log
}

fn controls(omit: Option<&str>) -> String {
    controls_adjusted(omit, None, None)
}

type ExtraRow<'a> = (&'a str, &'a str, (&'a str, &'a str, &'a str));

fn controls_adjusted(
    omit: Option<&str>,
    extra: Option<ExtraRow<'_>>,
    failing: Option<&str>,
) -> String {
    let mut groups = BTreeMap::<(&str, &str), Vec<(&str, &str, &str)>>::new();
    for &(source, stem, name) in CONTROLS {
        if Some(name) != omit {
            groups.entry((source, stem)).or_default().push((
                name,
                if Some(name) == failing {
                    "FAILED"
                } else {
                    "ok"
                },
                "owning control reason",
            ));
        }
    }
    if let Some((source, stem, row)) = extra {
        groups.entry((source, stem)).or_default().push(row);
    }
    groups
        .into_iter()
        .map(|((source, stem), rows)| target(source, stem, &rows))
        .collect()
}

fn alpha(result: &str) -> String {
    target("tests/alpha.rs", "alpha", &[(NAME, result, "alpha reason")])
}
fn beta(result: &str) -> String {
    target("tests/beta.rs", "beta", &[(NAME, result, "beta reason")])
}

struct TempRoot(PathBuf);
impl TempRoot {
    fn create(path: PathBuf) -> std::io::Result<Self> {
        // Existing directories do not grant ownership to this cleanup guard.
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn invoke(first: &str, second: &str) -> Result<Output, String> {
    invoke_with_missing(first, second, None)
}

// Match the process-local sequence pattern used by common/fixture_git.rs.
// The clock and PID alone are not a parallel-uniqueness guarantee.
fn corpus_root(nonce: u128) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);
    let sequence = NEXT_ROOT.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "ripr-winadv-identity-{}-{nonce}-{sequence}",
        std::process::id()
    ))
}

fn invoke_with_missing(first: &str, second: &str, missing: Option<&str>) -> Result<Output, String> {
    invoke_artifacts(first, second, missing, None)
}

fn invoke_artifacts(
    first: &str,
    second: &str,
    missing: Option<&str>,
    statuses: Option<[i32; 2]>,
) -> Result<Output, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let root = TempRoot::create(corpus_root(nonce)).map_err(|e| e.to_string())?;
    for (index, (label, log)) in [("run1", first), ("run2", second)].into_iter().enumerate() {
        fs::write(root.0.join(format!("{label}.log")), log).map_err(|e| e.to_string())?;
        let status = statuses.map_or_else(
            || if log.contains(" ... FAILED") { 101 } else { 0 },
            |values| values[index],
        );
        fs::write(
            root.0.join(format!("{label}.status")),
            format!("{status}\n"),
        )
        .map_err(|e| e.to_string())?;
    }
    if let Some(name) = missing {
        fs::remove_file(root.0.join(name)).map_err(|e| e.to_string())?;
    }
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("windows-advisory-summary")
        .arg("--run1")
        .arg(root.0.join("run1.log"))
        .arg("--run1-status")
        .arg(root.0.join("run1.status"))
        .arg("--run2")
        .arg(root.0.join("run2.log"))
        .arg("--run2-status")
        .arg(root.0.join("run2.status"))
        .output()
        .map_err(|e| format!("launch summarizer: {e}"))
}

fn verify(
    first: String,
    second: String,
    exit: i32,
    required: &[&str],
    forbidden: &[&str],
) -> Result<(), String> {
    let output = invoke(&first, &second)?;
    verify_output(output, exit, required, forbidden)
}

fn verify_output(
    output: Output,
    exit: i32,
    required: &[&str],
    forbidden: &[&str],
) -> Result<(), String> {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if output.status.code() != Some(exit) {
        return Err(format!(
            "expected exit {exit}, got {:?}: {text}",
            output.status.code()
        ));
    }
    for needle in required {
        if !text.contains(needle) {
            return Err(format!("missing {needle:?}: {text}"));
        }
    }
    for needle in forbidden {
        if text.contains(needle) {
            return Err(format!("unexpected {needle:?}: {text}"));
        }
    }
    Ok(())
}

fn replace_first_completion(log: &str, replacement: &str) -> Result<String, String> {
    let original = log
        .lines()
        .find(|line| line.starts_with("test result:"))
        .ok_or_else(|| "fixture has no completion to replace".to_string())?;
    Ok(log.replacen(original, replacement, 1))
}

#[test]
fn consistent_completion_preserves_all_required_controls() -> Result<(), String> {
    let log = controls(None);
    verify(
        log.clone(),
        log,
        0,
        &[
            "Run 1: `completed_clean`",
            "Run 2: `completed_clean`",
            "observed 15 pass, 0 fail",
            "No test failed in either run.",
        ],
        &["Evidence failure", "not_observed"],
    )
}

#[test]
fn contradictory_completion_cannot_credit_passing_rows() -> Result<(), String> {
    let clean = controls(None);
    // The first target is lsp_lifecycle with two passing control rows. Keep
    // target identity, announcement, rows and captured exit zero unchanged.
    assert!(clean.starts_with("Running tests/lsp_lifecycle.rs"));
    let contradictory = replace_first_completion(
        &clean,
        "test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s",
    )?;
    verify(
        contradictory,
        clean,
        1,
        &[
            "Run 1: `incomplete_evidence`",
            "provenance:",
            "observed 15 pass, 0 fail",
        ],
        &["Run 1: `completed_clean`", "No test failed in either run."],
    )
}

#[test]
fn malformed_completion_is_not_completion_evidence() -> Result<(), String> {
    let clean = controls(None);
    verify(
        replace_first_completion(&clean, "test result: not a libtest summary")?,
        clean,
        1,
        &["Run 1: `incomplete_evidence`", "provenance:"],
        &["Run 1: `completed_clean`", "No test failed in either run."],
    )
}

#[test]
fn an_orphan_completion_cannot_complete_a_run() -> Result<(), String> {
    let clean = controls(None);
    let orphan = "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n";
    verify(
        clean.clone() + orphan,
        clean,
        1,
        &["Run 1: `incomplete_evidence`", "provenance:"],
        &["Run 1: `completed_clean`", "No test failed in either run."],
    )
}

#[test]
fn missing_peer_evidence_cannot_issue_a_cross_run_verdict() -> Result<(), String> {
    let first = controls(None) + &alpha("FAILED");
    let output = invoke_with_missing(&first, &controls(None), Some("run2.log"))?;
    verify_output(
        output,
        1,
        &[
            "Run 1: `nonzero_with_observed_test_failures`",
            "Run 2: `log_missing`",
            "No verdict: see the evidence failure above.",
            "observed 15 pass, 1 fail",
            "Run 1: Error: alpha reason",
        ],
        &["masked_unknown (", "unstable (", "repeated_failure ("],
    )
}

#[test]
fn a_zero_status_cannot_hide_a_consistent_failed_completion() -> Result<(), String> {
    let failure = controls(None) + &alpha("FAILED");
    verify_output(
        invoke_artifacts(&failure, &controls(None), None, Some([0, 0]))?,
        1,
        &[
            "Run 1: `incomplete_evidence` (cargo exit 0)",
            "zero cargo exit status contradicts observed test failures",
            "No verdict: see the evidence failure above.",
            "observed 15 pass, 1 fail",
            "Run 1: Error: alpha reason",
        ],
        &["masked_unknown (", "unstable (", "repeated_failure ("],
    )
}

#[test]
fn a_nonzero_status_after_passing_targets_retains_the_infrastructure_distinction()
-> Result<(), String> {
    let failure = controls(None) + &alpha("FAILED");
    verify_output(
        invoke_artifacts(&failure, &controls(None), None, Some([101, 101]))?,
        0,
        &[
            "Run 2: `compile_or_harness_failure` (cargo exit 101)",
            "Infrastructure failure",
            "masked_unknown (1)",
        ],
        &[
            "Evidence failure",
            "incomplete_evidence",
            "repeated_failure (",
        ],
    )
}

#[test]
fn unusable_observed_failures_keep_diagnostics_without_any_verdict() -> Result<(), String> {
    let failure = controls(None) + &alpha("FAILED");
    for missing in ["run1.status", "run2.status"] {
        verify_output(
            invoke_with_missing(&failure, &failure, Some(missing))?,
            1,
            &[
                "status_missing",
                "No verdict: see the evidence failure above.",
                "Run 1: Error: alpha reason",
                "Run 2: Error: alpha reason",
            ],
            &["masked_unknown (", "unstable (", "repeated_failure ("],
        )?;
    }
    let incomplete = replace_first_completion(&failure, "test result: incomplete")?;
    for (first, second) in [
        (&incomplete, &failure),
        (&failure, &incomplete),
        (&incomplete, &incomplete),
    ] {
        verify(
            first.clone(),
            second.clone(),
            1,
            &[
                "incomplete_evidence",
                "No verdict: see the evidence failure above.",
                "Run 1: Error: alpha reason",
                "Run 2: Error: alpha reason",
            ],
            &["masked_unknown (", "unstable (", "repeated_failure ("],
        )?;
    }
    Ok(())
}

#[test]
fn completion_requires_matching_each_outcome_and_an_announced_owner() -> Result<(), String> {
    let clean = controls(None);
    let valid = "test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s";
    for summary in [
        valid.replace("ok.", "FAILED."),
        valid.replace("2 passed; 0 failed", "1 passed; 1 failed"),
        valid.replace(
            "2 passed; 0 failed; 0 ignored",
            "1 passed; 0 failed; 1 ignored",
        ),
        valid.replace("2 passed", "1 passed"),
        valid.replace("0 measured", "1 measured"),
        valid.replace("2 passed", "many passed"),
        valid.replace("2 passed", "9999999999999999999999999999999999999 passed"),
        valid.replace("0 ignored; ", ""),
        valid.replace("0 filtered out", "-1 filtered out"),
        valid.replace("0.01s", "NaNs"),
        valid.replace("0.01s", "-1s"),
        valid.replace("0.01s", "later"),
        format!("{valid} trailing evidence"),
    ] {
        verify(
            replace_first_completion(&clean, &summary)?,
            clean.clone(),
            1,
            &["incomplete_evidence", "provenance:"],
            &["No test failed in either run."],
        )?;
    }
    let unannounced = clean.replacen("running 2 tests\n", "", 1);
    verify(
        unannounced,
        clean,
        1,
        &[
            "incomplete_evidence",
            "completion summary without an announced owning harness",
        ],
        &["No test failed in either run."],
    )
}

#[test]
fn completion_preserves_ignored_filtered_empty_and_doctest_batches() -> Result<(), String> {
    let log = controls(None)
        + "Running tests/mixed.rs (target/debug/deps/mixed-1111111111111111.exe)\nrunning 3 tests\ntest success ... ok\ntest deferred ... ignored, requires a device\ntest other ... ignored\ntest result: ok. 1 passed; 0 failed; 2 ignored; 0 measured; 12 filtered out; finished in 0.00s\n"
        + "Running tests/empty.rs (target/debug/deps/empty-1111111111111111.exe)\nrunning 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n"
        + "Doc-tests ripr\nrunning 1 test\ntest src/lib.rs - example (line 1) ... ignored\ntest result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s\nrunning 1 test\ntest src/lib.rs - example (line 5) ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\nall doctests ran in 0.01s; merged doctests compilation took 0.01s\n";
    verify(
        log.clone(),
        log,
        0,
        &["completed_clean", "observed 17 pass, 0 fail"],
        &["Evidence failure", "incomplete_evidence"],
    )
}

#[test]
fn incomplete_or_duplicate_ignored_subjects_cannot_supply_completion() -> Result<(), String> {
    let clean = controls(None);
    let header = "Running tests/extra.rs (target/debug/deps/extra-1111111111111111.exe)\n";
    for tail in [
        header.to_string(),
        format!(
            "{header}running 0 tests\ntest unknown ... unsupported\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n"
        ),
        format!(
            "{header}running 2 tests\ntest duplicate ... ignored\ntest duplicate ... ignored\ntest result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s\n"
        ),
        format!(
            "{header}running 2 tests\ntest duplicate ... ignored\ntest duplicate ... ok\ntest result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s\n"
        ),
    ] {
        verify(
            clean.clone() + &tail,
            clean.clone(),
            1,
            &["incomplete_evidence", "provenance:"],
            &["No test failed in either run."],
        )?;
    }
    Ok(())
}

#[test]
fn rows_before_the_announcement_cannot_escape_completion_totals() -> Result<(), String> {
    let clean = controls(None);
    for result in ["ok", "FAILED", "ignored"] {
        let log = clean.clone()
            + &format!(
                "Running tests/extra.rs (target/debug/deps/extra-1111111111111111.exe)\ntest unannounced ... {result}\nrunning 1 test\ntest announced ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n"
            );
        verify(
            log,
            clean.clone(),
            1,
            &[
                "incomplete_evidence",
                "before its owning harness announcement",
            ],
            &["No test failed in either run.", "masked_unknown ("],
        )?;
    }
    Ok(())
}

#[test]
fn native_long_running_progress_is_not_an_extra_outcome() -> Result<(), String> {
    // Retained native logs contain this libtest notice before the final row.
    let progress = alpha("ok").replace(
        "test same_name ... ok",
        "test same_name has been running for over 60 seconds\ntest same_name ... ok",
    );
    let log = controls(None) + &progress;
    verify(
        log.clone(),
        log,
        0,
        &["completed_clean", "observed 16 pass, 0 fail"],
        &["incomplete_evidence", "provenance:"],
    )
}

#[test]
fn identical_names_in_distinct_targets_count_twice() -> Result<(), String> {
    let log = controls(None) + &alpha("ok") + &beta("ok");
    verify(log.clone(), log, 0, &["observed 17 pass, 0 fail"], &[])
}
#[test]
fn a_different_target_pass_cannot_mask_absence() -> Result<(), String> {
    verify(
        controls(None) + &alpha("FAILED"),
        controls(None) + &beta("ok"),
        0,
        &["masked_unknown (1)"],
        &["unstable (", "repeated_failure ("],
    )
}
#[test]
fn different_targets_cannot_invent_a_repeated_failure() -> Result<(), String> {
    verify(
        controls(None) + &alpha("FAILED"),
        controls(None) + &beta("FAILED"),
        0,
        &["masked_unknown (2)"],
        &["repeated_failure ("],
    )
}
#[test]
fn swapped_target_outcomes_are_two_unstable_subjects() -> Result<(), String> {
    verify(
        controls(None) + &alpha("FAILED") + &beta("ok"),
        controls(None) + &alpha("ok") + &beta("FAILED"),
        0,
        &["unstable (2)"],
        &["repeated_failure ("],
    )
}
#[test]
fn both_failure_reasons_keep_their_target_and_count() -> Result<(), String> {
    let log = controls(None) + &alpha("FAILED") + &beta("FAILED");
    verify(
        log.clone(),
        log,
        0,
        &[
            "observed 15 pass, 2 fail",
            "repeated_failure (2)",
            "- `tests/alpha.rs (alpha-1111111111111111) :: same_name`\n  - Run 1: Error: alpha reason\n  - Run 2: Error: alpha reason",
            "- `tests/beta.rs (beta-1111111111111111) :: same_name`\n  - Run 1: Error: beta reason\n  - Run 2: Error: beta reason",
        ],
        &[],
    )
}
#[test]
fn the_same_artifact_failure_is_repeated() -> Result<(), String> {
    let log = controls(None) + &alpha("FAILED");
    verify(
        log.clone(),
        log,
        0,
        &["repeated_failure (1)"],
        &["masked_unknown (", "unstable ("],
    )
}
#[test]
fn genuine_absence_remains_unknown() -> Result<(), String> {
    verify(
        controls(None) + &alpha("FAILED"),
        controls(None) + &target("tests/beta.rs", "beta", &[("different_name", "ok", "")]),
        0,
        &["masked_unknown (1)"],
        &["unstable ("],
    )
}
#[test]
fn changed_artifact_hash_cannot_borrow_a_pass() -> Result<(), String> {
    verify(
        controls(None) + &alpha("FAILED"),
        controls(None) + &alpha("ok").replace(HASH, "2222222222222222"),
        0,
        &["masked_unknown (1)"],
        &["unstable ("],
    )
}
#[test]
fn artifact_paths_and_windows_suffix_are_presentation() -> Result<(), String> {
    let first = controls(None) + &alpha("FAILED");
    let second = controls(None)
        + &alpha("ok")
            .replace("target/debug/deps/", "elsewhere/")
            .replace(".exe)", ")");
    verify(first, second, 0, &["unstable (1)"], &["masked_unknown ("])
}
#[test]
fn ansi_and_windows_separators_keep_the_same_artifact() -> Result<(), String> {
    let first = controls(None) + &alpha("FAILED");
    let second = (controls(None) + &alpha("ok"))
        .replace('/', "\\")
        .replace("Running ", "\u{1b}[92mRunning\u{1b}[0m ");
    verify(first, second, 0, &["unstable (1)"], &["masked_unknown ("])
}
#[test]
fn same_source_different_executables_remain_distinct() -> Result<(), String> {
    let first = controls(None)
        + &target(
            "unittests src/main.rs",
            "ripr",
            &[(NAME, "FAILED", "ripr main")],
        );
    let second = controls_adjusted(
        None,
        Some(("unittests src/main.rs", "xtask", (NAME, "ok", ""))),
        None,
    );
    verify(first, second, 0, &["masked_unknown (1)"], &["unstable ("])
}
#[test]
fn missing_release_control_is_refused() -> Result<(), String> {
    let log = controls(Some(CONTROLS[0].2));
    verify(
        log.clone(),
        log,
        1,
        &["not_observed", "could not produce trustworthy evidence"],
        &[],
    )
}
#[test]
fn wrong_target_release_control_cannot_supply_evidence() -> Result<(), String> {
    let name = CONTROLS[0].2;
    let log =
        controls(Some(name)) + &target("tests/unrelated.rs", "unrelated", &[(name, "ok", "")]);
    verify(log.clone(), log, 1, &["not_observed"], &[])
}
#[test]
fn duplicate_owner_artifacts_are_ambiguous() -> Result<(), String> {
    let name = CONTROLS[0].2;
    let log = controls(None)
        + &target("unittests src/lib.rs", "ripr", &[(name, "ok", "")])
            .replace(HASH, "2222222222222222");
    verify(log.clone(), log, 1, &["not_observed", "ambiguous"], &[])
}
#[test]
fn owner_hash_drift_across_the_pair_is_ambiguous() -> Result<(), String> {
    verify(
        controls(None),
        controls(None).replace("ripr-1111111111111111", "ripr-2222222222222222"),
        1,
        &["not_observed", "ambiguous"],
        &[],
    )
}
#[test]
fn unowned_rows_and_malformed_transitions_refuse_evidence() -> Result<(), String> {
    let name = CONTROLS[0].2;
    for header in [
        "",
        "Running custom command\n",
        "Running unittests src/lib.rs (target/debug/deps/ripr-not-a-hash.exe)\n",
    ] {
        let log = format!("{header}test {name} ... ok\n{}", controls(Some(name)));
        verify(
            log.clone(),
            log,
            1,
            &["incomplete_evidence", "provenance:"],
            &[],
        )?;
    }
    Ok(())
}
#[test]
fn malformed_transition_never_reuses_the_previous_owner() -> Result<(), String> {
    let name = CONTROLS[0].2;
    let log = controls(Some(name)) + &format!("Running malformed\ntest {name} ... ok\n");
    verify(
        log.clone(),
        log,
        1,
        &["incomplete_evidence", "unrecognized target header"],
        &[],
    )
}
#[test]
fn doctest_transition_cannot_borrow_a_unit_pass() -> Result<(), String> {
    let name = "src/lib.rs - example (line 1)";
    let first = controls(None)
        + &format!(
            "Doc-tests ripr\nrunning 1 test\ntest {name} ... FAILED\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n"
        );
    let second = controls_adjusted(
        None,
        Some(("unittests src/lib.rs", "ripr", (name, "ok", ""))),
        None,
    );
    verify(
        first,
        second,
        0,
        &["masked_unknown (1)", "Doc-tests ripr"],
        &["unstable ("],
    )
}
#[test]
fn explicit_doctest_transitions_preserve_names_with_spaces() -> Result<(), String> {
    let first = controls(None)
        + "Doc-tests ripr\nrunning 1 test\ntest src/lib.rs - example (line 1) ... FAILED\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n";
    let second = controls(None)
        + "Doc-tests ripr\nrunning 1 test\ntest src/lib.rs - example (line 1) ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n";
    verify(
        first,
        second,
        0,
        &[
            "unstable (1)",
            "Doc-tests ripr :: src/lib.rs - example (line 1)",
        ],
        &[],
    )
}
#[test]
fn duplicate_doctest_transitions_are_unproven() -> Result<(), String> {
    let doc = "Doc-tests ripr\nrunning 1 test\ntest src/lib.rs - example (line 1) ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n";
    let log = controls(None) + doc + doc;
    verify(
        log.clone(),
        log,
        1,
        &[
            "incomplete_evidence",
            "repeated target header: Doc-tests ripr",
        ],
        &[],
    )
}
#[test]
fn a_failing_owning_control_stays_advisory() -> Result<(), String> {
    let log = controls_adjusted(None, None, Some(CONTROLS[0].2));
    verify(
        log.clone(),
        log,
        0,
        &["repeated_failure (1)", "FAILED | FAILED"],
        &["not_observed"],
    )
}

#[test]
fn missing_log_or_status_never_becomes_clean_evidence() -> Result<(), String> {
    let log = controls(None);
    for name in ["run1.log", "run2.log", "run1.status", "run2.status"] {
        let output = invoke_with_missing(&log, &log, Some(name))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if output.status.code() != Some(1)
            || !stdout.contains("**Evidence failure.**")
            || stdout.contains("No test failed in either run.")
        {
            return Err(format!(
                "missing {name}: unexpected status {:?}: {stdout}",
                output.status.code()
            ));
        }
    }
    Ok(())
}

#[test]
fn completed_owner_cannot_lend_identity_to_an_orphan_tail() -> Result<(), String> {
    let required = CONTROLS[5].2;
    let log = controls_adjusted(
        Some(required),
        Some((
            "unittests src/main.rs",
            "xtask",
            ("unrelated_xtask_test", "ok", ""),
        )),
        None,
    ) + &format!(
        "running 1 test\ntest {required} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n"
    );
    verify(
        log.clone(),
        log,
        1,
        &["incomplete_evidence", "without an admitted target"],
        &[],
    )
}

#[test]
fn native_empty_child_summary_preserves_the_parent_harness() -> Result<(), String> {
    let required = CONTROLS[5].2;
    let log = controls(None).replace(&format!("test {required} ... ok"), &format!("running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 2528 filtered out; finished in 0.00s\ntest {required} ... ok"));
    verify(
        log.clone(),
        log,
        0,
        &["observed 15 pass, 0 fail"],
        &["incomplete_evidence", "not_observed"],
    )
}

#[test]
fn nested_child_cannot_supply_an_owning_release_control() -> Result<(), String> {
    let required = CONTROLS[5].2;
    let log = controls_adjusted(Some(required), Some(("unittests src/main.rs", "xtask", ("unrelated_xtask_test", "ok", ""))), None)
        .replace("test unrelated_xtask_test ... ok", &format!("running 1 test\ntest {required} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\ntest unrelated_xtask_test ... ok"));
    verify(
        log.clone(),
        log,
        1,
        &["incomplete_evidence", "unowned nested harness"],
        &[],
    )
}

#[test]
fn ordinary_captured_progress_keeps_its_real_failure_reason() -> Result<(), String> {
    let log = controls(None)
        + &alpha("FAILED").replace(
            "Error: alpha reason",
            "Running cleanup for fixture\nDoc-tests are checked separately\nError: alpha reason",
        );
    verify(
        log.clone(),
        log,
        0,
        &[
            "repeated_failure (1)",
            "- `tests/alpha.rs (alpha-1111111111111111) :: same_name`\n  - Run 1: Error: alpha reason\n  - Run 2: Error: alpha reason",
        ],
        &["incomplete_evidence"],
    )
}

#[test]
fn a_captured_header_cannot_authenticate_another_target() -> Result<(), String> {
    let log = controls(None) + &alpha("FAILED").replace("Error: alpha reason", "Running tests/echoed.rs (target/debug/deps/echoed-1111111111111111.exe)\ntest echoed_name ... ok\nError: alpha reason");
    verify(
        log.clone(),
        log,
        1,
        &[
            "incomplete_evidence",
            "target header inside an unterminated failure block",
        ],
        &[],
    )
}

#[test]
fn doctest_end_marker_cannot_hide_missing_rows_and_summary() -> Result<(), String> {
    let log = controls(None)
        + "Doc-tests ripr\nrunning 2 tests\ntest src/lib.rs - example (line 1) ... ok\nall doctests ran in 0.01s; merged doctests compilation took 0.01s\n";
    verify(
        log.clone(),
        log,
        1,
        &[
            "incomplete_evidence",
            "doctest completion without completed owning batches",
        ],
        &[],
    )
}

#[test]
fn corpus_roots_are_distinct_with_a_fixed_clock_in_parallel() -> Result<(), String> {
    let roots = std::thread::scope(|scope| {
        let handles = (0..16)
            .map(|_| scope.spawn(|| corpus_root(7)))
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|panic| format!("root worker failed: {panic:?}"))
            })
            .collect::<Result<Vec<_>, _>>()
    })?;
    let distinct = roots.iter().collect::<std::collections::BTreeSet<_>>();
    let first = corpus_root(7);
    let second = corpus_root(7);
    if distinct.len() != 16 || first == second {
        return Err(
            "a fixed clock must still produce distinct serial and concurrent roots".to_string(),
        );
    }
    Ok(())
}

#[test]
fn temp_root_refuses_existing_directories_files_and_failed_creation()
-> Result<(), Box<dyn std::error::Error>> {
    // The outer fixture owns only a directory that this call created exclusively.
    let path = corpus_root(11);
    fs::create_dir(&path)?;
    let parent = TempRoot(path);
    let directory = parent.0.join("existing");
    fs::create_dir(&directory)?;
    let sentinel = directory.join("sentinel");
    fs::write(&sentinel, "belongs to the previous owner")?;
    let collision = expect_fixture_refusal(
        TempRoot::create(directory.clone()),
        "an existing directory must never grant ownership",
    )?;
    assert_eq!(collision.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(
        fs::read_to_string(&sentinel)?,
        "belongs to the previous owner"
    );

    let file = parent.0.join("existing-file");
    fs::write(&file, "preserve this file")?;
    expect_fixture_refusal(TempRoot::create(file.clone()), "existing file")?;
    expect_fixture_refusal(TempRoot::create(file.join("child")), "file as parent")?;
    assert_eq!(fs::read_to_string(&file)?, "preserve this file");
    let missing_parent = parent.0.join("missing");
    expect_fixture_refusal(
        TempRoot::create(missing_parent.join("child")),
        "missing parent",
    )?;
    assert!(
        !missing_parent.exists(),
        "creation must not claim missing ancestors"
    );
    assert_eq!(
        fs::read_to_string(sentinel)?,
        "belongs to the previous owner"
    );
    Ok(())
}

#[test]
fn temp_root_concurrent_collision_grants_exactly_one_owner()
-> Result<(), Box<dyn std::error::Error>> {
    let path = corpus_root(12);
    fs::create_dir(&path)?;
    let parent = TempRoot(path);
    let child = parent.0.join("contended");
    let barrier = std::sync::Barrier::new(2);
    let attempts = std::thread::scope(|scope| {
        let handles = (0..2)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    TempRoot::create(child.clone())
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|panic| format!("creation worker failed: {panic:?}"))
            })
            .collect::<Result<Vec<_>, _>>()
    })?;
    assert_eq!(attempts.iter().filter(|attempt| attempt.is_ok()).count(), 1);
    assert_eq!(
        attempts
            .iter()
            .filter(|attempt| attempt
                .as_ref()
                .is_err_and(|error| error.kind() == std::io::ErrorKind::AlreadyExists))
            .count(),
        1
    );
    let sentinel = child.join("owned");
    fs::write(&sentinel, "winner remains live")?;
    assert_eq!(fs::read_to_string(&sentinel)?, "winner remains live");
    drop(attempts);
    assert!(
        !child.exists(),
        "the winning guard must clean its own directory"
    );
    Ok(())
}

#[test]
fn temp_root_cleans_owned_directories_on_success_and_later_error()
-> Result<(), Box<dyn std::error::Error>> {
    let path = corpus_root(13);
    {
        let root = TempRoot::create(path.clone())?;
        fs::write(root.0.join("owned"), "cleanup control")?;
    }
    assert!(!path.exists());
    let failing_path = corpus_root(14);
    let failed: std::io::Result<()> = (|| {
        let root = TempRoot::create(failing_path.clone())?;
        let file = root.0.join("file");
        fs::write(&file, "owned before failure")?;
        fs::create_dir(file.join("child"))?;
        Ok(())
    })();
    expect_fixture_refusal(failed, "intentional failure after acquiring the root")?;
    assert!(!failing_path.exists());
    Ok(())
}

fn expect_fixture_refusal<T>(
    result: std::io::Result<T>,
    context: &str,
) -> Result<std::io::Error, String> {
    match result {
        Err(error) => Ok(error),
        Ok(value) => {
            drop(value);
            Err(format!(
                "{context}: fixture operation unexpectedly succeeded"
            ))
        }
    }
}

fn evidence_report(log: &str) -> Result<String, String> {
    let output = invoke(log, log)?;
    if output.status.code() != Some(1) {
        return Err(format!(
            "expected evidence refusal, got {:?}",
            output.status.code()
        ));
    }
    let report = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
    if !report.contains("incomplete_evidence")
        || !report.contains("No verdict: see the evidence failure above.")
    {
        return Err("rendering must preserve incomplete evidence and refuse a verdict".to_string());
    }
    Ok(report)
}

#[test]
fn provenance_report_caps_legacy_rows_and_counts_omissions() -> Result<(), String> {
    let mut log = (0..1000)
        .map(|index| format!("test orphan_{index} ... ok\n"))
        .collect::<String>();
    log.push_str("test result: ok. 1000 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n");
    let report = evidence_report(&log)?;
    assert!(report.contains("### Provenance errors\n"));
    for label in ["Run 1", "Run 2"] {
        assert_eq!(
            report.matches(&format!("- {label} provenance:")).count(),
            20
        );
        assert!(report.contains(&format!(
            "- {label}: 982 additional provenance errors omitted (1002 total)."
        )));
    }
    assert!(!report.contains("orphan_20"));
    assert_eq!(
        report
            .matches("observed 0 pass, 0 fail across 1 reported result line(s)")
            .count(),
        2
    );
    assert!(
        report.len() < 20_000,
        "legacy rows must not bury the verdict"
    );
    Ok(())
}

#[test]
fn provenance_report_caps_malformed_headers_and_raw_presentation() -> Result<(), String> {
    let log = controls(None)
        + &(0..1000)
            .map(|index| format!("Running malformed_{index}\n"))
            .collect::<String>();
    let report = evidence_report(&log)?;
    for label in ["Run 1", "Run 2"] {
        assert_eq!(
            report.matches(&format!("- {label} provenance:")).count(),
            20
        );
        assert!(report.contains(&format!(
            "- {label}: 980 additional provenance errors omitted (1000 total)."
        )));
    }
    assert_eq!(report.matches("  - Header text:").count(), 40);
    assert_eq!(
        report
            .matches("984 additional raw headers omitted (1004 total).")
            .count(),
        2
    );
    assert!(!report.contains("malformed_20"));
    assert_eq!(
        report
            .matches("observed 15 pass, 0 fail across 4 reported result line(s)")
            .count(),
        2
    );
    assert!(
        report.len() < 20_000,
        "raw header echoes must not bypass the display bound"
    );
    Ok(())
}

#[test]
fn provenance_report_bounds_unicode_without_losing_totals() -> Result<(), String> {
    let header = format!("Running {}RAW_TAIL", "🦀界é`".repeat(400));
    let log = controls(None) + &header + "\n";
    let report = evidence_report(&log)?;
    let error = format!("unrecognized target header: {header}");
    let expected_error = error
        .chars()
        .take(240)
        .collect::<String>()
        .replace('`', "'");
    let expected_header = header
        .chars()
        .take(240)
        .collect::<String>()
        .replace('`', "'");
    for label in ["Run 1", "Run 2"] {
        assert!(report.contains(&format!(
            "- {label} provenance: {expected_error}… [truncated]\n"
        )));
    }
    assert_eq!(
        report
            .matches(&format!(
                "  - Header text: `{expected_header}… [truncated]`\n"
            ))
            .count(),
        2
    );
    assert!(!report.contains("RAW_TAIL"));
    assert!(!report.contains('\u{fffd}'));
    assert_eq!(
        report
            .matches("observed 15 pass, 0 fail across 4 reported result line(s)")
            .count(),
        2
    );
    assert!(!report.contains("additional provenance errors omitted"));
    Ok(())
}

#[test]
fn provenance_report_caps_repeated_admitted_targets() -> Result<(), String> {
    let header = format!("Running tests/alpha.rs (target/debug/deps/alpha-{HASH}.exe)\n");
    let log = controls(None) + &header.repeat(1000);
    let report = evidence_report(&log)?;
    let targets = report
        .split("### Targets reached\n")
        .nth(1)
        .and_then(|rest| rest.split("### Totals\n").next())
        .ok_or_else(|| "missing target section".to_string())?;
    assert_eq!(
        targets
            .matches(&format!("tests/alpha.rs (alpha-{HASH})"))
            .count(),
        32
    );
    assert_eq!(
        targets
            .matches("984 additional targets omitted (1004 total).")
            .count(),
        2
    );
    assert_eq!(
        targets
            .matches("984 additional raw headers omitted (1004 total).")
            .count(),
        2
    );
    assert_eq!(
        report
            .matches("1979 additional provenance errors omitted (1999 total).")
            .count(),
        2
    );
    assert_eq!(
        report
            .matches("observed 15 pass, 0 fail across 4 reported result line(s)")
            .count(),
        2
    );
    assert!(
        report.len() < 25_000,
        "admitted target echoes must also be bounded"
    );
    Ok(())
}

#[test]
fn provenance_report_bounds_admitted_unicode_subjects_on_every_surface() -> Result<(), String> {
    let source = format!("tests/{}OWNED_TAIL.rs", "🦀界é".repeat(400));
    let log = controls(None)
        + &target(
            &source,
            "unicode",
            &[(NAME, "FAILED", "owned Unicode reason")],
        );
    let output = invoke(&log, &log)?;
    assert_eq!(
        output.status.code(),
        Some(0),
        "an owned test failure remains advisory"
    );
    let report = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
    assert!(
        !report.contains("OWNED_TAIL"),
        "an admitted identity bypassed the excerpt bound"
    );
    assert!(!report.contains("incomplete_evidence"));
    assert!(report.contains("**repeated_failure (1)**"));
    assert_eq!(
        report
            .matches("observed 15 pass, 1 fail across 5 reported result line(s)")
            .count(),
        2
    );
    assert!(
        report
            .contains("Run 1: Error: owned Unicode reason\n  - Run 2: Error: owned Unicode reason")
    );
    let identity = format!("{source} (unicode-{HASH}) :: {NAME}");
    let expected = identity.chars().take(240).collect::<String>() + "… [truncated]";
    assert_eq!(
        report.matches(&format!("- `{expected}`\n")).count(),
        2,
        "verdict and reason headings must both use bounded presentation"
    );
    assert!(report.len() < 20_000);
    Ok(())
}

#[test]
fn provenance_report_caps_distinct_failure_subjects_without_merging_excerpt_collisions()
-> Result<(), String> {
    for label in ["repeated_failure", "unstable", "masked_unknown"] {
        let mut first = controls(None);
        let mut second = controls(None);
        for index in 0..26 {
            let source = format!("tests/{}_{index:02}.rs", "界".repeat(300));
            let reason = format!("reason_{index:02}");
            first.push_str(&target(&source, "long_names", &[(NAME, "FAILED", &reason)]));
            if label != "masked_unknown" {
                second.push_str(&target(
                    &source,
                    "long_names",
                    &[(
                        NAME,
                        if label == "unstable" { "ok" } else { "FAILED" },
                        &reason,
                    )],
                ));
            }
        }
        let output = invoke(&first, &second)?;
        assert_eq!(output.status.code(), Some(0));
        let report = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
        assert!(report.contains(&format!("**{label} (26)**")));
        assert!(report.contains(&format!(
            "6 additional {label} subjects omitted (26 total)."
        )));
        assert!(report.contains("6 additional failed subjects omitted (26 total)."));
        let excerpt = format!("tests/{}", "界".repeat(300))
            .chars()
            .take(240)
            .collect::<String>()
            + "… [truncated]";
        assert_eq!(
            report.matches(&format!("- `{excerpt}`\n")).count(),
            40,
            "each section must keep 20 rows even when their excerpts coincide"
        );
        for index in 0..20 {
            assert!(report.contains(&format!("Run 1: Error: reason_{index:02}\n")));
            if label == "repeated_failure" {
                assert!(report.contains(&format!("Run 2: Error: reason_{index:02}\n")));
            }
        }
        assert!(!report.contains("reason_20"));
        assert!(
            report.contains("Run 1: observed 15 pass, 26 fail across 30 reported result line(s)")
        );
        assert!(!report.contains("incomplete_evidence"));
    }
    Ok(())
}
