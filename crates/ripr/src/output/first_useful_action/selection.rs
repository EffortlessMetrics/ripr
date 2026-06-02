use super::{
    ActionInputs, FirstUsefulActionInput, FirstUsefulActionReport, ParsedSources,
    acknowledged_report, actionable_report, baseline_only_report, gap_record_report,
    missing_assistant_proof_report, no_actionable_report, read_error_report, receipt_report,
    stale_report, suppressed_report, waived_report,
};

pub(super) fn select_report(
    input: &FirstUsefulActionInput,
    parsed: &ParsedSources,
    inputs: &ActionInputs,
    generated_at: &str,
) -> FirstUsefulActionReport {
    if let Some(report) = stale_report(input, parsed, inputs, generated_at) {
        report
    } else if let Some(report) = read_error_report(input, parsed, inputs, generated_at) {
        report
    } else if let Some(report) = receipt_report(input, parsed, inputs, generated_at) {
        report
    } else if let Some(report) = suppressed_report(input, parsed, inputs, generated_at) {
        report
    } else if let Some(report) = acknowledged_report(input, parsed, inputs, generated_at) {
        report
    } else if let Some(report) = waived_report(input, parsed, inputs, generated_at) {
        report
    } else if let Some(report) = gap_record_report(input, parsed, inputs, generated_at) {
        report
    } else if let Some(report) = missing_assistant_proof_report(input, parsed, inputs, generated_at)
    {
        report
    } else if let Some(report) = actionable_report(input, parsed, inputs, generated_at) {
        report
    } else if let Some(report) = baseline_only_report(input, parsed, inputs, generated_at) {
        report
    } else {
        no_actionable_report(input, parsed, inputs, generated_at)
    }
}
