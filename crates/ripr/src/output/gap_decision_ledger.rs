use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "gap_decision_ledger";

pub(crate) const DEFAULT_GAP_DECISION_LEDGER_OUT: &str =
    "target/ripr/reports/gap-decision-ledger.json";
pub(crate) const DEFAULT_GAP_DECISION_LEDGER_MD_OUT: &str =
    "target/ripr/reports/gap-decision-ledger.md";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GapDecisionLedgerInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) records_path: String,
    pub(crate) records_json: Result<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GapDecisionLedgerReport {
    status: String,
    root: String,
    generated_at: String,
    inputs: GapDecisionLedgerInputs,
    summary: GapDecisionLedgerSummary,
    records: Vec<GapRecord>,
    warnings: Vec<String>,
    limits: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct GapDecisionLedgerInputs {
    records: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct GapDecisionLedgerSummary {
    records_total: usize,
    repairable_total: usize,
    static_limitation_total: usize,
    no_action_total: usize,
    missing_artifact_total: usize,
    projection_pr_comment_eligible: usize,
    projection_gate_candidate: usize,
    projection_agent_packet_eligible: usize,
    ripr_zero_target_count: usize,
    ripr_plus_target_count: usize,
    preview_ineligible_total: usize,
    receipt_improved_total: usize,
    receipt_unchanged_after_attempt_total: usize,
    missing_output_contract_total: usize,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct GapRecord {
    #[serde(default)]
    pub(crate) gap_id: String,
    #[serde(default)]
    pub(crate) canonical_gap_id: String,
    #[serde(default)]
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) language: String,
    #[serde(default)]
    pub(crate) language_status: String,
    #[serde(default)]
    pub(crate) scope: String,
    #[serde(default)]
    pub(crate) evidence_class: String,
    #[serde(default)]
    pub(crate) gap_state: String,
    #[serde(default)]
    pub(crate) policy_state: String,
    #[serde(default)]
    pub(crate) repairability: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) repair_route: Option<GapRepairRoute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) anchor: Option<GapAnchor>,
    #[serde(default)]
    pub(crate) evidence_ids: Vec<String>,
    #[serde(default)]
    pub(crate) projection_eligibility: BTreeMap<String, ProjectionEligibility>,
    #[serde(default)]
    pub(crate) verification_commands: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) regeneration_commands: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) receipt: Option<GapReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) safe_gate_predicate: Option<SafeGatePredicate>,
    #[serde(default)]
    pub(crate) authority_boundary: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct GapRepairRoute {
    #[serde(default)]
    pub(crate) route_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) target_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) target_line: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) related_test: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) assertion_shape: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) changed_behavior: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) stop_conditions: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct GapAnchor {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) line: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) dedupe_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct ProjectionEligibility {
    #[serde(default)]
    pub(crate) eligible: bool,
    #[serde(default)]
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct GapReceipt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) movement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct SafeGatePredicate {
    #[serde(default)]
    pub(crate) policy_target_enabled: bool,
    #[serde(default)]
    pub(crate) suppressed: bool,
    #[serde(default)]
    pub(crate) waived: bool,
    #[serde(default)]
    pub(crate) acknowledged_only: bool,
    #[serde(default)]
    pub(crate) baseline_known: bool,
    #[serde(default)]
    pub(crate) preview_language: bool,
    #[serde(default)]
    pub(crate) static_unknown_only: bool,
}

pub(crate) fn build_gap_decision_ledger_report(
    input: GapDecisionLedgerInput,
) -> GapDecisionLedgerReport {
    let mut warnings = Vec::new();
    let records = match input.records_json {
        Ok(contents) => match parse_gap_records(&contents) {
            Ok(records) => records,
            Err(err) => {
                warnings.push(format!("parse {} failed: {err}", input.records_path));
                Vec::new()
            }
        },
        Err(err) => {
            warnings.push(err);
            Vec::new()
        }
    };

    for record in &records {
        validate_record(record, &mut warnings);
    }

    let summary = summarize_records(&records);
    let status = if records.is_empty() {
        "blocked"
    } else if warnings.is_empty() {
        "advisory"
    } else {
        "advisory_with_warnings"
    }
    .to_string();

    GapDecisionLedgerReport {
        status,
        root: input.root,
        generated_at: input.generated_at,
        inputs: GapDecisionLedgerInputs {
            records: input.records_path,
        },
        summary,
        records,
        warnings,
        limits: vec![
            "Advisory static gap decisions only.".to_string(),
            "Gate-decision artifacts remain the only configured pass/fail authority.".to_string(),
            "This report does not rerun analysis, execute mutation tests, edit source, generate tests, call providers, publish comments, or change default CI blocking.".to_string(),
        ],
    }
}

