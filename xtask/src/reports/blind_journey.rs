//! `cargo xtask blind-journey-contract` (#4603, RIPR-SPEC-0200): runs the
//! committed blind-journey fixture corpus through the typed contract validator
//! in `crate::blind_journey`, validates the committed versioned contract
//! decision receipt against the live result, and writes the human/JSON report.
//! Both projections derive from one evaluated DTO, so prose cannot strengthen
//! machine state. The gate fails closed when a scenario's real validator
//! outcome drifts from its committed expectation or the receipt disagrees.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::blind_journey::{
    BLIND_JOURNEY_ANSWER_KEY_SCHEMA_VERSION, BLIND_JOURNEY_CLAIM_BOUNDARY,
    BLIND_JOURNEY_PROMPT_SCHEMA_VERSION, BLIND_JOURNEY_RECEIPT_SCHEMA_VERSION,
    BlindJourneyAssessmentV1, BlindJourneyFixtureCorpusV1, BlindJourneyResultV1,
    assess_blind_journey_packet, load_blind_journey_fixture_corpus,
    missing_blind_journey_required_scenarios,
};

const CORPUS_PATH: &str = "fixtures/blind_journey_contract/corpus.json";
const RECEIPT_PATH: &str = "metrics/blind-journey-contract/contract-receipt.json";

#[derive(Clone, Debug, Serialize)]
pub(crate) struct BlindJourneyScenarioRowV1 {
    pub scenario: String,
    pub expected_accepted: bool,
    pub expected_terminal: Option<BlindJourneyResultV1>,
    pub accepted: bool,
    pub terminal_result: BlindJourneyResultV1,
    pub rejection_reasons: Vec<String>,
    pub contamination_findings: Vec<String>,
    pub disqualifiers: Vec<BlindJourneyResultV1>,
    pub event_count: usize,
    pub portable_identity: String,
    pub matches_expectation: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct BlindJourneyContractReportV1 {
    pub schema_version: String,
    pub claim_boundary: String,
    pub corpus_path: String,
    pub scenario_count: usize,
    pub accepted_count: usize,
    pub positive_count: usize,
    pub rejected_count: usize,
    pub expectation_failures: Vec<String>,
    pub scenarios: Vec<BlindJourneyScenarioRowV1>,
}

pub(crate) const BLIND_JOURNEY_CONTRACT_REPORT_SCHEMA_VERSION: &str =
    "blind_journey_contract_report.v1";

fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative)
}

