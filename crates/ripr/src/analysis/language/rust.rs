//! Reference adapter for Rust.
//!
//! See `docs/specs/RIPR-SPEC-0026-language-adapter-contract.md`.
//!
//! This adapter hosts the existing Rust analysis pipeline behind the
//! `LanguageAdapter` seam. The bodies of `analyze_diff` and `analyze_repo`
//! are relocated from `analysis::pipeline` without behavior change; the
//! pipeline module is now a language-neutral orchestrator that loads the
//! diff, dispatches to this adapter, and applies sort + summary on the
//! returned findings.

use super::super::{AnalysisOptions, classifier, diff::ChangedFile, probes, rust_index, workspace};
use super::{LanguageAdapter, LanguageDiffResult, LanguageId, LanguageRepoResult, route};
use crate::config::OraclePolicy;
use std::path::Path;

/// Reference adapter for Rust.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RustAdapter;

impl LanguageAdapter for RustAdapter {
    fn accepts_path(&self, path: &Path) -> bool {
        matches!(route(path), Some(LanguageId::Rust))
    }

    fn analyze_diff(
        &self,
        options: &AnalysisOptions,
        oracle_policy: &OraclePolicy,
        changed_files: &[ChangedFile],
    ) -> Result<LanguageDiffResult, String> {
        let changed_rust_paths = changed_files
            .iter()
            .filter(|file| self.accepts_path(&file.path))
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
            .filter(|file| self.accepts_path(&file.path))
        {
            changed_rust_files += 1;
            let probes = probes::probes_for_file(&options.root, changed, &index);
            for probe in probes {
                findings.push(classifier::classify_probe(&probe, &index));
            }
        }

        Ok(LanguageDiffResult {
            findings,
            changed_files: changed_rust_files,
        })
    }

    fn analyze_repo(
        &self,
        options: &AnalysisOptions,
        oracle_policy: &OraclePolicy,
    ) -> Result<LanguageRepoResult, String> {
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

        Ok(LanguageRepoResult {
            findings,
            production_files: production_files.len(),
        })
    }
}
