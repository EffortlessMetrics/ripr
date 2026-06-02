use crate::agent::loop_commands::{outcome_command, shell_arg};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "gap_decision_ledger";
const DEFAULT_CHECK_AFTER_OUTPUT: &str = "target/ripr/reports/after-check.json";
const DEFAULT_RECEIPTS_DIR: &str = "target/ripr/receipts";

pub(crate) const DEFAULT_GAP_DECISION_LEDGER_OUT: &str =
    "target/ripr/reports/gap-decision-ledger.json";
pub(crate) const DEFAULT_GAP_DECISION_LEDGER_MD_OUT: &str =
    "target/ripr/reports/gap-decision-ledger.md";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GapDecisionLedgerInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) source_kind: GapDecisionLedgerSourceKind,
    pub(crate) records_path: String,
    pub(crate) records_json: Result<String, String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GapDecisionLedgerSourceKind {
    Records,
    RepoExposure,
    CheckOutput,
}

impl GapDecisionLedgerSourceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Records => "records",
            Self::RepoExposure => "repo_exposure",
            Self::CheckOutput => "check_output",
        }
    }
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ParsedGapRecordSource {
    pub(crate) root: Option<String>,
    pub(crate) generated_at: Option<String>,
    pub(crate) records: Vec<GapRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct GapDecisionLedgerInputs {
    source_kind: &'static str,
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
    pub(crate) static_limit_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) static_limit_detail: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) static_limits: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) anchor: Option<GapAnchor>,
    #[serde(default)]
    pub(crate) evidence_ids: Vec<String>,
    #[serde(default)]
    pub(crate) projection_eligibility: BTreeMap<String, ProjectionEligibility>,
    #[serde(default)]
    pub(crate) verification_commands: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) receipt_command: Option<String>,
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
    pub(crate) missing_discriminator: Option<String>,
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
    let mut records = match input.records_json {
        Ok(contents) => match parse_gap_decision_source(input.source_kind, &contents) {
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

    if input.source_kind == GapDecisionLedgerSourceKind::CheckOutput {
        attach_check_output_python_receipt_routes(&mut records, &input.root, &input.records_path);
    }

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
            source_kind: input.source_kind.as_str(),
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

pub(crate) fn parse_gap_records_json(contents: &str) -> Result<Vec<GapRecord>, String> {
    parse_gap_record_source_json(contents).map(|source| source.records)
}

pub(crate) fn parse_gap_record_source_json(
    contents: &str,
) -> Result<ParsedGapRecordSource, String> {
    let value: Value =
        serde_json::from_str(contents).map_err(|err| format!("invalid JSON: {err}"))?;
    let root = value
        .as_object()
        .and_then(|object| object.get("root"))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let generated_at = value
        .as_object()
        .and_then(|object| object.get("generated_at"))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let records = gap_records_from_value(&value)?;
    Ok(ParsedGapRecordSource {
        root,
        generated_at,
        records,
    })
}

fn parse_gap_decision_source(
    source_kind: GapDecisionLedgerSourceKind,
    contents: &str,
) -> Result<Vec<GapRecord>, String> {
    match source_kind {
        GapDecisionLedgerSourceKind::Records => parse_gap_records_json(contents),
        GapDecisionLedgerSourceKind::RepoExposure => gap_records_from_repo_exposure_json(contents),
        GapDecisionLedgerSourceKind::CheckOutput => gap_records_from_check_output_json(contents),
    }
}

fn gap_records_from_check_output_json(contents: &str) -> Result<Vec<GapRecord>, String> {
    let value: Value =
        serde_json::from_str(contents).map_err(|err| format!("invalid JSON: {err}"))?;
    let items = value
        .get("finding_alignment")
        .and_then(|alignment| alignment.get("items"))
        .and_then(Value::as_array);
    let findings = value.get("findings").and_then(Value::as_array);
    if items.is_none() && findings.is_none() {
        return Err(
            "expected check output object with finding_alignment.items or findings array"
                .to_string(),
        );
    }

    let mut records = Vec::new();
    if let Some(items) = items {
        for (index, item) in items.iter().enumerate() {
            let Some(record) = gap_record_from_finding_alignment_item(item, index) else {
                continue;
            };
            if record.gap_id.is_empty() {
                return Err(format!(
                    "finding_alignment item {index} produced an empty gap_id"
                ));
            }
            records.push(record);
        }
    }
    if let Some(findings) = findings {
        for (index, finding) in findings.iter().enumerate() {
            let Some(record) = gap_record_from_python_repair_finding(finding, index)
                .or_else(|| gap_record_from_python_static_limit_finding(finding, index))
                .or_else(|| gap_record_from_python_no_action_finding(finding, index))
            else {
                continue;
            };
            if record.gap_id.is_empty() {
                return Err(format!("finding {index} produced an empty gap_id"));
            }
            records.push(record);
        }
    }
    Ok(records)
}

fn gap_records_from_repo_exposure_json(contents: &str) -> Result<Vec<GapRecord>, String> {
    let value: Value =
        serde_json::from_str(contents).map_err(|err| format!("invalid JSON: {err}"))?;
    let seams = value
        .get("seams")
        .and_then(Value::as_array)
        .ok_or_else(|| "expected repo exposure object with seams array".to_string())?;
    let mut records = Vec::new();
    for (index, seam) in seams.iter().enumerate() {
        let Some(record) = gap_record_from_repo_exposure_seam(seam) else {
            continue;
        };
        if record.gap_id.is_empty() {
            return Err(format!("seam {index} produced an empty gap_id"));
        }
        records.push(record);
    }
    Ok(records)
}

fn gap_record_from_repo_exposure_seam(seam: &Value) -> Option<GapRecord> {
    let evidence = seam.get("evidence_record")?;
    let canonical_item = evidence.get("canonical_item")?;
    let gap_state = string_at(canonical_item, &["gap_state"]).unwrap_or("unknown");
    let actionability = string_at(canonical_item, &["actionability"]).unwrap_or("unknown");
    let seam_kind = string_at(evidence, &["seam_kind"])
        .or_else(|| string_at(canonical_item, &["evidence_class"]))
        .unwrap_or("unknown");
    let repairability = repairability_from_evidence(gap_state, actionability);
    let repair_route =
        repair_route_from_evidence(evidence, canonical_item, seam_kind, repairability);
    let verify_command = string_at(canonical_item, &["verify_command"])
        .or_else(|| string_at(evidence, &["recommendation", "verify_command"]));
    let verification_commands = verify_command
        .map(|command| vec![command.to_string()])
        .unwrap_or_default();
    let static_limits = array_values_at(canonical_item, &["static_limits"])
        .or_else(|| array_values_at(evidence, &["static_limits"]))
        .unwrap_or_default();
    let static_limit_kind = string_at(canonical_item, &["static_limit_kind"])
        .or_else(|| string_at(evidence, &["static_limit_kind"]))
        .or_else(|| {
            static_limits.iter().find_map(|limit| {
                string_at(limit, &["static_limit_kind"]).or_else(|| string_at(limit, &["kind"]))
            })
        })
        .map(ToString::to_string);
    let static_limit_detail = string_at(canonical_item, &["static_limit_detail"])
        .or_else(|| string_at(evidence, &["static_limit_detail"]))
        .or_else(|| {
            static_limits.iter().find_map(|limit| {
                string_at(limit, &["detail"])
                    .or_else(|| string_at(limit, &["reason"]))
                    .or_else(|| string_at(limit, &["message"]))
            })
        })
        .map(ToString::to_string);
    let receipt_command = string_at(canonical_item, &["receipt_command"])
        .or_else(|| string_at(evidence, &["receipt_command"]))
        .map(ToString::to_string);
    let seam_id = string_at(evidence, &["seam_id"])
        .or_else(|| string_at(seam, &["seam_id"]))
        .unwrap_or("unknown-seam");
    let canonical_gap_id = string_at(evidence, &["canonical_gap_id"])
        .or_else(|| string_at(canonical_item, &["canonical_gap_id"]))
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("gap:rust:{seam_id}"));
    let gap_id = format!("gap:repo:{canonical_gap_id}");
    let file = string_at(evidence, &["location", "file"])
        .or_else(|| string_at(seam, &["file"]))
        .map(ToString::to_string);
    let line = u64_at(evidence, &["location", "line"]).or_else(|| u64_at(seam, &["line"]));
    let owner = string_at(evidence, &["owner"]).map(ToString::to_string);
    let anchor = GapAnchor {
        file,
        line,
        owner,
        dedupe_fingerprint: Some(canonical_gap_id.clone()),
    };
    let projection_eligibility = projection_eligibility_from_repo_evidence(
        repairability,
        repair_route.is_some(),
        !verification_commands.is_empty(),
        anchor.file.is_some() && anchor.line.is_some(),
        gap_state,
    );

    Some(GapRecord {
        gap_id,
        canonical_gap_id,
        kind: gap_kind_from_evidence(gap_state, seam_kind).to_string(),
        language: "rust".to_string(),
        language_status: "stable".to_string(),
        scope: "repo_scoped".to_string(),
        evidence_class: seam_kind.to_string(),
        gap_state: gap_state.to_string(),
        policy_state: if gap_state == "actionable" {
            "new".to_string()
        } else {
            "not_policy_targeted".to_string()
        },
        repairability: repairability.to_string(),
        repair_route,
        static_limit_kind,
        static_limit_detail,
        static_limits,
        anchor: Some(anchor),
        evidence_ids: evidence_ids_from_repo_evidence(evidence, seam_id),
        projection_eligibility,
        verification_commands,
        receipt_command,
        regeneration_commands: Vec::new(),
        receipt: None,
        safe_gate_predicate: None,
        authority_boundary: "gate_decision_artifact_only".to_string(),
    })
}

fn array_values_at(value: &Value, path: &[&str]) -> Option<Vec<Value>> {
    let mut cursor = value;
    for segment in path {
        cursor = cursor.get(*segment)?;
    }
    cursor.as_array().map(|values| values.to_vec())
}

fn string_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut cursor = value;
    for segment in path {
        cursor = cursor.get(*segment)?;
    }
    cursor.as_str()
}

fn non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn string_array_at(value: &Value, path: &[&str]) -> Vec<String> {
    let mut cursor = value;
    for segment in path {
        let Some(next) = cursor.get(*segment) else {
            return Vec::new();
        };
        cursor = next;
    }
    cursor
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn u64_at(value: &Value, path: &[&str]) -> Option<u64> {
    let mut cursor = value;
    for segment in path {
        cursor = cursor.get(*segment)?;
    }
    cursor.as_u64()
}

fn repairability_from_evidence(gap_state: &str, actionability: &str) -> &'static str {
    match gap_state {
        "actionable"
            if matches!(
                actionability,
                "add_focused_test"
                    | "upgrade_assertion"
                    | "extend_related_test"
                    | "add_output_observer"
            ) =>
        {
            "repairable"
        }
        "already_observed" | "internal_only" | "no_related_test" | "heuristic_only" => "no_action",
        "static_limitation" => "analyzer_limitation",
        _ => "unknown",
    }
}

fn gap_kind_from_evidence(gap_state: &str, seam_kind: &str) -> &'static str {
    match gap_state {
        "already_observed" => "NoActionAlreadyObserved",
        "internal_only" => "NoActionInternal",
        "no_related_test" => "NoActionNoRelatedTest",
        "heuristic_only" => "NoActionHeuristicOnly",
        "static_limitation" => "StaticLimitation",
        "actionable" => match seam_kind {
            "presentation_text" => "MissingOutputContract",
            "predicate_boundary" | "match_arm" => "MissingBoundaryAssertion",
            "error_variant" | "exception_path" => "MissingErrorDiscriminator",
            "field_construction" | "field" | "dict_field" | "return_value" => {
                "MissingValueAssertion"
            }
            "call_presence" | "side_effect" | "output_log" => "MissingSideEffectObserver",
            _ => "Unknown",
        },
        _ => "Unknown",
    }
}

