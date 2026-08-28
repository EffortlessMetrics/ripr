//! Source-owned workflow harness for one exact source-promotion admission.
//!
//! This layer normalizes the existing controller packets for hosted workflow
//! transport. It deliberately has no ref, push, merge, release, or publication
//! operation. Rejected controller evidence is still a complete workflow packet;
//! enforcement is a separate command so artifact upload can happen first.

use super::source_promotion_admission_fixture::{
    LocatorMaterial, SyntheticFixture, SyntheticProfile, prepare_source_owned_fixture,
    verify_reviewed_tree_carrier, write_bound_qualification,
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
const REQUEST_SCHEMA: &str = "ripr.source_promotion_admission_request.v1";
const LOCATOR_SCHEMA: &str = "ripr.source_promotion_artifact_locator.v1";
const SUPPORTED_RECEIPT_SCHEMA: &str = SCHEMA;
const SOURCE_REPOSITORY: &str = "EffortlessMetrics/ripr";
const SWARM_REPOSITORY: &str = "EffortlessMetrics/ripr-swarm";
const REPORT_JSON: &str = "workflow-disposition.json";
const REPORT_MD: &str = "workflow-disposition.md";
const PACKET_INDEX: &str = "packet-index.json";
const MAX_CLOSURE_FILES: usize = 512;
const MAX_CLOSURE_BYTES: u64 = 32 * 1024 * 1024;
const LIVE_CONTROLLER_REPOSITORY: &str = "live-controller-repository";
const SYNTHETIC_CONTROLLER_REPOSITORY: &str = "synthetic-fixture/fixture-repository";

const RUN_KEYS: &[&str] = &[
    "--source-repository",
    "--source-parent-sha",
    "--workflow-source-sha",
    "--trusted-checker-identity",
    "--swarm-repository",
    "--protected-w7-ref",
    "--w7-peeled-sha",
    "--reviewed-tree-sha",
    "--reviewed-tree-carrier-sha",
    "--preflight-locator",
    "--resolution-manifest-locator",
    "--validation-packet-locator",
    "--integration-packet-locator",
    "--qualification-receipt-locator",
    "--receipt-schema",
    "--operation-mode",
    "--execution-profile",
    "--requested-identity",
    "--requested-identity-sha256",
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

#[derive(Clone, Debug)]
struct ClosureSource {
    source: PathBuf,
    destination: PathBuf,
}

#[derive(Clone, Debug)]
struct InlineClosureSource {
    bytes: Vec<u8>,
    destination: PathBuf,
}

#[derive(Debug)]
struct RequestedIdentity {
    value: Value,
    bytes: Vec<u8>,
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
    reviewed_tree_carrier: String,
    preflight: Locator,
    resolution: Locator,
    validation: Locator,
    integration: Locator,
    qualification: Locator,
    receipt_schema: String,
    mode: OperationMode,
    profile: ExecutionProfile,
    requested_identity_bytes: Vec<u8>,
    requested_identity_sha256: String,
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
    let attempts = normalized_attempts(&admission_report, &construction_report, false);
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
        "requested_identity_sha256": options.requested_identity_sha256,
        "controller_repository": stable_controller_repository(&options)?,
        "source_repository": options.source_repository,
        "source_parent_sha": options.source_parent,
        "workflow_source_sha": options.workflow_source_sha,
        "trusted_checker_identity": options.trusted_checker_identity,
        "swarm_repository": options.swarm_repository,
        "protected_w7_ref": options.swarm_ref,
        "w7_peeled_sha": options.swarm_parent,
        "reviewed_tree_sha": options.reviewed_tree,
        "reviewed_tree_carrier_sha": options.reviewed_tree_carrier,
        "receipt_schema": options.receipt_schema,
        "locators": {
            "preflight": options.preflight.value,
            "resolution_manifest": options.resolution.value,
            "validation_packet": options.validation.value,
            "integration_packet": options.integration.value,
            "qualification_receipt": qualification.value,
        },
        "controller_packets": {
            "trusted_builder": packet_state(&builder_out, "trusted-builder.json", "evidence/trusted-builder"),
            "resolved_tree_admission": packet_state(&admission_out, "resolved-tree-admission.json", "evidence/resolved-tree-admission"),
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
    let closure =
        admission_closure_sources(&options, &builder_out, &admission_out, &qualification)?;
    write_packet(
        &options.out,
        &report,
        &closure,
        &[InlineClosureSource {
            bytes: options.requested_identity_bytes.clone(),
            destination: PathBuf::from("requested-identity.json"),
        }],
    )?;
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
    let evidence = admission_packet.join("evidence");
    validate_directory(&evidence, "downloaded admission evidence closure")?;
    let mut construction_report = None;
    let mut construction_error = None;
    let construction_out = workspace.join("exact-join-construction");
    if mode == OperationMode::ConstructorDryRun {
        let admission_controller = evidence.join("resolved-tree-admission");
        let validation_index = evidence
            .join("locators/validation_packet")
            .join(PACKET_INDEX);
        let integration_index = closure_index_path(
            &evidence.join("locators/integration_packet"),
            &report["locators"]["integration_packet"],
        )?;
        let preflight = evidence.join("locators/preflight/input");
        let resolution = evidence.join("locators/resolution_manifest/input");
        let qualification = evidence.join("locators/qualification_receipt/input");
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
        construction_error = match controller_repository_path(&workspace, &report) {
            Ok(controller_repo) => invoke_controller(&controller_repo, &command).err(),
            Err(error) => Some(error),
        };
        construction_report =
            read_optional_json(&construction_out.join("exact-join-construction.json"));
    }
    let mut attempts = normalized_attempts(
        &None,
        &construction_report,
        mode == OperationMode::ConstructorDryRun,
    );
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
    report["producer"]["normalized_exit_code"] = Value::from(if constructor_ok { 0 } else { 1 });
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
    if mode == OperationMode::ConstructorDryRun {
        report["controller_packets"]["exact_join_construction"] = packet_state(
            &workspace.join("exact-join-construction"),
            "exact-join-construction.json",
            "evidence/exact-join-construction",
        );
    }
    report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
    let mut closure = vec![ClosureSource {
        source: evidence,
        destination: PathBuf::new(),
    }];
    if let Some(source) = optional_closure_source(
        &construction_out,
        "exact-join-construction",
        "construction evidence for final closure",
    )? {
        closure.push(source);
    }
    write_packet(&out, &report, &closure, &[])?;
    verify_packet(&out)?;
    if constructor_ok {
        Ok(())
    } else {
        Err("source-promotion finalizer produced a complete rejected packet".to_string())
    }
}

fn normalized_attempts(
    admission: &Option<Value>,
    construction: &Option<Value>,
    construction_was_invoked: bool,
) -> Value {
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
    let construction_unknown = construction_was_invoked && !construction_available;
    let commit_tree = if construction_unknown {
        Value::Null
    } else {
        Value::from(
            construction
                .as_ref()
                .and_then(|value| json_u64(value, "commit_tree_attempts"))
                .unwrap_or(0),
        )
    };
    let field = |name: &str| {
        if construction_unknown {
            Value::Null
        } else {
            Value::from(
                admission
                    .as_ref()
                    .and_then(|value| json_u64(value, name))
                    .unwrap_or(0)
                    .saturating_add(
                        construction
                            .as_ref()
                            .and_then(|value| json_u64(value, name))
                            .unwrap_or(0),
                    ),
            )
        }
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
    let values = parse_args(
        args,
        VERIFY,
        &[
            "--packet",
            "--requested-identity",
            "--requested-identity-sha256",
        ],
    )?;
    let packet = Path::new(required(&values, "--packet")?);
    let (report, embedded_request_bytes) = verify_packet_with_request(packet)?;
    let expected_sha256 = required_hex(&values, "--requested-identity-sha256", 64)?;
    let external = Path::new(required(&values, "--requested-identity")?);
    let external_request = read_requested_identity(external, &expected_sha256)?;
    if required_json_string(&report, "requested_identity_sha256")? != expected_sha256 {
        return Err(
            "workflow packet requested identity differs from verifier authority".to_string(),
        );
    }
    if external_request.bytes != embedded_request_bytes {
        return Err("embedded requested identity differs from verifier authority".to_string());
    }
    Ok(())
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
    verify_packet_with_request(root).map(|(report, _)| report)
}

fn verify_packet_with_request(root: &Path) -> Result<(Value, Vec<u8>), String> {
    validate_directory(root, "workflow packet")?;
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
    if !files.contains_key(REPORT_JSON)
        || !files.contains_key(REPORT_MD)
        || !files.keys().any(|name| name.starts_with("evidence/"))
    {
        return Err(
            "workflow packet index must bind reports and the uploaded evidence closure".to_string(),
        );
    }
    let observed = collect_packet_files(root)?;
    if observed.len() != files.len()
        || observed.keys().any(|name| !files.contains_key(name))
        || files.keys().any(|name| !observed.contains_key(name))
    {
        return Err("workflow packet inventory differs from indexed closure".to_string());
    }
    for (name, expected) in files {
        safe_relative(name)?;
        let expected = expected
            .as_str()
            .ok_or_else(|| format!("workflow packet digest for {name} is not a string"))?;
        validate_hex(expected, 64, "workflow packet digest")?;
        if observed.get(name).map(String::as_str) != Some(expected) {
            return Err(format!("workflow packet digest mismatch for {name}"));
        }
    }
    let report = read_json(&root.join(REPORT_JSON), "workflow disposition")?;
    if json_string(&index, "status") != json_string(&report, "status") {
        return Err("workflow packet index status differs from disposition".to_string());
    }
    let observed_markdown = fs::read(root.join(REPORT_MD))
        .map_err(|error| format!("failed to read workflow disposition Markdown: {error}"))?;
    let expected_markdown = render_markdown(&report)?;
    if observed_markdown != expected_markdown.as_bytes() {
        return Err(
            "workflow disposition Markdown differs from canonical JSON rendering".to_string(),
        );
    }
    validate_report(&report)?;
    let requested_identity_bytes = validate_closure_bindings(root, &report)?;
    Ok((report, requested_identity_bytes))
}

fn validate_closure_bindings(root: &Path, report: &Value) -> Result<Vec<u8>, String> {
    let evidence = root.join("evidence");
    validate_directory(&evidence, "workflow evidence closure")?;
    let requested_identity_sha256 = required_json_string(report, "requested_identity_sha256")?;
    let requested_identity = read_requested_identity(
        &evidence.join("requested-identity.json"),
        requested_identity_sha256,
    )?;
    validate_request_against_report(&requested_identity.value, report)?;
    let locators = report
        .get("locators")
        .ok_or_else(|| "workflow disposition is missing locators".to_string())?;
    for (key, fixed) in [
        ("preflight", Some(PathBuf::from("preflight/input"))),
        (
            "resolution_manifest",
            Some(PathBuf::from("resolution_manifest/input")),
        ),
        (
            "validation_packet",
            Some(PathBuf::from("validation_packet").join(PACKET_INDEX)),
        ),
        ("integration_packet", None),
        (
            "qualification_receipt",
            Some(PathBuf::from("qualification_receipt/input")),
        ),
    ] {
        let locator = &locators[key];
        if json_string(locator, "status") == Some("not_required") {
            continue;
        }
        let relative = if let Some(fixed) = fixed {
            fixed
        } else {
            let path = required_json_string(locator, "path")?;
            let name = Path::new(path)
                .file_name()
                .ok_or_else(|| format!("{key} locator path has no filename"))?;
            PathBuf::from(key).join(name)
        };
        let path = evidence.join("locators").join(relative);
        let observed = digest_file(&path, &format!("{key} closure member"))?;
        if observed != required_json_string(locator, "sha256")? {
            return Err(format!(
                "{key} closure digest differs from locator authority"
            ));
        }
    }
    for (key, directory, report_name, expected_schema, expected_status) in [
        (
            "trusted_builder",
            "trusted-builder",
            "trusted-builder.json",
            "ripr.source_promotion_trusted_builder.v1",
            "built",
        ),
        (
            "resolved_tree_admission",
            "resolved-tree-admission",
            "resolved-tree-admission.json",
            "ripr.source_promotion_resolved_tree_admission.v1",
            "admitted",
        ),
    ] {
        let expected_path = format!("evidence/{directory}");
        let expected_packet = packet_state(&evidence.join(directory), report_name, &expected_path);
        if report["controller_packets"][key] != expected_packet {
            return Err(format!("{key} summary differs from closure evidence"));
        }
        if expected_packet["available"].as_bool() == Some(true) {
            let receipt = read_json(
                &evidence.join(directory).join(report_name),
                &format!("{key} closure receipt"),
            )?;
            if json_string(report, "status") == Some("admitted")
                && (json_string(&receipt, "schema") != Some(expected_schema)
                    || json_string(&receipt, "status") != Some(expected_status))
            {
                return Err(format!(
                    "admitted workflow packet has invalid {key} evidence"
                ));
            }
        } else if json_string(report, "status") == Some("admitted") {
            return Err(format!(
                "admitted workflow packet is missing {key} evidence"
            ));
        }
    }
    let construction = &report["controller_packets"]["exact_join_construction"];
    let qualification_required = json_string(report, "operation_mode")
        == Some("constructor_dry_run")
        && (json_string(report, "phase") == Some("final")
            || json_string(report, "status") == Some("admitted"));
    let construction_required = json_string(report, "phase") == Some("final")
        && json_string(report, "operation_mode") == Some("constructor_dry_run");
    if construction_required {
        let expected_construction = packet_state(
            &evidence.join("exact-join-construction"),
            "exact-join-construction.json",
            "evidence/exact-join-construction",
        );
        if construction != &expected_construction {
            return Err(
                "exact_join_construction summary differs from closure evidence".to_string(),
            );
        }
        let available = expected_construction["available"].as_bool() == Some(true);
        if json_string(report, "status") == Some("admitted") && !available {
            return Err(
                "admitted constructor workflow packet is missing construction evidence".to_string(),
            );
        }
        if available {
            let receipt = read_json(
                &evidence
                    .join("exact-join-construction")
                    .join("exact-join-construction.json"),
                "exact_join_construction closure receipt",
            )?;
            if json_string(construction, "schema") != json_string(&receipt, "schema")
                || json_string(construction, "status") != json_string(&receipt, "status")
            {
                return Err(
                    "exact_join_construction summary differs from closure receipt".to_string(),
                );
            }
            if json_string(report, "status") == Some("admitted")
                && (json_string(&receipt, "schema")
                    != Some("ripr.source_promotion_exact_join_construction.v1")
                    || json_string(&receipt, "status") != Some("constructed"))
            {
                return Err(
                    "admitted workflow packet has invalid construction evidence".to_string()
                );
            }
        }
    } else {
        let expected = serde_json::json!({
            "path": null,
            "available": false,
            "status": "not_run",
            "schema": null,
        });
        if construction != &expected {
            return Err(
                "workflow packet outside final constructor has a construction summary".to_string(),
            );
        }
        if fs::symlink_metadata(evidence.join("exact-join-construction")).is_ok() {
            return Err(
                "workflow packet outside final constructor contains construction evidence"
                    .to_string(),
            );
        }
    }
    let packet_passed = |key: &str, expected_status: &str| {
        report["controller_packets"][key]["available"].as_bool() == Some(true)
            && json_string(&report["controller_packets"][key], "status") == Some(expected_status)
    };
    let builder_passed = packet_passed("trusted_builder", "built");
    let admission_passed = packet_passed("resolved_tree_admission", "admitted");
    let controller_prefix_passed = builder_passed && admission_passed;
    let builder_available =
        report["controller_packets"]["trusted_builder"]["available"].as_bool() == Some(true);
    let admission_available = report["controller_packets"]["resolved_tree_admission"]["available"]
        .as_bool()
        == Some(true);
    if !builder_passed && builder_available {
        super::source_promotion_control::replay_rejected_builder_packet(
            &evidence.join("trusted-builder"),
            required_json_string(report, "source_parent_sha")?,
            required_json_string(report, "workflow_source_sha")?,
        )?;
        let expected_attempts = normalized_attempts(&None, &None, false);
        if report.get("attempts") != Some(&expected_attempts) {
            return Err(
                "workflow attempt summary differs from rejected builder evidence".to_string(),
            );
        }
    }
    if !builder_passed && !builder_available {
        let expected_attempts = normalized_attempts(&None, &None, false);
        if report.get("attempts") != Some(&expected_attempts) {
            return Err(
                "workflow attempt summary differs from unavailable builder evidence".to_string(),
            );
        }
    }
    let needs_rejected_admission_replay =
        builder_passed && !admission_passed && admission_available;
    if needs_rejected_admission_replay || controller_prefix_passed {
        let integration_index = closure_index_path(
            &evidence.join("locators/integration_packet"),
            &locators["integration_packet"],
        )?;
        let qualification =
            qualification_required.then(|| evidence.join("locators/qualification_receipt/input"));
        let construction_packet = (construction_required
            && json_string(report, "status") == Some("admitted"))
        .then(|| evidence.join("exact-join-construction"));
        let validation_packet = evidence.join("locators/validation_packet");
        let builder_packet = evidence.join("trusted-builder");
        let admission_packet = evidence.join("resolved-tree-admission");
        let preflight = evidence.join("locators/preflight/input");
        let resolution_manifest = evidence.join("locators/resolution_manifest/input");
        let replay_input = super::source_promotion_control::AdmissionClosureReplayInput {
            validation_packet: &validation_packet,
            builder_packet: &builder_packet,
            admission_packet: &admission_packet,
            integration_index: &integration_index,
            preflight: &preflight,
            resolution_manifest: &resolution_manifest,
            qualification_receipt: qualification.as_deref(),
            construction_packet: construction_packet.as_deref(),
            source_parent: required_json_string(report, "source_parent_sha")?,
            swarm_parent: required_json_string(report, "w7_peeled_sha")?,
            join_tree: required_json_string(report, "reviewed_tree_sha")?,
            protected_w7_ref: required_json_string(report, "protected_w7_ref")?,
            preflight_sha256: required_json_string(&locators["preflight"], "sha256")?,
            resolution_sha256: required_json_string(&locators["resolution_manifest"], "sha256")?,
            integration_index_sha256: required_json_string(
                &locators["integration_packet"],
                "sha256",
            )?,
            qualification_sha256: qualification
                .as_ref()
                .map(|_| required_json_string(&locators["qualification_receipt"], "sha256"))
                .transpose()?,
        };
        if needs_rejected_admission_replay {
            let admission = super::source_promotion_control::replay_rejected_admission_closure(
                &replay_input,
                json_string(report, "execution_profile") == Some("j5_negative"),
            )?;
            let expected_attempts = normalized_attempts(&Some(admission), &None, false);
            if report.get("attempts") != Some(&expected_attempts) {
                return Err(
                    "workflow attempt summary differs from rejected admission evidence".to_string(),
                );
            }
        } else {
            let replay = super::source_promotion_control::replay_admitted_closure(&replay_input)?;
            let rejected_construction = if construction_required
                && json_string(report, "status") == Some("rejected")
                && construction["available"].as_bool() == Some(true)
            {
                Some(
                    super::source_promotion_control::replay_rejected_construction_packet(
                        &evidence.join("exact-join-construction"),
                        &replay_input,
                    )?,
                )
            } else {
                None
            };
            let replayed_attempts = normalized_attempts(
                &Some(replay.admission),
                if rejected_construction.is_some() {
                    &rejected_construction
                } else {
                    &replay.construction
                },
                construction_required,
            );
            if report.get("attempts") != Some(&replayed_attempts) {
                return Err(
                    "workflow attempt summary differs from replayed controller receipts"
                        .to_string(),
                );
            }
        }
    }
    if builder_passed && !admission_passed && !admission_available {
        let expected_attempts = normalized_attempts(&None, &None, false);
        if report.get("attempts") != Some(&expected_attempts) {
            return Err(
                "workflow attempt summary differs from unavailable admission evidence".to_string(),
            );
        }
    }
    Ok(requested_identity.bytes)
}

fn validate_disposition_semantics(
    report: &Value,
    status: &str,
    phase: &str,
    mode: OperationMode,
    profile: ExecutionProfile,
) -> Result<(), String> {
    let controller_packets = report
        .get("controller_packets")
        .ok_or_else(|| "workflow disposition is missing controller packets".to_string())?;
    exact_object_fields(
        controller_packets,
        "workflow controller packets",
        &[
            "trusted_builder",
            "resolved_tree_admission",
            "exact_join_construction",
        ],
    )?;
    for key in [
        "trusted_builder",
        "resolved_tree_admission",
        "exact_join_construction",
    ] {
        exact_object_fields(
            &controller_packets[key],
            &format!("{key} controller summary"),
            &["path", "available", "status", "schema"],
        )?;
    }

    let packet_passed = |key: &str, expected_status: &str| {
        controller_packets[key]["available"].as_bool() == Some(true)
            && json_string(&controller_packets[key], "status") == Some(expected_status)
    };
    let constructor_state = match (phase, mode, status) {
        ("admission", _, _) => "not_run_before_upload_and_enforcement",
        ("final", OperationMode::AdmitOnly, _) => "not_requested",
        ("final", OperationMode::ConstructorDryRun, "admitted") => "passed",
        ("final", OperationMode::ConstructorDryRun, "rejected") => "rejected",
        _ => return Err("workflow disposition has unsupported phase semantics".to_string()),
    };
    let expected_producer = serde_json::json!({
        "normalized_exit_code": if status == "admitted" { 0 } else { 1 },
        "trusted_builder_state": if packet_passed("trusted_builder", "built") { "passed" } else { "rejected" },
        "admission_state": if packet_passed("resolved_tree_admission", "admitted") { "passed" } else { "rejected" },
        "constructor_state": constructor_state,
    });
    if report.get("producer") != Some(&expected_producer) {
        return Err(
            "workflow disposition producer state differs from controller evidence".to_string(),
        );
    }
    let builder_passed = packet_passed("trusted_builder", "built");
    let admission_passed = packet_passed("resolved_tree_admission", "admitted");
    let construction_completed = packet_passed("exact_join_construction", "constructed");
    let admission_available =
        controller_packets["resolved_tree_admission"]["available"].as_bool() == Some(true);
    if !builder_passed && admission_available {
        return Err(
            "workflow disposition has admission evidence after a failed builder stage".to_string(),
        );
    }
    let reachable = match (phase, mode, status) {
        ("admission", _, "admitted") => builder_passed && admission_passed,
        ("admission", _, "rejected") => !builder_passed || !admission_passed,
        ("final", OperationMode::AdmitOnly, "admitted") => builder_passed && admission_passed,
        ("final", OperationMode::AdmitOnly, "rejected") => false,
        ("final", OperationMode::ConstructorDryRun, "admitted") => {
            builder_passed && admission_passed && construction_completed
        }
        ("final", OperationMode::ConstructorDryRun, "rejected") => {
            builder_passed && admission_passed && !construction_completed
        }
        _ => false,
    };
    if !reachable {
        return Err(
            "workflow disposition phase, mode, status, and stages are unreachable".to_string(),
        );
    }
    if profile == ExecutionProfile::J5Negative
        && (phase != "admission" || status != "rejected" || !builder_passed || admission_passed)
    {
        return Err("j5_negative disposition moved outside its failed admission stage".to_string());
    }

    let failure_reasons = report
        .get("failure_reasons")
        .and_then(Value::as_array)
        .ok_or_else(|| "workflow disposition is missing failure_reasons".to_string())?;
    if status == "admitted" && !failure_reasons.is_empty() {
        return Err("admitted workflow disposition has failure reasons".to_string());
    }
    if status == "rejected" && failure_reasons.is_empty() {
        return Err("rejected workflow disposition has no failure reason".to_string());
    }
    let mut prior: Option<&str> = None;
    for reason in failure_reasons {
        let reason = reason
            .as_str()
            .filter(|reason| !reason.trim().is_empty() && !reason.contains(['\n', '\r', '\0']))
            .ok_or_else(|| "workflow disposition has malformed failure reason".to_string())?;
        if prior.is_some_and(|value| value >= reason) {
            return Err(
                "workflow disposition failure reasons must be sorted and unique".to_string(),
            );
        }
        prior = Some(reason);
    }
    Ok(())
}

fn validate_report(report: &Value) -> Result<(), String> {
    exact_object_fields_with_optional(
        report,
        "workflow disposition",
        &[
            "schema",
            "phase",
            "status",
            "operation_mode",
            "execution_profile",
            "fixture_identity",
            "workflow_identity_sha256",
            "requested_identity_sha256",
            "controller_repository",
            "source_repository",
            "source_parent_sha",
            "workflow_source_sha",
            "trusted_checker_identity",
            "swarm_repository",
            "protected_w7_ref",
            "w7_peeled_sha",
            "reviewed_tree_sha",
            "reviewed_tree_carrier_sha",
            "receipt_schema",
            "locators",
            "controller_packets",
            "producer",
            "attempts",
            "failure_reasons",
            "complete",
        ],
        &["non_claims"],
    )?;
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
    if required_json_string(report, "controller_repository")?
        != expected_controller_repository(profile)
    {
        return Err("workflow disposition controller repository moved".to_string());
    }
    if required_json_string(report, "source_repository")? != SOURCE_REPOSITORY
        || required_json_string(report, "swarm_repository")? != SWARM_REPOSITORY
    {
        return Err("workflow disposition repository authority moved".to_string());
    }
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
    validate_reviewed_tree_carrier_identity(
        profile,
        required_json_string(report, "reviewed_tree_carrier_sha")?,
    )?;
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
    validate_hex(
        required_json_string(report, "requested_identity_sha256")?,
        64,
        "requested_identity_sha256",
    )?;
    validate_locator_provenance(report, profile, mode, status)?;
    let expected_workflow_identity = workflow_identity(report)?;
    if json_string(report, "workflow_identity_sha256") != Some(expected_workflow_identity.as_str())
    {
        return Err("workflow disposition exact identity binding moved".to_string());
    }
    let attempts = report
        .get("attempts")
        .ok_or_else(|| "workflow disposition is missing attempts".to_string())?;
    exact_object_fields(
        attempts,
        "workflow attempts",
        &[
            "admission_receipt_available",
            "construction_receipt_available",
            "constructor_refs_unchanged",
            "constructor_object_unreferenced",
            "constructor_final_identity_reread_passed",
            "constructor_commit_tree_attempts",
            "local_ref_attempts",
            "remote_push_attempts",
            "merge_command_attempts",
            "release_or_publication_attempts",
            "release_or_publication_command_reachable",
            "release_or_publication_proof",
        ],
    )?;
    if attempts
        .get("release_or_publication_command_reachable")
        .and_then(Value::as_bool)
        != Some(false)
        || json_string(attempts, "release_or_publication_proof")
            != Some("closed workflow harness dispatch contains no publication subcommand")
    {
        return Err("workflow disposition lacks closed publication reachability proof".to_string());
    }
    let admission_receipt_available = attempts
        .get("admission_receipt_available")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            "workflow disposition is missing admission receipt availability".to_string()
        })?;
    if (status == "admitted" || phase == "final") && !admission_receipt_available {
        return Err("workflow disposition has unavailable admission attempt evidence".to_string());
    }
    let construction_receipt_available = attempts
        .get("construction_receipt_available")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            "workflow disposition is missing construction receipt availability".to_string()
        })?;
    if status == "admitted"
        && phase == "final"
        && mode == OperationMode::ConstructorDryRun
        && !construction_receipt_available
    {
        return Err(
            "workflow disposition has unavailable construction attempt evidence".to_string(),
        );
    }
    let construction_attempts_unknown = status == "rejected"
        && phase == "final"
        && mode == OperationMode::ConstructorDryRun
        && !construction_receipt_available
        && profile != ExecutionProfile::J5Negative;
    let commit_tree = if construction_attempts_unknown {
        if !attempts["constructor_commit_tree_attempts"].is_null() {
            return Err(
                "rejected constructor without a receipt must report commit-tree attempts as unknown"
                    .to_string(),
            );
        }
        None
    } else {
        Some(required_u64(attempts, "constructor_commit_tree_attempts")?)
    };
    for key in [
        "local_ref_attempts",
        "remote_push_attempts",
        "merge_command_attempts",
    ] {
        if construction_attempts_unknown {
            if !attempts[key].is_null() {
                return Err(format!(
                    "rejected constructor without a receipt must report {key} as unknown"
                ));
            }
        } else if required_u64(attempts, key)? != 0 {
            return Err(format!("workflow disposition records forbidden {key}"));
        }
    }
    if required_u64(attempts, "release_or_publication_attempts")? != 0 {
        return Err(
            "workflow disposition records forbidden release_or_publication_attempts".to_string(),
        );
    }
    if mode == OperationMode::AdmitOnly && commit_tree != Some(0) {
        return Err("admit_only disposition records a constructor attempt".to_string());
    }
    if commit_tree.is_some_and(|attempts| attempts > 1) {
        return Err("constructor dry-run attempted more than one commit-tree".to_string());
    }
    if status == "admitted"
        && phase == "final"
        && mode == OperationMode::ConstructorDryRun
        && commit_tree != Some(1)
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
    if profile == ExecutionProfile::J5Negative
        && (phase != "admission"
            || status != "rejected"
            || construction_receipt_available
            || commit_tree != Some(0))
    {
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
    validate_disposition_semantics(report, status, phase, mode, profile)?;
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
    let reviewed_tree_carrier = required(&values, "--reviewed-tree-carrier-sha")?.to_string();
    let swarm_ref = required(&values, "--protected-w7-ref")?.to_string();
    validate_ref(&swarm_ref)?;
    let receipt_schema = required(&values, "--receipt-schema")?.to_string();
    if receipt_schema != SUPPORTED_RECEIPT_SCHEMA {
        return Err(format!("unsupported receipt schema {receipt_schema}"));
    }
    let mode = OperationMode::parse(required(&values, "--operation-mode")?)?;
    let profile = ExecutionProfile::parse(required(&values, "--execution-profile")?)?;
    validate_reviewed_tree_carrier_identity(profile, &reviewed_tree_carrier)?;
    let source_checkout = std::env::current_dir()
        .map_err(|error| format!("failed to locate source checkout: {error}"))?;
    let requested_identity =
        absolute_from(&source_checkout, required(&values, "--requested-identity")?);
    let requested_identity_sha256 = required_hex(&values, "--requested-identity-sha256", 64)?;
    let request = read_requested_identity(&requested_identity, &requested_identity_sha256)?;
    validate_request_against_inputs(&request.value, &values)?;
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
            reviewed_tree_carrier,
            preflight: synthetic_material(&fixture.preflight, "preflight"),
            resolution: synthetic_material(&fixture.resolution, "resolution manifest"),
            validation: synthetic_material(&fixture.validation_packet_index, "validation packet"),
            integration: synthetic_material(&fixture.integration_index, "integration packet"),
            qualification: empty_locator("qualification receipt"),
            receipt_schema,
            mode,
            profile,
            requested_identity_bytes: request.bytes,
            requested_identity_sha256,
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
            &reviewed_tree_carrier,
        )?,
        source_repository,
        source_parent,
        workflow_source_sha,
        trusted_checker_identity,
        swarm_repository,
        swarm_ref,
        swarm_parent,
        reviewed_tree,
        reviewed_tree_carrier,
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
        requested_identity_bytes: request.bytes,
        requested_identity_sha256,
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

fn expected_controller_repository(profile: ExecutionProfile) -> &'static str {
    match profile {
        ExecutionProfile::Live => LIVE_CONTROLLER_REPOSITORY,
        ExecutionProfile::PositiveSynthetic | ExecutionProfile::J5Negative => {
            SYNTHETIC_CONTROLLER_REPOSITORY
        }
    }
}

