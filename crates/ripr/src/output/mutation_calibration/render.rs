use super::report::{confidence_label_for_match, join_method_counts, runtime_outcome_counts};
use super::types::*;
use super::{MUTATION_CALIBRATION_SCHEMA_VERSION, STATIC_WITHOUT_RUNTIME_SAMPLE_LIMIT};
use serde_json::Value;

pub(crate) fn render_mutation_calibration_json(
    report: &MutationCalibrationReport,
) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": MUTATION_CALIBRATION_SCHEMA_VERSION,
        "scope": "repo",
        "status": "advisory",
        "metrics": {
            "static_seams_total": report.static_seams_total,
            "mutants_total": report.mutants_total,
            "matched_total": report.matched.len(),
            "ambiguous_file_line_total": report.ambiguous_file_line.len(),
            "unmatched_mutants_total": report.unmatched_mutants.len(),
            "static_without_runtime_total": report.static_without_runtime.len(),
            "runtime_outcome_counts": runtime_outcome_counts(report),
            "join_method_counts": join_method_counts(report),
        },
        "agreement": mutation_calibration_agreement_json(&report.agreement),
        "precision_notes": &report.precision_notes,
        "missed_runtime_signals": report
            .missed_runtime_signals
            .iter()
            .map(mutation_calibration_runtime_signal_json)
            .collect::<Vec<_>>(),
        "static_only_findings": report
            .static_only_findings
            .iter()
            .map(mutation_calibration_static_only_json)
            .collect::<Vec<_>>(),
        "matches": report
            .matched
            .iter()
            .map(mutation_calibration_match_json)
            .collect::<Vec<_>>(),
        "ambiguous_file_line_matches": report
            .ambiguous_file_line
            .iter()
            .map(ambiguous_mutation_calibration_match_json)
            .collect::<Vec<_>>(),
        "unmatched_mutants": report
            .unmatched_mutants
            .iter()
            .map(mutation_outcome_json)
            .collect::<Vec<_>>(),
        "static_without_runtime_sample": report
            .static_without_runtime
            .iter()
            .take(STATIC_WITHOUT_RUNTIME_SAMPLE_LIMIT)
            .map(static_seam_json)
            .collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render mutation calibration JSON: {err}"))
}

pub(crate) fn render_mutation_calibration_md(report: &MutationCalibrationReport) -> String {
    let mut out = String::new();
    render_markdown_header(&mut out);
    render_markdown_summary(&mut out, report);
    render_markdown_agreement(&mut out, report);
    render_markdown_runtime_signals(&mut out, report);
    render_markdown_static_gaps(&mut out, report);
    render_markdown_runtime_counts(&mut out, report);
    render_markdown_matched_mutants(&mut out, report);
    render_markdown_ambiguous_matches(&mut out, report);
    render_markdown_unmatched_mutants(&mut out, report);
    render_markdown_static_without_runtime(&mut out, report);
    out
}

fn render_markdown_header(out: &mut String) {
    out.push_str("# ripr mutation calibration report\n\n");
    out.push_str("Status: advisory\n\n");
    out.push_str(
        "This report joins static seam evidence to supplied cargo-mutants runtime data. \
     Runtime outcome vocabulary in this report comes from that runtime data; static \
     ripr reports continue to use audit vocabulary only.\n\n",
    );
}

fn render_markdown_summary(out: &mut String, report: &MutationCalibrationReport) {
    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Count |\n| --- | ---: |\n");
    out.push_str(&format!(
        "| static_seams_total | {} |\n",
        report.static_seams_total
    ));
    out.push_str(&format!("| mutants_total | {} |\n", report.mutants_total));
    out.push_str(&format!("| matched_total | {} |\n", report.matched.len()));
    out.push_str(&format!(
        "| ambiguous_file_line_total | {} |\n",
        report.ambiguous_file_line.len()
    ));
    out.push_str(&format!(
        "| unmatched_mutants_total | {} |\n",
        report.unmatched_mutants.len()
    ));
    out.push_str(&format!(
        "| static_without_runtime_total | {} |\n",
        report.static_without_runtime.len()
    ));
}

fn render_markdown_agreement(out: &mut String, report: &MutationCalibrationReport) {
    out.push_str("\n## Static/runtime agreement\n\n");
    out.push_str("| Agreement bucket | Count |\n| --- | ---: |\n");
    out.push_str(&format!(
        "| static_gap_and_runtime_signal | {} |\n",
        report.agreement.static_gap_and_runtime_signal
    ));
    out.push_str(&format!(
        "| static_gap_without_runtime_signal | {} |\n",
        report.agreement.static_gap_without_runtime_signal
    ));
    out.push_str(&format!(
        "| runtime_signal_without_static_gap | {} |\n",
        report.agreement.runtime_signal_without_static_gap
    ));
    out.push_str(&format!(
        "| static_clean_and_runtime_clean | {} |\n",
        report.agreement.static_clean_and_runtime_clean
    ));
    out.push_str(&format!(
        "| runtime_inconclusive | {} |\n",
        report.agreement.runtime_inconclusive
    ));

    out.push_str("\nPrecision notes:\n\n");
    for note in &report.precision_notes {
        out.push_str(&format!("- {}\n", md_cell(note)));
    }
}

