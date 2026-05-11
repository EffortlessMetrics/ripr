//! Language adapter boundary for the analysis pipeline.
//!
//! See `docs/specs/RIPR-SPEC-0026-language-adapter-contract.md`.
//!
//! This module introduces the typed seam between the diff/workspace router
//! and per-language fact extraction. The Rust adapter is the reference
//! adapter; TypeScript and Python preview adapters are added under later
//! work items in Campaign 27.
//!
//! The seam is scaffolding-only in this work item:
//!
//! - the `LanguageAdapter` trait declares the routing predicate
//! - the [`route`] function maps paths to language identifiers
//! - [`RustAdapter`] is the reference type for Rust and is wired into
//!   workspace discovery so the seam is alive without changing Rust
//!   analyzer behavior, fixtures, or goldens
//!
//! Subsequent work items add the `language` discriminator alongside the
//! additive optional `language` / `language_status` output metadata, move
//! Rust fact extraction behind this trait, and add the preview adapters
//! per RIPR-SPEC-0027 and RIPR-SPEC-0028.

mod adapter;
mod id;
mod router;
mod rust;

pub(crate) use adapter::LanguageAdapter;
pub(crate) use id::LanguageId;
pub(crate) use router::route;
pub(crate) use rust::RustAdapter;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn rust_adapter_accepts_rs_extension_only() {
        let adapter = RustAdapter;
        assert!(adapter.accepts_path(Path::new("src/lib.rs")));
        assert!(adapter.accepts_path(Path::new("crates/ripr/src/main.rs")));
        assert!(!adapter.accepts_path(Path::new("README.md")));
        assert!(!adapter.accepts_path(Path::new("src/index.ts")));
        assert!(!adapter.accepts_path(Path::new("src/index.tsx")));
        assert!(!adapter.accepts_path(Path::new("scripts/run.py")));
        assert!(!adapter.accepts_path(Path::new("no-extension")));
    }

    #[test]
    fn router_dispatches_known_extensions() {
        assert_eq!(route(Path::new("src/lib.rs")), Some(LanguageId::Rust));
        assert_eq!(
            route(Path::new("src/index.ts")),
            Some(LanguageId::TypeScript)
        );
        assert_eq!(
            route(Path::new("src/index.tsx")),
            Some(LanguageId::TypeScript)
        );
        assert_eq!(
            route(Path::new("src/index.js")),
            Some(LanguageId::TypeScript)
        );
        assert_eq!(
            route(Path::new("src/index.jsx")),
            Some(LanguageId::TypeScript)
        );
        assert_eq!(route(Path::new("scripts/run.py")), Some(LanguageId::Python));
    }

    #[test]
    fn router_returns_none_for_unhandled_paths() {
        assert!(route(Path::new("docs/README.md")).is_none());
        assert!(route(Path::new("Cargo.toml")).is_none());
        assert!(route(Path::new("no-extension")).is_none());
        assert!(route(Path::new("src/lib.Rs")).is_none());
    }
}
