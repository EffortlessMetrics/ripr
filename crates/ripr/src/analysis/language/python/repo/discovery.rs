//! Bounded repo working-set discovery for the Python adapter (#3554 PR A).
//!
//! Issue #2109 contract: repo-scoped analysis must never read a workspace
//! without a working-set bound. The Python repo walk prunes the config
//! excluded-directory subtrees at directory granularity via
//! `is_python_dir_pruned_from_repo_discovery` (#3672) — deliberately NOT
//! `vendor`, so vendor files are walked and counted as typed excluded-role
//! inputs while environment/cache/build subtrees cost nothing — counts every
//! adapter-routed Python file it sees, classifies each with the shared role
//! authority, and selects at most the working-set limit of eligible files in
//! deterministic (sorted) order.
//!
//! The cap source and the operator recovery route are retained as typed
//! fields so every surface can disclose the same remediation.

use super::super::{LanguageAdapter, PythonAdapter};
use super::roles::{PythonFileRole, classify_python_file_role, role_is_excluded_from_analysis};
use crate::config::is_python_dir_pruned_from_repo_discovery;
use std::path::{Path, PathBuf};

/// Default repo working-set limit for the Python adapter.
///
/// Same bound family as the Rust repo guard (`REPO_INDEX_FILE_LIMIT`,
/// #2109): repo-scoped analysis over a working set at this scale is the
/// protected envelope; larger workspaces must raise the limit explicitly.
pub(in crate::analysis::language::python) const PYTHON_REPO_FILE_LIMIT: usize = 800;

/// Shared repo working-set override (see #2109). One operator knob governs
/// the repo working-set bound across language producers.
pub(in crate::analysis::language::python) const PYTHON_REPO_FILE_LIMIT_ENV: &str =
    "RIPR_MAX_REPO_INDEX_FILES";

/// Where the effective working-set limit came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::analysis::language::python) enum RepoWorkingSetCapSource {
    /// The producer-owned default ([`PYTHON_REPO_FILE_LIMIT`]).
    Default,
    /// An operator raised or lowered it via
    /// [`PYTHON_REPO_FILE_LIMIT_ENV`].
    EnvOverride,
}

/// The effective repo working-set limit and its source (#2109).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::analysis::language::python) struct RepoWorkingSetLimit {
    pub(in crate::analysis::language::python) limit: usize,
    pub(in crate::analysis::language::python) source: RepoWorkingSetCapSource,
}

/// Operator recovery routes for a capped working set, retained as typed
/// values so every surface renders the same remediation (#2109).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::analysis::language::python) enum CapRecoveryRoute {
    /// Narrow the scope to changed behavior with a diff-based run.
    DiffBasedRun,
    /// Raise the working-set limit via the shared env override.
    RaiseWorkingSetLimit,
}

impl CapRecoveryRoute {
    pub(in crate::analysis::language::python) fn describe(self) -> &'static str {
        match self {
            Self::DiffBasedRun => "narrow the scope with a diff-based run (--base/--diff)",
            Self::RaiseWorkingSetLimit => "raise the limit via RIPR_MAX_REPO_INDEX_FILES=<number>",
        }
    }
}

/// Recovery routes in operator-facing priority order.
pub(in crate::analysis::language::python) const CAP_RECOVERY_ROUTES: &[CapRecoveryRoute] = &[
    CapRecoveryRoute::DiffBasedRun,
    CapRecoveryRoute::RaiseWorkingSetLimit,
];

