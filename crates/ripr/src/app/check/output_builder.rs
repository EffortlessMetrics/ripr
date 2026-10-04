use super::{CheckInput, CheckOutput};
use crate::analysis::AnalysisResult;
use crate::app::CHECK_OUTPUT_SCHEMA_VERSION;

pub(super) fn check_output_from_analysis(
    input: CheckInput,
    analysis: AnalysisResult,
) -> CheckOutput {
    CheckOutput {
        harness_projections: analysis.harness_projections,
        schema_version: CHECK_OUTPUT_SCHEMA_VERSION.to_string(),
        tool: "ripr".to_string(),
        mode: input.mode,
        root: input.root,
        // #3940: record the base the loader actually used (explicit or
        // resolved default) rather than only an explicitly supplied one, so
        // scope-less runs stay consumable by base-matching consumers.
        base: analysis.effective_base.or(input.base),
        analysis_outcome: analysis.analysis_outcome,
        summary: analysis.summary,
        findings: analysis.findings,
        preview_language_advisories: analysis.preview_language_advisories,
        language_runs: analysis.language_runs,
        no_scope_provided: false,
        // RIPR-SPEC-0112: the note fires only when uncommitted edits touched
        // a source or test file, i.e. when `--worktree` would differ.
        unanalyzed_working_tree: !analysis.uncommitted_source_paths.is_empty(),
        // #5258: name the untracked subset so the note can offer the real
        // repair; `--worktree` covers only the tracked remainder.
        untracked_working_tree_source_paths: analysis.untracked_source_paths,
        suppression: None,
        partial_scope: analysis.partial_scope,
    }
}
