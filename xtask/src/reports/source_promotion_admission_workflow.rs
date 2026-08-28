//! Source-owned workflow harness for one exact source-promotion admission.
//!
//! This layer normalizes the existing controller packets for hosted workflow
//! transport. It deliberately has no ref, push, merge, release, or publication
//! operation. Rejected controller evidence is still a complete workflow packet;
//! enforcement is a separate command so artifact upload can happen first.

use super::source_promotion_admission_fixture::{
    LocatorMaterial, SyntheticFixture, SyntheticProfile, prepare_source_owned_fixture,
    write_bound_qualification,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const RUN: &str = "run-admission-workflow";
const FINALIZE: &str = "finalize-admission-workflow";
const VERIFY: &str = "verify-admission-workflow";
const ENFORCE: &str = "enforce-admission-workflow";

const SCHEMA: &str = "ripr.source_promotion_admission_workflow.v1";
const PACKET_SCHEMA: &str = "ripr.source_promotion_admission_workflow_packet.v1";
const LOCATOR_SCHEMA: &str = "ripr.source_promotion_artifact_locator.v1";
const SUPPORTED_RECEIPT_SCHEMA: &str = SCHEMA;
const SOURCE_REPOSITORY: &str = "EffortlessMetrics/ripr";
const SWARM_REPOSITORY: &str = "EffortlessMetrics/ripr-swarm";
const REPORT_JSON: &str = "workflow-disposition.json";
const REPORT_MD: &str = "workflow-disposition.md";
const PACKET_INDEX: &str = "packet-index.json";

const RUN_KEYS: &[&str] = &[
    "--source-repository",
    "--source-parent-sha",
    "--workflow-source-sha",
    "--trusted-checker-identity",
    "--swarm-repository",
    "--protected-w7-ref",
    "--w7-peeled-sha",
    "--reviewed-tree-sha",
    "--preflight-locator",
    "--resolution-manifest-locator",
    "--validation-packet-locator",
    "--integration-packet-locator",
    "--qualification-receipt-locator",
    "--receipt-schema",
    "--operation-mode",
    "--execution-profile",
    "--workspace-root",
    "--out",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OperationMode {
    AdmitOnly,
    ConstructorDryRun,
}

impl OperationMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "admit_only" => Ok(Self::AdmitOnly),
            "constructor_dry_run" => Ok(Self::ConstructorDryRun),
            _ => Err("--operation-mode must be admit_only or constructor_dry_run".to_string()),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::AdmitOnly => "admit_only",
            Self::ConstructorDryRun => "constructor_dry_run",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExecutionProfile {
    Live,
    PositiveSynthetic,
    J5Negative,
}

impl ExecutionProfile {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "live" => Ok(Self::Live),
            "positive_synthetic" => Ok(Self::PositiveSynthetic),
            "j5_negative" => Ok(Self::J5Negative),
            _ => Err(
                "--execution-profile must be live, positive_synthetic, or j5_negative".to_string(),
            ),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::PositiveSynthetic => "positive_synthetic",
            Self::J5Negative => "j5_negative",
        }
    }
}

#[derive(Clone, Debug)]
struct Locator {
    value: Value,
    local_path: PathBuf,
    sha256: String,
}

#[derive(Debug)]
struct RunOptions {
    controller_repo: PathBuf,
    source_repository: String,
    source_parent: String,
    workflow_source_sha: String,
    trusted_checker_identity: String,
    swarm_repository: String,
    swarm_ref: String,
    swarm_parent: String,
    reviewed_tree: String,
    preflight: Locator,
    resolution: Locator,
    validation: Locator,
    integration: Locator,
    qualification: Locator,
    receipt_schema: String,
    mode: OperationMode,
    profile: ExecutionProfile,
    workspace_root: PathBuf,
    out: PathBuf,
    fixture_identity: Option<String>,
    synthetic_fixture: Option<SyntheticFixture>,
}

pub(crate) fn source_promotion_admission_workflow_handles(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some(RUN) | Some(FINALIZE) | Some(VERIFY) | Some(ENFORCE)
    )
}

pub(crate) fn source_promotion_admission_workflow(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some(RUN) => run(args),
        Some(FINALIZE) => finalize(args),
        Some(VERIFY) => verify_command(args),
        Some(ENFORCE) => enforce_command(args),
        _ => Err(usage()),
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let options = parse_run_options(args)?;
    reject_existing_output(&options.out)?;
    validate_workspace(&options.workspace_root, &options.out)?;

    let builder_out = options.workspace_root.join("trusted-builder");
    let admission_out = options.workspace_root.join("resolved-tree-admission");
    for path in [&builder_out, &admission_out] {
        reject_existing_output(path)?;
    }

    let current_exe = std::env::current_exe()
        .map_err(|error| format!("failed to locate running xtask executable: {error}"))?;
    let target_dir = current_exe
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| "running xtask executable has no Cargo target directory".to_string())?;
    let builder_args = strings(&[
        "write-trusted-builder-receipt",
        "--source-parent",
        &options.source_parent,
        "--workflow-source-sha",
        &options.workflow_source_sha,
        "--executable",
        &path_text(&current_exe)?,
        "--cargo-target-dir",
        &path_text(target_dir)?,
        "--locked-build",
        "--isolated-target-dir",
        "--out",
        &path_text(&builder_out)?,
    ]);
    let builder_result = invoke_controller(&options.controller_repo, &builder_args);

    let admission_args = strings(&[
        "admit-resolved-tree",
        "--source-parent",
        &options.source_parent,
        "--swarm-parent",
        &options.swarm_parent,
        "--join-tree",
        &options.reviewed_tree,
        "--preflight",
        &path_text(&options.preflight.local_path)?,
        "--preflight-sha256",
        &options.preflight.sha256,
        "--resolution-manifest",
        &path_text(&options.resolution.local_path)?,
        "--resolution-sha256",
        &options.resolution.sha256,
        "--validation-packet",
        &path_text(packet_root(&options.validation)?)?,
        "--builder-packet",
        &path_text(&builder_out)?,
        "--integration-index",
        &path_text(&options.integration.local_path)?,
        "--integration-index-sha256",
        &options.integration.sha256,
        "--out",
        &path_text(&admission_out)?,
    ]);
    let admission_result = if builder_result.is_ok() {
        invoke_controller(&options.controller_repo, &admission_args)
    } else {
        builder_result.as_ref().map(|_| ()).map_err(Clone::clone)
    };

    let admission_report = read_optional_json(&admission_out.join("resolved-tree-admission.json"));
    let admitted = admission_result.is_ok()
        && admission_report
            .as_ref()
            .is_some_and(|report| json_string(report, "status") == Some("admitted"));

    let construction_result: Option<Result<(), String>> = None;
    let construction_report = None;
    let status = if admitted { "admitted" } else { "rejected" };
    let qualification = if admitted && options.mode == OperationMode::ConstructorDryRun {
        if let Some(fixture) = options.synthetic_fixture.as_ref() {
            synthetic_material(
                &write_bound_qualification(fixture, &admission_out)?,
                "qualification receipt",
            )
        } else {
            options.qualification.clone()
        }
    } else {
        options.qualification.clone()
    };
    let failures = collect_failures(
        builder_result.as_ref().err(),
        admission_result.as_ref().err(),
        construction_result
            .as_ref()
            .and_then(|result| result.as_ref().err()),
        &admission_report,
        &construction_report,
    );
    let attempts = normalized_attempts(&admission_report, &construction_report);
    let fixture_identity = options.fixture_identity.clone().unwrap_or_else(|| {
        digest_bytes(
            format!(
                "{}:{}:{}:{}:{}:{}:{}",
                options.profile.as_str(),
                options.mode.as_str(),
                options.source_parent,
                options.swarm_parent,
                options.reviewed_tree,
                options.receipt_schema,
                options.trusted_checker_identity
            )
            .as_bytes(),
        )
    });
    let mut report = serde_json::json!({
        "schema": SCHEMA,
        "phase": "admission",
        "status": status,
        "operation_mode": options.mode.as_str(),
        "execution_profile": options.profile.as_str(),
        "fixture_identity": fixture_identity,
        "workflow_identity_sha256": null,
        "controller_repository": stable_controller_repository(&options),
        "source_repository": options.source_repository,
        "source_parent_sha": options.source_parent,
        "workflow_source_sha": options.workflow_source_sha,
        "trusted_checker_identity": options.trusted_checker_identity,
        "swarm_repository": options.swarm_repository,
        "protected_w7_ref": options.swarm_ref,
        "w7_peeled_sha": options.swarm_parent,
        "reviewed_tree_sha": options.reviewed_tree,
        "receipt_schema": options.receipt_schema,
        "locators": {
            "preflight": options.preflight.value,
            "resolution_manifest": options.resolution.value,
            "validation_packet": options.validation.value,
            "integration_packet": options.integration.value,
            "qualification_receipt": qualification.value,
        },
        "controller_packets": {
            "trusted_builder": relative_packet_state(&options.workspace_root, &builder_out, "trusted-builder.json"),
            "resolved_tree_admission": relative_packet_state(&options.workspace_root, &admission_out, "resolved-tree-admission.json"),
            "exact_join_construction": {
                "path": null,
                "available": false,
                "status": "not_run",
                "schema": null,
            },
        },
        "producer": {
            "normalized_exit_code": if status == "admitted" { 0 } else { 1 },
            "trusted_builder_state": if builder_result.is_ok() { "passed" } else { "rejected" },
            "admission_state": if admission_result.is_ok() { "passed" } else { "rejected" },
            "constructor_state": "not_run_before_upload_and_enforcement",
        },
        "attempts": attempts,
        "failure_reasons": failures,
        "complete": true,
        "non_claims": [
            "This workflow packet grants no ref, merge, release, signing, or publication authority.",
            "An admitted packet is not product or editor qualification.",
        ],
    });
    report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
    write_packet(&options.out, &report)?;
    verify_packet(&options.out)?;
    if status == "admitted" {
        Ok(())
    } else {
        Err("source-promotion admission workflow produced a complete rejected packet".to_string())
    }
}

