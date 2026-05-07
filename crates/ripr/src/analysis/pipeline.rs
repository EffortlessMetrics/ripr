use super::{
    AnalysisOptions, AnalysisResult, classifier, diff, probes, rust_index, sort, summary, workspace,
};
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
    let changed_rust_paths = changed_files
        .iter()
        .filter(|file| file.path.extension().and_then(|e| e.to_str()) == Some("rs"))
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let rust_files = workspace::discover_rust_files(&options.root)?;
    let index_files = workspace::select_rust_files_for_mode(
        &rust_files,
        &changed_rust_paths,
        options.mode,
        options.include_unchanged_tests,
    );
    let mut index = rust_index::build_index(&options.root, &index_files)?;
    rust_index::apply_oracle_policy(&mut index, oracle_policy);

    let mut findings = Vec::new();
    let mut changed_rust_files = 0usize;

    for changed in changed_files
        .iter()
        .filter(|file| file.path.extension().and_then(|e| e.to_str()) == Some("rs"))
    {
        changed_rust_files += 1;
        let probes = probes::probes_for_file(&options.root, changed, &index);
        for probe in probes {
            findings.push(classifier::classify_probe(&probe, &index));
        }
    }

    sort::sort_findings(&mut findings);
    let summary_result = summary::summarize_findings(changed_rust_files, &findings);

    Ok(AnalysisResult {
        summary: summary_result,
        findings,
    })
}

pub(crate) fn run_repo_pipeline_with_oracle_policy(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
) -> Result<AnalysisResult, String> {
    let rust_files = workspace::discover_rust_files(&options.root)?;
    let production_files = rust_files
        .iter()
        .filter(|path| workspace::is_production_rust_path(path))
        .cloned()
        .collect::<Vec<_>>();

    // Index all discovered Rust files (production + tests + benches +
    // examples). The classifier's `find_related_tests` looks up tests
    // in the index; without test files the repo headline silently
    // inflates `no_static_path` for owners that *are* exercised by
    // integration tests under `tests/` or `examples/`. Probe seeding
    // stays production-only so test bodies do not generate findings.
    let mut index = rust_index::build_index(&options.root, &rust_files)?;
    rust_index::apply_oracle_policy(&mut index, oracle_policy);

    let mut findings = Vec::new();

    for path in &production_files {
        let probes = probes::probes_for_repo_file(&options.root, path, &index);
        for probe in probes {
            findings.push(classifier::classify_probe(&probe, &index));
        }
    }

    sort::sort_findings(&mut findings);
    let summary_result = summary::summarize_findings(production_files.len(), &findings);

    Ok(AnalysisResult {
        summary: summary_result,
        findings,
    })
}

#[cfg(test)]
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
