//! Source-trusted validation of one exact reviewed source/W7 tree.
//!
//! This module deliberately runs from the source-parent `xtask` binary. It
//! materializes the reviewed tree through a deterministic, unreferenced Git
//! object, executes the source-owned governance catalog in a disposable clean
//! worktree, retains complete command evidence, and never creates or moves an
//! authoritative ref.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PREFLIGHT_SCHEMA: &str = "ripr.source_promotion_preflight.v1";
const RESOLUTION_SCHEMA: &str = "ripr.source_promotion_resolution.v1";
const RECEIPT_SCHEMA: &str = "ripr.source_promotion_resolved_tree_validation.v1";
const REPORT_JSON: &str = "resolved-tree-validation.json";
const REPORT_MD: &str = "resolved-tree-validation.md";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(180);

const REQUIRED_COMMANDS: &[&str] = &[
    "check-network-policy",
    "check-process-policy",
    "check-workflows",
    "check-file-policy",
    "check-dependencies",
    "check-generated-clean",
    "check-executable-files",
    "check-command-catalog",
    "check-spec-format",
    "check-traceability",
    "check-doc-artifacts",
    "check-public-api",
    "check-architecture",
];

#[derive(Clone, Debug)]
struct Options {
    repo: PathBuf,
    source_parent: String,
    swarm_parent: String,
    reviewed_tree: String,
    preflight: PathBuf,
    preflight_sha256: String,
    resolution_manifest: PathBuf,
    resolution_sha256: String,
    out: PathBuf,
}

#[derive(Debug)]
struct ValidationState {
    preflight_path: Option<String>,
    resolution_path: Option<String>,
    trusted_checker_sha: Option<String>,
    trusted_checker_executable: Option<String>,
    materialized_tree: Option<String>,
    disposable_commit: Option<String>,
    materialized_checkout: Option<String>,
    materialized_checkout_removed: bool,
    refs_before_sha256: Option<String>,
    refs_after_sha256: Option<String>,
    ref_mutation_observed: bool,
    commands: Vec<Value>,
    failure_reasons: Vec<String>,
}

impl ValidationState {
    fn new() -> Self {
        Self {
            preflight_path: None,
            resolution_path: None,
            trusted_checker_sha: None,
            trusted_checker_executable: None,
            materialized_tree: None,
            disposable_commit: None,
            materialized_checkout: None,
            materialized_checkout_removed: false,
            refs_before_sha256: None,
            refs_after_sha256: None,
            ref_mutation_observed: false,
            commands: REQUIRED_COMMANDS
                .iter()
                .map(|command| command_receipt(command, "not_run", None, 0, None, None, None))
                .collect(),
            failure_reasons: Vec::new(),
        }
    }
}

pub(crate) fn source_promotion_validate_resolved_tree(args: &[String]) -> Result<(), String> {
    let out = output_path_from_args(args)
        .unwrap_or_else(|| PathBuf::from("target/ripr/source-promotion/resolved-tree"));
    let options = match parse_args(args) {
        Ok(options) => options,
        Err(reason) => {
            let report = minimal_rejection_report(args, &reason);
            write_report(&out, &report)?;
            return Err(reason);
        }
    };

    let mut state = ValidationState::new();
    let validation_result = validate(&options, &mut state);
    let status = if validation_result.is_ok() {
        "validated"
    } else {
        "rejected"
    };
    if let Err(reason) = &validation_result
        && !state.failure_reasons.iter().any(|existing| existing == reason)
    {
        state.failure_reasons.push(reason.clone());
    }
    let report = report_value(&options, &state, status);
    let write_result = write_report(&options.out, &report);
    match (validation_result, write_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(reason), Ok(())) => Err(reason),
        (Ok(()), Err(write_error)) => Err(write_error),
        (Err(reason), Err(write_error)) => Err(format!(
            "{reason}; failed to write resolved-tree validation receipt: {write_error}"
        )),
    }
}

