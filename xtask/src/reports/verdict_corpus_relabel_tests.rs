use super::*;
use crate::reports::verdict_corpus::load_corpus;

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
}

fn failing(names: &[&str]) -> RunOutcome {
    RunOutcome::TestsFailed {
        failing_tests: names.iter().map(|name| name.to_string()).collect(),
    }
}

#[test]
fn parse_args_defaults_and_rejects_conflicts() -> Result<(), String> {
    let parsed = parse_args(&[])?;
    assert_eq!(parsed.sample, None);
    assert_eq!(parsed.repeat, DEFAULT_REPEAT);
    assert_eq!(parsed.seed, DEFAULT_SEED);
    let parsed = parse_args(&strings(&[
        "--sample",
        "5",
        "--seed",
        "2026-10-04",
        "--repeat",
        "3",
    ]))?;
    assert_eq!(parsed.sample, Some(5));
    assert_eq!(parsed.seed, "2026-10-04");
    assert_eq!(parsed.repeat, 3);
    for bad in [
        &["--sample", "0"][..],
        &["--repeat", "x"],
        &["--sample", "2", "--case", "a"],
        &["--sample"],
        &["--bogus"],
    ] {
        if parse_args(&strings(bad)).is_ok() {
            return Err(format!("{bad:?} should be refused"));
        }
    }
    Ok(())
}

#[test]
fn sample_order_is_deterministic_per_seed_and_rotates_across_seeds() -> Result<(), String> {
    let corpus = load_corpus(&crate::dogfood::repo_rooted_fixture_path(CORPUS_DIR))?;
    let cases: Vec<&Case> = corpus.cases.iter().collect();
    let ids = |order: Vec<&Case>| -> Vec<String> {
        order
            .into_iter()
            .take(8)
            .map(|c| c.case_id.clone())
            .collect()
    };
    let first = ids(sample_order(&cases, "a"));
    assert_eq!(first, ids(sample_order(&cases, "a")));
    assert_ne!(first, ids(sample_order(&cases, "b")));
    // Input order does not leak into the sample.
    let mut reversed = cases.clone();
    reversed.reverse();
    assert_eq!(first, ids(sample_order(&reversed, "a")));
    Ok(())
}

#[test]
fn test_command_args_accepts_only_plain_cargo_test() -> Result<(), String> {
    assert_eq!(
        test_command_args("cargo test -p serde_core --lib")?,
        strings(&["test", "-p", "serde_core", "--lib"])
    );
    assert_eq!(test_command_args("cargo test")?, strings(&["test"]));
    assert!(test_command_args("cargo build").err().is_some());
    assert!(test_command_args("make test").err().is_some());
    assert!(test_command_args("cargo test; rm -rf /").err().is_some());
    assert!(test_command_args("cargo test | tee log").err().is_some());
    // Commands that exit 0 without running a test cannot carry a label.
    assert!(test_command_args("cargo test --no-run").err().is_some());
    assert!(test_command_args("cargo test -- --list").err().is_some());
    assert!(
        test_command_args("cargo test -- --format terse")
            .err()
            .is_some()
    );
    assert!(
        test_command_args("cargo test -- --format=json")
            .err()
            .is_some()
    );
    assert!(test_command_args("cargo test -- -q").err().is_some());
    for escape in [
        "cargo test --manifest-path /elsewhere/Cargo.toml",
        "cargo test --target-dir=/tmp/x",
        "cargo test --config build.rustc-wrapper=w",
        "cargo test -Zunstable-options",
        "cargo test -C /elsewhere",
        "cargo test -vZunstable-options",
    ] {
        assert!(test_command_args(escape).err().is_some(), "{escape}");
    }
    // Arguments after `--` go to the test binary, not cargo.
    assert_eq!(
        test_command_args("cargo test -- --skip a -Z")?,
        strings(&["test", "--", "--skip", "a", "-Z"])
    );
    Ok(())
}

