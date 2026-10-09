use crate::analysis::committed_source::frozen::fs as frozen_fs;
use super::source_utils::normalized_path;
use super::{ChangedFile, LanguageAdapter, PythonAdapter, PythonOwner};
use crate::config::{is_detectable_generated_python_path, is_python_excluded_dir_everywhere};
use std::{
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

pub(super) fn owner_for_changed_line<'a>(
    file: &Path,
    line: usize,
    owners: &'a [PythonOwner],
) -> Option<&'a PythonOwner> {
    let changed_file = normalized_path(file);
    owners
        .iter()
        .filter(|owner| normalized_path(&owner.file) == changed_file)
        .filter(|owner| line >= owner.start_line && line <= owner.end_line)
        .min_by_key(|owner| (owner.span_width(), owner.specificity_rank()))
}

pub(super) fn collect_workspace_python_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    visit_workspace(root, root, &mut out);
    out.sort();
    out
}

pub(super) fn visit_workspace(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = frozen_fs::read_dir(dir) else {
        #[cfg(test)]
        if crate::analysis::source_calibration::active() {
            crate::analysis::source_calibration::read_attempt("python_discovery_io");
        }
        return;
    };
    for entry in entries.filter_map(|entry| {
        #[cfg(test)]
        if entry.is_err() && crate::analysis::source_calibration::active() {
            crate::analysis::source_calibration::read_attempt("python_discovery_io");
        }
        entry.ok()
    }) {
        #[cfg(test)]
        if !crate::analysis::source_calibration::continue_walk() {
            break;
        }
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        // Diff discovery prunes the config-owned excluded-directory
        // authority, including `vendor` (#3672): vendored Python is not
        // project or production source, so it never enters the diff-mode
        // working set.
        if is_python_excluded_dir_everywhere(name) {
            continue;
        }
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => {
                #[cfg(test)]
                if crate::analysis::source_calibration::active() {
                    crate::analysis::source_calibration::read_attempt("python_discovery_io");
                }
                continue;
            }
        };
        if file_type.is_dir() {
            visit_workspace(root, &path, out);
        } else if file_type.is_file() {
            let adapter = PythonAdapter;
            if adapter.accepts_path(&path) && !is_detectable_generated_python_path(&path) {
                let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                out.push(relative);
            }
        }
    }
}

/// Reconstructs the old side of one changed file from the current source and
/// the parsed unified-diff line coordinates. This keeps no-op classification
/// fail-closed: an interior docstring line is suppressed only when both parsed
/// source versions establish that it belongs to a docstring.
pub(super) fn reconstruct_old_source(new_source: &str, changed: &ChangedFile) -> Option<String> {
    let mut lines = new_source.lines().map(str::to_string).collect::<Vec<_>>();

    let mut added = changed.added_lines.iter().collect::<Vec<_>>();
    added.sort_by_key(|line| std::cmp::Reverse(line.line));
    for line in added {
        let index = line.line.checked_sub(1)?;
        if lines.get(index)? != &line.text {
            return None;
        }
        lines.remove(index);
    }

    let mut removed = changed.removed_lines.iter().collect::<Vec<_>>();
    removed.sort_by_key(|line| line.line);
    for line in removed {
        let index = line.line.checked_sub(1)?;
        if index > lines.len() {
            return None;
        }
        lines.insert(index, line.text.clone());
    }

    Some(lines.join("\n"))
}

pub(super) fn line_is_in_ranges(line: usize, ranges: &[RangeInclusive<usize>]) -> bool {
    ranges.iter().any(|range| range.contains(&line))
}

#[cfg(test)]
#[test]
fn frozen_discovery_keeps_snapshot_membership_and_logical_paths() -> Result<(), String> {
    use crate::analysis::committed_source::frozen;
    use crate::analysis::git_candidate_execution::prepare_named_tree;
    use crate::analysis::source_calibration::OwnedFixture;
    use crate::testing::fixture_git::fixture_git_ok;

    let fixture = OwnedFixture::new()?;
    let snapshot_path = std::path::PathBuf::from("src/snapshot.py");
    let live_path = std::path::PathBuf::from("src/live.py");
    fixture.seed("src/snapshot.py", b"snapshot = 1\n")?;
    fixture_git_ok(&fixture.root, &["init", "--initial-branch=main"])?;
    fixture_git_ok(&fixture.root, &["add", "."])?;
    fixture_git_ok(
        &fixture.root,
        &[
            "-c",
            "user.name=ripr fixture",
            "-c",
            "user.email=ripr@example.invalid",
            "commit",
            "-qm",
            "frozen discovery",
        ],
    )?;
    let prepared =
        prepare_named_tree(&fixture.root, "HEAD", None).map_err(|error| error.to_string())?;
    let authority = prepared
        .frozen_source_authority(&fixture.root)
        .map_err(|error| error.to_string())?;
    std::fs::remove_file(fixture.root.join(&snapshot_path))
        .map_err(|error| error.to_string())?;
    fixture.seed("src/live.py", b"live = 2\n")?;
    let ordinary = frozen::with_context(None, || {
        collect_workspace_python_files(&fixture.root)
    });
    assert_eq!(ordinary, vec![live_path.clone()]);
    let actual = frozen::with_context(Some(authority.clone()), || {
        collect_workspace_python_files(&fixture.root)
    });
    assert_eq!(actual, vec![snapshot_path]);
    authority.ensure_clean().map_err(|error| error.to_string())?;
    let recovered = frozen::with_context(None, || {
        collect_workspace_python_files(&fixture.root)
    });
    assert_eq!(recovered, vec![live_path]);
    Ok(())
}