fn validate(options: &Options, state: &mut ValidationState) -> Result<(), String> {
    let preflight = read_bound_json(
        &options.repo,
        &options.preflight,
        &options.preflight_sha256,
        "preflight",
    )?;
    state.preflight_path = Some(preflight.1.display().to_string());
    validate_preflight(&preflight.0, options)?;

    let resolution = read_bound_json(
        &options.repo,
        &options.resolution_manifest,
        &options.resolution_sha256,
        "resolution manifest",
    )?;
    state.resolution_path = Some(resolution.1.display().to_string());
    validate_resolution_manifest(&resolution.0, options)?;

    verify_exact_commit(&options.repo, &options.source_parent, "--source-parent")?;
    verify_exact_commit(&options.repo, &options.swarm_parent, "--swarm-parent")?;
    verify_exact_tree(&options.repo, &options.reviewed_tree)?;

    let live_head = git(&options.repo, &["rev-parse", "HEAD"])?;
    if live_head.trim() != options.source_parent {
        return Err(format!(
            "trusted checker checkout HEAD {} does not equal exact source parent {}",
            live_head.trim(),
            options.source_parent
        ));
    }
    state.trusted_checker_sha = Some(options.source_parent.clone());

    let checker = std::env::current_exe()
        .map_err(|error| format!("failed to identify source-trusted xtask executable: {error}"))?;
    let checker_metadata = fs::metadata(&checker).map_err(|error| {
        format!(
            "failed to inspect source-trusted xtask executable {}: {error}",
            checker.display()
        )
    })?;
    if !checker_metadata.is_file() {
        return Err(format!(
            "source-trusted xtask executable is not a regular file: {}",
            checker.display()
        ));
    }
    state.trusted_checker_executable = Some(checker.display().to_string());

    let refs_before = snapshot_refs(&options.repo)?;
    state.refs_before_sha256 = Some(digest_bytes(refs_before.as_bytes()));

    let mut materialized = MaterializedTree::create(options)?;
    state.materialized_tree = Some(options.reviewed_tree.clone());
    state.disposable_commit = Some(materialized.commit.clone());
    state.materialized_checkout = Some(materialized.root.display().to_string());

    let candidate_tree = git(&materialized.root, &["rev-parse", "HEAD^{tree}"])?;
    if candidate_tree.trim() != options.reviewed_tree {
        let reason = format!(
            "materialized checkout tree {} does not equal reviewed tree {}",
            candidate_tree.trim(),
            options.reviewed_tree
        );
        let _ = materialized.cleanup();
        return Err(reason);
    }
    let candidate_status = git(&materialized.root, &["status", "--porcelain=v1"])?;
    if !candidate_status.trim().is_empty() {
        let reason = format!(
            "materialized reviewed-tree checkout is not clean: {}",
            candidate_status.trim()
        );
        let _ = materialized.cleanup();
        return Err(reason);
    }

    let logs_dir = options.out.join("commands");
    fs::create_dir_all(&logs_dir)
        .map_err(|error| format!("failed to create command evidence directory: {error}"))?;

    for (index, command) in REQUIRED_COMMANDS.iter().enumerate() {
        let receipt = run_required_command(
            &checker,
            &materialized.root,
            command,
            index,
            &logs_dir,
            &options.source_parent,
        );
        let passed = receipt
            .get("state")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "passed");
        state.commands[index] = receipt;
        if !passed {
            let reason = state.commands[index]
                .get("failure_reason")
                .and_then(Value::as_str)
                .unwrap_or("required command did not pass")
                .to_string();
            state.failure_reasons.push(format!("{command}: {reason}"));
            break;
        }
    }

    let cleanup_result = materialized.cleanup();
    state.materialized_checkout_removed = cleanup_result.is_ok();
    if let Err(reason) = cleanup_result {
        state.failure_reasons.push(reason);
    }

    let refs_after = snapshot_refs(&options.repo)?;
    state.refs_after_sha256 = Some(digest_bytes(refs_after.as_bytes()));
    if refs_before != refs_after {
        state.ref_mutation_observed = true;
        state
            .failure_reasons
            .push("repository refs changed during resolved-tree validation".to_string());
    }

    let all_passed = state.commands.iter().all(|receipt| {
        receipt
            .get("state")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "passed")
    });
    if !all_passed || !state.failure_reasons.is_empty() || state.ref_mutation_observed {
        return Err(state
            .failure_reasons
            .first()
            .cloned()
            .unwrap_or_else(|| "resolved-tree validation rejected".to_string()));
    }
    Ok(())
}

