pub(crate) mod cancellation;
pub(crate) mod canonical_gap;
mod classifier;
mod classify;
mod diff;
mod extract;
mod facts;
mod language;
mod pipeline;
mod probes;
pub(crate) mod repair_route;
mod rust_index;
pub(crate) mod seam_cache;
mod seam_classification;
mod seam_inventory;
pub(crate) mod seams;
mod sort;
mod summary;
mod syntax;
pub(crate) mod test_grip_evidence;
mod value_resolution;
mod workspace;

pub(crate) use diff::{
    load_diff, load_diff_range, parse_unified_diff, resolve_base_commit,
    working_tree_has_tracked_changes,
};
pub(crate) use probes::{fingerprint_probe_id, normalize_expression};
pub(crate) use seam_classification::ClassifiedSeam;
#[cfg(test)]
pub(crate) use seam_classification::SeamGripClassCounts;
#[cfg(test)]
pub(crate) use seam_classification::classify_seam;
pub(crate) use seam_inventory::{
    DEFAULT_REPO_EXPOSURE_SEAM_LIMIT, ScopedClassifiedSeamInventory, SeamLimitInfo,
    SeamLimitSource, apply_pilot_seam_budget,
    inventory_changed_test_classified_seams_at_with_config_node,
    inventory_classified_seams_at_with_config, inventory_compact_classified_seams_at_with_config,
    inventory_diff_scoped_classified_seams_at_with_config, inventory_seams_at,
    workspace_cache_key_at_with_config,
};
pub(crate) use seams::{RepoSeam, RequiredDiscriminator};

/// Re-export workspace discovery helpers for the output layer so it can
/// detect TS-predominant workspaces without importing through analysis::workspace
/// directly. These are thin shims that forward to the inner workspace module.
pub(crate) fn workspace_preview_language_files(
    root: &Path,
) -> Vec<(language::LanguageId, PathBuf)> {
    workspace::discover_preview_language_files(root)
}

/// Re-export workspace Rust file discovery for the output layer so it can
/// check whether a workspace has any Rust source.
pub(crate) fn workspace_rust_files(root: &Path) -> Vec<PathBuf> {
    workspace::discover_rust_files(root).unwrap_or_default()
}

#[cfg(feature = "lang-typescript")]
pub(crate) fn targeted_typescript_findings_for_scope(
    root: &Path,
    config: &crate::config::RiprConfig,
    file: &Path,
    line: Option<u64>,
) -> Result<Vec<Finding>, String> {
    use diff::{ChangedFile, ChangedLine};
    use language::{LanguageAdapter, TypeScriptAdapter};

    // Ledger-supplied anchors are untrusted input: reject absolute paths and
    // parent traversal so a crafted rerun scope cannot read outside `root`.
    if file.is_absolute()
        || file
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(format!(
            "TypeScript rerun scope {} escapes the workspace root",
            file.display()
        ));
    }
    let absolute = root.join(file);
    let source = std::fs::read_to_string(&absolute).map_err(|err| {
        format!(
            "read TypeScript rerun scope {} failed: {err}",
            absolute.display()
        )
    })?;
    let mut added_lines = Vec::new();
    match line {
        Some(line) if line > 0 => {
            let line_usize = usize::try_from(line)
                .map_err(|err| format!("TypeScript rerun scope line {line} is too large: {err}"))?;
            let text = source
                .lines()
                .nth(line_usize.saturating_sub(1))
                .ok_or_else(|| {
                    format!(
                        "TypeScript rerun scope {} no longer has line {line}",
                        file.display()
                    )
                })?
                .to_string();
            added_lines.push(ChangedLine {
                line: line_usize,
                text,
                new_side_line: line_usize,
            });
        }
        Some(line) => {
            return Err(format!(
                "TypeScript rerun scope line must be 1-based; got {line}"
            ));
        }
        None => {
            for (index, text) in source.lines().enumerate() {
                let line = index + 1;
                added_lines.push(ChangedLine {
                    line,
                    text: text.to_string(),
                    new_side_line: line,
                });
            }
        }
    }

    let options = AnalysisOptions {
        root: root.to_path_buf(),
        base: None,
        diff_file: None,
        mode: AnalysisMode::Draft,
        include_unchanged_tests: config.analysis().include_unchanged_tests().unwrap_or(true),
        resolve_tsconfig_paths: config.typescript().resolve_tsconfig_paths(),
        perl_facts_path: None,
    };
    let result = TypeScriptAdapter.analyze_diff(
        &options,
        config.oracles(),
        &[ChangedFile {
            path: file.to_path_buf(),
            added_lines,
            removed_lines: Vec::new(),
        }],
    )?;
    Ok(result.findings)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptRepoReadiness {
    pub(crate) source_file_count: usize,
    pub(crate) test_file_count: usize,
    pub(crate) package_root_count: usize,
    pub(crate) package_confidence: String,
    pub(crate) runner_status: String,
    pub(crate) verify_command_count: usize,
    pub(crate) top_blocker: Option<String>,
}

