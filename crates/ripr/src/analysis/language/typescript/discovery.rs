//! Workspace discovery for the TypeScript preview adapter.

use super::*;

const TEST_FILE_STEM_SUFFIXES: &[&str] = &[".test", "-test", "_test", ".spec"];
const TEST_DIRECTORY_NAMES: &[&str] = &["test", "tests", "__tests__"];
// Every source extension routed to this adapter (router.rs), so Cypress
// `*.cy.<ext>` classification stays bounded to real adapter source surface.
const CYPRESS_SOURCE_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs"];
const JASMINE_SPEC_DIRECTORY_NAMES: &[&str] = &["spec"];

/// Whether a path is a test file by convention.
///
/// The adapter recognizes four bounded convention families:
///
/// 1. Jest/Vitest-style `*.test.*` and `*.spec.*` files across every
///    TypeScript/JavaScript source extension routed to this adapter
///    (.ts/.tsx/.js/.jsx/.mts/.cts/.mjs/.cjs).
/// 2. Node-style names: `test.*`, `test-*`, `*-test.*`, and `*_test.*`.
/// 3. Cypress `*.cy.{ts,tsx,js,jsx,mts,cts,mjs,cjs}` files.
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

// Directory prune authority for the workspace walk: a single table owned by
// `config::typescript` (`TYPESCRIPT_EXCLUDED_DIRS`), shared with the diff
// loop's pre-count refusal (#3743). The walk must prune exactly the trees
// the diff loop refuses, otherwise the denominator counts files no facts
// can back, or facts exist for files the denominator hides.
// `*.generated.*` files are additionally skipped at the file level below.

/// Env override for [`DEFAULT_TS_MAX_WORKSPACE_FILES`].
pub(crate) const TS_MAX_WORKSPACE_FILES_ENV: &str = "RIPR_TS_MAX_WORKSPACE_FILES";
/// Default ceiling on directory entries visited during workspace discovery.
const DEFAULT_TS_MAX_WORKSPACE_FILES: usize = 20_000;

/// Parse a positive workspace file-count limit, failing closed to the error
/// string on invalid input (mirrors `rust.rs::positive_limit_from_env`).
pub(crate) fn ts_workspace_file_limit_from_env(
    value: Result<String, std::env::VarError>,
) -> Result<usize, String> {
    match value {
        Ok(raw) => {
            let parsed = raw.trim().parse::<usize>().map_err(|err| {
                format!("{TS_MAX_WORKSPACE_FILES_ENV} must be a positive integer: {err}")
            })?;
            if parsed == 0 {
                return Err(format!(
                    "{TS_MAX_WORKSPACE_FILES_ENV} must be a positive integer"
                ));
            }
            Ok(parsed)
        }
        Err(std::env::VarError::NotPresent) => Ok(DEFAULT_TS_MAX_WORKSPACE_FILES),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!(
            "{TS_MAX_WORKSPACE_FILES_ENV} must be a positive integer"
        )),
    }
}

/// Resolved max-visited-files limit, failing closed to the default so a
/// malformed operator override cannot abort analysis.
pub(crate) fn ts_workspace_file_limit() -> usize {
    ts_workspace_file_limit_from_env(std::env::var(TS_MAX_WORKSPACE_FILES_ENV))
        .unwrap_or(DEFAULT_TS_MAX_WORKSPACE_FILES)
}

/// Workspace file discovery result: the accepted source files plus whether
/// the max-visited-files cap tripped (making `files` a partial set), and how
/// many symlink/junction entries were seen but deliberately not followed
/// (#4104-D: the count feeds a disclosure so link-hidden tests are not
/// silently invisible).
pub(crate) struct WorkspaceScan {
    pub(crate) files: Vec<PathBuf>,
    pub(crate) truncated: bool,
    pub(crate) skipped_links: usize,
}

pub(crate) fn collect_workspace_typescript_files(root: &Path) -> WorkspaceScan {
    let max_entries = ts_workspace_file_limit_from_env(std::env::var(TS_MAX_WORKSPACE_FILES_ENV))
        .unwrap_or(DEFAULT_TS_MAX_WORKSPACE_FILES);
    visit_workspace(root, max_entries)
}

