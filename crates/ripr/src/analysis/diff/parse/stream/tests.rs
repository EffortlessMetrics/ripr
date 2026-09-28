use std::cell::Cell;
use std::path::PathBuf;

use super::{ParsedDiff, parse_bounded_lines, parse_unbounded};
use crate::analysis_outcome::AnalysisLimitationKind;

const FIRST_FILE: &str =
    "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n-old\n+new\n";

fn assert_stops_at_header(prefix: &str, limit: usize, observed: usize) -> Result<(), String> {
    let prefix_reads = Cell::new(0);
    let tail_reads = Cell::new(0);
    let header = prefix.lines().inspect(|_| {
        prefix_reads.set(prefix_reads.get() + 1);
    });
    let tail = std::iter::once("@@ -0,0 +1,10000 @@")
        .chain(std::iter::repeat_n("+unread body", 10_000))
        .inspect(|_| {
            tail_reads.set(tail_reads.get() + 1);
        });

    let Err(error) = parse_bounded_lines(header.chain(tail), limit) else {
        return Err("over-limit scope returned a partial successful parse".to_string());
    };
    assert_eq!(prefix_reads.get(), prefix.lines().count());
    assert_eq!(
        tail_reads.get(),
        0,
        "the oversized file body must stay unread"
    );
    assert!(error.starts_with(&format!(
        "diff_scope_oversized: at least {observed} changed files"
    )));
    assert!(error.contains(&format!("limit ({limit})")));
    Ok(())
}

fn assert_matches_unbounded(input: &str, actual: &ParsedDiff) {
    let expected = parse_unbounded(input);
    assert_eq!(actual.changed_files.len(), expected.changed_files.len());
    for (actual, expected) in actual.changed_files.iter().zip(&expected.changed_files) {
        assert_eq!(actual.path, expected.path);
        assert_eq!(actual.added_lines, expected.added_lines);
        assert_eq!(actual.removed_lines, expected.removed_lines);
    }
    assert_eq!(actual.deleted_file_count, expected.deleted_file_count);
    assert_eq!(actual.submodule_file_count, expected.submodule_file_count);
    assert_eq!(actual.renamed_file_count, expected.renamed_file_count);
    assert_eq!(
        actual.pure_rename_file_count,
        expected.pure_rename_file_count
    );
    assert_eq!(actual.pure_rename_paths, expected.pure_rename_paths);
    assert_eq!(actual.limitations, expected.limitations);
}

#[test]
fn oversized_git_diff_stops_before_the_next_hunk_or_long_tail() -> Result<(), String> {
    let prefix =
        format!("{FIRST_FILE}diff --git a/src/b.rs b/src/b.rs\n--- a/src/b.rs\n+++ b/src/b.rs\n");
    assert_stops_at_header(&prefix, 1, 2)
}

#[test]
fn oversized_plain_diff_stops_after_marker_lookahead() -> Result<(), String> {
    let prefix =
        "--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n-old\n+new\n--- a/src/b.rs\n+++ b/src/b.rs\n";
    assert_stops_at_header(prefix, 1, 2)
}

#[test]
fn oversized_pure_rename_stops_without_waiting_for_path_markers() -> Result<(), String> {
    let prefix = format!(
        "{FIRST_FILE}diff --git a/src/old.rs b/src/new.rs\nsimilarity index 100%\nrename from src/old.rs\nrename to src/new.rs\n"
    );
    assert_stops_at_header(&prefix, 1, 2)
}

#[test]
fn exact_limit_preserves_plain_boundaries_and_old_new_coordinates() -> Result<(), String> {
    let input = "--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1,3 @@\n-old\n+one\n+two\n+three\n@@ -5 +7 @@\n-before\n+after\n--- a/src/b.rs\n+++ b/src/b.rs\n@@ -2 +2 @@\n-left\n+right\n";
    let parsed = parse_bounded_lines(input.lines(), 2)?;
    assert_eq!(parsed.changed_files.len(), 2);
    let a = &parsed.changed_files[0];
    let b = &parsed.changed_files[1];
    assert_eq!(a.path, PathBuf::from("src/a.rs"));
    assert_eq!(a.added_lines.len(), 4);
    assert_eq!(a.removed_lines.len(), 2);
    assert_eq!(a.removed_lines[1].line, 5);
    assert_eq!(a.removed_lines[1].new_side_line, 7);
    assert_eq!(a.added_lines[3].line, 7);
    assert_eq!(a.added_lines[3].text, "after");
    assert_eq!(b.path, PathBuf::from("src/b.rs"));
    assert_eq!(b.added_lines.len(), 1);
    assert_eq!(b.removed_lines.len(), 1);
    assert_eq!(b.added_lines[0].text, "right");
    assert_eq!(b.added_lines[0].line, 2);
    assert_matches_unbounded(input, &parsed);
    Ok(())
}

#[test]
fn normalized_duplicates_and_edited_rename_share_one_file_slot() -> Result<(), String> {
    let input = format!(
        "{FIRST_FILE}diff --git a/src/a.rs b/src/a.rs\n--- a/./src/a.rs\n+++ b/./src/a.rs\n@@ -2 +2 @@\n-before\n+after\ndiff --git a/src/old.rs b/src/a.rs\nsimilarity index 80%\nrename from src/old.rs\nrename to src/a.rs\n--- a/src/old.rs\n+++ b/src/a.rs\n@@ -3 +3 @@\n-previous\n+current\n"
    );
    let parsed = parse_bounded_lines(input.lines(), 1)?;
    assert_eq!(parsed.changed_files.len(), 1);
    assert_eq!(parsed.changed_files[0].path, PathBuf::from("src/a.rs"));
    assert_eq!(parsed.changed_files[0].added_lines.len(), 3);
    assert_eq!(parsed.changed_files[0].removed_lines.len(), 3);
    assert_eq!(parsed.changed_files[0].added_lines[2].text, "current");
    assert_eq!(parsed.renamed_file_count, 1);
    assert_eq!(parsed.pure_rename_file_count, 0);
    assert_matches_unbounded(&input, &parsed);
    Ok(())
}

