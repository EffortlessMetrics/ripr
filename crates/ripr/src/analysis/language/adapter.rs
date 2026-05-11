//! Boundary trait for per-language fact extraction.
//!
//! See `docs/specs/RIPR-SPEC-0026-language-adapter-contract.md`.

use std::path::Path;

/// Boundary trait for per-language adapters.
///
/// The trait surface is intentionally minimal in this work item: only the
/// routing predicate is part of the contract. Subsequent work items in
/// Campaign 27 extend it with the `language` discriminator (with the
/// output-metadata work item, where it gains a serialization consumer),
/// fact-extraction, probe-generation, and related-test methods (with the
/// rust-adapter-behind-boundary work item, where Rust extraction moves
/// behind the seam). See RIPR-SPEC-0027 and RIPR-SPEC-0028 for the
/// per-language behavior contracts.
pub(crate) trait LanguageAdapter {
    /// Returns true when the adapter should handle the given source path.
    ///
    /// Routing dispatches by file extension; per-repo opt-in for preview
    /// adapters is enforced at the pipeline layer where adapter dispatch
    /// happens, not here.
    fn accepts_path(&self, path: &Path) -> bool;
}
