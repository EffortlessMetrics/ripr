//! Exact producer admission and same-run analysis reuse for PR review guidance.

use super::CheckInput;
use crate::analysis_outcome::AnalysisOutcome;
use crate::config::{RiprConfig, repo_exposure_config_identity_hash};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;

pub(crate) const REVIEW_ANALYSIS_IDENTITY_SCHEMA: &str = "ripr.review_analysis_identity.v1";
const REVIEW_ANALYZER_GENERATION: &str = "diff_scoped_classified_seams.v1";

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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProducerAdmissionError {
    pub(crate) category: &'static str,
    pub(crate) message: String,
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
    let check_bytes = std::fs::read(check_path).map_err(|error| {
        let message = format!(
            "producer check {} is unreadable: {error}",
            check_path.display()
        );
        if error.kind() == std::io::ErrorKind::NotFound {
            ProducerAdmissionError::missing(message)
        } else {
            ProducerAdmissionError::malformed(message)
        }
    })?;
    let producer: Value = serde_json::from_slice(&check_bytes).map_err(|error| {
        ProducerAdmissionError::malformed(format!(
            "producer check {} is invalid JSON: {error}",
            check_path.display()
        ))
    })?;
    let producer_schema = required_string(&producer, "schema_version")?;
    require_equal("schema_version", producer_schema, "0.2")?;
    require_equal("tool", required_string(&producer, "tool")?, "ripr")?;
    require_equal(
        "mode",
        required_string(&producer, "mode")?,
        input.mode.as_str(),
    )?;
    require_equal(
        "root",
        required_string(&producer, "root")?,
        &logical_path(&input.root),
    )?;
    require_equal("base", required_string(&producer, "base")?, base)?;
    if !producer.get("summary").is_some_and(Value::is_object) {
        return Err(ProducerAdmissionError::malformed(
            "producer summary must be an object",
        ));
    }
    let findings = producer
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| ProducerAdmissionError::malformed("producer findings must be an array"))?;
    let envelope = producer
        .get("analysis_outcome")
        .filter(|value| value.is_object())
        .ok_or_else(|| {
            ProducerAdmissionError::malformed("producer check requires analysis_outcome")
        })?;
    let declared_complete = envelope
        .get("analysis_complete")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            ProducerAdmissionError::malformed(
                "producer analysis_outcome.analysis_complete must be boolean",
            )
        })?;
    let outcome: AnalysisOutcome = serde_json::from_value(
        envelope
            .get("outcome")
            .cloned()
            .ok_or_else(|| ProducerAdmissionError::malformed("producer outcome is missing"))?,
    )
    .map_err(|error| {
        ProducerAdmissionError::malformed(format!("producer outcome is invalid: {error}"))
    })?;
    if !declared_complete || !outcome.kind.is_complete() {
        return Err(ProducerAdmissionError {
            category: "incomplete_producer",
            message: "producer analysis is not complete and cannot be reused".to_string(),
        });
    }
    if outcome.counts.finding_count != findings.len() as u64 {
        return Err(ProducerAdmissionError::malformed(
            "producer finding count contradicts its findings payload",
        ));
    }
    require_equal(
        "base_revision",
        outcome
            .identity
            .base_revision
            .as_deref()
            .unwrap_or_default(),
        base,
    )?;
    let canonical_diff_sha256 = digest_bytes(diff_text.as_bytes());
    require_equal(
        "canonical_diff_sha256",
        outcome
            .identity
            .input_identity
            .as_deref()
            .unwrap_or_default(),
        &canonical_diff_sha256,
    )?;

    let subject_path = check_path.with_extension("subject.json");
    let subject: Value =
        serde_json::from_slice(&std::fs::read(&subject_path).map_err(|error| {
            ProducerAdmissionError::malformed(format!(
                "producer subject receipt {} is unreadable: {error}",
                subject_path.display()
            ))
        })?)
        .map_err(|error| {
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
    let check_sha256 = digest_bytes(&check_bytes);
    for (field, expected) in [
        ("base_sha", base_sha.as_str()),
        ("head_sha", head_sha.as_str()),
        ("head_tree", head_tree.as_str()),
        ("check_sha256", check_sha256.as_str()),
    ] {
        require_equal(field, required_string(&subject, field)?, expected)?;
    }
    Ok(AdmittedReviewAnalysis {
        identity: ReviewAnalysisIdentity {
            schema_version: REVIEW_ANALYSIS_IDENTITY_SCHEMA.to_string(),
            repository_identity: repository_identity(&input.root),
            root: logical_path(&input.root),
            base_sha,
            head_sha,
            head_tree,
            canonical_diff_sha256,
            mode: input.mode.as_str().to_string(),
            configuration_fingerprint: repo_exposure_config_identity_hash(config),
            producer_schema: producer_schema.to_string(),
            analyzer_generation: REVIEW_ANALYZER_GENERATION.to_string(),
            check_sha256,
        },
        outcome,
    })
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
        .unwrap_or_default();
    format!("sha256:{:x}", Sha256::digest(origin))
}

fn logical_path(path: &Path) -> String {
    crate::output::outcome::display_path(path).replace('\\', "/")
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
        let error = admit_producer_evidence(
            &path,
            &CheckInput::default(),
            &RiprConfig::default(),
            "main",
            "HEAD",
            "fixture diff",
        )
        .err()
        .ok_or_else(|| "wrong producer mode must fail".to_string())?;
        if error.category != "producer_mode_mismatch" {
            return Err(format!("unexpected category: {}", error.category));
        }
        std::fs::remove_file(&path).map_err(|error| format!("remove producer fixture: {error}"))?;
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
            (&path, "malformed_producer"),
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
            if error.category != "malformed_producer" {
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
            "root": ".",
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
            "base_sha": base_sha,
            "head_sha": head_sha,
            "head_tree": head_tree,
            "check_sha256": digest_bytes(&check_bytes),
        });
        std::fs::write(
            &subject_path,
            serde_json::to_vec(&subject).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("write subject fixture: {error}"))?;

        let admitted = admit_producer_evidence(
            &check_path,
            &CheckInput::default(),
            &RiprConfig::default(),
            base,
            head,
            &diff_text,
        )
        .map_err(|error| error.message)?;
        if admitted.identity.mode != "draft"
            || admitted.identity.base_sha != base_sha
            || admitted.identity.head_sha != head_sha
            || admitted.identity.head_tree != head_tree
        {
            return Err("admitted identity did not retain the exact subject".to_string());
        }
        std::fs::remove_file(&check_path).map_err(|error| error.to_string())?;
        std::fs::remove_file(&subject_path).map_err(|error| error.to_string())?;
        Ok(())
    }
}