fn finalize(args: &[String]) -> Result<(), String> {
    let values = parse_args(
        args,
        FINALIZE,
        &["--admission-packet", "--workspace-root", "--out"],
    )?;
    let current_dir = std::env::current_dir()
        .map_err(|error| format!("failed to read current directory: {error}"))?;
    let absolute = |value: &str| {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            path
        } else {
            current_dir.join(path)
        }
    };
    let admission_packet = absolute(required(&values, "--admission-packet")?);
    let workspace = absolute(required(&values, "--workspace-root")?);
    let out = absolute(required(&values, "--out")?);
    let mut report = verify_packet(&admission_packet)?;
    if json_string(&report, "phase") != Some("admission")
        || json_string(&report, "status") != Some("admitted")
    {
        return Err("finalization requires one admitted pre-enforcement packet".to_string());
    }
    validate_workspace(&workspace, &out)?;
    let mode = OperationMode::parse(required_json_string(&report, "operation_mode")?)?;
    let mut construction_report = None;
    let mut construction_error = None;
    if mode == OperationMode::ConstructorDryRun {
        let locators = report
            .get("locators")
            .ok_or_else(|| "admission packet is missing locators".to_string())?;
        let locator_path = |key: &str, slot: &str| -> Result<PathBuf, String> {
            if let Some(path) = locators
                .get(key)
                .and_then(|value| json_string(value, "local_path"))
            {
                let path = PathBuf::from(path);
                return Ok(if path.is_absolute() {
                    path
                } else {
                    workspace.join(path)
                });
            }
            let path = locators
                .get(key)
                .and_then(|value| json_string(value, "path"))
                .ok_or_else(|| format!("admission packet locator is missing {key}"))?;
            Ok(workspace
                .join("locators")
                .join(slot)
                .join("files")
                .join(path))
        };
        let admission_controller = workspace.join("resolved-tree-admission");
        let validation_index = locator_path("validation_packet", "validation")?;
        let integration_index = locator_path("integration_packet", "integration")?;
        let preflight = locator_path("preflight", "preflight")?;
        let resolution = locator_path("resolution_manifest", "resolution")?;
        let qualification = locator_path("qualification_receipt", "qualification")?;
        let construction_out = workspace.join("exact-join-construction");
        reject_existing_output(&construction_out)?;
        let command = strings(&[
            "construct-exact-join",
            "--admission-packet",
            &path_text(&admission_controller)?,
            "--validation-packet",
            &path_text(
                validation_index
                    .parent()
                    .ok_or_else(|| "validation packet index has no parent".to_string())?,
            )?,
            "--integration-index",
            &path_text(&integration_index)?,
            "--integration-index-sha256",
            required_json_string(&report["locators"]["integration_packet"], "sha256")?,
            "--preflight",
            &path_text(&preflight)?,
            "--resolution-manifest",
            &path_text(&resolution)?,
            "--qualification-receipt",
            &path_text(&qualification)?,
            "--qualification-receipt-sha256",
            required_json_string(&report["locators"]["qualification_receipt"], "sha256")?,
            "--source-main-ref",
            "refs/heads/main",
            "--swarm-ref",
            required_json_string(&report, "protected_w7_ref")?,
            "--candidate-ref",
            "refs/heads/promote/0.11.0-admission-dry-run",
            "--out",
            &path_text(&construction_out)?,
        ]);
        let controller_repo =
            PathBuf::from(required_json_string(&report, "controller_repository")?);
        let controller_repo = if controller_repo == Path::new("source-checkout") {
            current_dir.clone()
        } else if controller_repo.is_absolute() {
            controller_repo
        } else {
            workspace.join(controller_repo)
        };
        if let Err(error) = invoke_controller(&controller_repo, &command) {
            construction_error = Some(error);
        }
        construction_report =
            read_optional_json(&construction_out.join("exact-join-construction.json"));
    }
    let mut attempts = normalized_attempts(&None, &construction_report);
    attempts["admission_receipt_available"] = Value::Bool(true);
    let constructor_ok = mode == OperationMode::AdmitOnly
        || (construction_error.is_none()
            && construction_report
                .as_ref()
                .is_some_and(|value| json_string(value, "status") == Some("constructed")));
    report["phase"] = Value::String("final".to_string());
    report["status"] = Value::String(if constructor_ok {
        "admitted".to_string()
    } else {
        "rejected".to_string()
    });
    report["attempts"] = attempts;
    report["producer"]["constructor_state"] = Value::String(if mode == OperationMode::AdmitOnly {
        "not_requested".to_string()
    } else if constructor_ok {
        "passed".to_string()
    } else {
        "rejected".to_string()
    });
    if let Some(error) = construction_error {
        report["failure_reasons"] = serde_json::json!([error]);
    }
    write_packet(&out, &report)?;
    verify_packet(&out)?;
    if constructor_ok {
        Ok(())
    } else {
        Err("source-promotion finalizer produced a complete rejected packet".to_string())
    }
}

