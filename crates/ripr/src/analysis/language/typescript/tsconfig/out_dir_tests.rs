//! Build-output (`outDir`) mapping controls (#4551): a relative import of
//! `tsc` output maps back to the TypeScript source only through the root
//! tsconfig.json's own `outDir`/`rootDir`, and only onto an existing file.

use super::*;
use crate::analysis::language::typescript::{
    DirectoryModuleCacheScope, normalized_relative_import_module,
};
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static FIXTURE_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tree(label: &str, files: &[(&str, &str)]) -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("fixture clock: {err}"))?
        .as_nanos();
    let seq = FIXTURE_SEQ.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "ripr-tsconfig-outdir-{label}-{}-{stamp}-{seq}",
        std::process::id()
    ));
    for (path, contents) in files {
        let absolute = root.join(path);
        let parent = absolute.parent().ok_or("fixture path has no parent")?;
        fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))?;
        fs::write(&absolute, contents)
            .map_err(|err| format!("write {}: {err}", absolute.display()))?;
    }
    fs::create_dir_all(&root).map_err(|err| format!("create {}: {err}", root.display()))?;
    Ok(root)
}

/// Resolve `specifier` from `test_file` through the production relative
/// resolver, then remove the fixture.
fn resolve_in(
    label: &str,
    files: &[(&str, &str)],
    test_file: &str,
    specifier: &str,
) -> Result<Option<String>, String> {
    let root = tree(label, files)?;
    let resolved =
        normalized_relative_import_module(Path::new(test_file), specifier, None, Some(&root));
    let _ = fs::remove_dir_all(&root);
    Ok(resolved)
}

const BUILD_ROOT: &str = r#"{
  // yargs-parser shape: extends a package config, own outDir/rootDir.
  "extends": "./node_modules/gts/tsconfig-google.json",
  "compilerOptions": { "outDir": "build", "rootDir": ".", },
}"#;

#[test]
fn build_js_import_maps_to_ts_source() -> Result<(), String> {
    let resolved = resolve_in(
        "js",
        &[("tsconfig.json", BUILD_ROOT), ("lib/string-utils.ts", "")],
        "test/string-utils.mjs",
        "../build/lib/string-utils.js",
    )?;
    assert_eq!(resolved.as_deref(), Some("lib/string-utils"));
    Ok(())
}

#[test]
fn build_mjs_and_cjs_imports_map_to_mts_and_cts_sources() -> Result<(), String> {
    let files = [
        (
            "tsconfig.json",
            r#"{ "compilerOptions": { "outDir": "./build/", "rootDir": "." } }"#,
        ),
        ("lib/esm.mts", ""),
        ("lib/cjs.cts", ""),
        // A `.ts` beside the `.mts` is not what `.mjs` output came from.
        ("lib/esm.ts", ""),
    ];
    let esm = resolve_in("mjs", &files, "test/a.test.mjs", "../build/lib/esm.mjs")?;
    let cjs = resolve_in("cjs", &files, "test/a.test.mjs", "../build/lib/cjs.cjs")?;
    assert_eq!(esm.as_deref(), Some("lib/esm"));
    assert_eq!(cjs.as_deref(), Some("lib/cjs"));
    Ok(())
}

#[test]
fn root_dir_src_and_out_dir_dist_map_to_src() -> Result<(), String> {
    let resolved = resolve_in(
        "src-dist",
        &[
            (
                "tsconfig.json",
                r#"{ "compilerOptions": { "outDir": "dist", "rootDir": "./src" } }"#,
            ),
            ("src/cart.ts", ""),
        ],
        "test/cart.test.js",
        "../dist/cart",
    )?;
    assert_eq!(resolved.as_deref(), Some("src/cart"));
    Ok(())
}

#[test]
fn unmapped_imports_keep_the_lexical_build_module() -> Result<(), String> {
    let source = ("lib/string-utils.ts", "");
    let cases: [(&str, Vec<(&str, &str)>); 6] = [
        // No tsconfig.json at all.
        ("no-config", vec![source]),
        // No own outDir (an inherited one is not guessed).
        (
            "no-outdir",
            vec![
                (
                    "tsconfig.json",
                    r#"{ "extends": "./base.json", "compilerOptions": { "rootDir": "." } }"#,
                ),
                (
                    "base.json",
                    r#"{ "compilerOptions": { "outDir": "build" } }"#,
                ),
                source,
            ],
        ),
        // The mapped source does not exist.
        (
            "missing",
            vec![(
                "tsconfig.json",
                r#"{ "compilerOptions": { "outDir": "build" } }"#,
            )],
        ),
        // A real emitted file under build/ wins.
        (
            "real-build",
            vec![
                ("tsconfig.json", BUILD_ROOT),
                source,
                ("build/lib/string-utils.js", ""),
            ],
        ),
        // Unparseable config.
        (
            "broken",
            vec![("tsconfig.json", "{ \"compilerOptions\": "), source],
        ),
        // `.ts` and `.tsx` both present is ambiguous.
        (
            "ambiguous",
            vec![
                ("tsconfig.json", BUILD_ROOT),
                source,
                ("lib/string-utils.tsx", ""),
            ],
        ),
    ];
    for (label, files) in &cases {
        let resolved = resolve_in(
            label,
            files,
            "test/string-utils.mjs",
            "../build/lib/string-utils.js",
        )?;
        assert_eq!(
            resolved.as_deref(),
            Some("build/lib/string-utils"),
            "case {label}"
        );
    }
    Ok(())
}

