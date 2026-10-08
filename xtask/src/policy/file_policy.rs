use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use super::phase_diagnostics::Observation;
use crate::run::{TimedOutput, capture_output_with_timeout_observed};
use crate::{
    FilePolicyHost, FilePolicyTestCommand, FixKind, PolicyDisclosure, PolicyReportSpec,
    collect_files, finish_policy_report_with_disclosures, is_cargo_test_command,
    is_file_policy_candidate, is_non_rust_programming_candidate, matches_any_glob,
    non_rust_programming_retention_reason, normalize_path, read_file_policy_allowlist,
    read_file_policy_test_commands,
};

const TEST_COVERED_BY_LIST_TIMEOUT: Duration = Duration::from_mins(5);
/// Compile budget, separate from listing. A cold `cargo test --list` spends
/// almost all of its wall clock building; that must not be charged against
/// the list cap or reported as an unresolved `covered_by` pointer (#4141).
const TEST_COVERED_BY_BUILD_TIMEOUT: Duration = Duration::from_mins(30);
const COVERED_BY_INSTRUMENT_PREFIX: &str = "covered_by_instrument_timeout:";

/// Validate the repository's non-Rust file policy and write its standard
/// report. The parser and shared path predicates remain in `main.rs` until
/// their other policy/report consumers can move in a later slice.
pub(crate) fn check_file_policy() -> Result<(), String> {
    let policy_path = "policy/non-rust-allowlist.toml";
    let allowlist = read_file_policy_allowlist(policy_path)?;
    let coverage =
        validate_test_covered_by(policy_path, &read_file_policy_test_commands(policy_path)?)?;
    let mut violations = Vec::new();

    for path in collect_files(Path::new("."))? {
        let normalized = normalize_path(&path);
        if !is_file_policy_candidate(&normalized) {
            continue;
        }
        if normalized.ends_with(".rs") {
            continue;
        }
        if !matches_any_glob(&allowlist, &normalized) {
            violations.push(format!(
                "unapproved non-Rust programming/declarative file: {normalized}\n  preferred: implement automation in Rust/xtask or add a policy allowlist entry with owner and reason"
            ));
            continue;
        }
        if is_non_rust_programming_candidate(&normalized)
            && non_rust_programming_retention_reason(&normalized).is_none()
        {
            violations.push(format!(
                "non-Rust programming file lacks a keep-non-Rust retention rule: {normalized}\n  preferred: convert implementation/test automation to Rust/xtask unless the file is bound to an approved non-Rust runtime surface"
            ));
        }
    }

    finish_policy_report_with_disclosures(
        PolicyReportSpec {
            report_file: "file-policy.md",
            check: "check-file-policy",
            why_it_matters: "Rust and xtask are the default implementation surface so repo automation stays typed, tested, and reviewable.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Move implementation or automation logic into Rust/xtask.",
                "If the file belongs to an approved surface, add an allowlist entry with owner and reason.",
            ],
            rerun_command: "cargo xtask check-file-policy",
            exception_template: Some(
                "policy/non-rust-allowlist.toml entry:\n[[allow]]\nglob = \"path/**/*.ext\"\nkind = \"surface_kind\"\nowner = \"team/area\"\nsurface = \"docs|editor|fixtures|policy|rust|ci\"\nclassification = \"production|test|tooling|generated|config|docs|fixture|metadata\"\nreason = \"why this must remain non-Rust or declarative\"\ncovered_by = [\"cargo xtask check-file-policy\"]",
            ),
        },
        &violations,
        &[PolicyDisclosure {
            heading: "Test-valued coverage applicability".to_string(),
            intro: "Applicable selectors must enumerate nonzero tests. Enumeration is not test execution. Host-inapplicable selectors are not enumerated and confer no coverage on this host.".to_string(),
            items: coverage.iter().map(TestCoverageObservation::render).collect(),
        }],
    )
}

#[derive(Debug)]
struct TestCoverageObservation {
    selector: FilePolicyTestCommand,
    host: FilePolicyHost,
    // None is explicitly inapplicable, never a successful empty enumeration.
    selected: Option<Vec<String>>,
}

impl TestCoverageObservation {
    fn render(&self) -> String {
        let declared = self.selector.host.map_or("all", FilePolicyHost::name);
        let status = match &self.selected {
            Some(tests) => format!(
                "applicable; selected={}; tests=[{}]",
                tests.len(),
                tests.join(", ")
            ),
            None => "not_applicable; selected=not_enumerated".to_string(),
        };
        format!(
            "line {}; host={}; declared={declared}; {status}; `{}`",
            self.selector.line,
            self.host.name(),
            self.selector.command
        )
    }
}

