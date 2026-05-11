//! Language identifiers.
//!
//! See `docs/specs/RIPR-SPEC-0026-language-adapter-contract.md`.
//!
//! `LanguageStatus` (`Stable` vs `Preview`) and the wire-string helpers are
//! deferred to the output-metadata work item in Campaign 27 where they gain
//! real serialization consumers alongside the additive optional `language`
//! and `language_status` output fields.

/// The set of source languages an adapter can identify itself as.
///
/// `Rust` is the reference language. `TypeScript` and `Python` are preview
/// adapters added in later work items in Campaign 27. Adding a new variant
/// here is a deliberate contract change and must update RIPR-SPEC-0026.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum LanguageId {
    Rust,
    TypeScript,
    Python,
}
