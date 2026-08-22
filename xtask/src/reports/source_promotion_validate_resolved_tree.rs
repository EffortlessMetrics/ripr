//! Source-trusted validation of one exact reviewed source/W7 tree.
//!
//! The command is compiled from the held source parent, validates the complete
//! preflight and resolution contracts, materializes the reviewed tree without
//! moving an authoritative ref, and executes the source-owned governance
//! catalog with bounded retained evidence.

use super::source_promotion_verify::{validate_manifest, validate_preflight};
use crate::run::{TimedBoundedOutput, capture_output_in_dir_with_timeout_bounded};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const RECEIPT_SCHEMA: &str = "ripr.source_promotion_resolved_tree_validation.v1";
const REPORT_JSON: &str = "resolved-tree-validation.json";
const REPORT_MD: &str = "resolved-tree-validation.md";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(180);
const GIT_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_STREAM_BYTES: usize = 2 * 1024 * 1024;
const TEMP_ATTEMPTS: u32 = 128;

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

#[derive(Clone, Debug, Default)]
struct InputEcho {
    source_parent: Option<String>,
    swarm_parent: Option<String>,
    reviewed_tree: Option<String>,
    preflight_path: Option<String>,
    preflight_sha256: Option<String>,
    resolution_path: Option<String>,
    resolution_sha256: Option<String>,
}

#[derive(Debug)]
struct ValidationState {
    inputs: InputEcho,
    preflight_verified: bool,
    resolution_verified: bool,
    checker_source_sha: Option<String>,
    checker_executable_sha256: Option<String>,
    materialized_tree: Option<String>,
    disposable_commit: Option<String>,
    materialization_created: bool,
    materialization_clean_before: bool,
    materialization_clean_after: bool,
    worktree_remove_succeeded: bool,
    worktree_prune_succeeded: bool,
    materialization_directory_removed: bool,
    worktree_residue_observed: bool,
    cleanup_failure_reason: Option<String>,
    refs_before_sha256: Option<String>,
    refs_after_sha256: Option<String>,
    worktrees_before_sha256: Option<String>,
    worktrees_after_sha256: Option<String>,
    ref_mutation_observed: bool,
    worktree_registry_changed: bool,
    commands: Vec<Value>,
    failure_reasons: Vec<String>,
}

impl ValidationState {
    fn new(inputs: InputEcho) -> Self {
        Self {
            inputs,
            preflight_verified: false,
            resolution_verified: false,
            checker_source_sha: None,
            checker_executable_sha256: None,
            materialized_tree: None,
            disposable_commit: None,
            materialization_created: false,
            materialization_clean_before: false,
            materialization_clean_after: false,
            worktree_remove_succeeded: false,
            worktree_prune_succeeded: false,
            materialization_directory_removed: false,
            worktree_residue_observed: false,
            cleanup_failure_reason: None,
            refs_before_sha256: None,
            refs_after_sha256: None,
            worktrees_before_sha256: None,
            worktrees_after_sha256: None,
            ref_mutation_observed: false,
            worktree_registry_changed: false,
            commands: REQUIRED_COMMANDS
                .iter()
                .map(|command| {
                    command_receipt(
                        command,
                        "not_run",
                        None,
                        Duration::ZERO,
                        false,
                        false,
                        Some("validation did not reach governed command execution"),
                    )
                })
                .collect(),
            failure_reasons: Vec::new(),
        }
    }
}

pub(crate) fn source_promotion_validate_resolved_tree(args: &[String]) -> Result<(), String> {
    let echo = input_echo(args);
    let out = output_path_from_args(args)
        .unwrap_or_else(|| PathBuf::from("target/ripr/source-promotion/resolved-tree"));
    let options = match parse_args(args) {
        Ok(options) => options,
        Err(reason) => {
            let mut state = ValidationState::new(echo);
            state.failure_reasons.push(reason.clone());
            write_report(&out, &report_value(&state, "rejected"))?;
            return Err(reason);
        }
    };

    let mut state = ValidationState::new(input_echo_from_options(&options));
    let validation = validate(&options, &mut state);
    if let Err(reason) = &validation {
        if !state.failure_reasons.iter().any(|existing| existing == reason) {
            state.failure_reasons.push(reason.clone());
        }
    }
    let status = if validation.is_ok() {
        "validated"
    } else {
        "rejected"
    };
    let write_result = write_report(&options.out, &report_value(&state, status));
    match (validation, write_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(reason), Ok(())) => Err(reason),
        (Ok(()), Err(write_error)) => Err(write_error),
        (Err(reason), Err(write_error)) => Err(format!(
            "{reason}; failed to write resolved-tree validation receipt: {write_error}"
        )),
    }
}

