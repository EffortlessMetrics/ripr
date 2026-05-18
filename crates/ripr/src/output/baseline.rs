use serde_json::{Value, json};

const SCHEMA_VERSION: &str = "0.1";
const BASELINE_KIND: &str = "gate_baseline";
const DEFAULT_REVIEW_REASON: &str = "initial adoption baseline";
const LIMITS_NOTE: &str = "Reviewed baseline debt ledger over static RIPR gate evidence; baselines are not suppressions and do not change gate policy by themselves.";
pub(crate) const DEFAULT_BASELINE_OUT: &str = ".ripr/gate-baseline.json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BaselineCreateReport {
    created_at: String,
    source_report: String,
    mode: String,
    reviewed: bool,
    entries: Vec<BaselineEntry>,
    skipped: BaselineCreateSkipped,
    warnings: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BaselineEntry {
    identity: BaselineIdentity,
    path: Option<String>,
    line: Option<u64>,
    static_class: Option<String>,
    decision: String,
    severity: Option<String>,
    source: Option<String>,
    gate_reason: Option<String>,
    evidence: BaselineEntryEvidence,
    review: BaselineEntryReview,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BaselineIdentity {
    canonical_gap_id: Option<String>,
    seam_id: Option<String>,
    source_id: Option<String>,
    id: Option<String>,
    dedupe_key: Option<String>,
    fallback: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BaselineEntryEvidence {
    missing_discriminator: Option<String>,
    assertion_shape: Option<String>,
    recommended_test: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BaselineEntryReview {
    reviewed: bool,
    owner: Option<String>,
    reason: String,
    created_at: Option<String>,
    review_after: Option<String>,
    source: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct BaselineCreateSkipped {
    suppressed: usize,
    not_applicable: usize,
    malformed: usize,
    other: usize,
}

pub(crate) fn baseline_create_report_from_gate_decision_json(
    source_report: &str,
    created_at: &str,
    json_text: &str,
) -> Result<BaselineCreateReport, String> {
    let value = serde_json::from_str::<Value>(json_text)
        .map_err(|err| format!("parse gate-decision JSON failed: {err}"))?;
    let schema_version = value
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| "gate-decision JSON missing schema_version".to_string())?;
    if schema_version != SCHEMA_VERSION {
        return Err(format!(
            "unsupported gate-decision schema_version `{schema_version}`; expected `{SCHEMA_VERSION}`"
        ));
    }

    let decisions = value
        .get("decisions")
        .and_then(Value::as_array)
        .ok_or_else(|| "gate-decision JSON missing decisions array".to_string())?;
    let mode = string_field(value.get("mode")).unwrap_or_else(|| "unknown".to_string());
    let mut entries = Vec::new();
    let mut skipped = BaselineCreateSkipped::default();
    let mut warnings = Vec::new();

    for decision in decisions {
        match decision.get("decision").and_then(Value::as_str) {
            Some("advisory" | "acknowledged" | "blocking") => {
                match baseline_entry_from_decision(decision, created_at, source_report) {
                    Some(entry) => entries.push(entry),
                    None => {
                        skipped.malformed += 1;
                        warnings.push(
                            "skipped gate decision without a stable identity or fallback"
                                .to_string(),
                        );
                    }
                }
            }
            Some("suppressed") => skipped.suppressed += 1,
            Some("not_applicable") => skipped.not_applicable += 1,
            Some(_) | None => skipped.other += 1,
        }
    }

    entries.sort_by(|left, right| {
        entry_sort_key(left)
            .cmp(&entry_sort_key(right))
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.line.cmp(&right.line))
    });

    Ok(BaselineCreateReport {
        created_at: created_at.to_string(),
        source_report: source_report.to_string(),
        mode,
        reviewed: false,
        entries,
        skipped,
        warnings,
    })
}

pub(crate) fn render_baseline_create_json(report: &BaselineCreateReport) -> Result<String, String> {
    serde_json::to_string_pretty(&json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "kind": BASELINE_KIND,
        "created_at": report.created_at,
        "source_report": report.source_report,
        "mode": report.mode,
        "reviewed": report.reviewed,
        "summary": {
            "entries": report.entries.len(),
            "included": report.entries.len(),
            "skipped": skipped_json(&report.skipped),
        },
        "entries": report.entries.iter().map(entry_json).collect::<Vec<_>>(),
        "warnings": report.warnings,
        "limits_note": LIMITS_NOTE,
    }))
    .map_err(|err| format!("failed to render gate baseline JSON: {err}"))
}

pub(crate) fn baseline_entry_count(report: &BaselineCreateReport) -> usize {
    report.entries.len()
}

pub(crate) use crate::output::path::display_path;

