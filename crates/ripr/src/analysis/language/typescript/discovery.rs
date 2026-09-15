//! Workspace discovery for the TypeScript preview adapter.

use super::*;

const TEST_FILE_STEM_SUFFIXES: &[&str] = &[".test", "-test", "_test", ".spec"];
const TEST_DIRECTORY_NAMES: &[&str] = &["test", "tests", "__tests__"];
const CYPRESS_SOURCE_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx"];
const JASMINE_SPEC_DIRECTORY_NAMES: &[&str] = &["spec"];

/// Whether a path is a test file by convention.
///
/// The adapter recognizes four bounded convention families:
///
/// 1. Jest/Vitest-style `*.test.*` and `*.spec.*` files across every
///    TypeScript/JavaScript source extension routed to this adapter.
/// 2. Node-style names: `test.*`, `test-*`, `*-test.*`, and `*_test.*`.
/// 3. Cypress `*.cy.{js,jsx,ts,tsx}` files.
/// 4. Source files under exact `test`, `tests`, or `__tests__` directory
///    components, plus Jasmine-style `spec/**/[sS]pec.*` paths.
///
/// Directory matching is component-based, not substring-based, so
/// `src/latest/foo.ts`, `test-utils/foo.ts`, and `src/contest.ts` remain
/// production paths. The language router is checked before any naming or
/// directory convention, preventing discovery policy from drifting beyond the
/// adapter's real source surface. Test extraction remains fail-closed: a
/// recognized path contributes test evidence only when parsing finds supported
/// `test()` / `it()` / `describe()` call shapes.
///
/// The exact test-directory rule retains the controlled ky dogfood case:
/// `test/body-size.ts` directly exercised a changed owner but was invisible
/// before directory classification, producing a false `no_static_path`.
pub(crate) fn is_test_file(path: &Path) -> bool {
    is_typescript_or_javascript_source(path)
        && (has_test_file_stem(path) || has_test_directory_component(path))
}

fn is_typescript_or_javascript_source(path: &Path) -> bool {
    let adapter = TypeScriptAdapter;
    adapter.accepts_path(path)
}

fn has_extension_in(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extensions.contains(&extension))
}

fn has_test_file_stem(path: &Path) -> bool {
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };

    stem == "test"
        || stem
            .strip_prefix("test-")
            .is_some_and(|remainder| !remainder.is_empty())
        || TEST_FILE_STEM_SUFFIXES
            .iter()
            .any(|suffix| stem.ends_with(suffix))
        || (stem.ends_with(".cy") && has_extension_in(path, CYPRESS_SOURCE_EXTENSIONS))
        || has_jasmine_spec_stem(path, stem)
}

fn has_jasmine_spec_stem(path: &Path, stem: &str) -> bool {
    (stem.ends_with("Spec") || stem.ends_with("spec"))
        && has_directory_component(path, JASMINE_SPEC_DIRECTORY_NAMES)
}

fn has_test_directory_component(path: &Path) -> bool {
    has_directory_component(path, TEST_DIRECTORY_NAMES)
}

fn has_directory_component(path: &Path, names: &[&str]) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|name| names.contains(&name))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_names_cover_supported_runner_conventions() {
        for path in [
            "src/cart.test.ts",
            "src/cart.spec.tsx",
            "src/cart-test.js",
            "src/cart_test.jsx",
            "src/test-cart.ts",
            "src/test.tsx",
            "cypress/e2e/checkout.cy.ts",
            "src/Button.cy.tsx",
        ] {
            assert!(is_test_file(Path::new(path)), "expected test path: {path}");
        }
    }

    #[test]
    fn test_directories_cover_feature_named_and_jasmine_test_files() {
        for path in [
            "test/body-size.ts",
            "tests/utils.ts",
            "src/__tests__/Header.tsx",
            "packages/core/test/index.js",
            "spec/requestContractSpec.js",
            "spec/request_contractspec.js",
        ] {
            assert!(is_test_file(Path::new(path)), "expected test path: {path}");
        }
    }

    #[test]
    fn test_layout_matching_stays_component_route_and_extension_bounded() {
        for path in [
            "src/latest/feature.ts",
            "test-utils/helper.ts",
            "src/contest.ts",
            "src/cart_test.txt",
            "src/cart.test.mts",
            "test/cart.mjs",
            "spec/request_contract.md",
            "spec/request_contract.js",
            "spec/helpers/setup.js",
            "src/requestContractSpec.js",
            "src/cart.cy.mjs",
            "src/cypress.ts",
            "src/specification.ts",
        ] {
            assert!(
                !is_test_file(Path::new(path)),
                "unexpected test path: {path}"
            );
        }
    }

    #[test]
    fn newly_recognized_layouts_reach_supported_test_call_shapes() {
        let cases = [
            (
                "src/cart_test.ts",
                r#"test("node layout", () => { expect(cart()).toBe(1); });"#,
            ),
            (
                "cypress/e2e/cart.cy.ts",
                r#"describe("cart", () => { it("checks out", () => { expect(cart()).toBe(1); }); });"#,
            ),
            (
                "spec/cartContractSpec.js",
                r#"describe("cart", () => { it("keeps its contract", () => { expect(cart()).toBe(1); }); });"#,
            ),
        ];

        for (path, source) in cases {
            let path = Path::new(path);
            let display = path.display();
            assert!(is_test_file(path), "expected test path: {display}");
            assert_eq!(
                extract_tests(path, source).len(),
                1,
                "expected one extracted test for {display}"
            );
        }
    }
}