/// Resolve the effective working-set limit from a shared-env read.
///
/// Mirrors the Rust adapter's positive-integer env contract (#2109): absent
/// means the default; zero, non-integer, or non-UTF-8 values fail with a
/// named error instead of being silently ignored.
pub(in crate::analysis::language::python) fn resolve_repo_working_set_limit(
    env: Result<String, std::env::VarError>,
) -> Result<RepoWorkingSetLimit, String> {
    match env {
        Err(std::env::VarError::NotPresent) => Ok(RepoWorkingSetLimit {
            limit: PYTHON_REPO_FILE_LIMIT,
            source: RepoWorkingSetCapSource::Default,
        }),
        Ok(raw) => {
            let parsed = raw.trim().parse::<usize>().map_err(|err| {
                format!("{PYTHON_REPO_FILE_LIMIT_ENV} must be a positive integer: {err}")
            })?;
            if parsed == 0 {
                return Err(format!(
                    "{PYTHON_REPO_FILE_LIMIT_ENV} must be a positive integer"
                ));
            }
            Ok(RepoWorkingSetLimit {
                limit: parsed,
                source: RepoWorkingSetCapSource::EnvOverride,
            })
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(format!("{PYTHON_REPO_FILE_LIMIT_ENV} must be valid UTF-8"))
        }
    }
}

/// Read the effective working-set limit from the process environment.
pub(in crate::analysis::language::python) fn repo_working_set_limit()
-> Result<RepoWorkingSetLimit, String> {
    resolve_repo_working_set_limit(std::env::var(PYTHON_REPO_FILE_LIMIT_ENV))
}

/// Typed working-set counts for one bounded discovery pass (#2109, #3554).
///
/// Identities a reviewer or renderer can check:
///
/// - `discovered == selected + skipped`
/// - `skipped == excluded_by_role + capped`
/// - `analyzed_candidates <= selected` (the production-role subset)
/// - when analysis ran: `analyzed + failed == selected - ambiguous`
///   (ambiguous-role files are counted, never read; the current role
///   authority never routes one, so the identity reads
///   `analyzed + failed == selected`)
///
/// `failed` and `analyzed` describe source reads, which discovery performs
/// none of: both are always 0 here. The evidence producer (#3554 PR B) owns
/// populating them over the selected working set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::analysis::language::python) struct DiscoveryCounts {
    /// Adapter-routed Python files seen by the pruned walk, including
    /// generated and role-excluded files.
    pub(in crate::analysis::language::python) discovered: usize,
    /// Eligible files taken into the working set under the cap.
    pub(in crate::analysis::language::python) selected: usize,
    /// Selected production-role files: the analysis subjects.
    pub(in crate::analysis::language::python) analyzed_candidates: usize,
    /// Discovered files intentionally not selected:
    /// `excluded_by_role + capped`.
    pub(in crate::analysis::language::python) skipped: usize,
    /// Discovered files excluded by their typed role (generated,
    /// environment/cache/vendor).
    pub(in crate::analysis::language::python) excluded_by_role: usize,
    /// Eligible files beyond the cap: counted, not analyzed.
    pub(in crate::analysis::language::python) capped: usize,
    /// Selected files the evidence producer could not read or parse; zero
    /// after discovery — the producer (PR B) owns populating it.
    pub(in crate::analysis::language::python) failed: usize,
    /// Selected files the evidence producer successfully read and parsed;
    /// zero after discovery — the producer (PR B) owns populating it.
    /// Together with `failed` this keeps the analyzed denominator explicit.
    pub(in crate::analysis::language::python) analyzed: usize,
    /// Directory subtrees or entries the walk could not read (permission
    /// or I/O errors). Their contents were never discovered, so a
    /// non-zero count means the discovered set is incomplete: a
    /// full-denominator claim is not earned and the status must not
    /// report `NoPythonSource` (#3666 review).
    pub(in crate::analysis::language::python) unreadable_subtrees: usize,
}

/// Result of one bounded discovery pass: the selected working set with
/// roles, plus the typed counts.
pub(in crate::analysis::language::python) struct RepoDiscovery {
    /// Selected `(path, role)` pairs in deterministic sorted-path order.
    pub(in crate::analysis::language::python) selected: Vec<(PathBuf, PythonFileRole)>,
    pub(in crate::analysis::language::python) counts: DiscoveryCounts,
}

