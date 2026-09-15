use super::super::AnalysisMode;
use super::classify::package_root;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Depth bound for the module-context closure. Real parent chains nest a
/// handful of levels; anything past this bound stops pulling parents and
/// fails closed (their children keep their standalone roles).
const MAX_MODULE_CONTEXT_DEPTH: usize = 32;

/// Pre-#2970 selection entry, kept for the selection test matrix: identical
/// to [`select_rust_files_for_mode_with_dependent_packages`] with no
/// dependent roots. The workspace module re-exports only the expansion-aware
/// form, so this wrapper exists under `cfg(test)` alone.
#[cfg(test)]
fn select_rust_files_for_mode(
    all_files: &[PathBuf],
    changed_rust_files: &[PathBuf],
    mode: AnalysisMode,
    include_unchanged_tests: bool,
) -> Vec<PathBuf> {
    select_rust_files_for_mode_with_dependent_packages(
        all_files,
        changed_rust_files,
        mode,
        include_unchanged_tests,
        &BTreeSet::new(),
        &[],
    )
}

/// The package-narrowing selection with the reverse-dependency expansion of
/// the package scope (#2970 slice C): `dependent_package_roots` carries the
/// package roots that reach a changed package through path dependencies, so
/// the Draft/Fast package narrowing also indexes the tests of dependent
/// crates instead of dropping them from scope. Expansion only ever adds
/// packages; Instant's changed-files-only and Deep/Ready's whole-workspace
/// selections ignore it, as does every branch without package narrowing.
pub(crate) fn select_rust_files_for_mode_with_dependent_packages(
    all_files: &[PathBuf],
    changed_rust_files: &[PathBuf],
    mode: AnalysisMode,
    include_unchanged_tests: bool,
    dependent_package_roots: &BTreeSet<String>,
    manifest_dir_prefixes: &[String],
) -> Vec<PathBuf> {
    let changed_existing = changed_existing_files(all_files, changed_rust_files);
    if matches!(mode, AnalysisMode::Instant) || !include_unchanged_tests {
        return with_module_context_files(all_files, changed_existing);
    }

    if matches!(mode, AnalysisMode::Deep | AnalysisMode::Ready) {
        return sorted_unique(all_files.iter().cloned());
    }

    let package_roots = changed_rust_files
        .iter()
        .filter_map(|path| package_root(path))
        .collect::<Vec<_>>();
    // #3616 review: custom-target changed files can carry no heuristic root
    // while the expansion still attributed dependent packages, so the
    // dependent roots alone must keep the package narrowing alive. With
    // neither set the fallback to changed files only stands.
    if package_roots.is_empty() && dependent_package_roots.is_empty() {
        return with_module_context_files(all_files, changed_existing);
    }

    let package_files = all_files.iter().filter(|file| {
        if let Some(root) = package_root(file) {
            package_roots.iter().any(|changed| changed == &root)
                || dependent_package_roots.contains(&root)
        } else {
            // #3616 review: a file with no heuristic root can still belong
            // to an expanded dependent package (custom Cargo target paths
            // such as `b/lib/core.rs`). Ownership is the nearest discovered
            // manifest directory — the same longest-prefix rule the
            // expansion seeding used — so a nested crate inside a dependent
            // does not leak into scope unless the nested manifest is itself
            // a dependent (#3616 review, second round).
            let normalized = file.to_string_lossy().replace('\\', "/");
            let Some(owner) = manifest_dir_prefixes
                .iter()
                .filter(|prefix| normalized.starts_with(prefix.as_str()))
                .max_by_key(|prefix| prefix.len())
            else {
                return false;
            };
            dependent_package_roots.contains(owner)
        }
    });
    with_module_context_files(
        all_files,
        sorted_unique(package_files.cloned().chain(changed_existing)),
    )
}

