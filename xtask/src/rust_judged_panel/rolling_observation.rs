//! Rolling observation of production classification and canonical
//! actionability on the retained Rust judged panel (#4578).
//!
//! This is an adapter over the existing seed, portable packets, and #3806
//! release judgments. It does not create a second panel or corpus, does not
//! rewrite frozen release artifacts, and does not infer actionability from a
//! class name, a packet-shaped expected label, or a blocked/empty ledger.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::release_judgments::RELEASE_JUDGMENTS_PATH;
use super::{
    MANIFEST_PATH, RELEASE_SELECTION_PATH, RustJudgedPanelItem, RustJudgedPanelManifest,
    parse_json_without_duplicate_keys,
};

pub(super) const ROLLING_OBSERVATION_PATH: &str =
    "metrics/rust-judged-behavior-panel/rolling-observation.json";
const RERUN_COMMAND: &str = "cargo xtask rust-judged-panel check";
const KIND: &str = "rust_judged_panel_rolling_observation";
const AUTHORITY: &str = "EffortlessMetrics/ripr-swarm#4578";
const PORTABLE_CURRENT_PATH: &str = "metrics/rust-judged-behavior-panel/portable/current.json";

const STRATUM_PRODUCTION_QUIET: &str = "production_quiet";
const STRATUM_PRODUCTION_GAP: &str = "production_gap";
const STRATUM_PRODUCTION_LIMIT: &str = "production_limit";
const STRATUM_TEST_ONLY_QUIET: &str = "test_only_quiet";

const CLASS_SOURCE_CHECK_JSON: &str = "check_json_findings";
const ACTION_SOURCE_LEDGER: &str = "canonical_gap_decision_ledger";
const ACTION_SOURCE_BLOCKED: &str = "blocked_ledger";
const ACTION_SOURCE_MISSING_LEDGER: &str = "missing_ledger";
const ACTION_SOURCE_MISSING_ADAPTER: &str = "missing_adapter";
const ACTION_SOURCE_CHECK_JSON: &str = "check_json_lacks_repair_decision";
const ACTION_SOURCE_MANIFEST: &str = "governed_manifest_is_not_canonical_producer";