/// Iterative workspace walk over an explicit work stack.
///
/// Recursion was replaced by a loop so pathologically deep directory trees
/// cannot overflow the call stack (a hard abort). Every visited entry counts
/// against `max_entries`; exceeding the cap stops the walk and reports
/// `truncated: true` so the adapter can disclose the bound instead of
/// silently analyzing a partial workspace.
pub(crate) fn visit_workspace(root: &Path, max_entries: usize) -> WorkspaceScan {
    let mut out = Vec::new();
    let mut visited = 0usize;
    let mut truncated = false;
    let mut skipped_links = 0usize;
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut stop = false;
        for entry in entries.flatten() {
            visited += 1;
            if visited > max_entries {
                truncated = true;
                stop = true;
                break;
            }
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if is_typescript_dir_pruned_from_discovery(name) {
                continue;
            }
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };
            if file_type.is_symlink() {
                // Symlinks and NTFS junctions are NOT followed (fail-closed:
                // following them risks cycles and outside-root reads), but
                // the skip is counted so the adapter can disclose that
                // link-hidden tests/sources were not seen (#4104-D).
                skipped_links += 1;
                continue;
            }
            if file_type.is_dir() {
                stack.push(path);
            } else if file_type.is_file() && !is_detectable_generated_typescript_path(&path) {
                let adapter = TypeScriptAdapter;
                if adapter.accepts_path(&path) {
                    let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                    out.push(relative);
                }
            } else if is_special_non_link_file(&file_type) {
                // FIFOs, sockets and devices (Unix) are neither directories,
                // regular files, nor links and cannot carry TypeScript
                // sources. Counting them as skipped links would disclose a
                // symlink/junction limitation a workspace without any link
                // does not have (review #4138): they stay unanalyzed without
                // claiming link-hidden coverage loss.
            } else {
                // Reparse points std does not report as symlinks (notably
                // NTFS junctions on some toolchains) land here: neither dir
                // nor file. They are equally unfollowable — count them in
                // the same disclosure instead of silently dropping the
                // entry (#4104-D).
                skipped_links += 1;
            }
        }
        if stop {
            stack.clear();
            break;
        }
    }
    out.sort();
    WorkspaceScan {
        files: out,
        truncated,
        skipped_links,
    }
}

/// `true` for directory entry types that are neither directories, regular
/// files, nor links (FIFOs, sockets, character/block devices on Unix). Such
/// entries cannot contain or stand in for TypeScript sources, so discovery
/// skips them without counting them toward the `skipped_links` disclosure.
#[cfg(unix)]
fn is_special_non_link_file(file_type: &std::fs::FileType) -> bool {
    use std::os::unix::fs::FileTypeExt;
    file_type.is_fifo()
        || file_type.is_socket()
        || file_type.is_char_device()
        || file_type.is_block_device()
}