/// Extends a narrowed selection with the default-layout module parents of
/// the selected files (#3533): role composition derives a child file's
/// context from the declaration in its parent, so a changed-files-only index
/// that omits an unchanged parent leaves test-only helpers as production
/// subjects. Candidates are plain members of the workspace list — no disk
/// access, deterministic, depth-bounded — and files already selected are
/// never duplicated. `#[path]` redirections from files outside the default
/// layouts stay invisible in narrowed modes (fail closed: no composed role).
fn with_module_context_files(all_files: &[PathBuf], selected: Vec<PathBuf>) -> Vec<PathBuf> {
    if selected.len() >= all_files.len() {
        return selected;
    }
    let known: BTreeSet<&PathBuf> = all_files.iter().collect();
    let mut result: BTreeSet<PathBuf> = selected.into_iter().collect();
    let mut frontier: Vec<PathBuf> = result.iter().cloned().collect();
    for _ in 0..MAX_MODULE_CONTEXT_DEPTH {
        if frontier.is_empty() {
            break;
        }
        let mut next = Vec::new();
        for file in &frontier {
            for candidate in module_parent_candidates(file) {
                if result.contains(&candidate) || !known.contains(&candidate) {
                    continue;
                }
                result.insert(candidate.clone());
                next.push(candidate);
            }
        }
        frontier = next;
    }
    result.into_iter().collect()
}

/// The default-layout files that can declare `mod <name>;` resolving to
/// `file` (#3533). Default resolution looks for `<module-dir>/<name>.rs` and
/// `<module-dir>/<name>/mod.rs` under the declaring file's module directory,
/// so the candidate declaring files are the module-directory files of
/// `file`'s own directory plus the sibling stem file:
/// `src/foo/tests.rs` is declared by `src/foo/mod.rs`, `src/foo/lib.rs`,
/// `src/foo/main.rs`, or `src/foo.rs`; a `mod.rs` child
/// (`src/foo/tests/mod.rs`) is declared from the parent directory instead.
fn module_parent_candidates(file: &Path) -> Vec<PathBuf> {
    let file_name = file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let Some(directory) = file
        .parent()
        .filter(|directory| !directory.as_os_str().is_empty())
    else {
        return Vec::new();
    };
    // Candidates are built by string join, not `Path::join`, so the
    // constructed identities keep the workspace list's forward-slash form
    // and compare equal to its members on every platform.
    let join =
        |directory: &Path, name: &str| PathBuf::from(format!("{}/{}", directory.display(), name));
    let declaring_directory = if file_name == "mod.rs" {
        match directory
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            Some(parent) => parent.to_path_buf(),
            None => return Vec::new(),
        }
    } else {
        directory.to_path_buf()
    };
    let mut candidates = vec![
        join(&declaring_directory, "mod.rs"),
        join(&declaring_directory, "lib.rs"),
        join(&declaring_directory, "main.rs"),
    ];
    if let Some(stem) = declaring_directory.file_stem()
        && let Some(parent) = declaring_directory
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
    {
        candidates.push(join(parent, &format!("{}.rs", stem.to_string_lossy())));
    }
    candidates
}

fn changed_existing_files(all_files: &[PathBuf], changed_rust_files: &[PathBuf]) -> Vec<PathBuf> {
    sorted_unique(
        changed_rust_files
            .iter()
            .filter(|changed| all_files.iter().any(|file| file == *changed))
            .cloned(),
    )
}

