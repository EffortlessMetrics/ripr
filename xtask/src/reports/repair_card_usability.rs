//! `cargo xtask repair-card-usability-report` (#4669, RIPR-SPEC-0196):
//! runs the RepairCard usability measurement from the `ripr` library over
//! the governed #1702/#1579 corpus, validates the committed synthetic
//! expectations and the versioned budget decision receipt, and writes the
//! human/JSON report. The report gate fails closed when a synthetic profile
//! violates a ratified relation or the receipt disagrees with the live
//! measurement, so the ratified numbers cannot silently drift.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

const CORPUS_PATH: &str = "metrics/rust-repair-trust/corpus.json";
const EXPECTATIONS_PATH: &str = "metrics/repair-card-usability/ratified-expectations.json";
const RECEIPT_PATH: &str = "metrics/repair-card-usability/decision-receipt.json";

/// Relations every synthetic profile must satisfy for the ratification to
/// hold. These are the load-bearing claims of the decision receipt; a
/// regression in any of them fails this gate. The card-vs-packet size
/// comparison is reported per profile (`card_bytes_below_packet_bytes`,
/// `packet_over_card_percent`) but is not a ratification relation: on these
/// single-seam profiles the compact card wire is not smaller than the
/// single-seam packet wire, because both are small and the card carries its
/// envelope, nine detail references, and digests.
const PROFILE_RELATIONS: [&str; 4] = [
    "card_within_default_item_bound",
    "card_within_default_byte_bound",
    "wire_card_omits_packet_envelope",
    "packet_envelope_surfaces_seam",
];

fn read_json(path: &Path, label: &str) -> Result<Value, String> {
    let body = fs::read_to_string(path)
        .map_err(|error| format!("read {label} {}: {error}", path.display()))?;
    serde_json::from_str(&body)
        .map_err(|error| format!("parse {label} {}: {error}", path.display()))
}

fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative)
}

pub(crate) fn repair_card_usability_report() -> Result<(), String> {
    let corpus = read_json(&workspace_path(CORPUS_PATH), "governed corpus")?;
    let report = ripr::app::repair_card_usability::repair_card_usability_report(&corpus)?;
    let expectations = read_json(&workspace_path(EXPECTATIONS_PATH), "ratified expectations")?;
    let receipt = read_json(&workspace_path(RECEIPT_PATH), "decision receipt")?;
    validate_expectations(&report, &expectations)?;
    validate_receipt(&report, &receipt)?;
    let json_body = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("serialize repair card usability report: {error}"))?;
    crate::write_report("repair-card-usability.json", &format!("{json_body}\n"))?;
    crate::write_report("repair-card-usability.md", &markdown_report(&report))?;
    println!("{json_body}");
    Ok(())
}

fn report_profiles(report: &Value) -> Result<&[Value], String> {
    report
        .get("synthetic_profiles")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "report synthetic_profiles must be an array".to_string())
}

fn profile_ids(profiles: &[Value]) -> Result<Vec<String>, String> {
    let mut ids = Vec::with_capacity(profiles.len());
    for profile in profiles {
        let id = profile
            .get("profile")
            .and_then(Value::as_str)
            .ok_or_else(|| "synthetic profile is missing its id".to_string())?;
        ids.push(id.to_string());
    }
    Ok(ids)
}

