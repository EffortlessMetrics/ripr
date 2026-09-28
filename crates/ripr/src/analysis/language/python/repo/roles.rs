//! Shared Python workspace file-role authority (#3554 PR A).
//!
//! One classifier answers "what is this Python file for?" for repo mode, and
//! consumes the same config-owned discovery predicates (#3672) as project
//! detection, diff workspace collection, and repo discovery, so a file's
//! role cannot be re-derived differently by repo mode, diff mode, doctor,
//! and project detection.
//!
//! Every rule reuses the accepted config authorities:
//!
//! - environment/cache/vendor/build-tooling directories: the config
//!   excluded-directory authority `is_python_excluded_dir_everywhere`
//!   (`PYTHON_EXCLUDED_DIRS` plus the `vendor` family);
//! - generated source: the config generated-name authority
//!   `is_detectable_generated_python_path`;
//! - test-support helpers: the `conftest.py` pytest support convention;
//! - physical tests: the shared `is_test_file` rule (`test_*.py`,
//!   `*_test.py`, or a `tests`/`test` path component);
//! - production: any remaining adapter-routed Python source.

use super::super::{LanguageAdapter, PythonAdapter, is_test_file};
use crate::config::{is_detectable_generated_python_path, is_python_excluded_dir_everywhere};
use std::path::Path;

/// Shared workspace role for one Python file (#3554).
///
/// The taxonomy distinguishes production subjects from evidence sources and
/// from excluded material, so production findings can never be seeded from
/// test, helper, generated, or environment files.
/// `PartialOrd`/`Ord` order variants only for the bounded retention
/// heaps' `(path, role)` pairs (paths compare first, so role order is
/// the tiebreaker that never fires for distinct paths).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::analysis::language::python) enum PythonFileRole {
    /// Application/library source the adapter treats as a production-owner
    /// surface. The analysis-subject role.
    Production,
    /// A physically separate test file: `test_*.py`, `*_test.py`, or a file
    /// under a `tests`/`test` directory component. Evidence source.
    PhysicalTest,
    /// Test-support helper source that is not itself a test case (the
    /// pytest `conftest.py` convention). Evidence source.
    InlineHelper,
    /// Generated source recognized by the adapter's generated-file
    /// detection. Counted, never analyzed.
    Generated,
    /// Source under an environment, cache, vendor, or build-tooling
    /// directory. Counted, never analyzed.
    ExcludedEnvironment,
    /// A path this adapter does not route to Python, or whose role cannot
    /// be established. Kept typed so ambiguity stays visible.
    Unknown,
}

impl PythonFileRole {
    /// Stable role label for typed disclosure on output surfaces.
    pub(in crate::analysis::language::python) fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::PhysicalTest => "physical_test",
            Self::InlineHelper => "inline_helper",
            Self::Generated => "generated",
            Self::ExcludedEnvironment => "excluded_environment",
            Self::Unknown => "unknown",
        }
    }
}

/// Whether the role is excluded from the repo analysis working set.
///
/// Generated and environment/cache/vendor source is counted when discovered,
/// never analyzed.
pub(in crate::analysis::language::python) fn role_is_excluded_from_analysis(
    role: PythonFileRole,
) -> bool {
    matches!(
        role,
        PythonFileRole::Generated | PythonFileRole::ExcludedEnvironment
    )
}

/// Whether the file is test-support helper source (the pytest `conftest.py`
/// convention).
fn is_support_helper_file(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some("conftest.py")
}

/// Whether any path component is an excluded environment directory.
///
/// The config excluded-directory authority covers virtualenvs, caches,
/// site-packages, build/tooling output, and the `vendor` family, so a
/// discovered vendor tree lands in a typed excluded role instead of
/// production (#3672).
fn has_excluded_environment_component(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(is_python_excluded_dir_everywhere)
    })
}

/// Classify one path into the shared workspace role.
///
/// Precedence (first match wins), so every path has exactly one answer:
///
/// 1. not adapter-routed Python -> `Unknown`;
/// 2. environment/cache/vendor/tooling directory component ->
///    `ExcludedEnvironment`;
/// 3. adapter generated-file detection -> `Generated`;
/// 4. `conftest.py` support helper -> `InlineHelper` (including
///    `tests/conftest.py`: support, not a test case);
/// 5. shared test-file rule -> `PhysicalTest`;
/// 6. otherwise -> `Production`.
pub(in crate::analysis::language::python) fn classify_python_file_role(
    path: &Path,
) -> PythonFileRole {
    if !PythonAdapter.accepts_path(path) {
        return PythonFileRole::Unknown;
    }
    if has_excluded_environment_component(path) {
        return PythonFileRole::ExcludedEnvironment;
    }
    if is_detectable_generated_python_path(path) {
        return PythonFileRole::Generated;
    }
    if is_support_helper_file(path) {
        return PythonFileRole::InlineHelper;
    }
    if is_test_file(path) {
        return PythonFileRole::PhysicalTest;
    }
    PythonFileRole::Production
}

