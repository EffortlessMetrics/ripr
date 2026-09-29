//! Directory module specifiers (#4546): `require('..')`, `require('.')`,
//! `import x from '..'` are relative, and a specifier naming an in-root
//! directory resolves through its `package.json` `main`, else its `index`
//! file — only when no sibling file module wins and only on real files.

use super::*;

fn resolve(root: &Path, test_file: &str, specifier: &str) -> Option<String> {
    normalized_relative_import_module(Path::new(test_file), specifier, None, Some(root))
}

fn with_tree(label: &str, files: &[(&str, &str)]) -> Result<PathBuf, String> {
    let root = ts_unique_tempdir(label)?;
    for (path, contents) in files {
        ts_write_file(&root.join(path), contents)?;
    }
    Ok(root)
}

#[test]
fn bare_dot_and_dot_dot_specifiers_are_relative_without_a_root() {
    let at = |specifier: &str| {
        normalized_relative_import_module(Path::new("test/unit/a.test.js"), specifier, None, None)
    };
    assert_eq!(at("..").as_deref(), Some("test"));
    assert_eq!(at(".").as_deref(), Some("test/unit"));
    // Near-misses stay non-relative (no alias map → unresolved).
    assert_eq!(at("..foo"), None);
    assert_eq!(at(".foo"), None);
    assert_eq!(at("..."), None);
}

#[test]
fn parent_directory_resolves_to_root_index_file() -> Result<(), String> {
    let root = with_tree(
        "dir-index",
        &[
            ("index.js", "exports.charset = function charset() {};\n"),
            ("test/test.js", ""),
            ("lib/index.ts", "export const x = 1;\n"),
        ],
    )?;
    let parent = resolve(&root, "test/test.js", "..");
    let lib = resolve(&root, "test/test.js", "../lib");
    let lib_slash = resolve(&root, "test/test.js", "../lib/");
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(parent.as_deref(), Some("index"));
    assert_eq!(lib.as_deref(), Some("lib/index"));
    assert_eq!(lib_slash.as_deref(), Some("lib/index"));
    Ok(())
}

