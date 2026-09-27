//! Drift guard for the public command hierarchy (#2930).
//!
//! These tests exercise the *rendered* help surfaces — the stdout of the built
//! binary — rather than the help source files. A substring check against
//! `src/cli/help/overview.rs` passes even when a route disappears from the
//! default screen, because that one file holds the default overview, the
//! exhaustive reference, and prose; only the rendered output is the public
//! contract. Rendered help is matched after whitespace normalization, so
//! column realignment and line reflow do not break the guard. Documentation
//! checks bind tasks to commands, first-run examples, and navigation rather
//! than requiring the README to repeat the help's explanatory sentences.

use std::process::Command;

const COMMAND_HIERARCHY_DOC: &str = include_str!("../../../docs/COMMAND_HIERARCHY.md");
const ROOT_README: &str = include_str!("../../../README.md");
const QUICKSTART_DOC: &str = include_str!("../../../docs/QUICKSTART.md");
const EXIT_CODES_DOC: &str = include_str!("../../../docs/EXIT_CODES.md");

fn rendered_help(args: &[&str]) -> Result<String, String> {
    let output = Command::new(env!("CARGO_BIN_EXE_ripr"))
        .args(args)
        .output()
        .map_err(|error| format!("failed to run ripr {args:?}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "ripr {args:?} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("ripr {args:?} emitted non-UTF-8 stdout: {error}"))
}

fn assert_contains(surface: &str, text: &str, needle: &str) -> Result<(), String> {
    if text.contains(needle) {
        return Ok(());
    }
    Err(format!("{surface} lost the canonical route `{needle}`"))
}

/// Collapse all whitespace runs so a needle survives line reflows and help
/// column realignment; the vocabulary and token order, not the wrapping, are
/// the contract.
fn normalized(doc: &str) -> String {
    doc.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn default_help_keeps_the_task_roles_distinct() -> Result<(), String> {
    for args in [["--help"].as_slice(), ["help"].as_slice()] {
        let stdout = normalized(&rendered_help(args)?);
        for needle in [
            "Diagnose setup ripr doctor",
            "Inspect one change ripr check",
            "Guided repo adoption ripr pilot --root .",
            "Repair one named gap ripr agent repair",
            "Compose PR evidence ripr first-pr",
            "Adopt advisory CI ripr init --ci github",
            "ripr help --all",
        ] {
            assert_contains(
                "default help (`ripr --help` / `ripr help`)",
                &stdout,
                needle,
            )?;
        }
    }
    Ok(())
}

#[test]
fn exhaustive_help_keeps_the_same_roles_and_boundaries() -> Result<(), String> {
    let stdout = normalized(&rendered_help(&["help", "--all"])?);
    for needle in [
        "Diagnose setup ripr doctor",
        "Inspect one change ripr check",
        "Guided repo adoption ripr pilot --root .",
        "Repair one named gap ripr agent repair --seam-id ID --phase before|after|verify",
        "Compose PR evidence ripr first-pr --root . --base BASE --head HEAD",
        "Adopt advisory CI ripr init --ci github",
        "`ripr check` is the ordinary first-value analysis; `ripr pilot` is the guided repo-adoption workflow.",
        "`ripr first-pr` and `ripr start-here` compose `target/ripr/reports/start-here.{json,md}` from existing artifacts; they do not run analysis or repair a gap.",
    ] {
        assert_contains("exhaustive help (`ripr help --all`)", &stdout, needle)?;
    }
    Ok(())
}

/// The help screens are the exhaustive reference, so a line on one is a claim
/// about what the command accepts. Each needle below was wrong against source
/// until this test existed: the repair rows printed `--phase verify` forms that
/// always fail (the verify phase refuses without `--verify-authorized` and
/// `--verify-authority`, and takes only `--attempt`), two report rows named
/// default outputs the producers never write, and four rows omitted selectors
/// the parsers accept. The negative asserts are the discriminators — they fail
/// on the exact prior wording.
#[test]
fn help_screens_state_the_surfaces_the_parsers_accept() -> Result<(), String> {
    // The default screen is a bounded first screen with a line budget
    // (`help_leads_with_a_bounded_first_screen_that_routes_onward`), so the
    // verify phase does not belong on it at all: it is reachable only for
    // trust-bound Python attempts and needs an authorization pair that would
    // not fit. Its repair teaser is before -> edit -> after, the ordinary Rust
    // path, and the full invocation lives in `help --all` and
    // `agent repair --help`.
    let default_help = normalized(&rendered_help(&["--help"])?);
    for needle in [
        "ripr agent repair --seam-id ID --phase before",
        "ripr agent repair --attempt ID --phase after",
    ] {
        assert_contains("default help (`ripr --help`)", &default_help, needle)?;
    }
    if default_help.contains("ripr agent repair --attempt ID --phase verify") {
        return Err(
            "the bounded first screen must not print a verify invocation it cannot make runnable"
                .to_string(),
        );
    }

    let all = normalized(&rendered_help(&["help", "--all"])?);
    for needle in [
        // The verify phase takes `--attempt`, never `--seam-id`, and refuses
        // without both authorization signals.
        "ripr agent repair --root . --attempt ID --phase verify --verify-authorized --verify-authority ID",
        "ripr agent repair --root . (--attempt ID | --seam-id ID) --phase after",
        // Producer defaults, from typescript_limitations.rs and
        // typescript_false_actionable.rs.
        "target/ripr/reports/typescript-limitations.json",
        "target/ripr/reports/typescript-false-actionable-audit.json",
        // `gate evaluate --help` lists four modes; this screen listed two.
        "--mode visible-only|acknowledgeable|baseline-check|calibrated-gate",
        // Selectors the parsers accept that this screen did not name.
        "ripr reports gap-ledger (--records PATH | --repo-exposure PATH | --check-output PATH)",
        "ripr agent packet --root . (--seam-id ID | --gap-ledger PATH --gap-id ID) --json",
        "--gap CANONICAL_GAP_ID --gap-ledger PATH",
        // `ripr mcp --help` states `--stdio` is optional and the default.
        "ripr mcp [--stdio] [--root PATH]",
    ] {
        assert_contains("exhaustive help (`ripr help --all`)", &all, needle)?;
    }

    // Discriminators. The `Repair one named gap` role line keeps
    // `--phase before|after|verify`: it names which phases exist rather than
    // how to invoke one, and `default_help_keeps_the_task_roles_distinct`
    // pins it as role vocabulary.
    for (surface, text, stale) in [
        (
            "exhaustive help",
            &all,
            "ripr agent repair --root . --seam-id ID --phase before|after|verify",
        ),
        (
            "exhaustive help",
            &all,
            "target/ripr/reports/ts-limitations.json",
        ),
        (
            "exhaustive help",
            &all,
            "target/ripr/reports/ts-false-actionable.json",
        ),
        (
            "exhaustive help",
            &all,
            "--mode visible-only|acknowledgeable]",
        ),
        ("exhaustive help", &all, "ripr mcp --stdio [--root PATH]"),
    ] {
        if text.contains(stale) {
            return Err(format!("{surface} still prints the stale form `{stale}`"));
        }
    }
    Ok(())
}

#[test]
fn agent_help_makes_repair_primary_without_removing_control_surfaces() -> Result<(), String> {
    for args in [["agent", "--help"].as_slice(), ["agent"].as_slice()] {
        let stdout = rendered_help(args)?;
        let collapsed = normalized(&stdout);
        assert_contains("agent help", &collapsed, "Primary workflow:")?;

        assert_contains(
            "agent help",
            &collapsed,
            "repair Run the before/edit/after repair transaction and its verification phase for one seam.",
        )?;
        assert_contains(
            "agent help",
            &collapsed,
            "status Report existing agent-loop artifacts and the exact next command.",
        )?;
        let advanced = stdout
            .split("Advanced and compatibility workflows:")
            .nth(1)
            .ok_or_else(|| {
                "agent help lost the explicit advanced/compatibility boundary".to_string()
            })?;
        // Match each advanced command as the first token of a listed entry, so
        // a dropped entry cannot hide behind a substring of another command's
        // name or description (`verify` inside `verify-execute`, and so on).
        for command in [
            "start",
            "brief",
            "packet",
            "verify",
            "verify-execute",
            "receipt",
            "review-summary",
        ] {
            let listed = advanced
                .lines()
                .any(|line| line.split_whitespace().next() == Some(command));
            if !listed {
                return Err(format!(
                    "agent help lost the advanced command entry `{command}`"
                ));
            }
        }
    }
    Ok(())
}

/// The primary repair transaction has its own help surface; it must keep the
/// before/edit/after sequence and the explicit limits (no test generation, no
/// mutation execution, no merge authority) that keep the route advisory.
#[test]
fn agent_repair_help_names_the_primary_transaction_and_its_limits() -> Result<(), String> {
    let stdout = normalized(&rendered_help(&["agent", "repair", "--help"])?);
    for needle in [
        "Run the before/edit/after repair transaction and its verification phase for one named gap.",
        "ripr agent repair --seam-id ID --phase before",
        "# edit one focused test outside RIPR",
        "ripr agent repair --attempt ID --phase after",
        "--verify-authorized",
        "--verify-authority ID",
        "--verify-rollback",
        "ripr agent repair [--root PATH] --attempt ID --phase verify",
        "The repair command does not generate or apply tests, execute mutation testing, or declare the repository safe to merge.",
    ] {
        assert_contains(
            "agent repair help (`ripr agent repair --help`)",
            &stdout,
            needle,
        )?;
    }
    Ok(())
}

/// `check --help` must teach the loader's real default-base resolution order
/// (#3885), not the old `origin/main` shorthand. `diff` resolves an omitted
/// base through the same authority (#3952), so its help states the same order.
#[test]
fn check_and_diff_help_state_the_real_base_default() -> Result<(), String> {
    let check = normalized(&rendered_help(&["check", "--help"])?);
    for needle in [
        "the local origin/HEAD ref, then origin/main, origin/master, main, and master in order",
        "when none of those resolves, the analysis does not run",
    ] {
        assert_contains("check help (`ripr check --help`)", &check, needle)?;
    }
    if check.contains("Defaults to origin/main") {
        return Err(
            "check help still teaches the origin/main default instead of the resolution order"
                .to_string(),
        );
    }
    let diff = normalized(&rendered_help(&["diff", "--help"])?);
    assert_contains(
        "diff help (`ripr diff --help`)",
        &diff,
        "the local origin/HEAD ref, then origin/main, origin/master, main, and master",
    )?;
    if diff.contains("Defaults to origin/main") {
        return Err("diff help still teaches the origin/main default".to_string());
    }
    Ok(())
}

/// Validate the command cell of each task row, not mentions elsewhere in the
/// document. The result prose can change without changing the command's job.
fn assert_doc_command_routes(doc: &str) -> Result<(), String> {
    for (task, expected) in [
        ("Inspect one change", "ripr check"),
        ("Explore the repository", "ripr pilot --root ."),
        ("Repair a selected Rust gap", "ripr agent repair"),
        ("Resume a repair", "ripr agent status --root ."),
        ("Compose PR evidence", "ripr first-pr"),
        ("Add advisory CI", "ripr init --ci github"),
        ("Diagnose setup", "ripr doctor"),
    ] {
        let mut matches = doc.lines().filter_map(|line| {
            let mut cells = line.trim().strip_prefix('|')?.split('|');
            let role = cells.next()?.trim();
            let command = cells.next()?.trim();
            let result = cells.next()?.trim();
            (role == task && !result.is_empty()).then_some(command)
        });
        let cell = matches
            .next()
            .ok_or_else(|| format!("command guide lost task row `{task}`"))?;
        if matches.next().is_some() {
            return Err(format!(
                "command guide has duplicate task rows for `{task}`"
            ));
        }
        let correct_command = cell
            .split('`')
            .skip(1)
            .step_by(2)
            .any(|span| normalized(span) == expected);
        if !correct_command {
            return Err(format!("task `{task}` must route to `{expected}`"));
        }
    }
    Ok(())
}

fn first_bash_block(doc: &str) -> Result<String, String> {
    let (_, after) = doc
        .split_once("```bash")
        .ok_or_else(|| "document has no Bash example".to_string())?;
    let (body, _) = after
        .split_once("```")
        .ok_or_else(|| "Bash example has no closing fence".to_string())?;
    Ok(normalized(body))
}

fn doc_section(doc: &str, heading: &str) -> Result<String, String> {
    let lf = doc.replace("\r\n", "\n");
    let marker = format!("\n{heading}\n");
    let (_, after) = lf
        .split_once(&marker)
        .ok_or_else(|| format!("document lost linked section `{heading}`"))?;
    after
        .split("\n## ")
        .next()
        .map(str::to_string)
        .ok_or_else(|| format!("document has no content for `{heading}`"))
}

/// Keep task ownership and executable entry points aligned without pinning
/// editorial prose. The six rendered-help tests above remain unchanged.
#[test]
fn docs_keep_the_canonical_role_vocabulary() -> Result<(), String> {
    assert_doc_command_routes(COMMAND_HIERARCHY_DOC)?;
    if first_bash_block(ROOT_README)? != "cargo install ripr ripr check" {
        return Err("README first run must install ripr and inspect a change".to_string());
    }
    for target in [
        "docs/QUICKSTART.md#cli-first-hour",
        "docs/QUICKSTART.md#agent-or-reviewer-first-hour",
        "docs/QUICKSTART.md#vs-code-first-hour",
        "docs/QUICKSTART.md#ci-first-hour",
        "docs/COMMAND_HIERARCHY.md",
    ] {
        assert_contains("README task navigation", ROOT_README, target)?;
    }
    for (heading, command) in [
        ("## CLI First Hour", "ripr check"),
        ("## CI First Hour", "ripr init --ci github"),
        ("## Agent Or Reviewer First Hour", "ripr pilot --root ."),
    ] {
        let section = doc_section(QUICKSTART_DOC, heading)?;
        if first_bash_block(&section)? != command {
            return Err(format!(
                "Quickstart `{heading}` must start with `{command}`"
            ));
        }
    }
    let repair = doc_section(QUICKSTART_DOC, "## Agent Or Reviewer First Hour")?;
    for target in [
        "--phase before",
        "--attempt",
        "--phase after",
        "ripr agent status --root .",
        "REPAIR_ATTEMPT.md#governed-python-sequence",
    ] {
        assert_contains("Quickstart repair continuation", &repair, target)?;
    }
    Ok(())
}

#[test]
fn doc_routes_allow_editorial_rewording_and_table_spacing() -> Result<(), String> {
    let reworded = COMMAND_HIERARCHY_DOC
        .replace(
            "Static findings, or an explicit no-action or limited result.",
            "Findings for the selected change, with any limits disclosed.",
        )
        .replace("| Inspect one change |", "|   Inspect one change   |")
        .replace("`ripr check`", "`ripr   check`")
        .replace('\n', "\r\n");
    assert_doc_command_routes(&reworded)?;
    if first_bash_block("```bash\r\nripr   check\r\n```\r\n")? != "ripr check" {
        return Err("first-run matching must allow whitespace changes".to_string());
    }
    Ok(())
}

#[test]
fn doc_routes_reject_missing_wrong_and_duplicate_tasks() -> Result<(), String> {
    let missing = COMMAND_HIERARCHY_DOC
        .lines()
        .filter(|line| !line.starts_with("| Inspect one change |"))
        .collect::<Vec<_>>()
        .join("\n");
    let wrong = COMMAND_HIERARCHY_DOC.replace(
        "| Inspect one change | `ripr check` |",
        "| Inspect one change | `ripr pilot` |",
    );
    let duplicate =
        format!("{COMMAND_HIERARCHY_DOC}\n| Inspect one change | `ripr check` | Duplicate. |\n");
    for (case, changed) in [
        ("missing", missing),
        ("wrong", wrong),
        ("duplicate", duplicate),
    ] {
        if assert_doc_command_routes(&changed).is_ok() {
            return Err(format!("doc route guard accepted the {case} task mutation"));
        }
    }
    Ok(())
}

#[test]
fn first_run_guard_does_not_credit_a_later_correct_example() -> Result<(), String> {
    let changed = "```bash\nripr pilot\n```\nLater:\n```bash\nripr check\n```\n";
    if first_bash_block(changed)? == "ripr check" {
        return Err("a later command must not hide a wrong first-run route".to_string());
    }
    let cli = doc_section(QUICKSTART_DOC, "## CLI First Hour")?;
    let wrong = cli.replacen("ripr check", "ripr pilot", 1);
    if first_bash_block(&wrong)? == "ripr check" {
        return Err(
            "the CLI section credited a later check instead of its first command".to_string(),
        );
    }
    Ok(())
}

/// PR #4196 review: the doctor exit-code guide must describe both profiles.
/// The default analysis profile keeps a missing or old toolchain advisory
/// (exit `0`); only `--profile source-build` fails on it. A guide that still
/// says a Rust root fails on a missing toolchain contradicts the binary.
#[test]
fn doctor_exit_code_guide_distinguishes_analysis_and_source_build() -> Result<(), String> {
    let section = normalized(&doc_section(EXIT_CODES_DOC, "## `ripr doctor` exit codes")?);
    let help = normalized(&rendered_help(&["doctor", "--help"])?);
    for needle in [
        "--profile source-build",
        "`advisory`",
        "does not change the exit code",
    ] {
        assert_contains("docs/EXIT_CODES.md doctor section", &section, needle)?;
    }
    assert_contains(
        "ripr doctor --help",
        &help,
        "--profile analysis|source-build",
    )?;
    if section.contains("fail on a missing manifest or toolchain") {
        return Err(
            "docs/EXIT_CODES.md still says the default doctor fails on a missing toolchain"
                .to_string(),
        );
    }
    Ok(())
}
