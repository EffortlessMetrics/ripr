//! `cargo xtask blind-journey-execute` (#4604, RIPR-SPEC-0205): runs the
//! committed scripted blind-journey corpus through the deterministic executor
//! in `crate::blind_journey_execute`, validates the committed versioned
//! executor decision receipt against the live result, and writes the
//! human/JSON report. Both projections derive from one evaluated DTO, so
//! prose cannot strengthen machine state. The gate fails closed when a
//! scenario's real executor outcome drifts from its committed expectation or
//! the receipt disagrees.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::blind_journey::BlindJourneyResultV1;
use crate::blind_journey_execute::{
    BLIND_JOURNEY_EXECUTE_CLAIM_BOUNDARY, BLIND_JOURNEY_EXECUTE_CORPUS_SCHEMA_VERSION,
    BLIND_JOURNEY_EXECUTE_DECISION, BLIND_JOURNEY_JOURNEY_SCHEMA_VERSION,
    BlindJourneyExecuteCorpusV1, BlindJourneyExecuteOutcomeV1, execute_blind_journey,
    load_blind_journey_execute_corpus, missing_blind_journey_execute_required_scenarios,
};

const CORPUS_PATH: &str = "fixtures/blind_journey_execute/corpus.json";
const RECEIPT_PATH: &str = "metrics/blind-journey-execute/executor-receipt.json";

#[derive(Clone, Debug, Serialize)]
pub(crate) struct BlindJourneyExecuteRowV1 {
    pub scenario: String,
    pub expected_outcome: BlindJourneyExecuteOutcomeV1,
    pub expected_terminal: Option<BlindJourneyResultV1>,
    pub expected_reason_contains: Option<String>,
    pub observed_outcome: BlindJourneyExecuteOutcomeV1,
    pub observed_terminal: Option<BlindJourneyResultV1>,
    pub refusal_reason: Option<String>,
    /// How far the transcript reached: the scripted action count, so a late
    /// packet rejection is distinguishable from an empty script.
    pub attempted_action_count: usize,
    /// Emitted event count; `0` for a refused journey by definition.
    pub event_count: usize,
    pub portable_identity: Option<String>,
    pub matches_expectation: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct BlindJourneyExecuteReportV1 {
    pub schema_version: String,
    pub claim_boundary: String,
    pub corpus_path: String,
    pub scenario_count: usize,
    pub emitted_count: usize,
    pub positive_count: usize,
    pub refused_count: usize,
    pub expectation_failures: Vec<String>,
    pub scenarios: Vec<BlindJourneyExecuteRowV1>,
}

pub(crate) const BLIND_JOURNEY_EXECUTE_REPORT_SCHEMA_VERSION: &str =
    "blind_journey_execute_report.v1";

fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative)
}

