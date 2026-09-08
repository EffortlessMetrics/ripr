#[cfg(feature = "lang-perl")]
use super::language::PerlAdapter;
#[cfg(feature = "lang-python")]
use super::language::PythonAdapter;
#[cfg(feature = "lang-typescript")]
use super::language::TypeScriptAdapter;
use super::language::{
    LanguageAdapter, LanguageDiffResult, LanguageId, LanguageRepoResult, PartialDiffScope,
    RustAdapter, route,
};
use super::{
    AnalysisOptions, AnalysisResult, LanguageRun, LanguageRunStatus, PreviewLanguageAdvisory, diff,
    sort, summary,
};
use crate::analysis::cancellation;
use crate::analysis_outcome::{
    AnalysisIdentity, AnalysisLimitation, AnalysisLimitationKind, AnalysisOutcome,
    AnalysisOutcomeCounts, AnalysisOutcomeKind, AnalysisRecovery, AnalysisRecoveryKind,
    AnalysisStage,
};
use crate::config::OraclePolicy;
use crate::domain::Finding;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

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
    run_diff_pipeline_with_oracle_policy_and_generated_file_patterns(
        options,
        oracle_policy,
        languages,
        &[],
    )
}

pub(crate) fn run_diff_pipeline_with_oracle_policy_and_generated_file_patterns(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
    languages: &[LanguageId],
    generated_file_patterns: &[String],
) -> Result<AnalysisResult, String> {
    // Immutable Git candidate subject (#3237 / #3277): resolve the
    // bound identity through object plumbing, derive the exact
    // base→candidate diff, and analyze the materialized candidate root.
    // The worktree, index, `--diff` file, and `base` are never consulted
    // (the binding layer already rejects those combinations).
    if let Some(subject) = options.git_candidate.as_ref() {
        let resolved = super::git_candidate_execution::resolve(subject, options.git_timeout)
            .map_err(|error| error.to_string())?;
        if crate::is_verbose() {
            eprintln!(
                "ripr: immutable git candidate resolved: {}",
                super::git_candidate_execution::subject_identity(&resolved)
            );
        }
        let subject_identity_outcome = crate::analysis_outcome::GitCandidateSubjectIdentity {
            subject_kind: "tree_to_tree".to_string(),
            base_tree: resolved.base_tree.clone(),
            candidate_tree: resolved.candidate_tree.clone(),
            diff_identity: format!(
                "sha256:{}",
                Sha256::digest(resolved.diff.as_bytes()).iter().fold(
                    String::new(),
                    |mut acc, byte| {
                        use std::fmt::Write as _;
                        let _ = write!(acc, "{byte:02x}");
                        acc
                    }
                )
            ),
        };
        let candidate_options = AnalysisOptions {
            root: resolved.root.clone(),
            base: None,
            diff_file: None,
            git_candidate: None,
            resolved_subject_identity: Some(subject_identity_outcome),
            ..options.clone()
        };
        cancellation::checkpoint()?;
        let mut result = run_pipeline_for_diff_text(
            &candidate_options,
            oracle_policy,
            languages,
            generated_file_patterns,
            &resolved.diff,
        )?;
        // #3279 R4: finding locations name the user's repository, not
        // the ephemeral materialization directory — the temp root is
        // machine-local and unreplayable. The relative path inside the
        // candidate tree is unchanged; only the prefix is rebased from
        // the materialized root back to the named repository root at
        // the same seam that set it.
        rebase_finding_paths_to_repository(&mut result, &resolved.root, &options.root);
        return Ok(result);
    }
    let diff_text = diff::load_diff(
        &options.root,
        options.base.as_deref(),
        options.diff_file.as_ref(),
        options.git_timeout,
    )?;
    cancellation::checkpoint()?;
    run_pipeline_for_diff_text(
        options,
        oracle_policy,
        languages,
        generated_file_patterns,
        &diff_text,
    )
}

pub(crate) fn run_worktree_pipeline_with_oracle_policy_and_generated_file_patterns(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
    languages: &[LanguageId],
    generated_file_patterns: &[String],
) -> Result<AnalysisResult, String> {
    if options.diff_file.is_some() {
        return Err("worktree diff mode cannot be combined with --diff".to_string());
    }
    // #3237/#3277: the immutable subject's contract is exact-tree diff
    // semantics. Worktree mode analyzes the live tree by definition, so
    // a subject input must fail closed here rather than silently fall
    // back to worktree bytes (review blocker: the bound subject was
    // previously ignored in this mode).
    if options.git_candidate.is_some() {
        return Err(crate::domain::GitCandidateSubjectError::ExecutionFailed {
            detail: "git candidate subjects are diff-semantics inputs; worktree mode cannot execute them"
                .to_string(),
        }
        .to_string());
    }
    let diff_text =
        diff::load_worktree_diff(&options.root, options.base.as_deref(), options.git_timeout)?;
    cancellation::checkpoint()?;
    run_pipeline_for_diff_text(
        options,
        oracle_policy,
        languages,
        generated_file_patterns,
        &diff_text,
    )
}

