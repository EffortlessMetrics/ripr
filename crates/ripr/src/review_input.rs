//! Typed, bounded input exchanged between the PR producer and review-comments.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

pub const REVIEW_INPUT_SCHEMA_VERSION: &str = "ripr.review_input.v1";
pub const REVIEW_INPUT_SELECTION_POLICY: &str = "severity_actionability_stable_id_path_line";
pub const REVIEW_INPUT_SELECTION_POLICY_VERSION: &str = "v1";
pub const REVIEW_INPUT_PROJECTION_LIMIT: u64 = 10;
pub const REVIEW_INDEX_SCHEMA_VERSION: &str = "ripr.canonical_finding_index.v1";
pub const REVIEW_INDEX_MAX_ENTRIES: usize = 4096;
// The index is bounded independently from the forensic check packet.  The
// 4,096-entry cap and 2 MiB byte cap accommodate the promotion-scale canary
// while keeping subject admission firmly in the compact evidence plane.
pub const REVIEW_INDEX_MAX_BYTES: usize = 2 * 1024 * 1024;

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
    #[serde(default)]
    pub analysis_outcome: Option<Value>,
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

/// Bounded producer-owned authority for selecting the renderer projection.
/// This intentionally contains no finding bodies, source, AST, or seam state.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CanonicalFindingIndexV1 {
    pub schema_version: String,
    pub total_finding_count: u64,
    pub index_sha256: String,
    pub entries: Vec<ReviewFindingProjectionV1>,
}

fn canonical_relative_file(
    file: &str,
    root: &Path,
    canonical_files: &mut HashMap<String, String>,
) -> Result<String, String> {
    if let Some(relative_file) = canonical_files.get(file) {
        return Ok(relative_file.clone());
    }
    let file_path = Path::new(file);
    let absolute_file = if file_path.is_absolute() {
        file_path.to_path_buf()
    } else {
        root.join(file_path)
    };
    let relative_file = absolute_file
        .canonicalize()
        .map_err(|error| format!("canonicalize producer finding file: {error}"))?
        .strip_prefix(root)
        .map_err(|error| format!("producer finding file escapes root: {error}"))?
        .display()
        .to_string()
        .replace('\\', "/");
    canonical_files.insert(file.to_string(), relative_file.clone());
    Ok(relative_file)
}