/// Run one parsed corpus through the real executor and evaluate every
/// committed expectation. The returned report is `Ok` even when a scenario
/// mismatches; `expectation_failures` carries the drift so callers decide.
pub(crate) fn assess_blind_journey_execute_corpus(
    corpus: &BlindJourneyExecuteCorpusV1,
) -> BlindJourneyExecuteReportV1 {
    let mut rows = Vec::with_capacity(corpus.scenarios.len());
    let mut failures = Vec::new();
    let mut positive_count = 0usize;
    for scenario in &corpus.scenarios {
        let expected_reason = scenario
            .expected
            .reason_contains
            .clone()
            .unwrap_or_default();
        let (observed_outcome, observed_terminal, refusal_reason, event_count, portable_identity) =
            match execute_blind_journey(&scenario.journey) {
                Ok(packet) => {
                    let assessment = crate::blind_journey::assess_blind_journey_packet(&packet);
                    if assessment.positive() {
                        positive_count += 1;
                    }
                    (
                        BlindJourneyExecuteOutcomeV1::Accepted,
                        Some(packet.receipt.terminal_result),
                        None,
                        packet.receipt.events.len(),
                        Some(assessment.portable_identity),
                    )
                }
                Err(error) => (
                    BlindJourneyExecuteOutcomeV1::Refused,
                    None,
                    Some(error.clone()),
                    0,
                    None,
                ),
            };
        let attempted_action_count = scenario.journey.actions.len();
        let outcome_matches = observed_outcome == scenario.expected.outcome;
        let terminal_matches = scenario
            .expected
            .terminal
            .is_none_or(|expected| observed_terminal == Some(expected));
        let reason_matches = if scenario.expected.outcome == BlindJourneyExecuteOutcomeV1::Refused {
            refusal_reason
                .as_deref()
                .is_some_and(|reason| reason.contains(expected_reason.as_str()))
        } else {
            true
        };
        let matches_expectation = outcome_matches && terminal_matches && reason_matches;
        if !matches_expectation {
            failures.push(format!(
                "scenario `{}` drifted: expected outcome={:?} terminal={:?} \
                 reason_contains={:?}, got outcome={:?} terminal={:?} refusal={:?}",
                scenario.id,
                scenario.expected.outcome,
                scenario.expected.terminal,
                scenario.expected.reason_contains,
                observed_outcome,
                observed_terminal,
                refusal_reason
            ));
        }
        rows.push(BlindJourneyExecuteRowV1 {
            scenario: scenario.id.clone(),
            expected_outcome: scenario.expected.outcome,
            expected_terminal: scenario.expected.terminal,
            expected_reason_contains: scenario.expected.reason_contains.clone(),
            observed_outcome,
            observed_terminal,
            refusal_reason,
            attempted_action_count,
            event_count,
            portable_identity,
            matches_expectation,
        });
    }
    let emitted_count = rows
        .iter()
        .filter(|row| row.observed_outcome == BlindJourneyExecuteOutcomeV1::Accepted)
        .count();
    BlindJourneyExecuteReportV1 {
        schema_version: BLIND_JOURNEY_EXECUTE_REPORT_SCHEMA_VERSION.to_string(),
        claim_boundary: BLIND_JOURNEY_EXECUTE_CLAIM_BOUNDARY.to_string(),
        corpus_path: CORPUS_PATH.to_string(),
        scenario_count: rows.len(),
        emitted_count,
        positive_count,
        refused_count: rows.len() - emitted_count,
        expectation_failures: failures,
        scenarios: rows,
    }
}

fn require_required_scenarios(report: &BlindJourneyExecuteReportV1) -> Result<(), String> {
    let missing = missing_blind_journey_execute_required_scenarios(
        report.scenarios.iter().map(|row| row.scenario.as_str()),
    );
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "blind journey execute corpus is missing required scenarios: {missing:?}"
        ))
    }
}

fn read_json(path: &Path, label: &str) -> Result<Value, String> {
    let body = fs::read_to_string(path)
        .map_err(|error| format!("read {label} {}: {error}", path.display()))?;
    serde_json::from_str(&body)
        .map_err(|error| format!("parse {label} {}: {error}", path.display()))
}

