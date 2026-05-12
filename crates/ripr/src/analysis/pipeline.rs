use super::language::{LanguageAdapter, LanguageId, PythonAdapter, RustAdapter, TypeScriptAdapter};
use super::{AnalysisOptions, AnalysisResult, diff, sort, summary};
use crate::config::OraclePolicy;
use crate::domain::Finding;

pub(crate) fn run_diff_pipeline_with_oracle_policy(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
    languages: &[LanguageId],
) -> Result<AnalysisResult, String> {
    let diff_text = diff::load_diff(
        &options.root,
        options.base.as_deref(),
        options.diff_file.as_ref(),
    )?;
    let changed_files = diff::parse_unified_diff(&diff_text);

    let mut findings: Vec<Finding> = Vec::new();
    let mut total_changed_files: usize = 0;
    for language in languages {
        let result = match language {
            LanguageId::Rust => RustAdapter.analyze_diff(options, oracle_policy, &changed_files)?,
            LanguageId::TypeScript => {
                TypeScriptAdapter.analyze_diff(options, oracle_policy, &changed_files)?
            }
            LanguageId::Python => {
                PythonAdapter.analyze_diff(options, oracle_policy, &changed_files)?
            }
        };
        findings.extend(result.findings);
        total_changed_files += result.changed_files;
    }

    sort::sort_findings(&mut findings);
    let summary_result = summary::summarize_findings(total_changed_files, &findings);

    Ok(AnalysisResult {
        summary: summary_result,
        findings,
    })
}

pub(crate) fn run_repo_pipeline_with_oracle_policy(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
    languages: &[LanguageId],
) -> Result<AnalysisResult, String> {
    let mut findings: Vec<Finding> = Vec::new();
    let mut total_production_files: usize = 0;
    for language in languages {
        let result = match language {
            LanguageId::Rust => RustAdapter.analyze_repo(options, oracle_policy)?,
            LanguageId::TypeScript => TypeScriptAdapter.analyze_repo(options, oracle_policy)?,
            LanguageId::Python => PythonAdapter.analyze_repo(options, oracle_policy)?,
        };
        findings.extend(result.findings);
        total_production_files += result.production_files;
    }

    sort::sort_findings(&mut findings);
    let summary_result = summary::summarize_findings(total_production_files, &findings);

    Ok(AnalysisResult {
        summary: summary_result,
        findings,
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
            &[LanguageId::Rust],
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
            &[LanguageId::Rust],
        );
        // Should fail with a file system error, not a panic.
        result.expect_err("expected pipeline to surface file-system error");
    }
}
