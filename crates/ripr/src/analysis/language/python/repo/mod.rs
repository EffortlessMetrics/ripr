//! Python repo-mode input + evidence authorities (#3554 PR A / PR B) and
//! the producer surface `PythonAdapter::analyze_repo` consumes (PR C).
//!
//! PR A builds the input structure beneath the former repo-mode limitation
//! (see `docs/LANGUAGE_ADAPTER_PREVIEW.md`): shared workspace role
//! selection, bounded discovery (#2109), and typed run status. PR B adds
//! the native evidence producer ([`evidence::build_repo_evidence`]): facts
//! built once over the selected production/test set, native Python behavior
//! items with related-test and oracle evidence, typed limitations, and the
//! public `Finding` projection over the same pass. PR C wires
//! `PythonAdapter::analyze_repo` to
//! [`select_repo_input_with_limit`] + [`build_repo_evidence`] and maps
//! [`evidence::partial_disclosure`] onto the pipeline's shared
//! `LanguageRun` partial-run channel.
//!
//! [`select_repo_input_with_limit`] composes the input modules into one
//! [`PythonRepoInput`]: the bounded selected working set partitioned by
//! role, the typed discovery counts, the run status, the effective
//! working-set limit with its cap source, the retained operator recovery
//! routes, and the test framework detected by the shared producer.

#![allow(
    dead_code,
    reason = "PythonRepoRunStatus::Disabled and the config-disabled wrappers stay typed ahead of a config producer; remaining unconsumed items are exercised by unit tests (#3554)"
)]

mod discovery;
mod evidence;
mod roles;
mod run_status;

#[cfg(test)]
pub(in crate::analysis::language::python) use discovery::RepoWorkingSetCapSource;
use discovery::{CapRecoveryRoute, DiscoveryCounts, discover_repo_working_set};
pub(in crate::analysis::language::python) use discovery::{
    RepoWorkingSetLimit, repo_working_set_limit,
};
pub(in crate::analysis::language::python) use evidence::{build_repo_evidence, partial_disclosure};
use roles::PythonFileRole;
use run_status::{PartialRunReason, PythonRepoRunStatus};
use std::path::{Path, PathBuf};

use super::super::detect_python_test_framework;

/// The selected Python repo analysis input for one workspace.
///
/// Test and helper files are evidence sources; they never appear in
/// `production_files`, so a repo-mode producer cannot seed production
/// findings from them (#3554).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::analysis::language::python) struct PythonRepoInput {
    pub(in crate::analysis::language::python) status: PythonRepoRunStatus,
    /// Selected production-role files, sorted. The analysis subjects.
    pub(in crate::analysis::language::python) production_files: Vec<PathBuf>,
    /// Selected physical-test files, sorted. Evidence sources.
    pub(in crate::analysis::language::python) test_files: Vec<PathBuf>,
    /// Selected test-support helper files (`conftest.py`), sorted.
    pub(in crate::analysis::language::python) helper_files: Vec<PathBuf>,
    /// Selected files whose role is ambiguous. Retained so the partition
    /// stays exhaustive over the role taxonomy: a routed file is currently
    /// never `Unknown`, and if that changes it must be visible here rather
    /// than silently analyzed as production.
    pub(in crate::analysis::language::python) ambiguous_files: Vec<PathBuf>,
    /// Typed working-set counts (#2109).
    pub(in crate::analysis::language::python) counts: DiscoveryCounts,
    /// The effective working-set limit and its cap source.
    pub(in crate::analysis::language::python) working_set_limit: RepoWorkingSetLimit,
    /// Operator recovery routes retained for capped runs (#2109); stable
    /// typed values any surface can render.
    pub(in crate::analysis::language::python) recovery_routes: &'static [CapRecoveryRoute],
    /// Test framework detected by the shared producer
    /// (`detect_python_test_framework`) at the workspace root.
    pub(in crate::analysis::language::python) test_framework: Option<&'static str>,
}

