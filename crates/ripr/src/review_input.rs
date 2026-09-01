//! Typed, bounded input exchanged between the PR producer and review-comments.

use serde::de::{IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt;
use std::io::Read;
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

#[derive(Debug)]
pub struct StreamedProducerCheck {
    pub schema_version: String,
    pub tool: String,
    pub mode: String,
    pub root: String,
    pub base: String,
    pub summary: Value,
    pub analysis_outcome: Value,
    pub finding_count: u64,
    pub projection: Vec<ReviewFindingProjectionV1>,
}

#[derive(Debug, Deserialize)]
struct PendingFinding {
    id: String,
    probe: PendingProbe,
    severity: String,
    classification: String,
    suggested_next_action: Option<String>,
    recommended_next_step: Option<String>,
    related_tests: Option<Vec<PendingRelatedTest>>,
}

#[derive(Debug, Deserialize)]
struct PendingProbe {
    file: String,
    line: u64,
}

#[derive(Debug, Deserialize)]
struct PendingRelatedTest {
    name: String,
    file: String,
    line: u64,
}

#[derive(Debug, Serialize)]
struct ProjectionDigest<'a> {
    stable_id: String,
    file: &'a str,
    line: u64,
    severity: &'a str,
    finding_class: &'a str,
    summary: &'a str,
    related_test: Option<&'a ReviewRelatedTestProjectionV1>,
}

#[derive(Default, Debug)]
struct FindingStream {
    count: u64,
    findings: Vec<PendingFinding>,
}

impl<'de> Deserialize<'de> for FindingStream {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct FindingVisitor;

        impl<'de> Visitor<'de> for FindingVisitor {
            type Value = FindingStream;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a producer findings array")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut stream = FindingStream::default();
                while let Some(finding) = sequence.next_element::<PendingFinding>()? {
                    stream.count = stream.count.saturating_add(1);
                    stream.findings.push(finding);
                }
                Ok(stream)
            }
        }

        deserializer.deserialize_seq(FindingVisitor)
    }
}

#[derive(Deserialize)]
struct ProducerCheckStream {
    schema_version: String,
    tool: String,
    mode: String,
    root: String,
    base: String,
    summary: Value,
    analysis_outcome: Value,
    findings: FindingStream,
    #[serde(flatten)]
    _discarded: std::collections::BTreeMap<String, IgnoredAny>,
}

pub(crate) fn producer_mode(bytes: &[u8]) -> Result<Option<String>, String> {
    let Some(key_start) = bytes
        .windows(b"\"mode\"".len())
        .position(|window| window == b"\"mode\"")
    else {
        return Ok(None);
    };
    let Some(colon) = bytes[key_start + b"\"mode\"".len()..]
        .iter()
        .position(|byte| *byte == b':')
        .map(|offset| key_start + b"\"mode\"".len() + offset + 1)
    else {
        return Ok(None);
    };
    let value_start = bytes[colon..]
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .map(|offset| colon + offset)
        .ok_or_else(|| "producer mode value is missing".to_string())?;
    if bytes.get(value_start) != Some(&b'\"') {
        return Ok(None);
    }
    let mut escaped = false;
    let mut end = None;
    for (offset, byte) in bytes[value_start + 1..].iter().enumerate() {
        if escaped {
            escaped = false;
        } else if *byte == b'\\' {
            escaped = true;
        } else if *byte == b'\"' {
            end = Some(value_start + offset + 2);
            break;
        }
    }
    let end = end.ok_or_else(|| "producer mode string is unterminated".to_string())?;
    serde_json::from_slice(&bytes[value_start..end])
        .map(Some)
        .map_err(|error| format!("producer mode is invalid JSON: {error}"))
}