#[test]
fn classify_run_separates_test_failures_from_build_failures() {
    let failed = "running 2 tests\ntest tests::a ... ok\ntest tests::b ... FAILED\n\ntest result: FAILED. 1 passed; 1 failed\n";
    assert_eq!(
        classify_run(false, false, failed, ""),
        failing(&["tests::b"])
    );
    assert_eq!(
        classify_run(true, false, "test result: ok. 2 passed; 0 failed", ""),
        RunOutcome::TestsPassed
    );
    // A compile error prints no test lines at all.
    assert_eq!(
        classify_run(
            false,
            false,
            "",
            "   Compiling x\nerror[E0425]: cannot find value `y`\n"
        ),
        RunOutcome::BuildFailed {
            error: "error[E0425]: cannot find value `y`".to_string()
        }
    );
    // A doctest or harness failure with a result line but no named test is
    // still a test failure, not a build failure.
    assert_eq!(
        classify_run(false, false, "test result: FAILED. 0 passed; 1 failed", ""),
        failing(&[])
    );
    assert_eq!(classify_run(false, true, failed, ""), RunOutcome::TimedOut);
    // A success that executed nothing (a filter matching no test) is not a pass.
    assert_eq!(
        classify_run(
            true,
            false,
            "running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 12 filtered out\n",
            ""
        ),
        RunOutcome::NoTestsRan
    );
    assert_eq!(
        classify_run(
            true,
            false,
            "test result: ok. 0 passed; 0 failed; 0 ignored\ntest result: ok. 3 passed; 0 failed; 1 ignored\n",
            ""
        ),
        RunOutcome::TestsPassed
    );
    let should_panic = "running 1 test\ntest checks::tests::over - should panic ... FAILED\n\ntest result: FAILED.";
    assert_eq!(
        classify_run(false, false, should_panic, ""),
        failing(&["checks::tests::over"])
    );
    // A binary that aborts (stack overflow) started but named no failure.
    let aborted = "running 9 tests\n\nthread 'test_align' has overflowed its stack\n";
    assert_eq!(
        classify_run(false, false, aborted, "error: test failed"),
        failing(&[])
    );
    // Unit tests passed, then rustdoc failed before any doctest ran: the
    // earlier passing result does not make the failure a test failure.
    let passed_then_rustdoc =
        "running 2 tests\ntest a ... ok\ntest b ... ok\n\ntest result: ok. 2 passed; 0 failed\n";
    assert_eq!(
        classify_run(
            false,
            false,
            passed_then_rustdoc,
            "   Doc-tests x\nerror: unresolved import `x::gone`\n"
        ),
        RunOutcome::BuildFailed {
            error: "error: unresolved import `x::gone`".to_string()
        }
    );
    // A binary that exits nonzero after passing results is a test failure,
    // and so is a FAILED result line with no failed count.
    assert_eq!(
        classify_run(
            false,
            false,
            passed_then_rustdoc,
            "error: test failed, to rerun pass `--lib`\n"
        ),
        failing(&[])
    );
    assert_eq!(
        classify_run(
            false,
            false,
            "running 0 tests\ntest result: FAILED. 0 passed; 0 failed\n",
            ""
        ),
        failing(&[])
    );
    // A test's own `running ...` output is not a binary header, so it
    // cannot turn a rustdoc failure into a test failure.
    let printed = "running 1 test\nrunning the migration\ntest a ... ok\n\ntest result: ok. 1 passed; 0 failed\n";
    assert_eq!(
        classify_run(
            false,
            false,
            printed,
            "error: unresolved import `x::gone`\n"
        ),
        RunOutcome::BuildFailed {
            error: "error: unresolved import `x::gone`".to_string()
        }
    );
    // A doctest that fails to compile is a named failing test, as libtest
    // reports it.
    let doctest = "running 1 test\ntest src/lib.rs - f (line 3) ... FAILED\n\ntest result: FAILED. 0 passed; 1 failed\n";
    assert_eq!(
        classify_run(false, false, doctest, ""),
        failing(&["src/lib.rs - f (line 3)"])
    );
}

#[test]
fn names_failing_test_accepts_module_qualified_forms_only() {
    let observed: BTreeSet<String> = ["tests::tiers_change".to_string()].into();
    assert!(names_failing_test("tests::tiers_change", &observed));
    assert!(names_failing_test("tiers_change", &observed));
    assert!(!names_failing_test("change", &observed));
    assert!(!names_failing_test("tests::other", &observed));
    let short: BTreeSet<String> = ["ser::impls::test_format_u8".to_string()].into();
    assert!(names_failing_test(
        "serde_core::ser::impls::test_format_u8",
        &short
    ));
}

#[test]
fn apply_mutated_line_keeps_indent_and_line_numbers() -> Result<(), String> {
    let text = "fn f(n: u32) -> bool {\n    if n > 99 {\n        true\n    } else { false }\n}\n";
    assert_eq!(
        apply_mutated_line(text, 2, "if n > 100 {")?,
        "fn f(n: u32) -> bool {\n    if n > 100 {\n        true\n    } else { false }\n}\n"
    );
    let removed = apply_mutated_line(text, 3, "")?;
    assert_eq!(removed.lines().count(), text.lines().count());
    assert_eq!(removed.lines().nth(2), Some(""));
    let crlf = "a\r\n  b\r\nc";
    assert_eq!(apply_mutated_line(crlf, 2, "d")?, "a\r\n  d\r\nc");
    assert_eq!(apply_mutated_line("a\r\n\r\nc", 2, "x")?, "a\r\nx\r\nc");
    assert!(apply_mutated_line(text, 0, "x").err().is_some());
    assert!(apply_mutated_line(text, 99, "x").err().is_some());
    Ok(())
}