fn render_markdown_runtime_signals(out: &mut String, report: &MutationCalibrationReport) {
    out.push_str("\n### Runtime signals without static gaps\n\n");
    if report.missed_runtime_signals.is_empty() {
        out.push_str("No imported runtime gap signals lacked a matching static gap.\n");
    } else {
        out.push_str("| Runtime mutant | Location | Runtime outcome | Static class | Confidence label | Reason |\n");
        out.push_str("| --- | --- | --- | --- | --- | --- |\n");
        for record in &report.missed_runtime_signals {
            let mutant = record.runtime.mutant_id.as_deref().unwrap_or("unknown");
            let location = mutation_location_label(&record.runtime);
            let static_class = record
                .static_seam
                .as_ref()
                .map(|seam| seam.seam_grip_class.as_str())
                .unwrap_or("unmatched");
            out.push_str(&format!(
                "| `{}` | {} | {} | `{}` | `{}` | {} |\n",
                md_cell(mutant),
                md_cell(&location),
                md_cell(&record.runtime.runtime_outcome),
                md_cell(static_class),
                record.confidence_label,
                md_cell(&record.reason)
            ));
        }
    }
}

fn render_markdown_static_gaps(out: &mut String, report: &MutationCalibrationReport) {
    out.push_str("\n### Static gaps without runtime signals\n\n");
    if report.static_only_findings.is_empty() {
        out.push_str("No static gap seams lacked a runtime gap signal in this import.\n");
    } else {
        out.push_str("| Seam | Class | Location | Confidence label | Reason |\n");
        out.push_str("| --- | --- | --- | --- | --- |\n");
        for record in &report.static_only_findings {
            let location = format!("{}:{}", record.seam.file, record.seam.line);
            out.push_str(&format!(
                "| `{}` | `{}` | {} | `{}` | {} |\n",
                md_cell(&record.seam.seam_id),
                md_cell(&record.seam.seam_grip_class),
                md_cell(&location),
                record.confidence_label,
                md_cell(&record.reason)
            ));
        }
    }
}

fn render_markdown_runtime_counts(out: &mut String, report: &MutationCalibrationReport) {
    out.push_str("\n## Runtime Outcome Counts\n\n");
    out.push_str("| Runtime outcome | Count |\n| --- | ---: |\n");
    let counts = runtime_outcome_counts(report);
    if counts.is_empty() {
        out.push_str("| none | 0 |\n");
    } else {
        for (outcome, count) in counts {
            out.push_str(&format!("| {} | {} |\n", md_cell(&outcome), count));
        }
    }
}

