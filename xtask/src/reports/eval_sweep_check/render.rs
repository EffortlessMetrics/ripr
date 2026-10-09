//! Deterministic JSON/Markdown rendering of one completed check outcome.
//! The rendered bytes are the pinned output contract: verdict vocabulary
//! `valid`/`incomplete`/`not_run`, the full typed incomplete disclosures,
//! the accepted subject inventory, and the structural-only claim boundary —
//! never a robustness or adequacy claim.

use serde_json::{Value, json};

use super::{CheckOutcome, KNOWN_SPEC, MANIFEST_SCHEMA_VERSION, RERUN_COMMAND, Verdict};

pub(super) fn render_check_json(outcome: &CheckOutcome) -> Result<String, String> {
    let verdict = outcome.verdict();
    let manifest_incomplete: Vec<Value> = outcome
        .accepted
        .incomplete
        .iter()
        .map(|diagnostic| json!(diagnostic.render()))
        .collect();
    let receipt_json = match &outcome.receipt {
        None => json!(null),
        Some(receipt) => {
            let incomplete: Vec<Value> = receipt
                .incomplete
                .iter()
                .map(|diagnostic| json!(diagnostic.render()))
                .collect();
            json!({
                "path": receipt.path,
                "schema_version": receipt.schema_version,
                "denominator": {
                    "selected": receipt.denominator_selected,
                    "run": receipt.denominator_run,
                },
                "verdict": receipt.verdict().as_str(),
                "incomplete": incomplete,
            })
        }
    };
    let document = json!({
        "schema_version": "0.1",
        "kind": "python_eval_sweep_check_report",
        "spec": KNOWN_SPEC,
        "verdict": verdict.as_str(),
        "manifest": {
            "path": outcome.manifest_path,
            "schema_version": MANIFEST_SCHEMA_VERSION,
            "subjects": outcome.accepted.subjects.len(),
            "subject_ids": outcome.accepted.ids(),
            "sha256": outcome.accepted.sha256,
            "incomplete": manifest_incomplete,
        },
        "receipt": receipt_json,
        "offline": true,
        "claim_boundary": "structural validation only; not a currentness, robustness, or adequacy claim",
        "rerun": RERUN_COMMAND,
    });
    serde_json::to_string_pretty(&document)
        .map_err(|error| format!("failed to render eval-sweep check JSON: {error}"))
}

pub(super) fn render_check_markdown(outcome: &CheckOutcome, verdict: Verdict) -> String {
    let mut out = String::new();
    out.push_str("# Eval Sweep Check\n\n");
    out.push_str(&format!("Verdict: **{}**\n\n", verdict.as_str()));
    out.push_str(
        "Structural validation only — not a currentness, robustness, or adequacy claim.\n\n",
    );
    out.push_str(&format!(
        "- manifest: {} ({} subjects, sha256 `{}`)\n",
        outcome.manifest_path,
        outcome.accepted.subjects.len(),
        outcome.accepted.sha256
    ));
    for subject in &outcome.accepted.subjects {
        out.push_str(&format!(
            "  - {} @ {} ({} — {})\n",
            subject.id, subject.sha, subject.license, subject.shape
        ));
    }
    match &outcome.receipt {
        None => out.push_str(
            "- receipt: none supplied (`not_run`; not a pass — supply --runs <receipt> to validate retained rows)\n",
        ),
        Some(receipt) => {
            out.push_str(&format!(
                "- receipt: {} schema {}, denominator selected={} run={}, verdict={}\n",
                receipt.path,
                receipt.schema_version,
                receipt.denominator_selected,
                receipt.denominator_run,
                receipt.verdict().as_str()
            ));
        }
    }
    let disclosures = outcome.incomplete();
    out.push_str(&format!(
        "\n## Incomplete identities ({})\n\n",
        disclosures.len()
    ));
    out.push_str(
        "Missing identities are typed incomplete; they are not invented and not errors.\n\n",
    );
    if disclosures.is_empty() {
        out.push_str("None.\n");
    }
    for diagnostic in &disclosures {
        out.push_str(&format!("- {}\n", diagnostic.render()));
    }
    out.push_str(&format!("\nrerun: `{RERUN_COMMAND}`\n"));
    out
}