fn validate_test_covered_by(
    path: &str,
    commands: &[FilePolicyTestCommand],
) -> Result<Vec<TestCoverageObservation>, String> {
    let mut warmed = BTreeSet::new();
    let host = FilePolicyHost::current()?;
    let owned_scope = crate::run::file_policy_capture_scope()?;
    let mut selectors = commands
        .iter()
        .filter(|selector| selector.host.is_none_or(|declared| declared == host));
    let mut ordinal = 0;
    let observation = Observation::for_checker(|| diagnostic_plan(commands, host));
    let result = validate_test_covered_by_with(path, commands, host, |args| {
        ordinal += 1;
        let policy_line = selectors.next().map(|selector| selector.line);
        let spawns = enumeration_spawns(args);
        let warmup = spawns.len() == 2;
        let mut listed = None;
        for (index, spawn) in spawns.into_iter().enumerate() {
            let warmup_key = (index == 0 && warmup).then(|| successful_warmup_key(&spawn.args));
            if warmup_key.as_ref().is_some_and(|key| warmed.contains(key)) {
                continue;
            }
            let phase = observation.as_ref().and_then(|observation| {
                observation.phase(ordinal, policy_line, spawn.description, &spawn.args)
            });
            let output = if let Some(scope) = &owned_scope {
                crate::run::capture_file_policy_cargo(
                    &spawn.args,
                    spawn.timeout,
                    spawn.description,
                    scope,
                    phase.as_ref(),
                )?
            } else if let Some(phase) = &phase {
                capture_output_with_timeout_observed(
                    "cargo",
                    &spawn.args,
                    &[],
                    spawn.timeout,
                    spawn.description,
                    phase,
                )?
            } else {
                crate::run::capture_output_with_timeout(
                    "cargo",
                    &spawn.args,
                    &[],
                    spawn.timeout,
                    spawn.description,
                )?
            };
            if output.timed_out || !output.status.is_some_and(|status| status.success()) {
                return Ok(enumeration_result(output, spawn.timeout));
            }
            if let Some(key) = warmup_key {
                record_successful_warmup(&mut warmed, key, &output);
            }
            listed = Some(output);
        }
        let output = listed
            .ok_or_else(|| "test-valued covered_by enumeration produced no spawn".to_string())?;
        Ok(enumeration_result(output, TEST_COVERED_BY_LIST_TIMEOUT))
    });
    if let Some(observation) = &observation {
        observation.finished(result.is_ok());
    }
    result
}

/// One cargo spawn used to prove a test-valued `covered_by` pointer.
struct CoveredBySpawn {
    args: Vec<String>,
    timeout: Duration,
    description: &'static str,
}

/// Warm a cold compile under the build budget, then list under the list cap.
///
/// Ordinary `cargo test` pointers warm with `--no-run`. `cargo test --doc`
/// rejects `--no-run` (`can't skip running doc tests with --no-run`), and a
/// second `cargo test --doc -- --list` still runs `rustdoc --test` after the
/// library is fresh. That pointer is therefore enumerated once, under the
/// compile budget. A second spawn would put the doctest compile back inside
/// the five-minute list cap (#4141).
fn enumeration_spawns(args: &[String]) -> Vec<CoveredBySpawn> {
    if let Some(build_args) = build_args_before_list(args) {
        vec![
            CoveredBySpawn {
                args: build_args,
                timeout: TEST_COVERED_BY_BUILD_TIMEOUT,
                description: "test-valued covered_by build",
            },
            CoveredBySpawn {
                args: args.to_vec(),
                timeout: TEST_COVERED_BY_LIST_TIMEOUT,
                description: "test-valued covered_by enumeration",
            },
        ]
    } else {
        vec![CoveredBySpawn {
            args: args.to_vec(),
            timeout: TEST_COVERED_BY_BUILD_TIMEOUT,
            description: "test-valued covered_by doc enumeration",
        }]
    }
}

/// Invocation-local compilation identity, never a cached test enumeration.
/// Known syntax retains every scope/flag token and TESTNAME presence while
/// ignoring only its value. Unknown syntax retains the original complete argv.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct WarmupKey {
    argv: Vec<String>,
    testname_present: Option<bool>,
}
fn successful_warmup_key(args: &[String]) -> WarmupKey {
    let exact = || WarmupKey {
        argv: args.to_vec(),
        testname_present: None,
    };
    if args.first().map(String::as_str) != Some("test")
        || args.last().map(String::as_str) != Some("--no-run")
    {
        return exact();
    }
    let mut argv = vec!["test".to_string()];
    let mut testname = false;
    let mut index = 1;
    while index < args.len() {
        let argument = &args[index];
        match argument.as_str() {
            "-p" | "--package" | "--bin" | "--test" => {
                let Some(value) = args
                    .get(index + 1)
                    .filter(|value| !value.is_empty() && !value.starts_with('-'))
                else {
                    return exact();
                };
                argv.push(argument.clone());
                argv.push(value.clone());
                index += 2;
            }
            "--locked" | "--offline" => {
                argv.push(argument.clone());
                index += 1;
            }
            "--no-run" if index == args.len() - 1 => {
                argv.push(argument.clone());
                index += 1;
            }
            _ if argument.is_empty() || argument.starts_with('-') || testname => return exact(),
            _ => {
                testname = true;
                index += 1;
            }
        }
    }
    WarmupKey {
        argv,
        testname_present: Some(testname),
    }
}
fn record_successful_warmup(
    warmed: &mut BTreeSet<WarmupKey>,
    key: WarmupKey,
    output: &TimedOutput,
) {
    if !output.timed_out && output.status.is_some_and(|status| status.success()) {
        warmed.insert(key);
    }
}