fn normalized_attempts(admission: &Option<Value>, construction: &Option<Value>) -> Value {
    let required_counters = [
        "local_ref_attempts",
        "remote_push_attempts",
        "merge_command_attempts",
    ];
    let admission_available = admission.as_ref().is_some_and(|value| {
        required_counters
            .iter()
            .all(|key| json_u64(value, key).is_some())
    });
    let construction_available = construction.as_ref().is_some_and(|value| {
        required_counters
            .iter()
            .chain(std::iter::once(&"commit_tree_attempts"))
            .all(|key| json_u64(value, key).is_some())
    });
    let commit_tree = construction
        .as_ref()
        .and_then(|value| json_u64(value, "commit_tree_attempts"))
        .unwrap_or(0);
    let field = |name: &str| {
        admission
            .as_ref()
            .and_then(|value| json_u64(value, name))
            .unwrap_or(0)
            .saturating_add(
                construction
                    .as_ref()
                    .and_then(|value| json_u64(value, name))
                    .unwrap_or(0),
            )
    };
    serde_json::json!({
        "admission_receipt_available": admission_available,
        "construction_receipt_available": construction_available,
        "constructor_refs_unchanged": construction.as_ref().and_then(|value| value.get("refs_unchanged")).and_then(Value::as_bool),
        "constructor_object_unreferenced": construction.as_ref().and_then(|value| value.get("unreferenced_exact_join_constructed")).and_then(Value::as_bool),
        "constructor_final_identity_reread_passed": construction.as_ref().and_then(|value| value.get("final_identity_reread_passed")).and_then(Value::as_bool),
        "constructor_commit_tree_attempts": commit_tree,
        "local_ref_attempts": field("local_ref_attempts"),
        "remote_push_attempts": field("remote_push_attempts"),
        "merge_command_attempts": field("merge_command_attempts"),
        "release_or_publication_attempts": 0,
        "release_or_publication_command_reachable": false,
        "release_or_publication_proof": "closed workflow harness dispatch contains no publication subcommand",
    })
}

fn collect_failures(
    builder: Option<&String>,
    admission: Option<&String>,
    construction: Option<&String>,
    admission_report: &Option<Value>,
    construction_report: &Option<Value>,
) -> Vec<String> {
    let mut failures = BTreeSet::new();
    failures.extend(builder.into_iter().cloned());
    failures.extend(admission.into_iter().cloned());
    failures.extend(construction.into_iter().cloned());
    for report in [admission_report, construction_report]
        .into_iter()
        .flatten()
    {
        if let Some(reasons) = report.get("failure_reasons").and_then(Value::as_array) {
            failures.extend(reasons.iter().filter_map(Value::as_str).map(str::to_string));
        }
    }
    failures.into_iter().collect()
}

fn verify_command(args: &[String]) -> Result<(), String> {
    let values = parse_args(args, VERIFY, &["--packet"])?;
    verify_packet(Path::new(required(&values, "--packet")?)).map(|_| ())
}

fn enforce_command(args: &[String]) -> Result<(), String> {
    let values = parse_args(args, ENFORCE, &["--packet", "--expected-status"])?;
    let expected = required(&values, "--expected-status")?;
    if expected != "admitted" {
        return Err("--expected-status must be admitted".to_string());
    }
    let report = verify_packet(Path::new(required(&values, "--packet")?))?;
    if json_string(&report, "status") != Some(expected) {
        return Err(format!(
            "source-promotion admission workflow disposition is {}; expected {expected}",
            json_string(&report, "status").unwrap_or("missing")
        ));
    }
    Ok(())
}

