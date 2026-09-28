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
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
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
            Err(_) => continue,
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
