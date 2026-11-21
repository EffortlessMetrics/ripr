use std::path::Path;

use crate::analysis_outcome::{AnalysisLimitation, AnalysisLimitationKind, AnalysisStage};

use super::parse::{ParsedDiff, parse_unified_diff_with_metadata};

fn limitation_of_kind(
    parsed: &ParsedDiff,
    kind: AnalysisLimitationKind,
) -> Result<&AnalysisLimitation, String> {
    parsed
        .limitations
        .iter()
        .find(|limitation| limitation.kind == kind)
        .ok_or_else(|| format!("missing limitation {kind:?}: {:#?}", parsed.limitations))
}

fn changed_file<'a>(parsed: &'a ParsedDiff, path: &str) -> Result<&'a super::ChangedFile, String> {
    parsed
        .changed_files
        .iter()
        .find(|file| file.path == Path::new(path))
        .ok_or_else(|| format!("missing changed file {path}: {:#?}", parsed.changed_files))
}

#[test]
fn combined_cc_hunk_is_quarantined_and_typed() -> Result<(), String> {
    let parsed = parse_unified_diff_with_metadata(
        "diff --cc src/lib.rs\n\
         index 1111111,2222222..3333333\n\
         --- a/src/lib.rs\n\
         +++ b/src/lib.rs\n\
         @@@ -1,1 -1,1 +1,1 @@@\n\
         --left parent\n\
         - right parent\n\
         ++merged result\n",
    );

    let limitation = limitation_of_kind(&parsed, AnalysisLimitationKind::CombinedHunkUnsupported)?;
    assert_eq!(limitation.producer_stage, AnalysisStage::DiffParse);
    assert_eq!(limitation.path.as_deref(), Some("src/lib.rs"));
    assert_eq!(limitation.affected_items, Some(1));
    let file = changed_file(&parsed, "src/lib.rs")?;
    assert!(file.added_lines.is_empty());
    assert!(file.removed_lines.is_empty());
    Ok(())
}

#[test]
fn explicit_combined_hunk_uses_the_same_typed_limitation() -> Result<(), String> {
    let parsed = parse_unified_diff_with_metadata(
        "diff --combined src/combined.rs\n\
         index 1111111,2222222..3333333\n\
         --- a/src/combined.rs\n\
         +++ b/src/combined.rs\n\
         @@@ -4,1 -4,1 +4,1 @@@\n\
         --old\n\
         ++new\n",
    );

    let limitation = limitation_of_kind(&parsed, AnalysisLimitationKind::CombinedHunkUnsupported)?;
    assert_eq!(limitation.path.as_deref(), Some("src/combined.rs"));
    assert_eq!(parsed.limitations.len(), 1);
    Ok(())
}

#[test]
fn combined_hunk_does_not_consume_the_following_ordinary_file() -> Result<(), String> {
    let parsed = parse_unified_diff_with_metadata(
        "diff --cc src/merge.rs\n\
         --- a/src/merge.rs\n\
         +++ b/src/merge.rs\n\
         @@@ -1,1 -1,1 +1,1 @@@\n\
         --parent\n\
         ++merge\n\
         diff --git a/src/lib.rs b/src/lib.rs\n\
         --- a/src/lib.rs\n\
         +++ b/src/lib.rs\n\
         @@ -1,1 +1,1 @@\n\
         -old\n\
         +new\n",
    );

    let limitation = limitation_of_kind(&parsed, AnalysisLimitationKind::CombinedHunkUnsupported)?;
    assert_eq!(limitation.path.as_deref(), Some("src/merge.rs"));
    let ordinary = changed_file(&parsed, "src/lib.rs")?;
    assert_eq!(ordinary.removed_lines.len(), 1);
    assert_eq!(ordinary.removed_lines[0].text, "old");
    assert_eq!(ordinary.added_lines.len(), 1);
    assert_eq!(ordinary.added_lines[0].text, "new");
    Ok(())
}

#[test]
fn ordinary_file_before_combined_hunk_keeps_path_attribution_separate() -> Result<(), String> {
    let parsed = parse_unified_diff_with_metadata(
        "diff --git a/src/lib.rs b/src/lib.rs\n\
         --- a/src/lib.rs\n\
         +++ b/src/lib.rs\n\
         @@ -1,1 +1,1 @@\n\
         -old\n\
         +new\n\
         diff --cc src/merge.rs\n\
         --- a/src/merge.rs\n\
         +++ b/src/merge.rs\n\
         @@@ -1,1 -1,1 +1,1 @@@\n\
         --parent\n\
         ++merge\n",
    );

    let limitation = limitation_of_kind(&parsed, AnalysisLimitationKind::CombinedHunkUnsupported)?;
    assert_eq!(limitation.path.as_deref(), Some("src/merge.rs"));
    let ordinary = changed_file(&parsed, "src/lib.rs")?;
    assert_eq!(ordinary.added_lines.len(), 1);
    assert_eq!(ordinary.removed_lines.len(), 1);
    Ok(())
}

