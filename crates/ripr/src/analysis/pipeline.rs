use super::language::{LanguageAdapter, RustAdapter};
use super::{AnalysisOptions, AnalysisResult, diff, sort, summary};
use crate::config::OraclePolicy;

pub(crate) fn run_diff_pipeline_with_oracle_policy(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
) -> Result<AnalysisResult, String> {
    let diff_text = diff::load_diff(
        &options.root,
        options.base.as_deref(),
        options.diff_file.as_ref(),
    )?;
    let changed_files = diff::parse_unified_diff(&diff_text);

    let adapter = RustAdapter;
    let mut adapter_result = adapter.analyze_diff(options, oracle_policy, &changed_files)?;

    sort::sort_findings(&mut adapter_result.findings);
    let summary_result =
        summary::summarize_findings(adapter_result.changed_files, &adapter_result.findings);

    Ok(AnalysisResult {
        summary: summary_result,
        findings: adapter_result.findings,
    })
}

pub(crate) fn run_repo_pipeline_with_oracle_policy(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
) -> Result<AnalysisResult, String> {
    let adapter = RustAdapter;
    let mut adapter_result = adapter.analyze_repo(options, oracle_policy)?;

    sort::sort_findings(&mut adapter_result.findings);
    let summary_result =
        summary::summarize_findings(adapter_result.production_files, &adapter_result.findings);

    Ok(AnalysisResult {
        summary: summary_result,
        findings: adapter_result.findings,
    })
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "Tests assert an expected file-system error via `.expect_err(\"why\")`; the closure-style helper makes the expected failure mode part of the assertion message."
)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use super::super::AnalysisMode;
    use crate::config::OraclePolicy;

    #[test]
    fn diff_pipeline_is_callable() {
        // Seam test: verify the function signature and basic error handling.
        // Integration tests in analysis::tests verify actual pipeline output behavior.
        // This test simply ensures the extracted function compiles and can be called.
        let result = run_diff_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root: PathBuf::from("/nonexistent"),
                base: None,
                diff_file: None,
                mode: AnalysisMode::Draft,
                include_unchanged_tests: false,
            },
            &OraclePolicy::default(),
        );
        // Should fail with a file system error, not a panic.
        result.expect_err("expected pipeline to surface file-system error");
    }

    #[test]
    fn repo_pipeline_is_callable() {
        // Seam test: verify the function signature and basic error handling.
        // Integration tests in analysis::tests verify actual pipeline output behavior.
        let result = run_repo_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root: PathBuf::from("/nonexistent"),
                base: None,
                diff_file: None,
                mode: AnalysisMode::Draft,
                include_unchanged_tests: false,
            },
            &OraclePolicy::default(),
        );
        // Should fail with a file system error, not a panic.
        result.expect_err("expected pipeline to surface file-system error");
    }
}
