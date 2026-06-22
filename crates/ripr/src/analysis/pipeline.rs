#[cfg(feature = "lang-python")]
use super::language::PythonAdapter;
#[cfg(feature = "lang-typescript")]
use super::language::TypeScriptAdapter;
use super::language::{
    LanguageAdapter, LanguageDiffResult, LanguageId, LanguageRepoResult, RustAdapter,
};
use super::{
    AnalysisOptions, AnalysisResult, LanguageRun, LanguageRunStatus, PreviewLanguageAdvisory, diff,
    sort, summary,
};
use crate::config::OraclePolicy;
use crate::domain::Finding;

/// Whether a language id corresponds to a preview adapter.
///
/// Only Rust is stable. TypeScript/JavaScript and Python are preview per
/// RIPR-SPEC-0026.
fn is_preview_language(language: LanguageId) -> bool {
    matches!(
        language,
        LanguageId::TypeScript | LanguageId::JavaScript | LanguageId::Python | LanguageId::Perl
    )
}

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
    run_pipeline_for_diff_text(options, oracle_policy, languages, &diff_text)
}

pub(crate) fn run_worktree_pipeline_with_oracle_policy(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
    languages: &[LanguageId],
) -> Result<AnalysisResult, String> {
    if options.diff_file.is_some() {
        return Err("worktree diff mode cannot be combined with --diff".to_string());
    }
    let diff_text = diff::load_worktree_diff(&options.root, options.base.as_deref())?;
    run_pipeline_for_diff_text(options, oracle_policy, languages, &diff_text)
}

fn run_pipeline_for_diff_text(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
    languages: &[LanguageId],
    diff_text: &str,
) -> Result<AnalysisResult, String> {
    let changed_files = diff::parse_unified_diff(diff_text);

    let mut findings: Vec<Finding> = Vec::new();
    let mut total_changed_files: usize = 0;
    let mut language_runs: Vec<LanguageRun> = Vec::new();
    for language in languages {
        // Non-abort contract (Campaign 31 PR 10, #1403): a preview-language
        // adapter failure must not abort the report — the failed language is
        // recorded in `language_runs` and the other languages' findings still
        // emit. Rust (the stable reference adapter) still propagates via `?`:
        // a Rust failure is infra-class (e.g. diff_scope_oversized), not a
        // per-language advisory gap.
        if !is_preview_language(*language) {
            // Rust (stable) failures propagate via `?`.
            let result = RustAdapter.analyze_diff(options, oracle_policy, &changed_files)?;
            findings.extend(result.findings);
            total_changed_files += result.changed_files;
            continue;
        }
        let attempted = match language {
            LanguageId::TypeScript | LanguageId::JavaScript => {
                analyze_typescript_diff(options, oracle_policy, &changed_files)
            }
            LanguageId::Python => analyze_python_diff(options, oracle_policy, &changed_files),
            LanguageId::Perl => analyze_perl_diff(),
            LanguageId::Rust => {
                continue;
            }
        };
        match attempted {
            Ok(result) => {
                findings.extend(result.findings);
                total_changed_files += result.changed_files;
            }
            Err(reason) => language_runs.push(LanguageRun {
                language: language.as_str().to_string(),
                status: LanguageRunStatus::Unavailable,
                reason: Some(reason),
            }),
        }
    }

    // Detect preview-language files in the diff regardless of whether the
    // adapter is enabled, so an empty result is never silently presented as a
    // clean Rust-grade result for a TypeScript/JavaScript/Python change
    // (RIPR-SPEC-0082, #1111). Detection is pure path routing — it does not
    // require the adapter to be enabled.
    let preview_paths: Vec<&diff::ChangedFile> = changed_files.iter().collect();
    let preview_advisories = detect_preview_advisories(languages, preview_paths.into_iter());

    sort::sort_findings(&mut findings);
    let summary_result = summary::summarize_findings(total_changed_files, &findings);

    Ok(AnalysisResult {
        summary: summary_result,
        findings,
        preview_language_advisories: preview_advisories,
        language_runs,
    })
}