#[test]
fn added_conflict_region_is_quarantined_and_typed() -> Result<(), String> {
    let parsed = parse_unified_diff_with_metadata(
        "diff --git a/src/lib.rs b/src/lib.rs\n\
         --- a/src/lib.rs\n\
         +++ b/src/lib.rs\n\
         @@ -1,1 +1,5 @@\n\
         +<<<<<<< ours\n\
         +let value = 1;\n\
         +=======\n\
         +let value = 2;\n\
         +>>>>>>> theirs\n",
    );

    let limitation =
        limitation_of_kind(&parsed, AnalysisLimitationKind::UnresolvedConflictMarkers)?;
    assert_eq!(limitation.path.as_deref(), Some("src/lib.rs"));
    assert_eq!(limitation.affected_items, Some(1));
    let file = changed_file(&parsed, "src/lib.rs")?;
    assert!(file.added_lines.is_empty());
    assert!(file.removed_lines.is_empty());
    Ok(())
}

#[test]
fn removed_conflict_region_is_quarantined_and_typed() -> Result<(), String> {
    let parsed = parse_unified_diff_with_metadata(
        "diff --git a/src/lib.rs b/src/lib.rs\n\
         --- a/src/lib.rs\n\
         +++ b/src/lib.rs\n\
         @@ -1,5 +1,1 @@\n\
         -<<<<<<< ours\n\
         -let value = 1;\n\
         -=======\n\
         -let value = 2;\n\
         ->>>>>>> theirs\n",
    );

    let limitation =
        limitation_of_kind(&parsed, AnalysisLimitationKind::UnresolvedConflictMarkers)?;
    assert_eq!(limitation.path.as_deref(), Some("src/lib.rs"));
    let file = changed_file(&parsed, "src/lib.rs")?;
    assert!(file.added_lines.is_empty());
    assert!(file.removed_lines.is_empty());
    Ok(())
}

#[test]
fn unclosed_conflict_region_remains_limited_at_end_of_input() -> Result<(), String> {
    let parsed = parse_unified_diff_with_metadata(
        "diff --git a/src/lib.rs b/src/lib.rs\n\
         --- a/src/lib.rs\n\
         +++ b/src/lib.rs\n\
         @@ -1,1 +1,3 @@\n\
         +<<<<<<< ours\n\
         +let value = 1;\n\
         +=======\n",
    );

    let limitation =
        limitation_of_kind(&parsed, AnalysisLimitationKind::UnresolvedConflictMarkers)?;
    assert_eq!(limitation.path.as_deref(), Some("src/lib.rs"));
    assert!(changed_file(&parsed, "src/lib.rs")?.added_lines.is_empty());
    Ok(())
}

#[test]
fn isolated_separator_and_marker_text_inside_source_do_not_false_trigger() -> Result<(), String> {
    let parsed = parse_unified_diff_with_metadata(
        "diff --git a/src/lib.rs b/src/lib.rs\n\
         --- a/src/lib.rs\n\
         +++ b/src/lib.rs\n\
         @@ -1,1 +1,3 @@\n\
         +=======\n\
         +let marker = \"<<<<<<< not a conflict region\";\n\
         +let arrows = \">>>>>>> also text\";\n",
    );

    assert!(parsed.limitations.is_empty());
    let file = changed_file(&parsed, "src/lib.rs")?;
    assert_eq!(file.added_lines.len(), 3);
    assert_eq!(file.added_lines[0].text, "=======");
    Ok(())
}