pub(crate) fn render_gap_decision_ledger_json(
    report: &GapDecisionLedgerReport,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct JsonReport<'a> {
        schema_version: &'static str,
        tool: &'static str,
        kind: &'static str,
        status: &'a str,
        root: &'a str,
        generated_at: &'a str,
        inputs: &'a GapDecisionLedgerInputs,
        summary: &'a GapDecisionLedgerSummary,
        records: &'a [GapRecord],
        warnings: &'a [String],
        limits: &'a [String],
    }

    serde_json::to_string_pretty(&JsonReport {
        schema_version: SCHEMA_VERSION,
        tool: "ripr",
        kind: REPORT_KIND,
        status: &report.status,
        root: &report.root,
        generated_at: &report.generated_at,
        inputs: &report.inputs,
        summary: &report.summary,
        records: &report.records,
        warnings: &report.warnings,
        limits: &report.limits,
    })
    .map_err(|err| format!("serialize gap decision ledger JSON failed: {err}"))
}

pub(crate) fn render_gap_decision_ledger_markdown(report: &GapDecisionLedgerReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Gap Decision Ledger\n\n");
    out.push_str(&format!("Status: `{}`\n\n", md_inline(&report.status)));
    out.push_str(&format!("Root: `{}`\n\n", md_inline(&report.root)));
    out.push_str("Authority: gate-decision artifacts own pass/fail authority. This report is advisory projection input.\n\n");

    out.push_str("## Summary\n\n");
    out.push_str(&format!("- Records: `{}`\n", report.summary.records_total));
    out.push_str(&format!(
        "- Repairable: `{}`; static limitations: `{}`; no action: `{}`; missing artifacts: `{}`\n",
        report.summary.repairable_total,
        report.summary.static_limitation_total,
        report.summary.no_action_total,
        report.summary.missing_artifact_total
    ));
    out.push_str(&format!(
        "- Projections: PR comments=`{}`, gate candidates=`{}`, agent packets=`{}`\n",
        report.summary.projection_pr_comment_eligible,
        report.summary.projection_gate_candidate,
        report.summary.projection_agent_packet_eligible
    ));
    out.push_str(&format!(
        "- Badge targets: ripr 0=`{}`, ripr+=`{}`\n",
        report.summary.ripr_zero_target_count, report.summary.ripr_plus_target_count
    ));
    out.push_str(&format!(
        "- Receipts: improved=`{}`, unchanged_after_attempt=`{}`\n",
        report.summary.receipt_improved_total, report.summary.receipt_unchanged_after_attempt_total
    ));
    out.push_str(&format!(
        "- Output-contract gaps: `{}`; preview ineligible: `{}`\n\n",
        report.summary.missing_output_contract_total, report.summary.preview_ineligible_total
    ));

    if !report.warnings.is_empty() {
        out.push_str("## Warnings\n\n");
        for warning in &report.warnings {
            out.push_str(&format!("- {}\n", md_inline(warning)));
        }
        out.push('\n');
    }

    out.push_str("## Records\n\n");
    if report.records.is_empty() {
        out.push_str("No gap records were supplied.\n\n");
    } else {
        for record in &report.records {
            render_record_markdown(record, &mut out);
        }
    }

    out.push_str("## Limits\n\n");
    for limit in &report.limits {
        out.push_str(&format!("- {}\n", md_inline(limit)));
    }
    out
}

fn parse_gap_records(contents: &str) -> Result<Vec<GapRecord>, String> {
    let value: Value =
        serde_json::from_str(contents).map_err(|err| format!("invalid JSON: {err}"))?;
    gap_records_from_value(&value)
}

