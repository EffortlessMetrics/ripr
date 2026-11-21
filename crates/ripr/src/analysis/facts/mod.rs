mod build;
pub(crate) mod cfg_predicates;
mod harness_registry;
mod includes;
mod model;
mod parameterized_tests;
mod role_composition;
mod test_styles;

use std::path::{Path, PathBuf};

use crate::config::TestHarnessRegistration;

pub fn build_index(root: &Path, files: &[PathBuf]) -> Result<model::RustIndex, String> {
    build_index_with_test_harnesses(root, files, &[])
}

/// Index construction with the repository-governed harness registry
/// (#3532) applied: exact registrations derive typed subject and
/// limitation facts after the ordinary role authorities run. Without
/// registrations the result is identical to [`build_index`].
pub fn build_index_with_test_harnesses(
    root: &Path,
    files: &[PathBuf],
    registrations: &[TestHarnessRegistration],
) -> Result<model::RustIndex, String> {
    let mut index = build::build_index(root, files)?;
    parameterized_tests::promote_explicit_test_case_functions(&mut index);
    test_styles::normalize_index_test_styles(&mut index);
    // Composition runs strictly after the normalizer: the normalizer
    // recomputes every role from same-file text and would stomp composed
    // roles (#3533). Composed grants only ever upgrade `Production` to the
    // evidence-only `CfgTestModule`, never the reverse. The workspace root
    // anchors crate-root identity for default module resolution.
    role_composition::compose_index_source_roles(&mut index, root);
    // Explicit harness registrations are the most specific authority, so
    // they run after composition and a composed generic grant can never
    // overwrite a registered subject's role (#3532). The workspace root
    // anchors the Cargo target metadata validation (#3608).
    harness_registry::apply_registrations(&mut index, root, registrations);
    Ok(index)
}

pub(crate) fn build_index_from_loaded_files_with_cache_and_test_harnesses(
    root: &Path,
    files: &[(PathBuf, Vec<u8>)],
    registrations: &[TestHarnessRegistration],
) -> Result<build::CachedRustIndex, String> {
    let mut cached = build::build_index_from_loaded_files_with_cache(root, files)?;
    parameterized_tests::promote_explicit_test_case_functions(&mut cached.index);
    test_styles::normalize_index_test_styles(&mut cached.index);
    role_composition::compose_index_source_roles(&mut cached.index, root);
    // Explicit harness registrations are the most specific authority, so
    // they run after composition and a composed generic grant can never
    // overwrite a registered subject's role (#3532). The workspace root
    // anchors the Cargo target metadata validation (#3608); the verdict
    // is recomputed after cache retrieval, so a manifest edit re-validates
    // immediately.
    harness_registry::apply_registrations(&mut cached.index, root, registrations);
    Ok(cached)
}

// The Cargo-validated file-wide harness evidence grant (#3608) is shared
// by every role surface (diff seeding, seam inventory, LSP scope) so a
// misdeclared registration degrades identically everywhere.
pub(crate) use harness_registry::validated_file_wide_harness_targets;

// Keep compilation-unit rebasing available at the facts facade for index consumers.
pub(crate) use includes::compilation_unit_path_from_parents;
pub use model::{
    CallFact, FileFacts, FunctionFact, FunctionSourceRole, FunctionSummary, HarnessLimitationFact,
    HarnessSelectorCapability, HarnessSubjectClaim, HarnessSubjectFact, LetBindingFact,
    LiteralFact, ModuleDeclarationFact, ModulePathTarget, OracleFact, ProbeShapeFact,
    ResolvedIncludeParent, ReturnFact, RustIncludeLimitation, RustIndex, SourceRoleProvenance,
    SourceRoleProvenanceEdge, SourceRoleProvenanceEdgeKind, TestFact, TestSummary,
};
#[cfg(test)]
pub(crate) use model::{WorkspaceFileAuthority, WorkspaceRootAuthority};