struct MaterializedTree {
    source_repo: PathBuf,
    parent: PathBuf,
    root: PathBuf,
    commit: String,
    cleaned: bool,
}

impl MaterializedTree {
    fn create(options: &Options) -> Result<Self, String> {
        let parent = unique_temp_path("ripr-resolved-tree-validation")?;
        let root = parent.join("tree");
        fs::create_dir_all(&parent).map_err(|error| {
            format!(
                "failed to create materialization directory {}: {error}",
                parent.display()
            )
        })?;

        let mut commit_command = git_command(&options.repo);
        commit_command
            .args([
                "commit-tree",
                options.reviewed_tree.as_str(),
                "-p",
                options.source_parent.as_str(),
                "-m",
                "ripr resolved-tree validation materialization",
            ])
            .env("GIT_AUTHOR_NAME", "ripr resolved-tree validator")
            .env("GIT_AUTHOR_EMAIL", "ripr-validator@invalid")
            .env("GIT_AUTHOR_DATE", "1970-01-01T00:00:00Z")
            .env("GIT_COMMITTER_NAME", "ripr resolved-tree validator")
            .env("GIT_COMMITTER_EMAIL", "ripr-validator@invalid")
            .env("GIT_COMMITTER_DATE", "1970-01-01T00:00:00Z");
        let commit_output = commit_command
            .output()
            .map_err(|error| format!("failed to create disposable materialization object: {error}"))?;
        if !commit_output.status.success() {
            let stderr = String::from_utf8_lossy(&commit_output.stderr);
            let _ = fs::remove_dir_all(&parent);
            return Err(format!(
                "failed to create disposable materialization object: {}",
                stderr.trim()
            ));
        }
        let commit = String::from_utf8_lossy(&commit_output.stdout)
            .trim()
            .to_string();
        validate_exact_hex("disposable materialization commit", &commit, 40)?;

        let root_text = root.to_string_lossy().into_owned();
        let worktree_output = git_command(&options.repo)
            .args(["worktree", "add", "--detach", &root_text, &commit])
            .output()
            .map_err(|error| format!("failed to materialize reviewed tree: {error}"))?;
        if !worktree_output.status.success() {
            let stderr = String::from_utf8_lossy(&worktree_output.stderr);
            let _ = fs::remove_dir_all(&parent);
            return Err(format!(
                "failed to materialize reviewed tree: {}",
                stderr.trim()
            ));
        }

        Ok(Self {
            source_repo: options.repo.clone(),
            parent,
            root,
            commit,
            cleaned: false,
        })
    }

    fn cleanup(&mut self) -> Result<(), String> {
        if self.cleaned {
            return Ok(());
        }
        let root_text = self.root.to_string_lossy().into_owned();
        let remove_output = git_command(&self.source_repo)
            .args(["worktree", "remove", "--force", &root_text])
            .output()
            .map_err(|error| format!("failed to remove disposable worktree: {error}"))?;
        if !remove_output.status.success() {
            let stderr = String::from_utf8_lossy(&remove_output.stderr);
            return Err(format!(
                "failed to remove disposable worktree: {}",
                stderr.trim()
            ));
        }
        fs::remove_dir_all(&self.parent).map_err(|error| {
            format!(
                "failed to remove materialization directory {}: {error}",
                self.parent.display()
            )
        })?;
        self.cleaned = true;
        Ok(())
    }
}