const STATUS_OBSERVED: &str = "observed";
const STATUS_NOT_OBSERVED: &str = "not_observed";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RollingObservation {
    schema_version: String,
    kind: String,
    authority: String,
    inherited_authorities: Vec<String>,
    seed_manifest_path: String,
    seed_manifest_sha256: String,
    release_judgments_path: String,
    release_judgments_sha256: String,
    portable_current_path: String,
    portable_generation_id: String,
    limits: Vec<String>,
    coverage: CoverageInventory,
    unmet: Vec<UnmetRow>,
    observations: Vec<CaseObservation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoverageInventory {
    production_quiet: CoverageRow,
    production_gap: CoverageRow,
    production_limit: CoverageRow,
    test_only_quiet: CoverageRow,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoverageRow {
    status: String,
    case_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnmetRow {
    row: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseObservation {
    case_id: String,
    inherited_from: String,
    coverage_stratum: String,
    production_behavior: bool,
    classification: AxisObservation,
    actionability: AxisObservation,
    false_actionable: Option<bool>,
    disposition: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AxisObservation {
    status: String,
    source: String,
    value: Option<String>,
    cause: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CaseFacts {
    pub(super) case_id: String,
    pub(super) expected_direction: String,
    pub(super) behavior_family: String,
    pub(super) production_behavior: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ClassificationObservation {
    pub(super) status: &'static str,
    pub(super) source: &'static str,
    pub(super) class: Option<String>,
    pub(super) cause: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ActionabilityObservation {
    pub(super) status: &'static str,
    pub(super) source: &'static str,
    pub(super) decision: Option<String>,
    pub(super) cause: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ClassificationInput<'a> {
    #[cfg(test)]
    MissingAdapter,
    CheckJson {
        report: &'a Value,
        production_behavior: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ActionabilityInput {
    #[cfg(test)]
    MissingAdapter,
    MissingLedger,
    ManifestExpectation,
    #[cfg(test)]
    CanonicalLedger(Value),
}

pub(super) fn validate_at(root: &Path, seed: &RustJudgedPanelManifest) -> Result<(), String> {
    let body = fs::read_to_string(root.join(ROLLING_OBSERVATION_PATH)).map_err(|error| {
        format!("read rolling observation `{ROLLING_OBSERVATION_PATH}`: {error}")
    })?;
    let value = parse_json_without_duplicate_keys(&body).map_err(|error| {
        format!("parse rolling observation `{ROLLING_OBSERVATION_PATH}`: {error}")
    })?;
    let packet: RollingObservation = serde_json::from_value(value).map_err(|error| {
        format!("parse rolling observation `{ROLLING_OBSERVATION_PATH}`: {error}")
    })?;
    let mut violations = validate_packet(root, seed, &packet);
    violations.sort();
    violations.dedup();
    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Rust judged-panel rolling observation `{ROLLING_OBSERVATION_PATH}` has {} violation(s):\n- {}\nrerun: {RERUN_COMMAND}",
            violations.len(),
            violations.join("\n- ")
        ))
    }
}

pub(super) fn coverage_stratum(facts: &CaseFacts) -> Result<&'static str, String> {
    let test_only = is_test_only(facts);
    match facts.expected_direction.as_str() {
        "should_stay_quiet" if test_only => Ok(STRATUM_TEST_ONLY_QUIET),
        "should_stay_quiet" if facts.production_behavior => Ok(STRATUM_PRODUCTION_QUIET),
        "should_gap" if facts.production_behavior && !test_only => Ok(STRATUM_PRODUCTION_GAP),
        "should_limit" if facts.production_behavior && !test_only => Ok(STRATUM_PRODUCTION_LIMIT),
        other => Err(format!(
            "{}: cannot assign a rolling coverage stratum for direction `{other}` production={} test_only={test_only}",
            facts.case_id, facts.production_behavior
        )),
    }
}

pub(super) fn observe_classification(input: ClassificationInput<'_>) -> ClassificationObservation {
    match input {
        #[cfg(test)]
        ClassificationInput::MissingAdapter => ClassificationObservation {
            status: STATUS_NOT_OBSERVED,
            source: "missing_adapter",
            class: None,
            cause: Some("missing_adapter"),
        },
        ClassificationInput::CheckJson {
            report,
            production_behavior: _,
        } => classify_from_check_json(report),
    }
}

pub(super) fn observe_actionability(input: ActionabilityInput) -> ActionabilityObservation {
    match input {
        #[cfg(test)]
        ActionabilityInput::MissingAdapter => ActionabilityObservation {
            status: STATUS_NOT_OBSERVED,
            source: ACTION_SOURCE_MISSING_ADAPTER,
            decision: None,
            cause: Some("missing_adapter"),
        },
        ActionabilityInput::MissingLedger => ActionabilityObservation {
            status: STATUS_NOT_OBSERVED,
            source: ACTION_SOURCE_MISSING_LEDGER,
            decision: None,
            cause: Some("missing_ledger"),
        },
        ActionabilityInput::ManifestExpectation => ActionabilityObservation {
            status: STATUS_NOT_OBSERVED,
            source: ACTION_SOURCE_MANIFEST,
            decision: None,
            cause: Some("governed_manifest_is_not_canonical_producer"),
        },
        #[cfg(test)]
        ActionabilityInput::CanonicalLedger(ledger) => observe_ledger(&ledger),
    }
}

pub(super) fn test_only_cannot_fill_production_quiet(
    production_quiet_ids: &[String],
    facts: &BTreeMap<String, CaseFacts>,
) -> Vec<String> {
    let mut violations = Vec::new();
    for case_id in production_quiet_ids {
        match facts.get(case_id) {
            None => violations.push(format!(
                "coverage.production_quiet: unknown case `{case_id}`"
            )),
            Some(case) if is_test_only(case) || !case.production_behavior => {
                violations.push(format!(
                    "coverage.production_quiet: `{case_id}` is test-only or non-production and cannot satisfy the production-quiet coverage row"
                ));
            }
            Some(case) => {
                if let Ok(stratum) = coverage_stratum(case)
                    && stratum != STRATUM_PRODUCTION_QUIET
                {
                    violations.push(format!(
                        "coverage.production_quiet: `{case_id}` stratum is `{stratum}`"
                    ));
                }
            }
        }
    }
    violations
}

fn validate_packet(
    root: &Path,
    seed: &RustJudgedPanelManifest,
    packet: &RollingObservation,
) -> Vec<String> {
    let mut violations = Vec::new();
    if packet.schema_version != "0.1" {
        violations.push(format!(
            "schema_version: expected `0.1`, found `{}`",
            packet.schema_version
        ));
    }
    if packet.kind != KIND {
        violations.push(format!("kind: expected `{KIND}`"));
    }
    if packet.authority != AUTHORITY {
        violations.push(format!("authority: expected `{AUTHORITY}`"));
    }
    if !packet
        .inherited_authorities
        .iter()
        .any(|authority| authority == "EffortlessMetrics/ripr-swarm#3164")
        || !packet
            .inherited_authorities
            .iter()
            .any(|authority| authority == "EffortlessMetrics/ripr-swarm#3806")
    {
        violations.push(
            "inherited_authorities: must retain #3164 panel authority and #3806 judgments"
                .to_string(),
        );
    }
    if packet.seed_manifest_path != MANIFEST_PATH {
        violations.push(format!("seed_manifest_path: expected `{MANIFEST_PATH}`"));
    }
    if packet.release_judgments_path != RELEASE_JUDGMENTS_PATH {
        violations.push(format!(
            "release_judgments_path: expected `{RELEASE_JUDGMENTS_PATH}`"
        ));
    }
    if packet.portable_current_path != PORTABLE_CURRENT_PATH {
        violations.push(format!(
            "portable_current_path: expected `{PORTABLE_CURRENT_PATH}`"
        ));
    }
    let seed_digest = sha256_file(&root.join(MANIFEST_PATH));
    match seed_digest {
        Ok(digest) if digest != packet.seed_manifest_sha256 => {
            violations.push(format!(
                "seed_manifest_sha256: packet binds `{}` but `{MANIFEST_PATH}` is `{digest}`",
                packet.seed_manifest_sha256
            ));
        }
        Err(error) => violations.push(error),
        Ok(_) => {}
    }
    let judgments_digest = sha256_file(&root.join(RELEASE_JUDGMENTS_PATH));
    match judgments_digest {
        Ok(digest) if digest != packet.release_judgments_sha256 => {
            violations.push(format!(
                "release_judgments_sha256: packet binds `{}` but `{RELEASE_JUDGMENTS_PATH}` is `{digest}`; rolling evidence must not rewrite #3806",
                packet.release_judgments_sha256
            ));
        }
        Err(error) => violations.push(error),
        Ok(_) => {}
    }
    if packet.limits.is_empty() {
        violations.push("limits: must state the rolling packet's non-claims".to_string());
    }
    if packet.unmet.is_empty() {
        violations.push(
            "unmet: must name the unauthorized real-repository replay row rather than inventing it"
                .to_string(),
        );
    }
    for unmet in &packet.unmet {
        if unmet.row.trim().is_empty() || unmet.reason.trim().is_empty() {
            violations.push("unmet: row and reason must be non-empty".to_string());
        }
        if !unmet.reason.contains("unauthorized") {
            violations.push(format!(
                "unmet.{}: must name the authorization gap instead of inventing a case",
                unmet.row
            ));
        }
    }

    let release_items = match load_release_items(root) {
        Ok(items) => items,
        Err(error) => {
            violations.push(error);
            return violations;
        }
    };
    let mut facts = BTreeMap::new();
    for item in &seed.items {
        facts.insert(item.id.clone(), facts_from_item(item, true));
    }
    for item in &release_items {
        facts
            .entry(item.id.clone())
            .or_insert_with(|| facts_from_item(item, false));
    }

    let portable = match load_portable_observed(root, &packet.portable_generation_id) {
        Ok(observed) => observed,
        Err(error) => {
            violations.push(error);
            BTreeMap::new()
        }
    };

    violations.extend(test_only_cannot_fill_production_quiet(
        &packet.coverage.production_quiet.case_ids,
        &facts,
    ));
    violations.extend(require_row(
        "coverage.production_quiet",
        &packet.coverage.production_quiet,
        "covered",
        STRATUM_PRODUCTION_QUIET,
        &facts,
    ));
    violations.extend(require_row(
        "coverage.production_gap",
        &packet.coverage.production_gap,
        "covered",
        STRATUM_PRODUCTION_GAP,
        &facts,
    ));
    violations.extend(require_row(
        "coverage.production_limit",
        &packet.coverage.production_limit,
        "covered",
        STRATUM_PRODUCTION_LIMIT,
        &facts,
    ));
    violations.extend(require_row(
        "coverage.test_only_quiet",
        &packet.coverage.test_only_quiet,
        "control_only",
        STRATUM_TEST_ONLY_QUIET,
        &facts,
    ));

    let seed_ids = seed
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for observation in &packet.observations {
        if !seen.insert(observation.case_id.as_str()) {
            violations.push(format!(
                "observations: duplicate case `{}`",
                observation.case_id
            ));
        }
        violations.extend(validate_observation(
            observation,
            &facts,
            &seed_ids,
            &portable,
            &packet.coverage,
        ));
    }
    for required in packet
        .coverage
        .production_quiet
        .case_ids
        .iter()
        .chain(packet.coverage.production_gap.case_ids.iter())
        .chain(packet.coverage.production_limit.case_ids.iter())
        .chain(packet.coverage.test_only_quiet.case_ids.iter())
    {
        if !seen.contains(required.as_str()) {
            violations.push(format!(
                "observations: coverage case `{required}` has no observation row"
            ));
        }
    }
    violations
}

fn validate_observation(
    observation: &CaseObservation,
    facts: &BTreeMap<String, CaseFacts>,
    seed_ids: &BTreeSet<&str>,
    portable: &BTreeMap<String, PortableObserved>,
    coverage: &CoverageInventory,
) -> Vec<String> {
    let subject = format!("observations ({})", observation.case_id);
    let mut violations = Vec::new();
    let Some(case) = facts.get(&observation.case_id) else {
        violations.push(format!("{subject}: unknown inherited case"));
        return violations;
    };
    let expected_inherited = if seed_ids.contains(observation.case_id.as_str()) {
        "seed"
    } else {
        "release_challenge"
    };
    if observation.inherited_from != expected_inherited {
        violations.push(format!(
            "{subject}.inherited_from: expected `{expected_inherited}`, found `{}`",
            observation.inherited_from
        ));
    }
    match coverage_stratum(case) {
        Ok(stratum) if stratum != observation.coverage_stratum => {
            violations.push(format!(
                "{subject}.coverage_stratum: expected `{stratum}`, found `{}`",
                observation.coverage_stratum
            ));
        }
        Err(error) => violations.push(format!("{subject}: {error}")),
        Ok(_) => {}
    }
    if observation.production_behavior != case.production_behavior {
        violations.push(format!(
            "{subject}.production_behavior: expected {}",
            case.production_behavior
        ));
    }
    if !matches!(
        observation.disposition.as_str(),
        "completed" | "failed" | "unobserved" | "inconclusive"
    ) {
        violations.push(format!(
            "{subject}.disposition: expected completed, failed, unobserved, or inconclusive"
        ));
    }
    if observation.coverage_stratum == STRATUM_PRODUCTION_QUIET
        && coverage
            .test_only_quiet
            .case_ids
            .iter()
            .any(|id| id == &observation.case_id)
    {
        violations.push(format!(
            "{subject}: a test-only quiet case cannot also occupy production_quiet"
        ));
    }
    violations.extend(validate_classification_axis(
        &subject,
        observation,
        portable.get(&observation.case_id),
    ));
    violations.extend(validate_actionability_axis(&subject, observation, portable));
    violations
}

fn expected_actionability_without_ledger(
    portable: Option<&PortableObserved>,
) -> ActionabilityObservation {
    if portable.is_some_and(|observed| {
        observed.actionability_source == "governed_manifest_subject_contract"
    }) {
        observe_actionability(ActionabilityInput::ManifestExpectation)
    } else {
        observe_actionability(ActionabilityInput::MissingLedger)
    }
}

fn validate_classification_axis(
    subject: &str,
    observation: &CaseObservation,
    portable: Option<&PortableObserved>,
) -> Vec<String> {
    let mut violations = Vec::new();
    let axis = &observation.classification;
    if observation.coverage_stratum == STRATUM_PRODUCTION_QUIET {
        if axis.status != STATUS_OBSERVED || axis.source != CLASS_SOURCE_CHECK_JSON {
            violations.push(format!(
                "{subject}.classification: production quiet requires an observed check-JSON class, not a test-only zero-finding stand-in"
            ));
        }
        if axis.value.as_deref() != Some("exposed") {
            violations.push(format!(
                "{subject}.classification.value: production quiet requires `exposed`"
            ));
        }
    }
    if axis.status == STATUS_OBSERVED {
        if axis
            .value
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
        {
            violations.push(format!(
                "{subject}.classification.value: observed class required"
            ));
        }
        if axis.cause.is_some() {
            violations.push(format!(
                "{subject}.classification.cause: observed class carries no typed absence"
            ));
        }
        if let Some(portable) = portable {
            let report = serde_json::json!({
                "findings": [{ "classification": portable.classification }]
            });
            let expected = observe_classification(ClassificationInput::CheckJson {
                report: &report,
                production_behavior: observation.production_behavior,
            });
            if axis.status != expected.status
                || axis.source != expected.source
                || axis.value != expected.class
            {
                violations.push(format!(
                    "{subject}.classification: check-JSON adapter observed `{}` from `{}`",
                    expected.class.as_deref().unwrap_or("<none>"),
                    expected.source
                ));
            }
        }
    } else if axis.status != STATUS_NOT_OBSERVED {
        violations.push(format!(
            "{subject}.classification.status: expected observed or not_observed"
        ));
    } else if axis.value.is_some() {
        violations.push(format!(
            "{subject}.classification.value: not_observed cannot carry a class"
        ));
    }
    violations
}

fn validate_actionability_axis(
    subject: &str,
    observation: &CaseObservation,
    portable: &BTreeMap<String, PortableObserved>,
) -> Vec<String> {
    let mut violations = Vec::new();
    let axis = &observation.actionability;
    if observation.false_actionable.is_some() && axis.status == STATUS_NOT_OBSERVED {
        violations.push(format!(
            "{subject}.false_actionable: typed absence is not false_actionable=false"
        ));
    }
    match axis.status.as_str() {
        STATUS_OBSERVED => {
            let expected =
                expected_actionability_without_ledger(portable.get(&observation.case_id));
            if expected.status != STATUS_OBSERVED {
                violations.push(format!(
                    "{subject}.actionability: cannot count as observed `{}` without a non-blocked canonical gap-decision ledger (adapter: `{}`)",
                    axis.value.as_deref().unwrap_or("<none>"),
                    expected.source
                ));
            }
            if axis.source != ACTION_SOURCE_LEDGER {
                violations.push(format!(
                    "{subject}.actionability.source: observed actionability must come from `{ACTION_SOURCE_LEDGER}`"
                ));
            }
            if axis
                .value
                .as_ref()
                .is_none_or(|value| value.trim().is_empty())
            {
                violations.push(format!(
                    "{subject}.actionability.value: observed decision required"
                ));
            }
            if matches!(
                axis.source.as_str(),
                ACTION_SOURCE_BLOCKED
                    | ACTION_SOURCE_MISSING_LEDGER
                    | ACTION_SOURCE_MISSING_ADAPTER
            ) {
                violations.push(format!(
                    "{subject}.actionability: blocked, missing, or absent adapters cannot count as a correct no-action result"
                ));
            }
        }
        STATUS_NOT_OBSERVED => {
            if axis.value.is_some() {
                violations.push(format!(
                    "{subject}.actionability.value: not_observed cannot carry a decision, including no_action"
                ));
            }
            let Some(cause) = axis.cause.as_deref() else {
                violations.push(format!(
                    "{subject}.actionability.cause: not_observed requires a precise cause"
                ));
                return violations;
            };
            if !matches!(
                axis.source.as_str(),
                ACTION_SOURCE_BLOCKED
                    | ACTION_SOURCE_MISSING_LEDGER
                    | ACTION_SOURCE_MISSING_ADAPTER
                    | ACTION_SOURCE_CHECK_JSON
                    | ACTION_SOURCE_MANIFEST
            ) {
                violations.push(format!(
                    "{subject}.actionability.source: unsupported typed-absence source `{}`",
                    axis.source
                ));
            }
            if cause == "no_action" || axis.value.as_deref() == Some("no_action") {
                violations.push(format!(
                    "{subject}.actionability: a blocked or missing ledger cannot count as no_action"
                ));
            }
            let expected =
                expected_actionability_without_ledger(portable.get(&observation.case_id));
            if axis.source != expected.source || axis.cause.as_deref() != expected.cause {
                violations.push(format!(
                    "{subject}.actionability: typed absence must match the adapter (`{}` / `{:?}`), found `{}` / `{:?}`",
                    expected.source, expected.cause, axis.source, axis.cause
                ));
            }
        }
        other => violations.push(format!(
            "{subject}.actionability.status: expected observed or not_observed, found `{other}`"
        )),
    }
    violations
}

fn require_row(
    field: &str,
    row: &CoverageRow,
    expected_status: &str,
    expected_stratum: &str,
    facts: &BTreeMap<String, CaseFacts>,
) -> Vec<String> {
    let mut violations = Vec::new();
    if row.status != expected_status {
        violations.push(format!("{field}.status: expected `{expected_status}`"));
    }
    if row.case_ids.is_empty() {
        violations.push(format!("{field}.case_ids: coverage row cannot be empty"));
    }
    for case_id in &row.case_ids {
        match facts.get(case_id) {
            None => violations.push(format!("{field}: unknown case `{case_id}`")),
            Some(case) => match coverage_stratum(case) {
                Ok(stratum) if stratum != expected_stratum => {
                    violations.push(format!(
                        "{field}: `{case_id}` belongs to `{stratum}`, not `{expected_stratum}`"
                    ));
                }
                Err(error) => violations.push(format!("{field}: {error}")),
                Ok(_) => {}
            },
        }
    }
    violations
}

fn classify_from_check_json(report: &Value) -> ClassificationObservation {
    let Some(findings) = report.get("findings").and_then(Value::as_array) else {
        return ClassificationObservation {
            status: STATUS_NOT_OBSERVED,
            source: CLASS_SOURCE_CHECK_JSON,
            class: None,
            cause: Some("check_json_missing_findings"),
        };
    };
    if findings.is_empty() {
        return ClassificationObservation {
            status: STATUS_OBSERVED,
            source: CLASS_SOURCE_CHECK_JSON,
            class: Some("no_findings".to_string()),
            cause: None,
        };
    }
    let classes = findings
        .iter()
        .filter_map(|finding| {
            finding
                .get("classification")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
        .collect::<BTreeSet<_>>();
    if classes.len() == 1 {
        ClassificationObservation {
            status: STATUS_OBSERVED,
            source: CLASS_SOURCE_CHECK_JSON,
            class: classes.into_iter().next(),
            cause: None,
        }
    } else {
        ClassificationObservation {
            status: STATUS_NOT_OBSERVED,
            source: CLASS_SOURCE_CHECK_JSON,
            class: None,
            cause: Some("check_json_classification_ambiguous"),
        }
    }
}

#[cfg(test)]
fn observe_ledger(ledger: &Value) -> ActionabilityObservation {
    let kind = ledger.get("kind").and_then(Value::as_str);
    if kind != Some("gap_decision_ledger") {
        return ActionabilityObservation {
            status: STATUS_NOT_OBSERVED,
            source: ACTION_SOURCE_MISSING_ADAPTER,
            decision: None,
            cause: Some("missing_adapter"),
        };
    }
    let status = ledger.get("status").and_then(Value::as_str).unwrap_or("");
    let records = ledger
        .get("records")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if status == "blocked" || records.is_empty() {
        return ActionabilityObservation {
            status: STATUS_NOT_OBSERVED,
            source: ACTION_SOURCE_BLOCKED,
            decision: None,
            cause: Some("blocked_ledger"),
        };
    }
    let repairable = records
        .iter()
        .any(|record| record.get("repairability").and_then(Value::as_str) == Some("repairable"));
    let no_action = records.iter().any(|record| {
        record.get("repairability").and_then(Value::as_str) == Some("no_action")
            || matches!(
                record.get("kind").and_then(Value::as_str),
                Some("NoActionAlreadyObserved" | "NoActionInternal")
            )
    });
    if repairable && !no_action {
        ActionabilityObservation {
            status: STATUS_OBSERVED,
            source: ACTION_SOURCE_LEDGER,
            decision: Some("repair_candidate".to_string()),
            cause: None,
        }
    } else if no_action && !repairable {
        ActionabilityObservation {
            status: STATUS_OBSERVED,
            source: ACTION_SOURCE_LEDGER,
            decision: Some("no_action".to_string()),
            cause: None,
        }
    } else {
        ActionabilityObservation {
            status: STATUS_NOT_OBSERVED,
            source: ACTION_SOURCE_LEDGER,
            decision: None,
            cause: Some("ledger_decision_ambiguous"),
        }
    }
}

fn is_test_only(facts: &CaseFacts) -> bool {
    facts.behavior_family.contains("test_only")
        || facts.case_id.contains("test-only")
        || !facts.production_behavior
}

fn facts_from_item(item: &RustJudgedPanelItem, seed: bool) -> CaseFacts {
    CaseFacts {
        case_id: item.id.clone(),
        expected_direction: item.expected_direction.clone(),
        behavior_family: item.behavior_family.clone(),
        production_behavior: if seed {
            !item.behavior_family.contains("test_only")
        } else {
            production_behavior_from_release(item)
        },
    }
}

fn production_behavior_from_release(item: &RustJudgedPanelItem) -> bool {
    !item.behavior_family.contains("test_only")
        && !item.id.contains("test-only")
        && item.expected_classification != "no_findings"
}

fn load_release_items(root: &Path) -> Result<Vec<RustJudgedPanelItem>, String> {
    let body = fs::read_to_string(root.join(RELEASE_SELECTION_PATH))
        .map_err(|error| format!("read `{RELEASE_SELECTION_PATH}`: {error}"))?;
    let value = parse_json_without_duplicate_keys(&body)
        .map_err(|error| format!("parse `{RELEASE_SELECTION_PATH}`: {error}"))?;
    let manifest: RustJudgedPanelManifest = serde_json::from_value(value)
        .map_err(|error| format!("parse `{RELEASE_SELECTION_PATH}`: {error}"))?;
    Ok(manifest.items)
}

struct PortableObserved {
    classification: String,
    actionability_source: String,
}

fn load_portable_observed(
    root: &Path,
    expected_generation: &str,
) -> Result<BTreeMap<String, PortableObserved>, String> {
    let current_body = fs::read_to_string(root.join(PORTABLE_CURRENT_PATH))
        .map_err(|error| format!("read `{PORTABLE_CURRENT_PATH}`: {error}"))?;
    let current: Value = parse_json_without_duplicate_keys(&current_body)
        .map_err(|error| format!("parse `{PORTABLE_CURRENT_PATH}`: {error}"))?;
    let generation = current
        .get("generation_id")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{PORTABLE_CURRENT_PATH}: missing generation_id"))?;
    if generation != expected_generation {
        return Err(format!(
            "portable_generation_id: packet binds `{expected_generation}` but current is `{generation}`"
        ));
    }
    let index_path = current
        .get("index_path")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{PORTABLE_CURRENT_PATH}: missing index_path"))?;
    let index_body = fs::read_to_string(root.join(index_path))
        .map_err(|error| format!("read `{index_path}`: {error}"))?;
    let index: Value = parse_json_without_duplicate_keys(&index_body)
        .map_err(|error| format!("parse `{index_path}`: {error}"))?;
    let packets = index
        .get("packets")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{index_path}: missing packets"))?;
    let mut observed = BTreeMap::new();
    for entry in packets {
        let case_id = entry
            .get("case_id")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{index_path}: packet entry missing case_id"))?;
        let packet_path = entry
            .get("packet_path")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{index_path}: `{case_id}` missing packet_path"))?;
        let packet_body = fs::read_to_string(root.join(packet_path))
            .map_err(|error| format!("read `{packet_path}`: {error}"))?;
        let packet: Value = parse_json_without_duplicate_keys(&packet_body)
            .map_err(|error| format!("parse `{packet_path}`: {error}"))?;
        observed.insert(
            case_id.to_string(),
            PortableObserved {
                classification: packet
                    .pointer("/semantic/observed/classification")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                actionability_source: packet
                    .pointer("/semantic/observed/actionability_source")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            },
        );
    }
    Ok(observed)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    fs::read(path)
        .map(|bytes| format!("sha256:{:x}", Sha256::digest(bytes)))
        .map_err(|error| format!("hash `{}`: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::json;

    use super::*;

    fn quiet_production() -> CaseFacts {
        CaseFacts {
            case_id: "seed-quiet".to_string(),
            expected_direction: "should_stay_quiet".to_string(),
            behavior_family: "predicate_boundary".to_string(),
            production_behavior: true,
        }
    }

    fn quiet_test_only() -> CaseFacts {
        CaseFacts {
            case_id: "p1744-quiet-test-only".to_string(),
            expected_direction: "should_stay_quiet".to_string(),
            behavior_family: "test_only_quiet_control".to_string(),
            production_behavior: false,
        }
    }

    fn blocked_ledger() -> Value {
        json!({
            "schema_version": "0.1",
            "kind": "gap_decision_ledger",
            "status": "blocked",
            "summary": { "records_total": 0, "no_action_total": 0, "repairable_total": 0 },
            "records": [],
            "warnings": ["derived ledger blocked with 0 records"]
        })
    }

    #[test]
    fn test_only_quiet_cannot_satisfy_production_quiet_coverage() {
        let facts = BTreeMap::from([
            ("seed-quiet".to_string(), quiet_production()),
            ("p1744-quiet-test-only".to_string(), quiet_test_only()),
        ]);
        let violations =
            test_only_cannot_fill_production_quiet(&["p1744-quiet-test-only".to_string()], &facts);
        assert!(
            violations.iter().any(|violation| {
                violation.contains("p1744-quiet-test-only")
                    && violation.contains("cannot satisfy the production-quiet coverage row")
            }),
            "{violations:?}"
        );
        assert_eq!(
            coverage_stratum(&quiet_test_only()).ok(),
            Some(STRATUM_TEST_ONLY_QUIET)
        );
        assert_eq!(
            coverage_stratum(&quiet_production()).ok(),
            Some(STRATUM_PRODUCTION_QUIET)
        );
    }

    #[test]
    fn blocked_ledger_cannot_count_as_no_action() {
        let observed = observe_actionability(ActionabilityInput::CanonicalLedger(blocked_ledger()));
        assert_eq!(observed.status, STATUS_NOT_OBSERVED);
        assert_eq!(observed.source, ACTION_SOURCE_BLOCKED);
        assert_eq!(observed.decision, None);
        assert_eq!(observed.cause, Some("blocked_ledger"));
    }

    #[test]
    fn empty_ledger_is_blocked_and_is_not_no_action() {
        let ledger = json!({
            "kind": "gap_decision_ledger",
            "status": "advisory",
            "records": []
        });
        let observed = observe_actionability(ActionabilityInput::CanonicalLedger(ledger));
        assert_eq!(observed.status, STATUS_NOT_OBSERVED);
        assert_eq!(observed.source, ACTION_SOURCE_BLOCKED);
        assert_ne!(observed.decision.as_deref(), Some("no_action"));
    }

    #[test]
    fn missing_ledger_cannot_count_as_no_action() {
        let observed = observe_actionability(ActionabilityInput::MissingLedger);
        assert_eq!(observed.status, STATUS_NOT_OBSERVED);
        assert_eq!(observed.source, ACTION_SOURCE_MISSING_LEDGER);
        assert_eq!(observed.decision, None);
        assert_eq!(observed.cause, Some("missing_ledger"));
    }

    #[test]
    fn missing_adapter_is_not_observed_not_false_actionable_false() {
        let classification = observe_classification(ClassificationInput::MissingAdapter);
        let actionability = observe_actionability(ActionabilityInput::MissingAdapter);
        assert_eq!(classification.status, STATUS_NOT_OBSERVED);
        assert_eq!(actionability.status, STATUS_NOT_OBSERVED);
        assert_eq!(actionability.source, ACTION_SOURCE_MISSING_ADAPTER);
        assert_eq!(actionability.decision, None);
        assert_ne!(actionability.cause, Some("false_actionable=false"));
    }

    #[test]
    fn class_name_alone_does_not_observe_actionability() {
        let check = json!({
            "findings": [{ "id": "probe:src_lib.rs:1", "classification": "exposed" }]
        });
        let classification = observe_classification(ClassificationInput::CheckJson {
            report: &check,
            production_behavior: true,
        });
        assert_eq!(classification.class.as_deref(), Some("exposed"));
        let from_class_name = observe_actionability(ActionabilityInput::ManifestExpectation);
        assert_eq!(from_class_name.status, STATUS_NOT_OBSERVED);
        assert_eq!(from_class_name.decision, None);
        assert_eq!(from_class_name.source, ACTION_SOURCE_MANIFEST);
    }

    #[test]
    fn governed_manifest_actionability_is_not_canonical_observation() {
        let observed = observe_actionability(ActionabilityInput::ManifestExpectation);
        assert_eq!(observed.status, STATUS_NOT_OBSERVED);
        assert_eq!(observed.source, ACTION_SOURCE_MANIFEST);
        assert_ne!(observed.decision.as_deref(), Some("no_action"));
    }

    #[test]
    fn canonical_ledger_no_action_record_is_observed() {
        let ledger = json!({
            "kind": "gap_decision_ledger",
            "status": "advisory",
            "records": [{
                "gap_id": "gap:quiet",
                "kind": "NoActionAlreadyObserved",
                "repairability": "no_action"
            }]
        });
        let observed = observe_actionability(ActionabilityInput::CanonicalLedger(ledger));
        assert_eq!(observed.status, STATUS_OBSERVED);
        assert_eq!(observed.source, ACTION_SOURCE_LEDGER);
        assert_eq!(observed.decision.as_deref(), Some("no_action"));
    }

    #[test]
    fn canonical_ledger_repairable_record_is_observed_separately_from_class() {
        let ledger = json!({
            "kind": "gap_decision_ledger",
            "status": "advisory",
            "records": [{
                "gap_id": "gap:missing",
                "kind": "MissingBoundaryAssertion",
                "repairability": "repairable"
            }]
        });
        let observed = observe_actionability(ActionabilityInput::CanonicalLedger(ledger));
        assert_eq!(observed.status, STATUS_OBSERVED);
        assert_eq!(observed.decision.as_deref(), Some("repair_candidate"));
    }

    #[test]
    fn production_exposed_classification_comes_from_check_json() {
        let check = json!({
            "findings": [{
                "id": "probe:src_lib.rs:predicate:00a5a375",
                "classification": "exposed"
            }]
        });
        let observed = observe_classification(ClassificationInput::CheckJson {
            report: &check,
            production_behavior: true,
        });
        assert_eq!(observed.status, STATUS_OBSERVED);
        assert_eq!(observed.source, CLASS_SOURCE_CHECK_JSON);
        assert_eq!(observed.class.as_deref(), Some("exposed"));
    }

    #[test]
    fn zero_findings_check_json_is_not_production_quiet_stratum() {
        let check = json!({ "findings": [] });
        let observed = observe_classification(ClassificationInput::CheckJson {
            report: &check,
            production_behavior: false,
        });
        assert_eq!(observed.class.as_deref(), Some("no_findings"));
        assert_eq!(
            coverage_stratum(&quiet_test_only()).ok(),
            Some(STRATUM_TEST_ONLY_QUIET)
        );
        assert_ne!(
            coverage_stratum(&quiet_test_only()).ok(),
            Some(STRATUM_PRODUCTION_QUIET)
        );
    }

    #[test]
    fn packet_shaped_object_is_not_a_canonical_actionability_producer() {
        let packet_shaped = json!({
            "kind": "rust_judged_panel_portable_packet",
            "semantic": {
                "observed": {
                    "classification": "exposed",
                    "expected_actionability": "no_action",
                    "actionability_source": "governed_manifest_subject_contract"
                }
            }
        });
        let observed = observe_actionability(ActionabilityInput::CanonicalLedger(packet_shaped));
        assert_eq!(observed.status, STATUS_NOT_OBSERVED);
        assert_eq!(observed.source, ACTION_SOURCE_MISSING_ADAPTER);
        assert_ne!(observed.decision.as_deref(), Some("no_action"));
    }

    #[test]
    fn claimed_observed_no_action_without_ledger_is_rejected() {
        let observation = CaseObservation {
            case_id: "seed-quiet".to_string(),
            inherited_from: "seed".to_string(),
            coverage_stratum: STRATUM_PRODUCTION_QUIET.to_string(),
            production_behavior: true,
            classification: AxisObservation {
                status: STATUS_OBSERVED.to_string(),
                source: CLASS_SOURCE_CHECK_JSON.to_string(),
                value: Some("exposed".to_string()),
                cause: None,
            },
            actionability: AxisObservation {
                status: STATUS_OBSERVED.to_string(),
                source: ACTION_SOURCE_LEDGER.to_string(),
                value: Some("no_action".to_string()),
                cause: None,
            },
            false_actionable: Some(false),
            disposition: "completed".to_string(),
        };
        let portable = BTreeMap::from([(
            "seed-quiet".to_string(),
            PortableObserved {
                classification: "exposed".to_string(),
                actionability_source: "governed_manifest_subject_contract".to_string(),
            },
        )]);
        let violations =
            validate_actionability_axis("observations (seed-quiet)", &observation, &portable);
        assert!(
            violations.iter().any(|violation| {
                violation.contains("cannot count as observed `no_action`")
                    && violation.contains(ACTION_SOURCE_MANIFEST)
            }),
            "{violations:?}"
        );
    }

    #[test]
    fn retained_rolling_observation_packet_validates_against_current_panel() -> Result<(), String> {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or_else(|| "xtask manifest has no repository parent".to_string())?;
        let manifest = super::super::load_and_validate_at(
            repository_root,
            Path::new(super::super::MANIFEST_PATH),
        )?;
        validate_at(repository_root, &manifest)
    }
}