fn validate(options: &Options, state: &mut ValidationState) -> Result<(), String> {
    let (preflight, preflight_path) = read_bound_json(
        &options.repo,
        &options.preflight,
        &options.preflight_sha256,
        "preflight",
    )?;
    state.inputs.preflight_path = Some(preflight_path);
    validate_preflight(&preflight, &options.source_parent)?;
    if string_field(&preflight, "swarm_parent")? != options.swarm_parent {
        return Err("preflight swarm parent does not match exact input".to_string());
    }
    let preflight_tree = preflight
        .get("dry_merge")
        .and_then(|value| value.get("reviewed_resolved_tree"))
        .and_then(Value::as_str)
        .ok_or_else(|| "preflight is missing reviewed resolved tree".to_string())?;
    if preflight_tree != options.reviewed_tree {
        return Err("preflight reviewed tree does not match exact input".to_string());
    }
    state.preflight_verified = true;

    let (manifest, resolution_path) = read_bound_json(
        &options.repo,
        &options.resolution_manifest,
        &options.resolution_sha256,
        "resolution manifest",
    )?;
    state.inputs.resolution_path = Some(resolution_path);
    validate_manifest(
        &manifest,
        &preflight,
        &format!("sha256:{}", options.preflight_sha256),
    )?;
    state.resolution_verified = true;

    verify_exact_commit(&options.repo, &options.source_parent, "--source-parent")?;
    verify_exact_commit(&options.repo, &options.swarm_parent, "--swarm-parent")?;
    verify_exact_tree(&options.repo, &options.reviewed_tree)?;

    let live_head = git(&options.repo, &["rev-parse", "HEAD"], &[])?;
    if live_head.trim() != options.source_parent {
        return Err(format!(
            "validator checkout HEAD {} does not equal exact source parent {}",
            live_head.trim(),
            options.source_parent
        ));
    }

    let checker = std::env::current_exe()
        .map_err(|error| format!("failed to identify running xtask executable: {error}"))?;
    let checker_metadata = fs::metadata(&checker).map_err(|error| {
        format!(
            "failed to inspect running xtask executable {}: {error}",
            checker.display()
        )
    })?;
    if !checker_metadata.is_file() {
        return Err(format!(
            "running xtask executable is not a regular file: {}",
            checker.display()
        ));
    }
    state.checker_source_sha = Some(options.source_parent.clone());
    state.checker_executable_sha256 = Some(digest_file(&checker)?);

    let refs_before = snapshot_refs(&options.repo)?;
    let worktrees_before = snapshot_worktrees(&options.repo)?;
    state.refs_before_sha256 = Some(digest_bytes(refs_before.as_bytes()));
    state.worktrees_before_sha256 = Some(digest_bytes(worktrees_before.as_bytes()));

    let mut materialized = match MaterializedTree::create(options) {
        Ok(materialized) => materialized,
        Err(reason) => {
            observe_repository_after(options, state, &refs_before, &worktrees_before)?;
            return Err(reason);
        }
    };
    state.materialization_created = true;
    state.materialized_tree = Some(options.reviewed_tree.clone());
    state.disposable_commit = Some(materialized.commit.clone());

    let execution_result =
        validate_materialized_tree(options, state, &checker, &materialized.root);

    let cleanup = materialized.cleanup();
    state.worktree_remove_succeeded = cleanup.worktree_remove_succeeded;
    state.worktree_prune_succeeded = cleanup.worktree_prune_succeeded;
    state.materialization_directory_removed = cleanup.materialization_directory_removed;
    state.worktree_residue_observed = cleanup.worktree_residue_observed;
    state.cleanup_failure_reason = cleanup.failure_reason.clone();
    if let Some(reason) = cleanup.failure_reason {
        state.failure_reasons.push(reason);
    }

    observe_repository_after(options, state, &refs_before, &worktrees_before)?;

    execution_result?;
    if state.ref_mutation_observed {
        return Err("repository refs changed during resolved-tree validation".to_string());
    }
    if state.worktree_registry_changed || state.worktree_residue_observed {
        return Err("disposable worktree cleanup did not restore repository state".to_string());
    }
    if state.cleanup_failure_reason.is_some() {
        return Err("resolved-tree materialization cleanup failed".to_string());
    }
    if state.commands.iter().any(|command| {
        command
            .get("state")
            .and_then(Value::as_str)
            .is_none_or(|value| value != "passed")
    }) {
        return Err("one or more required governance commands did not pass".to_string());
    }
    Ok(())
}

