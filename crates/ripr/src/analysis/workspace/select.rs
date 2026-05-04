use super::super::AnalysisMode;
use super::classify::package_root;
use std::path::PathBuf;

pub fn select_rust_files_for_mode(
    all_files: &[PathBuf],
    changed_rust_files: &[PathBuf],
    mode: AnalysisMode,
    include_unchanged_tests: bool,
) -> Vec<PathBuf> {
    let changed_existing = changed_existing_files(all_files, changed_rust_files);
    if matches!(mode, AnalysisMode::Instant) || !include_unchanged_tests {
        return changed_existing;
    }

    if matches!(mode, AnalysisMode::Deep | AnalysisMode::Ready) {
        return sorted_unique(all_files.iter().cloned());
    }

    let package_roots = changed_rust_files
        .iter()
        .filter_map(|path| package_root(path))
        .collect::<Vec<_>>();
    if package_roots.is_empty() {
        return changed_existing;
    }

    let package_files = all_files.iter().filter(|file| {
        package_root(file)
            .as_ref()
            .is_some_and(|root| package_roots.iter().any(|changed| changed == root))
    });
    sorted_unique(package_files.cloned().chain(changed_existing))
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

    fn files(paths: &[&str]) -> Vec<PathBuf> {
        paths.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn instant_indexes_changed_rust_files_only() {
        let all = files(&["src/lib.rs", "tests/pricing.rs", "crates/other/src/lib.rs"]);
        let selected =
            select_rust_files_for_mode(&all, &files(&["src/lib.rs"]), AnalysisMode::Instant, true);
        assert_eq!(selected, files(&["src/lib.rs"]));
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
}