#[test]
fn package_json_main_wins_over_index() -> Result<(), String> {
    let root = with_tree(
        "dir-main",
        &[
            (
                "package.json",
                r#"{ "name": "m", "main": "./lib/entry.js" }"#,
            ),
            ("index.js", ""),
            ("lib/entry.js", ""),
            ("pkg/package.json", r#"{ "main": "src" }"#),
            ("pkg/src/index.ts", ""),
            ("bare/package.json", r#"{ "main": "main" }"#),
            ("bare/main.cjs", ""),
        ],
    )?;
    let parent = resolve(&root, "test/test.js", "..");
    let dir_main = resolve(&root, "test/test.js", "../pkg");
    let bare_main = resolve(&root, "test/test.js", "../bare");
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(parent.as_deref(), Some("lib/entry"));
    assert_eq!(dir_main.as_deref(), Some("pkg/src/index"));
    assert_eq!(bare_main.as_deref(), Some("bare/main"));
    Ok(())
}

#[test]
fn directory_resolution_fails_closed_to_the_lexical_module() -> Result<(), String> {
    let root = with_tree(
        "dir-negative",
        &[
            // A file module beside the directory wins the lookup.
            ("src.ts", ""),
            ("src/index.ts", ""),
            // A main that escapes the root, is absolute, missing, not a
            // string, or unparseable does not fall back to a guess.
            ("escape/package.json", r#"{ "main": "../../outside.js" }"#),
            ("escape/index.js", ""),
            ("absolute/package.json", r#"{ "main": "/etc/index.js" }"#),
            ("absolute/index.js", ""),
            ("missing/package.json", r#"{ "main": "gone.js" }"#),
            ("missing/index.js", ""),
            ("numeric/package.json", r#"{ "main": 3 }"#),
            ("numeric/index.js", ""),
            ("broken/package.json", "{ not json"),
            ("broken/index.js", ""),
            // A directory without an index file stays unresolved.
            ("empty/readme.md", ""),
            // An explicit extension never becomes a directory lookup.
            ("ext.js/index.js", ""),
        ],
    )?;
    let cases = [
        ("../src", "src"),
        ("../escape", "escape"),
        ("../absolute", "absolute"),
        ("../missing", "missing"),
        ("../numeric", "numeric"),
        ("../broken", "broken"),
        ("../empty", "empty"),
        ("../ext.js", "ext"),
        ("../nowhere", "nowhere"),
    ];
    let resolved: Vec<(&str, Option<String>)> = cases
        .iter()
        .map(|(specifier, _)| (*specifier, resolve(&root, "test/test.js", specifier)))
        .collect();
    // A join that escapes the workspace root keeps its lexical module.
    let escaped = resolve(&root, "a.test.js", "..");
    let _ = std::fs::remove_dir_all(&root);
    for ((specifier, expected), (_, actual)) in cases.iter().zip(&resolved) {
        assert_eq!(actual.as_deref(), Some(*expected), "specifier {specifier}");
    }
    assert_eq!(escaped.as_deref(), Some(""));
    Ok(())
}

const OWNER: &str = "'use strict';\n\nexports.charset = charset;\n\nfunction charset(type) {\n  return type === 'text/html' ? 'UTF-8' : false;\n}\n";
const CHANGED: (usize, &str) = (6, "  return type === 'text/html' ? 'UTF-8' : false;");

fn charset_class(label: &str, test_source: &str) -> Result<ExposureClass, String> {
    let root = ts_unique_tempdir(label)?;
    ts_write_file(&root.join("index.js"), OWNER)?;
    ts_write_file(&root.join("test/test.js"), test_source)?;
    let result = TypeScriptAdapter.analyze_diff(
        &ts_analysis_options(root.clone()),
        &OraclePolicy::default(),
        &[changed_with_lines("index.js", &[CHANGED])],
    );
    let _ = std::fs::remove_dir_all(&root);
    result?
        .findings
        .into_iter()
        .find(|finding| {
            finding
                .probe
                .owner
                .as_ref()
                .is_some_and(|owner| owner.0.ends_with("charset"))
        })
        .map(|finding| finding.class)
        .ok_or_else(|| format!("{label}: expected a finding for `charset`"))
}

#[test]
fn require_parent_directory_relates_to_root_index_owner() -> Result<(), String> {
    let class = charset_class(
        "dir-e2e",
        "var mimeTypes = require('..');\n\ntest('charset', function () {\n  expect(mimeTypes.charset('text/html')).toBe('UTF-8');\n});\n",
    )?;
    assert_eq!(class, ExposureClass::Exposed);
    Ok(())
}

#[test]
fn require_of_other_directory_does_not_relate_to_root_index_owner() -> Result<(), String> {
    let class = charset_class(
        "dir-e2e-neg",
        "var mimeTypes = require('.');\n\ntest('charset', function () {\n  expect(mimeTypes.charset('text/html')).toBe('UTF-8');\n});\n",
    )?;
    assert_ne!(class, ExposureClass::Exposed);
    Ok(())
}

/// `..` is relative at every site (#4638 review): a destructured
/// `require('..')` that resolves to no file (no root `index`, no
/// `package.json`) gets the relative-import-unresolved disclosure, never the
/// "alias resolution is not enabled" advice meant for bare aliases.
#[test]
fn unresolved_parent_directory_require_is_disclosed_as_relative_not_alias() -> Result<(), String> {
    let root = ts_unique_tempdir("dir-unresolved-parent")?;
    ts_write_file(
        &root.join("src/calc.js"),
        "exports.isAdult = function (age) {\n  return age >= 18;\n};\n",
    )?;
    ts_write_file(
        &root.join("test/calc.test.js"),
        "const { isAdult } = require('..');\n\ntest('adult', () => {\n  expect(isAdult(18)).toBe(true);\n});\n",
    )?;
    let result = TypeScriptAdapter.analyze_diff(
        &ts_analysis_options(root.clone()),
        &OraclePolicy::default(),
        &[changed_with_lines(
            "src/calc.js",
            &[(2, "  return age >= 18;")],
        )],
    );
    let _ = std::fs::remove_dir_all(&root);
    let finding = result?
        .findings
        .into_iter()
        .next()
        .ok_or("expected a finding for `isAdult`")?;
    let rendered = format!("{finding:?}");
    assert!(
        finding
            .evidence
            .iter()
            .any(|line| line.contains("typescript_relative_import_unresolved")),
        "`require('..')` must be disclosed as an unresolved relative import: {:?}",
        finding.evidence
    );
    assert!(
        !rendered.contains("alias resolution is not enabled"),
        "a relative specifier must not get alias advice: {rendered}"
    );
    Ok(())
}

/// Directory resolution is memoized for one run (#4638 review): inside a
/// `DirectoryModuleCacheScope` a repeated question is answered from the
/// cache (the removed `index.js` is not re-probed), while outside any scope,
/// and in a later scope, the filesystem is consulted again — no answer leaks
/// across runs.
#[test]
fn directory_resolution_is_memoized_per_run_only() -> Result<(), String> {
    let root = with_tree(
        "dir-memo",
        &[
            ("index.js", "exports.f = () => 1;\n"),
            ("test/a.test.js", ""),
        ],
    )?;
    let (first, cached) = {
        let _run = DirectoryModuleCacheScope::open();
        let first = resolve(&root, "test/a.test.js", "..");
        std::fs::remove_file(root.join("index.js")).map_err(|error| error.to_string())?;
        (first, resolve(&root, "test/a.test.js", ".."))
    };
    let unscoped = resolve(&root, "test/a.test.js", "..");
    let next_run = {
        let _run = DirectoryModuleCacheScope::open();
        resolve(&root, "test/a.test.js", "..")
    };
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(first.as_deref(), Some("index"));
    assert_eq!(
        cached.as_deref(),
        Some("index"),
        "same-run answer is memoized"
    );
    assert_eq!(unscoped.as_deref(), Some(""), "no scope, no cache");
    assert_eq!(next_run.as_deref(), Some(""), "a new run re-probes");
    Ok(())
}