fn stable_controller_repository(options: &RunOptions) -> Result<String, String> {
    let relative = options
        .controller_repo
        .strip_prefix(&options.workspace_root)
        .map_err(|error| {
            format!("controller repository escaped the runner-owned workspace: {error}")
        })?;
    let relative = relative.to_string_lossy().replace('\\', "/");
    safe_relative(&relative).map_err(|error| {
        format!("controller repository path is not a safe relative path: {error}")
    })?;
    if relative != expected_controller_repository(options.profile) {
        return Err("controller repository differs from the closed execution profile".to_string());
    }
    Ok(relative)
}

fn controller_repository_path(workspace: &Path, report: &Value) -> Result<PathBuf, String> {
    let profile = ExecutionProfile::parse(required_json_string(report, "execution_profile")?)?;
    let relative = required_json_string(report, "controller_repository")?;
    if relative != expected_controller_repository(profile) {
        return Err("workflow disposition controller repository moved".to_string());
    }
    safe_relative(relative).map_err(|error| {
        format!("controller repository path is not a safe relative path: {error}")
    })?;
    let workspace = workspace
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize controller workspace: {error}"))?;
    let repository = workspace.join(relative).canonicalize().map_err(|error| {
        format!("failed to canonicalize isolated controller repository: {error}")
    })?;
    if !repository.starts_with(&workspace) {
        return Err("controller repository escaped the runner-owned workspace".to_string());
    }
    validate_directory(&repository, "isolated controller repository")?;
    Ok(repository)
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
    reviewed_tree_carrier: &str,
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
    let source_url = format!("https://github.com/{SOURCE_REPOSITORY}.git");
    run_git(
        &runtime,
        &["fetch", "--no-tags", &source_url, reviewed_tree_carrier],
        "reviewed tree carrier materialization",
    )?;
    verify_reviewed_tree_carrier(
        &runtime,
        reviewed_tree_carrier,
        source_parent,
        swarm_parent,
        reviewed_tree,
    )?;
    run_git(
        &runtime,
        &["cat-file", "-e", &format!("{reviewed_tree}^{{tree}}")],
        "reviewed tree materialization",
    )?;
    Ok(runtime)
}

fn validate_reviewed_tree_carrier_identity(
    profile: ExecutionProfile,
    carrier: &str,
) -> Result<(), String> {
    match profile {
        ExecutionProfile::Live => validate_hex(carrier, 40, "reviewed tree carrier SHA"),
        ExecutionProfile::PositiveSynthetic | ExecutionProfile::J5Negative
            if carrier == "not_required" =>
        {
            Ok(())
        }
        ExecutionProfile::PositiveSynthetic | ExecutionProfile::J5Negative => Err(
            "exact-J-free synthetic profiles require reviewed tree carrier not_required"
                .to_string(),
        ),
    }
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

fn admission_closure_sources(
    options: &RunOptions,
    builder_out: &Path,
    admission_out: &Path,
    qualification: &Locator,
) -> Result<Vec<ClosureSource>, String> {
    let mut sources = Vec::new();
    for (source, destination) in [
        (builder_out, "trusted-builder"),
        (admission_out, "resolved-tree-admission"),
    ] {
        if fs::symlink_metadata(source).is_ok() {
            sources.push(ClosureSource {
                source: source.to_path_buf(),
                destination: PathBuf::from(destination),
            });
        }
    }
    for (key, locator, siblings) in [
        ("preflight", &options.preflight, false),
        ("resolution_manifest", &options.resolution, false),
        ("validation_packet", &options.validation, true),
        ("integration_packet", &options.integration, true),
        ("qualification_receipt", qualification, false),
    ] {
        if locator.local_path.as_os_str().is_empty() {
            continue;
        }
        let source = if siblings {
            locator
                .local_path
                .parent()
                .ok_or_else(|| format!("{key} locator has no sibling root"))?
                .to_path_buf()
        } else {
            locator.local_path.clone()
        };
        let destination = if siblings {
            PathBuf::from("locators").join(key)
        } else {
            PathBuf::from("locators").join(key).join("input")
        };
        sources.push(ClosureSource {
            source,
            destination,
        });
    }
    Ok(sources)
}

fn optional_closure_source(
    source: &Path,
    destination: &str,
    label: &str,
) -> Result<Option<ClosureSource>, String> {
    match fs::symlink_metadata(source) {
        Ok(_) => Ok(Some(ClosureSource {
            source: source.to_path_buf(),
            destination: PathBuf::from(destination),
        })),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("failed to inspect {label}: {error}")),
    }
}