#[cfg(feature = "lang-typescript")]
pub(crate) fn workspace_typescript_repo_readiness(root: &Path) -> Option<TypeScriptRepoReadiness> {
    use language::{
        TsPackageConfidence, is_test_file, resolve_package_discovery, verify_command_for_discovery,
    };

    let files = workspace_preview_language_files(root)
        .into_iter()
        .filter_map(|(language, path)| {
            matches!(
                language,
                language::LanguageId::TypeScript | language::LanguageId::JavaScript
            )
            .then_some(path)
        })
        .collect::<Vec<_>>();
    if files.is_empty() {
        return None;
    }

    let mut test_file_count = 0usize;
    let mut source_file_count = 0usize;
    let mut package_roots = BTreeSet::<String>::new();
    let mut best_confidence = TsPackageConfidence::None;
    let mut verify_command_count = 0usize;
    let mut package_root_missing = 0usize;
    let mut framework_missing = 0usize;
    let mut runner_missing = 0usize;
    let mut package_manager_missing = 0usize;
    let mut discovery_cache = HashMap::new();

    for file in &files {
        let file_is_test = is_test_file(file);
        if file_is_test {
            test_file_count += 1;
        } else {
            source_file_count += 1;
        }

        let parent = file.parent().map(Path::to_path_buf).unwrap_or_default();
        let discovery = discovery_cache
            .entry(parent)
            .or_insert_with(|| resolve_package_discovery(file, root));
        if let Some(package_root) = discovery.package_root.as_ref() {
            package_roots.insert(display_readiness_path(package_root));
        }
        best_confidence = max_ts_package_confidence(best_confidence, discovery.confidence);
        if file_is_test && verify_command_for_discovery(discovery, file).is_some() {
            verify_command_count += 1;
        }
        for limitation in &discovery.limitations {
            match limitation.as_str() {
                "typescript_package_root_unresolved" => package_root_missing += 1,
                "typescript_framework_hint_unresolved" => framework_missing += 1,
                "typescript_test_runner_unresolved" => runner_missing += 1,
                "typescript_package_manager_unresolved" => package_manager_missing += 1,
                _ => {}
            }
        }
    }

    let runner_status = if test_file_count == 0 {
        "no_tests_detected"
    } else if verify_command_count == test_file_count {
        "resolved"
    } else if verify_command_count > 0 {
        "partial"
    } else {
        "unresolved"
    };
    let top_blocker = top_typescript_readiness_blocker(
        test_file_count,
        package_root_missing,
        framework_missing,
        runner_missing,
        package_manager_missing,
    );

    Some(TypeScriptRepoReadiness {
        source_file_count,
        test_file_count,
        package_root_count: package_roots.len(),
        package_confidence: best_confidence.as_str().to_string(),
        runner_status: runner_status.to_string(),
        verify_command_count,
        top_blocker,
    })
}