fn render_markdown_matched_mutants(out: &mut String, report: &MutationCalibrationReport) {
    out.push_str("\n## Matched Mutants\n\n");
    if report.matched.is_empty() {
        out.push_str("No runtime mutants matched static seams.\n");
    } else {
        out.push_str("| Seam | Class | Oracle | Mutation operator | Runtime outcome | Join | Confidence label |\n");
        out.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
        for record in &report.matched {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}`/`{}` | {} | {} | `{}` | `{}` |\n",
                md_cell(&record.seam.seam_id),
                md_cell(&record.seam.seam_grip_class),
                md_cell(&record.seam.oracle_kind),
                md_cell(&record.seam.oracle_strength),
                md_cell(&record.mutation.mutation_operator),
                md_cell(&record.mutation.runtime_outcome),
                record.join_method,
                confidence_label_for_match(record)
            ));
        }
    }
}

fn render_markdown_ambiguous_matches(out: &mut String, report: &MutationCalibrationReport) {
    out.push_str("\n## Ambiguous File/Line Matches\n\n");
    if report.ambiguous_file_line.is_empty() {
        out.push_str(
            "No runtime mutants matched multiple static seams at the same file and line.\n",
        );
    } else {
        out.push_str("| Runtime mutant | Location | Runtime outcome | Confidence label | Candidate seams |\n");
        out.push_str("| --- | --- | --- | --- | --- |\n");
        for record in &report.ambiguous_file_line {
            let mutant = record.mutation.mutant_id.as_deref().unwrap_or("unknown");
            let location = mutation_location_label(&record.mutation);
            let candidates = record
                .candidates
                .iter()
                .map(|candidate| format!("`{}`", candidate.seam_id))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "| `{}` | {} | {} | `{}` | {} |\n",
                md_cell(mutant),
                md_cell(&location),
                md_cell(&record.mutation.runtime_outcome),
                "ambiguous_runtime_join",
                md_cell(&candidates)
            ));
        }
    }
}

fn render_markdown_unmatched_mutants(out: &mut String, report: &MutationCalibrationReport) {
    out.push_str("\n## Unmatched Runtime Mutants\n\n");
    if report.unmatched_mutants.is_empty() {
        out.push_str("All imported runtime mutants matched a static seam.\n");
    } else {
        out.push_str("| Location | Mutation operator | Runtime outcome | Test command |\n");
        out.push_str("| --- | --- | --- | --- |\n");
        for record in &report.unmatched_mutants {
            let location = mutation_location_label(record);
            let command = record.test_command.as_deref().unwrap_or("unknown");
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                md_cell(&location),
                md_cell(&record.mutation_operator),
                md_cell(&record.runtime_outcome),
                md_cell(command)
            ));
        }
    }
}

fn render_markdown_static_without_runtime(out: &mut String, report: &MutationCalibrationReport) {
    out.push_str("\n## Static Seams Without Runtime Data\n\n");
    if report.static_without_runtime.is_empty() {
        out.push_str(
            "Every static seam matched at least one runtime mutant in the imported data.\n",
        );
    } else {
        out.push_str(
            "Sample only; see JSON `static_without_runtime_total` for the full count.\n\n",
        );
        out.push_str("| Seam | Kind | Class | Location | Confidence label |\n");
        out.push_str("| --- | --- | --- | --- | --- |\n");
        for seam in report
            .static_without_runtime
            .iter()
            .take(STATIC_WITHOUT_RUNTIME_SAMPLE_LIMIT)
        {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | {}:{} | `no_runtime_data` |\n",
                md_cell(&seam.seam_id),
                md_cell(&seam.seam_kind),
                md_cell(&seam.seam_grip_class),
                md_cell(&seam.file),
                seam.line
            ));
        }
    }
}

fn mutation_calibration_agreement_json(agreement: &MutationCalibrationAgreement) -> Value {
    serde_json::json!({
        "static_gap_and_runtime_signal": agreement.static_gap_and_runtime_signal,
        "static_gap_without_runtime_signal": agreement.static_gap_without_runtime_signal,
        "runtime_signal_without_static_gap": agreement.runtime_signal_without_static_gap,
        "static_clean_and_runtime_clean": agreement.static_clean_and_runtime_clean,
        "runtime_inconclusive": agreement.runtime_inconclusive,
    })
}

fn mutation_calibration_runtime_signal_json(record: &MutationCalibrationRuntimeSignal) -> Value {
    serde_json::json!({
        "runtime": mutation_outcome_json(&record.runtime),
        "static": record.static_seam.as_ref().map(static_seam_json),
        "confidence_label": record.confidence_label,
        "reason": record.reason.as_str(),
    })
}

fn mutation_calibration_static_only_json(record: &MutationCalibrationStaticOnlyFinding) -> Value {
    serde_json::json!({
        "static": static_seam_json(&record.seam),
        "confidence_label": record.confidence_label,
        "reason": record.reason.as_str(),
    })
}

fn mutation_calibration_match_json(record: &MutationCalibrationMatch) -> Value {
    serde_json::json!({
        "join_method": record.join_method,
        "static": static_seam_json(&record.seam),
        "runtime": mutation_outcome_json(&record.mutation),
        "confidence_label": confidence_label_for_match(record),
    })
}

fn ambiguous_mutation_calibration_match_json(record: &AmbiguousMutationCalibrationMatch) -> Value {
    serde_json::json!({
        "runtime": mutation_outcome_json(&record.mutation),
        "confidence_label": "ambiguous_runtime_join",
        "candidates": record
            .candidates
            .iter()
            .map(static_seam_json)
            .collect::<Vec<_>>(),
    })
}

fn static_seam_json(record: &StaticSeamRecord) -> Value {
    serde_json::json!({
        "seam_id": record.seam_id.as_str(),
        "seam_kind": record.seam_kind.as_str(),
        "file": record.file.as_str(),
        "line": record.line,
        "seam_grip_class": record.seam_grip_class.as_str(),
        "oracle_kind": record.oracle_kind.as_str(),
        "oracle_strength": record.oracle_strength.as_str(),
        "observed_values": &record.observed_values,
        "missing_discriminators": &record.missing_discriminators,
    })
}

fn mutation_outcome_json(record: &MutationOutcomeRecord) -> Value {
    serde_json::json!({
        "mutant_id": record.mutant_id.as_deref(),
        "seam_id": record.seam_id.as_deref(),
        "file": record.file.as_deref(),
        "line": record.line,
        "mutation_operator": record.mutation_operator.as_str(),
        "runtime_outcome": record.runtime_outcome.as_str(),
        "duration": record.duration.as_deref(),
        "test_command": record.test_command.as_deref(),
    })
}

fn mutation_location_label(record: &MutationOutcomeRecord) -> String {
    if let Some(seam_id) = record.seam_id.as_ref() {
        return format!("seam:{seam_id}");
    }
    match (&record.file, record.line) {
        (Some(file), Some(line)) => format!("{file}:{line}"),
        (Some(file), None) => file.clone(),
        (None, Some(line)) => format!("line {line}"),
        (None, None) => "unknown".to_string(),
    }
}

fn md_cell(value: &str) -> String {
    value.replace('|', "\\|").replace(['\r', '\n'], " ")
}