#[test]
fn mutant_drift_names_each_way_a_label_can_be_wrong() {
    let id = "c";
    // Agreement is silent.
    assert!(
        mutant_drift(
            id,
            "m",
            MutantOutcome::TestsFailed,
            Some("tests::a"),
            &[failing(&["tests::a"]), failing(&["tests::a"])]
        )
        .is_empty()
    );
    assert!(
        mutant_drift(
            id,
            "m",
            MutantOutcome::TestsPassed,
            None,
            &[RunOutcome::TestsPassed]
        )
        .is_empty()
    );
    // A mutant the tests pass, labeled as failing them.
    let drift = mutant_drift(
        id,
        "m",
        MutantOutcome::TestsFailed,
        Some("tests::a"),
        &[RunOutcome::TestsPassed],
    );
    assert!(
        drift
            .iter()
            .any(|d| d.contains("labeled tests_failed but the tests pass"))
    );
    // A mutant the tests fail, labeled as passing them.
    let drift = mutant_drift(
        id,
        "m",
        MutantOutcome::TestsPassed,
        None,
        &[failing(&["tests::a"])],
    );
    assert!(
        drift
            .iter()
            .any(|d| d.contains("labeled tests_passed but the tests fail"))
    );
    // Failing, but in a different test than the label names.
    let drift = mutant_drift(
        id,
        "m",
        MutantOutcome::TestsFailed,
        Some("tests::a"),
        &[failing(&["tests::b"])],
    );
    assert!(drift.iter().any(|d| d.contains("did not fail")));
    // An aborted binary fails without naming a test; the outcome still counts.
    assert!(
        mutant_drift(
            id,
            "m",
            MutantOutcome::TestsFailed,
            Some("tests::a"),
            &[failing(&[])]
        )
        .is_empty()
    );
    // Does not compile: no runtime outcome.
    let drift = mutant_drift(
        id,
        "m",
        MutantOutcome::TestsFailed,
        Some("tests::a"),
        &[RunOutcome::BuildFailed {
            error: "error: x".to_string(),
        }],
    );
    assert!(drift.iter().any(|d| d.contains("does not compile")));
    // Flaky: repeated runs disagree, even when the first agrees.
    let drift = mutant_drift(
        id,
        "m",
        MutantOutcome::TestsFailed,
        Some("tests::a"),
        &[failing(&["tests::a"]), RunOutcome::TestsPassed],
    );
    assert!(drift.iter().any(|d| d.contains("repeated runs disagree")));
    // Failing both times, but in different tests, is still a disagreement.
    let drift = mutant_drift(
        id,
        "m",
        MutantOutcome::TestsFailed,
        Some("tests::a"),
        &[failing(&["tests::a"]), failing(&["tests::a", "tests::b"])],
    );
    assert!(drift.iter().any(|d| d.contains("repeated runs disagree")));
    let drift = mutant_drift(
        id,
        "m",
        MutantOutcome::TestsFailed,
        Some("tests::a"),
        &[RunOutcome::TimedOut],
    );
    assert!(drift.iter().any(|d| d.contains("timed out")));
}

#[test]
fn observed_truth_follows_the_corpus_rule() {
    let pass = RunOutcome::TestsPassed;
    let fail = failing(&["t"]);
    assert_eq!(
        observed_truth(&[&fail, &fail]),
        Some(TruthState::Discriminated)
    );
    assert_eq!(
        observed_truth(&[&pass, &pass]),
        Some(TruthState::NotDiscriminated)
    );
    assert_eq!(
        observed_truth(&[&fail, &pass]),
        Some(TruthState::PartiallyDiscriminated)
    );
    assert_eq!(
        observed_truth(&[
            &fail,
            &RunOutcome::BuildFailed {
                error: String::new()
            }
        ]),
        None
    );
}

#[test]
fn toolchain_release_ignores_the_host_triple() {
    assert_eq!(
        toolchain_release("rustc 1.95.0 (59807616e 2026-04-14), x86_64-unknown-linux-gnu"),
        toolchain_release("rustc 1.95.0 (59807616e 2026-04-14)\n")
    );
    assert_ne!(
        toolchain_release("rustc 1.95.0 (59807616e 2026-04-14)"),
        toolchain_release("rustc 1.96.0 (00000000a 2026-06-01)")
    );
}

