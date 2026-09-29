//! TypeScript preview adapter.
//!
//! See `docs/specs/RIPR-SPEC-0027-typescript-preview-static-facts.md` and
//! `docs/adr/0008-typescript-parser-substrate.md`.
//!
//! Split from a monolithic 9497-line file for maintainability.
//! The only public API of this module is `TypeScriptAdapter`.

pub(crate) use super::super::{
    AnalysisOptions, diff::ChangedFile, fingerprint_probe_id, normalize_expression,
};
// `probes` is a private module of `crate::analysis`; import it so submodules
// can call `probes::expected_sinks` / `probes::required_oracles` via `super::probes`.
pub(crate) use super::{
    LanguageAdapter, LanguageDiffResult, LanguageId, LanguageRepoResult, route,
};
pub(super) use crate::analysis::probes;
pub(crate) use crate::analysis_outcome::{
    AnalysisLimitation, AnalysisLimitationKind, AnalysisRecovery, AnalysisRecoveryKind,
    AnalysisStage,
};
pub(crate) use crate::config::{
    OraclePolicy, is_detectable_excluded_typescript_path, is_detectable_generated_typescript_path,
    is_typescript_dir_pruned_from_discovery,
};
pub(crate) use crate::domain::{
    ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding,
    LanguageId as DomainLanguageId, LanguageStatus, MissingDiscriminatorFact, OracleKind,
    OracleStrength, OwnerKind, Probe, ProbeFamily, RelatedTest, RevealEvidence, RiprEvidence,
    SourceLocation, StageEvidence, StageState, StaticLimitKind, StopReason, SymbolId,
};
pub(crate) use crate::domain::{FlowSinkFact, FlowSinkKind};
pub(crate) use oxc_allocator::Allocator;
pub(crate) use oxc_ast::ast::{
    Argument, ArrowFunctionExpression, BindingPattern, Class, ClassElement, Declaration,
    ExportDefaultDeclarationKind, Expression, FormalParameters, Function,
    ImportDeclarationSpecifier, ImportOrExportKind, MethodDefinition, ModuleExportName,
    ObjectPropertyKind, PropertyKey, Statement, VariableDeclaration, VariableDeclarator,
};
pub(crate) use oxc_parser::Parser;
pub(crate) use oxc_span::{GetSpan, SourceType};
pub(crate) use std::path::{Path, PathBuf};

mod actionability;
mod annotation_only;
#[cfg(test)]
mod annotation_only_tests;
#[cfg(test)]
mod assertion_library_tests;
mod boundary_input;
#[cfg(test)]
mod boundary_input_tests;
mod bounded_read;
mod bun_bridge;
mod classifier;
mod discovery;
#[cfg(test)]
mod line_index_tests;
mod module_entries;
#[cfg(test)]
mod new_declaration_tests;
mod oracle;
mod owners;
mod package;
pub(crate) use package::detect_framework_for_root;
#[cfg(test)]
mod ambient_declaration_tests;
mod parse;
mod paths;
mod probe_shape;
mod related_tests;
mod static_limit;
#[cfg(test)]
mod tests;
mod tests_extract;
#[cfg(test)]
mod tests_extract_tests;
pub(crate) mod tsconfig;
mod types;
mod workspace_packages;

// Re-export all submodule items unconditionally so that every sibling
// submodule's `use super::*;` resolves, and so that `tests.rs` which
// uses `use super::*;` can access all items.
pub(crate) use actionability::*;
pub(crate) use annotation_only::*;
pub(crate) use boundary_input::*;
pub(crate) use bounded_read::*;
pub(crate) use bun_bridge::*;
pub(crate) use classifier::*;
pub(crate) use discovery::*;
pub(crate) use module_entries::*;
pub(crate) use oracle::*;
pub(crate) use owners::*;
pub(crate) use package::*;
pub(crate) use parse::*;
pub(crate) use paths::*;
pub(crate) use probe_shape::*;
pub(crate) use related_tests::*;
pub(crate) use static_limit::*;
pub(crate) use tests_extract::*;
#[cfg(test)]
pub(crate) use tsconfig::load_alias_map;
pub(crate) use tsconfig::{
    TsAliasMap, TsAliasMapLoadGap, TsAliasUnresolveCause, load_alias_map_with_read_error,
};
pub(crate) use types::*;