/// Plan compilation only; the governed checker still runs every real warmup
/// and listing. Use the same scope owner as ordinary covered-by enumeration.
pub(crate) fn materialized_build_preparation_plan(
    policy: &Path,
    host: FilePolicyHost,
) -> Result<Vec<Vec<String>>, String> {
    let policy = policy
        .to_str()
        .ok_or("materialized file policy path is not UTF-8")?;
    let mut scopes = BTreeSet::new();
    let mut builds = Vec::new();
    for command in read_file_policy_test_commands(policy)?
        .iter()
        .filter(|command| command.host.is_none_or(|declared| declared == host))
    {
        let spawns = enumeration_spawns(&enumeration_args(&command.command));
        if spawns.len() != 2 {
            return Err("materialized preparation requires a separate build/list recipe".into());
        }
        let build = &spawns[0].args;
        if scopes.insert(successful_warmup_key(build)) {
            builds.push(build.clone());
        }
    }
    Ok(builds)
}

fn enumeration_args(command: &str) -> Vec<String> {
    let words = command.split_whitespace().skip(2);
    let mut args = vec!["test".to_string()];
    args.extend(words.map(ToString::to_string));
    args.extend([
        "--".to_string(),
        "--list".to_string(),
        "--format".to_string(),
        "terse".to_string(),
    ]);
    args
}
pub(crate) fn diagnostic_plan(
    commands: &[FilePolicyTestCommand],
    host: FilePolicyHost,
) -> Option<super::phase_diagnostics::DiagnosticPlan> {
    let mut argv = Vec::new();
    let mut steps = Vec::new();
    let mut warmed = BTreeSet::new();
    for (index, selector) in commands
        .iter()
        .filter(|v| v.host.is_none_or(|h| h == host))
        .enumerate()
    {
        for (spawn_index, spawn) in enumeration_spawns(&enumeration_args(&selector.command))
            .into_iter()
            .enumerate()
        {
            let warmup = spawn.description == "test-valued covered_by build";
            // Plan only the success path; an actual failed warmup returns before any reuse.
            if spawn_index == 0 && warmup && !warmed.insert(successful_warmup_key(&spawn.args)) {
                continue;
            }
            let kind = match spawn.description {
                "test-valued covered_by build" => "b",
                "test-valued covered_by enumeration" => "l",
                "test-valued covered_by doc enumeration" => "d",
                _ => return None,
            };
            let argid = if let Some(argid) = argv.iter().position(|v| v == &spawn.args) {
                argid
            } else {
                argv.push(spawn.args);
                argv.len() - 1
            };
            steps.push(super::phase_diagnostics::PlannedPhase {
                selector: index + 1,
                line: selector.line,
                kind,
                argv: argid,
            });
        }
    }
    super::phase_diagnostics::DiagnosticPlan::new(host.name(), argv, steps)
}

/// Build args for `cargo test … --no-run`, or `None` when Cargo rejects
/// that combination. `cargo test --doc --no-run` is an error, so a
/// documentation-test pointer is not given a `--no-run` spawn.
/// [`enumeration_spawns`] enumerates that pointer once under the build
/// budget. Cargo recompiles doctests on every listing, so a follow-up list
/// would charge that compile against the list cap.
fn build_args_before_list(args: &[String]) -> Option<Vec<String>> {
    let mut build_args = Vec::new();
    for arg in args {
        if arg == "--" {
            break;
        }
        if arg == "--doc" {
            return None;
        }
        build_args.push(arg.clone());
    }
    build_args.push("--no-run".to_string());
    Some(build_args)
}

fn enumeration_result(output: TimedOutput, timeout: Duration) -> (bool, String, String) {
    let status = output
        .status
        .map(|status| status.to_string())
        .unwrap_or_else(|| "not available".to_string());
    let timeout_note = if output.timed_out {
        format!("{COVERED_BY_INSTRUMENT_PREFIX} timed out after {timeout:?}; ")
    } else {
        String::new()
    };
    let stderr = format!(
        "{timeout_note}status: {status}\n{}",
        output.stderr.trim_end()
    );
    (
        output.status.is_some_and(|status| status.success()) && !output.timed_out,
        output.stdout,
        stderr,
    )
}

fn validate_test_covered_by_with(
    path: &str,
    commands: &[FilePolicyTestCommand],
    host: FilePolicyHost,
    mut enumerate: impl FnMut(&[String]) -> Result<(bool, String, String), String>,
) -> Result<Vec<TestCoverageObservation>, String> {
    let mut observations = Vec::new();
    for selector in commands {
        let FilePolicyTestCommand { line, command, .. } = selector;
        if !is_cargo_test_command(command) {
            return Err(format!(
                "{path}:{line} unsupported test-valued `covered_by`: {command}"
            ));
        }
        if selector.host.is_some_and(|declared| declared != host) {
            observations.push(TestCoverageObservation {
                selector: selector.clone(),
                host,
                selected: None,
            });
            continue;
        }
        let args = enumeration_args(command);
        let (success, stdout, stderr) = enumerate(&args)
            .map_err(|error| format!("{path}:{line} enumerate `{command}`: {error}"))?;
        if stderr.starts_with(COVERED_BY_INSTRUMENT_PREFIX) {
            return Err(format!(
                "{path}:{line} test-valued `covered_by` enumeration did not finish (instrument timeout, not an unresolved pointer): `{command}`\nstdout: {stdout}\nstderr: {stderr}"
            ));
        }
        if !success {
            return Err(format!(
                "{path}:{line} test-valued `covered_by` could not be enumerated: `{command}`\nstdout: {stdout}\nstderr: {stderr}"
            ));
        }
        let selected: Vec<String> = stdout
            .lines()
            .filter_map(|line| line.strip_suffix(": test").map(str::to_string))
            .collect();
        if selected.is_empty() {
            return Err(format!(
                "{path}:{line} test-valued `covered_by` selects zero tests: `{command}`"
            ));
        }
        observations.push(TestCoverageObservation {
            selector: selector.clone(),
            host,
            selected: Some(selected),
        });
    }
    Ok(observations)
}