/// The committed executor decision receipt is the versioned consumer record;
/// it must name this consumer's decision and schema identities, stay inside
/// its ratified fixture scope, bind to the assessed corpus, and carry
/// explicit limitations and not-exercised combinations with exact entries.
fn validate_executor_receipt(receipt: &Value, corpus_scenario_count: usize) -> Result<(), String> {
    if receipt.get("kind").and_then(Value::as_str) != Some("blind_journey_execute_decision_receipt")
    {
        return Err("executor receipt carries an unexpected kind".to_string());
    }
    if receipt.get("status").and_then(Value::as_str) != Some("ratified_fixture_scope") {
        return Err("executor receipt status must be ratified_fixture_scope".to_string());
    }
    if receipt.get("decision").and_then(Value::as_str) != Some(BLIND_JOURNEY_EXECUTE_DECISION) {
        return Err(format!(
            "executor receipt must name the {BLIND_JOURNEY_EXECUTE_DECISION} decision"
        ));
    }
    if receipt.get("claim_boundary").and_then(Value::as_str)
        != Some(BLIND_JOURNEY_EXECUTE_CLAIM_BOUNDARY)
    {
        return Err(
            "executor receipt claim_boundary drifted from the consumer claim boundary".to_string(),
        );
    }
    let schema_versions = receipt
        .get("consumer_schema_versions")
        .ok_or_else(|| "executor receipt must pin the consumer schema versions".to_string())?;
    for (key, expected) in [
        ("journey", BLIND_JOURNEY_JOURNEY_SCHEMA_VERSION),
        (
            "execute_corpus",
            BLIND_JOURNEY_EXECUTE_CORPUS_SCHEMA_VERSION,
        ),
        (
            "execute_report",
            BLIND_JOURNEY_EXECUTE_REPORT_SCHEMA_VERSION,
        ),
    ] {
        if schema_versions.get(key).and_then(Value::as_str) != Some(expected) {
            return Err(format!(
                "executor receipt schema version `{key}` drifted from the consumer \
                 constant `{expected}`"
            ));
        }
    }
    if receipt.get("corpus_scenario_count").and_then(Value::as_u64)
        != Some(corpus_scenario_count as u64)
    {
        return Err(
            "executor receipt corpus_scenario_count does not bind the assessed corpus".to_string(),
        );
    }
    for key in ["limitations", "combinations_not_exercised"] {
        let entries = receipt
            .get(key)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("executor receipt must carry a non-empty {key} list"))?;
        if entries.is_empty() {
            return Err(format!("executor receipt must record explicit {key}"));
        }
        if entries
            .iter()
            .any(|entry| entry.as_str().is_none_or(|entry| entry.trim().is_empty()))
        {
            return Err(format!(
                "executor receipt must record exact non-empty {key} entries"
            ));
        }
    }
    Ok(())
}

pub(crate) fn blind_journey_execute_report_value() -> Result<BlindJourneyExecuteReportV1, String> {
    let corpus_body = fs::read_to_string(workspace_path(CORPUS_PATH))
        .map_err(|error| format!("read blind journey execute corpus: {error}"))?;
    let corpus = load_blind_journey_execute_corpus(&corpus_body)?;
    let report = assess_blind_journey_execute_corpus(&corpus);
    require_required_scenarios(&report)?;
    let receipt = read_json(&workspace_path(RECEIPT_PATH), "executor receipt")?;
    validate_executor_receipt(&receipt, report.scenarios.len())?;
    if !report.expectation_failures.is_empty() {
        return Err(format!(
            "blind journey execute corpus drifted: {:?}",
            report.expectation_failures
        ));
    }
    Ok(report)
}

pub(crate) fn blind_journey_execute_report() -> Result<(), String> {
    let report = blind_journey_execute_report_value()?;
    let json_body = blind_journey_execute_report_json(&report)?;
    crate::write_report("blind-journey-execute.json", &json_body)?;
    crate::write_report(
        "blind-journey-execute.md",
        &blind_journey_execute_report_markdown(&report),
    )?;
    println!("{json_body}");
    Ok(())
}

pub(crate) fn blind_journey_execute_report_json(
    report: &BlindJourneyExecuteReportV1,
) -> Result<String, String> {
    let body = serde_json::to_string_pretty(report)
        .map_err(|error| format!("serialize blind journey execute report: {error}"))?;
    Ok(format!("{body}\n"))
}

