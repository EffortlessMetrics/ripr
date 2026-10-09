use std::cell::Cell;
use std::path::{Path, PathBuf};

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

fn assert_full_projection_eq(actual: &ParsedDiff, expected: &ParsedDiff) {
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
    assert_eq!(actual.truncated_file_sections, expected.truncated_file_sections);
    assert_eq!(actual.raw_line1_bom_paths, expected.raw_line1_bom_paths);
    assert_eq!(actual.limitations, expected.limitations);
}

fn assert_matches_unbounded(input: &str, actual: &ParsedDiff) -> Result<(), String> {
    let expected = parse_unbounded(input);
    assert_full_projection_eq(actual, &expected);
    let bounded = super::parse_bounded(input, 10_000)?;
    assert_full_projection_eq(actual, &bounded);
    Ok(())
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
    assert_matches_unbounded(input, &parsed)?;
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
    assert_matches_unbounded(&input, &parsed)?;
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
    assert_matches_unbounded(input, &parsed)?;
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
    assert_matches_unbounded(input, &parsed)?;
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
    assert_matches_unbounded(input, &parsed)?;
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
    assert_matches_unbounded(input, &parsed)?;
    Ok(())
}

fn check_declared_hunk_status(input: &str, malformed: bool) -> Result<(), String> {
    for parsed in [
        parse_unbounded(input),
        parse_bounded_lines(input.lines(), 8)?,
    ] {
        if parsed.changed_files.is_empty() {
            return Err("declared-hunk control did not admit a source file".to_string());
        }
        let observed = parsed
            .limitations
            .iter()
            .any(|item| item.kind == AnalysisLimitationKind::MalformedDiff);
        if observed != malformed {
            return Err(format!(
                "expected malformed={malformed}, observed {observed} for {input:?}"
            ));
        }
    }
    Ok(())
}

#[test]
fn unfinished_declared_hunks_are_malformed_at_eof_and_boundaries() -> Result<(), String> {
    let prefix = "--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,2 +1,2 @@\n-old\n+new\n";
    for suffix in [
        "",
        "@@ -4 +4 @@\n-next\n+next_new\n",
        "diff --git a/src/b.rs b/src/b.rs\n--- a/src/b.rs\n+++ b/src/b.rs\n@@ -1 +1 @@\n-before\n+after\n",
        "@@ -4,invalid +4,1 @@\n",
        "@@ -4,999999999999999999999999999999999999999999 +4,1 @@\n",
        "@@@ -4 -4 +4 @@@\n++hidden\n",
    ] {
        let input = format!("{prefix}{suffix}");
        check_declared_hunk_status(&input, true)?;
        let parsed = parse_bounded_lines(input.lines(), 8)?;
        let file = parsed
            .changed_files
            .iter()
            .find(|file| file.path == Path::new("src/a.rs"))
            .ok_or_else(|| "unfinished hunk lost its advisory source file".to_string())?;
        if !file.added_lines.iter().any(|line| line.text == "new")
            || !file.removed_lines.iter().any(|line| line.text == "old")
        {
            return Err("unfinished hunk lost its earlier advisory changed lines".to_string());
        }
    }
    check_declared_hunk_status("--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,7 +1,7 @@\n", true)
}

#[test]
fn declared_hunk_counts_reject_excess_body_and_invalid_numbers() -> Result<(), String> {
    for body in [
        "@@ -1 +1 @@\n-old\n+new\n+excess\n",
        "@@ -1,invalid +1,1 @@\n-old\n+new\n",
        "@@ -1,999999999999999999999999999999999999999999 +1,1 @@\n-old\n+new\n",
    ] {
        check_declared_hunk_status(&format!("--- a/src/a.rs\n+++ b/src/a.rs\n{body}"), true)?;
    }
    let marker_body =
        parse_unbounded("--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n--- a/name\n+++ b/name\n");
    let file = marker_body
        .changed_files
        .first()
        .ok_or_else(|| "marker-body file missing".to_string())?;
    if marker_body.changed_files.len() != 1
        || file.path != Path::new("src/a.rs")
        || !file
            .removed_lines
            .iter()
            .any(|line| line.text == "-- a/name")
        || !file.added_lines.iter().any(|line| line.text == "++ b/name")
    {
        return Err("valid body marker pair was mistaken for a file section".to_string());
    }
    Ok(())
}