#[cfg(test)]
mod tests {
    #[test]
    fn pypi_admission_retention_is_exactly_scoped() {
        for path in [
            ".github/scripts/pypi_admission.py",
            ".github/scripts/test_pypi_admission.py",
        ] {
            assert!(crate::non_rust_programming_retention_reason(path).is_some());
        }
        for path in [".github/scripts/unrelated.py", "scripts/pypi_admission.py"] {
            assert!(crate::non_rust_programming_retention_reason(path).is_none());
        }
    }

    #[test]
    fn npm_qualification_retention_is_exactly_scoped() {
        for path in [
            ".github/scripts/npm_package.py",
            ".github/scripts/npm_consumer.py",
            ".github/scripts/test_npm_package.py",
        ] {
            assert!(crate::non_rust_programming_retention_reason(path).is_some());
        }
        for path in [".github/scripts/npm_other.py", "scripts/npm_package.py"] {
            assert!(crate::non_rust_programming_retention_reason(path).is_none());
        }
    }

    use std::time::Duration;

    use super::COVERED_BY_INSTRUMENT_PREFIX;
    use super::TEST_COVERED_BY_BUILD_TIMEOUT;
    use super::TEST_COVERED_BY_LIST_TIMEOUT;
    use super::build_args_before_list;
    use super::enumeration_result;
    use super::enumeration_spawns;
    use super::validate_test_covered_by;
    use super::validate_test_covered_by_with;
    use crate::run::TimedOutput;
    use crate::{FilePolicyHost, FilePolicyTestCommand, is_cargo_test_command};

    #[test]
    fn materialized_build_preparation_uses_exact_actual_host_recipes() -> Result<(), String> {
        let policy = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or("xtask repository parent unavailable")?
            .join("policy/non-rust-allowlist.toml");
        // Independent frozen host recipes, not derived from the planner/key.
        let expected_unix = vec![
            vec![
                "test",
                "-p",
                "xtask",
                "repository_language_policy_admits_real_mixed_language_producer",
                "--no-run",
            ],
            vec![
                "test",
                "-p",
                "xtask",
                "--locked",
                "--offline",
                "rust_judged_panel::rolling_observation",
                "--no-run",
            ],
            vec!["test", "-p", "ripr", "--lib", "workflow", "--no-run"],
            vec![
                "test",
                "-p",
                "ripr",
                "--test",
                "generated_review_workflow",
                "--no-run",
            ],
            vec![
                "test",
                "-p",
                "ripr",
                "--test",
                "generated_review_workflow",
                "generated_released_publish_adapter_preserves_current_request_semantics",
                "--no-run",
            ],
            vec![
                "test",
                "-p",
                "xtask",
                "--bin",
                "xtask",
                "dx_scoreboard",
                "--no-run",
            ],
            vec![
                "test",
                "-p",
                "ripr",
                "--test",
                "causal_delta_fixture",
                "--no-run",
            ],
            vec![
                "test",
                "-p",
                "ripr",
                "--locked",
                "--offline",
                "--test",
                "portable_consumer_packet",
                "--no-run",
            ],
        ]
        .into_iter()
        .map(|row| row.into_iter().map(str::to_string).collect::<Vec<_>>())
        .collect::<Vec<_>>();
        let expected_windows = vec![
            vec![
                "test",
                "-p",
                "xtask",
                "repository_language_policy_admits_real_mixed_language_producer",
                "--no-run",
            ],
            vec![
                "test",
                "-p",
                "xtask",
                "--locked",
                "--offline",
                "rust_judged_panel::rolling_observation",
                "--no-run",
            ],
            vec!["test", "-p", "ripr", "--lib", "workflow", "--no-run"],
            vec![
                "test",
                "-p",
                "xtask",
                "--bin",
                "xtask",
                "dx_scoreboard",
                "--no-run",
            ],
            vec![
                "test",
                "-p",
                "ripr",
                "--test",
                "causal_delta_fixture",
                "--no-run",
            ],
        ]
        .into_iter()
        .map(|row| row.into_iter().map(str::to_string).collect::<Vec<_>>())
        .collect::<Vec<_>>();
        assert_eq!(
            super::materialized_build_preparation_plan(&policy, FilePolicyHost::Unix)?,
            expected_unix
        );
        assert_eq!(
            super::materialized_build_preparation_plan(&policy, FilePolicyHost::Windows)?,
            expected_windows
        );
        let commands =
            crate::read_file_policy_test_commands(policy.to_str().ok_or("policy path UTF-8")?)?;
        assert_eq!(commands.len(), 31);
        assert_eq!(
            commands
                .iter()
                .filter(|command| command
                    .host
                    .is_none_or(|host| host == FilePolicyHost::Windows))
                .count(),
            25
        );
        Ok(())
    }

