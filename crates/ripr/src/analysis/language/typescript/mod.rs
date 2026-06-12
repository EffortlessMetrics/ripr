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
pub(crate) use crate::config::OraclePolicy;
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
    ExportDefaultDeclarationKind, Expression, Function, ImportDeclarationSpecifier,
    ImportOrExportKind, MethodDefinition, ModuleExportName, ObjectPropertyKind, PropertyKey,
    Statement, VariableDeclaration, VariableDeclarator,
};
pub(crate) use oxc_parser::Parser;
pub(crate) use oxc_span::{GetSpan, SourceType};
pub(crate) use std::path::{Path, PathBuf};

mod actionability;
mod bun_bridge;
mod classifier;
mod discovery;
mod oracle;
mod owners;
mod package;
mod parse;
mod paths;
mod probe_shape;
mod related_tests;
mod static_limit;
#[cfg(test)]
mod tests;
mod tests_extract;
mod types;

// Re-export all submodule items unconditionally so that every sibling
// submodule's `use super::*;` resolves, and so that `tests.rs` which
// uses `use super::*;` can access all items.
pub(crate) use actionability::*;
pub(crate) use bun_bridge::*;
pub(crate) use classifier::*;
pub(crate) use discovery::*;
pub(crate) use oracle::*;
pub(crate) use owners::*;
pub(crate) use package::*;
pub(crate) use parse::*;
pub(crate) use paths::*;
pub(crate) use probe_shape::*;
pub(crate) use related_tests::*;
pub(crate) use static_limit::*;
pub(crate) use tests_extract::*;
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
        // Phase 1: discover and index every accepted file in the workspace
        // so we can find related tests for any owner regardless of whether
        // the test file itself changed in this diff.
        let workspace_files = collect_workspace_typescript_files(&options.root);
        let mut all_owners: Vec<TypeScriptOwner> = Vec::new();
        let mut all_tests: Vec<TypeScriptTest> = Vec::new();
        let mut parse_limits: Vec<TypeScriptParseLimit> = Vec::new();
        for relative in &workspace_files {
            let absolute = options.root.join(relative);
            let Ok(source) = std::fs::read_to_string(&absolute) else {
                continue;
            };
            if let Some(reason) = parse_error_reason(relative, &source) {
                if !is_test_file(relative) {
                    parse_limits.push(TypeScriptParseLimit {
                        file: relative.clone(),
                        reason,
                    });
                }
                continue;
            }
            if is_test_file(relative) {
                all_tests.extend(extract_tests(relative, &source));
            } else {
                all_owners.extend(extract_owners(relative, &source));
            }
        }

        // Phase 2: for each accepted changed file, classify each changed
        // line that falls inside an owner.
        let mut findings: Vec<Finding> = Vec::new();
        let mut changed_count: usize = 0;
        for changed in changed_files {
            for added in &changed.added_lines {
                if let Some(finding) = bun_cross_language_finding_for_changed_rust_line(
                    &changed.path,
                    added.line,
                    &added.text,
                    &all_tests,
                ) {
                    findings.push(finding);
                }
            }
            if !self.accepts_path(&changed.path) {
                continue;
            }
            changed_count += 1;
            // Skip test-file changes for finding generation; classifier
            // operates on production owners. Test file edits are still
            // counted in the file tally.
            if is_test_file(&changed.path) {
                continue;
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
            for added in &changed.added_lines {
                if let Some(mut finding) = classify_change(
                    &changed.path,
                    added.line,
                    &added.text,
                    &all_owners,
                    &all_tests,
                ) {
                    finding.evidence.extend(discovery_evidence.clone());
                    findings.push(finding);
                }
            }
        }
        Ok(LanguageDiffResult {
            findings,
            changed_files: changed_count,
        })
    }

    fn analyze_repo(
        &self,
        _options: &AnalysisOptions,
        _oracle_policy: &OraclePolicy,
    ) -> Result<LanguageRepoResult, String> {
        // Repo-mode preview output lands in a follow-up. The current
        // sub-slice scopes to diff-mode for the smallest useful fixture.
        Ok(LanguageRepoResult {
            findings: Vec::new(),
            production_files: 0,
        })
    }
}