fn gap_record_from_python_repair_finding(finding: &Value, index: usize) -> Option<GapRecord> {
    let card = finding.get("python_repair_card")?;
    if string_at(card, &["language"]) != Some("python") {
        return None;
    }

    let canonical_gap_id = string_at(card, &["canonical_gap_id"])
        .or_else(|| string_at(finding, &["canonical_gap_id"]))
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("gap:python:check-output:item_{index}"));
    let behavior_kind = string_at(finding, &["canonical_gap", "behavior_kind"])
        .or_else(|| string_at(finding, &["probe", "family"]))
        .unwrap_or("unknown");
    let source_file = string_at(finding, &["probe", "file"])
        .or_else(|| string_at(finding, &["canonical_gap", "file"]))
        .map(ToString::to_string);
    let source_line = u64_at(finding, &["probe", "line"]);
    let changed_owner = string_at(card, &["changed_owner"])
        .or_else(|| string_at(finding, &["canonical_gap", "owner"]))
        .map(ToString::to_string);
    let suggested_test_file =
        string_at(card, &["suggested_location", "test_file"]).map(ToString::to_string);
    let suggested_test_name =
        string_at(card, &["suggested_location", "test_name"]).map(ToString::to_string);
    let related_test_line = first_related_test_line(finding);
    let verify_command = string_at(card, &["verify", "command"]).map(ToString::to_string);
    let repair_action = string_at(card, &["repair_action"]).unwrap_or("add_or_strengthen_test");
    let repairability = if suggested_test_file.is_some() && verify_command.is_some() {
        "repairable"
    } else {
        "unknown"
    };
    let anchor = GapAnchor {
        file: source_file,
        line: source_line,
        owner: changed_owner,
        dedupe_fingerprint: Some(canonical_gap_id.clone()),
    };
    let repair_route = if repairability == "repairable" {
        Some(GapRepairRoute {
            route_kind: python_route_kind(behavior_kind, repair_action).to_string(),
            target_file: suggested_test_file,
            target_line: related_test_line,
            related_test: suggested_test_name,
            assertion_shape: string_at(card, &["suggested_assertion"])
                .or_else(|| string_at(card, &["missing_discriminator"]))
                .map(ToString::to_string),
            missing_discriminator: string_at(card, &["missing_discriminator"])
                .map(ToString::to_string),
            changed_behavior: string_at(card, &["changed_behavior"]).map(ToString::to_string),
            stop_conditions: string_array_at(card, &["stop_conditions"]),
        })
    } else {
        None
    };
    let verification_commands = verify_command.into_iter().collect::<Vec<_>>();
    let projection_eligibility = projection_eligibility_from_pr_evidence(
        repairability,
        repair_route.is_some(),
        !verification_commands.is_empty(),
        anchor.file.is_some() && anchor.line.is_some(),
        "actionable",
    );
    let receipt_command = string_at(card, &["receipt", "command"]).map(ToString::to_string);
    let mut evidence_ids = Vec::new();
    if let Some(id) = string_at(finding, &["id"]) {
        evidence_ids.push(id.to_string());
    }
    if !evidence_ids.iter().any(|id| id == &canonical_gap_id) {
        evidence_ids.push(canonical_gap_id.clone());
    }

    Some(GapRecord {
        gap_id: format!("gap:pr:{canonical_gap_id}"),
        canonical_gap_id,
        kind: gap_kind_from_evidence("actionable", behavior_kind).to_string(),
        language: "python".to_string(),
        language_status: string_at(card, &["language_status"])
            .unwrap_or("preview")
            .to_string(),
        scope: "pr_local".to_string(),
        evidence_class: behavior_kind.to_string(),
        gap_state: "actionable".to_string(),
        policy_state: "new".to_string(),
        repairability: repairability.to_string(),
        repair_route,
        static_limit_kind: Some("python_preview".to_string()),
        static_limit_detail: Some("Python repair cards are preview advisory evidence.".to_string()),
        static_limits: string_array_at(card, &["limits"])
            .into_iter()
            .map(|limit| {
                serde_json::json!({
                    "kind": "python_preview_limit",
                    "detail": limit,
                })
            })
            .collect(),
        anchor: Some(anchor),
        evidence_ids,
        projection_eligibility,
        verification_commands,
        receipt_command,
        regeneration_commands: Vec::new(),
        receipt: None,
        safe_gate_predicate: None,
        authority_boundary: string_at(card, &["authority_boundary"])
            .unwrap_or("preview_advisory_only")
            .to_string(),
    })
}

fn gap_record_from_python_static_limit_finding(finding: &Value, index: usize) -> Option<GapRecord> {
    if string_at(finding, &["language"]) != Some("python") {
        return None;
    }
    let static_limit_kind = string_at(finding, &["static_limit_kind"])?;
    let behavior_kind = string_at(finding, &["canonical_gap", "behavior_kind"])
        .or_else(|| string_at(finding, &["probe", "family"]))
        .unwrap_or("python_static_limit");
    let source_file = string_at(finding, &["canonical_gap", "file"])
        .or_else(|| string_at(finding, &["probe", "file"]))
        .map(ToString::to_string);
    let source_line = u64_at(finding, &["probe", "line"]);
    let changed_owner = string_at(finding, &["canonical_gap", "owner"])
        .or_else(|| string_at(finding, &["probe", "owner"]))
        .map(ToString::to_string);
    let canonical_gap_id = string_at(finding, &["canonical_gap_id"])
        .or_else(|| string_at(finding, &["canonical_gap", "id"]))
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            python_static_limit_gap_id(
                source_file.as_deref(),
                changed_owner.as_deref(),
                behavior_kind,
                static_limit_kind,
                index,
            )
        });
    let static_limit_detail = python_static_limit_detail(finding, static_limit_kind);
    let has_local_anchor = source_file.is_some() && source_line.is_some();
    let anchor = GapAnchor {
        file: source_file,
        line: source_line,
        owner: changed_owner,
        dedupe_fingerprint: Some(canonical_gap_id.clone()),
    };
    let mut evidence_ids = Vec::new();
    if let Some(id) = string_at(finding, &["id"]) {
        evidence_ids.push(id.to_string());
    }
    if !evidence_ids.iter().any(|id| id == &canonical_gap_id) {
        evidence_ids.push(canonical_gap_id.clone());
    }

    Some(GapRecord {
        gap_id: format!("gap:pr:{canonical_gap_id}"),
        canonical_gap_id,
        kind: "StaticLimitation".to_string(),
        language: "python".to_string(),
        language_status: string_at(finding, &["language_status"])
            .unwrap_or("preview")
            .to_string(),
        scope: "pr_local".to_string(),
        evidence_class: static_limit_kind.to_string(),
        gap_state: "static_limitation".to_string(),
        policy_state: "not_policy_targeted".to_string(),
        repairability: "analyzer_limitation".to_string(),
        repair_route: None,
        static_limit_kind: Some(static_limit_kind.to_string()),
        static_limit_detail: Some(static_limit_detail.clone()),
        static_limits: vec![serde_json::json!({
            "kind": static_limit_kind,
            "detail": static_limit_detail,
        })],
        anchor: Some(anchor),
        evidence_ids,
        projection_eligibility: projection_eligibility_from_pr_evidence(
            "analyzer_limitation",
            false,
            false,
            has_local_anchor,
            "static_limitation",
        ),
        verification_commands: Vec::new(),
        receipt_command: None,
        regeneration_commands: Vec::new(),
        receipt: None,
        safe_gate_predicate: None,
        authority_boundary: "preview_advisory_only".to_string(),
    })
}

fn gap_record_from_python_no_action_finding(finding: &Value, index: usize) -> Option<GapRecord> {
    if string_at(finding, &["language"]) != Some("python") {
        return None;
    }
    if finding.get("python_repair_card").is_some()
        || string_at(finding, &["static_limit_kind"]).is_some()
    {
        return None;
    }

    let classification = string_at(finding, &["classification"])?;
    let (gap_state, kind) = match classification {
        "exposed" => ("already_observed", "NoActionAlreadyObserved"),
        "no_static_path" => ("no_related_test", "NoActionNoRelatedTest"),
        "weakly_exposed" if python_finding_is_heuristic_only(finding) => {
            ("heuristic_only", "NoActionHeuristicOnly")
        }
        _ => return None,
    };
    let behavior_kind = string_at(finding, &["canonical_gap", "behavior_kind"])
        .or_else(|| string_at(finding, &["probe", "family"]))
        .unwrap_or("python_no_action");
    let source_file = string_at(finding, &["canonical_gap", "file"])
        .or_else(|| string_at(finding, &["probe", "file"]))
        .map(ToString::to_string);
    let source_line = u64_at(finding, &["probe", "line"]);
    let changed_owner = string_at(finding, &["canonical_gap", "owner"])
        .or_else(|| string_at(finding, &["probe", "owner"]))
        .map(ToString::to_string);
    let canonical_gap_id = string_at(finding, &["canonical_gap_id"])
        .or_else(|| string_at(finding, &["canonical_gap", "id"]))
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("gap:python:check-output:item_{index}:{gap_state}"));
    let has_local_anchor = source_file.is_some() && source_line.is_some();
    let anchor = GapAnchor {
        file: source_file,
        line: source_line,
        owner: changed_owner,
        dedupe_fingerprint: Some(canonical_gap_id.clone()),
    };
    let mut evidence_ids = Vec::new();
    if let Some(id) = string_at(finding, &["id"]) {
        evidence_ids.push(id.to_string());
    }
    if !evidence_ids.iter().any(|id| id == &canonical_gap_id) {
        evidence_ids.push(canonical_gap_id.clone());
    }

    Some(GapRecord {
        gap_id: format!("gap:pr:{canonical_gap_id}"),
        canonical_gap_id,
        kind: kind.to_string(),
        language: "python".to_string(),
        language_status: string_at(finding, &["language_status"])
            .unwrap_or("preview")
            .to_string(),
        scope: "pr_local".to_string(),
        evidence_class: behavior_kind.to_string(),
        gap_state: gap_state.to_string(),
        policy_state: "not_policy_targeted".to_string(),
        repairability: "no_action".to_string(),
        repair_route: None,
        static_limit_kind: None,
        static_limit_detail: None,
        static_limits: Vec::new(),
        anchor: Some(anchor),
        evidence_ids,
        projection_eligibility: projection_eligibility_from_pr_evidence(
            "no_action",
            false,
            false,
            has_local_anchor,
            gap_state,
        ),
        verification_commands: Vec::new(),
        receipt_command: None,
        regeneration_commands: Vec::new(),
        receipt: None,
        safe_gate_predicate: None,
        authority_boundary: "preview_advisory_only".to_string(),
    })
}

fn python_finding_is_heuristic_only(finding: &Value) -> bool {
    string_array_at(finding, &["evidence"])
        .into_iter()
        .any(|item| item.starts_with("related_test_uncertain:"))
        || finding
            .get("ripr")
            .and_then(|ripr| string_at(ripr, &["reach", "summary"]))
            .is_some_and(|summary| summary.contains("heuristic Python test link"))
}

fn python_static_limit_gap_id(
    source_file: Option<&str>,
    changed_owner: Option<&str>,
    behavior_kind: &str,
    static_limit_kind: &str,
    index: usize,
) -> String {
    let file = source_file
        .map(|file| file.replace('\\', "/"))
        .filter(|file| !file.trim().is_empty())
        .unwrap_or_else(|| format!("check-output-item-{index}"));
    let owner = changed_owner
        .and_then(non_empty)
        .and_then(|owner| owner.rsplit("::").next())
        .map(python_static_limit_gap_component)
        .unwrap_or_else(|| "module".to_string());
    format!(
        "gap:python:{}:{}:static_limit:{}:{}",
        file,
        owner,
        python_static_limit_gap_component(behavior_kind),
        python_static_limit_gap_component(static_limit_kind)
    )
}

fn python_static_limit_gap_component(value: &str) -> String {
    let mut out = String::new();
    let mut last_separator = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
            out.push(ch.to_ascii_lowercase());
            last_separator = false;
        } else if !last_separator {
            out.push('_');
            last_separator = true;
        }
    }
    let component = out.trim_matches('_');
    if component.is_empty() {
        "unknown".to_string()
    } else {
        component.to_string()
    }
}

fn python_static_limit_detail(finding: &Value, static_limit_kind: &str) -> String {
    string_array_at(finding, &["missing"])
        .into_iter()
        .find_map(|detail| non_empty(&detail).map(ToString::to_string))
        .or_else(|| {
            string_array_at(finding, &["evidence"])
                .into_iter()
                .find(|detail| detail.contains("static_limit"))
        })
        .unwrap_or_else(|| {
            format!(
                "Python preview reported static limit `{static_limit_kind}` without a bounded repair route."
            )
        })
}

fn attach_check_output_python_receipt_routes(
    records: &mut [GapRecord],
    root: &str,
    before_check_output: &str,
) {
    let after_check_command = format!(
        "ripr check --root {} --json > {}",
        shell_arg(root),
        shell_arg(DEFAULT_CHECK_AFTER_OUTPUT)
    );

    for record in records {
        if record.language != "python"
            || record.language_status != "preview"
            || record.repairability != "repairable"
        {
            continue;
        }
        if record
            .receipt_command
            .as_deref()
            .and_then(non_empty)
            .is_none()
        {
            let receipt_path = default_python_receipt_path(record);
            record.receipt_command = Some(outcome_command(
                before_check_output,
                DEFAULT_CHECK_AFTER_OUTPUT,
                Some(&receipt_path),
            ));
        }
        if !record
            .regeneration_commands
            .iter()
            .any(|command| command == &after_check_command)
        {
            record
                .regeneration_commands
                .push(after_check_command.clone());
        }
    }
}

fn default_python_receipt_path(record: &GapRecord) -> String {
    let id = non_empty(&record.canonical_gap_id)
        .or_else(|| non_empty(&record.gap_id))
        .unwrap_or("python-gap");
    format!(
        "{}/{}.json",
        DEFAULT_RECEIPTS_DIR,
        receipt_slug_for_gap_id(id)
    )
}

fn receipt_slug_for_gap_id(id: &str) -> String {
    let mut slug = String::new();
    let mut last_was_separator = false;
    for ch in id.chars() {
        let next = if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.') {
            last_was_separator = false;
            Some(ch.to_ascii_lowercase())
        } else if !last_was_separator {
            last_was_separator = true;
            Some('-')
        } else {
            None
        };
        if let Some(next) = next {
            slug.push(next);
        }
        if slug.len() >= 120 {
            break;
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "python-gap".to_string()
    } else {
        slug.to_string()
    }
}

fn first_related_test_line(finding: &Value) -> Option<u64> {
    finding
        .get("related_tests")
        .and_then(Value::as_array)
        .and_then(|tests| tests.first())
        .and_then(|test| u64_at(test, &["line"]))
}

fn python_route_kind(behavior_kind: &str, repair_action: &str) -> &'static str {
    if repair_action == "strengthen_existing_test" {
        return "StrengthenExistingTest";
    }
    match behavior_kind {
        "predicate_boundary" | "match_arm" => "AddBoundaryAssertion",
        "exception_path" | "error_variant" => "AddErrorDiscriminator",
        "call_presence" | "side_effect" | "output_log" => "AddSideEffectObserver",
        "field_construction" | "field" | "dict_field" | "return_value" => "AddValueAssertion",
        _ => "AddValueAssertion",
    }
}