/// Discover and select the bounded Python repo working set under `root`.
///
/// Deterministic: retained candidates are the lexicographically smallest
/// eligible paths, and the selected output is sorted. Walk memory is
/// bounded by the working-set limit (#3666 review): each role retains a
/// bounded max-heap of the smallest candidates seen, instead of holding
/// every eligible path.
pub(in crate::analysis::language::python) fn discover_repo_working_set(
    root: &Path,
    limit: usize,
) -> RepoDiscovery {
    let mut state = BoundedWalkState {
        production_heap: std::collections::BinaryHeap::new(),
        evidence_heap: std::collections::BinaryHeap::new(),
        excluded_by_role: 0,
        unreadable_subtrees: 0,
        production_seen_total: 0,
        evidence_seen_total: 0,
        limit,
    };
    visit_repo_workspace(root, root, &mut state);

    // True totals: the bounded heaps can drop the largest candidates when
    // a workspace exceeds the limit, so the run tracks seen counts
    // separately from retained heap contents.
    let production_seen = state.production_seen_total;
    let evidence_seen = state.evidence_seen_total;

    // A max-heap pops largest-first, so draining into a vector and
    // reversing yields the retained candidates in ascending order.
    let mut production: Vec<PathBuf> = state.production_heap.into_vec();
    production.sort();
    let mut evidence: Vec<(PathBuf, PythonFileRole)> = state.evidence_heap.into_vec();
    evidence.sort_by(|(left, _), (right, _)| left.cmp(right));
    let excluded_by_role = state.excluded_by_role;
    let unreadable_subtrees = state.unreadable_subtrees;

    // Production files are the analysis subjects and get cap priority
    // (#3666 review): a role-blind prefix could fill the cap with
    // evidence files and hide production source behind `capped`, which
    // would misreport the workspace as having no production source at
    // all.
    let selected_production = production.len().min(limit);
    let evidence_budget = limit - selected_production;
    let mut selected: Vec<(PathBuf, PythonFileRole)> = production
        .iter()
        .take(selected_production)
        .map(|path| (path.clone(), PythonFileRole::Production))
        .chain(
            evidence
                .iter()
                .take(evidence_budget)
                .map(|(path, role)| (path.clone(), *role)),
        )
        .collect();
    // The heaps drain per role, so the combined set must be re-sorted to
    // restore the documented sorted-path order when production and
    // evidence paths interleave (#3666 review).
    selected.sort_by(|(left, _), (right, _)| left.cmp(right));

    let selected_count = selected.len();
    let analyzed_candidates = selected_production;
    // Every routed file is either selected or capped (excluded files are
    // accounted separately), so the identity
    // `discovered == selected + skipped` pins the split.
    let capped = (production_seen + evidence_seen) - selected_count;

    RepoDiscovery {
        selected,
        counts: DiscoveryCounts {
            discovered: production_seen + evidence_seen + excluded_by_role,
            selected: selected_count,
            analyzed_candidates,
            skipped: excluded_by_role + capped,
            excluded_by_role,
            capped,
            failed: 0,
            analyzed: 0,
            unreadable_subtrees,
        },
    }
}

/// Pruned workspace walk.
///
/// Mirrors the adapter's `visit_workspace` pattern (no descent into
/// excluded-directory subtrees), except that `vendor` is deliberately NOT
/// pruned (#3672): vendor files must be discovered so the role authority
/// counts them as typed excluded-role inputs. Eligible files are retained
/// in the bounded per-role heaps; excluded-role and generated files are
/// counted but never stored, so walk memory is bounded by the working-set
/// cap (#3666 review). Read failures on subtrees or entries count as
/// incomplete-discovery accounting.
/// Accumulated bounded-retention walk state: per-role candidate heaps
/// (bounded by the working-set limit), the incomplete-discovery
/// accounting, and the true seen totals.
struct BoundedWalkState {
    production_heap: std::collections::BinaryHeap<PathBuf>,
    evidence_heap: std::collections::BinaryHeap<(PathBuf, PythonFileRole)>,
    excluded_by_role: usize,
    unreadable_subtrees: usize,
    production_seen_total: usize,
    evidence_seen_total: usize,
    limit: usize,
}