/// The committed expectations pin which profiles exist and the relations the
/// ratification depends on; the live measurement must satisfy every relation
/// on every pinned profile.
fn validate_expectations(report: &Value, expectations: &Value) -> Result<(), String> {
    if expectations.get("kind").and_then(Value::as_str)
        != Some("repair_card_synthetic_ratified_expectations")
    {
        return Err("ratified expectations carry an unexpected kind".to_string());
    }
    let live = report_profiles(report)?;
    let live_ids = profile_ids(live)?;
    let mut expected_ids = expectations
        .get("profiles")
        .and_then(Value::as_array)
        .ok_or_else(|| "ratified expectations must pin the profile ids".to_string())?
        .iter()
        .map(|id| {
            id.as_str()
                .map(str::to_string)
                .ok_or_else(|| "expected profile ids must be strings".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut live_sorted = live_ids.clone();
    live_sorted.sort();
    expected_ids.sort();
    if live_sorted != expected_ids {
        return Err(format!(
            "synthetic profile set drifted: committed {expected_ids:?} vs live {live_sorted:?}"
        ));
    }
    let mut pinned_relations = expectations
        .get("relations")
        .and_then(Value::as_array)
        .ok_or_else(|| "ratified expectations must pin the relations".to_string())?
        .iter()
        .map(|relation| {
            relation
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| "pinned relations must be strings".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut gate_relations = PROFILE_RELATIONS
        .iter()
        .map(|relation| (*relation).to_string())
        .collect::<Vec<_>>();
    pinned_relations.sort();
    gate_relations.sort();
    if pinned_relations != gate_relations {
        return Err(format!(
            "ratified relations drifted: committed {pinned_relations:?} vs gate {gate_relations:?}"
        ));
    }
    for relation in PROFILE_RELATIONS {
        for profile in live {
            if profile.get(relation).and_then(Value::as_bool) != Some(true) {
                let name = profile
                    .get("profile")
                    .and_then(Value::as_str)
                    .map_or("unknown", |name| name);
                return Err(format!(
                    "ratified relation {relation} no longer holds on profile {name}; \
                     refresh the evidence or re-open the budget decision"
                ));
            }
        }
    }
    let bounds = expectations
        .get("default_bounds")
        .ok_or_else(|| "ratified expectations must pin the default bounds".to_string())?;
    let defaults = report
        .get("ratified_defaults")
        .ok_or_else(|| "report must carry ratified_defaults".to_string())?;
    for key in [
        "max_detail_items",
        "max_serialized_bytes",
        "max_inline_detail_bytes",
    ] {
        if bounds.get(key) != defaults.get(key) {
            return Err(format!(
                "default bound {key} drifted between the committed expectations and the domain \
                 constants; refresh the decision receipt"
            ));
        }
    }
    Ok(())
}

/// The decision receipt is the versioned ratification record; it must agree
/// with the live measurement and carry explicit limitations and not-exercised
/// combinations.
fn validate_receipt(report: &Value, receipt: &Value) -> Result<(), String> {
    if receipt.get("kind").and_then(Value::as_str) != Some("repair_card_budget_decision_receipt") {
        return Err("decision receipt carries an unexpected kind".to_string());
    }
    if receipt.get("status").and_then(Value::as_str) != Some("ratified_synthetic_scope") {
        return Err("decision receipt status must be ratified_synthetic_scope".to_string());
    }
    let receipt_defaults = receipt
        .get("ratified_defaults")
        .ok_or_else(|| "decision receipt must carry ratified_defaults".to_string())?;
    let report_defaults = report
        .get("ratified_defaults")
        .ok_or_else(|| "report must carry ratified_defaults".to_string())?;
    for key in [
        "max_detail_items",
        "max_serialized_bytes",
        "max_inline_detail_bytes",
        "field_set",
    ] {
        if receipt_defaults.get(key) != report_defaults.get(key) {
            return Err(format!(
                "decision receipt disagrees with the live measurement on {key}"
            ));
        }
    }
    for key in ["limitations", "combinations_not_exercised"] {
        let entries = receipt
            .get(key)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("decision receipt must carry a non-empty {key} list"))?;
        if entries.is_empty() {
            return Err(format!("decision receipt must record explicit {key}"));
        }
    }
    let attempt_cases = report
        .get("real_opportunities")
        .and_then(|real| real.get("attempt_cases"))
        .and_then(Value::as_u64)
        .ok_or_else(|| "report must carry the governed attempt-case count".to_string())?;
    let ratification = receipt
        .get("real_usability_ratification")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "decision receipt must carry the real-usability ratification state".to_string()
        })?;
    if attempt_cases == 0 && ratification != "pending" {
        return Err(
            "the governed corpus still has zero attempt cases, so the real-usability \
             ratification must stay pending"
                .to_string(),
        );
    }
    if attempt_cases > 0 {
        // Fail closed: no real_usability_ratification value is accepted once
        // attempt cases exist, because per-opportunity real card measurement
        // is not implemented yet. Extending the corpus must land together
        // with the measurement and a refreshed receipt.
        return Err(
            "the governed corpus now carries attempt cases; per-opportunity real card \
             measurement is not implemented, so no real_usability_ratification value is \
             accepted — land the measurement and refresh the receipt in the same change"
                .to_string(),
        );
    }
    Ok(())
}

fn markdown_report(report: &Value) -> String {
    let mut body = String::new();
    body.push_str("# RepairCard usability report\n\n");
    body.push_str(&format!(
        "Ratification scope: `{}` (synthetic fixture profiles; real-attempt ratification is tracked separately).\n\n",
        report
            .get("ratification_scope")
            .and_then(Value::as_str)
            .map_or("unknown", |scope| scope)
    ));
    if let Some(real) = report.get("real_opportunities") {
        body.push_str("## Governed real opportunities\n\n");
        body.push_str(&format!(
            "- denominator authority: `{}`\n- attempt cases: {}\n- exclusions: {}\n- observations: {}\n- card measurement state: `{}`\n\n",
            real.get("denominator_authority")
                .and_then(Value::as_str)
                .map_or("unknown", |value| value),
            real.get("attempt_cases")
                .and_then(Value::as_u64)
                .map_or(0, |count| count),
            real.get("exclusions")
                .and_then(Value::as_u64)
                .map_or(0, |count| count),
            real.get("observations")
                .and_then(Value::as_u64)
                .map_or(0, |count| count),
            real.get("card_measurement_state")
                .and_then(Value::as_str)
                .map_or("unknown", |state| state),
        ));
        if let Some(reason) = real.get("reason").and_then(Value::as_str) {
            body.push_str(&format!("{reason}\n\n"));
        }
    }
    body.push_str("## Synthetic profile measurements\n\n");
    body.push_str(
        "| profile | card bytes | packet bytes | packet/card % | detail items | next action | canonical packet |\n",
    );
    body.push_str("| --- | ---: | ---: | ---: | ---: | --- | --- |\n");
    if let Some(profiles) = report.get("synthetic_profiles").and_then(Value::as_array) {
        for profile in profiles {
            body.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                profile
                    .get("profile")
                    .and_then(Value::as_str)
                    .map_or("unknown", |value| value),
                profile
                    .get("card_bytes")
                    .and_then(Value::as_u64)
                    .map_or(0, |count| count),
                profile
                    .get("packet_bytes")
                    .and_then(Value::as_u64)
                    .map_or(0, |count| count),
                profile
                    .get("packet_over_card_percent")
                    .and_then(Value::as_u64)
                    .map_or(0, |count| count),
                profile
                    .get("detail_items")
                    .and_then(Value::as_u64)
                    .map_or(0, |count| count),
                profile
                    .get("next_action_present")
                    .and_then(Value::as_bool)
                    .is_some_and(|present| present),
                profile
                    .get("canonical_packet_state")
                    .and_then(Value::as_str)
                    .map_or("unknown", |state| state),
            ));
        }
    }
    body.push_str(
        "\nNormalized unit: UTF-8 bytes of pretty JSON with exactly one trailing newline.\n",
    );
    if let Some(presentations) = report.get("presentations").and_then(Value::as_object) {
        body.push_str("\n## Presentation channels\n\n");
        for (channel, state) in presentations {
            let status = state
                .get("state")
                .and_then(Value::as_str)
                .map_or("unknown", |status| status);
            body.push_str(&format!("- `{channel}`: {status}"));
            if let Some(reason) = state.get("reason").and_then(Value::as_str) {
                body.push_str(&format!(" — {reason}"));
            }
            body.push('\n');
        }
    }
    if let Some(field_decision) = report.get("field_set_decision") {
        body.push_str("\n## Field set decision\n\n");
        if let Some(reason) = field_decision.get("reason").and_then(Value::as_str) {
            body.push_str(&format!("{reason}\n"));
        }
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_corpus() -> Value {
        serde_json::json!({"cases": [], "exclusions": [1], "observations": [1, 2]})
    }

    fn live_report() -> Result<Value, String> {
        ripr::app::repair_card_usability::repair_card_usability_report(&synthetic_corpus())
    }

    fn matching_expectations(report: &Value) -> Result<Value, String> {
        let ids = profile_ids(report_profiles(report)?)?;
        let defaults = report
            .get("ratified_defaults")
            .ok_or_else(|| "ratified_defaults missing".to_string())?;
        Ok(serde_json::json!({
            "schema_version": "1.0",
            "kind": "repair_card_synthetic_ratified_expectations",
            "profiles": ids,
            "default_bounds": {
                "max_detail_items": defaults.get("max_detail_items"),
                "max_serialized_bytes": defaults.get("max_serialized_bytes"),
                "max_inline_detail_bytes": defaults.get("max_inline_detail_bytes"),
            },
        }))
    }

    fn matching_receipt() -> Value {
        serde_json::json!({
            "schema_version": "1.0",
            "kind": "repair_card_budget_decision_receipt",
            "decision": "RIPR-SPEC-0196",
            "status": "ratified_synthetic_scope",
            "ratified_defaults": {
                "max_detail_items": 16,
                "max_serialized_bytes": 65536,
                "max_inline_detail_bytes": 4096,
                "field_set": "RIPR-SPEC-0192 RepairCardV1 default fields, unchanged",
            },
            "limitations": ["synthetic fixtures cannot ratify real usability"],
            "combinations_not_exercised": ["LSP and MCP card presentation (#4668)"],
            "real_usability_ratification": "pending",
        })
    }

    #[test]
    fn committed_evidence_files_validate_against_the_live_measurement() -> Result<(), String> {
        let corpus = read_json(&workspace_path(CORPUS_PATH), "governed corpus")?;
        let report = ripr::app::repair_card_usability::repair_card_usability_report(&corpus)?;
        let expectations = read_json(&workspace_path(EXPECTATIONS_PATH), "ratified expectations")?;
        let receipt = read_json(&workspace_path(RECEIPT_PATH), "decision receipt")?;
        validate_expectations(&report, &expectations)?;
        validate_receipt(&report, &receipt)?;
        // Keep the measured report in the retained CI artifact so the actual
        // normalized byte counts are readable without rerunning the command.
        // Tests run with the xtask crate as CWD, so anchor at the workspace
        // root; `crate::write_report` is CWD-relative and would land in
        // xtask/target where the artifact upload never looks.
        let json_body = serde_json::to_string_pretty(&report)
            .map_err(|error| format!("serialize committed-evidence report: {error}"))?;
        let reports_dir = workspace_path("target/ripr/reports");
        fs::create_dir_all(&reports_dir)
            .map_err(|error| format!("create {}: {error}", reports_dir.display()))?;
        fs::write(
            reports_dir.join("repair-card-usability.json"),
            format!("{json_body}\n"),
        )
        .map_err(|error| format!("write usability JSON report: {error}"))?;
        fs::write(
            reports_dir.join("repair-card-usability.md"),
            markdown_report(&report),
        )
        .map_err(|error| format!("write usability Markdown report: {error}"))
    }

    #[test]
    fn receipt_with_a_drifted_bound_is_rejected() -> Result<(), String> {
        let report = live_report()?;
        let mut receipt = matching_receipt();
        receipt["ratified_defaults"]["max_serialized_bytes"] = serde_json::json!(32 * 1024);
        match validate_receipt(&report, &receipt) {
            Err(_message) => Ok(()),
            Ok(()) => {
                Err("a receipt that drifts from the domain constants must be rejected".to_string())
            }
        }
    }

    #[test]
    fn receipt_without_limitations_is_rejected() -> Result<(), String> {
        let report = live_report()?;
        let mut receipt = matching_receipt();
        receipt["limitations"] = serde_json::json!([]);
        match validate_receipt(&report, &receipt) {
            Err(_message) => Ok(()),
            Ok(()) => Err("a receipt without explicit limitations must be rejected".to_string()),
        }
    }

    #[test]
    fn pending_ratification_with_attempt_cases_is_rejected() -> Result<(), String> {
        let corpus = serde_json::json!({"cases": [1], "exclusions": [], "observations": []});
        let report = ripr::app::repair_card_usability::repair_card_usability_report(&corpus)?;
        let receipt = matching_receipt();
        match validate_receipt(&report, &receipt) {
            Err(_message) => Ok(()),
            Ok(()) => Err(
                "a pending real ratification must be rejected once attempt cases exist".to_string(),
            ),
        }
    }

    #[test]
    fn ratified_real_usability_without_measurements_is_rejected() -> Result<(), String> {
        let corpus = serde_json::json!({"cases": [1], "exclusions": [], "observations": []});
        let report = ripr::app::repair_card_usability::repair_card_usability_report(&corpus)?;
        let mut receipt = matching_receipt();
        receipt["real_usability_ratification"] = serde_json::json!("ratified");
        match validate_receipt(&report, &receipt) {
            Err(_message) => Ok(()),
            Ok(()) => Err(
                "an invented real ratification without per-opportunity measurements must be rejected"
                    .to_string(),
            ),
        }
    }

    #[test]
    fn expectations_with_a_missing_profile_are_rejected() -> Result<(), String> {
        let report = live_report()?;
        let mut expectations = matching_expectations(&report)?;
        expectations["profiles"] = serde_json::json!(["boundary_no_witness"]);
        match validate_expectations(&report, &expectations) {
            Err(_message) => Ok(()),
            Ok(()) => Err("expectations that drop a profile must be rejected".to_string()),
        }
    }

    #[test]
    fn drifted_default_bounds_fail_the_expectations_gate() -> Result<(), String> {
        let report = live_report()?;
        let mut expectations = matching_expectations(&report)?;
        expectations["default_bounds"]["max_serialized_bytes"] = serde_json::json!(1);
        match validate_expectations(&report, &expectations) {
            Err(_message) => Ok(()),
            Ok(()) => Err("drifted default bounds must fail the expectations gate".to_string()),
        }
    }
}