/// Run one parsed corpus through the real validator and evaluate every
/// committed expectation. The returned report is `Ok` even when a scenario
/// mismatches; `expectation_failures` carries the drift so callers decide.
pub(crate) fn assess_blind_journey_fixture_corpus(
    corpus: &BlindJourneyFixtureCorpusV1,
) -> BlindJourneyContractReportV1 {
    // Pass 1: assess every packet and record its portable identity, so
    // identity pairings can reference scenarios regardless of corpus order.
    let mut assessments: Vec<BlindJourneyAssessmentV1> = Vec::with_capacity(corpus.scenarios.len());
    let mut identities: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    for scenario in &corpus.scenarios {
        let packet = crate::blind_journey::BlindJourneyPacketV1 {
            prompt: scenario.packet.prompt.clone(),
            answer_key: scenario.packet.answer_key.clone(),
            receipt: scenario.packet.receipt.clone(),
        };
        let assessment = assess_blind_journey_packet(&packet);
        identities.insert(scenario.id.clone(), assessment.portable_identity.clone());
        assessments.push(assessment);
    }
    // Pass 2: evaluate committed expectations and build the report rows.
    let mut rows = Vec::with_capacity(corpus.scenarios.len());
    let mut failures = Vec::new();
    let mut positive_count = 0usize;
    for (scenario, assessment) in corpus.scenarios.iter().zip(assessments.iter()) {
        let packet = crate::blind_journey::BlindJourneyPacketV1 {
            prompt: scenario.packet.prompt.clone(),
            answer_key: scenario.packet.answer_key.clone(),
            receipt: scenario.packet.receipt.clone(),
        };
        if assessment.positive() {
            positive_count += 1;
        }
        // Accepted receipts must be exactly producer-canonical: a hand-edited
        // accepted packet cannot drift from what `stamp_blind_journey_packet`
        // (the accepted producer #4604 runs through) would emit.
        if scenario.expected.accepted {
            match crate::blind_journey::stamp_blind_journey_packet(
                scenario.packet.prompt.clone(),
                scenario.packet.answer_key.clone(),
                scenario.packet.receipt.clone(),
            ) {
                Ok(stamped)
                    if stamped.prompt == packet.prompt
                        && stamped.answer_key == packet.answer_key
                        && stamped.receipt == packet.receipt => {}
                Ok(_stamped) => failures.push(format!(
                    "scenario `{}` is not producer-canonical: the accepted packet differs from the stamped producer output",
                    scenario.id
                )),
                Err(error) => failures.push(format!(
                    "scenario `{}` is not producer-canonical: {error}",
                    scenario.id
                )),
            }
        }
        let terminal_matches = scenario
            .expected
            .terminal
            .is_none_or(|expected| expected == assessment.terminal_result);
        let accepted_matches = scenario.expected.accepted == assessment.accepted;
        let identity_matches = scenario
            .expected
            .same_portable_identity_as
            .as_ref()
            .is_none_or(|other| {
                identities
                    .get(other)
                    .is_some_and(|id| id == &assessment.portable_identity)
            });
        let matches_expectation = terminal_matches && accepted_matches && identity_matches;
        if !matches_expectation {
            failures.push(format!(
                "scenario `{}` drifted: expected accepted={} terminal={:?}, got accepted={} terminal={:?} reasons={:?}",
                scenario.id,
                scenario.expected.accepted,
                scenario.expected.terminal,
                assessment.accepted,
                assessment.terminal_result,
                assessment.rejection_reasons
            ));
        }
        rows.push(BlindJourneyScenarioRowV1 {
            scenario: scenario.id.clone(),
            expected_accepted: scenario.expected.accepted,
            expected_terminal: scenario.expected.terminal,
            accepted: assessment.accepted,
            terminal_result: assessment.terminal_result,
            rejection_reasons: assessment.rejection_reasons.clone(),
            contamination_findings: assessment
                .contamination_findings
                .iter()
                .map(|finding| format!("{}:{}", finding.category, finding.matched_pattern))
                .collect(),
            disqualifiers: assessment.disqualifiers.clone(),
            event_count: assessment.event_count,
            portable_identity: assessment.portable_identity.clone(),
            matches_expectation,
        });
    }
    let accepted_count = rows.iter().filter(|row| row.accepted).count();
    BlindJourneyContractReportV1 {
        schema_version: BLIND_JOURNEY_CONTRACT_REPORT_SCHEMA_VERSION.to_string(),
        claim_boundary: BLIND_JOURNEY_CLAIM_BOUNDARY.to_string(),
        corpus_path: CORPUS_PATH.to_string(),
        scenario_count: rows.len(),
        accepted_count,
        positive_count,
        rejected_count: rows.len() - accepted_count,
        expectation_failures: failures,
        scenarios: rows,
    }
}

fn require_required_scenarios(report: &BlindJourneyContractReportV1) -> Result<(), String> {
    let missing = missing_blind_journey_required_scenarios(
        report.scenarios.iter().map(|row| row.scenario.as_str()),
    );
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "blind journey fixture corpus is missing required scenarios: {missing:?}"
        ))
    }
}

fn read_json(path: &Path, label: &str) -> Result<Value, String> {
    let body = fs::read_to_string(path)
        .map_err(|error| format!("read {label} {}: {error}", path.display()))?;
    serde_json::from_str(&body)
        .map_err(|error| format!("parse {label} {}: {error}", path.display()))
}

