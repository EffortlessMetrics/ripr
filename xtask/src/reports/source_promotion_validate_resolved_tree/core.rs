use super::source_promotion_verify::{validate_manifest, validate_preflight};
use crate::run::{TimedBoundedOutput, capture_output_in_dir_with_timeout_bounded};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) const SOURCE_PROMOTION_VALIDATE_RESOLVED_TREE_SUBCOMMAND: &str =
    "validate-resolved-tree";
pub(crate) const SOURCE_PROMOTION_RESOLVED_TREE_DEFAULT_OUT: &str =
    "target/ripr/source-promotion/resolved-tree";

const RECEIPT_SCHEMA: &str = "ripr.source_promotion_resolved_tree_validation.v1";
const PACKET_SCHEMA: &str = "ripr.source_promotion_resolved_tree_packet.v1";
const REPORT_JSON: &str = "resolved-tree-validation.json";
const REPORT_MD: &str = "resolved-tree-validation.md";
const PACKET_INDEX: &str = "packet-index.json";
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
    materialization_directory_removed: bool,
    worktree_residue_observed: bool,
    cleanup_failure_reason: Option<String>,
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
            materialization_directory_removed: false,
            worktree_residue_observed: false,
            cleanup_failure_reason: None,
            ref_mutation_observed: false,
            worktree_registry_changed: false,
            commands: REQUIRED_COMMANDS
                .iter()
                .map(|command| {
                    command_receipt(
                        command,
                        "not_run",
                        None,
                        None,
                        Some("validation did not reach governed command execution"),
                    )
                })
                .collect(),
            failure_reasons: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct CommandEvidence {
    stdout_path: String,
    stdout_bytes: usize,
    stdout_sha256: String,
    stdout_truncated: bool,
    stderr_path: String,
    stderr_bytes: usize,
    stderr_sha256: String,
    stderr_truncated: bool,
}

#[derive(Debug)]
struct PacketWorkspace {
    final_out: PathBuf,
    staging: PathBuf,
    published: bool,
}

impl PacketWorkspace {
    fn create(final_out: &Path, identity: &str) -> Result<Self, String> {
        reject_parent_components(final_out, "resolved-tree output")?;
        let final_out = if final_out.is_absolute() {
            final_out.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|error| format!("failed to read current directory: {error}"))?
                .join(final_out)
        };
        if fs::symlink_metadata(&final_out).is_ok() {
            return Err(format!(
                "resolved-tree output already exists: {}",
                final_out.display()
            ));
        }
        let parent = final_out
            .parent()
            .ok_or_else(|| "resolved-tree output has no parent directory".to_string())?;
        ensure_directory_path(parent)?;
        let file_name = final_out
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "resolved-tree output must have a UTF-8 final component".to_string())?;
        for attempt in 0..TEMP_ATTEMPTS {
            let seed = format!(
                "packet:{identity}:{}:{}:{attempt}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|error| format!("system clock predates Unix epoch: {error}"))?
                    .as_nanos()
            );
            let token = digest_bytes(seed.as_bytes());
            let staging = parent.join(format!(".{file_name}.ripr-tmp-{}", &token[..24]));
            match fs::create_dir(&staging) {
                Ok(()) => {
                    return Ok(Self {
                        final_out,
                        staging,
                        published: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!(
                        "failed to create exclusive resolved-tree packet workspace: {error}"
                    ));
                }
            }
        }
        Err("failed to allocate exclusive resolved-tree packet workspace".to_string())
    }

    fn root(&self) -> &Path {
        &self.staging
    }

    fn publish(&mut self, report: &Value) -> Result<(), String> {
        let json = serde_json::to_string_pretty(report)
            .map_err(|error| format!("failed to serialize resolved-tree receipt: {error}"))?;
        let markdown = render_markdown(report)?;
        write_new_file(&self.staging.join(REPORT_JSON), format!("{json}\n").as_bytes())?;
        write_new_file(&self.staging.join(REPORT_MD), markdown.as_bytes())?;

        let entries = packet_entries(&self.staging)?;
        let index = serde_json::json!({
            "schema": PACKET_SCHEMA,
            "status": report.get("status").and_then(Value::as_str).unwrap_or("rejected"),
            "complete": true,
            "files": entries,
        });
        let index_json = serde_json::to_string_pretty(&index)
            .map_err(|error| format!("failed to serialize resolved-tree packet index: {error}"))?;
        write_new_file(
            &self.staging.join(PACKET_INDEX),
            format!("{index_json}\n").as_bytes(),
        )?;

        if fs::symlink_metadata(&self.final_out).is_ok() {
            return Err(format!(
                "resolved-tree output appeared before packet publication: {}",
                self.final_out.display()
            ));
        }
        let parent = self
            .final_out
            .parent()
            .ok_or_else(|| "resolved-tree output has no parent directory".to_string())?;
        ensure_directory_path(parent)?;
        fs::rename(&self.staging, &self.final_out).map_err(|error| {
            format!(
                "failed to atomically publish resolved-tree packet {}: {error}",
                self.final_out.display()
            )
        })?;
        self.published = true;
        println!("Wrote {}", self.final_out.join(REPORT_JSON).display());
        println!("Wrote {}", self.final_out.join(REPORT_MD).display());
        println!("Wrote {}", self.final_out.join(PACKET_INDEX).display());
        Ok(())
    }
}

