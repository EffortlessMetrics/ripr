use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::app::causal_projection::{CausalDeltaArtifact, insert_canonical_delta_fields};

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "baseline_debt_delta";
const STATUS: &str = "advisory";
/// Identity authority recorded on a delta item whose current candidate matched
/// a reviewed baseline entry only through the legacy `path:line:static_class`
/// fallback (issue #1964, slice of #1934). Canonical matches never carry this
/// marker: the compatibility match must be impossible to mistake for a
/// canonical identity match.
pub(crate) const BASELINE_MATCH_KIND_LEGACY_PATH_LINE_CLASS: &str = "legacy_path_line_class";
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
    causal_projection: Option<CausalDeltaArtifact>,
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
    legacy_fallback_match: usize,
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
    /// `Some(legacy_path_line_class)` when the current candidate matched the
    /// reviewed baseline entry only through the legacy fallback (issue #1964).
    /// `None` on every canonical match, so canonical items render exactly as
    /// before the disclosure existed.
    baseline_match_kind: Option<String>,
    /// True exactly when `baseline_match_kind` is the legacy fallback: the
    /// match is a reviewable compatibility event, never silent.
    stale_baseline_warning: bool,
    /// The legacy `path:line:static_class` identity that joined the match.
    matched_legacy_identity: Option<String>,
    /// The current candidate's canonical gap id when it carries one, retained
    /// so `ripr baseline update --migrate-legacy-identities` can replace the
    /// reviewed legacy identity deterministically.
    canonical_replacement_candidate: Option<String>,
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
    /// Repository/root identity preserved when the baseline entry was created
    /// (issue #1964). `None` on entries written before root preservation: those
    /// stay comparable under the documented compatibility window.
    root: Option<String>,
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
    /// Top-level `root` of the current gate-decision report, used to refuse
    /// cross-repository fallback joins (issue #1964).
    root: Option<String>,
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
    let causal_projection = match CausalDeltaArtifact::load(std::path::Path::new(&input.root)) {
        Ok(projection) => projection,
        Err(error) => {
            warnings.push(format!(
                "causal comparison artifact is unavailable; causal fields omitted: {error}"
            ));
            None
        }
    };
    let mut items = baseline.invalid_items.clone();
    let mut matched_current = BTreeSet::new();

    if baseline.unavailable || current.unavailable {
        push_missing_input_items(&baseline, &current, &mut items);
    } else {
        let indexes = build_current_indexes(&current.decisions);
        for baseline_record in &baseline.entries {
            match match_current_decision(&baseline_record.identity, &indexes) {
                MatchResult::Match { index, matched_by } if matched_current.contains(&index) => {
                    let item = stale_item(
                        baseline_record,
                        format!(
                            "Baseline identity also matched a current decision already joined by another baseline entry using {matched_by}."
                        ),
                    );
                    // Every legacy-flagged item has a matching warning (issue
                    // #1964, review): warning consumers must see the same
                    // compatibility events the counter reports.
                    if matched_by == "fallback" {
                        warnings.push(format!(
                            "baseline entry {} also matched an already joined current decision by fallback path/line/static_class; treated as stale, not historical",
                            baseline_record.identity.sort_key()
                        ));
                        items.push(with_legacy_disclosure(
                            item,
                            baseline_record.identity.fallback.clone(),
                            current.decisions[index].identity.canonical_gap_id.clone(),
                        ));
                    } else {
                        items.push(item);
                    }
                }
                MatchResult::Match { index, matched_by } => {
                    let current_decision = &current.decisions[index];
                    // CANONICAL DIVERGENCE (issue #1964): both sides carry a
                    // canonical gap id and they differ, so the fallback join
                    // is stale evidence, not a historical match. The baseline
                    // entry goes stale and the current decision stays
                    // unmatched, so the genuinely new canonical gap surfaces
                    // as new policy-eligible instead of looking historical.
                    if matched_by == "fallback"
                        && let Some(divergence) =
                            canonical_divergence(baseline_record, current_decision)
                    {
                        warnings.push(format!(
                            "baseline entry {} matched current evidence by fallback path/line/static_class but canonical identity diverged ({divergence}); treated as stale, not historical",
                            baseline_record.identity.sort_key()
                        ));
                        items.push(with_legacy_disclosure(
                            stale_item(
                                baseline_record,
                                format!(
                                    "Reviewed baseline identity matched current evidence only by legacy fallback, but the canonical gap id diverged ({divergence}); refresh the baseline identity instead of treating the current gap as historical."
                                ),
                            ),
                            baseline_record.identity.fallback.clone(),
                            current_decision.identity.canonical_gap_id.clone(),
                        ));
                    } else if matched_by == "fallback"
                        && let Some(roots) =
                            foreign_root_join(baseline_record, current.root.as_deref())
                    {
                        // FOREIGN ROOT (issue #1964): the reviewed entry was
                        // recorded for another repository. Consuming the
                        // current decision here would launder foreign history
                        // into this repo's delta, so both sides stay visible.
                        warnings.push(format!(
                            "baseline entry {} matched current evidence by fallback path/line/static_class but belongs to another repository root ({roots}); treated as stale, not historical",
                            baseline_record.identity.sort_key()
                        ));
                        items.push(with_legacy_disclosure(
                            stale_item(
                                baseline_record,
                                format!(
                                    "Reviewed baseline identity matched current evidence only by legacy fallback, but it belongs to another repository root ({roots}); the current gap is evaluated on this repository's own history."
                                ),
                            ),
                            baseline_record.identity.fallback.clone(),
                            current_decision.identity.canonical_gap_id.clone(),
                        ));
                    } else {
                        matched_current.insert(index);
                        if matched_by == "fallback" {
                            warnings.push(format!(
                                "baseline entry {} matched current evidence by fallback path/line/static_class",
                                baseline_record.identity.sort_key()
                            ));
                        }
                        items.push(matched_item(baseline_record, current_decision, matched_by));
                    }
                }
                MatchResult::Ambiguous { matched_by, count } => {
                    let item = stale_item(
                        baseline_record,
                        format!(
                            "Baseline identity matched {count} current decisions by {matched_by}; refresh or narrow the baseline identity."
                        ),
                    );
                    // An ambiguous fallback join names no single replacement
                    // candidate, but the legacy identity stays retained and
                    // visible (issue #1964). The stale warning replaces the
                    // generic preserved notice so one join emits one warning.
                    if matched_by == "fallback" {
                        warnings.push(format!(
                            "baseline entry {} matched {count} current decisions by fallback path/line/static_class; treated as stale, not historical",
                            baseline_record.identity.sort_key()
                        ));
                    }
                    items.push(if matched_by == "fallback" {
                        with_legacy_disclosure(
                            item,
                            baseline_record.identity.fallback.clone(),
                            None,
                        )
                    } else {
                        item
                    });
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
        causal_projection,
    }
}

pub(crate) fn render_baseline_delta_json(report: &BaselineDeltaReport) -> Result<String, String> {
    let mut output = json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "kind": REPORT_KIND,
        "status": STATUS,
        "root": report.root,
        "inputs": inputs_json(&report.inputs),
        "baseline": baseline_summary_json(&report.baseline),
        "delta": delta_json(&report.delta),
        "items": report
            .items
            .iter()
            .map(|item| item_json(item, report.causal_projection.as_ref()))
            .collect::<Vec<_>>(),
        "warnings": report.warnings,
        "limits_note": LIMITS_NOTE,
    });
    if let Some(projection) = report.causal_projection.as_ref()
        && let Some(object) = output.as_object_mut()
    {
        projection.insert_comparison_fields(object);
    }
    serde_json::to_string_pretty(&output)
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

    // Human-visible legacy disclosure (issue #1964): every fallback-only
    // match names the retained legacy identity and the canonical replacement
    // candidate, so the compatibility event cannot stay silent in Markdown.
    // Unlike the capped top-new/resolved lists, this section is exhaustive:
    // the contract promises every fallback-only match is visible in human
    // output, and the JSON items carry the same set for machine consumers.
    let legacy_items = report
        .items
        .iter()
        .filter(|item| item.stale_baseline_warning)
        .collect::<Vec<_>>();
    if !legacy_items.is_empty() {
        out.push_str(&format!(
            "\nLegacy fallback matches ({} total):\n",
            report.delta.legacy_fallback_match
        ));
        for item in legacy_items {
            out.push_str(&format!("- {}\n", item_headline(item)));
            if let Some(legacy) = item.matched_legacy_identity.as_deref() {
                out.push_str(&format!("  Legacy identity: {legacy}\n"));
            }
            match item.canonical_replacement_candidate.as_deref() {
                Some(replacement) => out.push_str(&format!(
                    "  Canonical replacement candidate: {replacement}\n"
                )),
                None => out.push_str(
                    "  Canonical replacement candidate: none; refresh or narrow the baseline identity.\n",
                ),
            }
            out.push_str(
                "  Action: review, then run `ripr baseline update --migrate-legacy-identities` or refresh the entry.\n",
            );
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
    let value = match parse_required_json_input(path, "baseline", json_text) {
        Ok(value) => value,
        Err(warning) => {
            return BaselineParse {
                unavailable: true,
                warnings: vec![warning],
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
    let value = match parse_required_json_input(path, "current gate-decision", json_text) {
        Ok(value) => value,
        Err(warning) => {
            return CurrentParse {
                unavailable: true,
                warnings: vec![warning],
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
        root: string_field(value.get("root")),
    }
}

fn parse_required_json_input(
    path: &str,
    input_name: &str,
    json_text: Result<String, String>,
) -> Result<Value, String> {
    let text = json_text
        .map_err(|error| format!("required {input_name} input {path} is invalid: {error}"))?;
    serde_json::from_str::<Value>(&text)
        .map_err(|error| format!("required {input_name} input {path} is invalid: {error}"))
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
        root: string_field(value.get("root")),
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
    let mut first_ambiguity = None;
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
            if first_ambiguity.is_none() {
                first_ambiguity = Some(MatchResult::Ambiguous {
                    matched_by: method.to_string(),
                    count: matches.len(),
                });
            }
        }
    }
    first_ambiguity.unwrap_or(MatchResult::None)
}

/// The `old-canonical -> new-canonical` pair when a fallback join hides a
/// canonical identity change (issue #1964). `None` unless both sides carry a
/// canonical gap id and they differ.
fn canonical_divergence(baseline: &BaselineRecord, current: &CurrentDecision) -> Option<String> {
    match (
        baseline.identity.canonical_gap_id.as_deref(),
        current.identity.canonical_gap_id.as_deref(),
    ) {
        (Some(old), Some(new)) if old != new => Some(format!("{old} -> {new}")),
        _ => None,
    }
}

/// The `baseline-root -> current-root` pair when a fallback join crosses
/// repository boundaries (issue #1964). `None` when either side omits root
/// (pre-root-preservation baselines stay comparable under the compatibility
/// window) or when both roots agree.
fn foreign_root_join(baseline: &BaselineRecord, current_root: Option<&str>) -> Option<String> {
    match (baseline.root.as_deref(), current_root) {
        (Some(baseline_root), Some(current_root)) => {
            let normalized_baseline = normalize_root_for_comparison(baseline_root);
            let normalized_current = normalize_root_for_comparison(current_root);
            if normalized_baseline != normalized_current {
                Some(format!("{baseline_root} -> {current_root}"))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Normalize repository-root spellings before comparison (issue #1964,
/// review): separators unify, a single leading `./` folds away, and trailing
/// slashes drop, so `.`, `./`, and `.\\` compare equal. Spellings that name
/// the same checkout in genuinely different forms (relative vs absolute) stay
/// distinct: the refusal fails safe toward stale visibility, and the pair is
/// always quoted in the warning so the operator can see why.
pub(crate) fn normalize_root_for_comparison(root: &str) -> String {
    let mut normalized = root.replace('\\', "/");
    while normalized.ends_with('/') && normalized.len() > 1 {
        normalized.pop();
    }
    normalized
        .strip_prefix("./")
        .unwrap_or(&normalized)
        .to_string()
}

/// Attach the legacy-fallback disclosure quartet to a delta item built from a
/// fallback-only join (issue #1964): the match kind, the never-silent warning
/// flag, the retained legacy identity, and the retained canonical replacement
/// candidate (if the current candidate carries one).
fn with_legacy_disclosure(
    mut item: DeltaItem,
    matched_legacy_identity: Option<String>,
    canonical_replacement_candidate: Option<String>,
) -> DeltaItem {
    item.baseline_match_kind = Some(BASELINE_MATCH_KIND_LEGACY_PATH_LINE_CLASS.to_string());
    item.stale_baseline_warning = true;
    item.matched_legacy_identity = matched_legacy_identity;
    item.canonical_replacement_candidate = canonical_replacement_candidate;
    item
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
            baseline_match_kind: None,
            stale_baseline_warning: false,
            matched_legacy_identity: None,
            canonical_replacement_candidate: None,
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
    identity.matched_by = Some(matched_by.clone());
    let item = DeltaItem {
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
        baseline_match_kind: None,
        stale_baseline_warning: false,
        matched_legacy_identity: None,
        canonical_replacement_candidate: None,
    };
    // Every fallback-only join is a reviewable compatibility event (issue
    // #1964): the match kind, the warning flag, the retained legacy identity,
    // and the retained canonical replacement candidate travel on the item.
    if matched_by == "fallback" {
        with_legacy_disclosure(
            item,
            baseline.identity.fallback.clone(),
            current.identity.canonical_gap_id.clone(),
        )
    } else {
        item
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
        baseline_match_kind: None,
        stale_baseline_warning: false,
        matched_legacy_identity: None,
        canonical_replacement_candidate: None,
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
        baseline_match_kind: None,
        stale_baseline_warning: false,
        matched_legacy_identity: None,
        canonical_replacement_candidate: None,
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
        baseline_match_kind: None,
        stale_baseline_warning: false,
        matched_legacy_identity: None,
        canonical_replacement_candidate: None,
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
        baseline_match_kind: None,
        stale_baseline_warning: false,
        matched_legacy_identity: None,
        canonical_replacement_candidate: None,
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
        baseline_match_kind: None,
        stale_baseline_warning: false,
        matched_legacy_identity: None,
        canonical_replacement_candidate: None,
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
        if item.stale_baseline_warning {
            counts.legacy_fallback_match += 1;
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
        "legacy_fallback_match": delta.legacy_fallback_match,
    })
}

fn item_json(item: &DeltaItem, causal_projection: Option<&CausalDeltaArtifact>) -> Value {
    let mut output = json!({
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
    });
    if let Some(projection) = causal_projection
        && let Some(delta) = projection.delta_for(item.identity.canonical_gap_id.as_deref())
        && let Some(object) = output.as_object_mut()
    {
        insert_canonical_delta_fields(object, delta);
    }
    // Additive (issue #1964): present only on legacy fallback-only joins, so
    // canonical-match and baseline-new items render byte-identical to before
    // the disclosure existed. Mirrors the gate-decision `baseline_match_kind`
    // vocabulary (RIPR-SPEC-0014 § Baseline Comparison).
    if item.stale_baseline_warning
        && let Some(object) = output.as_object_mut()
    {
        object.insert(
            "baseline_match_kind".to_string(),
            Value::String(
                item.baseline_match_kind
                    .clone()
                    .unwrap_or_else(|| BASELINE_MATCH_KIND_LEGACY_PATH_LINE_CLASS.to_string()),
            ),
        );
        object.insert("stale_baseline_warning".to_string(), Value::Bool(true));
        object.insert(
            "matched_legacy_identity".to_string(),
            item.matched_legacy_identity
                .clone()
                .map_or(Value::Null, Value::String),
        );
        object.insert(
            "canonical_replacement_candidate".to_string(),
            item.canonical_replacement_candidate
                .clone()
                .map_or(Value::Null, Value::String),
        );
    }
    output
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
    fn baseline_delta_uses_unique_seam_after_shared_canonical_gap_is_ambiguous()
    -> Result<(), String> {
        let baseline = r#"{"schema_version":"0.1","entries":[
          {"identity":{"canonical_gap_id":"gap:shared","seam_id":"seam:a"}},
          {"identity":{"canonical_gap_id":"gap:shared","seam_id":"seam:b"}}
        ]}"#;
        let current = r#"{"schema_version":"0.1","decisions":[
          {"decision":"advisory","canonical_gap_id":"gap:shared","seam_id":"seam:a","static_class":"weakly_gripped","placement":{"path":"src/a.rs","line":1}},
          {"decision":"advisory","canonical_gap_id":"gap:shared","seam_id":"seam:b","static_class":"weakly_gripped","placement":{"path":"src/b.rs","line":2}}
        ]}"#;
        let report = build_baseline_delta_report(BaselineDeltaInput {
            root: ".".to_string(),
            baseline_path: "baseline.json".to_string(),
            current_gate_decision_path: "current.json".to_string(),
            baseline_json: Ok(baseline.to_string()),
            current_gate_decision_json: Ok(current.to_string()),
        });
        let rendered = render_baseline_delta_json(&report)?;
        assert!(rendered.contains("\"still_present\": 2"));
        assert!(rendered.contains("\"stale_baseline_entry\": 0"));
        assert!(rendered.contains("\"resolved\": 0"));
        assert!(rendered.contains("\"new_policy_eligible\": 0"));
        assert!(rendered.contains("\"matched_by\": \"seam_id\""));
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

    #[test]
    fn baseline_delta_treats_diverged_canonical_behind_shared_fallback_as_stale()
    -> Result<(), String> {
        // Issue #1964, fixture 1: same path/line/class, but the canonical gap
        // changed after a source edit. The stale fallback must not make the
        // genuinely new canonical gap look historical.
        let baseline = r#"{
          "schema_version": "0.1",
          "entries": [
            {
              "identity": {
                "canonical_gap_id": "pricing::discount::old_rule",
                "fallback": "src/pricing.rs:88:weakly_gripped"
              },
              "path": "src/pricing.rs",
              "line": 88,
              "static_class": "weakly_gripped"
            }
          ]
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "root": ".",
          "decisions": [
            {
              "decision": "advisory",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/pricing.rs", "line": 88},
              "evidence_record": {
                "canonical_gap_id": "pricing::discount::new_rule"
              },
              "evidence": {"missing_discriminator": "amount > threshold"}
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
        assert!(
            rendered.contains("\"stale_baseline_entry\": 1"),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"new_policy_eligible\": 1"),
            "{rendered}"
        );
        assert!(rendered.contains("\"still_present\": 0"), "{rendered}");
        assert!(
            rendered.contains("\"legacy_fallback_match\": 1"),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"baseline_match_kind\": \"legacy_path_line_class\""),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"stale_baseline_warning\": true"),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"matched_legacy_identity\": \"src/pricing.rs:88:weakly_gripped\""),
            "{rendered}"
        );
        assert!(
            rendered
                .contains("\"canonical_replacement_candidate\": \"pricing::discount::new_rule\""),
            "{rendered}"
        );
        assert!(
            rendered.contains("canonical identity diverged"),
            "{rendered}"
        );
        assert!(
            rendered.contains("pricing::discount::old_rule -> pricing::discount::new_rule"),
            "{rendered}"
        );
        let markdown = render_baseline_delta_markdown(&report);
        assert!(
            markdown.contains("Legacy fallback matches (1 total):"),
            "{markdown}"
        );
        assert!(
            markdown.contains("Legacy identity: src/pricing.rs:88:weakly_gripped"),
            "{markdown}"
        );
        assert!(
            markdown.contains("Canonical replacement candidate: pricing::discount::new_rule"),
            "{markdown}"
        );
        Ok(())
    }

    #[test]
    fn baseline_delta_discloses_pure_legacy_match_with_replacement_candidate() -> Result<(), String>
    {
        // Issue #1964, fixture 4: the reviewed entry carries no canonical
        // identity at all, but the current candidate does. The match is
        // preserved (compatibility window) and impossible to miss: match kind,
        // warning flag, retained legacy identity, retained replacement.
        let baseline = r#"{
          "schema_version": "0.1",
          "entries": [
            {
              "identity": {"fallback": "src/legacy.rs:7:weakly_gripped"},
              "path": "src/legacy.rs",
              "line": 7,
              "static_class": "weakly_gripped"
            }
          ]
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "root": ".",
          "decisions": [
            {
              "decision": "advisory",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/legacy.rs", "line": 7},
              "evidence_record": {"canonical_gap_id": "legacy::gap::seven"},
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
        assert!(rendered.contains("\"still_present\": 1"), "{rendered}");
        assert!(
            rendered.contains("\"legacy_fallback_match\": 1"),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"baseline_match_kind\": \"legacy_path_line_class\""),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"stale_baseline_warning\": true"),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"matched_legacy_identity\": \"src/legacy.rs:7:weakly_gripped\""),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"canonical_replacement_candidate\": \"legacy::gap::seven\""),
            "{rendered}"
        );
        assert!(
            rendered.contains("matched current evidence by fallback"),
            "{rendered}"
        );
        Ok(())
    }

    #[test]
    fn baseline_delta_refuses_cross_root_fallback_join_as_stale() -> Result<(), String> {
        // Issue #1964, fixture 5: a legacy baseline recorded for another
        // repository must not launder its history into this repo's delta. The
        // reviewed entry goes stale and the current gap stays new.
        let baseline = r#"{
          "schema_version": "0.1",
          "entries": [
            {
              "identity": {"fallback": "src/foreign.rs:7:weakly_gripped"},
              "path": "src/foreign.rs",
              "line": 7,
              "static_class": "weakly_gripped",
              "root": "/other/repo"
            }
          ]
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "root": ".",
          "decisions": [
            {
              "decision": "advisory",
              "static_class": "weakly_gripped",
              "placement": {"path": "src/foreign.rs", "line": 7},
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
        assert!(
            rendered.contains("\"stale_baseline_entry\": 1"),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"new_policy_eligible\": 1"),
            "{rendered}"
        );
        assert!(rendered.contains("\"still_present\": 0"), "{rendered}");
        assert!(rendered.contains("another repository root"), "{rendered}");
        assert!(rendered.contains("/other/repo -> ."), "{rendered}");
        Ok(())
    }

    #[test]
    fn baseline_delta_keeps_same_root_fallback_match_comparable() -> Result<(), String> {
        // Same-root entries (and pre-root-preservation entries without a root)
        // stay comparable: the foreign-root refusal only fires on an explicit
        // mismatch.
        let baseline = r#"{
          "schema_version": "0.1",
          "entries": [
            {
              "identity": {"fallback": "src/a.rs:1:weakly_gripped"},
              "path": "src/a.rs",
              "line": 1,
              "static_class": "weakly_gripped",
              "root": "."
            },
            {
              "identity": {"fallback": "src/b.rs:2:weakly_gripped"},
              "path": "src/b.rs",
              "line": 2,
              "static_class": "weakly_gripped"
            }
          ]
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "root": ".",
          "decisions": [
            {"decision": "advisory", "static_class": "weakly_gripped", "placement": {"path": "src/a.rs", "line": 1}, "evidence": {}},
            {"decision": "advisory", "static_class": "weakly_gripped", "placement": {"path": "src/b.rs", "line": 2}, "evidence": {}}
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
        assert!(rendered.contains("\"still_present\": 2"), "{rendered}");
        assert!(
            rendered.contains("\"stale_baseline_entry\": 0"),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"legacy_fallback_match\": 2"),
            "{rendered}"
        );
        assert!(!rendered.contains("another repository root"), "{rendered}");
        Ok(())
    }

    #[test]
    fn baseline_delta_marks_ambiguous_fallback_with_retained_legacy_identity() -> Result<(), String>
    {
        // Issue #1964, fixture 3: two gaps on one line with the same class.
        // No single replacement candidate exists, but the legacy identity is
        // retained and the match stays visible.
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
            {"decision": "advisory", "static_class": "weakly_gripped", "placement": {"path": "src/ambiguous.rs", "line": 7}, "evidence": {}},
            {"decision": "advisory", "static_class": "weakly_gripped", "placement": {"path": "src/ambiguous.rs", "line": 7}, "evidence": {}}
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
        assert!(
            rendered.contains("\"stale_baseline_entry\": 1"),
            "{rendered}"
        );
        assert!(
            rendered.contains("matched 2 current decisions by fallback"),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"baseline_match_kind\": \"legacy_path_line_class\""),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"stale_baseline_warning\": true"),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"matched_legacy_identity\": \"src/ambiguous.rs:7:weakly_gripped\""),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"canonical_replacement_candidate\": null"),
            "{rendered}"
        );
        Ok(())
    }

    #[test]
    fn baseline_delta_canonical_match_carries_no_legacy_disclosure() -> Result<(), String> {
        // Canonical authority must render exactly as before the disclosure
        // existed: no match-kind key, no warning flag, no legacy fields.
        let baseline = r#"{
          "schema_version": "0.1",
          "entries": [
            {
              "identity": {"canonical_gap_id": "gap::stable"},
              "path": "src/moved.rs",
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
              "static_class": "weakly_gripped",
              "placement": {"path": "src/moved.rs", "line": 44},
              "evidence_record": {"canonical_gap_id": "gap::stable"},
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
        assert!(rendered.contains("\"still_present\": 1"), "{rendered}");
        assert!(
            rendered.contains("\"legacy_fallback_match\": 0"),
            "{rendered}"
        );
        assert!(!rendered.contains("baseline_match_kind"), "{rendered}");
        assert!(!rendered.contains("stale_baseline_warning"), "{rendered}");
        assert!(!rendered.contains("matched_legacy_identity"), "{rendered}");
        assert!(
            !rendered.contains("canonical_replacement_candidate"),
            "{rendered}"
        );
        let markdown = render_baseline_delta_markdown(&report);
        assert!(!markdown.contains("Legacy fallback matches"), "{markdown}");
        Ok(())
    }

    #[test]
    fn baseline_delta_treats_equivalent_root_spellings_as_same_repository() -> Result<(), String> {
        // Issue #1964, review: `.`, `./`, and `.\\` name the same checkout
        // after normalization, so the cross-root refusal must not fire on
        // spelling alone.
        let baseline = r#"{
          "schema_version": "0.1",
          "entries": [
            {
              "identity": {"fallback": "src/a.rs:1:weakly_gripped"},
              "path": "src/a.rs",
              "line": 1,
              "static_class": "weakly_gripped",
              "root": "./"
            }
          ]
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "root": ".",
          "decisions": [
            {"decision": "advisory", "static_class": "weakly_gripped", "placement": {"path": "src/a.rs", "line": 1}, "evidence": {}}
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
        assert!(rendered.contains("\"still_present\": 1"), "{rendered}");
        assert!(
            rendered.contains("\"stale_baseline_entry\": 0"),
            "{rendered}"
        );
        assert!(!rendered.contains("another repository root"), "{rendered}");
        Ok(())
    }

    #[test]
    fn baseline_delta_warns_on_already_joined_fallback_match() -> Result<(), String> {
        // Issue #1964, review: the second entry sharing one fallback identity
        // is counted as a legacy match AND warned, so warning consumers see
        // the same compatibility events the counter reports.
        let baseline = r#"{
          "schema_version": "0.1",
          "entries": [
            {
              "identity": {"fallback": "src/dup.rs:7:weakly_gripped"},
              "path": "src/dup_a.rs",
              "line": 7,
              "static_class": "weakly_gripped"
            },
            {
              "identity": {"fallback": "src/dup.rs:7:weakly_gripped"},
              "path": "src/dup_b.rs",
              "line": 8,
              "static_class": "weakly_gripped"
            }
          ]
        }"#;
        let current = r#"{
          "schema_version": "0.1",
          "decisions": [
            {"decision": "advisory", "static_class": "weakly_gripped", "placement": {"path": "src/dup.rs", "line": 7}, "evidence": {}}
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
        assert!(rendered.contains("\"still_present\": 1"), "{rendered}");
        assert!(
            rendered.contains("\"stale_baseline_entry\": 1"),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"legacy_fallback_match\": 2"),
            "{rendered}"
        );
        assert!(
            rendered.contains("also matched an already joined current decision by fallback"),
            "{rendered}"
        );
        assert!(
            rendered.contains("\"baseline_match_kind\": \"legacy_path_line_class\""),
            "{rendered}"
        );
        Ok(())
    }

    #[test]
    fn baseline_delta_markdown_lists_every_legacy_match() -> Result<(), String> {
        // Issue #1964, review: the human compatibility section is exhaustive.
        // Twelve legacy joins list twelve headlines, not ten plus silence.
        let mut entries = Vec::new();
        let mut decisions = Vec::new();
        for index in 0..12 {
            entries.push(format!(
                "{{\"identity\": {{\"fallback\": \"src/legacy{index}.rs:7:weakly_gripped\"}}, \"path\": \"src/legacy{index}.rs\", \"line\": 7, \"static_class\": \"weakly_gripped\"}}"
            ));
            decisions.push(format!(
                "{{\"decision\": \"advisory\", \"static_class\": \"weakly_gripped\", \"placement\": {{\"path\": \"src/legacy{index}.rs\", \"line\": 7}}, \"evidence\": {{}}}}"
            ));
        }
        let baseline = format!(
            "{{\"schema_version\": \"0.1\", \"entries\": [{}]}}",
            entries.join(",")
        );
        let current = format!(
            "{{\"schema_version\": \"0.1\", \"decisions\": [{}]}}",
            decisions.join(",")
        );

        let report = build_baseline_delta_report(BaselineDeltaInput {
            root: ".".to_string(),
            baseline_path: "baseline.json".to_string(),
            current_gate_decision_path: "current.json".to_string(),
            baseline_json: Ok(baseline),
            current_gate_decision_json: Ok(current),
        });
        let rendered = render_baseline_delta_json(&report)?;
        assert!(rendered.contains("\"still_present\": 12"), "{rendered}");
        assert!(
            rendered.contains("\"legacy_fallback_match\": 12"),
            "{rendered}"
        );
        let markdown = render_baseline_delta_markdown(&report);
        assert!(
            markdown.contains("Legacy fallback matches (12 total):"),
            "{markdown}"
        );
        for index in 0..12 {
            assert!(
                markdown.contains(&format!("src/legacy{index}.rs:7")),
                "{markdown}"
            );
        }
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
