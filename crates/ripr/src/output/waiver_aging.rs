use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "waiver_aging";
const LIMITS_NOTE: &str = "Read-only advisory waiver-aging report over existing PR evidence ledgers; repeated waiver is a signal, not a failure or durable suppression.";

pub(crate) const DEFAULT_WAIVER_AGING_OUT: &str = "target/ripr/reports/waiver-aging.json";
pub(crate) const DEFAULT_WAIVER_AGING_MD_OUT: &str = "target/ripr/reports/waiver-aging.md";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WaiverAgingInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) ledger_path: Option<String>,
    pub(crate) history_path: Option<String>,
    pub(crate) ledger_json: Option<Result<String, String>>,
    pub(crate) history_json: Option<Result<String, String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WaiverAgingReport {
    root: String,
    generated_at: String,
    status: String,
    inputs: WaiverAgingInputs,
    summary: WaiverAgingSummary,
    records: Vec<WaiverAgingRecord>,
    warnings: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WaiverAgingInputs {
    ledger: Option<String>,
    history: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct WaiverAgingSummary {
    waiver_count: usize,
    identity_count: usize,
    repeated_seam_count: usize,
    repeated_file_count: usize,
    max_age_prs: usize,
    max_age_days: usize,
    focused_test_candidates: usize,
    durable_suppression_candidates: usize,
    warnings: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WaiverAgingRecord {
    identity: String,
    canonical_gap_id: Option<String>,
    seam_id: Option<String>,
    file: Option<String>,
    owner: Option<String>,
    waiver_count: usize,
    first_seen: String,
    last_seen: String,
    age_prs: usize,
    age_days: usize,
    same_seam_waived_repeatedly: bool,
    same_file_waived_repeatedly: bool,
    candidate_for_focused_test: bool,
    candidate_for_durable_suppression: bool,
    reasons: Vec<String>,
    labels: Vec<String>,
    still_visible: bool,
    source_records: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WaiverObservation {
    identity: String,
    canonical_gap_id: Option<String>,
    seam_id: Option<String>,
    file: Option<String>,
    owner: Option<String>,
    label: Option<String>,
    reason: Option<String>,
    age_prs: usize,
    age_days: usize,
    still_visible: bool,
    seen: String,
    source_record: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedInput {
    value: Option<Value>,
    invalid: bool,
}

pub(crate) fn build_waiver_aging_report(input: WaiverAgingInput) -> WaiverAgingReport {
    let mut warnings = Vec::new();
    let ledger = parse_optional_json(
        "ledger",
        input.ledger_path.as_deref(),
        &input.ledger_json,
        &mut warnings,
    );
    let history = parse_history_jsonl(
        input.history_path.as_deref(),
        &input.history_json,
        &mut warnings,
    );

    let mut observations = Vec::new();
    observations.extend(history.observations);
    if let Some(value) = ledger.value.as_ref() {
        observations.extend(observations_from_ledger(
            value,
            input.ledger_path.as_deref().unwrap_or("ledger"),
        ));
    }

    let mut records = aggregate_observations(observations);
    mark_same_file_repeats(&mut records);
    records.sort_by(|a, b| {
        b.candidate_for_focused_test
            .cmp(&a.candidate_for_focused_test)
            .then(b.age_days.cmp(&a.age_days))
            .then(b.waiver_count.cmp(&a.waiver_count))
            .then(a.identity.cmp(&b.identity))
    });

    if input.ledger_path.is_none() && input.history_path.is_none() {
        warnings.push("no waiver-aging input supplied; report is incomplete".to_string());
    } else if input.history_path.is_none() {
        warnings.push(
            "history input not supplied; waiver age is limited to the current ledger".to_string(),
        );
    }

    let has_config_error = ledger.invalid || history.invalid;
    let status = if has_config_error {
        "config_error"
    } else if input.ledger_path.is_none() && input.history_path.is_none() {
        "incomplete"
    } else if records.is_empty() {
        "no_waivers"
    } else {
        "advisory"
    }
    .to_string();

    let summary = summarize_records(&records, warnings.len());

    WaiverAgingReport {
        root: input.root,
        generated_at: input.generated_at,
        status,
        inputs: WaiverAgingInputs {
            ledger: input.ledger_path,
            history: input.history_path,
        },
        summary,
        records,
        warnings,
    }
}

pub(crate) fn render_waiver_aging_json(report: &WaiverAgingReport) -> Result<String, String> {
    serde_json::to_string_pretty(&json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "kind": REPORT_KIND,
        "status": report.status,
        "root": report.root,
        "generated_at": report.generated_at,
        "inputs": {
            "ledger": report.inputs.ledger,
            "history": report.inputs.history,
        },
        "summary": summary_json(&report.summary),
        "records": report.records.iter().map(record_json).collect::<Vec<_>>(),
        "warnings": report.warnings,
        "limits_note": LIMITS_NOTE,
    }))
    .map_err(|err| format!("failed to render waiver aging JSON: {err}"))
}

pub(crate) fn render_waiver_aging_markdown(report: &WaiverAgingReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Waiver Aging\n\n");
    out.push_str(&format!("Status: {}\n\n", report.status));
    out.push_str(
        "| Identity | Count | Age days | Focused test | Durable suppression review | Last seen |\n",
    );
    out.push_str("| --- | ---: | ---: | --- | --- | --- |\n");
    if report.records.is_empty() {
        out.push_str("| none | 0 | 0 | no | no | n/a |\n");
    } else {
        for record in &report.records {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(&record.identity),
                record.waiver_count,
                record.age_days,
                yes_no(record.candidate_for_focused_test),
                yes_no(record.candidate_for_durable_suppression),
                markdown_cell(&record.last_seen)
            ));
        }
    }
    if !report.warnings.is_empty() {
        out.push_str("\nWarnings:\n");
        for warning in &report.warnings {
            out.push_str(&format!("- {warning}\n"));
        }
    }
    out.push_str("\nPolicy boundary:\n");
    out.push_str("- Repeated waiver is not a failure.\n");
    out.push_str("- Repeated waiver is a visible signal for repair or explicit policy review.\n");
    out.push_str("- Waivers do not become suppressions automatically.\n");
    out.push_str("- Suppressions remain durable policy exceptions with separate ownership and reason metadata.\n");
    out.push_str("\nLimits:\n");
    out.push_str(LIMITS_NOTE);
    out.push('\n');
    out
}

pub(crate) fn waiver_aging_status(report: &WaiverAgingReport) -> &str {
    &report.status
}

pub(crate) use crate::output::path::display_path;

fn parse_optional_json(
    label: &str,
    path: Option<&str>,
    text: &Option<Result<String, String>>,
    warnings: &mut Vec<String>,
) -> ParsedInput {
    let Some(path) = path else {
        return ParsedInput {
            value: None,
            invalid: false,
        };
    };
    let Some(text) = text else {
        warnings.push(format!(
            "{label} path {path} was supplied but no input text was loaded"
        ));
        return ParsedInput {
            value: None,
            invalid: true,
        };
    };
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            warnings.push(format!("{label} input {path} is invalid: {error}"));
            return ParsedInput {
                value: None,
                invalid: true,
            };
        }
    };
    match serde_json::from_str::<Value>(text) {
        Ok(value) => ParsedInput {
            value: Some(value),
            invalid: false,
        },
        Err(error) => {
            warnings.push(format!("{label} input {path} is invalid JSON: {error}"));
            ParsedInput {
                value: None,
                invalid: true,
            }
        }
    }
}