#[cfg(not(feature = "lang-typescript"))]
pub(crate) fn workspace_typescript_repo_readiness(_root: &Path) -> Option<TypeScriptRepoReadiness> {
    None
}

#[cfg(feature = "lang-typescript")]
fn max_ts_package_confidence(
    left: language::TsPackageConfidence,
    right: language::TsPackageConfidence,
) -> language::TsPackageConfidence {
    if ts_package_confidence_rank(right) > ts_package_confidence_rank(left) {
        right
    } else {
        left
    }
}

#[cfg(feature = "lang-typescript")]
fn ts_package_confidence_rank(confidence: language::TsPackageConfidence) -> usize {
    use language::TsPackageConfidence;
    match confidence {
        TsPackageConfidence::None => 0,
        TsPackageConfidence::Low => 1,
        TsPackageConfidence::Medium => 2,
        TsPackageConfidence::High => 3,
    }
}

#[cfg(feature = "lang-typescript")]
fn display_readiness_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(feature = "lang-typescript")]
fn top_typescript_readiness_blocker(
    test_file_count: usize,
    package_root_missing: usize,
    framework_missing: usize,
    runner_missing: usize,
    package_manager_missing: usize,
) -> Option<String> {
    if test_file_count == 0 {
        return Some("typescript_tests_not_detected".to_string());
    }

    [
        (
            "typescript_package_root_unresolved",
            package_root_missing,
            "package roots",
        ),
        (
            "typescript_framework_hint_unresolved",
            framework_missing,
            "framework hints",
        ),
        (
            "typescript_test_runner_unresolved",
            runner_missing,
            "test runners",
        ),
        (
            "typescript_package_manager_unresolved",
            package_manager_missing,
            "package managers",
        ),
    ]
    .into_iter()
    .max_by(|(name_a, count_a, _), (name_b, count_b, _)| {
        count_a.cmp(count_b).then_with(|| name_b.cmp(name_a))
    })
    .and_then(|(name, count, label)| {
        (count > 0).then(|| format!("{name} ({count} {label} unresolved)"))
    })
}

use crate::config::OraclePolicy;
use crate::domain::{Finding, Summary};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisMode {
    Instant,
    Draft,
    Fast,
    Deep,
    Ready,
}

#[derive(Clone, Debug)]
pub struct AnalysisOptions {
    pub root: PathBuf,
    pub base: Option<String>,
    pub diff_file: Option<PathBuf>,
    pub mode: AnalysisMode,
    pub include_unchanged_tests: bool,
    /// When `true`, the TypeScript adapter reads `compilerOptions.paths` from
    /// `tsconfig.json` / `jsconfig.json` and uses alias maps to resolve
    /// non-relative import specifiers during owner↔test discovery.
    ///
    /// Default: `false` (opt-in, fail-closed per RIPR-SPEC-0099).
    pub resolve_tsconfig_paths: bool,
    /// Path to a `ripr-perl-facts-v1` packet file for the Perl adapter
    /// (Campaign 31, #1429). When `None`, the Perl adapter returns a named
    /// limitation (no analysis). When `Some`, the adapter reads the packet
    /// and produces Findings + limitations from it.
    pub perl_facts_path: Option<PathBuf>,
}