/// The docs-only disclosure message (#2304): `Some(message)` when the diff
/// changed at least one file but none of them routes to a source adapter.
/// An empty result in that case needs the explicit explanation that ripr
/// saw no analyzable source files — never a silent clean-looking zero
/// (#1888). Pure so the disclosure contract is testable without capturing
/// stderr.
fn non_source_disclosure_message(changed_files: &[diff::ChangedFile]) -> Option<String> {
    if changed_files.is_empty() || changed_files.iter().any(|file| route(&file.path).is_some()) {
        return None;
    }
    let non_source_count = changed_files.len();
    let extensions: Vec<String> = changed_files
        .iter()
        .filter_map(|file| {
            file.path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| format!(".{ext}"))
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let ext_summary = if extensions.is_empty() {
        "extensionless files".to_string()
    } else {
        extensions.join(", ")
    };
    Some(format!(
        "ripr: diff contained {non_source_count} non-source file(s) ({ext_summary}); \
         no analyzable Rust, TypeScript, Python, or Perl files found. \
         The empty result is correct — ripr cannot analyze non-source files."
    ))
}

fn deletion_disclosure_message(deleted_file_count: usize) -> Option<String> {
    (deleted_file_count > 0).then(|| {
        format!(
            "ripr: diff deleted {deleted_file_count} file(s); deletion-only diffs produce no probes \
             (deleted behavior has no new-side code to analyze)."
        )
    })
}

fn submodule_disclosure_message(submodule_file_count: usize) -> Option<String> {
    (submodule_file_count > 0).then(|| {
        format!(
            "ripr: skipped {submodule_file_count} submodule pointer change(s); \
             ripr does not analyze submodule contents."
        )
    })
}

fn emit_submodule_disclosure(submodule_file_count: usize, mut emit: impl FnMut(&str)) {
    if let Some(message) = submodule_disclosure_message(submodule_file_count) {
        emit(&message);
    }
}

fn emit_rename_disclosure(
    renamed_file_count: usize,
    pure_rename_file_count: usize,
    mut emit: impl FnMut(&str),
) {
    if let Some(message) = rename_disclosure_message(renamed_file_count, pure_rename_file_count) {
        emit(&message);
    }
}

fn rename_disclosure_message(
    renamed_file_count: usize,
    pure_rename_file_count: usize,
) -> Option<String> {
    (renamed_file_count > 0).then(|| {
        let edited_count = renamed_file_count.saturating_sub(pure_rename_file_count);
        match (pure_rename_file_count, edited_count) {
            (pure, 0) => format!(
                "ripr: detected {pure} pure rename(s) with no changed lines; rename-only changes produce no probes."
            ),
            (0, edited) => format!(
                "ripr: detected {edited} rename(s); changed lines are analyzed under the new path, but old-path test associations are not carried forward."
            ),
            (pure, edited) => format!(
                "ripr: detected {pure} pure rename(s) with no changed lines and {edited} rename(s) with edits; changed lines are analyzed under new paths, but old-path test associations are not carried forward."
            ),
        }
    })
}

fn run_pipeline_for_diff_text(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
    languages: &[LanguageId],
    generated_file_patterns: &[String],
    diff_text: &str,
) -> Result<AnalysisResult, String> {
    let parsed_diff = diff::parse_unified_diff_bounded_with_metadata(diff_text)?;
    let changed_files = parsed_diff.changed_files;
    let mut limitations = parsed_diff.limitations;
    let mut harness_projections: Vec<crate::analysis::harness_projection::TestHarnessProjection> =
        Vec::new();
    let deleted_file_count = parsed_diff.deleted_file_count;
    let submodule_file_count = parsed_diff.submodule_file_count;
    let renamed_file_count = parsed_diff.renamed_file_count;
    let pure_rename_file_count = parsed_diff.pure_rename_file_count;
    let pure_rename_paths = parsed_diff.pure_rename_paths;
    let analysis_changed_files = changed_files
        .iter()
        .filter(|file| !pure_rename_paths.contains(&file.path))
        .cloned()
        .collect::<Vec<_>>();

    let mut findings: Vec<Finding> = Vec::new();
    // `changed_rust_files` counts Rust adapter files only (#2103); every
    // adapter that ran records its own count in `changed_files_by_language`.
    let mut rust_changed_files: usize = 0;
    let mut changed_files_by_language: Vec<(LanguageId, usize)> = Vec::new();
    let mut language_runs: Vec<LanguageRun> = Vec::new();
    let mut partial_scope: Option<PartialDiffScope> = None;
    let mut candidate_line_count = 0usize;

    // Rust (the stable reference adapter) runs first, ahead of the preview
    // loop, so its partial-diff partition (RIPR-PROP-0019, #1999) governs the
    // changed-file set handed to every preview adapter below: a
    // `limited_partial_scope` run analyzes exactly the selected partition in
    // every language, never a per-language mix. Rust failures still propagate
    // via `?`: a Rust failure is infra-class (e.g. diff_scope_oversized,
    // partial_budget_invalid), not a per-language advisory gap.
    if languages.contains(&LanguageId::Rust) {
        cancellation::checkpoint()?;
        let result = RustAdapter.analyze_diff_for_languages_with_generated_file_patterns(
            options,
            oracle_policy,
            &analysis_changed_files,
            languages,
            generated_file_patterns,
        )?;
        cancellation::checkpoint()?;
        if result.skipped_files > 0 {
            limitations.push(
                AnalysisLimitation::new(
                    AnalysisLimitationKind::LanguageScopeUnsupported,
                    AnalysisStage::LanguageAdapter,
                    AnalysisRecovery::new(
                        AnalysisRecoveryKind::Retry,
                        "Review the configured generated-file predicate and re-run the analysis.",
                    )?,
                )
                .with_affected_items(result.skipped_files as u64)?
                .with_detail(format!(
                    "{} generated Rust file(s) were intentionally skipped by the configured generated-file predicate",
                    result.skipped_files
                ))?,
            );
        }
        limitations.extend(result.limitations);
        partial_scope = result.partial_scope.clone();
        harness_projections.extend(result.harness_projections);
        findings.extend(result.findings);
        rust_changed_files += result.changed_files;
        candidate_line_count += result.candidate_line_count;
        changed_files_by_language.push((LanguageId::Rust, result.changed_files));
    }
    // When the Rust adapter returned a partial partition, preview adapters
    // analyze only the selected files; uninspected accounting lives on the
    // scope record. Otherwise the full diff is dispatched as before.
    let restricted_changed_files;
    let preview_changed_files: &[diff::ChangedFile] = match &partial_scope {
        Some(scope) => {
            restricted_changed_files = analysis_changed_files
                .iter()
                .filter(|file| scope.selects(&file.path))
                .cloned()
                .collect::<Vec<_>>();
            &restricted_changed_files
        }
        None => &changed_files,
    };

    for language in languages {
        cancellation::checkpoint()?;
        // Non-abort contract (Campaign 31 PR 10, #1403): a preview-language
        // adapter failure must not abort the report — the failed language is
        // recorded in `language_runs` and the other languages' findings still
        // emit. Rust ran above and already propagated any failure.
        if matches!(language, LanguageId::Rust) {
            continue;
        }
        let attempted = match language {
            LanguageId::TypeScript | LanguageId::JavaScript => {
                analyze_typescript_diff(options, oracle_policy, preview_changed_files)
            }
            LanguageId::Python => {
                analyze_python_diff(options, oracle_policy, preview_changed_files)
            }
            LanguageId::Perl => analyze_perl_diff(options, oracle_policy, preview_changed_files),
            LanguageId::Rust => {
                continue;
            }
        };
        match attempted {
            Ok(result) => {
                cancellation::checkpoint()?;
                candidate_line_count += result
                    .candidate_line_count
                    .max(candidate_lines_from_findings(&result.findings));
                findings.extend(result.findings);
                limitations.extend(result.limitations);
                if result.changed_files_by_language.is_empty() {
                    changed_files_by_language.push((*language, result.changed_files));
                } else {
                    // An adapter covering several output languages (the
                    // TypeScript adapter handles .js/.jsx as javascript)
                    // reports its own split (#2103 review).
                    changed_files_by_language.extend(result.changed_files_by_language);
                }
            }
            Err(reason) => language_runs.push(LanguageRun {
                language: language.as_str().to_string(),
                status: perl_run_status_for_err(&reason),
                reason: Some(reason),
            }),
        }
    }

    // Detect preview-language files in the diff regardless of whether the
    // adapter is enabled, so an empty result is never silently presented as a
    // clean Rust-grade result for a TypeScript/JavaScript/Python change
    // (RIPR-SPEC-0082, #1111). Detection is pure path routing — it does not
    // require the adapter to be enabled.
    let preview_paths: Vec<&diff::ChangedFile> = analysis_changed_files.iter().collect();
    let preview_advisories = detect_preview_advisories(languages, preview_paths.into_iter());
    for advisory in &preview_advisories {
        if !advisory.enabled {
            limitations.push(
                AnalysisLimitation::new(
                    AnalysisLimitationKind::LanguageAdapterUnavailable,
                    AnalysisStage::LanguageAdapter,
                    AnalysisRecovery::new(
                        AnalysisRecoveryKind::EnableLanguage,
                        format!(
                            "Enable the {} preview adapter and re-run the analysis.",
                            advisory.language
                        ),
                    )?,
                )
                .with_affected_items(advisory.file_count as u64)?
                .with_detail(format!(
                    "{} changed {} file(s), but the preview adapter was not enabled or available",
                    advisory.language, advisory.file_count
                ))?,
            );
        }
    }

    // Disclose when the diff contains only non-source files (docs/config-only
    // PRs): an empty result with zero probes is not a "clean" result in that
    // case — the user needs to know ripr saw no analyzable source files
    // (#1888, #2304).
    if findings.is_empty()
        && rust_changed_files == 0
        && submodule_file_count == 0
        && renamed_file_count == 0
        && changed_files_by_language
            .iter()
            .all(|(_, count)| *count == 0)
        && let Some(message) = non_source_disclosure_message(&changed_files)
    {
        // Emit as stderr disclosure — this is not a Finding (no probe was
        // generated), but the user needs to know why the result is empty.
        eprintln!("{message}");
    }

    emit_submodule_disclosure(submodule_file_count, |message| eprintln!("{message}"));

    emit_rename_disclosure(renamed_file_count, pure_rename_file_count, |message| {
        eprintln!("{message}");
    });

    if findings.is_empty()
        && rust_changed_files == 0
        && changed_files_by_language
            .iter()
            .all(|(_, count)| *count == 0)
        && let Some(message) = deletion_disclosure_message(deleted_file_count)
    {
        eprintln!("{message}");
    }

    // Disclose when the diff parsed to zero changed files (#2425): the input
    // may be a non-diff file (a log, a source file, random text) or a
    // malformed diff with no parseable hunks. Without this disclosure, the
    // "0 probe(s)" output looks like a clean bill of health when the input
    // was garbage. This is the honesty complement to #2304: #2304 handles
    // "diff had files but no source"; this handles "diff had no files at all."
    if findings.is_empty()
        && rust_changed_files == 0
        && changed_files.is_empty()
        && deleted_file_count == 0
        && submodule_file_count == 0
        && renamed_file_count == 0
        && changed_files_by_language
            .iter()
            .all(|(_, count)| *count == 0)
        && !diff_text.trim().is_empty()
    {
        eprintln!(
            "ripr: the diff input contained no parseable file changes (0 hunks, 0 files). \
             If this is unexpected, verify the --diff path points to a valid unified diff. \
             The empty result may not reflect sufficient tests — it reflects an empty analysis scope."
        );
    }

    if !diff_text.trim().is_empty()
        && changed_files.is_empty()
        && deleted_file_count == 0
        && submodule_file_count == 0
        && renamed_file_count == 0
        && limitations.is_empty()
    {
        limitations.push(
            AnalysisLimitation::new(
                AnalysisLimitationKind::MalformedDiff,
                AnalysisStage::DiffParse,
                AnalysisRecovery::new(
                    AnalysisRecoveryKind::Retry,
                    "Provide a valid unified diff and re-run the analysis.",
                )?,
            )
            .with_detail(
                "The non-empty diff input contained no parseable file changes or hunks.",
            )?,
        );
    }

    sort::sort_findings(&mut findings);
    cancellation::checkpoint()?;
    let mut summary_result = summary::summarize_findings(rust_changed_files, &findings);
    for (language, files) in changed_files_by_language {
        summary_result.record_changed_files_by_language(language.as_str(), files);
    }

    limitations.extend(limitations_from_language_runs(&language_runs)?);
    if let Some(scope) = &partial_scope {
        limitations.push(
            AnalysisLimitation::new(
                AnalysisLimitationKind::DiffScopeOversized,
                AnalysisStage::AnalysisPipeline,
                AnalysisRecovery::new(
                    AnalysisRecoveryKind::IncreaseConfiguredLimit,
                    "Raise RIPR_PARTIAL_DIFF_FILE_BUDGET and/or RIPR_PARTIAL_DIFF_LINE_BUDGET, then re-run the analysis.",
                )?,
            )
            .with_affected_items(scope.uninspected_changed_lines_lower_bound.max(1) as u64)?
            .with_detail(format!(
                "The run analyzed {} changed line(s) and left at least {} changed line(s) outside the selected partition.",
                scope.selected_changed_lines, scope.uninspected_changed_lines_lower_bound
            ))?,
        );
    }

    let changed_line_count = changed_files
        .iter()
        .map(|file| {
            file.added_lines
                .len()
                .saturating_add(file.removed_lines.len())
        })
        .sum::<usize>();
    let kind = if limitations.iter().any(|limitation| {
        matches!(
            limitation.kind,
            AnalysisLimitationKind::CombinedHunkUnsupported
                | AnalysisLimitationKind::UnresolvedConflictMarkers
                | AnalysisLimitationKind::MalformedDiff
        )
    }) {
        AnalysisOutcomeKind::UnsupportedInput
    } else if !limitations.is_empty() {
        AnalysisOutcomeKind::PartialWithLimitations
    } else if changed_files.is_empty() {
        AnalysisOutcomeKind::NoScope
    } else if changed_line_count == 0 {
        AnalysisOutcomeKind::NoChangedLines
    } else if findings.is_empty() && candidate_line_count == 0 {
        AnalysisOutcomeKind::NoBehavioralCandidates
    } else if findings.is_empty() {
        AnalysisOutcomeKind::CompleteNoFindings
    } else {
        AnalysisOutcomeKind::CompleteWithFindings
    };
    let input_digest = Sha256::digest(diff_text.as_bytes());
    let input_identity = format!(
        "sha256:{}",
        input_digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let analysis_outcome = Some(AnalysisOutcome::new(
        kind,
        AnalysisIdentity {
            base_revision: options.base.clone(),
            input_identity: Some(input_identity),
            git_candidate_subject: options.resolved_subject_identity.clone(),
            ..AnalysisIdentity::default()
        },
        AnalysisOutcomeCounts {
            changed_file_count: changed_files.len() as u64,
            changed_line_count: changed_line_count as u64,
            candidate_line_count: candidate_line_count as u64,
            probe_count: findings.len() as u64,
            finding_count: findings.len() as u64,
        },
        limitations,
    )?);

    Ok(AnalysisResult {
        harness_projections,
        analysis_outcome,
        summary: summary_result,
        findings,
        preview_language_advisories: preview_advisories,
        language_runs,
        partial_scope,
    })
}

fn candidate_lines_from_findings(findings: &[Finding]) -> usize {
    findings
        .iter()
        .map(|finding| {
            (
                finding.probe.location.file.clone(),
                finding.probe.location.line,
            )
        })
        .collect::<BTreeSet<_>>()
        .len()
}

fn limitations_from_language_runs(
    language_runs: &[LanguageRun],
) -> Result<Vec<AnalysisLimitation>, String> {
    language_runs
        .iter()
        .map(|run| {
            let (kind, recovery) = match run.status {
                LanguageRunStatus::Unavailable | LanguageRunStatus::Invalid => (
                    AnalysisLimitationKind::LanguageAdapterUnavailable,
                    AnalysisRecoveryKind::EnableLanguage,
                ),
                LanguageRunStatus::Partial => (
                    AnalysisLimitationKind::ProducerFailure,
                    AnalysisRecoveryKind::InspectFailure,
                ),
                LanguageRunStatus::Ok => return Ok(None),
            };
            let detail = run
                .reason
                .clone()
                .unwrap_or_else(|| format!("{} adapter did not complete.", run.language));
            Ok(Some(
                AnalysisLimitation::new(
                    kind,
                    AnalysisStage::LanguageAdapter,
                    AnalysisRecovery::new(
                        recovery,
                        "Inspect the adapter result and re-run the analysis.",
                    )?,
                )
                .with_detail(bounded_language_run_detail(&run.language, &detail))?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()
        .map(|items| items.into_iter().flatten().collect())
}

fn bounded_language_run_detail(language: &str, reason: &str) -> String {
    let detail = format!("{language}: {reason}");
    detail
        .chars()
        .take(crate::analysis_outcome::MAX_ANALYSIS_LIMITATION_DETAIL_CHARS)
        .collect()
}

pub(crate) fn run_repo_pipeline_with_oracle_policy(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
    languages: &[LanguageId],
) -> Result<AnalysisResult, String> {
    // #3237/#3277: repo mode seeds probes from the live tree; a subject
    // input is a diff-semantics contract and fails closed here.
    if let Some(subject) = options.git_candidate.as_ref() {
        return Err(crate::domain::GitCandidateSubjectError::ExecutionFailed {
            detail: format!(
                "git candidate subjects are diff-semantics inputs; repo mode cannot execute subject `{}`",
                subject.candidate_tree.as_str()
            ),
        }
        .to_string());
    }

    run_repo_pipeline_with_oracle_policy_and_generated_file_patterns(
        options,
        oracle_policy,
        languages,
        &[],
    )
}

pub(crate) fn run_repo_pipeline_with_oracle_policy_and_generated_file_patterns(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
    languages: &[LanguageId],
    generated_file_patterns: &[String],
) -> Result<AnalysisResult, String> {
    let mut findings: Vec<Finding> = Vec::new();
    // Same accounting as the diff loop (#2103): `changed_rust_files` carries
    // the Rust adapter's count only; every adapter records its own count.
    let mut rust_production_files: usize = 0;
    let mut files_by_language: Vec<(LanguageId, usize)> = Vec::new();
    let mut language_runs: Vec<LanguageRun> = Vec::new();
    let mut rust_harness_projections: Vec<
        crate::analysis::harness_projection::TestHarnessProjection,
    > = Vec::new();
    for language in languages {
        cancellation::checkpoint()?;
        // Non-abort contract (see the diff loop above): preview-language
        // failures are recorded, Rust failures still propagate.
        if !is_preview_language(*language) {
            // Rust (stable) failures propagate via `?`.
            let result = RustAdapter.analyze_repo_with_generated_file_patterns(
                options,
                oracle_policy,
                generated_file_patterns,
            )?;
            cancellation::checkpoint()?;
            if result.skipped_files > 0 {
                language_runs.push(LanguageRun {
                    language: LanguageId::Rust.as_str().to_string(),
                    status: LanguageRunStatus::Partial,
                    reason: Some(format!(
                        "{} generated Rust file(s) skipped from static analysis by the configured generated-file predicate",
                        result.skipped_files
                    )),
                });
            }
            rust_harness_projections = result.harness_projections;
            findings.extend(result.findings);
            rust_production_files += result.production_files;
            files_by_language.push((LanguageId::Rust, result.production_files));
            continue;
        }
        let attempted = match language {
            LanguageId::TypeScript | LanguageId::JavaScript => {
                analyze_typescript_repo(options, oracle_policy)
            }
            LanguageId::Python => analyze_python_repo(options, oracle_policy),
            LanguageId::Perl => analyze_perl_repo(options, oracle_policy),
            LanguageId::Rust => {
                continue;
            }
        };
        match attempted {
            Ok(result) => {
                cancellation::checkpoint()?;
                // Mirror the Rust repo path's generated-file skip
                // disclosure: a capped/partial preview run records a
                // `Partial` language run on the shared channel, so human
                // and JSON output render the limitation and gates fail
                // closed on the partial denominator (#3554, #2109).
                if let Some(reason) = &result.partial_reason {
                    language_runs.push(LanguageRun {
                        language: language.as_str().to_string(),
                        status: LanguageRunStatus::Partial,
                        reason: Some(reason.clone()),
                    });
                }
                findings.extend(result.findings);
                files_by_language.push((*language, result.production_files));
            }
            Err(reason) => language_runs.push(LanguageRun {
                language: language.as_str().to_string(),
                status: perl_run_status_for_err(&reason),
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
    cancellation::checkpoint()?;
    let mut summary_result = summary::summarize_findings(rust_production_files, &findings);
    for (language, files) in files_by_language {
        summary_result.record_changed_files_by_language(language.as_str(), files);
    }

    Ok(AnalysisResult {
        harness_projections: rust_harness_projections,
        analysis_outcome: None,
        summary: summary_result,
        findings,
        preview_language_advisories: preview_advisories,
        language_runs,
        // Repo-scope analysis indexes the whole workspace; the partial
        // diff-selection budget (RIPR-PROP-0019) does not apply here.
        partial_scope: None,
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
        if !language.is_available() && *language != LanguageId::Perl {
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
            enabled: enabled.contains(language) && language.is_available(),
        });
    }
    advisories
}

/// Languages that route through a compiled preview adapter, in stable order.
const PREVIEW_LANGUAGE_ORDER: &[LanguageId] = &[
    LanguageId::TypeScript,
    LanguageId::JavaScript,
    LanguageId::Python,
    LanguageId::Perl,
];

/// Build preview-language advisories by routing a stream of paths, regardless
/// of whether the adapter is enabled.
///
/// Each path is routed via `analysis::language::route`. Files that route to a
/// compiled preview language (TypeScript/JavaScript, Python, or Perl) are
/// grouped by language. For each preview language with at least one file, one
/// advisory is emitted. Perl is disclosed even when its optional adapter is
/// not compiled in, because routing Perl files is required for the honesty
/// guard. `enabled` is `true` only when the language is present in `enabled`
/// (the active `[languages]` list) and its adapter is available;
/// analyzed under the default Rust-only config still breaks the silent
/// empty-result honesty gap (RIPR-SPEC-0082, #1111).
///
/// TypeScript/JavaScript/Python remain restricted to compiled-in adapters;
/// Perl is the exception because its file presence must remain visible even
/// when the optional adapter is unavailable.
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
        if !is_preview_language(language)
            || (!language.is_available() && language != LanguageId::Perl)
        {
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
                enabled: enabled.contains(language) && language.is_available(),
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

/// Classify a Perl adapter `Err` into a `LanguageRunStatus` (Campaign 31
/// item 2). A packet that was supplied but failed an ingestion integrity check
/// (fingerprint/coherence/capability/path/id/digest) surfaces as `Invalid` —
/// the producer emitted something untrustworthy, and that is distinct from the
/// adapter simply being unavailable (no packet path, feature off, read error,
/// parse error, or schema mismatch). The non-abort contract is preserved
/// either way: the run is recorded, never propagated.
fn perl_run_status_for_err(reason: &str) -> LanguageRunStatus {
    // Integrity-check failures all carry the `ingestion:` prefix emitted by
    // `PerlFactPacket::validate_ingestion`. Everything else is an
    // availability/config failure.
    if reason.starts_with("ingestion:") {
        LanguageRunStatus::Invalid
    } else {
        LanguageRunStatus::Unavailable
    }
}

#[cfg(feature = "lang-perl")]
fn analyze_perl_diff(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
    changed_files: &[diff::ChangedFile],
) -> Result<LanguageDiffResult, String> {
    PerlAdapter.analyze_diff(options, oracle_policy, changed_files)
}

#[cfg(not(feature = "lang-perl"))]
fn analyze_perl_diff(
    _options: &AnalysisOptions,
    _oracle_policy: &OraclePolicy,
    _changed_files: &[diff::ChangedFile],
) -> Result<LanguageDiffResult, String> {
    unavailable_language(LanguageId::Perl)
}

#[cfg(feature = "lang-perl")]
fn analyze_perl_repo(
    options: &AnalysisOptions,
    oracle_policy: &OraclePolicy,
) -> Result<LanguageRepoResult, String> {
    PerlAdapter.analyze_repo(options, oracle_policy)
}

#[cfg(not(feature = "lang-perl"))]
fn analyze_perl_repo(
    _options: &AnalysisOptions,
    _oracle_policy: &OraclePolicy,
) -> Result<LanguageRepoResult, String> {
    unavailable_language(LanguageId::Perl)
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

/// Rebase every finding/probe location prefix from the materialized
/// candidate root onto the named repository root (#3279 R4). The
/// relative path is preserved exactly; a path outside the materialized
/// root is left untouched (fail-open on the rewrite is honest — the
/// analyzer produced it from candidate bytes).
fn rebase_finding_paths_to_repository(
    result: &mut AnalysisResult,
    materialized_root: &std::path::Path,
    repository_root: &std::path::Path,
) {
    let rebase = |path: &std::path::Path| -> std::path::PathBuf {
        path.strip_prefix(materialized_root).map_or_else(
            |_| path.to_path_buf(),
            |relative| repository_root.join(relative),
        )
    };
    for finding in &mut result.findings {
        finding.probe.location.file = rebase(&finding.probe.location.file);
    }
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

    fn changed_file(path: &str) -> diff::ChangedFile {
        diff::ChangedFile {
            path: PathBuf::from(path),
            added_lines: Vec::new(),
            removed_lines: Vec::new(),
        }
    }

    #[test]
    fn non_source_disclosure_message_names_count_and_extensions() -> Result<(), String> {
        // #2304: a docs/config-only diff yields the disclosure naming the
        // changed-file count and the observed extensions, so an empty result
        // is explained instead of reading as a clean zero.
        let files = vec![
            changed_file("docs/ROADMAP.md"),
            changed_file("docs/CHANGELOG.md"),
            changed_file("policy/ci-budget.toml"),
        ];
        let message = non_source_disclosure_message(&files)
            .ok_or_else(|| "docs-only diff must produce the disclosure".to_string())?;
        if !message.contains("3 non-source file(s)") {
            return Err(format!("disclosure must name the file count: {message}"));
        }
        if !message.contains(".md") || !message.contains(".toml") {
            return Err(format!("disclosure must list extensions: {message}"));
        }
        if message.contains(".md, .md") {
            return Err(format!("extensions must be deduplicated: {message}"));
        }
        if !message.contains("empty result is correct") {
            return Err(format!("disclosure must carry the non-claim: {message}"));
        }
        Ok(())
    }

    #[test]
    fn non_source_disclosure_message_silent_for_source_or_empty_diffs() -> Result<(), String> {
        // #2304: the disclosure is scoped to the docs-only case — any
        // source-routed file, or an empty diff, suppresses it.
        let mixed = vec![
            changed_file("docs/ROADMAP.md"),
            changed_file("crates/ripr/src/lib.rs"),
        ];
        if non_source_disclosure_message(&mixed).is_some() {
            return Err("a diff containing a source file must not disclose".to_string());
        }
        let empty: Vec<diff::ChangedFile> = Vec::new();
        if non_source_disclosure_message(&empty).is_some() {
            return Err("an empty diff must not disclose".to_string());
        }
        Ok(())
    }

    #[test]
    fn malformed_diff_yields_zero_changed_files() {
        // #2425: a non-diff text file must parse to zero changed files. The
        // pipeline's empty-diff disclosure depends on this: if the parser
        // accidentally extracted file entries from non-diff text, the
        // disclosure would not fire and the user would see a silent "0 probes"
        // that looks like a clean result.
        let non_diff = "this is not a diff\njust some random text\n";
        let changed = diff::parse_unified_diff(non_diff);
        assert!(
            changed.is_empty(),
            "non-diff text must parse to zero changed files, got {}",
            changed.len()
        );
    }

    #[test]
    fn nonempty_unparseable_diff_projects_unsupported_outcome() -> Result<(), String> {
        let root = temp_root("analysis-outcome-malformed-diff")?;
        let result = run_pipeline_for_diff_text(
            &AnalysisOptions {
                root,
                base: None,
                diff_file: None,
                mode: AnalysisMode::Draft,
                resolved_subject_identity: None,
                include_unchanged_tests: false,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &[LanguageId::Rust],
            &[],
            "this is not a unified diff\n",
        )?;
        let outcome = result
            .analysis_outcome
            .ok_or_else(|| "malformed diff must carry an analysis outcome".to_string())?;
        assert_eq!(outcome.kind, AnalysisOutcomeKind::UnsupportedInput);
        assert!(outcome.limitations.iter().any(|limitation| {
            limitation.kind == AnalysisLimitationKind::MalformedDiff
                && limitation.producer_stage == AnalysisStage::DiffParse
        }));
        Ok(())
    }

    #[test]
    fn adapter_limitation_details_are_bounded_without_aborting_projection() -> Result<(), String> {
        let limitations = limitations_from_language_runs(&[LanguageRun {
            language: "typescript".to_string(),
            status: LanguageRunStatus::Partial,
            reason: Some("x".repeat(2_000)),
        }])?;
        let detail = limitations[0]
            .bounded_detail
            .as_deref()
            .ok_or_else(|| "bounded adapter limitation must retain a detail".to_string())?;
        assert!(detail.starts_with("typescript: "));
        assert!(
            detail.chars().count() <= crate::analysis_outcome::MAX_ANALYSIS_LIMITATION_DETAIL_CHARS
        );
        Ok(())
    }

    #[test]
    fn configured_generated_skip_is_a_scope_limitation_not_a_producer_failure() -> Result<(), String>
    {
        let root = temp_root("analysis-outcome-generated-skip")?;
        let result = run_pipeline_for_diff_text(
            &AnalysisOptions {
                root,
                base: None,
                diff_file: None,
                mode: AnalysisMode::Draft,
                resolved_subject_identity: None,
                include_unchanged_tests: false,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &[LanguageId::Rust],
            &["src/generated_*.rs".to_string()],
            "diff --git a/src/generated_values.rs b/src/generated_values.rs\n\
             --- /dev/null\n\
             +++ b/src/generated_values.rs\n\
             @@ -0,0 +1 @@\n\
             +pub fn generated_value() -> u32 { 1 }\n",
        )?;
        let outcome = result
            .analysis_outcome
            .ok_or_else(|| "generated skip must carry an analysis outcome".to_string())?;
        assert_eq!(outcome.kind, AnalysisOutcomeKind::PartialWithLimitations);
        assert!(outcome.limitations.iter().any(|limitation| {
            limitation.kind == AnalysisLimitationKind::LanguageScopeUnsupported
                && limitation
                    .bounded_detail
                    .as_deref()
                    .is_some_and(|detail| detail.contains("intentionally skipped"))
        }));
        assert!(
            !result
                .language_runs
                .iter()
                .any(|run| run.status == LanguageRunStatus::Partial)
        );
        Ok(())
    }

    #[test]
    fn deletion_disclosure_names_deleted_files_and_non_claim() -> Result<(), String> {
        let message = deletion_disclosure_message(2)
            .ok_or_else(|| "deleted files must produce a disclosure".to_string())?;
        if !message.contains("deleted 2 file(s)") {
            return Err(format!(
                "disclosure must name the deleted-file count: {message}"
            ));
        }
        if !message.contains("deletion-only diffs produce no probes") {
            return Err(format!(
                "disclosure must name the no-probe limitation: {message}"
            ));
        }
        if !message.contains("no new-side code to analyze") {
            return Err(format!("disclosure must carry the non-claim: {message}"));
        }
        if deletion_disclosure_message(0).is_some() {
            return Err("zero deleted files must not produce a disclosure".to_string());
        }
        Ok(())
    }

    #[test]
    fn rename_disclosure_names_pure_and_edited_renames_and_non_claim() -> Result<(), String> {
        let pure = rename_disclosure_message(2, 2)
            .ok_or_else(|| "pure renames must produce a disclosure".to_string())?;
        if !pure.contains("detected 2 pure rename(s)") || !pure.contains("no changed lines") {
            return Err(format!("pure rename disclosure is incomplete: {pure}"));
        }
        if !pure.contains("produce no probes") {
            return Err(format!(
                "pure rename disclosure must state the no-probe limit: {pure}"
            ));
        }

        let edited = rename_disclosure_message(1, 0)
            .ok_or_else(|| "edited renames must produce a disclosure".to_string())?;
        if !edited.contains("analyzed under the new path") {
            return Err(format!(
                "edited rename disclosure must name the new path: {edited}"
            ));
        }
        if !edited.contains("not carried forward") {
            return Err(format!(
                "edited rename disclosure must state the association limit: {edited}"
            ));
        }
        let mixed = rename_disclosure_message(2, 1)
            .ok_or_else(|| "mixed renames must produce a disclosure".to_string())?;
        if !mixed.contains("not carried forward") {
            return Err(format!(
                "mixed rename disclosure must state the association limit: {mixed}"
            ));
        }
        let mut emitted = None;
        emit_rename_disclosure(1, 1, |message| emitted = Some(message.to_string()));
        if emitted.as_deref()
            != Some(
                "ripr: detected 1 pure rename(s) with no changed lines; rename-only changes produce no probes.",
            )
        {
            return Err(format!(
                "pipeline emitted the wrong rename disclosure: {emitted:?}"
            ));
        }
        if rename_disclosure_message(0, 0).is_some() {
            return Err("zero renames must not produce a disclosure".to_string());
        }
        Ok(())
    }

    #[test]
    fn diff_pipeline_discloses_pure_rename_scope() -> Result<(), String> {
        let root = temp_root("pure-rename")?;
        write(
            &root.join("Cargo.toml"),
            "[package]\nname = \"rename-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )?;
        let diff_file = root.join("rename.diff");
        write(
            &diff_file,
            "diff --git a/src/old.rs b/src/new.rs\n\
             similarity index 100%\n\
             rename from src/old.rs\n\
             rename to src/new.rs\n",
        )?;

        let result = run_diff_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root: root.clone(),
                base: None,
                diff_file: Some(diff_file),
                mode: AnalysisMode::Draft,
                resolved_subject_identity: None,
                include_unchanged_tests: false,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &[LanguageId::Rust],
        )?;

        assert!(result.findings.is_empty());
        let mut disclosures = Vec::new();
        emit_rename_disclosure(1, 1, |message| disclosures.push(message.to_string()));
        assert_eq!(
            disclosures,
            vec![
                "ripr: detected 1 pure rename(s) with no changed lines; rename-only changes produce no probes."
            ]
        );
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn submodule_disclosure_names_skipped_pointer_changes_and_non_claim() -> Result<(), String> {
        let message = submodule_disclosure_message(2)
            .ok_or_else(|| "submodule changes must produce a disclosure".to_string())?;
        if !message.contains("skipped 2 submodule pointer change(s)") {
            return Err(format!("disclosure must name the count: {message}"));
        }
        if !message.contains("does not analyze submodule contents") {
            return Err(format!("disclosure must carry the non-claim: {message}"));
        }
        if submodule_disclosure_message(0).is_some() {
            return Err("zero submodule changes must not produce a disclosure".to_string());
        }
        Ok(())
    }

    #[test]
    fn submodule_pipeline_emits_exact_disclosure_message() {
        let mut emitted = None;
        emit_submodule_disclosure(1, |message| emitted = Some(message.to_string()));
        assert_eq!(
            emitted.as_deref(),
            Some(
                "ripr: skipped 1 submodule pointer change(s); ripr does not analyze submodule contents."
            )
        );
    }

    #[test]
    fn diff_pipeline_discloses_submodule_only_scope() -> Result<(), String> {
        let root = temp_root("submodule-only")?;
        write(
            &root.join("Cargo.toml"),
            "[package]\nname = \"submodule-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )?;
        let diff_file = root.join("submodule.diff");
        write(
            &diff_file,
            "diff --git a/vendor/lib b/vendor/lib\n\
             index 1111111..2222222 160000\n\
             --- a/vendor/lib\n\
             +++ b/vendor/lib\n\
             @@ -1 +1 @@\n\
             -Subproject commit 1111111\n\
             +Subproject commit 2222222\n\
             diff --git a/vendor/new b/vendor/new\n\
             new file mode 160000\n\
             index 0000000..3333333\n\
             --- /dev/null\n\
             +++ b/vendor/new\n\
             diff --git a/vendor/old b/vendor/old\n\
             deleted file mode 160000\n\
             index 4444444..0000000\n\
             --- a/vendor/old\n\
             +++ /dev/null\n",
        )?;

        let result = run_diff_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root: root.clone(),
                base: None,
                diff_file: Some(diff_file),
                mode: AnalysisMode::Draft,
                resolved_subject_identity: None,
                include_unchanged_tests: false,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &[LanguageId::Rust],
        )?;

        assert!(result.findings.is_empty());
        assert_eq!(result.summary.changed_rust_files, 0);
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
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
                resolved_subject_identity: None,
                include_unchanged_tests: false,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &[LanguageId::Rust],
        );
        // Rust failure on an invalid root propagates as a top-level Err.
        result.expect_err("expected Rust adapter failure to propagate");
    }

    #[test]
    fn diff_pipeline_projects_parser_limitation_and_distinguishes_complete_zero()
    -> Result<(), String> {
        let root = temp_root("analysis-outcome-projection")?;
        write(root.join("src/lib.rs").as_path(), "pub fn existing() {}\n")?;
        let options = AnalysisOptions {
            root: root.clone(),
            base: None,
            diff_file: None,
            mode: AnalysisMode::Draft,
            resolved_subject_identity: None,
            include_unchanged_tests: false,
            resolve_tsconfig_paths: false,
            perl_facts_path: None,
            git_timeout: None,
            git_candidate: None,
            production_like_targets: Default::default(),
            test_harnesses: Vec::new(),
        };
        let combined = "diff --cc src/lib.rs\n\
             index 1111111,2222222..3333333\n\
             --- a/src/lib.rs\n\
             +++ b/src/lib.rs\n\
             @@@ -1,1 -1,1 +1,1 @@@\n\
             -old\n\
             +new\n";
        let incomplete = run_pipeline_for_diff_text(
            &options,
            &OraclePolicy::default(),
            &[LanguageId::Rust],
            &[],
            combined,
        )?;
        let outcome = incomplete
            .analysis_outcome
            .ok_or_else(|| "combined diff must carry an analysis outcome".to_string())?;
        assert_eq!(outcome.kind, AnalysisOutcomeKind::UnsupportedInput);
        assert!(incomplete.findings.is_empty());
        assert_eq!(outcome.limitations.len(), 1);
        assert_eq!(
            outcome.limitations[0].kind,
            AnalysisLimitationKind::CombinedHunkUnsupported
        );

        let complete_zero = run_pipeline_for_diff_text(
            &options,
            &OraclePolicy::default(),
            &[LanguageId::Rust],
            &[],
            "diff --git a/docs/readme.md b/docs/readme.md\n\
             --- a/docs/readme.md\n\
             +++ b/docs/readme.md\n\
             @@ -1,1 +1,1 @@\n\
             -old\n\
             +new\n",
        )?;
        let outcome = complete_zero
            .analysis_outcome
            .ok_or_else(|| "ordinary zero result must carry an analysis outcome".to_string())?;
        assert_eq!(outcome.kind, AnalysisOutcomeKind::NoBehavioralCandidates);
        assert!(outcome.limitations.is_empty());
        assert!(complete_zero.findings.is_empty());

        let _ = fs::remove_dir_all(root);
        Ok(())
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
                resolved_subject_identity: None,
                include_unchanged_tests: false,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
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
                resolved_subject_identity: None,
                include_unchanged_tests: false,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
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

    /// Integrity-failure vs availability-failure status split (Campaign 31
    /// item 2). A packet that is SUPPLIED but fails an ingestion integrity
    /// check must surface as `LanguageRunStatus::Invalid` — distinct from the
    /// no-packet / feature-off path that stays `Unavailable`. The non-abort
    /// contract holds either way: the run is recorded, never propagated.
    #[cfg(feature = "lang-perl")]
    #[test]
    fn diff_pipeline_marks_integrity_failure_as_invalid() -> Result<(), String> {
        let root = temp_root("perl-integrity-invalid")?;
        // A packet with a fingerprint that does not match its recomputed value
        // — a tampered/stale packet that parses but fails validate_ingestion.
        let facts = root.join("bad-facts.json");
        write(
            &facts,
            r#"{
  "schema_version": "ripr-perl-facts-v1",
  "packet_id": "perl-facts:repo:bad",
  "packet_status": "complete",
  "packet_fingerprint": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
  "producer": {"name": "perl-lsp", "version": "0.0.0", "capabilities": ["syntax"]},
  "root": {"repo_relative": ".", "vcs_head": "abc", "path_style": "repo_relative"},
  "input": {"base": "origin/main", "head": "HEAD", "diff_id": null, "requested_fact_classes": []},
  "files": [], "owners": [], "changes": [], "tests": [], "oracles": [],
  "relations": [], "dynamic_boundaries": [], "verify_commands": [],
  "limitations": [], "provenance": []
}"#,
        )?;
        // A Perl file in the diff so the adapter is dispatched.
        let diff_file = root.join("perl.diff");
        write(
            &diff_file,
            "diff --git a/lib/App.pm b/lib/App.pm\n\
             --- /dev/null\n\
             +++ b/lib/App.pm\n\
             @@ -0,0 +1 @@\n\
             +sub discount { return 0 }\n",
        )?;

        let result = run_diff_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root: root.clone(),
                base: None,
                diff_file: Some(diff_file),
                mode: AnalysisMode::Draft,
                resolved_subject_identity: None,
                include_unchanged_tests: false,
                resolve_tsconfig_paths: false,
                perl_facts_path: Some(facts),
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &[LanguageId::Rust, LanguageId::Perl],
        );
        let analysis = match result {
            Ok(a) => a,
            Err(reason) => {
                return Err(format!(
                    "pipeline aborted on Perl integrity failure, expected non-abort: {reason}"
                ));
            }
        };
        let perl_run = analysis
            .language_runs
            .iter()
            .find(|run| run.language == "perl")
            .ok_or_else(|| "expected a perl language_run entry".to_string())?;
        assert_eq!(
            perl_run.status,
            super::LanguageRunStatus::Invalid,
            "an integrity-check failure must be `invalid`, not `unavailable`"
        );
        assert!(
            perl_run
                .reason
                .as_deref()
                .unwrap_or("")
                .starts_with("ingestion:"),
            "the reason must be the ingestion-check message, got: {:?}",
            perl_run.reason
        );
        let advisory = analysis
            .preview_language_advisories
            .iter()
            .find(|advisory| advisory.language == "perl")
            .ok_or_else(|| "expected an enabled Perl preview advisory".to_string())?;
        assert!(advisory.enabled, "the Perl adapter remains enabled");
        assert_eq!(advisory.file_count, 1);
        assert!(
            !advisory.analyzed(&analysis.language_runs),
            "an invalid adapter run must not claim the routed file was analyzed"
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
                resolved_subject_identity: None,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &[LanguageId::TypeScript, LanguageId::Python],
        )?;

        assert!(result.findings.is_empty());
        // #2103: preview-language file counts must not inflate the Rust count.
        assert_eq!(result.summary.changed_rust_files, 0);
        let per_language: Vec<(&str, usize)> = result
            .summary
            .changed_files_by_language
            .iter()
            .map(|count| (count.language.as_str(), count.files))
            .collect();
        assert_eq!(per_language, vec![("python", 1), ("typescript", 1)]);
        Ok(())
    }

    /// #2103: a mixed Rust+Python diff must attribute `changed_rust_files` to
    /// the Rust adapter only; every adapter that ran records its own count in
    /// `changed_files_by_language`.
    #[cfg(feature = "lang-python")]
    #[test]
    fn diff_pipeline_attributes_changed_files_per_language() -> Result<(), String> {
        let root = temp_root("mixed-rust-python")?;
        let src = root.join("src/lib.rs");
        write(&src, "pub fn discount(price: u32) -> u32 { price / 2 }\n")?;
        let diff_file = root.join("mixed.diff");
        write(
            &diff_file,
            "diff --git a/src/lib.rs b/src/lib.rs\n\
             --- a/src/lib.rs\n\
             +++ b/src/lib.rs\n\
             @@ -1 +1 @@\n\
             -pub fn discount(price: u32) -> u32 { price / 2 }\n\
             +pub fn discount(price: u32) -> u32 { price / 4 }\n\
             diff --git a/app/main.py b/app/main.py\n\
             --- /dev/null\n\
             +++ b/app/main.py\n\
             @@ -0,0 +1 @@\n\
             +def price(): return 1\n",
        )?;

        let result = run_diff_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root: root.clone(),
                base: None,
                diff_file: Some(diff_file),
                mode: AnalysisMode::Draft,
                resolved_subject_identity: None,
                include_unchanged_tests: false,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &[LanguageId::Rust, LanguageId::Python],
        )?;

        assert_eq!(
            result.summary.changed_rust_files, 1,
            "changed_rust_files must count Rust adapter files only (#2103)"
        );
        let per_language: Vec<(&str, usize)> = result
            .summary
            .changed_files_by_language
            .iter()
            .map(|count| (count.language.as_str(), count.files))
            .collect();
        assert_eq!(
            per_language,
            vec![("python", 1), ("rust", 1)],
            "every adapter that ran must record its own changed-file count"
        );

        let _ = std::fs::remove_dir_all(&root);
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
                resolved_subject_identity: None,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
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

    /// A parser failure owned by a preview adapter must remain a typed
    /// limitation in the shared outcome; a finding-side warning alone is not
    /// enough to make the completeness contract machine-consumable.
    #[cfg(feature = "lang-typescript")]
    #[test]
    fn diff_pipeline_projects_typescript_adapter_limitation() -> Result<(), String> {
        let root = temp_root("analysis-outcome-typescript-limitation")?;
        write(root.join("src/broken.ts").as_path(), "const = ;\n")?;
        let diff_file = root.join("broken.diff");
        write(
            &diff_file,
            r#"diff --git a/src/broken.ts b/src/broken.ts
index 0000000..1111111 100644
--- /dev/null
+++ b/src/broken.ts
@@ -0,0 +1 @@
+const = ;
"#,
        )?;

        let result = run_diff_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root: root.clone(),
                base: None,
                diff_file: Some(diff_file),
                mode: AnalysisMode::Draft,
                resolved_subject_identity: None,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &[LanguageId::TypeScript],
        )?;
        let outcome = result
            .analysis_outcome
            .ok_or_else(|| "TypeScript diff must carry an analysis outcome".to_string())?;
        assert_eq!(outcome.kind, AnalysisOutcomeKind::PartialWithLimitations);
        let limitation = outcome
            .limitations
            .iter()
            .find(|limitation| limitation.kind == AnalysisLimitationKind::LanguageScopeUnsupported)
            .ok_or_else(|| "missing typed TypeScript adapter limitation".to_string())?;
        assert_eq!(limitation.producer_stage, AnalysisStage::LanguageAdapter);
        assert_eq!(limitation.path.as_deref(), Some("src/broken.ts"));
        assert_eq!(limitation.recovery.kind, AnalysisRecoveryKind::Retry);

        let _ = std::fs::remove_dir_all(&root);
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
                resolved_subject_identity: None,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
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
        let outcome = result
            .analysis_outcome
            .ok_or_else(|| "disabled preview diff must carry an analysis outcome".to_string())?;
        assert_eq!(outcome.kind, AnalysisOutcomeKind::PartialWithLimitations);
        assert!(outcome.limitations.iter().any(|limitation| {
            limitation.kind == AnalysisLimitationKind::LanguageAdapterUnavailable
                && limitation
                    .bounded_detail
                    .as_deref()
                    .is_some_and(|detail| detail.contains("not enabled or available"))
        }));
        Ok(())
    }

    #[cfg(not(feature = "lang-perl"))]
    #[test]
    fn diff_pipeline_emits_not_enabled_advisory_for_perl_without_adapter() -> Result<(), String> {
        let root = temp_root("spec-0082-perl-not-enabled")?;
        let diff_file = root.join("perl.diff");
        write(
            &diff_file,
            r#"diff --git a/lib/My/App.pm b/lib/My/App.pm
--- /dev/null
+++ b/lib/My/App.pm
@@ -0,0 +1 @@
+sub value { return 1 }
"#,
        )?;

        let result = run_diff_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root,
                base: None,
                diff_file: Some(diff_file),
                mode: AnalysisMode::Draft,
                resolved_subject_identity: None,
                include_unchanged_tests: false,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &[LanguageId::Rust],
        )?;

        let advisory = result
            .preview_language_advisories
            .iter()
            .find(|advisory| advisory.language == "perl")
            .ok_or_else(|| "expected a Perl preview advisory".to_string())?;
        assert_eq!(advisory.file_count, 1);
        assert!(!advisory.enabled);
        assert_eq!(advisory.sample_paths, vec!["lib/My/App.pm"]);
        Ok(())
    }

    #[cfg(not(feature = "lang-perl"))]
    #[test]
    fn diff_pipeline_keeps_configured_unavailable_perl_disabled() -> Result<(), String> {
        let root = temp_root("spec-0082-perl-configured-unavailable")?;
        let diff_file = root.join("perl.diff");
        write(
            &diff_file,
            r#"diff --git a/lib/My/App.pm b/lib/My/App.pm
--- /dev/null
+++ b/lib/My/App.pm
@@ -0,0 +1 @@
+sub value { return 1 }
"#,
        )?;

        let result = run_diff_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root,
                base: None,
                diff_file: Some(diff_file),
                mode: AnalysisMode::Draft,
                resolved_subject_identity: None,
                include_unchanged_tests: false,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &[LanguageId::Rust, LanguageId::Perl],
        )?;

        let advisory = result
            .preview_language_advisories
            .iter()
            .find(|advisory| advisory.language == "perl")
            .ok_or_else(|| "expected a Perl preview advisory".to_string())?;
        assert!(!advisory.enabled);
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
                resolved_subject_identity: None,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
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
        // #2103: a Rust-only run records a single `rust` entry.
        let languages: Vec<&str> = result
            .summary
            .changed_files_by_language
            .iter()
            .map(|count| count.language.as_str())
            .collect();
        assert_eq!(
            languages,
            vec!["rust"],
            "Rust-only run must record only a rust per-language entry"
        );
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
                resolved_subject_identity: None,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &[LanguageId::TypeScript, LanguageId::Python],
        )?;

        assert!(result.findings.is_empty());
        assert_eq!(result.summary.changed_rust_files, 0);
        Ok(())
    }

    /// Mixed Rust/Python repo reconciliation (#3554 PR C, #2103): each
    /// language keeps its own production-file count, Python evidence adds no
    /// weight to `changed_rust_files`, and Python findings keep their native
    /// identity.
    #[cfg(feature = "lang-python")]
    #[test]
    fn repo_pipeline_mixed_rust_python_keeps_per_language_counts() -> Result<(), String> {
        use crate::domain::LanguageId as DomainLanguageId;
        let root = temp_root("mixed-repo")?;
        write(
            &root.join("src").join("lib.rs"),
            "pub fn discount(price: u32) -> u32 { price / 2 }\n",
        )?;
        write(&root.join("app.py"), "def run():\n    return 1\n")?;
        write(
            &root.join("test_app.py"),
            "from app import run\n\n\ndef test_run():\n    assert run() == 1\n",
        )?;

        let result = run_repo_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root,
                base: None,
                diff_file: None,
                mode: AnalysisMode::Deep,
                resolved_subject_identity: None,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &[LanguageId::Rust, LanguageId::Python],
        )?;

        // Both languages produced evidence.
        assert!(
            result
                .findings
                .iter()
                .any(|finding| finding.language == Some(DomainLanguageId::Rust)),
            "expected at least one Rust finding"
        );
        assert!(
            result
                .findings
                .iter()
                .any(|finding| finding.language == Some(DomainLanguageId::Python)),
            "expected at least one Python finding"
        );
        // #2103: `changed_rust_files` counts the Rust adapter only.
        assert_eq!(
            result.summary.changed_rust_files, 1,
            "Python production files must never inflate changed_rust_files"
        );
        let counts: Vec<(&str, usize)> = result
            .summary
            .changed_files_by_language
            .iter()
            .map(|count| (count.language.as_str(), count.files))
            .collect();
        assert_eq!(
            counts,
            vec![("python", 1), ("rust", 1)],
            "per-language counts stay separate and sorted"
        );
        // Both runs completed: no partial-run disclosure.
        assert!(
            result.language_runs.is_empty(),
            "unexpected language runs: {:?}",
            result.language_runs
        );
        Ok(())
    }

    /// A partial Python repo run (a parse failure leaves the analyzed
    /// denominator) records a `Partial` language run on the shared channel —
    /// the same disclosure the Rust repo path uses for generated-file skips
    /// (#3554, #2109) — while the analyzed files' findings still emit.
    #[cfg(feature = "lang-python")]
    #[test]
    fn repo_pipeline_records_partial_disclosure_for_python_parse_failures() -> Result<(), String> {
        let root = temp_root("py-repo-partial")?;
        write(&root.join("good.py"), "def good():\n    return 1\n")?;
        write(&root.join("broken.py"), "def broken(:\n    pass\n")?;

        let result = run_repo_pipeline_with_oracle_policy(
            &AnalysisOptions {
                root,
                base: None,
                diff_file: None,
                mode: AnalysisMode::Deep,
                resolved_subject_identity: None,
                include_unchanged_tests: true,
                resolve_tsconfig_paths: false,
                perl_facts_path: None,
                git_timeout: None,
                git_candidate: None,
                production_like_targets: Default::default(),
                test_harnesses: Vec::new(),
            },
            &OraclePolicy::default(),
            &[LanguageId::Python],
        )?;

        let run = result
            .language_runs
            .iter()
            .find(|run| run.language == "python")
            .ok_or("expected a python language run disclosure")?;
        assert_eq!(run.status, LanguageRunStatus::Partial);
        let reason = run
            .reason
            .as_deref()
            .ok_or("partial run must carry a reason")?;
        assert!(reason.contains("partial"), "{reason}");
        assert!(reason.contains("failed to read or parse"), "{reason}");
        // The analyzed file's findings still emit.
        assert!(
            result
                .findings
                .iter()
                .any(|finding| finding.language == Some(crate::domain::LanguageId::Python)),
            "findings from the analyzed files must survive the partial disclosure"
        );
        Ok(())
    }
}