fn verify_packet(root: &Path) -> Result<Value, String> {
    validate_directory(root, "workflow packet")?;
    let mut inventory = fs::read_dir(root)
        .map_err(|error| format!("failed to enumerate workflow packet: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to enumerate workflow packet entry: {error}"))?;
    inventory.sort_by_key(|entry| entry.file_name());
    let observed = inventory
        .iter()
        .map(|entry| {
            let metadata = fs::symlink_metadata(entry.path())
                .map_err(|error| format!("failed to inspect workflow packet entry: {error}"))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err("workflow packet contains a non-regular entry".to_string());
            }
            entry
                .file_name()
                .into_string()
                .map_err(|name| format!("workflow packet filename is not UTF-8: {name:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let expected = vec![
        PACKET_INDEX.to_string(),
        REPORT_JSON.to_string(),
        REPORT_MD.to_string(),
    ];
    if observed != expected {
        return Err(format!(
            "workflow packet inventory differs from exact contract: {observed:?}"
        ));
    }
    let index = read_json(&root.join(PACKET_INDEX), "workflow packet index")?;
    if json_string(&index, "schema") != Some(PACKET_SCHEMA)
        || index.get("complete").and_then(Value::as_bool) != Some(true)
    {
        return Err("workflow packet index is unsupported or incomplete".to_string());
    }
    let files = index
        .get("files")
        .and_then(Value::as_object)
        .ok_or_else(|| "workflow packet index is missing files".to_string())?;
    if files.len() != 2 || !files.contains_key(REPORT_JSON) || !files.contains_key(REPORT_MD) {
        return Err(
            "workflow packet index must bind exactly JSON and Markdown reports".to_string(),
        );
    }
    for (name, expected) in files {
        safe_relative(name)?;
        let expected = expected
            .as_str()
            .ok_or_else(|| format!("workflow packet digest for {name} is not a string"))?;
        validate_hex(expected, 64, "workflow packet digest")?;
        let actual = digest_file(&root.join(name), "workflow packet file")?;
        if actual != expected {
            return Err(format!("workflow packet digest mismatch for {name}"));
        }
    }
    let report = read_json(&root.join(REPORT_JSON), "workflow disposition")?;
    validate_report(&report)?;
    Ok(report)
}

fn validate_report(report: &Value) -> Result<(), String> {
    if json_string(report, "schema") != Some(SCHEMA)
        || report.get("complete").and_then(Value::as_bool) != Some(true)
    {
        return Err("workflow disposition is unsupported or incomplete".to_string());
    }
    let status = json_string(report, "status")
        .filter(|status| matches!(*status, "admitted" | "rejected"))
        .ok_or_else(|| "workflow disposition status is not admitted or rejected".to_string())?;
    let phase = json_string(report, "phase")
        .filter(|phase| matches!(*phase, "admission" | "final"))
        .ok_or_else(|| "workflow disposition phase is not admission or final".to_string())?;
    let mode = OperationMode::parse(
        json_string(report, "operation_mode")
            .ok_or_else(|| "workflow disposition is missing operation_mode".to_string())?,
    )?;
    let profile = ExecutionProfile::parse(
        json_string(report, "execution_profile")
            .ok_or_else(|| "workflow disposition is missing execution_profile".to_string())?,
    )?;
    for key in [
        "source_parent_sha",
        "workflow_source_sha",
        "w7_peeled_sha",
        "reviewed_tree_sha",
    ] {
        validate_hex(
            json_string(report, key)
                .ok_or_else(|| format!("workflow disposition is missing {key}"))?,
            40,
            key,
        )?;
    }
    let workflow_source_sha = required_json_string(report, "workflow_source_sha")?;
    if required_json_string(report, "trusted_checker_identity")?
        != format!("source-owned-xtask@{workflow_source_sha}")
    {
        return Err("workflow disposition trusted checker identity moved".to_string());
    }
    validate_hex(
        json_string(report, "fixture_identity")
            .ok_or_else(|| "workflow disposition is missing fixture_identity".to_string())?,
        64,
        "fixture_identity",
    )?;
    let expected_workflow_identity = workflow_identity(report)?;
    if json_string(report, "workflow_identity_sha256") != Some(expected_workflow_identity.as_str())
    {
        return Err("workflow disposition exact identity binding moved".to_string());
    }
    let attempts = report
        .get("attempts")
        .ok_or_else(|| "workflow disposition is missing attempts".to_string())?;
    if attempts
        .get("release_or_publication_command_reachable")
        .and_then(Value::as_bool)
        != Some(false)
        || json_string(attempts, "release_or_publication_proof")
            != Some("closed workflow harness dispatch contains no publication subcommand")
    {
        return Err("workflow disposition lacks closed publication reachability proof".to_string());
    }
    if attempts
        .get("admission_receipt_available")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err("workflow disposition has unavailable admission attempt evidence".to_string());
    }
    if phase == "final"
        && mode == OperationMode::ConstructorDryRun
        && attempts
            .get("construction_receipt_available")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err(
            "workflow disposition has unavailable construction attempt evidence".to_string(),
        );
    }
    let commit_tree = required_u64(attempts, "constructor_commit_tree_attempts")?;
    for key in [
        "local_ref_attempts",
        "remote_push_attempts",
        "merge_command_attempts",
        "release_or_publication_attempts",
    ] {
        if required_u64(attempts, key)? != 0 {
            return Err(format!("workflow disposition records forbidden {key}"));
        }
    }
    if mode == OperationMode::AdmitOnly && commit_tree != 0 {
        return Err("admit_only disposition records a constructor attempt".to_string());
    }
    if commit_tree > 1 {
        return Err("constructor dry-run attempted more than one commit-tree".to_string());
    }
    if status == "admitted"
        && phase == "final"
        && mode == OperationMode::ConstructorDryRun
        && commit_tree != 1
    {
        return Err("admitted constructor dry-run must record exactly one commit-tree".to_string());
    }
    if status == "admitted"
        && phase == "final"
        && mode == OperationMode::ConstructorDryRun
        && (attempts
            .get("constructor_refs_unchanged")
            .and_then(Value::as_bool)
            != Some(true)
            || attempts
                .get("constructor_object_unreferenced")
                .and_then(Value::as_bool)
                != Some(true)
            || attempts
                .get("constructor_final_identity_reread_passed")
                .and_then(Value::as_bool)
                != Some(true))
    {
        return Err(
            "admitted constructor dry-run lacks unreferenced-object and exact-ref proof"
                .to_string(),
        );
    }
    if profile == ExecutionProfile::J5Negative && (status != "rejected" || commit_tree != 0) {
        return Err("j5_negative must be rejected before constructor execution".to_string());
    }
    if json_string(report, "source_repository") != Some(SOURCE_REPOSITORY)
        || json_string(report, "swarm_repository") != Some(SWARM_REPOSITORY)
    {
        return Err("workflow disposition repository authority is unsupported".to_string());
    }
    if json_string(report, "receipt_schema") != Some(SUPPORTED_RECEIPT_SCHEMA) {
        return Err("workflow disposition receipt schema is unsupported".to_string());
    }
    let producer = report
        .get("producer")
        .ok_or_else(|| "workflow disposition is missing producer state".to_string())?;
    if producer.get("normalized_exit_code").and_then(Value::as_u64)
        != Some(if status == "admitted" { 0 } else { 1 })
        || !matches!(
            json_string(producer, "trusted_builder_state"),
            Some("passed" | "rejected")
        )
        || !matches!(
            json_string(producer, "admission_state"),
            Some("passed" | "rejected")
        )
    {
        return Err("workflow disposition producer state is unavailable or malformed".to_string());
    }
    Ok(())
}