/// Select the Python repo analysis input with an explicit working-set limit.
///
/// The deterministic core of the repo input selection; the caller resolves
/// the limit from the shared environment override (#2109, see
/// [`repo_working_set_limit`]) before delegating here.
pub(in crate::analysis::language::python) fn select_repo_input_with_limit(
    root: &Path,
    working_set_limit: RepoWorkingSetLimit,
) -> PythonRepoInput {
    let discovery::RepoDiscovery { selected, counts } =
        discover_repo_working_set(root, working_set_limit.limit);

    let mut production_files = Vec::new();
    let mut test_files = Vec::new();
    let mut helper_files = Vec::new();
    let mut ambiguous_files = Vec::new();
    for (path, role) in selected {
        match role {
            PythonFileRole::Production => production_files.push(path),
            PythonFileRole::PhysicalTest => test_files.push(path),
            PythonFileRole::InlineHelper => helper_files.push(path),
            PythonFileRole::Unknown => ambiguous_files.push(path),
            // Excluded roles never reach the selection: discovery filters
            // them out with counts.
            PythonFileRole::Generated | PythonFileRole::ExcludedEnvironment => {}
        }
    }

    // Status precedence: "no subject" is the more fundamental fact — a run
    // with no production source has nothing to analyze even when the working
    // set was also capped.
    // Status precedence: "no subject" is the more fundamental fact — a run
    // with no production source has nothing to analyze even when the working
    // set was also capped. A discovery-only selection can never be
    // `Complete`: analysis (PR B) has not run, so the honest state is
    // `Selected` (#3666 review).
    // Status precedence: incomplete discovery is the most fundamental
    // fact — an unreadable root or subtree can contain Python source
    // (production or evidence), so source absence is unestablished and
    // the run is partial before the no-source condition is even
    // evaluated (#3666 review). "No subject" is next: a run with no
    // production source has nothing to analyze even when the working set
    // was also capped. A discovery-only selection can never be
    // `Complete`: analysis (PR B) has not run, so the honest state is
    // `Selected`.
    let status = if counts.unreadable_subtrees > 0 {
        PythonRepoRunStatus::Partial {
            reason: PartialRunReason::DiscoveryIncomplete {
                unreadable: counts.unreadable_subtrees,
            },
        }
    } else if counts.discovered == 0 || production_files.is_empty() {
        PythonRepoRunStatus::NoPythonSource
    } else if counts.capped > 0 {
        PythonRepoRunStatus::Capped
    } else {
        PythonRepoRunStatus::Selected
    };

    PythonRepoInput {
        status,
        production_files,
        test_files,
        helper_files,
        ambiguous_files,
        counts,
        working_set_limit,
        recovery_routes: discovery::CAP_RECOVERY_ROUTES,
        test_framework: detect_python_test_framework(root),
    }
}

#[cfg(test)]
mod tests {
    use super::discovery::RepoWorkingSetCapSource;
    use super::*;

