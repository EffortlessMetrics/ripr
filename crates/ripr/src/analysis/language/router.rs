//! Language router: maps source paths to language identifiers.
//!
//! See `docs/specs/RIPR-SPEC-0026-language-adapter-contract.md`.
//!
//! Routing is path-based and stable. Per-repo opt-in for preview adapters
//! is enforced at the pipeline layer where adapter dispatch happens.

use super::LanguageId;
use std::path::Path;

/// Canonical TypeScript source extensions (RIPR #4116). One owner lists the
/// routed TS/JS suffixes so downstream consumers (repair route, targeted
/// rerun, packet projection, first-use output, LSP, doctor) cannot drift
/// from the router's surface.
pub(crate) const TYPESCRIPT_SOURCE_EXTENSIONS: &[&str] = &["ts", "tsx", "mts", "cts"];

/// Canonical JavaScript source extensions (RIPR #4116); see
/// [`TYPESCRIPT_SOURCE_EXTENSIONS`].
pub(crate) const JAVASCRIPT_SOURCE_EXTENSIONS: &[&str] = &["js", "jsx", "mjs", "cjs"];

/// Which side of the TypeScript/JavaScript source family an extension
/// belongs to. `.mts`/`.cts` are TypeScript; `.mjs`/`.cjs` are JavaScript.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TsJsSourceKind {
    TypeScript,
    JavaScript,
}

/// Classify an exact file extension into the canonical TypeScript/JavaScript
/// source family. Matching is exact against the shared lists — never
/// substring or final-character based — so near-misses such as `.mt`,
/// `.cj`, `.mjsx`, or `.ctsx` and every non-TS/JS extension return `None`.
/// Callers that historically folded case normalize before calling.
pub(crate) fn ts_js_source_kind(extension: &str) -> Option<TsJsSourceKind> {
    if TYPESCRIPT_SOURCE_EXTENSIONS.contains(&extension) {
        Some(TsJsSourceKind::TypeScript)
    } else if JAVASCRIPT_SOURCE_EXTENSIONS.contains(&extension) {
        Some(TsJsSourceKind::JavaScript)
    } else {
        None
    }
}

/// Whether an exact file extension is one of the eight routed TS/JS source
/// extensions; see [`ts_js_source_kind`].
pub(crate) fn is_ts_js_source_extension(extension: &str) -> bool {
    ts_js_source_kind(extension).is_some()
}

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
        // The whole TS/JS family rides the TypeScript adapter; `.d.ts`
        // declarations keep routing here incidentally because
        // `Path::extension` reports "ts" for them.
        _ if is_ts_js_source_extension(ext) => Some(LanguageId::TypeScript),
        "py" => Some(LanguageId::Python),
        "pm" | "pl" | "t" | "psgi" => Some(LanguageId::Perl),
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
            ("web/app.mts", LanguageId::TypeScript),
            ("web/app.cts", LanguageId::TypeScript),
            ("web/app.mjs", LanguageId::TypeScript),
            ("web/app.cjs", LanguageId::TypeScript),
            // `.d.ts` declarations keep routing via "ts" (`Path::extension`
            // of `foo.d.ts` is "ts") and must remain accepted.
            ("web/app.d.ts", LanguageId::TypeScript),
            ("tests/test_retry.py", LanguageId::Python),
        ];

        for (path, expected) in cases {
            assert_eq!(route(Path::new(path)), Some(expected));
        }
    }

    #[test]
    fn route_perl_extensions_regardless_of_adapter_availability() {
        assert_eq!(route(Path::new("lib/My/App.pm")), Some(LanguageId::Perl));
        assert_eq!(route(Path::new("script/run.pl")), Some(LanguageId::Perl));
        assert_eq!(route(Path::new("t/app.t")), Some(LanguageId::Perl));
        assert_eq!(route(Path::new("app.psgi")), Some(LanguageId::Perl));
    }

    #[test]
    fn route_ignores_unknown_or_extensionless_paths() {
        assert_eq!(route(Path::new("README.md")), None);
        assert_eq!(route(Path::new("Makefile")), None);
    }

    #[test]
    fn ts_js_extension_authority_classifies_exactly_the_routed_surface() {
        // #4116 removal control: deleting any extension from the shared
        // authority must flip at least one of these parity rows.
        let cases = [
            ("ts", Some(TsJsSourceKind::TypeScript)),
            ("tsx", Some(TsJsSourceKind::TypeScript)),
            ("mts", Some(TsJsSourceKind::TypeScript)),
            ("cts", Some(TsJsSourceKind::TypeScript)),
            ("js", Some(TsJsSourceKind::JavaScript)),
            ("jsx", Some(TsJsSourceKind::JavaScript)),
            ("mjs", Some(TsJsSourceKind::JavaScript)),
            ("cjs", Some(TsJsSourceKind::JavaScript)),
        ];

        for (extension, expected) in cases {
            assert_eq!(ts_js_source_kind(extension), expected, ".{extension}");
            assert!(is_ts_js_source_extension(extension), ".{extension}");
        }

        // Near-misses and unrelated extensions stay unknown; matching is
        // exact, never substring or final-character based.
        for extension in ["mt", "cj", "mjsx", "ctsx", "ts2", "tsx?", "py", "rs", ""] {
            assert_eq!(ts_js_source_kind(extension), None, ".{extension}");
            assert!(!is_ts_js_source_extension(extension), ".{extension}");
        }
    }
}
