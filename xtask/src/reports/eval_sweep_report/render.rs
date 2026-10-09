//! Bounded Markdown rendering derived from the same accepted receipt value:
//! table cells escape pipes and flatten newline runs, so a free-text value
//! (a disposition owner, recovery route, or evidence reference) can never
//! split or extend a table row. The accepted receipt JSON keeps the raw
//! values.

use serde_json::{Value, json};

use super::{RERUN_COMMAND, fail};

// ---------------------------------------------------------------------------
// Bounded Markdown (derived from the same validated rows)
// ---------------------------------------------------------------------------

/// Renders one bounded-Markdown table cell: pipe characters are escaped and
/// newline runs are flattened to spaces, so a free-text value (a disposition
/// owner, recovery route, or evidence reference) can never split or extend a
/// table row. The accepted receipt JSON keeps the raw value.
fn markdown_cell(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '|' => out.push_str("\\|"),
            '\n' | '\r' => out.push(' '),
            other => out.push(other),
        }
    }
    out
}

pub(super) fn render_accepted_markdown(receipt: &Value) -> Result<String, String> {
    let counts = receipt
        .get("counts")
        .and_then(Value::as_object)
        .ok_or_else(|| fail("markdown", "counts", "accepted receipt must carry counts"))?;
    let mut out = String::new();
    out.push_str("# Python Eval Sweep — Accepted Receipt\n\n");
    out.push_str(
        "Accepted operational-robustness evidence over the retained eight-subject denominator.\n",
    );
    out.push_str("Informational metrics — never judged accuracy, never a support-tier change.\n\n");

    out.push_str("## Identity\n\n");
    let candidate = receipt.get("candidate").cloned().unwrap_or(json!({}));
    out.push_str(&format!(
        "- candidate receipt: sha256 `{}` (schema {})\n",
        candidate
            .get("sha256")
            .and_then(Value::as_str)
            .unwrap_or("?"),
        candidate
            .get("schema_version")
            .and_then(Value::as_str)
            .unwrap_or("?")
    ));
    out.push_str(&format!(
        "- manifest: sha256 `{}`\n",
        receipt
            .get("identities")
            .and_then(|identities| identities.get("manifest_sha256"))
            .and_then(Value::as_str)
            .unwrap_or("?")
    ));
    out.push_str(&format!(
        "- command contract: {}\n\n",
        receipt
            .get("command_contract_version")
            .and_then(Value::as_str)
            .unwrap_or("?")
    ));

    out.push_str("## Counts\n\n");
    out.push_str("| count | numerator | denominator |\n| --- | ---: | ---: |\n");
    for (name, count) in counts {
        let numerator = count.get("numerator").and_then(Value::as_u64);
        let denominator = count.get("denominator").and_then(Value::as_u64);
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            name,
            numerator
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string()),
            denominator
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string()),
        ));
    }
    out.push('\n');

    out.push_str("## Subjects\n\n");
    out.push_str("| id | status | disposition | owner |\n| --- | --- | --- | --- |\n");
    if let Some(subjects) = receipt.get("subjects").and_then(Value::as_array) {
        for subject in subjects {
            let id = subject.get("id").and_then(Value::as_str).unwrap_or("?");
            let status = subject.get("status").and_then(Value::as_str).unwrap_or("?");
            let disposition = subject
                .get("disposition")
                .and_then(|disposition| disposition.get("disposition"))
                .and_then(Value::as_str)
                .unwrap_or("-");
            let owner = subject
                .get("disposition")
                .and_then(|disposition| disposition.get("owner"))
                .and_then(Value::as_str)
                .unwrap_or("-");
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                markdown_cell(id),
                markdown_cell(status),
                markdown_cell(disposition),
                markdown_cell(owner)
            ));
        }
    }
    out.push('\n');

    out.push_str("## Non-claims\n\n");
    if let Some(non_claims) = receipt.get("non_claims").and_then(Value::as_array) {
        for claim in non_claims {
            if let Some(claim) = claim.as_str() {
                out.push_str(&format!("- {claim}\n"));
            }
        }
    }
    out.push('\n');
    out.push_str(&format!("rerun: `{RERUN_COMMAND} --check-currentness`\n"));
    Ok(out)
}
