use std::path::Path;
use std::time::Duration;

use crate::run::TimedOutput;
use crate::{
    FixKind, PolicyReportSpec, capture_output_with_timeout, collect_files, finish_policy_report,
    is_cargo_test_command, is_file_policy_candidate, is_non_rust_programming_candidate,
    matches_any_glob, non_rust_programming_retention_reason, normalize_path,
    read_file_policy_allowlist, read_file_policy_test_commands,
};

const TEST_COVERED_BY_ENUMERATION_TIMEOUT: Duration = Duration::from_mins(5);

/// Validate the repository's non-Rust file policy and write its standard
/// report. The parser and shared path predicates remain in `main.rs` until
/// their other policy/report consumers can move in a later slice.
pub(crate) fn check_file_policy() -> Result<(), String> {
    let policy_path = "policy/non-rust-allowlist.toml";
    let allowlist = read_file_policy_allowlist(policy_path)?;
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

    finish_policy_report(
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
    )
}

fn validate_test_covered_by(path: &str, commands: &[(usize, String)]) -> Result<(), String> {
    validate_test_covered_by_with(path, commands, |args| {
        let output = capture_output_with_timeout(
            "cargo",
            args,
            &[],
            TEST_COVERED_BY_ENUMERATION_TIMEOUT,
            "test-valued covered_by enumeration",
        )?;
        Ok(map_test_covered_by_enumeration_output(output))
    })
}

fn map_test_covered_by_enumeration_output(output: TimedOutput) -> (bool, String, String) {
    let status = output
        .status
        .map(|status| status.to_string())
        .unwrap_or_else(|| "not available".to_string());
    let timeout = if output.timed_out {
        format!(
            "timed out after {:?}; ",
            TEST_COVERED_BY_ENUMERATION_TIMEOUT
        )
    } else {
        String::new()
    };
    let stderr = format!("{timeout}status: {status}\n{}", output.stderr.trim_end());
    (
        output.status.is_some_and(|status| status.success()) && !output.timed_out,
        output.stdout,
        stderr,
    )
}

fn validate_test_covered_by_with(
    path: &str,
    commands: &[(usize, String)],
    mut enumerate: impl FnMut(&[String]) -> Result<(bool, String, String), String>,
) -> Result<(), String> {
    for (line, command) in commands {
        if !is_cargo_test_command(command) {
            return Err(format!(
                "{path}:{line} unsupported test-valued `covered_by`: {command}"
            ));
        }
        let words = command.split_whitespace().skip(2);
        let mut args = vec!["test".to_string()];
        args.extend(words.map(ToString::to_string));
        args.extend([
            "--".to_string(),
            "--list".to_string(),
            "--format".to_string(),
            "terse".to_string(),
        ]);
        let (success, stdout, stderr) = enumerate(&args)
            .map_err(|error| format!("{path}:{line} enumerate `{command}`: {error}"))?;
        if !success {
            return Err(format!(
                "{path}:{line} test-valued `covered_by` could not be enumerated: `{command}`\nstdout: {stdout}\nstderr: {stderr}"
            ));
        }
        let selected = stdout
            .lines()
            .filter(|line| line.ends_with(": test"))
            .count();
        if selected == 0 {
            return Err(format!(
                "{path}:{line} test-valued `covered_by` selects zero tests: `{command}`"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::map_test_covered_by_enumeration_output;
    use super::validate_test_covered_by;
    use super::validate_test_covered_by_with;
    use crate::is_cargo_test_command;
    use crate::run::TimedOutput;

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
            let (success, stdout, stderr) = map_test_covered_by_enumeration_output(output);
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
    fn test_covered_by_requires_nonzero_successful_enumeration() -> Result<(), String> {
        let commands = [(7, "cargo test -p xtask missing-filter".to_string())];
        let empty = validate_test_covered_by_with("policy.toml", &commands, |_| {
            Ok((true, String::new(), String::new()))
        });
        let failed = validate_test_covered_by_with("policy.toml", &commands, |_| {
            Ok((false, String::new(), "instrument failed".to_string()))
        });
        let nonzero = validate_test_covered_by_with("policy.toml", &commands, |_| {
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
        let commands = [(
            12,
            "cargo test -p xtask test_covered_by_classification_is_token_aware".to_string(),
        )];
        validate_test_covered_by("policy.toml", &commands)
    }

    #[test]
    fn test_covered_by_production_wrapper_preserves_cargo_stderr() -> Result<(), String> {
        let commands = [(
            17,
            "cargo test -p package-that-does-not-exist-3528".to_string(),
        )];
        let error = match validate_test_covered_by("policy.toml", &commands) {
            Ok(()) => return Err("failed Cargo enumeration unexpectedly passed".to_string()),
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
        let commands = [(19, "cargo test -p xtask missing-filter".to_string())];
        let error = match validate_test_covered_by_with("policy.toml", &commands, |_| {
            Ok((
                false,
                "compiler stdout".to_string(),
                "runner stderr".to_string(),
            ))
        }) {
            Ok(()) => return Err("failed enumeration did not remain fail-closed".to_string()),
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
}
