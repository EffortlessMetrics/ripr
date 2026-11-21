//! Language identifiers (re-exported from the domain layer).
//!
//! See `docs/specs/RIPR-SPEC-0026-language-adapter-contract.md`.
//!
//! `LanguageId` is canonical in `crate::domain` so output renderers can
//! serialize it without depending on the analysis layer. This module
//! re-exports it (and `LanguageStatus`) for the analysis-side adapter
//! seam.

// LanguageStatus is reached via `crate::domain::LanguageStatus` directly;
// it has no analysis-side consumer in this work item.
pub(crate) use crate::domain::LanguageId;