/// The committed decision receipt is the versioned contract record; it must
/// name this contract's schema identities, stay inside its ratified fixture
/// scope, bind to the assessed corpus, and carry explicit limitations and
/// not-exercised combinations with exact entries.
fn validate_contract_receipt(receipt: &Value, corpus_scenario_count: usize) -> Result<(), String> {
    if receipt.get("kind").and_then(Value::as_str)
        != Some("blind_journey_contract_decision_receipt")
    {
        return Err("contract receipt carries an unexpected kind".to_string());
    }
    if receipt.get("status").and_then(Value::as_str) != Some("ratified_fixture_scope") {
        return Err("contract receipt status must be ratified_fixture_scope".to_string());
    }
    if receipt.get("decision").and_then(Value::as_str) != Some("RIPR-SPEC-0200") {
        return Err("contract receipt must name the RIPR-SPEC-0200 decision".to_string());
    }
    let schema_versions = receipt
        .get("contract_schema_versions")
        .ok_or_else(|| "contract receipt must pin the contract schema versions".to_string())?;
    for (key, expected) in [
        ("prompt", BLIND_JOURNEY_PROMPT_SCHEMA_VERSION),
        ("answer_key", BLIND_JOURNEY_ANSWER_KEY_SCHEMA_VERSION),
        ("receipt", BLIND_JOURNEY_RECEIPT_SCHEMA_VERSION),
    ] {
        if schema_versions.get(key).and_then(Value::as_str) != Some(expected) {
            return Err(format!(
                "contract receipt schema version `{key}` drifted from the contract constant `{expected}`"
            ));
        }
    }
    if receipt.get("corpus_scenario_count").and_then(Value::as_u64)
        != Some(corpus_scenario_count as u64)
    {
        return Err(
            "contract receipt corpus_scenario_count does not bind the assessed corpus".to_string(),
        );
    }
    for key in ["limitations", "combinations_not_exercised"] {
        let entries = receipt
            .get(key)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("contract receipt must carry a non-empty {key} list"))?;
        if entries.is_empty() {
            return Err(format!("contract receipt must record explicit {key}"));
        }
        if entries
            .iter()
            .any(|entry| entry.as_str().is_none_or(|entry| entry.trim().is_empty()))
        {
            return Err(format!(
                "contract receipt must record exact non-empty {key} entries"
            ));
        }
    }
    Ok(())
}

pub(crate) fn blind_journey_contract_report_value() -> Result<BlindJourneyContractReportV1, String>
{
    let corpus_body = fs::read_to_string(workspace_path(CORPUS_PATH))
        .map_err(|error| format!("read blind journey fixture corpus: {error}"))?;
    let corpus = load_blind_journey_fixture_corpus(&corpus_body)?;
    let report = assess_blind_journey_fixture_corpus(&corpus);
    require_required_scenarios(&report)?;
    let receipt = read_json(&workspace_path(RECEIPT_PATH), "contract receipt")?;
    validate_contract_receipt(&receipt, report.scenarios.len())?;
    if !report.expectation_failures.is_empty() {
        return Err(format!(
            "blind journey fixture corpus drifted: {:?}",
            report.expectation_failures
        ));
    }
    Ok(report)
}

pub(crate) fn blind_journey_contract_report() -> Result<(), String> {
    let report = blind_journey_contract_report_value()?;
    let json_body = blind_journey_contract_report_json(&report)?;
    crate::write_report("blind-journey-contract.json", &json_body)?;
    crate::write_report(
        "blind-journey-contract.md",
        &blind_journey_contract_report_markdown(&report),
    )?;
    println!("{json_body}");
    Ok(())
}

pub(crate) fn blind_journey_contract_report_json(
    report: &BlindJourneyContractReportV1,
) -> Result<String, String> {
    let body = serde_json::to_string_pretty(report)
        .map_err(|error| format!("serialize blind journey contract report: {error}"))?;
    Ok(format!("{body}\n"))
}