/// Read only the producer fields needed for admission. Large unrelated check
/// fields are skipped, and each finding is released after its canonical
/// projection facts have been retained.
pub fn stream_producer_check<R: Read>(
    reader: R,
    root: &Path,
) -> Result<StreamedProducerCheck, String> {
    let parsed: ProducerCheckStream = serde_json::from_reader(reader)
        .map_err(|error| format!("producer check is invalid JSON: {error}"))?;
    let root = root
        .canonicalize()
        .map_err(|error| format!("canonicalize review input root: {error}"))?;
    let mut projection = parsed
        .findings
        .findings
        .into_iter()
        .map(|pending| finish_pending_finding(pending, &root))
        .collect::<Result<Vec<_>, _>>()?;
    projection.sort_by_key(projection_order);
    projection.truncate(REVIEW_INPUT_PROJECTION_LIMIT as usize);
    Ok(StreamedProducerCheck {
        schema_version: parsed.schema_version,
        tool: parsed.tool,
        mode: parsed.mode,
        root: parsed.root,
        base: parsed.base,
        summary: parsed.summary,
        analysis_outcome: parsed.analysis_outcome,
        finding_count: parsed.findings.count,
        projection,
    })
}

fn finish_pending_finding(
    pending: PendingFinding,
    root: &Path,
) -> Result<ReviewFindingProjectionV1, String> {
    let file_path = Path::new(&pending.probe.file);
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
    let related_test = pending.related_tests.and_then(|tests| {
        tests
            .into_iter()
            .next()
            .map(|test| ReviewRelatedTestProjectionV1 {
                name: test.name,
                file: test.file,
                line: test.line,
            })
    });
    let summary = pending
        .suggested_next_action
        .filter(|summary| !summary.trim().is_empty())
        .or_else(|| {
            pending
                .recommended_next_step
                .filter(|summary| !summary.trim().is_empty())
        })
        .unwrap_or_else(|| "Inspect the producer-owned review finding.".to_string());
    let mut projection = ReviewFindingProjectionV1 {
        stable_id: pending.id,
        file: relative_file,
        line: Some(pending.probe.line),
        severity: pending.severity,
        finding_class: pending.classification,
        summary,
        evidence_digest: String::new(),
        related_test,
    };
    projection.evidence_digest = projection_digest(&projection)?;
    Ok(projection)
}

fn projection_digest(projection: &ReviewFindingProjectionV1) -> Result<String, String> {
    let digest_input = ProjectionDigest {
        stable_id: projection.stable_id.clone(),
        file: &projection.file,
        line: projection.line.unwrap_or_default(),
        severity: &projection.severity,
        finding_class: &projection.finding_class,
        summary: &projection.summary,
        related_test: projection.related_test.as_ref(),
    };
    let bytes = serde_json::to_vec(&digest_input)
        .map_err(|error| format!("serialize projection digest: {error}"))?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
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
                evidence_digest: String::new(),
                related_test,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    for finding in &mut projected {
        finding.evidence_digest = projection_digest(finding)?;
    }
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

    #[test]
    fn streamed_producer_check_discards_large_unrelated_fields() -> Result<(), String> {
        let root = std::env::current_dir().map_err(|error| error.to_string())?;
        let mut finding = finding();
        finding["evidence"] = serde_json::json!("x".repeat(10 * 1024 * 1024));
        let check = serde_json::json!({
            "schema_version": "0.2",
            "tool": "ripr",
            "mode": "draft",
            "root": root,
            "base": "HEAD",
            "summary": {},
            "findings": [finding],
            "analysis_outcome": {"analysis_complete": true, "outcome": {}},
            "finding_alignment": {"items": ["x".repeat(1024 * 1024)]}
        });
        let bytes = serde_json::to_vec(&check).map_err(|error| error.to_string())?;
        let parsed = stream_producer_check(bytes.as_slice(), &root)?;
        if parsed.finding_count != 1 || parsed.projection.len() != 1 {
            return Err("streamed producer check lost finding projection".to_string());
        }
        if parsed.projection[0].evidence_digest.is_empty() {
            return Err("streamed producer check omitted projection digest".to_string());
        }
        Ok(())
    }
}