#[test]
fn ordinary_two_way_hunk_output_and_existing_counters_remain_unchanged() -> Result<(), String> {
    let parsed = parse_unified_diff_with_metadata(
        "diff --git a/src/lib.rs b/src/lib.rs\n\
         --- a/src/lib.rs\n\
         +++ b/src/lib.rs\n\
         @@ -4,1 +4,1 @@\n\
         -old\n\
         +new\n",
    );

    assert!(parsed.limitations.is_empty());
    assert_eq!(parsed.deleted_file_count, 0);
    assert_eq!(parsed.submodule_file_count, 0);
    assert_eq!(parsed.renamed_file_count, 0);
    assert_eq!(parsed.pure_rename_file_count, 0);
    assert!(parsed.pure_rename_paths.is_empty());
    let file = changed_file(&parsed, "src/lib.rs")?;
    assert_eq!(file.removed_lines[0].line, 4);
    assert_eq!(file.removed_lines[0].new_side_line, 4);
    assert_eq!(file.removed_lines[0].text, "old");
    assert_eq!(file.added_lines[0].line, 4);
    assert_eq!(file.added_lines[0].new_side_line, 4);
    assert_eq!(file.added_lines[0].text, "new");
    Ok(())
}

#[test]
fn combined_quarantine_recognizes_plain_file_pair_without_diff_header() -> Result<(), String> {
    let parsed = parse_unified_diff_with_metadata(
        "diff --cc src/merge.rs\n\
         --- a/src/merge.rs\n\
         +++ b/src/merge.rs\n\
         @@@ -1,1 -1,1 +1,1 @@@\n\
         --parent\n\
         ++merge\n\
         --- a/src/next.rs\n\
         +++ b/src/next.rs\n\
         @@ -1,1 +1,1 @@\n\
         -old\n\
         +new\n",
    );

    limitation_of_kind(&parsed, AnalysisLimitationKind::CombinedHunkUnsupported)?;
    let next = changed_file(&parsed, "src/next.rs")?;
    assert_eq!(next.removed_lines.len(), 1);
    assert_eq!(next.added_lines.len(), 1);
    Ok(())
}

#[test]
fn combined_quarantine_does_not_reinterpret_prefixed_path_marker_shaped_body_text()
-> Result<(), String> {
    let parsed = parse_unified_diff_with_metadata(
        "diff --cc src/merge.rs\n\
         --- a/src/merge.rs\n\
         +++ b/src/merge.rs\n\
         @@@ -1,1 -1,1 +1,1 @@@\n\
         ++--- a/src/fake.rs\n\
         +++++ b/src/fake.rs\n",
    );

    limitation_of_kind(&parsed, AnalysisLimitationKind::CombinedHunkUnsupported)?;
    assert_eq!(parsed.changed_files.len(), 1);
    if changed_file(&parsed, "src/fake.rs").is_ok() {
        return Err("quarantined source text created a fake changed file".to_string());
    }
    Ok(())
}

#[test]
fn configurable_ten_character_conflict_markers_are_quarantined() -> Result<(), String> {
    let parsed = parse_unified_diff_with_metadata(
        "diff --git a/.gitattributes b/.gitattributes\n\
         --- a/.gitattributes\n\
         +++ b/.gitattributes\n\
         @@ -1,1 +1,1 @@\n\
         +*.rs conflict-marker-size=10\n\
         diff --git a/src/lib.rs b/src/lib.rs\n\
         --- a/src/lib.rs\n\
         +++ b/src/lib.rs\n\
         @@ -1,1 +1,5 @@\n\
         +<<<<<<<<<< ours\n\
         +let value = 1;\n\
         +==========\n\
         +let value = 2;\n\
         +>>>>>>>>>> theirs\n",
    );

    limitation_of_kind(&parsed, AnalysisLimitationKind::UnresolvedConflictMarkers)?;
    assert!(changed_file(&parsed, "src/lib.rs")?.added_lines.is_empty());
    Ok(())
}

#[test]
fn context_line_conflict_markers_advance_coordinates_and_quarantine_region() -> Result<(), String> {
    let parsed = parse_unified_diff_with_metadata(
        "diff --git a/src/lib.rs b/src/lib.rs\n\
         --- a/src/lib.rs\n\
         +++ b/src/lib.rs\n\
         @@ -1,7 +1,7 @@\n\
         \x20<<<<<<< ours\n\
         \x20left\n\
         \x20=======\n\
         \x20right\n\
         \x20>>>>>>> theirs\n\
         -old\n\
         +new\n",
    );

    limitation_of_kind(&parsed, AnalysisLimitationKind::UnresolvedConflictMarkers)?;
    let file = changed_file(&parsed, "src/lib.rs")?;
    assert_eq!(file.removed_lines.len(), 1);
    assert_eq!(file.added_lines.len(), 1);
    assert_eq!(file.removed_lines[0].line, 6);
    assert_eq!(file.added_lines[0].new_side_line, 6);
    Ok(())
}
