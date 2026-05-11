//! Boundary trait for per-language fact extraction.
//!
//! See `docs/specs/RIPR-SPEC-0026-language-adapter-contract.md`.

use super::super::{AnalysisOptions, diff::ChangedFile};
use crate::config::OraclePolicy;
use crate::domain::Finding;
use std::path::Path;

/// Per-language results returned by [`LanguageAdapter::analyze_diff`].
///
/// `findings` are unsorted; the orchestrating pipeline applies the
/// language-neutral sort and summary. `changed_files` is the number of
/// diff entries this adapter handled, used by the summary builder.
#[derive(Clone, Debug, Default)]
pub(crate) struct LanguageDiffResult {
    pub(crate) findings: Vec<Finding>,
    pub(crate) changed_files: usize,
}

/// Per-language results returned by [`LanguageAdapter::analyze_repo`].
///
/// `findings` are unsorted. `production_files` is the number of files
/// the adapter classified as production code, used by the summary builder.
#[derive(Clone, Debug, Default)]
pub(crate) struct LanguageRepoResult {
    pub(crate) findings: Vec<Finding>,
    pub(crate) production_files: usize,
}

/// Boundary trait for per-language adapters.
///
/// Pipelines call the adapter for Rust-shaped work (file selection,
/// indexing, probe generation, classification) and then perform the
/// language-neutral sort + summary on the returned `Vec<Finding>`.
///
/// The trait is internal to `crate::analysis`. Per-spec method extensions
/// for fact projection (`extract_facts`, `changed_owners`, ...) land
/// alongside their production consumers when TypeScript and Python
/// preview adapters arrive (see RIPR-SPEC-0027 and RIPR-SPEC-0028).
pub(crate) trait LanguageAdapter {
    /// Returns true when the adapter should handle the given source path.
    fn accepts_path(&self, path: &Path) -> bool;

    /// Produce findings for changed files from a diff.
    ///
    /// The adapter is responsible for filtering `changed_files` to the
    /// subset it accepts, building any language-specific index, applying
    /// oracle policy, generating probes, and classifying them.
    fn analyze_diff(
        &self,
        options: &AnalysisOptions,
        oracle_policy: &OraclePolicy,
        changed_files: &[ChangedFile],
    ) -> Result<LanguageDiffResult, String>;

    /// Produce findings for a whole-repo scan.
    ///
    /// The adapter discovers files it accepts under the workspace root,
    /// classifies production vs. test/example files, builds its index,
    /// generates probes, and classifies them.
    fn analyze_repo(
        &self,
        options: &AnalysisOptions,
        oracle_policy: &OraclePolicy,
    ) -> Result<LanguageRepoResult, String>;
}