fn gap_record_from_finding_alignment_item(item: &Value, index: usize) -> Option<GapRecord> {
    let evidence_class = string_at(item, &["evidence_class"]).unwrap_or("unknown");
    if evidence_class != "presentation_text" {
        return None;
    }

    let gap_state = string_at(item, &["gap_state"]).unwrap_or("unknown");
    let actionability = string_at(item, &["actionability"]).unwrap_or("unknown");
    let repairability = repairability_from_evidence(gap_state, actionability);
    let canonical_gap_id = string_at(item, &["canonical_gap_id"])
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("presentation_text::item_{index}"));
    let presentation_text = item.get("presentation_text");
    let raw_findings = array_values_at(item, &["raw_findings"]).unwrap_or_default();
    let first_raw = raw_findings.first();
    let anchor = GapAnchor {
        file: first_raw.and_then(|raw| string_at(raw, &["file"]).map(ToString::to_string)),
        line: first_raw.and_then(|raw| u64_at(raw, &["line"])),
        owner: presentation_text
            .and_then(|text| string_at(text, &["constant_name"]))
            .map(ToString::to_string),
        dedupe_fingerprint: Some(canonical_gap_id.clone()),
    };
    let static_limits = array_values_at(item, &["static_limitations"]).unwrap_or_default();
    let static_limit_kind = static_limits
        .iter()
        .find_map(|limit| string_at(limit, &["category"]).or_else(|| string_at(limit, &["kind"])))
        .map(ToString::to_string);
    let static_limit_detail = static_limits
        .iter()
        .find_map(|limit| {
            string_at(limit, &["repair_route"])
                .or_else(|| string_at(limit, &["detail"]))
                .or_else(|| string_at(limit, &["reason"]))
        })
        .map(ToString::to_string);
    let repair_route = presentation_text_repair_route_from_alignment_item(item, repairability);
    let verification_commands =
        verification_commands_from_alignment_item(item, evidence_class, repairability);
    let projection_eligibility = projection_eligibility_from_pr_evidence(
        repairability,
        repair_route.is_some(),
        !verification_commands.is_empty(),
        anchor.file.is_some() && anchor.line.is_some(),
        gap_state,
    );

    Some(GapRecord {
        gap_id: format!("gap:pr:{canonical_gap_id}"),
        canonical_gap_id: canonical_gap_id.clone(),
        kind: gap_kind_from_evidence(gap_state, evidence_class).to_string(),
        language: "rust".to_string(),
        language_status: "stable".to_string(),
        scope: "pr_local".to_string(),
        evidence_class: evidence_class.to_string(),
        gap_state: gap_state.to_string(),
        policy_state: if gap_state == "actionable" {
            "new".to_string()
        } else {
            "not_policy_targeted".to_string()
        },
        repairability: repairability.to_string(),
        repair_route,
        static_limit_kind,
        static_limit_detail,
        static_limits,
        anchor: Some(anchor),
        evidence_ids: evidence_ids_from_alignment_item(item, &canonical_gap_id),
        projection_eligibility,
        verification_commands,
        receipt_command: None,
        regeneration_commands: Vec::new(),
        receipt: None,
        safe_gate_predicate: None,
        authority_boundary: "gate_decision_artifact_only".to_string(),
    })
}

fn presentation_text_repair_route_from_alignment_item(
    item: &Value,
    repairability: &str,
) -> Option<GapRepairRoute> {
    if repairability != "repairable" {
        return None;
    }
    let presentation_text = item.get("presentation_text");
    Some(GapRepairRoute {
        route_kind: "AddOutputGolden".to_string(),
        target_file: string_at(item, &["related_test", "file"]).map(ToString::to_string),
        target_line: u64_at(item, &["related_test", "line"]),
        related_test: string_at(item, &["related_test", "name"]).map(ToString::to_string),
        assertion_shape: presentation_text
            .and_then(|text| string_at(text, &["suggested_assertion"]))
            .or_else(|| string_at(item, &["recommended_repair"]))
            .map(ToString::to_string),
        missing_discriminator: None,
        changed_behavior: presentation_text
            .and_then(|text| string_at(text, &["text_literal"]))
            .or_else(|| presentation_text.and_then(|text| string_at(text, &["constant_name"])))
            .map(ToString::to_string),
        stop_conditions: vec![
            "Stop if the changed text is not user-facing in the current product surface."
                .to_string(),
            "Stop if the output/golden fixture would assert unrelated formatting churn."
                .to_string(),
        ],
    })
}

fn verification_commands_from_alignment_item(
    item: &Value,
    evidence_class: &str,
    repairability: &str,
) -> Vec<String> {
    if evidence_class == "presentation_text" && repairability == "repairable" {
        return vec!["cargo xtask goldens check".to_string()];
    }
    string_at(item, &["verify_command"])
        .map(|command| vec![command.to_string()])
        .unwrap_or_default()
}

fn evidence_ids_from_alignment_item(item: &Value, canonical_gap_id: &str) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(raw_findings) = array_values_at(item, &["raw_findings"]) {
        for raw in raw_findings {
            if let Some(id) = string_at(&raw, &["evidence_record_ref"])
                .or_else(|| string_at(&raw, &["source_id"]))
                && !ids.iter().any(|existing| existing == id)
            {
                ids.push(id.to_string());
            }
        }
    }
    if ids.is_empty() {
        ids.push(canonical_gap_id.to_string());
    }
    ids
}

fn repair_route_from_evidence(
    evidence: &Value,
    canonical_item: &Value,
    seam_kind: &str,
    repairability: &str,
) -> Option<GapRepairRoute> {
    if repairability != "repairable" {
        return None;
    }
    let route_kind = match seam_kind {
        "predicate_boundary" | "match_arm" => "AddBoundaryAssertion",
        "error_variant" => "AddErrorAssertion",
        "field_construction" | "return_value" => "AddValueAssertion",
        "call_presence" | "side_effect" => "AddSideEffectObserver",
        _ => "AddValueAssertion",
    };
    Some(GapRepairRoute {
        route_kind: route_kind.to_string(),
        target_file: string_at(canonical_item, &["related_test", "file"])
            .or_else(|| string_at(evidence, &["recommendation", "recommended_test", "file"]))
            .map(ToString::to_string),
        target_line: u64_at(canonical_item, &["related_test", "line"]),
        related_test: string_at(canonical_item, &["related_test", "name"]).map(ToString::to_string),
        assertion_shape: string_at(evidence, &["recommendation", "assertion_shape", "example"])
            .or_else(|| string_at(canonical_item, &["recommended_repair"]))
            .map(ToString::to_string),
        missing_discriminator: string_at(canonical_item, &["missing_discriminator"])
            .or_else(|| string_at(evidence, &["recommendation", "missing_discriminator"]))
            .map(ToString::to_string),
        changed_behavior: first_raw_finding_expression(evidence).map(ToString::to_string),
        stop_conditions: vec![
            "Stop if the related test is outside the current workspace.".to_string(),
            "Stop if the suggested assertion would require changing production behavior first."
                .to_string(),
        ],
    })
}

fn first_raw_finding_expression(evidence: &Value) -> Option<&str> {
    evidence
        .get("raw_findings")
        .and_then(Value::as_array)?
        .first()
        .and_then(|finding| finding.get("expression"))
        .and_then(Value::as_str)
}

fn projection_eligibility_from_repo_evidence(
    repairability: &str,
    has_repair_route: bool,
    has_verify_command: bool,
    has_local_anchor: bool,
    gap_state: &str,
) -> BTreeMap<String, ProjectionEligibility> {
    let mut projections = BTreeMap::new();
    insert_projection(
        &mut projections,
        "ci_summary",
        true,
        "repo_scoped_gap_record",
    );
    insert_projection(
        &mut projections,
        "report_packet",
        true,
        "all_gap_records_are_reportable",
    );
    insert_projection(
        &mut projections,
        "pr_comment",
        false,
        "repo_scoped_not_pr_local",
    );
    insert_projection(
        &mut projections,
        "gate_candidate",
        false,
        "repo_scoped_not_pr_local",
    );
    let repairable = repairability == "repairable" && has_repair_route && has_verify_command;
    insert_projection(
        &mut projections,
        "agent_packet",
        repairable,
        if repairable {
            "bounded_repair_route"
        } else {
            "not_repairable"
        },
    );
    insert_projection(
        &mut projections,
        "lsp_diagnostic",
        repairable && has_local_anchor,
        if repairable && has_local_anchor {
            "local_file_scope"
        } else {
            "not_repairable_or_missing_anchor"
        },
    );
    insert_projection(
        &mut projections,
        "ripr_zero_count",
        repairable && gap_state == "actionable",
        if repairable && gap_state == "actionable" {
            "repo_scoped_policy_targeted_rust_gap"
        } else {
            "not_unresolved_repairable_repo_gap"
        },
    );
    insert_projection(
        &mut projections,
        "ripr_plus_count",
        repairable && gap_state == "actionable",
        if repairable && gap_state == "actionable" {
            "repo_scoped_advisory_rust_gap"
        } else {
            "not_unresolved_repairable_repo_gap"
        },
    );
    projections
}

fn projection_eligibility_from_pr_evidence(
    repairability: &str,
    has_repair_route: bool,
    has_verify_command: bool,
    has_local_anchor: bool,
    gap_state: &str,
) -> BTreeMap<String, ProjectionEligibility> {
    let mut projections = BTreeMap::new();
    insert_projection(&mut projections, "ci_summary", true, "pr_local_gap_record");
    insert_projection(
        &mut projections,
        "report_packet",
        true,
        "all_gap_records_are_reportable",
    );
    let repairable = repairability == "repairable" && has_repair_route && has_verify_command;
    insert_projection(
        &mut projections,
        "pr_comment",
        repairable && has_local_anchor,
        if repairable && has_local_anchor {
            "stable_anchor_and_repair_route"
        } else {
            "not_repairable_or_missing_anchor"
        },
    );
    insert_projection(
        &mut projections,
        "gate_candidate",
        false,
        "policy_target_not_supplied",
    );
    insert_projection(
        &mut projections,
        "agent_packet",
        repairable,
        if repairable {
            "bounded_repair_route"
        } else {
            "not_repairable"
        },
    );
    insert_projection(
        &mut projections,
        "lsp_diagnostic",
        repairable && has_local_anchor,
        if repairable && has_local_anchor {
            "local_file_scope"
        } else {
            "not_repairable_or_missing_anchor"
        },
    );
    insert_projection(
        &mut projections,
        "ripr_zero_count",
        false,
        "pr_local_not_repo_scoped",
    );
    insert_projection(
        &mut projections,
        "ripr_plus_count",
        false,
        "pr_local_not_repo_scoped",
    );
    if gap_state != "actionable" {
        insert_projection(&mut projections, "pr_comment", false, "not_actionable_gap");
    }
    projections
}

fn insert_projection(
    projections: &mut BTreeMap<String, ProjectionEligibility>,
    name: &str,
    eligible: bool,
    reason: &str,
) {
    projections.insert(
        name.to_string(),
        ProjectionEligibility {
            eligible,
            reason: reason.to_string(),
        },
    );
}