impl Drop for PacketWorkspace {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.staging);
        }
    }
}

pub(crate) fn source_promotion_validate_resolved_tree(args: &[String]) -> Result<(), String> {
    let echo = input_echo(args);
    let out = output_path_from_args(args)
        .unwrap_or_else(|| PathBuf::from(SOURCE_PROMOTION_RESOLVED_TREE_DEFAULT_OUT));
    let packet_identity = echo
        .reviewed_tree
        .as_deref()
        .unwrap_or("unparsed-resolved-tree");
    let mut packet = PacketWorkspace::create(&out, packet_identity)?;

    let options = match parse_args(args) {
        Ok(options) => options,
        Err(reason) => {
            let mut state = ValidationState::new(echo);
            state.failure_reasons.push(reason.clone());
            let report = report_value(&state);
            return match packet.publish(&report) {
                Ok(()) => Err(reason),
                Err(write_error) => Err(format!(
                    "{reason}; failed to publish resolved-tree validation packet: {write_error}"
                )),
            };
        }
    };
    if options.out != out {
        let reason = "resolved-tree output path changed between packet selection and argument parsing";
        let mut state = ValidationState::new(input_echo_from_options(&options));
        state.failure_reasons.push(reason.to_string());
        let report = report_value(&state);
        return match packet.publish(&report) {
            Ok(()) => Err(reason.to_string()),
            Err(write_error) => Err(format!(
                "{reason}; failed to publish resolved-tree validation packet: {write_error}"
            )),
        };
    }

    let mut state = ValidationState::new(input_echo_from_options(&options));
    let mut validation = validate(&options, &mut state, packet.root());
    if let Err(reason) = &validation {
        push_failure_once(&mut state, reason);
    }
    if validation.is_ok() && !state_earns_validated(&state) {
        let reason = "validation completed without earning every validated-state predicate".to_string();
        push_failure_once(&mut state, &reason);
        validation = Err(reason);
    }

    let report = report_value(&state);
    if report.get("status").and_then(Value::as_str) == Some("validated")
        && !resolved_tree_receipt_is_admissible(&report)
    {
        let reason = "generated validated receipt failed its admission invariant".to_string();
        push_failure_once(&mut state, &reason);
        validation = Err(reason);
    }
    let report = report_value(&state);
    let write_result = packet.publish(&report);
    match (validation, write_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(reason), Ok(())) => Err(reason),
        (Ok(()), Err(write_error)) => Err(write_error),
        (Err(reason), Err(write_error)) => Err(format!(
            "{reason}; failed to publish resolved-tree validation packet: {write_error}"
        )),
    }
}

fn push_failure_once(state: &mut ValidationState, reason: &str) {
    if !state
        .failure_reasons
        .iter()
        .any(|existing| existing == reason)
    {
        state.failure_reasons.push(reason.to_string());
    }
}
