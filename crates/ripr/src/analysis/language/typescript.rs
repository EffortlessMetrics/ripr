//! TypeScript preview adapter (scaffold).
//!
//! See `docs/specs/RIPR-SPEC-0027-typescript-preview-static-facts.md` and
//! `docs/adr/0008-typescript-parser-substrate.md`.
//!
//! This adapter is **scaffold-only** in this work item:
//!
//! - it implements [`LanguageAdapter::accepts_path`] via the shared
//!   router so `.ts`, `.tsx`, `.js`, and `.jsx` files route to it when
//!   `[languages] enabled` lists `typescript`;
//! - it implements [`LanguageAdapter::analyze_diff`] and
//!   [`LanguageAdapter::analyze_repo`] as no-op stubs that return empty
//!   [`LanguageDiffResult`]/[`LanguageRepoResult`] values;
//! - it carries a single `oxc_parser` parse path through
//!   [`TypeScriptAdapter::parse_source`] so the dependency has a real
//!   production consumer and the API surface is exercised by tests.
//!
//! Owner / test / assertion / probe / related-test fact extraction lands
//! in the next Campaign 27 work item (`analysis/typescript-preview-facts`),
//! where this stub is filled in against RIPR-SPEC-0027's fixture corpus.
//! Until then the adapter dispatches but produces no findings, which
//! matches the spec's preview-is-opt-in posture.

use super::super::{AnalysisOptions, diff::ChangedFile};
use super::{LanguageAdapter, LanguageDiffResult, LanguageId, LanguageRepoResult, route};
use crate::config::OraclePolicy;
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::path::Path;

/// TypeScript / JavaScript preview adapter.
///
/// Stateless: routing only in this work item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TypeScriptAdapter;

impl TypeScriptAdapter {
    /// Parse a TypeScript / JavaScript / JSX / TSX source string with
    /// `oxc_parser` and return whether the parse produced no errors.
    ///
    /// This helper exists to give the `oxc_parser` dependency a real
    /// production consumer in the scaffold work item. The next slice
    /// uses the resulting `Program` AST to extract owners, tests, and
    /// assertions per RIPR-SPEC-0027.
    pub(crate) fn parse_source(path: &Path, source: &str) -> bool {
        let allocator = Allocator::default();
        let source_type = source_type_for(path);
        let ret = Parser::new(&allocator, source, source_type).parse();
        ret.errors.is_empty()
    }
}

fn source_type_for(path: &Path) -> SourceType {
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
        // Scaffold: parse each accepted changed file to validate the
        // `oxc_parser` dependency works end-to-end; the AST is currently
        // discarded. Fact extraction lands in the next Campaign 27 work
        // item, where `parse_source` becomes the entry point for owner /
        // test / assertion / probe extraction per RIPR-SPEC-0027.
        let mut count: usize = 0;
        for file in changed_files {
            if !self.accepts_path(&file.path) {
                continue;
            }
            count += 1;
            let absolute = options.root.join(&file.path);
            if let Ok(source) = std::fs::read_to_string(&absolute) {
                let _ = Self::parse_source(&file.path, &source);
            }
        }
        Ok(LanguageDiffResult {
            findings: Vec::new(),
            changed_files: count,
        })
    }

    fn analyze_repo(
        &self,
        _options: &AnalysisOptions,
        _oracle_policy: &OraclePolicy,
    ) -> Result<LanguageRepoResult, String> {
        // Scaffold: no repo-mode preview output yet.
        Ok(LanguageRepoResult {
            findings: Vec::new(),
            production_files: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn changed(path: &str) -> ChangedFile {
        ChangedFile {
            path: PathBuf::from(path),
            added_lines: Vec::new(),
            removed_lines: Vec::new(),
        }
    }

    #[test]
    fn accepts_ts_jsx_paths() {
        let adapter = TypeScriptAdapter;
        assert!(adapter.accepts_path(Path::new("src/index.ts")));
        assert!(adapter.accepts_path(Path::new("src/component.tsx")));
        assert!(adapter.accepts_path(Path::new("src/index.js")));
        assert!(adapter.accepts_path(Path::new("src/component.jsx")));
        assert!(!adapter.accepts_path(Path::new("src/lib.rs")));
        assert!(!adapter.accepts_path(Path::new("scripts/run.py")));
        assert!(!adapter.accepts_path(Path::new("README.md")));
    }

    #[test]
    fn parse_source_accepts_simple_typescript() {
        let ok = TypeScriptAdapter::parse_source(
            Path::new("src/index.ts"),
            "export function discount(amount: number): number { return amount; }",
        );
        assert!(ok, "valid TypeScript should parse without errors");
    }

    #[test]
    fn parse_source_accepts_simple_tsx() {
        let ok = TypeScriptAdapter::parse_source(
            Path::new("src/Header.tsx"),
            "export const Header = ({ name }: { name: string }) => <h1>{name}</h1>;",
        );
        assert!(ok, "valid TSX should parse without errors");
    }

    #[test]
    fn parse_source_rejects_garbage() {
        let ok = TypeScriptAdapter::parse_source(
            Path::new("src/index.ts"),
            "this is not :: valid +++ typescript",
        );
        assert!(!ok, "garbage source should produce parse errors");
    }

    #[test]
    fn analyze_diff_returns_zero_findings_and_counts_accepted_files() -> Result<(), String> {
        let adapter = TypeScriptAdapter;
        let options = AnalysisOptions {
            root: PathBuf::from("."),
            base: None,
            diff_file: None,
            mode: crate::analysis::AnalysisMode::Draft,
            include_unchanged_tests: false,
        };
        let policy = OraclePolicy::default();
        let changed_files = vec![
            changed("src/index.ts"),
            changed("src/lib.rs"),
            changed("docs/README.md"),
            changed("src/Header.tsx"),
        ];
        let result = adapter.analyze_diff(&options, &policy, &changed_files)?;
        assert!(result.findings.is_empty());
        assert_eq!(result.changed_files, 2);
        Ok(())
    }

    #[test]
    fn analyze_repo_returns_empty_scaffold() -> Result<(), String> {
        let adapter = TypeScriptAdapter;
        let options = AnalysisOptions {
            root: PathBuf::from("."),
            base: None,
            diff_file: None,
            mode: crate::analysis::AnalysisMode::Deep,
            include_unchanged_tests: false,
        };
        let policy = OraclePolicy::default();
        let result = adapter.analyze_repo(&options, &policy)?;
        assert!(result.findings.is_empty());
        assert_eq!(result.production_files, 0);
        Ok(())
    }
}