#[test]
fn declares_workspace_reads_only_a_workspace_table() {
    assert!(declares_workspace(
        "[package]\nname = \"a\"\n\n[workspace]\n"
    ));
    assert!(declares_workspace("[workspace.package]\nversion = \"1\"\n"));
    assert!(!declares_workspace("[package]\nname = \"workspace\"\n"));
    assert!(!declares_workspace("[package]\nworkspace = \"..\"\n"));
}

#[test]
fn labeled_toolchain_names_the_rustup_release() {
    assert_eq!(
        labeled_toolchain("rustc 1.97.0 (2d8144b78 2026-07-07), x86_64-unknown-linux-gnu"),
        Some("1.97.0")
    );
    assert_eq!(
        labeled_toolchain("rustc 1.95.0 (59807616e 2026-04-14)"),
        Some("1.95.0")
    );
    assert_eq!(labeled_toolchain("nightly"), None);
    assert_eq!(labeled_toolchain("rustc nightly-2026 (x)"), None);
}

#[test]
fn link_stays_inside_refuses_links_that_leave_the_copy() {
    assert!(link_stays_inside(
        Path::new("crates/foo"),
        Path::new("../../README.md")
    ));
    assert!(link_stays_inside(Path::new(""), Path::new("./src/lib.rs")));
    assert!(!link_stays_inside(
        Path::new("crates/foo"),
        Path::new("../../../x")
    ));
    assert!(!link_stays_inside(Path::new(""), Path::new("../x")));
    assert!(!link_stays_inside(
        Path::new("src"),
        Path::new("/etc/passwd")
    ));
}

#[cfg(unix)]
#[test]
fn copy_checkout_refuses_a_chain_of_links_that_resolves_outside() -> Result<(), String> {
    use std::os::unix::fs::symlink;
    let base = std::env::temp_dir().join(format!("ripr-relabel-link-test-{}", std::process::id()));
    let checkout = base.join("checkout");
    let io = |err: std::io::Error| err.to_string();
    fs::create_dir_all(checkout.join("sub")).map_err(io)?;
    fs::write(checkout.join("lib.rs"), "").map_err(io)?;
    // Each link looks contained on its own: `sub/up` is the root, and
    // `esc` walks down and back up through it, ending above the root.
    symlink("..", checkout.join("sub/up")).map_err(io)?;
    symlink("sub/up/sub/up/..", checkout.join("esc")).map_err(io)?;
    assert!(link_stays_inside(
        Path::new(""),
        Path::new("sub/up/sub/up/..")
    ));
    let listed = |names: &[&str]| -> Vec<PathBuf> { names.iter().map(PathBuf::from).collect() };
    let escaped = copy_checkout(
        &checkout,
        &base.join("copy"),
        &listed(&["lib.rs", "sub/up", "esc"]),
    );
    fs::remove_file(checkout.join("esc")).map_err(io)?;
    // A dangling link cannot be shown to stay inside.
    symlink("sub/up/sub/up/../missing", checkout.join("dangling")).map_err(io)?;
    let dangling = copy_checkout(
        &checkout,
        &base.join("copy-dangling"),
        &listed(&["lib.rs", "dangling"]),
    );
    fs::remove_file(checkout.join("dangling")).map_err(io)?;
    // Only listed paths are copied; an unlisted file stays behind.
    fs::write(checkout.join("ignored.toml"), "").map_err(io)?;
    let contained = copy_checkout(
        &checkout,
        &base.join("copy-ok"),
        &listed(&["lib.rs", "sub/up"]),
    );
    let ignored_copied = base.join("copy-ok/ignored.toml").exists();
    // A link into an untracked directory would dangle in the copy.
    fs::create_dir_all(checkout.join("untracked")).map_err(io)?;
    symlink("untracked", checkout.join("into-untracked")).map_err(io)?;
    let into_untracked = copy_checkout(
        &checkout,
        &base.join("copy-untracked"),
        &listed(&["lib.rs", "into-untracked"]),
    );
    fs::remove_dir_all(&base).map_err(io)?;
    assert!(
        escaped
            .as_ref()
            .err()
            .is_some_and(|err| err.contains("links outside the checkout")),
        "{escaped:?}"
    );
    assert!(
        dangling
            .as_ref()
            .err()
            .is_some_and(|err| err.contains("does not resolve")),
        "{dangling:?}"
    );
    assert!(!ignored_copied, "an unlisted file reached the copy");
    assert!(
        into_untracked
            .as_ref()
            .err()
            .is_some_and(|err| err.contains("tracked files")),
        "{into_untracked:?}"
    );
    contained
}
