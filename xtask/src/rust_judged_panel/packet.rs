use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use ra_ap_syntax::{AstNode, Edition, SourceFile, TextSize, ast, ast::HasName};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::host_run::{self, ValidatedHostCase, ValidatedHostRun};
use super::subject::{self, PacketSubject, ReplaySubjectFile};
use super::{MANIFEST_PATH, RustJudgedPanelManifest};

const SUBJECTS_PATH: &str = "metrics/rust-judged-behavior-panel/subjects.json";
const PORTABLE_ROOT: &str = "metrics/rust-judged-behavior-panel/portable";
const CURRENT_PATH: &str = "metrics/rust-judged-behavior-panel/portable/current.json";
pub(super) const DEFAULT_HOST_CURRENT: &str = "target/ripr/rust-judged-panel/current.json";
const STAGING_ROOT: &str = "target/ripr/rust-judged-panel-packet";
static PACKET_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PortableCurrent {
    schema_version: String,
    kind: String,
    generation_id: String,
    index_path: String,
    index_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PortableIndex {
    schema_version: String,
    kind: String,
    publication_state: String,
    generation_id: String,
    manifest_sha256: String,
    subjects_sha256: String,
    packets: Vec<PortableIndexEntry>,
    non_claims: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PortableIndexEntry {
    case_id: String,
    packet_path: String,
    packet_sha256: String,
    semantic_sha256: String,
    attestation_path: String,
    attestation_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RetainedAttestation {
    schema_version: String,
    kind: String,
    case_id: String,
    semantic: PortableSemantic,
    host_evidence: HostEvidence,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PortablePacket {
    schema_version: String,
    kind: String,
    case_id: String,
    semantic: PortableSemantic,
    semantic_sha256: String,
    host_evidence: HostEvidence,
    judgment: NullJudgment,
    runtime_calibration: RuntimeNotRun,
    non_claims: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PortableSemantic {
    manifest_sha256: String,
    subjects_sha256: String,
    subject_id: String,
    repository: String,
    expected_direction: String,
    repository_base: String,
    repository_head: String,
    repository_tree: String,
    producer_source_head: String,
    producer_source_tree: String,
    producer_cargo_toml_sha256: String,
    producer_cargo_lock_sha256: String,
    producer_version: String,
    profile: String,
    features: Vec<String>,
    argv: Vec<String>,
    mode: String,
    format: String,
    config_path: String,
    config_sha256: String,
    diff_path: String,
    diff_sha256: String,
    executed_diff_identity: String,
    subject_inputs: Vec<SemanticInputDigest>,
    anchor: SemanticAnchor,
    observed: ObservedFinding,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SemanticInputDigest {
    role: String,
    source_path: String,
    repository_path: String,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SemanticAnchor {
    file: String,
    line: u64,
    owner: String,
    behavior_family: String,
    expression: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ObservedFinding {
    finding_id: String,
    analysis_complete: bool,
    outcome_kind: String,
    classification: String,
    probe_family: String,
    probe_file: String,
    probe_line: u64,
    probe_expression: String,
    expected_actionability: String,
    actionability_source: String,
    missing: Vec<String>,
    recommendation: String,
    static_limit_kind: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct HostEvidence {
    availability: String,
    host_target: String,
    binary_sha256: String,
    run_id: String,
    current_ref: String,
    current_sha256: String,
    index_ref: String,
    index_sha256: String,
    receipt_ref: String,
    receipt_sha256: String,
    stdout_ref: String,
    stdout_sha256: String,
    stderr_ref: String,
    stderr_sha256: String,
    analyzer_input_identity: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NullJudgment {
    disposition: String,
    structural_judgment: Option<bool>,
    false_actionable: Option<bool>,
    false_exposed: Option<bool>,
    static_under_credit: Option<bool>,
    wrong_target: Option<bool>,
    limitation_correct: Option<bool>,
    source: Option<String>,
    judged_at: Option<String>,
    judged_by: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeNotRun {
    status: String,
    outcome: Option<String>,
    evidence_ref: Option<String>,
}

struct PacketLock(PathBuf);

impl Drop for PacketLock {
    fn drop(&mut self) {
        let _result = fs::remove_file(&self.0);
    }
}

pub(super) fn publish(
    root: &Path,
    manifest: &RustJudgedPanelManifest,
    host_current: &str,
) -> Result<(), String> {
    let host = host_run::load_validated_current(root, host_current)?;
    let subjects = subject::load_for_packet(root, manifest)?;
    let manifest_sha256 = sha256_file(&root.join(MANIFEST_PATH))?;
    let subjects_sha256 = sha256_file(&root.join(SUBJECTS_PATH))?;
    let packets = project_all(
        manifest,
        &subjects,
        &host,
        &manifest_sha256,
        &subjects_sha256,
    )?;
    let attestations = attest_all(&subjects, &host, &manifest_sha256, &subjects_sha256)?;
    publish_all(
        root,
        &manifest_sha256,
        &subjects_sha256,
        &packets,
        &attestations,
        None,
    )?;
    validate_at(root, manifest)?;
    println!(
        "Rust judged-panel portable packet set published: cases={} current={CURRENT_PATH}",
        packets.len()
    );
    Ok(())
}

pub(super) fn validate_at(root: &Path, manifest: &RustJudgedPanelManifest) -> Result<(), String> {
    let subjects = subject::load_for_packet(root, manifest)?;
    let manifest_sha256 = sha256_file(&root.join(MANIFEST_PATH))?;
    let subjects_sha256 = sha256_file(&root.join(SUBJECTS_PATH))?;
    let current_path = root.join(CURRENT_PATH);
    require_canonical_portable(root, &current_path, "portable current")?;
    let current: PortableCurrent = read_strict_json(&current_path, "portable current")?;
    require_eq(&current.schema_version, "0.1", "portable current schema")?;
    require_eq(
        &current.kind,
        "rust_judged_panel_portable_current",
        "portable current kind",
    )?;
    safe_portable_path(&current.index_path)?;
    let index_path = root.join(&current.index_path);
    require_canonical_portable(root, &index_path, "portable index")?;
    require_eq(
        &sha256_file(&index_path)?,
        &current.index_sha256,
        "portable current index digest",
    )?;
    let index: PortableIndex = read_strict_json(&index_path, "portable index")?;
    validate_index_shape(
        &index,
        &current,
        &manifest_sha256,
        &subjects_sha256,
        manifest.items.len(),
    )?;
    let subject_by_id = subjects
        .iter()
        .map(|subject| (subject.case_id.as_str(), subject))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    for entry in &index.packets {
        if !seen.insert(entry.case_id.as_str()) {
            return Err(format!("portable index duplicates `{}`", entry.case_id));
        }
        validate_case_filename(&entry.case_id)?;
        let file_name = format!("{}.json", entry.case_id);
        let expected_packet = format!(
            "{PORTABLE_ROOT}/generations/{}/packets/{file_name}",
            current.generation_id
        );
        let expected_attestation = format!(
            "{PORTABLE_ROOT}/generations/{}/attestations/{file_name}",
            current.generation_id
        );
        require_eq(
            &entry.packet_path,
            &expected_packet,
            "portable current-generation packet path",
        )?;
        require_eq(
            &entry.attestation_path,
            &expected_attestation,
            "portable current-generation attestation path",
        )?;
        safe_portable_path(&entry.packet_path)?;
        let packet_path = root.join(&entry.packet_path);
        require_canonical_portable(root, &packet_path, "portable packet")?;
        require_eq(
            &sha256_file(&packet_path)?,
            &entry.packet_sha256,
            &format!("portable packet `{}` file digest", entry.case_id),
        )?;
        let packet: PortablePacket = read_strict_json(&packet_path, "portable packet")?;
        safe_portable_path(&entry.attestation_path)?;
        let attestation_path = root.join(&entry.attestation_path);
        require_canonical_portable(root, &attestation_path, "portable attestation")?;
        require_eq(
            &sha256_file(&attestation_path)?,
            &entry.attestation_sha256,
            &format!("portable attestation `{}` file digest", entry.case_id),
        )?;
        let attestation: RetainedAttestation =
            read_strict_json(&attestation_path, "portable attestation")?;
        let subject = subject_by_id
            .get(entry.case_id.as_str())
            .ok_or_else(|| format!("portable index references unknown case `{}`", entry.case_id))?;
        validate_retained_packet(
            &packet,
            entry,
            &attestation,
            subject,
            &manifest_sha256,
            &subjects_sha256,
        )?;
    }
    if seen != subject_by_id.keys().copied().collect::<BTreeSet<_>>() {
        return Err("portable index does not contain the exact selected case set".to_string());
    }
    Ok(())
}

fn project_all(
    manifest: &RustJudgedPanelManifest,
    subjects: &[PacketSubject],
    host: &ValidatedHostRun,
    manifest_sha256: &str,
    subjects_sha256: &str,
) -> Result<Vec<PortablePacket>, String> {
    if host.cases.len() != subjects.len() || subjects.len() != manifest.items.len() {
        return Err(
            "portable projection requires the exact complete selected case set".to_string(),
        );
    }
    let host_by_id = host
        .cases
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    let mut packets = Vec::new();
    for subject in subjects {
        let case = host_by_id
            .get(subject.case_id.as_str())
            .ok_or_else(|| format!("host run is missing `{}`", subject.case_id))?;
        packets.push(project_one(
            subject,
            case,
            host,
            manifest_sha256,
            subjects_sha256,
        )?);
    }
    packets.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    Ok(packets)
}

fn project_one(
    subject: &PacketSubject,
    case: &ValidatedHostCase,
    host: &ValidatedHostRun,
    manifest_sha256: &str,
    subjects_sha256: &str,
) -> Result<PortablePacket, String> {
    let semantic = project_semantic(subject, case, host, manifest_sha256, subjects_sha256)?;
    let semantic_sha256 = sha256_serialized(&semantic)?;
    let packet = PortablePacket {
        schema_version: "0.1".to_string(),
        kind: "rust_judged_panel_portable_packet".to_string(),
        case_id: subject.case_id.clone(),
        semantic,
        semantic_sha256,
        host_evidence: host_evidence(case, host),
        judgment: NullJudgment {
            disposition: "unjudged".to_string(),
            structural_judgment: None,
            false_actionable: None,
            false_exposed: None,
            static_under_credit: None,
            wrong_target: None,
            limitation_correct: None,
            source: None,
            judged_at: None,
            judged_by: Vec::new(),
        },
        runtime_calibration: RuntimeNotRun {
            status: "not_run".to_string(),
            outcome: None,
            evidence_ref: None,
        },
        non_claims: expected_non_claims(),
    };
    validate_retained_host(&packet, subject, host)?;
    Ok(packet)
}

fn project_semantic(
    subject: &PacketSubject,
    case: &ValidatedHostCase,
    host: &ValidatedHostRun,
    manifest_sha256: &str,
    subjects_sha256: &str,
) -> Result<PortableSemantic, String> {
    validate_host_join(subject, case, host)?;
    let report_text = std::str::from_utf8(&case.stdout)
        .map_err(|error| format!("host stdout for `{}` is not UTF-8: {error}", case.case_id))?;
    let report = super::parse_json_without_duplicate_keys(report_text)
        .map_err(|error| format!("parse host stdout for `{}`: {error}", case.case_id))?;
    let observed = project_observed(subject, case, host, &report)?;
    Ok(PortableSemantic {
        manifest_sha256: manifest_sha256.to_string(),
        subjects_sha256: subjects_sha256.to_string(),
        subject_id: subject.subject_id.clone(),
        repository: subject.repository.clone(),
        expected_direction: subject.expected_direction.clone(),
        repository_base: case.repository_base.clone(),
        repository_head: case.repository_head.clone(),
        repository_tree: case.repository_tree.clone(),
        producer_source_head: host.source_head.clone(),
        producer_source_tree: host.source_tree.clone(),
        producer_cargo_toml_sha256: host.cargo_toml_sha256.clone(),
        producer_cargo_lock_sha256: host.cargo_lock_sha256.clone(),
        producer_version: host.binary_version.clone(),
        profile: host.profile.clone(),
        features: host.features.clone(),
        argv: case.argv.clone(),
        mode: case.mode.clone(),
        format: case.format.clone(),
        config_path: case.config_path.clone(),
        config_sha256: case.config_sha256.clone(),
        diff_path: case.diff_path.clone(),
        diff_sha256: case.diff_sha256.clone(),
        executed_diff_identity: case.executed_diff_identity.clone(),
        subject_inputs: case
            .subject_inputs
            .iter()
            .map(|input| SemanticInputDigest {
                role: input.role.clone(),
                source_path: input.source_path.clone(),
                repository_path: input.repository_path.clone(),
                sha256: input.sha256.clone(),
            })
            .collect(),
        anchor: SemanticAnchor {
            file: subject.anchor_file.clone(),
            line: subject.anchor_line,
            owner: subject.owner.clone(),
            behavior_family: subject.behavior_family.clone(),
            expression: subject.changed_behavior.clone(),
        },
        observed,
    })
}

fn host_evidence(case: &ValidatedHostCase, host: &ValidatedHostRun) -> HostEvidence {
    HostEvidence {
        availability: "host_bound_not_committed".to_string(),
        host_target: host.host_target.clone(),
        binary_sha256: host.binary_sha256.clone(),
        run_id: host.run_id.clone(),
        current_ref: host.current_ref.clone(),
        current_sha256: host.current_sha256.clone(),
        index_ref: host.index_ref.clone(),
        index_sha256: host.index_sha256.clone(),
        receipt_ref: case.receipt_ref.clone(),
        receipt_sha256: case.receipt_sha256.clone(),
        stdout_ref: case.stdout_ref.clone(),
        stdout_sha256: case.stdout_sha256.clone(),
        stderr_ref: case.stderr_ref.clone(),
        stderr_sha256: case.stderr_sha256.clone(),
        analyzer_input_identity: case.analyzer_input_identity.clone(),
    }
}

fn retained_attestation(
    subject: &PacketSubject,
    case: &ValidatedHostCase,
    host: &ValidatedHostRun,
    manifest_sha256: &str,
    subjects_sha256: &str,
) -> Result<RetainedAttestation, String> {
    Ok(RetainedAttestation {
        schema_version: "0.1".to_string(),
        kind: "rust_judged_panel_retained_attestation".to_string(),
        case_id: subject.case_id.clone(),
        semantic: project_semantic(subject, case, host, manifest_sha256, subjects_sha256)?,
        host_evidence: host_evidence(case, host),
    })
}

fn attest_all(
    subjects: &[PacketSubject],
    host: &ValidatedHostRun,
    manifest_sha256: &str,
    subjects_sha256: &str,
) -> Result<Vec<RetainedAttestation>, String> {
    let cases = host
        .cases
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    subjects
        .iter()
        .map(|subject| {
            let case = cases
                .get(subject.case_id.as_str())
                .ok_or_else(|| format!("host run is missing `{}`", subject.case_id))?;
            retained_attestation(subject, case, host, manifest_sha256, subjects_sha256)
        })
        .collect()
}

fn validate_host_join(
    subject: &PacketSubject,
    case: &ValidatedHostCase,
    host: &ValidatedHostRun,
) -> Result<(), String> {
    for (label, actual, expected) in [
        (
            "subject id",
            case.subject_id.as_str(),
            subject.subject_id.as_str(),
        ),
        (
            "expected direction",
            case.expected_direction.as_str(),
            subject.expected_direction.as_str(),
        ),
        (
            "repository base",
            case.repository_base.as_str(),
            subject.expected_base.as_str(),
        ),
        (
            "repository head",
            case.repository_head.as_str(),
            subject.expected_head.as_str(),
        ),
        (
            "repository tree",
            case.repository_tree.as_str(),
            subject.expected_tree.as_str(),
        ),
        (
            "config path",
            case.config_path.as_str(),
            subject.config.repository_path.as_str(),
        ),
        (
            "config digest",
            case.config_sha256.as_str(),
            subject.config.sha256.as_str(),
        ),
        (
            "diff path",
            case.diff_path.as_str(),
            subject.diff.source_path.as_str(),
        ),
        (
            "diff digest",
            case.diff_sha256.as_str(),
            subject.diff.sha256.as_str(),
        ),
        (
            "analyzer input identity",
            case.analyzer_input_identity.as_str(),
            case.executed_diff_identity.as_str(),
        ),
    ] {
        require_eq(
            actual,
            expected,
            &format!("host `{}` {label}", subject.case_id),
        )?;
    }
    if host.profile != "dev"
        || host.features != ["default"]
        || case.mode != "draft"
        || case.format != "json"
    {
        return Err(format!(
            "host `{}` has an unexpected build or run mode",
            subject.case_id
        ));
    }
    let expected_argv = [
        "check",
        "--root",
        "<materialized-subject>",
        "--base",
        subject.expected_base.as_str(),
        "--mode",
        "draft",
        "--json",
    ];
    if case.argv.iter().map(String::as_str).collect::<Vec<_>>() != expected_argv {
        return Err(format!(
            "host `{}` argv is not the exact owned plan",
            subject.case_id
        ));
    }
    let expected_inputs = expected_inputs(subject);
    let actual_inputs = case
        .subject_inputs
        .iter()
        .map(|input| SemanticInputDigest {
            role: input.role.clone(),
            source_path: input.source_path.clone(),
            repository_path: input.repository_path.clone(),
            sha256: input.sha256.clone(),
        })
        .collect::<Vec<_>>();
    if actual_inputs != expected_inputs {
        return Err(format!(
            "host `{}` subject input authority is stale",
            subject.case_id
        ));
    }
    if case.disposition != "complete" {
        return Err(format!(
            "host `{}` disposition `{}` is not a complete analysis",
            subject.case_id, case.disposition
        ));
    }
    Ok(())
}

fn expected_inputs(subject: &PacketSubject) -> Vec<SemanticInputDigest> {
    let mut inputs = vec![
        semantic_input("cargo_toml", &subject.cargo_toml),
        semantic_input("cargo_lock", &subject.cargo_lock),
        semantic_input("config", &subject.config),
        semantic_input("source_before", &subject.source_before),
        semantic_input("source_after", &subject.source_after),
        semantic_input("diff", &subject.diff),
    ];
    inputs.extend(
        subject
            .tests
            .iter()
            .map(|file| semantic_input("test", file)),
    );
    inputs
}

fn semantic_input(role: &str, file: &ReplaySubjectFile) -> SemanticInputDigest {
    SemanticInputDigest {
        role: role.to_string(),
        source_path: file.source_path.clone(),
        repository_path: file.repository_path.clone(),
        sha256: file.sha256.clone(),
    }
}

fn project_observed(
    subject: &PacketSubject,
    case: &ValidatedHostCase,
    host: &ValidatedHostRun,
    report: &Value,
) -> Result<ObservedFinding, String> {
    if report
        .pointer("/analysis_outcome/analysis_complete")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err(format!("host `{}` analysis is incomplete", subject.case_id));
    }
    let outcome_kind = text_at(report, "/analysis_outcome/outcome/kind")?;
    require_eq(
        outcome_kind,
        "complete_with_findings",
        "analysis outcome kind",
    )?;
    let limitations = report
        .pointer("/analysis_outcome/outcome/limitations")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("host `{}` lacks typed outcome limitations", subject.case_id))?;
    if !limitations.is_empty() {
        return Err(format!(
            "host `{}` has an analysis-level limitation; finding-level static limits are distinct",
            subject.case_id
        ));
    }
    let findings = report
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("host `{}` lacks findings", subject.case_id))?;
    if findings.len() != 1 {
        return Err(format!(
            "host `{}` requires exactly one total finding, got {}",
            subject.case_id,
            findings.len()
        ));
    }
    let finding = findings
        .first()
        .ok_or_else(|| format!("host `{}` lacks its finding", subject.case_id))?;
    validate_probe(subject, case, host, report, finding)?;
    let classification = text_at(finding, "/classification")?;
    require_eq(
        classification,
        &subject.expected_classification,
        &format!("host `{}` classification", subject.case_id),
    )?;
    let missing = finding
        .get("missing")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("host `{}` finding lacks missing array", subject.case_id))?
        .iter()
        .map(|value| {
            value.as_str().map(ToString::to_string).ok_or_else(|| {
                format!("host `{}` has non-string missing evidence", subject.case_id)
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let recommendation = finding
        .pointer("/recommended_next_step")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("host `{}` lacks a string recommendation", subject.case_id))?
        .to_string();
    let static_limit_kind = finding
        .get("static_limit_kind")
        .filter(|value| !value.is_null())
        .map(|value| {
            value.as_str().map(ToString::to_string).ok_or_else(|| {
                format!(
                    "host `{}` has a non-string static limit kind",
                    subject.case_id
                )
            })
        })
        .transpose()?;
    validate_direction_witness(
        subject,
        &missing,
        &recommendation,
        static_limit_kind.as_deref(),
    )?;
    Ok(ObservedFinding {
        finding_id: text_at(finding, "/id")?.to_string(),
        analysis_complete: true,
        outcome_kind: outcome_kind.to_string(),
        classification: classification.to_string(),
        probe_family: text_at(finding, "/probe/family")?.to_string(),
        probe_file: subject.anchor_file.clone(),
        probe_line: subject.anchor_line,
        probe_expression: subject.changed_behavior.clone(),
        expected_actionability: subject.expected_actionability.clone(),
        actionability_source: "governed_manifest_subject_contract".to_string(),
        missing,
        recommendation,
        static_limit_kind,
    })
}

fn validate_probe(
    subject: &PacketSubject,
    case: &ValidatedHostCase,
    host: &ValidatedHostRun,
    report: &Value,
    finding: &Value,
) -> Result<(), String> {
    let raw_file = text_at(finding, "/probe/file")?;
    let raw_root = text_at(report, "/root")?;
    let normalize = |value: &str| {
        value
            .replace('\\', "/")
            .trim_start_matches("./")
            .to_string()
    };
    let output_root = Path::new(&host.current_ref)
        .parent()
        .ok_or_else(|| "host current lacks an output root".to_string())?;
    let expected_execution_root = super::normalize_path(
        &output_root
            .join(format!(".staging-{}", host.run_id))
            .join("subjects")
            .join(&subject.case_id),
    );
    if normalize(raw_root) != expected_execution_root
        || normalize(raw_file) != format!("{expected_execution_root}/{}", subject.anchor_file)
    {
        return Err(format!(
            "host `{}` probe file `{raw_file}` is not exact execution root `{expected_execution_root}`",
            subject.case_id
        ));
    }
    let expected_file = case.materialized_root.join(&subject.anchor_file);
    fs::canonicalize(&expected_file).map_err(|error| {
        format!(
            "resolve retained probe source `{}`: {error}",
            expected_file.display()
        )
    })?;
    let expected_family = match subject.behavior_family.as_str() {
        "predicate_boundary" => "predicate",
        "return_value" => "return_value",
        other => return Err(format!("unsupported behavior family `{other}`")),
    };
    require_eq(
        text_at(finding, "/probe/family")?,
        expected_family,
        &format!("host `{}` probe family", subject.case_id),
    )?;
    let line = finding
        .pointer("/probe/line")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("host `{}` lacks an integer probe line", subject.case_id))?;
    if line != subject.anchor_line {
        return Err(format!(
            "host `{}` probe line `{line}` does not equal `{}`",
            subject.case_id, subject.anchor_line
        ));
    }
    require_eq(
        text_at(finding, "/probe/expression")?,
        &subject.changed_behavior,
        &format!("host `{}` probe expression", subject.case_id),
    )?;
    validate_enclosing_owner(&expected_file, subject.anchor_line, &subject.owner)
}

fn validate_enclosing_owner(path: &Path, line: u64, expected_owner: &str) -> Result<(), String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("read probe source `{}`: {error}", path.display()))?;
    let parsed = SourceFile::parse(&source, Edition::CURRENT);
    if !parsed.errors().is_empty() {
        return Err(format!(
            "probe source `{}` does not parse as Rust",
            path.display()
        ));
    }
    let line_index = usize::try_from(
        line.checked_sub(1)
            .ok_or_else(|| format!("probe source `{}` uses invalid line zero", path.display()))?,
    )
    .map_err(|error| format!("probe line does not fit usize: {error}"))?;
    let byte_offset = source
        .split_inclusive('\n')
        .take(line_index)
        .try_fold(0usize, |offset, part| offset.checked_add(part.len()))
        .ok_or_else(|| "probe byte offset overflowed".to_string())?;
    if line_index >= source.lines().count() {
        return Err(format!(
            "probe line `{line}` is outside `{}`",
            path.display()
        ));
    }
    let offset = TextSize::try_from(byte_offset)
        .map_err(|error| format!("probe byte offset does not fit TextSize: {error}"))?;
    let owners = parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .filter(|function| function.syntax().text_range().contains(offset))
        .filter_map(|function| function.name().map(|name| name.text().to_string()))
        .collect::<Vec<_>>();
    if owners.as_slice() != [expected_owner] {
        return Err(format!(
            "probe line `{line}` in `{}` is not uniquely enclosed by `{expected_owner}`; got {:?}",
            path.display(),
            owners
        ));
    }
    Ok(())
}

fn validate_direction_witness(
    subject: &PacketSubject,
    missing: &[String],
    recommendation: &str,
    static_limit_kind: Option<&str>,
) -> Result<(), String> {
    if missing != subject.expected_missing || recommendation != subject.expected_recommendation {
        return Err(format!(
            "host direction witness for `{}` is not the governed subject witness",
            subject.case_id
        ));
    }
    let (expected_missing, expected_recommendation) = match subject.expected_direction.as_str() {
        "should_gap" | "should_stay_quiet" | "should_limit" => (
            subject.expected_missing.clone(),
            subject.expected_recommendation.as_str(),
        ),
        other => return Err(format!("unsupported expected direction `{other}")),
    };
    if missing != expected_missing || recommendation != expected_recommendation {
        return Err(format!(
            "host `{}` direction witness is not the governed subject witness",
            subject.case_id
        ));
    }
    match subject.expected_direction.as_str() {
        "should_gap" => {
            if missing.is_empty() || recommendation.is_empty() || static_limit_kind.is_some() {
                return Err(format!(
                    "host `{}` lacks the exact should-gap witness",
                    subject.case_id
                ));
            }
        }
        "should_stay_quiet" => {
            if !missing.is_empty() || !recommendation.is_empty() || static_limit_kind.is_some() {
                return Err(format!(
                    "host `{}` lacks the exact should-stay-quiet witness",
                    subject.case_id
                ));
            }
        }
        "should_limit" => {
            if missing.is_empty()
                || recommendation.is_empty()
                || static_limit_kind != subject.expected_static_limit_kind.as_deref()
            {
                return Err(format!(
                    "host `{}` lacks the exact should-limit witness",
                    subject.case_id
                ));
            }
        }
        other => return Err(format!("unsupported expected direction `{other}`")),
    }
    Ok(())
}

fn validate_retained_packet(
    packet: &PortablePacket,
    entry: &PortableIndexEntry,
    attestation: &RetainedAttestation,
    subject: &PacketSubject,
    manifest_sha256: &str,
    subjects_sha256: &str,
) -> Result<(), String> {
    require_eq(&packet.schema_version, "0.1", "portable packet schema")?;
    require_eq(
        &packet.kind,
        "rust_judged_panel_portable_packet",
        "portable packet kind",
    )?;
    require_eq(&packet.case_id, &entry.case_id, "portable packet case")?;
    require_eq(
        &attestation.schema_version,
        "0.1",
        "portable attestation schema",
    )?;
    require_eq(
        &attestation.kind,
        "rust_judged_panel_retained_attestation",
        "portable attestation kind",
    )?;
    require_eq(
        &attestation.case_id,
        &entry.case_id,
        "portable attestation case",
    )?;
    if packet.semantic != attestation.semantic || packet.host_evidence != attestation.host_evidence
    {
        return Err(format!(
            "portable `{}` disagrees with retained host attestation",
            packet.case_id
        ));
    }
    require_eq(
        &packet.semantic_sha256,
        &entry.semantic_sha256,
        "portable packet index semantic digest",
    )?;
    require_eq(
        &sha256_serialized(&packet.semantic)?,
        &packet.semantic_sha256,
        "portable packet semantic self-digest",
    )?;
    for (label, actual, expected) in [
        (
            "manifest digest",
            packet.semantic.manifest_sha256.as_str(),
            manifest_sha256,
        ),
        (
            "subjects digest",
            packet.semantic.subjects_sha256.as_str(),
            subjects_sha256,
        ),
        (
            "subject id",
            packet.semantic.subject_id.as_str(),
            subject.subject_id.as_str(),
        ),
        (
            "repository",
            packet.semantic.repository.as_str(),
            subject.repository.as_str(),
        ),
        (
            "direction",
            packet.semantic.expected_direction.as_str(),
            subject.expected_direction.as_str(),
        ),
        (
            "base",
            packet.semantic.repository_base.as_str(),
            subject.expected_base.as_str(),
        ),
        (
            "head",
            packet.semantic.repository_head.as_str(),
            subject.expected_head.as_str(),
        ),
        (
            "tree",
            packet.semantic.repository_tree.as_str(),
            subject.expected_tree.as_str(),
        ),
        (
            "anchor file",
            packet.semantic.anchor.file.as_str(),
            subject.anchor_file.as_str(),
        ),
        (
            "anchor owner",
            packet.semantic.anchor.owner.as_str(),
            subject.owner.as_str(),
        ),
        (
            "behavior family",
            packet.semantic.anchor.behavior_family.as_str(),
            subject.behavior_family.as_str(),
        ),
        (
            "anchor expression",
            packet.semantic.anchor.expression.as_str(),
            subject.changed_behavior.as_str(),
        ),
        (
            "classification",
            packet.semantic.observed.classification.as_str(),
            subject.expected_classification.as_str(),
        ),
        (
            "actionability",
            packet.semantic.observed.expected_actionability.as_str(),
            subject.expected_actionability.as_str(),
        ),
        (
            "probe file",
            packet.semantic.observed.probe_file.as_str(),
            subject.anchor_file.as_str(),
        ),
        (
            "probe expression",
            packet.semantic.observed.probe_expression.as_str(),
            subject.changed_behavior.as_str(),
        ),
    ] {
        require_eq(
            actual,
            expected,
            &format!("portable `{}` {label}", packet.case_id),
        )?;
    }
    if packet.semantic.anchor.line != subject.anchor_line
        || packet.semantic.observed.probe_line != subject.anchor_line
        || !packet.semantic.observed.analysis_complete
        || packet.semantic.observed.outcome_kind != "complete_with_findings"
        || packet.semantic.observed.actionability_source != "governed_manifest_subject_contract"
        || packet.semantic.subject_inputs != expected_inputs(subject)
    {
        return Err(format!(
            "portable `{}` has stale semantic authority",
            packet.case_id
        ));
    }
    validate_direction_witness(
        subject,
        &packet.semantic.observed.missing,
        &packet.semantic.observed.recommendation,
        packet.semantic.observed.static_limit_kind.as_deref(),
    )?;
    if packet.judgment.disposition != "unjudged"
        || packet.judgment.structural_judgment.is_some()
        || packet.judgment.false_actionable.is_some()
        || packet.judgment.false_exposed.is_some()
        || packet.judgment.static_under_credit.is_some()
        || packet.judgment.wrong_target.is_some()
        || packet.judgment.limitation_correct.is_some()
        || packet.judgment.source.is_some()
        || packet.judgment.judged_at.is_some()
        || !packet.judgment.judged_by.is_empty()
        || packet.runtime_calibration.status != "not_run"
        || packet.runtime_calibration.outcome.is_some()
        || packet.runtime_calibration.evidence_ref.is_some()
    {
        return Err(format!(
            "portable `{}` contains an unsupported result claim",
            packet.case_id
        ));
    }
    if packet.non_claims != expected_non_claims() {
        return Err(format!("portable `{}` non-claims drifted", packet.case_id));
    }
    for reference in [
        &packet.host_evidence.current_ref,
        &packet.host_evidence.index_ref,
        &packet.host_evidence.receipt_ref,
        &packet.host_evidence.stdout_ref,
        &packet.host_evidence.stderr_ref,
    ] {
        safe_relative_path(reference)?;
    }
    Ok(())
}

fn validate_retained_host(
    packet: &PortablePacket,
    subject: &PacketSubject,
    host: &ValidatedHostRun,
) -> Result<(), String> {
    validate_host_evidence(&packet.host_evidence, &packet.case_id, host)?;
    let case = host
        .cases
        .iter()
        .find(|case| case.case_id == packet.case_id)
        .ok_or_else(|| format!("host run is missing `{}`", packet.case_id))?;
    validate_host_join(subject, case, host)?;
    for (label, actual, expected) in [
        (
            "producer source head",
            &packet.semantic.producer_source_head,
            &host.source_head,
        ),
        (
            "producer source tree",
            &packet.semantic.producer_source_tree,
            &host.source_tree,
        ),
        (
            "producer Cargo.toml digest",
            &packet.semantic.producer_cargo_toml_sha256,
            &host.cargo_toml_sha256,
        ),
        (
            "producer Cargo.lock digest",
            &packet.semantic.producer_cargo_lock_sha256,
            &host.cargo_lock_sha256,
        ),
        (
            "producer version",
            &packet.semantic.producer_version,
            &host.binary_version,
        ),
    ] {
        require_eq(
            actual,
            expected,
            &format!("portable `{}` {label}", packet.case_id),
        )?;
    }
    if packet.semantic.profile != host.profile || packet.semantic.features != host.features {
        return Err(format!(
            "portable `{}` build authority drifted",
            packet.case_id
        ));
    }
    let inputs_match = packet.semantic.subject_inputs.len() == case.subject_inputs.len()
        && packet
            .semantic
            .subject_inputs
            .iter()
            .zip(&case.subject_inputs)
            .all(|(actual, expected)| {
                actual.role == expected.role
                    && actual.source_path == expected.source_path
                    && actual.repository_path == expected.repository_path
                    && actual.sha256 == expected.sha256
            });
    if packet.semantic.argv != case.argv
        || packet.semantic.mode != case.mode
        || packet.semantic.format != case.format
        || packet.semantic.config_path != case.config_path
        || packet.semantic.config_sha256 != case.config_sha256
        || packet.semantic.diff_path != case.diff_path
        || packet.semantic.diff_sha256 != case.diff_sha256
        || packet.semantic.executed_diff_identity != case.executed_diff_identity
        || !inputs_match
    {
        return Err(format!(
            "portable `{}` execution authority drifted",
            packet.case_id
        ));
    }
    let report_text = std::str::from_utf8(&case.stdout)
        .map_err(|error| format!("host stdout for `{}` is not UTF-8: {error}", packet.case_id))?;
    let report = super::parse_json_without_duplicate_keys(report_text)
        .map_err(|error| format!("parse host stdout for `{}`: {error}", packet.case_id))?;
    let observed = project_observed(subject, case, host, &report)?;
    if packet.semantic.observed != observed {
        return Err(format!(
            "portable `{}` finding authority drifted",
            packet.case_id
        ));
    }
    Ok(())
}

fn validate_host_evidence(
    evidence: &HostEvidence,
    case_id: &str,
    host: &ValidatedHostRun,
) -> Result<(), String> {
    require_eq(
        &evidence.availability,
        "host_bound_not_committed",
        "host evidence availability",
    )?;
    for (label, actual, expected) in [
        ("host target", &evidence.host_target, &host.host_target),
        (
            "binary digest",
            &evidence.binary_sha256,
            &host.binary_sha256,
        ),
        ("run id", &evidence.run_id, &host.run_id),
        ("current ref", &evidence.current_ref, &host.current_ref),
        (
            "current digest",
            &evidence.current_sha256,
            &host.current_sha256,
        ),
        ("index ref", &evidence.index_ref, &host.index_ref),
        ("index digest", &evidence.index_sha256, &host.index_sha256),
    ] {
        require_eq(actual, expected, &format!("host evidence {label}"))?;
    }
    let case = host
        .cases
        .iter()
        .find(|case| case.case_id == case_id)
        .ok_or_else(|| format!("host run is missing `{case_id}`"))?;
    for (label, actual, expected) in [
        ("receipt ref", &evidence.receipt_ref, &case.receipt_ref),
        (
            "receipt digest",
            &evidence.receipt_sha256,
            &case.receipt_sha256,
        ),
        ("stdout ref", &evidence.stdout_ref, &case.stdout_ref),
        (
            "stdout digest",
            &evidence.stdout_sha256,
            &case.stdout_sha256,
        ),
        ("stderr ref", &evidence.stderr_ref, &case.stderr_ref),
        (
            "stderr digest",
            &evidence.stderr_sha256,
            &case.stderr_sha256,
        ),
        (
            "analyzer input identity",
            &evidence.analyzer_input_identity,
            &case.analyzer_input_identity,
        ),
    ] {
        require_eq(actual, expected, &format!("host evidence {label}"))?;
    }
    Ok(())
}

fn validate_index_shape(
    index: &PortableIndex,
    current: &PortableCurrent,
    manifest_sha256: &str,
    subjects_sha256: &str,
    expected_len: usize,
) -> Result<(), String> {
    require_eq(&index.schema_version, "0.1", "portable index schema")?;
    require_eq(
        &index.kind,
        "rust_judged_panel_portable_index",
        "portable index kind",
    )?;
    require_eq(
        &index.publication_state,
        "complete",
        "portable publication state",
    )?;
    require_eq(
        &index.generation_id,
        &current.generation_id,
        "portable generation id",
    )?;
    require_eq(
        &index.manifest_sha256,
        manifest_sha256,
        "portable index manifest digest",
    )?;
    require_eq(
        &index.subjects_sha256,
        subjects_sha256,
        "portable index subjects digest",
    )?;
    if index.packets.len() != expected_len || expected_len != 3 {
        return Err(format!(
            "portable index requires exactly three packets, got {}",
            index.packets.len()
        ));
    }
    if index.non_claims != expected_non_claims() {
        return Err("portable index non-claims drifted".to_string());
    }
    let expected_id = generation_id(manifest_sha256, subjects_sha256, &index.packets)?;
    require_eq(
        &expected_id,
        &index.generation_id,
        "portable content-addressed generation",
    )?;
    let expected_path = format!("{PORTABLE_ROOT}/generations/{expected_id}/packet-index.json");
    require_eq(
        &current.index_path,
        &expected_path,
        "portable current index path",
    )
}

fn publish_all(
    root: &Path,
    manifest_sha256: &str,
    subjects_sha256: &str,
    packets: &[PortablePacket],
    attestations: &[RetainedAttestation],
    fail_after: Option<usize>,
) -> Result<(), String> {
    if packets.len() != 3 || attestations.len() != packets.len() {
        return Err(format!(
            "portable publication requires exactly three packet/attestation pairs, got {}/{}",
            packets.len(),
            attestations.len()
        ));
    }
    let portable_root = root.join(PORTABLE_ROOT);
    fs::create_dir_all(&portable_root).map_err(|error| {
        format!(
            "create portable root `{}`: {error}",
            portable_root.display()
        )
    })?;
    require_canonical_portable(root, &portable_root, "portable root")?;
    let staging_root = root.join(STAGING_ROOT);
    fs::create_dir_all(&staging_root).map_err(|error| {
        format!(
            "create packet staging root `{}`: {error}",
            staging_root.display()
        )
    })?;
    let lock_path = staging_root.join("packet.lock");
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .and_then(|mut file| file.write_all(b"rust-judged-panel packet publisher\n"))
        .map_err(|error| {
            format!(
                "acquire packet publication lock `{}`: {error}",
                lock_path.display()
            )
        })?;
    let _lock = PacketLock(lock_path);
    let sequence = PACKET_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let stage = staging_root.join(format!("stage-{}-{sequence}", std::process::id()));
    let result = publish_staged(
        root,
        &stage,
        manifest_sha256,
        subjects_sha256,
        packets,
        attestations,
        fail_after,
    );
    if stage.exists() {
        let _cleanup = fs::remove_dir_all(&stage);
    }
    result
}

fn publish_staged(
    root: &Path,
    stage: &Path,
    manifest_sha256: &str,
    subjects_sha256: &str,
    packets: &[PortablePacket],
    attestations: &[RetainedAttestation],
    fail_after: Option<usize>,
) -> Result<(), String> {
    let attestations = attestations
        .iter()
        .map(|attestation| (attestation.case_id.as_str(), attestation))
        .collect::<BTreeMap<_, _>>();
    let mut packet_bytes = Vec::new();
    for packet in packets {
        let attestation = attestations
            .get(packet.case_id.as_str())
            .ok_or_else(|| format!("packet `{}` lacks retained attestation", packet.case_id))?;
        packet_bytes.push((packet, pretty_json(packet)?, pretty_json(attestation)?));
    }
    packet_bytes.sort_by(|(left, _, _), (right, _, _)| left.case_id.cmp(&right.case_id));
    let provisional = packet_bytes
        .iter()
        .map(|(packet, bytes, attestation)| PortableIndexEntry {
            case_id: packet.case_id.clone(),
            packet_path: String::new(),
            packet_sha256: sha256_bytes(bytes),
            semantic_sha256: packet.semantic_sha256.clone(),
            attestation_path: String::new(),
            attestation_sha256: sha256_bytes(attestation),
        })
        .collect::<Vec<_>>();
    let generation_id = generation_id(manifest_sha256, subjects_sha256, &provisional)?;
    let relative_generation = format!("{PORTABLE_ROOT}/generations/{generation_id}");
    let staged_generation = stage.join("generation");
    let staged_packets = staged_generation.join("packets");
    let staged_attestations = staged_generation.join("attestations");
    fs::create_dir_all(&staged_packets).map_err(|error| {
        format!(
            "create staged packets `{}`: {error}",
            staged_packets.display()
        )
    })?;
    fs::create_dir_all(&staged_attestations).map_err(|error| {
        format!(
            "create staged attestations `{}`: {error}",
            staged_attestations.display()
        )
    })?;
    let mut entries = Vec::new();
    for (index, (packet, bytes, attestation)) in packet_bytes.iter().enumerate() {
        validate_case_filename(&packet.case_id)?;
        let file_name = format!("{}.json", packet.case_id);
        fs::write(staged_packets.join(&file_name), bytes)
            .map_err(|error| format!("write staged packet `{file_name}`: {error}"))?;
        fs::write(staged_attestations.join(&file_name), attestation)
            .map_err(|error| format!("write staged attestation `{file_name}`: {error}"))?;
        entries.push(PortableIndexEntry {
            case_id: packet.case_id.clone(),
            packet_path: format!("{relative_generation}/packets/{file_name}"),
            packet_sha256: sha256_bytes(bytes),
            semantic_sha256: packet.semantic_sha256.clone(),
            attestation_path: format!("{relative_generation}/attestations/{file_name}"),
            attestation_sha256: sha256_bytes(attestation),
        });
        if fail_after == Some(index + 1) {
            return Err(format!(
                "injected packet publication failure after {} packets",
                index + 1
            ));
        }
    }
    let index = PortableIndex {
        schema_version: "0.1".to_string(),
        kind: "rust_judged_panel_portable_index".to_string(),
        publication_state: "complete".to_string(),
        generation_id: generation_id.clone(),
        manifest_sha256: manifest_sha256.to_string(),
        subjects_sha256: subjects_sha256.to_string(),
        packets: entries,
        non_claims: expected_non_claims(),
    };
    let index_bytes = pretty_json(&index)?;
    fs::write(staged_generation.join("packet-index.json"), &index_bytes)
        .map_err(|error| format!("write staged packet index: {error}"))?;
    validate_generation_files(&staged_generation, &index)?;
    let final_generation = root.join(&relative_generation);
    if final_generation.exists() {
        require_canonical_portable(root, &final_generation, "retained generation")?;
        let retained_index = final_generation.join("packet-index.json");
        require_canonical_generation_file(
            &final_generation,
            &retained_index,
            "retained packet index",
        )?;
        let retained = fs::read(&retained_index)
            .map_err(|error| format!("read retained packet index: {error}"))?;
        if retained != index_bytes {
            return Err(format!(
                "content-addressed generation `{generation_id}` conflicts"
            ));
        }
        validate_generation_files(&final_generation, &index)?;
    } else {
        let parent = final_generation
            .parent()
            .ok_or_else(|| "portable generation has no parent".to_string())?;
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "create portable generations `{}`: {error}",
                parent.display()
            )
        })?;
        require_canonical_portable(root, parent, "portable generations")?;
        fs::rename(&staged_generation, &final_generation)
            .map_err(|error| format!("publish generation `{generation_id}`: {error}"))?;
        require_canonical_portable(root, &final_generation, "published generation")?;
        validate_generation_files(&final_generation, &index)?;
    }
    let current = PortableCurrent {
        schema_version: "0.1".to_string(),
        kind: "rust_judged_panel_portable_current".to_string(),
        generation_id: generation_id.clone(),
        index_path: format!("{relative_generation}/packet-index.json"),
        index_sha256: sha256_bytes(&index_bytes),
    };
    atomic_write(&root.join(CURRENT_PATH), &pretty_json(&current)?)
}

fn validate_generation_files(generation: &Path, index: &PortableIndex) -> Result<(), String> {
    for entry in &index.packets {
        validate_case_filename(&entry.case_id)?;
        let file_name = format!("{}.json", entry.case_id);
        let expected_packet = format!(
            "{PORTABLE_ROOT}/generations/{}/packets/{file_name}",
            index.generation_id
        );
        let expected_attestation = format!(
            "{PORTABLE_ROOT}/generations/{}/attestations/{file_name}",
            index.generation_id
        );
        require_eq(
            &entry.packet_path,
            &expected_packet,
            "retained generation packet path",
        )?;
        require_eq(
            &entry.attestation_path,
            &expected_attestation,
            "retained generation attestation path",
        )?;
        let packet_path = generation.join("packets").join(&file_name);
        let attestation_path = generation.join("attestations").join(&file_name);
        require_canonical_generation_file(generation, &packet_path, "retained packet")?;
        require_canonical_generation_file(generation, &attestation_path, "retained attestation")?;
        require_eq(
            &sha256_file(&packet_path)?,
            &entry.packet_sha256,
            "retained generation packet digest",
        )?;
        require_eq(
            &sha256_file(&attestation_path)?,
            &entry.attestation_sha256,
            "retained generation attestation digest",
        )?;
        let packet: PortablePacket = read_strict_json(&packet_path, "retained packet")?;
        let attestation: RetainedAttestation =
            read_strict_json(&attestation_path, "retained attestation")?;
        if packet.case_id != entry.case_id
            || packet.semantic_sha256 != entry.semantic_sha256
            || packet.semantic != attestation.semantic
            || packet.host_evidence != attestation.host_evidence
        {
            return Err(format!(
                "retained generation authority disagrees for `{}`",
                entry.case_id
            ));
        }
    }
    Ok(())
}

fn require_canonical_generation_file(
    generation: &Path,
    path: &Path,
    label: &str,
) -> Result<(), String> {
    let canonical_generation = fs::canonicalize(generation).map_err(|error| {
        format!(
            "canonicalize retained generation `{}`: {error}",
            generation.display()
        )
    })?;
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("canonicalize {label} `{}`: {error}", path.display()))?;
    let relative = path.strip_prefix(generation).map_err(|error| {
        format!("{label} is not lexically inside its retained generation: {error}")
    })?;
    if canonical == canonical_generation.join(relative) {
        Ok(())
    } else {
        Err(format!("{label} escapes its generation through a link"))
    }
}

fn generation_id(
    manifest_sha256: &str,
    subjects_sha256: &str,
    entries: &[PortableIndexEntry],
) -> Result<String, String> {
    let identity = entries
        .iter()
        .map(|entry| {
            (
                entry.case_id.as_str(),
                entry.packet_sha256.as_str(),
                entry.semantic_sha256.as_str(),
                entry.attestation_sha256.as_str(),
            )
        })
        .collect::<Vec<_>>();
    let digest = sha256_serialized(&(manifest_sha256, subjects_sha256, identity))?;
    digest
        .strip_prefix("sha256:")
        .map(ToString::to_string)
        .ok_or_else(|| "portable generation digest lacks sha256 prefix".to_string())
}

fn expected_non_claims() -> Vec<String> {
    vec![
        "bounded static projection only; no independent semantic judgment".to_string(),
        "no runtime mutation calibration, accuracy rate, badge, gate, or support claim".to_string(),
    ]
}

fn validate_case_filename(case_id: &str) -> Result<(), String> {
    if case_id.is_empty()
        || !case_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(format!("portable case id `{case_id}` is not filename-safe"));
    }
    Ok(())
}

fn safe_portable_path(value: &str) -> Result<(), String> {
    safe_relative_path(value)?;
    let prefix = format!("{PORTABLE_ROOT}/");
    if value.contains('\\') || value.contains("//") || !value.starts_with(&prefix) {
        return Err(format!("portable path `{value}` escapes `{PORTABLE_ROOT}`"));
    }
    Ok(())
}

fn require_canonical_portable(root: &Path, path: &Path, label: &str) -> Result<(), String> {
    let repository =
        fs::canonicalize(root).map_err(|error| format!("canonicalize repository root: {error}"))?;
    let portable = fs::canonicalize(root.join(PORTABLE_ROOT))
        .map_err(|error| format!("canonicalize portable root: {error}"))?;
    if portable != repository.join(PORTABLE_ROOT) {
        return Err("portable root is redirected through a link".to_string());
    }
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("canonicalize {label} `{}`: {error}", path.display()))?;
    let relative = path
        .strip_prefix(root.join(PORTABLE_ROOT))
        .map_err(|error| format!("{label} is not lexically inside the portable root: {error}"))?;
    if canonical == portable.join(relative) {
        Ok(())
    } else {
        Err(format!("{label} escapes the canonical portable root"))
    }
}

fn safe_relative_path(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "evidence path `{value}` is not a safe relative path"
        ));
    }
    Ok(())
}

