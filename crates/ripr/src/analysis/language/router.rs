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
    fn routes_supported_extensions_to_one_adapter() {
        assert_eq!(route(Path::new("src/lib.rs")), Some(LanguageId::Rust));
        assert_eq!(
            route(Path::new("web/component.tsx")),
            Some(LanguageId::TypeScript)
        );
        assert_eq!(
            route(Path::new("web/app.jsx")),
            Some(LanguageId::TypeScript)
        );
        assert_eq!(
            route(Path::new("scripts/test_fixture.py")),
            Some(LanguageId::Python)
        );
    }

    #[test]
    fn ignores_unknown_or_non_utf8_extensions() {
        use std::ffi::OsString;
        use std::path::PathBuf;

        assert_eq!(route(Path::new("README.md")), None);
        assert_eq!(route(Path::new("Makefile")), None);

        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;

            let path = PathBuf::from(OsString::from_vec(b"src/file.\xFF".to_vec()));
            assert_eq!(route(&path), None);
        }
    }
}