fn validate_materialized_tree(
    options: &Options,
    state: &mut ValidationState,
    checker: &Path,
    root: &Path,
) -> Result<(), String> {
    let candidate_tree = git(root, &["rev-parse", "HEAD^{tree}"], &[])?;
    if candidate_tree.trim() != options.reviewed_tree {
        return Err(format!(
            "materialized checkout tree {} does not equal reviewed tree {}",
            candidate_tree.trim(),
            options.reviewed_tree
        ));
    }
    let candidate_status = git(root, &["status", "--porcelain=v1"], &[])?;
    if !candidate_status.trim().is_empty() {
        return Err(format!(
            "materialized reviewed-tree checkout is not clean before validation: {}",
            candidate_status.trim()
        ));
    }
    state.materialization_clean_before = true;

    let logs_dir = options.out.join("commands");
    fs::create_dir_all(&logs_dir)
        .map_err(|error| format!("failed to create command evidence directory: {error}"))?;

    let mut prior_failure: Option<String> = None;
    for (index, command) in REQUIRED_COMMANDS.iter().enumerate() {
        if let Some(failed_command) = &prior_failure {
            state.commands[index] = command_receipt(
                command,
                "not_run",
                None,
                Duration::ZERO,
                false,
                false,
                Some(&format!(
                    "not run because prior required command {failed_command} did not pass"
                )),
            );
            continue;
        }

        let receipt = run_required_command(
            checker,
            root,
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
            prior_failure = Some((*command).to_string());
            let reason = state.commands[index]
                .get("failure_reason")
                .and_then(Value::as_str)
                .unwrap_or("required command did not pass")
                .to_string();
            state.failure_reasons.push(format!("{command}: {reason}"));
        }
    }

    let final_status = git(root, &["status", "--porcelain=v1"], &[])?;
    if !final_status.trim().is_empty() {
        return Err(format!(
            "governance commands changed the reviewed-tree checkout: {}",
            final_status.trim()
        ));
    }
    state.materialization_clean_after = true;

    if let Some(command) = prior_failure {
        return Err(format!("required governance command {command} did not pass"));
    }
    Ok(())
}

fn observe_repository_after(
    options: &Options,
    state: &mut ValidationState,
    refs_before: &str,
    worktrees_before: &str,
) -> Result<(), String> {
    let refs_after = snapshot_refs(&options.repo)?;
    let worktrees_after = snapshot_worktrees(&options.repo)?;
    state.refs_after_sha256 = Some(digest_bytes(refs_after.as_bytes()));
    state.worktrees_after_sha256 = Some(digest_bytes(worktrees_after.as_bytes()));
    state.ref_mutation_observed = refs_before != refs_after;
    state.worktree_registry_changed = worktrees_before != worktrees_after;
    Ok(())
}

#[derive(Debug)]
struct MaterializedTree {
    source_repo: PathBuf,
    parent: PathBuf,
    root: PathBuf,
    commit: String,
    cleaned: bool,
}

#[derive(Debug, Default)]
struct CleanupResult {
    worktree_remove_succeeded: bool,
    worktree_prune_succeeded: bool,
    materialization_directory_removed: bool,
    worktree_residue_observed: bool,
    failure_reason: Option<String>,
}