fn parse_run_options(args: &[String]) -> Result<RunOptions, String> {
    let values = parse_args(args, RUN, RUN_KEYS)?;
    let source_repository = required(&values, "--source-repository")?.to_string();
    let swarm_repository = required(&values, "--swarm-repository")?.to_string();
    if source_repository != SOURCE_REPOSITORY || swarm_repository != SWARM_REPOSITORY {
        return Err(
            "source and swarm repositories must be the exact supported HTTPS URLs".to_string(),
        );
    }
    let source_parent = required_hex(&values, "--source-parent-sha", 40)?;
    let workflow_source_sha = required_hex(&values, "--workflow-source-sha", 40)?;
    if workflow_source_sha != source_parent {
        return Err("workflow source SHA must equal source parent SHA".to_string());
    }
    let trusted_checker_identity = required(&values, "--trusted-checker-identity")?.to_string();
    if trusted_checker_identity != format!("source-owned-xtask@{workflow_source_sha}") {
        return Err(
            "trusted checker identity must be source-owned-xtask@<workflow-source-sha>".to_string(),
        );
    }
    let swarm_parent = required_hex(&values, "--w7-peeled-sha", 40)?;
    let reviewed_tree = required_hex(&values, "--reviewed-tree-sha", 40)?;
    let swarm_ref = required(&values, "--protected-w7-ref")?.to_string();
    validate_ref(&swarm_ref)?;
    let receipt_schema = required(&values, "--receipt-schema")?.to_string();
    if receipt_schema != SUPPORTED_RECEIPT_SCHEMA {
        return Err(format!("unsupported receipt schema {receipt_schema}"));
    }
    let mode = OperationMode::parse(required(&values, "--operation-mode")?)?;
    let profile = ExecutionProfile::parse(required(&values, "--execution-profile")?)?;
    let source_checkout = std::env::current_dir()
        .map_err(|error| format!("failed to locate source checkout: {error}"))?;
    let requested_workspace = PathBuf::from(required(&values, "--workspace-root")?);
    let workspace_root = if requested_workspace.is_absolute() {
        requested_workspace
    } else {
        source_checkout.join(requested_workspace)
    };
    let requested_out = PathBuf::from(required(&values, "--out")?);
    let out = if requested_out.is_absolute() {
        requested_out
    } else {
        source_checkout.join(requested_out)
    };
    if profile != ExecutionProfile::Live {
        let fixture_profile = if profile == ExecutionProfile::PositiveSynthetic {
            SyntheticProfile::Positive
        } else {
            SyntheticProfile::J5Negative
        };
        let fixture = prepare_source_owned_fixture(
            &source_checkout,
            &workspace_root.join("synthetic-fixture"),
            fixture_profile,
        )?;
        let mismatches = [
            (&fixture.source_parent, &source_parent, "source parent"),
            (&fixture.swarm_parent, &swarm_parent, "W7 parent"),
            (&fixture.reviewed_tree, &reviewed_tree, "reviewed tree"),
            (&fixture.protected_w7_ref, &swarm_ref, "protected W7 ref"),
        ]
        .into_iter()
        .filter(|(actual, expected, _)| actual != expected)
        .map(|(actual, expected, label)| format!("{label}: expected {expected}, observed {actual}"))
        .collect::<Vec<_>>();
        if !mismatches.is_empty() {
            return Err(format!(
                "synthetic fixture differs from requested identity: {}",
                mismatches.join("; ")
            ));
        }
        return Ok(RunOptions {
            controller_repo: fixture.repository.clone(),
            source_repository,
            source_parent,
            workflow_source_sha,
            trusted_checker_identity,
            swarm_repository,
            swarm_ref,
            swarm_parent,
            reviewed_tree,
            preflight: synthetic_material(&fixture.preflight, "preflight"),
            resolution: synthetic_material(&fixture.resolution, "resolution manifest"),
            validation: synthetic_material(&fixture.validation_packet_index, "validation packet"),
            integration: synthetic_material(&fixture.integration_index, "integration packet"),
            qualification: empty_locator("qualification receipt"),
            receipt_schema,
            mode,
            profile,
            workspace_root,
            out,
            fixture_identity: Some(fixture.fixture_identity.clone()),
            synthetic_fixture: Some(fixture),
        });
    }
    let locate = |key: &str, label: &str, slot: &str| {
        read_locator(
            required(&values, key)?,
            label,
            &workspace_root,
            slot,
            profile,
        )
    };
    let qualification = match (mode, required(&values, "--qualification-receipt-locator")?) {
        (OperationMode::AdmitOnly, "") => empty_locator("qualification receipt"),
        (_, locator) => read_locator(
            locator,
            "qualification receipt",
            &workspace_root,
            "qualification",
            profile,
        )?,
    };
    Ok(RunOptions {
        controller_repo: prepare_live_controller_repo(
            &source_checkout,
            &workspace_root,
            &source_parent,
            &swarm_ref,
            &swarm_parent,
            &reviewed_tree,
        )?,
        source_repository,
        source_parent,
        workflow_source_sha,
        trusted_checker_identity,
        swarm_repository,
        swarm_ref,
        swarm_parent,
        reviewed_tree,
        preflight: locate("--preflight-locator", "preflight", "preflight")?,
        resolution: locate(
            "--resolution-manifest-locator",
            "resolution manifest",
            "resolution",
        )?,
        validation: locate(
            "--validation-packet-locator",
            "validation packet",
            "validation",
        )?,
        integration: locate(
            "--integration-packet-locator",
            "integration packet",
            "integration",
        )?,
        qualification,
        receipt_schema,
        mode,
        profile,
        workspace_root,
        out,
        fixture_identity: None,
        synthetic_fixture: None,
    })
}

fn read_locator(
    locator: &str,
    label: &str,
    workspace: &Path,
    slot: &str,
    profile: ExecutionProfile,
) -> Result<Locator, String> {
    if locator.is_empty() {
        return synthetic_locator(workspace, slot, label, profile);
    }
    let (authority, sha256) = locator
        .rsplit_once("#sha256:")
        .ok_or_else(|| format!("{label} locator is missing #sha256 binding"))?;
    let (repo_revision, artifact_path) = authority
        .split_once(':')
        .ok_or_else(|| format!("{label} locator is missing fixed path"))?;
    let (repository, revision) = repo_revision
        .rsplit_once('@')
        .ok_or_else(|| format!("{label} locator is missing exact revision"))?;
    if repository != SOURCE_REPOSITORY && repository != SWARM_REPOSITORY {
        return Err(format!("{label} locator repository is unsupported"));
    }
    validate_hex(revision, 40, &format!("{label} locator revision"))?;
    safe_relative(artifact_path)?;
    validate_hex(sha256, 64, &format!("{label} locator sha256"))?;
    let local_path =
        materialize_locator(workspace, slot, repository, revision, artifact_path, label)?;
    if digest_file(&local_path, label)? != sha256 {
        return Err(format!(
            "{label} locator digest does not match materialized file"
        ));
    }
    if artifact_path.ends_with(PACKET_INDEX) {
        materialize_indexed_siblings(
            workspace,
            slot,
            repository,
            revision,
            artifact_path,
            &local_path,
            label,
        )?;
    } else if label == "integration packet" {
        materialize_integration_siblings(
            workspace,
            slot,
            repository,
            revision,
            artifact_path,
            &local_path,
        )?;
    }
    let value = serde_json::json!({
        "schema": LOCATOR_SCHEMA,
        "repository": repository,
        "revision": revision,
        "path": artifact_path,
        "mode": "100644",
        "sha256": sha256,
        "locator": locator,
    });
    Ok(Locator {
        value,
        local_path,
        sha256: sha256.to_string(),
    })
}

fn packet_root(locator: &Locator) -> Result<&Path, String> {
    if locator
        .local_path
        .file_name()
        .and_then(|name| name.to_str())
        != Some(PACKET_INDEX)
    {
        return Err("validation packet locator must name packet-index.json".to_string());
    }
    locator
        .local_path
        .parent()
        .ok_or_else(|| "validation packet locator has no packet root".to_string())
}

fn empty_locator(label: &str) -> Locator {
    Locator {
        value: serde_json::json!({
            "schema": LOCATOR_SCHEMA,
            "status": "not_required",
            "label": label,
        }),
        local_path: PathBuf::new(),
        sha256: String::new(),
    }
}

fn synthetic_material(material: &LocatorMaterial, label: &str) -> Locator {
    let relative = stable_synthetic_path(&material.path);
    Locator {
        value: serde_json::json!({
            "schema": LOCATOR_SCHEMA,
            "status": "source_owned_synthetic",
            "label": label,
            "mode": "100644",
            "sha256": material.sha256,
            "path": relative,
            "local_path": relative,
        }),
        local_path: material.path.clone(),
        sha256: material.sha256.clone(),
    }
}

fn stable_synthetic_path(path: &Path) -> String {
    let components = path.components().collect::<Vec<_>>();
    let start = components
        .iter()
        .position(|component| component.as_os_str() == "synthetic-fixture")
        .unwrap_or(0);
    components[start..]
        .iter()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn stable_controller_repository(options: &RunOptions) -> String {
    options
        .controller_repo
        .strip_prefix(&options.workspace_root)
        .ok()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| "source-checkout".to_string())
}