fn closure_index_path(root: &Path, locator: &Value) -> Result<PathBuf, String> {
    let path = required_json_string(locator, "path")?;
    safe_relative(path)?;
    let name = Path::new(path)
        .file_name()
        .ok_or_else(|| "integration locator path has no filename".to_string())?;
    Ok(root.join(name))
}

fn copy_closure_source(source: &Path, destination: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source).map_err(|error| {
        format!(
            "failed to inspect evidence closure source {}: {error}",
            source.display()
        )
    })?;
    if metadata.file_type().is_symlink() {
        return Err("evidence closure source may not be a symlink".to_string());
    }
    if metadata.is_file() {
        if metadata.len() > MAX_CLOSURE_BYTES {
            return Err("evidence closure exceeds byte budget".to_string());
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create evidence closure parent: {error}"))?;
        }
        fs::copy(source, destination)
            .map_err(|error| format!("failed to copy evidence closure file: {error}"))?;
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err("evidence closure source must be a regular file or directory".to_string());
    }

    let mut pending = vec![(source.to_path_buf(), PathBuf::new())];
    let mut files = 0usize;
    let mut bytes = 0u64;
    while let Some((directory, relative)) = pending.pop() {
        let mut entries = fs::read_dir(&directory)
            .map_err(|error| format!("failed to enumerate evidence closure: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to read evidence closure entry: {error}"))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|name| format!("evidence closure filename is not UTF-8: {name:?}"))?;
            safe_relative(&name)?;
            let entry_relative = relative.join(name);
            let entry_metadata = fs::symlink_metadata(entry.path())
                .map_err(|error| format!("failed to inspect evidence closure entry: {error}"))?;
            if entry_metadata.file_type().is_symlink() {
                return Err("evidence closure contains a symlink".to_string());
            }
            if entry_metadata.is_dir() {
                pending.push((entry.path(), entry_relative));
                continue;
            }
            if !entry_metadata.is_file() {
                return Err("evidence closure contains a non-regular entry".to_string());
            }
            files += 1;
            bytes = bytes.saturating_add(entry_metadata.len());
            if files > MAX_CLOSURE_FILES || bytes > MAX_CLOSURE_BYTES {
                return Err("evidence closure exceeds bounded inventory".to_string());
            }
            let target = destination.join(entry_relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    format!("failed to create evidence closure directory: {error}")
                })?;
            }
            fs::copy(entry.path(), target)
                .map_err(|error| format!("failed to copy evidence closure entry: {error}"))?;
        }
    }
    Ok(())
}

fn collect_packet_files(root: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut files = BTreeMap::new();
    let mut pending = vec![(root.to_path_buf(), PathBuf::new())];
    while let Some((directory, relative)) = pending.pop() {
        let mut entries = fs::read_dir(&directory)
            .map_err(|error| format!("failed to enumerate workflow packet: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to read workflow packet entry: {error}"))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|name| format!("workflow packet filename is not UTF-8: {name:?}"))?;
            let entry_relative = relative.join(name);
            let entry_metadata = fs::symlink_metadata(entry.path())
                .map_err(|error| format!("failed to inspect workflow packet entry: {error}"))?;
            if entry_metadata.file_type().is_symlink() {
                return Err("workflow packet contains a symlink".to_string());
            }
            if entry_metadata.is_dir() {
                pending.push((entry.path(), entry_relative));
                continue;
            }
            if !entry_metadata.is_file() {
                return Err("workflow packet contains a non-regular entry".to_string());
            }
            let relative_text = entry_relative.to_string_lossy().replace('\\', "/");
            safe_relative(&relative_text)?;
            if relative_text == PACKET_INDEX {
                continue;
            }
            let digest = digest_file(&entry.path(), "workflow packet file")?;
            if files.insert(relative_text.clone(), digest).is_some() {
                return Err(format!("duplicate workflow packet path: {relative_text}"));
            }
        }
    }
    Ok(files)
}

fn write_packet(
    out: &Path,
    report: &Value,
    closure: &[ClosureSource],
    inline_closure: &[InlineClosureSource],
) -> Result<(), String> {
    reject_existing_output(out)?;
    fs::create_dir(out).map_err(|error| {
        format!(
            "failed to create workflow packet {}: {error}",
            out.display()
        )
    })?;
    let evidence = out.join("evidence");
    fs::create_dir(&evidence)
        .map_err(|error| format!("failed to create workflow evidence closure: {error}"))?;
    for source in closure {
        copy_closure_source(&source.source, &evidence.join(&source.destination))?;
    }
    for source in inline_closure {
        let destination = source.destination.to_string_lossy().replace('\\', "/");
        safe_relative(&destination)?;
        if source.bytes.len() as u64 > MAX_CLOSURE_BYTES {
            return Err("inline evidence closure member exceeds byte budget".to_string());
        }
        let destination = evidence.join(&source.destination);
        if fs::symlink_metadata(&destination).is_ok() {
            return Err("inline evidence closure destination already exists".to_string());
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("failed to create inline evidence closure parent: {error}")
            })?;
        }
        fs::write(&destination, &source.bytes)
            .map_err(|error| format!("failed to write inline evidence closure: {error}"))?;
    }
    let json = serde_json::to_string_pretty(report)
        .map_err(|error| format!("failed to serialize workflow disposition: {error}"))?;
    let markdown = render_markdown(report)?;
    fs::write(out.join(REPORT_JSON), format!("{json}\n"))
        .map_err(|error| format!("failed to write workflow disposition JSON: {error}"))?;
    fs::write(out.join(REPORT_MD), markdown)
        .map_err(|error| format!("failed to write workflow disposition Markdown: {error}"))?;
    let files = collect_packet_files(out)?;
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
        "# Source Promotion Admission Workflow\n\n- Status: `{}`\n- Operation mode: `{}`\n- Execution profile: `{}`\n- Source parent: `{}`\n- W7 parent: `{}`\n- Reviewed tree: `{}`\n- Reviewed tree carrier: `{}`\n\nThis disposition grants no ref, merge, release, or publication authority.\n",
        required_json_string(report, "status")?,
        required_json_string(report, "operation_mode")?,
        required_json_string(report, "execution_profile")?,
        required_json_string(report, "source_parent_sha")?,
        required_json_string(report, "w7_peeled_sha")?,
        required_json_string(report, "reviewed_tree_sha")?,
        required_json_string(report, "reviewed_tree_carrier_sha")?,
    ))
}

fn packet_state(root: &Path, report: &str, closure_path: &str) -> Value {
    let value = read_optional_json(&root.join(report));
    serde_json::json!({
        "path": closure_path,
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
    "usage: cargo xtask source-promotion (run-admission-workflow <exact inputs and requested identity> | verify-admission-workflow --packet <dir> --requested-identity <file> --requested-identity-sha256 <digest> | enforce-admission-workflow --packet <dir> --expected-status admitted)".to_string()
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

fn absolute_from(root: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn read_requested_identity(
    path: &Path,
    expected_sha256: &str,
) -> Result<RequestedIdentity, String> {
    validate_hex(expected_sha256, 64, "requested identity SHA-256")?;
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "failed to inspect requested identity {}: {error}",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("requested identity must be a non-symlink regular file".to_string());
    }
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "failed to read requested identity {}: {error}",
            path.display()
        )
    })?;
    if digest_bytes(&bytes) != expected_sha256 {
        return Err("requested identity digest differs from pre-producer authority".to_string());
    }
    let request: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("malformed requested identity JSON: {error}"))?;
    exact_object_fields(
        &request,
        "requested identity",
        &[
            "schema",
            "source_repository",
            "source_parent_sha",
            "workflow_source_sha",
            "trusted_checker_identity",
            "swarm_repository",
            "protected_w7_ref",
            "w7_peeled_sha",
            "reviewed_tree_sha",
            "reviewed_tree_carrier_sha",
            "preflight_locator",
            "resolution_manifest_locator",
            "validation_packet_locator",
            "integration_packet_locator",
            "qualification_receipt_locator",
            "receipt_schema",
            "operation_mode",
            "execution_profile",
        ],
    )?;
    if json_string(&request, "schema") != Some(REQUEST_SCHEMA) {
        return Err("requested identity schema is unsupported".to_string());
    }
    Ok(RequestedIdentity {
        value: request,
        bytes,
    })
}

fn validate_request_against_inputs(
    request: &Value,
    values: &BTreeMap<String, String>,
) -> Result<(), String> {
    for (field, option) in [
        ("source_repository", "--source-repository"),
        ("source_parent_sha", "--source-parent-sha"),
        ("workflow_source_sha", "--workflow-source-sha"),
        ("trusted_checker_identity", "--trusted-checker-identity"),
        ("swarm_repository", "--swarm-repository"),
        ("protected_w7_ref", "--protected-w7-ref"),
        ("w7_peeled_sha", "--w7-peeled-sha"),
        ("reviewed_tree_sha", "--reviewed-tree-sha"),
        ("reviewed_tree_carrier_sha", "--reviewed-tree-carrier-sha"),
        ("preflight_locator", "--preflight-locator"),
        (
            "resolution_manifest_locator",
            "--resolution-manifest-locator",
        ),
        ("validation_packet_locator", "--validation-packet-locator"),
        ("integration_packet_locator", "--integration-packet-locator"),
        (
            "qualification_receipt_locator",
            "--qualification-receipt-locator",
        ),
        ("receipt_schema", "--receipt-schema"),
        ("operation_mode", "--operation-mode"),
        ("execution_profile", "--execution-profile"),
    ] {
        if json_string(request, field) != Some(required(values, option)?) {
            return Err(format!(
                "requested identity {field} differs from producer input {option}"
            ));
        }
    }
    Ok(())
}

fn validate_request_against_report(request: &Value, report: &Value) -> Result<(), String> {
    for field in [
        "source_repository",
        "source_parent_sha",
        "workflow_source_sha",
        "trusted_checker_identity",
        "swarm_repository",
        "protected_w7_ref",
        "w7_peeled_sha",
        "reviewed_tree_sha",
        "reviewed_tree_carrier_sha",
        "receipt_schema",
        "operation_mode",
        "execution_profile",
    ] {
        if json_string(request, field) != json_string(report, field) {
            return Err(format!(
                "workflow disposition {field} differs from requested identity"
            ));
        }
    }
    let locators = report
        .get("locators")
        .ok_or_else(|| "workflow disposition is missing locators".to_string())?;
    for (request_field, report_field) in [
        ("preflight_locator", "preflight"),
        ("resolution_manifest_locator", "resolution_manifest"),
        ("validation_packet_locator", "validation_packet"),
        ("integration_packet_locator", "integration_packet"),
        ("qualification_receipt_locator", "qualification_receipt"),
    ] {
        let requested = required_json_string(request, request_field)?;
        let locator = locators
            .get(report_field)
            .ok_or_else(|| format!("workflow disposition is missing {report_field} locator"))?;
        let profile = required_json_string(report, "execution_profile")?;
        if profile == "live" {
            if requested.is_empty() {
                if json_string(locator, "status") != Some("not_required") {
                    return Err(format!(
                        "workflow disposition {report_field} locator differs from requested identity"
                    ));
                }
            } else if json_string(locator, "locator") != Some(requested) {
                return Err(format!(
                    "workflow disposition {report_field} locator differs from requested identity"
                ));
            }
        } else {
            let locator_status = json_string(locator, "status");
            let status_allowed = locator_status == Some("source_owned_synthetic")
                || (report_field == "qualification_receipt"
                    && locator_status == Some("not_required"));
            if !requested.is_empty() || !status_allowed {
                return Err(format!(
                    "synthetic workflow {report_field} locator differs from requested identity contract"
                ));
            }
        }
    }
    Ok(())
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

fn exact_object_fields(value: &Value, label: &str, expected: &[&str]) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{label} must be an object"))?;
    let observed = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if observed != expected {
        return Err(format!(
            "{label} fields differ from exact contract: {observed:?}"
        ));
    }
    Ok(())
}

fn exact_object_fields_with_optional(
    value: &Value,
    label: &str,
    required: &[&str],
    optional: &[&str],
) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{label} must be an object"))?;
    let observed = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let required = required.iter().copied().collect::<BTreeSet<_>>();
    let allowed = required
        .iter()
        .copied()
        .chain(optional.iter().copied())
        .collect::<BTreeSet<_>>();
    if !required.is_subset(&observed) || !observed.is_subset(&allowed) {
        return Err(format!(
            "{label} fields differ from exact contract: {observed:?}"
        ));
    }
    Ok(())
}

fn canonical_locator_identity(
    value: &Value,
    label: &str,
    profile: ExecutionProfile,
    allow_not_required: bool,
) -> Result<String, String> {
    if json_string(value, "status") == Some("not_required") {
        if !allow_not_required {
            return Err(format!("{label} locator may not be not_required"));
        }
        exact_object_fields(value, label, &["label", "schema", "status"])?;
        if json_string(value, "schema") != Some(LOCATOR_SCHEMA)
            || json_string(value, "label") != Some(label)
        {
            return Err(format!("{label} not-required locator identity moved"));
        }
        return serde_json::to_string(&serde_json::json!({
            "schema": LOCATOR_SCHEMA,
            "status": "not_required",
            "label": label,
        }))
        .map_err(|error| format!("failed to canonicalize {label} locator: {error}"));
    }

    match profile {
        ExecutionProfile::Live => {
            exact_object_fields(
                value,
                label,
                &[
                    "locator",
                    "mode",
                    "path",
                    "repository",
                    "revision",
                    "schema",
                    "sha256",
                ],
            )?;
            if json_string(value, "schema") != Some(LOCATOR_SCHEMA)
                || json_string(value, "mode") != Some("100644")
            {
                return Err(format!("{label} live locator schema or mode moved"));
            }
            let repository = required_json_string(value, "repository")?;
            if repository != SOURCE_REPOSITORY && repository != SWARM_REPOSITORY {
                return Err(format!("{label} live locator repository is unsupported"));
            }
            let revision = required_json_string(value, "revision")?;
            validate_hex(revision, 40, &format!("{label} locator revision"))?;
            let path = required_json_string(value, "path")?;
            safe_relative(path)?;
            let sha256 = required_json_string(value, "sha256")?;
            validate_hex(sha256, 64, &format!("{label} locator sha256"))?;
            let expected_locator = format!("{repository}@{revision}:{path}#sha256:{sha256}");
            if json_string(value, "locator") != Some(expected_locator.as_str()) {
                return Err(format!("{label} canonical locator reconstruction moved"));
            }
            serde_json::to_string(&serde_json::json!({
                "schema": LOCATOR_SCHEMA,
                "repository": repository,
                "revision": revision,
                "path": path,
                "mode": "100644",
                "sha256": sha256,
                "locator": expected_locator,
            }))
            .map_err(|error| format!("failed to canonicalize {label} locator: {error}"))
        }
        ExecutionProfile::PositiveSynthetic | ExecutionProfile::J5Negative => {
            exact_object_fields(
                value,
                label,
                &[
                    "label",
                    "local_path",
                    "mode",
                    "path",
                    "schema",
                    "sha256",
                    "status",
                ],
            )?;
            if json_string(value, "schema") != Some(LOCATOR_SCHEMA)
                || json_string(value, "status") != Some("source_owned_synthetic")
                || json_string(value, "label") != Some(label)
                || json_string(value, "mode") != Some("100644")
            {
                return Err(format!("{label} synthetic locator identity moved"));
            }
            let path = required_json_string(value, "path")?;
            let local_path = required_json_string(value, "local_path")?;
            safe_relative(path)?;
            safe_relative(local_path)?;
            if path != local_path || !path.starts_with("synthetic-fixture/") {
                return Err(format!(
                    "{label} synthetic local_path escapes its admitted workspace"
                ));
            }
            let sha256 = required_json_string(value, "sha256")?;
            validate_hex(sha256, 64, &format!("{label} locator sha256"))?;
            serde_json::to_string(&serde_json::json!({
                "schema": LOCATOR_SCHEMA,
                "status": "source_owned_synthetic",
                "label": label,
                "mode": "100644",
                "sha256": sha256,
                "path": path,
                "local_path": local_path,
            }))
            .map_err(|error| format!("failed to canonicalize {label} locator: {error}"))
        }
    }
}