/// Advisory record for one compiled preview-language adapter whose files are
/// present in the analyzed scope.
///
/// Produced by the pipeline when TypeScript, JavaScript, or Python files are
/// present in the diff or repo — regardless of whether the adapter is
/// `enabled` in `ripr.toml` and regardless of whether any findings were
/// emitted. The count and sample paths come from real path routing
/// (`analysis::language::route`); they are never fabricated.
///
/// The `enabled` flag distinguishes the two honesty cases per
/// RIPR-SPEC-0082:
///
/// - `enabled == true` — the adapter ran; an empty result is advisory and may
///   be incomplete, not a Rust-grade clean result.
/// - `enabled == false` — the adapter is preview and NOT enabled, so these
///   files were not analyzed at all; the empty result must not be read as
///   clean.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewLanguageAdvisory {
    /// Stable language wire string (e.g. `"typescript"`, `"python"`).
    pub language: String,
    /// Number of files routed to this preview adapter.
    pub file_count: usize,
    /// Up to three sample file paths (normalized, forward-slash).
    pub sample_paths: Vec<String>,
    /// Whether this preview adapter was enabled (ran) for this analysis.
    ///
    /// `false` means the preview-language files were detected in scope but not
    /// analyzed because the adapter is not enabled in `ripr.toml`.
    pub enabled: bool,
}

/// Per-language run status for one language adapter invocation.
///
/// A `LanguageRun` is recorded for every language that was *attempted* but
/// did not complete successfully. Languages that ran to completion are
/// omitted (their findings speak for themselves). This keeps the field
/// conditional on non-success so the common single-language-success case
/// stays silent and no golden re-bless is needed.
///
/// This is the non-abort contract (Campaign 31 PR 10, ripr-swarm#1403): a
/// single language's failure (e.g. a Perl preview adapter that returns
/// `Err`, or a packet-ingestion rejection) must not abort the whole report.
/// Instead the failed language is recorded here and the other languages'
/// findings still emit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageRun {
    /// Stable language wire string (e.g. `"rust"`, `"perl"`).
    pub language: String,
    /// Outcome of the run.
    pub status: LanguageRunStatus,
    /// Human-readable reason for a non-ok status (absent for `Ok`).
    pub reason: Option<String>,
}

/// Outcome of one language adapter invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LanguageRunStatus {
    /// The adapter completed (findings, if any, are in `result.findings`).
    /// Recorded only when the caller explicitly asks for full accounting;
    /// the pipeline omits `Ok` runs by default to keep the field conditional.
    Ok,
    /// The adapter was invoked but returned a named limitation (e.g. a Perl
    /// preview adapter that is scaffold-only and returns `Err`).
    Unavailable,
    /// The adapter ran but produced a partial result (e.g. a packet was
    /// rejected by ingestion checks; some findings may still be present).
    Partial,
    /// The adapter could not run at all (e.g. required Cargo feature is off,
    /// or the producer binary is missing).
    Invalid,
}

