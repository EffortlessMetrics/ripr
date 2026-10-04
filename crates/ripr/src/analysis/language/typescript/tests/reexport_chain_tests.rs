//! Bounded barrel re-export chains (RIPR-SPEC-0095). A test that imports a
//! changed owner through a barrel — a directory specifier resolving to its
//! `index` module, `export * from` or `export { N } from` hops, a few hops
//! deep — reaches the owner and is credited. A name the chain does not
//! actually export to the owner (another name, a non-exported owner, an
//! ambiguous star, an over-deep or cyclic chain, a file module that shadows
//! the directory index) stays uncredited.
//!
//! Every case builds the index through `ReExportIndex::build` over real
//! sources and relates through `find_related_tests`, the production path.

use super::*;
use crate::domain::RelationReason;

/// Build the index from `(path, source)` pairs and return the related tests
/// of the owner `owner_name` declared in `owner_file`, for the single test
/// parsed from `test_source` in `tests/barrel.test.ts`.
fn related_through_barrel(
    files: &[(&str, &str)],
    owner_file: &str,
    owner_name: &str,
    test_source: &str,
) -> Result<Vec<RelatedTest>, String> {
    let test_file = Path::new("tests/barrel.test.ts");
    let mut workspace_files: Vec<PathBuf> = files.iter().map(|(p, _)| PathBuf::from(p)).collect();
    workspace_files.push(test_file.to_path_buf());
    let mut sources = std::collections::HashMap::new();
    for (path, source) in files {
        sources.insert(PathBuf::from(path), (*source).to_string());
    }
    sources.insert(test_file.to_path_buf(), test_source.to_string());
    let index = ReExportIndex::build(
        &workspace_files,
        &sources,
        Path::new(""),
        None,
        |path: &Path| path.starts_with("tests"),
    );
    let owner_source = files
        .iter()
        .find(|(path, _)| *path == owner_file)
        .map(|(_, source)| *source)
        .ok_or("owner file missing from the fixture")?;
    let owner = extract_owners(Path::new(owner_file), owner_source)
        .into_iter()
        .find(|owner| owner.name == owner_name)
        .ok_or_else(|| format!("owner `{owner_name}` was not extracted"))?;
    let tests = extract_tests(test_file, test_source);
    if tests.len() != 1 {
        return Err(format!(
            "expected exactly one parsed test, got {}",
            tests.len()
        ));
    }
    Ok(find_related_tests(&owner, &tests, None, &index, None))
}

fn assert_credited(related: &[RelatedTest], label: &str) {
    assert_eq!(
        related.len(),
        1,
        "{label}: the barrel test must be credited"
    );
    assert_eq!(
        related.first().and_then(|test| test.relation_reason),
        Some(RelationReason::ReExportChainFollowed),
        "{label}: credited through the re-export chain"
    );
}

const UTILS: &str = "export function withBase(input: string, base: string): string {\n  return base + input;\n}\n\nexport function withoutBase(input: string, base: string): string {\n  return input.slice(base.length);\n}\n";
const URL: &str = "export function parseURL(input: string): string {\n  return input;\n}\n";

/// The unjs/ufo import shape (eb29945): a directory import of a star
/// barrel, called inside `describe`. (The upstream test also registers its
/// cases from a `for` loop with computed titles, which test extraction does
/// not index; that separate extraction gap is out of this module's scope.)
const UFO_TEST: &str = r#"import { describe, expect, test } from "vitest";
import { withBase, withoutBase } from "../src";

describe("base helpers", () => {
  test("strips the base", () => {
    const t = { base: "/foo", input: "/foo/bar", out: "/bar" };
    expect(withoutBase(t.input, t.base)).toBe(t.out);
  });
});
"#;

#[test]
fn directory_star_barrel_import_credits_the_changed_owner() -> Result<(), String> {
    let files = [
        ("src/utils.ts", UTILS),
        ("src/url.ts", URL),
        (
            "src/index.ts",
            "export * from \"./url\";\nexport * from \"./utils\";\n",
        ),
    ];
    let related = related_through_barrel(&files, "src/utils.ts", "withoutBase", UFO_TEST)?;
    assert_credited(&related, "ufo directory star barrel");
    Ok(())
}

#[test]
fn directory_named_barrel_import_credits_the_changed_owner() -> Result<(), String> {
    let files = [
        ("src/utils.ts", UTILS),
        (
            "src/index.ts",
            "export { withBase, withoutBase } from \"./utils\";\n",
        ),
    ];
    let related = related_through_barrel(&files, "src/utils.ts", "withoutBase", UFO_TEST)?;
    assert_credited(&related, "directory named barrel");
    Ok(())
}

#[test]
fn directory_import_of_owner_index_module_credits_the_owner() -> Result<(), String> {
    let files = [("src/utils/index.ts", UTILS)];
    let test = "import { withoutBase } from \"../src/utils\";\ntest(\"strips\", () => {\n  expect(withoutBase(\"/foo/bar\", \"/foo\")).toBe(\"/bar\");\n});\n";
    let related = related_through_barrel(&files, "src/utils/index.ts", "withoutBase", test)?;
    assert_credited(&related, "directory import of the owner's index module");
    Ok(())
}