#[cfg(test)]
mod tests {
    use super::*;

    fn role(path: &str) -> PythonFileRole {
        classify_python_file_role(Path::new(path))
    }

    #[test]
    fn classifies_plain_source_as_production() {
        assert_eq!(role("app.py"), PythonFileRole::Production);
        assert_eq!(role("src/pkg/core.py"), PythonFileRole::Production);
    }

    #[test]
    fn classifies_physical_tests_by_name_and_directory() {
        assert_eq!(role("test_app.py"), PythonFileRole::PhysicalTest);
        assert_eq!(role("app_test.py"), PythonFileRole::PhysicalTest);
        assert_eq!(role("tests/test_app.py"), PythonFileRole::PhysicalTest);
        assert_eq!(role("tests/helpers.py"), PythonFileRole::PhysicalTest);
        assert_eq!(role("src/pkg/test_core.py"), PythonFileRole::PhysicalTest);
    }

    #[test]
    fn classifies_conftest_as_inline_helper_before_test_directory() {
        assert_eq!(role("conftest.py"), PythonFileRole::InlineHelper);
        // tests/conftest.py is support, not a test case.
        assert_eq!(role("tests/conftest.py"), PythonFileRole::InlineHelper);
    }

    #[test]
    fn classifies_generated_families() {
        assert_eq!(role("gen_pb2.py"), PythonFileRole::Generated);
        assert_eq!(role("gen_pb2_grpc.py"), PythonFileRole::Generated);
        assert_eq!(role("client.generated.py"), PythonFileRole::Generated);
        assert_eq!(role("client_generated.py"), PythonFileRole::Generated);
        assert_eq!(role("generated_client.py"), PythonFileRole::Generated);
        // Near-misses stay production (#3672): no suffix terminates on them
        // and the `generated_` prefix does not apply.
        assert_eq!(role("regenerated_client.py"), PythonFileRole::Production);
        assert_eq!(role("pb2.py"), PythonFileRole::Production);
    }

    #[test]
    fn classifies_environment_cache_and_vendor_roles() {
        assert_eq!(
            role(".venv/lib/pkg.py"),
            PythonFileRole::ExcludedEnvironment
        );
        assert_eq!(role("venv/lib/pkg.py"), PythonFileRole::ExcludedEnvironment);
        assert_eq!(role("env/lib/pkg.py"), PythonFileRole::ExcludedEnvironment);
        assert_eq!(
            role(".tox/py310/lib/pkg.py"),
            PythonFileRole::ExcludedEnvironment
        );
        assert_eq!(
            role("site-packages/pkg.py"),
            PythonFileRole::ExcludedEnvironment
        );
        assert_eq!(
            role("pkg/__pycache__/core.py"),
            PythonFileRole::ExcludedEnvironment
        );
        assert_eq!(
            role(".pytest_cache/data.py"),
            PythonFileRole::ExcludedEnvironment
        );
        assert_eq!(
            role(".mypy_cache/data.py"),
            PythonFileRole::ExcludedEnvironment
        );
        // The vendor family the config authority names; the repo discovery
        // walk does not prune it, so discovered vendor files must land in a
        // typed role.
        assert_eq!(role("vendor/dep.py"), PythonFileRole::ExcludedEnvironment);
    }

    #[test]
    fn excluded_environment_takes_precedence_over_generated() {
        assert_eq!(
            role(".venv/lib/gen_pb2.py"),
            PythonFileRole::ExcludedEnvironment
        );
    }

    #[test]
    fn classifies_unrouted_paths_as_unknown() {
        assert_eq!(role("README.md"), PythonFileRole::Unknown);
        assert_eq!(role("src/app.ts"), PythonFileRole::Unknown);
        assert_eq!(role("src/main.rs"), PythonFileRole::Unknown);
    }

    #[test]
    fn excluded_roles_are_exactly_generated_and_environment() {
        assert!(role_is_excluded_from_analysis(PythonFileRole::Generated));
        assert!(role_is_excluded_from_analysis(
            PythonFileRole::ExcludedEnvironment
        ));
        assert!(!role_is_excluded_from_analysis(PythonFileRole::Production));
        assert!(!role_is_excluded_from_analysis(
            PythonFileRole::PhysicalTest
        ));
        assert!(!role_is_excluded_from_analysis(
            PythonFileRole::InlineHelper
        ));
        assert!(!role_is_excluded_from_analysis(PythonFileRole::Unknown));
    }

    #[test]
    fn role_labels_are_stable() {
        assert_eq!(PythonFileRole::Production.as_str(), "production");
        assert_eq!(PythonFileRole::PhysicalTest.as_str(), "physical_test");
        assert_eq!(PythonFileRole::InlineHelper.as_str(), "inline_helper");
        assert_eq!(PythonFileRole::Generated.as_str(), "generated");
        assert_eq!(
            PythonFileRole::ExcludedEnvironment.as_str(),
            "excluded_environment"
        );
        assert_eq!(PythonFileRole::Unknown.as_str(), "unknown");
    }
}
