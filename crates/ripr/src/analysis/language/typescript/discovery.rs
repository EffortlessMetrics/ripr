//! Workspace discovery for the TypeScript preview adapter.

use super::*;

/// Whether a path is a test file by convention.
///
/// Two recognized conventions:
/// 1. `*.test.ts` / `*.spec.ts` naming (and `.tsx` / `.js` / `.jsx` variants) —
///    the Jest/Vitest default.
/// 2. A TypeScript/JavaScript source under a `test/`, `tests/`, or `__tests__/`
///    **directory** with a feature name (no `.test`/`.spec` suffix) — the AVA /
///    Mocha / node:test / tape convention (e.g. `test/body-size.ts`). Dogfood
///    (sindresorhus/ky): without this, ripr never scans these files and reports
///    a false `no_static_path` ("you have no tests") when a test exists.
///
/// The directory match is on an exact path component (not a substring), so
/// `src/latest/foo.ts` and `test-utils/foo.ts` are NOT treated as tests. This is
/// fail-closed: the test extractor only yields tests for files that actually
/// contain `test()` / `it()` / `describe()` calls, so non-test helpers or
/// fixtures under a `test/` directory produce no tests.
pub(crate) fn is_test_file(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let stem_extensions: &[&str] = &[
        ".test.ts",
        ".test.tsx",
        ".test.js",
        ".test.jsx",
        ".spec.ts",
        ".spec.tsx",
        ".spec.js",
        ".spec.jsx",
    ];
    if stem_extensions
        .iter()
        .any(|suffix| file_name.ends_with(suffix))
    {
        return true;
    }

    const TS_JS_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs"];
    let has_ts_js_extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| TS_JS_EXTENSIONS.contains(&ext));
    has_ts_js_extension
        && path.components().any(|component| {
            matches!(
                component.as_os_str().to_str(),
                Some("test") | Some("tests") | Some("__tests__")
            )
        })
}

pub(crate) fn collect_workspace_typescript_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    visit_workspace(root, root, &mut out);
    out.sort();
    out
}

pub(crate) fn visit_workspace(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if name == ".git"
            || name == "target"
            || name == "node_modules"
            || name == ".ripr"
            || name == ".direnv"
        {
            continue;
        }
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            visit_workspace(root, &path, out);
        } else if file_type.is_file() {
            let adapter = TypeScriptAdapter;
            if adapter.accepts_path(&path) {
                let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                out.push(relative);
            }
        }
    }
}
