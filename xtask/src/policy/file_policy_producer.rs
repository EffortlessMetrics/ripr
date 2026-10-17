//! Source-owned Cargo producers for resolved-tree file-policy enumeration.
//! Every reference still runs through Cargo inside the governed command bound.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::run::capture_output_in_dir_with_timeout_bounded;

pub(crate) const PRODUCER_OUTPUT_ENV: &str = "RIPR_FILE_POLICY_PRODUCER_OUTPUT";
const LIMIT: Duration = Duration::from_mins(5);
const STREAM_LIMIT: usize = 2 * 1024 * 1024;
const SCHEMA: &str = "ripr.file_policy_test_producer.v1";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Inputs {
    root: PathBuf,
    head: String,
    tree: String,
    lock_sha256: String,
    cargo_sha256: String,
    rustc_sha256: String,
    config_sha256: BTreeMap<PathBuf, String>,
    environment_sha256: String,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    compiler_record: Value,
    executable: PathBuf,
    executable_sha256: String,
    depfile: PathBuf,
    depfile_sha256: String,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    line: usize,
    command: String,
    actual_args: Vec<String>,
    selected_tests: usize,
    exit_code: i32,
    timed_out: bool,
    stdout_truncated: bool,
    stderr_truncated: bool,
    duration_ms: u64,
    stdout_path: String,
    stdout_sha256: String,
    stderr_path: String,
    stderr_sha256: String,
    compiler_graph_sha256: String,
    artifacts: Vec<Artifact>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Report {
    schema: String,
    mode: String,
    source_parent: String,
    subject_tree: String,
    checker_sha256: String,
    references_sha256: String,
    inputs: Inputs,
    subject_inputs: Inputs,
    cargo_version: String,
    rustc_version: String,
    metadata: Value,
    test_profile: Value,
    references: Vec<Reference>,
    complete: bool,
    failure_reason: Option<String>,
    failed_invocation: Option<Value>,
}

include!("file_policy_producer/identity.rs");

include!("file_policy_producer/artifacts.rs");
include!("file_policy_producer/retained.rs");

fn verify_artifact(artifact: &Artifact, deadline: Option<Instant>) -> Result<(), String> {
    if file_sha_before(&artifact.executable, deadline)? != artifact.executable_sha256
        || file_sha_before(&artifact.depfile, deadline)? != artifact.depfile_sha256
    {
        return Err("enumeration executable or depfile moved".to_string());
    }
    Ok(())
}

pub(crate) fn checker_digest(checker: &Path, deadline: Instant) -> Result<String, String> {
    file_sha_before(checker, Some(deadline))
}

pub(crate) fn enumerate(commands: &[(usize, String)]) -> Result<(), String> {
    let subject = std::env::current_dir().map_err(|error| error.to_string())?;
    let source_parent = std::env::var("RIPR_SOURCE_PROMOTION_TRUSTED_CHECKER_SHA")
        .map_err(|error| error.to_string())?;
    let output =
        std::env::var_os(PRODUCER_OUTPUT_ENV).ok_or("source-owned producer output is missing")?;
    let candidate = anchor().ok();
    produce(
        &subject,
        &source_parent,
        candidate.as_deref(),
        Path::new(&output),
        commands,
    )
}

fn produce(
    subject: &Path,
    source_parent: &str,
    candidate: Option<&Path>,
    output: &Path,
    commands: &[(usize, String)],
) -> Result<(), String> {
    let observed_files = ObservedFiles::start()?;
    let subject_inputs = inputs(subject)?;
    let subject_tree = subject_inputs.tree.clone();
    let (mode, initial) = choose_root(subject, source_parent, candidate)?;
    let root = initial.root.clone();
    let cargo = Path::new(env!("CARGO"));
    let metadata: Value = serde_json::from_str(&successful(
        cargo,
        &["metadata", "--format-version", "1", "--no-deps", "--locked"],
        &root,
    )?)
    .map_err(|error| error.to_string())?;
    let profile = test_profile(&initial)?;
    let mut report = Report {
        schema: SCHEMA.to_string(),
        mode,
        source_parent: source_parent.to_string(),
        subject_tree,
        checker_sha256: file_sha(&std::env::current_exe().map_err(|error| error.to_string())?)?,
        references_sha256: json_sha(&commands)?,
        inputs: initial,
        subject_inputs,
        cargo_version: successful(cargo, &["--version", "--verbose"], &root)?,
        rustc_version: successful(&compiler(), &["--version", "--verbose"], &root)?,
        test_profile: profile,
        metadata,
        references: Vec::new(),
        complete: false,
        failure_reason: None,
        failed_invocation: None,
    };
    let result = super::file_policy::validate_test_covered_by_with(
        "policy/non-rust-allowlist.toml",
        commands,
        |args| {
            if inputs(&root)? != report.inputs {
                return Err("source or Cargo inputs moved after producer selection".to_string());
            }
            let mut actual_args = args.to_vec();
            let separator = actual_args
                .iter()
                .position(|arg| arg == "--")
                .ok_or("enumeration arguments lack separator")?;
            actual_args.insert(separator, "--message-format=json".to_string());
            let captured = capture(cargo, &actual_args, &root)?;
            let index = report.references.len();
            let (line, command) = commands
                .get(index)
                .ok_or("unexpected producer invocation")?;
            let parent = output.parent().ok_or("producer evidence has no parent")?;
            let stdout_path = format!("file-policy-test-{:02}.stdout.log", index + 1);
            let stderr_path = format!("file-policy-test-{:02}.stderr.log", index + 1);
            write_new(&parent.join(&stdout_path), captured.stdout.as_bytes())?;
            write_new(&parent.join(&stderr_path), captured.stderr.as_bytes())?;
            report.failed_invocation = Some(
                serde_json::json!({"line": line, "command": command, "actual_args": actual_args,
            "exit_code": captured.status.as_ref().and_then(|status| status.code()), "timed_out": captured.timed_out,
            "stdout_truncated": captured.stdout_truncated, "stderr_truncated": captured.stderr_truncated,
            "duration_ms": captured.duration.as_millis(), "stdout_path": stdout_path, "stderr_path": stderr_path,
            "stdout_sha256": sha(captured.stdout.as_bytes()), "stderr_sha256": sha(captured.stderr.as_bytes())}),
            );
            if captured.timed_out
                || captured.stdout_truncated
                || captured.stderr_truncated
                || !captured.status.is_some_and(|status| status.success())
            {
                return Err(format!(
                    "actual Cargo enumeration failed; status={:?}; timeout={}; stdout={}; stderr={}",
                    captured.status, captured.timed_out, captured.stdout, captured.stderr
                ));
            }
            let (graph_sha, artifacts) = artifacts(
                &captured.stdout,
                &report.metadata,
                &root,
                command,
                &report.test_profile,
                None,
            )?;
            if inputs(&root)? != report.inputs {
                return Err("source or Cargo inputs moved during enumeration".to_string());
            }
            report.failed_invocation = None;
            report.references.push(Reference {
                line: *line,
                exit_code: captured
                    .status
                    .and_then(|status| status.code())
                    .ok_or("Cargo producer lacks exit code")?,
                timed_out: captured.timed_out,
                stdout_truncated: captured.stdout_truncated,
                stderr_truncated: captured.stderr_truncated,
                duration_ms: captured
                    .duration
                    .as_millis()
                    .try_into()
                    .map_err(|_| "producer duration is too large")?,
                command: command.clone(),
                actual_args,
                selected_tests: captured
                    .stdout
                    .lines()
                    .filter(|line| line.ends_with(": test"))
                    .count(),
                stdout_path,
                stdout_sha256: sha(captured.stdout.as_bytes()),
                stderr_path,
                stderr_sha256: sha(captured.stderr.as_bytes()),
                compiler_graph_sha256: graph_sha,
                artifacts,
            });
            Ok((true, captured.stdout, captured.stderr))
        },
    );
    let result = result.and_then(|()| {
        for reference in &report.references {
            for artifact in &reference.artifacts {
                verify_artifact(artifact, None)?;
            }
        }
        if inputs(&root)? != report.inputs || inputs(subject)? != report.subject_inputs {
            return Err("source inputs moved before producer completion".to_string());
        }
        Ok(())
    });
    let result = result.and_then(|()| observed_files.finish(None));
    report.complete = result.is_ok();
    report.failure_reason = result.as_ref().err().cloned();
    let bytes = serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?;
    write_new(output, &bytes)?;
    result
}

pub(crate) fn verify(
    output: &Path,
    subject: &Path,
    source_parent: &str,
    checker_sha: &str,
    commands: &[(usize, String)],
    deadline: Instant,
) -> Result<Value, String> {
    verify_with_anchor(
        output,
        subject,
        source_parent,
        checker_sha,
        commands,
        anchor().ok().as_deref(),
        deadline,
    )
}

fn verify_with_anchor(
    output: &Path,
    subject: &Path,
    source_parent: &str,
    checker_sha: &str,
    commands: &[(usize, String)],
    candidate: Option<&Path>,
    deadline: Instant,
) -> Result<Value, String> {
    let observed_files = ObservedFiles::start()?;
    remaining(Some(deadline))?;
    let bytes = read_regular(output, 8 * 1024 * 1024)?;
    let report: Report = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let (mode, expected) = choose_root_before(subject, source_parent, candidate, Some(deadline))?;
    if report.schema != SCHEMA
        || !report.complete
        || report.failure_reason.is_some()
        || report.failed_invocation.is_some()
        || report.source_parent != source_parent
        || report.subject_inputs != inputs_before(subject, Some(deadline))?
        || report.subject_tree != report.subject_inputs.tree
        || report.checker_sha256 != checker_sha
        || report.inputs != expected
        || report.mode != mode
        || report.references_sha256 != json_sha(&commands)?
        || report.references.len() != commands.len()
    {
        return Err(
            "file-policy producer identity, inputs or reference set is incompatible".to_string(),
        );
    }
    let observed_metadata: Value = serde_json::from_str(&successful_before(
        Path::new(env!("CARGO")),
        &["metadata", "--format-version", "1", "--no-deps", "--locked"],
        &expected.root,
        Some(deadline),
    )?)
    .map_err(|error| error.to_string())?;
    if report.test_profile != test_profile(&expected)?
        || report.metadata != observed_metadata
        || report.cargo_version
            != successful_before(
                Path::new(env!("CARGO")),
                &["--version", "--verbose"],
                &expected.root,
                Some(deadline),
            )?
        || report.rustc_version
            != successful_before(
                &compiler(),
                &["--version", "--verbose"],
                &expected.root,
                Some(deadline),
            )?
    {
        return Err(
            "producer metadata or toolchain identity differs from current context".to_string(),
        );
    }
    for (reference, (line, command)) in report.references.iter().zip(commands) {
        remaining(Some(deadline))?;
        if reference.line != *line
            || reference.command != *command
            || reference.exit_code != 0
            || reference.timed_out
            || reference.stdout_truncated
            || reference.stderr_truncated
            || reference.actual_args != actual_args(command)
            || reference.selected_tests == 0
            || reference.artifacts.is_empty()
        {
            return Err("file-policy producer is missing an actual nonzero reference".to_string());
        }
        let parent = output.parent().ok_or("producer evidence has no parent")?;
        for (name, digest) in [
            (&reference.stdout_path, &reference.stdout_sha256),
            (&reference.stderr_path, &reference.stderr_sha256),
        ] {
            if Path::new(name).components().count() != 1
                || file_sha_before(&parent.join(name), Some(deadline))? != *digest
            {
                return Err("file-policy producer command evidence moved".to_string());
            }
        }
        let stdout = String::from_utf8(read_regular(
            &parent.join(&reference.stdout_path),
            STREAM_LIMIT as u64,
        )?)
        .map_err(|error| error.to_string())?;
        let (graph_sha, observed_artifacts) = artifacts(
            &stdout,
            &report.metadata,
            &expected.root,
            command,
            &report.test_profile,
            Some(deadline),
        )?;
        if stdout
            .lines()
            .filter(|line| line.ends_with(": test"))
            .count()
            != reference.selected_tests
            || graph_sha != reference.compiler_graph_sha256
            || observed_artifacts != reference.artifacts
        {
            return Err(
                "file-policy producer output differs from actual artifact evidence".to_string(),
            );
        }
        for artifact in &reference.artifacts {
            verify_artifact(artifact, Some(deadline))?;
        }
    }
    if inputs_before(&expected.root, Some(deadline))? != expected
        || inputs_before(subject, Some(deadline))? != report.subject_inputs
    {
        return Err("producer inputs moved during independent verification".to_string());
    }
    observed_files.finish(Some(deadline))?;
    let summary = producer_summary(&report, &bytes);
    remaining(Some(deadline))?;
    Ok(summary)
}

pub(crate) fn summary_is_valid(summary: &Value, receipt: &Value) -> bool {
    const FIELDS: &[&str] = &[
        "schema",
        "mode",
        "subject_tree",
        "source_parent",
        "checker_sha256",
        "producer_root",
        "subject_head",
        "references_sha256",
        "reference_count",
        "path",
        "sha256",
        "complete",
    ];
    let hex = |value: &Value| {
        value.as_str().is_some_and(|text| {
            text.len() == 64
                && text
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
    };
    summary.as_object().is_some_and(|object| {
        object.len() == FIELDS.len() && FIELDS.iter().all(|field| object.contains_key(*field))
    }) && summary["schema"] == SCHEMA
        && summary["complete"] == true
        && matches!(
            summary["mode"].as_str(),
            Some("exact_source_parent_checkout" | "reviewed_tree_worktree")
        )
        && summary["subject_tree"] == receipt["reviewed_tree"]
        && summary["source_parent"] == receipt["source_parent"]
        && summary["producer_root"]
            .as_str()
            .is_some_and(|root| Path::new(root).is_absolute())
        && summary["subject_head"] == receipt["materialization"]["disposable_commit"]
        && summary["checker_sha256"] == receipt["trusted_checker"]["executable_sha256"]
        && summary["path"] == "commands/file-policy-test-producer.json"
        && summary["reference_count"]
            .as_u64()
            .is_some_and(|count| count > 0)
        && hex(&summary["sha256"])
        && hex(&summary["references_sha256"])
}

#[cfg(test)]
#[path = "file_policy_producer_tests.rs"]
mod tests;

#[cfg(test)]
pub(crate) fn retained_fixture(receipt: &Value) -> Result<(Value, Vec<(String, Vec<u8>)>), String> {
    tests::retained_fixture(receipt)
}
