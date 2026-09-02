//! Exact producer admission and same-run analysis reuse for PR review guidance.

use super::CheckInput;
use crate::analysis_outcome::AnalysisOutcome;
use crate::config::{RiprConfig, repo_exposure_config_identity_hash};
use crate::review_input::{
    CanonicalFindingIndexV1, ReviewInputV1, canonical_projection_from_index,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};
pub(crate) const REVIEW_ANALYSIS_IDENTITY_SCHEMA: &str = "ripr.review_analysis_identity.v1";
// Keep this admission bound synchronized with the producer projection limit.
const REVIEW_INPUT_PROJECTION_LIMIT: u64 = 10;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReviewAnalysisIdentity {
    pub(crate) schema_version: String,
    pub(crate) repository_identity: String,
    pub(crate) root: String,
    pub(crate) base_sha: String,
    pub(crate) head_sha: String,
    pub(crate) head_tree: String,
    pub(crate) canonical_diff_sha256: String,
    pub(crate) mode: String,
    pub(crate) configuration_fingerprint: String,
    pub(crate) producer_schema: String,
    pub(crate) analyzer_generation: String,
    pub(crate) check_sha256: String,
}
#[derive(Clone, Debug)]
pub(crate) struct AdmittedReviewAnalysis {
    pub(crate) identity: ReviewAnalysisIdentity,
    pub(crate) outcome: AnalysisOutcome,
    pub(crate) producer_projection: Vec<crate::review_input::ReviewFindingProjectionV1>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AdmittedReviewInput {
    pub(crate) projection_sha256: String,
    pub(crate) reviewed_count: usize,
    pub(crate) projection: Value,
    pub(crate) analysis_outcome: Option<AnalysisOutcome>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProducerAdmissionError {
    pub(crate) category: &'static str,
    pub(crate) message: String,
}
pub(crate) fn run_analysis_with_timeout<T>(
    timeout_ms: u64,
    work: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let token = crate::analysis::cancellation::AnalysisCancellationToken::new();
    let (finished_sender, finished_receiver) = mpsc::sync_channel(1);
    std::thread::scope(|scope| {
        let started = Instant::now();
        let timer_token = token.clone();
        scope.spawn(move || {
            if finished_receiver
                .recv_timeout(Duration::from_millis(timeout_ms))
                .is_err()
            {
                let _ = timer_token
                    .cancel(crate::analysis::cancellation::AnalysisAbortKind::DeadlineExceeded);
            }
        });
        let result = crate::analysis::cancellation::with_token(&token, work);
        let deadline = if started.elapsed() >= Duration::from_millis(timeout_ms) {
            let _ =
                token.cancel(crate::analysis::cancellation::AnalysisAbortKind::DeadlineExceeded);
            Err("analysis cancelled: DeadlineExceeded".to_string())
        } else {
            token.checkpoint().map_err(|error| error.to_string())
        };
        let _ = finished_sender.send(());
        result.and_then(|value| deadline.map(|_| value))
    })
}
impl ProducerAdmissionError {
    fn malformed(message: impl Into<String>) -> Self {
        Self {
            category: "malformed_producer",
            message: message.into(),
        }
    }

    fn missing(message: impl Into<String>) -> Self {
        Self {
            category: "missing_producer",
            message: message.into(),
        }
    }

    fn mismatch(field: &str) -> Self {
        Self {
            category: if field == "mode" {
                "producer_mode_mismatch"
            } else {
                "producer_identity_mismatch"
            },
            message: format!("producer {field} does not match the requested analysis identity"),
        }
    }
}
pub(crate) fn admit_producer_evidence(
    check_path: &Path,
    input: &CheckInput,
    config: &RiprConfig,
    base: &str,
    head: &str,
    diff_text: &str,
) -> Result<AdmittedReviewAnalysis, ProducerAdmissionError> {
    let subject_path = check_path.with_extension("subject.json");
    let subject_bytes = std::fs::read(&subject_path).map_err(|error| {
        let message = format!(
            "producer subject receipt {} is unreadable: {error}",
            subject_path.display()
        );
        if error.kind() == std::io::ErrorKind::NotFound {
            ProducerAdmissionError::missing(message)
        } else {
            ProducerAdmissionError::malformed(message)
        }
    })?;
    let subject: Value = serde_json::from_slice(&subject_bytes).map_err(|error| {
        ProducerAdmissionError::malformed(format!(
            "producer subject receipt {} is invalid JSON: {error}",
            subject_path.display()
        ))
    })?;
    require_equal(
        "subject_schema_version",
        required_string(&subject, "schema_version")?,
        "ripr.pr_check_subject.v1",
    )?;
    let base_sha = resolve_revision(&input.root, base, "commit")?;
    let head_sha = resolve_revision(&input.root, head, "commit")?;
    let head_tree = resolve_revision(&input.root, head, "tree")?;
    let expected_root = logical_path(&input.root);
    let expected_diff = digest_bytes(diff_text.as_bytes());
    let expected_config = repo_exposure_config_identity_hash(config);
    for (field, expected) in [
        ("root_identity", expected_root.as_str()),
        ("base_sha", base_sha.as_str()),
        ("head_sha", head_sha.as_str()),
        ("head_tree", head_tree.as_str()),
        ("mode", input.mode.as_str()),
        ("canonical_diff_sha256", expected_diff.as_str()),
        ("configuration_fingerprint", expected_config.as_str()),
        (
            "analyzer_generation",
            crate::review_input::REVIEW_ANALYZER_GENERATION,
        ),
        ("check_schema", "0.2"),
    ] {
        require_equal(field, required_string(&subject, field)?, expected)?;
    }
    let review_input_path = check_path.with_file_name("review-input.json");
    let review_input_bytes = std::fs::read(&review_input_path).map_err(|error| {
        let message = format!(
            "producer review input {} is unreadable: {error}",
            review_input_path.display()
        );
        if error.kind() == std::io::ErrorKind::NotFound {
            ProducerAdmissionError::missing(message)
        } else {
            ProducerAdmissionError::malformed(message)
        }
    })?;
    let review_input: crate::review_input::ReviewInputV1 =
        serde_json::from_slice(&review_input_bytes).map_err(|error| {
            ProducerAdmissionError::malformed(format!(
                "producer review input {} is invalid: {error}",
                review_input_path.display()
            ))
        })?;
    let review_input_sha256 = digest_bytes(&review_input_bytes);
    require_equal(
        "review_input_sha256",
        required_string(&subject, "review_input_sha256")?,
        &review_input_sha256,
    )?;
    let review_input_byte_count = required_value(&subject, "review_input_byte_count")?
        .as_u64()
        .ok_or_else(|| ProducerAdmissionError::malformed("review_input_byte_count is invalid"))?;
    if review_input_byte_count != review_input_bytes.len() as u64 {
        return Err(ProducerAdmissionError::malformed(
            "review_input_byte_count does not match review-input.json",
        ));
    }
    let identity = ReviewAnalysisIdentity {
        schema_version: REVIEW_ANALYSIS_IDENTITY_SCHEMA.to_string(),
        repository_identity: repository_identity(&input.root),
        root: logical_path(&input.root),
        base_sha,
        head_sha,
        head_tree,
        canonical_diff_sha256: digest_bytes(diff_text.as_bytes()),
        mode: input.mode.as_str().to_string(),
        configuration_fingerprint: repo_exposure_config_identity_hash(config),
        producer_schema: "0.2".to_string(),
        analyzer_generation: crate::review_input::REVIEW_ANALYZER_GENERATION.to_string(),
        check_sha256: required_string(&subject, "check_sha256")?.to_string(),
    };
    let admitted_input = admit_review_input(
        &review_input_path,
        &input.root,
        &identity,
        review_input.total_finding_count,
        Some(&canonical_projection_from_subject(&subject)?),
    )?;
    let outcome_value = required_value(&subject, "analysis_outcome")?.clone();
    let outcome_value = outcome_value.get("outcome").cloned().ok_or_else(|| {
        ProducerAdmissionError::malformed(
            "producer review input analysis_outcome is missing outcome",
        )
    })?;
    let outcome: AnalysisOutcome = serde_json::from_value(outcome_value).map_err(|error| {
        ProducerAdmissionError::malformed(format!(
            "producer review input analysis_outcome is invalid: {error}"
        ))
    })?;
    if !outcome.kind.is_complete()
        || outcome.counts.finding_count != review_input.total_finding_count
    {
        return Err(ProducerAdmissionError {
            category: "incomplete_producer",
            message: "producer analysis is not complete and cannot be reused".to_string(),
        });
    }
    let producer_projection: Vec<crate::review_input::ReviewFindingProjectionV1> =
        serde_json::from_value(admitted_input.projection["findings"].clone()).map_err(|error| {
            ProducerAdmissionError::malformed(format!(
                "producer review input projection is invalid: {error}"
            ))
        })?;
    Ok(AdmittedReviewAnalysis {
        identity,
        outcome,
        producer_projection,
    })
}
pub(crate) fn admit_review_input(
    review_input_path: &Path,
    root: &Path,
    identity: &ReviewAnalysisIdentity,
    producer_finding_count: u64,
    producer_projection: Option<&[crate::review_input::ReviewFindingProjectionV1]>,
) -> Result<AdmittedReviewInput, ProducerAdmissionError> {
    let bytes = std::fs::read(review_input_path).map_err(|error| {
        let message = format!(
            "producer review input {} is unreadable: {error}",
            review_input_path.display()
        );
        if error.kind() == std::io::ErrorKind::NotFound {
            ProducerAdmissionError::missing(message)
        } else {
            ProducerAdmissionError::malformed(message)
        }
    })?;
    let raw: Value = serde_json::from_slice(&bytes).map_err(|error| {
        ProducerAdmissionError::malformed(format!(
            "producer review input {} is invalid or does not match ReviewInputV1: {error}",
            review_input_path.display()
        ))
    })?;
    require_equal(
        "review_input_schema_version",
        required_string(&raw, "schema_version")?,
        crate::review_input::REVIEW_INPUT_SCHEMA_VERSION,
    )?;
    require_equal(
        "root_identity",
        required_string(&raw, "root_identity")?,
        &logical_path(root),
    )?;
    let input: ReviewInputV1 = serde_json::from_value(raw).map_err(|error| {
        ProducerAdmissionError::malformed(format!(
            "producer review input does not match ReviewInputV1: {error}"
        ))
    })?;
    let value = serde_json::to_value(&input).map_err(|error| {
        ProducerAdmissionError::malformed(format!(
            "serialize admitted producer review input: {error}"
        ))
    })?;
    require_equal(
        "review_input_schema_version",
        required_string(&value, "schema_version")?,
        "ripr.review_input.v1",
    )?;
    let root_identity = logical_path(root);
    require_equal(
        "root_identity",
        required_string(&value, "root_identity")?,
        &root_identity,
    )?;
    for (field, expected) in [
        ("base_sha", identity.base_sha.as_str()),
        ("head_sha", identity.head_sha.as_str()),
        ("head_tree", identity.head_tree.as_str()),
        ("check_sha256", identity.check_sha256.as_str()),
        (
            "canonical_diff_sha256",
            identity.canonical_diff_sha256.as_str(),
        ),
        ("mode", identity.mode.as_str()),
    ] {
        require_equal(field, required_string(&value, field)?, expected)?;
    }
    let findings = value
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ProducerAdmissionError::malformed("review input findings must be an array")
        })?;
    let reviewed_count = value
        .get("reviewed_count")
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or_else(|| {
            ProducerAdmissionError::malformed("review input reviewed_count is invalid")
        })?;
    if reviewed_count != findings.len() {
        return Err(ProducerAdmissionError::malformed(
            "review input reviewed_count contradicts its findings payload",
        ));
    }
    let analysis_complete = value
        .get("analysis_complete")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            ProducerAdmissionError::malformed("review input analysis_complete is invalid")
        })?;
    if !analysis_complete {
        return Err(ProducerAdmissionError {
            category: "incomplete_producer",
            message: "producer analysis is not complete and cannot be reused".to_string(),
        });
    }
    let total_finding_count = value
        .get("total_finding_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            ProducerAdmissionError::malformed("review input total_finding_count is invalid")
        })?;
    let projected_finding_count = value
        .get("projected_finding_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            ProducerAdmissionError::malformed("review input projected_finding_count is invalid")
        })?;
    let projection_limit = value
        .get("projection_limit")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            ProducerAdmissionError::malformed("review input projection_limit is invalid")
        })?;
    let projection_truncated = value
        .get("projection_truncated")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            ProducerAdmissionError::malformed("review input projection_truncated is invalid")
        })?;
    if total_finding_count != producer_finding_count
        || projected_finding_count != reviewed_count as u64
        || projected_finding_count != findings.len() as u64
        || projection_limit != REVIEW_INPUT_PROJECTION_LIMIT
        || projection_truncated != (total_finding_count > projected_finding_count)
        || total_finding_count < projected_finding_count
    {
        return Err(ProducerAdmissionError::malformed(
            "review input projection bounds contradict its findings payload",
        ));
    }
    if let Some(expected) = producer_projection {
        let expected_value = serde_json::to_value(expected).map_err(|error| {
            ProducerAdmissionError::malformed(format!(
                "serialize canonical review input projection: {error}"
            ))
        })?;
        if value.get("findings") != Some(&expected_value) {
            return Err(ProducerAdmissionError::malformed(
                "review input findings are not the canonical producer projection",
            ));
        }
    }
    require_equal(
        "projection_selection_policy",
        required_string(&value, "projection_selection_policy")?,
        "severity_actionability_stable_id_path_line",
    )?;
    require_equal(
        "projection_selection_policy_version",
        required_string(&value, "projection_selection_policy_version")?,
        "v1",
    )?;
    let projection_sha256 = required_string(&value, "projection_sha256")?;
    let projection_bytes = serde_json::to_vec(findings).map_err(|error| {
        ProducerAdmissionError::malformed(format!("serialize review input digest: {error}"))
    })?;
    require_equal(
        "projection_sha256",
        projection_sha256,
        &digest_bytes(&projection_bytes),
    )?;
    let analysis_outcome = input
        .analysis_outcome
        .clone()
        .and_then(|value| value.get("outcome").cloned())
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| {
            ProducerAdmissionError::malformed(format!(
                "review input analysis_outcome is invalid: {error}"
            ))
        })?;
    Ok(AdmittedReviewInput {
        projection_sha256: projection_sha256.to_string(),
        reviewed_count,
        projection: value,
        analysis_outcome,
    })
}
fn canonical_projection_from_subject(
    subject: &Value,
) -> Result<Vec<crate::review_input::ReviewFindingProjectionV1>, ProducerAdmissionError> {
    let index: CanonicalFindingIndexV1 =
        serde_json::from_value(required_value(subject, "canonical_finding_index")?.clone())
            .map_err(|error| {
                ProducerAdmissionError::malformed(format!(
                    "invalid canonical finding index: {error}"
                ))
            })?;
    let projection =
        canonical_projection_from_index(&index).map_err(ProducerAdmissionError::malformed)?;
    let entry_count = required_value(subject, "canonical_finding_index_entry_count")?
        .as_u64()
        .ok_or_else(|| {
            ProducerAdmissionError::malformed("canonical finding index entry count is invalid")
        })?;
    if entry_count != index.entries.len() as u64 {
        return Err(ProducerAdmissionError::malformed(
            "canonical finding index entry count is contradictory",
        ));
    }
    let byte_count = required_value(subject, "canonical_finding_index_byte_count")?
        .as_u64()
        .ok_or_else(|| {
            ProducerAdmissionError::malformed("canonical finding index byte count is invalid")
        })?;
    let encoded = serde_json::to_vec(&index.entries).map_err(|error| {
        ProducerAdmissionError::malformed(format!("serialize canonical finding index: {error}"))
    })?;
    if byte_count != encoded.len() as u64 {
        return Err(ProducerAdmissionError::malformed(
            "canonical finding index byte count is contradictory",
        ));
    }
    Ok(projection)
}
fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, ProducerAdmissionError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| {
            ProducerAdmissionError::malformed(format!("producer field {field} is missing"))
        })
}
fn required_value<'a>(value: &'a Value, field: &str) -> Result<&'a Value, ProducerAdmissionError> {
    value.get(field).ok_or_else(|| {
        ProducerAdmissionError::malformed(format!("producer field {field} is missing"))
    })
}
fn require_equal(field: &str, actual: &str, expected: &str) -> Result<(), ProducerAdmissionError> {
    if actual == expected {
        Ok(())
    } else {
        Err(ProducerAdmissionError::mismatch(field))
    }
}
fn resolve_revision(
    root: &Path,
    revision: &str,
    kind: &str,
) -> Result<String, ProducerAdmissionError> {
    let object = format!("{revision}^{{{kind}}}");
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--verify", &object])
        .output()
        .map_err(|error| {
            ProducerAdmissionError::malformed(format!(
                "resolve producer revision {revision:?} failed: {error}"
            ))
        })?;
    if !output.status.success() {
        return Err(ProducerAdmissionError::malformed(format!(
            "resolve producer revision {revision:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let value = String::from_utf8(output.stdout)
        .map_err(|error| {
            ProducerAdmissionError::malformed(format!("revision identity is not UTF-8: {error}"))
        })?
        .trim()
        .to_string();
    if value.is_empty() {
        Err(ProducerAdmissionError::malformed(
            "revision identity is empty",
        ))
    } else {
        Ok(value)
    }
}
fn repository_identity(root: &Path) -> String {
    let origin = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| output.stdout)
        .filter(|origin| !origin.iter().all(u8::is_ascii_whitespace));
    origin_identity(origin.as_deref())
}
fn origin_identity(origin: Option<&[u8]>) -> String {
    match origin.filter(|origin| !origin.iter().all(u8::is_ascii_whitespace)) {
        Some(origin) => format!("sha256:{:x}", Sha256::digest(origin)),
        None => "unavailable".to_string(),
    }
}
fn logical_path(path: &Path) -> String {
    let textual = crate::output::outcome::display_path(path).replace('\\', "/");
    let textual = textual.strip_prefix("//?/").unwrap_or(&textual);
    let display = Path::new(textual)
        .canonicalize()
        .map(|canonical| crate::output::outcome::display_path(&canonical))
        .unwrap_or_else(|_| textual.to_string())
        .replace('\\', "/");
    let display = display.strip_prefix("//?/").unwrap_or(&display);
    display.strip_suffix("/.").unwrap_or(display).to_string()
}
fn digest_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn producer_mode_mismatch_is_rejected_before_subject_or_analysis() -> Result<(), String> {
        let path = std::env::temp_dir().join(format!(
            "ripr-review-mode-mismatch-{}.json",
            std::process::id()
        ));
        let producer = serde_json::json!({
            "schema_version": "0.2",
            "tool": "ripr",
            "mode": "fast",
            "root": ".",
            "base": "main"
        });
        std::fs::write(
            &path,
            serde_json::to_vec(&producer).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("write producer fixture: {error}"))?;
        let subject_path = path.with_extension("subject.json");
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        let subject = serde_json::json!({
            "schema_version": "ripr.pr_check_subject.v1",
            "root_identity": logical_path(&root),
            "base_sha": resolve_revision(&root, "HEAD", "commit").unwrap_or_default(),
            "head_sha": resolve_revision(&root, "HEAD", "commit").unwrap_or_default(),
            "head_tree": resolve_revision(&root, "HEAD", "tree").unwrap_or_default(),
            "check_schema": "0.2",
            "canonical_diff_sha256": digest_bytes(b"fixture diff"),
            "configuration_fingerprint":
                crate::config::repo_exposure_config_identity_hash(&RiprConfig::default()),
            "analyzer_generation": crate::review_input::REVIEW_ANALYZER_GENERATION,
            "mode": "fast"
        });
        std::fs::write(
            &subject_path,
            serde_json::to_vec(&subject).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("write subject fixture: {error}"))?;
        let error = admit_producer_evidence(
            &path,
            &CheckInput::default(),
            &RiprConfig::default(),
            "HEAD",
            "HEAD",
            "fixture diff",
        )
        .err()
        .ok_or_else(|| "wrong producer mode must fail".to_string())?;
        if error.category != "producer_mode_mismatch" {
            return Err(format!("unexpected category: {}", error.category));
        }
        std::fs::remove_file(&path).map_err(|error| format!("remove producer fixture: {error}"))?;
        std::fs::remove_file(&subject_path)
            .map_err(|error| format!("remove subject fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn malformed_and_missing_producers_are_input_failures() -> Result<(), String> {
        let path =
            std::env::temp_dir().join(format!("ripr-review-malformed-{}.json", std::process::id()));
        std::fs::write(&path, b"{")
            .map_err(|error| format!("write malformed producer: {error}"))?;
        let missing_path = path.with_extension("missing.json");
        for (candidate, expected) in [
            (&path, "missing_producer"),
            (&missing_path, "missing_producer"),
        ] {
            let error = admit_producer_evidence(
                candidate,
                &CheckInput::default(),
                &RiprConfig::default(),
                "main",
                "HEAD",
                "fixture diff",
            )
            .err()
            .ok_or_else(|| "invalid producer must fail".to_string())?;
            if error.category != expected {
                return Err(format!("unexpected category: {}", error.category));
            }
        }
        std::fs::remove_file(&path)
            .map_err(|error| format!("remove malformed producer: {error}"))?;
        Ok(())
    }

    #[test]
    fn admission_helper_identity_functions_are_deterministic() -> Result<(), String> {
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        let repository = repository_identity(&root);
        if !repository.starts_with("sha256:") {
            return Err(format!("unexpected repository identity: {repository}"));
        }
        let logical = logical_path(Path::new("target\\ripr\\check.json"));
        if logical != "target/ripr/check.json" {
            return Err(format!("unexpected logical path: {logical}"));
        }
        let first = digest_bytes(b"identity");
        let second = digest_bytes(b"identity");
        if first != second || !first.starts_with("sha256:") {
            return Err("digest identity was not stable".to_string());
        }
        if required_string(&serde_json::json!({"field": "value"}), "field")
            .map_err(|error| error.message)?
            != "value"
        {
            return Err("required string did not return its value".to_string());
        }
        if required_string(&serde_json::json!({"field": ""}), "field").is_ok()
            || required_string(&serde_json::json!({}), "field").is_ok()
        {
            return Err("empty or missing strings must fail closed".to_string());
        }
        if require_equal("field", "same", "same").is_err()
            || require_equal("field", "actual", "expected").is_ok()
        {
            return Err("identity equality helper violated its contract".to_string());
        }
        Ok(())
    }

    #[test]
    fn logical_path_canonicalizes_existing_root_identity() -> Result<(), String> {
        let current = std::env::current_dir().map_err(|error| error.to_string())?;
        let expected = current.display().to_string().replace('\\', "/");
        let actual = logical_path(Path::new("."));
        if actual != expected {
            return Err(format!(
                "existing root identity was not canonicalized: expected {expected:?}, got {actual:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn admission_helpers_fail_closed_for_unavailable_and_non_regular_inputs() -> Result<(), String>
    {
        let root = std::env::temp_dir()
            .join("..")
            .join("..")
            .join("..")
            .join(format!(
                "ripr-review-helper-boundaries-{}",
                std::process::id()
            ));
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;

        if repository_identity(&root) != "unavailable" {
            return Err("non-repository origin must remain unavailable".to_string());
        }
        let nonexistent = root.join("missing").join("..").join("review-input.json");
        if !logical_path(&nonexistent).contains("review-input.json") {
            return Err("nonexistent logical path lost its filename".to_string());
        }

        let review_input_path = root.join("review-input.json");
        std::fs::create_dir(&review_input_path).map_err(|error| error.to_string())?;
        let identity = ReviewAnalysisIdentity {
            schema_version: REVIEW_ANALYSIS_IDENTITY_SCHEMA.to_string(),
            repository_identity: "unavailable".to_string(),
            root: logical_path(&root),
            base_sha: "base".to_string(),
            head_sha: "head".to_string(),
            head_tree: "tree".to_string(),
            canonical_diff_sha256: "sha256:diff".to_string(),
            mode: "draft".to_string(),
            configuration_fingerprint: "sha256:config".to_string(),
            producer_schema: "0.2".to_string(),
            analyzer_generation: crate::review_input::REVIEW_ANALYZER_GENERATION.to_string(),
            check_sha256: "sha256:check".to_string(),
        };
        let error = admit_review_input(&review_input_path, &root, &identity, 0, None)
            .err()
            .ok_or_else(|| "directory review input must fail closed".to_string())?;
        if error.category != "malformed_producer" {
            return Err(format!(
                "directory review input had unexpected category: {}",
                error.category
            ));
        }

        std::fs::remove_dir(&review_input_path).map_err(|error| error.to_string())?;
        std::fs::remove_dir(&root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn timeout_wrapper_preserves_success_and_reports_deadline() -> Result<(), String> {
        if run_analysis_with_timeout(5_000, || Ok::<_, String>(7))
            .map_err(|error| error.to_string())?
            != 7
        {
            return Err("successful work did not complete".to_string());
        }
        let timed_out = run_analysis_with_timeout(1, || {
            std::thread::sleep(Duration::from_millis(10));
            Ok::<_, String>(())
        });
        if timed_out.is_ok() {
            return Err("deadline did not cancel slow work".to_string());
        }
        Ok(())
    }

    #[test]
    fn admission_rejects_missing_required_fields_as_malformed() -> Result<(), String> {
        let path = std::env::temp_dir().join(format!(
            "ripr-review-required-fields-{}.json",
            std::process::id()
        ));
        for producer in [
            serde_json::json!({}),
            serde_json::json!({"schema_version": "0.2"}),
            serde_json::json!({"schema_version": "0.2", "tool": "ripr"}),
            serde_json::json!({"schema_version": "0.2", "tool": "ripr", "mode": "draft"}),
        ] {
            std::fs::write(
                &path,
                serde_json::to_vec(&producer).map_err(|error| error.to_string())?,
            )
            .map_err(|error| format!("write producer fixture: {error}"))?;
            let error = admit_producer_evidence(
                &path,
                &CheckInput::default(),
                &RiprConfig::default(),
                "main",
                "HEAD",
                "fixture diff",
            )
            .err()
            .ok_or_else(|| "missing required field must fail".to_string())?;
            if error.category != "missing_producer" {
                return Err(format!("unexpected category: {}", error.category));
            }
        }
        std::fs::remove_file(&path).map_err(|error| format!("remove producer fixture: {error}"))?;
        Ok(())
    }

    #[test]
    fn complete_producer_is_admitted_with_exact_subject_identity() -> Result<(), String> {
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        // Hosted test jobs use a depth-one checkout; keep the fixture
        // independent of unavailable parent objects while still exercising
        // exact commit/tree subject binding.
        let base = "HEAD";
        let head = "HEAD";
        let diff_text = "fixture diff";
        let root_identity = logical_path(&root);
        let configuration_fingerprint =
            crate::config::repo_exposure_config_identity_hash(&RiprConfig::default());
        let outcome = AnalysisOutcome::new(
            crate::analysis_outcome::AnalysisOutcomeKind::NoScope,
            crate::analysis_outcome::AnalysisIdentity {
                base_revision: Some(base.to_string()),
                input_identity: Some(digest_bytes(diff_text.as_bytes())),
                ..Default::default()
            },
            Default::default(),
            Vec::new(),
        )
        .map_err(|error| format!("build fixture outcome: {error}"))?;
        let check_path = root.join(format!(
            "target/ripr-review-admission-{}.json",
            std::process::id()
        ));
        let subject_path = check_path.with_extension("subject.json");
        let producer = serde_json::json!({
            "schema_version": "0.2",
            "tool": "ripr",
            "mode": "draft",
            "root": root_identity,
            "base": base,
            "summary": {},
            "findings": [],
            "analysis_outcome": {
                "analysis_complete": true,
                "outcome": outcome,
            },
        });
        let check_bytes = serde_json::to_vec(&producer).map_err(|error| error.to_string())?;
        std::fs::create_dir_all(
            check_path
                .parent()
                .ok_or_else(|| "fixture parent missing".to_string())?,
        )
        .map_err(|error| format!("create fixture parent: {error}"))?;
        std::fs::write(&check_path, &check_bytes)
            .map_err(|error| format!("write producer fixture: {error}"))?;
        let base_sha =
            resolve_revision(&root, base, "commit").map_err(|error| error.message.clone())?;
        let head_sha =
            resolve_revision(&root, head, "commit").map_err(|error| error.message.clone())?;
        let head_tree =
            resolve_revision(&root, head, "tree").map_err(|error| error.message.clone())?;
        let subject = serde_json::json!({
            "schema_version": "ripr.pr_check_subject.v1",
            "check_schema": "0.2",
            "root_identity": root_identity,
            "base_sha": base_sha,
            "head_sha": head_sha,
            "head_tree": head_tree,
            "canonical_diff_sha256": digest_bytes(diff_text.as_bytes()),
            "configuration_fingerprint": configuration_fingerprint,
            "analyzer_generation": crate::review_input::REVIEW_ANALYZER_GENERATION,
            "check_sha256": digest_bytes(&check_bytes),
            "mode": "draft",
            "analysis_outcome": {"analysis_complete": true, "outcome": outcome},
            "canonical_finding_index": {
                "schema_version": crate::review_input::REVIEW_INDEX_SCHEMA_VERSION,
                "total_finding_count": 0,
                "index_sha256": digest_bytes(b"[]"),
                "entries": []
            },
            "canonical_finding_index_entry_count": 0,
            "canonical_finding_index_byte_count": 2,
        });
        let review_input_path = check_path.with_file_name("review-input.json");
        let review_input = serde_json::json!({
            "schema_version": crate::review_input::REVIEW_INPUT_SCHEMA_VERSION,
            "root_identity": root_identity,
            "base_sha": subject["base_sha"],
            "head_sha": subject["head_sha"],
            "head_tree": subject["head_tree"],
            "check_sha256": subject["check_sha256"],
            "canonical_diff_sha256": digest_bytes(diff_text.as_bytes()),
            "mode": "draft",
            "analysis_complete": true,
            "total_finding_count": 0,
            "projected_finding_count": 0,
            "projection_limit": crate::review_input::REVIEW_INPUT_PROJECTION_LIMIT,
            "projection_truncated": false,
            "projection_selection_policy": crate::review_input::REVIEW_INPUT_SELECTION_POLICY,
            "projection_selection_policy_version": crate::review_input::REVIEW_INPUT_SELECTION_POLICY_VERSION,
            "reviewed_count": 0,
            "projection_sha256": digest_bytes(b"[]"),
            "findings": [],
            "analysis_outcome": {
                "analysis_complete": true,
                "outcome": outcome,
            },
        });
        let review_input_bytes = serde_json::to_vec(&review_input)
            .map_err(|error| format!("serialize review input fixture: {error}"))?;
        let mut subject = subject;
        subject["review_input_sha256"] = serde_json::json!(digest_bytes(&review_input_bytes));
        subject["review_input_byte_count"] = serde_json::json!(review_input_bytes.len());
        std::fs::write(
            &subject_path,
            serde_json::to_vec(&subject).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("write subject fixture: {error}"))?;
        let original_subject_bytes =
            serde_json::to_vec(&subject).map_err(|error| error.to_string())?;
        std::fs::write(&review_input_path, &review_input_bytes)
            .map_err(|error| format!("write review input fixture: {error}"))?;

        let admitted = admit_producer_evidence(
            &check_path,
            &CheckInput::default(),
            &RiprConfig::default(),
            base,
            head,
            diff_text,
        )
        .map_err(|error| error.message)?;
        if admitted.identity.mode != "draft"
            || admitted.identity.base_sha != base_sha
            || admitted.identity.head_sha != head_sha
            || admitted.identity.head_tree != head_tree
        {
            return Err("admitted identity did not retain the exact subject".to_string());
        }

        std::fs::remove_file(&subject_path).map_err(|error| error.to_string())?;
        std::fs::create_dir(&subject_path)
            .map_err(|error| format!("create unreadable subject fixture: {error}"))?;
        let unreadable_subject = admit_producer_evidence(
            &check_path,
            &CheckInput::default(),
            &RiprConfig::default(),
            base,
            head,
            diff_text,
        )
        .err()
        .ok_or_else(|| "directory subject receipt must fail".to_string())?;
        if unreadable_subject.category != "malformed_producer" {
            return Err(format!(
                "directory subject receipt returned {}",
                unreadable_subject.category
            ));
        }
        std::fs::remove_dir(&subject_path).map_err(|error| error.to_string())?;
        std::fs::write(&subject_path, &original_subject_bytes)
            .map_err(|error| format!("restore subject after unreadable case: {error}"))?;

        std::fs::remove_file(&review_input_path).map_err(|error| error.to_string())?;
        std::fs::create_dir(&review_input_path)
            .map_err(|error| format!("create unreadable review input: {error}"))?;
        let unreadable_review_input = admit_producer_evidence(
            &check_path,
            &CheckInput::default(),
            &RiprConfig::default(),
            base,
            head,
            diff_text,
        )
        .err()
        .ok_or_else(|| "directory review input must fail".to_string())?;
        if unreadable_review_input.category != "malformed_producer" {
            return Err(format!(
                "directory review input returned {}",
                unreadable_review_input.category
            ));
        }
        std::fs::remove_dir(&review_input_path).map_err(|error| error.to_string())?;
        std::fs::write(&review_input_path, &review_input_bytes)
            .map_err(|error| format!("restore review input after unreadable case: {error}"))?;

        for (name, mutation) in [
            ("invalid-index", serde_json::json!("invalid")),
            ("invalid-index-entry-count", serde_json::json!(1)),
            ("invalid-index-byte-count", serde_json::json!(0)),
        ] {
            let mut mutated = subject.clone();
            match name {
                "invalid-index" => mutated["canonical_finding_index"] = mutation,
                "invalid-index-entry-count" => {
                    mutated["canonical_finding_index_entry_count"] = mutation
                }
                _ => mutated["canonical_finding_index_byte_count"] = mutation,
            }
            std::fs::write(
                &subject_path,
                serde_json::to_vec(&mutated).map_err(|error| error.to_string())?,
            )
            .map_err(|error| format!("write {name} subject: {error}"))?;
            let error = admit_producer_evidence(
                &check_path,
                &CheckInput::default(),
                &RiprConfig::default(),
                base,
                head,
                diff_text,
            )
            .err()
            .ok_or_else(|| format!("{name} must fail"))?;
            if error.category != "malformed_producer" {
                return Err(format!("{name} returned {}", error.category));
            }
        }
        std::fs::write(&subject_path, &original_subject_bytes)
            .map_err(|error| format!("restore subject after index mutations: {error}"))?;

        std::fs::write(&review_input_path, b"{")
            .map_err(|error| format!("write malformed review input: {error}"))?;
        let malformed_review_input = admit_producer_evidence(
            &check_path,
            &CheckInput::default(),
            &RiprConfig::default(),
            base,
            head,
            diff_text,
        )
        .err()
        .ok_or_else(|| "malformed review input must fail".to_string())?;
        if malformed_review_input.category != "malformed_producer" {
            return Err(format!(
                "malformed review input returned {}",
                malformed_review_input.category
            ));
        }
        std::fs::write(&review_input_path, &review_input_bytes)
            .map_err(|error| format!("restore review input: {error}"))?;

        let mut invalid_byte_count = subject.clone();
        invalid_byte_count["review_input_byte_count"] = serde_json::json!(0);
        std::fs::write(
            &subject_path,
            serde_json::to_vec(&invalid_byte_count).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("write invalid byte-count subject: {error}"))?;
        let invalid_byte_count_error = admit_producer_evidence(
            &check_path,
            &CheckInput::default(),
            &RiprConfig::default(),
            base,
            head,
            diff_text,
        )
        .err()
        .ok_or_else(|| "invalid review-input byte count must fail".to_string())?;
        if invalid_byte_count_error.category != "malformed_producer" {
            return Err(format!(
                "invalid review-input byte count returned {}",
                invalid_byte_count_error.category
            ));
        }

        for (name, mutation, expected_category) in [
            (
                "missing-outcome",
                serde_json::json!({}),
                "malformed_producer",
            ),
            (
                "invalid-outcome",
                serde_json::json!("invalid"),
                "malformed_producer",
            ),
            (
                "incomplete-outcome",
                serde_json::json!("limited_timeout"),
                "malformed_producer",
            ),
        ] {
            let mut mutated = subject.clone();
            mutated["analysis_outcome"]["outcome"] = mutation;
            std::fs::write(
                &subject_path,
                serde_json::to_vec(&mutated).map_err(|error| error.to_string())?,
            )
            .map_err(|error| format!("write {name} subject: {error}"))?;
            let error = admit_producer_evidence(
                &check_path,
                &CheckInput::default(),
                &RiprConfig::default(),
                base,
                head,
                diff_text,
            )
            .err()
            .ok_or_else(|| format!("{name} must fail"))?;
            if error.category != expected_category {
                return Err(format!("{name} returned {}", error.category));
            }
        }

        let mut mismatched_outcome = subject.clone();
        mismatched_outcome["analysis_outcome"]["outcome"]["kind"] =
            serde_json::json!("complete_with_findings");
        mismatched_outcome["analysis_outcome"]["outcome"]["counts"]["finding_count"] =
            serde_json::json!(1);
        std::fs::write(
            &subject_path,
            serde_json::to_vec(&mismatched_outcome).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("write mismatched-outcome subject: {error}"))?;
        let mismatched_outcome_error = admit_producer_evidence(
            &check_path,
            &CheckInput::default(),
            &RiprConfig::default(),
            base,
            head,
            diff_text,
        )
        .err()
        .ok_or_else(|| "mismatched outcome must fail".to_string())?;
        if mismatched_outcome_error.category != "incomplete_producer" {
            return Err(format!(
                "mismatched outcome returned {}",
                mismatched_outcome_error.category
            ));
        }

        std::fs::write(&subject_path, &original_subject_bytes)
            .map_err(|error| format!("restore subject before cleanup: {error}"))?;
        std::fs::remove_file(&check_path).map_err(|error| error.to_string())?;
        admit_producer_evidence(
            &check_path,
            &CheckInput::default(),
            &RiprConfig::default(),
            base,
            head,
            diff_text,
        )
        .map_err(|error| {
            format!(
                "consumer unexpectedly required check.json: {}",
                error.message
            )
        })?;

        let original_subject = subject.clone();
        let mut incomplete_input = review_input.clone();
        incomplete_input["analysis_complete"] = serde_json::json!(false);
        incomplete_input["analysis_outcome"]["analysis_complete"] = serde_json::json!(false);
        let incomplete_bytes = serde_json::to_vec(&incomplete_input)
            .map_err(|error| format!("serialize incomplete review input: {error}"))?;
        let mut incomplete_subject = subject.clone();
        incomplete_subject["review_input_sha256"] =
            serde_json::json!(digest_bytes(&incomplete_bytes));
        incomplete_subject["review_input_byte_count"] = serde_json::json!(incomplete_bytes.len());
        std::fs::write(
            &subject_path,
            serde_json::to_vec(&incomplete_subject).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("write incomplete subject: {error}"))?;
        std::fs::write(&review_input_path, &incomplete_bytes)
            .map_err(|error| format!("write incomplete review input: {error}"))?;
        let incomplete = admit_producer_evidence(
            &check_path,
            &CheckInput::default(),
            &RiprConfig::default(),
            base,
            head,
            diff_text,
        )
        .err()
        .ok_or_else(|| "incomplete producer must fail closed".to_string())?;
        if incomplete.category != "incomplete_producer" {
            return Err(format!(
                "unexpected incomplete-producer category: {}",
                incomplete.category
            ));
        }
        std::fs::write(
            &subject_path,
            serde_json::to_vec(&original_subject).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("restore subject fixture: {error}"))?;
        std::fs::write(&review_input_path, &review_input_bytes)
            .map_err(|error| format!("restore review input fixture: {error}"))?;
        for (field, value) in [
            ("subject_schema_version", serde_json::json!("unknown")),
            ("base_sha", serde_json::json!("other")),
            ("head_sha", serde_json::json!("other")),
            ("head_tree", serde_json::json!("other")),
            ("mode", serde_json::json!("fast")),
            ("review_input_sha256", serde_json::json!("sha256:other")),
            ("review_input_byte_count", serde_json::json!("not-a-count")),
        ] {
            let mut mutated = original_subject.clone();
            let key = if field == "subject_schema_version" {
                "schema_version"
            } else {
                field
            };
            mutated[key] = value;
            std::fs::write(
                &subject_path,
                serde_json::to_vec(&mutated).map_err(|error| error.to_string())?,
            )
            .map_err(|error| format!("write subject mutation {field}: {error}"))?;
            let error = admit_producer_evidence(
                &check_path,
                &CheckInput::default(),
                &RiprConfig::default(),
                base,
                head,
                diff_text,
            )
            .err()
            .ok_or_else(|| format!("subject mutation {field} must fail"))?;
            let expected_category = if field == "mode" {
                "producer_mode_mismatch"
            } else if field == "review_input_byte_count" {
                "malformed_producer"
            } else {
                "producer_identity_mismatch"
            };
            if error.category != expected_category {
                return Err(format!(
                    "subject mutation {field} returned {}",
                    error.category
                ));
            }
        }
        std::fs::write(
            &subject_path,
            serde_json::to_vec(&original_subject).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("restore subject before cleanup: {error}"))?;
        std::fs::remove_file(&subject_path).map_err(|error| error.to_string())?;
        let missing_subject = admit_producer_evidence(
            &check_path,
            &CheckInput::default(),
            &RiprConfig::default(),
            base,
            head,
            diff_text,
        )
        .err()
        .ok_or_else(|| "missing subject receipt must fail closed".to_string())?;
        if missing_subject.category != "missing_producer" {
            return Err(format!(
                "unexpected missing-subject category: {}",
                missing_subject.category
            ));
        }
        std::fs::write(&subject_path, b"{").map_err(|error| error.to_string())?;
        let malformed_subject = admit_producer_evidence(
            &check_path,
            &CheckInput::default(),
            &RiprConfig::default(),
            base,
            head,
            diff_text,
        )
        .err()
        .ok_or_else(|| "malformed subject receipt must fail closed".to_string())?;
        if malformed_subject.category != "malformed_producer" {
            return Err(format!(
                "unexpected malformed-subject category: {}",
                malformed_subject.category
            ));
        }
        let _ = std::fs::remove_file(&check_path);
        std::fs::remove_file(&subject_path).map_err(|error| error.to_string())?;
        std::fs::remove_file(&review_input_path).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn review_input_admission_rejects_stale_incomplete_and_malformed_packets() -> Result<(), String>
    {
        let path = std::env::temp_dir().join(format!(
            "ripr-review-input-admission-{}.json",
            std::process::id()
        ));
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        let producer_findings = vec![serde_json::json!({
            "id": "finding-1",
            "probe": {"file": "Cargo.toml", "line": 1},
            "severity": "warning",
            "classification": "exposed",
            "related_tests": []
        })];
        let producer_projection =
            crate::review_input::canonical_projection(&producer_findings, &root)?;
        let findings = serde_json::to_value(crate::review_input::canonical_projection(
            &producer_findings,
            &root,
        )?)
        .map_err(|error| error.to_string())?;
        let projection_sha256 =
            digest_bytes(&serde_json::to_vec(&findings).map_err(|error| error.to_string())?);
        let identity = ReviewAnalysisIdentity {
            schema_version: REVIEW_ANALYSIS_IDENTITY_SCHEMA.to_string(),
            repository_identity: "repository".to_string(),
            root: logical_path(&root),
            base_sha: "base".to_string(),
            head_sha: "head".to_string(),
            head_tree: "tree".to_string(),
            canonical_diff_sha256: "diff".to_string(),
            mode: "draft".to_string(),
            configuration_fingerprint: "config".to_string(),
            producer_schema: "0.2".to_string(),
            analyzer_generation: "analyzer".to_string(),
            check_sha256: "check".to_string(),
        };
        let valid = serde_json::json!({
            "schema_version": "ripr.review_input.v1",
            "root_identity": logical_path(&root),
            "base_sha": "base",
            "head_sha": "head",
            "head_tree": "tree",
            "check_sha256": "check",
            "canonical_diff_sha256": "diff",
            "mode": "draft",
            "findings": findings,
            "reviewed_count": 1,
            "analysis_complete": true,
            "total_finding_count": 1,
            "projected_finding_count": 1,
            "projection_limit": 10,
            "projection_truncated": false,
            "projection_selection_policy": "severity_actionability_stable_id_path_line",
            "projection_selection_policy_version": "v1",
            "projection_sha256": projection_sha256,
        });
        std::fs::write(
            &path,
            serde_json::to_vec(&valid).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("write valid review input: {error}"))?;
        let admitted = admit_review_input(&path, &root, &identity, 1, Some(&producer_projection))
            .map_err(|error| error.message.clone())?;
        if admitted.reviewed_count != 1 || admitted.projection_sha256 != projection_sha256 {
            return Err("valid review input was not admitted".to_string());
        }
        std::fs::remove_file(&path).map_err(|error| error.to_string())?;
        let missing = admit_review_input(&path, &root, &identity, 1, None)
            .err()
            .ok_or_else(|| "missing review input must fail".to_string())?;
        if missing.category != "missing_producer" {
            return Err(format!(
                "unexpected missing review-input category: {}",
                missing.category
            ));
        }

        let mut cases = Vec::new();
        cases.push(("not-json", serde_json::json!("{"), "malformed_producer"));
        cases.push((
            "wrong-schema",
            serde_json::json!({"schema_version": "wrong"}),
            "producer_identity_mismatch",
        ));
        cases.push((
            "wrong-root",
            serde_json::json!({"schema_version": "ripr.review_input.v1", "root_identity": "other"}),
            "producer_identity_mismatch",
        ));
        cases.push((
            "missing-findings",
            serde_json::json!({"schema_version": "ripr.review_input.v1", "root_identity": logical_path(&root)}),
            "malformed_producer",
        ));
        for (name, value, expected_category) in cases {
            let bytes = if name == "not-json" {
                value.as_str().unwrap_or_default().as_bytes().to_vec()
            } else {
                serde_json::to_vec(&value).map_err(|error| error.to_string())?
            };
            std::fs::write(&path, bytes).map_err(|error| format!("write {name}: {error}"))?;
            let error = admit_review_input(&path, &root, &identity, 1, None)
                .err()
                .ok_or_else(|| format!("{name} must fail"))?;
            if error.category != expected_category {
                return Err(format!("{name} returned {}", error.category));
            }
        }

        for field in [
            "schema_version",
            "root_identity",
            "base_sha",
            "head_sha",
            "head_tree",
            "check_sha256",
            "canonical_diff_sha256",
            "mode",
            "findings",
            "reviewed_count",
            "analysis_complete",
            "total_finding_count",
            "projected_finding_count",
            "projection_limit",
            "projection_truncated",
            "projection_selection_policy",
            "projection_selection_policy_version",
            "projection_sha256",
        ] {
            let mut mutation = valid.clone();
            let Some(object) = mutation.as_object_mut() else {
                return Err("valid review input fixture must be an object".to_string());
            };
            if object.remove(field).is_none() {
                return Err(format!("valid review input fixture lacks {field}"));
            }
            std::fs::write(
                &path,
                serde_json::to_vec(&mutation).map_err(|error| error.to_string())?,
            )
            .map_err(|error| format!("write missing field {field}: {error}"))?;
            let error = admit_review_input(&path, &root, &identity, 1, None)
                .err()
                .ok_or_else(|| format!("missing field {field} must fail"))?;
            if error.category != "malformed_producer" {
                return Err(format!("missing field {field} returned {}", error.category));
            }
        }

        let mut mutations = Vec::new();
        for (field, value, expected_category) in [
            (
                "base_sha",
                serde_json::json!("other"),
                "producer_identity_mismatch",
            ),
            ("mode", serde_json::json!("fast"), "producer_mode_mismatch"),
            ("reviewed_count", serde_json::json!(2), "malformed_producer"),
            (
                "analysis_complete",
                serde_json::json!(false),
                "incomplete_producer",
            ),
            (
                "total_finding_count",
                serde_json::json!(2),
                "malformed_producer",
            ),
            (
                "projected_finding_count",
                serde_json::json!(2),
                "malformed_producer",
            ),
            (
                "projection_limit",
                serde_json::json!(9),
                "malformed_producer",
            ),
            (
                "projection_truncated",
                serde_json::json!(true),
                "malformed_producer",
            ),
            (
                "projection_selection_policy",
                serde_json::json!("unstable"),
                "producer_identity_mismatch",
            ),
            (
                "projection_selection_policy_version",
                serde_json::json!("v2"),
                "producer_identity_mismatch",
            ),
            (
                "projection_sha256",
                serde_json::json!("sha256:wrong"),
                "producer_identity_mismatch",
            ),
        ] {
            let mut mutation = valid.clone();
            mutation[field] = value;
            mutations.push(field);
            std::fs::write(
                &path,
                serde_json::to_vec(&mutation).map_err(|error| error.to_string())?,
            )
            .map_err(|error| format!("write mutation {field}: {error}"))?;
            let error = admit_review_input(&path, &root, &identity, 1, None)
                .err()
                .ok_or_else(|| format!("mutation {field} must fail"))?;
            if error.category != expected_category {
                return Err(format!("mutation {field} returned {}", error.category));
            }
        }
        if mutations.len() != 11 {
            return Err("not all review-input mutations were exercised".to_string());
        }
        let mut substituted = valid.clone();
        substituted["findings"][0]["summary"] = serde_json::json!("substituted");
        let substituted_bytes =
            serde_json::to_vec(&substituted["findings"]).map_err(|error| error.to_string())?;
        substituted["projection_sha256"] = serde_json::json!(digest_bytes(&substituted_bytes));
        std::fs::write(
            &path,
            serde_json::to_vec(&substituted).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("write substituted projection: {error}"))?;
        let error = admit_review_input(&path, &root, &identity, 1, Some(&producer_projection))
            .err()
            .ok_or_else(|| "substituted projection must fail canonical comparison".to_string())?;
        if error.category != "malformed_producer" {
            return Err(format!(
                "substituted projection returned {}",
                error.category
            ));
        }
        std::fs::remove_file(&path).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn admission_helpers_cover_required_values_and_revision_failures() -> Result<(), String> {
        let missing = required_value(&serde_json::json!({}), "missing")
            .err()
            .ok_or_else(|| "missing required value must fail".to_string())?;
        if missing.category != "malformed_producer" {
            return Err(format!(
                "unexpected required-value category: {}",
                missing.category
            ));
        }

        let mismatch = require_equal("mode", "draft", "fast")
            .err()
            .ok_or_else(|| "unequal identity values must fail".to_string())?;
        if mismatch.category != "producer_mode_mismatch" {
            return Err(format!(
                "unexpected mode mismatch category: {}",
                mismatch.category
            ));
        }
        let identity_mismatch = require_equal("head_sha", "old", "new")
            .err()
            .ok_or_else(|| "unequal identity values must fail".to_string())?;
        if identity_mismatch.category != "producer_identity_mismatch" {
            return Err(format!(
                "unexpected identity mismatch category: {}",
                identity_mismatch.category
            ));
        }

        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        let revision_error = resolve_revision(&root, "definitely-not-a-revision", "commit")
            .err()
            .ok_or_else(|| "invalid revision must fail".to_string())?;
        if revision_error.category != "malformed_producer" {
            return Err(format!(
                "unexpected revision category: {}",
                revision_error.category
            ));
        }
        Ok(())
    }

    #[test]
    fn admission_helpers_reject_empty_strings_and_unavailable_origin() -> Result<(), String> {
        for value in [
            serde_json::json!({}),
            serde_json::json!({"field": ""}),
            serde_json::json!({"field": 7}),
        ] {
            let error = required_string(&value, "field")
                .err()
                .ok_or_else(|| "invalid required string must fail".to_string())?;
            if error.category != "malformed_producer" {
                return Err(format!(
                    "unexpected required-string category: {}",
                    error.category
                ));
            }
        }

        let unavailable =
            repository_identity(Path::new("target/ripr/review-comments-missing-origin"));
        if unavailable != "unavailable" {
            return Err(format!(
                "unavailable origin was identified as {unavailable}"
            ));
        }
        Ok(())
    }

    #[test]
    fn origin_identity_distinguishes_missing_blank_and_present_origins() -> Result<(), String> {
        if origin_identity(None) != "unavailable" || origin_identity(Some(b"\n\t")) != "unavailable"
        {
            return Err("missing or blank origins must be unavailable".to_string());
        }
        let present = origin_identity(Some(b"https://github.com/example/repo.git\n"));
        if !present.starts_with("sha256:") || present == "unavailable" {
            return Err("present origin must have a digest identity".to_string());
        }
        Ok(())
    }
}