fn baseline_entry_from_decision(
    value: &Value,
    created_at: &str,
    source_report: &str,
) -> Option<BaselineEntry> {
    let path = string_field(value.pointer("/placement/path"));
    let line = value.pointer("/placement/line").and_then(Value::as_u64);
    let static_class = string_field(value.get("static_class"));
    let identity = BaselineIdentity {
        canonical_gap_id: canonical_gap_id_from_value(value),
        seam_id: string_field(value.get("seam_id")),
        source_id: string_field(value.get("source_id")),
        id: string_field(value.get("id")),
        dedupe_key: string_field(value.get("dedupe_key")),
        fallback: fallback_identity(path.as_deref(), line, static_class.as_deref()),
    };
    if !identity.has_stable_value() {
        return None;
    }

    Some(BaselineEntry {
        identity,
        path,
        line,
        static_class,
        decision: string_field(value.get("decision")).unwrap_or_else(|| "unknown".to_string()),
        severity: string_field(value.get("severity")),
        source: string_field(value.get("source")),
        gate_reason: string_field(value.get("gate_reason")),
        evidence: BaselineEntryEvidence {
            missing_discriminator: string_field(value.pointer("/evidence/missing_discriminator")),
            assertion_shape: string_field(value.pointer("/evidence/assertion_shape")),
            recommended_test: string_field(value.pointer("/evidence/recommended_test")),
        },
        review: BaselineEntryReview {
            reviewed: false,
            owner: None,
            reason: DEFAULT_REVIEW_REASON.to_string(),
            created_at: Some(created_at.to_string()),
            review_after: None,
            source: Some(source_report.to_string()),
        },
    })
}

fn fallback_identity(
    path: Option<&str>,
    line: Option<u64>,
    static_class: Option<&str>,
) -> Option<String> {
    match (path, line, static_class) {
        (Some(path), Some(line), Some(static_class)) => Some(format!(
            "{}:{line}:{static_class}",
            path.replace('\\', "/").trim_start_matches("./")
        )),
        _ => None,
    }
}

impl BaselineIdentity {
    fn has_stable_value(&self) -> bool {
        self.canonical_gap_id.is_some()
            || self.seam_id.is_some()
            || self.source_id.is_some()
            || self.id.is_some()
            || self.dedupe_key.is_some()
            || self.fallback.is_some()
    }
}

fn entry_sort_key(entry: &BaselineEntry) -> String {
    entry
        .identity
        .canonical_gap_id
        .as_deref()
        .or(entry.identity.seam_id.as_deref())
        .or(entry.identity.source_id.as_deref())
        .or(entry.identity.id.as_deref())
        .or(entry.identity.dedupe_key.as_deref())
        .or(entry.identity.fallback.as_deref())
        .unwrap_or("")
        .to_string()
}

fn canonical_gap_id_from_value(value: &Value) -> Option<String> {
    string_field(value.get("canonical_gap_id"))
        .or_else(|| string_field(value.pointer("/identity/canonical_gap_id")))
        .or_else(|| string_field(value.pointer("/evidence_record/canonical_gap_id")))
}

fn entry_json(entry: &BaselineEntry) -> Value {
    json!({
        "identity": {
            "canonical_gap_id": entry.identity.canonical_gap_id,
            "seam_id": entry.identity.seam_id,
            "source_id": entry.identity.source_id,
            "id": entry.identity.id,
            "dedupe_key": entry.identity.dedupe_key,
            "fallback": entry.identity.fallback,
        },
        "path": entry.path,
        "line": entry.line,
        "static_class": entry.static_class,
        "decision": entry.decision,
        "severity": entry.severity,
        "source": entry.source,
        "gate_reason": entry.gate_reason,
        "evidence": {
            "missing_discriminator": entry.evidence.missing_discriminator,
            "assertion_shape": entry.evidence.assertion_shape,
            "recommended_test": entry.evidence.recommended_test,
        },
        "review": {
            "reviewed": entry.review.reviewed,
            "owner": entry.review.owner,
            "reason": entry.review.reason,
            "created_at": entry.review.created_at,
            "review_after": entry.review.review_after,
            "source": entry.review.source,
        }
    })
}

fn skipped_json(skipped: &BaselineCreateSkipped) -> Value {
    json!({
        "suppressed": skipped.suppressed,
        "not_applicable": skipped.not_applicable,
        "malformed": skipped.malformed,
        "other": skipped.other,
    })
}

