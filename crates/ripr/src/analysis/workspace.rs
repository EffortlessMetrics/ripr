use super::AnalysisMode;
use std::path::{Path, PathBuf};

pub fn discover_rust_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    visit(root, root, &mut out)?;
    out.sort();
    Ok(out)
}

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

fn package_root(path: &Path) -> Option<String> {
    let normalized = normalize_path(path);
    if normalized.starts_with("src/")
        || normalized.starts_with("tests/")
        || normalized.starts_with("examples/")
        || normalized.starts_with("benches/")
    {
        return Some(String::new());
    }
    if let Some(rest) = normalized.strip_prefix("crates/")
        && let Some((crate_name, crate_relative)) = rest.split_once('/')
        && (crate_relative.starts_with("src/") || crate_relative.starts_with("tests/"))
    {
        return Some(format!("crates/{crate_name}/"));
    }
    for marker in ["/src/", "/tests/", "/examples/", "/benches/"] {
        if let Some(idx) = normalized.rfind(marker) {
            let prefix = &normalized[..idx];
            if !prefix.is_empty() {
                return Some(format!("{prefix}/"));
            }
        }
    }
    None
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

fn visit(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|err| format!("failed to read {}: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("failed to read dir entry: {err}"))?;
        let path = entry.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if matches!(
                name,
                ".git" | "target" | ".ripr" | ".direnv" | "node_modules"
            ) {
                continue;
            }
            visit(root, &path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            out.push(relative);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

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

    proptest! {
        #[test]
        fn changed_existing_files_is_a_sorted_unique_subset(
            all in prop::collection::vec("[a-z]{1,8}/[a-z]{1,8}\\.rs", 0..40),
            changed in prop::collection::vec("[a-z]{1,8}/[a-z]{1,8}\\.rs", 0..40),
        ) {
            let all_files = all.iter().map(PathBuf::from).collect::<Vec<_>>();
            let changed_files = changed.iter().map(PathBuf::from).collect::<Vec<_>>();

            let selected = changed_existing_files(&all_files, &changed_files);

            prop_assert!(selected.windows(2).all(|pair| pair[0] < pair[1]));
            prop_assert!(selected.iter().all(|entry| all_files.iter().any(|file| file == entry)));
            prop_assert!(selected.iter().all(|entry| changed_files.iter().any(|file| file == entry)));
        }

        #[test]
        fn deep_and_ready_with_unchanged_tests_matches_all_files(
            all in prop::collection::vec(
                prop_oneof![
                    Just("src/lib.rs".to_string()),
                    Just("tests/smoke.rs".to_string()),
                    Just("crates/app/src/lib.rs".to_string()),
                    Just("crates/app/tests/smoke.rs".to_string()),
                    Just("crates/core/src/lib.rs".to_string()),
                    Just("crates/core/tests/smoke.rs".to_string()),
                ],
                0..40
            ),
            changed in prop::collection::vec(
                prop_oneof![
                    Just("src/lib.rs".to_string()),
                    Just("tests/smoke.rs".to_string()),
                    Just("crates/app/src/lib.rs".to_string()),
                    Just("crates/app/tests/smoke.rs".to_string()),
                    Just("crates/core/src/lib.rs".to_string()),
                    Just("crates/core/tests/smoke.rs".to_string()),
                ],
                0..40
            ),
        ) {
            let all_files = all.iter().map(PathBuf::from).collect::<Vec<_>>();
            let changed_files = changed.iter().map(PathBuf::from).collect::<Vec<_>>();
            let expected = sorted_unique(all_files.clone());

            let deep_selected =
                select_rust_files_for_mode(&all_files, &changed_files, AnalysisMode::Deep, true);
            let ready_selected =
                select_rust_files_for_mode(&all_files, &changed_files, AnalysisMode::Ready, true);

            prop_assert_eq!(deep_selected, expected.clone());
            prop_assert_eq!(ready_selected, expected);
        }
    }
}