pub(crate) fn run_repo_pipeline_with_oracle_policy(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
    languages: &[LanguageId],
) -> Result<AnalysisResult, String> {
    let mut findings: Vec<Finding> = Vec::new();
    let mut total_production_files: usize = 0;
    let mut language_runs: Vec<LanguageRun> = Vec::new();
    for language in languages {
        // Non-abort contract (see the diff loop above): preview-language
        // failures are recorded, Rust failures still propagate.
        if !is_preview_language(*language) {
            // Rust (stable) failures propagate via `?`.
            let result = RustAdapter.analyze_repo(options, oracle_policy)?;
            findings.extend(result.findings);
            total_production_files += result.production_files;
            continue;
        }
        let attempted = match language {
            LanguageId::TypeScript | LanguageId::JavaScript => {
                analyze_typescript_repo(options, oracle_policy)
            }
            LanguageId::Python => analyze_python_repo(options, oracle_policy),
            LanguageId::Perl => analyze_perl_repo(),
            LanguageId::Rust => {
                continue;
            }
        };
        match attempted {
            Ok(result) => {
                findings.extend(result.findings);
                total_production_files += result.production_files;
            }
            Err(reason) => language_runs.push(LanguageRun {
                language: language.as_str().to_string(),
                status: LanguageRunStatus::Unavailable,
                reason: Some(reason),
            }),
        }
    }

    // Detect preview-language files anywhere in the workspace regardless of
    // adapter enablement, so a repo-scope clean result is never silently
    // presented as Rust-grade clean when TypeScript/JavaScript/Python files
    // exist but are not analyzed (RIPR-SPEC-0082, #1111).
    let preview_advisories = detect_repo_preview_advisories(&options.root, languages);

    sort::sort_findings(&mut findings);
    let summary_result = summary::summarize_findings(total_production_files, &findings);

    Ok(AnalysisResult {
        summary: summary_result,
        findings,
        preview_language_advisories: preview_advisories,
        language_runs,
    })
}

/// Build repo-scope preview advisories by walking the workspace for
/// preview-language files, grouped by language, regardless of enablement.
fn detect_repo_preview_advisories(
    root: &std::path::Path,
    enabled: &[LanguageId],
) -> Vec<PreviewLanguageAdvisory> {
    let discovered = super::workspace::discover_preview_language_files(root);
    let mut advisories: Vec<PreviewLanguageAdvisory> = Vec::new();
    for language in PREVIEW_LANGUAGE_ORDER {
        if !language.is_available() {
            continue;
        }
        let files: Vec<String> = discovered
            .iter()
            .filter(|(lang, _)| lang == language)
            .map(|(_, path)| path.to_string_lossy().replace('\\', "/"))
            .collect();
        if files.is_empty() {
            continue;
        }
        let file_count = files.len();
        let sample_paths: Vec<String> = files.into_iter().take(3).collect();
        advisories.push(PreviewLanguageAdvisory {
            language: language.as_str().to_string(),
            file_count,
            sample_paths,
            enabled: enabled.contains(language),
        });
    }
    advisories
}

/// Languages that route through a compiled preview adapter, in stable order.
const PREVIEW_LANGUAGE_ORDER: &[LanguageId] = &[
    LanguageId::TypeScript,
    LanguageId::JavaScript,
    LanguageId::Python,
];