fn text_at<'a>(value: &'a Value, pointer: &str) -> Result<&'a str, String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| format!("JSON field `{pointer}` is missing or empty"))
}

fn require_eq(actual: &str, expected: &str, label: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{label} `{actual}` does not equal `{expected}`"))
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("output `{}` has no parent", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("create `{}`: {error}", parent.display()))?;
    let sequence = PACKET_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp = parent.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name().and_then(OsStr::to_str).unwrap_or("packet"),
        std::process::id(),
        sequence
    ));
    fs::write(&temp, bytes).map_err(|error| format!("write `{}`: {error}", temp.display()))?;
    replace_file(&temp, path)
}

#[cfg(not(windows))]
fn replace_file(temp: &Path, path: &Path) -> Result<(), String> {
    fs::rename(temp, path).map_err(|error| format!("publish `{}`: {error}", path.display()))
}

#[cfg(windows)]
fn replacement_backup_path(path: &Path) -> PathBuf {
    path.with_extension(format!(
        "{}-backup",
        path.extension().and_then(OsStr::to_str).unwrap_or("json")
    ))
}

#[cfg(windows)]
fn remove_replacement_backup(path: &Path, backup: &Path) -> Result<(), String> {
    fs::remove_file(backup).map_err(|error| {
        format!(
            "published `{}` but could not remove replacement backup `{}`: {error}",
            path.display(),
            backup.display()
        )
    })
}