impl BoundedWalkState {
    fn retain_eligible(&mut self, relative: PathBuf, role: PythonFileRole) {
        if role == PythonFileRole::Production {
            self.production_seen_total += 1;
            self.production_heap.push(relative);
            if self.production_heap.len() > self.limit {
                self.production_heap.pop();
            }
        } else {
            self.evidence_seen_total += 1;
            self.evidence_heap.push((relative, role));
            if self.evidence_heap.len() > self.limit {
                self.evidence_heap.pop();
            }
        }
    }
}

fn visit_repo_workspace(root: &Path, dir: &Path, state: &mut BoundedWalkState) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => {
            state.unreadable_subtrees += 1;
            return;
        }
    };
    for entry in entries {
        // A failed entry is an omitted file: count it in the same
        // incomplete-discovery accounting.
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                state.unreadable_subtrees += 1;
                continue;
            }
        };
        let name = entry.file_name();
        let name_text = name.to_str().unwrap_or_default();
        if is_python_dir_pruned_from_repo_discovery(name_text) {
            continue;
        }
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => {
                state.unreadable_subtrees += 1;
                continue;
            }
        };
        let path = entry.path();
        if file_type.is_dir() {
            visit_repo_workspace(root, &path, state);
        } else if file_type.is_file() {
            let adapter = PythonAdapter;
            if adapter.accepts_path(&path) {
                let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                let role = classify_python_file_role(&relative);
                if role_is_excluded_from_analysis(role) {
                    state.excluded_by_role += 1;
                } else {
                    state.retain_eligible(relative, role);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
            "ripr-py-repo-discovery-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn selected_paths(discovery: &RepoDiscovery) -> Vec<String> {
        discovery
            .selected
            .iter()
            .map(|(path, _)| path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    #[test]
    fn resolve_limit_defaults_when_env_absent() -> Result<(), String> {
        let resolved = resolve_repo_working_set_limit(Err(std::env::VarError::NotPresent))?;
        assert_eq!(resolved.limit, PYTHON_REPO_FILE_LIMIT);
        assert_eq!(resolved.source, RepoWorkingSetCapSource::Default);
        Ok(())
    }

    #[test]
    fn resolve_limit_accepts_positive_override() -> Result<(), String> {
        let resolved = resolve_repo_working_set_limit(Ok(" 120 ".to_string()))?;
        assert_eq!(resolved.limit, 120);
        assert_eq!(resolved.source, RepoWorkingSetCapSource::EnvOverride);
        Ok(())
    }

    #[test]
    fn resolve_limit_rejects_zero_garbage_and_non_unicode() -> Result<(), String> {
        let zero = resolve_repo_working_set_limit(Ok("0".to_string()))
            .err()
            .ok_or("a zero limit must be rejected")?;
        assert!(zero.contains(PYTHON_REPO_FILE_LIMIT_ENV), "{zero}");

        let garbage = resolve_repo_working_set_limit(Ok("many".to_string()))
            .err()
            .ok_or("a non-integer limit must be rejected")?;
        assert!(garbage.contains(PYTHON_REPO_FILE_LIMIT_ENV), "{garbage}");

        let non_unicode =
            resolve_repo_working_set_limit(Err(std::env::VarError::NotUnicode("x".into())))
                .err()
                .ok_or("a non-UTF-8 limit must be rejected")?;
        assert!(
            non_unicode.contains(PYTHON_REPO_FILE_LIMIT_ENV),
            "{non_unicode}"
        );
        Ok(())
    }

    #[test]
    fn discovery_selects_nothing_from_an_empty_workspace() -> Result<(), String> {
        let root = unique_test_root("empty");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        let discovery = discover_repo_working_set(&root, 800);
        assert!(discovery.selected.is_empty());
        assert_eq!(discovery.counts, DiscoveryCounts::default());
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn discovery_enforces_the_working_set_cap_with_counts() -> Result<(), String> {
        let root = unique_test_root("capped");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        for name in ["alpha.py", "beta.py", "gamma.py"] {
            write_file(&root.join(name), "def value():\n    return 1\n")?;
        }
        let discovery = discover_repo_working_set(&root, 2);
        assert_eq!(discovery.counts.discovered, 3);
        assert_eq!(discovery.counts.selected, 2);
        assert_eq!(discovery.counts.analyzed_candidates, 2);
        assert_eq!(discovery.counts.capped, 1);
        assert_eq!(discovery.counts.skipped, 1);
        assert_eq!(discovery.counts.excluded_by_role, 0);
        assert_eq!(selected_paths(&discovery).len(), 2);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn discovery_keeps_deterministic_sorted_order() -> Result<(), String> {
        let root = unique_test_root("order");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        for name in ["zeta.py", "src/alpha.py", "beta.py"] {
            write_file(&root.join(name), "VALUE = 1\n")?;
        }
        let first = discover_repo_working_set(&root, 800);
        let second = discover_repo_working_set(&root, 800);
        assert_eq!(
            selected_paths(&first),
            vec!["beta.py", "src/alpha.py", "zeta.py"]
        );
        assert_eq!(first.selected, second.selected);
        assert_eq!(first.counts, second.counts);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn discovery_counts_generated_and_environment_exclusions_by_role() -> Result<(), String> {
        let root = unique_test_root("excluded");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(&root.join("app.py"), "VALUE = 1\n")?;
        write_file(&root.join("gen_pb2.py"), "# generated\n")?;
        write_file(&root.join("vendor/dep.py"), "VALUE = 2\n")?;
        // Excluded-directory subtrees are pruned, not walked: the .venv file
        // contributes nothing to any count.
        write_file(&root.join(".venv/lib/hidden.py"), "VALUE = 3\n")?;
        let discovery = discover_repo_working_set(&root, 800);
        assert_eq!(discovery.counts.discovered, 3);
        assert_eq!(discovery.counts.excluded_by_role, 2);
        assert_eq!(discovery.counts.selected, 1);
        assert_eq!(discovery.counts.skipped, 2);
        assert_eq!(selected_paths(&discovery), vec!["app.py"]);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn discovery_ignores_non_python_files_in_a_mixed_workspace() -> Result<(), String> {
        let root = unique_test_root("mixed");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(&root.join("app.py"), "VALUE = 1\n")?;
        write_file(&root.join("main.rs"), "fn main() {}\n")?;
        write_file(&root.join("README.md"), "# mixed\n")?;
        let discovery = discover_repo_working_set(&root, 800);
        assert_eq!(discovery.counts.discovered, 1);
        assert_eq!(selected_paths(&discovery), vec!["app.py"]);
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn discovery_counts_reconcile_across_roles() -> Result<(), String> {
        let root = unique_test_root("reconcile");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        write_file(&root.join("app.py"), "VALUE = 1\n")?;
        write_file(
            &root.join("tests/test_app.py"),
            "def test_it():\n    pass\n",
        )?;
        write_file(&root.join("conftest.py"), "import pytest\n")?;
        write_file(&root.join("gen_pb2.py"), "# generated\n")?;
        write_file(&root.join("vendor/dep.py"), "VALUE = 2\n")?;
        let discovery = discover_repo_working_set(&root, 3);
        let counts = discovery.counts;
        assert_eq!(counts.discovered, 5);
        assert_eq!(counts.excluded_by_role, 2);
        assert_eq!(counts.selected, 3);
        assert_eq!(counts.capped, 0);
        assert_eq!(counts.skipped, counts.excluded_by_role + counts.capped);
        assert_eq!(counts.discovered, counts.selected + counts.skipped);
        assert_eq!(counts.analyzed_candidates, 1);
        let roles: Vec<PythonFileRole> = discovery.selected.iter().map(|(_, role)| *role).collect();
        assert!(roles.contains(&PythonFileRole::Production));
        assert!(roles.contains(&PythonFileRole::PhysicalTest));
        assert!(roles.contains(&PythonFileRole::InlineHelper));
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }
}