/// TypeScript / JavaScript preview adapter.
///
/// Stateless: routing, parsing, and per-file extraction only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TypeScriptAdapter;

pub(crate) fn source_type_for(path: &Path) -> SourceType {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("tsx") => SourceType::tsx(),
        Some("ts") => SourceType::ts(),
        Some("jsx") => SourceType::jsx(),
        // oxc 0.130 exposes no `mts()`/`cts()` constructors, so build the
        // TypeScript ESM/CJS flavors from `ts()` plus the module-kind
        // markers, mirroring how `js` maps onto `mjs()` below.
        Some("mts") => SourceType::ts().with_module(true),
        Some("cts") => SourceType::ts().with_commonjs(true),
        Some("mjs") => SourceType::mjs(),
        Some("cjs") => SourceType::cjs(),
        Some("js") => SourceType::mjs(),
        _ => SourceType::mjs(),
    }
}

impl LanguageAdapter for TypeScriptAdapter {
    fn accepts_path(&self, path: &Path) -> bool {
        matches!(route(path), Some(LanguageId::TypeScript))
    }

    fn analyze_diff(
        &self,
        options: &AnalysisOptions,
        _oracle_policy: &OraclePolicy,
        changed_files: &[ChangedFile],
    ) -> Result<LanguageDiffResult, String> {
        // Directory-module resolution (#4546) and the tsconfig outDir
        // mapping (#4551) are memoized for this run only (#4638 and #4800
        // reviews); the scope drops the cache when the run returns.
        let _directory_modules = DirectoryModuleCacheScope::open();
        // Phase 1: discover and index every accepted file in the workspace
        // so we can find related tests for any owner regardless of whether
        // the test file itself changed in this diff.
        let workspace_scan = collect_workspace_typescript_files(&options.root);
        let workspace_files = workspace_scan.files;
        let changed_paths = changed_files
            .iter()
            .map(|changed| normalized_path(&changed.path))
            .collect::<Vec<_>>();
        let mut all_owners: Vec<TypeScriptOwner> = Vec::new();
        let mut all_tests: Vec<TypeScriptTest> = Vec::new();
        let mut parse_limits: Vec<TypeScriptParseLimit> = Vec::new();
        let mut read_failures: Vec<TypeScriptReadFailure> = Vec::new();
        let mut extraction_gaps: Vec<TypeScriptTestExtractionGap> = Vec::new();
        // Owner shapes the extractor does not index, detected on CHANGED
        // production files only (#4104-A): their changed lines produce no
        // finding today, so the disclosure is the only honest signal.
        let mut owner_extraction_gaps: Vec<TypeScriptOwnerExtractionGap> = Vec::new();
        // Files that vanished from the index entirely (unreadable): the real
        // count is reported instead of a hardcoded 0 so downstream consumers
        // can tell an empty workspace from a silently incomplete one.
        let mut skipped_files = 0usize;
        // Capped reads: every workspace source is read ONCE into a shared
        // cache under a per-file cap and a per-run aggregate byte budget
        // (bounded_read.rs, mirroring the edit_cage contract). Files over
        // either bound become named limitations; plain IO failures feed the
        // read-failure disclosure lane (#4099) as skipped files.
        let workspace_read = read_workspace_sources_capped(
            &options.root,
            &workspace_files,
            ts_file_read_limit(),
            ts_workspace_read_budget(),
        );
        let source_cache = workspace_read.sources;
        // Normalized-key view of the cache so Phase 2 can look a changed
        // file's source up regardless of path-separator spelling (the
        // `parse_limit_for_file` convention).
        let source_by_normalized: std::collections::HashMap<String, &String> = source_cache
            .iter()
            .map(|(key, source)| (normalized_path(key), source))
            .collect();
        let read_limits: Vec<TypeScriptParseLimit> = workspace_read
            .limits
            .into_iter()
            .map(|(file, err)| TypeScriptParseLimit {
                file,
                reason: err.reason(),
            })
            .collect();
        for (file, error) in workspace_read.io_failures {
            skipped_files += 1;
            read_failures.push(TypeScriptReadFailure { file, error });
        }
        for relative in &workspace_files {
            let Some(source) = source_cache.get(relative) else {
                continue;
            };
            if let Some(reason) = parse_error_reason(relative, source) {
                // Disclose parse failures for CHANGED files of either role:
                // a changed production file's added lines are never
                // classified, and a changed test file's tests silently vanish
                // from `all_tests` (which can flip owners to false
                // `no_static_path`). Unchanged-file parse errors stay out of
                // the diff-scoped limitation set; the per-file index effect is
                // bounded to owners this diff touches.
                if changed_paths
                    .iter()
                    .any(|changed| changed == &normalized_path(relative))
                {
                    parse_limits.push(TypeScriptParseLimit {
                        file: relative.clone(),
                        reason,
                    });
                }
                continue;
            }
            if is_test_file(relative) {
                let tests = extract_tests(relative, source);
                // A recognized test file that parses but registers test
                // shapes the extractor drops (template-literal titles,
                // tagged-template `.each`, tests generated in loops or
                // callbacks) gets a partial-extraction disclosure so a
                // confident `no_static_path` is known to be possibly false.
                if let Some(gap) = detect_partial_test_extraction(relative, source, &tests) {
                    extraction_gaps.push(gap);
                }
                all_tests.extend(tests);
            } else {
                all_owners.extend(extract_owners(relative, source));
            }
        }
        // Build tsconfig.json alias map when opt-in flag is enabled (RIPR-SPEC-0099).
        // fail-closed: None when flag is off, when tsconfig is absent, when extends/
        // references are present, or when any other parse/resolution failure occurs.
        // A capped-read size limit on the config itself is surfaced below as a
        // named limitation rather than failing silently closed.
        let (alias_map, alias_read_limit, alias_load_gap): (Option<TsAliasMap>, _, _) =
            if options.resolve_tsconfig_paths {
                load_alias_map_with_read_error(&options.root)
            } else {
                // Flag off: no gap — the MapUnavailable cause carries the
                // honest "enable the flag" advice for this path (#4106-B).
                (None, None, None)
            };
        // In-workspace package names resolve whether or not the tsconfig
        // flag is on (#4554): `import ... from '@scope/pkg/sub'` in a sibling
        // package's test names that package's source when its manifest says
        // so unambiguously. The load gap is kept, so alias advice is as before.
        let workspace_packages =
            workspace_packages::WorkspacePackages::discover(&options.root, &workspace_files);
        let alias_map = if workspace_packages.is_empty() {
            alias_map
        } else {
            Some(match alias_map {
                Some(map) => map.with_workspace_packages(workspace_packages),
                None => TsAliasMap::workspace_packages_only(&options.root, workspace_packages),
            })
        };
        let alias_map_ref: Option<&TsAliasMap> = alias_map.as_ref();

        // Build the bounded re-export index from all non-test workspace files
        // (RIPR-SPEC-0095). The index enables crediting tests that reach the owner
        // through `export { N } from` / `export * from` barrel chains.
        // Sources come from the Phase-1 cache so each file is read once per run.
        let reexport_index = ReExportIndex::build(
            &workspace_files,
            &source_cache,
            &options.root,
            alias_map_ref,
            is_test_file,
        );

        // Phase 2: for each accepted changed file, classify each changed
        // line that falls inside an owner.
        let mut findings: Vec<Finding> = Vec::new();
        let mut changed_count: usize = 0;
        // Per-output-language tally (#2103 review): this adapter covers
        // typescript (.ts/.tsx/.mts/.cts) and javascript (.js/.jsx/.mjs/.cjs),
        // so the summary must not attribute JS files to typescript.
        let mut changed_typescript: usize = 0;
        let mut changed_javascript: usize = 0;
        // Whether any finding in this diff was classified against the
        // TypeScript test index: an owner-backed TS/JS finding, or a Bun
        // cross-language finding. Partial test extraction is disclosed only
        // then (#4261); an ownerless, import-only or deletion-only TS change
        // never reads the index.
        let mut test_index_consumed = false;
        for changed in changed_files {
            for added in &changed.added_lines {
                if let Some(finding) = bun_cross_language_finding_for_changed_rust_line(
                    &changed.path,
                    added.line,
                    &added.text,
                    &all_tests,
                ) {
                    test_index_consumed = true;
                    findings.push(finding);
                }
            }
            // Excluded subtrees (node_modules, dist, build, coverage, vendor,
            // __generated__) and `*.generated.*` files are skipped BEFORE
            // counting (#3743). The workspace walk prunes the same trees, so
            // no facts can back a changed file under one of them. Counting it
            // would put an uninspected file in the report denominator.
            if !self.accepts_path(&changed.path)
                || is_detectable_generated_typescript_path(&changed.path)
                || is_detectable_excluded_typescript_path(&changed.path)
            {
                continue;
            }
            changed_count += 1;
            match output_language_for(&changed.path) {
                DomainLanguageId::JavaScript => changed_javascript += 1,
                _ => changed_typescript += 1,
            }
            // Skip test-file changes for finding generation; classifier
            // operates on production owners. Test file edits are still
            // counted in the file tally.
            if is_test_file(&changed.path) {
                continue;
            }
            // Declaration files are counted but never probed: they are
            // type-only and have no runtime behavior a test could observe.
            if is_typescript_declaration_file(&changed.path) {
                continue;
            }

            // Owner-extraction gap detection (#4104-A): a changed line inside
            // an owner shape the extractor does not index produces NO finding
            // below, so this bounded disclosure is the only honest signal.
            if let Some(source) = source_by_normalized.get(&normalized_path(&changed.path)) {
                let changed_lines: Vec<usize> =
                    changed.added_lines.iter().map(|added| added.line).collect();
                if let Some(gap) =
                    detect_owner_extraction_gap(&changed.path, source, &changed_lines)
                {
                    owner_extraction_gaps.push(gap);
                }
            }

            // Resolve package/workspace discovery facts for this changed file.
            // Evidence lines are injected into every finding generated below
            // so that the rendering layer (typescript_preview_card) and the
            // next-PR runner-inference step can consume them without re-reading
            // the filesystem.
            let pkg_discovery = resolve_package_discovery(&changed.path, &options.root);
            let discovery_evidence = pkg_discovery.evidence_lines();

            if let Some(limit) = parse_limit_for_file(&changed.path, &parse_limits) {
                if let Some(added) = changed.added_lines.first() {
                    let mut finding =
                        unsupported_syntax_finding(&changed.path, added.line, &added.text, limit);
                    finding.evidence.extend(discovery_evidence.clone());
                    findings.push(finding);
                }
                continue;
            }
            // A decorator on the line above a method is invisible to the
            // one-line annotation-only check, so any decorator in the file
            // keeps method lines probed (#4282).
            let file_has_decorators = source_by_normalized
                .get(&normalized_path(&changed.path))
                .is_none_or(|source| {
                    source
                        .lines()
                        .any(|line| line.trim_start().starts_with('@'))
                });
            // Ambient declarations are type-only; their lines are found from
            // the syntax tree, since `declare` is also a legal runtime
            // identifier and can start a line inside a template literal.
            let ambient = source_by_normalized
                .get(&normalized_path(&changed.path))
                .map(|source| ambient_declaration_lines(&changed.path, source))
                .unwrap_or_default();
            let is_probe_candidate = |line: usize, text: &str| {
                !should_ignore_typescript_changed_line(text)
                    && !ambient
                        .iter()
                        .any(|(start, end)| (*start..=*end).contains(&line))
            };
            let removed_texts: Vec<&str> = changed
                .removed_lines
                .iter()
                .map(|removed| removed.text.as_str())
                .collect();
            for added in &changed.added_lines {
                if !is_probe_candidate(added.line, &added.text) {
                    continue;
                }
                // New-declaration guard: the opening line of a NEW function,
                // method, or arrow owner (no paired removed line) whose body
                // carries its own added lines has no behavior of its own —
                // the body lines are the probes. Probing it would ask for a
                // discriminator no test can supply. A changed signature
                // (paired removed line), a default value, or a one-line
                // body keeps its probe.
                if !changed
                    .removed_lines
                    .iter()
                    .any(|removed| removed.new_side_line == added.line)
                    && is_new_owner_opening_line(
                        &changed.path,
                        added.line,
                        &added.text,
                        &all_owners,
                        &removed_texts,
                        |line| {
                            changed.added_lines.iter().any(|other| {
                                other.line == line && is_probe_candidate(other.line, &other.text)
                            })
                        },
                    )
                {
                    continue;
                }
                // Annotation-only guard (#4282): TypeScript erases types, so a
                // line whose in-place removed counterpart differs only in type
                // syntax has no behavior for a test to discriminate. Pairing
                // mirrors the Python adapter (same new-side position).
                if changed
                    .removed_lines
                    .iter()
                    .find(|removed| removed.new_side_line == added.line)
                    .is_some_and(|removed| {
                        is_annotation_only_signature_change(
                            &changed.path,
                            &removed.text,
                            &added.text,
                            file_has_decorators,
                        )
                    })
                {
                    continue;
                }
                if let Some(mut finding) = classify_change_with_alias_state(
                    &changed.path,
                    added.line,
                    &added.text,
                    &all_owners,
                    &all_tests,
                    Some(&options.root),
                    &reexport_index,
                    alias_map_ref,
                    alias_load_gap.as_ref(),
                ) {
                    test_index_consumed = true;
                    finding.evidence.extend(discovery_evidence.clone());
                    // Inject verify-command evidence derived from the strongest
                    // related test and the package-discovery facts already
                    // resolved above. Fail-closed: when the runner is
                    // unresolved the named limitation
                    // `typescript_test_runner_unresolved` is emitted instead
                    // so consumers know why no command is available.
                    let inferred_cmd = finding
                        .related_tests
                        .iter()
                        .max_by_key(|t| t.oracle_strength.rank())
                        .and_then(|best_test| {
                            verify_command_for_discovery(&pkg_discovery, &best_test.file)
                        });
                    if let Some(cmd) = inferred_cmd {
                        finding
                            .evidence
                            .push(format!("typescript_verify_command: {cmd}"));
                        // The runner resolved: drop `verify_command` from the
                        // static `missing_actionability_fields` list so the JSON
                        // output does not self-contradict (claiming a field is
                        // missing while also carrying its value two lines over).
                        // Fail-closed: this path is only reached when
                        // `inferred_cmd.is_some()` — when the runner is
                        // unresolved the `missing_actionability_fields` line is
                        // left intact so consumers see the correct gap.
                        // (RIPR-SPEC cockpit delta #5 / issue #1245)
                        for ev in &mut finding.evidence {
                            if ev.starts_with("missing_actionability_fields:") {
                                *ev = remove_field_from_missing_list(ev, "verify_command");
                            }
                        }
                        finding.evidence.retain(|ev| !ev.is_empty());
                    } else {
                        // Fail-closed: no verify command resolved. Emit the
                        // named limitation so consumers know why no command is
                        // available. This must fire whenever the inferred
                        // command is `None` — not only when no framework was
                        // detected: mocha has no file-target command mapping,
                        // so a detected mocha framework with no lockfile/runner
                        // evidence previously produced NEITHER a command NOR a
                        // limitation, contradicting the contract documented
                        // above. (When the command IS resolved the
                        // `typescript_test_runner: <name>` evidence line
                        // already identifies the runner, so no limitation.)
                        finding.evidence.push(
                            "typescript_package_limitation: typescript_test_runner_unresolved"
                                .to_string(),
                        );
                    }
                    findings.push(finding);
                }
            }
        }
        let mut changed_files_by_language = Vec::new();
        if changed_typescript > 0 {
            changed_files_by_language.push((LanguageId::TypeScript, changed_typescript));
        }
        if changed_javascript > 0 {
            changed_files_by_language.push((LanguageId::JavaScript, changed_javascript));
        }
        let mut limitations = parse_limits
            .iter()
            .map(|limit| {
                AnalysisLimitation::new(
                    AnalysisLimitationKind::LanguageScopeUnsupported,
                    AnalysisStage::LanguageAdapter,
                    AnalysisRecovery::new(
                        AnalysisRecoveryKind::Retry,
                        "Fix the TypeScript or JavaScript parse error, then re-run the analysis.",
                    )?,
                )
                .with_path(limit.file.to_string_lossy())?
                .with_affected_items(1)?
                .with_detail(limit.reason.clone())
            })
            .collect::<Result<Vec<_>, String>>()?;
        // Unreadable CHANGED files: their added lines are never classified
        // (production) or their tests vanish from the index (test files), so
        // the diff-scoped result must name the path and the read failure
        // instead of silently dropping the file. Unreadable unchanged files
        // are counted in `skipped_files` above but stay out of the
        // diff-scoped limitation set.
        for failure in &read_failures {
            if !changed_paths
                .iter()
                .any(|changed| changed == &normalized_path(&failure.file))
            {
                continue;
            }
            limitations.push(
                AnalysisLimitation::new(
                    AnalysisLimitationKind::LanguageScopeUnsupported,
                    AnalysisStage::LanguageAdapter,
                    AnalysisRecovery::new(
                        AnalysisRecoveryKind::Retry,
                        "Restore read access to the file (check permissions and UTF-8 encoding), then re-run the analysis.",
                    )?,
                )
                .with_path(failure.file.to_string_lossy())?
                .with_affected_items(1)?
                .with_detail(format!("read failed: {}", failure.error))?,
            );
        }
        // Partial test extraction: one typed limitation summarizing the
        // affected test files (a single file keeps its path), carrying the
        // taxonomy name so JSON consumers can key on it.
        // The index is workspace-wide, so a diff that classified nothing
        // against it (Rust-only, or TS test edits only) is not made partial by
        // test shapes it never consulted (#4261).
        let consulted_gaps: &[_] = if test_index_consumed {
            &extraction_gaps
        } else {
            &[]
        };
        // One summary limitation for the whole workspace-wide index, not one
        // line per unrelated test file: the gaps are not in the diff, so a
        // per-file list buried the changed-file result.
        if let Some(limitation) = test_extraction_partial_summary(consulted_gaps)? {
            limitations.push(limitation);
        }
        // Partial owner extraction (#4104-A): changed lines inside owner
        // shapes the extractor does not index produce no finding, so this
        // typed limitation replaces the silent skip.
        for gap in &owner_extraction_gaps {
            let limitation = owner_extraction_partial_limitation(gap);
            limitations.push(
                AnalysisLimitation::new(
                    AnalysisLimitationKind::LanguageScopeUnsupported,
                    AnalysisStage::LanguageAdapter,
                    AnalysisRecovery::new(
                        AnalysisRecoveryKind::Retry,
                        "Re-run analysis after the adapter learns to extract the disclosed owner shape.",
                    )?,
                )
                .with_path(gap.file.to_string_lossy())?
                .with_affected_items(1)?
                .with_detail(format!(
                    "typescript_owner_extraction_partial: {} at {}",
                    gap.shape, limitation.sample_source
                ))?,
            );
        }
        // Capped-read bounds are named limitations, never silent skips. The
        // recovery names the env knobs so operators can raise the bounds.
        limitations.extend(read_limits.iter().map(|limit| {
            AnalysisLimitation::new(
                AnalysisLimitationKind::LanguageScopeUnsupported,
                AnalysisStage::LanguageAdapter,
                AnalysisRecovery::new(
                    AnalysisRecoveryKind::IncreaseConfiguredLimit,
                    "Raise RIPR_TS_MAX_FILE_READ_BYTES and/or RIPR_TS_MAX_WORKSPACE_READ_BYTES, then re-run the analysis.",
                )?,
            )
            .with_path(limit.file.to_string_lossy())?
            .with_affected_items(1)?
            .with_detail(limit.reason.clone())
        }).collect::<Result<Vec<_>, String>>()?);
        // An over-limit tsconfig/jsconfig fail-closes the alias map; disclose
        // the size limit so the missing alias resolution is not silent.
        if let Some((file, err)) = alias_read_limit.filter(|(_, err)| err.is_size_limit()) {
            // Limitation paths must be repository-relative: the config always
            // lives at the workspace root, so strip the root prefix.
            let relative = file
                .strip_prefix(&options.root)
                .unwrap_or(file.as_path())
                .to_string_lossy();
            limitations.push(
                AnalysisLimitation::new(
                    AnalysisLimitationKind::LanguageScopeUnsupported,
                    AnalysisStage::LanguageAdapter,
                    AnalysisRecovery::new(
                        AnalysisRecoveryKind::IncreaseConfiguredLimit,
                        "Raise RIPR_TS_MAX_FILE_READ_BYTES, then re-run the analysis.",
                    )?,
                )
                .with_path(relative)?
                .with_affected_items(1)?
                .with_detail(err.reason())?,
            );
        }
        // An absolute (or non-normal) compilerOptions.baseUrl cannot be
        // anchored to the workspace root by single-hop resolution. The map
        // fail-closes every lookup; disclose the named limitation so the
        // missing alias resolution is not silent.
        if alias_map_ref.is_some_and(TsAliasMap::base_url_absolute) {
            limitations.push(
                AnalysisLimitation::new(
                    AnalysisLimitationKind::LanguageScopeUnsupported,
                    AnalysisStage::LanguageAdapter,
                    AnalysisRecovery::new(
                        AnalysisRecoveryKind::Retry,
                        "Change compilerOptions.baseUrl to a workspace-relative path, then re-run the analysis.",
                    )?,
                )
                .with_path("tsconfig.json")?
                .with_affected_items(1)?
                .with_detail(
                    "typescript_base_url_absolute_unsupported: compilerOptions.baseUrl is absolute; single-hop resolution only supports workspace-relative baseUrl",
                )?,
            );
        }
        if workspace_scan.truncated {
            // Workspace discovery hit the max-visited-files cap; the file list
            // is partial, so disclose the bound instead of silently analyzing
            // a subset.
            limitations.push(
                AnalysisLimitation::new(
                    AnalysisLimitationKind::DiffScopeOversized,
                    AnalysisStage::LanguageAdapter,
                    AnalysisRecovery::new(
                        AnalysisRecoveryKind::IncreaseConfiguredLimit,
                        "Raise RIPR_TS_MAX_WORKSPACE_FILES, then re-run the analysis.",
                    )?,
                )
                .with_detail(format!(
                    "Workspace discovery stopped at the {}-entry visit cap ({TS_MAX_WORKSPACE_FILES_ENV}); discovered files are a partial set.",
                    ts_workspace_file_limit()
                ))?,
            );
        }
        if workspace_scan.skipped_links > 0 {
            // Symlinks/junctions are not followed during discovery (#4104-D);
            // disclose the count so link-hidden tests or sources are not
            // silently invisible to the analysis.
            let skipped_links = workspace_scan.skipped_links;
            limitations.push(
                AnalysisLimitation::new(
                    AnalysisLimitationKind::LanguageScopeUnsupported,
                    AnalysisStage::LanguageAdapter,
                    AnalysisRecovery::new(
                        AnalysisRecoveryKind::Retry,
                        "Replace symlinked/junctioned paths with real files or directories (discovery does not follow links), then re-run the analysis.",
                    )?,
                )
                .with_affected_items(u64::try_from(skipped_links).unwrap_or(u64::MAX))?
                .with_detail(format!(
                    "typescript_workspace_links_skipped: {skipped_links} symlink/junction entr{} not followed; tests or sources behind them are invisible to this analysis",
                    if skipped_links == 1 { "y was" } else { "ies were" }
                ))?,
            );
        }
        // Post-hoc collision de-dup: identical added lines in the same
        // owner share a content-addressed probe id (path/family/owner/
        // expression, no line number); the ordinal pass keeps them
        // distinct (mirror of the Rust path's `dedup_probe_ids`).
        dedup_typescript_probe_ids(&mut findings);
        Ok(LanguageDiffResult {
            findings,
            harness_projections: Vec::new(),
            changed_files: changed_count,
            candidate_line_count: 0,
            changed_files_by_language,
            partial_scope: None,
            skipped_files,
            limitations,
        })
    }

