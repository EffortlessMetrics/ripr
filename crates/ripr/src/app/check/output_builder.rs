use super::{CheckInput, CheckOutput};
use crate::analysis::AnalysisResult;
use crate::app::CHECK_OUTPUT_SCHEMA_VERSION;

pub(super) fn check_output_from_analysis(
    input: CheckInput,
    analysis: AnalysisResult,
) -> CheckOutput {
    CheckOutput {
        schema_version: CHECK_OUTPUT_SCHEMA_VERSION.to_string(),
        tool: "ripr".to_string(),
        mode: input.mode,
        root: input.root,
        base: input.base,
        summary: analysis.summary,
        findings: analysis.findings,
    }
}