    const FROZEN_WARMUP_SELECTORS: &str = r#"[[107,["test","-p","xtask","repository_language_policy_admits_real_mixed_language_producer"]],[348,["test","-p","xtask","implementation_slices_validate_and_coexist"]],[357,["test","-p","xtask","committed_spec_review_receipts_validate"]],[411,["test","-p","xtask","--locked","--offline","rust_judged_panel::rolling_observation"]],[411,["test","-p","xtask","--locked","--offline","rust_judged_panel::calibration"]],[411,["test","-p","xtask","--locked","--offline","rust_analysis_feedback"]],[420,["test","-p","xtask","rust_judged_panel::subject"]],[429,["test","-p","xtask","--locked","--offline","rust_judged_panel::packet::tests"]],[661,["test","-p","xtask","--bin","xtask","dx_scoreboard"]],[697,["test","-p","xtask","--bin","xtask","pilot_ranking"]],[706,["test","-p","xtask","--bin","xtask","pilot_ranking"]],[733,["test","-p","ripr","--test","causal_delta_fixture"]],[742,["test","-p","xtask","--bin","xtask","source_promotion_workflow"]],[751,["test","-p","xtask","source_promotion_control"]],[764,["test","-p","xtask","--locked","--offline","portable_consumer"]],[764,["test","-p","ripr","--locked","--offline","--test","portable_consumer_packet"]],[893,["test","-p","xtask","public_proof"]],[903,["test","-p","xtask","public_proof"]],[913,["test","-p","xtask","public_proof"]],[923,["test","-p","xtask","public_proof"]],[932,["test","-p","xtask","public_proof"]],[941,["test","-p","xtask","public_proof"]],[950,["test","-p","xtask","public_proof"]]]"#;

    #[test]
    fn test_covered_by_successful_warmup_reuse_preserves_scope_and_runtime_lists()
    -> Result<(), String> {
        let words = |values: &[&str]| {
            values
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>()
        };
        let original = words(&["test", "-p", "xtask", "first_filter", "--no-run"]);
        let other = words(&["test", "-p", "xtask", "second_filter", "--no-run"]);
        let key = super::successful_warmup_key(&original);
        if key != super::successful_warmup_key(&other)
            || key.testname_present != Some(true)
            || original != words(&["test", "-p", "xtask", "first_filter", "--no-run"])
        {
            return Err("warmup key changed original argv or retained only filter value".into());
        }
        for different in [
            words(&["test", "-p", "xtask", "--no-run"]),
            words(&["test", "-p", "ripr", "first_filter", "--no-run"]),
            words(&["test", "--package", "xtask", "first_filter", "--no-run"]),
            words(&[
                "test",
                "-p",
                "xtask",
                "--bin",
                "xtask",
                "first_filter",
                "--no-run",
            ]),
            words(&[
                "test",
                "-p",
                "xtask",
                "--test",
                "integration",
                "first_filter",
                "--no-run",
            ]),
            words(&[
                "test",
                "-p",
                "xtask",
                "--locked",
                "first_filter",
                "--no-run",
            ]),
            words(&[
                "test",
                "-p",
                "xtask",
                "--offline",
                "first_filter",
                "--no-run",
            ]),
            words(&[
                "test",
                "-p",
                "xtask",
                "--features",
                "feature_one",
                "first_filter",
                "--no-run",
            ]),
            words(&[
                "test",
                "-p",
                "xtask",
                "--target",
                "x86_64-unknown-linux-gnu",
                "first_filter",
                "--no-run",
            ]),
            words(&[
                "test",
                "-p",
                "xtask",
                "--profile",
                "release",
                "first_filter",
                "--no-run",
            ]),
        ] {
            if key == super::successful_warmup_key(&different) {
                return Err(format!(
                    "different compile scope or TESTNAME presence reused:{different:?}"
                ));
            }
        }
        for unknown in [
            words(&[
                "test",
                "-p",
                "xtask",
                "--features",
                "feature_one",
                "first_filter",
                "--no-run",
            ]),
            words(&[
                "test",
                "-p",
                "xtask",
                "--future-option",
                "first_filter",
                "--no-run",
            ]),
            words(&[
                "test",
                "-p",
                "xtask",
                "first_filter",
                "second_filter",
                "--no-run",
            ]),
            words(&["test", "-p", "--no-run"]),
            words(&["test", "--package=xtask", "first_filter", "--no-run"]),
            words(&["test", "-p", "xtask", "first_filter", "--", "--no-run"]),
        ] {
            let unknown_key = super::successful_warmup_key(&unknown);
            if unknown_key.argv != unknown || unknown_key.testname_present.is_some() {
                return Err(
                    "unsupported syntax was normalized instead of using exact original argv".into(),
                );
            }
            let mut changed = unknown.clone();
            changed.insert(changed.len() - 1, "another_filter".into());
            if unknown_key == super::successful_warmup_key(&changed) {
                return Err("opaque syntax enabled cross-filter warmup reuse".into());
            }
        }
        let mut warmed = std::collections::BTreeSet::new();
        for failed in [
            timed_output(Some(status(1)), "original_stdout", "original_stderr", false),
            timed_output(None, "original_stdout", "original_stderr", false),
            timed_output(Some(status(0)), "original_stdout", "original_stderr", true),
        ] {
            super::record_successful_warmup(&mut warmed, key.clone(), &failed);
            if !warmed.is_empty() {
                return Err("failed/unavailable/timed-out warmup entered successful cache".into());
            }
        }
        let succeeded = timed_output(Some(status(0)), "original_stdout", "original_stderr", false);
        super::record_successful_warmup(&mut warmed, key, &succeeded);
        if !warmed.contains(&super::successful_warmup_key(&other))
            || succeeded.stdout != "original_stdout"
            || succeeded.stderr != "original_stderr"
        {
            return Err("successful reuse or untouched producer streams lost".into());
        }

        let rows: Vec<(usize, Vec<String>)> =
            serde_json::from_str(FROZEN_WARMUP_SELECTORS).map_err(|error| error.to_string())?;
        let commands = rows
            .iter()
            .map(|(line, args)| common(*line, &format!("cargo {}", args.join(" "))))
            .collect::<Vec<_>>();
        let mut actual_lists = Vec::new();
        let observations = super::validate_test_covered_by_with(
            "frozen-policy.toml",
            &commands,
            FilePolicyHost::Unix,
            |args| {
                actual_lists.push(args.to_vec());
                Ok((
                    true,
                    format!("owned_case_{}: test\n", actual_lists.len()),
                    String::new(),
                ))
            },
        )?;
        if observations.len() != 23
            || actual_lists.len() != 23
            || super::diagnostic_plan(&commands, FilePolicyHost::Unix).is_none()
        {
            return Err("actual selector/list route or bounded diagnostic plan lost".into());
        }
        let mut warmed = std::collections::BTreeSet::new();
        let mut warmups = Vec::new();
        let mut lists = Vec::new();
        for args in &actual_lists {
            let spawns = super::enumeration_spawns(args);
            if spawns.len() != 2 {
                return Err("original non-doc compile/list route changed".into());
            }
            for (index, spawn) in spawns.into_iter().enumerate() {
                if index == 0 {
                    let key = super::successful_warmup_key(&spawn.args);
                    if warmed.contains(&key) {
                        continue;
                    }
                    warmups.push(spawn.args.clone());
                    super::record_successful_warmup(
                        &mut warmed,
                        key,
                        &timed_output(Some(status(0)), "", "", false),
                    );
                } else {
                    lists.push(spawn.args);
                }
            }
        }
        let expected_first_original = [0, 3, 8, 11, 15]
            .into_iter()
            .map(|index| {
                super::build_args_before_list(&actual_lists[index])
                    .ok_or("original first warmup missing")
            })
            .collect::<Result<Vec<_>, _>>()?;
        if warmups != expected_first_original
            || warmups.len() != 5
            || lists != actual_lists
            || lists.len() != 23
        {
            return Err("first original warmup argv, five compile scopes or all23 original filtered lists lost".into());
        }
        for (index, args) in actual_lists.iter().enumerate() {
            if args != &super::enumeration_args(&commands[index].command) {
                return Err("actual per-row enumeration argv was changed".into());
            }
        }
        Ok(())
    }