fn evidence_ids_from_repo_evidence(evidence: &Value, seam_id: &str) -> Vec<String> {
    let mut ids = vec![seam_id.to_string()];
    if let Some(raw_findings) = evidence.get("raw_findings").and_then(Value::as_array) {
        for finding in raw_findings {
            if let Some(source_id) = finding.get("source_id").and_then(Value::as_str)
                && !ids.iter().any(|id| id == source_id)
            {
                ids.push(source_id.to_string());
            }
        }
    }
    ids
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

pub(crate) fn safe_gate_predicate_satisfied(record: &GapRecord) -> bool {
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

pub(crate) fn projection_eligible(record: &GapRecord, projection: &str) -> bool {
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
    if let Some(command) = &record.receipt_command {
        out.push_str("- Receipt:\n");
        out.push_str(&format!("  - `{}`\n", md_inline(command)));
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

    fn python_static_limit_check_output() -> String {
        include_str!("../../../../fixtures/python_dynamic_dispatch_limit/expected/check.json")
            .to_string()
    }

    fn python_dynamic_import_static_limit_check_output() -> String {
        include_str!("../../../../fixtures/python_dynamic_import_limit/expected/check.json")
            .to_string()
    }

    fn minimal_record() -> Value {
        serde_json::json!({
            "gap_id": "gap:minimal",
            "canonical_gap_id": "gap:minimal",
            "kind": "AlreadyObserved",
            "language": "rust",
            "language_status": "stable",
            "scope": "repo_scoped",
            "evidence_class": "already_observed",
            "gap_state": "already_improved",
            "policy_state": "baseline",
            "repairability": "no_action",
            "projection_eligibility": {
                "agent_packet": {"eligible": false, "reason": "already_observed"}
            },
            "authority_boundary": "gate_decision_artifact_only"
        })
    }

    fn report_from_json(value: Value) -> GapDecisionLedgerReport {
        build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: ".".to_string(),
            generated_at: "test".to_string(),
            source_kind: GapDecisionLedgerSourceKind::Records,
            records_path: "records.json".to_string(),
            records_json: Ok(value.to_string()),
        })
    }

    fn report_from_check_output(value: Value) -> GapDecisionLedgerReport {
        build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: ".".to_string(),
            generated_at: "test".to_string(),
            source_kind: GapDecisionLedgerSourceKind::CheckOutput,
            records_path: "check.json".to_string(),
            records_json: Ok(value.to_string()),
        })
    }

    #[test]
    fn gap_decision_ledger_parses_corpus_records_and_summarizes_projection_boundaries() {
        let report = build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: ".".to_string(),
            generated_at: "test".to_string(),
            source_kind: GapDecisionLedgerSourceKind::Records,
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
    fn gap_decision_ledger_accepts_supported_record_roots() {
        let raw_array = report_from_json(serde_json::json!([minimal_record()]));
        assert_eq!(raw_array.status, "advisory");
        assert_eq!(raw_array.summary.records_total, 1);

        let records = report_from_json(serde_json::json!({"records": [minimal_record()]}));
        assert_eq!(records.status, "advisory");
        assert_eq!(records.summary.no_action_total, 1);

        let gap_records = report_from_json(serde_json::json!({"gap_records": [minimal_record()]}));
        assert_eq!(gap_records.status, "advisory");
        assert_eq!(gap_records.summary.no_action_total, 1);
    }

    #[test]
    fn gap_decision_ledger_derives_missing_output_contract_from_check_output_alignment() {
        let report = report_from_check_output(serde_json::json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "finding_alignment": {
                "scope": "supported_classes",
                "items": [
                    {
                        "canonical_gap_id": "presentation_text::HELP_DEVICE_LABEL",
                        "canonical_item_kind": "gap",
                        "evidence_class": "presentation_text",
                        "gap_state": "actionable",
                        "actionability": "add_output_observer",
                        "raw_group_size": 2,
                        "group_reason": "declaration_and_literal_same_text_constant",
                        "why": "Changed text flows to CLI help output and no supported output observer is found.",
                        "recommended_repair": "Add or update a help-output snapshot assertion for HELP_DEVICE_LABEL.",
                        "related_test": null,
                        "verify_command": "cargo xtask evidence-quality-scorecard",
                        "static_limitations": [],
                        "confidence": {
                            "basis": "fixture_backed",
                            "notes": ["Visible unobserved presentation text is actionable only for supported sink patterns."]
                        },
                        "raw_findings": [
                            {
                                "file": "crates/ripr/src/cli/help.rs",
                                "line": 42,
                                "kind": "exposed",
                                "expression": "pub const HELP_DEVICE_LABEL: &str =",
                                "probe_kind": "field_construction",
                                "source_id": "help-label-decl",
                                "evidence_record_ref": "help-label-decl"
                            },
                            {
                                "file": "crates/ripr/src/cli/help.rs",
                                "line": 43,
                                "kind": "static_unknown",
                                "expression": "\"Device label\";",
                                "probe_kind": "static_unknown",
                                "source_id": "help-label-literal",
                                "evidence_record_ref": "help-label-literal"
                            }
                        ],
                        "presentation_text": {
                            "constant_name": "HELP_DEVICE_LABEL",
                            "text_literal": "Device label",
                            "visibility": "user_visible",
                            "observer": "none",
                            "actionability": "add_output_observer",
                            "source_kind": "const_decl",
                            "canonical_group_reason": "declaration_and_literal_same_text_constant",
                            "recommended_observer": "cli_help_output",
                            "repair_kind": "output_observer",
                            "target_test_type": "help_output_snapshot",
                            "suggested_assertion": "Assert CLI help output includes the HELP_DEVICE_LABEL text."
                        }
                    }
                ]
            }
        }));

        assert_eq!(report.status, "advisory");
        assert_eq!(report.summary.records_total, 1);
        assert_eq!(report.summary.repairable_total, 1);
        assert_eq!(report.summary.missing_output_contract_total, 1);
        assert_eq!(report.summary.projection_pr_comment_eligible, 1);
        assert_eq!(report.summary.projection_gate_candidate, 0);
        assert_eq!(report.summary.ripr_zero_target_count, 0);
        let record = &report.records[0];
        assert_eq!(record.kind, "MissingOutputContract");
        assert_eq!(record.scope, "pr_local");
        assert_eq!(
            record
                .repair_route
                .as_ref()
                .map(|route| route.route_kind.as_str()),
            Some("AddOutputGolden")
        );
        assert_eq!(
            record.verification_commands,
            vec!["cargo xtask goldens check".to_string()]
        );
        assert_eq!(
            record
                .projection_eligibility
                .get("pr_comment")
                .map(|projection| projection.eligible),
            Some(true)
        );
        assert_eq!(
            record.evidence_ids,
            vec![
                "help-label-decl".to_string(),
                "help-label-literal".to_string()
            ]
        );
    }

    #[test]
    fn gap_decision_ledger_renders_json_and_markdown() -> Result<(), String> {
        let report = build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: ".".to_string(),
            generated_at: "test".to_string(),
            source_kind: GapDecisionLedgerSourceKind::Records,
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
    fn gap_decision_ledger_renders_optional_markdown_fields_and_escapes_inline_text() {
        let report = report_from_json(serde_json::json!({
            "records": [
                {
                    "gap_id": "gap:`pipe|line",
                    "canonical_gap_id": "gap:escaped",
                    "kind": "MissingArtifact",
                    "language": "rust",
                    "language_status": "stable",
                    "scope": "artifact_missing",
                    "evidence_class": "missing_artifact",
                    "gap_state": "missing_artifact",
                    "policy_state": "not_policy_targeted",
                    "repairability": "repairable",
                    "repair_route": {
                        "route_kind": "RegenerateArtifact",
                        "target_file": "target/ripr/reports/index.md",
                        "assertion_shape": "report contains `start|here`"
                    },
                    "anchor": {
                        "file": "docs|OUTPUT_SCHEMA.md",
                        "line": 7,
                        "owner": "output::`schema`",
                        "dedupe_fingerprint": "gap:escaped"
                    },
                    "projection_eligibility": {
                        "agent_packet": {"eligible": true, "reason": "repair_command_present"}
                    },
                    "verification_commands": ["cargo xtask check-output-contracts"],
                    "regeneration_commands": ["cargo xtask reports\nindex"],
                    "receipt": {"movement": "missing_receipt"}
                }
            ]
        }));

        let markdown = render_gap_decision_ledger_markdown(&report);
        assert!(markdown.contains("gap:'pipe\\|line"));
        assert!(markdown.contains("docs\\|OUTPUT_SCHEMA.md`:7"));
        assert!(markdown.contains("owner `output::'schema'`"));
        assert!(markdown.contains("Assertion or observer: `report contains 'start\\|here'`"));
        assert!(markdown.contains("cargo xtask reports index"));
        assert!(markdown.contains("Receipt movement: `missing_receipt`"));
    }

    #[test]
    fn gap_decision_ledger_reports_bad_or_missing_records_as_blocked() {
        let report = build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: ".".to_string(),
            generated_at: "test".to_string(),
            source_kind: GapDecisionLedgerSourceKind::Records,
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
    fn gap_decision_ledger_reports_malformed_record_inputs() {
        let invalid_json = build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: ".".to_string(),
            generated_at: "test".to_string(),
            source_kind: GapDecisionLedgerSourceKind::Records,
            records_path: "bad.json".to_string(),
            records_json: Ok("{".to_string()),
        });
        assert_eq!(invalid_json.status, "blocked");
        assert!(invalid_json.warnings[0].contains("invalid JSON"));

        let wrong_root = report_from_json(serde_json::json!("not records"));
        assert_eq!(wrong_root.status, "blocked");
        assert!(wrong_root.warnings[0].contains("expected object or array"));

        let missing_case_record = report_from_json(serde_json::json!({
            "cases": [{"id": "missing"}]
        }));
        assert_eq!(missing_case_record.status, "blocked");
        assert!(missing_case_record.warnings[0].contains("missing expected_gap_record"));
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
            source_kind: GapDecisionLedgerSourceKind::Records,
            records_path: "bad.json".to_string(),
            records_json: Ok(record.to_string()),
        });

        let warnings = report.warnings.join("\n");
        assert!(warnings.contains("repairable but missing repair_route"));
        assert!(warnings.contains("repairable but missing verification_commands"));
        assert!(warnings.contains("gate-candidate eligible but safe_gate_predicate is incomplete"));
        assert!(warnings.contains("preview evidence but eligible for gate or badge authority"));
    }

    // ── repairability_from_evidence ──────────────────────────────────────────

    #[test]
    fn repairability_from_evidence_covers_all_branches() {
        // actionable + supported actionability values → repairable
        for actionability in &[
            "add_focused_test",
            "upgrade_assertion",
            "extend_related_test",
            "add_output_observer",
        ] {
            assert_eq!(
                repairability_from_evidence("actionable", actionability),
                "repairable",
                "expected repairable for actionability={actionability}"
            );
        }
        // actionable + unsupported actionability → unknown
        assert_eq!(
            repairability_from_evidence("actionable", "some_other_kind"),
            "unknown"
        );
        // already_observed / internal_only → no_action
        assert_eq!(
            repairability_from_evidence("already_observed", "anything"),
            "no_action"
        );
        assert_eq!(
            repairability_from_evidence("internal_only", "anything"),
            "no_action"
        );
        // static_limitation → analyzer_limitation
        assert_eq!(
            repairability_from_evidence("static_limitation", "anything"),
            "analyzer_limitation"
        );
        // unknown gap_state → unknown
        assert_eq!(
            repairability_from_evidence("unrecognized_state", "x"),
            "unknown"
        );
    }

    // ── gap_kind_from_evidence ───────────────────────────────────────────────

    #[test]
    fn gap_kind_from_evidence_covers_all_branches() {
        assert_eq!(
            gap_kind_from_evidence("already_observed", "anything"),
            "NoActionAlreadyObserved"
        );
        assert_eq!(
            gap_kind_from_evidence("internal_only", "anything"),
            "NoActionInternal"
        );
        assert_eq!(
            gap_kind_from_evidence("static_limitation", "anything"),
            "StaticLimitation"
        );
        assert_eq!(
            gap_kind_from_evidence("actionable", "presentation_text"),
            "MissingOutputContract"
        );
        for seam_kind in &["predicate_boundary", "match_arm"] {
            assert_eq!(
                gap_kind_from_evidence("actionable", seam_kind),
                "MissingBoundaryAssertion",
                "expected MissingBoundaryAssertion for seam_kind={seam_kind}"
            );
        }
        assert_eq!(
            gap_kind_from_evidence("actionable", "error_variant"),
            "MissingErrorDiscriminator"
        );
        for seam_kind in &["field_construction", "return_value"] {
            assert_eq!(
                gap_kind_from_evidence("actionable", seam_kind),
                "MissingValueAssertion",
                "expected MissingValueAssertion for seam_kind={seam_kind}"
            );
        }
        for seam_kind in &["call_presence", "side_effect"] {
            assert_eq!(
                gap_kind_from_evidence("actionable", seam_kind),
                "MissingSideEffectObserver",
                "expected MissingSideEffectObserver for seam_kind={seam_kind}"
            );
        }
        assert_eq!(
            gap_kind_from_evidence("actionable", "unrecognized"),
            "Unknown"
        );
        assert_eq!(gap_kind_from_evidence("unknown_state", "x"), "Unknown");
    }

    // ── md_inline ───────────────────────────────────────────────────────────

    #[test]
    fn md_inline_escapes_special_characters() {
        assert_eq!(md_inline("plain"), "plain");
        assert_eq!(md_inline("a|b"), "a\\|b");
        assert_eq!(md_inline("a`b"), "a'b");
        assert_eq!(md_inline("a\nb"), "a b");
        assert_eq!(md_inline("a\rb"), "a b");
        assert_eq!(md_inline("a`b|c\nd"), "a'b\\|c d");
        assert_eq!(md_inline(""), "");
    }

    // ── fallback_gap_id ──────────────────────────────────────────────────────

    #[test]
    fn fallback_gap_id_returns_unknown_for_blank_gap_id() {
        let blank = GapRecord {
            gap_id: "   ".to_string(),
            ..GapRecord::default()
        };
        assert_eq!(fallback_gap_id(&blank), "unknown-gap");

        let empty = GapRecord {
            gap_id: String::new(),
            ..GapRecord::default()
        };
        assert_eq!(fallback_gap_id(&empty), "unknown-gap");

        let with_id = GapRecord {
            gap_id: "gap:real".to_string(),
            ..GapRecord::default()
        };
        assert_eq!(fallback_gap_id(&with_id), "gap:real");
    }

    // ── GapDecisionLedgerSourceKind::as_str ─────────────────────────────────

    #[test]
    fn source_kind_as_str_returns_correct_values() {
        assert_eq!(GapDecisionLedgerSourceKind::Records.as_str(), "records");
        assert_eq!(
            GapDecisionLedgerSourceKind::RepoExposure.as_str(),
            "repo_exposure"
        );
        assert_eq!(
            GapDecisionLedgerSourceKind::CheckOutput.as_str(),
            "check_output"
        );
    }

    // ── gap_records_from_repo_exposure_json ──────────────────────────────────

    fn make_repo_exposure_seam(
        gap_state: &str,
        actionability: &str,
        seam_kind: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "seam_id": "seam:test",
            "evidence_record": {
                "seam_id": "seam:test",
                "seam_kind": seam_kind,
                "canonical_gap_id": "gap:rust:seam:test",
                "location": {"file": "src/lib.rs", "line": 10},
                "owner": "my_crate",
                "raw_findings": [
                    {"source_id": "finding-001", "expression": "fn foo()"}
                ],
                "recommendation": {
                    "assertion_shape": {"example": "assert_eq!(foo(), expected)"},
                    "recommended_test": {"file": "tests/lib_test.rs"},
                    "verify_command": "cargo test foo"
                },
                "canonical_item": {
                    "gap_state": gap_state,
                    "actionability": actionability,
                    "evidence_class": seam_kind,
                    "related_test": {"name": "test_foo", "file": "tests/lib_test.rs", "line": 5},
                    "verify_command": "cargo test"
                }
            }
        })
    }

    #[test]
    fn repo_exposure_json_parses_seam_to_record() -> Result<(), String> {
        let payload = serde_json::json!({
            "seams": [make_repo_exposure_seam("actionable", "add_focused_test", "predicate_boundary")]
        });
        let records = gap_records_from_repo_exposure_json(&payload.to_string())
            .map_err(|e| format!("parse failed: {e}"))?;
        assert_eq!(records.len(), 1);
        let r = &records[0];
        assert_eq!(r.repairability, "repairable");
        assert_eq!(r.kind, "MissingBoundaryAssertion");
        assert_eq!(r.language, "rust");
        assert_eq!(r.scope, "repo_scoped");
        assert_eq!(r.policy_state, "new");
        let Some(route) = &r.repair_route else {
            return Err("expected repair_route".to_string());
        };
        assert_eq!(route.route_kind, "AddBoundaryAssertion");
        assert!(r.evidence_ids.contains(&"seam:test".to_string()));
        assert!(r.evidence_ids.contains(&"finding-001".to_string()));
        let anchor = r.anchor.as_ref().ok_or("expected anchor")?;
        assert_eq!(anchor.file.as_deref(), Some("src/lib.rs"));
        assert_eq!(anchor.line, Some(10));
        Ok(())
    }

    #[test]
    fn repo_exposure_json_error_paths() -> Result<(), String> {
        // invalid JSON
        let err_msg = gap_records_from_repo_exposure_json("not-json")
            .err()
            .ok_or("expected error for invalid JSON")?;
        assert!(
            err_msg.contains("invalid JSON"),
            "unexpected error: {err_msg}"
        );

        // missing seams array
        let err_msg = gap_records_from_repo_exposure_json(r#"{"other": []}"#)
            .err()
            .ok_or("expected error for missing seams")?;
        assert!(
            err_msg.contains("seams array"),
            "unexpected error: {err_msg}"
        );

        // seam with no evidence_record → skipped (not an error)
        let payload = serde_json::json!({"seams": [{"seam_id": "bare"}]});
        let records = gap_records_from_repo_exposure_json(&payload.to_string())
            .map_err(|e| format!("unexpected error: {e}"))?;
        assert!(records.is_empty());
        Ok(())
    }

    #[test]
    fn repo_exposure_json_all_seam_kinds_produce_correct_routes() -> Result<(), String> {
        let cases = [
            ("match_arm", "AddBoundaryAssertion"),
            ("error_variant", "AddErrorAssertion"),
            ("return_value", "AddValueAssertion"),
            ("field_construction", "AddValueAssertion"),
            ("call_presence", "AddSideEffectObserver"),
            ("side_effect", "AddSideEffectObserver"),
            ("unknown_kind", "AddValueAssertion"),
        ];
        for (seam_kind, expected_route) in &cases {
            let payload = serde_json::json!({
                "seams": [make_repo_exposure_seam("actionable", "add_focused_test", seam_kind)]
            });
            let records = gap_records_from_repo_exposure_json(&payload.to_string())
                .map_err(|e| format!("parse failed for {seam_kind}: {e}"))?;
            let route_kind = records
                .first()
                .and_then(|r| r.repair_route.as_ref())
                .map(|r| r.route_kind.as_str())
                .ok_or_else(|| format!("missing repair_route for {seam_kind}"))?;
            assert_eq!(
                route_kind, *expected_route,
                "wrong route kind for seam_kind={seam_kind}"
            );
        }
        Ok(())
    }

    #[test]
    fn repo_exposure_seam_non_actionable_states_produce_no_repair_route() -> Result<(), String> {
        for gap_state in &["already_observed", "internal_only", "static_limitation"] {
            let payload = serde_json::json!({
                "seams": [make_repo_exposure_seam(gap_state, "add_focused_test", "predicate_boundary")]
            });
            let records = gap_records_from_repo_exposure_json(&payload.to_string())
                .map_err(|e| format!("parse failed for {gap_state}: {e}"))?;
            let r = records.first().ok_or("expected record")?;
            assert!(
                r.repair_route.is_none(),
                "expected no repair_route for gap_state={gap_state}"
            );
            assert_eq!(
                r.policy_state, "not_policy_targeted",
                "wrong policy_state for gap_state={gap_state}"
            );
        }
        Ok(())
    }

    #[test]
    fn repo_exposure_seam_static_limits_populated_from_canonical_item() -> Result<(), String> {
        let payload = serde_json::json!({
            "seams": [{
                "evidence_record": {
                    "seam_id": "s1",
                    "seam_kind": "predicate_boundary",
                    "canonical_item": {
                        "gap_state": "static_limitation",
                        "actionability": "none",
                        "static_limit_kind": "cfg_gate",
                        "static_limit_detail": "hidden behind #[cfg(test)]",
                        "static_limits": [
                            {"kind": "cfg_gate", "detail": "hidden behind #[cfg(test)]"}
                        ]
                    }
                }
            }]
        });
        let records = gap_records_from_repo_exposure_json(&payload.to_string())
            .map_err(|e| format!("parse failed: {e}"))?;
        let r = records.first().ok_or("expected record")?;
        assert_eq!(r.static_limit_kind.as_deref(), Some("cfg_gate"));
        assert!(
            r.static_limit_detail
                .as_deref()
                .unwrap_or("")
                .contains("cfg(test)")
        );
        assert!(!r.static_limits.is_empty());
        Ok(())
    }

    #[test]
    fn repo_exposure_seam_static_limits_fallback_from_evidence_limits() -> Result<(), String> {
        let payload = serde_json::json!({
            "seams": [{
                "evidence_record": {
                    "seam_id": "s2",
                    "seam_kind": "predicate_boundary",
                    "static_limits": [
                        {"kind": "opaque_body", "reason": "body not available"}
                    ],
                    "canonical_item": {
                        "gap_state": "static_limitation",
                        "actionability": "none"
                    }
                }
            }]
        });
        let records = gap_records_from_repo_exposure_json(&payload.to_string())
            .map_err(|e| format!("parse failed: {e}"))?;
        let r = records.first().ok_or("expected record")?;
        assert_eq!(r.static_limit_kind.as_deref(), Some("opaque_body"));
        assert_eq!(r.static_limit_detail.as_deref(), Some("body not available"));
        Ok(())
    }

    #[test]
    fn repo_exposure_seam_fallback_canonical_gap_id_when_missing() -> Result<(), String> {
        let payload = serde_json::json!({
            "seams": [{
                "seam_id": "outer-seam",
                "evidence_record": {
                    "canonical_item": {
                        "gap_state": "already_observed",
                        "actionability": "none"
                    }
                }
            }]
        });
        let records = gap_records_from_repo_exposure_json(&payload.to_string())
            .map_err(|e| format!("parse failed: {e}"))?;
        let r = records.first().ok_or("expected record")?;
        assert_eq!(r.canonical_gap_id, "gap:rust:outer-seam");
        assert_eq!(r.gap_id, "gap:repo:gap:rust:outer-seam");
        Ok(())
    }

    // ── gap_records_from_check_output_json additional error paths ────────────

    #[test]
    fn check_output_json_error_paths() -> Result<(), String> {
        // invalid JSON
        let err_msg = gap_records_from_check_output_json("bad json")
            .err()
            .ok_or("expected error for invalid JSON")?;
        assert!(
            err_msg.contains("invalid JSON"),
            "unexpected error: {err_msg}"
        );

        // missing finding_alignment
        let err_msg = gap_records_from_check_output_json(r#"{"other": {}}"#)
            .err()
            .ok_or("expected error for missing finding_alignment")?;
        assert!(
            err_msg.contains("finding_alignment.items or findings array"),
            "unexpected error: {err_msg}"
        );

        // non-presentation_text items are skipped
        let payload = serde_json::json!({
            "finding_alignment": {
                "items": [
                    {"evidence_class": "predicate_boundary", "canonical_gap_id": "g1"}
                ]
            }
        });
        let records = gap_records_from_check_output_json(&payload.to_string())
            .map_err(|e| format!("unexpected error: {e}"))?;
        assert!(records.is_empty());

        // presentation_text item produces a valid record
        let payload = serde_json::json!({
            "finding_alignment": {
                "items": [
                    {
                        "evidence_class": "presentation_text",
                        "gap_state": "actionable",
                        "actionability": "add_output_observer",
                        "canonical_gap_id": "pt::MY_CONST",
                        "raw_findings": [{"file": "src/lib.rs", "line": 1}],
                        "presentation_text": {
                            "constant_name": "MY_CONST",
                            "text_literal": "hello",
                            "suggested_assertion": "assert output includes MY_CONST"
                        },
                        "related_test": {"name": "test_help", "file": "tests/help.rs", "line": 10}
                    }
                ]
            }
        });
        let records = gap_records_from_check_output_json(&payload.to_string())
            .map_err(|e| format!("unexpected error: {e}"))?;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, "MissingOutputContract");
        Ok(())
    }

    #[test]
    fn check_output_python_repair_card_becomes_agent_packet_gap_record() -> Result<(), String> {
        let payload = serde_json::json!({
            "findings": [{
                "id": "probe:src_pricing.py:2:python_preview",
                "canonical_gap_id": "gap:python:src/pricing.py:calculate_discount:predicate_boundary:predicate:amount>=threshold",
                "canonical_gap": {
                    "file": "src/pricing.py",
                    "owner": "calculate_discount",
                    "behavior_kind": "predicate_boundary"
                },
                "probe": {
                    "file": "src/pricing.py",
                    "line": 2,
                    "family": "predicate"
                },
                "related_tests": [{
                    "name": "test_calculate_discount_smoke",
                    "file": "tests/test_pricing.py",
                    "line": 4
                }],
                "python_repair_card": {
                    "canonical_gap_id": "gap:python:src/pricing.py:calculate_discount:predicate_boundary:predicate:amount>=threshold",
                    "language": "python",
                    "language_status": "preview",
                    "authority_boundary": "preview_advisory_only",
                    "repair_action": "strengthen_existing_test",
                    "changed_owner": "calculate_discount",
                    "changed_behavior": "predicate_boundary changed at src/pricing.py:2: `if amount >= threshold:`",
                    "missing_discriminator": "amount == threshold",
                    "suggested_assertion": "Assert the owner result or effect at the boundary `amount == threshold`.",
                    "suggested_location": {
                        "test_file": "tests/test_pricing.py",
                        "test_name": "test_calculate_discount_smoke"
                    },
                    "verify": {
                        "command": "pytest tests/test_pricing.py::test_calculate_discount_smoke"
                    },
                    "receipt": {
                        "command": null,
                        "status": "unavailable_until_python_gap_ledger"
                    },
                    "stop_conditions": [
                        "Stop if imports, fixtures, or test setup cannot call the changed owner.",
                        "Stop if adding the test appears to require a production-code edit."
                    ],
                    "limits": ["Syntax-first Python preview evidence only."]
                }
            }]
        });
        let report = build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: ".".to_string(),
            generated_at: "test".to_string(),
            source_kind: GapDecisionLedgerSourceKind::CheckOutput,
            records_path: "target/ripr/reports/check.json".to_string(),
            records_json: Ok(payload.to_string()),
        });
        assert!(
            report.warnings.is_empty(),
            "unexpected warnings: {:?}",
            report.warnings
        );
        let records = &report.records;

        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.language, "python");
        assert_eq!(record.language_status, "preview");
        assert_eq!(record.kind, "MissingBoundaryAssertion");
        assert_eq!(record.scope, "pr_local");
        assert_eq!(record.repairability, "repairable");
        assert!(projection_eligible(record, "agent_packet"));
        assert!(!projection_eligible(record, "gate_candidate"));
        assert_eq!(
            record.verification_commands,
            vec!["pytest tests/test_pricing.py::test_calculate_discount_smoke".to_string()]
        );
        let route = record
            .repair_route
            .as_ref()
            .ok_or("expected repair route")?;
        assert_eq!(route.route_kind, "StrengthenExistingTest");
        assert_eq!(route.target_file.as_deref(), Some("tests/test_pricing.py"));
        assert_eq!(
            route.missing_discriminator.as_deref(),
            Some("amount == threshold")
        );
        assert_eq!(
            route.related_test.as_deref(),
            Some("test_calculate_discount_smoke")
        );
        assert_eq!(route.target_line, Some(4));
        assert_eq!(
            record
                .anchor
                .as_ref()
                .and_then(|anchor| anchor.file.as_deref()),
            Some("src/pricing.py")
        );
        assert_eq!(record.authority_boundary, "preview_advisory_only");
        assert_eq!(
            record.regeneration_commands,
            vec!["ripr check --root . --json > target/ripr/reports/after-check.json".to_string()]
        );
        assert_eq!(
            record.receipt_command.as_deref(),
            Some(
                "ripr outcome --before target/ripr/reports/check.json --after target/ripr/reports/after-check.json --format json --out target/ripr/receipts/gap-python-src-pricing.py-calculate_discount-predicate_boundary-predicate-amount-threshold.json"
            )
        );
        let packet = crate::output::agent_seam_packets::render_agent_gap_record_packet_json(
            "target/ripr/reports/gap-decision-ledger.json",
            record,
        )?;
        assert!(
            packet.contains("\"receipt_status\": \"available\""),
            "expected packet receipt availability in {packet}"
        );
        Ok(())
    }

    #[test]
    fn check_output_non_actionable_item_produces_record_without_repair_route() -> Result<(), String>
    {
        let payload = serde_json::json!({
            "finding_alignment": {
                "items": [
                    {
                        "evidence_class": "presentation_text",
                        "gap_state": "already_observed",
                        "actionability": "none",
                        "canonical_gap_id": "pt::OBSERVED"
                    }
                ]
            }
        });
        let records = gap_records_from_check_output_json(&payload.to_string())
            .map_err(|e| format!("parse failed: {e}"))?;
        let r = records.first().ok_or("expected record")?;
        assert_eq!(r.repairability, "no_action");
        assert!(r.repair_route.is_none());
        assert!(r.verification_commands.is_empty());
        Ok(())
    }

    #[test]
    fn check_output_item_with_static_limitations() -> Result<(), String> {
        let payload = serde_json::json!({
            "finding_alignment": {
                "items": [
                    {
                        "evidence_class": "presentation_text",
                        "gap_state": "static_limitation",
                        "actionability": "none",
                        "canonical_gap_id": "pt::STATIC",
                        "static_limitations": [
                            {"category": "dynamic_text", "repair_route": "needs_runtime"}
                        ]
                    }
                ]
            }
        });
        let records = gap_records_from_check_output_json(&payload.to_string())
            .map_err(|e| format!("parse failed: {e}"))?;
        let r = records.first().ok_or("expected record")?;
        assert_eq!(r.repairability, "analyzer_limitation");
        assert_eq!(r.static_limit_kind.as_deref(), Some("dynamic_text"));
        assert_eq!(r.static_limit_detail.as_deref(), Some("needs_runtime"));
        Ok(())
    }

    #[test]
    fn check_output_python_no_action_findings_become_report_only_records() -> Result<(), String> {
        let payload = serde_json::json!({
            "findings": [
                {
                    "id": "probe:src_discount.py:2:python_preview",
                    "canonical_gap_id": "gap:python:src/discount.py:apply_discount:predicate_boundary:predicate:amount>=threshold",
                    "canonical_gap": {
                        "language": "python",
                        "file": "src/discount.py",
                        "owner": "apply_discount",
                        "behavior_kind": "predicate_boundary"
                    },
                    "classification": "exposed",
                    "probe": {
                        "family": "predicate_boundary",
                        "file": "src/discount.py",
                        "line": 2,
                        "owner": "python:src/discount.py::apply_discount"
                    },
                    "language": "python",
                    "language_status": "preview"
                },
                {
                    "id": "probe:src_pricing.py:2:python_preview",
                    "canonical_gap_id": "gap:python:src/pricing.py:apply_discount:return_value:return_value:amount-10",
                    "canonical_gap": {
                        "language": "python",
                        "file": "src/pricing.py",
                        "owner": "apply_discount",
                        "behavior_kind": "return_value"
                    },
                    "classification": "no_static_path",
                    "probe": {
                        "family": "return_value",
                        "file": "src/pricing.py",
                        "line": 2,
                        "owner": "python:src/pricing.py::apply_discount"
                    },
                    "language": "python",
                    "language_status": "preview"
                },
                {
                    "id": "probe:src_pricing.py:3:python_preview",
                    "canonical_gap_id": "gap:python:src/pricing.py:shipping_fee:return_value:return_value:amount+2",
                    "canonical_gap": {
                        "language": "python",
                        "file": "src/pricing.py",
                        "owner": "shipping_fee",
                        "behavior_kind": "return_value"
                    },
                    "classification": "weakly_exposed",
                    "probe": {
                        "family": "return_value",
                        "file": "src/pricing.py",
                        "line": 3,
                        "owner": "python:src/pricing.py::shipping_fee"
                    },
                    "evidence": [
                        "owner: shipping_fee",
                        "related_test_uncertain: test_name_similarity (test_shipping_fee)"
                    ],
                    "language": "python",
                    "language_status": "preview"
                }
            ]
        });
        let records = gap_records_from_check_output_json(&payload.to_string())
            .map_err(|e| format!("parse failed: {e}"))?;
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].gap_state, "already_observed");
        assert_eq!(records[0].kind, "NoActionAlreadyObserved");
        assert_eq!(records[1].gap_state, "no_related_test");
        assert_eq!(records[1].kind, "NoActionNoRelatedTest");
        assert_eq!(records[2].gap_state, "heuristic_only");
        assert_eq!(records[2].kind, "NoActionHeuristicOnly");
        for record in records {
            assert_eq!(record.language, "python");
            assert_eq!(record.language_status, "preview");
            assert_eq!(record.repairability, "no_action");
            assert!(record.repair_route.is_none());
            assert!(record.verification_commands.is_empty());
            assert!(!projection_eligible(&record, "agent_packet"));
            assert!(!projection_eligible(&record, "pr_comment"));
        }
        Ok(())
    }

    #[test]
    fn check_output_item_static_limit_kind_fallback_to_kind_field() -> Result<(), String> {
        let payload = serde_json::json!({
            "finding_alignment": {
                "items": [
                    {
                        "evidence_class": "presentation_text",
                        "gap_state": "static_limitation",
                        "actionability": "none",
                        "canonical_gap_id": "pt::STATIC2",
                        "static_limitations": [
                            {"kind": "opaque_body", "reason": "body not visible"}
                        ]
                    }
                ]
            }
        });
        let records = gap_records_from_check_output_json(&payload.to_string())
            .map_err(|e| format!("parse failed: {e}"))?;
        let r = records.first().ok_or("expected record")?;
        assert_eq!(r.static_limit_kind.as_deref(), Some("opaque_body"));
        assert_eq!(r.static_limit_detail.as_deref(), Some("body not visible"));
        Ok(())
    }

    // ── evidence_ids_from_alignment_item ────────────────────────────────────

    #[test]
    fn evidence_ids_from_alignment_item_deduplicates_and_falls_back() {
        // With evidence_record_ref in multiple raw_findings, only unique ones kept
        let item = serde_json::json!({
            "raw_findings": [
                {"evidence_record_ref": "id-1"},
                {"evidence_record_ref": "id-1"},
                {"source_id": "id-2", "evidence_record_ref": "id-1"},
                {"source_id": "id-3"}
            ]
        });
        let ids = evidence_ids_from_alignment_item(&item, "fallback-id");
        assert!(ids.contains(&"id-1".to_string()));
        assert!(ids.contains(&"id-2".to_string()) || ids.contains(&"id-3".to_string()));
        // "id-1" should not appear twice
        assert_eq!(ids.iter().filter(|id| *id == "id-1").count(), 1);

        // No raw_findings → fallback to canonical_gap_id
        let item_no_findings = serde_json::json!({});
        let ids = evidence_ids_from_alignment_item(&item_no_findings, "gap:fallback");
        assert_eq!(ids, vec!["gap:fallback".to_string()]);

        // Raw_findings but none with evidence_record_ref or source_id → fallback
        let item_empty_findings = serde_json::json!({"raw_findings": [{"kind": "exposed"}]});
        let ids = evidence_ids_from_alignment_item(&item_empty_findings, "gap:fallback2");
        assert_eq!(ids, vec!["gap:fallback2".to_string()]);
    }

    // ── evidence_ids_from_repo_evidence ─────────────────────────────────────

    #[test]
    fn evidence_ids_from_repo_evidence_deduplicates_source_ids() {
        let evidence = serde_json::json!({
            "raw_findings": [
                {"source_id": "s1"},
                {"source_id": "s2"},
                {"source_id": "s1"}
            ]
        });
        let ids = evidence_ids_from_repo_evidence(&evidence, "seam-id");
        assert_eq!(ids[0], "seam-id");
        assert!(ids.contains(&"s1".to_string()));
        assert!(ids.contains(&"s2".to_string()));
        assert_eq!(ids.iter().filter(|id| *id == "s1").count(), 1);

        // No raw_findings
        let evidence_empty = serde_json::json!({});
        let ids = evidence_ids_from_repo_evidence(&evidence_empty, "seam-only");
        assert_eq!(ids, vec!["seam-only".to_string()]);
    }

    // ── first_raw_finding_expression ────────────────────────────────────────

    #[test]
    fn first_raw_finding_expression_returns_first_expression() {
        let evidence = serde_json::json!({
            "raw_findings": [
                {"expression": "fn foo()"},
                {"expression": "fn bar()"}
            ]
        });
        assert_eq!(first_raw_finding_expression(&evidence), Some("fn foo()"));

        // No raw_findings
        let no_findings = serde_json::json!({});
        assert_eq!(first_raw_finding_expression(&no_findings), None);

        // Empty array
        let empty = serde_json::json!({"raw_findings": []});
        assert_eq!(first_raw_finding_expression(&empty), None);

        // Finding without expression
        let no_expr = serde_json::json!({"raw_findings": [{"kind": "exposed"}]});
        assert_eq!(first_raw_finding_expression(&no_expr), None);
    }

    // ── projection_eligibility_from_repo_evidence ───────────────────────────

    #[test]
    fn repo_projection_eligibility_eligible_when_fully_repairable() {
        let projections = projection_eligibility_from_repo_evidence(
            "repairable",
            true, // has_repair_route
            true, // has_verify_command
            true, // has_local_anchor
            "actionable",
        );
        assert!(projections["agent_packet"].eligible);
        assert!(projections["lsp_diagnostic"].eligible);
        assert!(projections["ripr_zero_count"].eligible);
        assert!(projections["ripr_plus_count"].eligible);
        assert!(projections["ci_summary"].eligible);
        assert!(projections["report_packet"].eligible);
        // repo-scoped never gets pr_comment or gate_candidate
        assert!(!projections["pr_comment"].eligible);
        assert!(!projections["gate_candidate"].eligible);
    }

    #[test]
    fn repo_projection_eligibility_ineligible_without_repair_route() {
        let projections = projection_eligibility_from_repo_evidence(
            "repairable",
            false, // no repair route
            true,
            true,
            "actionable",
        );
        assert!(!projections["agent_packet"].eligible);
        assert!(!projections["lsp_diagnostic"].eligible);
        assert!(!projections["ripr_zero_count"].eligible);
        assert!(!projections["ripr_plus_count"].eligible);
    }

    #[test]
    fn repo_projection_eligibility_lsp_ineligible_without_anchor() {
        let projections = projection_eligibility_from_repo_evidence(
            "repairable",
            true,
            true,
            false, // no local anchor
            "actionable",
        );
        assert!(projections["agent_packet"].eligible);
        assert!(!projections["lsp_diagnostic"].eligible);
    }

    #[test]
    fn repo_projection_eligibility_ripr_counts_ineligible_when_not_actionable() {
        let projections = projection_eligibility_from_repo_evidence(
            "repairable",
            true,
            true,
            true,
            "already_observed",
        );
        assert!(projections["agent_packet"].eligible);
        assert!(!projections["ripr_zero_count"].eligible);
        assert!(!projections["ripr_plus_count"].eligible);
    }

    // ── projection_eligibility_from_pr_evidence ──────────────────────────────

    #[test]
    fn pr_projection_eligibility_eligible_when_fully_repairable() {
        let projections =
            projection_eligibility_from_pr_evidence("repairable", true, true, true, "actionable");
        assert!(projections["pr_comment"].eligible);
        assert!(projections["agent_packet"].eligible);
        assert!(projections["lsp_diagnostic"].eligible);
        assert!(projections["ci_summary"].eligible);
        assert!(projections["report_packet"].eligible);
        // pr-local never gets gate_candidate or ripr counts
        assert!(!projections["gate_candidate"].eligible);
        assert!(!projections["ripr_zero_count"].eligible);
        assert!(!projections["ripr_plus_count"].eligible);
    }

    #[test]
    fn pr_projection_eligibility_pr_comment_ineligible_when_not_actionable() {
        let projections = projection_eligibility_from_pr_evidence(
            "repairable",
            true,
            true,
            true,
            "already_observed",
        );
        // non-actionable gap_state overrides pr_comment to false
        assert!(!projections["pr_comment"].eligible);
    }

    #[test]
    fn pr_projection_eligibility_ineligible_without_anchor() {
        let projections = projection_eligibility_from_pr_evidence(
            "repairable",
            true,
            true,
            false, // no anchor
            "actionable",
        );
        assert!(!projections["pr_comment"].eligible);
        assert!(!projections["lsp_diagnostic"].eligible);
        assert!(projections["agent_packet"].eligible);
    }

    #[test]
    fn pr_projection_eligibility_not_repairable_disables_all_action_projections() {
        let projections = projection_eligibility_from_pr_evidence(
            "analyzer_limitation",
            false,
            false,
            false,
            "static_limitation",
        );
        assert!(!projections["pr_comment"].eligible);
        assert!(!projections["agent_packet"].eligible);
        assert!(!projections["lsp_diagnostic"].eligible);
    }

    // ── safe_gate_predicate_satisfied ───────────────────────────────────────

    #[test]
    fn safe_gate_predicate_satisfied_requires_all_conditions() {
        let good_predicate = SafeGatePredicate {
            policy_target_enabled: true,
            suppressed: false,
            waived: false,
            acknowledged_only: false,
            baseline_known: false,
            preview_language: false,
            static_unknown_only: false,
        };
        let good_record = GapRecord {
            gap_id: "gap:test".to_string(),
            language: "rust".to_string(),
            language_status: "stable".to_string(),
            scope: "pr_local".to_string(),
            policy_state: "new".to_string(),
            repairability: "repairable".to_string(),
            repair_route: Some(GapRepairRoute {
                route_kind: "AddBoundaryAssertion".to_string(),
                ..GapRepairRoute::default()
            }),
            verification_commands: vec!["cargo test".to_string()],
            safe_gate_predicate: Some(good_predicate.clone()),
            ..GapRecord::default()
        };
        assert!(safe_gate_predicate_satisfied(&good_record));

        // Missing predicate → false
        let no_predicate = GapRecord {
            safe_gate_predicate: None,
            ..good_record.clone()
        };
        assert!(!safe_gate_predicate_satisfied(&no_predicate));

        // Wrong language
        let wrong_lang = GapRecord {
            language: "typescript".to_string(),
            safe_gate_predicate: Some(good_predicate.clone()),
            ..good_record.clone()
        };
        assert!(!safe_gate_predicate_satisfied(&wrong_lang));

        // Preview language_status
        let preview = GapRecord {
            language_status: "preview".to_string(),
            safe_gate_predicate: Some(good_predicate.clone()),
            ..good_record.clone()
        };
        assert!(!safe_gate_predicate_satisfied(&preview));

        // Suppressed
        let suppressed = GapRecord {
            safe_gate_predicate: Some(SafeGatePredicate {
                suppressed: true,
                ..good_predicate.clone()
            }),
            ..good_record.clone()
        };
        assert!(!safe_gate_predicate_satisfied(&suppressed));

        // policy_target_enabled = false
        let not_enabled = GapRecord {
            safe_gate_predicate: Some(SafeGatePredicate {
                policy_target_enabled: false,
                ..good_predicate.clone()
            }),
            ..good_record.clone()
        };
        assert!(!safe_gate_predicate_satisfied(&not_enabled));

        // waived
        let waived = GapRecord {
            safe_gate_predicate: Some(SafeGatePredicate {
                waived: true,
                ..good_predicate.clone()
            }),
            ..good_record.clone()
        };
        assert!(!safe_gate_predicate_satisfied(&waived));

        // acknowledged_only
        let ack_only = GapRecord {
            safe_gate_predicate: Some(SafeGatePredicate {
                acknowledged_only: true,
                ..good_predicate.clone()
            }),
            ..good_record.clone()
        };
        assert!(!safe_gate_predicate_satisfied(&ack_only));

        // baseline_known
        let baseline = GapRecord {
            safe_gate_predicate: Some(SafeGatePredicate {
                baseline_known: true,
                ..good_predicate.clone()
            }),
            ..good_record.clone()
        };
        assert!(!safe_gate_predicate_satisfied(&baseline));

        // static_unknown_only
        let static_only = GapRecord {
            safe_gate_predicate: Some(SafeGatePredicate {
                static_unknown_only: true,
                ..good_predicate
            }),
            ..good_record.clone()
        };
        assert!(!safe_gate_predicate_satisfied(&static_only));

        // Wrong scope
        let wrong_scope = GapRecord {
            scope: "repo_scoped".to_string(),
            ..good_record.clone()
        };
        assert!(!safe_gate_predicate_satisfied(&wrong_scope));

        // blocked policy_state is accepted
        let blocked_policy = GapRecord {
            policy_state: "blocked".to_string(),
            ..good_record.clone()
        };
        assert!(safe_gate_predicate_satisfied(&blocked_policy));

        // Wrong policy_state
        let wrong_policy = GapRecord {
            policy_state: "not_policy_targeted".to_string(),
            ..good_record.clone()
        };
        assert!(!safe_gate_predicate_satisfied(&wrong_policy));

        // No repair_route
        let no_route = GapRecord {
            repair_route: None,
            ..good_record.clone()
        };
        assert!(!safe_gate_predicate_satisfied(&no_route));

        // No verification_commands
        let no_cmds = GapRecord {
            verification_commands: vec![],
            ..good_record
        };
        assert!(!safe_gate_predicate_satisfied(&no_cmds));
    }

    // ── validate_record warning paths ───────────────────────────────────────

    #[test]
    fn validate_record_warns_on_blank_gap_id() {
        let record = GapRecord {
            gap_id: "".to_string(),
            ..GapRecord::default()
        };
        let mut warnings = Vec::new();
        validate_record(&record, &mut warnings);
        assert!(warnings.iter().any(|w| w.contains("missing gap_id")));
    }

    #[test]
    fn validate_record_warns_on_blank_kind() {
        let record = GapRecord {
            gap_id: "gap:test".to_string(),
            kind: "".to_string(),
            ..GapRecord::default()
        };
        let mut warnings = Vec::new();
        validate_record(&record, &mut warnings);
        assert!(warnings.iter().any(|w| w.contains("missing kind")));
    }

    #[test]
    fn validate_record_warns_pr_comment_eligible_without_dedupe_fingerprint() {
        let mut projections = BTreeMap::new();
        insert_projection(&mut projections, "pr_comment", true, "test");
        let record = GapRecord {
            gap_id: "gap:test".to_string(),
            kind: "SomeKind".to_string(),
            projection_eligibility: projections,
            anchor: Some(GapAnchor {
                dedupe_fingerprint: None,
                ..GapAnchor::default()
            }),
            ..GapRecord::default()
        };
        let mut warnings = Vec::new();
        validate_record(&record, &mut warnings);
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("missing anchor.dedupe_fingerprint"))
        );
    }

    #[test]
    fn validate_record_warns_artifact_missing_without_regeneration_commands() {
        let record = GapRecord {
            gap_id: "gap:test".to_string(),
            kind: "MissingArtifact".to_string(),
            scope: "artifact_missing".to_string(),
            regeneration_commands: vec![],
            ..GapRecord::default()
        };
        let mut warnings = Vec::new();
        validate_record(&record, &mut warnings);
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("no regeneration_commands"))
        );
    }

    #[test]
    fn validate_record_no_warnings_for_clean_record() {
        let record = GapRecord {
            gap_id: "gap:clean".to_string(),
            kind: "NoActionAlreadyObserved".to_string(),
            repairability: "no_action".to_string(),
            ..GapRecord::default()
        };
        let mut warnings = Vec::new();
        validate_record(&record, &mut warnings);
        assert!(warnings.is_empty());
    }

    // ── summarize_records counters ───────────────────────────────────────────

    #[test]
    fn summarize_records_counts_all_categories() {
        let mut projections_gate = BTreeMap::new();
        insert_projection(&mut projections_gate, "gate_candidate", true, "test");
        insert_projection(&mut projections_gate, "ripr_zero_count", true, "test");
        insert_projection(&mut projections_gate, "ripr_plus_count", true, "test");
        insert_projection(&mut projections_gate, "pr_comment", true, "test");
        insert_projection(&mut projections_gate, "agent_packet", true, "test");

        let records = vec![
            GapRecord {
                gap_id: "gap:r1".to_string(),
                kind: "MissingValueAssertion".to_string(),
                repairability: "repairable".to_string(),
                ..GapRecord::default()
            },
            GapRecord {
                gap_id: "gap:r2".to_string(),
                kind: "StaticLimitation".to_string(),
                repairability: "analyzer_limitation".to_string(),
                ..GapRecord::default()
            },
            GapRecord {
                gap_id: "gap:r3".to_string(),
                kind: "NoActionAlreadyObserved".to_string(),
                repairability: "no_action".to_string(),
                ..GapRecord::default()
            },
            GapRecord {
                gap_id: "gap:r4".to_string(),
                kind: "NoActionInternal".to_string(),
                repairability: "no_action".to_string(),
                ..GapRecord::default()
            },
            GapRecord {
                gap_id: "gap:r5".to_string(),
                kind: "MissingArtifact".to_string(),
                scope: "artifact_missing".to_string(),
                repairability: "unknown".to_string(),
                ..GapRecord::default()
            },
            GapRecord {
                gap_id: "gap:r6".to_string(),
                kind: "MissingOutputContract".to_string(),
                projection_eligibility: projections_gate.clone(),
                repairability: "no_action".to_string(),
                ..GapRecord::default()
            },
            GapRecord {
                gap_id: "gap:r7".to_string(),
                kind: "MissingValueAssertion".to_string(),
                language_status: "preview".to_string(),
                repairability: "no_action".to_string(),
                ..GapRecord::default()
            },
            GapRecord {
                gap_id: "gap:r8".to_string(),
                kind: "MissingValueAssertion".to_string(),
                repairability: "no_action".to_string(),
                receipt: Some(GapReceipt {
                    movement: Some("improved".to_string()),
                    ..GapReceipt::default()
                }),
                ..GapRecord::default()
            },
            GapRecord {
                gap_id: "gap:r9".to_string(),
                kind: "MissingValueAssertion".to_string(),
                repairability: "no_action".to_string(),
                receipt: Some(GapReceipt {
                    movement: Some("unchanged_after_attempt".to_string()),
                    ..GapReceipt::default()
                }),
                ..GapRecord::default()
            },
        ];
        let summary = summarize_records(&records);
        assert_eq!(summary.records_total, 9);
        assert_eq!(summary.repairable_total, 1);
        assert_eq!(summary.static_limitation_total, 1);
        assert_eq!(summary.no_action_total, 6); // r3, r4, r6, r7, r8, r9 (repairability=no_action or kind NoAction*)
        assert_eq!(summary.missing_artifact_total, 1);
        assert_eq!(summary.projection_gate_candidate, 1);
        assert_eq!(summary.ripr_zero_target_count, 1);
        assert_eq!(summary.ripr_plus_target_count, 1);
        assert_eq!(summary.projection_pr_comment_eligible, 1);
        assert_eq!(summary.projection_agent_packet_eligible, 1);
        assert_eq!(summary.missing_output_contract_total, 1);
        assert_eq!(summary.receipt_improved_total, 1);
        assert_eq!(summary.receipt_unchanged_after_attempt_total, 1);
        // r7: preview language_status, not eligible for gate/ripr counts → preview_ineligible
        assert_eq!(summary.preview_ineligible_total, 1);
    }

    // ── render_record_markdown covers no-anchor, no-repair, no-projections ──

    #[test]
    fn render_record_markdown_without_optional_fields() {
        let record = GapRecord {
            gap_id: "gap:bare".to_string(),
            kind: "NoActionAlreadyObserved".to_string(),
            scope: "repo_scoped".to_string(),
            policy_state: "baseline".to_string(),
            repairability: "no_action".to_string(),
            evidence_class: "already_observed".to_string(),
            gap_state: "already_observed".to_string(),
            language: "rust".to_string(),
            language_status: "stable".to_string(),
            anchor: None,
            repair_route: None,
            projection_eligibility: BTreeMap::new(),
            verification_commands: vec![],
            regeneration_commands: vec![],
            receipt: None,
            ..GapRecord::default()
        };
        let mut out = String::new();
        render_record_markdown(&record, &mut out);
        assert!(out.contains("### `gap:bare`"));
        assert!(!out.contains("Anchor:"));
        assert!(!out.contains("Repair:"));
        assert!(!out.contains("Eligible projections:"));
        assert!(!out.contains("Verify:"));
        assert!(!out.contains("Regenerate:"));
        assert!(!out.contains("Receipt movement:"));
    }

    #[test]
    fn render_record_markdown_with_anchor_no_line_no_owner() {
        let record = GapRecord {
            gap_id: "gap:anchor".to_string(),
            kind: "MissingBoundaryAssertion".to_string(),
            anchor: Some(GapAnchor {
                file: Some("src/lib.rs".to_string()),
                line: None,
                owner: None,
                dedupe_fingerprint: None,
            }),
            ..GapRecord::default()
        };
        let mut out = String::new();
        render_record_markdown(&record, &mut out);
        assert!(out.contains("Anchor: `src/lib.rs`"));
        assert!(!out.contains("owner"));
    }

    #[test]
    fn render_record_markdown_repair_route_with_no_target_file_no_assertion() {
        let record = GapRecord {
            gap_id: "gap:nofile".to_string(),
            kind: "MissingValueAssertion".to_string(),
            repair_route: Some(GapRepairRoute {
                route_kind: "AddValueAssertion".to_string(),
                target_file: None,
                assertion_shape: None,
                ..GapRepairRoute::default()
            }),
            ..GapRecord::default()
        };
        let mut out = String::new();
        render_record_markdown(&record, &mut out);
        assert!(out.contains("Repair: `AddValueAssertion`"));
        assert!(!out.contains("in `"));
        assert!(!out.contains("Assertion or observer:"));
    }

    // ── eligible_projection_names ───────────────────────────────────────────

    #[test]
    fn eligible_projection_names_returns_only_eligible() {
        let mut projections = BTreeMap::new();
        insert_projection(&mut projections, "ci_summary", true, "test");
        insert_projection(&mut projections, "agent_packet", false, "not_repairable");
        insert_projection(&mut projections, "report_packet", true, "test");
        let record = GapRecord {
            projection_eligibility: projections,
            ..GapRecord::default()
        };
        let names = eligible_projection_names(&record);
        assert!(names.contains(&"ci_summary".to_string()));
        assert!(names.contains(&"report_packet".to_string()));
        assert!(!names.contains(&"agent_packet".to_string()));
        assert_eq!(names.len(), 2);
    }

    // ── parse_gap_records_json / gap_records_from_value ─────────────────────

    #[test]
    fn parse_gap_records_json_invalid_json_returns_error() -> Result<(), String> {
        let err_msg = parse_gap_records_json("{invalid")
            .err()
            .ok_or("expected error for invalid JSON")?;
        assert!(
            err_msg.contains("invalid JSON"),
            "unexpected error: {err_msg}"
        );
        Ok(())
    }

    #[test]
    fn parse_gap_record_source_json_preserves_report_metadata() -> Result<(), String> {
        let value = serde_json::json!({
            "root": "fixtures/python/basic",
            "generated_at": "unix_ms:1778240100000",
            "records": [
                {
                    "gap_id": "gap:python:pricing-boundary",
                    "kind": "MissingBoundaryAssertion",
                    "language": "python",
                    "language_status": "preview",
                    "scope": "pr_local",
                    "repairability": "repairable",
                    "authority_boundary": "preview_static_advisory"
                }
            ]
        });
        let source = parse_gap_record_source_json(&value.to_string())
            .map_err(|err| format!("parse failed: {err}"))?;
        assert_eq!(source.root.as_deref(), Some("fixtures/python/basic"));
        assert_eq!(
            source.generated_at.as_deref(),
            Some("unix_ms:1778240100000")
        );
        assert_eq!(source.records.len(), 1);
        assert_eq!(source.records[0].gap_id, "gap:python:pricing-boundary");
        Ok(())
    }

    #[test]
    fn gap_records_from_value_cases_key_parses_expected_gap_record() -> Result<(), String> {
        let value = serde_json::json!({
            "cases": [
                {
                    "id": "case1",
                    "expected_gap_record": {
                        "gap_id": "gap:case1",
                        "kind": "MissingBoundaryAssertion",
                        "language": "rust",
                        "language_status": "stable",
                        "scope": "pr_local",
                        "repairability": "repairable",
                        "authority_boundary": "gate_decision_artifact_only"
                    }
                }
            ]
        });
        let records =
            parse_gap_records_json(&value.to_string()).map_err(|e| format!("failed: {e}"))?;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].gap_id, "gap:case1");
        Ok(())
    }

    #[test]
    fn gap_records_from_value_cases_key_missing_expected_gap_record_returns_error()
    -> Result<(), String> {
        let value = serde_json::json!({
            "cases": [{"id": "case-bad"}]
        });
        let err_msg = parse_gap_records_json(&value.to_string())
            .err()
            .ok_or("expected error for missing expected_gap_record")?;
        assert!(
            err_msg.contains("missing expected_gap_record"),
            "unexpected error: {err_msg}"
        );
        Ok(())
    }

    #[test]
    fn gap_records_from_value_no_known_key_returns_error() -> Result<(), String> {
        let value = serde_json::json!({"unknown_key": []});
        let err_msg = parse_gap_records_json(&value.to_string())
            .err()
            .ok_or("expected error for unknown key")?;
        assert!(
            err_msg.contains("expected records"),
            "unexpected error: {err_msg}"
        );
        Ok(())
    }

    // ── repo_exposure source kind via build_gap_decision_ledger_report ───────

    #[test]
    fn build_report_with_repo_exposure_source_kind() -> Result<(), String> {
        let payload = serde_json::json!({
            "seams": [
                make_repo_exposure_seam("actionable", "add_focused_test", "error_variant")
            ]
        });
        let report = build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: ".".to_string(),
            generated_at: "test".to_string(),
            source_kind: GapDecisionLedgerSourceKind::RepoExposure,
            records_path: "exposure.json".to_string(),
            records_json: Ok(payload.to_string()),
        });
        assert_eq!(report.summary.records_total, 1);
        assert_eq!(report.records[0].kind, "MissingErrorDiscriminator");
        Ok(())
    }

    #[test]
    fn build_report_with_repo_exposure_parse_error_is_blocked() {
        let report = build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: ".".to_string(),
            generated_at: "test".to_string(),
            source_kind: GapDecisionLedgerSourceKind::RepoExposure,
            records_path: "exposure.json".to_string(),
            records_json: Ok("not-json".to_string()),
        });
        assert_eq!(report.status, "blocked");
        assert!(report.warnings[0].contains("parse exposure.json failed"));
    }

    #[test]
    fn build_report_with_check_output_parse_error_is_blocked() {
        let report = build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: ".".to_string(),
            generated_at: "test".to_string(),
            source_kind: GapDecisionLedgerSourceKind::CheckOutput,
            records_path: "check.json".to_string(),
            records_json: Ok("not-json".to_string()),
        });
        assert_eq!(report.status, "blocked");
        assert!(report.warnings[0].contains("parse check.json failed"));
    }

    // ── presentation_text_repair_route uses related_test fields ─────────────

    #[test]
    fn check_output_repair_route_uses_related_test_and_suggested_assertion() -> Result<(), String> {
        let payload = serde_json::json!({
            "finding_alignment": {
                "items": [{
                    "evidence_class": "presentation_text",
                    "gap_state": "actionable",
                    "actionability": "add_focused_test",
                    "canonical_gap_id": "pt::LABEL",
                    "related_test": {
                        "name": "test_label_output",
                        "file": "tests/output_test.rs",
                        "line": 42
                    },
                    "presentation_text": {
                        "constant_name": "LABEL",
                        "text_literal": "some label",
                        "suggested_assertion": "assert_eq!(output, \"some label\")"
                    },
                    "raw_findings": []
                }]
            }
        });
        let records = gap_records_from_check_output_json(&payload.to_string())
            .map_err(|e| format!("parse failed: {e}"))?;
        let route = records[0]
            .repair_route
            .as_ref()
            .ok_or("expected repair_route")?;
        assert_eq!(route.related_test.as_deref(), Some("test_label_output"));
        assert_eq!(route.target_file.as_deref(), Some("tests/output_test.rs"));
        assert_eq!(route.target_line, Some(42));
        assert_eq!(
            route.assertion_shape.as_deref(),
            Some("assert_eq!(output, \"some label\")")
        );
        assert_eq!(route.changed_behavior.as_deref(), Some("some label"));
        assert_eq!(route.stop_conditions.len(), 2);
        Ok(())
    }

    #[test]
    fn check_output_repair_route_changed_behavior_falls_back_to_constant_name() -> Result<(), String>
    {
        let payload = serde_json::json!({
            "finding_alignment": {
                "items": [{
                    "evidence_class": "presentation_text",
                    "gap_state": "actionable",
                    "actionability": "add_focused_test",
                    "canonical_gap_id": "pt::LABEL2",
                    "presentation_text": {
                        "constant_name": "MY_CONST"
                    },
                    "raw_findings": []
                }]
            }
        });
        let records = gap_records_from_check_output_json(&payload.to_string())
            .map_err(|e| format!("parse failed: {e}"))?;
        let route = records[0]
            .repair_route
            .as_ref()
            .ok_or("expected repair_route")?;
        // text_literal absent → falls back to constant_name
        assert_eq!(route.changed_behavior.as_deref(), Some("MY_CONST"));
        Ok(())
    }

    #[test]
    fn check_output_verify_command_fallback_when_not_presentation_text_repairable()
    -> Result<(), String> {
        let payload = serde_json::json!({
            "finding_alignment": {
                "items": [{
                    "evidence_class": "presentation_text",
                    "gap_state": "static_limitation",
                    "actionability": "none",
                    "canonical_gap_id": "pt::SL",
                    "verify_command": "cargo xtask check-output-contracts"
                }]
            }
        });
        let records = gap_records_from_check_output_json(&payload.to_string())
            .map_err(|e| format!("parse failed: {e}"))?;
        // not repairable, so falls through to verify_command from item
        assert_eq!(
            records[0].verification_commands,
            vec!["cargo xtask check-output-contracts".to_string()]
        );
        Ok(())
    }

    // ── repair_route_from_evidence uses assertion_shape example fallback ─────

    #[test]
    fn check_output_python_static_limit_becomes_report_only_gap_record() -> Result<(), String> {
        let payload = python_static_limit_check_output();
        let records = gap_records_from_check_output_json(&payload)
            .map_err(|e| format!("parse failed: {e}"))?;
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.kind, "StaticLimitation");
        assert_eq!(record.language, "python");
        assert_eq!(record.language_status, "preview");
        assert_eq!(record.gap_state, "static_limitation");
        assert_eq!(record.policy_state, "not_policy_targeted");
        assert_eq!(record.repairability, "analyzer_limitation");
        assert!(record.repair_route.is_none());
        assert!(record.verification_commands.is_empty());
        assert_eq!(
            record.static_limit_kind.as_deref(),
            Some("dynamic_dispatch")
        );
        assert!(
            record
                .static_limit_detail
                .as_deref()
                .is_some_and(|detail| detail.contains("syntax alone cannot resolve"))
        );
        assert_eq!(
            record.canonical_gap_id,
            "gap:python:src/dispatch.py:call_named:static_limit:return_value:dynamic_dispatch"
        );
        assert_eq!(
            record
                .anchor
                .as_ref()
                .and_then(|anchor| anchor.file.as_deref()),
            Some("src/dispatch.py")
        );
        assert_eq!(
            record.anchor.as_ref().and_then(|anchor| anchor.line),
            Some(2)
        );
        assert!(!projection_eligible(record, "agent_packet"));
        assert!(!projection_eligible(record, "pr_comment"));
        assert!(!projection_eligible(record, "gate_candidate"));
        assert!(!projection_eligible(record, "ripr_zero_count"));
        assert!(!projection_eligible(record, "ripr_plus_count"));

        let report = build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: "fixtures/python_dynamic_dispatch_limit/input".to_string(),
            generated_at: "test".to_string(),
            source_kind: GapDecisionLedgerSourceKind::CheckOutput,
            records_path: "before-check.json".to_string(),
            records_json: Ok(payload),
        });
        assert_eq!(report.status, "advisory");
        assert_eq!(report.summary.records_total, 1);
        assert_eq!(report.summary.static_limitation_total, 1);
        assert_eq!(report.summary.preview_ineligible_total, 1);
        assert_eq!(report.summary.projection_agent_packet_eligible, 0);
        Ok(())
    }

    #[test]
    fn check_output_python_dynamic_import_limit_stays_report_only() -> Result<(), String> {
        let payload = python_dynamic_import_static_limit_check_output();
        let records = gap_records_from_check_output_json(&payload)
            .map_err(|e| format!("parse failed: {e}"))?;
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.kind, "StaticLimitation");
        assert_eq!(record.language, "python");
        assert_eq!(record.repairability, "analyzer_limitation");
        assert!(record.repair_route.is_none());
        assert!(record.verification_commands.is_empty());
        assert_eq!(
            record.static_limit_kind.as_deref(),
            Some("missing_import_graph")
        );
        assert!(!projection_eligible(record, "agent_packet"));

        let report = build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: "fixtures/python_dynamic_import_limit/input".to_string(),
            generated_at: "test".to_string(),
            source_kind: GapDecisionLedgerSourceKind::CheckOutput,
            records_path: "before-check.json".to_string(),
            records_json: Ok(payload),
        });
        assert_eq!(report.summary.records_total, 1);
        assert_eq!(report.summary.repairable_total, 0);
        assert_eq!(report.summary.static_limitation_total, 1);
        assert_eq!(report.summary.projection_agent_packet_eligible, 0);
        Ok(())
    }

    #[test]
    fn swarm_queue_excludes_python_static_limit_gap_records() -> Result<(), String> {
        let payload = python_static_limit_check_output();
        let records = gap_records_from_check_output_json(&payload)
            .map_err(|e| format!("parse failed: {e}"))?;

        let rendered = crate::output::agent_seam_packets::render_agent_gap_record_queue_json(
            ".",
            "target/ripr/reports/gap-decision-ledger.json",
            &records,
            "python",
            10,
        )?;
        let queue: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("queue JSON should parse: {err}"))?;
        assert_eq!(queue["summary"]["language_records_total"], 1);
        assert_eq!(queue["summary"]["queue_total"], 0);
        assert_eq!(queue["summary"]["excluded_records_total"], 1);
        assert!(
            queue["exclusion_reasons"]
                .as_array()
                .is_some_and(|reasons| reasons.iter().any(|reason| {
                    reason["reason"]
                        .as_str()
                        .is_some_and(|text| text.contains("not_repairable"))
                }))
        );
        assert!(queue["packets"].as_array().is_some_and(Vec::is_empty));
        Ok(())
    }

    #[test]
    fn repo_repair_route_uses_assertion_example_and_recommended_repair_fallback()
    -> Result<(), String> {
        // With assertion_shape.example
        let seam_with_example = serde_json::json!({
            "seams": [{
                "evidence_record": {
                    "seam_id": "s-example",
                    "seam_kind": "predicate_boundary",
                    "recommendation": {
                        "assertion_shape": {"example": "assert!(result.is_ok())"},
                        "recommended_test": {"file": "tests/foo.rs"}
                    },
                    "raw_findings": [
                        {"expression": "fn example()"}
                    ],
                    "canonical_item": {
                        "gap_state": "actionable",
                        "actionability": "add_focused_test",
                        "related_test": {
                            "name": "test_example",
                            "file": "tests/foo.rs",
                            "line": 5
                        }
                    }
                }
            }]
        });
        let records = gap_records_from_repo_exposure_json(&seam_with_example.to_string())
            .map_err(|e| format!("parse failed: {e}"))?;
        let route = records[0]
            .repair_route
            .as_ref()
            .ok_or("expected repair_route")?;
        assert_eq!(
            route.assertion_shape.as_deref(),
            Some("assert!(result.is_ok())")
        );
        assert_eq!(route.changed_behavior.as_deref(), Some("fn example()"));
        assert_eq!(route.related_test.as_deref(), Some("test_example"));
        assert_eq!(route.target_line, Some(5));

        // Fallback: recommended_repair in canonical_item (no assertion_shape.example)
        let seam_recommended = serde_json::json!({
            "seams": [{
                "evidence_record": {
                    "seam_id": "s-rec",
                    "seam_kind": "match_arm",
                    "canonical_item": {
                        "gap_state": "actionable",
                        "actionability": "add_focused_test",
                        "recommended_repair": "Add a branch assertion for arm."
                    }
                }
            }]
        });
        let records = gap_records_from_repo_exposure_json(&seam_recommended.to_string())
            .map_err(|e| format!("parse failed: {e}"))?;
        let route = records[0]
            .repair_route
            .as_ref()
            .ok_or("expected repair_route for recommended")?;
        assert_eq!(
            route.assertion_shape.as_deref(),
            Some("Add a branch assertion for arm.")
        );
        Ok(())
    }

    // ── render_gap_decision_ledger_markdown with no records ──────────────────

    #[test]
    fn render_gap_decision_ledger_markdown_no_records_shows_placeholder() {
        let report = build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: ".".to_string(),
            generated_at: "test".to_string(),
            source_kind: GapDecisionLedgerSourceKind::Records,
            records_path: "empty.json".to_string(),
            records_json: Ok("[]".to_string()),
        });
        let markdown = render_gap_decision_ledger_markdown(&report);
        assert!(markdown.contains("No gap records were supplied."));
        // Blocked status produces "blocked"
        assert!(markdown.contains("Status: `blocked`"));
    }

    #[test]
    fn render_gap_decision_ledger_markdown_with_warnings() {
        let report = build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: "/repo".to_string(),
            generated_at: "test".to_string(),
            source_kind: GapDecisionLedgerSourceKind::Records,
            records_path: "bad.json".to_string(),
            records_json: Ok("not-json".to_string()),
        });
        let markdown = render_gap_decision_ledger_markdown(&report);
        assert!(markdown.contains("## Warnings"));
        assert!(markdown.contains("invalid JSON"));
    }

    // ── parse_record_array error formatting ──────────────────────────────────

    #[test]
    fn parse_record_array_reports_index_on_invalid_record() -> Result<(), String> {
        let value = serde_json::json!([
            {
                "gap_id": "gap:valid",
                "kind": "NoActionAlreadyObserved",
                "authority_boundary": "gate_decision_artifact_only"
            },
            "not-a-record"
        ]);
        let msg = parse_gap_records_json(&value.to_string())
            .err()
            .ok_or("expected error for invalid record at index 1")?;
        assert!(msg.contains("record 1"), "expected 'record 1' in: {msg}");
        assert!(
            msg.contains("invalid GapRecord"),
            "expected 'invalid GapRecord' in: {msg}"
        );
        Ok(())
    }
}
