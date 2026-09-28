//! TypeScript/JavaScript path authorities for the preview adapter (#3743).
//!
//! One table owns the directories the diff loop must not count and the
//! workspace walk must not index. Python splits those decisions because repo
//! discovery keeps walking `vendor` so those files can be counted as an
//! excluded role. TypeScript preview has no excluded-role ledger and only the
//! diff-mode fact index, so the walk prunes every directory the diff loop
//! refuses, including `vendor`. A vendored, built, or generated tree therefore
//! contributes neither findings nor denominator counts.

use std::path::Path;

/// Directory components excluded from TypeScript/JavaScript diff analysis
/// and from the preview workspace walk (#3743):
///
/// - repository and tooling state: `.git`, `target`, `node_modules`,
///   `.ripr`, `.direnv`;
/// - build and coverage output: `dist`, `build`, `out`, `coverage`;
/// - toolchain caches and generated sites: `.next`, `.cache`;
/// - vendored third-party trees: `vendor`;
/// - generated-output directories: `__generated__`.
///
/// Matching is an exact component comparison, not a substring, so
/// `src/build.ts` and `src/vendor.ts` stay ordinary source.
pub(crate) const TYPESCRIPT_EXCLUDED_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    ".ripr",
    ".direnv",
    "dist",
    "build",
    "out",
    "coverage",
    ".next",
    ".cache",
    "vendor",
    "__generated__",
];

/// Whether a directory component is excluded from every TypeScript preview
/// surface: diff counting, finding generation, and workspace indexing.
pub(crate) fn is_typescript_excluded_dir_everywhere(name: &str) -> bool {
    TYPESCRIPT_EXCLUDED_DIRS.contains(&name)
}

/// Whether a directory component is pruned from the TypeScript preview
/// workspace walk.
///
/// This is the same set as [`is_typescript_excluded_dir_everywhere`], on
/// purpose. Python repo discovery keeps walking `vendor` so the files can be
/// recorded as an excluded role. This adapter has no such ledger, and indexing
/// those trees would only feed untrusted files into owner and test extraction.
pub(crate) fn is_typescript_dir_pruned_from_discovery(name: &str) -> bool {
    is_typescript_excluded_dir_everywhere(name)
}

/// Path-level form of [`is_typescript_excluded_dir_everywhere`].
///
/// A component that cannot be read as UTF-8 is never compared against the
/// UTF-8 table and is therefore not excluded by this predicate.
pub(crate) fn is_detectable_excluded_typescript_path(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(is_typescript_excluded_dir_everywhere)
    })
}

/// Whether a UTF-8 file name is a generated TypeScript/JavaScript artifact.
///
/// The family is `*.generated.*` (`cart.generated.ts`,
/// `types.generated.d.ts`). `generated.ts` and `regenerated.ts` are
/// near-misses and stay ordinary source.
pub(crate) fn is_detectable_generated_typescript_name(name: &str) -> bool {
    name.contains(".generated.")
}

/// Path form of [`is_detectable_generated_typescript_name`].
///
/// A non-UTF-8 file name can never match the UTF-8 `.generated.` family.
pub(crate) fn is_detectable_generated_typescript_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(is_detectable_generated_typescript_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excluded_dirs_match_components_not_substrings() {
        for path in [
            "node_modules/left-pad/index.js",
            "dist/bundle.js",
            "build/out.ts",
            "coverage/lcov-report/index.js",
            "vendor/lib.ts",
            "src/__generated__/types.ts",
            "packages/app/dist/index.js",
        ] {
            assert!(
                is_detectable_excluded_typescript_path(Path::new(path)),
                "{path} is an excluded subtree"
            );
        }
        for path in [
            "src/ok.ts",
            "src/build.ts",
            "src/vendor.ts",
            "src/coverage.ts",
            "packages/app/src/index.tsx",
        ] {
            assert!(
                !is_detectable_excluded_typescript_path(Path::new(path)),
                "{path} is ordinary source"
            );
        }
    }

    #[test]
    fn generated_names_match_the_dotted_family_only() {
        for name in [
            "cart.generated.ts",
            "Button.generated.tsx",
            "client.generated.js",
            "view.generated.jsx",
            "types.generated.d.ts",
        ] {
            assert!(
                is_detectable_generated_typescript_name(name),
                "{name} is generated"
            );
        }
        for name in [
            "generated.ts",
            "regenerated.ts",
            "notgenerated.js",
            "cart.ts",
        ] {
            assert!(
                !is_detectable_generated_typescript_name(name),
                "{name} is a near-miss"
            );
        }
    }

    #[test]
    fn discovery_prunes_the_same_directories_the_diff_refuses() {
        for name in TYPESCRIPT_EXCLUDED_DIRS {
            assert!(is_typescript_dir_pruned_from_discovery(name));
            assert!(is_typescript_excluded_dir_everywhere(name));
        }
        assert!(!is_typescript_dir_pruned_from_discovery("src"));
    }
}