fn synthetic_locator(
    _workspace: &Path,
    _slot: &str,
    label: &str,
    profile: ExecutionProfile,
) -> Result<Locator, String> {
    Err(format!(
        "source-owned {profile:?} fixture preparation for {label} is unavailable"
    ))
}

fn materialize_locator(
    workspace: &Path,
    slot: &str,
    repository: &str,
    revision: &str,
    artifact_path: &str,
    label: &str,
) -> Result<PathBuf, String> {
    let root = workspace.join("locators").join(slot);
    let git_dir = root.join("objects.git");
    fs::create_dir_all(&root)
        .map_err(|error| format!("failed to create {label} materialization root: {error}"))?;
    run_git(
        &root,
        &["init", "--bare", path_text(&git_dir)?.as_str()],
        label,
    )?;
    let url = format!("https://github.com/{repository}.git");
    run_git(
        &root,
        &[
            "--git-dir",
            path_text(&git_dir)?.as_str(),
            "fetch",
            "--no-tags",
            "--depth=1",
            &url,
            revision,
        ],
        label,
    )?;
    let tree = run_git_output(
        &root,
        &[
            "--git-dir",
            path_text(&git_dir)?.as_str(),
            "ls-tree",
            revision,
            "--",
            artifact_path,
        ],
        label,
    )?;
    let tree_text = String::from_utf8(tree)
        .map_err(|error| format!("{label} ls-tree output was not UTF-8: {error}"))?;
    if !tree_text.starts_with("100644 blob ") {
        return Err(format!("{label} locator must resolve to mode 100644 blob"));
    }
    let bytes = run_git_output(
        &root,
        &[
            "--git-dir",
            path_text(&git_dir)?.as_str(),
            "show",
            &format!("{revision}:{artifact_path}"),
        ],
        label,
    )?;
    let local = root.join("files").join(Path::new(artifact_path));
    if let Some(parent) = local.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {label} parent: {error}"))?;
    }
    fs::write(&local, bytes).map_err(|error| format!("failed to write {label}: {error}"))?;
    Ok(local)
}

fn prepare_live_controller_repo(
    source_checkout: &Path,
    workspace: &Path,
    source_parent: &str,
    swarm_ref: &str,
    swarm_parent: &str,
    reviewed_tree: &str,
) -> Result<PathBuf, String> {
    let runtime = workspace.join("live-controller-repository");
    reject_existing_output(&runtime)?;
    run_git(
        workspace,
        &[
            "clone",
            "--local",
            "--no-hardlinks",
            "--no-checkout",
            "--quiet",
            &path_text(source_checkout)?,
            &path_text(&runtime)?,
        ],
        "live controller repository",
    )?;
    run_git(
        &runtime,
        &["checkout", "--quiet", "--detach", source_parent],
        "live source parent",
    )?;
    run_git(
        &runtime,
        &["update-ref", "refs/heads/main", source_parent],
        "live source main materialization",
    )?;
    let swarm_url = format!("https://github.com/{SWARM_REPOSITORY}.git");
    let fetch_refspec = format!("+{swarm_ref}:{swarm_ref}");
    run_git(
        &runtime,
        &[
            "fetch",
            "--no-tags",
            "--depth=1",
            &swarm_url,
            &fetch_refspec,
        ],
        "protected W7 materialization",
    )?;
    let peeled = run_git_output(
        &runtime,
        &["rev-parse", &format!("{swarm_ref}^{{commit}}")],
        "protected W7 identity",
    )?;
    if String::from_utf8_lossy(&peeled).trim() != swarm_parent {
        return Err("protected W7 ref peeled SHA differs from requested identity".to_string());
    }
    run_git(
        &runtime,
        &["cat-file", "-e", &format!("{reviewed_tree}^{{tree}}")],
        "reviewed tree materialization",
    )?;
    Ok(runtime)
}

fn materialize_indexed_siblings(
    workspace: &Path,
    slot: &str,
    repository: &str,
    revision: &str,
    index_path: &str,
    local_index: &Path,
    label: &str,
) -> Result<(), String> {
    let index = read_json(local_index, &format!("{label} index"))?;
    let entries = index
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{label} index is missing files array"))?;
    let prefix = Path::new(index_path)
        .parent()
        .ok_or_else(|| format!("{label} index has no repository parent"))?;
    for entry in entries {
        let relative = entry
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{label} index entry is missing path"))?;
        safe_relative(relative)?;
        let repository_path = prefix.join(relative).to_string_lossy().replace('\\', "/");
        let local = materialize_locator(
            workspace,
            slot,
            repository,
            revision,
            &repository_path,
            label,
        )?;
        let expected = entry
            .get("sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{label} index entry is missing sha256"))?;
        if digest_file(&local, label)? != expected {
            return Err(format!(
                "{label} indexed sidecar digest mismatch for {relative}"
            ));
        }
    }
    Ok(())
}

fn materialize_integration_siblings(
    workspace: &Path,
    slot: &str,
    repository: &str,
    revision: &str,
    index_path: &str,
    local_index: &Path,
) -> Result<(), String> {
    let index = read_json(local_index, "integration packet index")?;
    let rows = index
        .get("receipts")
        .and_then(Value::as_array)
        .ok_or_else(|| "integration packet index is missing receipts array".to_string())?;
    let prefix = Path::new(index_path)
        .parent()
        .ok_or_else(|| "integration packet index has no repository parent".to_string())?;
    for row in rows {
        let relative = row
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| "integration receipt row is missing path".to_string())?;
        safe_relative(relative)?;
        let repository_path = prefix.join(relative).to_string_lossy().replace('\\', "/");
        let local = materialize_locator(
            workspace,
            slot,
            repository,
            revision,
            &repository_path,
            "integration receipt",
        )?;
        let expected = row
            .get("sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| "integration receipt row is missing sha256".to_string())?;
        if digest_file(&local, "integration receipt")? != expected {
            return Err(format!(
                "integration receipt digest mismatch for {relative}"
            ));
        }
    }
    Ok(())
}

fn run_git(cwd: &Path, args: &[&str], label: &str) -> Result<(), String> {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .map_err(|error| format!("failed to execute git for {label}: {error}"))?;
    if !status.success() {
        return Err(format!("git materialization failed for {label}: {status}"));
    }
    Ok(())
}

fn run_git_output(cwd: &Path, args: &[&str], label: &str) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| format!("failed to execute git for {label}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git materialization failed for {label}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

fn invoke_controller(repo: &Path, args: &[String]) -> Result<(), String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("failed to locate workflow controller executable: {error}"))?;
    let output = Command::new(executable)
        .arg("source-promotion")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|error| format!("failed to execute source-promotion controller: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Err(format!(
        "source-promotion controller rejected: status={} stdout={stdout:?} stderr={stderr:?}",
        output.status
    ))
}