    fn analyze_repo(
        &self,
        _options: &AnalysisOptions,
        _oracle_policy: &OraclePolicy,
    ) -> Result<LanguageRepoResult, String> {
        // Repo-mode preview output lands in a follow-up. The current
        // sub-slice scopes to diff-mode for the smallest useful fixture.
        // The stub still returns an empty result, but it now discloses
        // the partial run through `partial_reason` so the pipeline
        // records a `Partial` language run on the shared `language_runs`
        // channel (adapter.rs): human/JSON output renders the limitation
        // and gates fail closed on the partial denominator. Without this
        // disclosure the empty adapter result was silent — and in a mixed
        // Rust+TypeScript repo the render-side `typescript_diff_first`
        // guidance (output/render.rs) never fires because it requires an
        // empty seam inventory AND no Rust files. See
        // docs/LANGUAGE_ADAPTER_PREVIEW.md § "Repo-Mode Analysis" for
        // the limitation contract.
        Ok(LanguageRepoResult {
            findings: Vec::new(),
            harness_projections: Vec::new(),
            production_files: 0,
            skipped_files: 0,
            partial_reason: Some("typescript_repo_mode_not_implemented_diff_first".to_string()),
        })
    }
}

/// Recovery for partial test extraction. Re-running cannot change the
/// result, so the recovery names what the reader can inspect or rewrite.
const TEST_EXTRACTION_PARTIAL_RECOVERY: &str = "Some test files (affected items) register tests in shapes the TypeScript extractor does not index; the JSON limitation detail names samples. Check whether they exercise the changed code before trusting a no-path or weak result, or register them as top-level `test`/`it` calls with plain string titles (array-form `.each` is indexed)";