pub(crate) fn blind_journey_contract_report_markdown(
    report: &BlindJourneyContractReportV1,
) -> String {
    let mut body = String::new();
    body.push_str("# Blind journey contract report\n\n");
    body.push_str(&format!("Claim boundary: {}\n\n", report.claim_boundary));
    body.push_str(&format!(
        "- corpus: `{}`\n- scenarios: {}\n- accepted: {}\n- positive: {}\n- rejected: {}\n- expectation failures: {}\n\n",
        report.corpus_path,
        report.scenario_count,
        report.accepted_count,
        report.positive_count,
        report.rejected_count,
        report.expectation_failures.len()
    ));
    body.push_str("| scenario | expected | observed | accepted | events |\n");
    body.push_str("| --- | --- | --- | --- | ---: |\n");
    for row in &report.scenarios {
        let expected = match row.expected_terminal {
            Some(terminal) => format!("{} / {terminal:?}", row.expected_accepted),
            None => format!("{} / any terminal", row.expected_accepted),
        };
        body.push_str(&format!(
            "| {} | {} | {:?} | {} | {} |\n",
            row.scenario,
            expected,
            row.terminal_result,
            if row.accepted { "yes" } else { "no" },
            row.event_count
        ));
    }
    for row in &report.scenarios {
        if !row.rejection_reasons.is_empty() || !row.contamination_findings.is_empty() {
            body.push_str(&format!("\n## {}\n\n", row.scenario));
            for finding in &row.contamination_findings {
                body.push_str(&format!("- contamination: `{finding}`\n"));
            }
            for reason in &row.rejection_reasons {
                body.push_str(&format!("- rejection: {reason}\n"));
            }
        }
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live_report() -> Result<BlindJourneyContractReportV1, String> {
        let corpus_body = fs::read_to_string(workspace_path(CORPUS_PATH))
            .map_err(|error| format!("read blind journey fixture corpus: {error}"))?;
        let corpus = load_blind_journey_fixture_corpus(&corpus_body)?;
        Ok(assess_blind_journey_fixture_corpus(&corpus))
    }

    fn committed_receipt() -> Result<Value, String> {
        read_json(&workspace_path(RECEIPT_PATH), "contract receipt")
    }

    #[test]
    fn committed_corpus_and_receipt_validate_against_the_live_validator() -> Result<(), String> {
        let report = live_report()?;
        require_required_scenarios(&report)?;
        validate_contract_receipt(&committed_receipt()?, report.scenarios.len())?;
        if !report.expectation_failures.is_empty() {
            return Err(format!(
                "committed corpus drifted from the live validator: {:?}",
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
            reports_dir.join("blind-journey-contract.json"),
            blind_journey_contract_report_json(&report)?,
        )
        .map_err(|error| format!("write blind journey JSON report: {error}"))?;
        fs::write(
            reports_dir.join("blind-journey-contract.md"),
            blind_journey_contract_report_markdown(&report),
        )
        .map_err(|error| format!("write blind journey Markdown report: {error}"))
    }

    #[test]
    fn receipt_with_a_drifted_schema_version_is_rejected() -> Result<(), String> {
        let mut receipt = committed_receipt()?;
        receipt["contract_schema_versions"]["prompt"] =
            serde_json::json!("blind_journey_prompt.v2");
        match validate_contract_receipt(&receipt, live_report()?.scenarios.len()) {
            Err(message) if message.contains("schema version") => Ok(()),
            Err(message) => Err(format!(
                "expected a schema-version drift error, got: {message}"
            )),
            Ok(()) => {
                Err("a contract receipt with a drifted schema version must be rejected".to_string())
            }
        }
    }

    #[test]
    fn receipt_without_limitations_is_rejected() -> Result<(), String> {
        let mut receipt = committed_receipt()?;
        receipt["limitations"] = serde_json::json!([]);
        match validate_contract_receipt(&receipt, live_report()?.scenarios.len()) {
            Err(message) if message.contains("limitations") => Ok(()),
            Err(message) => Err(format!(
                "expected an explicit-limitations error, got: {message}"
            )),
            Ok(()) => {
                Err("a contract receipt without explicit limitations must be rejected".to_string())
            }
        }
    }

    #[test]
    fn receipt_with_a_blank_limitation_is_rejected() -> Result<(), String> {
        let mut receipt = committed_receipt()?;
        receipt["limitations"] = serde_json::json!([" "]);
        match validate_contract_receipt(&receipt, live_report()?.scenarios.len()) {
            Err(message) if message.contains("non-empty limitations") => Ok(()),
            Err(message) => Err(format!(
                "expected an exact-limitations error, got: {message}"
            )),
            Ok(()) => {
                Err("a contract receipt with a blank limitation must be rejected".to_string())
            }
        }
    }

    #[test]
    fn receipt_without_not_exercised_combinations_is_rejected() -> Result<(), String> {
        let mut receipt = committed_receipt()?;
        receipt["combinations_not_exercised"] = serde_json::json!([]);
        match validate_contract_receipt(&receipt, live_report()?.scenarios.len()) {
            Err(message) if message.contains("combinations_not_exercised") => Ok(()),
            Err(message) => Err(format!(
                "expected an explicit-combinations error, got: {message}"
            )),
            Ok(()) => Err(
                "a contract receipt without not-exercised combinations must be rejected"
                    .to_string(),
            ),
        }
    }

    #[test]
    fn json_and_markdown_derive_from_one_dto_deterministically() -> Result<(), String> {
        let report = live_report()?;
        let first = blind_journey_contract_report_json(&report)?;
        let second = blind_journey_contract_report_json(&report)?;
        if first != second {
            return Err("the JSON projection must be byte-identical across runs".to_string());
        }
        let markdown = blind_journey_contract_report_markdown(&report);
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
            !crate::blind_journey::REQUIRED_BLIND_JOURNEY_SCENARIO_IDS
                .contains(&row.scenario.as_str())
                || row.scenario == "clean_generic_prompt_accepted"
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
