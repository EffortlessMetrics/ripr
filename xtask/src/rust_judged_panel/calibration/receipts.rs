//! Typed calibration receipts joined to judged cases by exact identity (#4795).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::super::parse_json_without_duplicate_keys;
use super::{
    BehaviorIdentity, CalibrationReceipt, JudgedCase, RERUN, RESULT_CAUGHT, RESULT_SURVIVED,
    SubjectCounts, digest_pref,
};

const RECEIPT_KIND: &str = "rust_judged_panel_calibration_receipt";
const RECEIPT_SCHEMA: &str = "0.1";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptFile {
    schema_version: String,
    kind: String,
    case_id: String,
    selection_sha256: String,
    judgments_sha256: String,
    repository: String,
    base: String,
    head: String,
    tree_identity: String,
    behavior_identity: BehaviorIdentityFile,
    expected_direction: String,
    terminal_structural_judgment: String,
    mutant: MutantFile,
    selector: SelectorFile,
    runner: RunnerFile,
    command: CommandFile,
    subjects: SubjectCounts,
    exit: ExitFile,
    output: OutputFile,
    currentness: String,
    runtime_result: String,
    limitations: Vec<String>,
    semantic_receipt_digest: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BehaviorIdentityFile {
    file: String,
    line: u64,
    owner: String,
    changed_behavior: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MutantFile {
    operator: String,
    source_range: String,
    identity: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectorFile {
    package: String,
    target: String,
    filter: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunnerFile {
    tool: String,
    version: String,
    binary_path: String,
    binary_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandFile {
    argv: Vec<String>,
    cwd: String,
    timeout_ms: u64,
    environment_policy: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExitFile {
    code: Option<i32>,
    timed_out: bool,
    compile_failed: bool,
    process_failed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputFile {
    stdout_sha256: String,
    stderr_sha256: String,
}

pub(super) fn load_receipts_at(
    root: &Path,
    records_dir: &Path,
) -> Result<BTreeMap<String, CalibrationReceipt>, String> {
    let dir = root.join(records_dir);
    if !dir.exists() {
        return Ok(BTreeMap::new());
    }
    let mut receipts = BTreeMap::new();
    let entries = fs::read_dir(&dir).map_err(|error| {
        format!(
            "read calibration receipts `{}`: {error}\nrerun: {RERUN}",
            dir.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "read calibration receipts `{}`: {error}\nrerun: {RERUN}",
                dir.display()
            )
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let receipt = load_receipt(&path)?;
        if receipts.insert(receipt.case_id.clone(), receipt).is_some() {
            return Err(format!(
                "calibration receipts re-declare case `{}`\nrerun: {RERUN}",
                path.display()
            ));
        }
    }
    Ok(receipts)
}

fn load_receipt(path: &Path) -> Result<CalibrationReceipt, String> {
    let display = path.display();
    let body = fs::read_to_string(path).map_err(|error| {
        format!("read calibration receipt `{display}`: {error}\nrerun: {RERUN}")
    })?;
    let value = parse_json_without_duplicate_keys(&body).map_err(|error| {
        format!("parse calibration receipt `{display}`: {error}\nrerun: {RERUN}")
    })?;
    let parsed: ReceiptFile = serde_json::from_value(value).map_err(|error| {
        format!("parse calibration receipt `{display}`: {error}\nrerun: {RERUN}")
    })?;
    if parsed.schema_version != RECEIPT_SCHEMA || parsed.kind != RECEIPT_KIND {
        return Err(format!(
            "calibration receipt `{display}` carries unknown identity (schema `{}`, kind `{}`)\nrerun: {RERUN}",
            parsed.schema_version, parsed.kind
        ));
    }
    if parsed.semantic_receipt_digest.trim().is_empty()
        || !parsed.semantic_receipt_digest.starts_with("sha256:")
    {
        return Err(format!(
            "calibration receipt `{display}` is missing a semantic receipt digest\nrerun: {RERUN}"
        ));
    }
    Ok(CalibrationReceipt {
        case_id: parsed.case_id,
        selection_sha256: parsed.selection_sha256,
        judgments_sha256: parsed.judgments_sha256,
        repository: parsed.repository,
        base: parsed.base,
        head: parsed.head,
        tree_identity: parsed.tree_identity,
        behavior: BehaviorIdentity {
            file: parsed.behavior_identity.file,
            line: parsed.behavior_identity.line,
            owner: parsed.behavior_identity.owner,
            changed_behavior: parsed.behavior_identity.changed_behavior,
        },
        expected_direction: parsed.expected_direction,
        terminal: parsed.terminal_structural_judgment,
        mutant_operator: parsed.mutant.operator,
        mutant_range: parsed.mutant.source_range,
        mutant_identity: parsed.mutant.identity,
        package: parsed.selector.package,
        target: parsed.selector.target,
        filter: parsed.selector.filter,
        runner_tool: parsed.runner.tool,
        runner_version: parsed.runner.version,
        runner_path: parsed.runner.binary_path,
        runner_sha256: parsed.runner.binary_sha256,
        argv: parsed.command.argv,
        cwd: parsed.command.cwd,
        timeout_ms: parsed.command.timeout_ms,
        environment_policy: parsed.command.environment_policy,
        subjects: parsed.subjects,
        exit_code: parsed.exit.code,
        timed_out: parsed.exit.timed_out,
        compile_failed: parsed.exit.compile_failed,
        process_failed: parsed.exit.process_failed,
        stdout_sha256: parsed.output.stdout_sha256,
        stderr_sha256: parsed.output.stderr_sha256,
        currentness: parsed.currentness,
        claimed_result: parsed.runtime_result,
        limitations: parsed.limitations,
        semantic_receipt_digest: parsed.semantic_receipt_digest,
    })
}

pub(super) fn identity_matches(
    case: &JudgedCase,
    receipt: &CalibrationReceipt,
    selection_sha256: &str,
    judgments_sha256: &str,
) -> bool {
    receipt.case_id == case.case_id
        && receipt.selection_sha256 == selection_sha256
        && receipt.judgments_sha256 == judgments_sha256
        && receipt.repository == case.repository
        && receipt.base == case.base
        && receipt.head == case.head
        && receipt.tree_identity == case.tree_identity
        && receipt.behavior == case.behavior
        && receipt.expected_direction == case.expected_direction
        && receipt.terminal == case.terminal
        && receipt.currentness == "current"
}

pub(super) fn selector_matches(case: &JudgedCase, receipt: &CalibrationReceipt) -> bool {
    match &case.required_selector {
        Some(required) => {
            receipt.package == required.package
                && receipt.target == required.target
                && receipt.filter == required.filter
        }
        None => true,
    }
}

pub(super) fn claimed_caught_or_survived_without_subjects(receipt: &CalibrationReceipt) -> bool {
    matches!(
        receipt.claimed_result.as_str(),
        RESULT_CAUGHT | RESULT_SURVIVED
    ) && (receipt.subjects.intended == 0
        || receipt.subjects.selected == 0
        || receipt.subjects.executed == 0)
}

pub(super) fn digest_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("hash `{}`: {error}\nrerun: {RERUN}", path.display()))?;
    Ok(digest_pref(&bytes))
}