fn parse_history_jsonl(
    path: Option<&str>,
    text: &Option<Result<String, String>>,
    warnings: &mut Vec<String>,
) -> ParsedHistory {
    let Some(path) = path else {
        return ParsedHistory::default();
    };
    let Some(text) = text else {
        warnings.push(format!(
            "history path {path} was supplied but no input text was loaded"
        ));
        return ParsedHistory {
            observations: Vec::new(),
            invalid: true,
        };
    };
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            warnings.push(format!("history input {path} is invalid: {error}"));
            return ParsedHistory {
                observations: Vec::new(),
                invalid: true,
            };
        }
    };

    let mut observations = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(line) {
            Ok(value) => observations.extend(observations_from_ledger(
                &value,
                &format!("{path}:{}", index + 1),
            )),
            Err(error) => warnings.push(format!(
                "history input {path} contains an invalid JSONL record at line {}: {error}",
                index + 1
            )),
        }
    }
    ParsedHistory {
        observations,
        invalid: false,
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ParsedHistory {
    observations: Vec<WaiverObservation>,
    invalid: bool,
}

fn observations_from_ledger(value: &Value, source_record: &str) -> Vec<WaiverObservation> {
    let seen = seen_label(value, source_record);
    let route = path_value(value, &["top_repair_route"]);
    array_path(value, &["waivers"])
        .iter()
        .enumerate()
        .map(|(index, waiver)| {
            let canonical_gap_id = canonical_gap_id_from_value(waiver);
            let seam_id = string_path(waiver, &["seam_id"]);
            let identity =
                waiver_identity(waiver, &canonical_gap_id, &seam_id, source_record, index);
            WaiverObservation {
                identity,
                canonical_gap_id,
                seam_id: seam_id.clone(),
                file: waiver_file(waiver)
                    .or_else(|| route_file_for_identity(route, seam_id.as_deref())),
                owner: string_path(waiver, &["owner"])
                    .or_else(|| string_path(waiver, &["policy", "owner"])),
                label: string_path(waiver, &["label"]),
                reason: string_path(waiver, &["reason"]),
                age_prs: usize_path(waiver, &["age_prs"]).max(1),
                age_days: usize_path(waiver, &["age_days"]),
                still_visible: bool_path(waiver, &["still_visible"]).unwrap_or(true),
                seen: seen.clone(),
                source_record: source_record.to_string(),
            }
        })
        .collect()
}

fn aggregate_observations(observations: Vec<WaiverObservation>) -> Vec<WaiverAgingRecord> {
    let mut by_identity: BTreeMap<String, Vec<WaiverObservation>> = BTreeMap::new();
    let mut seen_pairs = BTreeSet::new();
    for observation in observations {
        let pair = format!("{}|{}", observation.identity, observation.source_record);
        if seen_pairs.insert(pair) {
            by_identity
                .entry(observation.identity.clone())
                .or_default()
                .push(observation);
        }
    }

    by_identity
        .into_iter()
        .map(|(identity, observations)| {
            let mut reasons = BTreeSet::new();
            let mut labels = BTreeSet::new();
            let mut sources = BTreeSet::new();
            let mut seam_counts = BTreeMap::new();
            let mut canonical_gap_id = None;
            let mut seam_id = None;
            let mut file = None;
            let mut owner = None;
            let mut first_seen = None;
            let mut last_seen = None;
            let mut age_prs = 0usize;
            let mut age_days = 0usize;
            let mut still_visible = true;
            for observation in &observations {
                canonical_gap_id =
                    canonical_gap_id.or_else(|| observation.canonical_gap_id.clone());
                if let Some(observed_seam_id) = observation.seam_id.as_ref() {
                    *seam_counts
                        .entry(observed_seam_id.clone())
                        .or_insert(0usize) += 1;
                }
                seam_id = seam_id.or_else(|| observation.seam_id.clone());
                file = file.or_else(|| observation.file.clone());
                owner = owner.or_else(|| observation.owner.clone());
                if let Some(reason) = observation.reason.as_ref() {
                    reasons.insert(reason.clone());
                }
                if let Some(label) = observation.label.as_ref() {
                    labels.insert(label.clone());
                }
                sources.insert(observation.source_record.clone());
                first_seen.get_or_insert_with(|| observation.seen.clone());
                last_seen = Some(observation.seen.clone());
                age_prs = age_prs.max(observation.age_prs);
                age_days = age_days.max(observation.age_days);
                still_visible &= observation.still_visible;
            }
            let waiver_count = observations.len();
            let same_seam_waived_repeatedly = seam_counts.values().any(|count| *count > 1);
            let candidate_for_focused_test = waiver_count > 1 || age_days > 0;
            let candidate_for_durable_suppression = waiver_count >= 3 || age_days >= 30;
            WaiverAgingRecord {
                identity,
                canonical_gap_id,
                seam_id,
                file,
                owner,
                waiver_count,
                first_seen: first_seen.unwrap_or_else(|| "unknown".to_string()),
                last_seen: last_seen.unwrap_or_else(|| "unknown".to_string()),
                age_prs: age_prs.max(waiver_count),
                age_days,
                same_seam_waived_repeatedly,
                same_file_waived_repeatedly: false,
                candidate_for_focused_test,
                candidate_for_durable_suppression,
                reasons: reasons.into_iter().collect(),
                labels: labels.into_iter().collect(),
                still_visible,
                source_records: sources.into_iter().collect(),
            }
        })
        .collect()
}

fn mark_same_file_repeats(records: &mut [WaiverAgingRecord]) {
    let mut counts = BTreeMap::new();
    for record in records.iter() {
        if let Some(file) = record.file.as_ref() {
            *counts.entry(file.clone()).or_insert(0usize) += record.waiver_count;
        }
    }
    for record in records {
        record.same_file_waived_repeatedly = record
            .file
            .as_ref()
            .and_then(|file| counts.get(file))
            .copied()
            .unwrap_or(0)
            > 1;
    }
}

fn summarize_records(records: &[WaiverAgingRecord], warnings: usize) -> WaiverAgingSummary {
    WaiverAgingSummary {
        waiver_count: records.iter().map(|record| record.waiver_count).sum(),
        identity_count: records.len(),
        repeated_seam_count: records
            .iter()
            .filter(|record| record.same_seam_waived_repeatedly)
            .count(),
        repeated_file_count: records
            .iter()
            .filter(|record| record.same_file_waived_repeatedly)
            .count(),
        max_age_prs: records
            .iter()
            .map(|record| record.age_prs)
            .max()
            .unwrap_or(0),
        max_age_days: records
            .iter()
            .map(|record| record.age_days)
            .max()
            .unwrap_or(0),
        focused_test_candidates: records
            .iter()
            .filter(|record| record.candidate_for_focused_test)
            .count(),
        durable_suppression_candidates: records
            .iter()
            .filter(|record| record.candidate_for_durable_suppression)
            .count(),
        warnings,
    }
}

fn summary_json(summary: &WaiverAgingSummary) -> Value {
    json!({
        "waiver_count": summary.waiver_count,
        "identity_count": summary.identity_count,
        "repeated_seam_count": summary.repeated_seam_count,
        "repeated_file_count": summary.repeated_file_count,
        "max_age_prs": summary.max_age_prs,
        "max_age_days": summary.max_age_days,
        "focused_test_candidates": summary.focused_test_candidates,
        "durable_suppression_candidates": summary.durable_suppression_candidates,
        "warnings": summary.warnings,
    })
}

fn record_json(record: &WaiverAgingRecord) -> Value {
    json!({
        "identity": record.identity,
        "canonical_gap_id": record.canonical_gap_id,
        "seam_id": record.seam_id,
        "file": record.file,
        "owner": record.owner,
        "waiver_count": record.waiver_count,
        "first_seen": record.first_seen,
        "last_seen": record.last_seen,
        "age_prs": record.age_prs,
        "age_days": record.age_days,
        "same_seam_waived_repeatedly": record.same_seam_waived_repeatedly,
        "same_file_waived_repeatedly": record.same_file_waived_repeatedly,
        "candidate_for_focused_test": record.candidate_for_focused_test,
        "candidate_for_durable_suppression": record.candidate_for_durable_suppression,
        "reasons": record.reasons,
        "labels": record.labels,
        "still_visible": record.still_visible,
        "source_records": record.source_records,
    })
}

fn seen_label(value: &Value, source_record: &str) -> String {
    pr_number_label(value)
        .map(|number| format!("pr#{number}"))
        .or_else(|| string_path(value, &["generated_at"]))
        .unwrap_or_else(|| source_record.to_string())
}

fn pr_number_label(value: &Value) -> Option<String> {
    let number = path_value(value, &["pr", "number"])?;
    string_value(number).or_else(|| number.as_u64().map(|value| value.to_string()))
}

fn waiver_identity(
    waiver: &Value,
    canonical_gap_id: &Option<String>,
    seam_id: &Option<String>,
    source_record: &str,
    index: usize,
) -> String {
    canonical_gap_id
        .clone()
        .or_else(|| seam_id.clone())
        .or_else(|| string_path(waiver, &["decision_id"]))
        .unwrap_or_else(|| format!("unknown:{source_record}:{index}"))
}

fn waiver_file(waiver: &Value) -> Option<String> {
    string_path(waiver, &["file"])
        .or_else(|| string_path(waiver, &["path"]))
        .or_else(|| string_path(waiver, &["location", "path"]))
}

fn route_file_for_identity(route: Option<&Value>, seam_id: Option<&str>) -> Option<String> {
    let route = route?;
    let route_seam = string_path(route, &["seam_id"]);
    if seam_id.is_some() && route_seam.as_deref() == seam_id {
        string_path(route, &["path"])
    } else {
        None
    }
}

fn canonical_gap_id_from_value(value: &Value) -> Option<String> {
    string_path(value, &["canonical_gap_id"])
        .or_else(|| string_path(value, &["identity", "canonical_gap_id"]))
        .or_else(|| string_path(value, &["evidence_record", "canonical_gap_id"]))
        .or_else(|| string_path(value, &["provenance", "canonical_gap_id"]))
        .or_else(|| string_path(value, &["seam", "canonical_gap_id"]))
        .or_else(|| string_path(value, &["guidance", "canonical_gap_id"]))
}

fn path_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cursor = value;
    for segment in path {
        cursor = cursor.get(*segment)?;
    }
    Some(cursor)
}

