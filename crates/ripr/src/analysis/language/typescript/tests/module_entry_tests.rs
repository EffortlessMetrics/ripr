//! Owners reached through a same-module entry: an exported wrapper, or a
//! value a same-module factory built (`export const defu = createDefu()`).
//! Tests of the entry reached the changed private helper at runtime but were
//! reported `no_static_path` (unjs/defu 11ba022, 2026-09-28 OSS replay).

use super::*;

const MERGE: &str = "function _merge(base: any, defaults: any): any {\n  const out = { ...defaults };\n  for (const key of Object.keys(base)) {\n    out[key] = base[key];\n  }\n  return out;\n}\n\nexport function createMerge(): (...args: any[]) => any {\n  return (...args) => args.reduce((p, c) => _merge(p, c), {});\n}\n\nexport const merge = createMerge() as (...args: any[]) => any;\nexport default merge;\n";
const LOOP_LINE: (usize, &str) = (3, "  for (const key of Object.keys(base)) {");

const MERGE_TEST: &str = "import { it, expect } from 'vitest';\nimport { merge } from '../src/merge';\n\nit('merges', () => {\n  expect(merge({ a: 1 }, { b: 2 })).toEqual({ a: 1, b: 2 });\n});\n";

fn merge_finding(label: &str, owner_source: &str, test_source: &str) -> Result<Finding, String> {
    let root = ts_unique_tempdir(label)?;
    ts_write_file(&root.join("src/merge.ts"), owner_source)?;
    ts_write_file(&root.join("tests/merge.test.ts"), test_source)?;
    let result = TypeScriptAdapter.analyze_diff(
        &ts_analysis_options(root.clone()),
        &OraclePolicy::default(),
        &[changed_with_lines("src/merge.ts", &[LOOP_LINE])],
    )?;
    let _ = std::fs::remove_dir_all(&root);
    result
        .findings
        .into_iter()
        .find(|finding| {
            finding
                .probe
                .owner
                .as_ref()
                .is_some_and(|owner| owner.0.ends_with("::_merge"))
        })
        .ok_or_else(|| format!("{label}: expected a finding for `_merge`"))
}

fn relates_through_entry(finding: &Finding) -> bool {
    finding
        .related_tests
        .iter()
        .any(|test| test.relation_reason == Some(crate::domain::RelationReason::HelperOwnerCall))
}

fn assert_entry_related(label: &str, owner_source: &str, test_source: &str) -> Result<(), String> {
    let finding = merge_finding(label, owner_source, test_source)?;
    assert!(
        relates_through_entry(&finding),
        "{label}: the test calls an entry that reaches `_merge`, related: {:?}",
        finding.related_tests
    );
    assert_eq!(finding.ripr.reach.state, StageState::Yes, "{label}");
    // Indirect reach never reaches `exposed` on its own.
    assert_eq!(finding.class, ExposureClass::WeaklyExposed, "{label}");
    assert!(
        finding
            .missing
            .iter()
            .any(|line| line.contains("only through same-module callers")),
        "{label}: missing: {:?}",
        finding.missing
    );
    Ok(())
}

fn assert_not_entry_related(
    label: &str,
    owner_source: &str,
    test_source: &str,
) -> Result<(), String> {
    let finding = merge_finding(label, owner_source, test_source)?;
    assert!(
        !relates_through_entry(&finding),
        "{label}: the test does not reach `_merge` through an entry, related: {:?}",
        finding.related_tests
    );
    assert_ne!(finding.class, ExposureClass::Exposed, "{label}");
    Ok(())
}