#[test]
fn two_hop_named_and_star_chain_credits_the_owner() -> Result<(), String> {
    let files = [
        ("src/utils.ts", UTILS),
        ("src/base/index.ts", "export * from \"../utils\";\n"),
        ("src/index.ts", "export { withoutBase } from \"./base\";\n"),
    ];
    let related = related_through_barrel(&files, "src/utils.ts", "withoutBase", UFO_TEST)?;
    assert_credited(&related, "named hop then star hop");
    Ok(())
}

#[test]
fn barrel_import_of_only_another_name_does_not_credit() -> Result<(), String> {
    const A: &str = "export function a(x: number): number {\n  return x + 1;\n}\n";
    const B: &str = "export function b(x: number): number {\n  return x * 2;\n}\n";
    let test =
        "import { b } from \"../src\";\ntest(\"doubles\", () => {\n  expect(b(2)).toBe(4);\n});\n";
    for barrel in [
        "export { a } from \"./a\";\nexport { b } from \"./b\";\n",
        "export * from \"./a\";\nexport * from \"./b\";\n",
    ] {
        let files = [("src/a.ts", A), ("src/b.ts", B), ("src/index.ts", barrel)];
        // Positive control: the same barrel does credit `b` for the test.
        let control = related_through_barrel(&files, "src/b.ts", "b", test)?;
        assert_credited(&control, barrel);
        let related = related_through_barrel(&files, "src/a.ts", "a", test)?;
        assert!(
            related.is_empty(),
            "importing only `b` must not credit the changed `a` ({barrel}): {related:?}"
        );
    }
    Ok(())
}

#[test]
fn star_hop_to_a_module_that_does_not_export_the_owner_does_not_credit() -> Result<(), String> {
    // `hidden` is a module-private function: `export *` cannot forward it.
    let files = [
        (
            "src/a.ts",
            "function hidden(x: number): number {\n  return x + 1;\n}\nexport function shown(x: number): number {\n  return hidden(x);\n}\n",
        ),
        ("src/index.ts", "export * from \"./a\";\n"),
    ];
    // The explicit `../src/index` specifier keeps the star hop itself in
    // reach, so only the guard under test can refuse the credit.
    let test = "import { hidden } from \"../src/index\";\ntest(\"hidden\", () => {\n  expect(hidden(1)).toBe(2);\n});\n";
    let related = related_through_barrel(&files, "src/a.ts", "hidden", test)?;
    assert!(related.is_empty(), "a non-exported owner: {related:?}");
    Ok(())
}

#[test]
fn ambiguous_star_export_does_not_credit() -> Result<(), String> {
    const DUP: &str = "export function dup(x: number): number {\n  return x + 1;\n}\n";
    let files = [
        ("src/a.ts", DUP),
        ("src/b.ts", DUP),
        (
            "src/index.ts",
            "export * from \"./a\";\nexport * from \"./b\";\n",
        ),
    ];
    // The explicit `../src/index` specifier keeps the star hop itself in
    // reach, so only the guard under test can refuse the credit.
    let test = "import { dup } from \"../src/index\";\ntest(\"dup\", () => {\n  expect(dup(1)).toBe(2);\n});\n";
    let related = related_through_barrel(&files, "src/a.ts", "dup", test)?;
    assert!(related.is_empty(), "an ambiguous star name: {related:?}");
    Ok(())
}

#[test]
fn chain_beyond_the_hop_bound_does_not_credit() -> Result<(), String> {
    // `src/h{n}.ts` re-exports from `src/h{n-1}.ts`; `src/h0.ts` is the
    // owner module. The test imports from `src/h{depth}`.
    const OWNER: &str = "export function deep(x: number): number {\n  return x + 1;\n}\n";
    let test_for = |depth: usize| {
        format!(
            "import {{ deep }} from \"../src/h{depth}\";\ntest(\"deep\", () => {{\n  expect(deep(1)).toBe(2);\n}});\n"
        )
    };
    let hops: Vec<(String, String)> = (1..=MAX_REEXPORT_HOPS + 1)
        .map(|n| {
            (
                format!("src/h{n}.ts"),
                format!("export {{ deep }} from \"./h{}\";\n", n - 1),
            )
        })
        .collect();
    let mut files: Vec<(&str, &str)> = vec![("src/h0.ts", OWNER)];
    files.extend(
        hops.iter()
            .map(|(path, source)| (path.as_str(), source.as_str())),
    );

    let within = related_through_barrel(&files, "src/h0.ts", "deep", &test_for(MAX_REEXPORT_HOPS))?;
    assert_credited(&within, "chain at the hop bound");
    let beyond = related_through_barrel(
        &files,
        "src/h0.ts",
        "deep",
        &test_for(MAX_REEXPORT_HOPS + 1),
    )?;
    assert!(
        beyond.is_empty(),
        "a chain beyond the hop bound: {beyond:?}"
    );
    Ok(())
}

