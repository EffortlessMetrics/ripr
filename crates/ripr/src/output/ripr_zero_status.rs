use super::first_pr::{ProofPathLabels, REPAIR_AFTER_PHASE_LABEL, REPAIR_AFTER_PHASE_STEP};
use super::gap_decision_ledger::{self, GapRecord};
use super::gate::{
    GATE_STATUS_CONFIG_ERROR, blocked_producer_warnings, discloses_blocked_producer_outcome,
    discloses_incomplete_analysis_outcome, discloses_limited_findings_bound,
    discloses_limited_partial_scope, incomplete_analysis_outcome_kind,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "ripr_zero_status";
const LIMITS_NOTE: &str = "Read-only advisory RIPR Zero status over existing static RIPR artifacts; gate-decision remains the pass/fail authority.";
const RIPR_ZERO_LIMITS_NOTE: &str = "RIPR 0 means no visible unresolved behavioral test-grip gaps under configured scope and policy; it is not a coverage or runtime adequacy claim.";
pub(crate) const DEFAULT_RIPR_ZERO_STATUS_OUT: &str = "target/ripr/reports/ripr-zero-status.json";
pub(crate) const DEFAULT_RIPR_ZERO_STATUS_MD_OUT: &str = "target/ripr/reports/ripr-zero-status.md";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiprZeroStatusInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) baseline_path: Option<String>,
    pub(crate) delta_path: String,
    pub(crate) gap_ledger_path: Option<String>,
    pub(crate) gate_path: Option<String>,
    pub(crate) pr_guidance_path: Option<String>,
    pub(crate) recommendation_calibration_path: Option<String>,
    pub(crate) baseline_json: Option<Result<String, String>>,
    pub(crate) delta_json: Result<String, String>,
    pub(crate) gap_ledger_json: Option<Result<String, String>>,
    pub(crate) gate_json: Option<Result<String, String>>,
    pub(crate) pr_guidance_json: Option<Result<String, String>>,
    pub(crate) recommendation_calibration_json: Option<Result<String, String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiprZeroStatusReport {
    root: String,
    generated_at: String,
    status: String,
    inputs: RiprZeroInputs,
    ripr_zero: RiprZeroSummary,
    baseline: BaselineSummary,
    debt_delta: DebtDeltaSummary,
    trend: TrendSummary,
    top_debt_areas: Vec<TopDebtArea>,
    repair_routes: Vec<RepairRoute>,
    warnings: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiprZeroInputs {
    baseline: Option<String>,
    baseline_debt_delta: String,
    gap_decision_ledger: Option<String>,
    gate_decision: Option<String>,
    pr_guidance: Option<String>,
    recommendation_calibration: Option<String>,
    previous_status: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiprZeroSummary {
    state: String,
    target_source: String,
    visible_unresolved: usize,
    new_policy_eligible: usize,
    blocking_candidates: usize,
    acknowledged: usize,
    suppressed: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BaselineSummary {
    path: Option<String>,
    entries: usize,
    still_present: usize,
    resolved: usize,
    age_days: Option<i64>,
    metadata: MetadataCounts,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct MetadataCounts {
    current: usize,
    stale: usize,
    missing_metadata: usize,
    unknown: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DebtDeltaSummary {
    still_present: usize,
    resolved: usize,
    new: usize,
    new_policy_eligible: usize,
    acknowledged: usize,
    suppressed: usize,
    stale: usize,
    invalid: usize,
    missing_input: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TrendSummary {
    source: String,
    window: Option<String>,
    visible_unresolved_delta: Option<i64>,
    resolved_delta: Option<i64>,
    new_policy_eligible_delta: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TopDebtArea {
    rank: usize,
    area: String,
    visible_unresolved: usize,
    new_policy_eligible: usize,
    stale_baseline_entries: usize,
    top_static_class: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RepairRoute {
    rank: usize,
    source: String,
    gap_id: Option<String>,
    canonical_gap_id: Option<String>,
    seam_id: Option<String>,
    path: Option<String>,
    line: Option<u64>,
    static_class: Option<String>,
    missing_discriminator: Option<String>,
    suggested_test: Option<String>,
    related_test: Option<String>,
    verify_command: Option<String>,
    /// The repair transaction's start (#3906), carried from the delta item's
    /// `evidence_record.canonical_item.repair_command`. That record names it
    /// only past the fail-closed repair-packet flip; zero status never builds
    /// one from a seam id.
    repair_command: Option<String>,
    /// The carried repair start, or `None`. Never a synthesized `agent start`.
    agent_command: Option<String>,
    static_limitations: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DeltaParse {
    status: ParseStatus,
    baseline_path: Option<String>,
    baseline_entries: usize,
    counts: DebtDeltaSummary,
    items: Vec<DeltaItem>,
    warnings: Vec<String>,
    partial_denominator: bool,
    counts_items_mismatch: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct GapLedgerParse {
    status: ParseStatus,
    supplied: bool,
    ripr_zero_targets: usize,
    repair_routes: Vec<RepairRoute>,
    warnings: Vec<String>,
    partial_denominator: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ParseStatus {
    #[default]
    Loaded,
    Missing,
    Invalid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DeltaItem {
    bucket: String,
    identity: Identity,
    path: Option<String>,
    line: Option<u64>,
    static_class: Option<String>,
    missing_discriminator: Option<String>,
    suggested_test: SuggestedTest,
    repair: Repair,
    evidence_record: Option<EvidenceRecordRepairContext>,
    review: Option<ReviewMetadata>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct EvidenceRecordRepairContext {
    seam_id: Option<String>,
    path: Option<String>,
    line: Option<u64>,
    static_class: Option<String>,
    missing_discriminator: Option<String>,
    suggested_test: Option<String>,
    related_test: Option<String>,
    verify_command: Option<String>,
    repair_command: Option<String>,
    static_limitations: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Identity {
    seam_id: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SuggestedTest {
    recommended_test: Option<String>,
    assertion_shape: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Repair {
    verify_command: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ReviewMetadata {
    invalid: bool,
    owner: Option<String>,
    reason: Option<String>,
    created_at: Option<String>,
    review_after: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct GateParse {
    blocking_candidates: usize,
    failed: bool,
    warnings: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct BaselineParse {
    entries: usize,
    metadata: MetadataCounts,
    created_at: Option<String>,
    warnings: Vec<String>,
    supplied: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct AreaAccumulator {
    visible_unresolved: usize,
    new_policy_eligible: usize,
    stale_baseline_entries: usize,
    class_counts: BTreeMap<String, usize>,
}

pub(crate) fn build_ripr_zero_status_report(input: RiprZeroStatusInput) -> RiprZeroStatusReport {
    let delta = parse_delta(&input.delta_path, input.delta_json);
    let gap_ledger = parse_gap_ledger(input.gap_ledger_path.as_deref(), input.gap_ledger_json);
    let gate = parse_gate(input.gate_path.as_deref(), input.gate_json);
    let baseline = parse_baseline(
        input.baseline_path.as_deref(),
        input.baseline_json,
        &delta,
        &input.generated_at,
    );
    let mut warnings = Vec::new();
    warnings.extend(delta.warnings.clone());
    warnings.extend(gap_ledger.warnings.clone());
    warnings.extend(gate.warnings.clone());
    warnings.extend(baseline.warnings.clone());
    warnings.extend(optional_input_warnings(
        "pr_guidance",
        input.pr_guidance_path.as_deref(),
        input.pr_guidance_json,
    ));
    warnings.extend(optional_input_warnings(
        "recommendation_calibration",
        input.recommendation_calibration_path.as_deref(),
        input.recommendation_calibration_json,
    ));
    warnings.push(
        "Trend evidence is not available; previous status or ledger input was not supplied."
            .to_string(),
    );

    // A partial or contradictory denominator makes the report incomplete,
    // even when every input parsed (#5251 Z2/Z3/Z4).
    let partial_denominator = delta.partial_denominator || gap_ledger.partial_denominator;
    let status = if delta.status == ParseStatus::Loaded
        && !partial_denominator
        && !delta.counts_items_mismatch
        && !gate.failed
    {
        "advisory"
    } else {
        "incomplete"
    }
    .to_string();
    let delta_visible_unresolved =
        delta.counts.still_present + delta.counts.new_policy_eligible + delta.counts.acknowledged;
    // A partial-denominator ledger (blocked, partial-scope, findings-bounded,
    // or incomplete-outcome producer run) is disclosed but never selected as
    // the target denominator: its zero would erase visible delta debt. The
    // delta keeps the debt signal while the report stays incomplete (#6095
    // review).
    let target_from_gap_ledger = gap_ledger.supplied
        && gap_ledger.status == ParseStatus::Loaded
        && !gap_ledger.partial_denominator;
    let visible_unresolved = if target_from_gap_ledger {
        gap_ledger.ripr_zero_targets
    } else {
        delta_visible_unresolved
    };
    let target_source = if target_from_gap_ledger {
        "gap_decision_ledger"
    } else {
        "baseline_debt_delta"
    };
    // Contradictory inputs can never yield a verdict, and a partial
    // denominator or a failed gate evaluation can never yield bare achieved
    // — but visible debt keeps its not_yet signal, disclosed by the parse
    // warnings (#5251 Z2/Z3/Z4/Z5).
    let all_clear = visible_unresolved == 0
        && delta.counts.stale == 0
        && delta.counts.invalid == 0
        && delta.counts.missing_input == 0;
    let state = if delta.status != ParseStatus::Loaded
        || delta.counts_items_mismatch
        || (all_clear && partial_denominator)
        || (all_clear && gate.failed)
    {
        "unknown"
    } else if all_clear {
        "achieved"
    } else {
        "not_yet"
    }
    .to_string();
    let ripr_zero = RiprZeroSummary {
        state,
        target_source: target_source.to_string(),
        visible_unresolved,
        new_policy_eligible: delta.counts.new_policy_eligible,
        blocking_candidates: gate.blocking_candidates,
        acknowledged: delta.counts.acknowledged,
        suppressed: delta.counts.suppressed,
    };
    let baseline_summary = BaselineSummary {
        path: baseline_path_for_summary(
            input.baseline_path.as_deref(),
            delta.baseline_path.as_deref(),
        ),
        entries: baseline.entries,
        still_present: delta.counts.still_present,
        resolved: delta.counts.resolved,
        age_days: baseline
            .created_at
            .as_deref()
            .and_then(|created_at| age_days(created_at, &input.generated_at)),
        metadata: baseline.metadata,
    };
    RiprZeroStatusReport {
        root: input.root,
        generated_at: input.generated_at,
        status,
        inputs: RiprZeroInputs {
            baseline: input.baseline_path,
            baseline_debt_delta: input.delta_path,
            gap_decision_ledger: input.gap_ledger_path,
            gate_decision: input.gate_path,
            pr_guidance: input.pr_guidance_path,
            recommendation_calibration: input.recommendation_calibration_path,
            previous_status: None,
        },
        ripr_zero,
        baseline: baseline_summary,
        debt_delta: delta.counts,
        trend: TrendSummary {
            source: "not_available".to_string(),
            window: None,
            visible_unresolved_delta: None,
            resolved_delta: None,
            new_policy_eligible_delta: None,
        },
        top_debt_areas: top_debt_areas(&delta.items),
        repair_routes: if target_from_gap_ledger && !gap_ledger.repair_routes.is_empty() {
            gap_ledger.repair_routes
        } else {
            repair_routes(&delta.items)
        },
        warnings,
    }
}

pub(crate) fn render_ripr_zero_status_json(
    report: &RiprZeroStatusReport,
) -> Result<String, String> {
    serde_json::to_string_pretty(&json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "kind": REPORT_KIND,
        "status": report.status,
        "root": report.root,
        "generated_at": report.generated_at,
        "inputs": inputs_json(&report.inputs),
        "ripr_zero": ripr_zero_json(&report.ripr_zero),
        "baseline": baseline_json(&report.baseline),
        "debt_delta": debt_delta_json(&report.debt_delta),
        "trend": trend_json(&report.trend),
        "top_debt_areas": report.top_debt_areas.iter().map(top_debt_area_json).collect::<Vec<_>>(),
        "repair_routes": report.repair_routes.iter().map(repair_route_json).collect::<Vec<_>>(),
        "warnings": report.warnings,
        "limits_note": LIMITS_NOTE,
    }))
    .map_err(|err| format!("failed to render RIPR Zero status JSON: {err}"))
}

pub(crate) fn render_ripr_zero_status_markdown(report: &RiprZeroStatusReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Zero Status\n\n");
    out.push_str(&format!("Status: {}\n", report.status));
    out.push_str(&format!("RIPR 0: {}\n\n", report.ripr_zero.state));
    out.push_str(&format!(
        "Target source: `{}`\n\n",
        report.ripr_zero.target_source
    ));
    out.push_str("| Measure | Count |\n");
    out.push_str("| --- | ---: |\n");
    out.push_str(&format!(
        "| Visible unresolved gaps | {} |\n",
        report.ripr_zero.visible_unresolved
    ));
    out.push_str(&format!(
        "| Existing baseline gaps still present | {} |\n",
        report.baseline.still_present
    ));
    out.push_str(&format!(
        "| Baseline gaps resolved | {} |\n",
        report.baseline.resolved
    ));
    out.push_str(&format!(
        "| New policy-eligible gaps | {} |\n",
        report.debt_delta.new_policy_eligible
    ));
    out.push_str(&format!(
        "| Acknowledged gaps | {} |\n",
        report.debt_delta.acknowledged
    ));
    out.push_str(&format!(
        "| Suppressed gaps | {} |\n",
        report.debt_delta.suppressed
    ));
    out.push_str(&format!(
        "| Stale baseline entries | {} |\n",
        report.baseline.metadata.stale
    ));
    out.push_str(&format!(
        "| Missing metadata entries | {} |\n",
        report.baseline.metadata.missing_metadata
    ));

    if let Some(route) = report.repair_routes.first() {
        out.push_str("\nTop repair route:\n");
        out.push_str(&format!("- {}\n", route_headline(route)));
        if let Some(missing) = route.missing_discriminator.as_deref() {
            out.push_str(&format!("  Missing: {missing}\n"));
        }
        if let Some(suggested) = route.suggested_test.as_deref() {
            out.push_str(&format!("  Suggested test: {suggested}\n"));
        }
        // #3906: a carried repair start leads; its after phase runs verify,
        // so the verify command is the manual alternative. Without one it
        // runs after the test edit.
        if let Some(repair) = route.repair_command.as_deref() {
            out.push_str(&format!("  Repair start: {repair}\n"));
            out.push_str(&format!(
                "  {REPAIR_AFTER_PHASE_LABEL}: {REPAIR_AFTER_PHASE_STEP}\n"
            ));
        }
        if let Some(verify) = route.verify_command.as_deref() {
            let labels = ProofPathLabels::for_repair_start(route.repair_command.is_some());
            out.push_str(&format!("  {}: {verify}\n", labels.verify));
        }
        if let Some(limit) = route.static_limitations.first() {
            out.push_str(&format!("  Static limit: {limit}\n"));
        }
    }

    if !report.top_debt_areas.is_empty() {
        out.push_str("\nTop debt areas:\n");
        for area in report.top_debt_areas.iter().take(5) {
            out.push_str(&format!(
                "- {}: visible_unresolved={}, new_policy_eligible={}, stale={}\n",
                area.area,
                area.visible_unresolved,
                area.new_policy_eligible,
                area.stale_baseline_entries
            ));
        }
    }

    if !report.warnings.is_empty() {
        out.push_str("\nWarnings:\n");
        for warning in &report.warnings {
            out.push_str(&format!("- {warning}\n"));
        }
    }

    out.push_str("\nLimits:\n");
    out.push_str(RIPR_ZERO_LIMITS_NOTE);
    out.push('\n');
    out.push_str(LIMITS_NOTE);
    out.push('\n');
    out
}

fn parse_delta(path: &str, text: Result<String, String>) -> DeltaParse {
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            return DeltaParse {
                status: ParseStatus::Missing,
                warnings: vec![format!(
                    "required baseline debt delta input {path} is invalid: {error}"
                )],
                ..DeltaParse::default()
            };
        }
    };
    let value = match serde_json::from_str::<Value>(&text) {
        Ok(value) => value,
        Err(error) => {
            return DeltaParse {
                status: ParseStatus::Invalid,
                warnings: vec![format!(
                    "required baseline debt delta input {path} is invalid: {error}"
                )],
                ..DeltaParse::default()
            };
        }
    };
    if value.get("schema_version").and_then(Value::as_str) != Some(SCHEMA_VERSION) {
        return DeltaParse {
            status: ParseStatus::Invalid,
            warnings: vec![format!(
                "required baseline debt delta input {path} has unsupported schema_version; expected {SCHEMA_VERSION}"
            )],
            ..DeltaParse::default()
        };
    }
    if value.get("kind").and_then(Value::as_str) != Some("baseline_debt_delta") {
        return DeltaParse {
            status: ParseStatus::Invalid,
            warnings: vec![format!(
                "required baseline debt delta input {path} has unsupported kind; expected baseline_debt_delta"
            )],
            ..DeltaParse::default()
        };
    }
    // Fail closed on content-free input (#5251 Z1): a delta document without
    // a delta section, or with a partial one, is not evidence of zero debt.
    // Real `ripr baseline diff` output always carries all eight counts.
    let Some(delta_section) = value.get("delta").and_then(Value::as_object) else {
        return DeltaParse {
            status: ParseStatus::Invalid,
            warnings: vec![format!(
                "required baseline debt delta input {path} has no delta section; an empty input is not evidence of zero debt"
            )],
            ..DeltaParse::default()
        };
    };
    let mut missing_counts = Vec::new();
    for key in [
        "still_present",
        "resolved",
        "new_policy_eligible",
        "acknowledged",
        "suppressed",
        "stale_baseline_entry",
        "invalid_baseline_entry",
        "missing_current_input",
    ] {
        if delta_section.get(key).is_none() {
            missing_counts.push(key);
        }
    }
    if !missing_counts.is_empty() {
        return DeltaParse {
            status: ParseStatus::Invalid,
            warnings: vec![format!(
                "required baseline debt delta input {path} has a partial delta section (missing counts: {}); partial counts are not evidence of zero debt",
                missing_counts.join(", ")
            )],
            ..DeltaParse::default()
        };
    }
    // Present-but-malformed counts fail closed too (#6095 review): a null,
    // string, or negative count must never read as zero debt.
    let mut malformed_counts = Vec::new();
    for key in [
        "still_present",
        "resolved",
        "new_policy_eligible",
        "acknowledged",
        "suppressed",
        "stale_baseline_entry",
        "invalid_baseline_entry",
        "missing_current_input",
    ] {
        if validated_count(delta_section, key).is_none() {
            malformed_counts.push(key);
        }
    }
    if !malformed_counts.is_empty() {
        return DeltaParse {
            status: ParseStatus::Invalid,
            warnings: vec![format!(
                "required baseline debt delta input {path} has malformed counts ({}); counts must be valid nonnegative integers, and malformed counts are not evidence of zero debt",
                malformed_counts.join(", ")
            )],
            ..DeltaParse::default()
        };
    }
    // Every count above validated as a real usize, so these reads are exact.
    let still_present = usize_path(&value, &["delta", "still_present"]);
    let resolved = usize_path(&value, &["delta", "resolved"]);
    let new_policy_eligible = usize_path(&value, &["delta", "new_policy_eligible"]);
    let acknowledged = usize_path(&value, &["delta", "acknowledged"]);
    let suppressed = usize_path(&value, &["delta", "suppressed"]);
    let stale = usize_path(&value, &["delta", "stale_baseline_entry"]);
    let invalid = usize_path(&value, &["delta", "invalid_baseline_entry"]);
    let missing_input = usize_path(&value, &["delta", "missing_current_input"]);
    // Aggregates must not overflow: a wrapped sum would fabricate zero
    // visible debt and `achieved` (or panic in checked builds), so
    // overflowing counts fail closed as Invalid (#6095 review).
    let new_sum = new_policy_eligible
        .checked_add(acknowledged)
        .and_then(|sum| sum.checked_add(suppressed));
    let visible_sum = still_present
        .checked_add(new_policy_eligible)
        .and_then(|sum| sum.checked_add(acknowledged));
    if new_sum.is_none() || visible_sum.is_none() {
        return DeltaParse {
            status: ParseStatus::Invalid,
            warnings: vec![format!(
                "required baseline debt delta input {path} has counts whose aggregates overflow; overflowing counts are not evidence of zero debt"
            )],
            ..DeltaParse::default()
        };
    }
    let counts = DebtDeltaSummary {
        still_present,
        resolved,
        // Validated non-overflowing above; parse_delta is the only
        // constructor of nonzero counts, so downstream sums are safe.
        new: new_sum.unwrap_or(0),
        new_policy_eligible,
        acknowledged,
        suppressed,
        stale,
        invalid,
        missing_input,
    };
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .map(delta_item_from_value)
                .collect::<Vec<DeltaItem>>()
        })
        .unwrap_or_default();
    // A partial-scope, findings-bounded, or otherwise incomplete producer
    // denominator is disclosed, never silently counted (#5251 Z2). The
    // predicates are the shared gate fail-closed vocabulary.
    let mut warnings = warnings_from_value(&value);
    let mut partial_denominator = false;
    if discloses_limited_partial_scope(&value) {
        partial_denominator = true;
        warnings.push(format!(
            "required baseline debt delta input {path} discloses a limited_partial_scope producer run; a partial denominator can never yield achieved"
        ));
    }
    if discloses_limited_findings_bound(&value) {
        partial_denominator = true;
        warnings.push(format!(
            "required baseline debt delta input {path} discloses a findings-bounded producer run; a bounded denominator can never yield achieved"
        ));
    }
    if discloses_incomplete_analysis_outcome(&value) {
        partial_denominator = true;
        warnings.push(format!(
            "required baseline debt delta input {path} discloses an incomplete analysis outcome ({}); an incomplete denominator can never yield achieved",
            incomplete_analysis_outcome_kind(&value)
        ));
    }
    // Counts and items must reconcile (#5251 Z3): the check below
    // requires exact bucket cardinalities, since the producer aggregates
    // counts from the emitted items.
    let clear_counts_zero = counts.still_present == 0
        && counts.new_policy_eligible == 0
        && counts.acknowledged == 0
        && counts.stale == 0
        && counts.invalid == 0
        && counts.missing_input == 0;
    // The producer aggregates every bucket count from the emitted items, so
    // in a faithful document each bucket cardinality equals its count
    // exactly. Any deviation is a self-contradictory document, which cannot
    // yield a verdict (#5251 Z3, #6095 review). Buckets outside the
    // producer's eight carry no count to contradict, so they block
    // `achieved` only when the counts would otherwise clear it: against
    // visible debt the document is already `not_yet`. Historical `resolved`
    // and accepted `suppressed` counts never mask them.
    let mut bucket_items: BTreeMap<&str, usize> = BTreeMap::new();
    let mut unknown_bucket_items = 0usize;
    for item in &items {
        match item.bucket.as_str() {
            "still_present"
            | "resolved"
            | "new_policy_eligible"
            | "acknowledged"
            | "suppressed"
            | "stale_baseline_entry"
            | "invalid_baseline_entry"
            | "missing_current_input" => {
                *bucket_items.entry(item.bucket.as_str()).or_insert(0) += 1;
            }
            _ => {
                unknown_bucket_items += 1;
            }
        }
    }
    let bucket_counts = [
        ("still_present", counts.still_present),
        ("resolved", counts.resolved),
        ("new_policy_eligible", counts.new_policy_eligible),
        ("acknowledged", counts.acknowledged),
        ("suppressed", counts.suppressed),
        ("stale_baseline_entry", counts.stale),
        ("invalid_baseline_entry", counts.invalid),
        ("missing_current_input", counts.missing_input),
    ];
    let mut counts_items_mismatch = false;
    for (bucket, count) in bucket_counts {
        let carried = bucket_items.get(bucket).copied().unwrap_or(0);
        if carried != count {
            counts_items_mismatch = true;
            warnings.push(format!(
                "required baseline debt delta input {path} bucket {bucket} reports count {count} but carries {carried} item(s); contradictory items and counts cannot yield a verdict"
            ));
        }
    }
    if unknown_bucket_items > 0 && clear_counts_zero {
        counts_items_mismatch = true;
        warnings.push(format!(
            "required baseline debt delta input {path} carries {unknown_bucket_items} item(s) outside the producer buckets against zero counts; contradictory items and counts cannot yield a verdict"
        ));
    }
    DeltaParse {
        status: ParseStatus::Loaded,
        baseline_path: string_path(&value, &["baseline", "path"])
            .or_else(|| string_path(&value, &["inputs", "baseline"])),
        baseline_entries: usize_path(&value, &["baseline", "entries"]),
        counts,
        items,
        warnings,
        partial_denominator,
        counts_items_mismatch,
    }
}

fn parse_gap_ledger(path: Option<&str>, text: Option<Result<String, String>>) -> GapLedgerParse {
    let Some((path, text)) = path.zip(text) else {
        return GapLedgerParse::default();
    };
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            return GapLedgerParse {
                status: ParseStatus::Missing,
                supplied: true,
                warnings: vec![format!(
                    "optional gap decision ledger input {path} is invalid: {error}"
                )],
                ..GapLedgerParse::default()
            };
        }
    };
    let records = match gap_decision_ledger::parse_gap_records_json(&text) {
        Ok(records) => records,
        Err(error) => {
            return GapLedgerParse {
                status: ParseStatus::Invalid,
                supplied: true,
                warnings: vec![format!(
                    "optional gap decision ledger input {path} is invalid: {error}"
                )],
                ..GapLedgerParse::default()
            };
        }
    };
    let ripr_zero_targets = records
        .iter()
        .filter(|record| gap_decision_ledger::projection_eligible(record, "ripr_zero_count"))
        .count();
    let repair_routes = gap_repair_routes(&records);
    // A partial-scope ledger is disclosed, never silently counted (#5251
    // Z4): its targets are a partial denominator, like the gate input rule.
    let mut warnings = Vec::new();
    let mut partial_denominator = false;
    if let Ok(ledger_value) = serde_json::from_str::<Value>(&text) {
        if discloses_limited_partial_scope(&ledger_value) {
            partial_denominator = true;
            warnings.push(format!(
                "optional gap decision ledger input {path} discloses a limited_partial_scope producer run; a partial denominator can never yield achieved"
            ));
        }
        if discloses_limited_findings_bound(&ledger_value) {
            partial_denominator = true;
            warnings.push(format!(
                "optional gap decision ledger input {path} discloses a findings-bounded producer run; a bounded denominator can never yield achieved"
            ));
        }
        if discloses_incomplete_analysis_outcome(&ledger_value) {
            partial_denominator = true;
            warnings.push(format!(
                "optional gap decision ledger input {path} discloses an incomplete analysis outcome ({}); an incomplete denominator can never yield achieved",
                incomplete_analysis_outcome_kind(&ledger_value)
            ));
        }
        if discloses_blocked_producer_outcome(&ledger_value) {
            partial_denominator = true;
            warnings.push(format!(
                "optional gap decision ledger input {path} discloses a blocked producer run; a blocked denominator can never yield achieved"
            ));
            for producer_warning in blocked_producer_warnings(&ledger_value) {
                warnings.push(format!(
                    "optional gap decision ledger input {path} producer warning: {producer_warning}"
                ));
            }
        }
    }
    GapLedgerParse {
        status: ParseStatus::Loaded,
        supplied: true,
        ripr_zero_targets,
        repair_routes,
        warnings,
        partial_denominator,
    }
}

fn parse_gate(path: Option<&str>, text: Option<Result<String, String>>) -> GateParse {
    let Some((path, text)) = path.zip(text) else {
        return GateParse {
            warnings: vec![
                "gate decision input not supplied; blocking candidates are reported as 0."
                    .to_string(),
            ],
            ..GateParse::default()
        };
    };
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            return GateParse {
                warnings: vec![format!(
                    "optional gate decision input {path} is invalid: {error}"
                )],
                ..GateParse::default()
            };
        }
    };
    let value = match serde_json::from_str::<Value>(&text) {
        Ok(value) => value,
        Err(error) => {
            return GateParse {
                warnings: vec![format!(
                    "optional gate decision input {path} is invalid: {error}"
                )],
                ..GateParse::default()
            };
        }
    };
    // A failed gate evaluation is surfaced, never silently zeroed (#5251
    // Z5): its blocking count is meaningless because evaluation did not
    // complete, so it is forced to 0 and the failure feeds the verdict.
    let mut gate_warnings = warnings_from_value(&value);
    let failed = string_field(value.get("status")).as_deref() == Some(GATE_STATUS_CONFIG_ERROR);
    if failed {
        let config_error_count = value
            .get("config_errors")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        gate_warnings.push(format!(
            "optional gate decision input {path} reports status config_error ({config_error_count} config errors); evaluation did not complete, so blocking candidates are reported as 0"
        ));
    }
    GateParse {
        blocking_candidates: if failed {
            0
        } else {
            usize_path(&value, &["summary", "blocking"])
        },
        failed,
        warnings: gate_warnings,
    }
}

fn parse_baseline(
    path: Option<&str>,
    text: Option<Result<String, String>>,
    delta: &DeltaParse,
    generated_at: &str,
) -> BaselineParse {
    let Some((path, text)) = path.zip(text) else {
        return metadata_from_delta(delta, generated_at);
    };
    let text = match text {
        Ok(text) => text,
        Err(error) => {
            let mut parse = metadata_from_delta(delta, generated_at);
            parse.warnings.push(format!(
                "optional baseline input {path} is invalid: {error}"
            ));
            return parse;
        }
    };
    let value = match serde_json::from_str::<Value>(&text) {
        Ok(value) => value,
        Err(error) => {
            let mut parse = metadata_from_delta(delta, generated_at);
            parse.warnings.push(format!(
                "optional baseline input {path} is invalid: {error}"
            ));
            return parse;
        }
    };
    let Some(entries) = value.get("entries").and_then(Value::as_array) else {
        let mut parse = metadata_from_delta(delta, generated_at);
        parse.warnings.push(format!(
            "optional baseline input {path} is missing entries array"
        ));
        return parse;
    };
    let mut metadata = MetadataCounts::default();
    for entry in entries {
        count_metadata(
            &mut metadata,
            classify_review(
                review_metadata_from_value(entry.get("review")),
                generated_at,
            ),
        );
    }
    warn_for_metadata(metadata.clone(), &mut Vec::new());
    let mut warnings = warnings_from_value(&value);
    warnings.extend(metadata_warnings(&metadata));
    BaselineParse {
        entries: entries.len(),
        metadata,
        created_at: string_field(value.get("created_at")),
        warnings,
        supplied: true,
    }
}

fn metadata_from_delta(delta: &DeltaParse, generated_at: &str) -> BaselineParse {
    let mut metadata = MetadataCounts::default();
    let mut baseline_items = 0usize;
    for item in &delta.items {
        if is_baseline_derived_bucket(&item.bucket) {
            baseline_items += 1;
            count_metadata(
                &mut metadata,
                classify_review(item.review.clone(), generated_at),
            );
        }
    }
    let entries = if delta.baseline_entries == 0 {
        baseline_items
    } else {
        delta.baseline_entries
    };
    if entries > baseline_items {
        metadata.missing_metadata += entries - baseline_items;
    }
    let mut warnings = vec![
        "baseline input not supplied; metadata health is derived from baseline debt delta items."
            .to_string(),
    ];
    warnings.extend(metadata_warnings(&metadata));
    BaselineParse {
        entries,
        metadata,
        created_at: None,
        warnings,
        supplied: false,
    }
}

fn optional_input_warnings(
    label: &str,
    path: Option<&str>,
    text: Option<Result<String, String>>,
) -> Vec<String> {
    match (path, text) {
        (Some(path), Some(Ok(text))) => match serde_json::from_str::<Value>(&text) {
            Ok(_) => Vec::new(),
            Err(error) => vec![format!("optional {label} input {path} is invalid: {error}")],
        },
        (Some(path), Some(Err(error))) => {
            vec![format!("optional {label} input {path} is invalid: {error}")]
        }
        _ => vec![format!("optional {label} input not supplied.")],
    }
}

fn delta_item_from_value(value: &Value) -> DeltaItem {
    DeltaItem {
        bucket: string_field(value.get("bucket")).unwrap_or_else(|| "unknown".to_string()),
        identity: Identity {
            seam_id: string_path(value, &["identity", "seam_id"]),
        },
        path: string_field(value.get("path")),
        line: value.get("line").and_then(Value::as_u64),
        static_class: string_field(value.get("static_class")),
        missing_discriminator: string_field(value.get("missing_discriminator")),
        suggested_test: SuggestedTest {
            recommended_test: string_path(value, &["suggested_test", "recommended_test"]),
            assertion_shape: string_path(value, &["suggested_test", "assertion_shape"]),
        },
        repair: Repair {
            verify_command: string_path(value, &["repair", "verify_command"]),
        },
        evidence_record: evidence_record_repair_context_from_value(value.get("evidence_record")),
        review: review_metadata_from_value(value.get("review")),
    }
}

fn evidence_record_repair_context_from_value(
    value: Option<&Value>,
) -> Option<EvidenceRecordRepairContext> {
    let value = value?;
    if !value.is_object() {
        return None;
    }
    let recommendation = value.get("recommendation");
    Some(EvidenceRecordRepairContext {
        seam_id: string_field(value.get("seam_id")),
        path: string_path(value, &["location", "file"]),
        line: path_value(value, &["location", "line"]).and_then(Value::as_u64),
        static_class: string_field(value.get("grip_class")),
        missing_discriminator: first_string_array_object_field(
            value.get("missing_discriminators"),
            "value",
        ),
        suggested_test: recommendation
            .and_then(|recommendation| string_path(recommendation, &["assertion_shape", "example"]))
            .or_else(|| {
                recommendation.and_then(|recommendation| {
                    test_label_from_value(recommendation.get("recommended_test"))
                })
            }),
        related_test: recommendation
            .and_then(|recommendation| {
                test_label_from_value(recommendation.get("nearest_test_to_imitate"))
            })
            .or_else(|| {
                recommendation.and_then(|recommendation| {
                    test_label_from_value(recommendation.get("recommended_test"))
                })
            }),
        verify_command: recommendation
            .and_then(|recommendation| string_field(recommendation.get("verify_command"))),
        repair_command: path_value(value, &["canonical_item", "repair_command"])
            .and_then(Value::as_str)
            .filter(|command| !command.trim().is_empty())
            .map(ToOwned::to_owned),
        static_limitations: static_limitations_from_evidence_record(value),
    })
}

fn first_string_array_object_field(value: Option<&Value>, field: &str) -> Option<String> {
    value
        .and_then(Value::as_array)
        .and_then(|items| items.iter().find_map(|item| string_field(item.get(field))))
}

fn test_label_from_value(value: Option<&Value>) -> Option<String> {
    let value = value?;
    let name = string_field(value.get("name"));
    let file = string_field(value.get("file"));
    match (file, name) {
        (Some(file), Some(name)) => Some(format!("{file}::{name}")),
        (Some(file), None) => Some(file),
        (None, Some(name)) => Some(name),
        (None, None) => None,
    }
}

fn static_limitations_from_evidence_record(value: &Value) -> Vec<String> {
    value
        .get("static_limitations")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(static_limitation_label)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn static_limitation_label(value: &Value) -> Option<String> {
    let reason = string_field(value.get("reason"))?;
    let stage = string_field(value.get("stage"));
    let state = string_field(value.get("state"));
    match (stage, state) {
        (Some(stage), Some(state)) => Some(format!("{stage}/{state}: {reason}")),
        (Some(stage), None) => Some(format!("{stage}: {reason}")),
        (None, Some(state)) => Some(format!("{state}: {reason}")),
        (None, None) => Some(reason),
    }
}

fn review_metadata_from_value(value: Option<&Value>) -> Option<ReviewMetadata> {
    let value = value?;
    if !value.is_object() {
        return Some(ReviewMetadata {
            invalid: true,
            ..ReviewMetadata::default()
        });
    }
    Some(ReviewMetadata {
        invalid: false,
        owner: string_field(value.get("owner")),
        reason: string_field(value.get("reason")),
        created_at: string_field(value.get("created_at")),
        review_after: string_field(value.get("review_after")),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MetadataState {
    Current,
    Stale,
    Missing,
    Unknown,
}

fn classify_review(review: Option<ReviewMetadata>, generated_at: &str) -> MetadataState {
    let Some(review) = review else {
        return MetadataState::Missing;
    };
    if review.invalid {
        return MetadataState::Unknown;
    }
    let Some(review_after) = review.review_after.as_deref() else {
        return MetadataState::Missing;
    };
    if review.owner.is_none() || review.reason.is_none() || review.created_at.is_none() {
        return MetadataState::Missing;
    }
    if is_stale_review(review_after, generated_at) {
        MetadataState::Stale
    } else {
        MetadataState::Current
    }
}

fn count_metadata(counts: &mut MetadataCounts, state: MetadataState) {
    match state {
        MetadataState::Current => counts.current += 1,
        MetadataState::Stale => counts.stale += 1,
        MetadataState::Missing => counts.missing_metadata += 1,
        MetadataState::Unknown => counts.unknown += 1,
    }
}

fn metadata_warnings(metadata: &MetadataCounts) -> Vec<String> {
    let mut warnings = Vec::new();
    warn_for_metadata(metadata.clone(), &mut warnings);
    warnings
}

fn warn_for_metadata(metadata: MetadataCounts, warnings: &mut Vec<String>) {
    if metadata.missing_metadata > 0 {
        warnings.push(format!(
            "{} baseline entries are missing review metadata",
            metadata.missing_metadata
        ));
    }
    if metadata.stale > 0 {
        warnings.push(format!(
            "{} baseline entries have stale review metadata",
            metadata.stale
        ));
    }
    if metadata.unknown > 0 {
        warnings.push(format!(
            "{} baseline entries have unparseable review metadata",
            metadata.unknown
        ));
    }
}

fn top_debt_areas(items: &[DeltaItem]) -> Vec<TopDebtArea> {
    let mut areas: BTreeMap<String, AreaAccumulator> = BTreeMap::new();
    for item in items {
        if !is_visible_area_bucket(&item.bucket) {
            continue;
        }
        let area = item.path.clone().unwrap_or_else(|| "unknown".to_string());
        let entry = areas.entry(area).or_default();
        if is_visible_unresolved_bucket(&item.bucket) {
            entry.visible_unresolved += 1;
        }
        if item.bucket == "new_policy_eligible" {
            entry.new_policy_eligible += 1;
        }
        if item.bucket == "stale_baseline_entry" {
            entry.stale_baseline_entries += 1;
        }
        if let Some(class) = item.static_class.as_ref() {
            *entry.class_counts.entry(class.clone()).or_insert(0) += 1;
        }
    }
    let mut rows = areas.into_iter().collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .1
            .visible_unresolved
            .cmp(&left.1.visible_unresolved)
            .then_with(|| right.1.new_policy_eligible.cmp(&left.1.new_policy_eligible))
            .then_with(|| left.0.cmp(&right.0))
    });
    rows.into_iter()
        .take(5)
        .enumerate()
        .map(|(index, (area, counts))| TopDebtArea {
            rank: index + 1,
            area,
            visible_unresolved: counts.visible_unresolved,
            new_policy_eligible: counts.new_policy_eligible,
            stale_baseline_entries: counts.stale_baseline_entries,
            top_static_class: top_static_class(&counts.class_counts),
        })
        .collect()
}

fn gap_repair_routes(records: &[GapRecord]) -> Vec<RepairRoute> {
    records
        .iter()
        .filter(|record| {
            gap_decision_ledger::projection_eligible(record, "ripr_zero_count")
                && record.repairability == "repairable"
                && record.repair_route.is_some()
        })
        .take(5)
        .enumerate()
        .map(|(index, record)| {
            let route = record.repair_route.as_ref();
            let anchor = record.anchor.as_ref();
            let seam_id = if record.canonical_gap_id.is_empty() {
                (!record.gap_id.is_empty()).then(|| record.gap_id.clone())
            } else {
                Some(record.canonical_gap_id.clone())
            };
            RepairRoute {
                rank: index + 1,
                source: "gap_decision_ledger".to_string(),
                gap_id: (!record.gap_id.is_empty()).then(|| record.gap_id.clone()),
                canonical_gap_id: (!record.canonical_gap_id.is_empty())
                    .then(|| record.canonical_gap_id.clone()),
                seam_id,
                path: anchor
                    .and_then(|anchor| anchor.file.clone())
                    .or_else(|| route.and_then(|route| route.target_file.clone())),
                line: anchor.and_then(|anchor| anchor.line),
                static_class: (!record.evidence_class.is_empty())
                    .then(|| record.evidence_class.clone()),
                missing_discriminator: (!record.kind.is_empty()).then(|| record.kind.clone()),
                suggested_test: route.and_then(|route| route.assertion_shape.clone()),
                related_test: route.and_then(|route| route.related_test.clone()),
                verify_command: record.verification_commands.first().cloned(),
                // Gap records carry no repair start (#3906), and this route's
                // `seam_id` is a gap identity, so no agent command is named.
                repair_command: None,
                agent_command: None,
                static_limitations: Vec::new(),
            }
        })
        .collect()
}

fn repair_routes(items: &[DeltaItem]) -> Vec<RepairRoute> {
    let mut candidates = items
        .iter()
        .filter(|item| repair_route_priority(&item.bucket).is_some())
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        repair_route_priority(&left.bucket)
            .cmp(&repair_route_priority(&right.bucket))
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.line.cmp(&right.line))
    });
    candidates
        .into_iter()
        .take(5)
        .enumerate()
        .map(|(index, item)| {
            let evidence = item.evidence_record.as_ref();
            let seam_id = item
                .identity
                .seam_id
                .clone()
                .or_else(|| evidence.and_then(|record| record.seam_id.clone()));
            RepairRoute {
                rank: index + 1,
                source: "baseline_debt_delta".to_string(),
                gap_id: None,
                canonical_gap_id: None,
                seam_id,
                path: evidence
                    .and_then(|record| record.path.clone())
                    .or_else(|| item.path.clone()),
                line: evidence.and_then(|record| record.line).or(item.line),
                static_class: evidence
                    .and_then(|record| record.static_class.clone())
                    .or_else(|| item.static_class.clone()),
                missing_discriminator: evidence
                    .and_then(|record| record.missing_discriminator.clone())
                    .or_else(|| item.missing_discriminator.clone()),
                suggested_test: evidence
                    .and_then(|record| record.suggested_test.clone())
                    .or_else(|| {
                        item.suggested_test
                            .assertion_shape
                            .clone()
                            .or_else(|| item.suggested_test.recommended_test.clone())
                    }),
                related_test: evidence
                    .and_then(|record| record.related_test.clone())
                    .or_else(|| item.suggested_test.recommended_test.clone()),
                verify_command: evidence
                    .and_then(|record| record.verify_command.clone())
                    .or_else(|| item.repair.verify_command.clone()),
                repair_command: evidence.and_then(|record| record.repair_command.clone()),
                agent_command: evidence.and_then(|record| record.repair_command.clone()),
                static_limitations: evidence
                    .map(|record| record.static_limitations.clone())
                    .unwrap_or_default(),
            }
        })
        .collect()
}

fn is_baseline_derived_bucket(bucket: &str) -> bool {
    matches!(
        bucket,
        "still_present"
            | "resolved"
            | "stale_baseline_entry"
            | "invalid_baseline_entry"
            | "missing_current_input"
    )
}

fn is_visible_area_bucket(bucket: &str) -> bool {
    matches!(
        bucket,
        "still_present" | "new_policy_eligible" | "acknowledged" | "stale_baseline_entry"
    )
}

fn is_visible_unresolved_bucket(bucket: &str) -> bool {
    matches!(
        bucket,
        "still_present" | "new_policy_eligible" | "acknowledged"
    )
}

fn repair_route_priority(bucket: &str) -> Option<u8> {
    match bucket {
        "new_policy_eligible" => Some(0),
        "still_present" => Some(1),
        "acknowledged" => Some(2),
        "stale_baseline_entry" => Some(3),
        _ => None,
    }
}

fn top_static_class(counts: &BTreeMap<String, usize>) -> Option<String> {
    counts
        .iter()
        .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
        .map(|(class, _count)| class.clone())
}

fn is_stale_review(review_after: &str, generated_at: &str) -> bool {
    match (unix_ms(review_after), unix_ms(generated_at)) {
        (Some(review_after), Some(generated_at)) => review_after < generated_at,
        _ => match (iso_day(review_after), iso_day(generated_at)) {
            (Some(review_after), Some(generated_at)) => review_after < generated_at,
            _ => false,
        },
    }
}

fn age_days(created_at: &str, generated_at: &str) -> Option<i64> {
    match (unix_ms(created_at), unix_ms(generated_at)) {
        (Some(created_at), Some(generated_at)) => {
            let millis_per_day = 86_400_000i128;
            let days = (generated_at - created_at) / millis_per_day;
            i64::try_from(days).ok()
        }
        _ => None,
    }
}

fn unix_ms(value: &str) -> Option<i128> {
    value.strip_prefix("unix_ms:")?.parse().ok()
}

fn iso_day(value: &str) -> Option<String> {
    let day = value.get(0..10)?;
    if day.len() == 10
        && day.as_bytes().get(4) == Some(&b'-')
        && day.as_bytes().get(7) == Some(&b'-')
    {
        Some(day.to_string())
    } else {
        None
    }
}

fn baseline_path_for_summary(input_path: Option<&str>, delta_path: Option<&str>) -> Option<String> {
    input_path
        .map(ToOwned::to_owned)
        .or_else(|| delta_path.map(ToOwned::to_owned))
}

fn inputs_json(inputs: &RiprZeroInputs) -> Value {
    json!({
        "baseline": inputs.baseline,
        "baseline_debt_delta": inputs.baseline_debt_delta,
        "gap_decision_ledger": inputs.gap_decision_ledger,
        "gate_decision": inputs.gate_decision,
        "pr_guidance": inputs.pr_guidance,
        "recommendation_calibration": inputs.recommendation_calibration,
        "previous_status": inputs.previous_status,
    })
}

fn ripr_zero_json(summary: &RiprZeroSummary) -> Value {
    json!({
        "state": summary.state,
        "target_source": summary.target_source,
        "visible_unresolved": summary.visible_unresolved,
        "new_policy_eligible": summary.new_policy_eligible,
        "blocking_candidates": summary.blocking_candidates,
        "acknowledged": summary.acknowledged,
        "suppressed": summary.suppressed,
        "limits_note": RIPR_ZERO_LIMITS_NOTE,
    })
}

fn baseline_json(summary: &BaselineSummary) -> Value {
    json!({
        "path": summary.path,
        "entries": summary.entries,
        "still_present": summary.still_present,
        "resolved": summary.resolved,
        "age_days": summary.age_days,
        "metadata": {
            "current": summary.metadata.current,
            "stale": summary.metadata.stale,
            "missing_metadata": summary.metadata.missing_metadata,
            "unknown": summary.metadata.unknown,
        }
    })
}

fn debt_delta_json(summary: &DebtDeltaSummary) -> Value {
    json!({
        "still_present": summary.still_present,
        "resolved": summary.resolved,
        "new": summary.new,
        "new_policy_eligible": summary.new_policy_eligible,
        "acknowledged": summary.acknowledged,
        "suppressed": summary.suppressed,
        "stale": summary.stale,
        "invalid": summary.invalid,
        "missing_input": summary.missing_input,
    })
}

fn trend_json(summary: &TrendSummary) -> Value {
    json!({
        "source": summary.source,
        "window": summary.window,
        "visible_unresolved_delta": summary.visible_unresolved_delta,
        "resolved_delta": summary.resolved_delta,
        "new_policy_eligible_delta": summary.new_policy_eligible_delta,
    })
}

fn top_debt_area_json(area: &TopDebtArea) -> Value {
    json!({
        "rank": area.rank,
        "area": area.area,
        "visible_unresolved": area.visible_unresolved,
        "new_policy_eligible": area.new_policy_eligible,
        "stale_baseline_entries": area.stale_baseline_entries,
        "top_static_class": area.top_static_class,
    })
}

fn repair_route_json(route: &RepairRoute) -> Value {
    let mut value = json!({
        "rank": route.rank,
        "source": route.source,
        "gap_id": route.gap_id,
        "canonical_gap_id": route.canonical_gap_id,
        "seam_id": route.seam_id,
        "path": route.path,
        "line": route.line,
        "static_class": route.static_class,
        "missing_discriminator": route.missing_discriminator,
        "suggested_test": route.suggested_test,
        "related_test": route.related_test,
        "verify_command": route.verify_command,
        "agent_command": route.agent_command,
        "static_limitations": route.static_limitations,
    });
    if let Some(repair_command) = route.repair_command.as_ref()
        && let Some(fields) = value.as_object_mut()
    {
        fields.insert(
            "repair_command".to_string(),
            Value::String(repair_command.clone()),
        );
    }
    value
}

fn route_headline(route: &RepairRoute) -> String {
    match (
        route.path.as_deref(),
        route.line,
        route.static_class.as_deref(),
    ) {
        (Some(path), Some(line), Some(class)) => format!("{path}:{line} {class}"),
        (Some(path), Some(line), None) => format!("{path}:{line}"),
        (Some(path), None, Some(class)) => format!("{path} {class}"),
        (Some(path), None, None) => path.to_string(),
        _ => route
            .seam_id
            .clone()
            .unwrap_or_else(|| "unknown route".to_string()),
    }
}

fn warnings_from_value(value: &Value) -> Vec<String> {
    value
        .get("warnings")
        .and_then(Value::as_array)
        .map(|warnings| warnings.iter().filter_map(string_value).collect())
        .unwrap_or_default()
}

fn usize_path(value: &Value, path: &[&str]) -> usize {
    path_value(value, path)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
}

/// A delta count that must be a valid nonnegative integer: `None` when the
/// key is missing or present-but-malformed (null, string, float, negative,
/// or overflowing). Used where a silent zero would fabricate zero debt.
fn validated_count(delta_section: &serde_json::Map<String, Value>, key: &str) -> Option<usize> {
    delta_section
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn string_path(value: &Value, path: &[&str]) -> Option<String> {
    path_value(value, path).and_then(string_value)
}

fn path_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cursor = value;
    for segment in path {
        cursor = cursor.get(*segment)?;
    }
    Some(cursor)
}

fn string_field(value: Option<&Value>) -> Option<String> {
    value.and_then(string_value)
}

fn string_value(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(ToOwned::to_owned)
}

pub(crate) use crate::output::path::display_path;

#[cfg(test)]
mod tests {
    use super::{
        RiprZeroStatusInput, build_ripr_zero_status_report, render_ripr_zero_status_json,
        render_ripr_zero_status_markdown,
    };
    use crate::output::first_pr::{REPAIR_AFTER_PHASE_LABEL, REPAIR_AFTER_PHASE_STEP};
    use serde_json::Value;

    #[test]
    fn ripr_zero_status_reports_not_yet_with_metadata_and_repair_route() -> Result<(), String> {
        let baseline = r#"{
          "schema_version": "0.1",
          "kind": "gate_baseline",
          "created_at": "unix_ms:0",
          "entries": [
            {"identity": {"seam_id": "same"}, "path": "src/same.rs", "review": {"owner": "team", "reason": "baseline", "created_at": "unix_ms:0", "review_after": "unix_ms:200000000"}},
            {"identity": {"seam_id": "stale"}, "path": "src/stale.rs", "review": {"owner": "team", "reason": "baseline", "created_at": "unix_ms:0", "review_after": "unix_ms:1"}},
            {"identity": {"seam_id": "missing"}, "path": "src/missing.rs", "review": {"reason": "baseline"}},
            {"identity": {"seam_id": "unknown"}, "path": "src/unknown.rs", "review": "legacy-note"}
          ]
        }"#;
        let delta = r#"{
          "schema_version": "0.1",
          "tool": "ripr",
          "kind": "baseline_debt_delta",
          "baseline": {"path": ".ripr/gate-baseline.json", "entries": 4},
          "delta": {
            "still_present": 1,
            "resolved": 1,
            "new_policy_eligible": 1,
            "acknowledged": 1,
            "suppressed": 1,
            "stale_baseline_entry": 1,
            "invalid_baseline_entry": 0,
            "missing_current_input": 0
          },
          "items": [
            {"bucket": "still_present", "identity": {"seam_id": "same"}, "path": "src/same.rs", "line": 1, "static_class": "weakly_gripped", "missing_discriminator": "same == 1", "suggested_test": {"assertion_shape": "assert_eq!(same(), 1)", "recommended_test": "tests/same.rs::boundary"}, "repair": {"verify_command": "ripr agent verify --json"}},
            {"bucket": "resolved", "identity": {"seam_id": "gone"}, "path": "src/gone.rs", "line": 2, "static_class": "weakly_gripped", "repair": {}},
            {"bucket": "new_policy_eligible", "identity": {"seam_id": "new"}, "path": "src/new.rs", "line": 4, "static_class": "weakly_gripped", "missing_discriminator": "new == 4", "suggested_test": {"assertion_shape": "assert_eq!(new(), 4)", "recommended_test": "tests/new.rs::boundary"}, "repair": {"verify_command": "ripr agent verify --json"}},
            {"bucket": "acknowledged", "identity": {"seam_id": "ack"}, "path": "src/ack.rs", "line": 5, "static_class": "weakly_gripped", "repair": {}},
            {"bucket": "suppressed", "identity": {"seam_id": "suppressed"}, "path": "src/suppressed.rs", "line": 6, "static_class": "weakly_gripped", "repair": {}},
            {"bucket": "stale_baseline_entry", "identity": {"seam_id": "stale"}, "path": "src/stale.rs", "line": 7, "static_class": "weakly_gripped", "repair": {}}
          ],
          "warnings": []
        }"#;
        let gate = r#"{"schema_version":"0.1","summary":{"blocking":2},"warnings":[]}"#;

        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: Some(".ripr/gate-baseline.json".to_string()),
            delta_path: "target/ripr/reports/baseline-debt-delta.json".to_string(),
            gap_ledger_path: None,
            gate_path: Some("target/ripr/reports/gate-decision.json".to_string()),
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: Some(Ok(baseline.to_string())),
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: None,
            gate_json: Some(Ok(gate.to_string())),
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(rendered.contains("\"state\": \"not_yet\""));
        assert!(rendered.contains("\"visible_unresolved\": 3"));
        assert!(rendered.contains("\"blocking_candidates\": 2"));
        assert!(rendered.contains("\"current\": 1"));
        assert!(rendered.contains("\"stale\": 1"));
        assert!(rendered.contains("\"missing_metadata\": 1"));
        assert!(rendered.contains("\"unknown\": 1"));
        assert!(rendered.contains("\"agent_command\""));

        let markdown = render_ripr_zero_status_markdown(&report);
        assert!(markdown.contains("RIPR 0: not_yet"));
        assert!(markdown.contains("Top repair route:"));
        assert!(markdown.contains("new == 4"));
        Ok(())
    }

    #[test]
    fn ripr_zero_status_uses_gap_ledger_targets_when_supplied() -> Result<(), String> {
        let delta = r#"{
          "schema_version": "0.1",
          "tool": "ripr",
          "kind": "baseline_debt_delta",
          "baseline": {"entries": 0},
          "delta": {
            "still_present": 0,
            "resolved": 0,
            "new_policy_eligible": 0,
            "acknowledged": 0,
            "suppressed": 0,
            "stale_baseline_entry": 0,
            "invalid_baseline_entry": 0,
            "missing_current_input": 0
          },
          "items": []
        }"#;
        let gap_ledger = r#"{
          "gap_records": [
            {
              "gap_id": "gap:repo:pricing:reintroduced-boundary",
              "source_currentness": "candidate_current",
              "canonical_gap_id": "gap:rust:pricing:discount:threshold-boundary",
              "kind": "MissingBoundaryAssertion",
              "language": "rust",
              "language_status": "stable",
              "scope": "repo_scoped",
              "evidence_class": "predicate_boundary",
              "gap_state": "reintroduced",
              "policy_state": "reintroduced",
              "repairability": "repairable",
              "anchor": {"file": "src/pricing.rs", "line": 42},
              "repair_route": {
                "route_kind": "AddBoundaryAssertion",
                "target_file": "tests/pricing.rs",
                "assertion_shape": "assert_eq!(discount(100, 100), 90)"
              },
              "verification_commands": ["cargo xtask fixtures boundary_gap"],
              "projection_eligibility": {
                "ripr_zero_count": {"eligible": true, "reason": "repo_policy_targeted_unresolved_gap"},
                "ripr_plus_count": {"eligible": true, "reason": "broader_repo_advisory_gap"}
              }
            }
          ]
        }"#;

        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "target/ripr/reports/baseline-debt-delta.json".to_string(),
            gap_ledger_path: Some("target/ripr/reports/gap-decision-ledger.json".to_string()),
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: Some(Ok(gap_ledger.to_string())),
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });

        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(rendered.contains("\"target_source\": \"gap_decision_ledger\""));
        assert!(rendered.contains("\"visible_unresolved\": 1"));
        assert!(rendered.contains("\"source\": \"gap_decision_ledger\""));
        assert!(rendered.contains("\"gap_id\": \"gap:repo:pricing:reintroduced-boundary\""));
        assert!(rendered.contains("\"verify_command\": \"cargo xtask fixtures boundary_gap\""));
        let markdown = render_ripr_zero_status_markdown(&report);
        assert!(markdown.contains("Target source: `gap_decision_ledger`"));
        assert!(markdown.contains("assert_eq!(discount(100, 100), 90)"));
        Ok(())
    }

    #[test]
    fn ripr_zero_status_prefers_evidence_record_repair_context() -> Result<(), String> {
        let delta = r#"{
          "schema_version": "0.1",
          "tool": "ripr",
          "kind": "baseline_debt_delta",
          "baseline": {"entries": 0},
          "delta": {
            "still_present": 0,
            "resolved": 0,
            "new_policy_eligible": 1,
            "acknowledged": 0,
            "suppressed": 0,
            "stale_baseline_entry": 0,
            "invalid_baseline_entry": 0,
            "missing_current_input": 0
          },
          "items": [
            {
              "bucket": "new_policy_eligible",
              "identity": {"seam_id": "legacy-seam"},
              "path": "src/legacy.rs",
              "line": 1,
              "static_class": "legacy_class",
              "missing_discriminator": "legacy discriminator",
              "suggested_test": {
                "assertion_shape": "legacy assertion",
                "recommended_test": "tests/legacy.rs::legacy"
              },
              "repair": {"verify_command": "legacy verify"},
              "evidence_record": {
                "schema_version": "0.1",
                "seam_id": "record-seam",
                "canonical_gap_id": null,
                "owner": "pricing::discounted_total",
                "location": {"file": "src/pricing.rs", "line": 88},
                "seam_kind": "predicate_boundary",
                "grip_class": "weakly_gripped",
                "headline_eligible": true,
                "evidence_path": {},
                "observed_values": [],
                "missing_discriminators": [
                  {"value": "amount == discount_threshold", "reason": "missing equality boundary"}
                ],
                "related_tests": [],
                "recommendation": {
                  "action": "write_targeted_test",
                  "reason": "extend the nearest related test",
                  "recommended_test": {
                    "name": "discounted_total_boundary_discriminator",
                    "file": "tests/pricing.rs",
                    "reason": "nearest related test"
                  },
                  "nearest_test_to_imitate": {
                    "name": "above_threshold_discount",
                    "file": "tests/pricing.rs",
                    "line": 12,
                    "oracle_kind": "exact_value",
                    "oracle_strength": "strong",
                    "evidence_summary": "exact value assertion",
                    "relation_reason": "direct_owner_call",
                    "relation_confidence": "high"
                  },
                  "candidate_values": [],
                  "assertion_shape": {
                    "kind": "exact_return_value",
                    "example": "assert_eq!(discounted_total(/* threshold */), expected)"
                  },
                  "verify_command": "ripr evidence-movement --before before.json --after after.json"
                },
                "actionability": {},
                "calibration": {},
                "static_limitations": [
                  {"stage": "propagate", "state": "unknown", "reason": "call target unresolved"}
                ]
              }
            }
          ],
          "warnings": []
        }"#;

        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "target/ripr/reports/baseline-debt-delta.json".to_string(),
            gap_ledger_path: None,
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: None,
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let markdown = render_ripr_zero_status_markdown(&report);
        let rendered = render_ripr_zero_status_json(&report)?;
        let value = serde_json::from_str::<Value>(&rendered)
            .map_err(|err| format!("RIPR Zero status JSON should parse: {err}"))?;
        let route = value
            .get("repair_routes")
            .and_then(Value::as_array)
            .and_then(|routes| routes.first())
            .ok_or_else(|| format!("missing repair route in: {rendered}"))?;
        assert_eq!(
            route.get("path").and_then(Value::as_str),
            Some("src/pricing.rs"),
            "expected evidence_record path in: {rendered}"
        );
        assert_eq!(
            route.get("static_class").and_then(Value::as_str),
            Some("weakly_gripped"),
            "expected evidence_record grip class in: {rendered}"
        );
        assert_eq!(
            route.get("missing_discriminator").and_then(Value::as_str),
            Some("amount == discount_threshold"),
            "expected evidence_record missing discriminator in: {rendered}"
        );
        assert_eq!(
            route.get("suggested_test").and_then(Value::as_str),
            Some("assert_eq!(discounted_total(/* threshold */), expected)"),
            "expected evidence_record assertion shape in: {rendered}"
        );
        assert_eq!(
            route.get("related_test").and_then(Value::as_str),
            Some("tests/pricing.rs::above_threshold_discount"),
            "expected evidence_record related test in: {rendered}"
        );
        assert_eq!(
            route.get("verify_command").and_then(Value::as_str),
            Some("ripr evidence-movement --before before.json --after after.json"),
            "expected evidence_record verify command in: {rendered}"
        );
        // #3906: this record carries no repair start, so neither the route
        // nor its Markdown names a repair-loop command for `record-seam`.
        assert_eq!(route.get("agent_command"), Some(&Value::Null));
        assert!(route.get("repair_command").is_none());
        assert!(!rendered.contains("agent start") && !rendered.contains("agent repair"));
        assert!(!markdown.contains("agent start") && !markdown.contains("agent repair"));

        let carried = "ripr agent repair --root . --seam-id record-seam --phase before";
        let mut carried_delta: Value =
            serde_json::from_str(delta).map_err(|err| format!("parse delta fixture: {err}"))?;
        carried_delta["items"][0]["evidence_record"]["canonical_item"] =
            serde_json::json!({ "repair_command": carried });
        let carried_report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "target/ripr/reports/baseline-debt-delta.json".to_string(),
            gap_ledger_path: None,
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(carried_delta.to_string()),
            gap_ledger_json: None,
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let carried_rendered = render_ripr_zero_status_json(&carried_report)?;
        let carried_value = serde_json::from_str::<Value>(&carried_rendered)
            .map_err(|err| format!("RIPR Zero status JSON should parse: {err}"))?;
        assert_eq!(
            carried_value["repair_routes"][0]["repair_command"],
            Value::from(carried)
        );
        assert_eq!(
            carried_value["repair_routes"][0]["agent_command"],
            Value::from(carried)
        );
        let carried_markdown = render_ripr_zero_status_markdown(&carried_report);
        // #3906 (F60-14): the after phase follows the carried start.
        assert!(
            carried_markdown.contains(&format!(
                "  Repair start: {carried}\n  {REPAIR_AFTER_PHASE_LABEL}: {REPAIR_AFTER_PHASE_STEP}\n"
            )),
            "{carried_markdown}"
        );
        assert!(!carried_markdown.contains("  Verify after the test edit:"));
        let limitation = route
            .get("static_limitations")
            .and_then(Value::as_array)
            .and_then(|limits| limits.first())
            .and_then(Value::as_str);
        assert_eq!(
            limitation,
            Some("propagate/unknown: call target unresolved"),
            "expected evidence_record static limitation in: {rendered}"
        );
        assert!(
            markdown.contains("Static limit: propagate/unknown: call target unresolved"),
            "expected markdown static limitation in: {markdown}"
        );
        Ok(())
    }

    #[test]
    fn evidence_record_context_handles_invalid_and_fallback_shapes() -> Result<(), String> {
        assert!(
            super::evidence_record_repair_context_from_value(None).is_none(),
            "missing evidence_record should not produce repair context"
        );
        let invalid = serde_json::json!("not an object");
        assert!(
            super::evidence_record_repair_context_from_value(Some(&invalid)).is_none(),
            "non-object evidence_record should not produce repair context"
        );

        let record = serde_json::json!({
          "schema_version": "0.1",
          "seam_id": "record-seam",
          "canonical_gap_id": null,
          "owner": "pricing::discounted_total",
          "location": {"file": "src/pricing.rs"},
          "seam_kind": "predicate_boundary",
          "grip_class": "reachable_unrevealed",
          "headline_eligible": true,
          "evidence_path": {},
          "observed_values": [],
          "missing_discriminators": [
            {"value": "amount == discount_threshold"}
          ],
          "related_tests": [],
          "recommendation": {
            "recommended_test": {
              "file": "tests/pricing.rs",
              "name": "discounted_total_boundary_discriminator"
            },
            "verify_command": "ripr evidence-movement --before before.json --after after.json"
          },
          "actionability": {},
          "calibration": {},
          "static_limitations": [
            {"stage": "activate", "reason": "constant unresolved"},
            {"state": "unknown", "reason": "state only"},
            {"reason": "plain reason"},
            {"stage": "observe"}
          ]
        });

        let context = super::evidence_record_repair_context_from_value(Some(&record))
            .ok_or_else(|| "expected valid evidence_record repair context".to_string())?;
        assert!(
            context.line.is_none(),
            "line should be absent in partial context: {context:?}"
        );
        assert_eq!(
            context.suggested_test.as_deref(),
            Some("tests/pricing.rs::discounted_total_boundary_discriminator"),
            "recommended_test should be assertion fallback: {context:?}"
        );
        assert_eq!(
            context.related_test.as_deref(),
            Some("tests/pricing.rs::discounted_total_boundary_discriminator"),
            "recommended_test should be related-test fallback: {context:?}"
        );
        assert_eq!(
            context.static_limitations,
            [
                "activate: constant unresolved",
                "unknown: state only",
                "plain reason",
            ],
            "unexpected static limitation labels: {context:?}"
        );
        Ok(())
    }

    #[test]
    fn evidence_record_test_labels_accept_partial_labels() -> Result<(), String> {
        let file_only = serde_json::json!({"file": "tests/pricing.rs"});
        let name_only = serde_json::json!({"name": "discounted_total_boundary"});
        let empty = serde_json::json!({});
        assert_eq!(
            super::test_label_from_value(Some(&file_only)),
            Some("tests/pricing.rs".to_string()),
            "file-only test label should use file"
        );
        assert_eq!(
            super::test_label_from_value(Some(&name_only)),
            Some("discounted_total_boundary".to_string()),
            "name-only test label should use name"
        );
        assert!(
            super::test_label_from_value(Some(&empty)).is_none(),
            "empty test label should not produce a label"
        );
        assert!(
            super::test_label_from_value(None).is_none(),
            "missing test label should not produce a label"
        );
        Ok(())
    }

    #[test]
    fn ripr_zero_status_falls_back_to_legacy_fields_when_record_is_partial() -> Result<(), String> {
        let delta = r#"{
          "schema_version": "0.1",
          "tool": "ripr",
          "kind": "baseline_debt_delta",
          "baseline": {"entries": 0},
          "delta": {
            "still_present": 0,
            "resolved": 0,
            "new_policy_eligible": 1,
            "acknowledged": 0,
            "suppressed": 0,
            "stale_baseline_entry": 0,
            "invalid_baseline_entry": 0,
            "missing_current_input": 0
          },
          "items": [
            {
              "bucket": "new_policy_eligible",
              "identity": {},
              "path": "src/legacy.rs",
              "line": 7,
              "static_class": "legacy_class",
              "missing_discriminator": "legacy discriminator",
              "suggested_test": {
                "assertion_shape": "legacy assertion",
                "recommended_test": "tests/legacy.rs::legacy_case"
              },
              "repair": {"verify_command": "legacy verify"},
              "evidence_record": {
                "schema_version": "0.1",
                "seam_id": "record-seam",
                "canonical_gap_id": null,
                "owner": "pricing::discounted_total",
                "seam_kind": "predicate_boundary",
                "headline_eligible": true,
                "evidence_path": {},
                "observed_values": [],
                "missing_discriminators": [],
                "related_tests": [],
                "recommendation": {},
                "actionability": {},
                "calibration": {},
                "static_limitations": []
              }
            }
          ],
          "warnings": []
        }"#;

        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "target/ripr/reports/baseline-debt-delta.json".to_string(),
            gap_ledger_path: None,
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: None,
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        let value = serde_json::from_str::<Value>(&rendered)
            .map_err(|err| format!("RIPR Zero status JSON should parse: {err}"))?;
        let route = value
            .get("repair_routes")
            .and_then(Value::as_array)
            .and_then(|routes| routes.first())
            .ok_or_else(|| format!("missing repair route in: {rendered}"))?;
        assert_eq!(
            route.get("seam_id").and_then(Value::as_str),
            Some("record-seam"),
            "expected record seam fallback in: {rendered}"
        );
        assert_eq!(
            route.get("path").and_then(Value::as_str),
            Some("src/legacy.rs"),
            "expected legacy path fallback in: {rendered}"
        );
        assert_eq!(
            route.get("line").and_then(Value::as_u64),
            Some(7),
            "expected legacy line fallback in: {rendered}"
        );
        assert_eq!(
            route.get("static_class").and_then(Value::as_str),
            Some("legacy_class"),
            "expected legacy static class fallback in: {rendered}"
        );
        assert_eq!(
            route.get("missing_discriminator").and_then(Value::as_str),
            Some("legacy discriminator"),
            "expected legacy missing discriminator fallback in: {rendered}"
        );
        assert_eq!(
            route.get("suggested_test").and_then(Value::as_str),
            Some("legacy assertion"),
            "expected legacy assertion fallback in: {rendered}"
        );
        assert_eq!(
            route.get("related_test").and_then(Value::as_str),
            Some("tests/legacy.rs::legacy_case"),
            "expected legacy related test fallback in: {rendered}"
        );
        assert_eq!(
            route.get("verify_command").and_then(Value::as_str),
            Some("legacy verify"),
            "expected legacy verify fallback in: {rendered}"
        );
        assert_eq!(
            route.get("agent_command"),
            Some(&Value::Null),
            "a seam id without a carried repair start names no agent command: {rendered}"
        );
        assert!(!rendered.contains("agent start") && !rendered.contains("agent repair"));
        Ok(())
    }

    #[test]
    fn ripr_zero_status_reports_achieved_when_no_visible_debt_remains() -> Result<(), String> {
        let delta = r#"{
          "schema_version": "0.1",
          "kind": "baseline_debt_delta",
          "baseline": {"entries": 1},
          "delta": {
            "still_present": 0,
            "resolved": 1,
            "new_policy_eligible": 0,
            "acknowledged": 0,
            "suppressed": 0,
            "stale_baseline_entry": 0,
            "invalid_baseline_entry": 0,
            "missing_current_input": 0
          },
          "items": [
            {"bucket": "resolved", "identity": {"seam_id": "r"}, "path": "src/r.rs"}
          ]
        }"#;
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "delta.json".to_string(),
            gap_ledger_path: None,
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: None,
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(rendered.contains("\"state\": \"achieved\""));
        assert!(rendered.contains("\"visible_unresolved\": 0"));
        Ok(())
    }

    #[test]
    fn ripr_zero_status_reports_unknown_for_a_content_free_delta() -> Result<(), String> {
        let delta = r#"{
          "schema_version": "0.1",
          "kind": "baseline_debt_delta"
        }"#;
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "delta.json".to_string(),
            gap_ledger_path: None,
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: None,
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(rendered.contains("\"state\": \"unknown\""), "{rendered}");
        assert!(rendered.contains("has no delta section"), "{rendered}");
        assert!(!rendered.contains("\"state\": \"achieved\""), "{rendered}");
        Ok(())
    }

    #[test]
    fn ripr_zero_status_reports_unknown_for_a_partial_delta_section() -> Result<(), String> {
        let delta = r#"{
          "schema_version": "0.1",
          "kind": "baseline_debt_delta",
          "delta": {"resolved": 1}
        }"#;
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "delta.json".to_string(),
            gap_ledger_path: None,
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: None,
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(rendered.contains("\"state\": \"unknown\""), "{rendered}");
        assert!(rendered.contains("partial delta section"), "{rendered}");
        Ok(())
    }

    #[test]
    fn ripr_zero_status_reports_unknown_for_a_malformed_delta_count() -> Result<(), String> {
        // Present-but-malformed counts must never read as zero debt (#6095
        // review): null, string, and negative counts are all Invalid.
        let delta = r#"{
          "schema_version": "0.1",
          "kind": "baseline_debt_delta",
          "delta": {
            "still_present": null,
            "resolved": 0,
            "new_policy_eligible": "0",
            "acknowledged": 0,
            "suppressed": 0,
            "stale_baseline_entry": -1,
            "invalid_baseline_entry": 0,
            "missing_current_input": 0
          },
          "items": []
        }"#;
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "delta.json".to_string(),
            gap_ledger_path: None,
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: None,
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(rendered.contains("\"state\": \"unknown\""), "{rendered}");
        assert!(rendered.contains("malformed counts"), "{rendered}");
        assert!(rendered.contains("still_present"), "{rendered}");
        assert!(!rendered.contains("\"state\": \"achieved\""), "{rendered}");
        Ok(())
    }

    #[test]
    fn ripr_zero_status_reports_unknown_for_overflowing_delta_counts() -> Result<(), String> {
        // Aggregates that overflow must fail closed (#6095 review): a wrapped
        // sum would fabricate zero visible debt and `achieved`. usize::MAX
        // keeps the overflow exact on any pointer width.
        let delta = format!(
            r#"{{
          "schema_version": "0.1",
          "kind": "baseline_debt_delta",
          "delta": {{
            "still_present": {},
            "resolved": 0,
            "new_policy_eligible": 1,
            "acknowledged": 0,
            "suppressed": 0,
            "stale_baseline_entry": 0,
            "invalid_baseline_entry": 0,
            "missing_current_input": 0
          }},
          "items": []
        }}"#,
            usize::MAX
        );
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "delta.json".to_string(),
            gap_ledger_path: None,
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta),
            gap_ledger_json: None,
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(rendered.contains("\"state\": \"unknown\""), "{rendered}");
        assert!(rendered.contains("aggregates overflow"), "{rendered}");
        assert!(!rendered.contains("\"state\": \"achieved\""), "{rendered}");
        Ok(())
    }

    #[test]
    fn ripr_zero_status_reports_unknown_for_unclassified_items_under_resolved_counts()
    -> Result<(), String> {
        // An unclassifiable item blocks achieved even when historical
        // resolved counts are positive: resolved is not current-debt
        // evidence for an item of unknown classification (#6095 review).
        let delta = r#"{
          "schema_version": "0.1",
          "kind": "baseline_debt_delta",
          "delta": {
            "still_present": 0,
            "resolved": 1,
            "new_policy_eligible": 0,
            "acknowledged": 0,
            "suppressed": 0,
            "stale_baseline_entry": 0,
            "invalid_baseline_entry": 0,
            "missing_current_input": 0
          },
          "items": [{"bucket": "unrecognized", "path": "src/mystery.rs"}]
        }"#;
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "delta.json".to_string(),
            gap_ledger_path: None,
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: None,
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(rendered.contains("\"state\": \"unknown\""), "{rendered}");
        assert!(rendered.contains("contradictory"), "{rendered}");
        assert!(!rendered.contains("\"state\": \"achieved\""), "{rendered}");
        Ok(())
    }

    #[test]
    fn ripr_zero_status_discloses_a_partial_scope_delta_and_withholds_achieved()
    -> Result<(), String> {
        let delta = r#"{
          "schema_version": "0.1",
          "kind": "baseline_debt_delta",
          "run_status": "limited_partial_scope",
          "delta": {
            "still_present": 0,
            "resolved": 0,
            "new_policy_eligible": 0,
            "acknowledged": 0,
            "suppressed": 0,
            "stale_baseline_entry": 0,
            "invalid_baseline_entry": 0,
            "missing_current_input": 0
          },
          "items": []
        }"#;
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "delta.json".to_string(),
            gap_ledger_path: None,
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: None,
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(rendered.contains("\"state\": \"unknown\""), "{rendered}");
        assert!(rendered.contains("limited_partial_scope"), "{rendered}");
        assert!(!rendered.contains("\"state\": \"achieved\""), "{rendered}");
        Ok(())
    }

    #[test]
    fn ripr_zero_status_keeps_not_yet_for_debt_under_a_partial_scope_delta() -> Result<(), String> {
        let delta = r#"{
          "schema_version": "0.1",
          "kind": "baseline_debt_delta",
          "run_status": "limited_partial_scope",
          "delta": {
            "still_present": 2,
            "resolved": 0,
            "new_policy_eligible": 0,
            "acknowledged": 0,
            "suppressed": 0,
            "stale_baseline_entry": 0,
            "invalid_baseline_entry": 0,
            "missing_current_input": 0
          },
          "items": [
            {"bucket": "still_present", "identity": {"seam_id": "a"}, "path": "src/a.rs"},
            {"bucket": "still_present", "identity": {"seam_id": "b"}, "path": "src/b.rs"}
          ]
        }"#;
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "delta.json".to_string(),
            gap_ledger_path: None,
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: None,
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(rendered.contains("\"state\": \"not_yet\""), "{rendered}");
        assert!(rendered.contains("limited_partial_scope"), "{rendered}");
        Ok(())
    }

    #[test]
    fn ripr_zero_status_reports_unknown_when_items_contradict_zero_counts() -> Result<(), String> {
        let delta = r#"{
          "schema_version": "0.1",
          "kind": "baseline_debt_delta",
          "delta": {
            "still_present": 0,
            "resolved": 0,
            "new_policy_eligible": 0,
            "acknowledged": 0,
            "suppressed": 0,
            "stale_baseline_entry": 0,
            "invalid_baseline_entry": 0,
            "missing_current_input": 0
          },
          "items": [{"bucket": "still_present", "path": "src/a.rs"}]
        }"#;
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "delta.json".to_string(),
            gap_ledger_path: None,
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: None,
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(rendered.contains("\"state\": \"unknown\""), "{rendered}");
        assert!(rendered.contains("contradict"), "{rendered}");
        Ok(())
    }

    #[test]
    fn ripr_zero_status_discloses_a_partial_scope_gap_ledger() -> Result<(), String> {
        let delta = r#"{
          "schema_version": "0.1",
          "kind": "baseline_debt_delta",
          "delta": {
            "still_present": 0,
            "resolved": 1,
            "new_policy_eligible": 0,
            "acknowledged": 0,
            "suppressed": 0,
            "stale_baseline_entry": 0,
            "invalid_baseline_entry": 0,
            "missing_current_input": 0
          },
          "items": [
            {"bucket": "resolved", "identity": {"seam_id": "r"}, "path": "src/r.rs"}
          ]
        }"#;
        let gap_ledger = r#"{
          "run_status": "limited_partial_scope",
          "gap_records": []
        }"#;
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "delta.json".to_string(),
            gap_ledger_path: Some("gap-ledger.json".to_string()),
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: Some(Ok(gap_ledger.to_string())),
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(rendered.contains("\"state\": \"unknown\""), "{rendered}");
        assert!(
            rendered.contains("gap decision ledger input gap-ledger.json discloses"),
            "{rendered}"
        );
        Ok(())
    }

    #[test]
    fn ripr_zero_status_rejects_a_blocked_gap_ledger_with_producer_warnings() -> Result<(), String>
    {
        // A `status: "blocked"` ledger with producer warnings is a failed
        // producer, never a complete zero denominator: an otherwise clean
        // delta must report unknown and preserve the warnings (#6095 review).
        let delta = r#"{
          "schema_version": "0.1",
          "kind": "baseline_debt_delta",
          "delta": {
            "still_present": 0,
            "resolved": 1,
            "new_policy_eligible": 0,
            "acknowledged": 0,
            "suppressed": 0,
            "stale_baseline_entry": 0,
            "invalid_baseline_entry": 0,
            "missing_current_input": 0
          },
          "items": [
            {"bucket": "resolved", "identity": {"seam_id": "r"}, "path": "src/r.rs"}
          ]
        }"#;
        let gap_ledger = r#"{
          "status": "blocked",
          "records": [],
          "warnings": ["parse ledger-source.json failed: invalid JSON: expected value at line 1 column 1"]
        }"#;
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "delta.json".to_string(),
            gap_ledger_path: Some("gap-ledger.json".to_string()),
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: Some(Ok(gap_ledger.to_string())),
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(rendered.contains("\"state\": \"unknown\""), "{rendered}");
        assert!(
            rendered.contains(
                "gap decision ledger input gap-ledger.json discloses a blocked producer run"
            ),
            "{rendered}"
        );
        assert!(
            rendered.contains("producer warning: parse ledger-source.json failed"),
            "{rendered}"
        );
        Ok(())
    }

    #[test]
    fn ripr_zero_status_accepts_a_warning_free_blocked_gap_ledger_as_zero() -> Result<(), String> {
        // The producer also reports `status: "blocked"` for a genuinely
        // empty zero-gap ledger; with no warnings that stays a complete zero
        // denominator and a clean delta still reports achieved.
        let delta = r#"{
          "schema_version": "0.1",
          "kind": "baseline_debt_delta",
          "delta": {
            "still_present": 0,
            "resolved": 1,
            "new_policy_eligible": 0,
            "acknowledged": 0,
            "suppressed": 0,
            "stale_baseline_entry": 0,
            "invalid_baseline_entry": 0,
            "missing_current_input": 0
          },
          "items": [
            {"bucket": "resolved", "identity": {"seam_id": "r"}, "path": "src/r.rs"}
          ]
        }"#;
        let gap_ledger = r#"{
          "status": "blocked",
          "records": [],
          "warnings": []
        }"#;
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "delta.json".to_string(),
            gap_ledger_path: Some("gap-ledger.json".to_string()),
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: Some(Ok(gap_ledger.to_string())),
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(rendered.contains("\"state\": \"achieved\""), "{rendered}");
        assert!(
            rendered.contains("\"target_source\": \"gap_decision_ledger\""),
            "{rendered}"
        );
        Ok(())
    }

    #[test]
    fn ripr_zero_status_keeps_delta_debt_under_a_failed_gap_ledger() -> Result<(), String> {
        // A failed (blocked) ledger must not erase visible delta debt: the
        // delta keeps the not_yet signal while the report stays incomplete
        // and preserves the producer warnings (#6095 review).
        let delta = r#"{
          "schema_version": "0.1",
          "kind": "baseline_debt_delta",
          "delta": {
            "still_present": 2,
            "resolved": 0,
            "new_policy_eligible": 0,
            "acknowledged": 0,
            "suppressed": 0,
            "stale_baseline_entry": 0,
            "invalid_baseline_entry": 0,
            "missing_current_input": 0
          },
          "items": [
            {"bucket": "still_present", "identity": {"seam_id": "a"}, "path": "src/a.rs"},
            {"bucket": "still_present", "identity": {"seam_id": "b"}, "path": "src/b.rs"}
          ]
        }"#;
        let gap_ledger = r#"{
          "status": "blocked",
          "records": [],
          "warnings": ["parse ledger-source.json failed: source file not found"]
        }"#;
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "delta.json".to_string(),
            gap_ledger_path: Some("gap-ledger.json".to_string()),
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: Some(Ok(gap_ledger.to_string())),
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(rendered.contains("\"state\": \"not_yet\""), "{rendered}");
        assert!(rendered.contains("\"visible_unresolved\": 2"), "{rendered}");
        assert!(
            rendered.contains("\"target_source\": \"baseline_debt_delta\""),
            "{rendered}"
        );
        assert!(
            rendered.contains("producer warning: parse ledger-source.json failed"),
            "{rendered}"
        );
        Ok(())
    }

    #[test]
    fn ripr_zero_status_surfaces_a_failed_gate_decision() -> Result<(), String> {
        let delta = r#"{
          "schema_version": "0.1",
          "kind": "baseline_debt_delta",
          "delta": {
            "still_present": 0,
            "resolved": 1,
            "new_policy_eligible": 0,
            "acknowledged": 0,
            "suppressed": 0,
            "stale_baseline_entry": 0,
            "invalid_baseline_entry": 0,
            "missing_current_input": 0
          },
          "items": [
            {"bucket": "resolved", "identity": {"seam_id": "r"}, "path": "src/r.rs"}
          ]
        }"#;
        let gate = r#"{
          "schema_version": "0.1",
          "kind": "gate_decision",
          "status": "config_error",
          "config_errors": ["gate evaluate requires --pr-guidance <path> or --gap-ledger <path>"],
          "summary": {"blocking": 2}
        }"#;
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "delta.json".to_string(),
            gap_ledger_path: None,
            gate_path: Some("gate.json".to_string()),
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: None,
            gate_json: Some(Ok(gate.to_string())),
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(
            rendered.contains("reports status config_error (1 config errors)"),
            "{rendered}"
        );
        // A failed gate evaluation is meaningless blocking evidence: the
        // count is forced to 0 and a zero-count delta reports unknown, never
        // achieved (#6095 review).
        assert!(
            rendered.contains("\"blocking_candidates\": 0"),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"status\": \"incomplete\""),
            "{rendered}"
        );
        assert!(rendered.contains("\"state\": \"unknown\""), "{rendered}");
        Ok(())
    }

    #[test]
    fn ripr_zero_status_reports_unknown_when_items_exceed_a_nonzero_count() -> Result<(), String> {
        // The producer aggregates counts from the emitted items, so a
        // still_present count of 1 carrying 2 items is a contradictory
        // document, which cannot yield a verdict (#6095 review).
        let delta = r#"{
          "schema_version": "0.1",
          "kind": "baseline_debt_delta",
          "delta": {
            "still_present": 1,
            "resolved": 0,
            "new_policy_eligible": 0,
            "acknowledged": 0,
            "suppressed": 0,
            "stale_baseline_entry": 0,
            "invalid_baseline_entry": 0,
            "missing_current_input": 0
          },
          "items": [
            {"bucket": "still_present", "identity": {"seam_id": "a"}, "path": "src/a.rs"},
            {"bucket": "still_present", "identity": {"seam_id": "b"}, "path": "src/b.rs"}
          ]
        }"#;
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "delta.json".to_string(),
            gap_ledger_path: None,
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: None,
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(rendered.contains("\"state\": \"unknown\""), "{rendered}");
        assert!(
            rendered.contains("bucket still_present reports count 1 but carries 2 item(s)"),
            "{rendered}"
        );
        Ok(())
    }

    #[test]
    fn ripr_zero_status_reports_unknown_when_a_nonzero_count_carries_no_items() -> Result<(), String>
    {
        // A positive count with no items is the same contradiction from the
        // other side: the counts cannot be trusted, so no verdict (#6095
        // review).
        let delta = r#"{
          "schema_version": "0.1",
          "kind": "baseline_debt_delta",
          "delta": {
            "still_present": 1,
            "resolved": 0,
            "new_policy_eligible": 0,
            "acknowledged": 0,
            "suppressed": 0,
            "stale_baseline_entry": 0,
            "invalid_baseline_entry": 0,
            "missing_current_input": 0
          },
          "items": []
        }"#;
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "delta.json".to_string(),
            gap_ledger_path: None,
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
            gap_ledger_json: None,
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(rendered.contains("\"state\": \"unknown\""), "{rendered}");
        assert!(
            rendered.contains("bucket still_present reports count 1 but carries 0 item(s)"),
            "{rendered}"
        );
        Ok(())
    }

    #[test]
    fn ripr_zero_status_reports_incomplete_when_delta_is_missing() -> Result<(), String> {
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "missing.json".to_string(),
            gap_ledger_path: None,
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Err("read missing.json failed: not found".to_string()),
            gap_ledger_json: None,
            gate_json: None,
            pr_guidance_json: None,
            recommendation_calibration_json: None,
        });
        let rendered = render_ripr_zero_status_json(&report)?;
        assert!(rendered.contains("\"status\": \"incomplete\""));
        assert!(rendered.contains("\"state\": \"unknown\""));
        assert!(rendered.contains("required baseline debt delta input missing.json is invalid"));
        Ok(())
    }
}