fn entry_names(owner_source: &str, owner: &str) -> Vec<String> {
    extract_owners(Path::new("src/merge.ts"), owner_source)
        .into_iter()
        .find(|candidate| candidate.name == owner)
        .map(|candidate| {
            candidate
                .module_entries
                .into_iter()
                .map(|entry| entry.name)
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn factory_product_call_relates_to_private_helper() -> Result<(), String> {
    assert_entry_related("factory-product", MERGE, MERGE_TEST)?;
    let finding = merge_finding("factory-product-summary", MERGE, MERGE_TEST)?;
    assert!(
        finding
            .missing
            .iter()
            .any(|line| line.contains("(`merge`)")),
        "the summary names the entry the test calls: {:?}",
        finding.missing
    );
    Ok(())
}

#[test]
fn default_import_of_factory_product_relates() -> Result<(), String> {
    let test = "import { it, expect } from 'vitest';\nimport combine from '../src/merge';\n\nit('merges', () => {\n  expect(combine({ a: 1 }, { b: 2 })).toEqual({ a: 1, b: 2 });\n});\n";
    assert_entry_related("default-import", MERGE, test)?;
    // The summary names the export the default import binds.
    let finding = merge_finding("default-import-summary", MERGE, test)?;
    assert!(
        finding
            .missing
            .iter()
            .any(|line| line.contains("(`default`)")),
        "missing: {:?}",
        finding.missing
    );
    Ok(())
}

#[test]
fn exported_wrapper_call_relates_to_private_helper() -> Result<(), String> {
    let wrapper = "function _merge(base: any, defaults: any): any {\n  const out = { ...defaults };\n  for (const key of Object.keys(base)) {\n    out[key] = base[key];\n  }\n  return out;\n}\n\nexport function merge(base: any, defaults: any): any {\n  return _merge(base, defaults);\n}\n";
    assert_entry_related("wrapper", wrapper, MERGE_TEST)
}

#[test]
fn entry_imported_from_another_module_does_not_relate() -> Result<(), String> {
    assert_not_entry_related(
        "other-module",
        MERGE,
        "import { it, expect } from 'vitest';\nimport { merge } from 'lodash-es';\n\nit('merges', () => {\n  expect(merge({ a: 1 }, { b: 2 })).toEqual({ a: 1, b: 2 });\n});\n",
    )
}

#[test]
fn mocked_owner_module_does_not_relate_through_entry() -> Result<(), String> {
    assert_not_entry_related(
        "mocked",
        MERGE,
        "import { it, expect, vi } from 'vitest';\nimport { merge } from '../src/merge';\n\nvi.mock('../src/merge');\n\nit('merges', () => {\n  expect(merge({ a: 1 }, { b: 2 })).toEqual({ a: 1, b: 2 });\n});\n",
    )
}

#[test]
fn entry_shadowed_in_test_body_does_not_relate() -> Result<(), String> {
    assert_not_entry_related(
        "body-shadow",
        MERGE,
        "import { it, expect } from 'vitest';\nimport { merge } from '../src/merge';\n\nit('merges', () => {\n  const merge = (a: any, b: any) => ({ ...b, ...a });\n  expect(merge({ a: 1 }, { b: 2 })).toEqual({ a: 1, b: 2 });\n});\n",
    )
}

#[test]
fn direct_owner_relation_is_not_capped_by_entry_reach() -> Result<(), String> {
    let exported = MERGE.replacen("function _merge", "export function _merge", 1);
    let finding = merge_finding(
        "direct-and-entry",
        &exported,
        "import { it, expect } from 'vitest';\nimport { _merge, merge } from '../src/merge';\n\nit('merges', () => {\n  expect(_merge({ a: 1 }, { b: 2 })).toEqual({ a: 1, b: 2 });\n  expect(merge({ a: 1 }, { b: 2 })).toEqual({ a: 1, b: 2 });\n});\n",
    )?;
    assert!(
        finding.related_tests.iter().any(
            |test| test.relation_reason == Some(crate::domain::RelationReason::DirectOwnerCall)
        ),
        "the direct call leads: {:?}",
        finding.related_tests
    );
    assert!(
        !finding
            .missing
            .iter()
            .any(|line| line.contains("only through same-module callers")),
        "a direct relation is not reported as entry-only reach: {:?}",
        finding.missing
    );
    Ok(())
}

#[test]
fn entries_follow_factory_products_wrappers_and_export_lists() {
    // `createMerge` only returns the closure that calls `_merge`; calling the
    // factory itself does not run it (Codex review on #4526).
    assert_eq!(entry_names(MERGE, "_merge"), ["default", "merge"]);
    let listed = "function _merge(a: any): any {\n  return a;\n}\nconst combine = (a: any) => _merge(a);\nexport { combine as merge };\n";
    assert_eq!(entry_names(listed, "_merge"), ["merge"]);
    let curried = "function _merge(a: any): any {\n  return a;\n}\nconst make = () => (a: any) => _merge(a);\nexport const merge = make();\n";
    assert_eq!(entry_names(curried, "_merge"), ["merge"]);
}

#[test]
fn entries_are_not_invented_for_rebound_names() {
    // A parameter of the same name binds the call, not the top-level helper.
    let parameter = "function _merge(a: any): any {\n  return a;\n}\nexport function merge(_merge: (a: any) => any, a: any): any {\n  return _merge(a);\n}\n";
    assert!(entry_names(parameter, "_merge").is_empty());
    // So does a body-local declaration.
    let local = "function _merge(a: any): any {\n  return a;\n}\nexport function merge(a: any): any {\n  const _merge = (b: any) => b;\n  return _merge(a);\n}\n";
    assert!(entry_names(local, "_merge").is_empty());
    // A destructured parameter binds it too.
    let destructured = "function _merge(a: any): any {\n  return a;\n}\nexport function merge({ _merge }: any, a: any): any {\n  return _merge(a);\n}\n";
    assert!(entry_names(destructured, "_merge").is_empty());
    let array = "function _merge(a: any): any {\n  return a;\n}\nexport function merge([_merge]: any[], a: any): any {\n  return _merge(a);\n}\n";
    assert!(entry_names(array, "_merge").is_empty());
    // So does a nested callback's parameter.
    let callback = "function _merge(a: any): any {\n  return a;\n}\nexport function merge(fns: any[], a: any): any {\n  return fns.map((_merge) => _merge(a));\n}\n";
    assert!(entry_names(callback, "_merge").is_empty());
    // A factory parameter captured by the returned closure is not the helper.
    let captured = "function _merge(a: any): any {\n  return a;\n}\nfunction build(_merge: (a: any) => any) {\n  return (a: any) => _merge(a);\n}\nexport const merge = build((a: any) => a);\n";
    assert!(entry_names(captured, "_merge").is_empty());
}

#[test]
fn anonymous_default_exports_are_entries() {
    let function = "function _merge(a: any): any {\n  return a;\n}\nexport default function (a: any): any {\n  return _merge(a);\n}\n";
    assert_eq!(entry_names(function, "_merge"), ["default"]);
    let arrow =
        "function _merge(a: any): any {\n  return a;\n}\nexport default (a: any) => _merge(a);\n";
    assert_eq!(entry_names(arrow, "_merge"), ["default"]);
}

#[test]
fn test_calling_only_the_factory_does_not_relate() -> Result<(), String> {
    assert_not_entry_related(
        "factory-only",
        MERGE,
        "import { it, expect } from 'vitest';\nimport { createMerge } from '../src/merge';\n\nit('builds', () => {\n  expect(typeof createMerge()).toBe('function');\n});\n",
    )
}

#[test]
fn anonymous_default_wrapper_relates_through_default_import() -> Result<(), String> {
    let wrapper = "function _merge(base: any, defaults: any): any {\n  const out = { ...defaults };\n  for (const key of Object.keys(base)) {\n    out[key] = base[key];\n  }\n  return out;\n}\n\nexport default function (base: any, defaults: any): any {\n  return _merge(base, defaults);\n}\n";
    assert_entry_related(
        "anonymous-default",
        wrapper,
        "import { it, expect } from 'vitest';\nimport combine from '../src/merge';\n\nit('merges', () => {\n  expect(combine({ a: 1 }, { b: 2 })).toEqual({ a: 1, b: 2 });\n});\n",
    )
}

#[test]
fn factory_product_reaches_only_what_the_returned_function_calls() {
    // `build` calls `_merge` once while building, then returns a function
    // that never calls it: calling the product does not run `_merge`.
    let eager = "function _merge(a: any): any {\n  return a;\n}\nfunction build() {\n  _merge({});\n  return (a: any) => a;\n}\nexport const merge = build();\n";
    assert!(entry_names(eager, "_merge").is_empty());
    // A product of an imported factory has no same-module body to follow.
    let imported = "import { build } from './factory';\nfunction _merge(a: any): any {\n  return a;\n}\nexport const merge = build(_merge);\n";
    assert!(entry_names(imported, "_merge").is_empty());
}

#[test]
fn entries_stop_at_the_hop_bound_and_ignore_type_and_foreign_exports() {
    let three_hops = "function _merge(a: any): any {\n  return a;\n}\nfunction one(a: any) {\n  return _merge(a);\n}\nfunction two(a: any) {\n  return one(a);\n}\nexport function three(a: any) {\n  return two(a);\n}\n";
    assert_eq!(entry_names(three_hops, "_merge"), ["three"]);
    let four_hops = three_hops.replace("export function three", "function three")
        + "export function four(a: any) {\n  return three(a);\n}\n";
    assert!(entry_names(&four_hops, "_merge").is_empty());
    let foreign = "function _merge(a: any): any {\n  return a;\n}\nexport type { Merge } from './types';\nexport { merge } from './other';\n";
    assert!(entry_names(foreign, "_merge").is_empty());
}

#[test]
fn entry_tests_are_not_admitted_beside_a_direct_owner_test() -> Result<(), String> {
    // The direct test's weak oracle decides; the entry test's exact-value
    // assertion must not lend it strength.
    let exported = MERGE.replacen("function _merge", "export function _merge", 1);
    let finding = merge_finding(
        "direct-weak-entry-strong",
        &exported,
        "import { it, expect } from 'vitest';\nimport { _merge, merge } from '../src/merge';\n\nit('runs', () => {\n  expect(_merge({ a: 1 }, { b: 2 })).toBeDefined();\n});\n\nit('merges', () => {\n  expect(merge({ a: 1 }, { b: 2 })).toEqual({ a: 1, b: 2 });\n});\n",
    )?;
    assert!(
        !relates_through_entry(&finding),
        "related: {:?}",
        finding.related_tests
    );
    assert!(
        finding
            .related_tests
            .iter()
            .all(|test| test.name.contains("runs")),
        "only the direct test relates: {:?}",
        finding.related_tests
    );
    assert_eq!(finding.class, ExposureClass::WeaklyExposed);
    Ok(())
}
