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
