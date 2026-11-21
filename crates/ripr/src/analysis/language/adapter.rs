//! Boundary trait for per-language fact extraction.
//!
//! See `docs/specs/RIPR-SPEC-0026-language-adapter-contract.md`.

use super::super::{AnalysisOptions, diff::ChangedFile};
use super::rust::PartialDiffScope;
use crate::analysis_outcome::AnalysisLimitation;
use crate::config::OraclePolicy;
use crate::domain::Finding;
use std::path::Path;

/// Per-language results returned by [`LanguageAdapter::analyze_diff`].
///
/// `findings` are unsorted; the orchestrating pipeline applies the
/// language-neutral sort and summary. `changed_files` is the number of
/// diff entries this adapter handled, used by the summary builder.
/// `partial_scope` is `Some` only when the adapter analyzed a deterministic
/// bounded partition of an over-budget diff (`limited_partial_scope`,
/// RIPR-PROP-0019); the pipeline then restricts the changed-file set handed
/// to every other adapter to the same partition.
#[derive(Clone, Debug, Default)]
pub(crate) struct LanguageDiffResult {
    pub(crate) findings: Vec<Finding>,
    /// Test-harness registry projections (#3532): what each exact
    /// registration established for this run. Empty without registrations.
    pub(crate) harness_projections: Vec<crate::analysis::harness_projection::TestHarnessProjection>,
    pub(crate) changed_files: usize,
    /// Number of distinct changed source lines for which the adapter
    /// generated at least one probe. This is a producer fact, not a proxy for
    /// every changed source line.
    pub(crate) candidate_line_count: usize,
    /// Per-output-language breakdown of `changed_files` for adapters that
    /// cover more than one output language — the TypeScript adapter handles
    /// `.ts/.tsx` (typescript) and `.js/.jsx` (javascript) (#2103 review).
    /// Empty when the adapter covers exactly one language; the pipeline then
    /// attributes `changed_files` to the adapter's own language.
    pub(crate) changed_files_by_language: Vec<(super::LanguageId, usize)>,
    pub(crate) partial_scope: Option<PartialDiffScope>,
    /// Number of accepted-language files intentionally excluded as generated
    /// source. The pipeline records this as a partial run disclosure.
    pub(crate) skipped_files: usize,
    /// Typed adapter-owned limitations that the pipeline publishes in the
    /// shared analysis outcome.
    pub(crate) limitations: Vec<AnalysisLimitation>,
}

/// Per-language results returned by [`LanguageAdapter::analyze_repo`].
///
/// `findings` are unsorted. `production_files` is the number of files
/// the adapter classified as production code, used by the summary builder.
#[derive(Clone, Debug, Default)]
pub(crate) struct LanguageRepoResult {
    pub(crate) findings: Vec<Finding>,
    /// Test-harness registry projections (#3532); empty without
    /// registrations.
    pub(crate) harness_projections: Vec<crate::analysis::harness_projection::TestHarnessProjection>,
    pub(crate) production_files: usize,
    /// Number of discovered-language files intentionally excluded as generated
    /// source. The pipeline records this as a partial run disclosure.
    pub(crate) skipped_files: usize,
    /// Typed partial-run disclosure (#3554, #2109): `Some` only when the
    /// adapter ran over a capped or otherwise partial repo working set, so
    /// the run can never back a full-denominator claim. The pipeline records
    /// it as a `LanguageRun` with status `Partial` — the same disclosure
    /// channel the Rust repo path uses for generated-file skips — which
    /// human/JSON output renders and gates fail closed on. `None` for
    /// complete runs and honest zeros.
    pub(crate) partial_reason: Option<String>,
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