impl MaterializedTree {
    fn create(options: &Options) -> Result<Self, String> {
        let parent =
            create_exclusive_temp_dir("ripr-resolved-tree-validation", &options.reviewed_tree)?;
        let root = parent.join("tree");

        let commit = git(
            &options.repo,
            &[
                "commit-tree",
                options.reviewed_tree.as_str(),
                "-p",
                options.source_parent.as_str(),
                "-m",
                "ripr resolved-tree validation materialization",
            ],
            &[
                ("GIT_AUTHOR_NAME", "ripr resolved-tree validator"),
                ("GIT_AUTHOR_EMAIL", "ripr-validator@invalid"),
                ("GIT_AUTHOR_DATE", "1970-01-01T00:00:00Z"),
                ("GIT_COMMITTER_NAME", "ripr resolved-tree validator"),
                ("GIT_COMMITTER_EMAIL", "ripr-validator@invalid"),
                ("GIT_COMMITTER_DATE", "1970-01-01T00:00:00Z"),
            ],
        )?;
        let commit = commit.trim().to_string();
        validate_exact_hex("disposable materialization commit", &commit, 40)?;

        let root_text = root.to_string_lossy().into_owned();
        if let Err(reason) = git(
            &options.repo,
            &["worktree", "add", "--detach", &root_text, &commit],
            &[],
        ) {
            let _ = fs::remove_dir_all(&parent);
            return Err(reason);
        }

        Ok(Self {
            source_repo: options.repo.clone(),
            parent,
            root,
            commit,
            cleaned: false,
        })
    }