fn string_field(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{baseline_create_report_from_gate_decision_json, render_baseline_create_json};

    #[test]
    fn baseline_create_includes_visible_gate_decisions_and_skips_hidden_ones() -> Result<(), String>
    {
        let json = r#"{
          "schema_version": "0.1",
          "mode": "acknowledgeable",
          "decisions": [
            {
              "decision": "suppressed",
              "id": "ripr-gate-hidden",
              "seam_id": "hidden",
              "source_id": "hidden-source",
              "static_class": "weakly_gripped",
              "severity": "off",
              "placement": {"path": "src/lib.rs", "line": 1},
              "evidence": {"configured_off": true, "suppressed": true}
            },
            {
              "decision": "acknowledged",
              "id": "ripr-gate-bbb",
              "seam_id": "bbb",
              "source_id": "ripr-review-bbb",
              "static_class": "weakly_gripped",
              "severity": "warning",
              "source": "pr_guidance",
              "gate_reason": "policy-eligible gap acknowledged by ripr-waive",
              "placement": {"path": "src/b.rs", "line": 20},
              "evidence": {
                "missing_discriminator": "b == 2",
                "assertion_shape": "assert_eq!(b(), 2)",
                "recommended_test": "tests/b.rs::boundary"
              }
            },
            {
              "decision": "advisory",
              "id": "ripr-gate-aaa",
              "seam_id": "aaa",
              "source_id": "ripr-review-aaa",
              "static_class": "weakly_gripped",
              "severity": "warning",
              "source": "pr_guidance",
              "gate_reason": "visible-only mode records evidence without blocking",
              "placement": {"path": "src/a.rs", "line": 10},
              "evidence": {
                "missing_discriminator": "a == 1",
                "assertion_shape": "assert_eq!(a(), 1)",
                "recommended_test": "tests/a.rs::boundary"
              }
            }
          ]
        }"#;

        let report = baseline_create_report_from_gate_decision_json(
            "target/ripr/reports/gate-decision.json",
            "unix_ms:1",
            json,
        )?;
        let rendered = render_baseline_create_json(&report)?;
        assert!(rendered.contains("\"kind\": \"gate_baseline\""));
        assert!(rendered.contains("\"created_at\": \"unix_ms:1\""));
        assert!(rendered.contains("\"entries\": 2"));
        assert!(rendered.contains("\"suppressed\": 1"));
        assert!(rendered.contains("\"review_after\": null"));
        assert!(rendered.contains("\"source\": \"target/ripr/reports/gate-decision.json\""));
        assert!(rendered.contains("\"seam_id\": \"aaa\""));
        assert!(rendered.contains("\"seam_id\": \"bbb\""));
        assert!(rendered.find("\"aaa\"") < rendered.find("\"bbb\""));
        assert!(!rendered.contains("hidden-source"));
        Ok(())
    }

    #[test]
    fn baseline_create_uses_canonical_gap_identity_when_supplied() -> Result<(), String> {
        let json = r#"{
          "schema_version": "0.1",
          "mode": "visible-only",
          "decisions": [
            {
              "decision": "advisory",
              "id": "ripr-gate-old",
              "seam_id": "old",
              "source_id": "ripr-review-old",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/old.rs", "line": 7},
              "evidence_record": {
                "canonical_gap_id": "pricing::discount::threshold_equality"
              },
              "evidence": {"missing_discriminator": "amount == threshold"}
            }
          ]
        }"#;

        let report =
            baseline_create_report_from_gate_decision_json("gate.json", "unix_ms:1", json)?;
        let rendered = render_baseline_create_json(&report)?;
        assert!(
            rendered.contains("\"canonical_gap_id\": \"pricing::discount::threshold_equality\"")
        );
        Ok(())
    }

    #[test]
    fn baseline_create_uses_fallback_identity_when_direct_ids_are_missing() -> Result<(), String> {
        let json = r#"{
          "schema_version": "0.1",
          "mode": "visible-only",
          "decisions": [
            {
              "decision": "advisory",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/pricing.rs", "line": 88},
              "evidence": {}
            }
          ]
        }"#;

        let report =
            baseline_create_report_from_gate_decision_json("gate.json", "unix_ms:2", json)?;
        let rendered = render_baseline_create_json(&report)?;
        assert!(rendered.contains("\"fallback\": \"src/pricing.rs:88:weakly_gripped\""));
        assert!(rendered.contains("\"entries\": 1"));
        Ok(())
    }

    #[test]
    fn baseline_create_reports_malformed_entries_without_stopping() -> Result<(), String> {
        let json = r#"{
          "schema_version": "0.1",
          "mode": "visible-only",
          "decisions": [
            {
              "decision": "advisory",
              "evidence": {}
            }
          ]
        }"#;

        let report =
            baseline_create_report_from_gate_decision_json("gate.json", "unix_ms:3", json)?;
        let rendered = render_baseline_create_json(&report)?;
        assert!(rendered.contains("\"entries\": 0"));
        assert!(rendered.contains("\"malformed\": 1"));
        assert!(rendered.contains("skipped gate decision without a stable identity or fallback"));
        Ok(())
    }

    #[test]
    fn baseline_create_rejects_unsupported_schema_version() {
        let err = baseline_create_report_from_gate_decision_json(
            "gate.json",
            "unix_ms:4",
            r#"{"schema_version":"9.9","decisions":[]}"#,
        );
        assert_eq!(
            err,
            Err("unsupported gate-decision schema_version `9.9`; expected `0.1`".to_string())
        );
    }
}
