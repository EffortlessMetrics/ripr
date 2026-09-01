//! Typed, bounded input exchanged between the PR producer and review-comments.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;

pub const REVIEW_INPUT_SCHEMA_VERSION: &str = "ripr.review_input.v1";
pub const REVIEW_INPUT_SELECTION_POLICY: &str = "severity_actionability_stable_id_path_line";
pub const REVIEW_INPUT_SELECTION_POLICY_VERSION: &str = "v1";
pub const REVIEW_INPUT_PROJECTION_LIMIT: u64 = 10;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReviewInputV1 {
    pub schema_version: String,
    pub root_identity: String,
    pub base_sha: String,
    pub head_sha: String,
    pub head_tree: String,
    pub check_sha256: String,
    pub canonical_diff_sha256: String,
    pub mode: String,
    pub analysis_complete: bool,
    pub total_finding_count: u64,
    pub projected_finding_count: u64,
    pub projection_limit: u64,
    pub projection_truncated: bool,
    pub projection_selection_policy: String,
    pub projection_selection_policy_version: String,
    pub reviewed_count: u64,
    pub projection_sha256: String,
    pub findings: Vec<ReviewFindingProjectionV1>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReviewFindingProjectionV1 {
    pub stable_id: String,
    pub file: String,
    pub line: Option<u64>,
    pub severity: String,
    pub finding_class: String,
    pub summary: String,
    pub evidence_digest: String,
    pub related_test: Option<ReviewRelatedTestProjectionV1>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReviewRelatedTestProjectionV1 {
    pub name: String,
    pub file: String,
    pub line: u64,
}

/// Derive the only projection that may be admitted for review-comments.
/// Missing or malformed producer fields are errors; they are never silently
/// filtered out of the bounded packet.
pub fn canonical_projection(
    findings: &[Value],
    root: &Path,
) -> Result<Vec<ReviewFindingProjectionV1>, String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("canonicalize review input root: {error}"))?;
    let mut projected = findings
        .iter()
        .map(|finding| {
            let probe = finding
                .get("probe")
                .and_then(Value::as_object)
                .ok_or_else(|| "producer finding probe must be an object".to_string())?;
            let file = probe
                .get("file")
                .and_then(Value::as_str)
                .ok_or_else(|| "producer finding probe.file must be a string".to_string())?;
            let line = probe
                .get("line")
                .and_then(Value::as_u64)
                .ok_or_else(|| "producer finding probe.line must be an integer".to_string())?;
            let stable_id = finding
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| "producer finding id must be a string".to_string())?;
            let severity = finding
                .get("severity")
                .and_then(Value::as_str)
                .ok_or_else(|| "producer finding severity must be a string".to_string())?;
            let finding_class = finding
                .get("classification")
                .and_then(Value::as_str)
                .ok_or_else(|| "producer finding classification must be a string".to_string())?;
            let related_test = match finding.get("related_tests") {
                None | Some(Value::Null) => None,
                Some(Value::Array(tests)) => tests.first().map(|test| {
                    Ok::<ReviewRelatedTestProjectionV1, String>(ReviewRelatedTestProjectionV1 {
                        name: test
                            .get("name")
                            .and_then(Value::as_str)
                            .ok_or_else(|| "related test name must be a string".to_string())?
                            .to_string(),
                        file: test
                            .get("file")
                            .and_then(Value::as_str)
                            .ok_or_else(|| "related test file must be a string".to_string())?
                            .to_string(),
                        line: test
                            .get("line")
                            .and_then(Value::as_u64)
                            .ok_or_else(|| "related test line must be an integer".to_string())?,
                    })
                }),
                Some(_) => {
                    return Err("producer finding related_tests must be an array".to_string());
                }
            }
            .transpose()?;
            let file_path = Path::new(file);
            let absolute_file = if file_path.is_absolute() {
                file_path.to_path_buf()
            } else {
                root.join(file_path)
            };
            let relative_file = absolute_file
                .canonicalize()
                .map_err(|error| format!("canonicalize producer finding file: {error}"))?
                .strip_prefix(&root)
                .map_err(|error| format!("producer finding file escapes root: {error}"))?
                .display()
                .to_string()
                .replace('\\', "/");
            Ok(ReviewFindingProjectionV1 {
                stable_id: stable_id.to_string(),
                file: relative_file,
                line: Some(line),
                severity: severity.to_string(),
                finding_class: finding_class.to_string(),
                summary: projection_summary(finding).to_string(),
                evidence_digest: format!(
                    "sha256:{:x}",
                    Sha256::digest(
                        serde_json::to_vec(finding)
                            .map_err(|error| format!("serialize finding digest: {error}"))?,
                    )
                ),
                related_test,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    projected.sort_by_key(projection_order);
    projected.truncate(REVIEW_INPUT_PROJECTION_LIMIT as usize);
    Ok(projected)
}

pub fn projection_summary(finding: &Value) -> &str {
    finding
        .get("suggested_next_action")
        .and_then(Value::as_str)
        .filter(|summary| !summary.trim().is_empty())
        .or_else(|| {
            finding
                .get("recommended_next_step")
                .and_then(Value::as_str)
                .filter(|summary| !summary.trim().is_empty())
        })
        .unwrap_or("Inspect the producer-owned review finding.")
}

fn projection_order(value: &ReviewFindingProjectionV1) -> (u8, u8, String, String, u64) {
    let severity = match value.severity.as_str() {
        "critical" => 0,
        "error" => 1,
        "warning" => 2,
        "note" => 3,
        _ => 4,
    };
    let actionability = match value.finding_class.as_str() {
        "exposed" => 0,
        "weakly_exposed" | "weakly_gripped" => 1,
        _ => 2,
    };
    (
        severity,
        actionability,
        value.stable_id.clone(),
        value.file.clone(),
        value.line.unwrap_or(u64::MAX),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding() -> Value {
        serde_json::json!({
            "id": "finding-1",
            "probe": {"file": "Cargo.toml", "line": 1},
            "severity": "warning",
            "classification": "exposed",
            "suggested_next_action": "Inspect the finding.",
            "related_tests": []
        })
    }

    #[test]
    fn canonical_projection_rejects_unprojectable_findings() -> Result<(), String> {
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        for (name, mutation) in [
            ("probe", serde_json::json!(null)),
            ("id", serde_json::json!(null)),
            ("severity", serde_json::json!(null)),
            ("classification", serde_json::json!(null)),
            ("related_tests", serde_json::json!("not-an-array")),
        ] {
            let mut value = finding();
            value[name] = mutation;
            if canonical_projection(&[value], &root).is_ok() {
                return Err(format!("{name} mutation was accepted"));
            }
        }
        Ok(())
    }

    #[test]
    fn canonical_projection_rejects_malformed_related_tests_and_paths() -> Result<(), String> {
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        let mut related = finding();
        related["related_tests"] = serde_json::json!([{"name": "test", "file": "Cargo.toml"}]);
        if canonical_projection(&[related], &root).is_ok() {
            return Err("related test missing line was accepted".to_string());
        }
        let mut escaping = finding();
        escaping["probe"]["file"] = serde_json::json!("../outside");
        if canonical_projection(&[escaping], &root).is_ok() {
            return Err("path escaping the root was accepted".to_string());
        }
        Ok(())
    }
}