    fn cleanup(&mut self) -> CleanupResult {
        if self.cleaned {
            return CleanupResult {
                worktree_remove_succeeded: true,
                worktree_prune_succeeded: true,
                materialization_directory_removed: true,
                worktree_residue_observed: false,
                failure_reason: None,
            };
        }

        let mut result = CleanupResult::default();
        let root_text = self.root.to_string_lossy().into_owned();
        result.worktree_remove_succeeded = git(
            &self.source_repo,
            &["worktree", "remove", "--force", &root_text],
            &[],
        )
        .is_ok();
        if !result.worktree_remove_succeeded {
            let _ = fs::remove_dir_all(&self.root);
        }
        result.worktree_prune_succeeded = git(
            &self.source_repo,
            &["worktree", "prune", "--expire", "now"],
            &[],
        )
        .is_ok();

        let worktrees = match snapshot_worktrees(&self.source_repo) {
            Ok(worktrees) => worktrees,
            Err(error) => {
                result.failure_reason = Some(error);
                String::new()
            }
        };
        result.worktree_residue_observed = worktrees.contains(&root_text);

        result.materialization_directory_removed = if self.parent.exists() {
            fs::remove_dir_all(&self.parent).is_ok()
        } else {
            true
        };

        let mut failures = Vec::new();
        if !result.worktree_remove_succeeded && !result.worktree_prune_succeeded {
            failures.push("worktree removal and prune both failed".to_string());
        }
        if result.worktree_residue_observed {
            failures.push("disposable worktree remains registered after cleanup".to_string());
        }
        if !result.materialization_directory_removed {
            failures.push("disposable materialization directory remains on disk".to_string());
        }
        if !failures.is_empty() {
            let joined = failures.join("; ");
            result.failure_reason = match result.failure_reason.take() {
                Some(existing) => Some(format!("{existing}; {joined}")),
                None => Some(joined),
            };
        }
        self.cleaned = result.failure_reason.is_none();
        result
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
    let args = vec![command.to_string()];
    let output = capture_output_in_dir_with_timeout_bounded(
        checker,
        &args,
        &[
            ("RIPR_SOURCE_PROMOTION_TRUSTED_CHECKER_SHA", source_parent),
            ("RIPR_SOURCE_PROMOTION_VALIDATION", "1"),
        ],
        root,
        COMMAND_TIMEOUT,
        MAX_STREAM_BYTES,
        &format!("source-trusted governance command {command}"),
    );

    match output {
        Ok(output) => {
            if let Err(reason) = write_command_logs(&stdout_path, &stderr_path, &output) {
                return command_receipt(
                    command,
                    "unavailable",
                    output.status.as_ref().and_then(|status| status.code()),
                    output.duration,
                    output.stdout_truncated,
                    output.stderr_truncated,
                    Some(&reason),
                );
            }
            let exit_code = output.status.as_ref().and_then(|status| status.code());
            if output.timed_out {
                command_receipt(
                    command,
                    "failed",
                    exit_code,
                    output.duration,
                    output.stdout_truncated,
                    output.stderr_truncated,
                    Some(
                        "command exceeded the 180 second bound and its process tree was terminated",
                    ),
                )
            } else if output.status.as_ref().is_some_and(|status| status.success()) {
                command_receipt(
                    command,
                    "passed",
                    exit_code,
                    output.duration,
                    output.stdout_truncated,
                    output.stderr_truncated,
                    None,
                )
            } else {
                command_receipt(
                    command,
                    "failed",
                    exit_code,
                    output.duration,
                    output.stdout_truncated,
                    output.stderr_truncated,
                    Some("command exited non-zero"),
                )
            }
        }
        Err(reason) => command_receipt(
            command,
            "unavailable",
            None,
            Duration::ZERO,
            false,
            false,
            Some(&reason),
        ),
    }
}

fn write_command_logs(
    stdout_path: &Path,
    stderr_path: &Path,
    output: &TimedBoundedOutput,
) -> Result<(), String> {
    fs::write(stdout_path, output.stdout.as_bytes())
        .map_err(|error| format!("failed to write command stdout evidence: {error}"))?;
    fs::write(stderr_path, output.stderr.as_bytes())
        .map_err(|error| format!("failed to write command stderr evidence: {error}"))?;
    Ok(())
}

fn command_receipt(
    command: &str,
    state: &str,
    exit_code: Option<i32>,
    duration: Duration,
    stdout_truncated: bool,
    stderr_truncated: bool,
    failure_reason: Option<&str>,
) -> Value {
    let evidence_index = REQUIRED_COMMANDS
        .iter()
        .position(|candidate| *candidate == command)
        .map_or(1, |index| index + 1);
    serde_json::json!({
        "command": command,
        "state": state,
        "exit_code": exit_code,
        "duration_ms": duration_ms(duration),
        "stdout_path": format!("commands/{evidence_index:02}-{command}.stdout.log"),
        "stderr_path": format!("commands/{evidence_index:02}-{command}.stderr.log"),
        "stdout_truncated": stdout_truncated,
        "stderr_truncated": stderr_truncated,
        "failure_reason": failure_reason,
    })
}

fn report_value(state: &ValidationState, status: &str) -> Value {
    serde_json::json!({
        "schema": RECEIPT_SCHEMA,
        "tool_version": env!("CARGO_PKG_VERSION"),
        "status": status,
        "source_parent": &state.inputs.source_parent,
        "swarm_parent": &state.inputs.swarm_parent,
        "reviewed_tree": &state.inputs.reviewed_tree,
        "preflight": {
            "path_role": "source_checkout_regular_file",
            "path": &state.inputs.preflight_path,
            "sha256": &state.inputs.preflight_sha256,
            "verified": state.preflight_verified,
        },
        "resolution_manifest": {
            "path_role": "source_checkout_regular_file",
            "path": &state.inputs.resolution_path,
            "sha256": &state.inputs.resolution_sha256,
            "verified": state.resolution_verified,
        },
        "trusted_checker": {
            "selection": "running xtask executable from checkout whose HEAD equals source_parent",
            "source_sha": &state.checker_source_sha,
            "executable_sha256": &state.checker_executable_sha256,
        },
        "materialization": {
            "path_role": "os_temp_disposable_checkout",
            "reviewed_tree": &state.materialized_tree,
            "disposable_commit": &state.disposable_commit,
            "created": state.materialization_created,
            "clean_before": state.materialization_clean_before,
            "clean_after": state.materialization_clean_after,
            "worktree_remove_succeeded": state.worktree_remove_succeeded,
            "worktree_prune_succeeded": state.worktree_prune_succeeded,
            "directory_removed": state.materialization_directory_removed,
            "worktree_residue_observed": state.worktree_residue_observed,
            "cleanup_failure_reason": &state.cleanup_failure_reason,
            "authoritative": false,
        },
        "required_command_catalog": REQUIRED_COMMANDS,
        "commands": &state.commands,
        "repository_observation": {
            "refs_before_sha256": &state.refs_before_sha256,
            "refs_after_sha256": &state.refs_after_sha256,
            "worktrees_before_sha256": &state.worktrees_before_sha256,
            "worktrees_after_sha256": &state.worktrees_after_sha256,
            "ref_mutation_observed": state.ref_mutation_observed,
            "worktree_registry_changed": state.worktree_registry_changed,
        },
        "disposable_git_object_write_attempted": state.materialization_created,
        "authoritative_commit_attempted": false,
        "branch_attempted": false,
        "tag_attempted": false,
        "push_attempted": false,
        "ref_mutation_attempted": false,
        "failure_reasons": &state.failure_reasons,
        "invalidation_rules": [
            "Changing the exact source parent, W7 parent, reviewed tree, preflight bytes, resolution-manifest bytes, running checker identity, required-command catalog, or receipt schema invalidates this validation.",
            "A failed, unavailable, or not_run required command rejects construction eligibility.",
            "Any observed or attempted authoritative ref mutation or retained worktree residue rejects construction eligibility.",
        ],
        "non_claims": [
            "The disposable commit is an unreferenced materialization object only; it is not J, a release object, a branch, or publication authority.",
            "The checker claim is bounded to the running executable selected from a checkout whose HEAD equals the exact source parent and whose executable digest is recorded.",
            "This receipt proves only the named source-governed repository contracts on one exact reviewed tree.",
            "It does not prove product correctness, editor journeys, release readiness, merge eligibility beyond the named contracts, or publication authority.",
        ],
    })
}

fn write_report(out: &Path, report: &Value) -> Result<(), String> {
    ensure_output_directory(out)?;
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

fn ensure_output_directory(out: &Path) -> Result<(), String> {
    if out.exists() {
        let metadata = fs::symlink_metadata(out)
            .map_err(|error| format!("failed to inspect output directory: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("resolved-tree output must be a non-symlink directory".to_string());
        }
    } else {
        fs::create_dir_all(out)
            .map_err(|error| format!("failed to create {}: {error}", out.display()))?;
    }
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
        "# Resolved-tree validation\n\n- Schema: `{schema}`\n- Status: **{status}**\n- SOURCE_PARENT: `{source}`\n- SWARM_PARENT: `{swarm}`\n- REVIEWED_TREE: `{tree}`\n\n## Claim boundary\n\nThis source-parent-selected receipt reports the named repository-governance commands on one exact reviewed tree. It does not construct J, move an authoritative ref, qualify product/editor behavior, or authorize publication.\n\n## Structured receipt\n\n```json\n{structured}\n```\n"
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
        let value = args[index + 1].clone();
        if value.trim().is_empty() || value.starts_with("--") {
            return Err(format!("missing value for {key}\n{}", usage()));
        }
        if values.insert(key, value).is_some() {
            return Err(format!("duplicate option {key}"));
        }
        index += 2;
    }
    let required = |key: &str| {
        values
            .get(key)
            .cloned()
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

fn input_echo(args: &[String]) -> InputEcho {
    InputEcho {
        source_parent: value_after(args, "--source-parent"),
        swarm_parent: value_after(args, "--swarm-parent"),
        reviewed_tree: value_after(args, "--reviewed-tree"),
        preflight_path: value_after(args, "--preflight"),
        preflight_sha256: value_after(args, "--preflight-sha256"),
        resolution_path: value_after(args, "--resolution-manifest"),
        resolution_sha256: value_after(args, "--resolution-sha256"),
    }
}

fn input_echo_from_options(options: &Options) -> InputEcho {
    InputEcho {
        source_parent: Some(options.source_parent.clone()),
        swarm_parent: Some(options.swarm_parent.clone()),
        reviewed_tree: Some(options.reviewed_tree.clone()),
        preflight_path: Some(path_for_receipt(&options.repo, &options.preflight)),
        preflight_sha256: Some(options.preflight_sha256.clone()),
        resolution_path: Some(path_for_receipt(
            &options.repo,
            &options.resolution_manifest,
        )),
        resolution_sha256: Some(options.resolution_sha256.clone()),
    }
}

fn value_after(args: &[String], key: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| {
            pair[0] == key && !pair[1].trim().is_empty() && !pair[1].starts_with("--")
        })
        .map(|pair| pair[1].clone())
}

fn read_bound_json(
    repo: &Path,
    path: &Path,
    expected_digest: &str,
    label: &str,
) -> Result<(Value, String), String> {
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
    let relative = canonical
        .strip_prefix(&canonical_repo)
        .map_err(|_| format!("{label} is outside the source checkout"))?;
    Ok((value, normalize_path(relative)))
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

fn verify_exact_commit(repo: &Path, value: &str, label: &str) -> Result<(), String> {
    let resolved = git(
        repo,
        &["rev-parse", "--verify", &format!("{value}^{{commit}}")],
        &[],
    )?;
    if resolved.trim() != value {
        return Err(format!("{label} is not an exact commit object"));
    }
    Ok(())
}

fn verify_exact_tree(repo: &Path, value: &str) -> Result<(), String> {
    let resolved = git(
        repo,
        &["rev-parse", "--verify", &format!("{value}^{{tree}}")],
        &[],
    )?;
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
        &[],
    )
}

fn snapshot_worktrees(repo: &Path) -> Result<String, String> {
    git(repo, &["worktree", "list", "--porcelain"], &[])
}

fn git(repo: &Path, args: &[&str], envs: &[(&str, &str)]) -> Result<String, String> {
    let owned_args: Vec<String> = args.iter().map(|value| (*value).to_string()).collect();
    let output = capture_output_in_dir_with_timeout_bounded(
        Path::new("git"),
        &owned_args,
        envs,
        repo,
        GIT_TIMEOUT,
        MAX_STREAM_BYTES,
        &format!("git {}", args.join(" ")),
    )?;
    if output.timed_out {
        return Err(format!(
            "git {} exceeded the 60 second bound",
            args.join(" ")
        ));
    }
    if !output.status.as_ref().is_some_and(|status| status.success()) {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            output.stderr.trim()
        ));
    }
    Ok(output.stdout)
}

fn create_exclusive_temp_dir(prefix: &str, identity: &str) -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock predates Unix epoch: {error}"))?
        .as_nanos();
    for attempt in 0..TEMP_ATTEMPTS {
        let seed = format!(
            "{prefix}:{}:{timestamp}:{attempt}:{identity}",
            std::process::id()
        );
        let token = digest_bytes(seed.as_bytes());
        let candidate = std::env::temp_dir().join(format!("{prefix}-{}", &token[..24]));
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "failed to create exclusive materialization directory: {error}"
                ));
            }
        }
    }
    Err("failed to allocate an exclusive materialization directory".to_string())
}

