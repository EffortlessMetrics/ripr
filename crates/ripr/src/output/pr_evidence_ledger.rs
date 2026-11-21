use serde_json::{Value, json};
use std::collections::BTreeMap;

use super::receipt_lifecycle::{
    RECEIPT_MISSING, receipt_lifecycle_state, receipt_lifecycle_state_from_movement,
    receipt_lifecycle_state_from_receipt_value,
};

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "pr_evidence_ledger";
const LIMITS_NOTE: &str = "Read-only advisory PR evidence ledger over existing static RIPR artifacts; gate-decision remains the pass/fail authority.";
pub(crate) const DEFAULT_PR_EVIDENCE_LEDGER_OUT: &str =
    "target/ripr/reports/pr-evidence-ledger.json";
pub(crate) const DEFAULT_PR_EVIDENCE_LEDGER_MD_OUT: &str =
    "target/ripr/reports/pr-evidence-ledger.md";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrEvidenceLedgerInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) pr_number: String,
    pub(crate) base: String,
    pub(crate) head: String,
    pub(crate) labels: Vec<String>,
    pub(crate) gate_path: Option<String>,
    pub(crate) baseline_delta_path: Option<String>,
    pub(crate) zero_status_path: Option<String>,
    pub(crate) pr_guidance_path: Option<String>,
    pub(crate) gap_ledger_path: Option<String>,
    pub(crate) recommendation_calibration_path: Option<String>,
    pub(crate) agent_receipt_path: Option<String>,
    pub(crate) coverage_path: Option<String>,
    pub(crate) history_path: Option<String>,
    pub(crate) gate_json: Option<Result<String, String>>,
    pub(crate) baseline_delta_json: Option<Result<String, String>>,
    pub(crate) zero_status_json: Option<Result<String, String>>,
    pub(crate) pr_guidance_json: Option<Result<String, String>>,
    pub(crate) gap_ledger_json: Option<Result<String, String>>,
    pub(crate) recommendation_calibration_json: Option<Result<String, String>>,
    pub(crate) agent_receipt_json: Option<Result<String, String>>,
    pub(crate) coverage_json: Option<Result<String, String>>,
    pub(crate) history_json: Option<Result<String, String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrEvidenceLedgerReport {
    root: String,
    generated_at: String,
    status: String,
    pr: PrIdentity,
    inputs: LedgerInputs,
    movement: Movement,
    gate: GateSummary,
    waivers: Vec<WaiverRecord>,
    suppressions: Vec<SuppressionRecord>,
    repair_receipts: Vec<RepairReceipt>,
    coverage_grip_frontier: CoverageGripFrontier,
    top_repair_route: Option<RepairRoute>,
    history: Option<HistorySummary>,
    warnings: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PrIdentity {
    number: String,
    base: String,
    head: String,
    labels: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LedgerInputs {
    gate_decision: Option<String>,
    baseline_debt_delta: Option<String>,
    ripr_zero_status: Option<String>,
    pr_guidance: Option<String>,
    gap_decision_ledger: Option<String>,
    recommendation_calibration: Option<String>,
    agent_receipt: Option<String>,
    coverage: Option<String>,
    history: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Movement {
    new_policy_eligible: usize,
    baseline_still_present: usize,
    baseline_resolved: usize,
    acknowledged: usize,
    suppressed: usize,
    blocking_candidates: usize,
    visible_unresolved: usize,
    ripr_zero_state: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GateSummary {
    mode: Option<String>,
    decision: Option<String>,
    pass_fail_authority: Option<String>,
    acknowledgement_label: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WaiverRecord {
    label: String,
    decision_id: Option<String>,
    canonical_gap_id: Option<String>,
    seam_id: Option<String>,
    age_prs: usize,
    age_days: usize,
    reason: String,
    still_visible: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SuppressionRecord {
    decision_id: Option<String>,
    canonical_gap_id: Option<String>,
    seam_id: Option<String>,
    source: Option<String>,
    owner: Option<String>,
    reason: Option<String>,
    still_visible: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RepairReceipt {
    source: String,
    canonical_gap_id: Option<String>,
    seam_id: Option<String>,
    receipt_state: String,
    static_movement: StaticMovement,
    focused_test: Option<String>,
    receipt: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StaticMovement {
    state: String,
    source: String,
    artifact: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoverageGripFrontier {
    status: String,
    coverage_delta_percent: Option<String>,
    ripr_visible_unresolved_delta: Option<i64>,
    interpretation: String,
    quadrants: CoverageQuadrants,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CoverageQuadrants {
    covered_with_ripr_gap: usize,
    covered_without_ripr_gap: usize,
    uncovered_with_ripr_gap: usize,
    uncovered_without_ripr_gap: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RepairRoute {
    source: String,
    gap_id: Option<String>,
    canonical_gap_id: Option<String>,
    language: Option<String>,
    language_status: Option<String>,
    seam_id: Option<String>,
    path: Option<String>,
    line: Option<u64>,
    repair_route: Option<String>,
    missing_discriminator: Option<String>,
    suggested_test: Option<String>,
    related_test: Option<String>,
    verify_command: Option<String>,
    receipt_command: Option<String>,
    receipt_state: Option<String>,
    static_limit_kind: Option<String>,
    static_limit_detail: Option<String>,
    agent_command: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HistorySummary {
    source: String,
    records: usize,
    waiver_age_max_days: usize,
    baseline_resolved_total: usize,
    new_policy_eligible_total: usize,
    trend: String,
    waiver_seen_by_seam: BTreeMap<String, usize>,
    waiver_age_days_by_seam: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ParsedSources {
    gate: Option<Value>,
    baseline_delta: Option<Value>,
    zero_status: Option<Value>,
    pr_guidance: Option<Value>,
    gap_ledger: Option<Value>,
    recommendation_calibration: Option<Value>,
    agent_receipt: Option<Value>,
    coverage: Option<Value>,
    history: Option<HistorySummary>,
    warnings: Vec<String>,
}

pub(crate) fn build_pr_evidence_ledger_report(
    input: PrEvidenceLedgerInput,
) -> PrEvidenceLedgerReport {
    let parsed = parse_sources(&input);
    let mut labels = input.labels.clone();
    labels.extend(labels_from_gate(parsed.gate.as_ref()));
    labels.sort();
    labels.dedup();

    let movement = movement_from_sources(
        parsed.baseline_delta.as_ref(),
        parsed.zero_status.as_ref(),
        parsed.gate.as_ref(),
    );
    let gate = gate_summary_from_value(parsed.gate.as_ref());
    let waivers = waiver_records_from_gate(parsed.gate.as_ref(), parsed.history.as_ref());
    let suppressions =
        suppression_records_from_sources(parsed.gate.as_ref(), parsed.baseline_delta.as_ref());
    let repair_receipts = repair_receipts_from_sources(
        input.agent_receipt_path.as_deref(),
        parsed.agent_receipt.as_ref(),
        input.recommendation_calibration_path.as_deref(),
        parsed.recommendation_calibration.as_ref(),
    );
    let coverage_grip_frontier = coverage_frontier(
        input.coverage_path.as_deref(),
        parsed.coverage.as_ref(),
        &movement,
    );
    let top_repair_route = top_repair_route(
        parsed.gap_ledger.as_ref(),
        parsed.zero_status.as_ref(),
        parsed.pr_guidance.as_ref(),
        parsed.gate.as_ref(),
        parsed.baseline_delta.as_ref(),
    );

    let evidence_count = [
        parsed.gate.as_ref(),
        parsed.baseline_delta.as_ref(),
        parsed.zero_status.as_ref(),
        parsed.pr_guidance.as_ref(),
        parsed.gap_ledger.as_ref(),
    ]
    .iter()
    .filter(|value| value.is_some())
    .count();
    let status = if input.pr_number.trim().is_empty()
        || input.base.trim().is_empty()
        || input.head.trim().is_empty()
        || evidence_count == 0
    {
        "incomplete"
    } else {
        "advisory"
    }
    .to_string();

    let mut warnings = parsed.warnings;
    if input.coverage_path.is_none() {
        warnings.push(
            "coverage input not supplied; coverage/grip frontier is not_available".to_string(),
        );
    }
    if parsed.history.is_none() {
        warnings.push(
            "history input not supplied; waiver age and long-term trends are local to this PR"
                .to_string(),
        );
    }

    PrEvidenceLedgerReport {
        root: input.root,
        generated_at: input.generated_at,
        status,
        pr: PrIdentity {
            number: input.pr_number,
            base: input.base,
            head: input.head,
            labels,
        },
        inputs: LedgerInputs {
            gate_decision: input.gate_path,
            baseline_debt_delta: input.baseline_delta_path,
            ripr_zero_status: input.zero_status_path,
            pr_guidance: input.pr_guidance_path,
            gap_decision_ledger: input.gap_ledger_path,
            recommendation_calibration: input.recommendation_calibration_path,
            agent_receipt: input.agent_receipt_path,
            coverage: input.coverage_path,
            history: input.history_path,
        },
        movement,
        gate,
        waivers,
        suppressions,
        repair_receipts,
        coverage_grip_frontier,
        top_repair_route,
        history: parsed.history,
        warnings,
    }
}

pub(crate) fn render_pr_evidence_ledger_json(
    report: &PrEvidenceLedgerReport,
) -> Result<String, String> {
    serde_json::to_string_pretty(&json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "kind": REPORT_KIND,
        "status": report.status,
        "root": report.root,
        "generated_at": report.generated_at,
        "pr": pr_json(&report.pr),
        "inputs": inputs_json(&report.inputs),
        "movement": movement_json(&report.movement),
        "gate": gate_json(&report.gate),
        "waivers": report.waivers.iter().map(waiver_json).collect::<Vec<_>>(),
        "suppressions": report.suppressions.iter().map(suppression_json).collect::<Vec<_>>(),
        "repair_receipts": report.repair_receipts.iter().map(repair_receipt_json).collect::<Vec<_>>(),
        "coverage_grip_frontier": coverage_json(&report.coverage_grip_frontier),
        "top_repair_route": report.top_repair_route.as_ref().map(repair_route_json),
        "history": report.history.as_ref().map(history_json),
        "warnings": report.warnings,
        "limits_note": LIMITS_NOTE,
    }))
    .map_err(|err| format!("failed to render PR evidence ledger JSON: {err}"))
}

pub(crate) fn render_pr_evidence_ledger_markdown(report: &PrEvidenceLedgerReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR PR Evidence Ledger\n\n");
    out.push_str(&format!("Status: {}\n", report.status));
    out.push_str(&format!(
        "Gate: {} / {}\n\n",
        report.gate.mode.as_deref().unwrap_or("not_available"),
        report.gate.decision.as_deref().unwrap_or("not_available")
    ));

    out.push_str("Start here:\n");
    if let Some(route) = report.top_repair_route.as_ref() {
        out.push_str(&format!("- Identity: {}\n", route_identity(route)));
        if let Some(gap_id) = route.gap_id.as_deref() {
            out.push_str(&format!("- Gap: {gap_id}\n"));
        }
        out.push_str(&format!("- Source: {}\n", route.source));
        out.push_str(&format!("- Location: {}\n", route_location(route)));
        out.push_str(&format!(
            "- Repair route: {}\n",
            route.repair_route.as_deref().unwrap_or("focused_test")
        ));
        if let Some(missing) = route.missing_discriminator.as_deref() {
            out.push_str(&format!("- Missing discriminator: {missing}\n"));
        }
        if let Some(test) = route.suggested_test.as_deref() {
            out.push_str(&format!("- Suggested test: {test}\n"));
        }
        if let Some(related) = route.related_test.as_deref() {
            out.push_str(&format!("- Related test: {related}\n"));
        }
        if let Some(verify) = route.verify_command.as_deref() {
            out.push_str(&format!("- Verify command: `{verify}`\n"));
        }
        if let Some(receipt) = route.receipt_command.as_deref() {
            out.push_str(&format!("- Receipt command: `{receipt}`\n"));
        }
        out.push_str(&format!(
            "- Receipt state: {}\n",
            route.receipt_state.as_deref().unwrap_or(RECEIPT_MISSING)
        ));
        if let Some(language) = route.language.as_deref() {
            let status = route.language_status.as_deref().unwrap_or("unknown");
            out.push_str(&format!("- Language: {language} ({status})\n"));
        }
        if let Some(limit) = route.static_limit_kind.as_deref() {
            out.push_str(&format!("- Static limit: {limit}\n"));
            if let Some(detail) = route.static_limit_detail.as_deref() {
                out.push_str(&format!("  - {detail}\n"));
            }
        }
        if let Some(agent) = route.agent_command.as_deref() {
            out.push_str(&format!("- Agent handoff: `{agent}`\n"));
        }
        out.push_str("- Boundary: advisory static evidence only; raw counts below are supporting evidence and gate authority remains separate.\n");
    } else {
        out.push_str("- State: no canonical repair route available\n");
        out.push_str("- Boundary: no route is not a coverage, runtime, mutation, gate, or merge-readiness claim.\n");
    }

    out.push_str("\nSupporting movement counts:\n");
    out.push_str("| Measure | Count |\n");
    out.push_str("| --- | ---: |\n");
    out.push_str(&format!(
        "| New policy-eligible gaps | {} |\n",
        report.movement.new_policy_eligible
    ));
    out.push_str(&format!(
        "| Existing baseline gaps still present | {} |\n",
        report.movement.baseline_still_present
    ));
    out.push_str(&format!(
        "| Baseline gaps resolved | {} |\n",
        report.movement.baseline_resolved
    ));
    out.push_str(&format!(
        "| Acknowledged gaps | {} |\n",
        report.movement.acknowledged
    ));
    out.push_str(&format!(
        "| Suppressed gaps | {} |\n",
        report.movement.suppressed
    ));
    out.push_str(&format!(
        "| Blocking candidates | {} |\n",
        report.movement.blocking_candidates
    ));
    out.push_str(&format!(
        "| Visible unresolved gaps | {} |\n",
        report.movement.visible_unresolved
    ));

    out.push_str("\nReceipts:\n");
    out.push_str(&format!(
        "- Gate decision: {}\n",
        report
            .inputs
            .gate_decision
            .as_deref()
            .unwrap_or("not supplied")
    ));
    out.push_str(&format!(
        "- Baseline debt delta: {}\n",
        report
            .inputs
            .baseline_debt_delta
            .as_deref()
            .unwrap_or("not supplied")
    ));
    out.push_str(&format!(
        "- RIPR Zero status: {}\n",
        report
            .inputs
            .ripr_zero_status
            .as_deref()
            .unwrap_or("not supplied")
    ));
    if let Some(gap_decision_ledger) = report.inputs.gap_decision_ledger.as_deref() {
        out.push_str(&format!("- Gap decision ledger: {gap_decision_ledger}\n"));
    }
    out.push_str(&format!(
        "- Agent receipt: {}\n",
        report
            .inputs
            .agent_receipt
            .as_deref()
            .unwrap_or("not supplied")
    ));

    out.push_str("\nCoverage/grip frontier:\n");
    out.push_str(&format!(
        "- Status: {}\n",
        report.coverage_grip_frontier.status
    ));
    out.push_str(&format!(
        "- Coverage delta: {}\n",
        report
            .coverage_grip_frontier
            .coverage_delta_percent
            .as_deref()
            .unwrap_or("not_available")
    ));
    out.push_str(&format!(
        "- RIPR visible unresolved delta: {}\n",
        report
            .coverage_grip_frontier
            .ripr_visible_unresolved_delta
            .map(|value| value.to_string())
            .unwrap_or_else(|| "not_available".to_string())
    ));
    out.push_str(&format!(
        "- Interpretation: {}\n",
        report.coverage_grip_frontier.interpretation
    ));

    if let Some(history) = report.history.as_ref() {
        out.push_str("\nHistory:\n");
        out.push_str(&format!("- Records: {}\n", history.records));
        out.push_str(&format!(
            "- Baseline resolved total: {}\n",
            history.baseline_resolved_total
        ));
        out.push_str(&format!(
            "- New policy-eligible total: {}\n",
            history.new_policy_eligible_total
        ));
        out.push_str(&format!("- Trend: {}\n", history.trend));
    }

    if !report.warnings.is_empty() {
        out.push_str("\nWarnings:\n");
        for warning in &report.warnings {
            out.push_str(&format!("- {warning}\n"));
        }
    }

    out.push_str("\nLimits: ");
    out.push_str(LIMITS_NOTE);
    out.push('\n');
    out
}

fn route_identity(route: &RepairRoute) -> String {
    route
        .canonical_gap_id
        .as_deref()
        .or(route.gap_id.as_deref())
        .or(route.seam_id.as_deref())
        .unwrap_or("not_available")
        .to_string()
}

fn route_location(route: &RepairRoute) -> String {
    match (route.path.as_deref(), route.line) {
        (Some(path), Some(line)) => format!("{path}:{line}"),
        (Some(path), None) => path.to_string(),
        _ => "not_available".to_string(),
    }
}

fn parse_sources(input: &PrEvidenceLedgerInput) -> ParsedSources {
    let mut parsed = ParsedSources::default();
    parsed.gate = parse_optional_json(
        "gate decision",
        input.gate_path.as_deref(),
        &input.gate_json,
        &mut parsed.warnings,
    );
    parsed.baseline_delta = parse_optional_json(
        "baseline debt delta",
        input.baseline_delta_path.as_deref(),
        &input.baseline_delta_json,
        &mut parsed.warnings,
    );
    parsed.zero_status = parse_optional_json(
        "RIPR Zero status",
        input.zero_status_path.as_deref(),
        &input.zero_status_json,
        &mut parsed.warnings,
    );
    parsed.pr_guidance = parse_optional_json(
        "PR guidance",
        input.pr_guidance_path.as_deref(),
        &input.pr_guidance_json,
        &mut parsed.warnings,
    );
    parsed.gap_ledger = parse_optional_json(
        "gap decision ledger",
        input.gap_ledger_path.as_deref(),
        &input.gap_ledger_json,
        &mut parsed.warnings,
    );
    parsed.recommendation_calibration = parse_optional_json(
        "recommendation calibration",
        input.recommendation_calibration_path.as_deref(),
        &input.recommendation_calibration_json,
        &mut parsed.warnings,
    );
    parsed.agent_receipt = parse_optional_json(
        "agent receipt",
        input.agent_receipt_path.as_deref(),
        &input.agent_receipt_json,
        &mut parsed.warnings,
    );
    parsed.coverage = parse_optional_json(
        "coverage",
        input.coverage_path.as_deref(),
        &input.coverage_json,
        &mut parsed.warnings,
    );
    parsed.history = parse_history(
        input.history_path.as_deref(),
        &input.history_json,
        &mut parsed.warnings,
    );
    parsed
}

fn parse_optional_json(
    label: &str,
    path: Option<&str>,
    text: &Option<Result<String, String>>,
    warnings: &mut Vec<String>,
) -> Option<Value> {
    let path = path?;
    let Some(text) = text else {
        warnings.push(format!(
            "{label} path {path} was supplied but no input text was loaded"
        ));
        return None;
    };
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            warnings.push(format!("optional {label} input {path} is invalid: {error}"));
            return None;
        }
    };
    match serde_json::from_str::<Value>(text) {
        Ok(value) => Some(value),
        Err(error) => {
            warnings.push(format!("optional {label} input {path} is invalid: {error}"));
            None
        }
    }
}

fn parse_history(
    path: Option<&str>,
    text: &Option<Result<String, String>>,
    warnings: &mut Vec<String>,
) -> Option<HistorySummary> {
    let path = path?;
    let text = match text {
        Some(Ok(text)) => text,
        Some(Err(error)) => {
            warnings.push(format!("optional history input {path} is invalid: {error}"));
            return None;
        }
        None => {
            warnings.push(format!(
                "history path {path} was supplied but no input text was loaded"
            ));
            return None;
        }
    };

    let mut records = 0usize;
    let mut waiver_age_max_days = 0usize;
    let mut baseline_resolved_total = 0usize;
    let mut new_policy_eligible_total = 0usize;
    let mut waiver_seen_by_seam = BTreeMap::new();
    let mut waiver_age_days_by_seam: BTreeMap<String, usize> = BTreeMap::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        match serde_json::from_str::<Value>(line) {
            Ok(value) => {
                records += 1;
                baseline_resolved_total += usize_path(&value, &["movement", "baseline_resolved"]);
                new_policy_eligible_total +=
                    usize_path(&value, &["movement", "new_policy_eligible"]);
                for waiver in array_path(&value, &["waivers"]) {
                    let age_days = usize_path(waiver, &["age_days"]);
                    if let Some(seam_id) = string_path(waiver, &["seam_id"]) {
                        *waiver_seen_by_seam.entry(seam_id.clone()).or_insert(0) += 1;
                        waiver_age_days_by_seam
                            .entry(seam_id)
                            .and_modify(|current| *current = (*current).max(age_days))
                            .or_insert(age_days);
                    }
                    waiver_age_max_days = waiver_age_max_days.max(age_days);
                }
            }
            Err(error) => warnings.push(format!(
                "optional history input {path} contains an invalid JSONL record: {error}"
            )),
        }
    }

    let trend = if baseline_resolved_total > new_policy_eligible_total {
        "improving"
    } else if new_policy_eligible_total > baseline_resolved_total {
        "regressing"
    } else if records > 0 {
        "stable"
    } else {
        "unknown"
    }
    .to_string();

    Some(HistorySummary {
        source: path.to_string(),
        records,
        waiver_age_max_days,
        baseline_resolved_total,
        new_policy_eligible_total,
        trend,
        waiver_seen_by_seam,
        waiver_age_days_by_seam,
    })
}

fn movement_from_sources(
    baseline_delta: Option<&Value>,
    zero_status: Option<&Value>,
    gate: Option<&Value>,
) -> Movement {
    let new_policy_eligible = baseline_delta
        .map(|value| usize_path(value, &["delta", "new_policy_eligible"]))
        .or_else(|| {
            zero_status.map(|value| usize_path(value, &["ripr_zero", "new_policy_eligible"]))
        })
        .unwrap_or(0);
    let baseline_still_present = baseline_delta
        .map(|value| usize_path(value, &["delta", "still_present"]))
        .or_else(|| zero_status.map(|value| usize_path(value, &["baseline", "still_present"])))
        .unwrap_or(0);
    let baseline_resolved = baseline_delta
        .map(|value| usize_path(value, &["delta", "resolved"]))
        .or_else(|| zero_status.map(|value| usize_path(value, &["baseline", "resolved"])))
        .unwrap_or(0);
    let acknowledged = baseline_delta
        .map(|value| usize_path(value, &["delta", "acknowledged"]))
        .or_else(|| gate.map(|value| usize_path(value, &["summary", "acknowledged"])))
        .unwrap_or(0);
    let suppressed = baseline_delta
        .map(|value| usize_path(value, &["delta", "suppressed"]))
        .or_else(|| gate.map(|value| usize_path(value, &["summary", "suppressed"])))
        .unwrap_or(0);
    let blocking_candidates = gate
        .map(|value| usize_path(value, &["summary", "blocking"]))
        .or_else(|| {
            zero_status.map(|value| usize_path(value, &["ripr_zero", "blocking_candidates"]))
        })
        .unwrap_or(0);
    let visible_unresolved = zero_status
        .map(|value| usize_path(value, &["ripr_zero", "visible_unresolved"]))
        .unwrap_or(baseline_still_present + new_policy_eligible + acknowledged);
    let ripr_zero_state = zero_status
        .and_then(|value| string_path(value, &["ripr_zero", "state"]))
        .unwrap_or_else(|| "unknown".to_string());
    Movement {
        new_policy_eligible,
        baseline_still_present,
        baseline_resolved,
        acknowledged,
        suppressed,
        blocking_candidates,
        visible_unresolved,
        ripr_zero_state,
    }
}

fn gate_summary_from_value(gate: Option<&Value>) -> GateSummary {
    let Some(gate) = gate else {
        return GateSummary {
            mode: None,
            decision: None,
            pass_fail_authority: None,
            acknowledgement_label: None,
        };
    };
    GateSummary {
        mode: string_path(gate, &["mode"]).or_else(|| string_path(gate, &["policy", "mode"])),
        decision: string_path(gate, &["status"]),
        pass_fail_authority: Some("ripr gate evaluate".to_string()),
        acknowledgement_label: array_path(gate, &["policy", "acknowledgement_labels"])
            .first()
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
    }
}

fn labels_from_gate(gate: Option<&Value>) -> Vec<String> {
    gate.and_then(|value| path_value(value, &["inputs", "labels"]))
        .and_then(Value::as_array)
        .map(|labels| labels.iter().filter_map(string_value).collect())
        .unwrap_or_default()
}

fn waiver_records_from_gate(
    gate: Option<&Value>,
    history: Option<&HistorySummary>,
) -> Vec<WaiverRecord> {
    let Some(gate) = gate else {
        return Vec::new();
    };
    array_path(gate, &["decisions"])
        .iter()
        .filter(|decision| string_path(decision, &["decision"]).as_deref() == Some("acknowledged"))
        .map(|decision| {
            let seam_id = string_path(decision, &["seam_id"]);
            let label = string_path(decision, &["policy", "acknowledgement_label"])
                .or_else(|| {
                    array_path(gate, &["policy", "acknowledgement_labels"])
                        .first()
                        .and_then(|value| value.as_str())
                        .map(ToOwned::to_owned)
                })
                .unwrap_or_else(|| "ripr-waive".to_string());
            let previous = seam_id
                .as_ref()
                .and_then(|seam_id| {
                    history.and_then(|history| history.waiver_seen_by_seam.get(seam_id))
                })
                .copied()
                .unwrap_or(0);
            let age_days = seam_id
                .as_ref()
                .and_then(|seam_id| {
                    history.and_then(|history| history.waiver_age_days_by_seam.get(seam_id))
                })
                .copied()
                .unwrap_or(0);
            WaiverRecord {
                label,
                decision_id: string_path(decision, &["id"]),
                canonical_gap_id: canonical_gap_id_from_value(decision),
                seam_id,
                age_prs: previous + 1,
                age_days,
                reason: string_path(decision, &["gate_reason"])
                    .unwrap_or_else(|| "accepted for this PR".to_string()),
                still_visible: true,
            }
        })
        .collect()
}

fn suppression_records_from_sources(
    gate: Option<&Value>,
    baseline_delta: Option<&Value>,
) -> Vec<SuppressionRecord> {
    let mut records = Vec::new();
    if let Some(gate) = gate {
        for decision in array_path(gate, &["decisions"]) {
            let is_suppressed = string_path(decision, &["decision"]).as_deref()
                == Some("suppressed")
                || bool_path(decision, &["evidence", "suppressed"])
                || bool_path(decision, &["evidence", "configured_off"]);
            if is_suppressed {
                records.push(SuppressionRecord {
                    decision_id: string_path(decision, &["id"]),
                    canonical_gap_id: canonical_gap_id_from_value(decision),
                    seam_id: string_path(decision, &["seam_id"]),
                    source: string_path(decision, &["policy", "suppression_source"])
                        .or_else(|| Some("gate_decision".to_string())),
                    owner: string_path(decision, &["policy", "owner"]),
                    reason: string_path(decision, &["gate_reason"])
                        .or_else(|| string_path(decision, &["reason"])),
                    still_visible: true,
                });
            }
        }
    }
    if records.is_empty()
        && let Some(delta) = baseline_delta
    {
        for item in array_path(delta, &["items"]) {
            if string_path(item, &["bucket"]).as_deref() == Some("suppressed") {
                records.push(SuppressionRecord {
                    decision_id: string_path(item, &["identity", "id"]),
                    canonical_gap_id: canonical_gap_id_from_value(item),
                    seam_id: string_path(item, &["identity", "seam_id"]),
                    source: Some("baseline_debt_delta".to_string()),
                    owner: None,
                    reason: string_path(item, &["reason"]),
                    still_visible: true,
                });
            }
        }
    }
    records
}

fn repair_receipts_from_sources(
    agent_receipt_path: Option<&str>,
    agent_receipt: Option<&Value>,
    recommendation_calibration_path: Option<&str>,
    recommendation_calibration: Option<&Value>,
) -> Vec<RepairReceipt> {
    let mut receipts = Vec::new();
    if let Some((path, receipt)) = agent_receipt_path.zip(agent_receipt) {
        let state = string_path(receipt, &["provenance", "movement"])
            .or_else(|| string_path(receipt, &["static_movement", "state"]))
            .or_else(|| string_path(receipt, &["seam", "change"]))
            .unwrap_or_else(|| "unknown".to_string());
        receipts.push(RepairReceipt {
            source: "agent_receipt".to_string(),
            canonical_gap_id: canonical_gap_id_from_value(receipt),
            seam_id: string_path(receipt, &["provenance", "seam_id"])
                .or_else(|| string_path(receipt, &["seam", "seam_id"]))
                .or_else(|| string_path(receipt, &["guidance", "seam_id"])),
            receipt_state: receipt_lifecycle_state_from_receipt_value(receipt),
            static_movement: StaticMovement {
                state,
                source: "agent_receipt".to_string(),
                artifact: path.to_string(),
            },
            focused_test: string_path(receipt, &["test_changed"])
                .or_else(|| string_path(receipt, &["suggested_test", "actual_file"]))
                .or_else(|| string_path(receipt, &["suggested_test", "near_test"])),
            receipt: path.to_string(),
        });
    }

    if let Some((path, calibration)) =
        recommendation_calibration_path.zip(recommendation_calibration)
    {
        for recommendation in array_path(calibration, &["recommendations"])
            .iter()
            .filter(|recommendation| {
                matches!(
                    string_path(recommendation, &["static_movement", "state"]).as_deref(),
                    Some("improved" | "unchanged" | "regressed" | "resolved")
                )
            })
            .take(3)
        {
            receipts.push(RepairReceipt {
                source: "recommendation_calibration".to_string(),
                canonical_gap_id: canonical_gap_id_from_value(recommendation),
                seam_id: string_path(recommendation, &["seam_id"]),
                receipt_state: receipt_lifecycle_state_from_movement(
                    string_path(recommendation, &["static_movement", "state"]).as_deref(),
                ),
                static_movement: StaticMovement {
                    state: string_path(recommendation, &["static_movement", "state"])
                        .unwrap_or_else(|| "unknown".to_string()),
                    source: string_path(recommendation, &["static_movement", "source"])
                        .unwrap_or_else(|| "recommendation_calibration".to_string()),
                    artifact: path.to_string(),
                },
                focused_test: string_path(recommendation, &["suggested_test", "recommended_file"])
                    .or_else(|| string_path(recommendation, &["suggested_test", "near_test"])),
                receipt: path.to_string(),
            });
        }
    }
    receipts
}

fn coverage_frontier(
    path: Option<&str>,
    coverage: Option<&Value>,
    movement: &Movement,
) -> CoverageGripFrontier {
    let Some(coverage) = coverage else {
        return CoverageGripFrontier {
            status: "not_available".to_string(),
            coverage_delta_percent: None,
            ripr_visible_unresolved_delta: None,
            interpretation:
                "coverage input not supplied; coverage and behavioral grip remain separate axes"
                    .to_string(),
            quadrants: CoverageQuadrants::default(),
        };
    };
    let coverage_delta = string_or_number_path(coverage, &["coverage_delta_percent"])
        .or_else(|| string_or_number_path(coverage, &["coverage", "delta_percent"]))
        .or_else(|| string_or_number_path(coverage, &["summary", "coverage_delta_percent"]));
    let ripr_delta = i64_path(coverage, &["ripr_visible_unresolved_delta"])
        .or_else(|| i64_path(coverage, &["ripr", "visible_unresolved_delta"]));
    let quadrants = CoverageQuadrants {
        covered_with_ripr_gap: usize_path_any(
            coverage,
            &[
                &["quadrants", "covered_with_ripr_gap"],
                &["covered_with_ripr_gap"],
            ],
        ),
        covered_without_ripr_gap: usize_path_any(
            coverage,
            &[
                &["quadrants", "covered_without_ripr_gap"],
                &["covered_without_ripr_gap"],
            ],
        ),
        uncovered_with_ripr_gap: usize_path_any(
            coverage,
            &[
                &["quadrants", "uncovered_with_ripr_gap"],
                &["uncovered_with_ripr_gap"],
            ],
        ),
        uncovered_without_ripr_gap: usize_path_any(
            coverage,
            &[
                &["quadrants", "uncovered_without_ripr_gap"],
                &["uncovered_without_ripr_gap"],
            ],
        ),
    };
    let available = coverage_delta.is_some()
        || ripr_delta.is_some()
        || quadrants.covered_with_ripr_gap > 0
        || quadrants.covered_without_ripr_gap > 0
        || quadrants.uncovered_with_ripr_gap > 0
        || quadrants.uncovered_without_ripr_gap > 0;
    let status = if available {
        "available"
    } else {
        "unsupported"
    }
    .to_string();
    let interpretation = if coverage_delta.as_deref() == Some("0")
        || coverage_delta.as_deref() == Some("0.0")
    {
        if movement.baseline_resolved > 0 || ripr_delta.unwrap_or(0) < 0 {
            "behavioral grip improved without line-coverage movement".to_string()
        } else {
            "coverage was flat; inspect RIPR movement separately".to_string()
        }
    } else if available {
        "coverage and RIPR movement are reported separately; coverage is execution evidence, not adequacy".to_string()
    } else {
        format!(
            "coverage input {} has no supported coverage/grip frontier fields",
            path.unwrap_or("unknown")
        )
    };
    CoverageGripFrontier {
        status,
        coverage_delta_percent: coverage_delta,
        ripr_visible_unresolved_delta: ripr_delta,
        interpretation,
        quadrants,
    }
}

fn top_repair_route(
    gap_ledger: Option<&Value>,
    zero_status: Option<&Value>,
    pr_guidance: Option<&Value>,
    gate: Option<&Value>,
    baseline_delta: Option<&Value>,
) -> Option<RepairRoute> {
    gap_ledger
        .and_then(route_from_gap_ledger)
        .or_else(|| zero_status.and_then(route_from_zero_status))
        .or_else(|| pr_guidance.and_then(route_from_pr_guidance))
        .or_else(|| gate.and_then(route_from_gate))
        .or_else(|| baseline_delta.and_then(route_from_baseline_delta))
}

fn route_from_gap_ledger(value: &Value) -> Option<RepairRoute> {
    let record = first_repairable_gap_record(value)?;
    let route = record.get("repair_route")?;
    let anchor = record.get("anchor");
    let gap_id = string_path(record, &["gap_id"]);
    let seam_id = string_from_sources(&[
        (Some(record), &["seam_id"]),
        (anchor, &["seam_id"]),
        (Some(route), &["seam_id"]),
    ]);
    Some(RepairRoute {
        source: "gap_decision_ledger".to_string(),
        gap_id,
        canonical_gap_id: string_path(record, &["canonical_gap_id"]),
        language: string_path(record, &["language"]),
        language_status: string_path(record, &["language_status"]),
        seam_id: seam_id.clone(),
        path: string_from_sources(&[(anchor, &["file"]), (Some(route), &["target_file"])]),
        line: u64_from_sources(&[(anchor, &["line"]), (Some(route), &["target_line"])]),
        repair_route: string_path(route, &["route_kind"]),
        missing_discriminator: string_from_sources(&[
            (Some(record), &["missing_discriminator"]),
            (Some(record), &["evidence_class"]),
            (Some(record), &["kind"]),
        ]),
        suggested_test: string_path(route, &["assertion_shape"])
            .or_else(|| string_path(route, &["suggested_test"])),
        related_test: string_path(route, &["related_test"])
            .or_else(|| string_path(route, &["target_file"])),
        verify_command: first_string_array_item(record, &["verification_commands"]),
        receipt_command: string_path(record, &["receipt_command"]),
        receipt_state: string_path(record, &["receipt", "state"])
            .or_else(|| string_path(record, &["receipt", "movement"]))
            .map(|state| receipt_lifecycle_state(Some(&state))),
        static_limit_kind: string_path(record, &["static_limit_kind"]),
        static_limit_detail: string_path(record, &["static_limit_detail"]),
        agent_command: string_path(route, &["agent_command"]).or_else(|| {
            seam_id.map(|id| {
                format!("ripr agent start --root . --seam-id {id} --out target/ripr/workflow")
            })
        }),
    })
}

fn first_repairable_gap_record(gap_ledger: &Value) -> Option<&Value> {
    gap_records(gap_ledger).into_iter().find(|record| {
        string_path(record, &["language"]).is_some_and(|value| value == "rust")
            && string_path(record, &["language_status"]).is_some_and(|value| value == "stable")
            && string_path(record, &["scope"]).is_some_and(|value| value == "pr_local")
            && string_path(record, &["gap_state"]).is_some_and(|value| value == "actionable")
            && string_path(record, &["repairability"]).is_some_and(|value| value == "repairable")
            && string_path(record, &["policy_state"])
                .is_some_and(|value| value == "new" || value == "reintroduced")
            && record.get("repair_route").is_some()
            && record
                .get("verification_commands")
                .and_then(Value::as_array)
                .is_some_and(|commands| {
                    commands
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|command| !command.trim().is_empty())
                })
    })
}

fn gap_records(value: &Value) -> Vec<&Value> {
    if let Some(records) = value.as_array() {
        return records.iter().collect();
    }
    if let Some(records) = value.get("records").and_then(Value::as_array) {
        return records.iter().collect();
    }
    if let Some(records) = value.get("gap_records").and_then(Value::as_array) {
        return records.iter().collect();
    }
    value
        .get("cases")
        .and_then(Value::as_array)
        .map(|cases| {
            cases
                .iter()
                .filter_map(|case| case.get("expected_gap_record"))
                .collect()
        })
        .unwrap_or_default()
}

fn route_from_zero_status(value: &Value) -> Option<RepairRoute> {
    let route = array_path(value, &["repair_routes"]).first().copied()?;
    Some(RepairRoute {
        source: "ripr_zero_status".to_string(),
        gap_id: string_path(route, &["gap_id"]),
        canonical_gap_id: canonical_gap_id_from_value(route),
        language: string_path(route, &["language"]),
        language_status: string_path(route, &["language_status"]),
        seam_id: string_path(route, &["seam_id"]),
        path: string_path(route, &["path"]),
        line: u64_path(route, &["line"]),
        repair_route: string_path(route, &["repair_route"])
            .or_else(|| string_path(route, &["route_kind"])),
        missing_discriminator: string_path(route, &["missing_discriminator"]),
        suggested_test: string_path(route, &["suggested_test"]),
        related_test: string_path(route, &["related_test"]),
        verify_command: string_path(route, &["verify_command"]),
        receipt_command: string_path(route, &["receipt_command"]),
        receipt_state: string_path(route, &["receipt_state"])
            .map(|state| receipt_lifecycle_state(Some(&state))),
        static_limit_kind: string_path(route, &["static_limit_kind"]),
        static_limit_detail: string_path(route, &["static_limit_detail"]),
        agent_command: string_path(route, &["agent_command"]),
    })
}

fn route_from_pr_guidance(value: &Value) -> Option<RepairRoute> {
    let item = array_path(value, &["comments"])
        .first()
        .copied()
        .or_else(|| array_path(value, &["summary_only"]).first().copied())?;
    let seam_id = string_path(item, &["seam_id"]);
    Some(RepairRoute {
        source: "pr_guidance".to_string(),
        gap_id: string_path(item, &["gap_id"]),
        canonical_gap_id: canonical_gap_id_from_value(item),
        language: string_path(item, &["language"]),
        language_status: string_path(item, &["language_status"]),
        seam_id: seam_id.clone(),
        path: string_path(item, &["placement", "path"])
            .or_else(|| string_path(item, &["seam", "file"])),
        line: u64_path(item, &["placement", "line"]).or_else(|| u64_path(item, &["seam", "line"])),
        repair_route: string_path(item, &["repair_route", "route_kind"])
            .or_else(|| string_path(item, &["repair_route"])),
        missing_discriminator: string_path(item, &["missing_discriminator"]),
        suggested_test: string_path(item, &["suggested_test", "assertion_shape"])
            .or_else(|| string_path(item, &["suggested_test", "intent"])),
        related_test: string_path(item, &["suggested_test", "near_test"]),
        verify_command: string_path(item, &["llm_guidance", "verify_command"]),
        receipt_command: string_path(item, &["llm_guidance", "receipt_command"]),
        receipt_state: string_path(item, &["receipt_state"])
            .map(|state| receipt_lifecycle_state(Some(&state))),
        static_limit_kind: string_path(item, &["static_limit_kind"]),
        static_limit_detail: string_path(item, &["static_limit_detail"]),
        agent_command: seam_id.map(|id| {
            format!("ripr agent start --root . --seam-id {id} --out target/ripr/workflow")
        }),
    })
}

fn route_from_gate(value: &Value) -> Option<RepairRoute> {
    let item = array_path(value, &["decisions"])
        .iter()
        .find(|decision| {
            matches!(
                string_path(decision, &["decision"]).as_deref(),
                Some("blocking" | "advisory" | "acknowledged")
            )
        })
        .copied()?;
    let seam_id = string_path(item, &["seam_id"]);
    Some(RepairRoute {
        source: "gate_decision".to_string(),
        gap_id: string_path(item, &["gap_id"]),
        canonical_gap_id: canonical_gap_id_from_value(item),
        language: string_path(item, &["language"]),
        language_status: string_path(item, &["language_status"]),
        seam_id: seam_id.clone(),
        path: string_path(item, &["placement", "path"]),
        line: u64_path(item, &["placement", "line"]),
        repair_route: string_path(item, &["repair_route"])
            .or_else(|| string_path(item, &["evidence", "repair_route"])),
        missing_discriminator: string_path(item, &["evidence", "missing_discriminator"]),
        suggested_test: string_path(item, &["evidence", "assertion_shape"]),
        related_test: string_path(item, &["evidence", "recommended_test"]),
        verify_command: Some("ripr agent verify --root . --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --json".to_string()),
        receipt_command: None,
        receipt_state: None,
        static_limit_kind: string_path(item, &["static_limit_kind"])
            .or_else(|| string_path(item, &["evidence", "static_limit_kind"])),
        static_limit_detail: string_path(item, &["static_limit_detail"])
            .or_else(|| string_path(item, &["evidence", "static_limit_detail"])),
        agent_command: seam_id.map(|id| {
            format!("ripr agent start --root . --seam-id {id} --out target/ripr/workflow")
        }),
    })
}

fn route_from_baseline_delta(value: &Value) -> Option<RepairRoute> {
    let item = array_path(value, &["items"])
        .iter()
        .find(|item| {
            matches!(
                string_path(item, &["bucket"]).as_deref(),
                Some("new_policy_eligible" | "still_present" | "acknowledged")
            )
        })
        .copied()?;
    let seam_id = string_path(item, &["identity", "seam_id"]);
    Some(RepairRoute {
        source: "baseline_debt_delta".to_string(),
        gap_id: string_path(item, &["gap_id"])
            .or_else(|| string_path(item, &["identity", "gap_id"])),
        canonical_gap_id: canonical_gap_id_from_value(item),
        language: string_path(item, &["language"]),
        language_status: string_path(item, &["language_status"]),
        seam_id: seam_id.clone(),
        path: string_path(item, &["path"]),
        line: u64_path(item, &["line"]),
        repair_route: string_path(item, &["repair", "route_kind"])
            .or_else(|| string_path(item, &["repair_route"])),
        missing_discriminator: string_path(item, &["missing_discriminator"]),
        suggested_test: string_path(item, &["suggested_test", "assertion_shape"]),
        related_test: string_path(item, &["suggested_test", "recommended_test"]),
        verify_command: string_path(item, &["repair", "verify_command"]),
        receipt_command: string_path(item, &["repair", "receipt_command"]),
        receipt_state: string_path(item, &["receipt_state"])
            .map(|state| receipt_lifecycle_state(Some(&state))),
        static_limit_kind: string_path(item, &["static_limit_kind"]),
        static_limit_detail: string_path(item, &["static_limit_detail"]),
        agent_command: seam_id.map(|id| {
            format!("ripr agent start --root . --seam-id {id} --out target/ripr/workflow")
        }),
    })
}

fn pr_json(pr: &PrIdentity) -> Value {
    let number = pr
        .number
        .parse::<u64>()
        .ok()
        .map(Value::from)
        .unwrap_or_else(|| Value::String(pr.number.clone()));
    json!({
        "number": number,
        "base": pr.base,
        "head": pr.head,
        "labels": pr.labels,
    })
}

fn inputs_json(inputs: &LedgerInputs) -> Value {
    let mut value = json!({
        "gate_decision": inputs.gate_decision,
        "baseline_debt_delta": inputs.baseline_debt_delta,
        "ripr_zero_status": inputs.ripr_zero_status,
        "pr_guidance": inputs.pr_guidance,
        "recommendation_calibration": inputs.recommendation_calibration,
        "agent_receipt": inputs.agent_receipt,
        "coverage": inputs.coverage,
        "history": inputs.history,
    });
    if let Some(gap_decision_ledger) = inputs.gap_decision_ledger.as_ref()
        && let Some(fields) = value.as_object_mut()
    {
        fields.insert(
            "gap_decision_ledger".to_string(),
            Value::String(gap_decision_ledger.clone()),
        );
    }
    value
}

fn movement_json(movement: &Movement) -> Value {
    json!({
        "new_policy_eligible": movement.new_policy_eligible,
        "baseline_still_present": movement.baseline_still_present,
        "baseline_resolved": movement.baseline_resolved,
        "acknowledged": movement.acknowledged,
        "suppressed": movement.suppressed,
        "blocking_candidates": movement.blocking_candidates,
        "visible_unresolved": movement.visible_unresolved,
        "ripr_zero_state": movement.ripr_zero_state,
    })
}

fn gate_json(gate: &GateSummary) -> Value {
    json!({
        "mode": gate.mode,
        "decision": gate.decision,
        "pass_fail_authority": gate.pass_fail_authority,
        "acknowledgement_label": gate.acknowledgement_label,
    })
}

fn waiver_json(record: &WaiverRecord) -> Value {
    json!({
        "label": record.label,
        "decision_id": record.decision_id,
        "canonical_gap_id": record.canonical_gap_id,
        "seam_id": record.seam_id,
        "age_prs": record.age_prs,
        "age_days": record.age_days,
        "reason": record.reason,
        "still_visible": record.still_visible,
    })
}

fn suppression_json(record: &SuppressionRecord) -> Value {
    json!({
        "decision_id": record.decision_id,
        "canonical_gap_id": record.canonical_gap_id,
        "seam_id": record.seam_id,
        "source": record.source,
        "owner": record.owner,
        "reason": record.reason,
        "still_visible": record.still_visible,
    })
}

fn repair_receipt_json(record: &RepairReceipt) -> Value {
    json!({
        "source": record.source,
        "canonical_gap_id": record.canonical_gap_id,
        "seam_id": record.seam_id,
        "receipt_state": record.receipt_state,
        "static_movement": {
            "state": record.static_movement.state,
            "source": record.static_movement.source,
            "artifact": record.static_movement.artifact,
        },
        "focused_test": record.focused_test,
        "receipt": record.receipt,
    })
}

fn coverage_json(frontier: &CoverageGripFrontier) -> Value {
    json!({
        "status": frontier.status,
        "coverage_delta_percent": frontier.coverage_delta_percent,
        "ripr_visible_unresolved_delta": frontier.ripr_visible_unresolved_delta,
        "interpretation": frontier.interpretation,
        "quadrants": {
            "covered_with_ripr_gap": frontier.quadrants.covered_with_ripr_gap,
            "covered_without_ripr_gap": frontier.quadrants.covered_without_ripr_gap,
            "uncovered_with_ripr_gap": frontier.quadrants.uncovered_with_ripr_gap,
            "uncovered_without_ripr_gap": frontier.quadrants.uncovered_without_ripr_gap,
        },
    })
}

fn repair_route_json(route: &RepairRoute) -> Value {
    let mut value = json!({
        "source": route.source,
        "canonical_gap_id": route.canonical_gap_id,
        "language": route.language,
        "language_status": route.language_status,
        "seam_id": route.seam_id,
        "path": route.path,
        "line": route.line,
        "repair_route": route.repair_route,
        "missing_discriminator": route.missing_discriminator,
        "suggested_test": route.suggested_test,
        "related_test": route.related_test,
        "verify_command": route.verify_command,
        "receipt_command": route.receipt_command,
        "receipt_state": route.receipt_state,
        "static_limit_kind": route.static_limit_kind,
        "static_limit_detail": route.static_limit_detail,
        "agent_command": route.agent_command,
    });
    if let Some(gap_id) = route.gap_id.as_ref()
        && let Some(fields) = value.as_object_mut()
    {
        fields.insert("gap_id".to_string(), Value::String(gap_id.clone()));
    }
    value
}

fn history_json(history: &HistorySummary) -> Value {
    json!({
        "source": history.source,
        "records": history.records,
        "waiver_age_max_days": history.waiver_age_max_days,
        "baseline_resolved_total": history.baseline_resolved_total,
        "new_policy_eligible_total": history.new_policy_eligible_total,
        "trend": history.trend,
    })
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

fn string_from_sources(sources: &[(Option<&Value>, &[&str])]) -> Option<String> {
    sources
        .iter()
        .find_map(|(value, path)| value.and_then(|value| string_path(value, path)))
}

fn canonical_gap_id_from_value(value: &Value) -> Option<String> {
    string_path(value, &["canonical_gap_id"])
        .or_else(|| string_path(value, &["identity", "canonical_gap_id"]))
        .or_else(|| string_path(value, &["evidence_record", "canonical_gap_id"]))
        .or_else(|| string_path(value, &["provenance", "canonical_gap_id"]))
        .or_else(|| string_path(value, &["seam", "canonical_gap_id"]))
        .or_else(|| string_path(value, &["guidance", "canonical_gap_id"]))
}

fn string_value(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn bool_path(value: &Value, path: &[&str]) -> bool {
    path_value(value, path)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn u64_path(value: &Value, path: &[&str]) -> Option<u64> {
    path_value(value, path).and_then(Value::as_u64)
}

fn u64_from_sources(sources: &[(Option<&Value>, &[&str])]) -> Option<u64> {
    sources
        .iter()
        .find_map(|(value, path)| value.and_then(|value| u64_path(value, path)))
}

fn first_string_array_item(value: &Value, path: &[&str]) -> Option<String> {
    path_value(value, path)
        .and_then(Value::as_array)?
        .iter()
        .filter_map(Value::as_str)
        .find(|item| !item.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn i64_path(value: &Value, path: &[&str]) -> Option<i64> {
    path_value(value, path).and_then(Value::as_i64)
}

fn usize_path(value: &Value, path: &[&str]) -> usize {
    u64_path(value, path)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
}

fn usize_path_any(value: &Value, paths: &[&[&str]]) -> usize {
    paths
        .iter()
        .find_map(|path| u64_path(value, path))
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
}

fn string_or_number_path(value: &Value, path: &[&str]) -> Option<String> {
    path_value(value, path).and_then(|value| {
        string_value(value).or_else(|| {
            value.as_f64().map(|number| {
                if number.fract() == 0.0 {
                    format!("{number:.0}")
                } else {
                    number.to_string()
                }
            })
        })
    })
}

pub(crate) use crate::output::path::display_path;

#[cfg(test)]
mod tests {
    use super::{
        PrEvidenceLedgerInput, build_pr_evidence_ledger_report, render_pr_evidence_ledger_json,
        render_pr_evidence_ledger_markdown,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn pr_evidence_ledger_joins_primary_artifacts() -> Result<(), String> {
        let gate = r#"{
          "schema_version": "0.1",
          "mode": "acknowledgeable",
          "status": "acknowledged",
          "summary": {"blocking": 0, "acknowledged": 1, "suppressed": 1},
          "policy": {"acknowledgement_labels": ["ripr-waive"]},
          "inputs": {"labels": ["ripr-waive"]},
          "decisions": [
            {
              "id": "ripr-gate-ack",
              "decision": "acknowledged",
              "seam_id": "ack",
              "source_id": "ripr-review-ack",
              "evidence_record": {"canonical_gap_id": "pricing::ack::boundary"},
              "gate_reason": "policy-eligible gap acknowledged by ripr-waive",
              "policy": {"acknowledgement_label": "ripr-waive"},
              "placement": {"path": "src/ack.rs", "line": 5},
              "evidence": {
                "missing_discriminator": "ack == 5",
                "assertion_shape": "assert_eq!(ack(), 5)",
                "recommended_test": "tests/ack.rs::boundary"
              }
            },
            {
              "id": "ripr-gate-suppressed",
              "decision": "suppressed",
              "seam_id": "suppressed",
              "evidence_record": {"canonical_gap_id": "pricing::suppressed::boundary"},
              "gate_reason": "configured-hidden candidate preserved",
              "evidence": {"suppressed": true}
            }
          ]
        }"#;
        let delta = r#"{
          "schema_version": "0.1",
          "kind": "baseline_debt_delta",
          "delta": {
            "still_present": 2,
            "resolved": 3,
            "new_policy_eligible": 1,
            "acknowledged": 1,
            "suppressed": 1
          },
          "items": [
            {
              "bucket": "new_policy_eligible",
              "identity": {"canonical_gap_id": "pricing::new::boundary", "seam_id": "new", "id": "ripr-gate-new"},
              "path": "src/new.rs",
              "line": 4,
              "missing_discriminator": "new == 4",
              "suggested_test": {"assertion_shape": "assert_eq!(new(), 4)", "recommended_test": "tests/new.rs::boundary"},
              "repair": {"verify_command": "ripr agent verify --json"}
            }
          ]
        }"#;
        let zero = r#"{
          "schema_version": "0.1",
          "kind": "ripr_zero_status",
          "ripr_zero": {"state": "not_yet", "visible_unresolved": 4, "new_policy_eligible": 1, "blocking_candidates": 0},
          "baseline": {"still_present": 2, "resolved": 3},
          "repair_routes": [
            {
              "source": "baseline_debt_delta",
              "canonical_gap_id": "pricing::new::boundary",
              "seam_id": "new",
              "path": "src/new.rs",
              "line": 4,
              "missing_discriminator": "new == 4",
              "suggested_test": "assert_eq!(new(), 4)",
              "related_test": "tests/new.rs::boundary",
              "verify_command": "ripr agent verify --json",
              "agent_command": "ripr agent start --root . --seam-id new --out target/ripr/workflow"
            }
          ]
        }"#;
        let agent_receipt = r#"{
          "schema_version": "0.3",
          "provenance": {"canonical_gap_id": "pricing::new::boundary", "seam_id": "new", "movement": "improved"},
          "test_changed": "tests/new.rs::boundary"
        }"#;
        let history = r#"{"movement":{"baseline_resolved":2,"new_policy_eligible":0},"waivers":[{"seam_id":"ack","age_days":2}]}"#;

        let report = build_pr_evidence_ledger_report(PrEvidenceLedgerInput {
            root: ".".to_string(),
            generated_at: "unix_ms:1000".to_string(),
            pr_number: "123".to_string(),
            base: "base".to_string(),
            head: "head".to_string(),
            labels: Vec::new(),
            gate_path: Some("gate.json".to_string()),
            baseline_delta_path: Some("delta.json".to_string()),
            zero_status_path: Some("zero.json".to_string()),
            pr_guidance_path: None,
            gap_ledger_path: None,
            recommendation_calibration_path: None,
            agent_receipt_path: Some("agent-receipt.json".to_string()),
            coverage_path: None,
            history_path: Some("history.jsonl".to_string()),
            gate_json: Some(Ok(gate.to_string())),
            baseline_delta_json: Some(Ok(delta.to_string())),
            zero_status_json: Some(Ok(zero.to_string())),
            pr_guidance_json: None,
            gap_ledger_json: None,
            recommendation_calibration_json: None,
            agent_receipt_json: Some(Ok(agent_receipt.to_string())),
            coverage_json: None,
            history_json: Some(Ok(history.to_string())),
        });
        let rendered = render_pr_evidence_ledger_json(&report)?;
        assert!(rendered.contains("\"kind\": \"pr_evidence_ledger\""));
        assert!(rendered.contains("\"new_policy_eligible\": 1"));
        assert!(rendered.contains("\"baseline_resolved\": 3"));
        assert!(rendered.contains("\"decision\": \"acknowledged\""));
        assert!(rendered.contains("\"label\": \"ripr-waive\""));
        assert!(rendered.contains("\"age_prs\": 2"));
        assert!(rendered.contains("\"age_days\": 2"));
        assert!(rendered.contains("\"source\": \"agent_receipt\""));
        assert!(rendered.contains("\"state\": \"improved\""));
        assert!(rendered.contains("\"status\": \"not_available\""));
        assert!(rendered.contains("\"top_repair_route\""));
        assert!(rendered.contains("\"canonical_gap_id\": \"pricing::ack::boundary\""));
        assert!(rendered.contains("\"canonical_gap_id\": \"pricing::suppressed::boundary\""));
        assert!(rendered.contains("\"canonical_gap_id\": \"pricing::new::boundary\""));

        let markdown = render_pr_evidence_ledger_markdown(&report);
        assert!(markdown.contains("# RIPR PR Evidence Ledger"));
        assert!(markdown.contains("Start here:"));
        assert!(markdown.contains("Supporting movement counts:"));
        assert!(markdown.contains("| Baseline gaps resolved | 3 |"));
        assert!(markdown.contains("Limits: Read-only advisory PR evidence ledger"));
        Ok(())
    }

    #[test]
    fn pr_evidence_ledger_routes_repairable_gap_records() -> Result<(), String> {
        let gap_ledger = r#"{
          "schema_version": "0.1",
          "kind": "gap_decision_ledger",
          "gap_records": [
            {
              "gap_id": "gap:pr:pricing:threshold-boundary",
              "canonical_gap_id": "gap:rust:pricing:discount:threshold-boundary",
              "kind": "MissingBoundaryAssertion",
              "language": "rust",
              "language_status": "stable",
              "scope": "pr_local",
              "evidence_class": "predicate_boundary",
              "gap_state": "actionable",
              "policy_state": "new",
              "repairability": "repairable",
              "anchor": {
                "file": "src/pricing.rs",
                "line": 42,
                "owner": "pricing::discount"
              },
              "repair_route": {
                "route_kind": "AddBoundaryAssertion",
                "target_file": "tests/pricing.rs",
                "assertion_shape": "assert_eq!(discount(100, 100), 90)",
                "related_test": "tests/pricing.rs::discount_threshold"
              },
              "verification_commands": [
                "cargo xtask fixtures boundary_gap",
                "cargo xtask goldens check"
              ]
            }
          ]
        }"#;

        let report = build_pr_evidence_ledger_report(PrEvidenceLedgerInput {
            root: ".".to_string(),
            generated_at: "unix_ms:1000".to_string(),
            pr_number: "123".to_string(),
            base: "base".to_string(),
            head: "head".to_string(),
            labels: Vec::new(),
            gate_path: None,
            baseline_delta_path: None,
            zero_status_path: None,
            pr_guidance_path: None,
            gap_ledger_path: Some("gap-ledger.json".to_string()),
            recommendation_calibration_path: None,
            agent_receipt_path: None,
            coverage_path: None,
            history_path: None,
            gate_json: None,
            baseline_delta_json: None,
            zero_status_json: None,
            pr_guidance_json: None,
            gap_ledger_json: Some(Ok(gap_ledger.to_string())),
            recommendation_calibration_json: None,
            agent_receipt_json: None,
            coverage_json: None,
            history_json: None,
        });
        let rendered = render_pr_evidence_ledger_json(&report)?;
        assert!(rendered.contains("\"status\": \"advisory\""));
        assert!(rendered.contains("\"gap_decision_ledger\": \"gap-ledger.json\""));
        assert!(rendered.contains("\"source\": \"gap_decision_ledger\""));
        assert!(rendered.contains("\"gap_id\": \"gap:pr:pricing:threshold-boundary\""));
        assert!(
            rendered
                .contains("\"canonical_gap_id\": \"gap:rust:pricing:discount:threshold-boundary\"")
        );
        assert!(rendered.contains("\"path\": \"src/pricing.rs\""));
        assert!(rendered.contains("\"line\": 42"));
        assert!(rendered.contains("\"suggested_test\": \"assert_eq!(discount(100, 100), 90)\""));
        assert!(rendered.contains("\"related_test\": \"tests/pricing.rs::discount_threshold\""));
        assert!(rendered.contains("\"verify_command\": \"cargo xtask fixtures boundary_gap\""));

        let markdown = render_pr_evidence_ledger_markdown(&report);
        assert!(markdown.contains("src/pricing.rs:42"));
        assert!(markdown.contains("Gap: gap:pr:pricing:threshold-boundary"));
        assert!(markdown.contains("Gap decision ledger: gap-ledger.json"));
        assert!(markdown.contains("Verify command: `cargo xtask fixtures boundary_gap`"));
        Ok(())
    }

    #[test]
    fn pr_evidence_ledger_reports_incomplete_without_evidence() -> Result<(), String> {
        let report = build_pr_evidence_ledger_report(PrEvidenceLedgerInput {
            root: ".".to_string(),
            generated_at: "unix_ms:1000".to_string(),
            pr_number: "123".to_string(),
            base: "base".to_string(),
            head: "head".to_string(),
            labels: Vec::new(),
            gate_path: None,
            baseline_delta_path: None,
            zero_status_path: None,
            pr_guidance_path: None,
            gap_ledger_path: None,
            recommendation_calibration_path: None,
            agent_receipt_path: None,
            coverage_path: None,
            history_path: None,
            gate_json: None,
            baseline_delta_json: None,
            zero_status_json: None,
            pr_guidance_json: None,
            gap_ledger_json: None,
            recommendation_calibration_json: None,
            agent_receipt_json: None,
            coverage_json: None,
            history_json: None,
        });
        let rendered = render_pr_evidence_ledger_json(&report)?;
        assert!(rendered.contains("\"status\": \"incomplete\""));
        assert!(rendered.contains("\"coverage_grip_frontier\""));
        Ok(())
    }

    #[test]
    fn pr_evidence_ledger_matches_fixture_contract() -> Result<(), String> {
        let repo_root = repo_root()?;
        let fixture = repo_root.join("fixtures/boundary_gap/expected/pr-evidence-ledger/mixed");
        let gate_path = fixture.join("gate-decision.json");
        let delta_path = fixture.join("baseline-debt-delta.json");
        let zero_path = fixture.join("ripr-zero-status.json");
        let guidance_path = fixture.join("comments.json");
        let receipt_path = fixture.join("agent-receipt.json");
        let history_path = fixture.join("history.jsonl");
        let expected_json_path = fixture.join("pr-evidence-ledger.json");
        let expected_md_path = fixture.join("pr-evidence-ledger.md");

        let report = build_pr_evidence_ledger_report(PrEvidenceLedgerInput {
            root: ".".to_string(),
            generated_at: "unix_ms:1000".to_string(),
            pr_number: "123".to_string(),
            base: "base".to_string(),
            head: "head".to_string(),
            labels: Vec::new(),
            gate_path: Some(fixture_path(&repo_root, &gate_path)),
            baseline_delta_path: Some(fixture_path(&repo_root, &delta_path)),
            zero_status_path: Some(fixture_path(&repo_root, &zero_path)),
            pr_guidance_path: Some(fixture_path(&repo_root, &guidance_path)),
            gap_ledger_path: None,
            recommendation_calibration_path: None,
            agent_receipt_path: Some(fixture_path(&repo_root, &receipt_path)),
            coverage_path: None,
            history_path: Some(fixture_path(&repo_root, &history_path)),
            gate_json: Some(Ok(read_file(&gate_path)?)),
            baseline_delta_json: Some(Ok(read_file(&delta_path)?)),
            zero_status_json: Some(Ok(read_file(&zero_path)?)),
            pr_guidance_json: Some(Ok(read_file(&guidance_path)?)),
            gap_ledger_json: None,
            recommendation_calibration_json: None,
            agent_receipt_json: Some(Ok(read_file(&receipt_path)?)),
            coverage_json: None,
            history_json: Some(Ok(read_file(&history_path)?)),
        });
        assert_eq!(
            render_pr_evidence_ledger_json(&report)?,
            read_file(&expected_json_path)?.trim_end()
        );
        assert_eq!(
            render_pr_evidence_ledger_markdown(&report),
            read_file(&expected_md_path)?
        );
        Ok(())
    }

    #[test]
    fn pr_evidence_ledger_warns_for_invalid_optional_inputs() -> Result<(), String> {
        let report = build_pr_evidence_ledger_report(PrEvidenceLedgerInput {
            root: ".".to_string(),
            generated_at: "unix_ms:1000".to_string(),
            pr_number: "123".to_string(),
            base: "base".to_string(),
            head: "head".to_string(),
            labels: Vec::new(),
            gate_path: Some("gate.json".to_string()),
            baseline_delta_path: Some("delta.json".to_string()),
            zero_status_path: Some("zero.json".to_string()),
            pr_guidance_path: Some("comments.json".to_string()),
            gap_ledger_path: None,
            recommendation_calibration_path: None,
            agent_receipt_path: None,
            coverage_path: None,
            history_path: Some("history.jsonl".to_string()),
            gate_json: None,
            baseline_delta_json: Some(Err("missing fixture".to_string())),
            zero_status_json: Some(Ok("{".to_string())),
            pr_guidance_json: Some(Ok(r#"{"comments":[]}"#.to_string())),
            gap_ledger_json: None,
            recommendation_calibration_json: None,
            agent_receipt_json: None,
            coverage_json: None,
            history_json: Some(Err("missing history".to_string())),
        });
        let rendered = render_pr_evidence_ledger_json(&report)?;
        assert!(rendered.contains("gate decision path gate.json was supplied"));
        assert!(rendered.contains("optional baseline debt delta input delta.json is invalid"));
        assert!(rendered.contains("optional RIPR Zero status input zero.json is invalid"));
        assert!(rendered.contains("optional history input history.jsonl is invalid"));
        Ok(())
    }

    #[test]
    fn pr_evidence_ledger_covers_pr_guidance_calibration_and_frontier() -> Result<(), String> {
        let guidance = r#"{
          "comments": [
            {
              "seam_id": "guidance-seam",
              "placement": {"path": "src/guidance.rs", "line": 9},
              "missing_discriminator": "guidance == true",
              "suggested_test": {"assertion_shape": "assert!(guidance())", "near_test": "tests/guidance.rs::existing"},
              "llm_guidance": {"verify_command": "ripr agent verify --json"}
            }
          ]
        }"#;
        let calibration = r#"{
          "recommendations": [
            {
              "seam_id": "guidance-seam",
              "static_movement": {"state": "improved", "source": "receipt"},
              "suggested_test": {"recommended_file": "tests/guidance.rs"}
            },
            {
              "seam_id": "unchanged-seam",
              "static_movement": {"state": "unchanged"},
              "suggested_test": {"near_test": "tests/unchanged.rs::case"}
            },
            {
              "seam_id": "regressed-seam",
              "static_movement": {"state": "regressed"}
            },
            {
              "seam_id": "ignored-seam",
              "static_movement": {"state": "unknown"}
            }
          ]
        }"#;
        let coverage = r#"{
          "coverage_delta_percent": "0.0",
          "ripr_visible_unresolved_delta": -3,
          "quadrants": {
            "covered_with_ripr_gap": 2,
            "covered_without_ripr_gap": 5,
            "uncovered_with_ripr_gap": 1,
            "uncovered_without_ripr_gap": 7
          }
        }"#;

        let report = build_pr_evidence_ledger_report(PrEvidenceLedgerInput {
            root: ".".to_string(),
            generated_at: "unix_ms:1000".to_string(),
            pr_number: "123".to_string(),
            base: "base".to_string(),
            head: "head".to_string(),
            labels: Vec::new(),
            gate_path: None,
            baseline_delta_path: None,
            zero_status_path: None,
            pr_guidance_path: Some("comments.json".to_string()),
            gap_ledger_path: None,
            recommendation_calibration_path: Some("recommendation-calibration.json".to_string()),
            agent_receipt_path: None,
            coverage_path: Some("coverage.json".to_string()),
            history_path: None,
            gate_json: None,
            baseline_delta_json: None,
            zero_status_json: None,
            pr_guidance_json: Some(Ok(guidance.to_string())),
            gap_ledger_json: None,
            recommendation_calibration_json: Some(Ok(calibration.to_string())),
            agent_receipt_json: None,
            coverage_json: Some(Ok(coverage.to_string())),
            history_json: None,
        });
        let rendered = render_pr_evidence_ledger_json(&report)?;
        assert!(rendered.contains("\"source\": \"pr_guidance\""));
        assert!(rendered.contains("\"coverage_delta_percent\": \"0.0\""));
        assert!(rendered.contains("behavioral grip improved without line-coverage movement"));
        assert!(rendered.contains("\"source\": \"recommendation_calibration\""));
        assert!(rendered.contains("\"state\": \"regressed\""));
        assert!(!rendered.contains("ignored-seam"));

        let markdown = render_pr_evidence_ledger_markdown(&report);
        assert!(markdown.contains("src/guidance.rs:9"));
        assert!(markdown.contains("Coverage delta: 0.0"));
        Ok(())
    }

    #[test]
    fn pr_evidence_ledger_covers_gate_and_baseline_route_fallbacks() -> Result<(), String> {
        let gate = r#"{
          "mode": "baseline-check",
          "status": "blocked",
          "summary": {"blocking": 1},
          "policy": {"acknowledgement_labels": ["ripr-waive"]},
          "decisions": [
            {
              "id": "gate-blocking",
              "decision": "blocking",
              "seam_id": "gate-seam",
              "evidence": {
                "missing_discriminator": "gate == true",
                "assertion_shape": "assert!(gate())",
                "recommended_test": "tests/gate.rs::case"
              }
            }
          ]
        }"#;
        let coverage = r#"{"coverage": {"delta_percent": 1.25}}"#;

        let gate_report = build_pr_evidence_ledger_report(PrEvidenceLedgerInput {
            root: ".".to_string(),
            generated_at: "unix_ms:1000".to_string(),
            pr_number: "123".to_string(),
            base: "base".to_string(),
            head: "head".to_string(),
            labels: Vec::new(),
            gate_path: Some("gate.json".to_string()),
            baseline_delta_path: None,
            zero_status_path: None,
            pr_guidance_path: None,
            gap_ledger_path: None,
            recommendation_calibration_path: None,
            agent_receipt_path: None,
            coverage_path: Some("coverage.json".to_string()),
            history_path: None,
            gate_json: Some(Ok(gate.to_string())),
            baseline_delta_json: None,
            zero_status_json: None,
            pr_guidance_json: None,
            gap_ledger_json: None,
            recommendation_calibration_json: None,
            agent_receipt_json: None,
            coverage_json: Some(Ok(coverage.to_string())),
            history_json: None,
        });
        let gate_markdown = render_pr_evidence_ledger_markdown(&gate_report);
        assert!(gate_markdown.contains("- Identity: gate-seam"));
        assert!(gate_markdown.contains("Coverage delta: 1.25"));
        let gate_json = render_pr_evidence_ledger_json(&gate_report)?;
        assert!(gate_json.contains("\"source\": \"gate_decision\""));

        let delta = r#"{
          "delta": {"still_present": 1, "resolved": 0, "new_policy_eligible": 0, "acknowledged": 0, "suppressed": 1},
          "items": [
            {
              "bucket": "suppressed",
              "identity": {"id": "suppressed-id", "seam_id": "suppressed-seam"},
              "reason": "fixture suppression"
            },
            {
              "bucket": "still_present",
              "identity": {"seam_id": "baseline-seam"},
              "path": "src/baseline.rs",
              "missing_discriminator": "baseline == true",
              "suggested_test": {"assertion_shape": "assert!(baseline())"},
              "repair": {"verify_command": "ripr agent verify --json"}
            }
          ]
        }"#;
        let unsupported_coverage = "{}";
        let baseline_report = build_pr_evidence_ledger_report(PrEvidenceLedgerInput {
            root: ".".to_string(),
            generated_at: "unix_ms:1000".to_string(),
            pr_number: "123".to_string(),
            base: "base".to_string(),
            head: "head".to_string(),
            labels: Vec::new(),
            gate_path: None,
            baseline_delta_path: Some("delta.json".to_string()),
            zero_status_path: None,
            pr_guidance_path: None,
            gap_ledger_path: None,
            recommendation_calibration_path: None,
            agent_receipt_path: None,
            coverage_path: Some("coverage.json".to_string()),
            history_path: None,
            gate_json: None,
            baseline_delta_json: Some(Ok(delta.to_string())),
            zero_status_json: None,
            pr_guidance_json: None,
            gap_ledger_json: None,
            recommendation_calibration_json: None,
            agent_receipt_json: None,
            coverage_json: Some(Ok(unsupported_coverage.to_string())),
            history_json: None,
        });
        let baseline_json = render_pr_evidence_ledger_json(&baseline_report)?;
        assert!(baseline_json.contains("\"source\": \"baseline_debt_delta\""));
        assert!(baseline_json.contains("fixture suppression"));
        assert!(baseline_json.contains("has no supported coverage/grip frontier fields"));
        Ok(())
    }

    #[test]
    fn pr_evidence_ledger_reports_history_trends() -> Result<(), String> {
        let regressing = report_with_history(
            r#"{"movement":{"baseline_resolved":0,"new_policy_eligible":2},"waivers":[]}"#,
        );
        let stable = report_with_history(
            r#"{"movement":{"baseline_resolved":0,"new_policy_eligible":0},"waivers":[]}"#,
        );
        let unknown = report_with_history("");
        let invalid = report_with_history("{");

        assert!(render_pr_evidence_ledger_json(&regressing)?.contains("\"trend\": \"regressing\""));
        assert!(render_pr_evidence_ledger_json(&stable)?.contains("\"trend\": \"stable\""));
        assert!(render_pr_evidence_ledger_json(&unknown)?.contains("\"trend\": \"unknown\""));
        assert!(
            render_pr_evidence_ledger_json(&invalid)?.contains("contains an invalid JSONL record")
        );
        Ok(())
    }

    fn report_with_history(history: &str) -> super::PrEvidenceLedgerReport {
        build_pr_evidence_ledger_report(PrEvidenceLedgerInput {
            root: ".".to_string(),
            generated_at: "unix_ms:1000".to_string(),
            pr_number: "123".to_string(),
            base: "base".to_string(),
            head: "head".to_string(),
            labels: Vec::new(),
            gate_path: None,
            baseline_delta_path: None,
            zero_status_path: None,
            pr_guidance_path: Some("comments.json".to_string()),
            gap_ledger_path: None,
            recommendation_calibration_path: None,
            agent_receipt_path: None,
            coverage_path: None,
            history_path: Some("history.jsonl".to_string()),
            gate_json: None,
            baseline_delta_json: None,
            zero_status_json: None,
            pr_guidance_json: Some(Ok(r#"{"comments":[]}"#.to_string())),
            gap_ledger_json: None,
            recommendation_calibration_json: None,
            agent_receipt_json: None,
            coverage_json: None,
            history_json: Some(Ok(history.to_string())),
        })
    }

    fn repo_root() -> Result<PathBuf, String> {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .ok_or_else(|| "CARGO_MANIFEST_DIR did not have a workspace parent".to_string())
    }

    fn read_file(path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path)
            .map_err(|err| format!("read {} failed: {err}", path.display()))
    }

    fn fixture_path(repo_root: &Path, path: &Path) -> String {
        match path.strip_prefix(repo_root) {
            Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
            Err(_) => path.to_string_lossy().replace('\\', "/"),
        }
    }
}