fn validate_locator_provenance(
    report: &Value,
    profile: ExecutionProfile,
    mode: OperationMode,
    status: &str,
) -> Result<(), String> {
    let locators = report
        .get("locators")
        .and_then(Value::as_object)
        .ok_or_else(|| "workflow disposition is missing locators".to_string())?;
    let expected = [
        "integration_packet",
        "preflight",
        "qualification_receipt",
        "resolution_manifest",
        "validation_packet",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let observed = locators.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if observed != expected {
        return Err(format!(
            "workflow disposition locator inventory moved: {observed:?}"
        ));
    }
    for (key, label) in [
        ("preflight", "preflight"),
        ("resolution_manifest", "resolution manifest"),
        ("validation_packet", "validation packet"),
        ("integration_packet", "integration packet"),
    ] {
        canonical_locator_identity(&locators[key], label, profile, false)?;
    }
    canonical_locator_identity(
        &locators["qualification_receipt"],
        "qualification receipt",
        profile,
        mode == OperationMode::AdmitOnly
            || (json_string(report, "phase") == Some("admission") && status == "rejected"),
    )?;
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
    let profile = ExecutionProfile::parse(required_json_string(report, "execution_profile")?)?;
    let mode = OperationMode::parse(required_json_string(report, "operation_mode")?)?;
    let status = required_json_string(report, "status")?;
    let locator_identity = |key: &str, label: &str, allow_not_required: bool| {
        canonical_locator_identity(
            locators
                .get(key)
                .ok_or_else(|| format!("workflow disposition is missing {key} locator"))?,
            label,
            profile,
            allow_not_required,
        )
    };
    let fields = vec![
        required_json_string(report, "execution_profile")?.to_string(),
        required_json_string(report, "operation_mode")?.to_string(),
        required_json_string(report, "source_repository")?.to_string(),
        required_json_string(report, "source_parent_sha")?.to_string(),
        required_json_string(report, "workflow_source_sha")?.to_string(),
        required_json_string(report, "trusted_checker_identity")?.to_string(),
        required_json_string(report, "swarm_repository")?.to_string(),
        required_json_string(report, "protected_w7_ref")?.to_string(),
        required_json_string(report, "w7_peeled_sha")?.to_string(),
        required_json_string(report, "reviewed_tree_sha")?.to_string(),
        required_json_string(report, "reviewed_tree_carrier_sha")?.to_string(),
        required_json_string(report, "receipt_schema")?.to_string(),
        required_json_string(report, "fixture_identity")?.to_string(),
        required_json_string(report, "controller_repository")?.to_string(),
        required_json_string(report, "requested_identity_sha256")?.to_string(),
        locator_identity("preflight", "preflight", false)?,
        locator_identity("resolution_manifest", "resolution manifest", false)?,
        locator_identity("validation_packet", "validation packet", false)?,
        locator_identity("integration_packet", "integration packet", false)?,
        locator_identity(
            "qualification_receipt",
            "qualification receipt",
            mode == OperationMode::AdmitOnly
                || (json_string(report, "phase") == Some("admission") && status == "rejected"),
        )?,
    ];
    let canonical = serde_json::to_vec(&fields)
        .map_err(|error| format!("failed to canonicalize workflow identity: {error}"))?;
    Ok(digest_bytes(&canonical))
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
    fn j5_negative_rejects_final_phase_and_passed_admission_mutations() -> Result<(), String> {
        let baseline = test_report("rejected", "constructor_dry_run", "j5_negative", 0)?;
        validate_report(&baseline)?;

        let mut final_phase = baseline.clone();
        final_phase["phase"] = Value::String("final".to_string());
        final_phase["producer"]["constructor_state"] = Value::String("rejected".to_string());
        if validate_report(&final_phase).is_ok() {
            return Err("final-phase J5 disposition escaped reachability".to_string());
        }

        let mut passed_admission = baseline;
        passed_admission["controller_packets"]["resolved_tree_admission"]["status"] =
            Value::String("admitted".to_string());
        passed_admission["producer"]["admission_state"] = Value::String("passed".to_string());
        passed_admission["workflow_identity_sha256"] =
            Value::String(workflow_identity(&passed_admission)?);
        if validate_report(&passed_admission).is_ok() {
            return Err("passed-admission J5 disposition escaped reachability".to_string());
        }
        Ok(())
    }

    #[test]
    fn rejected_admission_replays_built_builder_and_rejection_semantics() -> Result<(), String> {
        let (builder_root, builder_packet) =
            write_admission_rejected_test_closure("rejected-admission-builder-replay")?;
        verify_packet(&builder_packet)?;
        let builder = builder_packet.join("evidence/trusted-builder");
        let builder_receipt = builder.join("trusted-builder.json");
        let mut mutated_builder = read_json(&builder_receipt, "rejected builder replay fixture")?;
        mutated_builder["commit_tree_attempts"] = Value::from(1);
        write_pretty_json(
            &builder_receipt,
            &mutated_builder,
            "mutated rejected builder",
        )?;
        reindex_control_packet(&builder)?;
        reindex_workflow_packet(&builder_packet)?;
        if verify_packet(&builder_packet).is_ok() {
            return Err("rejected-admission builder mutation escaped semantic replay".to_string());
        }
        fs::remove_dir_all(&builder_root)
            .map_err(|error| format!("failed to clean rejected builder fixture: {error}"))?;

        let (admission_root, admission_packet) =
            write_admission_rejected_test_closure("rejected-admission-replay")?;
        verify_packet(&admission_packet)?;
        let admission = admission_packet.join("evidence/resolved-tree-admission");
        let admission_receipt = admission.join("resolved-tree-admission.json");
        let mut mutated_admission =
            read_json(&admission_receipt, "rejected admission replay fixture")?;
        mutated_admission["source_parent"] = Value::String("f".repeat(40));
        write_pretty_json(
            &admission_receipt,
            &mutated_admission,
            "mutated rejected admission",
        )?;
        reindex_control_packet(&admission)?;
        reindex_workflow_packet(&admission_packet)?;
        if verify_packet(&admission_packet).is_ok() {
            return Err("rejected admission identity mutation escaped semantic replay".to_string());
        }
        fs::remove_dir_all(&admission_root)
            .map_err(|error| format!("failed to clean rejected admission fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn rejected_admission_requires_a_failed_builder_or_admission_stage() -> Result<(), String> {
        let mut report = test_report("admitted", "admit_only", "positive_synthetic", 0)?;
        report["phase"] = Value::String("admission".to_string());
        report["status"] = Value::String("rejected".to_string());
        report["producer"]["normalized_exit_code"] = Value::from(1);
        report["producer"]["constructor_state"] =
            Value::String("not_run_before_upload_and_enforcement".to_string());
        report["failure_reasons"] = serde_json::json!(["fabricated rejection"]);
        report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
        if validate_report(&report).is_ok() {
            return Err("rejected admission with two passed stages escaped parity".to_string());
        }
        Ok(())
    }

    #[test]
    fn rejected_admission_accepts_unavailable_admission_receipt_after_builder_failure()
    -> Result<(), String> {
        let mut report = test_report("admitted", "admit_only", "positive_synthetic", 0)?;
        report["phase"] = Value::String("admission".to_string());
        report["status"] = Value::String("rejected".to_string());
        for key in ["trusted_builder", "resolved_tree_admission"] {
            report["controller_packets"][key] = serde_json::json!({
                "path": format!("evidence/{}", key.replace('_', "-")),
                "available": false,
                "status": null,
                "schema": null,
            });
        }
        report["producer"] = serde_json::json!({
            "normalized_exit_code": 1,
            "trusted_builder_state": "rejected",
            "admission_state": "rejected",
            "constructor_state": "not_run_before_upload_and_enforcement",
        });
        report["attempts"]["admission_receipt_available"] = Value::Bool(false);
        report["failure_reasons"] = serde_json::json!(["builder rejected before admission"]);
        report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
        validate_report(&report)
    }

    #[test]
    fn failed_builder_rejects_every_available_admission_state() -> Result<(), String> {
        for (builder_label, builder_summary) in [
            (
                "unavailable",
                serde_json::json!({
                    "path": "evidence/trusted-builder",
                    "available": false,
                    "status": null,
                    "schema": null,
                }),
            ),
            (
                "rejected",
                serde_json::json!({
                    "path": "evidence/trusted-builder",
                    "available": true,
                    "status": "rejected",
                    "schema": "ripr.source_promotion_trusted_builder.v1",
                }),
            ),
        ] {
            for (admission_label, admission_status) in
                [("admitted", "admitted"), ("rejected", "rejected")]
            {
                let mut report = test_report("admitted", "admit_only", "positive_synthetic", 0)?;
                report["phase"] = Value::String("admission".to_string());
                report["status"] = Value::String("rejected".to_string());
                report["controller_packets"]["trusted_builder"] = builder_summary.clone();
                report["controller_packets"]["resolved_tree_admission"] = serde_json::json!({
                    "path": "evidence/resolved-tree-admission",
                    "available": true,
                    "status": admission_status,
                    "schema": "ripr.source_promotion_resolved_tree_admission.v1",
                });
                report["producer"] = serde_json::json!({
                    "normalized_exit_code": 1,
                    "trusted_builder_state": "rejected",
                    "admission_state": if admission_status == "admitted" { "passed" } else { "rejected" },
                    "constructor_state": "not_run_before_upload_and_enforcement",
                });
                report["attempts"] = normalized_attempts(&None, &None, false);
                report["failure_reasons"] = serde_json::json!([format!(
                    "{builder_label} builder with {admission_label} admission"
                )]);
                report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
                if validate_report(&report).is_ok() {
                    return Err(format!(
                        "{builder_label} builder accepted available {admission_label} admission"
                    ));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn builder_rejection_replays_optional_provenance_and_zero_authority() -> Result<(), String> {
        let (root, packet) = write_builder_rejected_test_closure("builder-rejection-replay")?;
        verify_packet(&packet)?;
        let builder = packet.join("evidence/trusted-builder");
        let receipt_path = builder.join("trusted-builder.json");
        let mut receipt = read_json(&receipt_path, "rejected builder fixture")?;
        receipt["source_parent"] = Value::String("f".repeat(40));
        receipt["ref_mutation_attempted"] = Value::Bool(true);
        write_pretty_json(&receipt_path, &receipt, "mutated rejected builder")?;
        reindex_control_packet(&builder)?;
        reindex_workflow_packet(&packet)?;
        if verify_packet(&packet).is_ok() {
            return Err(
                "rejected builder provenance/authority mutation escaped replay".to_string(),
            );
        }
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean builder rejection fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn malformed_controller_receipts_keep_attempts_exactly_unavailable() -> Result<(), String> {
        for (label, builder) in [
            ("malformed-builder-attempts", true),
            ("malformed-admission-attempts", false),
        ] {
            let (root, packet) = if builder {
                write_builder_rejected_test_closure(label)?
            } else {
                write_admission_rejected_test_closure(label)?
            };
            let (directory, report_name, summary_key) = if builder {
                ("trusted-builder", "trusted-builder.json", "trusted_builder")
            } else {
                (
                    "resolved-tree-admission",
                    "resolved-tree-admission.json",
                    "resolved_tree_admission",
                )
            };
            let control = packet.join("evidence").join(directory);
            fs::write(control.join(report_name), b"{malformed")
                .map_err(|error| format!("failed to corrupt {label} receipt: {error}"))?;
            reindex_packet_inventory(&control)?;
            let mut report = read_json(&packet.join(REPORT_JSON), label)?;
            report["controller_packets"][summary_key] =
                packet_state(&control, report_name, &format!("evidence/{directory}"));
            report["attempts"] = normalized_attempts(&None, &None, false);
            report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
            rewrite_test_packet_reports_and_index(&packet, &report)?;
            verify_packet(&packet)?;

            report["attempts"]["construction_receipt_available"] = Value::Bool(true);
            report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
            rewrite_test_packet_reports_and_index(&packet, &report)?;
            if verify_packet(&packet).is_ok() {
                return Err(format!(
                    "digest-rebound attempts escaped malformed {summary_key} evidence"
                ));
            }
            fs::remove_dir_all(&root)
                .map_err(|error| format!("failed to clean {label} fixture: {error}"))?;
        }
        Ok(())
    }

    #[test]
    fn final_dispositions_require_an_admitted_prefix_and_reachable_terminal_status()
    -> Result<(), String> {
        let mut failed_prefix = test_report("admitted", "admit_only", "positive_synthetic", 0)?;
        failed_prefix["controller_packets"]["resolved_tree_admission"] = serde_json::json!({
            "path": "evidence/resolved-tree-admission",
            "available": false,
            "status": null,
            "schema": null,
        });
        failed_prefix["producer"]["admission_state"] = Value::String("rejected".to_string());
        failed_prefix["workflow_identity_sha256"] =
            Value::String(workflow_identity(&failed_prefix)?);
        if validate_report(&failed_prefix).is_ok() {
            return Err("final disposition with a failed admission prefix escaped".to_string());
        }

        let mut admit_only = test_report("admitted", "admit_only", "positive_synthetic", 0)?;
        admit_only["status"] = Value::String("rejected".to_string());
        admit_only["producer"]["normalized_exit_code"] = Value::from(1);
        admit_only["failure_reasons"] = serde_json::json!(["fabricated rejection"]);
        admit_only["workflow_identity_sha256"] = Value::String(workflow_identity(&admit_only)?);
        if validate_report(&admit_only).is_ok() {
            return Err("rejected final admit_only disposition escaped stage parity".to_string());
        }

        let mut constructor =
            test_report("admitted", "constructor_dry_run", "positive_synthetic", 1)?;
        constructor["status"] = Value::String("rejected".to_string());
        constructor["producer"]["normalized_exit_code"] = Value::from(1);
        constructor["producer"]["constructor_state"] = Value::String("rejected".to_string());
        constructor["failure_reasons"] = serde_json::json!(["fabricated rejection"]);
        constructor["workflow_identity_sha256"] = Value::String(workflow_identity(&constructor)?);
        if validate_report(&constructor).is_ok() {
            return Err("rejected final constructor with constructed evidence escaped".to_string());
        }
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
    fn reviewed_tree_carrier_identity_is_live_only() -> Result<(), String> {
        validate_reviewed_tree_carrier_identity(ExecutionProfile::Live, &"a".repeat(40))?;
        validate_reviewed_tree_carrier_identity(
            ExecutionProfile::PositiveSynthetic,
            "not_required",
        )?;
        validate_reviewed_tree_carrier_identity(ExecutionProfile::J5Negative, "not_required")?;
        if validate_reviewed_tree_carrier_identity(ExecutionProfile::Live, "not_required").is_ok()
            || validate_reviewed_tree_carrier_identity(
                ExecutionProfile::PositiveSynthetic,
                &"a".repeat(40),
            )
            .is_ok()
        {
            return Err("reviewed tree carrier profile boundary was bypassed".to_string());
        }
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
    fn verifier_rejects_self_consistent_report_that_moves_requested_identity() -> Result<(), String>
    {
        let (root, packet) = write_test_closure("requested-identity-moved")?;
        let mut report = read_json(&packet.join(REPORT_JSON), "test workflow disposition")?;
        report["protected_w7_ref"] =
            Value::String("refs/tags/ripr-release-fixture-moved".to_string());
        report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
        rewrite_test_packet_reports_and_index(&packet, &report)?;
        if verify_packet(&packet).is_ok() {
            return Err(
                "self-consistent report escaped the pre-producer requested identity".to_string(),
            );
        }
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean requested identity fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn verifier_rejects_digest_rebound_controller_repository() -> Result<(), String> {
        let (root, packet) = write_test_closure("controller-repository-moved")?;
        let mut report = read_json(&packet.join(REPORT_JSON), "test workflow disposition")?;
        report["controller_repository"] = Value::String("source-checkout".to_string());
        report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
        rewrite_test_packet_reports_and_index(&packet, &report)?;
        if verify_packet(&packet).is_ok() {
            return Err(
                "digest-rebound controller repository escaped profile authority".to_string(),
            );
        }
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean controller repository fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn verifier_rejects_digest_rebound_unknown_public_fields() -> Result<(), String> {
        for (label, pointer) in [
            ("unknown-workflow-field", ""),
            ("unknown-attempt-field", "/attempts"),
        ] {
            let (root, packet) = write_test_closure(label)?;
            verify_packet(&packet)?;
            let mut report = read_json(&packet.join(REPORT_JSON), label)?;
            report
                .pointer_mut(pointer)
                .and_then(Value::as_object_mut)
                .ok_or_else(|| format!("{label} target is not an object"))?
                .insert("merge_authorized".to_string(), Value::Bool(false));
            report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
            rewrite_test_packet_reports_and_index(&packet, &report)?;
            if verify_packet(&packet).is_ok() {
                return Err(format!("digest-rebound {label} escaped exact schema"));
            }
            fs::remove_dir_all(&root)
                .map_err(|error| format!("failed to clean {label}: {error}"))?;
        }

        for (label, directory, report_name) in [
            (
                "unknown-admitted-builder-field",
                "trusted-builder",
                "trusted-builder.json",
            ),
            (
                "unknown-admitted-admission-field",
                "resolved-tree-admission",
                "resolved-tree-admission.json",
            ),
        ] {
            let (root, packet) = write_test_closure(label)?;
            verify_packet(&packet)?;
            let control = packet.join("evidence").join(directory);
            let receipt_path = control.join(report_name);
            let mut receipt = read_json(&receipt_path, label)?;
            if label == "unknown-admitted-admission-field" {
                receipt["integration_receipts"]["merge_authorized"] = Value::Bool(false);
            } else {
                receipt["merge_authorized"] = Value::Bool(false);
            }
            write_pretty_json(&receipt_path, &receipt, label)?;
            reindex_control_packet(&control)?;
            reindex_workflow_packet(&packet)?;
            if verify_packet(&packet).is_ok() {
                return Err(format!("digest-rebound {label} escaped exact schema"));
            }
            fs::remove_dir_all(&root)
                .map_err(|error| format!("failed to clean {label}: {error}"))?;
        }

        for (label, constructor) in [
            ("unknown-rejected-builder-field", "builder"),
            ("unknown-rejected-admission-field", "admission"),
            ("unknown-rejected-construction-field", "construction"),
        ] {
            let (root, packet) = match constructor {
                "builder" => write_builder_rejected_test_closure(label)?,
                "admission" => write_admission_rejected_test_closure(label)?,
                _ => write_parseable_rejected_constructor_test_closure(label, false)?,
            };
            verify_packet(&packet)?;
            let (directory, report_name) = match constructor {
                "builder" => ("trusted-builder", "trusted-builder.json"),
                "admission" => ("resolved-tree-admission", "resolved-tree-admission.json"),
                _ => ("exact-join-construction", "exact-join-construction.json"),
            };
            let control = packet.join("evidence").join(directory);
            let receipt_path = control.join(report_name);
            let mut receipt = read_json(&receipt_path, label)?;
            if constructor == "admission" {
                receipt["identity"]["merge_authorized"] = Value::Bool(false);
            } else {
                receipt["merge_authorized"] = Value::Bool(false);
            }
            write_pretty_json(&receipt_path, &receipt, label)?;
            reindex_control_packet(&control)?;
            reindex_workflow_packet(&packet)?;
            if verify_packet(&packet).is_ok() {
                return Err(format!("digest-rebound {label} escaped exact schema"));
            }
            fs::remove_dir_all(&root)
                .map_err(|error| format!("failed to clean {label}: {error}"))?;
        }
        Ok(())
    }

    #[test]
    fn verifier_rejects_digest_rebound_construction_summary_outside_final_constructor()
    -> Result<(), String> {
        let (root, packet) = write_test_closure("construction-summary-rebound")?;
        let baseline = verify_packet(&packet)?;
        let mut moved = baseline.clone();
        moved["controller_packets"]["exact_join_construction"]["path"] =
            Value::String("evidence/exact-join-construction".to_string());
        rewrite_test_packet_reports_and_index(&packet, &moved)?;
        if verify_packet(&packet).is_ok() {
            return Err(
                "digest-rebound construction summary escaped phase and mode authority".to_string(),
            );
        }
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean construction summary fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn verifier_rejects_digest_rebound_producer_state_families() -> Result<(), String> {
        let (root, packet) = write_test_closure("producer-state-rebound")?;
        let baseline = verify_packet(&packet)?;
        for (field, replacement) in [
            ("normalized_exit_code", Value::from(1)),
            (
                "trusted_builder_state",
                Value::String("rejected".to_string()),
            ),
            ("admission_state", Value::String("rejected".to_string())),
            ("constructor_state", Value::String("passed".to_string())),
        ] {
            let mut moved = baseline.clone();
            moved["producer"][field] = replacement;
            rewrite_test_packet_reports_and_index(&packet, &moved)?;
            if verify_packet(&packet).is_ok() {
                return Err(format!(
                    "digest-rebound producer state escaped semantic parity at {field}"
                ));
            }
            rewrite_test_packet_reports_and_index(&packet, &baseline)?;
            verify_packet(&packet)?;
        }
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean producer state fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn verifier_rejects_digest_rebound_failure_reasons_on_admitted_packet() -> Result<(), String> {
        let (root, packet) = write_test_closure("failure-reasons-rebound")?;
        let baseline = verify_packet(&packet)?;
        let mut moved = baseline;
        moved["failure_reasons"] = serde_json::json!(["fabricated admitted failure"]);
        rewrite_test_packet_reports_and_index(&packet, &moved)?;
        if verify_packet(&packet).is_ok() {
            return Err("digest-rebound admitted failure reasons escaped verifier".to_string());
        }
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean failure reason fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn verifier_rejects_digest_rebound_admitted_status_to_rejected() -> Result<(), String> {
        let (root, packet) = write_test_closure("status-stage-rebound")?;
        let baseline = verify_packet(&packet)?;
        let mut moved = baseline;
        moved["status"] = Value::String("rejected".to_string());
        moved["producer"]["normalized_exit_code"] = Value::from(1);
        moved["failure_reasons"] = serde_json::json!(["fabricated rejection"]);
        moved["workflow_identity_sha256"] = Value::String(workflow_identity(&moved)?);
        rewrite_test_packet_reports_and_index(&packet, &moved)?;
        if verify_packet(&packet).is_ok() {
            return Err(
                "digest-rebound rejected status escaped unchanged controller evidence".to_string(),
            );
        }
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean status parity fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn verifier_rejects_digest_rebound_markdown_and_packet_status() -> Result<(), String> {
        let (markdown_root, markdown_packet) = write_test_closure("markdown-moved")?;
        fs::write(
            markdown_packet.join(REPORT_MD),
            b"# Source Promotion Admission Workflow\n\n- Status: `rejected`\n",
        )
        .map_err(|error| format!("failed to mutate workflow Markdown: {error}"))?;
        reindex_workflow_packet(&markdown_packet)?;
        if verify_packet(&markdown_packet).is_ok() {
            return Err("digest-rebound workflow Markdown escaped JSON parity".to_string());
        }
        fs::remove_dir_all(&markdown_root)
            .map_err(|error| format!("failed to clean Markdown parity fixture: {error}"))?;

        let (status_root, status_packet) = write_test_closure("packet-status-moved")?;
        let index_path = status_packet.join(PACKET_INDEX);
        let mut index = read_json(&index_path, "workflow packet status fixture")?;
        index["status"] = Value::String("rejected".to_string());
        write_pretty_json(&index_path, &index, "moved workflow packet status")?;
        if verify_packet(&status_packet).is_ok() {
            return Err("workflow packet index status escaped disposition parity".to_string());
        }
        fs::remove_dir_all(&status_root)
            .map_err(|error| format!("failed to clean packet status fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn verifier_replays_nested_builder_semantics_after_digest_rebinding() -> Result<(), String> {
        let (root, packet) = write_test_closure("nested-builder-replay")?;
        let builder_root = packet.join("evidence/trusted-builder");
        let builder_report = builder_root.join("trusted-builder.json");
        let mut builder = read_json(&builder_report, "nested builder fixture")?;
        builder["commit_tree_attempts"] = Value::from(1);
        write_pretty_json(&builder_report, &builder, "mutated nested builder fixture")?;
        let builder_index_sha256 = reindex_control_packet(&builder_root)?;
        let builder_receipt_sha256 = digest_file(&builder_report, "nested builder receipt")?;

        let admission_root = packet.join("evidence/resolved-tree-admission");
        let admission_report = admission_root.join("resolved-tree-admission.json");
        let mut admission = read_json(&admission_report, "nested admission fixture")?;
        admission["trusted_builder_packet_index_sha256"] = Value::String(builder_index_sha256);
        admission["trusted_builder_receipt_sha256"] = Value::String(builder_receipt_sha256);
        write_pretty_json(
            &admission_report,
            &admission,
            "rebound nested admission fixture",
        )?;
        reindex_control_packet(&admission_root)?;
        reindex_workflow_packet(&packet)?;
        if verify_packet(&packet).is_ok() {
            return Err(
                "digest-rebound forbidden builder attempt escaped semantic replay".to_string(),
            );
        }
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean nested replay fixture: {error}"))?;
        Ok(())
    }

    fn verify_digest_rebound_validation_contract_mutation_rejected(
        label: &str,
        mutate: impl FnOnce(&mut Value) -> Result<(), String>,
    ) -> Result<(), String> {
        let (root, packet) = write_test_closure(label)?;
        let validation_root = packet.join("evidence/locators/validation_packet");
        let validation_report = validation_root.join("resolved-tree-validation.json");
        let mut validation = read_json(&validation_report, "nested validation fixture")?;
        mutate(&mut validation)?;
        write_pretty_json(
            &validation_report,
            &validation,
            "mutated nested validation fixture",
        )?;
        let validation_index_sha256 = reindex_control_packet(&validation_root)?;
        let validation_receipt_sha256 =
            digest_file(&validation_report, "rebound validation receipt")?;

        let admission_root = packet.join("evidence/resolved-tree-admission");
        let admission_report = admission_root.join("resolved-tree-admission.json");
        let mut admission = read_json(&admission_report, "nested admission fixture")?;
        admission["resolved_tree_packet_index_sha256"] =
            Value::String(validation_index_sha256.clone());
        admission["resolved_tree_validation_receipt_sha256"] =
            Value::String(validation_receipt_sha256);
        write_pretty_json(
            &admission_report,
            &admission,
            "rebound nested admission fixture",
        )?;
        reindex_control_packet(&admission_root)?;

        let mut workflow = read_json(&packet.join(REPORT_JSON), "rebound workflow fixture")?;
        workflow["locators"]["validation_packet"]["sha256"] =
            Value::String(validation_index_sha256);
        workflow["workflow_identity_sha256"] = Value::String(workflow_identity(&workflow)?);
        rewrite_test_packet_reports_and_index(&packet, &workflow)?;

        let escaped = verify_packet(&packet).is_ok();
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean validation contract fixture: {error}"))?;
        if escaped {
            return Err(format!(
                "digest-rebound {label} escaped resolved-tree receipt contract replay"
            ));
        }
        Ok(())
    }

    #[test]
    fn verifier_rejects_digest_rebound_validation_catalog_and_envelope_tampering()
    -> Result<(), String> {
        verify_digest_rebound_validation_contract_mutation_rejected(
            "removed-validation-catalog-row",
            |report| {
                let catalog = report["required_command_catalog"]
                    .as_array_mut()
                    .ok_or_else(|| "validation command catalog is not an array".to_string())?;
                if catalog.is_empty() {
                    return Err("validation command catalog is unexpectedly empty".to_string());
                }
                let _ = catalog.remove(0);
                Ok(())
            },
        )?;
        verify_digest_rebound_validation_contract_mutation_rejected(
            "changed-validation-catalog-row",
            |report| {
                let first = report["required_command_catalog"]
                    .as_array_mut()
                    .and_then(|catalog| catalog.first_mut())
                    .ok_or_else(|| "validation command catalog has no first row".to_string())?;
                *first = Value::String("check-forged-policy".to_string());
                Ok(())
            },
        )?;
        verify_digest_rebound_validation_contract_mutation_rejected(
            "unknown-validation-top-level-field",
            |report| {
                report
                    .as_object_mut()
                    .ok_or_else(|| "validation report is not an object".to_string())?
                    .insert("merge_authorized".to_string(), Value::Bool(true));
                Ok(())
            },
        )?;
        verify_digest_rebound_validation_contract_mutation_rejected(
            "unknown-validation-nested-field",
            |report| {
                report["trusted_checker"]
                    .as_object_mut()
                    .ok_or_else(|| "trusted checker is not an object".to_string())?
                    .insert("merge_authorized".to_string(), Value::Bool(true));
                Ok(())
            },
        )?;
        verify_digest_rebound_validation_contract_mutation_rejected(
            "changed-validation-checker-source",
            |report| {
                report["trusted_checker"]["source_sha"] = Value::String("2".repeat(40));
                Ok(())
            },
        )?;
        verify_digest_rebound_validation_contract_mutation_rejected(
            "changed-validation-authority-attempt",
            |report| {
                report["authoritative_commit_attempted"] = Value::Bool(true);
                Ok(())
            },
        )?;
        verify_digest_rebound_validation_contract_mutation_rejected(
            "changed-validation-non-claim",
            |report| {
                let claim = report["non_claims"]
                    .as_array_mut()
                    .and_then(|claims| claims.first_mut())
                    .ok_or_else(|| "validation report has no first non-claim".to_string())?;
                *claim = Value::String("This receipt grants release authority.".to_string());
                Ok(())
            },
        )
    }

    #[test]
    fn verifier_binds_validation_receipt_to_indexed_command_logs() -> Result<(), String> {
        let (root, packet) = write_test_closure("nested-command-log-replay")?;
        let validation_root = packet.join("evidence/locators/validation_packet");
        fs::write(
            validation_root.join("commands/01-check-network-policy.stdout.log"),
            b"forged command output\n",
        )
        .map_err(|error| format!("failed to mutate indexed command log: {error}"))?;
        let validation_index_sha256 = reindex_control_packet(&validation_root)?;
        let validation_receipt_sha256 = digest_file(
            &validation_root.join("resolved-tree-validation.json"),
            "rebound validation receipt",
        )?;
        let admission_root = packet.join("evidence/resolved-tree-admission");
        let admission_report = admission_root.join("resolved-tree-admission.json");
        let mut admission = read_json(&admission_report, "nested admission fixture")?;
        admission["resolved_tree_packet_index_sha256"] = Value::String(validation_index_sha256);
        admission["resolved_tree_validation_receipt_sha256"] =
            Value::String(validation_receipt_sha256);
        write_pretty_json(
            &admission_report,
            &admission,
            "rebound command-log admission fixture",
        )?;
        reindex_control_packet(&admission_root)?;
        reindex_workflow_packet(&packet)?;
        if verify_packet(&packet).is_ok() {
            return Err("digest-rebound command log escaped validation receipt replay".to_string());
        }
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean command-log replay fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn verifier_rejects_digest_rebound_undeclared_nested_members() -> Result<(), String> {
        let (validation_root, validation_packet) = write_test_closure("extra-validation-member")?;
        let nested_validation = validation_packet.join("evidence/locators/validation_packet");
        fs::write(
            nested_validation.join("commands/99-undeclared.stdout.log"),
            b"undeclared command evidence\n",
        )
        .map_err(|error| format!("failed to add undeclared validation member: {error}"))?;
        let validation_index_sha256 = reindex_control_packet(&nested_validation)?;
        let validation_receipt_sha256 = digest_file(
            &nested_validation.join("resolved-tree-validation.json"),
            "validation receipt with extra member",
        )?;
        let admission_root = validation_packet.join("evidence/resolved-tree-admission");
        let admission_report = admission_root.join("resolved-tree-admission.json");
        let mut admission = read_json(&admission_report, "extra-member admission fixture")?;
        admission["resolved_tree_packet_index_sha256"] = Value::String(validation_index_sha256);
        admission["resolved_tree_validation_receipt_sha256"] =
            Value::String(validation_receipt_sha256);
        write_pretty_json(
            &admission_report,
            &admission,
            "rebound extra-member admission fixture",
        )?;
        reindex_control_packet(&admission_root)?;
        reindex_workflow_packet(&validation_packet)?;
        if verify_packet(&validation_packet).is_ok() {
            return Err(
                "digest-rebound undeclared validation member escaped exact inventory".to_string(),
            );
        }
        fs::remove_dir_all(&validation_root).map_err(|error| {
            format!("failed to clean undeclared validation member fixture: {error}")
        })?;

        let (controller_root, controller_packet) = write_test_closure("extra-controller-member")?;
        let builder_root = controller_packet.join("evidence/trusted-builder");
        fs::write(
            builder_root.join("undeclared.log"),
            b"undeclared controller evidence\n",
        )
        .map_err(|error| format!("failed to add undeclared controller member: {error}"))?;
        let builder_index_sha256 = reindex_control_packet(&builder_root)?;
        let builder_receipt_sha256 = digest_file(
            &builder_root.join("trusted-builder.json"),
            "builder receipt with extra member",
        )?;
        let admission_root = controller_packet.join("evidence/resolved-tree-admission");
        let admission_report = admission_root.join("resolved-tree-admission.json");
        let mut admission = read_json(&admission_report, "extra-controller admission fixture")?;
        admission["trusted_builder_packet_index_sha256"] = Value::String(builder_index_sha256);
        admission["trusted_builder_receipt_sha256"] = Value::String(builder_receipt_sha256);
        write_pretty_json(
            &admission_report,
            &admission,
            "rebound extra-controller admission fixture",
        )?;
        reindex_control_packet(&admission_root)?;
        reindex_workflow_packet(&controller_packet)?;
        if verify_packet(&controller_packet).is_ok() {
            return Err(
                "digest-rebound undeclared controller member escaped exact inventory".to_string(),
            );
        }
        fs::remove_dir_all(&controller_root).map_err(|error| {
            format!("failed to clean undeclared controller member fixture: {error}")
        })?;

        let (admission_root, admission_packet) = write_test_closure("extra-admission-member")?;
        let nested_admission = admission_packet.join("evidence/resolved-tree-admission");
        fs::write(
            nested_admission.join("undeclared.log"),
            b"undeclared admission evidence\n",
        )
        .map_err(|error| format!("failed to add undeclared admission member: {error}"))?;
        reindex_control_packet(&nested_admission)?;
        reindex_workflow_packet(&admission_packet)?;
        if verify_packet(&admission_packet).is_ok() {
            return Err(
                "digest-rebound undeclared admission member escaped exact inventory".to_string(),
            );
        }
        fs::remove_dir_all(&admission_root).map_err(|error| {
            format!("failed to clean undeclared admission member fixture: {error}")
        })?;

        let (construction_root, construction_packet) =
            write_admitted_test_closure_for("extra-construction-member", "constructor_dry_run")?;
        let nested_admission = construction_packet.join("evidence/resolved-tree-admission");
        let admission_receipt = read_json(
            &nested_admission.join("resolved-tree-admission.json"),
            "construction replay admission receipt",
        )?;
        let nested_validation = construction_packet.join("evidence/locators/validation_packet");
        let qualification =
            construction_packet.join("evidence/locators/qualification_receipt/input");
        let nested_construction = construction_packet.join("evidence/exact-join-construction");
        let construction_receipt = super::super::source_promotion_control::source_promotion_control_tests::write_construction_replay_packet(
            &nested_construction,
            &admission_receipt,
            digest_file(&nested_admission.join(PACKET_INDEX), "construction replay admission index")?,
            digest_file(
                &nested_admission.join("resolved-tree-admission.json"),
                "construction replay admission receipt",
            )?,
            digest_file(
                &nested_validation.join(PACKET_INDEX),
                "construction replay validation index",
            )?,
            digest_file(&qualification, "construction replay qualification")?,
        )?;
        let mut report = read_json(
            &construction_packet.join(REPORT_JSON),
            "construction replay workflow report",
        )?;
        report["phase"] = Value::String("final".to_string());
        report["controller_packets"]["exact_join_construction"] = serde_json::json!({
            "path": "evidence/exact-join-construction",
            "available": true,
            "status": "constructed",
            "schema": "ripr.source_promotion_exact_join_construction.v1",
        });
        report["producer"]["constructor_state"] = Value::String("passed".to_string());
        report["attempts"] =
            normalized_attempts(&Some(admission_receipt), &Some(construction_receipt), true);
        report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
        rewrite_test_packet_reports_and_index(&construction_packet, &report)?;
        verify_packet(&construction_packet)?;

        let construction_receipt_path = nested_construction.join("exact-join-construction.json");
        let construction_receipt = read_json(
            &construction_receipt_path,
            "exact admitted construction schema fixture",
        )?;
        let mut receipt_with_extra = construction_receipt.clone();
        receipt_with_extra["merge_authorized"] = Value::Bool(false);
        write_pretty_json(
            &construction_receipt_path,
            &receipt_with_extra,
            "digest-rebound admitted construction schema fixture",
        )?;
        reindex_control_packet(&nested_construction)?;
        reindex_workflow_packet(&construction_packet)?;
        if verify_packet(&construction_packet).is_ok() {
            return Err(
                "digest-rebound admitted construction field escaped exact schema".to_string(),
            );
        }
        write_pretty_json(
            &construction_receipt_path,
            &construction_receipt,
            "restored admitted construction schema fixture",
        )?;
        reindex_control_packet(&nested_construction)?;
        reindex_workflow_packet(&construction_packet)?;
        verify_packet(&construction_packet)?;

        fs::write(
            nested_construction.join("undeclared.log"),
            b"undeclared construction evidence\n",
        )
        .map_err(|error| format!("failed to add undeclared construction member: {error}"))?;
        reindex_control_packet(&nested_construction)?;
        reindex_workflow_packet(&construction_packet)?;
        if verify_packet(&construction_packet).is_ok() {
            return Err(
                "digest-rebound undeclared construction member escaped exact inventory".to_string(),
            );
        }
        fs::remove_dir_all(&construction_root).map_err(|error| {
            format!("failed to clean undeclared construction member fixture: {error}")
        })?;
        Ok(())
    }

    #[test]
    fn verifier_rejects_digest_rebound_nested_markdown() -> Result<(), String> {
        let (root, packet) = write_test_closure("nested-markdown-moved")?;
        let builder_root = packet.join("evidence/trusted-builder");
        fs::write(
            builder_root.join("trusted-builder.md"),
            b"# Trusted source-promotion builder\n\n- Status: **rejected**\n",
        )
        .map_err(|error| format!("failed to mutate nested builder Markdown: {error}"))?;
        let builder_index_sha256 = reindex_packet_inventory(&builder_root)?;
        let builder_receipt_sha256 = digest_file(
            &builder_root.join("trusted-builder.json"),
            "builder receipt with moved Markdown",
        )?;
        let admission_root = packet.join("evidence/resolved-tree-admission");
        let admission_report = admission_root.join("resolved-tree-admission.json");
        let mut admission = read_json(&admission_report, "nested-Markdown admission fixture")?;
        admission["trusted_builder_packet_index_sha256"] = Value::String(builder_index_sha256);
        admission["trusted_builder_receipt_sha256"] = Value::String(builder_receipt_sha256);
        write_pretty_json(
            &admission_report,
            &admission,
            "rebound nested-Markdown admission fixture",
        )?;
        reindex_control_packet(&admission_root)?;
        reindex_workflow_packet(&packet)?;
        if verify_packet(&packet).is_ok() {
            return Err("digest-rebound nested Markdown escaped JSON parity".to_string());
        }
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean nested Markdown fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn verifier_requires_original_external_request_and_digest() -> Result<(), String> {
        let (root, packet) = write_test_closure("external-request-authority")?;
        let external = root.join("original-request.json");
        fs::copy(packet.join("evidence/requested-identity.json"), &external)
            .map_err(|error| format!("failed to copy external request fixture: {error}"))?;
        let original_digest = digest_file(&external, "original external request")?;
        let valid_args = strings(&[
            VERIFY,
            "--packet",
            &path_text(&packet)?,
            "--requested-identity",
            &path_text(&external)?,
            "--requested-identity-sha256",
            &original_digest,
        ]);
        verify_command(&valid_args)?;
        let mut moved = read_json(&external, "external request fixture")?;
        moved["protected_w7_ref"] =
            Value::String("refs/tags/ripr-release-external-moved".to_string());
        write_pretty_json(&external, &moved, "moved external request fixture")?;
        let moved_digest = digest_file(&external, "moved external request")?;
        let moved_args = strings(&[
            VERIFY,
            "--packet",
            &path_text(&packet)?,
            "--requested-identity",
            &path_text(&external)?,
            "--requested-identity-sha256",
            &moved_digest,
        ]);
        if verify_command(&moved_args).is_ok() {
            return Err("packet accepted a different external request authority".to_string());
        }
        let stale_digest_args = strings(&[
            VERIFY,
            "--packet",
            &path_text(&packet)?,
            "--requested-identity",
            &path_text(&external)?,
            "--requested-identity-sha256",
            &original_digest,
        ]);
        if verify_command(&stale_digest_args).is_ok() {
            return Err(
                "packet accepted request bytes behind a stale authority digest".to_string(),
            );
        }
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean external request fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn admitted_constructor_packet_replays_qualification_before_enforcement() -> Result<(), String>
    {
        let (root, packet) =
            write_admitted_test_closure_for("qualification-replay", "constructor_dry_run")?;
        verify_packet(&packet)?;
        let qualification_path = packet.join("evidence/locators/qualification_receipt/input");
        let mut qualification = read_json(&qualification_path, "qualification fixture")?;
        qualification["lanes"][0]["state"] = Value::String("failed".to_string());
        write_pretty_json(
            &qualification_path,
            &qualification,
            "mutated qualification fixture",
        )?;
        let mut report = read_json(&packet.join(REPORT_JSON), "qualification workflow report")?;
        report["locators"]["qualification_receipt"]["sha256"] =
            Value::String(digest_file(&qualification_path, "mutated qualification")?);
        report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
        rewrite_test_packet_reports_and_index(&packet, &report)?;
        if verify_packet(&packet).is_ok() {
            return Err("pre-enforcement qualification mutation escaped replay".to_string());
        }
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean qualification fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn rejected_constructor_packet_replays_digest_rebound_qualification() -> Result<(), String> {
        let (root, packet) =
            write_rejected_constructor_test_closure("rejected-qualification-replay", false)?;
        verify_packet(&packet)?;
        let qualification_path = packet.join("evidence/locators/qualification_receipt/input");
        let mut qualification = read_json(&qualification_path, "rejected qualification fixture")?;
        qualification["lanes"][0]["state"] = Value::String("failed".to_string());
        write_pretty_json(
            &qualification_path,
            &qualification,
            "mutated rejected qualification fixture",
        )?;
        let mut report = read_json(
            &packet.join(REPORT_JSON),
            "rejected qualification workflow report",
        )?;
        report["locators"]["qualification_receipt"]["sha256"] = Value::String(digest_file(
            &qualification_path,
            "mutated rejected qualification",
        )?);
        report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
        rewrite_test_packet_reports_and_index(&packet, &report)?;
        if verify_packet(&packet).is_ok() {
            return Err("rejected constructor qualification mutation escaped replay".to_string());
        }
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean rejected qualification fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn rejected_constructor_replays_parseable_attempt_and_outer_parity() -> Result<(), String> {
        for attempted in [false, true] {
            let label = if attempted {
                "parseable-rejection-attempted"
            } else {
                "parseable-rejection-not-attempted"
            };
            let (root, packet) =
                write_parseable_rejected_constructor_test_closure(label, attempted)?;
            let baseline = verify_packet(&packet)?;
            let mut moved = baseline;
            moved["attempts"]["constructor_commit_tree_attempts"] =
                Value::from(usize::from(!attempted));
            rewrite_test_packet_reports_and_index(&packet, &moved)?;
            if verify_packet(&packet).is_ok() {
                return Err(format!(
                    "parseable rejected construction attempted={attempted} escaped outer parity"
                ));
            }
            fs::remove_dir_all(&root)
                .map_err(|error| format!("failed to clean parseable rejection fixture: {error}"))?;
        }
        Ok(())
    }

    #[test]
    fn rejected_constructor_replays_forbidden_inner_authority() -> Result<(), String> {
        let (root, packet) =
            write_parseable_rejected_constructor_test_closure("rejected-inner-authority", true)?;
        verify_packet(&packet)?;
        let construction = packet.join("evidence/exact-join-construction");
        let receipt_path = construction.join("exact-join-construction.json");
        let mut receipt = read_json(&receipt_path, "rejected construction authority fixture")?;
        receipt["ref_mutation_attempted"] = Value::Bool(true);
        write_pretty_json(
            &receipt_path,
            &receipt,
            "mutated rejected construction receipt",
        )?;
        reindex_control_packet(&construction)?;
        reindex_workflow_packet(&packet)?;
        if verify_packet(&packet).is_ok() {
            return Err("rejected construction inner ref authority escaped replay".to_string());
        }
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean rejected authority fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn attempted_rejected_constructor_requires_complete_identity() -> Result<(), String> {
        for key in [
            "source_parent",
            "swarm_parent",
            "join_tree",
            "preflight_sha256",
            "resolution_manifest_sha256",
        ] {
            let (root, packet) = write_parseable_rejected_constructor_test_closure(
                &format!("attempted-construction-null-{key}"),
                true,
            )?;
            verify_packet(&packet)?;
            let construction = packet.join("evidence/exact-join-construction");
            let receipt_path = construction.join("exact-join-construction.json");
            let mut receipt = read_json(&receipt_path, "attempted construction identity fixture")?;
            receipt[key] = Value::Null;
            write_pretty_json(
                &receipt_path,
                &receipt,
                "digest-rebound attempted construction identity fixture",
            )?;
            reindex_control_packet(&construction)?;
            reindex_workflow_packet(&packet)?;
            if verify_packet(&packet).is_ok() {
                return Err(format!(
                    "attempted rejected construction accepted null {key}"
                ));
            }
            fs::remove_dir_all(&root).map_err(|error| {
                format!("failed to clean attempted construction {key} fixture: {error}")
            })?;
        }
        Ok(())
    }

    #[test]
    fn finalizer_records_rejected_exit_and_unknown_attempts_without_constructor_receipt()
    -> Result<(), String> {
        let (root, packet) =
            write_admitted_test_closure_for("finalizer-rejection", "constructor_dry_run")?;
        let workspace = root.join("workspace");
        fs::create_dir(&workspace)
            .map_err(|error| format!("failed to create finalizer workspace: {error}"))?;
        let final_packet = workspace.join("final-packet");
        let args = strings(&[
            FINALIZE,
            "--admission-packet",
            &path_text(&packet)?,
            "--workspace-root",
            &path_text(&workspace)?,
            "--out",
            &path_text(&final_packet)?,
        ]);
        if finalize(&args).is_ok() {
            return Err("fixture finalizer unexpectedly constructed an exact join".to_string());
        }
        let report = verify_packet(&final_packet)?;
        if json_string(&report, "status") != Some("rejected")
            || report["producer"]["normalized_exit_code"].as_u64() != Some(1)
        {
            return Err("finalizer rejection did not preserve its normalized exit".to_string());
        }
        for key in [
            "constructor_commit_tree_attempts",
            "local_ref_attempts",
            "remote_push_attempts",
            "merge_command_attempts",
        ] {
            if !report["attempts"][key].is_null() {
                return Err(format!(
                    "finalizer invented observed zero for unknown {key}"
                ));
            }
        }
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean finalizer fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn finalizer_rejects_canonical_controller_escape_and_retains_evidence() -> Result<(), String> {
        let (root, packet) =
            write_admitted_test_closure_for("finalizer-controller-escape", "constructor_dry_run")?;
        let workspace = root.join("workspace");
        let controller_parent = workspace.join("synthetic-fixture");
        fs::create_dir_all(&controller_parent)
            .map_err(|error| format!("failed to create controller parent: {error}"))?;
        let outside = root.join("outside-controller-repository");
        fs::create_dir(&outside)
            .map_err(|error| format!("failed to create outside controller: {error}"))?;
        let alias = controller_parent.join("fixture-repository");
        #[cfg(windows)]
        {
            let alias_text = alias.to_string_lossy().into_owned();
            let outside_text = outside.to_string_lossy().into_owned();
            let output = Command::new("cmd")
                .args([
                    "/c",
                    "mklink",
                    "/J",
                    alias_text.as_str(),
                    outside_text.as_str(),
                ])
                .output()
                .map_err(|error| format!("failed to start controller junction command: {error}"))?;
            if !output.status.success() {
                return Err(format!(
                    "failed to create controller directory junction: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &alias)
            .map_err(|error| format!("failed to create controller directory alias: {error}"))?;
        #[cfg(not(any(windows, unix)))]
        return Err("canonical controller escape test requires symlink support".to_string());

        let report = read_json(&packet.join(REPORT_JSON), "controller escape disposition")?;
        let resolver_error = match controller_repository_path(&workspace, &report) {
            Ok(path) => {
                return Err(format!(
                    "profile-correct controller alias escaped to {}",
                    path.display()
                ));
            }
            Err(error) => error,
        };
        if !resolver_error.contains("escaped the runner-owned workspace") {
            return Err(format!(
                "controller resolver returned the wrong escape reason: {resolver_error}"
            ));
        }
        let final_packet = workspace.join("final-packet");
        let args = strings(&[
            FINALIZE,
            "--admission-packet",
            &path_text(&packet)?,
            "--workspace-root",
            &path_text(&workspace)?,
            "--out",
            &path_text(&final_packet)?,
        ]);
        if finalize(&args).is_ok() {
            return Err("controller repository escape unexpectedly finalized".to_string());
        }
        let final_report = verify_packet(&final_packet)?;
        if json_string(&final_report, "status") != Some("rejected")
            || final_report["producer"]["normalized_exit_code"].as_u64() != Some(1)
            || !final_report["failure_reasons"]
                .as_array()
                .is_some_and(|reasons| {
                    reasons.iter().any(|reason| {
                        reason.as_str().is_some_and(|reason| {
                            reason.contains("escaped the runner-owned workspace")
                        })
                    })
                })
        {
            return Err(
                "canonical controller escape did not retain a terminal rejected packet".to_string(),
            );
        }
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean finalizer escape fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn live_locator_provenance_rejects_same_byte_movement() -> Result<(), String> {
        let report = test_report("admitted", "admit_only", "live", 0)?;
        for (pointer, replacement) in [
            (
                "/locators/preflight/repository",
                Value::String(SWARM_REPOSITORY.to_string()),
            ),
            (
                "/locators/preflight/revision",
                Value::String("9".repeat(40)),
            ),
            (
                "/locators/preflight/path",
                Value::String("moved/preflight.json".to_string()),
            ),
            (
                "/locators/preflight/mode",
                Value::String("100755".to_string()),
            ),
            (
                "/locators/preflight/locator",
                Value::String("EffortlessMetrics/ripr@moved".to_string()),
            ),
        ] {
            let mut moved = report.clone();
            let slot = moved
                .pointer_mut(pointer)
                .ok_or_else(|| format!("test report is missing {pointer}"))?;
            *slot = replacement;
            if validate_report(&moved).is_ok() {
                return Err(format!("same-byte locator movement escaped at {pointer}"));
            }
        }
        Ok(())
    }

    #[test]
    fn synthetic_locator_rejects_absolute_or_out_of_workspace_local_path() -> Result<(), String> {
        let report = test_report("admitted", "admit_only", "positive_synthetic", 0)?;
        for local_path in [
            "/tmp/escaped/preflight.json",
            "../escaped/preflight.json",
            "other-root/preflight.json",
        ] {
            let mut moved = report.clone();
            moved["locators"]["preflight"]["local_path"] = Value::String(local_path.to_string());
            if validate_report(&moved).is_ok() {
                return Err(format!(
                    "synthetic locator local_path escaped workspace: {local_path}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn packet_index_binds_complete_evidence_closure() -> Result<(), String> {
        let (root, packet) = write_test_closure("inventory")?;
        let report = verify_packet(&packet)?;
        let index = read_json(&packet.join(PACKET_INDEX), "test packet index")?;
        let files = index["files"]
            .as_object()
            .ok_or_else(|| "test packet index has no files".to_string())?;
        for required in [
            "evidence/trusted-builder/trusted-builder.json",
            "evidence/resolved-tree-admission/resolved-tree-admission.json",
            "evidence/locators/preflight/input",
            "evidence/locators/resolution_manifest/input",
            "evidence/locators/validation_packet/packet-index.json",
            "evidence/locators/validation_packet/resolved-tree-validation.json",
            "evidence/locators/integration_packet/integration-index.json",
            REPORT_JSON,
            REPORT_MD,
        ] {
            if !files.contains_key(required) {
                return Err(format!("closure index omitted {required}"));
            }
        }
        for (pointer, replacement) in [
            (
                "/controller_packets/trusted_builder/available",
                Value::Bool(false),
            ),
            (
                "/controller_packets/resolved_tree_admission/status",
                Value::String("rejected".to_string()),
            ),
        ] {
            let mut inconsistent = report.clone();
            let slot = inconsistent
                .pointer_mut(pointer)
                .ok_or_else(|| format!("test report is missing {pointer}"))?;
            *slot = replacement;
            if validate_closure_bindings(&packet, &inconsistent).is_ok() {
                return Err(format!(
                    "controller closure inconsistency escaped at {pointer}"
                ));
            }
        }
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean closure inventory fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn packet_verifier_rejects_removed_or_corrupted_closure_member() -> Result<(), String> {
        let (removed_root, removed_packet) = write_test_closure("removed")?;
        fs::remove_file(
            removed_packet
                .join("evidence/locators/validation_packet/resolved-tree-validation.json"),
        )
        .map_err(|error| format!("failed to remove closure member: {error}"))?;
        if verify_packet(&removed_packet).is_ok() {
            return Err("packet verifier accepted removed closure member".to_string());
        }
        fs::remove_dir_all(&removed_root)
            .map_err(|error| format!("failed to clean removed closure fixture: {error}"))?;

        let (corrupt_root, corrupt_packet) = write_test_closure("corrupt")?;
        fs::write(
            corrupt_packet.join("evidence/locators/preflight/input"),
            b"corrupted",
        )
        .map_err(|error| format!("failed to corrupt closure member: {error}"))?;
        if verify_packet(&corrupt_packet).is_ok() {
            return Err("packet verifier accepted corrupted closure member".to_string());
        }
        fs::remove_dir_all(&corrupt_root)
            .map_err(|error| format!("failed to clean corrupt closure fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn rejected_constructor_packet_retains_partial_evidence_or_accepts_no_output()
    -> Result<(), String> {
        let (partial_root, partial_packet) = write_test_closure_for(
            "partial-construction",
            "rejected",
            "constructor_dry_run",
            true,
        )?;
        let partial_report = verify_packet(&partial_packet)?;
        if json_string(&partial_report, "status") != Some("rejected")
            || !partial_packet
                .join("evidence/exact-join-construction/partial.log")
                .is_file()
            || !partial_packet
                .join("evidence/exact-join-construction/exact-join-construction.json")
                .is_file()
        {
            return Err("rejected constructor packet did not retain partial evidence".to_string());
        }
        let enforce_args = strings(&[
            ENFORCE,
            "--packet",
            &path_text(&partial_packet)?,
            "--expected-status",
            "admitted",
        ]);
        if enforce_command(&enforce_args).is_ok() {
            return Err("rejected constructor packet escaped terminal enforcement".to_string());
        }
        fs::remove_dir_all(&partial_root)
            .map_err(|error| format!("failed to clean partial constructor fixture: {error}"))?;

        let (absent_root, absent_packet) = write_test_closure_for(
            "absent-construction",
            "rejected",
            "constructor_dry_run",
            false,
        )?;
        verify_packet(&absent_packet)?;
        if collect_packet_files(&absent_packet)?
            .keys()
            .any(|path| path.starts_with("evidence/exact-join-construction/"))
        {
            return Err("absent constructor output created synthetic evidence".to_string());
        }
        let enforce_args = strings(&[
            ENFORCE,
            "--packet",
            &path_text(&absent_packet)?,
            "--expected-status",
            "admitted",
        ]);
        if enforce_command(&enforce_args).is_ok() {
            return Err("absent-output rejection escaped terminal enforcement".to_string());
        }
        fs::remove_dir_all(&absent_root)
            .map_err(|error| format!("failed to clean absent constructor fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn synthetic_paths_are_root_independent() -> Result<(), String> {
        let left = Path::new("one/root/synthetic-fixture/fixture-repository/.git/evidence.json");
        let right =
            Path::new("another/root/synthetic-fixture/fixture-repository/.git/evidence.json");
        if stable_synthetic_path(left) != stable_synthetic_path(right) {
            return Err("synthetic locator retained its absolute root".to_string());
        }
        Ok(())
    }

    fn write_test_closure(label: &str) -> Result<(PathBuf, PathBuf), String> {
        write_test_closure_for(label, "admitted", "admit_only", false)
    }

    fn write_test_closure_for(
        label: &str,
        status: &str,
        mode: &str,
        partial_construction: bool,
    ) -> Result<(PathBuf, PathBuf), String> {
        if status == "admitted" && mode == "admit_only" {
            return write_admitted_test_closure(label);
        }
        if status == "rejected" && mode == "constructor_dry_run" {
            return write_rejected_constructor_test_closure(label, partial_construction);
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("test clock precedes epoch: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-admission-closure-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root)
            .map_err(|error| format!("failed to create closure test root: {error}"))?;
        let source = root.join("source");
        for directory in [
            "trusted-builder",
            "resolved-tree-admission",
            "preflight",
            "resolution_manifest",
            "validation_packet",
            "integration_packet",
        ] {
            fs::create_dir_all(source.join(directory))
                .map_err(|error| format!("failed to create closure fixture directory: {error}"))?;
        }
        let files = [
            (
                "trusted-builder/trusted-builder.json",
                br#"{"schema":"ripr.source_promotion_trusted_builder.v1","status":"built"}"#
                    .as_slice(),
            ),
            (
                "resolved-tree-admission/resolved-tree-admission.json",
                br#"{"schema":"ripr.source_promotion_resolved_tree_admission.v1","status":"admitted"}"#
                    .as_slice(),
            ),
            ("preflight/input", b"preflight".as_slice()),
            ("resolution_manifest/input", b"resolution".as_slice()),
            (
                "validation_packet/packet-index.json",
                b"validation-index".as_slice(),
            ),
            (
                "validation_packet/validation.json",
                b"validation-receipt".as_slice(),
            ),
            ("integration_packet/index.json", b"integration".as_slice()),
        ];
        for (path, bytes) in files {
            fs::write(source.join(path), bytes)
                .map_err(|error| format!("failed to write closure fixture: {error}"))?;
        }
        if partial_construction {
            let construction = source.join("exact-join-construction");
            fs::create_dir_all(&construction).map_err(|error| {
                format!("failed to create partial construction fixture: {error}")
            })?;
            fs::write(construction.join("partial.log"), b"constructor started\n")
                .map_err(|error| format!("failed to write partial construction log: {error}"))?;
            fs::write(
                construction.join("exact-join-construction.json"),
                b"{malformed",
            )
            .map_err(|error| format!("failed to write malformed construction receipt: {error}"))?;
        }
        let mut report = test_report(status, mode, "positive_synthetic", 0)?;
        for (key, path) in [
            ("preflight", "preflight/input"),
            ("resolution_manifest", "resolution_manifest/input"),
            ("validation_packet", "validation_packet/packet-index.json"),
            ("integration_packet", "integration_packet/index.json"),
        ] {
            report["locators"][key]["sha256"] =
                Value::String(digest_file(&source.join(path), "closure fixture")?);
        }
        report["controller_packets"] = serde_json::json!({
            "trusted_builder": {
                "path": "evidence/trusted-builder",
                "available": true,
                "status": "built",
                "schema": "ripr.source_promotion_trusted_builder.v1",
            },
            "resolved_tree_admission": {
                "path": "evidence/resolved-tree-admission",
                "available": true,
                "status": "admitted",
                "schema": "ripr.source_promotion_resolved_tree_admission.v1",
            },
            "exact_join_construction": if mode == "constructor_dry_run" {
                serde_json::json!({
                    "path": "evidence/exact-join-construction",
                    "available": false,
                    "status": null,
                    "schema": null,
                })
            } else {
                serde_json::json!({
                    "path": null,
                    "available": false,
                    "status": "not_run",
                    "schema": null,
                })
            },
        });
        let request = requested_identity_for_report(&report)?;
        let request_bytes = serde_json::to_vec_pretty(&request)
            .map_err(|error| format!("failed to serialize requested identity fixture: {error}"))?;
        fs::write(source.join("requested-identity.json"), &request_bytes)
            .map_err(|error| format!("failed to write requested identity fixture: {error}"))?;
        report["requested_identity_sha256"] = Value::String(digest_bytes(&request_bytes));
        report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
        let mut closure = [
            ("requested-identity.json", "requested-identity.json"),
            ("trusted-builder", "trusted-builder"),
            ("resolved-tree-admission", "resolved-tree-admission"),
            ("preflight", "locators/preflight"),
            ("resolution_manifest", "locators/resolution_manifest"),
            ("validation_packet", "locators/validation_packet"),
            ("integration_packet", "locators/integration_packet"),
        ]
        .into_iter()
        .map(|(from, to)| ClosureSource {
            source: source.join(from),
            destination: PathBuf::from(to),
        })
        .collect::<Vec<_>>();
        if let Some(construction) = optional_closure_source(
            &source.join("exact-join-construction"),
            "exact-join-construction",
            "test construction evidence",
        )? {
            closure.push(construction);
        } else if partial_construction {
            return Err("partial construction fixture was not selected for closure".to_string());
        }
        let packet = root.join("packet");
        write_packet(&packet, &report, &closure, &[])?;
        Ok((root, packet))
    }

    fn write_admitted_test_closure(label: &str) -> Result<(PathBuf, PathBuf), String> {
        write_admitted_test_closure_for(label, "admit_only")
    }

    fn write_rejected_constructor_test_closure(
        label: &str,
        partial_construction: bool,
    ) -> Result<(PathBuf, PathBuf), String> {
        let (root, packet) = write_admitted_test_closure_for(label, "constructor_dry_run")?;
        let construction = packet.join("evidence/exact-join-construction");
        if partial_construction {
            fs::create_dir(&construction).map_err(|error| {
                format!("failed to create partial construction fixture: {error}")
            })?;
            fs::write(construction.join("partial.log"), b"constructor started\n")
                .map_err(|error| format!("failed to write partial construction log: {error}"))?;
            fs::write(
                construction.join("exact-join-construction.json"),
                b"{malformed",
            )
            .map_err(|error| format!("failed to write malformed construction receipt: {error}"))?;
        }
        let mut report = read_json(&packet.join(REPORT_JSON), "rejected constructor fixture")?;
        let admission = read_json(
            &packet
                .join("evidence/resolved-tree-admission")
                .join("resolved-tree-admission.json"),
            "rejected constructor admission receipt",
        )?;
        report["phase"] = Value::String("final".to_string());
        report["status"] = Value::String("rejected".to_string());
        report["controller_packets"]["exact_join_construction"] = packet_state(
            &construction,
            "exact-join-construction.json",
            "evidence/exact-join-construction",
        );
        report["producer"]["normalized_exit_code"] = Value::from(1);
        report["producer"]["constructor_state"] = Value::String("rejected".to_string());
        report["attempts"] = normalized_attempts(&Some(admission), &None, true);
        report["failure_reasons"] = serde_json::json!(["synthetic constructor rejection"]);
        report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
        rewrite_test_packet_reports_and_index(&packet, &report)?;
        Ok((root, packet))
    }

    fn write_parseable_rejected_constructor_test_closure(
        label: &str,
        attempted: bool,
    ) -> Result<(PathBuf, PathBuf), String> {
        let (root, packet) = write_rejected_constructor_test_closure(label, false)?;
        let mut report = read_json(&packet.join(REPORT_JSON), "parseable rejection fixture")?;
        let construction = packet.join("evidence/exact-join-construction");
        let receipt = serde_json::json!({
            "schema": "ripr.source_promotion_exact_join_construction.v1",
            "status": "rejected",
            "source_parent": required_json_string(&report, "source_parent_sha")?,
            "swarm_parent": required_json_string(&report, "w7_peeled_sha")?,
            "join_tree": required_json_string(&report, "reviewed_tree_sha")?,
            "preflight_sha256": required_json_string(&report["locators"]["preflight"], "sha256")?,
            "resolution_manifest_sha256": required_json_string(&report["locators"]["resolution_manifest"], "sha256")?,
            "candidate_ref": "refs/heads/promote/0.11.0-admission-dry-run",
            "join_commit": null,
            "ordered_parents": [],
            "final_identity_reread_passed": false,
            "refs_unchanged": null,
            "authoritative_commit_attempted": attempted,
            "commit_tree_attempts": usize::from(attempted),
            "local_ref_attempts": 0,
            "remote_push_attempts": 0,
            "merge_command_attempts": 0,
            "unreferenced_exact_join_constructed": false,
            "ref_mutation_attempted": false,
            "push_attempted": false,
            "merge_command": null,
            "failure_reasons": ["synthetic parseable constructor rejection"],
            "non_claims": [
                "A rejected construction receipt cannot be published.",
                "No candidate ref, merge command, source integration, release, or public channel authority was created.",
            ],
        });
        write_rejected_control_packet(
            &construction,
            "exact_join_construction",
            "exact-join-construction.json",
            &receipt,
        )?;
        let admission = read_json(
            &packet
                .join("evidence/resolved-tree-admission")
                .join("resolved-tree-admission.json"),
            "parseable rejection admission receipt",
        )?;
        report["controller_packets"]["exact_join_construction"] = packet_state(
            &construction,
            "exact-join-construction.json",
            "evidence/exact-join-construction",
        );
        report["attempts"] = normalized_attempts(&Some(admission), &Some(receipt), true);
        report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
        rewrite_test_packet_reports_and_index(&packet, &report)?;
        Ok((root, packet))
    }

    fn write_admission_rejected_test_closure(label: &str) -> Result<(PathBuf, PathBuf), String> {
        let (root, packet) = write_admitted_test_closure_for(label, "constructor_dry_run")?;
        let mut report = read_json(&packet.join(REPORT_JSON), "rejected workflow fixture")?;
        let admission = packet.join("evidence/resolved-tree-admission");
        fs::remove_dir_all(&admission)
            .map_err(|error| format!("failed to replace rejected admission packet: {error}"))?;
        let receipt = serde_json::json!({
            "schema": "ripr.source_promotion_resolved_tree_admission.v1",
            "status": "rejected",
            "identity": {
                "source_parent": required_json_string(&report, "source_parent_sha")?,
                "swarm_parent": required_json_string(&report, "w7_peeled_sha")?,
                "join_tree": required_json_string(&report, "reviewed_tree_sha")?,
                "preflight_sha256": required_json_string(&report["locators"]["preflight"], "sha256")?,
                "resolution_manifest_sha256": required_json_string(&report["locators"]["resolution_manifest"], "sha256")?,
            },
            "source_parent": required_json_string(&report, "source_parent_sha")?,
            "swarm_parent": required_json_string(&report, "w7_peeled_sha")?,
            "join_tree": required_json_string(&report, "reviewed_tree_sha")?,
            "preflight_sha256": required_json_string(&report["locators"]["preflight"], "sha256")?,
            "resolution_manifest_sha256": required_json_string(&report["locators"]["resolution_manifest"], "sha256")?,
            "all_required_typed_integration_receipts_present": false,
            "final_identity_reread_passed": false,
            "constructor_eligible_after_tree_qualification": false,
            "authoritative_commit_attempted": false,
            "commit_tree_attempts": 0,
            "local_ref_attempts": 0,
            "remote_push_attempts": 0,
            "merge_command_attempts": 0,
            "ref_mutation_attempted": false,
            "push_attempted": false,
            "merge_command": null,
            "failure_reasons": ["synthetic admission rejection"],
            "non_claims": [
                "A rejected admission is not construction eligibility.",
                "No exact join object, candidate ref, merge command, release, or publication authority was created.",
            ],
        });
        write_rejected_control_packet(
            &admission,
            "resolved_tree_admission",
            "resolved-tree-admission.json",
            &receipt,
        )?;
        let qualification = packet.join("evidence/locators/qualification_receipt/input");
        fs::remove_file(&qualification)
            .map_err(|error| format!("failed to remove rejected qualification fixture: {error}"))?;
        report["phase"] = Value::String("admission".to_string());
        report["status"] = Value::String("rejected".to_string());
        report["execution_profile"] = Value::String("positive_synthetic".to_string());
        report["locators"]["qualification_receipt"] = serde_json::json!({
            "schema": LOCATOR_SCHEMA,
            "status": "not_required",
            "label": "qualification receipt",
        });
        report["controller_packets"]["resolved_tree_admission"] = packet_state(
            &admission,
            "resolved-tree-admission.json",
            "evidence/resolved-tree-admission",
        );
        report["controller_packets"]["exact_join_construction"] = serde_json::json!({
            "path": null,
            "available": false,
            "status": "not_run",
            "schema": null,
        });
        report["producer"] = serde_json::json!({
            "normalized_exit_code": 1,
            "trusted_builder_state": "passed",
            "admission_state": "rejected",
            "constructor_state": "not_run_before_upload_and_enforcement",
        });
        report["attempts"] = normalized_attempts(&Some(receipt), &None, false);
        report["failure_reasons"] = serde_json::json!(["synthetic admission rejection"]);
        let request = requested_identity_for_report(&report)?;
        let request_bytes = serde_json::to_vec_pretty(&request)
            .map_err(|error| format!("failed to serialize rejected requested identity: {error}"))?;
        fs::write(
            packet.join("evidence/requested-identity.json"),
            &request_bytes,
        )
        .map_err(|error| format!("failed to write rejected requested identity: {error}"))?;
        report["requested_identity_sha256"] = Value::String(digest_bytes(&request_bytes));
        report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
        rewrite_test_packet_reports_and_index(&packet, &report)?;
        Ok((root, packet))
    }

    fn write_builder_rejected_test_closure(label: &str) -> Result<(PathBuf, PathBuf), String> {
        let (root, packet) = write_admitted_test_closure(label)?;
        let mut report = read_json(&packet.join(REPORT_JSON), "builder rejection fixture")?;
        let builder = packet.join("evidence/trusted-builder");
        fs::remove_dir_all(&builder)
            .map_err(|error| format!("failed to replace rejected builder packet: {error}"))?;
        let receipt = serde_json::json!({
            "schema": "ripr.source_promotion_trusted_builder.v1",
            "status": "rejected",
            "source_parent": required_json_string(&report, "source_parent_sha")?,
            "workflow_source_sha": required_json_string(&report, "workflow_source_sha")?,
            "clean_checkout": false,
            "rust_toolchain": null,
            "cargo_lock_sha256": null,
            "locked_build": true,
            "isolated_cargo_target_dir": true,
            "executable_sha256": null,
            "failure_reasons": ["synthetic builder rejection"],
            "authoritative_commit_attempted": false,
            "commit_tree_attempts": 0,
            "local_ref_attempts": 0,
            "remote_push_attempts": 0,
            "merge_command_attempts": 0,
            "merge_command": null,
            "ref_mutation_attempted": false,
            "push_attempted": false,
        });
        write_rejected_control_packet(
            &builder,
            "trusted_builder",
            "trusted-builder.json",
            &receipt,
        )?;
        let admission = packet.join("evidence/resolved-tree-admission");
        fs::remove_dir_all(&admission)
            .map_err(|error| format!("failed to remove skipped admission packet: {error}"))?;
        report["phase"] = Value::String("admission".to_string());
        report["status"] = Value::String("rejected".to_string());
        report["controller_packets"]["trusted_builder"] =
            packet_state(&builder, "trusted-builder.json", "evidence/trusted-builder");
        report["controller_packets"]["resolved_tree_admission"] = packet_state(
            &admission,
            "resolved-tree-admission.json",
            "evidence/resolved-tree-admission",
        );
        report["producer"] = serde_json::json!({
            "normalized_exit_code": 1,
            "trusted_builder_state": "rejected",
            "admission_state": "rejected",
            "constructor_state": "not_run_before_upload_and_enforcement",
        });
        report["attempts"] = normalized_attempts(&None, &None, false);
        report["failure_reasons"] = serde_json::json!(["synthetic builder rejection"]);
        report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
        rewrite_test_packet_reports_and_index(&packet, &report)?;
        Ok((root, packet))
    }

    fn write_rejected_control_packet(
        root: &Path,
        kind: &str,
        report_name: &str,
        report: &Value,
    ) -> Result<(), String> {
        fs::create_dir(root)
            .map_err(|error| format!("failed to create rejected control packet: {error}"))?;
        fs::write(root.join("control-attempt.json"), b"{}\n")
            .map_err(|error| format!("failed to write rejected attempt journal: {error}"))?;
        write_pretty_json(&root.join(report_name), report, "rejected control receipt")?;
        let markdown = super::super::source_promotion_control::render_rejected_control_markdown(
            report_name,
            report,
        )?;
        let markdown_name = report_name
            .strip_suffix(".json")
            .map(|stem| format!("{stem}.md"))
            .ok_or_else(|| "rejected report name must end in .json".to_string())?;
        fs::write(root.join(markdown_name), markdown)
            .map_err(|error| format!("failed to write rejected control Markdown: {error}"))?;
        write_pretty_json(
            &root.join(PACKET_INDEX),
            &serde_json::json!({
                "schema": "ripr.source_promotion_control_packet.v1",
                "kind": kind,
                "status": "rejected",
                "complete": true,
                "files": [],
            }),
            "rejected control packet index",
        )?;
        reindex_packet_inventory(root)?;
        Ok(())
    }

    fn write_admitted_test_closure_for(
        label: &str,
        mode: &str,
    ) -> Result<(PathBuf, PathBuf), String> {
        let fixture = super::super::source_promotion_control::source_promotion_control_tests::admission_replay_fixture(label)?;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("test clock precedes epoch: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-admission-replay-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root)
            .map_err(|error| format!("failed to create replay closure root: {error}"))?;
        let request_path = root.join("requested-identity.json");
        let mut report = test_report("admitted", mode, "positive_synthetic", 0)?;
        report["phase"] = Value::String("admission".to_string());
        report["producer"]["constructor_state"] =
            Value::String("not_run_before_upload_and_enforcement".to_string());
        report["source_parent_sha"] = Value::String(fixture.source_parent.clone());
        report["workflow_source_sha"] = Value::String(fixture.source_parent.clone());
        report["trusted_checker_identity"] =
            Value::String(format!("source-owned-xtask@{}", fixture.source_parent));
        report["w7_peeled_sha"] = Value::String(fixture.swarm_parent.clone());
        report["reviewed_tree_sha"] = Value::String(fixture.join_tree.clone());
        report["protected_w7_ref"] = Value::String(fixture.swarm_ref.clone());
        report["locators"]["integration_packet"]["path"] =
            Value::String("synthetic-fixture/integration/integration-index.json".to_string());
        report["locators"]["integration_packet"]["local_path"] =
            Value::String("synthetic-fixture/integration/integration-index.json".to_string());
        let validation_index = fixture.validation_packet.join(PACKET_INDEX);
        for (key, path) in [
            ("preflight", fixture.preflight.as_path()),
            ("resolution_manifest", fixture.resolution.as_path()),
            ("validation_packet", validation_index.as_path()),
            ("integration_packet", fixture.integration_index.as_path()),
        ] {
            report["locators"][key]["sha256"] =
                Value::String(digest_file(path, "replay fixture locator")?);
        }
        if mode == "constructor_dry_run" {
            report["locators"]["qualification_receipt"]["sha256"] = Value::String(digest_file(
                &fixture.qualification,
                "replay qualification locator",
            )?);
        }
        report["controller_packets"] = serde_json::json!({
            "trusted_builder": {
                "path": "evidence/trusted-builder",
                "available": true,
                "status": "built",
                "schema": "ripr.source_promotion_trusted_builder.v1",
            },
            "resolved_tree_admission": {
                "path": "evidence/resolved-tree-admission",
                "available": true,
                "status": "admitted",
                "schema": "ripr.source_promotion_resolved_tree_admission.v1",
            },
            "exact_join_construction": {
                "path": null,
                "available": false,
                "status": "not_run",
                "schema": null,
            },
        });
        let admission_receipt = read_json(
            &fixture
                .admission_packet
                .join("resolved-tree-admission.json"),
            "replay admission receipt",
        )?;
        report["attempts"] = normalized_attempts(&Some(admission_receipt), &None, false);
        let request = requested_identity_for_report(&report)?;
        let request_bytes = serde_json::to_vec_pretty(&request)
            .map_err(|error| format!("failed to serialize replay requested identity: {error}"))?;
        fs::write(&request_path, &request_bytes)
            .map_err(|error| format!("failed to write replay requested identity: {error}"))?;
        report["requested_identity_sha256"] = Value::String(digest_bytes(&request_bytes));
        report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
        let integration_root = fixture
            .integration_index
            .parent()
            .ok_or_else(|| "replay integration index has no parent".to_string())?
            .to_path_buf();
        let mut closure = vec![
            ClosureSource {
                source: request_path,
                destination: PathBuf::from("requested-identity.json"),
            },
            ClosureSource {
                source: fixture.builder_packet,
                destination: PathBuf::from("trusted-builder"),
            },
            ClosureSource {
                source: fixture.admission_packet,
                destination: PathBuf::from("resolved-tree-admission"),
            },
            ClosureSource {
                source: fixture.preflight,
                destination: PathBuf::from("locators/preflight/input"),
            },
            ClosureSource {
                source: fixture.resolution,
                destination: PathBuf::from("locators/resolution_manifest/input"),
            },
            ClosureSource {
                source: fixture.validation_packet,
                destination: PathBuf::from("locators/validation_packet"),
            },
            ClosureSource {
                source: integration_root,
                destination: PathBuf::from("locators/integration_packet"),
            },
        ];
        if mode == "constructor_dry_run" {
            closure.push(ClosureSource {
                source: fixture.qualification,
                destination: PathBuf::from("locators/qualification_receipt/input"),
            });
        }
        let packet = root.join("packet");
        write_packet(&packet, &report, &closure, &[])?;
        Ok((root, packet))
    }

    fn test_report(
        status: &str,
        mode: &str,
        profile: &str,
        commit_tree: u64,
    ) -> Result<Value, String> {
        let synthetic = profile != "live";
        let j5_negative = profile == "j5_negative";
        let phase = if j5_negative { "admission" } else { "final" };
        let construction_attempts_unknown =
            phase == "final" && mode == "constructor_dry_run" && status == "rejected";
        let locator = |label: &str, path: &str, sha256: String| {
            if synthetic {
                serde_json::json!({
                    "schema": LOCATOR_SCHEMA,
                    "status": "source_owned_synthetic",
                    "label": label,
                    "mode": "100644",
                    "sha256": sha256,
                    "path": format!("synthetic-fixture/{path}"),
                    "local_path": format!("synthetic-fixture/{path}"),
                })
            } else {
                let revision = "5".repeat(40);
                let locator = format!("{SOURCE_REPOSITORY}@{revision}:{path}#sha256:{sha256}");
                serde_json::json!({
                    "schema": LOCATOR_SCHEMA,
                    "repository": SOURCE_REPOSITORY,
                    "revision": revision,
                    "path": path,
                    "mode": "100644",
                    "sha256": sha256,
                    "locator": locator,
                })
            }
        };
        let qualification =
            if mode == "constructor_dry_run" && (phase == "final" || status == "admitted") {
                locator(
                    "qualification receipt",
                    "qualification/receipt.json",
                    "2".repeat(64),
                )
            } else {
                serde_json::json!({
                    "schema": LOCATOR_SCHEMA,
                    "status": "not_required",
                    "label": "qualification receipt",
                })
            };
        let mut report = serde_json::json!({
            "schema": SCHEMA,
            "phase": phase,
            "status": status,
            "complete": true,
            "operation_mode": mode,
            "execution_profile": profile,
            "fixture_identity": "a".repeat(64),
            "controller_repository": if synthetic { SYNTHETIC_CONTROLLER_REPOSITORY } else { LIVE_CONTROLLER_REPOSITORY },
            "workflow_identity_sha256": null,
            "requested_identity_sha256": "9".repeat(64),
            "source_repository": SOURCE_REPOSITORY,
            "source_parent_sha": "a".repeat(40),
            "workflow_source_sha": "a".repeat(40),
            "trusted_checker_identity": format!("source-owned-xtask@{}", "a".repeat(40)),
            "swarm_repository": SWARM_REPOSITORY,
            "protected_w7_ref": "refs/tags/ripr-release-fixture-w7",
            "w7_peeled_sha": "b".repeat(40),
            "reviewed_tree_sha": "c".repeat(40),
            "reviewed_tree_carrier_sha": if synthetic { "not_required".to_string() } else { "4".repeat(40) },
            "receipt_schema": SUPPORTED_RECEIPT_SCHEMA,
            "locators": {
                "preflight": locator("preflight", "preflight/preflight.json", "d".repeat(64)),
                "resolution_manifest": locator("resolution manifest", "resolution/manifest.json", "e".repeat(64)),
                "validation_packet": locator("validation packet", "validation/packet-index.json", "f".repeat(64)),
                "integration_packet": locator("integration packet", "integration/index.json", "1".repeat(64)),
                "qualification_receipt": qualification,
            },
            "controller_packets": {
                "trusted_builder": {
                    "path": "evidence/trusted-builder",
                    "available": true,
                    "status": "built",
                    "schema": "ripr.source_promotion_trusted_builder.v1",
                },
                "resolved_tree_admission": if j5_negative {
                    serde_json::json!({
                        "path": "evidence/resolved-tree-admission",
                        "available": true,
                        "status": "rejected",
                        "schema": "ripr.source_promotion_resolved_tree_admission.v1",
                    })
                } else { serde_json::json!({
                    "path": "evidence/resolved-tree-admission",
                    "available": true,
                    "status": "admitted",
                    "schema": "ripr.source_promotion_resolved_tree_admission.v1",
                })},
                "exact_join_construction": if phase == "final" && mode == "constructor_dry_run" && status == "admitted" {
                    serde_json::json!({
                        "path": "evidence/exact-join-construction",
                        "available": true,
                        "status": "constructed",
                        "schema": "ripr.source_promotion_exact_join_construction.v1",
                    })
                } else if phase == "final" && mode == "constructor_dry_run" {
                    serde_json::json!({
                        "path": "evidence/exact-join-construction",
                        "available": false,
                        "status": null,
                        "schema": null,
                    })
                } else {
                    serde_json::json!({
                        "path": null,
                        "available": false,
                        "status": "not_run",
                        "schema": null,
                    })
                },
            },
            "producer": {
                "normalized_exit_code": if status == "admitted" { 0 } else { 1 },
                "trusted_builder_state": "passed",
                "admission_state": if j5_negative { "rejected" } else { "passed" },
                "constructor_state": if phase == "admission" { "not_run_before_upload_and_enforcement" } else if mode == "admit_only" { "not_requested" } else if status == "admitted" { "passed" } else { "rejected" },
            },
            "attempts": {
                "admission_receipt_available": true,
                "construction_receipt_available": mode == "constructor_dry_run" && status == "admitted",
                "constructor_refs_unchanged": commit_tree == 1,
                "constructor_object_unreferenced": commit_tree == 1,
                "constructor_final_identity_reread_passed": commit_tree == 1,
                "constructor_commit_tree_attempts": if construction_attempts_unknown { Value::Null } else { Value::from(commit_tree) },
                "local_ref_attempts": if construction_attempts_unknown { Value::Null } else { Value::from(0) },
                "remote_push_attempts": if construction_attempts_unknown { Value::Null } else { Value::from(0) },
                "merge_command_attempts": if construction_attempts_unknown { Value::Null } else { Value::from(0) },
                "release_or_publication_attempts": 0,
                "release_or_publication_command_reachable": false,
                "release_or_publication_proof": "closed workflow harness dispatch contains no publication subcommand",
            },
            "failure_reasons": if status == "admitted" { Vec::<String>::new() } else { vec!["synthetic rejection".to_string()] },
        });
        report["workflow_identity_sha256"] = Value::String(workflow_identity(&report)?);
        Ok(report)
    }

    fn requested_identity_for_report(report: &Value) -> Result<Value, String> {
        let synthetic = required_json_string(report, "execution_profile")? != "live";
        let locators = report
            .get("locators")
            .ok_or_else(|| "test report is missing locators".to_string())?;
        let requested_locator = |key: &str| -> Result<String, String> {
            if synthetic {
                Ok(String::new())
            } else {
                Ok(json_string(&locators[key], "locator")
                    .unwrap_or_default()
                    .to_string())
            }
        };
        Ok(serde_json::json!({
            "schema": REQUEST_SCHEMA,
            "source_repository": required_json_string(report, "source_repository")?,
            "source_parent_sha": required_json_string(report, "source_parent_sha")?,
            "workflow_source_sha": required_json_string(report, "workflow_source_sha")?,
            "trusted_checker_identity": required_json_string(report, "trusted_checker_identity")?,
            "swarm_repository": required_json_string(report, "swarm_repository")?,
            "protected_w7_ref": required_json_string(report, "protected_w7_ref")?,
            "w7_peeled_sha": required_json_string(report, "w7_peeled_sha")?,
            "reviewed_tree_sha": required_json_string(report, "reviewed_tree_sha")?,
            "reviewed_tree_carrier_sha": required_json_string(report, "reviewed_tree_carrier_sha")?,
            "preflight_locator": requested_locator("preflight")?,
            "resolution_manifest_locator": requested_locator("resolution_manifest")?,
            "validation_packet_locator": requested_locator("validation_packet")?,
            "integration_packet_locator": requested_locator("integration_packet")?,
            "qualification_receipt_locator": requested_locator("qualification_receipt")?,
            "receipt_schema": required_json_string(report, "receipt_schema")?,
            "operation_mode": required_json_string(report, "operation_mode")?,
            "execution_profile": required_json_string(report, "execution_profile")?,
        }))
    }

    fn rewrite_test_packet_reports_and_index(root: &Path, report: &Value) -> Result<(), String> {
        let json = serde_json::to_string_pretty(report)
            .map_err(|error| format!("failed to serialize mutated test report: {error}"))?;
        fs::write(root.join(REPORT_JSON), format!("{json}\n"))
            .map_err(|error| format!("failed to write mutated test report: {error}"))?;
        fs::write(root.join(REPORT_MD), render_markdown(report)?)
            .map_err(|error| format!("failed to write mutated test markdown: {error}"))?;
        let files = collect_packet_files(root)?;
        let index = serde_json::json!({
            "schema": PACKET_SCHEMA,
            "status": json_string(report, "status"),
            "complete": true,
            "files": files,
        });
        let index = serde_json::to_string_pretty(&index)
            .map_err(|error| format!("failed to serialize mutated packet index: {error}"))?;
        fs::write(root.join(PACKET_INDEX), format!("{index}\n"))
            .map_err(|error| format!("failed to write mutated packet index: {error}"))
    }

    fn write_pretty_json(path: &Path, value: &Value, label: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(value)
            .map_err(|error| format!("failed to serialize {label}: {error}"))?;
        fs::write(path, format!("{json}\n"))
            .map_err(|error| format!("failed to write {label}: {error}"))
    }

    fn reindex_control_packet(root: &Path) -> Result<String, String> {
        let index_path = root.join(PACKET_INDEX);
        let index = read_json(&index_path, "test controller packet index")?;
        let kind = required_json_string(&index, "kind")?;
        let (report_name, markdown) = match kind {
            "resolved_tree_validation" => {
                let report = read_json(
                    &root.join("resolved-tree-validation.json"),
                    "test validation packet report",
                )?;
                (
                    "resolved-tree-validation.json",
                    super::super::source_promotion_validate_resolved_tree::render_markdown(
                        &report,
                    )?,
                )
            }
            "trusted_builder" | "resolved_tree_admission" | "exact_join_construction" => {
                let report_name = match kind {
                    "trusted_builder" => "trusted-builder.json",
                    "resolved_tree_admission" => "resolved-tree-admission.json",
                    "exact_join_construction" => "exact-join-construction.json",
                    _ => return Err(format!("unsupported test controller packet kind: {kind}")),
                };
                let report = read_json(&root.join(report_name), "test controller packet report")?;
                let markdown = if json_string(&report, "status") == Some("rejected") {
                    super::super::source_promotion_control::render_rejected_control_markdown(
                        report_name,
                        &report,
                    )?
                } else {
                    super::super::source_promotion_control::render_admitted_control_markdown(
                        report_name,
                        &report,
                    )?
                };
                (report_name, markdown)
            }
            _ => return Err(format!("unsupported test packet kind: {kind}")),
        };
        let markdown_name = report_name
            .strip_suffix(".json")
            .map(|stem| format!("{stem}.md"))
            .ok_or_else(|| format!("test report name must end in .json: {report_name}"))?;
        fs::write(root.join(markdown_name), markdown).map_err(|error| {
            format!("failed to rewrite canonical test packet Markdown: {error}")
        })?;
        reindex_packet_inventory(root)
    }

    fn reindex_packet_inventory(root: &Path) -> Result<String, String> {
        let index_path = root.join(PACKET_INDEX);
        let mut index = read_json(&index_path, "test packet index")?;
        let mut entries = Vec::new();
        for (path, sha256) in collect_packet_files(root)? {
            let bytes = fs::read(root.join(&path))
                .map_err(|error| format!("failed to read rebound controller member: {error}"))?;
            entries.push(serde_json::json!({
                "path": path,
                "bytes": bytes.len(),
                "sha256": sha256,
            }));
        }
        index["files"] = Value::Array(entries);
        write_pretty_json(&index_path, &index, "rebound controller packet index")?;
        digest_file(&index_path, "rebound controller packet index")
    }

    fn reindex_workflow_packet(root: &Path) -> Result<(), String> {
        let report = read_json(&root.join(REPORT_JSON), "rebound workflow report")?;
        let files = collect_packet_files(root)?;
        let index = serde_json::json!({
            "schema": PACKET_SCHEMA,
            "status": json_string(&report, "status"),
            "complete": true,
            "files": files,
        });
        write_pretty_json(
            &root.join(PACKET_INDEX),
            &index,
            "rebound workflow packet index",
        )
    }
}