/// Samples named in a multi-file extraction-partial summary.
const TEST_EXTRACTION_PARTIAL_SAMPLES: usize = 3;

/// Collapse per-file test-extraction gaps into one typed limitation. A single
/// gap keeps its path; several gaps carry the file count, the first few
/// samples, and the remainder count.
fn test_extraction_partial_summary(
    gaps: &[TypeScriptTestExtractionGap],
) -> Result<Option<AnalysisLimitation>, String> {
    let recovery = || {
        AnalysisRecovery::new(
            AnalysisRecoveryKind::InspectFailure,
            TEST_EXTRACTION_PARTIAL_RECOVERY,
        )
    };
    match gaps {
        [] => Ok(None),
        [gap] => {
            let limitation = test_extraction_partial_limitation(gap);
            Ok(Some(
                AnalysisLimitation::new(
                    AnalysisLimitationKind::LanguageScopeUnsupported,
                    AnalysisStage::LanguageAdapter,
                    recovery()?,
                )
                .with_path(gap.file.to_string_lossy())?
                .with_affected_items(1)?
                .with_detail(format!(
                    "typescript_test_extraction_partial: {} at {}",
                    gap.shape, limitation.sample_source
                ))?,
            ))
        }
        _ => {
            let files = gaps
                .iter()
                .map(|gap| normalized_path(&gap.file))
                .collect::<std::collections::BTreeSet<_>>();
            let samples = gaps
                .iter()
                .take(TEST_EXTRACTION_PARTIAL_SAMPLES)
                .map(|gap| {
                    format!(
                        "{} at {}",
                        gap.shape,
                        test_extraction_partial_limitation(gap).sample_source
                    )
                })
                .collect::<Vec<_>>();
            let remainder = gaps.len().saturating_sub(samples.len());
            let head = format!(
                "typescript_test_extraction_partial: {} test file(s) register tests the extractor does not index",
                files.len()
            );
            let mut detail = format!("{head}; e.g. {}", samples.join("; "));
            if remainder > 0 {
                detail.push_str(&format!(" (+{remainder} more)"));
            }
            if detail.chars().count()
                > crate::analysis_outcome::MAX_ANALYSIS_LIMITATION_DETAIL_CHARS
            {
                detail = head;
            }
            Ok(Some(
                AnalysisLimitation::new(
                    AnalysisLimitationKind::LanguageScopeUnsupported,
                    AnalysisStage::LanguageAdapter,
                    recovery()?,
                )
                .with_affected_items(u64::try_from(files.len()).unwrap_or(u64::MAX))?
                .with_detail(detail)?,
            ))
        }
    }
}