impl Drop for MaterializedTree {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

fn run_required_command(
    checker: &Path,
    root: &Path,
    command: &str,
    index: usize,
    logs_dir: &Path,
    source_parent: &str,
) -> Value {
    let stdout_path = logs_dir.join(format!("{:02}-{command}.stdout.log", index + 1));
    let stderr_path = logs_dir.join(format!("{:02}-{command}.stderr.log", index + 1));
    let stdout = match File::create(&stdout_path) {
        Ok(file) => file,
        Err(error) => {
            return command_receipt(
                command,
                "unavailable",
                None,
                0,
                Some(&stdout_path),
                Some(&stderr_path),
                Some(&format!("failed to create stdout evidence file: {error}")),
            );
        }
    };
    let stderr = match File::create(&stderr_path) {
        Ok(file) => file,
        Err(error) => {
            return command_receipt(
                command,
                "unavailable",
                None,
                0,
                Some(&stdout_path),
                Some(&stderr_path),
                Some(&format!("failed to create stderr evidence file: {error}")),
            );
        }
    };

    let started = Instant::now();
    let mut child = match Command::new(checker)
        .arg(command)
        .current_dir(root)
        .env("RIPR_SOURCE_PROMOTION_TRUSTED_CHECKER_SHA", source_parent)
        .env("RIPR_SOURCE_PROMOTION_VALIDATION", "1")
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return command_receipt(
                command,
                "unavailable",
                None,
                duration_ms(started.elapsed()),
                Some(&stdout_path),
                Some(&stderr_path),
                Some(&format!("failed to start source-trusted command: {error}")),
            );
        }
    };

    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if started.elapsed() >= COMMAND_TIMEOUT => {
                timed_out = true;
                let _ = child.kill();
                break child.wait().ok();
            }
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return command_receipt(
                    command,
                    "failed",
                    None,
                    duration_ms(started.elapsed()),
                    Some(&stdout_path),
                    Some(&stderr_path),
                    Some(&format!("failed while waiting for command: {error}")),
                );
            }
        }
    };

    let elapsed = duration_ms(started.elapsed());
    if timed_out {
        return command_receipt(
            command,
            "failed",
            status.as_ref().and_then(ExitStatus::code),
            elapsed,
            Some(&stdout_path),
            Some(&stderr_path),
            Some("command exceeded the 180 second validation bound"),
        );
    }
    match status {
        Some(status) if status.success() => command_receipt(
            command,
            "passed",
            status.code(),
            elapsed,
            Some(&stdout_path),
            Some(&stderr_path),
            None,
        ),
        Some(status) => command_receipt(
            command,
            "failed",
            status.code(),
            elapsed,
            Some(&stdout_path),
            Some(&stderr_path),
            Some(&format!("command exited with {status}")),
        ),
        None => command_receipt(
            command,
            "failed",
            None,
            elapsed,
            Some(&stdout_path),
            Some(&stderr_path),
            Some("command ended without an exit status"),
        ),
    }
}

fn command_receipt(
    command: &str,
    state: &str,
    exit_code: Option<i32>,
    duration_ms: u64,
    stdout_path: Option<&Path>,
    stderr_path: Option<&Path>,
    failure_reason: Option<&str>,
) -> Value {
    serde_json::json!({
        "command": command,
        "state": state,
        "exit_code": exit_code,
        "duration_ms": duration_ms,
        "stdout_path": stdout_path.map(|path| path.display().to_string()),
        "stderr_path": stderr_path.map(|path| path.display().to_string()),
        "failure_reason": failure_reason,
    })
}

fn report_value(options: &Options, state: &ValidationState, status: &str) -> Value {
    serde_json::json!({
        "schema": RECEIPT_SCHEMA,
        "tool_version": env!("CARGO_PKG_VERSION"),
        "status": status,
        "source_parent": options.source_parent,
        "swarm_parent": options.swarm_parent,
        "reviewed_tree": options.reviewed_tree,
        "preflight": {
            "path": state.preflight_path,
            "sha256": options.preflight_sha256,
        },
        "resolution_manifest": {
            "path": state.resolution_path,
            "sha256": options.resolution_sha256,
        },
        "trusted_checker_sha": state.trusted_checker_sha,
        "trusted_checker_executable": state.trusted_checker_executable,
        "materialized_checkout": {
            "tree": state.materialized_tree,
            "disposable_commit": state.disposable_commit,
            "path": state.materialized_checkout,
            "removed": state.materialized_checkout_removed,
            "authoritative": false,
        },
        "required_command_catalog": REQUIRED_COMMANDS,
        "commands": state.commands,
        "ref_observation": {
            "before_sha256": state.refs_before_sha256,
            "after_sha256": state.refs_after_sha256,
            "mutation_observed": state.ref_mutation_observed,
        },
        "authoritative_commit_attempted": false,
        "branch_attempted": false,
        "tag_attempted": false,
        "push_attempted": false,
        "ref_mutation_attempted": false,
        "failure_reasons": state.failure_reasons,
        "invalidation_rules": [
            "Changing the exact source parent, W7 parent, reviewed tree, preflight bytes, resolution-manifest bytes, trusted checker identity, required-command catalog, or receipt schema invalidates this validation.",
            "A failed, unavailable, or not_run required command rejects construction eligibility.",
            "Any observed or attempted authoritative ref mutation rejects construction eligibility.",
        ],
        "non_claims": [
            "The disposable one-parent commit exists only to provide clean Git worktree semantics for the reviewed tree; it is not J, a release object, a branch, or publication authority.",
            "This receipt proves only the named source-governed repository contracts on one exact reviewed tree.",
            "It does not prove product correctness, editor journeys, release readiness, merge eligibility beyond the named contracts, or publication authority.",
        ],
    })
}

