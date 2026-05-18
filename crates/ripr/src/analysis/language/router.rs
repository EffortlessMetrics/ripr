//! Language router: maps source paths to language identifiers.
//!
//! See `docs/specs/RIPR-SPEC-0026-language-adapter-contract.md`.
//!
//! Routing is path-based and stable. Per-repo opt-in for preview adapters
//! is enforced at the pipeline layer where adapter dispatch happens.

use super::LanguageId;
use std::path::Path;

/// Map a source-file path to the language adapter that should handle it.
///
/// Returns `None` when no adapter handles the path. Matched paths route to
/// at most one adapter. Preview adapters (TypeScript, Python) are reported
/// here regardless of repo configuration; the pipeline layer is responsible
/// for honoring `[languages]` opt-in before dispatching to a preview
/// adapter.
pub(crate) fn route(path: &Path) -> Option<LanguageId> {
    let ext = path.extension()?.to_str()?;
    match ext {
        "rs" => Some(LanguageId::Rust),
        "ts" | "tsx" | "js" | "jsx" => Some(LanguageId::TypeScript),
        "py" => Some(LanguageId::Python),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_rust_and_preview_languages_by_extension() {
        let cases = [
            ("src/lib.rs", LanguageId::Rust),
            ("web/app.ts", LanguageId::TypeScript),
            ("web/app.tsx", LanguageId::TypeScript),
            ("web/app.js", LanguageId::TypeScript),
            ("web/app.jsx", LanguageId::TypeScript),
            ("tests/test_retry.py", LanguageId::Python),
        ];

        for (path, expected) in cases {
            assert_eq!(route(Path::new(path)), Some(expected));
        }
    }

    #[test]
    fn route_ignores_unknown_or_extensionless_paths() {
        assert_eq!(route(Path::new("README.md")), None);
        assert_eq!(route(Path::new("Makefile")), None);
    }
}
