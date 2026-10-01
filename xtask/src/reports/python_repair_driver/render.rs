//! Deterministic JSON and Markdown projection of a driver-check outcome.
//!
//! Both renderings are derived from the aggregated outcome only. Schema
//! version, accepted vocabulary, ordering, and claim-boundary text stay
//! byte-identical to the pre-split module.

use serde_json::{Value, json};

use super::aggregate::DriverCheckOutcome;
use super::schema::{BINDING_SPEC, RERUN_COMMAND};

pub(crate) fn render_check_driver_json(outcome: &DriverCheckOutcome) -> Result<String, String> {
    let verdict = outcome.verdict();
    let manifest_json = match &outcome.manifest {
        None => json!(null),
        Some(manifest) => json!({
            "path": outcome.manifest_path,
            "sha256": manifest.sha256,
            "selections": manifest.selections.len(),
        }),
    };
    let records: Vec<Value> = outcome
        .records
        .iter()
        .map(|record| {
            json!({
                "file": record.display,
                "phase": record.phase,
                "trust_attempt_id": record.trust_attempt_id,
                "target_path": record.target_path,
                "cage_status": record.cage_status,
            })
        })
        .collect();
    let violations: Vec<Value> = outcome
        .violations
        .iter()
        .map(|violation| json!(violation))
        .collect();
    let document = json!({
        "schema_version": "0.1",
        "kind": "python_repair_driver_check_report",
        "spec": BINDING_SPEC,
        "verdict": verdict,
        "manifest": manifest_json,
        "bindings": {
            "input": outcome.bindings_input,
            "records": records,
            "violations": violations,
        },
        "offline": true,
        "claim_boundary": "binding structural validation only; no completed or correct repair is established and no verification phase is claimed",
        "rerun": RERUN_COMMAND,
    });
    serde_json::to_string_pretty(&document)
        .map_err(|error| format!("failed to render python-repair-trust check-driver JSON: {error}"))
}

pub(crate) fn render_check_driver_markdown(outcome: &DriverCheckOutcome) -> String {
    let verdict = outcome.verdict();
    let mut out = String::new();
    out.push_str("# Python Repair Driver Binding Check\n\n");
    out.push_str(&format!("Verdict: **{verdict}**\n\n"));
    out.push_str(
        "Binding structural validation only — no completed or correct repair is established and no verification phase is claimed.\n\n",
    );
    match &outcome.manifest {
        None => {
            out.push_str("- manifest: none (records cannot bind without an accepted manifest)\n")
        }
        Some(manifest) => out.push_str(&format!(
            "- manifest: {} ({} selections, sha256 `{}`)\n",
            outcome.manifest_path,
            manifest.selections.len(),
            manifest.sha256
        )),
    }
    match &outcome.bindings_input {
        None => out.push_str("- bindings: none supplied (not_run; not a pass)\n"),
        Some(input) => {
            out.push_str(&format!(
                "- bindings: {input} ({} record(s), {} violation(s))\n",
                outcome.records.len(),
                outcome.violations.len()
            ));
            for record in &outcome.records {
                out.push_str(&format!(
                    "  - {} phase={} trust attempt `{}` target `{}` cage {}\n",
                    record.display,
                    record.phase,
                    record.trust_attempt_id,
                    record.target_path,
                    record.cage_status.as_deref().unwrap_or("n/a (prepare)"),
                ));
            }
        }
    }
    if outcome.violations.is_empty() {
        out.push_str("\n## Violations\n\nNone.\n");
    } else {
        out.push_str("\n## Violations\n\n");
        for violation in &outcome.violations {
            out.push_str(&format!("- {violation}\n"));
        }
    }
    out.push_str(&format!("\nrerun: `{RERUN_COMMAND}`\n"));
    out
}