    fn common(line: usize, command: &str) -> FilePolicyTestCommand {
        FilePolicyTestCommand {
            line,
            command: command.to_string(),
            host: None,
        }
    }

    #[cfg(windows)]
    fn status(code: u32) -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt;

        ExitStatusExt::from_raw(code)
    }

    #[cfg(unix)]
    fn status(code: i32) -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;

        ExitStatusExt::from_raw(code << 8)
    }

    fn timed_output(
        status: Option<std::process::ExitStatus>,
        stdout: &str,
        stderr: &str,
        timed_out: bool,
    ) -> TimedOutput {
        TimedOutput {
            status,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            duration: Duration::ZERO,
            timed_out,
        }
    }

    #[test]
    fn test_covered_by_output_mapping_fails_closed_with_partial_diagnostics() -> Result<(), String>
    {
        let failed_status = status(1);
        let failed_status_text = failed_status.to_string();
        let cases = [
            (
                timed_output(
                    Some(failed_status),
                    "selected_case: test\n",
                    "compiler stderr",
                    false,
                ),
                (format!("status: {failed_status_text}"), "compiler stderr"),
            ),
            (
                timed_output(
                    Some(status(0)),
                    "selected_case: test\n",
                    "partial stderr",
                    true,
                ),
                ("timed out after 300s".to_string(), "partial stderr"),
            ),
            (
                timed_output(None, "selected_case: test\n", "spawn stderr", false),
                ("status: not available".to_string(), "spawn stderr"),
            ),
        ];

        for (output, (expected_status, expected_stderr)) in cases {
            let (success, stdout, stderr) =
                enumeration_result(output, TEST_COVERED_BY_LIST_TIMEOUT);
            if success || stdout != "selected_case: test\n" || !stderr.contains(&expected_status) {
                return Err(format!(
                    "enumeration output mapping was not fail-closed: success={success}, stdout={stdout:?}, stderr={stderr:?}"
                ));
            }
            if !stderr.contains(expected_stderr) {
                return Err(format!(
                    "enumeration stderr payload was lost: expected={expected_stderr:?}, actual={stderr:?}"
                ));
            }
        }

        Ok(())
    }

    #[test]
    fn test_covered_by_classification_is_token_aware() -> Result<(), String> {
        for command in ["cargo test", "cargo\ttest -p xtask", "cargo\ntest filtered"] {
            if !is_cargo_test_command(command) {
                return Err(format!(
                    "cargo-test command was not classified: {command:?}"
                ));
            }
        }
        for command in ["cargo testable", "cargo check", "xcargo test"] {
            if is_cargo_test_command(command) {
                return Err(format!("non-test command was classified: {command:?}"));
            }
        }
        Ok(())
    }

    #[test]
    fn test_covered_by_host_selection_retains_common_and_discloses_inapplicable()
    -> Result<(), String> {
        let commands = [
            common(1, "cargo test common_case"),
            FilePolicyTestCommand {
                host: Some(FilePolicyHost::Unix),
                ..common(2, "cargo test unix_case")
            },
            FilePolicyTestCommand {
                host: Some(FilePolicyHost::Windows),
                ..common(3, "cargo test windows_case")
            },
        ];
        for host in [FilePolicyHost::Unix, FilePolicyHost::Windows] {
            let mut enumerated = Vec::new();
            let observations =
                validate_test_covered_by_with("policy.toml", &commands, host, |args| {
                    let subject = args.get(1).ok_or("missing test filter")?.clone();
                    enumerated.push(subject.clone());
                    Ok((true, format!("{subject}: test\n"), String::new()))
                })?;
            let applicable = format!("{}_case", host.name());
            assert_eq!(enumerated, ["common_case", applicable.as_str()]);
            assert_eq!(observations.len(), 3);
            assert_eq!(
                observations[0].selected.as_deref(),
                Some(["common_case".to_string()].as_slice())
            );
            for observation in observations {
                let rendered = observation.render();
                assert!(rendered.contains(&format!("host={}", host.name())));
                if observation
                    .selector
                    .host
                    .is_some_and(|declared| declared != host)
                {
                    assert!(observation.selected.is_none());
                    assert!(rendered.contains("not_applicable; selected=not_enumerated;"));
                    assert!(!rendered.contains("tests=["));
                } else {
                    assert!(rendered.contains("applicable; selected=1; tests=["));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn test_covered_by_empty_applicable_host_selector_is_rejected() -> Result<(), String> {
        for host in [FilePolicyHost::Unix, FilePolicyHost::Windows] {
            for declared in [None, Some(host)] {
                let commands = [FilePolicyTestCommand {
                    host: declared,
                    ..common(1, "cargo test nonexistent_subject")
                }];
                let result = validate_test_covered_by_with("policy.toml", &commands, host, |_| {
                    Ok((true, "0 tests, 0 benchmarks\n".to_string(), String::new()))
                });
                match result {
                    Err(error) if error.contains("selects zero tests") => {}
                    other => {
                        return Err(format!("empty applicable selector did not fail: {other:?}"));
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn test_covered_by_unknown_host_family_is_rejected() {
        assert_eq!(FilePolicyHost::parse("unix"), Ok(FilePolicyHost::Unix));
        assert_eq!(
            FilePolicyHost::parse("windows"),
            Ok(FilePolicyHost::Windows)
        );
        for unsupported in ["", "linux", "Windows", "wasm", "unix,windows"] {
            assert_eq!(
                FilePolicyHost::parse(unsupported),
                Err(format!(
                    "unsupported file-policy host family `{unsupported}`"
                ))
            );
        }
    }

    #[test]
    fn test_covered_by_requires_nonzero_successful_enumeration() -> Result<(), String> {
        let commands = [common(7, "cargo test -p xtask missing-filter")];
        let empty =
            validate_test_covered_by_with("policy.toml", &commands, FilePolicyHost::Unix, |_| {
                Ok((true, String::new(), String::new()))
            });
        let failed =
            validate_test_covered_by_with("policy.toml", &commands, FilePolicyHost::Unix, |_| {
                Ok((false, String::new(), "instrument failed".to_string()))
            });
        let nonzero =
            validate_test_covered_by_with("policy.toml", &commands, FilePolicyHost::Unix, |_| {
                Ok((true, "selected_case: test\n".to_string(), String::new()))
            });
        if empty.is_err() && failed.is_err() && nonzero.is_ok() {
            Ok(())
        } else {
            Err("test-valued covered_by did not fail closed on its denominator".to_string())
        }
    }

    #[test]
    fn test_covered_by_production_wrapper_enumerates_through_cargo() -> Result<(), String> {
        // End-to-end pin on the production wrapper (not the injected
        // closure): the args construction, bounded capture, and status
        // mapping all run for real, and an existing test filter enumerates
        // successfully.
        let commands = [common(
            12,
            "cargo test -p xtask test_covered_by_classification_is_token_aware",
        )];
        validate_test_covered_by("policy.toml", &commands).map(|_| ())
    }

    #[test]
    fn test_covered_by_production_wrapper_preserves_cargo_stderr() -> Result<(), String> {
        let commands = [common(17, "cargo test -p package-that-does-not-exist-3528")];
        let error = match validate_test_covered_by("policy.toml", &commands) {
            Ok(_) => return Err("failed Cargo enumeration unexpectedly passed".to_string()),
            Err(error) => error,
        };

        if error.contains("did not match any packages")
            && error.contains("stderr:")
            && error.contains("status:")
        {
            Ok(())
        } else {
            Err(format!("Cargo enumeration diagnostics were lost: {error}"))
        }
    }

    #[test]
    fn test_covered_by_preserves_enumeration_diagnostics() -> Result<(), String> {
        let commands = [common(19, "cargo test -p xtask missing-filter")];
        let error = match validate_test_covered_by_with(
            "policy.toml",
            &commands,
            FilePolicyHost::Unix,
            |_| {
                Ok((
                    false,
                    "compiler stdout".to_string(),
                    "runner stderr".to_string(),
                ))
            },
        ) {
            Ok(_) => return Err("failed enumeration did not remain fail-closed".to_string()),
            Err(error) => error,
        };

        if error.contains("cargo test -p xtask missing-filter")
            && error.contains("compiler stdout")
            && error.contains("runner stderr")
        {
            Ok(())
        } else {
            Err(format!("enumeration diagnostics were lost: {error}"))
        }
    }

    #[test]
    fn test_covered_by_build_args_stop_before_the_list_harness() -> Result<(), String> {
        let args = [
            "test",
            "-p",
            "xtask",
            "some_filter",
            "--",
            "--list",
            "--format",
            "terse",
        ]
        .map(str::to_string);
        let build = build_args_before_list(&args)
            .ok_or("ordinary cargo test lost its separate build step")?;
        let expected = ["test", "-p", "xtask", "some_filter", "--no-run"].map(str::to_string);
        if build != expected {
            return Err(format!("build args drifted: {build:?}"));
        }
        let spawns = enumeration_spawns(&args);
        if spawns.len() != 2
            || spawns[0].args != expected
            || spawns[0].timeout != TEST_COVERED_BY_BUILD_TIMEOUT
            || spawns[1].args != args
            || spawns[1].timeout != TEST_COVERED_BY_LIST_TIMEOUT
        {
            return Err(
                "ordinary pointer no longer warms with --no-run before the list cap".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn test_covered_by_doc_tests_do_not_append_no_run() -> Result<(), String> {
        let args = [
            "test",
            "--workspace",
            "--doc",
            "--",
            "--list",
            "--format",
            "terse",
        ]
        .map(str::to_string);
        if build_args_before_list(&args).is_none() {
            Ok(())
        } else {
            Err("cargo test --doc must not be combined with --no-run".to_string())
        }
    }

    #[test]
    fn test_covered_by_doc_pointer_is_enumerated_once_under_the_build_budget() -> Result<(), String>
    {
        let args = [
            "test",
            "--workspace",
            "--doc",
            "--",
            "--list",
            "--format",
            "terse",
        ]
        .map(str::to_string);
        let spawns = enumeration_spawns(&args);
        if spawns.len() != 1 {
            return Err(format!(
                "doc pointer spawned twice, which recompiles doctests ({})",
                spawns.len()
            ));
        }
        if spawns[0].timeout != TEST_COVERED_BY_BUILD_TIMEOUT {
            return Err(
                "doc enumeration was charged against the list cap instead of the build budget"
                    .to_string(),
            );
        }
        if spawns[0].args.iter().any(|arg| arg == "--no-run") {
            return Err("doc enumeration appended --no-run, which Cargo rejects".to_string());
        }
        if spawns[0].args != args {
            return Err("doc enumeration changed the list command".to_string());
        }
        if spawns[0].description != "test-valued covered_by doc enumeration" {
            return Err("doc enumeration is no longer the spawn that counts tests".to_string());
        }
        Ok(())
    }

    #[test]
    fn test_covered_by_instrument_timeout_is_not_an_unresolved_pointer() -> Result<(), String> {
        let commands = [common(4, "cargo test -p xtask slow_filter")];
        let error = match validate_test_covered_by_with(
            "policy.toml",
            &commands,
            FilePolicyHost::Unix,
            |_| {
                Ok((
                    false,
                    String::new(),
                    format!("{COVERED_BY_INSTRUMENT_PREFIX} timed out after 300s"),
                ))
            },
        ) {
            Ok(_) => {
                return Err("instrument timeout unexpectedly passed".to_string());
            }
            Err(error) => error,
        };
        if error.contains("not an unresolved pointer") && !error.contains("selects zero tests") {
            Ok(())
        } else {
            Err(format!(
                "instrument timeout was reported as an unresolved pointer: {error}"
            ))
        }
    }

    #[test]
    fn test_covered_by_zero_tests_stay_unresolved_and_slow_success_passes() -> Result<(), String> {
        let commands = [common(9, "cargo test -p xtask missing_pointer")];
        let unresolved = match validate_test_covered_by_with(
            "policy.toml",
            &commands,
            FilePolicyHost::Unix,
            |_| Ok((true, String::new(), String::new())),
        ) {
            Ok(_) => return Err("zero-test enumeration unexpectedly passed".to_string()),
            Err(error) => error,
        };
        if !unresolved.contains("selects zero tests") || unresolved.contains("instrument timeout") {
            return Err(format!(
                "unresolved pointer was not kept distinct: {unresolved}"
            ));
        }
        let slow_but_complete =
            validate_test_covered_by_with("policy.toml", &commands, FilePolicyHost::Unix, |_| {
                Ok((true, "slow_case: test\n".to_string(), String::new()))
            });
        if slow_but_complete.is_ok() {
            Ok(())
        } else {
            Err(format!(
                "a completed enumeration failed only because it was slow: {slow_but_complete:?}"
            ))
        }
    }
}