fn array_path<'a>(value: &'a Value, path: &[&str]) -> Vec<&'a Value> {
    path_value(value, path)
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

fn string_path(value: &Value, path: &[&str]) -> Option<String> {
    path_value(value, path).and_then(string_value)
}

fn string_value(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn bool_path(value: &Value, path: &[&str]) -> Option<bool> {
    path_value(value, path).and_then(Value::as_bool)
}

fn usize_path(value: &Value, path: &[&str]) -> usize {
    path_value(value, path)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|")
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> WaiverAgingInput {
        WaiverAgingInput {
            root: ".".to_string(),
            generated_at: "unix_ms:1".to_string(),
            ledger_path: None,
            history_path: None,
            ledger_json: None,
            history_json: None,
        }
    }

    fn ledger(pr: &str, age_days: usize) -> String {
        format!(
            r#"{{
              "schema_version": "0.1",
              "kind": "pr_evidence_ledger",
              "generated_at": "unix_ms:{pr}",
              "pr": {{"number": "{pr}"}},
              "top_repair_route": {{"seam_id": "seam-a", "path": "src/lib.rs"}},
              "waivers": [{{
                "label": "ripr-waive",
                "canonical_gap_id": "gap-a",
                "decision_id": "decision-a",
                "seam_id": "seam-a",
                "age_prs": 1,
                "age_days": {age_days},
                "reason": "accepted for this PR",
                "still_visible": true
              }}]
            }}"#
        )
    }

    fn ledger_with_seam(pr: &str, canonical_gap_id: &str, seam_id: &str) -> String {
        format!(
            r#"{{
              "pr": {{"number": "{pr}"}},
              "top_repair_route": {{"seam_id": "{seam_id}", "path": "src/lib.rs"}},
              "waivers": [{{
                "canonical_gap_id": "{canonical_gap_id}",
                "seam_id": "{seam_id}",
                "file": "src/lib.rs",
                "still_visible": true
              }}]
            }}"#
        )
    }

    fn compact_record(text: &str) -> Result<String, String> {
        let value: Value =
            serde_json::from_str(text).map_err(|error| format!("parse fixture: {error}"))?;
        serde_json::to_string(&value).map_err(|error| format!("compact fixture: {error}"))
    }

    fn history_record(pr: &str, age_days: usize) -> Result<String, String> {
        compact_record(&ledger(pr, age_days))
    }

    #[test]
    fn missing_inputs_are_incomplete() -> Result<(), String> {
        let report = build_waiver_aging_report(input());
        assert_eq!(report.status, "incomplete");
        assert_eq!(report.summary.waiver_count, 0);
        assert_eq!(report.warnings.len(), 1);
        let rendered = render_waiver_aging_json(&report)?;
        assert!(rendered.contains("\"kind\": \"waiver_aging\""));
        assert!(rendered.contains("\"status\": \"incomplete\""));
        Ok(())
    }

    #[test]
    fn current_ledger_without_history_is_visible_but_not_repeated() -> Result<(), String> {
        let mut input = input();
        input.ledger_path = Some("target/ripr/reports/pr-evidence-ledger.json".to_string());
        input.ledger_json = Some(Ok(ledger("123", 0)));

        let report = build_waiver_aging_report(input);

        assert_eq!(report.status, "advisory");
        assert_eq!(report.summary.waiver_count, 1);
        assert_eq!(report.summary.focused_test_candidates, 0);
        assert_eq!(report.records[0].identity, "gap-a");
        assert_eq!(report.records[0].file.as_deref(), Some("src/lib.rs"));
        assert!(report.records[0].still_visible);
        let markdown = render_waiver_aging_markdown(&report);
        assert!(markdown.contains("Repeated waiver is not a failure."));
        Ok(())
    }

    #[test]
    fn history_repeats_same_seam_and_marks_policy_candidates() -> Result<(), String> {
        let mut input = input();
        input.ledger_path = Some("target/ripr/reports/pr-evidence-ledger.json".to_string());
        input.ledger_json = Some(Ok(ledger("125", 40)));
        input.history_path = Some(".ripr/pr-evidence-ledger.jsonl".to_string());
        input.history_json = Some(Ok(format!(
            "{}\n{}\n",
            history_record("123", 10)?,
            history_record("124", 20)?
        )));

        let report = build_waiver_aging_report(input);

        assert_eq!(report.status, "advisory");
        assert_eq!(report.summary.waiver_count, 3);
        assert_eq!(report.summary.identity_count, 1);
        assert_eq!(report.summary.repeated_seam_count, 1);
        assert_eq!(report.summary.repeated_file_count, 1);
        assert_eq!(report.summary.max_age_days, 40);
        assert_eq!(report.summary.focused_test_candidates, 1);
        assert_eq!(report.summary.durable_suppression_candidates, 1);
        let record = &report.records[0];
        assert_eq!(record.first_seen, "pr#123");
        assert_eq!(record.last_seen, "pr#125");
        assert!(record.same_seam_waived_repeatedly);
        assert!(record.same_file_waived_repeatedly);
        assert!(record.candidate_for_focused_test);
        assert!(record.candidate_for_durable_suppression);
        Ok(())
    }

    #[test]
    fn same_canonical_gap_with_different_seams_is_not_same_seam_repeat() -> Result<(), String> {
        let mut input = input();
        input.ledger_path = Some("target/ripr/reports/pr-evidence-ledger.json".to_string());
        input.ledger_json = Some(Ok(ledger_with_seam("125", "gap-a", "seam-b")));
        input.history_path = Some(".ripr/pr-evidence-ledger.jsonl".to_string());
        input.history_json = Some(Ok(format!(
            "{}\n",
            compact_record(&ledger_with_seam("124", "gap-a", "seam-a"))?
        )));

        let report = build_waiver_aging_report(input);

        assert_eq!(report.summary.waiver_count, 2);
        assert_eq!(report.summary.identity_count, 1);
        assert_eq!(report.summary.repeated_seam_count, 0);
        assert!(!report.records[0].same_seam_waived_repeatedly);
        assert!(report.records[0].candidate_for_focused_test);
        Ok(())
    }

    #[test]
    fn numeric_pr_numbers_render_seen_labels() {
        let mut input = input();
        input.ledger_path = Some("target/ripr/reports/pr-evidence-ledger.json".to_string());
        input.ledger_json = Some(Ok(r#"{
              "pr": {"number": 123},
              "waivers": [
                {"canonical_gap_id": "gap-a", "seam_id": "seam-a", "still_visible": true}
              ]
            }"#
        .to_string()));

        let report = build_waiver_aging_report(input);

        assert_eq!(report.records[0].first_seen, "pr#123");
        assert_eq!(report.records[0].last_seen, "pr#123");
    }

    #[test]
    fn distinct_waivers_in_same_file_mark_file_repeat() {
        let mut input = input();
        input.ledger_path = Some("target/ripr/reports/pr-evidence-ledger.json".to_string());
        input.ledger_json = Some(Ok(
            r#"{
              "pr": {"number": "9"},
              "waivers": [
                {"canonical_gap_id": "gap-a", "seam_id": "seam-a", "file": "src/lib.rs", "age_prs": 1, "age_days": 0, "still_visible": true},
                {"canonical_gap_id": "gap-b", "seam_id": "seam-b", "file": "src/lib.rs", "age_prs": 1, "age_days": 0, "still_visible": true}
              ]
            }"#
            .to_string(),
        ));

        let report = build_waiver_aging_report(input);

        assert_eq!(report.summary.waiver_count, 2);
        assert_eq!(report.summary.repeated_seam_count, 0);
        assert_eq!(report.summary.repeated_file_count, 2);
        assert!(
            report
                .records
                .iter()
                .all(|record| record.same_file_waived_repeatedly)
        );
    }

    #[test]
    fn invalid_supplied_ledger_is_config_error() -> Result<(), String> {
        let mut input = input();
        input.ledger_path = Some("target/ripr/reports/pr-evidence-ledger.json".to_string());
        input.ledger_json = Some(Ok("{".to_string()));

        let report = build_waiver_aging_report(input);

        assert_eq!(report.status, "config_error");
        assert_eq!(report.summary.warnings, 2);
        let rendered = render_waiver_aging_json(&report)?;
        assert!(rendered.contains("invalid JSON"));
        Ok(())
    }

    #[test]
    fn history_jsonl_parse_warnings_do_not_hide_valid_records() -> Result<(), String> {
        let mut input = input();
        input.history_path = Some(".ripr/pr-evidence-ledger.jsonl".to_string());
        input.history_json = Some(Ok(format!("not-json\n{}\n", history_record("7", 1)?)));

        let report = build_waiver_aging_report(input);

        assert_eq!(report.status, "advisory");
        assert_eq!(report.summary.waiver_count, 1);
        assert_eq!(report.summary.warnings, 1);
        assert!(report.warnings[0].contains("invalid JSONL record"));
        Ok(())
    }
}