fn write_packet(out: &Path, report: &Value) -> Result<(), String> {
    reject_existing_output(out)?;
    fs::create_dir(out).map_err(|error| {
        format!(
            "failed to create workflow packet {}: {error}",
            out.display()
        )
    })?;
    let json = serde_json::to_string_pretty(report)
        .map_err(|error| format!("failed to serialize workflow disposition: {error}"))?;
    let markdown = render_markdown(report)?;
    fs::write(out.join(REPORT_JSON), format!("{json}\n"))
        .map_err(|error| format!("failed to write workflow disposition JSON: {error}"))?;
    fs::write(out.join(REPORT_MD), markdown)
        .map_err(|error| format!("failed to write workflow disposition Markdown: {error}"))?;
    let files = BTreeMap::from([
        (
            REPORT_JSON,
            digest_file(&out.join(REPORT_JSON), "workflow disposition JSON")?,
        ),
        (
            REPORT_MD,
            digest_file(&out.join(REPORT_MD), "workflow disposition Markdown")?,
        ),
    ]);
    let index = serde_json::json!({
        "schema": PACKET_SCHEMA,
        "status": json_string(report, "status"),
        "complete": true,
        "files": files,
    });
    fs::write(
        out.join(PACKET_INDEX),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&index)
                .map_err(|error| format!("failed to serialize workflow packet index: {error}"))?
        ),
    )
    .map_err(|error| format!("failed to write workflow packet index: {error}"))?;
    println!("Wrote {}", out.join(REPORT_JSON).display());
    println!("Wrote {}", out.join(REPORT_MD).display());
    println!("Wrote {}", out.join(PACKET_INDEX).display());
    Ok(())
}

fn render_markdown(report: &Value) -> Result<String, String> {
    Ok(format!(
        "# Source Promotion Admission Workflow\n\n- Status: `{}`\n- Operation mode: `{}`\n- Execution profile: `{}`\n- Source parent: `{}`\n- W7 parent: `{}`\n- Reviewed tree: `{}`\n\nThis disposition grants no ref, merge, release, or publication authority.\n",
        required_json_string(report, "status")?,
        required_json_string(report, "operation_mode")?,
        required_json_string(report, "execution_profile")?,
        required_json_string(report, "source_parent_sha")?,
        required_json_string(report, "w7_peeled_sha")?,
        required_json_string(report, "reviewed_tree_sha")?,
    ))
}

fn relative_packet_state(workspace: &Path, root: &Path, report: &str) -> Value {
    let relative = root
        .strip_prefix(workspace)
        .ok()
        .map(|path| path.to_string_lossy().replace('\\', "/"));
    let value = read_optional_json(&root.join(report));
    serde_json::json!({
        "path": relative,
        "available": value.is_some(),
        "status": value.as_ref().and_then(|report| json_string(report, "status")),
        "schema": value.as_ref().and_then(|report| json_string(report, "schema")),
    })
}

fn validate_workspace(workspace: &Path, out: &Path) -> Result<(), String> {
    validate_directory(workspace, "runner-owned workspace root")?;
    let current_dir = std::env::current_dir()
        .map_err(|error| format!("failed to read current directory: {error}"))?;
    let workspace_absolute = if workspace.is_absolute() {
        workspace.to_path_buf()
    } else {
        current_dir.join(workspace)
    };
    let workspace = workspace
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize workspace root: {error}"))?;
    if out
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("workflow output contains a parent-directory escape".to_string());
    }
    let absolute_out = if out.is_absolute() {
        out.to_path_buf()
    } else {
        current_dir.join(out)
    };
    if !absolute_out.starts_with(&workspace_absolute) {
        return Err("workflow output must stay inside runner-owned workspace root".to_string());
    }
    let out_parent = absolute_out
        .parent()
        .ok_or_else(|| "workflow output has no parent".to_string())?;
    fs::create_dir_all(out_parent)
        .map_err(|error| format!("failed to create workflow output parent: {error}"))?;
    let out_parent = out_parent
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize workflow output parent: {error}"))?;
    if !out_parent.starts_with(&workspace) {
        return Err("workflow output must stay inside runner-owned workspace root".to_string());
    }
    Ok(())
}

fn parse_args(
    args: &[String],
    command: &str,
    allowed: &[&str],
) -> Result<BTreeMap<String, String>, String> {
    if args.first().map(String::as_str) != Some(command) {
        return Err(usage());
    }
    let allowed = allowed.iter().copied().collect::<BTreeSet<_>>();
    let mut values = BTreeMap::new();
    let mut index = 1;
    while index < args.len() {
        let key = args[index].as_str();
        if !allowed.contains(key) {
            return Err(format!("unknown option {key}\n{}", usage()));
        }
        let value = args
            .get(index + 1)
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| format!("missing value for {key}"))?;
        if values.insert(key.to_string(), value.clone()).is_some() {
            return Err(format!("duplicate option {key}"));
        }
        index += 2;
    }
    for key in allowed {
        required(&values, key)?;
    }
    Ok(values)
}

fn usage() -> String {
    "usage: cargo xtask source-promotion (run-admission-workflow <exact inputs> | verify-admission-workflow --packet <dir> | enforce-admission-workflow --packet <dir> --expected-status admitted)".to_string()
}

fn required<'a>(values: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, String> {
    values
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("missing {key}"))
}

fn required_hex(
    values: &BTreeMap<String, String>,
    key: &str,
    width: usize,
) -> Result<String, String> {
    let value = required(values, key)?;
    validate_hex(value, width, key)?;
    Ok(value.to_string())
}

fn validate_hex(value: &str, width: usize, label: &str) -> Result<(), String> {
    if value.len() != width
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!(
            "{label} must be exactly {width} lowercase hexadecimal characters"
        ));
    }
    Ok(())
}

fn validate_ref(value: &str) -> Result<(), String> {
    if !value.starts_with("refs/tags/ripr-release-")
        || value.contains("..")
        || value.contains(' ')
        || value.ends_with('/')
    {
        return Err("protected W7 ref must be a full refs/tags/ripr-release-* ref".to_string());
    }
    Ok(())
}

fn safe_relative(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("locator path must be a non-empty safe relative path".to_string());
    }
    Ok(())
}

fn reject_existing_output(path: &Path) -> Result<(), String> {
    if fs::symlink_metadata(path).is_ok() {
        return Err(format!("output already exists: {}", path.display()));
    }
    Ok(())
}

fn validate_directory(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {label} {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("{label} must be a non-symlink directory"));
    }
    Ok(())
}

