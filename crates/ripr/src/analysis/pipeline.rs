#[cfg(feature = "lang-python")]
use super::language::PythonAdapter;
#[cfg(feature = "lang-typescript")]
use super::language::TypeScriptAdapter;
use super::language::{
    LanguageAdapter, LanguageDiffResult, LanguageId, LanguageRepoResult, RustAdapter,
};
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
            LanguageId::TypeScript | LanguageId::JavaScript => {
                analyze_typescript_diff(options, oracle_policy, &changed_files)?
            }
            LanguageId::Python => analyze_python_diff(options, oracle_policy, &changed_files)?,
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
            LanguageId::TypeScript | LanguageId::JavaScript => {
                analyze_typescript_repo(options, oracle_policy)?
            }
            LanguageId::Python => analyze_python_repo(options, oracle_policy)?,
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

#[cfg(feature = "lang-typescript")]
fn analyze_typescript_diff(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
    changed_files: &[diff::ChangedFile],
) -> Result<LanguageDiffResult, String> {
    TypeScriptAdapter.analyze_diff(options, oracle_policy, changed_files)
}

#[cfg(not(feature = "lang-typescript"))]
fn analyze_typescript_diff(
    _options: &AnalysisOptions,
    _oracle_policy: &OraclePolicy,
    _changed_files: &[diff::ChangedFile],
) -> Result<LanguageDiffResult, String> {
    unavailable_language(LanguageId::TypeScript)
}

#[cfg(feature = "lang-python")]
fn analyze_python_diff(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
    changed_files: &[diff::ChangedFile],
) -> Result<LanguageDiffResult, String> {
    PythonAdapter.analyze_diff(options, oracle_policy, changed_files)
}

#[cfg(not(feature = "lang-python"))]
fn analyze_python_diff(
    _options: &AnalysisOptions,
    _oracle_policy: &OraclePolicy,
    _changed_files: &[diff::ChangedFile],
) -> Result<LanguageDiffResult, String> {
    unavailable_language(LanguageId::Python)
}

#[cfg(feature = "lang-typescript")]
fn analyze_typescript_repo(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
) -> Result<LanguageRepoResult, String> {
    TypeScriptAdapter.analyze_repo(options, oracle_policy)
}

#[cfg(not(feature = "lang-typescript"))]
fn analyze_typescript_repo(
    _options: &AnalysisOptions,
    _oracle_policy: &OraclePolicy,
) -> Result<LanguageRepoResult, String> {
    unavailable_language(LanguageId::TypeScript)
}

#[cfg(feature = "lang-python")]
fn analyze_python_repo(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
) -> Result<LanguageRepoResult, String> {
    PythonAdapter.analyze_repo(options, oracle_policy)
}

#[cfg(not(feature = "lang-python"))]
fn analyze_python_repo(
    _options: &AnalysisOptions,
    _oracle_policy: &OraclePolicy,
) -> Result<LanguageRepoResult, String> {
    unavailable_language(LanguageId::Python)
}

#[cfg(any(not(feature = "lang-typescript"), not(feature = "lang-python")))]
fn unavailable_language<T>(language: LanguageId) -> Result<T, String> {
    Err(format!(
        "language `{}` is not available in this ripr binary; rebuild with Cargo feature `{}` to enable it",
        language.as_str(),
        language.required_feature()
    ))
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "Tests assert an expected file-system error via `.expect_err(\"why\")`; the closure-style helper makes the expected failure mode part of the assertion message."
)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::super::AnalysisMode;
    use crate::config::OraclePolicy;

    fn temp_root(name: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("ripr-pipeline-{name}-{stamp}"));
        fs::create_dir_all(&root).map_err(|err| format!("create temp root failed: {err}"))?;
        Ok(root)
    }

    fn write(path: &std::path::Path, text: &str) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("create parent failed: {err}"))?;
        }
        fs::write(path, text).map_err(|err| format!("write {} failed: {err}", path.display()))
    }

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

    #[cfg(all(feature = "lang-typescript", feature = "lang-python"))]
    #[test]
    fn diff_pipeline_dispatches_enabled_preview_feature_adapters() -> Result<(), String> {
        let root = temp_root("preview-diff")?;
        let diff_file = root.join("preview.diff");
        write(
            &diff_file,
            r#"diff --git a/src/lib.ts b/src/lib.ts
index 0000000..1111111 100644
--- a/src/lib.ts
+++ b/src/lib.ts
@@ -1,0 +1,1 @@
+export function price() { return 1; }
diff --git a/app/main.py b/app/main.py
index 0000000..1111111 100644
--- a/app/main.py
+++ b/app/main.py
@@ -1,0 +1,1 @@
+def price(): return 1
"#,
        )?;

        let result = run_diff_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root: root.clone(),
                base: None,
                diff_file: Some(diff_file),
                mode: AnalysisMode::Draft,
                include_unchanged_tests: true,
            },
            &OraclePolicy::default(),
            &[LanguageId::TypeScript, LanguageId::Python],
        )?;

        assert!(result.findings.is_empty());
        assert_eq!(result.summary.changed_rust_files, 2);
        Ok(())
    }

    #[cfg(all(feature = "lang-typescript", feature = "lang-python"))]
    #[test]
    fn repo_pipeline_dispatches_enabled_preview_feature_adapters() -> Result<(), String> {
        let root = temp_root("preview-repo")?;

        let result = run_repo_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root,
                base: None,
                diff_file: None,
                mode: AnalysisMode::Deep,
                include_unchanged_tests: true,
            },
            &OraclePolicy::default(),
            &[LanguageId::TypeScript, LanguageId::Python],
        )?;

        assert!(result.findings.is_empty());
        assert_eq!(result.summary.changed_rust_files, 0);
        Ok(())
    }
}