    fn write_file(path: &Path, contents: &str) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| format!("create parent: {err}"))?;
        }
        std::fs::write(path, contents).map_err(|err| format!("write {}: {err}", path.display()))
    }

    fn unique_test_root(label: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "ripr-py-repo-input-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn default_limit() -> RepoWorkingSetLimit {
        RepoWorkingSetLimit {
            limit: 800,
            source: RepoWorkingSetCapSource::Default,
        }
    }

    fn path_strings(files: &[PathBuf]) -> Vec<String> {
        let mut paths: Vec<String> = files.iter().map(|path| relative(path)).collect();
        paths.sort();
        paths
    }

    fn relative(path: &Path) -> String {
        path.to_string_lossy().replace('\\', "/")
    }

    #[test]
    fn select_covers_the_ordinary_flat_layout() -> Result<(), String> {
        let root = unique_test_root("flat");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(&root.join("app.py"), "def run():\n    return 1\n")?;
        write_file(
            &root.join("test_app.py"),
            "from app import run\n\n\ndef test_run():\n    assert run() == 1\n",
        )?;
        let input = select_repo_input_with_limit(&root, default_limit());
        assert_eq!(input.status, PythonRepoRunStatus::Selected);
        // Discovery alone cannot claim a full denominator: analysis (PR B)
        // has not run. The honest post-selection state is Selected.
        assert!(!input.status.can_support_full_denominator());
        assert_eq!(path_strings(&input.production_files), vec!["app.py"]);
        assert_eq!(path_strings(&input.test_files), vec!["test_app.py"]);
        assert!(input.helper_files.is_empty());
        assert!(input.ambiguous_files.is_empty());
        assert_eq!(input.counts.discovered, 2);
        assert_eq!(input.counts.selected, 2);
        assert_eq!(input.counts.analyzed_candidates, 1);
        assert_eq!(input.counts.skipped, 0);
        assert_eq!(input.counts.excluded_by_role, 0);
        assert_eq!(input.counts.capped, 0);
        assert_eq!(input.counts.failed, 0);
        assert_eq!(input.working_set_limit, default_limit());
        // A plain `def test_run` carries no framework markers, so the shared
        // producer reports none.
        assert_eq!(input.test_framework, None);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn select_covers_the_src_layout() -> Result<(), String> {
        let root = unique_test_root("src-layout");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(&root.join("src/pkg/core.py"), "def core():\n    return 1\n")?;
        write_file(
            &root.join("tests/test_core.py"),
            "def test_core():\n    assert True\n",
        )?;
        let input = select_repo_input_with_limit(&root, default_limit());
        assert_eq!(input.status, PythonRepoRunStatus::Selected);
        assert_eq!(
            path_strings(&input.production_files),
            vec!["src/pkg/core.py"]
        );
        assert_eq!(path_strings(&input.test_files), vec!["tests/test_core.py"]);
        assert_eq!(input.counts.analyzed_candidates, 1);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn select_covers_the_tests_layout_with_helpers_and_framework() -> Result<(), String> {
        let root = unique_test_root("tests-layout");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(&root.join("pkg/api.py"), "def api():\n    return 1\n")?;
        write_file(
            &root.join("tests/test_api.py"),
            "import unittest\n\nclass ApiTest(unittest.TestCase):\n    def test_api(self):\n        self.assertEqual(api(), 1)\n",
        )?;
        write_file(&root.join("conftest.py"), "import pytest\n")?;
        let input = select_repo_input_with_limit(&root, default_limit());
        assert_eq!(input.status, PythonRepoRunStatus::Selected);
        assert_eq!(path_strings(&input.production_files), vec!["pkg/api.py"]);
        assert_eq!(path_strings(&input.test_files), vec!["tests/test_api.py"]);
        assert_eq!(path_strings(&input.helper_files), vec!["conftest.py"]);
        // The framework comes from the shared producer, not a repo-mode
        // re-derivation. The root `conftest.py` is pytest evidence, and the
        // producer's precedence reports pytest before the unittest scan.
        assert_eq!(input.test_framework, Some("pytest"));
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn select_reports_no_python_source_for_an_empty_workspace() -> Result<(), String> {
        let root = unique_test_root("no-python");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        let input = select_repo_input_with_limit(&root, default_limit());
        assert_eq!(input.status, PythonRepoRunStatus::NoPythonSource);
        assert!(!input.status.can_support_full_denominator());
        assert_eq!(input.counts.discovered, 0);
        assert!(input.production_files.is_empty());
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn select_reports_no_python_source_for_a_tests_only_workspace() -> Result<(), String> {
        let root = unique_test_root("tests-only");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(
            &root.join("test_only.py"),
            "def test_only():\n    assert True\n",
        )?;
        let input = select_repo_input_with_limit(&root, default_limit());
        assert_eq!(input.status, PythonRepoRunStatus::NoPythonSource);
        assert!(!input.status.can_support_full_denominator());
        // The evidence sources stay visible even though no subject exists.
        assert_eq!(input.counts.discovered, 1);
        assert_eq!(path_strings(&input.test_files), vec!["test_only.py"]);
        assert!(input.production_files.is_empty());
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn select_covers_a_mixed_rust_python_workspace() -> Result<(), String> {
        let root = unique_test_root("mixed");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(&root.join("main.rs"), "fn main() {}\n")?;
        write_file(&root.join("app.py"), "def run():\n    return 1\n")?;
        write_file(
            &root.join("test_app.py"),
            "def test_run():\n    assert True\n",
        )?;
        let input = select_repo_input_with_limit(&root, default_limit());
        assert_eq!(input.status, PythonRepoRunStatus::Selected);
        // Rust files are a different producer's scope; counts are
        // Python-only.
        assert_eq!(input.counts.discovered, 2);
        assert_eq!(path_strings(&input.production_files), vec!["app.py"]);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn select_reports_a_capped_run_that_cannot_support_a_full_denominator() -> Result<(), String> {
        let root = unique_test_root("capped");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        for name in ["alpha.py", "beta.py", "gamma.py"] {
            write_file(&root.join(name), "def value():\n    return 1\n")?;
        }
        let input = select_repo_input_with_limit(
            &root,
            RepoWorkingSetLimit {
                limit: 2,
                source: RepoWorkingSetCapSource::EnvOverride,
            },
        );
        assert_eq!(input.status, PythonRepoRunStatus::Capped);
        assert!(!input.status.can_support_full_denominator());
        assert_eq!(input.counts.selected, 2);
        assert_eq!(input.counts.capped, 1);
        assert_eq!(input.counts.skipped, 1);
        assert_eq!(
            input.working_set_limit,
            RepoWorkingSetLimit {
                limit: 2,
                source: RepoWorkingSetCapSource::EnvOverride,
            }
        );
        // The operator recovery route is retained with the capped run.
        assert_eq!(input.recovery_routes.len(), 2);
        let descriptions: Vec<&str> = input
            .recovery_routes
            .iter()
            .map(|route| route.describe())
            .collect();
        assert!(descriptions[0].contains("--base"), "{}", descriptions[0]);
        assert!(
            descriptions[1].contains("RIPR_MAX_REPO_INDEX_FILES"),
            "{}",
            descriptions[1]
        );
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn select_excludes_generated_and_environment_files_with_counts() -> Result<(), String> {
        let root = unique_test_root("excluded");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(&root.join("app.py"), "def run():\n    return 1\n")?;
        write_file(&root.join("gen_pb2.py"), "# generated\n")?;
        write_file(&root.join("vendor/dep.py"), "VALUE = 2\n")?;
        let input = select_repo_input_with_limit(&root, default_limit());
        assert_eq!(input.counts.discovered, 3);
        assert_eq!(input.counts.excluded_by_role, 2);
        assert_eq!(input.counts.skipped, 2);
        assert_eq!(input.counts.selected, 1);
        assert_eq!(path_strings(&input.production_files), vec!["app.py"]);
        assert_eq!(input.status, PythonRepoRunStatus::Selected);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn select_is_deterministic_across_runs() -> Result<(), String> {
        let root = unique_test_root("determinism");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(&root.join("app.py"), "def run():\n    return 1\n")?;
        write_file(
            &root.join("tests/test_app.py"),
            "def test_run():\n    assert True\n",
        )?;
        write_file(&root.join("vendor/dep.py"), "VALUE = 2\n")?;
        let first = select_repo_input_with_limit(&root, default_limit());
        let second = select_repo_input_with_limit(&root, default_limit());
        assert_eq!(first, second);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }
    /// #3666 review: a role-blind sorted prefix could fill the cap with
    /// evidence files and hide production source behind `capped`. The cap
    /// gives production files priority, so they are always selected first.
    #[test]
    fn cap_gives_production_files_selection_priority() -> Result<(), String> {
        let root = unique_test_root("priority-cap");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        // The test file sorts before the production file, so a role-blind
        // prefix would select the test and cap the production source.
        write_file(
            &root.join("test_helper.py"),
            "def helper() {\n    pass\n}\n",
        )?;
        write_file(&root.join("z_prod.py"), "def one() {\n    return 1\n}\n")?;
        let input = select_repo_input_with_limit(
            &root,
            RepoWorkingSetLimit {
                limit: 1,
                source: RepoWorkingSetCapSource::Default,
            },
        );
        assert_eq!(
            path_strings(&input.production_files),
            vec!["z_prod.py"],
            "production files must win the cap over evidence files: {:?}",
            input.counts
        );
        // The evidence file is what the cap excludes.
        assert!(input.test_files.is_empty());
        assert_eq!(input.counts.capped, 1);
        assert_eq!(input.status, PythonRepoRunStatus::Capped);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    /// #3666 review: production source behind the cap must report `Capped`
    /// (production exists, denominator truncated), never `NoPythonSource`
    /// (which claims the workspace has no production source at all).
    #[test]
    fn capped_production_source_is_capped_not_no_python_source() -> Result<(), String> {
        let root = unique_test_root("capped-production");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        // Helper sorts first; the production file lands beyond a cap of 1.
        write_file(
            &root.join("a helper.py"),
            "def helper():
    pass
",
        )?;
        write_file(
            &root.join("z prod.py"),
            "def one():
    return 1
",
        )?;
        let input = select_repo_input_with_limit(
            &root,
            RepoWorkingSetLimit {
                limit: 1,
                source: RepoWorkingSetCapSource::Default,
            },
        );
        assert_eq!(input.status, PythonRepoRunStatus::Capped);
        assert!(!input.status.can_support_full_denominator());
        assert_eq!(input.counts.analyzed_candidates, 1);
        assert_eq!(input.counts.capped, 1);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    /// #3666 review: the walk counts subtrees it could not read, and the
    /// count discloses that the discovered set may be incomplete.
    #[test]
    fn readable_walk_reports_zero_unreadable_subtrees() -> Result<(), String> {
        let root = unique_test_root("unreadable-zero");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(
            &root.join("app.py"),
            "def run():
    return 1
",
        )?;
        let input = select_repo_input_with_limit(&root, default_limit());
        assert_eq!(input.counts.unreadable_subtrees, 0);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }
}