impl LanguageRunStatus {
    /// Stable wire string for JSON / human output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Unavailable => "unavailable",
            Self::Partial => "partial",
            Self::Invalid => "invalid",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AnalysisResult {
    pub summary: Summary,
    pub findings: Vec<Finding>,
    /// Advisory records for preview-language files in the analyzed scope.
    ///
    /// Empty when only Rust (stable) files are in scope. Non-empty only when
    /// at least one file routed to a preview adapter (TypeScript/JS or Python).
    pub preview_language_advisories: Vec<PreviewLanguageAdvisory>,
    /// Per-language run-status records for languages that did NOT complete
    /// successfully. Empty when every enabled language ran to completion
    /// (the common single-language-success case). Non-abort contract: a
    /// failure here does not abort the report (Campaign 31 PR 10, #1403).
    pub language_runs: Vec<LanguageRun>,
}

/// Default language list when callers do not pass `[languages]` config.
///
/// Keeps existing public entry points (`run_analysis`, `run_repo_analysis`)
/// behaviorally identical to the pre-Campaign-27 Rust-only pipeline.
const DEFAULT_LANGUAGES: &[language::LanguageId] = &[language::LanguageId::Rust];

pub fn run_analysis(options: &AnalysisOptions) -> Result<AnalysisResult, String> {
    run_analysis_with_oracle_policy(options, &OraclePolicy::default(), DEFAULT_LANGUAGES)
}

pub(crate) fn run_analysis_with_oracle_policy(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
    languages: &[language::LanguageId],
) -> Result<AnalysisResult, String> {
    pipeline::run_diff_pipeline_with_oracle_policy(options, oracle_policy, languages)
}

pub(crate) fn run_worktree_analysis_with_oracle_policy(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
    languages: &[language::LanguageId],
) -> Result<AnalysisResult, String> {
    pipeline::run_worktree_pipeline_with_oracle_policy(options, oracle_policy, languages)
}

pub fn run_repo_analysis(options: &AnalysisOptions) -> Result<AnalysisResult, String> {
    run_repo_analysis_with_oracle_policy(options, &OraclePolicy::default(), DEFAULT_LANGUAGES)
}

pub(crate) fn run_repo_analysis_with_oracle_policy(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
    languages: &[language::LanguageId],
) -> Result<AnalysisResult, String> {
    pipeline::run_repo_pipeline_with_oracle_policy(options, oracle_policy, languages)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChangedLineOwner {
    pub(crate) file: PathBuf,
    pub(crate) line: usize,
    pub(crate) owner: String,
}

pub(crate) fn owner_symbols_for_lines(
    root: &Path,
    lines: &[(PathBuf, usize)],
) -> Result<Vec<ChangedLineOwner>, String> {
    let files = lines
        .iter()
        .map(|(file, _)| file.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let index = rust_index::build_index(root, &files)?;
    let mut owners = lines
        .iter()
        .filter_map(|(file, line)| {
            rust_index::find_owner_function(&index, file, *line).map(|function| ChangedLineOwner {
                file: file.clone(),
                line: *line,
                owner: function.id.to_string(),
            })
        })
        .collect::<Vec<_>>();
    owners.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.owner.cmp(&right.owner))
    });
    owners.dedup();
    Ok(owners)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "Test fixture builders use unwrap on fs operations against fresh temp dirs; receipted via policy/no-panic-allowlist.toml entries for crates/ripr/src/analysis/mod.rs."
)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ripr-{name}-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn analyzes_simple_predicate_gap() {
        let root = temp_dir("simple");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='x'\nversion='0.1.0'\nedition='2024'\n",
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn price(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("tests/pricing.rs"),
            r#"
#[test]
fn premium_customer_gets_discount() {
    let total = x::price(10000, 100);
    assert!(total > 0);
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("diff.patch"),
            r#"diff --git a/src/lib.rs b/src/lib.rs
index 0000000..1111111 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,3 @@
 pub fn price(amount: i32, threshold: i32) -> i32 {
+    if amount >= threshold { amount - 10 } else { amount }
 }
"#,
        )
        .unwrap();
        let out = run_analysis(&AnalysisOptions {
            root: root.clone(),
            base: None,
            diff_file: Some(root.join("diff.patch")),
            mode: AnalysisMode::Draft,
            include_unchanged_tests: true,
            resolve_tsconfig_paths: false,
            perl_facts_path: None,
        })
        .unwrap();
        assert!(!out.findings.is_empty());
        assert!(
            out.findings
                .iter()
                .any(|f| f.class == crate::domain::ExposureClass::WeaklyExposed
                    || f.class == crate::domain::ExposureClass::InfectionUnknown)
        );

        let instant = run_analysis(&AnalysisOptions {
            root: root.clone(),
            base: None,
            diff_file: Some(root.join("diff.patch")),
            mode: AnalysisMode::Instant,
            include_unchanged_tests: true,
            resolve_tsconfig_paths: false,
            perl_facts_path: None,
        })
        .unwrap();
        assert!(instant.findings.iter().any(|finding| {
            finding.class == crate::domain::ExposureClass::NoStaticPath
                && finding.related_tests.is_empty()
        }));
    }

    #[test]
    fn repo_analysis_finds_predicate_in_production_file() -> Result<(), String> {
        let root = temp_dir("repo_pred");
        fs::create_dir_all(root.join("src"))
            .map_err(|e| format!("failed to create src dir: {e}"))?;
        fs::create_dir_all(root.join("tests"))
            .map_err(|e| format!("failed to create tests dir: {e}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='x'\nversion='0.1.0'\nedition='2024'\n",
        )
        .map_err(|e| format!("failed to write Cargo.toml: {e}"))?;
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn price(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#,
        )
        .map_err(|e| format!("failed to write src/lib.rs: {e}"))?;
        fs::write(
            root.join("tests/pricing.rs"),
            r#"
#[test]
fn premium_customer_gets_discount() {
    let total = x::price(10000, 100);
    assert!(total > 0);
}
"#,
        )
        .map_err(|e| format!("failed to write tests/pricing.rs: {e}"))?;

        let out = run_repo_analysis(&AnalysisOptions {
            root,
            base: None,
            diff_file: None,
            mode: AnalysisMode::Draft,
            include_unchanged_tests: true,
            resolve_tsconfig_paths: false,
            perl_facts_path: None,
        })?;

        if out.findings.is_empty() {
            return Err("expected at least one finding from repo analysis".to_string());
        }
        if !out
            .findings
            .iter()
            .any(|f| f.probe.family == crate::domain::ProbeFamily::Predicate)
        {
            return Err("expected at least one Predicate family finding".to_string());
        }
        Ok(())
    }

    #[test]
    fn typescript_repo_readiness_uses_preview_file_scope() -> Result<(), String> {
        let root = temp_dir("ts_readiness_scope");
        fs::create_dir_all(root.join("src"))
            .map_err(|e| format!("failed to create src dir: {e}"))?;
        fs::create_dir_all(root.join("fixtures/noise/src"))
            .map_err(|e| format!("failed to create ignored fixture dir: {e}"))?;
        fs::write(
            root.join("package.json"),
            r#"{"devDependencies":{"vitest":"^1.0.0"}}"#,
        )
        .map_err(|e| format!("failed to write package.json: {e}"))?;
        fs::write(root.join("pnpm-lock.yaml"), "")
            .map_err(|e| format!("failed to write pnpm-lock.yaml: {e}"))?;
        fs::write(root.join("src/app.ts"), "export const value = 1;\n")
            .map_err(|e| format!("failed to write source file: {e}"))?;
        fs::write(
            root.join("src/app.test.ts"),
            "import { value } from './app';\n",
        )
        .map_err(|e| format!("failed to write test file: {e}"))?;
        fs::write(
            root.join("fixtures/noise/src/noise.test.ts"),
            "test('noise', () => {});\n",
        )
        .map_err(|e| format!("failed to write ignored fixture test file: {e}"))?;

        let readiness = workspace_typescript_repo_readiness(&root)
            .ok_or_else(|| "expected TypeScript readiness card".to_string())?;

        assert_eq!(readiness.source_file_count, 1);
        assert_eq!(readiness.test_file_count, 1);
        assert_eq!(readiness.verify_command_count, 1);
        assert_eq!(readiness.runner_status, "resolved");

        fs::remove_dir_all(&root).map_err(|e| format!("failed to remove temp dir: {e}"))?;
        Ok(())
    }

    #[test]
    fn owner_symbols_for_lines_names_containing_function() -> Result<(), String> {
        let root = temp_dir("owner_lines");
        fs::create_dir_all(root.join("src"))
            .map_err(|e| format!("failed to create src dir: {e}"))?;
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    let discount = 10;
    if amount >= threshold {
        amount - discount
    } else {
        amount
    }
}

pub fn unrelated() -> i32 {
    0
}
"#,
        )
        .map_err(|e| format!("failed to write src/lib.rs: {e}"))?;

        let owners = owner_symbols_for_lines(
            &root,
            &[
                (PathBuf::from("src/lib.rs"), 3),
                (PathBuf::from("src/lib.rs"), 11),
            ],
        )?;

        if !owners
            .iter()
            .any(|owner| owner.line == 3 && owner.owner.ends_with("discounted_total"))
        {
            return Err(format!("expected discounted_total owner, got {owners:?}"));
        }
        if !owners
            .iter()
            .any(|owner| owner.line == 11 && owner.owner.ends_with("unrelated"))
        {
            return Err(format!("expected unrelated owner, got {owners:?}"));
        }
        Ok(())
    }

    #[test]
    fn repo_analysis_excludes_test_files_from_probe_seed() -> Result<(), String> {
        let root = temp_dir("repo_exclude_tests");
        fs::create_dir_all(root.join("src"))
            .map_err(|e| format!("failed to create src dir: {e}"))?;
        fs::create_dir_all(root.join("tests"))
            .map_err(|e| format!("failed to create tests dir: {e}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='x'\nversion='0.1.0'\nedition='2024'\n",
        )
        .map_err(|e| format!("failed to write Cargo.toml: {e}"))?;
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn dummy() {
}
"#,
        )
        .map_err(|e| format!("failed to write src/lib.rs: {e}"))?;
        fs::write(
            root.join("tests/test_file.rs"),
            r#"
#[test]
fn test_with_predicate() {
    let x = 5;
    if x > 3 {
        assert!(true);
    }
}
"#,
        )
        .map_err(|e| format!("failed to write tests/test_file.rs: {e}"))?;

        let out = run_repo_analysis(&AnalysisOptions {
            root,
            base: None,
            diff_file: None,
            mode: AnalysisMode::Draft,
            include_unchanged_tests: true,
            resolve_tsconfig_paths: false,
            perl_facts_path: None,
        })?;

        for finding in &out.findings {
            let file_str = finding.probe.location.file.to_string_lossy().to_lowercase();
            if file_str.contains("test") || file_str.contains("tests") {
                return Err(format!(
                    "expected no findings from test files, but found one at {}",
                    file_str
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn empty_diff_yields_zero_diff_findings_but_repo_has_findings() -> Result<(), String> {
        let root = temp_dir("repo_vs_diff");
        fs::create_dir_all(root.join("src"))
            .map_err(|e| format!("failed to create src dir: {e}"))?;
        fs::create_dir_all(root.join("tests"))
            .map_err(|e| format!("failed to create tests dir: {e}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='x'\nversion='0.1.0'\nedition='2024'\n",
        )
        .map_err(|e| format!("failed to write Cargo.toml: {e}"))?;
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn price(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#,
        )
        .map_err(|e| format!("failed to write src/lib.rs: {e}"))?;
        fs::write(
            root.join("tests/pricing.rs"),
            r#"
#[test]
fn premium_customer_gets_discount() {
    let total = x::price(10000, 100);
    assert!(total > 0);
}
"#,
        )
        .map_err(|e| format!("failed to write tests/pricing.rs: {e}"))?;
        fs::write(
            root.join("empty.patch"),
            r#"diff --git a/src/lib.rs b/src/lib.rs
index 0000000..1111111 100644
--- a/src/lib.rs
+++ b/src/lib.rs
"#,
        )
        .map_err(|e| format!("failed to write empty.patch: {e}"))?;

        let diff_out = run_analysis(&AnalysisOptions {
            root: root.clone(),
            base: None,
            diff_file: Some(root.join("empty.patch")),
            mode: AnalysisMode::Draft,
            include_unchanged_tests: true,
            resolve_tsconfig_paths: false,
            perl_facts_path: None,
        })?;

        if !diff_out.findings.is_empty() {
            return Err("expected zero findings from empty diff".to_string());
        }

        let repo_out = run_repo_analysis(&AnalysisOptions {
            root,
            base: None,
            diff_file: None,
            mode: AnalysisMode::Draft,
            include_unchanged_tests: true,
            resolve_tsconfig_paths: false,
            perl_facts_path: None,
        })?;

        if repo_out.findings.is_empty() {
            return Err("expected at least one finding from repo analysis".to_string());
        }
        Ok(())
    }
}
