//! Vocabulary, row contract, and derivation from immutable judgment labels.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use serde::Deserialize;
use serde_json::Value;

pub(crate) const FAILURE_DIRECTIONS: [&str; 8] = [
    "false_actionable",
    "false_exposed",
    "static_under_credit",
    "wrong_target",
    "incorrect_limitation",
    "instrument_or_case_defect",
    "inconclusive_no_feedback",
    "no_confirmed_failure",
];

pub(crate) const ANALYZER_DEFECTS: [&str; 5] = [
    "false_actionable",
    "false_exposed",
    "static_under_credit",
    "wrong_target",
    "incorrect_limitation",
];

pub(crate) const STATUSES: [&str; 6] = [
    "open",
    "candidate_in_review",
    "repaired_pending_replay",
    "closed_with_replay",
    "accepted_limitation",
    "no_repair_required",
];

pub(crate) const REDUCTIONS: [&str; 4] =
    ["fixture_backed", "replay_only", "unreduced", "not_a_defect"];

const CALIBRATION_STATUSES: [&str; 3] = ["not_run", "unavailable", "recorded"];
/// Row-level calibration may only carry absent metadata; `recorded` results
/// belong to the #4795 producer even when the vocabulary names them.
const ROW_CALIBRATION_STATUSES: [&str; 2] = ["not_run", "unavailable"];

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CalibrationMeta {
    pub(crate) status: String,
    pub(crate) authority: String,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FeedbackRow {
    pub(crate) feedback_id: String,
    pub(crate) case_id: String,
    pub(crate) source: SourceIdentities,
    pub(crate) behavior_identity: String,
    pub(crate) terminal_judgment: String,
    pub(crate) runtime_calibration: RowCalibration,
    pub(crate) failure_direction: String,
    pub(crate) mechanism: String,
    pub(crate) analyzer_family: String,
    pub(crate) semantic_owner: String,
    pub(crate) reduction: Reduction,
    #[serde(default)]
    pub(crate) target_identity: Option<String>,
    pub(crate) owner: OwnerRecord,
    pub(crate) status: String,
    pub(crate) repair: RepairRecord,
    pub(crate) non_claims: Vec<String>,
    #[serde(default)]
    pub(crate) notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceIdentities {
    pub(crate) panel: String,
    pub(crate) judgments: String,
    pub(crate) calibration: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RowCalibration {
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) result: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Reduction {
    pub(crate) disposition: String,
    #[serde(default)]
    pub(crate) fixture_id: Option<String>,
    #[serde(default)]
    pub(crate) expected_class: Option<String>,
    pub(crate) reason: String,
    #[serde(default)]
    pub(crate) materialization_boundary: Option<String>,
    #[serde(default)]
    pub(crate) positive_control: Option<String>,
    #[serde(default)]
    pub(crate) negative_control: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OwnerRecord {
    #[serde(default)]
    pub(crate) existing: Option<String>,
    pub(crate) designated: String,
    pub(crate) search_receipt: String,
    #[serde(default)]
    pub(crate) competing: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepairRecord {
    #[serde(default)]
    pub(crate) merged_implementation: Option<String>,
    #[serde(default)]
    pub(crate) original_case_replay: bool,
    #[serde(default)]
    pub(crate) replay_identities: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct JudgmentFact {
    pub(crate) case_id: String,
    pub(crate) expected_direction: String,
    pub(crate) terminal: String,
    pub(crate) owner: String,
    pub(crate) observer: String,
    pub(crate) false_actionable: Option<bool>,
    pub(crate) false_exposed: Option<bool>,
    pub(crate) under_credit: Option<bool>,
    pub(crate) limitation_correct: Option<bool>,
}

pub(crate) fn judgment_facts(value: &Value) -> Result<Vec<JudgmentFact>, String> {
    let rows = value
        .get("judgments")
        .and_then(Value::as_array)
        .ok_or_else(|| "release judgments carry no `judgments` array".to_string())?;
    let mut facts = Vec::new();
    for row in rows {
        let case_id = required_str(row, "case_id")?;
        let expected_direction = required_str(row, "expected_direction")?;
        let terminal = required_str(row, "terminal")?;
        let outcome = row
            .get("reference_outcome")
            .ok_or_else(|| format!("{case_id}: missing reference_outcome"))?;
        let structural = row
            .get("structural")
            .ok_or_else(|| format!("{case_id}: missing structural"))?;
        facts.push(JudgmentFact {
            case_id,
            expected_direction,
            terminal,
            owner: required_str(structural, "owner")?,
            observer: required_str(structural, "observer")?,
            false_actionable: bool_or_null(outcome, "false_actionable")?,
            false_exposed: bool_or_null(outcome, "false_exposed")?,
            under_credit: bool_or_null(outcome, "under_credit")?,
            limitation_correct: bool_or_null(outcome, "limitation_correct")?,
        });
    }
    Ok(facts)
}

pub(crate) fn derived_failure_direction(fact: &JudgmentFact, claimed: &str) -> &'static str {
    if fact.false_exposed == Some(true) {
        return "false_exposed";
    }
    if fact.false_actionable == Some(true) {
        return "false_actionable";
    }
    if fact.limitation_correct == Some(false) {
        return "incorrect_limitation";
    }
    if fact.under_credit == Some(true) {
        return "static_under_credit";
    }
    if fact.terminal.starts_with("inconclusive_") {
        return "inconclusive_no_feedback";
    }
    if fact.terminal == "invalid_case_identity" {
        return "instrument_or_case_defect";
    }
    if claimed == "wrong_target" {
        return "wrong_target";
    }
    "no_confirmed_failure"
}

pub(crate) fn is_analyzer_defect(direction: &str) -> bool {
    ANALYZER_DEFECTS.contains(&direction)
}

pub(crate) fn validate_calibration(meta: &CalibrationMeta) -> Vec<String> {
    let mut violations = Vec::new();
    if !CALIBRATION_STATUSES.contains(&meta.status.as_str()) {
        violations.push(format!("calibration.status: unknown `{}`", meta.status));
    }
    if meta.authority != "EffortlessMetrics/ripr-swarm#4795" {
        violations.push("calibration.authority: expected EffortlessMetrics/ripr-swarm#4795".into());
    }
    if meta.reason.trim().is_empty() {
        violations.push("calibration.reason: required when calibration is not absorbed".into());
    }
    if meta.status == "recorded" {
        violations.push(
            "calibration.status: recorded runtime results belong to #4795 and cannot land here"
                .into(),
        );
    }
    violations
}

pub(crate) fn validate_coverage(rows: &[FeedbackRow], facts: &[JudgmentFact]) -> Vec<String> {
    let mut violations = Vec::new();
    let mut seen = BTreeSet::new();
    for row in rows {
        if !seen.insert(&row.case_id) {
            violations.push(format!(
                "{}: duplicate feedback row for the same judged case",
                row.case_id
            ));
        }
    }
    let fact_ids: BTreeSet<&str> = facts.iter().map(|fact| fact.case_id.as_str()).collect();
    let row_ids: BTreeSet<&str> = rows.iter().map(|row| row.case_id.as_str()).collect();
    for missing in fact_ids.difference(&row_ids) {
        violations.push(format!(
            "{missing}: terminal judged case has no feedback disposition"
        ));
    }
    for extra in row_ids.difference(&fact_ids) {
        violations.push(format!(
            "{extra}: feedback row is not a terminal judged case"
        ));
    }
    violations
}

pub(crate) fn validate_order(rows: &[FeedbackRow]) -> Vec<String> {
    let mut ordered = rows
        .iter()
        .map(|row| row.case_id.as_str())
        .collect::<Vec<_>>();
    let original = ordered.clone();
    ordered.sort_unstable();
    if original == ordered {
        Vec::new()
    } else {
        vec!["rows: must be sorted by case_id for byte-stable reports".into()]
    }
}

pub(crate) fn validate_row(row: &FeedbackRow, fact: &JudgmentFact, root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    if row.case_id != fact.case_id {
        violations.push(format!(
            "{}: row case_id does not match bound judgment",
            row.case_id
        ));
    }
    if row.terminal_judgment != fact.terminal {
        violations.push(format!(
            "{}: terminal_judgment `{}` rewrites immutable judgment `{}`",
            row.case_id, row.terminal_judgment, fact.terminal
        ));
    }
    if !FAILURE_DIRECTIONS.contains(&row.failure_direction.as_str()) {
        violations.push(format!(
            "{}: unknown failure_direction `{}`",
            row.case_id, row.failure_direction
        ));
    }
    let derived = derived_failure_direction(fact, &row.failure_direction);
    if row.failure_direction != derived {
        violations.push(format!(
            "{}: failure_direction `{}` disagrees with immutable labels (derived `{derived}`); notes cannot strengthen this",
            row.case_id, row.failure_direction
        ));
    }
    if !STATUSES.contains(&row.status.as_str()) {
        violations.push(format!("{}: unknown status `{}`", row.case_id, row.status));
    }
    if !REDUCTIONS.contains(&row.reduction.disposition.as_str()) {
        violations.push(format!(
            "{}: unknown reduction `{}`",
            row.case_id, row.reduction.disposition
        ));
    }
    if row.source.judgments != "EffortlessMetrics/ripr-swarm#3806" {
        violations.push(format!(
            "{}: source.judgments must remain the #3806 packet",
            row.case_id
        ));
    }
    if row.source.calibration != "EffortlessMetrics/ripr-swarm#4795" {
        violations.push(format!(
            "{}: source.calibration must remain the #4795 owner even when not_run",
            row.case_id
        ));
    }
    if let Some(result) = &row.runtime_calibration.result
        && !result.trim().is_empty()
    {
        violations.push(format!(
            "{}: runtime calibration result belongs to #4795 and cannot land in this ledger",
            row.case_id
        ));
    }
    if !ROW_CALIBRATION_STATUSES.contains(&row.runtime_calibration.status.as_str()) {
        violations.push(format!(
            "{}: runtime_calibration.status `{}` is outside the ledger vocabulary {ROW_CALIBRATION_STATUSES:?}; recorded results belong to #4795, not this ledger",
            row.case_id, row.runtime_calibration.status
        ));
    }
    violations.extend(validate_reduction(row, derived, root));
    violations.extend(validate_status(row, derived, fact));
    if derived == "wrong_target" {
        match row.target_identity.as_deref() {
            None | Some("") => violations.push(format!(
                "{}: wrong_target requires an exact target_identity",
                row.case_id
            )),
            Some(target) => {
                // The full qualified identity must match: the judged owner
                // path plus the adjudicated observer symbol. Stripping the
                // path would let a nearby module claim the same observer.
                let owner_path = fact.owner.split(':').next().unwrap_or("");
                let qualified = format!("{owner_path}::{}", fact.observer);
                if target != qualified {
                    violations.push(format!(
                        "{}: wrong_target target_identity `{target}` is not the adjudicated observer identity `{qualified}` of the frozen judgment; a ledger-authored or nearby target cannot create the defect",
                        row.case_id
                    ));
                }
            }
        }
    }
    for (field, value) in [
        ("feedback_id", row.feedback_id.as_str()),
        ("behavior_identity", row.behavior_identity.as_str()),
        ("mechanism", row.mechanism.as_str()),
        ("analyzer_family", row.analyzer_family.as_str()),
        ("semantic_owner", row.semantic_owner.as_str()),
        ("source.panel", row.source.panel.as_str()),
        ("reduction.reason", row.reduction.reason.as_str()),
    ] {
        if value.trim().is_empty() {
            violations.push(format!("{}: `{field}` is required", row.case_id));
        }
    }
    if row.non_claims.is_empty() || row.non_claims.iter().all(|item| item.trim().is_empty()) {
        violations.push(format!(
            "{}: non_claims must name at least one boundary",
            row.case_id
        ));
    }
    violations
}

fn validate_reduction(row: &FeedbackRow, derived: &str, root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    let defect = is_analyzer_defect(derived);
    match row.reduction.disposition.as_str() {
        "not_a_defect" if defect => violations.push(format!(
            "{}: confirmed analyzer defect cannot use reduction `not_a_defect`",
            row.case_id
        )),
        "fixture_backed" | "replay_only" | "unreduced" if !defect => violations.push(format!(
            "{}: non-defect row cannot claim a defect reduction",
            row.case_id
        )),
        "fixture_backed" => {
            let fixture_id = row.reduction.fixture_id.as_deref().unwrap_or("");
            if fixture_id.is_empty() {
                violations.push(format!(
                    "{}: fixture_backed reduction needs a fixture identity",
                    row.case_id
                ));
            } else {
                let relative = Path::new(fixture_id);
                if relative.is_absolute()
                    || relative
                        .components()
                        .any(|component| component == Component::ParentDir)
                {
                    violations.push(format!(
                        "{}: fixture identity `{fixture_id}` must be a repository-relative path",
                        row.case_id
                    ));
                } else if !root.join(relative).is_file() {
                    violations.push(format!(
                        "{}: fixture_backed reduction names fixture `{fixture_id}`, which does not resolve to a repository file; a fixture claim requires a real producer path",
                        row.case_id
                    ));
                }
            }
            if row
                .reduction
                .positive_control
                .as_deref()
                .unwrap_or("")
                .is_empty()
                || row
                    .reduction
                    .negative_control
                    .as_deref()
                    .unwrap_or("")
                    .is_empty()
            {
                violations.push(format!(
                    "{}: fixture_backed reduction needs positive and negative controls",
                    row.case_id
                ));
            }
        }
        "replay_only" | "unreduced"
            if row
                .reduction
                .materialization_boundary
                .as_deref()
                .unwrap_or("")
                .is_empty() =>
        {
            violations.push(format!(
                "{}: replay-only/unreduced rows need a named materialization/authorization boundary",
                row.case_id
            ));
        }
        _ => {}
    }
    if let Some(fixture_id) = &row.reduction.fixture_id
        && (fixture_id == &row.case_id || fixture_id == &row.feedback_id)
    {
        violations.push(format!(
            "{}: fixture identity hard-coded to case/feedback id",
            row.case_id
        ));
    }
    if row.reduction.expected_class.is_some() {
        violations.push(format!(
            "{}: expected_class shortcut is forbidden; fixtures must not hard-code the judged class",
            row.case_id
        ));
    }
    violations
}

fn validate_status(row: &FeedbackRow, derived: &str, fact: &JudgmentFact) -> Vec<String> {
    let mut violations = Vec::new();
    let defect = is_analyzer_defect(derived);
    match row.status.as_str() {
        "no_repair_required" if defect => violations.push(format!(
            "{}: analyzer defect cannot be `no_repair_required`",
            row.case_id
        )),
        "accepted_limitation" => {
            if defect {
                violations.push(format!(
                    "{}: analyzer defect cannot close as `accepted_limitation` without a limitation_correct judgment",
                    row.case_id
                ));
            } else if fact.limitation_correct != Some(true) {
                violations.push(format!(
                    "{}: `accepted_limitation` requires a correct-limitation judgment; an unjudged or inconclusive limitation cannot be presented as accepted",
                    row.case_id
                ));
            }
        }
        "open" | "candidate_in_review" | "repaired_pending_replay" | "closed_with_replay"
            if !defect =>
        {
            violations.push(format!(
                "{}: non-defect row cannot enter the repair lifecycle (`{}`)",
                row.case_id, row.status
            ));
        }
        _ => {}
    }
    violations
}

fn required_str(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("judgment row missing `{key}`"))
}

fn bool_or_null(value: &Value, key: &str) -> Result<Option<bool>, String> {
    match value.get(key) {
        None => Err(format!("reference_outcome missing `{key}`")),
        Some(Value::Null) => Ok(None),
        Some(Value::Bool(flag)) => Ok(Some(*flag)),
        Some(other) => Err(format!(
            "reference_outcome.{key} is not bool-or-null: {other}"
        )),
    }
}

pub(crate) fn count_by_direction(rows: &[FeedbackRow]) -> BTreeMap<String, usize> {
    tally(
        rows.iter().map(|row| row.failure_direction.clone()),
        &FAILURE_DIRECTIONS,
    )
}

pub(crate) fn count_by_status(rows: &[FeedbackRow]) -> BTreeMap<String, usize> {
    tally(rows.iter().map(|row| row.status.clone()), &STATUSES)
}

pub(crate) fn count_by_reduction(rows: &[FeedbackRow]) -> BTreeMap<String, usize> {
    tally(
        rows.iter().map(|row| row.reduction.disposition.clone()),
        &REDUCTIONS,
    )
}

pub(crate) fn count_by_family(rows: &[FeedbackRow]) -> BTreeMap<String, usize> {
    tally(rows.iter().map(|row| row.analyzer_family.clone()), &[])
}

pub(crate) fn count_by_semantic_owner(rows: &[FeedbackRow]) -> BTreeMap<String, usize> {
    tally(rows.iter().map(|row| row.semantic_owner.clone()), &[])
}

pub(crate) fn count_new_owners(rows: &[FeedbackRow]) -> usize {
    rows.iter()
        .filter(|row| {
            is_analyzer_defect(&row.failure_direction)
                && row.owner.existing.is_none()
                && row.owner.designated != "unowned_no_github_mutation"
                && row.owner.designated != "none"
        })
        .count()
}

fn tally(values: impl IntoIterator<Item = String>, known: &[&str]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for key in known {
        counts.insert((*key).to_string(), 0);
    }
    for value in values {
        *counts.entry(value).or_insert(0) += 1;
    }
    counts
}
