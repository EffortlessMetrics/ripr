mod build;
pub(crate) use build::{RUST_SOURCE_NOT_UTF8_REASON, rust_source_text};
pub(crate) mod cfg_predicates;
mod harness_registry;
mod includes;
mod index;
mod model;
mod parameterized_tests;
mod role_composition;
mod test_helpers;
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
    let mut index = index_phase("index_parse", || build::build_index(root, files))?;
    index_phase("index_parameterized_tests", || {
        parameterized_tests::promote_explicit_test_case_functions(&mut index);
        index.refresh_memberships()
    })?;
    index_phase("index_test_styles", || {
        test_styles::normalize_index_test_styles(&mut index)?;
        index.refresh_memberships()
    })?;
    // Composition runs strictly after the normalizer: the normalizer
    // recomputes every role from same-file text and would stomp composed
    // roles (#3533). Composed grants only ever upgrade `Production` to the
    // evidence-only `CfgTestModule`, never the reverse. The workspace root
    // anchors crate-root identity for default module resolution.
    index_phase("index_role_composition", || {
        role_composition::compose_index_source_roles(&mut index, root);
        Ok(())
    })?;
    // Explicit harness registrations are the most specific authority, so
    // they run after composition and a composed generic grant can never
    // overwrite a registered subject's role (#3532). The workspace root
    // anchors the Cargo target metadata validation (#3608).
    index_phase("index_harness_registry", || {
        harness_registry::apply_registrations(&mut index, root, registrations);
        Ok(())
    })?;
    // Helper crediting reads final roles, so it runs after every role
    // authority.
    index_phase("index_test_helper_credit", || {
        test_helpers::credit_same_file_assertion_helpers(&mut index);
        Ok(())
    })?;
    index.finalize()?;
    Ok(index)
}

pub(crate) fn build_index_from_loaded_files_with_cache_and_test_harnesses(
    root: &Path,
    files: &[(PathBuf, Vec<u8>)],
    registrations: &[TestHarnessRegistration],
) -> Result<build::CachedRustIndex, String> {
    let mut cached = index_phase("index_cached_parse", || {
        build::build_index_from_loaded_files_with_cache(root, files)
    })?;
    index_phase("index_parameterized_tests", || {
        parameterized_tests::promote_explicit_test_case_functions(&mut cached.index);
        cached.index.refresh_memberships()
    })?;
    index_phase("index_test_styles", || {
        test_styles::normalize_index_test_styles(&mut cached.index)?;
        cached.index.refresh_memberships()
    })?;
    index_phase("index_role_composition", || {
        role_composition::compose_index_source_roles(&mut cached.index, root);
        Ok(())
    })?;
    // Explicit harness registrations are the most specific authority, so
    // they run after composition and a composed generic grant can never
    // overwrite a registered subject's role (#3532). The workspace root
    // anchors the Cargo target metadata validation (#3608); the verdict
    // is recomputed after cache retrieval, so a manifest edit re-validates
    // immediately.
    index_phase("index_harness_registry", || {
        harness_registry::apply_registrations(&mut cached.index, root, registrations);
        Ok(())
    })?;
    // Helper crediting reads final roles, so it runs after every role
    // authority.
    index_phase("index_test_helper_credit", || {
        test_helpers::credit_same_file_assertion_helpers(&mut cached.index);
        Ok(())
    })?;
    cached.index.finalize()?;
    Ok(cached)
}

// The Cargo-validated file-wide harness evidence grant (#3608) is shared
// by every role surface (diff seeding, seam inventory, LSP scope) so a
// misdeclared registration degrades identically everywhere.
pub(crate) use harness_registry::validated_file_wide_harness_targets;

// Keep compilation-unit rebasing available at the facts facade for index consumers.
pub(crate) use includes::compilation_unit_path_from_parents;
pub use index::{FactSlice, FileData, FileFactsView};
#[cfg(test)]
pub(crate) use model::OwnedRustIndex;
pub use model::{
    CallFact, FileFacts, FunctionContainer, FunctionFact, FunctionImplContext, FunctionItemFact,
    FunctionSourceRole, FunctionSummary, HarnessLimitationFact, HarnessSelectorCapability,
    HarnessSubjectClaim, HarnessSubjectFact, LetBindingFact, LiteralFact, ModuleDeclarationFact,
    ModulePathTarget, OracleFact, ProbeShapeFact, ResolvedIncludeParent, ReturnFact,
    RustIncludeLimitation, RustIndex, SourceRoleProvenance, SourceRoleProvenanceEdge,
    SourceRoleProvenanceEdgeKind, TestFact, TestSummary, UnresolvedPropertyMacroFact,
};
// Hot evidence loops hash each indexed file once and validate by digest.
pub(crate) use model::WorkspaceFileAuthority;
#[cfg(test)]
pub(crate) use model::WorkspaceRootAuthority;
pub(crate) use model::source_digest;

/// Phase tracing extends the existing opt-in latency stream; normal output is
/// unchanged. A cancelled intermediate index is never returned as complete.
fn index_phase<T>(name: &str, work: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    crate::analysis::cancellation::checkpoint()?;
    let started = std::time::Instant::now();
    let trace = std::env::var_os("RIPR_REPO_EXPOSURE_LATENCY_TRACE").is_some();
    if trace {
        eprintln!("ripr_repo_exposure_latency phase={name} status=start duration_ms=0");
    }
    let result = work();
    if trace {
        let status = if result.is_ok() { "finished" } else { "failed" };
        eprintln!(
            "ripr_repo_exposure_latency phase={name} status={status} duration_ms={}",
            started.elapsed().as_millis()
        );
    }
    let value = result?;
    crate::analysis::cancellation::checkpoint()?;
    Ok(value)
}
