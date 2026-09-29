//! Relative imports of `tsc` build output (#4551): a test importing
//! `../build/lib/x.js` relates to `lib/x.ts` through the root tsconfig.json
//! `outDir`/`rootDir`, independent of `resolve_tsconfig_paths`.

use super::*;

const OWNER: &str = "export function looksLikeNumber (x: null | undefined | number | string): boolean {\n  if (x === null || x === undefined) return false\n  return /^[-]?(?:\\d+(?:\\.\\d*)?|\\.\\d+)(e[-+]?\\d+)?$/.test(x as string)\n}\n";
const CHANGED: (usize, &str) = (2, "  if (x === null || x === undefined) return false");
const TEST: &str = "/* global describe, it */\n\nimport { strictEqual } from 'assert'\nimport { looksLikeNumber } from '../build/lib/string-utils.js'\n\ndescribe('string-utils', function () {\n  it('looksLikeNumber', function () {\n    strictEqual(looksLikeNumber('3293'), true)\n    strictEqual(looksLikeNumber(null), false)\n  })\n})\n";
const TSCONFIG: &str = "{\n  \"extends\": \"./node_modules/gts/tsconfig-google.json\",\n  \"compilerOptions\": {\n    \"outDir\": \"build\",\n    \"rootDir\": \".\",\n  },\n}\n";

fn looks_like_number_class(label: &str, tsconfig: Option<&str>) -> Result<ExposureClass, String> {
    let root = ts_unique_tempdir(label)?;
    ts_write_file(&root.join("lib/string-utils.ts"), OWNER)?;
    ts_write_file(&root.join("test/string-utils.mjs"), TEST)?;
    if let Some(tsconfig) = tsconfig {
        ts_write_file(&root.join("tsconfig.json"), tsconfig)?;
    }
    let result = TypeScriptAdapter.analyze_diff(
        &ts_analysis_options(root.clone()),
        &OraclePolicy::default(),
        &[changed_with_lines("lib/string-utils.ts", &[CHANGED])],
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
                .is_some_and(|owner| owner.0.ends_with("looksLikeNumber"))
        })
        .map(|finding| finding.class)
        .ok_or_else(|| format!("{label}: expected a finding for `looksLikeNumber`"))
}

#[test]
fn build_output_import_relates_to_typescript_source_owner() -> Result<(), String> {
    let class = looks_like_number_class("outdir-e2e", Some(TSCONFIG))?;
    // The test is related (the literal `strictEqual` oracle grips it); the
    // exact strength is the classifier's call, not this relation's.
    assert!(
        matches!(class, ExposureClass::Exposed | ExposureClass::WeaklyExposed),
        "expected a related exposure, got {class:?}"
    );
    Ok(())
}

#[test]
fn build_output_import_without_out_dir_stays_no_static_path() -> Result<(), String> {
    let class = looks_like_number_class("outdir-e2e-neg", None)?;
    assert_eq!(class, ExposureClass::NoStaticPath);
    Ok(())
}