fn gap_records_from_value(value: &Value) -> Result<Vec<GapRecord>, String> {
    if let Some(records) = value.as_array() {
        return parse_record_array(records);
    }
    let Some(object) = value.as_object() else {
        return Err("expected object or array at gap record root".to_string());
    };
    if let Some(records) = object.get("records").and_then(Value::as_array) {
        return parse_record_array(records);
    }
    if let Some(records) = object.get("gap_records").and_then(Value::as_array) {
        return parse_record_array(records);
    }
    if let Some(cases) = object.get("cases").and_then(Value::as_array) {
        let mut records = Vec::new();
        for case in cases {
            let case_id = case.get("id").and_then(Value::as_str).unwrap_or("unknown");
            let Some(record) = case.get("expected_gap_record") else {
                return Err(format!("case {case_id} is missing expected_gap_record"));
            };
            records.push(parse_record(record).map_err(|err| format!("case {case_id}: {err}"))?);
        }
        return Ok(records);
    }
    Err("expected records, gap_records, cases, or record array".to_string())
}

fn parse_record_array(records: &[Value]) -> Result<Vec<GapRecord>, String> {
    records
        .iter()
        .enumerate()
        .map(|(index, record)| parse_record(record).map_err(|err| format!("record {index}: {err}")))
        .collect()
}

fn parse_record(record: &Value) -> Result<GapRecord, String> {
    serde_json::from_value(record.clone()).map_err(|err| format!("invalid GapRecord: {err}"))
}

fn summarize_records(records: &[GapRecord]) -> GapDecisionLedgerSummary {
    let mut summary = GapDecisionLedgerSummary {
        records_total: records.len(),
        ..GapDecisionLedgerSummary::default()
    };
    for record in records {
        if record.repairability == "repairable" {
            summary.repairable_total += 1;
        }
        if record.kind == "StaticLimitation" {
            summary.static_limitation_total += 1;
        }
        if record.repairability == "no_action"
            || matches!(
                record.kind.as_str(),
                "NoActionAlreadyObserved" | "NoActionInternal"
            )
        {
            summary.no_action_total += 1;
        }
        if record.scope == "artifact_missing" {
            summary.missing_artifact_total += 1;
        }
        if projection_eligible(record, "pr_comment") {
            summary.projection_pr_comment_eligible += 1;
        }
        if projection_eligible(record, "gate_candidate") {
            summary.projection_gate_candidate += 1;
        }
        if projection_eligible(record, "agent_packet") {
            summary.projection_agent_packet_eligible += 1;
        }
        if projection_eligible(record, "ripr_zero_count") {
            summary.ripr_zero_target_count += 1;
        }
        if projection_eligible(record, "ripr_plus_count") {
            summary.ripr_plus_target_count += 1;
        }
        if record.language_status == "preview"
            && !projection_eligible(record, "gate_candidate")
            && !projection_eligible(record, "ripr_zero_count")
            && !projection_eligible(record, "ripr_plus_count")
        {
            summary.preview_ineligible_total += 1;
        }
        if record.kind == "MissingOutputContract" {
            summary.missing_output_contract_total += 1;
        }
        if record
            .receipt
            .as_ref()
            .and_then(|receipt| receipt.movement.as_deref())
            == Some("improved")
        {
            summary.receipt_improved_total += 1;
        }
        if record
            .receipt
            .as_ref()
            .and_then(|receipt| receipt.movement.as_deref())
            == Some("unchanged_after_attempt")
        {
            summary.receipt_unchanged_after_attempt_total += 1;
        }
    }
    summary
}

fn validate_record(record: &GapRecord, warnings: &mut Vec<String>) {
    if record.gap_id.trim().is_empty() {
        warnings.push("gap record is missing gap_id".to_string());
    }
    if record.kind.trim().is_empty() {
        warnings.push(format!(
            "gap record {} is missing kind",
            fallback_gap_id(record)
        ));
    }
    if record.repairability == "repairable" && record.repair_route.is_none() {
        warnings.push(format!(
            "gap record {} is repairable but missing repair_route",
            fallback_gap_id(record)
        ));
    }
    if record.repairability == "repairable" && record.verification_commands.is_empty() {
        warnings.push(format!(
            "gap record {} is repairable but missing verification_commands",
            fallback_gap_id(record)
        ));
    }
    if projection_eligible(record, "pr_comment")
        && record
            .anchor
            .as_ref()
            .and_then(|anchor| anchor.dedupe_fingerprint.as_deref())
            .is_none()
    {
        warnings.push(format!(
            "gap record {} is PR-comment eligible but missing anchor.dedupe_fingerprint",
            fallback_gap_id(record)
        ));
    }
    if projection_eligible(record, "gate_candidate") && !safe_gate_predicate_satisfied(record) {
        warnings.push(format!(
            "gap record {} is gate-candidate eligible but safe_gate_predicate is incomplete",
            fallback_gap_id(record)
        ));
    }
    if record.language_status == "preview"
        && (projection_eligible(record, "gate_candidate")
            || projection_eligible(record, "ripr_zero_count")
            || projection_eligible(record, "ripr_plus_count"))
    {
        warnings.push(format!(
            "gap record {} is preview evidence but eligible for gate or badge authority",
            fallback_gap_id(record)
        ));
    }
    if record.scope == "artifact_missing" && record.regeneration_commands.is_empty() {
        warnings.push(format!(
            "gap record {} has artifact_missing scope but no regeneration_commands",
            fallback_gap_id(record)
        ));
    }
}

