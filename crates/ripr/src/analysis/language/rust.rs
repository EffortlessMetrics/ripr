//! Reference adapter for Rust.
//!
//! See `docs/specs/RIPR-SPEC-0026-language-adapter-contract.md`.
//!
//! This work item adds the type and the trait implementation. Later work
//! items move the existing Rust fact extraction in `analysis::facts`,
//! `analysis::syntax`, `analysis::extract`, and `analysis::probes` behind
//! this adapter without changing observable analyzer behavior, fixtures,
//! goldens, or output schemas.

use super::{LanguageAdapter, LanguageId, route};
use std::path::Path;

/// Reference adapter for Rust. Stateless: routing only in this work item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RustAdapter;

impl LanguageAdapter for RustAdapter {
    fn accepts_path(&self, path: &Path) -> bool {
        matches!(route(path), Some(LanguageId::Rust))
    }
}