pub(crate) fn blind_journey_execute_report_markdown(
    report: &BlindJourneyExecuteReportV1,
) -> String {
    let mut body = String::new();
    body.push_str("# Blind journey execute report\n\n");
    body.push_str(&format!("Claim boundary: {}\n\n", report.claim_boundary));
    body.push_str(&format!(
        "- corpus: `{}`
- scenarios: {}
- emitted: {}
- positive: {}
- \
         refused: {}
- expectation failures: {}

",
        report.corpus_path,
        report.scenario_count,
        report.emitted_count,
        report.positive_count,
        report.refused_count,
        report.expectation_failures.len()
    ));
    body.push_str("| scenario | expected | observed | emitted | events |\n");
    body.push_str("| --- | --- | --- | --- | ---: |\n");
    for row in &report.scenarios {
        let expected = match row.expected_terminal {
            Some(terminal) => format!("{:?} / {terminal:?}", row.expected_outcome),
            None => format!("{:?} / refusal", row.expected_outcome),
        };
        let observed = match row.observed_terminal {
            Some(terminal) => format!("{:?}", terminal),
            None => "refused".to_string(),
        };
        body.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            row.scenario,
            expected,
            observed,
            if row.observed_outcome == BlindJourneyExecuteOutcomeV1::Accepted {
                "yes"
            } else {
                "no"
            },
            row.event_count
        ));
    }
    for row in &report.scenarios {
        if let Some(reason) = &row.refusal_reason {
            body.push_str(&format!(
                "\n## {}\n\n- refusal: {reason}\n- attempted actions: {}\n",
                row.scenario, row.attempted_action_count
            ));
        }
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live_report() -> Result<BlindJourneyExecuteReportV1, String> {
        let corpus_body = fs::read_to_string(workspace_path(CORPUS_PATH))
            .map_err(|error| format!("read blind journey execute corpus: {error}"))?;
        let corpus = load_blind_journey_execute_corpus(&corpus_body)?;
        Ok(assess_blind_journey_execute_corpus(&corpus))
    }

    fn committed_receipt() -> Result<Value, String> {
        read_json(&workspace_path(RECEIPT_PATH), "executor receipt")
    }

    #[test]
    fn committed_corpus_and_receipt_validate_against_the_live_executor() -> Result<(), String> {
        let report = live_report()?;
        require_required_scenarios(&report)?;
        validate_executor_receipt(&committed_receipt()?, report.scenarios.len())?;
        if !report.expectation_failures.is_empty() {
            return Err(format!(
                "committed executor corpus drifted from the live executor: {:?}",
                report.expectation_failures
            ));
        }
        // Keep the measured report in the retained CI artifact so the actual
        // scenario outcomes are readable without rerunning the command. Tests
        // run with the xtask crate as CWD, so anchor at the workspace root.
        let reports_dir = workspace_path("target/ripr/reports");
        fs::create_dir_all(&reports_dir)
            .map_err(|error| format!("create {}: {error}", reports_dir.display()))?;
        fs::write(
            reports_dir.join("blind-journey-execute.json"),
            blind_journey_execute_report_json(&report)?,
        )
        .map_err(|error| format!("write blind journey execute JSON report: {error}"))?;
        fs::write(
            reports_dir.join("blind-journey-execute.md"),
            blind_journey_execute_report_markdown(&report),
        )
        .map_err(|error| format!("write blind journey execute Markdown report: {error}"))
    }

    #[test]
    fn receipt_with_a_drifted_schema_version_is_rejected() -> Result<(), String> {
        let mut receipt = committed_receipt()?;
        receipt["consumer_schema_versions"]["journey"] =
            serde_json::json!("blind_journey_journey.v2");
        match validate_executor_receipt(&receipt, live_report()?.scenarios.len()) {
            Err(message) if message.contains("schema version") => Ok(()),
            Err(message) => Err(format!(
                "expected a schema-version drift error, got: {message}"
            )),
            Ok(()) => Err(
                "an executor receipt with a drifted schema version must be rejected".to_string(),
            ),
        }
    }

    #[test]
    fn receipt_with_a_drifted_claim_boundary_is_rejected() -> Result<(), String> {
        let mut receipt = committed_receipt()?;
        receipt["claim_boundary"] =
            serde_json::json!("executor success qualifies the installed candidate");
        match validate_executor_receipt(&receipt, live_report()?.scenarios.len()) {
            Err(message) if message.contains("claim_boundary") => Ok(()),
            Err(message) => Err(format!(
                "expected a claim-boundary drift error, got: {message}"
            )),
            Ok(()) => Err(
                "an executor receipt with a drifted claim boundary must be rejected".to_string(),
            ),
        }
    }

    #[test]
    fn receipt_without_limitations_is_rejected() -> Result<(), String> {
        let mut receipt = committed_receipt()?;
        receipt["limitations"] = serde_json::json!([]);
        match validate_executor_receipt(&receipt, live_report()?.scenarios.len()) {
            Err(message) if message.contains("limitations") => Ok(()),
            Err(message) => Err(format!(
                "expected an explicit-limitations error, got: {message}"
            )),
            Ok(()) => {
                Err("an executor receipt without explicit limitations must be rejected".to_string())
            }
        }
    }

    #[test]
    fn receipt_with_a_blank_limitation_is_rejected() -> Result<(), String> {
        let mut receipt = committed_receipt()?;
        receipt["limitations"] = serde_json::json!([" "]);
        match validate_executor_receipt(&receipt, live_report()?.scenarios.len()) {
            Err(message) if message.contains("non-empty limitations") => Ok(()),
            Err(message) => Err(format!(
                "expected an exact-limitations error, got: {message}"
            )),
            Ok(()) => {
                Err("an executor receipt with a blank limitation must be rejected".to_string())
            }
        }
    }

    #[test]
    fn receipt_without_not_exercised_combinations_is_rejected() -> Result<(), String> {
        let mut receipt = committed_receipt()?;
        receipt["combinations_not_exercised"] = serde_json::json!([]);
        match validate_executor_receipt(&receipt, live_report()?.scenarios.len()) {
            Err(message) if message.contains("combinations_not_exercised") => Ok(()),
            Err(message) => Err(format!(
                "expected an explicit-combinations error, got: {message}"
            )),
            Ok(()) => Err(
                "an executor receipt without not-exercised combinations must be rejected"
                    .to_string(),
            ),
        }
    }

    #[test]
    fn json_and_markdown_derive_from_one_dto_deterministically() -> Result<(), String> {
        let report = live_report()?;
        let first = blind_journey_execute_report_json(&report)?;
        let second = blind_journey_execute_report_json(&report)?;
        if first != second {
            return Err("the JSON projection must be byte-identical across runs".to_string());
        }
        let markdown = blind_journey_execute_report_markdown(&report);
        for row in &report.scenarios {
            if !markdown.contains(&row.scenario) {
                return Err(format!(
                    "the Markdown projection must name every scenario, missing `{}`",
                    row.scenario
                ));
            }
        }
        if !first.ends_with('\n') || first.ends_with("\n\n") {
            return Err("the JSON projection must end in exactly one newline".to_string());
        }
        Ok(())
    }

    #[test]
    fn required_scenarios_cannot_be_dropped_from_the_corpus() -> Result<(), String> {
        let report = live_report()?;
        let mut reduced = report.clone();
        reduced.scenarios.retain(|row| {
            !crate::blind_journey_execute::REQUIRED_BLIND_JOURNEY_EXECUTE_SCENARIO_IDS
                .contains(&row.scenario.as_str())
                || row.scenario == "executor_positive_journey_emits_receipt"
        });
        match require_required_scenarios(&reduced) {
            Err(message) if message.contains("missing required scenarios") => Ok(()),
            Err(message) => Err(format!(
                "expected a missing-required-scenarios error, got: {message}"
            )),
            Ok(()) => Err("dropping a required scenario must fail the coverage gate".to_string()),
        }
    }
}
