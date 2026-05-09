use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;

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
    pub(crate) gate_path: Option<String>,
    pub(crate) pr_guidance_path: Option<String>,
    pub(crate) recommendation_calibration_path: Option<String>,
    pub(crate) baseline_json: Option<Result<String, String>>,
    pub(crate) delta_json: Result<String, String>,
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
    gate_decision: Option<String>,
    pr_guidance: Option<String>,
    recommendation_calibration: Option<String>,
    previous_status: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiprZeroSummary {
    state: String,
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
    seam_id: Option<String>,
    path: Option<String>,
    line: Option<u64>,
    static_class: Option<String>,
    missing_discriminator: Option<String>,
    suggested_test: Option<String>,
    related_test: Option<String>,
    verify_command: Option<String>,
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
    let gate = parse_gate(input.gate_path.as_deref(), input.gate_json);
    let baseline = parse_baseline(
        input.baseline_path.as_deref(),
        input.baseline_json,
        &delta,
        &input.generated_at,
    );
    let mut warnings = Vec::new();
    warnings.extend(delta.warnings.clone());
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

    let status = if delta.status == ParseStatus::Loaded {
        "advisory"
    } else {
        "incomplete"
    }
    .to_string();
    let visible_unresolved =
        delta.counts.still_present + delta.counts.new_policy_eligible + delta.counts.acknowledged;
    let state = if delta.status != ParseStatus::Loaded {
        "unknown"
    } else if visible_unresolved == 0
        && delta.counts.stale == 0
        && delta.counts.invalid == 0
        && delta.counts.missing_input == 0
    {
        "achieved"
    } else {
        "not_yet"
    }
    .to_string();
    let ripr_zero = RiprZeroSummary {
        state,
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
        repair_routes: repair_routes(&delta.items),
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
        if let Some(verify) = route.verify_command.as_deref() {
            out.push_str(&format!("  Verify: {verify}\n"));
        }
        if let Some(agent) = route.agent_command.as_deref() {
            out.push_str(&format!("  Agent: {agent}\n"));
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
    let counts = DebtDeltaSummary {
        still_present: usize_path(&value, &["delta", "still_present"]),
        resolved: usize_path(&value, &["delta", "resolved"]),
        new: usize_path(&value, &["delta", "new_policy_eligible"])
            + usize_path(&value, &["delta", "acknowledged"])
            + usize_path(&value, &["delta", "suppressed"]),
        new_policy_eligible: usize_path(&value, &["delta", "new_policy_eligible"]),
        acknowledged: usize_path(&value, &["delta", "acknowledged"]),
        suppressed: usize_path(&value, &["delta", "suppressed"]),
        stale: usize_path(&value, &["delta", "stale_baseline_entry"]),
        invalid: usize_path(&value, &["delta", "invalid_baseline_entry"]),
        missing_input: usize_path(&value, &["delta", "missing_current_input"]),
    };
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .map(|items| items.iter().map(delta_item_from_value).collect())
        .unwrap_or_default();
    DeltaParse {
        status: ParseStatus::Loaded,
        baseline_path: string_path(&value, &["baseline", "path"])
            .or_else(|| string_path(&value, &["inputs", "baseline"])),
        baseline_entries: usize_path(&value, &["baseline", "entries"]),
        counts,
        items,
        warnings: warnings_from_value(&value),
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
    GateParse {
        blocking_candidates: usize_path(&value, &["summary", "blocking"]),
        warnings: warnings_from_value(&value),
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
                seam_id: seam_id.clone(),
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
                agent_command: seam_id.as_ref().map(|seam_id| {
                    format!(
                        "ripr agent start --root . --seam-id {seam_id} --out target/ripr/workflow"
                    )
                }),
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
        "gate_decision": inputs.gate_decision,
        "pr_guidance": inputs.pr_guidance,
        "recommendation_calibration": inputs.recommendation_calibration,
        "previous_status": inputs.previous_status,
    })
}

fn ripr_zero_json(summary: &RiprZeroSummary) -> Value {
    json!({
        "state": summary.state,
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
    json!({
        "rank": route.rank,
        "source": route.source,
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
    })
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

pub(crate) fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::{
        RiprZeroStatusInput, build_ripr_zero_status_report, render_ripr_zero_status_json,
        render_ripr_zero_status_markdown,
    };
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
            gate_path: Some("target/ripr/reports/gate-decision.json".to_string()),
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: Some(Ok(baseline.to_string())),
            delta_json: Ok(delta.to_string()),
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
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
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
        if route.get("path").and_then(Value::as_str) != Some("src/pricing.rs") {
            return Err(format!("expected evidence_record path in: {rendered}"));
        }
        if route.get("static_class").and_then(Value::as_str) != Some("weakly_gripped") {
            return Err(format!(
                "expected evidence_record grip class in: {rendered}"
            ));
        }
        if route.get("missing_discriminator").and_then(Value::as_str)
            != Some("amount == discount_threshold")
        {
            return Err(format!(
                "expected evidence_record missing discriminator in: {rendered}"
            ));
        }
        if route.get("suggested_test").and_then(Value::as_str)
            != Some("assert_eq!(discounted_total(/* threshold */), expected)")
        {
            return Err(format!(
                "expected evidence_record assertion shape in: {rendered}"
            ));
        }
        if route.get("related_test").and_then(Value::as_str)
            != Some("tests/pricing.rs::above_threshold_discount")
        {
            return Err(format!(
                "expected evidence_record related test in: {rendered}"
            ));
        }
        if route.get("verify_command").and_then(Value::as_str)
            != Some("ripr evidence-movement --before before.json --after after.json")
        {
            return Err(format!(
                "expected evidence_record verify command in: {rendered}"
            ));
        }
        let limitation = route
            .get("static_limitations")
            .and_then(Value::as_array)
            .and_then(|limits| limits.first())
            .and_then(Value::as_str);
        if limitation != Some("propagate/unknown: call target unresolved") {
            return Err(format!(
                "expected evidence_record static limitation in: {rendered}"
            ));
        }
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
          "items": []
        }"#;
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "delta.json".to_string(),
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Ok(delta.to_string()),
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
    fn ripr_zero_status_reports_incomplete_when_delta_is_missing() -> Result<(), String> {
        let report = build_ripr_zero_status_report(RiprZeroStatusInput {
            root: ".".to_string(),
            generated_at: "unix_ms:100000000".to_string(),
            baseline_path: None,
            delta_path: "missing.json".to_string(),
            gate_path: None,
            pr_guidance_path: None,
            recommendation_calibration_path: None,
            baseline_json: None,
            delta_json: Err("read missing.json failed: not found".to_string()),
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