#[test]
fn exact_limit_preserves_rename_deletion_binary_and_submodule_metadata() -> Result<(), String> {
    let input = concat!(
        "diff --git a/src/old.rs b/src/new.rs\nsimilarity index 100%\nrename from src/old.rs\nrename to src/new.rs\n",
        "diff --git a/src/gone.rs b/src/gone.rs\ndeleted file mode 100644\n--- a/src/gone.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n-gone\n",
        "diff --git a/src/blob.bin b/src/blob.bin\nBinary files a/src/blob.bin and /dev/null differ\n",
        "diff --git a/vendor/lib b/vendor/lib\nindex 1111111..2222222 160000\n--- a/vendor/lib\n+++ b/vendor/lib\n@@ -1 +1 @@\n-Subproject commit 1111111\n+Subproject commit 2222222\n",
    );
    let parsed = parse_bounded_lines(input.lines(), 2)?;
    assert_eq!(parsed.changed_files.len(), 2);
    assert_eq!(parsed.deleted_file_count, 2);
    assert_eq!(parsed.submodule_file_count, 1);
    assert_eq!(parsed.renamed_file_count, 1);
    assert_eq!(parsed.pure_rename_file_count, 1);
    assert_eq!(parsed.pure_rename_paths, vec![PathBuf::from("src/new.rs")]);
    assert!(parsed.changed_files[0].added_lines.is_empty());
    assert!(parsed.changed_files[0].removed_lines.is_empty());
    assert_eq!(parsed.changed_files[1].path, PathBuf::from("vendor/lib"));
    assert_eq!(parsed.changed_files[1].added_lines.len(), 1);
    assert_matches_unbounded(input, &parsed);
    Ok(())
}

#[test]
fn bounded_quarantines_preserve_limitations_and_coordinates() -> Result<(), String> {
    let input = concat!(
        "diff --cc src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@@ -1 -1 +1 @@@\n++hidden\n",
        "--- a/src/b.rs\n+++ b/src/b.rs\n@@ -0,0 +1,4 @@\n+<<<<<<< ours\n+hidden\n+>>>>>>> theirs\n+visible\n",
    );
    let parsed = parse_bounded_lines(input.lines(), 2)?;
    assert_eq!(parsed.changed_files.len(), 2);
    assert!(parsed.changed_files[0].added_lines.is_empty());
    assert!(parsed.changed_files[0].removed_lines.is_empty());
    assert_eq!(parsed.changed_files[1].path, PathBuf::from("src/b.rs"));
    assert_eq!(parsed.changed_files[1].added_lines.len(), 1);
    assert_eq!(parsed.changed_files[1].added_lines[0].text, "visible");
    assert_eq!(parsed.changed_files[1].added_lines[0].line, 4);
    assert_eq!(parsed.limitations.len(), 2);
    assert_eq!(
        parsed.limitations[0].kind,
        AnalysisLimitationKind::CombinedHunkUnsupported
    );
    assert_eq!(
        parsed.limitations[1].kind,
        AnalysisLimitationKind::UnresolvedConflictMarkers
    );
    assert_matches_unbounded(input, &parsed);
    Ok(())
}

#[test]
fn zero_limit_counts_only_accepted_paths_and_keeps_deletion_metadata() -> Result<(), String> {
    assert!(parse_bounded_lines("".lines(), 0)?.changed_files.is_empty());
    let input = concat!(
        "diff --git a/src/gone.rs b/src/gone.rs\ndeleted file mode 100644\n--- a/src/gone.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n-gone\n",
        "diff --git a/../escape.rs b/../escape.rs\n--- a/../escape.rs\n+++ b/../escape.rs\n@@ -1 +1 @@\n-old\n+new\n",
    );
    let parsed = parse_bounded_lines(input.lines(), 0)?;
    assert!(parsed.changed_files.is_empty());
    assert_eq!(parsed.deleted_file_count, 1);
    assert_matches_unbounded(input, &parsed);
    assert_stops_at_header("--- /dev/null\n+++ b/src/new.rs\n", 0, 1)
}

#[test]
fn bounded_malformed_hunk_does_not_hide_a_later_valid_hunk() -> Result<(), String> {
    let input = "--- a/src/a.rs\n+++ b/src/a.rs\n@@ malformed @@\n-ignored\n+ignored\n@@ -4 +8 @@\n-old\n+new\n";
    let parsed = parse_bounded_lines(input.lines(), 1)?;
    assert_eq!(parsed.changed_files.len(), 1);
    assert_eq!(parsed.changed_files[0].added_lines.len(), 1);
    assert_eq!(parsed.changed_files[0].removed_lines.len(), 1);
    assert_eq!(parsed.changed_files[0].removed_lines[0].line, 4);
    assert_eq!(parsed.changed_files[0].removed_lines[0].new_side_line, 8);
    assert_eq!(parsed.changed_files[0].added_lines[0].line, 8);
    assert_matches_unbounded(input, &parsed);
    Ok(())
}
