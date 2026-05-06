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
    use crate::analysis::workspace::{discover_rust_files, is_production_rust_path};
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
    fn production_path_excludes_xtask_automation() {
        assert!(!is_production_rust_path(Path::new("xtask/src/main.rs")));
        assert!(is_production_rust_path(Path::new("crates/ripr/src/lib.rs")));
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
