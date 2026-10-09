//! CommonJS assignment-export owners (#4545). A changed line inside
//! `exports.NAME = function ...`, `module.exports.NAME = ...`,
//! `module.exports = function NAME ...` or a `module.exports = { ... }`
//! function property must map to an owner; non-function values and
//! computed properties must not invent one.

use super::*;

fn owner_names(file: &str, source: &str) -> Vec<(String, bool)> {
    extract_owners(Path::new(file), source)
        .into_iter()
        .map(|owner| (owner.name, owner.exported_as_default))
        .collect()
}

#[test]
fn exports_property_function_yields_named_owner() {
    let owners = extract_owners(
        Path::new("src/k.cjs"),
        "'use strict';\n\nexports.thrice = function thrice(x) {\n  return x * 3;\n};\n",
    );
    assert_eq!(owners.len(), 1, "owners: {owners:?}");
    assert_eq!(owners[0].name, "thrice");
    assert!(!owners[0].exported_as_default);
    assert_eq!((owners[0].start_line, owners[0].end_line), (3, 5));
    assert_eq!(owners[0].arity, Some(1));
}

#[test]
fn module_exports_property_arrow_and_anonymous_function_yield_named_owners() {
    assert_eq!(
        owner_names(
            "src/k.js",
            "module.exports.twice = (x) => x * 2;\nexports.half = function (x) {\n  return x / 2;\n};\n",
        ),
        vec![("twice".to_string(), false), ("half".to_string(), false)]
    );
}

#[test]
fn module_exports_function_yields_default_owner() {
    assert_eq!(
        owner_names(
            "src/k.cjs",
            "module.exports = function thrice(x) {\n  return x * 3;\n};\n"
        ),
        vec![("thrice".to_string(), true)]
    );
    assert_eq!(
        owner_names("src/k.cjs", "module.exports = (x) => x * 3;\n"),
        vec![("default".to_string(), true)]
    );
}

#[test]
fn module_exports_object_literal_yields_function_property_owners() {
    let source = "module.exports = {\n  thrice(x) {\n    return x * 3;\n  },\n  twice: function (x) {\n    return x * 2;\n  },\n  half: (x) => x / 2,\n  limit: 10,\n  get size() {\n    return 1;\n  },\n  ['computed']: function () {\n    return 1;\n  },\n  'quoted': function () {\n    return 1;\n  },\n  helper,\n};\n";
    let owners = extract_owners(Path::new("src/k.js"), source);
    let names: Vec<(&str, usize, usize, bool)> = owners
        .iter()
        .map(|owner| {
            (
                owner.name.as_str(),
                owner.start_line,
                owner.end_line,
                owner.exported_as_default,
            )
        })
        .collect();
    assert_eq!(
        names,
        vec![
            ("thrice", 2, 4, false),
            ("twice", 5, 7, false),
            ("half", 8, 8, false)
        ]
    );
}

#[test]
fn non_function_and_non_export_assignments_yield_no_owner() {
    for source in [
        "exports.limit = 10;\n",
        "exports.alias = thrice;\n",
        "exports['thrice'] = function (x) { return x * 3; };\n",
        "exports[name] = function (x) { return x * 3; };\n",
        "module['exports'].thrice = function (x) { return x * 3; };\n",
        "other.exports.thrice = function (x) { return x * 3; };\n",
        "module.other.thrice = function (x) { return x * 3; };\n",
        "window.thrice = function (x) { return x * 3; };\n",
        "exports.thrice ||= function (x) { return x * 3; };\n",
        "exports.a = exports.b = function (x) { return x * 3; };\n",
        "module.exports = require('./impl');\n",
        "module.exports = 42;\n",
        "if (ok) { exports.thrice = function (x) { return x * 3; }; }\n",
    ] {
        assert_eq!(
            owner_names("src/k.js", source),
            Vec::<(String, bool)>::new(),
            "no owner expected for {source:?}"
        );
    }
}

const OWNER: &str = "'use strict';\n\nexports.thrice = function thrice(x) {\n  return x * 3;\n};\n";
const CHANGED: (usize, &str) = (4, "  return x * 3;");

/// Analyze `src/k.cjs` with a changed body line and a test file whose body is
/// `test_source`; returns the finding class for the `thrice` owner.
fn thrice_class(label: &str, test_source: &str) -> Result<ExposureClass, String> {
    let root = ts_unique_tempdir(label)?;
    ts_write_file(&root.join("src/k.cjs"), OWNER)?;
    ts_write_file(&root.join("test/k.test.js"), test_source)?;
    let result = TypeScriptAdapter.analyze_diff(
        &ts_analysis_options(root.clone()),
        &OraclePolicy::default(),
        &[changed_with_lines("src/k.cjs", &[CHANGED])],
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
                .is_some_and(|owner| owner.0.ends_with("thrice"))
        })
        .map(|finding| finding.class)
        .ok_or_else(|| format!("{label}: expected a finding for `thrice`"))
}

#[test]
fn namespace_require_of_commonjs_export_relates() -> Result<(), String> {
    let class = thrice_class(
        "cjs-namespace",
        "const k = require('../src/k.cjs');\n\ntest('thrice', () => {\n  expect(k.thrice(2)).toBe(6);\n});\n",
    )?;
    assert_eq!(class, ExposureClass::Exposed);
    Ok(())
}

#[test]
fn destructured_require_of_commonjs_export_relates() -> Result<(), String> {
    let class = thrice_class(
        "cjs-destructure",
        "const { thrice } = require('../src/k.cjs');\n\ntest('thrice', () => {\n  expect(thrice(2)).toBe(6);\n});\n",
    )?;
    assert_eq!(class, ExposureClass::Exposed);
    Ok(())
}

#[test]
fn require_of_unrelated_module_does_not_credit_commonjs_export() -> Result<(), String> {
    let class = thrice_class(
        "cjs-unrelated",
        "const k = require('../src/other.cjs');\n\ntest('thrice', () => {\n  expect(k.thrice(2)).toBe(6);\n});\n",
    )?;
    assert_ne!(class, ExposureClass::Exposed);
    Ok(())
}

/// Negative (#4638 review): `module.exports = { f: fn }` followed by
/// `module.exports.f = ...` defines `f` twice; only one value is live and the
/// syntax walk cannot tell which, so neither becomes an owner (fail-closed).
/// A distinct export in the same file is kept (positive control).
#[test]
fn reassigned_commonjs_export_yields_no_owner() {
    assert_eq!(
        owner_names(
            "src/k.js",
            "module.exports = {\n  f: function (x) {\n    return x > 1;\n  },\n  g: (x) => x + 1,\n};\nmodule.exports.f = function (x) {\n  return x > 2;\n};\nexports.f = (x) => x > 3;\n",
        ),
        vec![("g".to_string(), false)]
    );
}