fn minimal_rejection_report(args: &[String], reason: &str) -> Value {
    serde_json::json!({
        "schema": RECEIPT_SCHEMA,
        "tool_version": env!("CARGO_PKG_VERSION"),
        "status": "rejected",
        "source_parent": value_after(args, "--source-parent"),
        "swarm_parent": value_after(args, "--swarm-parent"),
        "reviewed_tree": value_after(args, "--reviewed-tree"),
        "preflight": {
            "path": value_after(args, "--preflight"),
            "sha256": value_after(args, "--preflight-sha256"),
        },
        "resolution_manifest": {
            "path": value_after(args, "--resolution-manifest"),
            "sha256": value_after(args, "--resolution-sha256"),
        },
        "trusted_checker_sha": null,
        "trusted_checker_executable": null,
        "materialized_checkout": null,
        "required_command_catalog": REQUIRED_COMMANDS,
        "commands": REQUIRED_COMMANDS.iter().map(|command| {
            command_receipt(command, "not_run", None, 0, None, None, None)
        }).collect::<Vec<_>>(),
        "authoritative_commit_attempted": false,
        "branch_attempted": false,
        "tag_attempted": false,
        "push_attempted": false,
        "ref_mutation_attempted": false,
        "failure_reasons": [reason],
        "invalidation_rules": ["Correct the exact inputs and generate a fresh receipt."],
        "non_claims": ["A rejected receipt provides no construction or publication authority."],
    })
}

fn write_report(out: &Path, report: &Value) -> Result<(), String> {
    fs::create_dir_all(out)
        .map_err(|error| format!("failed to create {}: {error}", out.display()))?;
    let json = serde_json::to_string_pretty(report)
        .map_err(|error| format!("failed to serialize resolved-tree receipt: {error}"))?;
    let markdown = render_markdown(report)?;
    fs::write(out.join(REPORT_JSON), format!("{json}\n"))
        .map_err(|error| format!("failed to write resolved-tree JSON receipt: {error}"))?;
    fs::write(out.join(REPORT_MD), markdown)
        .map_err(|error| format!("failed to write resolved-tree Markdown receipt: {error}"))?;
    println!("Wrote {}", out.join(REPORT_JSON).display());
    println!("Wrote {}", out.join(REPORT_MD).display());
    Ok(())
}