fn safe_gate_predicate_satisfied(record: &GapRecord) -> bool {
    let Some(predicate) = &record.safe_gate_predicate else {
        return false;
    };
    record.language == "rust"
        && record.language_status == "stable"
        && record.scope == "pr_local"
        && matches!(record.policy_state.as_str(), "new" | "blocked")
        && record.repairability == "repairable"
        && record.repair_route.is_some()
        && !record.verification_commands.is_empty()
        && predicate.policy_target_enabled
        && !predicate.suppressed
        && !predicate.waived
        && !predicate.acknowledged_only
        && !predicate.baseline_known
        && !predicate.preview_language
        && !predicate.static_unknown_only
}

fn projection_eligible(record: &GapRecord, projection: &str) -> bool {
    record
        .projection_eligibility
        .get(projection)
        .is_some_and(|projection| projection.eligible)
}

fn render_record_markdown(record: &GapRecord, out: &mut String) {
    out.push_str(&format!(
        "### `{}`\n\n",
        md_inline(&fallback_gap_id(record))
    ));
    out.push_str(&format!(
        "- Kind: `{}`; scope: `{}`; policy: `{}`; repairability: `{}`\n",
        md_inline(&record.kind),
        md_inline(&record.scope),
        md_inline(&record.policy_state),
        md_inline(&record.repairability)
    ));
    out.push_str(&format!(
        "- Evidence: `{}` / `{}`; language: `{}` / `{}`\n",
        md_inline(&record.evidence_class),
        md_inline(&record.gap_state),
        md_inline(&record.language),
        md_inline(&record.language_status)
    ));
    if let Some(anchor) = &record.anchor {
        out.push_str(&format!(
            "- Anchor: `{}`{}{}\n",
            md_inline(anchor.file.as_deref().unwrap_or("unknown")),
            anchor
                .line
                .map(|line| format!(":{line}"))
                .unwrap_or_default(),
            anchor
                .owner
                .as_ref()
                .map(|owner| format!(" owner `{}`", md_inline(owner)))
                .unwrap_or_default()
        ));
    }
    if let Some(route) = &record.repair_route {
        out.push_str(&format!(
            "- Repair: `{}`{}\n",
            md_inline(&route.route_kind),
            route
                .target_file
                .as_ref()
                .map(|target| format!(" in `{}`", md_inline(target)))
                .unwrap_or_default()
        ));
        if let Some(assertion) = &route.assertion_shape {
            out.push_str(&format!(
                "- Assertion or observer: `{}`\n",
                md_inline(assertion)
            ));
        }
    }
    let eligible = eligible_projection_names(record);
    if !eligible.is_empty() {
        out.push_str(&format!(
            "- Eligible projections: `{}`\n",
            eligible.join("`, `")
        ));
    }
    if !record.verification_commands.is_empty() {
        out.push_str("- Verify:\n");
        for command in &record.verification_commands {
            out.push_str(&format!("  - `{}`\n", md_inline(command)));
        }
    }
    if !record.regeneration_commands.is_empty() {
        out.push_str("- Regenerate:\n");
        for command in &record.regeneration_commands {
            out.push_str(&format!("  - `{}`\n", md_inline(command)));
        }
    }
    if let Some(receipt) = &record.receipt {
        out.push_str(&format!(
            "- Receipt movement: `{}`\n",
            md_inline(receipt.movement.as_deref().unwrap_or("unknown"))
        ));
    }
    out.push('\n');
}

