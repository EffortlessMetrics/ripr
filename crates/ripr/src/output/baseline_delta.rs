use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "baseline_debt_delta";
const STATUS: &str = "advisory";
const LIMITS_NOTE: &str = "Advisory baseline debt movement over static RIPR gate evidence; pass/fail remains owned by ripr gate evaluate.";
pub(crate) const DEFAULT_BASELINE_DELTA_OUT: &str = "target/ripr/reports/baseline-debt-delta.json";
pub(crate) const DEFAULT_BASELINE_DELTA_MD_OUT: &str = "target/ripr/reports/baseline-debt-delta.md";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BaselineDeltaInput {
    pub(crate) root: String,
    pub(crate) baseline_path: String,
    pub(crate) current_gate_decision_path: String,
    pub(crate) baseline_json: Result<String, String>,
    pub(crate) current_gate_decision_json: Result<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BaselineDeltaReport {
    root: String,
    inputs: BaselineDeltaInputs,
    baseline: BaselineSummary,
    delta: DeltaCounts,
    items: Vec<DeltaItem>,
    warnings: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BaselineDeltaInputs {
    baseline: String,
    current_gate_decision: String,
    pr_guidance: Option<String>,
    agent_receipt: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BaselineSummary {
    path: String,
    schema_version: Option<String>,
    entries: usize,
    valid: usize,
    stale: usize,
    invalid: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DeltaCounts {
    still_present: usize,
    resolved: usize,
    new_policy_eligible: usize,
    acknowledged: usize,
    suppressed: usize,
    stale_baseline_entry: usize,
    invalid_baseline_entry: usize,
    missing_current_input: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DeltaItem {
    bucket: Bucket,
    identity: Identity,
    path: Option<String>,
    line: Option<u64>,
    static_class: Option<String>,
    decision: Option<String>,
    reason: String,
    missing_discriminator: Option<String>,
    suggested_test: SuggestedTest,
    repair: Repair,
    review: Option<ReviewMetadata>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ReviewMetadata {
    reviewed: Option<bool>,
    owner: Option<String>,
    reason: Option<String>,
    created_at: Option<String>,
    review_after: Option<String>,
    source: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SuggestedTest {
    recommended_test: Option<String>,
    assertion_shape: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Repair {
    action: String,
    verify_command: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Bucket {
    StillPresent,
    Resolved,
    NewPolicyEligible,
    Acknowledged,
    Suppressed,
    StaleBaselineEntry,
    InvalidBaselineEntry,
    MissingCurrentInput,
}

impl Bucket {
    fn as_str(self) -> &'static str {
        match self {
            Self::StillPresent => "still_present",
            Self::Resolved => "resolved",
            Self::NewPolicyEligible => "new_policy_eligible",
            Self::Acknowledged => "acknowledged",
            Self::Suppressed => "suppressed",
            Self::StaleBaselineEntry => "stale_baseline_entry",
            Self::InvalidBaselineEntry => "invalid_baseline_entry",
            Self::MissingCurrentInput => "missing_current_input",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::StillPresent => "Still present",
            Self::Resolved => "Resolved",
            Self::NewPolicyEligible => "New policy-eligible",
            Self::Acknowledged => "Acknowledged",
            Self::Suppressed => "Suppressed",
            Self::StaleBaselineEntry => "Stale baseline entry",
            Self::InvalidBaselineEntry => "Invalid baseline entry",
            Self::MissingCurrentInput => "Missing current input",
        }
    }

    fn order(self) -> u8 {
        match self {
            Self::StillPresent => 0,
            Self::Resolved => 1,
            Self::NewPolicyEligible => 2,
            Self::Acknowledged => 3,
            Self::Suppressed => 4,
            Self::StaleBaselineEntry => 5,
            Self::InvalidBaselineEntry => 6,
            Self::MissingCurrentInput => 7,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Identity {
    canonical_gap_id: Option<String>,
    seam_id: Option<String>,
    source_id: Option<String>,
    id: Option<String>,
    dedupe_key: Option<String>,
    fallback: Option<String>,
    matched_by: Option<String>,
}

impl Identity {
    fn has_stable_value(&self) -> bool {
        self.canonical_gap_id.is_some()
            || self.seam_id.is_some()
            || self.source_id.is_some()
            || self.id.is_some()
            || self.dedupe_key.is_some()
            || self.fallback.is_some()
    }

    fn sort_key(&self) -> String {
        match self
            .canonical_gap_id
            .as_deref()
            .or(self.seam_id.as_deref())
            .or(self.source_id.as_deref())
            .or(self.id.as_deref())
            .or(self.dedupe_key.as_deref())
            .or(self.fallback.as_deref())
        {
            Some(value) => value.to_string(),
            None => String::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BaselineRecord {
    identity: Identity,
    path: Option<String>,
    line: Option<u64>,
    static_class: Option<String>,
    decision: Option<String>,
    evidence: Evidence,
    review: Option<ReviewMetadata>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CurrentDecision {
    identity: Identity,
    path: Option<String>,
    line: Option<u64>,
    static_class: Option<String>,
    decision: String,
    severity: Option<String>,
    gate_reason: Option<String>,
    evidence: Evidence,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Evidence {
    missing_discriminator: Option<String>,
    assertion_shape: Option<String>,
    recommended_test: Option<String>,
    suppressed: bool,
    configured_off: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct BaselineParse {
    schema_version: Option<String>,
    entries: Vec<BaselineRecord>,
    invalid_items: Vec<DeltaItem>,
    warnings: Vec<String>,
    unavailable: bool,
    raw_entries: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CurrentParse {
    decisions: Vec<CurrentDecision>,
    warnings: Vec<String>,
    unavailable: bool,
}

#[derive(Clone, Debug)]
struct CurrentIndexes {
    canonical_gap_id: BTreeMap<String, Vec<usize>>,
    seam_id: BTreeMap<String, Vec<usize>>,
    source_id: BTreeMap<String, Vec<usize>>,
    id: BTreeMap<String, Vec<usize>>,
    dedupe_key: BTreeMap<String, Vec<usize>>,
    fallback: BTreeMap<String, Vec<usize>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MatchResult {
    Match { index: usize, matched_by: String },
    Ambiguous { matched_by: String, count: usize },
    None,
}

pub(crate) fn build_baseline_delta_report(input: BaselineDeltaInput) -> BaselineDeltaReport {
    let baseline = parse_baseline_records(&input.baseline_path, input.baseline_json);
    let current = parse_current_decisions(
        &input.current_gate_decision_path,
        input.current_gate_decision_json,
    );
    let mut warnings = baseline.warnings.clone();
    warnings.extend(current.warnings.clone());
    let mut items = baseline.invalid_items.clone();
    let mut matched_current = BTreeSet::new();

    if baseline.unavailable || current.unavailable {
        push_missing_input_items(&baseline, &current, &mut items);
    } else {
        let indexes = build_current_indexes(&current.decisions);
        for baseline_record in &baseline.entries {
            match match_current_decision(&baseline_record.identity, &indexes) {
                MatchResult::Match { index, matched_by } if matched_current.contains(&index) => {
                    items.push(stale_item(
                        baseline_record,
                        format!(
                            "Baseline identity also matched a current decision already joined by another baseline entry using {matched_by}."
                        ),
                    ));
                }
                MatchResult::Match { index, matched_by } => {
                    matched_current.insert(index);
                    if matched_by == "fallback" {
                        warnings.push(format!(
                            "baseline entry {} matched current evidence by fallback path/line/static_class",
                            baseline_record.identity.sort_key()
                        ));
                    }
                    let current_decision = &current.decisions[index];
                    items.push(matched_item(baseline_record, current_decision, matched_by));
                }
                MatchResult::Ambiguous { matched_by, count } => {
                    items.push(stale_item(
                        baseline_record,
                        format!(
                            "Baseline identity matched {count} current decisions by {matched_by}; refresh or narrow the baseline identity."
                        ),
                    ));
                }
                MatchResult::None => items.push(resolved_item(baseline_record)),
            }
        }

        for (index, current_decision) in current.decisions.iter().enumerate() {
            if !matched_current.contains(&index) {
                items.push(unmatched_current_item(current_decision));
            }
        }
    }

    items.sort_by(|left, right| {
        left.bucket
            .order()
            .cmp(&right.bucket.order())
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.identity.sort_key().cmp(&right.identity.sort_key()))
    });

    let delta = count_items(&items);
    BaselineDeltaReport {
        root: input.root,
        inputs: BaselineDeltaInputs {
            baseline: input.baseline_path.clone(),
            current_gate_decision: input.current_gate_decision_path,
            pr_guidance: None,
            agent_receipt: None,
        },
        baseline: BaselineSummary {
            path: input.baseline_path,
            schema_version: baseline.schema_version,
            entries: baseline.raw_entries,
            valid: baseline.entries.len(),
            stale: delta.stale_baseline_entry,
            invalid: delta.invalid_baseline_entry,
        },
        delta,
        items,
        warnings,
    }
}

pub(crate) fn render_baseline_delta_json(report: &BaselineDeltaReport) -> Result<String, String> {
    serde_json::to_string_pretty(&json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "kind": REPORT_KIND,
        "status": STATUS,
        "root": report.root,
        "inputs": inputs_json(&report.inputs),
        "baseline": baseline_summary_json(&report.baseline),
        "delta": delta_json(&report.delta),
        "items": report.items.iter().map(item_json).collect::<Vec<_>>(),
        "warnings": report.warnings,
        "limits_note": LIMITS_NOTE,
    }))
    .map_err(|err| format!("failed to render baseline debt delta JSON: {err}"))
}

pub(crate) fn render_baseline_delta_markdown(report: &BaselineDeltaReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Baseline Debt Delta\n\n");
    out.push_str("Status: advisory\n");
    out.push_str(&format!("Baseline: {}\n\n", report.baseline.path));
    out.push_str("| Bucket | Count |\n");
    out.push_str("| --- | ---: |\n");
    for bucket in bucket_order() {
        out.push_str(&format!(
            "| {} | {} |\n",
            bucket.title(),
            count_for_bucket(&report.delta, bucket)
        ));
    }

    let new_items = report
        .items
        .iter()
        .filter(|item| item.bucket == Bucket::NewPolicyEligible)
        .take(10)
        .collect::<Vec<_>>();
    if !new_items.is_empty() {
        out.push_str("\nTop new policy-eligible gaps:\n");
        for item in new_items {
            out.push_str(&format!("- {}\n", item_headline(item)));
            if let Some(missing) = item.missing_discriminator.as_deref() {
                out.push_str(&format!("  Missing: {missing}\n"));
            }
            out.push_str("  Action: add a focused test or acknowledge visibly.\n");
        }
    }

    let resolved_items = report
        .items
        .iter()
        .filter(|item| item.bucket == Bucket::Resolved)
        .take(10)
        .collect::<Vec<_>>();
    if !resolved_items.is_empty() {
        out.push_str("\nResolved baseline entries:\n");
        for item in resolved_items {
            out.push_str(&format!("- {}\n", item_headline(item)));
        }
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

pub(crate) fn baseline_delta_item_count(report: &BaselineDeltaReport) -> usize {
    report.items.len()
}

fn parse_baseline_records(path: &str, json_text: Result<String, String>) -> BaselineParse {
    let text = match json_text {
        Ok(text) => text,
        Err(error) => {
            return BaselineParse {
                unavailable: true,
                warnings: vec![format!(
                    "required baseline input {path} is invalid: {error}"
                )],
                ..BaselineParse::default()
            };
        }
    };
    let value = match serde_json::from_str::<Value>(&text) {
        Ok(value) => value,
        Err(error) => {
            return BaselineParse {
                unavailable: true,
                warnings: vec![format!(
                    "required baseline input {path} is invalid: {error}"
                )],
                ..BaselineParse::default()
            };
        }
    };
    let schema_version = string_field(value.get("schema_version"));
    if schema_version.as_deref() != Some(SCHEMA_VERSION) {
        return BaselineParse {
            schema_version,
            unavailable: true,
            warnings: vec![format!(
                "required baseline input {path} has unsupported schema_version; expected {SCHEMA_VERSION}"
            )],
            ..BaselineParse::default()
        };
    }

    let records = value
        .get("entries")
        .and_then(Value::as_array)
        .or_else(|| value.get("decisions").and_then(Value::as_array));
    let Some(records) = records else {
        return BaselineParse {
            schema_version,
            unavailable: true,
            warnings: vec![format!(
                "required baseline input {path} is missing entries or decisions array"
            )],
            ..BaselineParse::default()
        };
    };

    let mut entries = Vec::new();
    let mut invalid_items = Vec::new();
    for record in records {
        match baseline_record_from_value(record) {
            Some(entry) => entries.push(entry),
            None => invalid_items.push(invalid_baseline_item(record)),
        }
    }

    BaselineParse {
        schema_version,
        entries,
        invalid_items,
        warnings: Vec::new(),
        unavailable: false,
        raw_entries: records.len(),
    }
}

fn parse_current_decisions(path: &str, json_text: Result<String, String>) -> CurrentParse {
    let text = match json_text {
        Ok(text) => text,
        Err(error) => {
            return CurrentParse {
                unavailable: true,
                warnings: vec![format!(
                    "required current gate-decision input {path} is invalid: {error}"
                )],
                ..CurrentParse::default()
            };
        }
    };
    let value = match serde_json::from_str::<Value>(&text) {
        Ok(value) => value,
        Err(error) => {
            return CurrentParse {
                unavailable: true,
                warnings: vec![format!(
                    "required current gate-decision input {path} is invalid: {error}"
                )],
                ..CurrentParse::default()
            };
        }
    };
    if value.get("schema_version").and_then(Value::as_str) != Some(SCHEMA_VERSION) {
        return CurrentParse {
            unavailable: true,
            warnings: vec![format!(
                "required current gate-decision input {path} has unsupported schema_version; expected {SCHEMA_VERSION}"
            )],
            ..CurrentParse::default()
        };
    }
    let Some(decisions) = value.get("decisions").and_then(Value::as_array) else {
        return CurrentParse {
            unavailable: true,
            warnings: vec![format!(
                "required current gate-decision input {path} is missing decisions array"
            )],
            ..CurrentParse::default()
        };
    };

    CurrentParse {
        decisions: decisions
            .iter()
            .filter_map(current_decision_from_value)
            .collect(),
        warnings: Vec::new(),
        unavailable: false,
    }
}

fn baseline_record_from_value(value: &Value) -> Option<BaselineRecord> {
    let identity_value = match value.get("identity") {
        Some(identity) => identity,
        None => value,
    };
    let path =
        string_field(value.get("path")).or_else(|| string_field(value.pointer("/placement/path")));
    let line = value
        .get("line")
        .and_then(Value::as_u64)
        .or_else(|| value.pointer("/placement/line").and_then(Value::as_u64));
    let static_class = string_field(value.get("static_class"));
    let identity = Identity {
        canonical_gap_id: canonical_gap_id_from_value(value),
        seam_id: string_field(identity_value.get("seam_id"))
            .or_else(|| string_field(value.get("seam_id"))),
        source_id: string_field(identity_value.get("source_id"))
            .or_else(|| string_field(value.get("source_id"))),
        id: string_field(identity_value.get("id")).or_else(|| string_field(value.get("id"))),
        dedupe_key: string_field(identity_value.get("dedupe_key"))
            .or_else(|| string_field(value.get("dedupe_key"))),
        fallback: string_field(identity_value.get("fallback"))
            .or_else(|| fallback_identity(path.as_deref(), line, static_class.as_deref())),
        matched_by: None,
    };
    if !identity.has_stable_value() {
        return None;
    }

    Some(BaselineRecord {
        identity,
        path,
        line,
        static_class,
        decision: string_field(value.get("decision")),
        evidence: evidence_from_value(value),
        review: review_metadata_from_value(value.get("review")),
    })
}

fn current_decision_from_value(value: &Value) -> Option<CurrentDecision> {
    let path = string_field(value.pointer("/placement/path"));
    let line = value.pointer("/placement/line").and_then(Value::as_u64);
    let static_class = string_field(value.get("static_class"));
    let identity = Identity {
        canonical_gap_id: canonical_gap_id_from_value(value),
        seam_id: string_field(value.get("seam_id")),
        source_id: string_field(value.get("source_id")),
        id: string_field(value.get("id")),
        dedupe_key: string_field(value.get("dedupe_key")),
        fallback: fallback_identity(path.as_deref(), line, static_class.as_deref()),
        matched_by: None,
    };
    if !identity.has_stable_value() {
        return None;
    }

    Some(CurrentDecision {
        identity,
        path,
        line,
        static_class,
        decision: match string_field(value.get("decision")) {
            Some(decision) => decision,
            None => "unknown".to_string(),
        },
        severity: string_field(value.get("severity")),
        gate_reason: string_field(value.get("gate_reason")),
        evidence: evidence_from_value(value),
    })
}

fn evidence_from_value(value: &Value) -> Evidence {
    Evidence {
        missing_discriminator: string_field(value.pointer("/evidence/missing_discriminator")),
        assertion_shape: string_field(value.pointer("/evidence/assertion_shape")),
        recommended_test: string_field(value.pointer("/evidence/recommended_test")),
        suppressed: value
            .pointer("/evidence/suppressed")
            .and_then(Value::as_bool)
            .is_some_and(|value| value),
        configured_off: value
            .pointer("/evidence/configured_off")
            .and_then(Value::as_bool)
            .is_some_and(|value| value),
    }
}

fn build_current_indexes(decisions: &[CurrentDecision]) -> CurrentIndexes {
    let mut indexes = CurrentIndexes {
        canonical_gap_id: BTreeMap::new(),
        seam_id: BTreeMap::new(),
        source_id: BTreeMap::new(),
        id: BTreeMap::new(),
        dedupe_key: BTreeMap::new(),
        fallback: BTreeMap::new(),
    };
    for (index, decision) in decisions.iter().enumerate() {
        push_index(
            &mut indexes.canonical_gap_id,
            decision.identity.canonical_gap_id.as_ref(),
            index,
        );
        push_index(
            &mut indexes.seam_id,
            decision.identity.seam_id.as_ref(),
            index,
        );
        push_index(
            &mut indexes.source_id,
            decision.identity.source_id.as_ref(),
            index,
        );
        push_index(&mut indexes.id, decision.identity.id.as_ref(), index);
        push_index(
            &mut indexes.dedupe_key,
            decision.identity.dedupe_key.as_ref(),
            index,
        );
        push_index(
            &mut indexes.fallback,
            decision.identity.fallback.as_ref(),
            index,
        );
    }
    indexes
}

fn push_index(index: &mut BTreeMap<String, Vec<usize>>, key: Option<&String>, value: usize) {
    if let Some(key) = key {
        index.entry(key.clone()).or_default().push(value);
    }
}

fn match_current_decision(identity: &Identity, indexes: &CurrentIndexes) -> MatchResult {
    for (method, key, index) in [
        (
            "canonical_gap_id",
            identity.canonical_gap_id.as_ref(),
            &indexes.canonical_gap_id,
        ),
        ("seam_id", identity.seam_id.as_ref(), &indexes.seam_id),
        ("source_id", identity.source_id.as_ref(), &indexes.source_id),
        ("id", identity.id.as_ref(), &indexes.id),
        (
            "dedupe_key",
            identity.dedupe_key.as_ref(),
            &indexes.dedupe_key,
        ),
        ("fallback", identity.fallback.as_ref(), &indexes.fallback),
    ] {
        if let Some(key) = key
            && let Some(matches) = index.get(key)
        {
            if matches.len() == 1 {
                return MatchResult::Match {
                    index: matches[0],
                    matched_by: method.to_string(),
                };
            }
            return MatchResult::Ambiguous {
                matched_by: method.to_string(),
                count: matches.len(),
            };
        }
    }
    MatchResult::None
}

fn push_missing_input_items(
    baseline: &BaselineParse,
    current: &CurrentParse,
    items: &mut Vec<DeltaItem>,
) {
    if !baseline.unavailable && !baseline.entries.is_empty() {
        for record in &baseline.entries {
            items.push(missing_current_input_item(record));
        }
    } else if items.is_empty() || current.unavailable {
        items.push(DeltaItem {
            bucket: Bucket::MissingCurrentInput,
            identity: Identity::default(),
            path: None,
            line: None,
            static_class: None,
            decision: None,
            reason:
                "Required baseline or current gate-decision input is unavailable; rerun or repair the missing artifact."
                    .to_string(),
            missing_discriminator: None,
            suggested_test: SuggestedTest::default(),
            repair: repair("provide_required_input"),
            review: None,
        });
    }
}

fn matched_item(
    baseline: &BaselineRecord,
    current: &CurrentDecision,
    matched_by: String,
) -> DeltaItem {
    let bucket = current_matched_bucket(current);
    let mut identity = current.identity.clone();
    identity.matched_by = Some(matched_by);
    DeltaItem {
        bucket,
        identity,
        path: current.path.clone().or_else(|| baseline.path.clone()),
        line: current.line.or(baseline.line),
        static_class: current
            .static_class
            .clone()
            .or_else(|| baseline.static_class.clone()),
        decision: Some(current.decision.clone()),
        reason: matched_reason(bucket, current),
        missing_discriminator: current
            .evidence
            .missing_discriminator
            .clone()
            .or_else(|| baseline.evidence.missing_discriminator.clone()),
        suggested_test: suggested_test(&current.evidence, &baseline.evidence),
        repair: repair_for_bucket(bucket),
        review: baseline.review.clone(),
    }
}

fn current_matched_bucket(current: &CurrentDecision) -> Bucket {
    if is_suppressed(current) {
        Bucket::Suppressed
    } else if current.decision == "acknowledged" {
        Bucket::Acknowledged
    } else {
        Bucket::StillPresent
    }
}

fn unmatched_current_item(current: &CurrentDecision) -> DeltaItem {
    let bucket = unmatched_current_bucket(current);
    DeltaItem {
        bucket,
        identity: current.identity.clone(),
        path: current.path.clone(),
        line: current.line,
        static_class: current.static_class.clone(),
        decision: Some(current.decision.clone()),
        reason: unmatched_current_reason(bucket, current),
        missing_discriminator: current.evidence.missing_discriminator.clone(),
        suggested_test: suggested_test(&current.evidence, &Evidence::default()),
        repair: repair_for_bucket(bucket),
        review: None,
    }
}

fn unmatched_current_bucket(current: &CurrentDecision) -> Bucket {
    if is_suppressed(current) {
        Bucket::Suppressed
    } else if current.decision == "acknowledged" {
        Bucket::Acknowledged
    } else {
        Bucket::NewPolicyEligible
    }
}

fn resolved_item(baseline: &BaselineRecord) -> DeltaItem {
    DeltaItem {
        bucket: Bucket::Resolved,
        identity: baseline.identity.clone(),
        path: baseline.path.clone(),
        line: baseline.line,
        static_class: baseline.static_class.clone(),
        decision: baseline.decision.clone(),
        reason: "Reviewed baseline identity is absent from current gate-decision evidence."
            .to_string(),
        missing_discriminator: baseline.evidence.missing_discriminator.clone(),
        suggested_test: suggested_test(&baseline.evidence, &Evidence::default()),
        repair: repair("remove_resolved_from_baseline_when_reviewed"),
        review: baseline.review.clone(),
    }
}

fn stale_item(baseline: &BaselineRecord, reason: String) -> DeltaItem {
    DeltaItem {
        bucket: Bucket::StaleBaselineEntry,
        identity: baseline.identity.clone(),
        path: baseline.path.clone(),
        line: baseline.line,
        static_class: baseline.static_class.clone(),
        decision: baseline.decision.clone(),
        reason,
        missing_discriminator: baseline.evidence.missing_discriminator.clone(),
        suggested_test: suggested_test(&baseline.evidence, &Evidence::default()),
        repair: repair("inspect_or_refresh_baseline_entry"),
        review: baseline.review.clone(),
    }
}

fn missing_current_input_item(baseline: &BaselineRecord) -> DeltaItem {
    DeltaItem {
        bucket: Bucket::MissingCurrentInput,
        identity: baseline.identity.clone(),
        path: baseline.path.clone(),
        line: baseline.line,
        static_class: baseline.static_class.clone(),
        decision: baseline.decision.clone(),
        reason: "Required current gate-decision evidence is unavailable; baseline movement cannot be classified.".to_string(),
        missing_discriminator: baseline.evidence.missing_discriminator.clone(),
        suggested_test: suggested_test(&baseline.evidence, &Evidence::default()),
        repair: repair("provide_current_gate_decision"),
        review: baseline.review.clone(),
    }
}

fn invalid_baseline_item(value: &Value) -> DeltaItem {
    DeltaItem {
        bucket: Bucket::InvalidBaselineEntry,
        identity: Identity::default(),
        path: string_field(value.get("path"))
            .or_else(|| string_field(value.pointer("/placement/path"))),
        line: value
            .get("line")
            .and_then(Value::as_u64)
            .or_else(|| value.pointer("/placement/line").and_then(Value::as_u64)),
        static_class: string_field(value.get("static_class")),
        decision: string_field(value.get("decision")),
        reason:
            "Baseline entry is missing canonical_gap_id, seam_id, source_id, id, dedupe_key, and fallback identity."
                .to_string(),
        missing_discriminator: string_field(value.pointer("/evidence/missing_discriminator")),
        suggested_test: SuggestedTest {
            recommended_test: string_field(value.pointer("/evidence/recommended_test")),
            assertion_shape: string_field(value.pointer("/evidence/assertion_shape")),
        },
        repair: repair("repair_or_remove_baseline_entry"),
        review: review_metadata_from_value(value.get("review")),
    }
}

fn is_suppressed(current: &CurrentDecision) -> bool {
    current.decision == "suppressed"
        || current.decision == "not_applicable"
        || current.severity.as_deref() == Some("off")
        || current.evidence.suppressed
        || current.evidence.configured_off
}

fn matched_reason(bucket: Bucket, current: &CurrentDecision) -> String {
    match bucket {
        Bucket::StillPresent => {
            "Reviewed baseline identity is still present in current gate-decision evidence."
                .to_string()
        }
        Bucket::Acknowledged => current
            .gate_reason
            .clone()
            .unwrap_or_else(|| "Current decision is acknowledged and remains visible.".to_string()),
        Bucket::Suppressed => current
            .gate_reason
            .clone()
            .unwrap_or_else(|| "Current decision is suppressed or configured off.".to_string()),
        _ => "Current decision matched reviewed baseline evidence.".to_string(),
    }
}

fn unmatched_current_reason(bucket: Bucket, current: &CurrentDecision) -> String {
    match bucket {
        Bucket::NewPolicyEligible => {
            "Current policy-eligible gap is not present in the reviewed baseline.".to_string()
        }
        Bucket::Acknowledged => current
            .gate_reason
            .clone()
            .unwrap_or_else(|| "Current decision is acknowledged and remains visible.".to_string()),
        Bucket::Suppressed => current
            .gate_reason
            .clone()
            .unwrap_or_else(|| "Current decision is suppressed or configured off.".to_string()),
        _ => "Current decision is not present in the reviewed baseline.".to_string(),
    }
}

fn suggested_test(primary: &Evidence, fallback: &Evidence) -> SuggestedTest {
    SuggestedTest {
        recommended_test: primary
            .recommended_test
            .clone()
            .or_else(|| fallback.recommended_test.clone()),
        assertion_shape: primary
            .assertion_shape
            .clone()
            .or_else(|| fallback.assertion_shape.clone()),
    }
}

fn repair_for_bucket(bucket: Bucket) -> Repair {
    match bucket {
        Bucket::StillPresent => repair("keep_visible_or_burn_down"),
        Bucket::Resolved => repair("remove_resolved_from_baseline_when_reviewed"),
        Bucket::NewPolicyEligible => Repair {
            action: "add_focused_test_or_acknowledge".to_string(),
            verify_command: Some("ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json".to_string()),
        },
        Bucket::Acknowledged => repair("review_acknowledgement"),
        Bucket::Suppressed => repair("review_suppression_or_config"),
        Bucket::StaleBaselineEntry => repair("inspect_or_refresh_baseline_entry"),
        Bucket::InvalidBaselineEntry => repair("repair_or_remove_baseline_entry"),
        Bucket::MissingCurrentInput => repair("provide_current_gate_decision"),
    }
}

fn repair(action: &str) -> Repair {
    Repair {
        action: action.to_string(),
        verify_command: None,
    }
}

fn count_items(items: &[DeltaItem]) -> DeltaCounts {
    let mut counts = DeltaCounts::default();
    for item in items {
        match item.bucket {
            Bucket::StillPresent => counts.still_present += 1,
            Bucket::Resolved => counts.resolved += 1,
            Bucket::NewPolicyEligible => counts.new_policy_eligible += 1,
            Bucket::Acknowledged => counts.acknowledged += 1,
            Bucket::Suppressed => counts.suppressed += 1,
            Bucket::StaleBaselineEntry => counts.stale_baseline_entry += 1,
            Bucket::InvalidBaselineEntry => counts.invalid_baseline_entry += 1,
            Bucket::MissingCurrentInput => counts.missing_current_input += 1,
        }
    }
    counts
}

fn inputs_json(inputs: &BaselineDeltaInputs) -> Value {
    json!({
        "baseline": inputs.baseline,
        "current_gate_decision": inputs.current_gate_decision,
        "pr_guidance": inputs.pr_guidance,
        "agent_receipt": inputs.agent_receipt,
    })
}

fn baseline_summary_json(summary: &BaselineSummary) -> Value {
    json!({
        "path": summary.path,
        "schema_version": summary.schema_version,
        "entries": summary.entries,
        "valid": summary.valid,
        "stale": summary.stale,
        "invalid": summary.invalid,
    })
}

fn delta_json(delta: &DeltaCounts) -> Value {
    json!({
        "still_present": delta.still_present,
        "resolved": delta.resolved,
        "new_policy_eligible": delta.new_policy_eligible,
        "acknowledged": delta.acknowledged,
        "suppressed": delta.suppressed,
        "stale_baseline_entry": delta.stale_baseline_entry,
        "invalid_baseline_entry": delta.invalid_baseline_entry,
        "missing_current_input": delta.missing_current_input,
    })
}

fn item_json(item: &DeltaItem) -> Value {
    json!({
        "bucket": item.bucket.as_str(),
        "identity": {
            "canonical_gap_id": item.identity.canonical_gap_id,
            "seam_id": item.identity.seam_id,
            "source_id": item.identity.source_id,
            "id": item.identity.id,
            "dedupe_key": item.identity.dedupe_key,
            "fallback": item.identity.fallback,
            "matched_by": item.identity.matched_by,
        },
        "path": item.path,
        "line": item.line,
        "static_class": item.static_class,
        "decision": item.decision,
        "reason": item.reason,
        "missing_discriminator": item.missing_discriminator,
        "suggested_test": {
            "recommended_test": item.suggested_test.recommended_test,
            "assertion_shape": item.suggested_test.assertion_shape,
        },
        "repair": {
            "action": item.repair.action,
            "verify_command": item.repair.verify_command,
        },
        "review": review_metadata_json(&item.review),
    })
}

fn review_metadata_from_value(value: Option<&Value>) -> Option<ReviewMetadata> {
    let value = value?;
    if !value.is_object() {
        return None;
    }
    Some(ReviewMetadata {
        reviewed: value.get("reviewed").and_then(Value::as_bool),
        owner: string_field(value.get("owner")),
        reason: string_field(value.get("reason")),
        created_at: string_field(value.get("created_at")),
        review_after: string_field(value.get("review_after")),
        source: string_field(value.get("source")),
    })
}

fn review_metadata_json(review: &Option<ReviewMetadata>) -> Value {
    match review {
        Some(review) => json!({
            "reviewed": review.reviewed,
            "owner": review.owner,
            "reason": review.reason,
            "created_at": review.created_at,
            "review_after": review.review_after,
            "source": review.source,
        }),
        None => Value::Null,
    }
}

fn bucket_order() -> [Bucket; 8] {
    [
        Bucket::StillPresent,
        Bucket::Resolved,
        Bucket::NewPolicyEligible,
        Bucket::Acknowledged,
        Bucket::Suppressed,
        Bucket::StaleBaselineEntry,
        Bucket::InvalidBaselineEntry,
        Bucket::MissingCurrentInput,
    ]
}

fn count_for_bucket(delta: &DeltaCounts, bucket: Bucket) -> usize {
    match bucket {
        Bucket::StillPresent => delta.still_present,
        Bucket::Resolved => delta.resolved,
        Bucket::NewPolicyEligible => delta.new_policy_eligible,
        Bucket::Acknowledged => delta.acknowledged,
        Bucket::Suppressed => delta.suppressed,
        Bucket::StaleBaselineEntry => delta.stale_baseline_entry,
        Bucket::InvalidBaselineEntry => delta.invalid_baseline_entry,
        Bucket::MissingCurrentInput => delta.missing_current_input,
    }
}

fn item_headline(item: &DeltaItem) -> String {
    match (
        item.path.as_deref(),
        item.line,
        item.static_class.as_deref(),
    ) {
        (Some(path), Some(line), Some(class)) => format!("{path}:{line} {class}"),
        (Some(path), Some(line), None) => format!("{path}:{line}"),
        (Some(path), None, Some(class)) => format!("{path} {class}"),
        (Some(path), None, None) => path.to_string(),
        _ => item.identity.sort_key(),
    }
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

fn canonical_gap_id_from_value(value: &Value) -> Option<String> {
    string_field(value.get("canonical_gap_id"))
        .or_else(|| string_field(value.pointer("/identity/canonical_gap_id")))
        .or_else(|| string_field(value.pointer("/evidence_record/canonical_gap_id")))
}

fn string_field(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(ToOwned::to_owned)
}

pub(crate) use crate::output::path::display_path;

#[cfg(test)]
mod tests {
    use super::{
        BaselineDeltaInput, build_baseline_delta_report, render_baseline_delta_json,
        render_baseline_delta_markdown,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn baseline_delta_reports_all_primary_buckets() -> Result<(), String> {
        let baseline = r#"{
          "schema_version": "0.1",
          "tool": "ripr",
          "kind": "gate_baseline",
          "entries": [
            {
              "identity": {"seam_id": "same"},
              "path": "src/same.rs",
              "line": 1,
              "static_class": "weakly_gripped",
              "decision": "advisory",
              "evidence": {"missing_discriminator": "same == 1"}
            },
            {
              "identity": {"seam_id": "gone"},
              "path": "src/gone.rs",
              "line": 2,
              "static_class": "weakly_gripped",
              "decision": "advisory"
            },
            {
              "identity": {},
              "decision": "advisory"
            }
          ]
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "decisions": [
            {
              "decision": "advisory",
              "id": "ripr-gate-same",
              "seam_id": "same",
              "source_id": "ripr-review-same",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/same.rs", "line": 1},
              "evidence": {"missing_discriminator": "same == 1", "suppressed": false, "configured_off": false}
            },
            {
              "decision": "blocking",
              "id": "ripr-gate-new",
              "seam_id": "new",
              "source_id": "ripr-review-new",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/new.rs", "line": 4},
              "evidence": {"missing_discriminator": "new == 4", "assertion_shape": "assert_eq!(new(), 4)", "recommended_test": "tests/new.rs::boundary", "suppressed": false, "configured_off": false}
            },
            {
              "decision": "acknowledged",
              "id": "ripr-gate-ack",
              "seam_id": "ack",
              "source_id": "ripr-review-ack",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/ack.rs", "line": 5},
              "gate_reason": "policy-eligible gap acknowledged by ripr-waive",
              "evidence": {"suppressed": false, "configured_off": false}
            },
            {
              "decision": "suppressed",
              "id": "ripr-gate-suppressed",
              "seam_id": "suppressed",
              "source_id": "ripr-review-suppressed",
              "static_class": "weakly_gripped",
              "severity": "off",
              "placement": {"path": "src/suppressed.rs", "line": 6},
              "evidence": {"suppressed": true, "configured_off": true}
            }
          ]
        }"#;

        let report = build_baseline_delta_report(BaselineDeltaInput {
            root: ".".to_string(),
            baseline_path: ".ripr/gate-baseline.json".to_string(),
            current_gate_decision_path: "target/ripr/reports/gate-decision.json".to_string(),
            baseline_json: Ok(baseline.to_string()),
            current_gate_decision_json: Ok(current.to_string()),
        });
        let rendered = render_baseline_delta_json(&report)?;
        assert!(rendered.contains("\"still_present\": 1"));
        assert!(rendered.contains("\"resolved\": 1"));
        assert!(rendered.contains("\"new_policy_eligible\": 1"));
        assert!(rendered.contains("\"acknowledged\": 1"));
        assert!(rendered.contains("\"suppressed\": 1"));
        assert!(rendered.contains("\"invalid_baseline_entry\": 1"));
        assert!(rendered.contains("\"matched_by\": \"seam_id\""));
        assert!(rendered.contains("\"verify_command\""));

        let markdown = render_baseline_delta_markdown(&report);
        assert!(markdown.contains("| New policy-eligible | 1 |"));
        assert!(markdown.contains("Top new policy-eligible gaps:"));
        assert!(markdown.contains("Missing: new == 4"));
        assert!(markdown.contains("Resolved baseline entries:"));
        Ok(())
    }

    #[test]
    fn baseline_delta_matches_by_fallback_and_warns() -> Result<(), String> {
        let baseline = r#"{
          "schema_version": "0.1",
          "entries": [
            {
              "identity": {"fallback": "src/fallback.rs:7:weakly_gripped"},
              "path": "src/fallback.rs",
              "line": 7,
              "static_class": "weakly_gripped"
            }
          ]
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "decisions": [
            {
              "decision": "advisory",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/fallback.rs", "line": 7},
              "evidence": {}
            }
          ]
        }"#;

        let report = build_baseline_delta_report(BaselineDeltaInput {
            root: ".".to_string(),
            baseline_path: "baseline.json".to_string(),
            current_gate_decision_path: "current.json".to_string(),
            baseline_json: Ok(baseline.to_string()),
            current_gate_decision_json: Ok(current.to_string()),
        });
        let rendered = render_baseline_delta_json(&report)?;
        assert!(rendered.contains("\"still_present\": 1"));
        assert!(rendered.contains("\"matched_by\": \"fallback\""));
        assert!(rendered.contains("matched current evidence by fallback"));
        Ok(())
    }

    #[test]
    fn baseline_delta_matches_by_canonical_gap_id_across_line_movement() -> Result<(), String> {
        let baseline = r#"{
          "schema_version": "0.1",
          "entries": [
            {
              "identity": {
                "canonical_gap_id": "pricing::discount::threshold_equality",
                "seam_id": "old-seam"
              },
              "path": "src/pricing.rs",
              "line": 10,
              "static_class": "weakly_gripped"
            }
          ]
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "decisions": [
            {
              "decision": "advisory",
              "seam_id": "new-seam-after-refactor",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/pricing.rs", "line": 88},
              "evidence_record": {
                "canonical_gap_id": "pricing::discount::threshold_equality"
              },
              "evidence": {"missing_discriminator": "amount == threshold"}
            }
          ]
        }"#;

        let report = build_baseline_delta_report(BaselineDeltaInput {
            root: ".".to_string(),
            baseline_path: "baseline.json".to_string(),
            current_gate_decision_path: "current.json".to_string(),
            baseline_json: Ok(baseline.to_string()),
            current_gate_decision_json: Ok(current.to_string()),
        });
        let rendered = render_baseline_delta_json(&report)?;
        assert!(rendered.contains("\"still_present\": 1"));
        assert!(rendered.contains("\"matched_by\": \"canonical_gap_id\""));
        assert!(rendered.contains("\"seam_id\": \"new-seam-after-refactor\""));
        assert!(!rendered.contains("\"resolved\": 1"));
        Ok(())
    }

    #[test]
    fn baseline_delta_marks_ambiguous_fallback_as_stale() -> Result<(), String> {
        let baseline = r#"{
          "schema_version": "0.1",
          "entries": [
            {
              "identity": {"fallback": "src/ambiguous.rs:7:weakly_gripped"},
              "path": "src/ambiguous.rs",
              "line": 7,
              "static_class": "weakly_gripped"
            }
          ]
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "decisions": [
            {
              "decision": "advisory",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/ambiguous.rs", "line": 7},
              "evidence": {}
            },
            {
              "decision": "advisory",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/ambiguous.rs", "line": 7},
              "evidence": {}
            }
          ]
        }"#;

        let report = build_baseline_delta_report(BaselineDeltaInput {
            root: ".".to_string(),
            baseline_path: "baseline.json".to_string(),
            current_gate_decision_path: "current.json".to_string(),
            baseline_json: Ok(baseline.to_string()),
            current_gate_decision_json: Ok(current.to_string()),
        });
        let rendered = render_baseline_delta_json(&report)?;
        assert!(rendered.contains("\"stale_baseline_entry\": 1"));
        assert!(rendered.contains("matched 2 current decisions by fallback"));
        Ok(())
    }

    #[test]
    fn baseline_delta_reports_missing_current_input() -> Result<(), String> {
        let baseline = r#"{
          "schema_version": "0.1",
          "entries": [
            {"identity": {"seam_id": "same"}, "path": "src/same.rs", "line": 1}
          ]
        }"#;
        let report = build_baseline_delta_report(BaselineDeltaInput {
            root: ".".to_string(),
            baseline_path: "baseline.json".to_string(),
            current_gate_decision_path: "missing.json".to_string(),
            baseline_json: Ok(baseline.to_string()),
            current_gate_decision_json: Err("read missing.json failed: not found".to_string()),
        });
        let rendered = render_baseline_delta_json(&report)?;
        assert!(rendered.contains("\"missing_current_input\": 1"));
        assert!(rendered.contains("required current gate-decision input missing.json is invalid"));
        Ok(())
    }

    #[test]
    fn baseline_delta_treats_non_object_review_metadata_as_absent() -> Result<(), String> {
        let baseline = r#"{
          "schema_version": "0.1",
          "entries": [
            {
              "identity": {"seam_id": "legacy"},
              "path": "src/legacy.rs",
              "line": 7,
              "review": "legacy-freeform-note"
            }
          ]
        }"#;
        let current = r#"{"schema_version": "0.1", "decisions": []}"#;

        let report = build_baseline_delta_report(BaselineDeltaInput {
            root: ".".to_string(),
            baseline_path: "baseline.json".to_string(),
            current_gate_decision_path: "current.json".to_string(),
            baseline_json: Ok(baseline.to_string()),
            current_gate_decision_json: Ok(current.to_string()),
        });
        let rendered = render_baseline_delta_json(&report)?;
        assert!(rendered.contains("\"resolved\": 1"));
        assert!(rendered.contains("\"review\": null"));
        Ok(())
    }

    #[test]
    fn baseline_delta_reports_input_shape_errors() -> Result<(), String> {
        let valid_baseline = r#"{
          "schema_version": "0.1",
          "entries": [
            {"identity": {"seam_id": "same"}, "path": "src/same.rs", "line": 1}
          ]
        }"#;
        let valid_current = r#"{
          "schema_version": "0.1",
          "decisions": [
            {"decision": "advisory", "seam_id": "same", "evidence": {}}
          ]
        }"#;

        for (baseline_json, current_json, expected) in [
            (
                Err("read baseline.json failed: not found".to_string()),
                Ok(valid_current.to_string()),
                "required baseline input baseline.json is invalid",
            ),
            (
                Ok("{".to_string()),
                Ok(valid_current.to_string()),
                "required baseline input baseline.json is invalid",
            ),
            (
                Ok(r#"{"schema_version":"9","entries":[]}"#.to_string()),
                Ok(valid_current.to_string()),
                "unsupported schema_version",
            ),
            (
                Ok(r#"{"schema_version":"0.1"}"#.to_string()),
                Ok(valid_current.to_string()),
                "missing entries or decisions array",
            ),
            (
                Ok(valid_baseline.to_string()),
                Ok("{".to_string()),
                "required current gate-decision input current.json is invalid",
            ),
            (
                Ok(valid_baseline.to_string()),
                Ok(r#"{"schema_version":"9","decisions":[]}"#.to_string()),
                "current gate-decision input current.json has unsupported schema_version",
            ),
            (
                Ok(valid_baseline.to_string()),
                Ok(r#"{"schema_version":"0.1"}"#.to_string()),
                "current gate-decision input current.json is missing decisions array",
            ),
        ] {
            let report = build_baseline_delta_report(BaselineDeltaInput {
                root: ".".to_string(),
                baseline_path: "baseline.json".to_string(),
                current_gate_decision_path: "current.json".to_string(),
                baseline_json,
                current_gate_decision_json: current_json,
            });
            let rendered = render_baseline_delta_json(&report)?;
            assert!(rendered.contains(expected), "{rendered}");
            assert!(rendered.contains("\"missing_current_input\""), "{rendered}");
        }

        Ok(())
    }

    #[test]
    fn baseline_delta_matches_identity_methods_and_matched_states() -> Result<(), String> {
        let baseline = r#"{
          "schema_version": "0.1",
          "entries": [
            {"identity": {"source_id": "source-match"}, "path": "src/source.rs", "line": 1},
            {"identity": {"id": "id-match"}, "path": "src/id.rs", "line": 2},
            {"identity": {"dedupe_key": "dedupe-match"}, "path": "src/dedupe.rs", "line": 3},
            {"identity": {"seam_id": "ack-match"}, "path": "src/ack.rs", "line": 4},
            {"identity": {"seam_id": "suppressed-match"}, "path": "src/suppressed.rs", "line": 5},
            {"identity": {"seam_id": "unknown-decision"}, "path": "src/unknown.rs", "line": 6},
            {"identity": {"seam_id": "duplicate"}, "path": "src/dup_a.rs", "line": 7},
            {"identity": {"seam_id": "duplicate"}, "path": "src/dup_b.rs", "line": 8}
          ]
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "decisions": [
            {"decision": "advisory", "source_id": "source-match", "evidence": {}},
            {"decision": "advisory", "id": "id-match", "evidence": {}},
            {"decision": "advisory", "dedupe_key": "dedupe-match", "evidence": {}},
            {"decision": "acknowledged", "seam_id": "ack-match", "gate_reason": "waived by ripr-waive", "evidence": {}},
            {"decision": "not_applicable", "seam_id": "suppressed-match", "gate_reason": "configured off", "evidence": {"configured_off": true}},
            {"seam_id": "unknown-decision", "evidence": {}},
            {"decision": "advisory", "seam_id": "duplicate", "evidence": {}},
            {"decision": "advisory", "evidence": {}}
          ]
        }"#;

        let report = build_baseline_delta_report(BaselineDeltaInput {
            root: ".".to_string(),
            baseline_path: "baseline.json".to_string(),
            current_gate_decision_path: "current.json".to_string(),
            baseline_json: Ok(baseline.to_string()),
            current_gate_decision_json: Ok(current.to_string()),
        });
        let rendered = render_baseline_delta_json(&report)?;
        assert!(rendered.contains("\"matched_by\": \"source_id\""));
        assert!(rendered.contains("\"matched_by\": \"id\""));
        assert!(rendered.contains("\"matched_by\": \"dedupe_key\""));
        assert!(rendered.contains("\"decision\": \"unknown\""));
        assert!(rendered.contains("\"acknowledged\": 1"));
        assert!(rendered.contains("\"suppressed\": 1"));
        assert!(rendered.contains("\"stale_baseline_entry\": 1"));
        assert!(rendered.contains("already joined by another baseline entry"));
        assert!(rendered.contains("waived by ripr-waive"));
        assert!(rendered.contains("configured off"));
        Ok(())
    }

    #[test]
    fn baseline_delta_markdown_renders_headline_variants() {
        let baseline = r#"{
          "schema_version": "0.1",
          "entries": [
            {"identity": {"seam_id": "line-only"}, "path": "src/line.rs", "line": 3},
            {"identity": {"seam_id": "class-only"}, "path": "src/class.rs", "static_class": "weakly_gripped"},
            {"identity": {"seam_id": "path-only"}, "path": "src/path.rs"},
            {"identity": {"seam_id": "identity-only"}}
          ]
        }"#;
        let current = r#"{"schema_version": "0.1", "decisions": []}"#;

        let report = build_baseline_delta_report(BaselineDeltaInput {
            root: ".".to_string(),
            baseline_path: "baseline.json".to_string(),
            current_gate_decision_path: "current.json".to_string(),
            baseline_json: Ok(baseline.to_string()),
            current_gate_decision_json: Ok(current.to_string()),
        });
        let markdown = render_baseline_delta_markdown(&report);
        assert!(markdown.contains("- src/line.rs:3"));
        assert!(markdown.contains("- src/class.rs weakly_gripped"));
        assert!(markdown.contains("- src/path.rs"));
        assert!(markdown.contains("- identity-only"));
    }

    #[test]
    fn baseline_delta_matches_mixed_fixture_contract() -> Result<(), String> {
        let repo_root = repo_root()?;
        let fixture = repo_root.join("fixtures/boundary_gap/expected/baseline-debt-delta/mixed");
        let baseline_path = fixture.join("baseline.json");
        let current_path = fixture.join("current-gate-decision.json");
        let expected_json_path = fixture.join("baseline-debt-delta.json");
        let expected_md_path = fixture.join("baseline-debt-delta.md");
        let baseline_text = read_file(&baseline_path)?;
        let current_text = read_file(&current_path)?;

        let report = build_baseline_delta_report(BaselineDeltaInput {
            root: ".".to_string(),
            baseline_path: fixture_path(&repo_root, &baseline_path),
            current_gate_decision_path: fixture_path(&repo_root, &current_path),
            baseline_json: Ok(baseline_text),
            current_gate_decision_json: Ok(current_text),
        });
        let rendered_json = render_baseline_delta_json(&report)?;
        let rendered_md = render_baseline_delta_markdown(&report);
        assert_eq!(rendered_json, read_file(&expected_json_path)?.trim_end());
        assert_eq!(rendered_md, read_file(&expected_md_path)?);
        Ok(())
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