fn render_markdown(report: &Value) -> Result<String, String> {
    let structured = serde_json::to_string_pretty(report)
        .map_err(|error| format!("failed to serialize structured receipt: {error}"))?;
    let schema = string_field(report, "schema")?;
    let status = string_field(report, "status")?;
    let source = report
        .get("source_parent")
        .and_then(Value::as_str)
        .unwrap_or("not available");
    let swarm = report
        .get("swarm_parent")
        .and_then(Value::as_str)
        .unwrap_or("not available");
    let tree = report
        .get("reviewed_tree")
        .and_then(Value::as_str)
        .unwrap_or("not available");
    Ok(format!(
        "# Resolved-tree validation\n\n- Schema: `{schema}`\n- Status: **{status}**\n- SOURCE_PARENT: `{source}`\n- SWARM_PARENT: `{swarm}`\n- REVIEWED_TREE: `{tree}`\n\n## Claim boundary\n\nThis source-trusted receipt reports the named repository-governance commands on one exact reviewed tree. It does not construct J, move an authoritative ref, qualify product/editor behavior, or authorize publication.\n\n## Structured receipt\n\n```json\n{structured}\n```\n"
    ))
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    if args.first().map(String::as_str) != Some("validate-resolved-tree") {
        return Err(usage());
    }
    let mut values = BTreeMap::<&str, String>::new();
    let mut index = 1;
    while index < args.len() {
        let key = args[index].as_str();
        if !key.starts_with("--") || index + 1 >= args.len() {
            return Err(usage());
        }
        if !matches!(
            key,
            "--source-parent"
                | "--swarm-parent"
                | "--reviewed-tree"
                | "--preflight"
                | "--preflight-sha256"
                | "--resolution-manifest"
                | "--resolution-sha256"
                | "--out"
        ) {
            return Err(format!("unknown option {key}\n{}", usage()));
        }
        if values.insert(key, args[index + 1].clone()).is_some() {
            return Err(format!("duplicate option {key}"));
        }
        index += 2;
    }
    let required = |key: &str| {
        values
            .get(key)
            .cloned()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| format!("missing {key}\n{}", usage()))
    };
    let source_parent = required("--source-parent")?;
    let swarm_parent = required("--swarm-parent")?;
    let reviewed_tree = required("--reviewed-tree")?;
    let preflight_sha256 = required("--preflight-sha256")?;
    let resolution_sha256 = required("--resolution-sha256")?;
    validate_exact_hex("--source-parent", &source_parent, 40)?;
    validate_exact_hex("--swarm-parent", &swarm_parent, 40)?;
    validate_exact_hex("--reviewed-tree", &reviewed_tree, 40)?;
    validate_exact_hex("--preflight-sha256", &preflight_sha256, 64)?;
    validate_exact_hex("--resolution-sha256", &resolution_sha256, 64)?;
    let repo = std::env::current_dir()
        .map_err(|error| format!("failed to read current repository directory: {error}"))?;
    let out = values
        .get("--out")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/ripr/source-promotion/resolved-tree"));
    Ok(Options {
        repo,
        source_parent,
        swarm_parent,
        reviewed_tree,
        preflight: PathBuf::from(required("--preflight")?),
        preflight_sha256,
        resolution_manifest: PathBuf::from(required("--resolution-manifest")?),
        resolution_sha256,
        out,
    })
}

fn usage() -> String {
    "usage: cargo xtask source-promotion validate-resolved-tree --source-parent <40-char-sha> --swarm-parent <40-char-sha> --reviewed-tree <40-char-tree> --preflight <path> --preflight-sha256 <64-char-digest> --resolution-manifest <path> --resolution-sha256 <64-char-digest> [--out <dir>]".to_string()
}

fn output_path_from_args(args: &[String]) -> Option<PathBuf> {
    value_after(args, "--out").map(PathBuf::from)
}

fn value_after(args: &[String], key: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == key && !pair[1].trim().is_empty())
        .map(|pair| pair[1].clone())
}

fn read_bound_json(
    repo: &Path,
    path: &Path,
    expected_digest: &str,
    label: &str,
) -> Result<(Value, PathBuf), String> {
    reject_parent_components(path, label)?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo.join(path)
    };
    let metadata = fs::symlink_metadata(&candidate)
        .map_err(|error| format!("failed to inspect {label} {}: {error}", candidate.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("{label} must be a non-symlink regular file"));
    }
    let canonical_repo = repo
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize repository root: {error}"))?;
    let canonical = candidate
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize {label}: {error}"))?;
    if !canonical.starts_with(&canonical_repo) {
        return Err(format!("{label} escapes the source checkout"));
    }
    let bytes = fs::read(&canonical)
        .map_err(|error| format!("failed to read {label} {}: {error}", canonical.display()))?;
    let actual_digest = digest_bytes(&bytes);
    if actual_digest != expected_digest {
        return Err(format!(
            "{label} SHA-256 mismatch: expected {expected_digest}, observed {actual_digest}"
        ));
    }
    let value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("malformed {label} JSON: {error}"))?;
    Ok((value, canonical))
}

fn reject_parent_components(path: &Path, label: &str) -> Result<(), String> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!("{label} path contains a parent-directory escape"));
    }
    Ok(())
}

