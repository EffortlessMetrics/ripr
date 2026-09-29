use super::model::{PackageQualificationGate, PackageQualificationReceipt, SelectionScope};

pub(crate) fn render_receipt_json(receipt: &PackageQualificationReceipt) -> Result<String, String> {
    render_json(receipt, "package qualification receipt")
}

pub(crate) fn render_gate_json(gate: &PackageQualificationGate) -> Result<String, String> {
    render_json(gate, "package qualification gate")
}

pub(crate) fn render_gate_markdown(gate: &PackageQualificationGate) -> String {
    let mut lines = vec![
        "# Package qualification gate".to_string(),
        String::new(),
        format!("Verdict: {}", gate.verdict.as_str()),
        format!("Source: {} / tree {}", gate.source.commit, gate.source.tree),
        format!(
            "Package: {} {} {}",
            gate.package.channel.as_str(),
            gate.package.name,
            gate.package.hash
        ),
        format!("Payload: {} {}", gate.payload.target, gate.payload.hash),
        format!(
            "Selection: {}",
            selection_scope_label(&gate.selection_scope)
        ),
        format!(
            "Subjects: selected={} executed={} failed={} skipped={}",
            gate.subjects.selected,
            gate.subjects.executed,
            gate.subjects.failed,
            gate.subjects.skipped
        ),
        String::new(),
        "Required rows:".to_string(),
    ];
    for row in &gate.required_row_results {
        let status = row
            .status
            .map(|status| status.as_str())
            .unwrap_or("missing");
        let subjects = row
            .subject_count
            .map(|count| count.to_string())
            .unwrap_or_else(|| "n/a".to_string());
        lines.push(format!(
            "- {}/{}: {} (subjects={subjects}, disposition={})",
            row.channel.as_str(),
            row.target,
            status,
            row.disposition
        ));
    }
    lines.push(String::new());
    if gate.failures.is_empty() {
        lines.push("Failures: none".to_string());
    } else {
        lines.push("Failures:".to_string());
        for failure in &gate.failures {
            lines.push(format!("- {failure}"));
        }
    }
    lines.push(String::new());
    lines.push(format!("Claim boundary: {}", gate.claim_boundary));
    lines.push("Non-claims:".to_string());
    for item in &gate.non_claims {
        lines.push(format!("- {item}"));
    }
    if !gate.limitations.is_empty() {
        lines.push(String::new());
        lines.push("Limitations:".to_string());
        for item in &gate.limitations {
            lines.push(format!("- {item}"));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn render_json<T: serde::Serialize>(value: &T, label: &str) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to render {label} JSON: {error}"))?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

fn selection_scope_label(scope: &SelectionScope) -> &'static str {
    match scope {
        SelectionScope::ExplicitSubset => "explicit_subset",
        SelectionScope::DeclaredFullMatrix => "declared_full_matrix",
    }
}
