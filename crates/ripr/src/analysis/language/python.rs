//! Python preview adapter (scaffold).
//!
//! See `docs/specs/RIPR-SPEC-0028-python-preview-static-facts.md` and
//! `docs/adr/0009-python-parser-substrate.md`.
//!
//! This adapter is **scaffold-only** in this work item:
//!
//! - it implements [`LanguageAdapter::accepts_path`] via the shared
//!   router so `.py` files route to it when `[languages] enabled`
//!   lists `python`;
//! - it implements [`LanguageAdapter::analyze_diff`] and
//!   [`LanguageAdapter::analyze_repo`] as no-op stubs that return empty
//!   [`LanguageDiffResult`]/[`LanguageRepoResult`] values;
//! - it carries a single `rustpython_parser` parse path through
//!   [`PythonAdapter::parse_source`] so the dependency has a real
//!   production consumer and the API surface is exercised by tests.
//!
//! Owner / test / assertion / probe / related-test fact extraction lands
//! in the next Campaign 27 work item against RIPR-SPEC-0028's fixture
//! corpus. Until then the adapter dispatches but produces no findings,
//! which matches the spec's preview-is-opt-in posture.
//!
//! ADR 0009 originally selected `ruff_python_parser`; that pick was
//! superseded in-place after discovering the crate is `publish = false`
//! in the astral-sh/ruff workspace and unavailable on crates.io. The
//! corrected substrate is `rustpython-parser`, the documented natural
//! fallback already named under the ADR's Revisit Criteria.

use super::super::{AnalysisOptions, diff::ChangedFile};
use super::{LanguageAdapter, LanguageDiffResult, LanguageId, LanguageRepoResult, route};
use crate::config::OraclePolicy;
use rustpython_parser::{Mode, parse};
use std::path::Path;

/// Python preview adapter.
///
/// Stateless: routing only in this work item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PythonAdapter;

impl PythonAdapter {
    /// Parse a Python source string with `rustpython_parser` and return
    /// whether the parse produced a usable module AST.
    ///
    /// This helper exists to give the `rustpython-parser` dependency a
    /// real production consumer in the scaffold work item. The next
    /// slice uses the resulting `Mod::Module` AST to extract owners,
    /// tests, and assertions per RIPR-SPEC-0028.
    pub(crate) fn parse_source(path: &Path, source: &str) -> bool {
        let source_path = path.to_string_lossy();
        parse(source, Mode::Module, source_path.as_ref()).is_ok()
    }
}

impl LanguageAdapter for PythonAdapter {
    fn accepts_path(&self, path: &Path) -> bool {
        matches!(route(path), Some(LanguageId::Python))
    }

    fn analyze_diff(
        &self,
        options: &AnalysisOptions,
        _oracle_policy: &OraclePolicy,
        changed_files: &[ChangedFile],
    ) -> Result<LanguageDiffResult, String> {
        // Scaffold: parse each accepted changed file to validate the
        // `rustpython-parser` dependency works end-to-end; the AST is
        // currently discarded. Fact extraction lands in the next
        // Campaign 27 work item, where `parse_source` becomes the entry
        // point for owner / test / assertion / probe extraction per
        // RIPR-SPEC-0028.
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
    fn accepts_py_paths() {
        let adapter = PythonAdapter;
        assert!(adapter.accepts_path(Path::new("scripts/run.py")));
        assert!(adapter.accepts_path(Path::new("src/lib/util.py")));
        assert!(!adapter.accepts_path(Path::new("src/lib.rs")));
        assert!(!adapter.accepts_path(Path::new("src/index.ts")));
        assert!(!adapter.accepts_path(Path::new("src/index.tsx")));
        assert!(!adapter.accepts_path(Path::new("README.md")));
        assert!(!adapter.accepts_path(Path::new("no-extension")));
    }

    #[test]
    fn parse_source_accepts_simple_python() {
        let ok = PythonAdapter::parse_source(
            Path::new("src/discount.py"),
            "def discount(amount: int) -> int:\n    return amount\n",
        );
        assert!(ok, "valid Python should parse without errors");
    }

    #[test]
    fn parse_source_accepts_class_and_decorator() {
        let ok = PythonAdapter::parse_source(
            Path::new("src/repo.py"),
            "class Repo:\n    @staticmethod\n    def make() -> 'Repo':\n        return Repo()\n",
        );
        assert!(ok, "decorated class methods should parse");
    }

    #[test]
    fn parse_source_accepts_async_def_and_fstring() {
        let ok = PythonAdapter::parse_source(
            Path::new("src/http.py"),
            "async def fetch(url: str) -> str:\n    return f\"{url}!\"\n",
        );
        assert!(ok, "async def + f-string should parse");
    }

    #[test]
    fn parse_source_rejects_garbage() {
        let ok = PythonAdapter::parse_source(
            Path::new("src/oops.py"),
            "this is not :: valid +++ python at all",
        );
        assert!(!ok, "garbage source should produce parse errors");
    }

    #[test]
    fn analyze_diff_returns_zero_findings_and_counts_accepted_files() -> Result<(), String> {
        let adapter = PythonAdapter;
        let options = AnalysisOptions {
            root: PathBuf::from("."),
            base: None,
            diff_file: None,
            mode: crate::analysis::AnalysisMode::Draft,
            include_unchanged_tests: false,
        };
        let policy = OraclePolicy::default();
        let changed_files = vec![
            changed("scripts/run.py"),
            changed("src/lib.rs"),
            changed("docs/README.md"),
            changed("src/util.py"),
            changed("src/index.ts"),
        ];
        let result = adapter.analyze_diff(&options, &policy, &changed_files)?;
        assert!(result.findings.is_empty());
        assert_eq!(result.changed_files, 2);
        Ok(())
    }

    #[test]
    fn analyze_repo_returns_empty_scaffold() -> Result<(), String> {
        let adapter = PythonAdapter;
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
