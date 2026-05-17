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
    use super::{LanguageId, route};
    use std::path::Path;

    #[test]
    fn routes_stable_and_preview_source_extensions() {
        for (path, expected) in [
            ("src/lib.rs", LanguageId::Rust),
            ("src/app.ts", LanguageId::TypeScript),
            ("src/component.tsx", LanguageId::TypeScript),
            ("src/app.js", LanguageId::TypeScript),
            ("src/component.jsx", LanguageId::TypeScript),
            ("src/validation.py", LanguageId::Python),
        ] {
            assert_eq!(route(Path::new(path)), Some(expected), "route for {path}");
        }
    }

    #[test]
    fn leaves_unknown_extension_and_extensionless_paths_unrouted() {
        assert_eq!(route(Path::new("README.md")), None);
        assert_eq!(route(Path::new("Makefile")), None);
    }

    #[cfg(unix)]
    #[test]
    fn leaves_non_utf8_paths_unrouted() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        use std::path::PathBuf;

        let non_utf8 = PathBuf::from(OsString::from_vec(vec![b's', b'r', b'c', b'/', 0xff]));
        assert_eq!(route(&non_utf8), None);
    }
}