/// Build preview-language advisories by routing a stream of paths, regardless
/// of whether the adapter is enabled.
///
/// Each path is routed via `analysis::language::route`. Files that route to a
/// compiled preview language (TypeScript/JavaScript or Python) are grouped by
/// language. For each preview language with at least one file, one advisory is
/// emitted. `enabled` is `true` when the language is present in `enabled`
/// (the active `[languages]` list), `false` otherwise — so a TypeScript change
/// analyzed under the default Rust-only config still breaks the silent
/// empty-result honesty gap (RIPR-SPEC-0082, #1111).
///
/// Only compiled-in preview adapters are reported (`LanguageId::is_available`);
/// a preview language whose feature is not built is skipped, since the binary
/// could not analyze it under any config.
fn detect_preview_advisories<'a, I>(
    enabled: &[LanguageId],
    paths: I,
) -> Vec<PreviewLanguageAdvisory>
where
    I: Iterator<Item = &'a diff::ChangedFile>,
{
    let mut counts: Vec<(LanguageId, usize, Vec<String>)> = Vec::new();
    for changed in paths {
        let Some(language) = super::language::route(&changed.path) else {
            continue;
        };
        if !is_preview_language(language) || !language.is_available() {
            continue;
        }
        let normalized = changed.path.to_string_lossy().replace('\\', "/");
        match counts.iter_mut().find(|(lang, _, _)| *lang == language) {
            Some((_, count, samples)) => {
                *count += 1;
                if samples.len() < 3 {
                    samples.push(normalized);
                }
            }
            None => counts.push((language, 1, vec![normalized])),
        }
    }

    let mut advisories: Vec<PreviewLanguageAdvisory> = Vec::new();
    for language in PREVIEW_LANGUAGE_ORDER {
        if let Some((_, file_count, sample_paths)) =
            counts.iter().find(|(lang, _, _)| lang == language)
        {
            advisories.push(PreviewLanguageAdvisory {
                language: language.as_str().to_string(),
                file_count: *file_count,
                sample_paths: sample_paths.clone(),
                enabled: enabled.contains(language),
            });
        }
    }
    advisories
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

#[cfg(feature = "lang-perl")]
fn analyze_perl_diff() -> Result<LanguageDiffResult, String> {
    perl_fact_packet_preview_unavailable()
}

#[cfg(not(feature = "lang-perl"))]
fn analyze_perl_diff() -> Result<LanguageDiffResult, String> {
    unavailable_language(LanguageId::Perl)
}

#[cfg(feature = "lang-perl")]
fn analyze_perl_repo() -> Result<LanguageRepoResult, String> {
    perl_fact_packet_preview_unavailable()
}

#[cfg(not(feature = "lang-perl"))]
fn analyze_perl_repo() -> Result<LanguageRepoResult, String> {
    unavailable_language(LanguageId::Perl)
}

#[cfg(feature = "lang-perl")]
fn perl_fact_packet_preview_unavailable<T>() -> Result<T, String> {
    Err(
        "language `perl` is a fact-packet preview; `ripr check` does not launch perl-lsp or consume live Perl fact packets yet".to_string(),
    )
}

#[cfg(any(
    not(feature = "lang-typescript"),
    not(feature = "lang-python"),
    not(feature = "lang-perl")
))]
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

    /// A root path that is guaranteed to fail file-system traversal on both
    /// Linux and Windows: a path that points at a *file* (not a directory),
    /// so any attempt to walk it as a repository root surfaces an error.
    ///
    /// The earlier form used `/nonexistent`, which on Windows is coerced to a
    /// drive-relative path that the walker treats as an empty-but-valid root,
    /// turning the expected error into an empty `Ok` result. Using a real file
    /// as the root makes the failure mode identical across platforms.
    fn invalid_root_path() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let file_path = std::env::temp_dir().join(format!("ripr-pipeline-not-a-dir-{stamp}.txt"));
        // Create the file (not a directory). A subsequent directory walk of
        // this path fails with ENOTDIR / "not a directory" on both platforms.
        let _ = fs::write(&file_path, b"this is a file, not a directory");
        file_path
    }

    #[test]
    fn diff_pipeline_is_callable() {
        // Seam test: verify the function signature and basic error handling.
        // Integration tests in analysis::tests verify actual pipeline output behavior.
        // Rust (stable) failures still propagate via `?` — only preview-language
        // failures are non-abort (Campaign 31 PR 10, #1403).
        let result = run_diff_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root: invalid_root_path(),
                base: None,
                diff_file: None,
                mode: AnalysisMode::Draft,
                include_unchanged_tests: false,
                resolve_tsconfig_paths: false,
            },
            &OraclePolicy::default(),
            &[LanguageId::Rust],
        );
        // Rust failure on an invalid root propagates as a top-level Err.
        result.expect_err("expected Rust adapter failure to propagate");
    }

    #[test]
    fn repo_pipeline_is_callable() {
        // Seam test: verify the function signature and basic error handling.
        // Integration tests in analysis::tests verify actual pipeline output behavior.
        // Rust (stable) failures still propagate via `?` — only preview-language
        // failures are non-abort (Campaign 31 PR 10, #1403).
        let result = run_repo_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root: invalid_root_path(),
                base: None,
                diff_file: None,
                mode: AnalysisMode::Draft,
                include_unchanged_tests: false,
                resolve_tsconfig_paths: false,
            },
            &OraclePolicy::default(),
            &[LanguageId::Rust],
        );
        // Rust failure on an invalid root propagates as a top-level Err.
        result.expect_err("expected Rust adapter failure to propagate");
    }

    /// Non-abort contract (Campaign 31 PR 10, #1403): a single language's
    /// failure must not abort the whole report. Perl always returns `Err`
    /// today (scaffold-only stub at `analyze_perl_diff`), so a Rust+Perl
    /// mixed run must still produce Rust findings AND record a Perl
    /// `unavailable` entry in `language_runs` — not propagate the `Err`.
    #[test]
    fn diff_pipeline_does_not_abort_when_one_language_fails() -> Result<(), String> {
        let root = temp_root("perl-non-abort")?;
        let src = root.join("src/lib.rs");
        write(&src, "pub fn discount(price: u32) -> u32 { price / 2 }\n")?;
        let diff_file = root.join("mixed.diff");
        write(
            &diff_file,
            "diff --git a/src/lib.rs b/src/lib.rs\n\
             --- /dev/null\n\
             +++ b/src/lib.rs\n\
             @@ -0,0 +1 @@\n\
             +pub fn discount(price: u32) -> u32 { price / 2 }\n",
        )?;

        let result = run_diff_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root: root.clone(),
                base: None,
                diff_file: Some(diff_file),
                mode: AnalysisMode::Draft,
                include_unchanged_tests: false,
                resolve_tsconfig_paths: false,
            },
            &OraclePolicy::default(),
            &[LanguageId::Rust, LanguageId::Perl],
        );

        // The run must NOT abort — the top-level Ok must succeed.
        let analysis = match result {
            Ok(a) => a,
            Err(reason) => {
                return Err(format!(
                    "pipeline aborted on Perl failure, expected non-abort: {reason}"
                ));
            }
        };

        // Rust findings must survive (the Rust adapter ran to completion).
        assert!(
            analysis.summary.changed_rust_files >= 1,
            "Rust findings must survive a sibling Perl failure"
        );

        // A Perl `unavailable` entry must appear in language_runs.
        let perl_run = analysis.language_runs.iter().find(|r| r.language == "perl");
        assert!(
            perl_run.is_some(),
            "Perl failure must be recorded in language_runs, got: {:?}",
            analysis.language_runs
        );
        let perl_run = match perl_run {
            Some(r) => r,
            None => return Ok(()),
        };
        assert_eq!(
            perl_run.status,
            super::LanguageRunStatus::Unavailable,
            "Perl run status must be Unavailable"
        );
        assert!(
            perl_run.reason.is_some(),
            "Perl run must carry a reason string"
        );

        // Rust must NOT appear in language_runs (it ran to completion, so it's
        // omitted by design — the field is conditional on non-success).
        assert!(
            !analysis.language_runs.iter().any(|r| r.language == "rust"),
            "Rust (successful) must not appear in language_runs"
        );

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
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
                resolve_tsconfig_paths: false,
            },
            &OraclePolicy::default(),
            &[LanguageId::TypeScript, LanguageId::Python],
        )?;

        assert!(result.findings.is_empty());
        assert_eq!(result.summary.changed_rust_files, 2);
        Ok(())
    }

    // RIPR-SPEC-0082: preview-language advisory detection tests
    #[cfg(feature = "lang-typescript")]
    #[test]
    fn diff_pipeline_emits_preview_advisory_when_ts_files_present() -> Result<(), String> {
        let root = temp_root("spec-0082-ts-advisory")?;
        let diff_file = root.join("ts.diff");
        write(
            &diff_file,
            r#"diff --git a/src/discount.ts b/src/discount.ts
index 0000000..1111111 100644
--- a/src/discount.ts
+++ b/src/discount.ts
@@ -1,0 +1,3 @@
+export function discount(amount: number, threshold: number): number {
+  return amount >= threshold ? amount - 10 : amount;
+}
"#,
        )?;

        let result = run_diff_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root: root.clone(),
                base: None,
                diff_file: Some(diff_file),
                mode: AnalysisMode::Draft,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
            },
            &OraclePolicy::default(),
            &[LanguageId::TypeScript],
        )?;

        // The advisory must be present with correct language and non-zero count.
        if result.preview_language_advisories.is_empty() {
            return Err(
                "expected preview_language_advisories to be non-empty for TS diff".to_string(),
            );
        }
        let advisory = &result.preview_language_advisories[0];
        if advisory.language != "typescript" {
            return Err(format!(
                "expected language=typescript, got {}",
                advisory.language
            ));
        }
        if advisory.file_count == 0 {
            return Err("expected file_count > 0 in preview advisory".to_string());
        }
        if !advisory.enabled {
            return Err("expected enabled=true when TypeScript is in the enabled list".to_string());
        }
        Ok(())
    }

    // RIPR-SPEC-0082 / #1111 default case: a TypeScript diff with NO ripr.toml
    // (only Rust enabled) must STILL produce a preview advisory, marked
    // `enabled == false`, so the silent empty-result honesty gap is closed.
    #[cfg(feature = "lang-typescript")]
    #[test]
    fn diff_pipeline_emits_not_enabled_advisory_for_ts_diff_with_rust_only_config()
    -> Result<(), String> {
        let root = temp_root("spec-0082-ts-not-enabled")?;
        let diff_file = root.join("ts.diff");
        write(
            &diff_file,
            r#"diff --git a/src/utils.ts b/src/utils.ts
index 0000000..1111111 100644
--- a/src/utils.ts
+++ b/src/utils.ts
@@ -1,0 +1,3 @@
+export function add(a: number, b: number): number {
+  return a === 0 ? b : a + b;
+}
"#,
        )?;

        // ONLY Rust enabled — the default config. TypeScript is NOT enabled.
        let result = run_diff_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root: root.clone(),
                base: None,
                diff_file: Some(diff_file),
                mode: AnalysisMode::Draft,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
            },
            &OraclePolicy::default(),
            &[LanguageId::Rust],
        )?;

        if result.preview_language_advisories.is_empty() {
            return Err(
                "expected a preview advisory for a TS diff even when only Rust is enabled (#1111)"
                    .to_string(),
            );
        }
        let advisory = &result.preview_language_advisories[0];
        if advisory.language != "typescript" {
            return Err(format!(
                "expected language=typescript, got {}",
                advisory.language
            ));
        }
        if advisory.file_count != 1 {
            return Err(format!(
                "expected file_count=1, got {}",
                advisory.file_count
            ));
        }
        if advisory.enabled {
            return Err(
                "expected enabled=false when TypeScript is NOT in the enabled list".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn diff_pipeline_no_preview_advisory_for_rust_only_diff() -> Result<(), String> {
        let root = temp_root("spec-0082-rust-only")?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create src dir failed: {err}"))?;
        let diff_file = root.join("rust.diff");
        write(
            &diff_file,
            r#"diff --git a/src/lib.rs b/src/lib.rs
index 0000000..1111111 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,0 +1,3 @@
+pub fn price(amount: i32, threshold: i32) -> i32 {
+    if amount >= threshold { amount - 10 } else { amount }
+}
"#,
        )?;

        let result = run_diff_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root: root.clone(),
                base: None,
                diff_file: Some(diff_file),
                mode: AnalysisMode::Draft,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
            },
            &OraclePolicy::default(),
            &[LanguageId::Rust],
        )?;

        if !result.preview_language_advisories.is_empty() {
            return Err(format!(
                "expected no preview advisories for Rust-only diff, got: {:?}",
                result.preview_language_advisories
            ));
        }
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
                resolve_tsconfig_paths: false,
            },
            &OraclePolicy::default(),
            &[LanguageId::TypeScript, LanguageId::Python],
        )?;

        assert!(result.findings.is_empty());
        assert_eq!(result.summary.changed_rust_files, 0);
        Ok(())
    }
}