/// Non-Unix platforms expose no FIFO/socket/device entry types through
/// `std::fs::read_dir`; every non-dir/non-file entry is a reparse point and
/// stays in the `skipped_links` disclosure.
#[cfg(not(unix))]
fn is_special_non_link_file(_file_type: &std::fs::FileType) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempWorkspace(PathBuf);

    impl TempWorkspace {
        fn new(label: &str) -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0);
            let root = std::env::temp_dir().join(format!(
                "ripr-ts-discovery-{label}-{}-{stamp}",
                std::process::id()
            ));
            let created = fs::create_dir_all(&root);
            assert!(
                created.is_ok(),
                "create temp workspace {}: {:?}",
                root.display(),
                created.err()
            );
            Self(root)
        }

        fn write(&self, relative: &str, source: &str) {
            let absolute = self.0.join(relative);
            let parent = absolute.parent().unwrap_or(Path::new("."));
            let created = fs::create_dir_all(parent);
            assert!(
                created.is_ok(),
                "create fixture dir {}: {:?}",
                parent.display(),
                created.err()
            );
            let written = fs::write(&absolute, source);
            assert!(
                written.is_ok(),
                "write fixture file {}: {:?}",
                absolute.display(),
                written.err()
            );
        }
    }

    impl Drop for TempWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

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
            // Modern ESM/CJS extensions ride the same conventions now that
            // the router accepts them.
            "src/cart.test.mts",
            "src/cart.spec.cts",
            "src/cart-test.mjs",
            "src/cart_test.cjs",
            "src/Button.cy.mts",
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
            // Routed extensions stay NON-test when no test marker is present;
            // the ESM/CJS variants below were previously unrouted entirely
            // and pinned here as "not test files" — they are still not test
            // files by name, they are simply analyzed as production sources.
            "src/cart.mts",
            "src/cart.cts",
            "src/cart.mjs",
            "src/cart.cjs",
            "spec/request_contract.md",
            "spec/request_contract.js",
            "spec/helpers/setup.js",
            "src/requestContractSpec.js",
            "src/cypress.ts",
            "src/specification.ts",
        ] {
            assert!(
                !is_test_file(Path::new(path)),
                "unexpected test path: {path}"
            );
        }
    }

    /// Regression pins for the previously unrouted ESM/CJS extension family:
    /// with `.mts`/`.cts`/`.mjs`/`.cjs` now routed to this adapter, the
    /// ordinary test conventions apply to them (these paths used to be pinned
    /// as NOT-test files when the router dropped the extensions).
    #[test]
    fn esm_cjs_extensions_follow_test_conventions() {
        for path in [
            "src/cart.test.mts",
            "src/cart.spec.mjs",
            "test/cart.mjs",
            "tests/cart.cts",
            "src/__tests__/cart.cjs",
            "src/cart.cy.mjs",
        ] {
            assert!(is_test_file(Path::new(path)), "expected test path: {path}");
        }
        for path in [
            "src/cart.mts",
            "src/cart.cts",
            "src/cart.mjs",
            "src/cart.cjs",
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

    #[test]
    fn nested_source_files_are_still_discovered() {
        let workspace = TempWorkspace::new("nested");
        workspace.write("src/main.ts", "export const a = 1;\n");
        workspace.write("packages/core/src/index.tsx", "export const b = 2;\n");
        workspace.write("packages/core/README.md", "not source\n");
        let scan = visit_workspace(&workspace.0, 10_000);
        assert!(!scan.truncated, "scan must not trip the cap");
        assert_eq!(
            scan.files,
            vec![
                PathBuf::from("packages/core/src/index.tsx"),
                PathBuf::from("src/main.ts"),
            ]
        );
    }

    #[test]
    fn generated_output_directories_are_not_discovered() {
        let workspace = TempWorkspace::new("generated-dirs");
        workspace.write("src/keep.ts", "export const kept = 1;\n");
        for generated in [
            "dist/bundle.ts",
            "build/output.ts",
            "out/tsc.ts",
            "coverage/lcov.ts",
            ".next/server.ts",
            ".cache/sw.ts",
            "vendor/lib.ts",
            "node_modules/pkg/index.ts",
            "target/debug/build.ts",
        ] {
            workspace.write(generated, "export const generated = 1;\n");
        }
        let scan = visit_workspace(&workspace.0, 10_000);
        assert!(!scan.truncated, "scan must not trip the cap");
        assert_eq!(
            scan.files,
            vec![PathBuf::from("src/keep.ts")],
            "generated/vendor dirs must be excluded; got {:?}",
            scan.files
        );
    }

    #[test]
    fn workspace_file_limit_trips_truncation_disclosure() {
        let workspace = TempWorkspace::new("file-limit");
        workspace.write("a/one.ts", "export const a = 1;\n");
        workspace.write("b/two.ts", "export const b = 2;\n");
        workspace.write("c/three.ts", "export const c = 3;\n");
        // Cap below the number of visited entries: each of the three
        // subdirectory entries plus the three files exceeds a limit of 4.
        let scan = visit_workspace(&workspace.0, 4);
        assert!(scan.truncated, "cap must trip the truncated flag");
        assert!(
            scan.files.len() < 3,
            "partial scan must not report the full set; got {:?}",
            scan.files
        );
        // Without a cap the same workspace scans clean.
        let full = visit_workspace(&workspace.0, 10_000);
        assert!(!full.truncated);
        assert_eq!(full.files.len(), 3);
    }

    #[test]
    fn workspace_file_limit_env_parsing_matches_repo_conventions() {
        assert_eq!(
            ts_workspace_file_limit_from_env(Err(std::env::VarError::NotPresent)),
            Ok(DEFAULT_TS_MAX_WORKSPACE_FILES)
        );
        assert_eq!(
            ts_workspace_file_limit_from_env(Ok(" 42 ".to_string())),
            Ok(42)
        );
        assert!(
            ts_workspace_file_limit_from_env(Ok("0".to_string())).is_err(),
            "zero limit must be rejected"
        );
        assert!(
            ts_workspace_file_limit_from_env(Ok("nope".to_string())).is_err(),
            "non-numeric limit must be rejected"
        );
    }

    /// Symlink/junction entries are not followed (#4104-D) but are COUNTED so
    /// the adapter can disclose the skip instead of leaving link-hidden tests
    /// silently invisible. On Windows, `symlink_dir` requires developer-mode
    /// or admin privileges; when the platform refuses, the assertion is
    /// skipped (Linux CI covers the deterministic path).
    #[test]
    fn discovery_counts_skipped_links_without_following() {
        let workspace = TempWorkspace::new("link-skip");
        workspace.write(
            "src/sum.ts",
            "export function sum(a: number, b: number): number {\n  return a + b;\n}\n",
        );
        workspace.write(
            "tests/sum.test.ts",
            "import { sum } from '../src/sum';\n\ntest('adds', () => {\n  expect(sum(2, 3)).toBe(5);\n});\n",
        );
        // A real directory holding a linked test file...
        workspace.write(
            "real_tests/linked.test.ts",
            "import { sum } from '../src/sum';\n\ntest('linked adds', () => {\n  expect(sum(4, 1)).toBe(5);\n});\n",
        );
        // ...and a link pointing at it.
        #[cfg(unix)]
        let linked = {
            use std::os::unix::fs::symlink;
            let target = workspace.0.join("real_tests");
            let link = workspace.0.join("tests_linked");
            if symlink(&target, &link).is_err() {
                return; // platform refused; nothing to assert here
            }
            link
        };
        #[cfg(windows)]
        let linked = {
            use std::os::windows::fs::symlink_dir;
            let target = workspace.0.join("real_tests");
            let link = workspace.0.join("tests_linked");
            if symlink_dir(&target, &link).is_err() {
                return; // privilege missing; nothing to assert here
            }
            link
        };
        #[cfg(not(any(unix, windows)))]
        let linked: std::path::PathBuf = match () {
            _ => return,
        };

        let scan = visit_workspace(&workspace.0, DEFAULT_TS_MAX_WORKSPACE_FILES);
        assert_eq!(scan.skipped_links, 1, "the one link must be counted");
        assert!(
            !scan
                .files
                .iter()
                .any(|file| file.starts_with("tests_linked")),
            "linked files must not be followed into the index: {:?}",
            scan.files
        );
        assert!(
            scan.files
                .iter()
                .any(|file| file.ends_with("tests/sum.test.ts")),
            "the real test file must still be indexed: {:?}",
            scan.files
        );
        let _ = linked;
    }

    /// A Unix socket is neither dir, file, nor link, so before the
    /// special-file exclusion it incremented `skipped_links` — a workspace
    /// with no link at all would have disclosed a link limitation
    /// (review #4138). The socket must be excluded without counting, and
    /// real sources stay indexed.
    #[cfg(unix)]
    #[test]
    fn discovery_does_not_count_sockets_as_skipped_links() {
        let workspace = TempWorkspace::new("socket-skip");
        workspace.write(
            "src/sum.ts",
            "export function sum(a: number, b: number): number {\n  return a + b;\n}\n",
        );
        let socket_path = workspace.0.join("agent.sock");
        let listener = match std::os::unix::net::UnixListener::bind(&socket_path) {
            Ok(listener) => listener,
            Err(_) => return, // platform refused; nothing to assert here
        };

        let scan = visit_workspace(&workspace.0, DEFAULT_TS_MAX_WORKSPACE_FILES);
        assert_eq!(
            scan.skipped_links, 0,
            "a socket is not a link and must not disclose a link limitation"
        );
        assert!(
            scan.files.iter().any(|file| file.ends_with("src/sum.ts")),
            "the real source must still be indexed: {:?}",
            scan.files
        );
        drop(listener);
    }

    #[test]
    fn workspace_walk_skips_excluded_and_generated_files_but_keeps_near_misses() {
        let workspace = TempWorkspace::new("excluded-near-miss");
        for rel in [
            "src/ok.ts",
            "src/generated.ts",
            "src/regenerated.js",
            "src/build.ts",
            "node_modules/left-pad/index.js",
            "dist/bundle.js",
            "build/out.ts",
            "out/tsc.ts",
            "coverage/lcov.js",
            ".next/server.ts",
            ".cache/sw.js",
            "vendor/lib.ts",
            "src/__generated__/types.ts",
            "src/cart.generated.ts",
            "src/types.generated.d.ts",
        ] {
            workspace.write(rel, "export const value = 1;\n");
        }
        let scan = visit_workspace(&workspace.0, 10_000);
        assert!(!scan.truncated, "scan must not trip the cap");
        for expected in [
            "src/ok.ts",
            "src/generated.ts",
            "src/regenerated.js",
            "src/build.ts",
        ] {
            assert!(
                scan.files.iter().any(|path| path == Path::new(expected)),
                "expected near-miss {expected} in {:?}",
                scan.files
            );
        }
        for excluded in [
            "node_modules/left-pad/index.js",
            "dist/bundle.js",
            "build/out.ts",
            "out/tsc.ts",
            "coverage/lcov.js",
            ".next/server.ts",
            ".cache/sw.js",
            "vendor/lib.ts",
            "src/__generated__/types.ts",
            "src/cart.generated.ts",
            "src/types.generated.d.ts",
        ] {
            assert!(
                !scan.files.iter().any(|path| path == Path::new(excluded)),
                "did not expect {excluded} in {:?}",
                scan.files
            );
        }
    }
}