fn sorted_unique(files: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut out = files.into_iter().collect::<Vec<_>>();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::workspace::discover_rust_files;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn files(paths: &[&str]) -> Vec<PathBuf> {
        paths.iter().map(PathBuf::from).collect()
    }

    fn temp_dir(name: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before unix epoch: {err}"))?
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ripr-workspace-{name}-{stamp}"));
        fs::create_dir_all(&dir).map_err(|err| format!("create temp dir failed: {err}"))?;
        Ok(dir)
    }

    #[test]
    fn instant_indexes_changed_rust_files_only() {
        let all = files(&["src/lib.rs", "tests/pricing.rs", "crates/other/src/lib.rs"]);
        let selected =
            select_rust_files_for_mode(&all, &files(&["src/lib.rs"]), AnalysisMode::Instant, true);
        assert_eq!(selected, files(&["src/lib.rs"]));
    }

    /// A narrowed selection must bring the unchanged default-layout parents
    /// of changed module children (#3533): composition derives a child's
    /// context from its parent declaration, so a changed-files-only index
    /// would leave test-only helpers as production subjects.
    #[test]
    fn narrowed_selection_brings_out_of_line_module_parents() {
        let all = files(&["src/lib.rs", "src/foo.rs", "src/foo/tests.rs"]);
        let expected_identity = |mut expected: Vec<PathBuf>| {
            expected.sort();
            expected
        };
        let selected = select_rust_files_for_mode(
            &all,
            &files(&["src/foo/tests.rs"]),
            AnalysisMode::Instant,
            true,
        );
        assert_eq!(
            selected,
            expected_identity(files(&["src/foo.rs", "src/foo/tests.rs", "src/lib.rs"])),
            "the child's stem-directory parent and the workspace root arrive with it"
        );
        // include_unchanged_tests=false narrows Deep the same way.
        assert_eq!(
            select_rust_files_for_mode(
                &all,
                &files(&["src/foo/tests.rs"]),
                AnalysisMode::Deep,
                false
            ),
            expected_identity(files(&["src/foo.rs", "src/foo/tests.rs", "src/lib.rs"]))
        );
    }

    /// The common sibling include shape: a changed fragment's compilation
    /// unit in the same directory is pulled in, so the include edge resolves
    /// and the fragment inherits its context.
    #[test]
    fn narrowed_selection_brings_the_include_unit_for_sibling_fragments() {
        let all = files(&["src/lib.rs", "src/fragment.rs"]);
        let selected = select_rust_files_for_mode(
            &all,
            &files(&["src/fragment.rs"]),
            AnalysisMode::Instant,
            true,
        );
        let mut expected = files(&["src/fragment.rs", "src/lib.rs"]);
        expected.sort();
        assert_eq!(selected, expected);
    }

    /// A `mod.rs` child's declaring files live in the parent directory.
    #[test]
    fn narrowed_selection_brings_mod_rs_children_parents() {
        let all = files(&["src/foo.rs", "src/foo/tests/mod.rs"]);
        let selected = select_rust_files_for_mode(
            &all,
            &files(&["src/foo/tests/mod.rs"]),
            AnalysisMode::Instant,
            true,
        );
        let mut expected = files(&["src/foo.rs", "src/foo/tests/mod.rs"]);
        expected.sort();
        assert_eq!(selected, expected);
    }

    /// The closure adds only real candidates: a child whose default-layout
    /// parents do not exist in the workspace pulls nothing extra in.
    #[test]
    fn narrowed_selection_closure_adds_only_existing_candidates() {
        let all = files(&["crates/alpha/src/lib.rs", "crates/alpha/src/foo/tests.rs"]);
        let selected = select_rust_files_for_mode(
            &all,
            &files(&["crates/alpha/src/foo/tests.rs"]),
            AnalysisMode::Instant,
            true,
        );
        assert_eq!(
            selected,
            files(&["crates/alpha/src/foo/tests.rs"]),
            "crates/alpha/src/foo.rs is absent from the workspace list, so nothing is added"
        );
    }

    #[test]
    fn draft_and_fast_index_changed_package_files() {
        let all = files(&[
            "crates/pricing/src/lib.rs",
            "crates/pricing/tests/pricing.rs",
            "crates/risk/src/lib.rs",
            "crates/risk/tests/risk.rs",
        ]);
        let changed = files(&["crates/pricing/src/lib.rs"]);

        for mode in [AnalysisMode::Draft, AnalysisMode::Fast] {
            let selected = select_rust_files_for_mode(&all, &changed, mode, true);
            assert_eq!(
                selected,
                files(&[
                    "crates/pricing/src/lib.rs",
                    "crates/pricing/tests/pricing.rs"
                ])
            );
        }
    }

    #[test]
    fn deep_and_ready_index_entire_workspace() {
        let all = files(&["src/lib.rs", "tests/pricing.rs", "crates/other/src/lib.rs"]);
        let changed = files(&["src/lib.rs"]);

        for mode in [AnalysisMode::Deep, AnalysisMode::Ready] {
            let selected = select_rust_files_for_mode(&all, &changed, mode, true);
            assert_eq!(
                selected,
                files(&["crates/other/src/lib.rs", "src/lib.rs", "tests/pricing.rs"])
            );
        }
    }

    #[test]
    fn operator_mode_tiers_are_pinned_for_defaults_first_adoption() {
        let all = files(&[
            "crates/pricing/src/lib.rs",
            "crates/pricing/tests/pricing.rs",
            "crates/risk/src/lib.rs",
            "crates/risk/tests/risk.rs",
        ]);
        let changed = files(&["crates/pricing/src/lib.rs"]);

        assert_eq!(
            select_rust_files_for_mode(&all, &changed, AnalysisMode::Instant, true),
            files(&["crates/pricing/src/lib.rs"])
        );
        assert_eq!(
            select_rust_files_for_mode(&all, &changed, AnalysisMode::Draft, true),
            files(&[
                "crates/pricing/src/lib.rs",
                "crates/pricing/tests/pricing.rs"
            ])
        );
        assert_eq!(
            select_rust_files_for_mode(&all, &changed, AnalysisMode::Fast, true),
            files(&[
                "crates/pricing/src/lib.rs",
                "crates/pricing/tests/pricing.rs"
            ])
        );
        assert_eq!(
            select_rust_files_for_mode(&all, &changed, AnalysisMode::Deep, true),
            files(&[
                "crates/pricing/src/lib.rs",
                "crates/pricing/tests/pricing.rs",
                "crates/risk/src/lib.rs",
                "crates/risk/tests/risk.rs"
            ])
        );
        assert_eq!(
            select_rust_files_for_mode(&all, &changed, AnalysisMode::Ready, true),
            files(&[
                "crates/pricing/src/lib.rs",
                "crates/pricing/tests/pricing.rs",
                "crates/risk/src/lib.rs",
                "crates/risk/tests/risk.rs"
            ])
        );
    }

    #[test]
    fn source_role_excludes_xtask_automation() {
        assert!(
            !crate::analysis::workspace::classify_with(
                Path::new("xtask/src/main.rs"),
                &crate::analysis::workspace::SourceRoleContext::empty(),
            )
            .seeds_production_findings()
        );
        assert!(
            crate::analysis::workspace::classify_with(
                Path::new("crates/ripr/src/lib.rs"),
                &crate::analysis::workspace::SourceRoleContext::empty(),
            )
            .seeds_production_findings()
        );
    }

    #[test]
    fn repo_discovery_skips_fixture_tree_but_fixture_roots_still_work() -> Result<(), String> {
        let root = temp_dir("fixtures")?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create root src failed: {err}"))?;
        fs::create_dir_all(root.join("fixtures/boundary/input/src"))
            .map_err(|err| format!("create fixture src failed: {err}"))?;
        fs::write(root.join("src/lib.rs"), "")
            .map_err(|err| format!("write root src failed: {err}"))?;
        fs::write(root.join("fixtures/boundary/input/src/lib.rs"), "")
            .map_err(|err| format!("write fixture src failed: {err}"))?;

        assert_eq!(discover_rust_files(&root)?, files(&["src/lib.rs"]));
        assert_eq!(
            discover_rust_files(&root.join("fixtures/boundary/input"))?,
            files(&["src/lib.rs"])
        );
        Ok(())
    }

    #[test]
    fn no_unchanged_tests_limits_any_mode_to_changed_files() {
        let all = files(&["src/lib.rs", "tests/pricing.rs"]);
        let selected =
            select_rust_files_for_mode(&all, &files(&["src/lib.rs"]), AnalysisMode::Deep, false);
        assert_eq!(selected, files(&["src/lib.rs"]));
    }

    #[test]
    fn draft_and_fast_selection_is_stable_and_subset_of_workspace() {
        let corpus = [
            "src/lib.rs",
            "src/main.rs",
            "tests/root.rs",
            "examples/root.rs",
            "benches/root.rs",
            "crates/alpha/src/lib.rs",
            "crates/alpha/tests/alpha.rs",
            "crates/beta/src/lib.rs",
            "crates/beta/tests/beta.rs",
            "crates/gamma/src/lib.rs",
            "crates/gamma/tests/gamma.rs",
            "tools/helper/src/lib.rs",
            "tools/helper/tests/helper.rs",
        ];
        let all = files(&corpus);

        let mut seed = 0x5EED_u64;
        for _case in 0..256 {
            let mut changed = Vec::new();
            for path in &all {
                if next_u64(&mut seed) & 1 == 0 {
                    changed.push(path.clone());
                }
            }

            for mode in [AnalysisMode::Draft, AnalysisMode::Fast] {
                let selected = select_rust_files_for_mode(&all, &changed, mode, true);

                assert!(selected.windows(2).all(|w| w[0] < w[1]));
                assert!(selected.iter().all(|path| all.contains(path)));
                assert!(
                    changed
                        .iter()
                        .filter(|path| all.contains(path))
                        .all(|path| selected.contains(path))
                );
            }
        }
    }

    fn next_u64(seed: &mut u64) -> u64 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *seed
    }

    /// #2970 slice C: the a <- b <- c path-dep chain plus unrelated d. The
    /// dependent roots enter the Draft/Fast package selection, `d` stays out,
    /// and the direction is discriminated: expansion from `a` (nothing
    /// depends on `a` in the forward direction) is what brings `b`/`c` in, so
    /// a swapped forward/reverse adjacency — whose expansion of `a` is empty
    /// — loses the dependent files.
    #[test]
    fn dependent_packages_enter_draft_and_fast_selection() {
        let all = files(&[
            "a/src/lib.rs",
            "b/src/lib.rs",
            "b/tests/behavior.rs",
            "c/src/lib.rs",
            "d/src/lib.rs",
            "d/tests/d.rs",
        ]);
        let changed = files(&["a/src/lib.rs"]);
        let dependents = ["b/".to_string(), "c/".to_string()]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        let prefixes = ["", "a/", "b/", "c/", "d/"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();

        for mode in [AnalysisMode::Draft, AnalysisMode::Fast] {
            let selected = select_rust_files_for_mode_with_dependent_packages(
                &all,
                &changed,
                mode,
                true,
                &dependents,
                &prefixes,
            );
            assert_eq!(
                selected,
                files(&[
                    "a/src/lib.rs",
                    "b/src/lib.rs",
                    "b/tests/behavior.rs",
                    "c/src/lib.rs"
                ]),
                "{mode:?}: dependent crates b and c enter scope, unrelated d stays out"
            );
        }
    }

    /// Empty dependent roots reproduce the unchanged package selection
    /// exactly: expansion only ever adds, never narrows.
    #[test]
    fn empty_dependent_roots_reproduce_the_unchanged_selection() {
        let all = files(&["a/src/lib.rs", "b/src/lib.rs", "b/tests/b.rs"]);
        let changed = files(&["a/src/lib.rs"]);
        let selected = select_rust_files_for_mode_with_dependent_packages(
            &all,
            &changed,
            AnalysisMode::Draft,
            true,
            &std::collections::BTreeSet::new(),
            &[],
        );
        assert_eq!(selected, files(&["a/src/lib.rs"]));
    }

    /// #3616 review: a dependent crate's custom-target file carries no
    /// heuristic package root (`b/lib/core.rs` matches no `src/`/`tests/`
    /// pattern), so heuristic attribution alone would exclude it even
    /// though `b` was attributed as a dependent. Prefix membership against
    /// the graph-attributed dependent roots keeps such files indexed.
    #[test]
    fn dependent_custom_target_files_enter_selection_by_root_prefix() {
        let all = files(&[
            "a/src/lib.rs",
            "b/lib/core.rs",
            "b/src/lib.rs",
            "d/src/lib.rs",
        ]);
        let changed = files(&["a/src/lib.rs"]);
        let dependents = ["b/".to_string()]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        let prefixes = ["", "a/", "b/", "d/"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();

        let selected = select_rust_files_for_mode_with_dependent_packages(
            &all,
            &changed,
            AnalysisMode::Draft,
            true,
            &dependents,
            &prefixes,
        );
        assert_eq!(
            selected,
            files(&["a/src/lib.rs", "b/lib/core.rs", "b/src/lib.rs"]),
            "the dependent's custom-target file enters by root prefix; unrelated d stays out"
        );
    }

    /// #3616 review, second round: a nested crate inside a dependent must
    /// not leak into scope through the bare prefix rule — ownership of an
    /// unattributed file is its nearest manifest, so the nested crate's
    /// custom-target files stay out unless the nested manifest is itself a
    /// dependent.
    #[test]
    fn nested_crate_inside_dependent_stays_out_unless_itself_dependent() {
        let all = files(&[
            "a/src/lib.rs",
            "b/src/lib.rs",
            "b/lib/core.rs",
            "b/plugins/inner/lib.rs",
            "d/src/lib.rs",
        ]);
        let changed = files(&["a/src/lib.rs"]);
        let dependents = ["b/".to_string()]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        let prefixes = ["", "a/", "b/", "b/plugins/inner/", "d/"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();

        let selected = select_rust_files_for_mode_with_dependent_packages(
            &all,
            &changed,
            AnalysisMode::Draft,
            true,
            &dependents,
            &prefixes,
        );
        assert_eq!(
            selected,
            files(&["a/src/lib.rs", "b/lib/core.rs", "b/src/lib.rs"]),
            "b's own custom target enters; the nested crate's files stay out while              b/plugins/inner is not itself a dependent"
        );
    }

    /// #3616 review: custom-target changed files carry no heuristic package
    /// root, yet the expansion can still attribute dependent packages. The
    /// dependent roots alone must keep the package narrowing alive — the
    /// changed custom-target file enters through the changed-file set and
    /// the dependents' files through their package roots.
    #[test]
    fn dependent_packages_narrow_selection_without_heuristic_changed_roots() {
        let all = files(&["t/lib/core.rs", "u/src/lib.rs", "u/tests/u.rs"]);
        let changed = files(&["t/lib/core.rs"]);
        let dependents = ["u/".to_string()]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();

        let prefixes = ["", "t/", "u/"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        assert_eq!(
            select_rust_files_for_mode_with_dependent_packages(
                &all,
                &changed,
                AnalysisMode::Draft,
                true,
                &dependents,
                &prefixes
            ),
            files(&["t/lib/core.rs", "u/src/lib.rs", "u/tests/u.rs"]),
            "the attributed dependents enter scope although no changed file has a heuristic root"
        );
        assert_eq!(
            select_rust_files_for_mode_with_dependent_packages(
                &all,
                &changed,
                AnalysisMode::Draft,
                true,
                &std::collections::BTreeSet::new(),
                &prefixes
            ),
            files(&["t/lib/core.rs"]),
            "without dependents the selection stays the changed files only"
        );
    }

    /// Modes and toggles without package narrowing ignore the dependent
    /// roots: Instant stays changed-files-only, Deep/Ready stay
    /// whole-workspace, and `include_unchanged_tests = false` stays
    /// changed-files-only in every mode.
    #[test]
    fn dependent_roots_do_not_change_non_narrowing_selections() {
        let all = files(&["a/src/lib.rs", "b/src/lib.rs", "b/tests/b.rs"]);
        let changed = files(&["a/src/lib.rs"]);
        let dependents = ["b/".to_string()]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        let prefixes = ["", "a/", "b/"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();

        assert_eq!(
            select_rust_files_for_mode_with_dependent_packages(
                &all,
                &changed,
                AnalysisMode::Instant,
                true,
                &dependents,
                &prefixes
            ),
            files(&["a/src/lib.rs"]),
            "Instant must stay changed-files-only"
        );
        for mode in [AnalysisMode::Deep, AnalysisMode::Ready] {
            assert_eq!(
                select_rust_files_for_mode_with_dependent_packages(
                    &all,
                    &changed,
                    mode,
                    true,
                    &dependents,
                    &prefixes
                ),
                files(&["a/src/lib.rs", "b/src/lib.rs", "b/tests/b.rs"]),
                "{mode:?} already spans the workspace"
            );
        }
        for mode in [
            AnalysisMode::Instant,
            AnalysisMode::Draft,
            AnalysisMode::Fast,
            AnalysisMode::Deep,
            AnalysisMode::Ready,
        ] {
            assert_eq!(
                select_rust_files_for_mode_with_dependent_packages(
                    &all,
                    &changed,
                    mode,
                    false,
                    &dependents,
                    &prefixes
                ),
                files(&["a/src/lib.rs"]),
                "{mode:?} with include_unchanged_tests=false must stay changed-files-only"
            );
        }
    }

    /// #3705 / RIPR-SPEC-0174: the package-together baseline. A binary-only
    /// change in a multi-target package keeps lib, integration tests,
    /// examples, and benches together under Draft/Fast. A future narrowing
    /// may omit one of these only with closure evidence; the binary label
    /// alone is not such evidence, so removing package narrowing here must
    /// fail this test (the Instant control below shows the edge is
    /// load-bearing).
    #[test]
    fn draft_and_fast_keep_workspace_root_multi_target_package_together() {
        let all = files(&[
            "src/lib.rs",
            "src/main.rs",
            "tests/it.rs",
            "examples/ex.rs",
            "benches/b.rs",
            "crates/other/src/lib.rs",
        ]);
        let changed = files(&["src/main.rs"]);

        for mode in [AnalysisMode::Draft, AnalysisMode::Fast] {
            assert_eq!(
                select_rust_files_for_mode(&all, &changed, mode, true),
                files(&[
                    "benches/b.rs",
                    "examples/ex.rs",
                    "src/lib.rs",
                    "src/main.rs",
                    "tests/it.rs",
                ]),
                "{mode:?}: binary-only change keeps the whole multi-target package"
            );
        }
    }

    /// #3705 / RIPR-SPEC-0174: per-crate separation for nested multi-target
    /// crates. A binary-only change in `crates/app` keeps app's lib and
    /// integration tests but excludes `crates/other`.
    #[test]
    fn draft_and_fast_keep_nested_multi_target_crate_together() {
        let all = files(&[
            "crates/app/src/lib.rs",
            "crates/app/src/main.rs",
            "crates/app/tests/app.rs",
            "crates/other/src/lib.rs",
            "crates/other/src/main.rs",
            "crates/other/tests/other.rs",
        ]);
        let changed = files(&["crates/app/src/main.rs"]);

        for mode in [AnalysisMode::Draft, AnalysisMode::Fast] {
            assert_eq!(
                select_rust_files_for_mode(&all, &changed, mode, true),
                files(&[
                    "crates/app/src/lib.rs",
                    "crates/app/src/main.rs",
                    "crates/app/tests/app.rs",
                ]),
                "{mode:?}: app stays together, other stays out"
            );
        }
    }

    /// #3705 / RIPR-SPEC-0174: the load-bearing control. The
    /// changed-files-plus-parents path (Instant) omits `tests/it.rs` for
    /// the same binary-only change, proving the Draft/Fast package edge —
    /// not proximity or the binary label — carries the integration test.
    #[test]
    fn instant_selection_leaves_integration_tests_behind_for_bin_only_change() {
        let all = files(&[
            "src/lib.rs",
            "src/main.rs",
            "tests/it.rs",
            "examples/ex.rs",
            "benches/b.rs",
        ]);
        let selected =
            select_rust_files_for_mode(&all, &files(&["src/main.rs"]), AnalysisMode::Instant, true);
        assert_eq!(selected, files(&["src/lib.rs", "src/main.rs"]));
        assert!(
            !selected.contains(&PathBuf::from("tests/it.rs")),
            "without the package edge the integration test is lost"
        );
    }

    /// #3705 / RIPR-SPEC-0174: the module-parent edge for the binary shape,
    /// pinned directly. `src/lib.rs` is the declaring sibling of
    /// `src/main.rs`; removing the closure from the pipeline must fail the
    /// Instant pipeline tests and this direct pin.
    #[test]
    fn module_closure_brings_lib_sibling_for_bin_only_change() {
        let all = files(&["src/lib.rs", "src/main.rs", "tests/it.rs"]);
        assert_eq!(
            with_module_context_files(&all, files(&["src/main.rs"])),
            files(&["src/lib.rs", "src/main.rs"]),
            "the closure — not package narrowing — brings the lib sibling"
        );
    }

    /// #3705 / RIPR-SPEC-0174: honest custom-path fallback. A changed
    /// custom-target file with no heuristic root and no attributed
    /// dependents selects changed files only — the sibling integration
    /// test stays out until dependents attribute the package. That
    /// omission is the current limitation, not an exclusion license.
    #[test]
    fn custom_target_change_without_dependents_stays_changed_files_only() {
        let all = files(&["pkg/lib/core.rs", "pkg/tests/core.rs"]);
        let changed = files(&["pkg/lib/core.rs"]);
        let prefixes = ["pkg/".to_string()];

        assert_eq!(
            select_rust_files_for_mode_with_dependent_packages(
                &all,
                &changed,
                AnalysisMode::Draft,
                true,
                &std::collections::BTreeSet::new(),
                &prefixes
            ),
            files(&["pkg/lib/core.rs"]),
            "no heuristic root and no dependents: changed files only"
        );
        let dependents = ["pkg/".to_string()]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            select_rust_files_for_mode_with_dependent_packages(
                &all,
                &changed,
                AnalysisMode::Draft,
                true,
                &dependents,
                &prefixes
            ),
            files(&["pkg/lib/core.rs", "pkg/tests/core.rs"]),
            "attributed dependents bring the package's tests back in"
        );
    }
}