#[test]
fn star_cycle_terminates_without_credit() -> Result<(), String> {
    let files = [
        ("src/a.ts", "export * from \"./b\";\n"),
        ("src/b.ts", "export * from \"./a\";\n"),
        ("src/owner.ts", UTILS),
    ];
    let test = "import { withoutBase } from \"../src/a\";\ntest(\"cycle\", () => {\n  expect(withoutBase(\"/a/b\", \"/a\")).toBe(\"/b\");\n});\n";
    let related = related_through_barrel(&files, "src/owner.ts", "withoutBase", test)?;
    assert!(related.is_empty(), "a star cycle: {related:?}");
    Ok(())
}

#[test]
fn file_module_shadows_directory_index_barrel() -> Result<(), String> {
    // `../src` names `src.ts` (file module first), not `src/index.ts`.
    let files = [
        ("src/utils.ts", UTILS),
        ("src/index.ts", "export * from \"./utils\";\n"),
        ("src.ts", URL),
    ];
    let related = related_through_barrel(&files, "src/utils.ts", "withoutBase", UFO_TEST)?;
    assert!(
        related.is_empty(),
        "file module wins over the index: {related:?}"
    );
    Ok(())
}

#[test]
fn shadowed_barrel_import_does_not_credit() -> Result<(), String> {
    let files = [
        ("src/utils.ts", UTILS),
        ("src/index.ts", "export * from \"./utils\";\n"),
    ];
    // The explicit `../src/index` specifier keeps the star hop itself in
    // reach, so only the guard under test can refuse the credit.
    let test = "import { withoutBase } from \"../src/index\";\ntest(\"shadow\", () => {\n  const withoutBase = (a: string, b: string) => a;\n  expect(withoutBase(\"/a/b\", \"/a\")).toBe(\"/a/b\");\n});\n";
    let related = related_through_barrel(&files, "src/utils.ts", "withoutBase", test)?;
    assert!(related.is_empty(), "a body-local shadow: {related:?}");
    Ok(())
}

#[test]
fn describe_scoped_shadow_of_barrel_import_does_not_credit() -> Result<(), String> {
    let files = [
        ("src/utils.ts", UTILS),
        ("src/index.ts", "export * from \"./utils\";\n"),
    ];
    // The shadow lives in the enclosing `describe`, not the test body, so
    // only the scope-binding guard can refuse the credit.
    let test = "import { withoutBase } from \"../src/index\";\ndescribe(\"s\", () => {\n  const withoutBase = (a: string, b: string) => a;\n  test(\"shadow\", () => {\n    expect(withoutBase(\"/a/b\", \"/a\")).toBe(\"/a/b\");\n  });\n});\n";
    let related = related_through_barrel(&files, "src/utils.ts", "withoutBase", test)?;
    assert!(related.is_empty(), "a describe-scoped shadow: {related:?}");
    Ok(())
}

const TWO_HOP_FILES: [(&str, &str); 3] = [
    ("src/utils.ts", UTILS),
    ("src/base/index.ts", "export * from \"../utils\";\n"),
    ("src/index.ts", "export { withoutBase } from \"./base\";\n"),
];

fn mocked_barrel_test(mock_specifier: &str) -> String {
    format!(
        "import {{ vi, expect, test }} from \"vitest\";\nimport {{ withoutBase }} from \"../src\";\nvi.mock(\"{mock_specifier}\", () => ({{ withoutBase: vi.fn(() => \"/b\") }}));\ntest(\"mocked\", () => {{\n  expect(withoutBase(\"/a/b\", \"/a\")).toBe(\"/b\");\n}});\n"
    )
}

#[test]
fn mocked_import_barrel_does_not_credit() -> Result<(), String> {
    let test = mocked_barrel_test("../src");
    let related = related_through_barrel(&TWO_HOP_FILES, "src/utils.ts", "withoutBase", &test)?;
    assert!(
        related.is_empty(),
        "the imported barrel is mocked: {related:?}"
    );
    Ok(())
}

#[test]
fn mocked_intermediate_barrel_does_not_credit() -> Result<(), String> {
    let test = mocked_barrel_test("../src/base");
    let related = related_through_barrel(&TWO_HOP_FILES, "src/utils.ts", "withoutBase", &test)?;
    assert!(
        related.is_empty(),
        "a hop on the chain is mocked: {related:?}"
    );
    Ok(())
}

#[test]
fn mock_of_a_module_off_the_chain_keeps_the_credit() -> Result<(), String> {
    let test = mocked_barrel_test("../src/url");
    let related = related_through_barrel(&TWO_HOP_FILES, "src/utils.ts", "withoutBase", &test)?;
    assert_credited(&related, "an unrelated mock");
    Ok(())
}