fn finding_evidence_digest(finding: &Value) -> Result<String, String> {
    let evidence = finding.get("evidence").cloned().unwrap_or(Value::Null);
    let bytes = serde_json::to_vec(&evidence)
        .map_err(|error| format!("serialize finding evidence digest: {error}"))?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

/// Derive the only projection that may be admitted for review-comments.
/// Missing or malformed producer fields are errors; they are never silently
/// filtered out of the bounded packet.
pub fn canonical_projection(
    findings: &[Value],
    root: &Path,
) -> Result<Vec<ReviewFindingProjectionV1>, String> {
    let mut projected = canonical_projection_all(findings, root)?;
    projected.truncate(REVIEW_INPUT_PROJECTION_LIMIT as usize);
    Ok(projected)
}

pub fn canonical_projection_all(
    findings: &[Value],
    root: &Path,
) -> Result<Vec<ReviewFindingProjectionV1>, String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("canonicalize review input root: {error}"))?;
    let mut canonical_files = HashMap::new();
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
            let relative_file = canonical_relative_file(file, &root, &mut canonical_files)?;
            let evidence_digest = finding_evidence_digest(finding)?;
            Ok(ReviewFindingProjectionV1 {
                stable_id: stable_id.to_string(),
                file: relative_file,
                line: Some(line),
                severity: severity.to_string(),
                finding_class: finding_class.to_string(),
                summary: projection_summary(finding).to_string(),
                evidence_digest,
                related_test,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    projected.sort_by_key(projection_order);
    Ok(projected)
}

pub fn canonical_projection_from_index(
    index: &CanonicalFindingIndexV1,
) -> Result<Vec<ReviewFindingProjectionV1>, String> {
    if index.schema_version != REVIEW_INDEX_SCHEMA_VERSION {
        return Err("canonical finding index schema is unsupported".to_string());
    }
    if index.entries.len() > REVIEW_INDEX_MAX_ENTRIES {
        return Err("canonical finding index exceeds entry limit".to_string());
    }
    if index.entries.len() as u64 != index.total_finding_count {
        return Err("canonical finding index count is contradictory".to_string());
    }
    let encoded = serde_json::to_vec(&index.entries)
        .map_err(|error| format!("serialize canonical finding index: {error}"))?;
    if encoded.len() > REVIEW_INDEX_MAX_BYTES {
        return Err("canonical finding index exceeds byte limit".to_string());
    }
    let expected = format!("sha256:{:x}", Sha256::digest(&encoded));
    if index.index_sha256 != expected {
        return Err("canonical finding index digest does not match entries".to_string());
    }
    let mut entries = index.entries.clone();
    let mut ids = std::collections::HashSet::new();
    if entries
        .iter()
        .any(|entry| !ids.insert(entry.stable_id.clone()))
    {
        return Err("canonical finding index contains duplicate stable IDs".to_string());
    }
    entries.sort_by_key(projection_order);
    let limit = REVIEW_INPUT_PROJECTION_LIMIT as usize;
    entries.truncate(limit);
    Ok(entries)
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
    fn canonical_index_rejects_duplicate_ids_and_projects_deterministically() -> Result<(), String>
    {
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        let entries = canonical_projection_all(&[finding()], &root)?;
        let encoded = serde_json::to_vec(&entries).map_err(|error| error.to_string())?;
        let index = CanonicalFindingIndexV1 {
            schema_version: REVIEW_INDEX_SCHEMA_VERSION.to_string(),
            total_finding_count: 1,
            index_sha256: format!("sha256:{:x}", Sha256::digest(&encoded)),
            entries: entries.clone(),
        };
        if canonical_projection_from_index(&index)? != entries {
            return Err("canonical index projection changed its ordering".to_string());
        }
        let mut duplicate = index;
        duplicate.total_finding_count = 2;
        duplicate.entries.push(entries[0].clone());
        let encoded = serde_json::to_vec(&duplicate.entries).map_err(|error| error.to_string())?;
        duplicate.index_sha256 = format!("sha256:{:x}", Sha256::digest(&encoded));
        if canonical_projection_from_index(&duplicate).is_ok() {
            return Err("duplicate canonical finding ID was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn canonical_index_rejects_bad_schema_count_digest_and_size() -> Result<(), String> {
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        let entries = canonical_projection_all(&[finding()], &root)?;
        let encoded = serde_json::to_vec(&entries).map_err(|error| error.to_string())?;
        let valid = CanonicalFindingIndexV1 {
            schema_version: REVIEW_INDEX_SCHEMA_VERSION.to_string(),
            total_finding_count: 1,
            index_sha256: format!("sha256:{:x}", Sha256::digest(&encoded)),
            entries: entries.clone(),
        };
        let mut bad_schema = valid.clone();
        bad_schema.schema_version = "unknown".to_string();
        if canonical_projection_from_index(&bad_schema).is_ok() {
            return Err("unsupported index schema was accepted".to_string());
        }
        let mut bad_count = valid.clone();
        bad_count.total_finding_count = 2;
        if canonical_projection_from_index(&bad_count).is_ok() {
            return Err("contradictory index count was accepted".to_string());
        }
        let mut bad_digest = valid.clone();
        bad_digest.index_sha256 = "sha256:wrong".to_string();
        if canonical_projection_from_index(&bad_digest).is_ok() {
            return Err("wrong index digest was accepted".to_string());
        }
        let too_many = vec![entries[0].clone(); REVIEW_INDEX_MAX_ENTRIES + 1];
        let too_many_index = CanonicalFindingIndexV1 {
            schema_version: REVIEW_INDEX_SCHEMA_VERSION.to_string(),
            total_finding_count: too_many.len() as u64,
            index_sha256: String::new(),
            entries: too_many,
        };
        if canonical_projection_from_index(&too_many_index).is_ok() {
            return Err("oversized index entry count was accepted".to_string());
        }
        let oversized = ReviewFindingProjectionV1 {
            summary: "x".repeat(REVIEW_INDEX_MAX_BYTES),
            ..entries[0].clone()
        };
        let oversized_bytes = serde_json::to_vec(std::slice::from_ref(&oversized))
            .map_err(|error| error.to_string())?;
        let oversized_index = CanonicalFindingIndexV1 {
            schema_version: REVIEW_INDEX_SCHEMA_VERSION.to_string(),
            total_finding_count: 1,
            index_sha256: format!("sha256:{:x}", Sha256::digest(&oversized_bytes)),
            entries: vec![oversized],
        };
        if canonical_projection_from_index(&oversized_index).is_ok() {
            return Err("oversized index was accepted".to_string());
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
