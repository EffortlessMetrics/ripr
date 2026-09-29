//! Fail-closed local-wheelhouse qualification for the PyPI channel (#4631).
//!
//! The generic cross-channel package-qualification DTO remains #4630. This
//! owner is the wheelhouse row/aggregate gate and the no-publish consumer
//! workflow that feeds it.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub(crate) const POLICY_PATH: &str = "policy/python-wheelhouse-qualification.toml";
pub(crate) const WORKFLOW_PATH: &str = ".github/workflows/python-wheelhouse-qualification.yml";
#[cfg(test)]
const WORKFLOW_TEXT: &str =
    include_str!("../../../../.github/workflows/python-wheelhouse-qualification.yml");
const POLICY_TEXT: &str = include_str!("../../../../policy/python-wheelhouse-qualification.toml");

const EXPECTED_CHANNEL: &str = "pypi-wheelhouse";
const EXPECTED_SCHEMA: &str = "1.0";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WheelhousePolicy {
    pub(crate) schema_version: String,
    pub(crate) policy_state: String,
    pub(crate) enforcement: String,
    pub(crate) owner: String,
    pub(crate) workflow: String,
    pub(crate) issues: Vec<u32>,
    pub(crate) channel: String,
    pub(crate) publication: String,
    pub(crate) secrets: String,
    pub(crate) receipt_transport: String,
    pub(crate) max_artifact_retention_days: u32,
    pub(crate) admitted_os: String,
    pub(crate) admitted_arch: String,
    pub(crate) required_clients: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WheelhouseRowStatus {
    Passed,
    Failed,
    NotRun,
    Unsupported,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WheelhouseRow {
    pub(crate) client: String,
    pub(crate) os: String,
    pub(crate) arch: String,
    pub(crate) status: WheelhouseRowStatus,
    #[serde(default)]
    pub(crate) wheel_filename: String,
    #[serde(default)]
    pub(crate) wheel_sha256: String,
    #[serde(default)]
    pub(crate) payload_sha256: String,
    #[serde(default)]
    pub(crate) subject_count: u64,
    pub(crate) cargo_reachable: bool,
    pub(crate) source_checkout_reachable: bool,
    pub(crate) ambient_ripr_reachable: bool,
    pub(crate) planted_wrong_path_detected: bool,
    pub(crate) network_disabled_after_staging: bool,
    #[serde(default)]
    pub(crate) limitations: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WheelhouseReceipt {
    pub(crate) schema_version: String,
    pub(crate) channel: String,
    pub(crate) source_commit: String,
    pub(crate) source_tree: String,
    pub(crate) publication_authority: bool,
    pub(crate) artifact_retention_days: u32,
    pub(crate) rows: Vec<WheelhouseRow>,
    #[serde(default)]
    pub(crate) limitations: Vec<String>,
    #[serde(default)]
    pub(crate) non_claims: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct WheelhouseGate {
    pub(crate) passed: bool,
    pub(crate) reasons: Vec<String>,
}

pub(crate) fn parse_policy(path: &str, text: &str) -> Result<WheelhousePolicy, String> {
    toml::from_str(text).map_err(|err| format!("{path}: invalid wheelhouse policy: {err}"))
}

pub(crate) fn load_policy() -> Result<WheelhousePolicy, String> {
    parse_policy(POLICY_PATH, POLICY_TEXT)
}

pub(crate) fn parse_receipt(path: &str, text: &str) -> Result<WheelhouseReceipt, String> {
    serde_json::from_str(text).map_err(|err| format!("{path}: invalid wheelhouse receipt: {err}"))
}

pub(crate) fn evaluate_wheelhouse(
    policy: &WheelhousePolicy,
    receipt: &WheelhouseReceipt,
) -> WheelhouseGate {
    let mut reasons = Vec::new();
    validate_policy(policy, &mut reasons);
    if receipt.schema_version != EXPECTED_SCHEMA {
        reasons.push(format!(
            "receipt schema_version must be {EXPECTED_SCHEMA}, got {}",
            receipt.schema_version
        ));
    }
    if receipt.channel != policy.channel || receipt.channel != EXPECTED_CHANNEL {
        reasons.push(format!(
            "receipt channel must be {}, got {}",
            policy.channel, receipt.channel
        ));
    }
    if !is_commit_sha(&receipt.source_commit) {
        reasons.push("source_commit must be a lowercase 40-character hex SHA".to_string());
    }
    if !is_commit_sha(&receipt.source_tree) {
        reasons.push("source_tree must be a lowercase 40-character hex SHA".to_string());
    }
    if receipt.publication_authority {
        reasons.push(
            "publication_authority must be false; this lane has no registry write".to_string(),
        );
    }
    if receipt.artifact_retention_days == 0
        || receipt.artifact_retention_days > policy.max_artifact_retention_days
    {
        reasons.push(format!(
            "artifact_retention_days must be 1..={}, got {}",
            policy.max_artifact_retention_days, receipt.artifact_retention_days
        ));
    }
    if receipt
        .non_claims
        .iter()
        .any(|claim| claim.trim().is_empty())
    {
        reasons.push("non_claims must not contain empty entries".to_string());
    }

    let required = required_row_ids(policy);
    if required.is_empty() {
        reasons.push("policy required_clients produced zero required rows".to_string());
    }

    let mut seen = BTreeMap::new();
    for (index, row) in receipt.rows.iter().enumerate() {
        let id = row_id(row);
        if seen.insert(id.clone(), index).is_some() {
            reasons.push(format!("duplicate row `{id}`"));
        }
        if (row.os != policy.admitted_os || row.arch != policy.admitted_arch)
            && row.status == WheelhouseRowStatus::Passed
        {
            reasons.push(format!(
                "row `{id}` is outside the admitted {os}/{arch} matrix and cannot pass",
                os = policy.admitted_os,
                arch = policy.admitted_arch
            ));
        }
        if !policy.required_clients.contains(&row.client)
            && row.status == WheelhouseRowStatus::Passed
        {
            reasons.push(format!(
                "row `{id}` uses a client outside the required set and cannot pass"
            ));
        }
        if row.status == WheelhouseRowStatus::Passed {
            reasons.extend(passed_row_violations(row, &id));
        }
    }

    for id in &required {
        match seen.get(id).copied() {
            None => reasons.push(format!(
                "required row `{id}` is missing; skipped or unobserved work cannot pass"
            )),
            Some(index) => {
                let row = &receipt.rows[index];
                if row.status != WheelhouseRowStatus::Passed {
                    reasons.push(format!(
                        "required row `{id}` is {:?}; failed, not_run, or unsupported rows cannot satisfy the aggregate",
                        row.status
                    ));
                }
            }
        }
    }

    reasons.extend(mismatched_identity_reasons(receipt, &required));
    WheelhouseGate {
        passed: reasons.is_empty(),
        reasons,
    }
}

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let receipts = parse_run_args(args)?;
    let policy = load_policy()?;
    let receipt = load_receipt_from_path(&receipts)?;
    let gate = evaluate_wheelhouse(&policy, &receipt);
    write_gate_reports(&receipts, &receipt, &gate)?;
    if gate.passed {
        Ok(())
    } else {
        Err(format!(
            "python wheelhouse qualification did not pass:\n- {}",
            gate.reasons.join("\n- ")
        ))
    }
}

fn parse_run_args(args: &[String]) -> Result<PathBuf, String> {
    match args {
        [flag, path] if flag == "--receipts" => {
            if path.starts_with('-') {
                return Err("qualify-python-wheelhouse --receipts requires a path".to_string());
            }
            Ok(PathBuf::from(path))
        }
        _ => {
            Err("usage: cargo xtask qualify-python-wheelhouse --receipts <file-or-dir>".to_string())
        }
    }
}

fn load_receipt_from_path(path: &Path) -> Result<WheelhouseReceipt, String> {
    if path.is_dir() {
        assemble_receipt_dir(path)
    } else {
        let text = fs::read_to_string(path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        parse_receipt(&path.display().to_string(), &text)
    }
}

fn assemble_receipt_dir(dir: &Path) -> Result<WheelhouseReceipt, String> {
    let identity_path = dir.join("candidate-identity.json");
    let identity_text = fs::read_to_string(&identity_path)
        .map_err(|err| format!("failed to read {}: {err}", identity_path.display()))?;
    let identity: CandidateIdentity = serde_json::from_str(&identity_text).map_err(|err| {
        format!(
            "{}: invalid candidate identity: {err}",
            identity_path.display()
        )
    })?;
    let rows_dir = dir.join("rows");
    let mut rows = Vec::new();
    if rows_dir.is_dir() {
        let mut files = fs::read_dir(&rows_dir)
            .map_err(|err| format!("failed to read {}: {err}", rows_dir.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to list {}: {err}", rows_dir.display()))?;
        files.sort_by_key(|entry| entry.file_name());
        for entry in files {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let text = fs::read_to_string(&path)
                .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
            rows.push(parse_row(&path.display().to_string(), &text)?);
        }
    }
    let policy = load_policy()?;
    Ok(WheelhouseReceipt {
        schema_version: EXPECTED_SCHEMA.to_string(),
        channel: policy.channel,
        source_commit: identity.source_commit,
        source_tree: identity.source_tree,
        publication_authority: false,
        artifact_retention_days: policy.max_artifact_retention_days,
        rows,
        limitations: identity.limitations,
        non_claims: identity.non_claims,
    })
}

fn parse_row(path: &str, text: &str) -> Result<WheelhouseRow, String> {
    serde_json::from_str(text).map_err(|err| format!("{path}: invalid wheelhouse row: {err}"))
}

fn write_gate_reports(
    receipts: &Path,
    receipt: &WheelhouseReceipt,
    gate: &WheelhouseGate,
) -> Result<(), String> {
    let report = GateReport {
        schema_version: EXPECTED_SCHEMA,
        channel: EXPECTED_CHANNEL,
        passed: gate.passed,
        reasons: &gate.reasons,
        receipt,
    };
    let json = serde_json::to_string_pretty(&report)
        .map_err(|err| format!("failed to render wheelhouse gate JSON: {err}"))?;
    let markdown = render_gate_markdown(&report);
    let reports = Path::new("target/ripr/reports");
    fs::create_dir_all(reports)
        .map_err(|err| format!("failed to create {}: {err}", reports.display()))?;
    let report_json = reports.join("python-wheelhouse-qualification.json");
    let report_md = reports.join("python-wheelhouse-qualification.md");
    fs::write(&report_json, format!("{json}\n"))
        .map_err(|err| format!("failed to write {}: {err}", report_json.display()))?;
    fs::write(&report_md, markdown)
        .map_err(|err| format!("failed to write {}: {err}", report_md.display()))?;
    if receipts.is_dir() {
        let copy_json = receipts.join("wheelhouse-qualification.json");
        fs::write(&copy_json, format!("{json}\n"))
            .map_err(|err| format!("failed to write {}: {err}", copy_json.display()))?;
    }
    Ok(())
}

fn render_gate_markdown(report: &GateReport<'_>) -> String {
    let mut lines = vec![
        "# Python wheelhouse qualification".to_string(),
        String::new(),
        format!("Status: {}", if report.passed { "pass" } else { "failed" }),
        format!("Channel: {}", report.channel),
        format!("Source commit: {}", report.receipt.source_commit),
        format!("Source tree: {}", report.receipt.source_tree),
        format!(
            "Publication authority: {}",
            report.receipt.publication_authority
        ),
        format!(
            "Artifact retention days: {}",
            report.receipt.artifact_retention_days
        ),
        String::new(),
        "## Rows".to_string(),
        String::new(),
    ];
    if report.receipt.rows.is_empty() {
        lines.push("- none".to_string());
    } else {
        for row in &report.receipt.rows {
            lines.push(format!(
                "- `{}`: {:?} subjects={} wheel={} payload={}",
                row_id(row),
                row.status,
                row.subject_count,
                nonempty_or_dash(&row.wheel_sha256),
                nonempty_or_dash(&row.payload_sha256)
            ));
        }
    }
    if !report.reasons.is_empty() {
        lines.push(String::new());
        lines.push("## Reasons".to_string());
        lines.push(String::new());
        for reason in report.reasons {
            lines.push(format!("- {reason}"));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn nonempty_or_dash(value: &str) -> &str {
    if value.is_empty() { "-" } else { value }
}

fn validate_policy(policy: &WheelhousePolicy, reasons: &mut Vec<String>) {
    if policy.schema_version != EXPECTED_SCHEMA {
        reasons.push(format!(
            "policy schema_version must be {EXPECTED_SCHEMA}, got {}",
            policy.schema_version
        ));
    }
    if policy.channel != EXPECTED_CHANNEL {
        reasons.push(format!(
            "policy channel must be {EXPECTED_CHANNEL}, got {}",
            policy.channel
        ));
    }
    if policy.publication != "forbidden" || policy.secrets != "forbidden" {
        reasons.push("policy must forbid publication and secrets".to_string());
    }
    if policy.enforcement != "workflow_dispatch_only" {
        reasons.push("policy enforcement must be workflow_dispatch_only".to_string());
    }
    if policy.workflow != WORKFLOW_PATH {
        reasons.push(format!(
            "policy workflow must be {WORKFLOW_PATH}, got {}",
            policy.workflow
        ));
    }
    if policy.max_artifact_retention_days == 0 || policy.max_artifact_retention_days > 7 {
        reasons.push("policy max_artifact_retention_days must be 1..=7".to_string());
    }
    if policy.required_clients.is_empty() {
        reasons.push("policy required_clients must not be empty".to_string());
    }
}

fn required_row_ids(policy: &WheelhousePolicy) -> BTreeSet<String> {
    policy
        .required_clients
        .iter()
        .map(|client| format!("{client}:{}:{}", policy.admitted_os, policy.admitted_arch))
        .collect()
}

fn row_id(row: &WheelhouseRow) -> String {
    format!("{}:{}:{}", row.client, row.os, row.arch)
}

fn passed_row_violations(row: &WheelhouseRow, id: &str) -> Vec<String> {
    let mut reasons = Vec::new();
    if row.wheel_filename.trim().is_empty() {
        reasons.push(format!("passed row `{id}` is missing wheel_filename"));
    }
    if !is_content_digest(&row.wheel_sha256) {
        reasons.push(format!(
            "passed row `{id}` must record a nonempty lowercase 64-hex wheel digest"
        ));
    }
    if !is_content_digest(&row.payload_sha256) {
        reasons.push(format!(
            "passed row `{id}` must record a nonempty lowercase 64-hex payload digest"
        ));
    }
    if row.wheel_sha256 == row.payload_sha256 && is_content_digest(&row.wheel_sha256) {
        reasons.push(format!(
            "passed row `{id}` wheel and payload digests must not be identical; container hash is not installed-payload identity"
        ));
    }
    if row.subject_count == 0 {
        reasons.push(format!(
            "passed row `{id}` has zero subjects; zero-subject work cannot pass"
        ));
    }
    if row.cargo_reachable {
        reasons.push(format!("passed row `{id}` still has Cargo reachable"));
    }
    if row.source_checkout_reachable {
        reasons.push(format!(
            "passed row `{id}` still has a source checkout reachable"
        ));
    }
    if row.ambient_ripr_reachable {
        reasons.push(format!(
            "passed row `{id}` still has an ambient ripr reachable"
        ));
    }
    if !row.planted_wrong_path_detected {
        reasons.push(format!(
            "passed row `{id}` did not detect the planted wrong-PATH binary"
        ));
    }
    if !row.network_disabled_after_staging {
        reasons.push(format!(
            "passed row `{id}` did not disable network after staging"
        ));
    }
    reasons
}

fn mismatched_identity_reasons(
    receipt: &WheelhouseReceipt,
    required: &BTreeSet<String>,
) -> Vec<String> {
    let mut reasons = Vec::new();
    let mut wheel_names = BTreeSet::new();
    let mut wheel_hashes = BTreeSet::new();
    let mut payload_hashes = BTreeSet::new();
    for row in &receipt.rows {
        let id = row_id(row);
        if !required.contains(&id) || row.status != WheelhouseRowStatus::Passed {
            continue;
        }
        if !row.wheel_filename.trim().is_empty() {
            wheel_names.insert(row.wheel_filename.as_str());
        }
        if is_content_digest(&row.wheel_sha256) {
            wheel_hashes.insert(row.wheel_sha256.as_str());
        }
        if is_content_digest(&row.payload_sha256) {
            payload_hashes.insert(row.payload_sha256.as_str());
        }
    }
    if wheel_names.len() > 1 {
        reasons.push(
            "required passed rows record mismatched wheel filenames for the same wheelhouse"
                .to_string(),
        );
    }
    if wheel_hashes.len() > 1 {
        reasons.push(
            "required passed rows record mismatched wheel digests for the same wheelhouse"
                .to_string(),
        );
    }
    if payload_hashes.len() > 1 {
        reasons.push(
            "required passed rows record mismatched payload digests for the same installed binary"
                .to_string(),
        );
    }
    reasons
}

fn is_commit_sha(value: &str) -> bool {
    value.len() == 40 && is_lowercase_hex(value)
}

fn is_content_digest(value: &str) -> bool {
    value.len() == 64 && is_lowercase_hex(value) && value.bytes().any(|byte| byte != b'0')
}

fn is_lowercase_hex(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateIdentity {
    source_commit: String,
    source_tree: String,
    #[serde(default)]
    limitations: Vec<String>,
    #[serde(default)]
    non_claims: Vec<String>,
}

#[derive(Serialize)]
struct GateReport<'a> {
    schema_version: &'static str,
    channel: &'static str,
    passed: bool,
    reasons: &'a [String],
    receipt: &'a WheelhouseReceipt,
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const TREE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const WHEEL_SHA: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const PAYLOAD_SHA: &str = "2222222222222222222222222222222222222222222222222222222222222222";

    fn policy() -> Result<WheelhousePolicy, String> {
        parse_policy(POLICY_PATH, POLICY_TEXT)
    }

    fn passed_row(client: &str) -> WheelhouseRow {
        WheelhouseRow {
            client: client.to_string(),
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
            status: WheelhouseRowStatus::Passed,
            wheel_filename: "ripr_rs-0.11.0-py3-none-linux_x86_64.whl".to_string(),
            wheel_sha256: WHEEL_SHA.to_string(),
            payload_sha256: PAYLOAD_SHA.to_string(),
            subject_count: 3,
            cargo_reachable: false,
            source_checkout_reachable: false,
            ambient_ripr_reachable: false,
            planted_wrong_path_detected: true,
            network_disabled_after_staging: true,
            limitations: Vec::new(),
        }
    }

    fn valid_receipt() -> WheelhouseReceipt {
        WheelhouseReceipt {
            schema_version: EXPECTED_SCHEMA.to_string(),
            channel: EXPECTED_CHANNEL.to_string(),
            source_commit: COMMIT.to_string(),
            source_tree: TREE.to_string(),
            publication_authority: false,
            artifact_retention_days: 5,
            rows: vec![passed_row("pip"), passed_row("uv")],
            limitations: Vec::new(),
            non_claims: vec![
                "no PyPI publication".to_string(),
                "no compatibility-floor claim".to_string(),
            ],
        }
    }

    fn reasons(receipt: WheelhouseReceipt) -> Result<Vec<String>, String> {
        Ok(evaluate_wheelhouse(&policy()?, &receipt).reasons)
    }

    fn must_reject(receipt: WheelhouseReceipt, needle: &str) -> Result<(), String> {
        let reasons = reasons(receipt)?;
        if reasons.iter().any(|reason| reason.contains(needle)) {
            Ok(())
        } else {
            Err(format!(
                "expected a reason containing {needle:?}, got {reasons:#?}"
            ))
        }
    }

    #[test]
    fn committed_policy_matches_the_selected_linux_x64_matrix() -> Result<(), String> {
        let policy = policy()?;
        assert_eq!(policy.channel, EXPECTED_CHANNEL);
        assert_eq!(policy.admitted_os, "linux");
        assert_eq!(policy.admitted_arch, "x86_64");
        assert_eq!(policy.required_clients, ["pip", "uv"]);
        assert_eq!(policy.max_artifact_retention_days, 5);
        assert_eq!(policy.publication, "forbidden");
        assert_eq!(policy.secrets, "forbidden");
        assert_eq!(policy.workflow, WORKFLOW_PATH);
        assert!(policy.issues.contains(&4631));
        Ok(())
    }

    #[test]
    fn matching_pip_and_uv_rows_pass() -> Result<(), String> {
        let gate = evaluate_wheelhouse(&policy()?, &valid_receipt());
        if !gate.passed {
            return Err(format!("expected pass, got {:#?}", gate.reasons));
        }
        if !gate.reasons.is_empty() {
            return Err(format!("expected no reasons, got {:#?}", gate.reasons));
        }
        Ok(())
    }

    #[test]
    fn missing_required_uv_row_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows.retain(|row| row.client != "uv");
        must_reject(receipt, "required row `uv:linux:x86_64` is missing")
    }

    #[test]
    fn not_run_required_row_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows[1].status = WheelhouseRowStatus::NotRun;
        receipt.rows[1].subject_count = 0;
        must_reject(receipt, "`uv:linux:x86_64` is NotRun")
    }

    #[test]
    fn failed_required_row_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows[0].status = WheelhouseRowStatus::Failed;
        must_reject(receipt, "`pip:linux:x86_64` is Failed")
    }

    #[test]
    fn unsupported_required_row_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows[0].status = WheelhouseRowStatus::Unsupported;
        must_reject(receipt, "`pip:linux:x86_64` is Unsupported")
    }

    #[test]
    fn zero_subject_passed_row_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows[0].subject_count = 0;
        must_reject(receipt, "zero subjects")
    }

    #[test]
    fn missing_wheel_digest_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows[0].wheel_sha256.clear();
        must_reject(receipt, "wheel digest")
    }

    #[test]
    fn placeholder_zero_payload_digest_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows[0].payload_sha256 = "0".repeat(64);
        must_reject(receipt, "payload digest")
    }

    #[test]
    fn identical_wheel_and_payload_digests_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows[0].payload_sha256 = WHEEL_SHA.to_string();
        receipt.rows[1].payload_sha256 = WHEEL_SHA.to_string();
        must_reject(receipt, "must not be identical")
    }

    #[test]
    fn cargo_escape_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows[0].cargo_reachable = true;
        must_reject(receipt, "Cargo reachable")
    }

    #[test]
    fn source_checkout_escape_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows[0].source_checkout_reachable = true;
        must_reject(receipt, "source checkout")
    }

    #[test]
    fn ambient_ripr_escape_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows[0].ambient_ripr_reachable = true;
        must_reject(receipt, "ambient ripr")
    }

    #[test]
    fn undetected_planted_path_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows[0].planted_wrong_path_detected = false;
        must_reject(receipt, "planted wrong-PATH")
    }

    #[test]
    fn network_left_enabled_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows[0].network_disabled_after_staging = false;
        must_reject(receipt, "disable network after staging")
    }

    #[test]
    fn mismatched_wheel_digests_across_clients_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows[1].wheel_sha256 =
            "3333333333333333333333333333333333333333333333333333333333333333".to_string();
        must_reject(receipt, "mismatched wheel digests")
    }

    #[test]
    fn mixed_wheel_filenames_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows[1].wheel_filename = "other-client.whl".to_string();
        must_reject(receipt, "mismatched wheel filenames")
    }

    #[test]
    fn mixed_filenames_on_failed_rows_do_not_trip_the_identity_gate() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows[1].status = WheelhouseRowStatus::Failed;
        receipt.rows[1].wheel_filename = "other-client.whl".to_string();
        let reasons = reasons(receipt)?;
        if reasons
            .iter()
            .any(|reason| reason.contains("mismatched wheel filenames"))
        {
            return Err(
                "failed-row filename mismatch must not be treated as identity drift".to_string(),
            );
        }
        if reasons
            .iter()
            .any(|reason| reason.contains("required row `uv:linux:x86_64`"))
        {
            Ok(())
        } else {
            Err(format!("expected a required-row failure, got {reasons:#?}"))
        }
    }

    #[test]
    fn publication_authority_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.publication_authority = true;
        must_reject(receipt, "publication_authority")
    }

    #[test]
    fn unbounded_retention_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.artifact_retention_days = 90;
        must_reject(receipt, "artifact_retention_days")
    }

    #[test]
    fn macos_passed_row_cannot_widen_the_matrix() -> Result<(), String> {
        let mut receipt = valid_receipt();
        let mut extra = passed_row("pip");
        extra.os = "macos".to_string();
        extra.arch = "arm64".to_string();
        receipt.rows.push(extra);
        must_reject(receipt, "outside the admitted linux/x86_64 matrix")
    }

    #[test]
    fn unknown_receipt_fields_are_rejected() -> Result<(), String> {
        match parse_receipt(
            "fixture.json",
            r#"{"schema_version":"1.0","channel":"pypi-wheelhouse","source_commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","source_tree":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","publication_authority":false,"artifact_retention_days":5,"rows":[],"pypi_token":"secret"}"#,
        ) {
            Ok(_) => Err("unknown credential field must fail closed".to_string()),
            Err(err) if err.contains("unknown field") => Ok(()),
            Err(err) => Err(format!("expected unknown field error, got {err}")),
        }
    }

    #[test]
    fn uppercase_commit_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.source_commit = COMMIT.to_ascii_uppercase();
        must_reject(receipt, "source_commit")
    }

    #[test]
    fn round_trip_json_preserves_a_valid_receipt() -> Result<(), String> {
        let receipt = valid_receipt();
        let encoded = serde_json::to_string(&receipt).map_err(|err| err.to_string())?;
        let decoded = parse_receipt("round-trip.json", &encoded)?;
        assert_eq!(decoded, receipt);
        Ok(())
    }

    #[test]
    fn missing_receipts_argument_is_refused() -> Result<(), String> {
        match parse_run_args(&[]) {
            Ok(_) => Err("usage must be required".to_string()),
            Err(err) if err.contains("--receipts") => Ok(()),
            Err(err) => Err(format!("expected --receipts usage, got {err}")),
        }
    }

    #[test]
    fn dash_prefixed_receipts_path_is_refused() -> Result<(), String> {
        match parse_run_args(&["--receipts".to_string(), "-evil".to_string()]) {
            Ok(_) => Err("dash-prefixed receipts path must be refused".to_string()),
            Err(err) if err.contains("--receipts") => Ok(()),
            Err(err) => Err(format!("expected --receipts usage, got {err}")),
        }
    }

    #[test]
    fn duplicate_required_row_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows.push(passed_row("pip"));
        must_reject(receipt, "duplicate row `pip:linux:x86_64`")
    }

    #[test]
    fn extra_client_passed_row_cannot_widen_the_matrix() -> Result<(), String> {
        let mut receipt = valid_receipt();
        let mut extra = passed_row("poetry");
        extra.payload_sha256 =
            "4444444444444444444444444444444444444444444444444444444444444444".to_string();
        receipt.rows.push(extra);
        must_reject(receipt, "outside the required set")
    }

    #[test]
    fn empty_wheel_filename_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows[0].wheel_filename.clear();
        must_reject(receipt, "missing wheel_filename")
    }

    #[test]
    fn zero_retention_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.artifact_retention_days = 0;
        must_reject(receipt, "artifact_retention_days")
    }

    #[test]
    fn empty_non_claim_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.non_claims.push("   ".to_string());
        must_reject(receipt, "non_claims")
    }

    #[test]
    fn uppercase_wheel_digest_cannot_pass() -> Result<(), String> {
        let mut receipt = valid_receipt();
        receipt.rows[0].wheel_sha256 =
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_ascii_uppercase();
        must_reject(receipt, "wheel digest")
    }

    #[test]
    fn unknown_row_fields_are_rejected() -> Result<(), String> {
        match parse_row(
            "row.json",
            r#"{"client":"pip","os":"linux","arch":"x86_64","status":"failed","cargo_reachable":false,"source_checkout_reachable":false,"ambient_ripr_reachable":false,"planted_wrong_path_detected":false,"network_disabled_after_staging":false,"pypi_token":"secret"}"#,
        ) {
            Ok(_) => Err("unknown credential field on a row must fail closed".to_string()),
            Err(err) if err.contains("unknown field") => Ok(()),
            Err(err) => Err(format!("expected unknown field error, got {err}")),
        }
    }

    #[test]
    fn publication_allowed_policy_cannot_pass() -> Result<(), String> {
        let mut policy = policy()?;
        policy.publication = "allowed".to_string();
        let gate = evaluate_wheelhouse(&policy, &valid_receipt());
        if gate
            .reasons
            .iter()
            .any(|reason| reason.contains("forbid publication"))
        {
            Ok(())
        } else {
            Err(format!(
                "expected publication forbidden reason, got {:#?}",
                gate.reasons
            ))
        }
    }

    fn write_identity(root: &Path) -> Result<(), String> {
        fs::create_dir_all(root.join("rows")).map_err(|err| err.to_string())?;
        let identity = serde_json::json!({
            "source_commit": COMMIT,
            "source_tree": TREE,
            "limitations": ["fixture"],
            "non_claims": ["no PyPI publication"]
        });
        fs::write(
            root.join("candidate-identity.json"),
            format!("{identity}\n"),
        )
        .map_err(|err| err.to_string())
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("ripr-wheelhouse-{}-{label}", std::process::id()))
    }

    #[test]
    fn assemble_dir_with_no_row_files_cannot_pass() -> Result<(), String> {
        let root = temp_root("missing-rows");
        write_identity(&root)?;
        let receipt = assemble_receipt_dir(&root)?;
        let gate = evaluate_wheelhouse(&policy()?, &receipt);
        let _ = fs::remove_dir_all(&root);
        if gate.passed {
            return Err("missing rows must not pass".to_string());
        }
        if gate
            .reasons
            .iter()
            .any(|reason| reason.contains("required row `pip:linux:x86_64` is missing"))
        {
            Ok(())
        } else {
            Err(format!(
                "missing pip row was not reported: {:#?}",
                gate.reasons
            ))
        }
    }

    #[test]
    fn assemble_dir_with_zero_subject_failed_rows_cannot_pass() -> Result<(), String> {
        let root = temp_root("zero-subject-rows");
        write_identity(&root)?;
        for client in ["pip", "uv"] {
            let mut row = passed_row(client);
            row.status = WheelhouseRowStatus::Failed;
            row.subject_count = 0;
            fs::write(
                root.join("rows").join(format!("{client}.json")),
                format!(
                    "{}\n",
                    serde_json::to_string(&row).map_err(|err| err.to_string())?
                ),
            )
            .map_err(|err| err.to_string())?;
        }
        let receipt = assemble_receipt_dir(&root)?;
        let gate = evaluate_wheelhouse(&policy()?, &receipt);
        let _ = fs::remove_dir_all(&root);
        if gate.passed {
            return Err("zero-subject failed rows must not pass".to_string());
        }
        if gate
            .reasons
            .iter()
            .any(|reason| reason.contains("`pip:linux:x86_64` is Failed"))
        {
            Ok(())
        } else {
            Err(format!(
                "failed pip row was not reported: {:#?}",
                gate.reasons
            ))
        }
    }

    #[test]
    fn assemble_dir_with_matching_passed_rows_passes() -> Result<(), String> {
        let root = temp_root("passed-rows");
        write_identity(&root)?;
        for client in ["pip", "uv"] {
            let row = passed_row(client);
            fs::write(
                root.join("rows").join(format!("{client}.json")),
                format!(
                    "{}\n",
                    serde_json::to_string(&row).map_err(|err| err.to_string())?
                ),
            )
            .map_err(|err| err.to_string())?;
        }
        let receipt = assemble_receipt_dir(&root)?;
        let gate = evaluate_wheelhouse(&policy()?, &receipt);
        let _ = fs::remove_dir_all(&root);
        if gate.passed {
            Ok(())
        } else {
            Err(format!("expected assembled pass, got {:#?}", gate.reasons))
        }
    }

    fn workflow_lines() -> Vec<&'static str> {
        WORKFLOW_TEXT
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect()
    }

    #[test]
    fn workflow_is_read_only_dispatch_only_and_non_publishing() {
        let lines = workflow_lines();
        assert!(lines.contains(&"workflow_dispatch:"));
        assert!(!lines.iter().any(|line| line.starts_with("pull_request:")));
        assert!(!lines.iter().any(|line| line.starts_with("push:")));
        assert_eq!(
            lines.iter().find(|line| line.starts_with("contents:")),
            Some(&"contents: read")
        );
        assert!(!WORKFLOW_TEXT.contains("id-token"));
        assert!(!WORKFLOW_TEXT.contains("secrets."));
        assert!(!WORKFLOW_TEXT.contains("twine"));
        assert!(!WORKFLOW_TEXT.contains("pypi.org/legacy"));
        assert!(!WORKFLOW_TEXT.contains("gh release"));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("persist-credentials: false"))
        );
        assert!(lines.contains(&"if-no-files-found: error"));
        assert!(lines.contains(&"retention-days: 5"));
        assert!(
            WORKFLOW_TEXT
                .contains("actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a")
        );
        assert!(
            WORKFLOW_TEXT.contains("actions/checkout@d23441a48e516b6c34aea4fa41551a30e30af803")
        );
    }

    #[test]
    fn workflow_installs_from_a_local_wheelhouse_with_isolation_controls() {
        let text = WORKFLOW_TEXT;
        assert!(text.contains("install --no-index --find-links"));
        assert!(text.contains("uv tool install --offline"));
        assert!(text.contains("PIP_NO_INDEX=1"));
        assert!(text.contains("UV_OFFLINE=1"));
        assert!(text.contains("planted-wrong-path"));
        assert!(text.contains("command -v cargo"));
        assert!(text.contains("command -v rustc"));
        assert!(text.contains("command -v ripr"));
        assert!(text.contains(r#"= "${DECOY_ROOT}/ripr""#));
        assert!(text.contains("/usr/bin/ripr"));
        assert!(text.contains("cargo xtask qualify-python-wheelhouse --receipts"));
        assert!(text.contains("urllib.request.urlopen('https://pypi.org'"));
        assert!(text.contains(r#"cd "${CONSUMER_ROOT}""#));
        assert!(text.contains(r#"test -r "${GITHUB_WORKSPACE}/Cargo.toml""#));
        assert!(text.contains("${#wheels[@]} -ne 1"));
        assert!(text.contains(r#""${wheels[0]}""#));
        assert!(text.contains("pathlib.Path(sys.argv[8])"));
        assert!(!text.contains("cowsay"));
        assert!(!text.contains(".glob(\"*.whl\")"));
        assert!(text.contains(r#""status": "failed""#));
        assert!(text.contains(r#""subject_count": 0"#));
        assert!(text.contains("flag(3)"));
        assert!(text.contains("flag(7)"));
    }

    #[test]
    fn workflow_run_blocks_stay_within_the_visible_budget() {
        let mut max_non_empty = 0usize;
        let lines: Vec<&str> = WORKFLOW_TEXT.lines().collect();
        let mut idx = 0usize;
        while idx < lines.len() {
            let line = lines[idx];
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("run:") {
                let indent = line.len() - trimmed.len();
                let run_value = rest.trim();
                if matches!(run_value, "|" | ">" | "|-" | ">-") {
                    let mut next_idx = idx + 1;
                    let mut non_empty = 0usize;
                    while next_idx < lines.len() {
                        let next = lines[next_idx];
                        let next_trimmed = next.trim_start();
                        let next_indent = next.len() - next_trimmed.len();
                        if !next_trimmed.is_empty() && next_indent <= indent {
                            break;
                        }
                        if !next_trimmed.is_empty() {
                            non_empty += 1;
                        }
                        next_idx += 1;
                    }
                    max_non_empty = max_non_empty.max(non_empty);
                    idx = next_idx;
                    continue;
                }
            }
            idx += 1;
        }
        assert!(
            max_non_empty <= 50,
            "largest run block has {max_non_empty} non-empty lines"
        );
    }
}