fn digest_file(path: &Path, label: &str) -> Result<String, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {label} {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("{label} must be a non-symlink regular file"));
    }
    let bytes = fs::read(path)
        .map_err(|error| format!("failed to read {label} {}: {error}", path.display()))?;
    Ok(digest_bytes(&bytes))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn workflow_identity(report: &Value) -> Result<String, String> {
    let locators = report
        .get("locators")
        .ok_or_else(|| "workflow disposition is missing locators".to_string())?;
    let locator_digest = |key: &str| {
        locators
            .get(key)
            .and_then(|value| json_string(value, "sha256"))
            .unwrap_or("not_required")
    };
    let fields = [
        required_json_string(report, "execution_profile")?,
        required_json_string(report, "operation_mode")?,
        required_json_string(report, "source_repository")?,
        required_json_string(report, "source_parent_sha")?,
        required_json_string(report, "workflow_source_sha")?,
        required_json_string(report, "trusted_checker_identity")?,
        required_json_string(report, "swarm_repository")?,
        required_json_string(report, "protected_w7_ref")?,
        required_json_string(report, "w7_peeled_sha")?,
        required_json_string(report, "reviewed_tree_sha")?,
        required_json_string(report, "receipt_schema")?,
        required_json_string(report, "fixture_identity")?,
        required_json_string(report, "controller_repository")?,
        locator_digest("preflight"),
        locator_digest("resolution_manifest"),
        locator_digest("validation_packet"),
        locator_digest("integration_packet"),
        locator_digest("qualification_receipt"),
    ];
    Ok(digest_bytes(fields.join(":").as_bytes()))
}

fn read_json(path: &Path, label: &str) -> Result<Value, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("failed to read {label} {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("malformed {label} JSON: {error}"))
}

fn read_optional_json(path: &Path) -> Option<Value> {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

fn json_string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn json_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn required_json_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    json_string(value, key).ok_or_else(|| format!("workflow disposition is missing {key}"))
}

fn required_u64(value: &Value, key: &str) -> Result<u64, String> {
    json_u64(value, key).ok_or_else(|| format!("workflow disposition is missing {key}"))
}

fn path_text(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| format!("path is not valid UTF-8: {}", path.display()))
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn j5_negative_is_fail_closed_before_constructor() -> Result<(), String> {
        let report = test_report("rejected", "constructor_dry_run", "j5_negative", 0)?;
        validate_report(&report)
    }

    #[test]
    fn j5_negative_rejects_constructor_attempt() -> Result<(), String> {
        let report = test_report("rejected", "constructor_dry_run", "j5_negative", 1)?;
        let Err(_) = validate_report(&report) else {
            return Err("J5 negative constructor attempt unexpectedly passed".to_string());
        };
        Ok(())
    }

    #[test]
    fn admitted_constructor_requires_exactly_one_unreferenced_object() -> Result<(), String> {
        let zero = test_report("admitted", "constructor_dry_run", "positive_synthetic", 0)?;
        let two = test_report("admitted", "constructor_dry_run", "positive_synthetic", 2)?;
        if validate_report(&zero).is_ok() || validate_report(&two).is_ok() {
            return Err("constructor attempt cardinality escaped verifier".to_string());
        }
        let one = test_report("admitted", "constructor_dry_run", "positive_synthetic", 1)?;
        validate_report(&one)?;
        Ok(())
    }

    #[test]
    fn verifier_rejects_every_forbidden_attempt_counter() -> Result<(), String> {
        for key in [
            "local_ref_attempts",
            "remote_push_attempts",
            "merge_command_attempts",
            "release_or_publication_attempts",
        ] {
            let mut report = test_report("admitted", "admit_only", "positive_synthetic", 0)?;
            report["attempts"][key] = Value::from(1);
            assert!(validate_report(&report).is_err(), "counter {key} escaped");
        }
        Ok(())
    }

    #[test]
    fn parser_closes_profile_and_mode_enums() -> Result<(), String> {
        let Err(_) = OperationMode::parse("publish") else {
            return Err("publication operation mode unexpectedly passed".to_string());
        };
        let Err(_) = ExecutionProfile::parse("caller_command") else {
            return Err("caller-selected execution profile unexpectedly passed".to_string());
        };
        Ok(())
    }

    #[test]
    fn exact_identity_binding_rejects_moved_ref_and_sidecar() -> Result<(), String> {
        let report = test_report("admitted", "admit_only", "positive_synthetic", 0)?;
        for (pointer, replacement) in [
            (
                "/protected_w7_ref",
                Value::String("refs/tags/ripr-release-moved".to_string()),
            ),
            ("/locators/preflight/sha256", Value::String("2".repeat(64))),
        ] {
            let mut moved = report.clone();
            let slot = moved
                .pointer_mut(pointer)
                .ok_or_else(|| format!("test report is missing {pointer}"))?;
            *slot = replacement;
            let Err(_) = validate_report(&moved) else {
                return Err(format!("moved identity escaped verifier at {pointer}"));
            };
        }
        Ok(())
    }

    #[test]
    fn synthetic_paths_are_root_independent() -> Result<(), String> {
        let left = Path::new("one/root/synthetic-fixture/fixture-repository/.git/evidence.json");
        let right = Path::new("another/root/synthetic-fixture/fixture-repository/.git/evidence.json");
        if stable_synthetic_path(left) != stable_synthetic_path(right) {
            return Err("synthetic locator retained its absolute root".to_string());
        }
        Ok(())
    }

    fn test_report(
        status: &str,
        mode: &str,
        profile: &str,
        commit_tree: u64,
    ) -> Result<Value, String> {
        let mut report = serde_json::json!({
            "schema": SCHEMA,
            "phase": "final",
            "status": status,
            "complete": true,
            "operation_mode": mode,
            "execution_profile": profile,
            "fixture_identity": "a".repeat(64),
            "controller_repository": "synthetic-fixture/fixture-repository",
            "workflow_identity_sha256": null,
            "source_repository": SOURCE_REPOSITORY,
            "source_parent_sha": "a".repeat(40),
            "workflow_source_sha": "a".repeat(40),
            "trusted_checker_identity": format!("source-owned-xtask@{}", "a".repeat(40)),
            "swarm_repository": SWARM_REPOSITORY,
            "protected_w7_ref": "refs/tags/ripr-release-fixture-w7",
            "w7_peeled_sha": "b".repeat(40),
            "reviewed_tree_sha": "c".repeat(40),
            "receipt_schema": SUPPORTED_RECEIPT_SCHEMA,
            "locators": {
                "preflight": {"sha256": "d".repeat(64)},
                "resolution_manifest": {"sha256": "e".repeat(64)},
                "validation_packet": {"sha256": "f".repeat(64)},
                "integration_packet": {"sha256": "1".repeat(64)},
                "qualification_receipt": {"status": "not_required"},
            },
            "producer": {
                "normalized_exit_code": if status == "admitted" { 0 } else { 1 },
                "trusted_builder_state": "passed",
                "admission_state": if status == "admitted" { "passed" } else { "rejected" },
                "constructor_state": "not_run_before_upload_and_enforcement",
            },
            "attempts": {
                "admission_receipt_available": true,
                "construction_receipt_available": mode == "constructor_dry_run",
                "constructor_refs_unchanged": commit_tree == 1,
                "constructor_object_unreferenced": commit_tree == 1,
                "constructor_final_identity_reread_passed": commit_tree == 1,
                "constructor_commit_tree_attempts": commit_tree,
                "local_ref_attempts": 0,
                "remote_push_attempts": 0,
                "merge_command_attempts": 0,
                "release_or_publication_attempts": 0,
                "release_or_publication_command_reachable": false,
                "release_or_publication_proof": "closed workflow harness dispatch contains no publication subcommand",
            },
        });
        report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
        Ok(report)
    }
}