#[test]
fn escaping_or_absolute_config_dirs_yield_no_mapping() {
    for config in [
        r#"{ "compilerOptions": { "outDir": "../build" } }"#,
        r#"{ "compilerOptions": { "outDir": "/tmp/build" } }"#,
        r#"{ "compilerOptions": { "outDir": "C:build" } }"#,
        r#"{ "compilerOptions": { "outDir": "." } }"#,
        r#"{ "compilerOptions": { "outDir": "build", "rootDir": "../pkg" } }"#,
        r#"{ "compilerOptions": { "outDir": "build", "rootDir": "/src" } }"#,
        r#"{ "compilerOptions": { "outDir": "build", "rootDir": 3 } }"#,
        r#"{ "compilerOptions": { "outDir": 3 } }"#,
        // No own `rootDir`: `tsc` infers it from the inputs (#4800 review).
        r#"{ "compilerOptions": { "outDir": "build" } }"#,
        r#"{ "include": ["src"], "compilerOptions": { "outDir": "dist" } }"#,
    ] {
        assert_eq!(parse_out_dir_map(config), None, "config {config}");
    }
    assert_eq!(
        parse_out_dir_map(r#"{ "compilerOptions": { "outDir": "./out/lib/", "rootDir": "./" } }"#),
        Some(TsOutDirMap {
            out_dir: "out/lib".to_string(),
            root_dir: String::new(),
        })
    );
}

#[test]
fn root_escaping_import_is_not_mapped() -> Result<(), String> {
    // `../../build` from a root-level test escapes the workspace root.
    let resolved = resolve_in(
        "escape",
        &[("tsconfig.json", BUILD_ROOT), ("lib/x.ts", "")],
        "x.test.js",
        "../build/lib/x.js",
    )?;
    assert_ne!(resolved.as_deref(), Some("lib/x"));
    Ok(())
}

/// Negative (#4800 review): with no own `rootDir`, `tsc` infers `src` from
/// `include: ["src"]` (or inherits it through `extends`), so `dist/index.js`
/// came from `src/index.ts`. Mapping it onto a root-level `index.ts` shim
/// relates the test to code it never runs; no mapping is made.
#[test]
fn inferred_or_inherited_root_dir_is_not_guessed() -> Result<(), String> {
    for (label, files) in [
        (
            "inferred",
            vec![(
                "tsconfig.json",
                r#"{ "include": ["src"], "compilerOptions": { "outDir": "dist" } }"#,
            )],
        ),
        (
            "inherited",
            vec![
                (
                    "tsconfig.json",
                    r#"{ "extends": "./base.json", "compilerOptions": { "outDir": "dist" } }"#,
                ),
                (
                    "base.json",
                    r#"{ "compilerOptions": { "rootDir": "src" } }"#,
                ),
            ],
        ),
    ] {
        let mut files = files;
        files.extend([("src/index.ts", ""), ("index.ts", "")]);
        let resolved = resolve_in(label, &files, "test/parse.test.ts", "../dist/index.js")?;
        assert_ne!(resolved.as_deref(), Some("index"), "{label}");
        assert_ne!(resolved.as_deref(), Some("src/index"), "{label}");
    }
    Ok(())
}

/// Negative (#4800 review): a symlink at the imported build path is what
/// the test loads, so it wins over the mapping like a real emitted file.
#[cfg(unix)]
#[test]
fn symlink_at_build_path_keeps_the_lexical_module() -> Result<(), String> {
    let root = tree(
        "symlink",
        &[
            (
                "tsconfig.json",
                r#"{ "compilerOptions": { "outDir": "dist", "rootDir": "src" } }"#,
            ),
            ("src/index.ts", ""),
            ("other.js", ""),
        ],
    )?;
    fs::create_dir_all(root.join("dist")).map_err(|err| format!("create dist: {err}"))?;
    std::os::unix::fs::symlink(root.join("other.js"), root.join("dist/index.js"))
        .map_err(|err| format!("symlink: {err}"))?;
    let resolved = normalized_relative_import_module(
        Path::new("test/a.test.js"),
        "../dist/index.js",
        None,
        Some(&root),
    );
    let _ = fs::remove_dir_all(&root);
    assert_eq!(resolved.as_deref(), Some("dist/index"));
    Ok(())
}

/// The mapping is memoized only inside a run scope: a config edited between
/// runs is read afresh by the next run (#4800 review; no process-global
/// cache).
#[test]
fn out_dir_map_is_reloaded_by_the_next_run() -> Result<(), String> {
    let root = tree(
        "rerun",
        &[
            (
                "tsconfig.json",
                r#"{ "compilerOptions": { "outDir": "build", "rootDir": "." } }"#,
            ),
            ("lib/x.ts", ""),
        ],
    )?;
    let resolve = || {
        normalized_relative_import_module(
            Path::new("test/x.test.js"),
            "../build/lib/x.js",
            None,
            Some(&root),
        )
    };
    let first = {
        let _run = DirectoryModuleCacheScope::open();
        let first = resolve();
        // Same run: an edit is not observed (the run's answer is stable).
        fs::write(root.join("tsconfig.json"), "{}").map_err(|err| format!("rewrite: {err}"))?;
        assert_eq!(resolve(), first);
        first
    };
    let second = {
        let _run = DirectoryModuleCacheScope::open();
        resolve()
    };
    let _ = fs::remove_dir_all(&root);
    assert_eq!(first.as_deref(), Some("lib/x"));
    assert_eq!(second.as_deref(), Some("build/lib/x"));
    Ok(())
}