fn eligible_projection_names(record: &GapRecord) -> Vec<String> {
    record
        .projection_eligibility
        .iter()
        .filter(|(_, projection)| projection.eligible)
        .map(|(name, _)| name.clone())
        .collect()
}

fn fallback_gap_id(record: &GapRecord) -> String {
    if record.gap_id.trim().is_empty() {
        "unknown-gap".to_string()
    } else {
        record.gap_id.clone()
    }
}

fn md_inline(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\r' | '\n' => escaped.push(' '),
            '|' => escaped.push_str("\\|"),
            '`' => escaped.push('\''),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus() -> String {
        include_str!("../../../../fixtures/gap-decision-ledger/corpus.json").to_string()
    }

    #[test]
    fn gap_decision_ledger_parses_corpus_records_and_summarizes_projection_boundaries() {
        let report = build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: ".".to_string(),
            generated_at: "test".to_string(),
            records_path: "fixtures/gap-decision-ledger/corpus.json".to_string(),
            records_json: Ok(corpus()),
        });

        assert_eq!(report.status, "advisory");
        assert_eq!(report.summary.records_total, 18);
        assert_eq!(report.summary.projection_gate_candidate, 1);
        assert_eq!(report.summary.ripr_zero_target_count, 1);
        assert_eq!(report.summary.preview_ineligible_total, 1);
        assert_eq!(report.summary.missing_output_contract_total, 1);
        assert_eq!(report.summary.receipt_improved_total, 1);
        assert_eq!(report.summary.receipt_unchanged_after_attempt_total, 1);
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn gap_decision_ledger_renders_json_and_markdown() -> Result<(), String> {
        let report = build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: ".".to_string(),
            generated_at: "test".to_string(),
            records_path: "fixtures/gap-decision-ledger/corpus.json".to_string(),
            records_json: Ok(corpus()),
        });

        let json = render_gap_decision_ledger_json(&report)?;
        assert!(json.contains("\"kind\": \"gap_decision_ledger\""));
        assert!(json.contains("\"MissingOutputContract\""));
        assert!(json.contains("\"AddOutputGolden\""));

        let markdown = render_gap_decision_ledger_markdown(&report);
        assert!(markdown.starts_with("# RIPR Gap Decision Ledger"));
        assert!(markdown.contains("gate candidates=`1`"));
        assert!(
            markdown
                .contains("Gate-decision artifacts remain the only configured pass/fail authority")
        );
        assert!(markdown.contains("AddOutputGolden"));
        Ok(())
    }

    #[test]
    fn gap_decision_ledger_reports_bad_or_missing_records_as_blocked() {
        let report = build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: ".".to_string(),
            generated_at: "test".to_string(),
            records_path: "missing.json".to_string(),
            records_json: Err("read missing.json failed: not found".to_string()),
        });

        assert_eq!(report.status, "blocked");
        assert_eq!(report.summary.records_total, 0);
        assert_eq!(
            report.warnings,
            vec!["read missing.json failed: not found".to_string()]
        );
    }

    #[test]
    fn gap_decision_ledger_warns_on_unsafe_projection() {
        let record = serde_json::json!({
            "records": [
                {
                    "gap_id": "gap:bad",
                    "canonical_gap_id": "gap:bad",
                    "kind": "MissingBoundaryAssertion",
                    "language": "typescript",
                    "language_status": "preview",
                    "scope": "pr_local",
                    "evidence_class": "predicate_boundary",
                    "gap_state": "actionable",
                    "policy_state": "new",
                    "repairability": "repairable",
                    "projection_eligibility": {
                        "gate_candidate": {"eligible": true, "reason": "bad"},
                        "ripr_zero_count": {"eligible": true, "reason": "bad"}
                    },
                    "verification_commands": []
                }
            ]
        });

        let report = build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: ".".to_string(),
            generated_at: "test".to_string(),
            records_path: "bad.json".to_string(),
            records_json: Ok(record.to_string()),
        });

        let warnings = report.warnings.join("\n");
        assert!(warnings.contains("repairable but missing repair_route"));
        assert!(warnings.contains("repairable but missing verification_commands"));
        assert!(warnings.contains("gate-candidate eligible but safe_gate_predicate is incomplete"));
        assert!(warnings.contains("preview evidence but eligible for gate or badge authority"));
    }
}
