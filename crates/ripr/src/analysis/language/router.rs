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
    use std::path::PathBuf;

    #[test]
    fn route_maps_supported_extensions_to_language_ids() {
        let cases = [
            ("src/lib.rs", LanguageId::Rust),
            ("web/app.ts", LanguageId::TypeScript),
            ("web/app.tsx", LanguageId::TypeScript),
            ("web/app.js", LanguageId::TypeScript),
            ("web/app.jsx", LanguageId::TypeScript),
            ("tests/test_checkout.py", LanguageId::Python),
        ];

        for (path, expected) in cases {
            assert_eq!(route(&PathBuf::from(path)), Some(expected));
        }
    }

    #[test]
    fn route_returns_none_for_unknown_missing_or_non_utf8_extensions() {
        assert_eq!(route(&PathBuf::from("README.md")), None);
        assert_eq!(route(&PathBuf::from("Makefile")), None);

        #[cfg(unix)]
        {
            use std::ffi::OsString;
            use std::os::unix::ffi::OsStringExt;

            let mut path = PathBuf::from("src/lib.");
            path.set_extension(OsString::from_vec(vec![0xFF]));
            assert_eq!(route(&path), None);
        }
    }
}