fn digest_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path)
        .map_err(|error| format!("failed to open {} for hashing: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("failed to hash {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
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

fn path_for_receipt(repo: &Path, path: &Path) -> String {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo.join(path)
    };
    match (repo.canonicalize(), candidate.canonicalize()) {
        (Ok(root), Ok(canonical)) => canonical
            .strip_prefix(root)
            .map(normalize_path)
            .unwrap_or_else(|_| "outside-source-checkout".to_string()),
        _ => normalize_path(path),
    }
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::{REQUIRED_COMMANDS, ValidationState, input_echo, report_value, validate_exact_hex};

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
        assert_eq!(
            REQUIRED_COMMANDS.first().copied(),
            Some("check-network-policy")
        );
        assert_eq!(
            REQUIRED_COMMANDS.last().copied(),
            Some("check-architecture")
        );
    }

    #[test]
    fn option_value_that_is_another_flag_is_rejected_as_a_value() {
        let args = vec![
            "validate-resolved-tree".to_string(),
            "--source-parent".to_string(),
            "--swarm-parent".to_string(),
        ];
        let echo = input_echo(&args);
        assert_eq!(echo.source_parent, None);
    }

    #[test]
    fn rejected_and_validated_receipts_share_the_same_top_level_keys() {
        let rejected = report_value(&ValidationState::new(Default::default()), "rejected");
        let validated = report_value(&ValidationState::new(Default::default()), "validated");
        let rejected_keys = rejected
            .as_object()
            .map(|object| object.keys().collect::<Vec<_>>());
        let validated_keys = validated
            .as_object()
            .map(|object| object.keys().collect::<Vec<_>>());
        assert_eq!(rejected_keys, validated_keys);
    }
}