#[cfg(windows)]
fn replace_file(temp: &Path, path: &Path) -> Result<(), String> {
    let backup = replacement_backup_path(path);
    if backup.exists() {
        if path.exists() {
            fs::remove_file(&backup).map_err(|error| {
                format!(
                    "remove stale current replacement backup `{}`: {error}",
                    backup.display()
                )
            })?;
        } else {
            fs::rename(&backup, path).map_err(|error| {
                format!(
                    "recover interrupted current replacement from `{}`: {error}",
                    backup.display()
                )
            })?;
        }
    }
    if !path.exists() {
        return fs::rename(temp, path)
            .map_err(|error| format!("publish `{}`: {error}", path.display()));
    }
    fs::rename(path, &backup).map_err(|error| {
        format!(
            "stage existing `{}` for replacement: {error}",
            path.display()
        )
    })?;
    match fs::rename(temp, path) {
        Ok(()) => remove_replacement_backup(path, &backup),
        Err(error) => {
            let restore = fs::rename(&backup, path);
            if let Err(restore_error) = restore {
                return Err(format!(
                    "publish `{}` failed: {error}; restoring prior current also failed: {restore_error}",
                    path.display()
                ));
            }
            Err(format!("publish `{}`: {error}", path.display()))
        }
    }
}

fn read_strict_json<T: for<'de> Deserialize<'de>>(path: &Path, label: &str) -> Result<T, String> {
    let body = fs::read_to_string(path)
        .map_err(|error| format!("read {label} `{}`: {error}", path.display()))?;
    let value = super::parse_json_without_duplicate_keys(&body)
        .map_err(|error| format!("parse {label} `{}`: {error}", path.display()))?;
    serde_json::from_value(value)
        .map_err(|error| format!("parse {label} `{}`: {error}", path.display()))
}

#[cfg(test)]
fn read_strict_json_bytes<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
    label: &str,
) -> Result<T, String> {
    let body = std::str::from_utf8(bytes)
        .map_err(|error| format!("decode {label} bytes as UTF-8: {error}"))?;
    let value = super::parse_json_without_duplicate_keys(body)
        .map_err(|error| format!("parse {label}: {error}"))?;
    serde_json::from_value(value).map_err(|error| format!("parse {label}: {error}"))
}

fn pretty_json<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("serialize portable packet JSON: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sha256_serialized<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_bytes(&bytes))
        .map_err(|error| format!("serialize portable identity: {error}"))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    fs::read(path)
        .map(|bytes| sha256_bytes(&bytes))
        .map_err(|error| format!("hash `{}`: {error}", path.display()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests;
