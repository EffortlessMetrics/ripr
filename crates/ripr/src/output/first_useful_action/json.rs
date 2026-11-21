use super::{
    ActionCommands, ActionEvidence, ActionFallback, ActionInputs, ActionSelected, ActionTarget,
    FirstUsefulActionReport, REPORT_KIND, SCHEMA_VERSION,
};
use serde::Serialize;

pub(crate) fn render_first_useful_action_json(
    report: &FirstUsefulActionReport,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct JsonReport<'a> {
        schema_version: &'static str,
        tool: &'static str,
        kind: &'static str,
        status: &'a str,
        audience: &'a str,
        action_kind: &'a str,
        root: &'a str,
        generated_at: &'a str,
        inputs: &'a ActionInputs,
        selected: &'a Option<ActionSelected>,
        title: &'a str,
        why: &'a str,
        why_first: &'a [String],
        target: &'a Option<ActionTarget>,
        commands: &'a ActionCommands,
        evidence: &'a ActionEvidence,
        fallback: &'a Option<ActionFallback>,
        warnings: &'a [String],
        limits: &'a [String],
    }

    serde_json::to_string_pretty(&JsonReport {
        schema_version: SCHEMA_VERSION,
        tool: "ripr",
        kind: REPORT_KIND,
        status: &report.status,
        audience: &report.audience,
        action_kind: &report.action_kind,
        root: &report.root,
        generated_at: &report.generated_at,
        inputs: &report.inputs,
        selected: &report.selected,
        title: &report.title,
        why: &report.why,
        why_first: &report.why_first,
        target: &report.target,
        commands: &report.commands,
        evidence: &report.evidence,
        fallback: &report.fallback,
        warnings: &report.warnings,
        limits: &report.limits,
    })
    .map_err(|err| format!("render first useful action JSON failed: {err}"))
}