#[test]
fn declared_ranges_reject_line_zero_and_unusable_end_coordinates() -> Result<(), String> {
    let last_usable = usize::MAX - 1;
    let mut failures = Vec::new();
    for body in [
        "@@ -0,1 +1,1 @@\n-old\n+new\n".to_string(),
        "@@ -1,1 +0,1 @@\n-old\n+new\n".to_string(),
        format!("@@ -0,0 +{last_usable},2 @@\n+first\n+last\n"),
        format!("@@ -{last_usable},2 +0,0 @@\n-first\n-last\n"),
    ] {
        let input = format!("--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n-old\n+new\n{body}");
        if let Err(error) = check_declared_hunk_status(&input, true) {
            failures.push(error);
        }
        let parsed = parse_unbounded(&input);
        let file = parsed
            .changed_files
            .first()
            .ok_or_else(|| "range control lost its file".to_string())?;
        if !file.added_lines.iter().any(|line| line.text == "new")
            || !file.removed_lines.iter().any(|line| line.text == "old")
        {
            failures.push("invalid range lost earlier advisory changes".to_string());
        }
    }
    for body in [
        "@@ -0,0 +1,1 @@\n+new\n".to_string(),
        "@@ -1,1 +0,0 @@\n-old\n".to_string(),
        "@@ -0,0 +0,0 @@\n".to_string(),
        format!("@@ -0,0 +{last_usable},1 @@\n+last\n"),
        format!("@@ -{last_usable},1 +0,0 @@\n-last\n"),
    ] {
        if let Err(error) =
            check_declared_hunk_status(&format!("--- a/src/a.rs\n+++ b/src/a.rs\n{body}"), false)
        {
            failures.push(error);
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

#[test]
fn signed_declared_hunk_numbers_are_malformed() -> Result<(), String> {
    let mut failures = Vec::new();
    for body in [
        "@@ -+1 +1 @@\n-old\n+new\n",
        "@@ -1 ++1 @@\n-old\n+new\n",
        "@@ -1,+1 +1,1 @@\n-old\n+new\n",
        "@@ -1,1 +1,+1 @@\n-old\n+new\n",
        "@@ -+0,0 +1,1 @@\n+new\n",
        "@@ -0,+0 +1,1 @@\n+new\n",
    ] {
        let input = format!("{FIRST_FILE}--- a/src/b.rs\n+++ b/src/b.rs\n{body}");
        if let Err(error) = check_declared_hunk_status(&input, true) {
            failures.push(error);
        }
        let parsed = parse_unbounded(&input);
        let first = parsed
            .changed_files
            .iter()
            .find(|file| file.path == Path::new("src/a.rs"))
            .ok_or_else(|| "signed range lost earlier valid file".to_string())?;
        if !first.added_lines.iter().any(|line| line.text == "new") {
            failures.push("signed range lost earlier advisory source".to_string());
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

#[test]
fn excess_zero_count_sides_never_emit_line_zero() -> Result<(), String> {
    for body in [
        "@@ -0,0 +1,1 @@\n-unexpected\n+expected\n",
        "@@ -1,1 +0,0 @@\n+unexpected\n-expected\n",
    ] {
        let input = format!("{FIRST_FILE}--- a/src/b.rs\n+++ b/src/b.rs\n{body}");
        check_declared_hunk_status(&input, true)?;
        for parsed in [
            parse_unbounded(&input),
            parse_bounded_lines(input.lines(), 8)?,
        ] {
            if parsed
                .changed_files
                .iter()
                .flat_map(|file| file.added_lines.iter().chain(&file.removed_lines))
                .any(|line| line.line == 0)
            {
                return Err(format!(
                    "malformed excess published nonexistent coordinate zero: {parsed:?}"
                ));
            }
            let first = parsed
                .changed_files
                .iter()
                .find(|file| file.path == Path::new("src/a.rs"))
                .ok_or_else(|| "zero-side excess lost earlier valid file".to_string())?;
            if !first.added_lines.iter().any(|line| line.text == "new")
                || !parsed.limitations.iter().any(|item| {
                    item.kind == AnalysisLimitationKind::MalformedDiff
                        && item.affected_items == Some(1)
                })
            {
                return Err(
                    "zero-side excess lost valid advisory source or its single limitation".into(),
                );
            }
        }
        if crate::analysis::diff::parse_unified_diff(&input)
            .iter()
            .flat_map(|file| file.added_lines.iter().chain(&file.removed_lines))
            .any(|line| line.line == 0)
        {
            return Err("legacy Vec API exposed nonexistent coordinate zero".into());
        }
    }
    Ok(())
}

#[test]
fn excess_body_counter_overflow_keeps_only_usable_advisory_coordinates() -> Result<(), String> {
    let start = usize::MAX - 2;
    for (added, body) in [
        (
            true,
            format!("@@ -0,0 +{start},2 @@\n+first\n+second\n+third\n+fourth\n"),
        ),
        (
            false,
            format!("@@ -{start},2 +0,0 @@\n-first\n-second\n-third\n-fourth\n"),
        ),
    ] {
        let input = format!("--- a/src/a.rs\n+++ b/src/a.rs\n{body}");
        for parsed in [
            parse_unbounded(&input),
            parse_bounded_lines(input.lines(), 8)?,
        ] {
            let file = parsed
                .changed_files
                .first()
                .ok_or_else(|| "counter control lost its source file".to_string())?;
            let lines = if added {
                &file.added_lines
            } else {
                &file.removed_lines
            };
            if lines.len() != 2
                || !lines
                    .iter()
                    .any(|line| line.line == start && line.text == "first")
                || !lines
                    .iter()
                    .any(|line| line.line == start + 1 && line.text == "second")
                || !parsed.limitations.iter().any(|item| {
                    item.kind == AnalysisLimitationKind::MalformedDiff
                        && item.affected_items == Some(1)
                })
            {
                return Err(format!(
                    "excess body must retain only usable advisory coordinates: {parsed:?}"
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn malformed_hunk_counts_are_not_duplicated_by_later_boundaries() -> Result<(), String> {
    let prefix = "--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,2 +1,2 @@\n-old\n+new\n";
    for (next, expected) in [
        ("@@ -4 +4 @@\n-before\n+after\n", 1),
        ("@@ -4,invalid +4 @@\n", 2),
    ] {
        let input = format!(
            "{prefix}{next}diff --git a/src/b.rs b/src/b.rs\n--- a/src/b.rs\n+++ b/src/b.rs\n@@ -1 +1 @@\n-before\n+after\n"
        );
        for parsed in [
            parse_unbounded(&input),
            parse_bounded_lines(input.lines(), 8)?,
        ] {
            let limitation = parsed
                .limitations
                .iter()
                .find(|item| {
                    item.kind == AnalysisLimitationKind::MalformedDiff
                        && item.path.as_deref() == Some("src/a.rs")
                })
                .ok_or_else(|| "boundary control lost its malformed-hunk limitation".to_string())?;
            if limitation.affected_items != Some(expected) {
                return Err(format!(
                    "expected {expected} distinct malformed hunks, got {limitation:?}"
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn complete_declared_hunks_preserve_ordinary_and_quarantined_controls() -> Result<(), String> {
    for body in [
        "@@ -1 +1 @@\n-old\n+new\n",
        "@@ -0,0 +1,1 @@\n+new\n",
        "@@ -1,1 +0,0 @@\n-old\n",
        "@@ -1,0 +1,0 @@\n",
        "@@ -1,2 +1,2 @@\n-old\n+new\n context\n",
        "@@ -1 +1 @@\n--- a/name\n+++ b/name\n",
        "@@ -1 +1 @@\n-old\n\\ No newline at end of file\n+new\n\\ No newline at end of file\n",
        "@@ -1 +1 @@\n-old\n+new\n@@ -4 +4 @@\n-next\n+next_new\n",
        "@@ -0,0 +1,4 @@\n+<<<<<<< ours\n+hidden\n+>>>>>>> theirs\n+visible\n",
        "@@@ -1 -1 +1 @@@\n++hidden\n",
    ] {
        let input = format!("--- a/src/a.rs\n+++ b/src/a.rs\n{body}");
        check_declared_hunk_status(&input, false)?;
        check_declared_hunk_status(&input.replace('\n', "\r\n"), false)?;
    }
    for metadata in [
        "diff --git a/src/a.rs b/src/a.rs\nold mode 100644\nnew mode 100755\n",
        "diff --git a/src/a.rs b/src/b.rs\nsimilarity index 100%\nrename from src/a.rs\nrename to src/b.rs\n",
        "diff --git a/src/a.rs b/src/a.rs\nBinary files a/src/a.rs and b/src/a.rs differ\n",
    ] {
        let parsed = parse_bounded_lines(metadata.lines(), 8)?;
        if parsed
            .limitations
            .iter()
            .any(|item| item.kind == AnalysisLimitationKind::MalformedDiff)
        {
            return Err(format!("valid metadata became malformed: {metadata:?}"));
        }
    }
    Ok(())
}

fn fixture_added_lines_match_head(patch: &str, head: &str) -> Result<bool, String> {
    let parsed = parse_bounded_lines(patch.lines(), 8)?;
    let mut observed = 0;
    for file in &parsed.changed_files {
        for added in &file.added_lines {
            observed += 1;
            if head.lines().nth(added.line.saturating_sub(1)) != Some(added.text.as_str()) {
                return Ok(false);
            }
        }
    }
    if observed == 0 {
        return Err("fixture alignment control admitted no added lines".to_string());
    }
    Ok(true)
}

#[test]
fn causal_fixture_hunks_are_complete_and_match_their_head_sources() -> Result<(), String> {
    let cases = [
        (
            include_str!("../../../../../../../fixtures/unsafe_boundary_probe/diff.patch"),
            include_str!("../../../../../../../fixtures/unsafe_boundary_probe/input/src/lib.rs"),
            "@@ -2,7 +2,7 @@",
            "@@ -2,8 +2,7 @@",
            "@@ -2,7 +3,7 @@",
        ),
        (
            include_str!("../../../../../../../fixtures/infect_wildcard_discard/diff.patch"),
            include_str!("../../../../../../../fixtures/infect_wildcard_discard/input/src/lib.rs"),
            "@@ -1,5 +1,7 @@",
            "@@ -1,5 +1,6 @@",
            "@@ -1,5 +2,7 @@",
        ),
        (
            include_str!("../../../../../../../fixtures/error_return_unresolved_guard/diff.patch"),
            include_str!(
                "../../../../../../../fixtures/error_return_unresolved_guard/input/src/lib.rs"
            ),
            "@@ -14,6 +14,6 @@",
            "@@ -14,7 +14,7 @@",
            "@@ -13,6 +13,6 @@",
        ),
    ];
    for (patch, head, header, bad_count, bad_coordinates) in cases {
        check_declared_hunk_status(patch, false)?;
        assert!(fixture_added_lines_match_head(patch, head)?);
        if !patch.contains(header) {
            return Err(format!("fixture control lacks expected header {header}"));
        }
        check_declared_hunk_status(&patch.replacen(header, bad_count, 1), true)?;
        let shifted = patch.replacen(header, bad_coordinates, 1);
        check_declared_hunk_status(&shifted, false)?;
        assert!(!fixture_added_lines_match_head(&shifted, head)?);
    }
    Ok(())
}

// #1627: the test-only predecessor reconstructed decoded lines. The retained
// assertions now reach the same borrowed records used by production parsing.
// Its RED is adapter sensitivity, not a defect in existing String input.
fn intake_record_bytes_for_witness(input: &[u8]) -> Vec<Vec<u8>> {
    super::RawDiffRecords::new(input)
        .map(|record| record.bytes.to_vec())
        .collect()
}

#[test]
fn borrowed_intake_records_preserve_original_bytes_and_terminators() -> Result<(), String> {
    let input = b"body:\xff\r\nbody:\xfe\nlast\r";
    let expected: &[&[u8]] = &[b"body:\xff\r\n", b"body:\xfe\n", b"last\r"];
    assert_eq!(expected.concat(), input);
    let Err(_) = std::str::from_utf8(input) else {
        return Err("raw record fixture unexpectedly became valid UTF8".to_string());
    };
    let semantic = String::from_utf8_lossy(input);
    assert_eq!(
        semantic.lines().collect::<Vec<_>>(),
        vec!["body:\u{fffd}", "body:\u{fffd}", "last\r"]
    );
    let reconstructed = semantic
        .lines()
        .map(|line| line.as_bytes().to_vec())
        .collect::<Vec<_>>();
    assert_ne!(reconstructed.concat(), input);

    let actual = intake_record_bytes_for_witness(input);
    let actual = actual.iter().map(Vec::as_slice).collect::<Vec<_>>();
    assert_eq!(
        actual, expected,
        "intake records lost original bytes or terminators before semantic view"
    );
    Ok(())
}

#[test]
fn borrowed_record_views_match_std_lines_and_preserve_complete_slices() {
    use super::ParserLine;
    use std::borrow::Cow;

    let cases: &[&[u8]] = &[
        b"",
        b"\n",
        b"\n\n",
        b"\r\n\r\n",
        b"last",
        b"last\n",
        b"last\r",
        b"\r",
        b"\r\r\n",
        b"\n\n\r\nx\r\nx\r",
        b"unicode:\xc3\xa9\xe2\x80\xa8next\n",
        b"\xff\n\xfe\r\n\xf0\x90\n\xf0\x90\r\n",
    ];
    for input in cases {
        let records = super::RawDiffRecords::new(input)
            .map(|record| record.bytes)
            .collect::<Vec<_>>();
        assert_eq!(records.concat(), *input, "original partition: {input:?}");
        let decoded = String::from_utf8_lossy(input);
        let expected = decoded.lines().map(str::to_string).collect::<Vec<_>>();
        let mut actual = Vec::new();
        for record in super::RawDiffRecords::new(input) {
            let text = record.semantic_text();
            actual.push(text.to_string());
            match std::str::from_utf8(record.bytes) {
                Ok(_) => assert!(matches!(text, Cow::Borrowed(_))),
                Err(_) => assert!(matches!(text, Cow::Owned(_))),
            }
        }
        assert_eq!(actual, expected, "semantic lines: {input:?}");
        assert_eq!(records.len(), expected.len(), "record count: {input:?}");
    }

    let raw = b"\n\n\r\nx\r\nx\r";
    let expected: &[&[u8]] = &[b"\n", b"\n", b"\r\n", b"x\r\n", b"x\r"];
    let actual = super::RawDiffRecords::new(raw)
        .map(|record| record.bytes)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);

    let repeated = super::RawDiffRecords::new(b"same\nsame\n")
        .map(|record| record.bytes)
        .collect::<Vec<_>>();
    assert_eq!(repeated, vec![b"same\n".as_slice(), b"same\n".as_slice()]);
}

#[test]
fn borrowed_records_reject_lossy_reconstruction_and_trim_all_cr() -> Result<(), String> {
    use super::ParserLine;

    let input = b"+body:\xff\n+body:\xfe\n";
    let records = super::RawDiffRecords::new(input)
        .map(|record| (record.bytes, record.semantic_text().into_owned()))
        .collect::<Vec<_>>();
    let expected: &[&[u8]] = &[b"+body:\xff\n", b"+body:\xfe\n"];
    let actual = records.iter().map(|(bytes, _)| *bytes).collect::<Vec<_>>();
    assert_eq!(actual, expected);
    let texts = records.iter().map(|(_, text)| text.as_str()).collect::<Vec<_>>();
    assert_eq!(texts, vec!["+body:\u{fffd}", "+body:\u{fffd}"]);
    let reconstructed = String::from_utf8_lossy(input)
        .lines()
        .map(|line| line.as_bytes().to_vec())
        .collect::<Vec<_>>();
    assert_ne!(reconstructed.concat(), input);
    assert_ne!(
        reconstructed.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        actual
    );

    let Some(record) = super::RawDiffRecords::new(b"last\r").next() else {
        return Err("lone-CR fixture produced no record".to_string());
    };
    assert_eq!(record.semantic_text(), "last\r");
    let wrong_trim = String::from_utf8_lossy(record.bytes)
        .trim_end_matches('\r')
        .to_string();
    assert_ne!(wrong_trim, record.semantic_text());
    Ok(())
}

#[test]
fn borrowed_record_production_routes_preserve_all_metadata_and_final_debt() -> Result<(), String> {
    let input = concat!(
        "diff --git a/src/mode.rs b/src/mode.rs\nold mode 100644\nnew mode 100755\n",
        "diff --git a/src/from.rs b/src/copied.rs\nsimilarity index 100%\ncopy from src/from.rs\ncopy to src/copied.rs\n",
        "diff --git a/src/empty.rs b/src/empty.rs\nnew file mode 100644\nindex 0000000..e69de29\n",
        "diff --git a/src/bom.rs b/src/bom.rs\n--- a/src/bom.rs\n+++ b/src/bom.rs\n@@ -1 +1 @@\n-\u{feff}old\n+\u{feff}new\n\\ No newline at end of file\n",
        "diff --git a/src/truncated.rs b/src/truncated.rs\n--- a/src/truncated.rs\n+++ b/src/truncated.rs\n@@ -1 +1 @@\ngarbage",
    );
    let legacy = parse_bounded_lines(input.lines(), 2)?;
    let bounded = super::parse_bounded(input, 2)?;
    let unbounded = parse_unbounded(input);
    assert_full_projection_eq(&bounded, &legacy);
    assert_full_projection_eq(&unbounded, &legacy);
    assert_eq!(bounded.changed_files.len(), 2);
    assert_eq!(bounded.truncated_file_sections, 1);
    assert_eq!(bounded.raw_line1_bom_paths, vec![PathBuf::from("src/bom.rs")]);
    assert_eq!(bounded.renamed_file_count, 0);
    assert!(
        bounded
            .limitations
            .iter()
            .any(|item| item.kind == AnalysisLimitationKind::MalformedDiff)
    );

    let payload = "--- a/src/payload.rs\n+++ b/src/payload.rs\n@@ -1 +1 @@\n--- a/name\n+++ b/name\n\\ No newline at end of file";
    let legacy = parse_bounded_lines(payload.lines(), 1)?;
    let actual = super::parse_bounded(payload, 1)?;
    assert_full_projection_eq(&actual, &legacy);
    let Some(file) = actual.changed_files.first() else {
        return Err("payload fixture produced no changed file".to_string());
    };
    assert_eq!(file.path, PathBuf::from("src/payload.rs"));
    let Some(removed) = file.removed_lines.first() else {
        return Err("payload fixture produced no removed line".to_string());
    };
    let Some(added) = file.added_lines.first() else {
        return Err("payload fixture produced no added line".to_string());
    };
    assert_eq!(removed.text, "-- a/name");
    assert_eq!(added.text, "++ b/name");
    Ok(())
}

#[test]
fn borrowed_records_stop_at_existing_admission_sites_before_large_body() -> Result<(), String> {
    let git = format!("{FIRST_FILE}diff --git a/src/b.rs b/src/b.rs\n--- a/src/b.rs\n+++ b/src/b.rs\n");
    let plain = "--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n-old\n+new\n--- a/src/b.rs\n+++ b/src/b.rs\n".to_string();
    let rename = format!("{FIRST_FILE}diff --git a/src/old.rs b/src/new.rs\nsimilarity index 100%\nrename from src/old.rs\nrename to src/new.rs\n");
    for prefix in [git, plain, rename] {
        let input = format!(
            "{prefix}@@ -0,0 +1,10000 @@\n{}",
            "+unread body\n".repeat(10_000)
        );
        let reads = Cell::new(0);
        let records = super::RawDiffRecords::new(input.as_bytes()).inspect(|_| {
            reads.set(reads.get() + 1);
        });
        let Err(error) = super::parse_bounded_records(records, 1) else {
            return Err("record route admitted an oversized prefix".to_string());
        };
        assert_eq!(reads.get(), prefix.lines().count(), "body was read");
        assert!(error.starts_with("diff_scope_oversized: at least 2 changed files"));
        assert!(error.contains("limit (1)"));
    }
    Ok(())
}

#[derive(Default)]
struct ByteLedger {
    records: Vec<(usize, usize, usize, Vec<u8>)>,
    reductions: Vec<OwnedReduction>,
    ends: Vec<super::RawEnd>,
    refuse_record: Option<usize>,
    refuse_reduction: Option<usize>,
    refuse_finish: bool,
}

struct OwnedReduction {
    ordinal: usize,
    section: Option<usize>,
    hunk: Option<usize>,
    kind: super::RawRecordKind,
    ranges: Option<((usize, usize), (usize, usize))>,
    before: Option<(usize, usize)>,
    after: Option<(usize, usize)>,
    consumed: Option<(usize, usize)>,
    coordinates: Option<(usize, usize)>,
    token: Option<Vec<u8>>,
    decoded: Option<Vec<u8>>,
    path: Option<PathBuf>,
    projection: Option<(super::RawChangeSide, usize)>,
}

impl super::RawDiffObserver for ByteLedger {
    fn record(&mut self, record: super::RawRecord<'_>) -> Result<(), String> {
        if self.refuse_record == Some(record.ordinal) {
            return Err("record sink refused".to_string());
        }
        assert_eq!(record.ordinal, self.records.len());
        assert_eq!(record.start, self.records.last().map_or(0, |last| last.2));
        assert_eq!(record.end - record.start, record.bytes.len());
        self.records.push((record.ordinal, record.start, record.end, record.bytes.to_vec()));
        Ok(())
    }

    fn reduction(&mut self, reduction: super::RawReduction<'_>) -> Result<(), String> {
        if self.refuse_reduction == Some(reduction.record.ordinal) {
            return Err("reduction sink refused".to_string());
        }
        assert_eq!(self.records.len(), self.reductions.len() + 1);
        assert_eq!(reduction.record.ordinal, self.reductions.len());
        self.reductions.push(OwnedReduction {
            ordinal: reduction.record.ordinal, section: reduction.section,
            hunk: reduction.hunk, kind: reduction.kind, ranges: reduction.declared_ranges,
            before: reduction.remaining_before, after: reduction.remaining_after,
            consumed: reduction.consumed, coordinates: reduction.coordinates,
            token: reduction.raw_path_token.map(<[u8]>::to_vec),
            decoded: reduction.decoded_path_bytes,
            path: reduction.native_path.map(Path::to_path_buf), projection: reduction.projection,
        });
        Ok(())
    }

    fn finish(&mut self, end: super::RawEnd, _parsed: &ParsedDiff) -> Result<(), String> {
        if self.refuse_finish { return Err("finish sink refused".to_string()); }
        assert_eq!(end.records, self.records.len());
        assert_eq!(end.records, self.reductions.len());
        assert_eq!(end.bytes, self.records.last().map_or(0, |last| last.2));
        self.ends.push(end);
        Ok(())
    }
}

fn observed(input: &[u8], limit: usize) -> Result<(ParsedDiff, ByteLedger), String> {
    let mut ledger = ByteLedger::default();
    let parsed = super::parse_bytes_bounded(input, limit, &mut ledger)?;
    let semantic = String::from_utf8_lossy(input);
    let expected = parse_bounded_lines(semantic.lines(), limit)?;
    assert_full_projection_eq(&parsed, &expected);
    assert_eq!(ledger.ends.len(), 1);
    assert_eq!(ledger.records.iter().flat_map(|r| r.3.iter().copied()).collect::<Vec<_>>(), input);
    Ok((parsed, ledger))
}

#[test]
fn original_bytes_precede_decoding_and_include_final_record() -> Result<(), String> {
    let input = b"diff --git a/src/a.rs b/src/a.rs\r\n--- a/src/a.rs\r\n+++ b/src/a.rs\r\n@@ -1 +1 @@\r\n-\xfe\r\n+\xff";
    let (parsed, ledger) = observed(input, 1)?;
    assert_eq!(ledger.records.len(), 6);
    assert_eq!(ledger.records[4].3, b"-\xfe\r\n");
    assert_eq!(ledger.records[5].3, b"+\xff");
    assert_eq!(ledger.reductions[5].ordinal, 5);
    assert_eq!(ledger.reductions[5].projection, Some((super::RawChangeSide::Added, 0)));
    assert_eq!(parsed.changed_files[0].added_lines[0].text, "\u{fffd}");
    // A retained sensitivity control: the semantic round trip cannot satisfy
    // the original-byte ledger, even when its whole semantic projection matches.
    let lossy = String::from_utf8_lossy(input);
    let (_, wrong) = observed(lossy.as_bytes(), 1)?;
    assert_ne!(wrong.records[4].3, ledger.records[4].3);
    assert_ne!(wrong.records[5].3, ledger.records[5].3);
    Ok(())
}

#[test]
fn coalesced_paths_keep_original_tokens_sections_and_insertion_slots() -> Result<(), String> {
    let input = concat!(
        "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n-old\n+new\n",
        "diff --git a/./src/a.rs b/./src/a.rs\n--- a/./src/a.rs\n+++ b/./src/a.rs\n@@ -2 +2 @@\n-before\n+after\n",
        "diff --git a/src/old.rs b/src/a.rs\nsimilarity index 80%\nrename from src/old.rs\nrename to src/a.rs\n--- a/src/old.rs\n+++ b/src/a.rs\n@@ -3 +3 @@\n-previous\n+current\n"
    );
    let (parsed, ledger) = observed(input.as_bytes(), 1)?;
    assert_eq!(parsed.changed_files.len(), 1);
    let additions: Vec<_> = ledger.reductions.iter()
        .filter(|r| matches!(r.projection, Some((super::RawChangeSide::Added, _)))).collect();
    assert_eq!(additions.len(), 3);
    for (index, reduction) in additions.iter().enumerate() {
        assert_eq!(reduction.section, Some(index));
        assert_eq!(reduction.hunk, Some(index));
        assert_eq!(reduction.path.as_deref(), Some(Path::new("src/a.rs")));
        assert_eq!(reduction.projection, Some((super::RawChangeSide::Added, index)));
    }
    let tokens: Vec<_> = ledger.reductions.iter().filter_map(|r| r.decoded.as_deref()).collect();
    assert!(tokens.contains(&b"b/src/a.rs".as_slice()));
    assert!(tokens.contains(&b"b/./src/a.rs".as_slice()));
    assert_eq!(ledger.ends[0].sections, 3);
    assert_eq!(ledger.ends[0].hunks, 3);
    Ok(())
}

#[test]
fn octal_raw_path_identity_survives_native_platform_conversion() -> Result<(), String> {
    let input = concat!(
        "diff --git \"a/src/pricing_\\377.rs\" \"b/src/pricing_\\377.rs\"\n--- \"a/src/pricing_\\377.rs\"\n+++ \"b/src/pricing_\\377.rs\"\n@@ -1 +1 @@\n-a\n+b\n",
        "diff --git \"a/src/pricing_\\\\377.rs\" \"b/src/pricing_\\\\377.rs\"\n--- \"a/src/pricing_\\\\377.rs\"\n+++ \"b/src/pricing_\\\\377.rs\"\n@@ -1 +1 @@\n-c\n+d\n"
    );
    let (_, ledger) = observed(input.as_bytes(), 2)?;
    let markers: Vec<_> = ledger.reductions.iter().filter(|r|
        r.token.as_deref().is_some_and(|token| token.starts_with(b"\"b/"))).collect();
    assert_eq!(markers.len(), 2);
    assert_eq!(markers[0].decoded.as_deref(), Some(b"b/src/pricing_\xff.rs".as_slice()));
    assert_eq!(markers[1].decoded.as_deref(), Some(br"b/src/pricing_\377.rs".as_slice()));
    assert_ne!(markers[0].token, markers[1].token);
    assert_ne!(markers[0].decoded, markers[1].decoded);
    assert_ne!(markers[0].section, markers[1].section);
    Ok(())
}

#[test]
fn deletion_counts_use_declared_ranges_when_no_changed_path_survives() -> Result<(), String> {
    let input = b"diff --git a/src/gone.rs b/src/gone.rs\ndeleted file mode 100644\n--- a/src/gone.rs\n+++ /dev/null\n@@ -5,2 +4,0 @@\n-A\n-B\n";
    let (parsed, ledger) = observed(input, 1)?;
    assert!(parsed.changed_files.is_empty());
    assert_eq!(parsed.deleted_file_count, 1);
    let removed: Vec<_> = ledger.reductions.iter().filter(|r| r.consumed == Some((1, 0))).collect();
    assert_eq!(removed.len(), 2);
    assert_eq!(removed[0].coordinates, Some((5, 4)));
    assert_eq!(removed[1].coordinates, Some((6, 4)));
    assert_eq!(removed[0].before, Some((2, 0)));
    assert_eq!(removed[1].after, Some((0, 0)));
    for reduction in removed {
        assert_eq!(reduction.ranges, Some(((5, 2), (4, 0))));
        assert_eq!(reduction.kind, super::RawRecordKind::Body(super::BodyDisposition::NoPath));
        assert!(reduction.projection.is_none());
    }
    Ok(())
}

#[test]
fn malformed_excess_never_recovers_coordinates_until_a_valid_header() -> Result<(), String> {
    let input = b"diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -0,0 +1 @@\n+one\n+excess\n+more\n@@ -0,0 +9 @@\n+valid\n";
    let (parsed, ledger) = observed(input, 1)?;
    assert!(!parsed.limitations.is_empty());
    let added: Vec<_> = ledger.reductions.iter().filter(|r|
        matches!(r.projection, Some((super::RawChangeSide::Added, _)))).collect();
    assert_eq!(added.len(), 4); // Existing advisory projection is preserved.
    assert_eq!(added[0].coordinates, Some((0, 1)));
    assert_eq!(added[1].coordinates, None);
    assert_eq!(added[1].after, None);
    assert_eq!(added[2].coordinates, None);
    assert_eq!(added[3].coordinates, Some((0, 9)));
    Ok(())
}

#[test]
fn lookahead_is_not_consumption_and_plain_sections_are_unanchored() -> Result<(), String> {
    let input = b"diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n--- token\n+++ token\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -2 +2 @@\n-old\n+new\n";
    let (_, ledger) = observed(input, 1)?;
    assert_eq!(ledger.records.len(), 11);
    assert_eq!(ledger.reductions[4].projection, Some((super::RawChangeSide::Removed, 0)));
    assert_eq!(ledger.reductions[5].projection, Some((super::RawChangeSide::Added, 0)));
    assert_eq!(ledger.reductions[4].section, Some(0));
    assert_eq!(ledger.reductions[6].section, None);
    assert_eq!(ledger.reductions[10].section, None);
    Ok(())
}

#[test]
fn refusals_after_a_prefix_never_emit_an_end_callback() -> Result<(), String> {
    for refuse_record in [true, false] {
        let mut ledger = ByteLedger::default();
        if refuse_record { ledger.refuse_record = Some(4); }
        else { ledger.refuse_reduction = Some(4); }
        let Err(error) = super::parse_bytes_bounded(FIRST_FILE.as_bytes(), 1, &mut ledger) else {
            return Err("sink refusal became partial success".to_string());
        };
        assert!(error.contains("sink refused"));
        assert!(ledger.ends.is_empty());
    }
    let input = format!("{FIRST_FILE}diff --git a/src/b.rs b/src/b.rs\n--- a/src/b.rs\n+++ b/src/b.rs\n@@ -1 +1 @@\n-unread\n+unread\n");
    let mut ledger = ByteLedger::default();
    let Err(error) = super::parse_bytes_bounded(input.as_bytes(), 1, &mut ledger) else {
        return Err("file admission refusal became partial success".to_string());
    };
    assert!(error.starts_with("diff_scope_oversized:"));
    assert_eq!(ledger.records.len(), 9);
    assert_eq!(ledger.reductions.len(), 8);
    assert!(ledger.ends.is_empty());
    Ok(())
}

#[test]
fn empty_metadata_only_quarantined_and_truncated_streams_are_all_accounted() -> Result<(), String> {
    for input in [
        "",
        "trailing opaque metadata\r",
        "diff --git a/old.rs b/new.rs\nsimilarity index 100%\nrename from old.rs\nrename to new.rs\n",
        "diff --git a/old.rs b/new.rs\nsimilarity index 100%\ncopy from old.rs\ncopy to new.rs\n",
        "diff --git a/a.rs b/a.rs\nold mode 100644\nnew mode 100755\n",
        "diff --git a/a.bin b/a.bin\nBinary files a/a.bin and b/a.bin differ\n",
        "diff --git a/link.rs b/link.rs\nnew file mode 120000\n--- /dev/null\n+++ b/link.rs\n@@ -0,0 +1 @@\n+target.rs\n",
        "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/../../escape.rs\n@@ -0,0 +1 @@\n+excluded\n",
        "diff --cc a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@@ -1,1 -1,1 +1,1 @@@\n++excluded\n",
        "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -0,0 +1,3 @@\n+<<<<<<< ours\n+conflict\n+>>>>>>> theirs\n",
        "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ malformed @@\n+ignored\n",
        "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1,2 +1,2 @@\n-only\n",
        "diff --git a/lib b/lib\nindex 1111111..2222222 160000\n--- a/lib\n+++ b/lib\n@@ -1 +1 @@\n-Subproject commit 1111111\n+Subproject commit 2222222\n",
        "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-\u{feff}old\n+\u{feff}new\n\\ No newline at end of file\n",
    ] {
        let (_, ledger) = observed(input.as_bytes(), 10)?;
        assert_eq!(ledger.ends[0].bytes, input.len());
    }
    let (_, empty) = observed(b"", 1)?;
    assert_eq!(empty.ends[0], super::RawEnd { bytes: 0, records: 0, sections: 0, hunks: 0 });
    Ok(())
}

#[test]
fn overflow_and_premature_end_are_errors_without_finish() -> Result<(), String> {
    use super::ParserObserver;
    for overflow_records in [false, true] {
        let mut ledger = ByteLedger::default();
        let mut observer = super::OriginalByteObserver {
            observer: &mut ledger, expected_bytes: usize::MAX,
            end: super::RawEnd { bytes: if overflow_records { 0 } else { usize::MAX }, records: if overflow_records { usize::MAX } else { 0 }, sections: 0, hunks: 0 },
            pending: None, section: None, hunk: None, header: None, marker_opened: false,
        };
        let line = super::RawDiffRecord { bytes: b"x" };
        let Err(error) = observer.begin(&line) else {
            return Err("checked raw ordinal/offset overflow succeeded".to_string());
        };
        assert!(error.contains("overflow"));
        assert!(ledger.ends.is_empty());
    }
    let mut ledger = ByteLedger::default();
    let mut observer = super::OriginalByteObserver {
        observer: &mut ledger, expected_bytes: FIRST_FILE.len() + 1,
        end: super::RawEnd { bytes: 0, records: 0, sections: 0, hunks: 0 },
        pending: None, section: None, hunk: None, header: None, marker_opened: false,
    };
    let Err(error) = observer.finish(&ParsedDiff::default()) else {
        return Err("prefix-only completion succeeded".to_string());
    };
    assert!(error.contains("before exhaustive"));
    assert!(ledger.ends.is_empty());
    Ok(())
}

#[test]
fn no_path_or_closed_sections_cannot_anchor_later_plain_markers() -> Result<(), String> {
    for prefix in [
        "diff --git a/gone.rs b/gone.rs\n--- a/gone.rs\n+++ /dev/null\n@@ malformed @@\n",
        "diff --git a/link.rs b/link.rs\nnew file mode 120000\n--- /dev/null\n+++ b/link.rs\n@@ malformed @@\n",
        "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/../../escape.rs\n@@ malformed @@\n",
        "diff --git a/blob.bin b/blob.bin\nBinary files a/blob.bin and b/blob.bin differ\n",
        "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ malformed @@\n",
    ] {
        let input = format!("{prefix}--- a/next.rs\n+++ b/next.rs\n@@ -0,0 +1 @@\n+x\n");
        let (_, ledger) = observed(input.as_bytes(), 2)?;
        let tail = &ledger.reductions[ledger.reductions.len() - 4..];
        for reduction in tail {
            assert_eq!(reduction.section, None);
        }
        assert_eq!(tail[3].coordinates, Some((0, 1)));
        assert_eq!(tail[3].projection, Some((super::RawChangeSide::Added, 0)));
    }
    Ok(())
}

#[test]
fn unsupported_original_path_token_is_explicit_and_not_lossy_identity() -> Result<(), String> {
    let input = b"diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/\xff.rs\n@@ -0,0 +1 @@\n+x\n";
    let (_, ledger) = observed(input, 1)?;
    let marker = &ledger.reductions[2];
    assert_eq!(marker.token.as_deref(), Some(b"b/\xff.rs".as_slice()));
    assert!(marker.decoded.is_none());
    // Original capture is accounted; this is unsupported token evidence.
    // Semantic/native acceptance is advisory and cannot authenticate identity.
    assert_eq!(marker.section, Some(0));
    Ok(())
}

#[test]
fn overflowing_declared_ranges_have_no_raw_coordinates() -> Result<(), String> {
    let input = format!("diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -0,0 +{},2 @@\n+first\n+second\n", usize::MAX - 1);
    let (_, ledger) = observed(input.as_bytes(), 1)?;
    for reduction in &ledger.reductions[4..] {
        assert!(reduction.coordinates.is_none());
    }
    assert_eq!(ledger.reductions[4].projection, Some((super::RawChangeSide::Added, 0)));
    assert_eq!(ledger.reductions[5].projection, None);
    assert_eq!(ledger.reductions[5].kind, super::RawRecordKind::Body(super::BodyDisposition::CoordinateOverflow));
    Ok(())
}

#[test]
fn reduction_kinds_and_associated_fields_follow_actual_winners() -> Result<(), String> {
    use super::{BodyDisposition as B, PathMarkerOutcome as M, RawRecordKind as K};
    for (input, expected) in [
        ("diff --git a/a.bin b/a.bin\nBinary files a/a.bin and b/a.bin differ\n",
         vec![K::GitBoundary, K::Binary]),
        ("diff --git a/lib b/lib\nnew file mode 160000\nindex 1111111..2222222 160000\n",
         vec![K::GitBoundary, K::SubmoduleMode, K::SubmoduleIndex]),
        ("diff --git a/old.rs b/new.rs\nsimilarity index 100%\nrename from old.rs\nrename to new.rs\ncopy from opaque.rs\n",
         vec![K::GitBoundary, K::Rename, K::Rename, K::Rename, K::Body(B::Outside)]),
        ("diff --cc a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@@ -1,1 -1,1 +1,1 @@@\n++quarantined\n",
         vec![K::GitBoundary, K::PathMarker(M::Old), K::PathMarker(M::New { opened: true }), K::CombinedHunk, K::Body(B::Outside)]),
        ("diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ malformed @@\n+ignored\n",
         vec![K::GitBoundary, K::PathMarker(M::Old), K::PathMarker(M::New { opened: true }), K::MalformedHunk, K::Body(B::Outside)]),
        ("diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -0,0 +1,3 @@\n+<<<<<<< ours\n+conflict\n+>>>>>>> theirs\n",
         vec![K::GitBoundary, K::PathMarker(M::Old), K::PathMarker(M::New { opened: true }), K::HunkHeader, K::Body(B::Conflict), K::Body(B::Conflict), K::Body(B::Conflict)]),
        ("diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -0,0 +1 @@\n+x\n\\ No newline at end of file\n",
         vec![K::GitBoundary, K::PathMarker(M::Old), K::PathMarker(M::New { opened: true }), K::HunkHeader, K::Body(B::Added(0)), K::Body(B::NoNewline)]),
    ] {
        let (_, ledger) = observed(input.as_bytes(), 1)?;
        assert_eq!(ledger.reductions.iter().map(|r| r.kind).collect::<Vec<_>>(), expected);
        let header = &ledger.reductions[0];
        assert_eq!(header.section, Some(0));
        assert!(header.hunk.is_none());
        assert!(header.token.is_none());
        assert!(header.decoded.is_none());
        assert!(header.path.is_none());
        assert!(header.projection.is_none());
        assert!(header.ranges.is_none());
        for reduction in &ledger.reductions {
            match reduction.kind {
                K::PathMarker(M::Old) => {
                    assert_eq!(reduction.token.as_deref(), Some(b"a/a.rs".as_slice()));
                    assert_eq!(reduction.decoded.as_deref(), Some(b"a/a.rs".as_slice()));
                    assert!(reduction.projection.is_none());
                }
                K::PathMarker(M::New { opened: true }) => {
                    assert_eq!(reduction.token.as_deref(), Some(b"b/a.rs".as_slice()));
                    assert_eq!(reduction.decoded.as_deref(), Some(b"b/a.rs".as_slice()));
                    assert_eq!(reduction.path.as_deref(), Some(Path::new("a.rs")));
                    assert!(reduction.projection.is_none());
                }
                K::Body(B::Conflict) => {
                    assert_eq!(reduction.ranges, Some(((0, 0), (1, 3))));
                    assert_eq!(reduction.consumed, Some((0, 1)));
                    assert_eq!(reduction.path.as_deref(), Some(Path::new("a.rs")));
                    assert!(reduction.projection.is_none());
                }
                K::Body(B::Added(index)) => {
                    assert_eq!(reduction.projection, Some((super::RawChangeSide::Added, index)));
                    assert_eq!(reduction.coordinates, Some((0, 1)));
                }
                K::Body(B::NoNewline) => {
                    assert_eq!(reduction.before, Some((0, 0)));
                    assert_eq!(reduction.after, Some((0, 0)));
                    assert!(reduction.consumed.is_none());
                    assert!(reduction.coordinates.is_none());
                    assert!(reduction.projection.is_none());
                }
                K::Binary | K::SubmoduleMode | K::SubmoduleIndex
                | K::CombinedHunk | K::MalformedHunk | K::Body(B::Outside) => {
                    assert!(reduction.ranges.is_none());
                    assert!(reduction.coordinates.is_none());
                    assert!(reduction.projection.is_none());
                }
                _ => {}
            }
        }
        if input.contains("rename from ") {
            assert_eq!(ledger.reductions[2].token.as_deref(), Some(b"old.rs".as_slice()));
            assert_eq!(ledger.reductions[2].decoded.as_deref(), Some(b"old.rs".as_slice()));
            assert_eq!(ledger.reductions[3].token.as_deref(), Some(b"new.rs".as_slice()));
            assert_eq!(ledger.reductions[3].decoded.as_deref(), Some(b"new.rs".as_slice()));
            assert_eq!(ledger.reductions[3].path.as_deref(), Some(Path::new("new.rs")));
        }
    }
    Ok(())
}

#[test]
fn section_hunk_overflow_and_finish_refusal_never_return_success() -> Result<(), String> {
    use super::ParserObserver;
    for section in [false, true] {
        let mut ledger = ByteLedger::default();
        let mut observer = super::OriginalByteObserver {
            observer: &mut ledger, expected_bytes: 1,
            end: super::RawEnd { bytes: 0, records: 0,
                sections: if section { usize::MAX } else { 0 },
                hunks: if section { 0 } else { usize::MAX } },
            pending: None, section: None, hunk: None, header: None, marker_opened: false,
        };
        let line = super::RawDiffRecord { bytes: b"x" };
        observer.begin(&line)?;
        let reduction = if section { super::Reduction::GitBoundary }
            else { super::Reduction::Hunk(super::HunkOutcome::Malformed) };
        let Err(error) = observer.reduce(&line, reduction, &super::parser_state::ParserState::default()) else {
            return Err("checked section/hunk ordinal overflow succeeded".to_string());
        };
        assert!(error.contains("ordinal overflow"));
        assert!(ledger.reductions.is_empty());
        assert!(ledger.ends.is_empty());
    }
    let mut ledger = ByteLedger { refuse_finish: true, ..ByteLedger::default() };
    let Err(error) = super::parse_bytes_bounded(FIRST_FILE.as_bytes(), 1, &mut ledger) else {
        return Err("finish refusal returned a successful projection".to_string());
    };
    assert_eq!(error, "finish sink refused");
    assert_eq!(ledger.records.len(), 6);
    assert_eq!(ledger.reductions.len(), 6);
    assert!(ledger.ends.is_empty());
    Ok(())
}