fn validate_preflight(preflight: &Value, options: &Options) -> Result<(), String> {
    if string_field(preflight, "schema")? != PREFLIGHT_SCHEMA {
        return Err("unsupported preflight schema".to_string());
    }
    if string_field(preflight, "source_parent")? != options.source_parent {
        return Err("preflight source parent does not match exact input".to_string());
    }
    if string_field(preflight, "swarm_parent")? != options.swarm_parent {
        return Err("preflight swarm parent does not match exact input".to_string());
    }
    let dry_merge = object_field(preflight, "dry_merge")?;
    if dry_merge
        .get("reviewed_resolved_tree")
        .and_then(Value::as_str)
        != Some(options.reviewed_tree.as_str())
    {
        return Err("preflight reviewed tree does not match exact input".to_string());
    }
    if dry_merge
        .get("reviewed_resolved_tree_verified")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err("preflight reviewed tree is not verified".to_string());
    }
    Ok(())
}

fn validate_resolution_manifest(manifest: &Value, options: &Options) -> Result<(), String> {
    if string_field(manifest, "schema")? != RESOLUTION_SCHEMA {
        return Err("unsupported resolution-manifest schema".to_string());
    }
    if string_field(manifest, "source_parent")? != options.source_parent {
        return Err("resolution source parent does not match exact input".to_string());
    }
    if string_field(manifest, "swarm_parent")? != options.swarm_parent {
        return Err("resolution swarm parent does not match exact input".to_string());
    }
    if string_field(manifest, "reviewed_join_tree")? != options.reviewed_tree {
        return Err("resolution reviewed tree does not match exact input".to_string());
    }
    let dispositions = manifest
        .get("dispositions")
        .and_then(Value::as_array)
        .ok_or_else(|| "resolution manifest is missing dispositions".to_string())?;
    if dispositions.is_empty() {
        return Err("resolution manifest dispositions must not be empty".to_string());
    }
    Ok(())
}

fn verify_exact_commit(repo: &Path, value: &str, label: &str) -> Result<(), String> {
    let resolved = git(repo, &["rev-parse", "--verify", &format!("{value}^{{commit}}")])?;
    if resolved.trim() != value {
        return Err(format!("{label} is not an exact commit object"));
    }
    Ok(())
}

fn verify_exact_tree(repo: &Path, value: &str) -> Result<(), String> {
    let resolved = git(repo, &["rev-parse", "--verify", &format!("{value}^{{tree}}")])?;
    if resolved.trim() != value {
        return Err("--reviewed-tree is not an exact tree object".to_string());
    }
    Ok(())
}

fn snapshot_refs(repo: &Path) -> Result<String, String> {
    git(
        repo,
        &[
            "for-each-ref",
            "--sort=refname",
            "--format=%(refname)%00%(objectname)",
            "refs",
        ],
    )
}

fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = git_command(repo)
        .args(args)
        .output()
        .map_err(|error| format!("failed to run git {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed with {}: {}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn git_command(repo: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(repo);
    command
}

fn unique_temp_path(prefix: &str) -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock predates Unix epoch: {error}"))?
        .as_nanos();
    Ok(std::env::temp_dir().join(format!(
        "{prefix}-{}-{timestamp}",
        std::process::id()
    )))
}

fn duration_ms(duration: Duration) -> u64 {
    match u64::try_from(duration.as_millis()) {
        Ok(value) => value,
        Err(_) => u64::MAX,
    }
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn validate_exact_hex(label: &str, value: &str, length: usize) -> Result<(), String> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!(
            "{label} must be an exact {length}-character lowercase hexadecimal identity"
        ));
    }
    Ok(())
}

fn string_field<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|field| !field.trim().is_empty())
        .ok_or_else(|| format!("missing string field {key}"))
}

fn object_field<'a>(value: &'a Value, key: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("missing object field {key}"))
}

#[cfg(test)]
mod tests {
    use super::{REQUIRED_COMMANDS, validate_exact_hex};

    #[test]
    fn exact_identity_rejects_abbreviated_and_uppercase_values() {
        assert!(validate_exact_hex("sha", "abc123", 40).is_err());
        assert!(
            validate_exact_hex("sha", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", 40).is_err()
        );
        assert!(
            validate_exact_hex("sha", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 40).is_ok()
        );
    }

    #[test]
    fn required_command_catalog_is_complete_and_stable() {
        assert_eq!(REQUIRED_COMMANDS.len(), 13);
        assert_eq!(REQUIRED_COMMANDS.first().copied(), Some("check-network-policy"));
        assert_eq!(REQUIRED_COMMANDS.last().copied(), Some("check-architecture"));
    }
}
