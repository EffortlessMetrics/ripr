use std::collections::BTreeMap;
use std::path::PathBuf;

use super::model::{ChangedFile, ChangedLine};
use super::path::{parse_new_path_marker, parse_old_path_marker};

pub fn parse_unified_diff(input: &str) -> Vec<ChangedFile> {
    let mut files: BTreeMap<PathBuf, ChangedFile> = BTreeMap::new();
    let mut state = parser_state::ParserState::default();

    // Collect lines so we can peek at the next line for the RANK-2 fix:
    // when `in_hunk` and we see `--- <plausible-path>` immediately followed by
    // `+++ <path>`, we must close the current hunk and open a new file section
    // rather than misinterpreting the markers as hunk-body payload.
    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let raw = lines[i];

        if state.handle_diff_boundary(raw) {
            i += 1;
            continue;
        }

        // RANK-2 fix: detect a plain-diff file-section boundary while in_hunk.
        // A genuine `--- payload` line inside a hunk starts with `-` (one dash)
        // and is consumed as a removed line.  A file-section separator starts
        // with `--- ` (three dashes + space) and is always followed immediately
        // by `+++ <path>`.  We peek at the next line before committing.
        if state.in_hunk()
            && parse_old_path_marker(raw)
            && lines
                .get(i + 1)
                .is_some_and(|next| parse_new_path_marker(next).is_some())
        {
            // This `--- ` line opens a new file section: close the current hunk
            // and fall through to the normal path-marker handler below.
            state.close_hunk();
        }

        if state.register_path_marker(raw, &mut files) {
            i += 1;
            continue;
        }

        if state.handle_hunk_header(raw) {
            i += 1;
            continue;
        }

        state.consume_hunk_line(raw, &mut files);
        i += 1;
    }

    files.into_values().collect()
}

fn parse_hunk_header(raw: &str) -> Option<(usize, usize)> {
    // Format: @@ -old,count +new,count @@ optional
    let mut parts = raw.split_whitespace();
    let _at = parts.next()?;
    let old = parts.next()?;
    let new = parts.next()?;
    Some((
        parse_start(old.trim_start_matches('-'))?,
        parse_start(new.trim_start_matches('+'))?,
    ))
}

fn parse_start(segment: &str) -> Option<usize> {
    let start = segment.split(',').next()?;
    start.parse::<usize>().ok()
}

mod parser_state {
    use super::{
        ChangedFile, ChangedLine, parse_hunk_header, parse_new_path_marker, parse_old_path_marker,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    #[derive(Default)]
    pub(super) struct ParserState {
        current_path: Option<PathBuf>,
        old_line: usize,
        new_line: usize,
        in_hunk: bool,
        saw_old_path_marker: bool,
    }

    impl ParserState {
        /// Whether the parser is currently inside a hunk body.
        pub(super) fn in_hunk(&self) -> bool {
            self.in_hunk
        }

        /// Close the current hunk without consuming a line.  Called by the
        /// outer loop when it detects a plain-diff file-section boundary while
        /// a hunk is still open (RANK-2 fix).
        pub(super) fn close_hunk(&mut self) {
            self.in_hunk = false;
            self.saw_old_path_marker = false;
        }

        pub(super) fn register_path_marker(
            &mut self,
            raw: &str,
            files: &mut BTreeMap<PathBuf, ChangedFile>,
        ) -> bool {
            if self.in_hunk {
                return false;
            }

            if parse_old_path_marker(raw) {
                self.saw_old_path_marker = true;
                return true;
            }

            let Some(path) = parse_new_path_marker(raw) else {
                return false;
            };
            if self.current_path.is_none() || self.saw_old_path_marker {
                self.current_path = Some(path.clone());
                files.entry(path.clone()).or_insert_with(|| ChangedFile {
                    path,
                    ..ChangedFile::default()
                });
            }
            self.saw_old_path_marker = false;
            true
        }

        pub(super) fn handle_diff_boundary(&mut self, raw: &str) -> bool {
            if !raw.starts_with("diff --git ") {
                return false;
            }
            self.current_path = None;
            self.in_hunk = false;
            self.saw_old_path_marker = false;
            true
        }

        pub(super) fn handle_hunk_header(&mut self, raw: &str) -> bool {
            if !raw.starts_with("@@") {
                return false;
            }
            self.saw_old_path_marker = false;
            if let Some((old_start, new_start)) = parse_hunk_header(raw) {
                self.old_line = old_start;
                self.new_line = new_start;
                self.in_hunk = true;
            } else {
                self.in_hunk = false;
            }
            true
        }

        pub(super) fn consume_hunk_line(
            &mut self,
            raw: &str,
            files: &mut BTreeMap<PathBuf, ChangedFile>,
        ) {
            if !self.in_hunk {
                self.saw_old_path_marker = false;
                return;
            }

            let Some(path) = self.current_path.clone() else {
                return;
            };
            let Some(file) = files.get_mut(&path) else {
                return;
            };

            if let Some(text) = raw.strip_prefix('+') {
                file.added_lines.push(ChangedLine {
                    line: self.new_line,
                    new_side_line: self.new_line,
                    text: text.to_string(),
                });
                self.new_line = self.new_line.saturating_add(1);
            } else if let Some(text) = raw.strip_prefix('-') {
                // RANK-1 fix: record both the old-side line (`line`) and the
                // current new-side position (`new_side_line`).  When an earlier
                // hunk has a non-zero net line-delta, `line != new_side_line`.
                // Callers that build a SourceLocation pointing into the NEW file
                // MUST use `new_side_line`; using `line` (the old-side counter)
                // would target the wrong position in the new file.
                file.removed_lines.push(ChangedLine {
                    line: self.old_line,
                    new_side_line: self.new_line,
                    text: text.to_string(),
                });
                self.old_line = self.old_line.saturating_add(1);
            } else if raw.starts_with(' ') || raw.is_empty() {
                self.old_line = self.old_line.saturating_add(1);
                self.new_line = self.new_line.saturating_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_added_lines() {
        let diff = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,2 +1,2 @@\n-a\n+b\n c\n";
        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, PathBuf::from("src/lib.rs"));
        assert_eq!(files[0].added_lines[0].line, 1);
        assert_eq!(files[0].added_lines[0].text, "b");
    }

    #[test]
    fn parses_removed_and_context_lines_across_multiple_hunks() {
        let diff = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -3,3 +3,3 @@\n old_keep\n-old_remove\n+new_add\n next_keep\n@@ -10,2 +10,3 @@\n-old_again\n+new_again\n+new_tail\n unchanged\n";

        let files = parse_unified_diff(diff);

        assert_eq!(files.len(), 1);
        let file = &files[0];
        assert_eq!(file.path, PathBuf::from("src/lib.rs"));
        assert_eq!(file.removed_lines.len(), 2);
        assert_eq!(file.removed_lines[0].line, 4);
        assert_eq!(file.removed_lines[0].text, "old_remove");
        assert_eq!(file.removed_lines[1].line, 10);
        assert_eq!(file.removed_lines[1].text, "old_again");

        assert_eq!(file.added_lines.len(), 3);
        assert_eq!(file.added_lines[0].line, 4);
        assert_eq!(file.added_lines[0].text, "new_add");
        assert_eq!(file.added_lines[1].line, 10);
        assert_eq!(file.added_lines[1].text, "new_again");
        assert_eq!(file.added_lines[2].line, 11);
        assert_eq!(file.added_lines[2].text, "new_tail");
    }

    #[test]
    fn ignores_headers_without_valid_hunk_coordinates() {
        let diff = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ malformed header @@\n-removed\n+added\n";

        let files = parse_unified_diff(diff);

        assert_eq!(files.len(), 1);
        let file = &files[0];
        assert!(file.removed_lines.is_empty());
        assert!(file.added_lines.is_empty());
    }

    #[test]
    fn tracks_multiple_files_in_single_diff() {
        let diff = "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,1 +1,1 @@\n-a\n+b\ndiff --git a/src/b.rs b/src/b.rs\n--- a/src/b.rs\n+++ b/src/b.rs\n@@ -5,1 +5,2 @@\n-old\n+new\n+extra\n";

        let files = parse_unified_diff(diff);

        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, PathBuf::from("src/a.rs"));
        assert_eq!(files[0].added_lines.len(), 1);
        assert_eq!(files[1].path, PathBuf::from("src/b.rs"));
        assert_eq!(files[1].added_lines.len(), 2);
    }

    #[test]
    fn ignores_diff_metadata_lines_that_start_with_pluses_or_dashes() {
        let diff = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,1 +1,1 @@\n-legacy\n+current\n";

        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].added_lines,
            vec![ChangedLine {
                line: 1,
                new_side_line: 1,
                text: "current".to_string()
            }]
        );
        assert_eq!(
            files[0].removed_lines,
            vec![ChangedLine {
                line: 1,
                new_side_line: 1,
                text: "legacy".to_string()
            }]
        );
    }

    #[test]
    fn parses_new_file_diff_with_dev_null_source() {
        let diff = "diff --git a/src/new.rs b/src/new.rs\nnew file mode 100644\n--- /dev/null\n+++ b/src/new.rs\n@@ -0,0 +1,2 @@\n+pub fn answer() -> u32 {\n+    42\n";

        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, PathBuf::from("src/new.rs"));
        assert_eq!(files[0].removed_lines.len(), 0);
        assert_eq!(files[0].added_lines.len(), 2);
        assert_eq!(files[0].added_lines[0].line, 1);
        assert_eq!(files[0].added_lines[1].line, 2);
    }

    #[test]
    fn parses_git_quoted_new_paths_with_spaces() {
        let diff = "diff --git \"a/src/price rules.rs\" \"b/src/price rules.rs\"\n--- \"a/src/price rules.rs\"\n+++ \"b/src/price rules.rs\"\n@@ -7,1 +7,1 @@\n-old\n+new\n";

        let files = parse_unified_diff(diff);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, PathBuf::from("src/price rules.rs"));
        assert_eq!(files[0].removed_lines[0].line, 7);
        assert_eq!(files[0].removed_lines[0].text, "old");
        assert_eq!(files[0].added_lines[0].line, 7);
        assert_eq!(files[0].added_lines[0].text, "new");
    }

    #[test]
    fn parses_git_quoted_new_paths_with_escaped_characters() {
        let diff = "diff --git \"a/src/tab\\tquote\\\".rs\" \"b/src/tab\\tquote\\\".rs\"\n--- \"a/src/tab\\tquote\\\".rs\"\n+++ \"b/src/tab\\tquote\\\".rs\"\n@@ -1,1 +1,1 @@\n-old\n+new\n";

        let files = parse_unified_diff(diff);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, PathBuf::from("src/tab\tquote\".rs"));
        assert_eq!(files[0].added_lines[0].line, 1);
    }

    #[test]
    fn parses_git_quoted_new_paths_with_octal_escapes() {
        let diff = "diff --git \"a/src/price\\040rules.rs\" \"b/src/price\\040rules.rs\"\n--- \"a/src/price\\040rules.rs\"\n+++ \"b/src/price\\040rules.rs\"\n@@ -1,1 +1,1 @@\n-old\n+new\n";

        let files = parse_unified_diff(diff);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, PathBuf::from("src/price rules.rs"));
        assert_eq!(files[0].added_lines[0].line, 1);
    }

    #[test]
    fn ignores_unclosed_quoted_new_path_marker() {
        let diff = "diff --git \"a/src/lib.rs\" \"b/src/lib.rs\"\n--- \"a/src/lib.rs\"\n+++ \"b/src/lib.rs\n@@ -1,1 +1,1 @@\n-old\n+new\n";

        let files = parse_unified_diff(diff);

        assert!(files.is_empty());
    }

    #[test]
    fn parses_unquoted_new_paths_with_tab_metadata() {
        let diff =
            "--- src/lib.rs\t2026-01-01\n+++ src/lib.rs\t2026-01-02\n@@ -2,1 +2,1 @@\n-old\n+new\n";

        let files = parse_unified_diff(diff);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, PathBuf::from("src/lib.rs"));
        assert_eq!(files[0].added_lines[0].line, 2);
    }

    #[test]
    fn keeps_payload_that_looks_like_file_markers_in_current_hunk() {
        let diff = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,2 +1,2 @@\n old\n--- removed payload not a file marker\n+++ added payload not a file marker\n";

        let files = parse_unified_diff(diff);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, PathBuf::from("src/lib.rs"));
        assert_eq!(files[0].removed_lines.len(), 1);
        assert_eq!(files[0].removed_lines[0].line, 2);
        assert_eq!(
            files[0].removed_lines[0].text,
            "-- removed payload not a file marker"
        );
        assert_eq!(files[0].added_lines.len(), 1);
        assert_eq!(files[0].added_lines[0].line, 2);
        assert_eq!(
            files[0].added_lines[0].text,
            "++ added payload not a file marker"
        );
    }

    #[test]
    fn malformed_hunk_header_resets_hunk_state() {
        let diff = "diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,1 +1,1 @@
-old
+new
@@ malformed header @@
--- metadata should be ignored
+++ metadata should be ignored
+line should be ignored
-dropped should be ignored
";

        let files = parse_unified_diff(diff);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].added_lines.len(), 1);
        assert_eq!(files[0].removed_lines.len(), 1);
    }

    #[test]
    fn malformed_hunk_header_allows_following_plain_file_section() {
        let diff = "--- src/a.rs
+++ src/a.rs
@@ -1,1 +1,1 @@
-old a
+new a
@@ malformed header @@
--- metadata should be ignored
+++ metadata should be ignored
--- src/b.rs
+++ src/b.rs
@@ -5,1 +5,1 @@
-old b
+new b
";

        let files = parse_unified_diff(diff);

        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, PathBuf::from("src/a.rs"));
        assert_eq!(files[0].added_lines.len(), 1);
        assert_eq!(files[0].removed_lines.len(), 1);
        assert_eq!(files[1].path, PathBuf::from("src/b.rs"));
        assert_eq!(files[1].added_lines[0].line, 5);
        assert_eq!(files[1].added_lines[0].text, "new b");
        assert_eq!(files[1].removed_lines[0].line, 5);
        assert_eq!(files[1].removed_lines[0].text, "old b");
    }

    #[test]
    fn valid_hunk_after_malformed_hunk_still_parses() {
        let diff = "diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ malformed header @@
+ignored
-dropped
@@ -4,1 +4,1 @@
-old
+new
";

        let files = parse_unified_diff(diff);

        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].removed_lines,
            vec![ChangedLine {
                line: 4,
                new_side_line: 4,
                text: "old".to_string()
            }]
        );
        assert_eq!(
            files[0].added_lines,
            vec![ChangedLine {
                line: 4,
                new_side_line: 4,
                text: "new".to_string()
            }]
        );
    }

    #[test]
    fn ignores_deleted_file_hunks_without_new_path_marker() {
        let diff = "diff --git a/src/old.rs b/src/old.rs
deleted file mode 100644
--- a/src/old.rs
+++ /dev/null
@@ -1,2 +0,0 @@
-old
-lines
";

        let files = parse_unified_diff(diff);
        assert!(files.is_empty());
    }

    #[test]
    fn ignores_file_sections_without_plus_plus_plus_b_header() {
        let diff = "diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
@@ -1,1 +1,1 @@
-old
+new
";

        let files = parse_unified_diff(diff);
        assert!(files.is_empty());
    }

    // RANK-1 regression test (#1222): when an earlier hunk has a non-zero net
    // line delta, the old-side line counter and the new-side line counter
    // diverge.  A removed line in the later hunk must record the CORRECT
    // new-side coordinate in `new_side_line`, not just the old-side counter.
    #[test]
    fn removed_line_new_side_line_correct_after_net_delta_in_earlier_hunk() {
        // Hunk 1: replaces one line with three lines (net +2).
        // Hunk 2: changes a line in a different function.
        //   Old-side starts at line 5, new-side starts at line 7.
        //   Context ` pub fn two...` advances both to old=6, new=8.
        //   The removed `-    if x > 0 {` is at old-side 6, new-side 8.
        let diff = concat!(
            "diff --git a/src/lib.rs b/src/lib.rs\n",
            "--- a/src/lib.rs\n",
            "+++ b/src/lib.rs\n",
            "@@ -1,4 +1,6 @@\n",
            " pub fn one(x: i32) -> i32 {\n",
            "-    x + 1\n",
            "+    let y = x + 1;\n",
            "+    let z = y * 2;\n",
            "+    z\n",
            " }\n",
            "@@ -5,4 +7,4 @@\n",
            " pub fn two(x: i32) -> bool {\n",
            "-    if x > 0 {\n",
            "+    if x >= 0 {\n",
            "     true\n",
            " } else {\n",
        );

        let files = parse_unified_diff(diff);

        assert_eq!(files.len(), 1);
        let file = &files[0];

        // The removed line `if x > 0 {` is at OLD-side line 6, but the
        // corresponding new-side position is 8 (shifted by +2 from hunk 1).
        let removed = file
            .removed_lines
            .iter()
            .find(|l| l.text == "    if x > 0 {");
        assert!(
            removed.is_some(),
            "should have the removed predicate line; got: {:?}",
            file.removed_lines
        );
        if let Some(removed) = removed {
            assert_eq!(removed.line, 6, "old-side line must be 6");
            assert_eq!(
                removed.new_side_line, 8,
                "new_side_line must be 8 (shifted +2 by hunk 1); \
                 using old-side 6 would point at the wrong location in the new file"
            );
        }
    }

    // RANK-2 regression test (#1222): a plain unified diff (no `diff --git`
    // headers) whose second file section opens while the first hunk is still
    // open must be recognized as a file-section boundary, not treated as a
    // removed hunk-body line.
    #[test]
    fn plain_diff_two_file_sections_while_in_hunk_recognized_as_boundary() {
        let diff = concat!(
            "--- a/src/a.rs\n",
            "+++ b/src/a.rs\n",
            "@@ -5,4 +5,4 @@\n",
            " pub fn beta(x: i32) -> bool {\n",
            "-    if x > 0 {\n",
            "+    if x >= 0 {\n",
            "     true\n",
            " } else {\n",
            "--- a/src/b.rs\n",
            "+++ b/src/b.rs\n",
            "@@ -5,4 +5,4 @@\n",
            " pub fn delta(x: i32) -> bool {\n",
            "-    if x > 0 {\n",
            "+    if x >= 0 {\n",
            "     true\n",
            " } else {\n",
        );

        let files = parse_unified_diff(diff);

        assert_eq!(files.len(), 2, "both file sections must be parsed");
        let a = files
            .iter()
            .find(|f| f.path == std::path::Path::new("src/a.rs"));
        let b = files
            .iter()
            .find(|f| f.path == std::path::Path::new("src/b.rs"));

        assert!(a.is_some(), "src/a.rs must be present");
        assert!(b.is_some(), "src/b.rs must be present");

        if let (Some(a), Some(b)) = (a, b) {
            // src/a.rs changes
            assert_eq!(a.added_lines.len(), 1);
            assert_eq!(a.removed_lines.len(), 1);
            assert_eq!(a.removed_lines[0].text, "    if x > 0 {");
            assert_eq!(a.added_lines[0].text, "    if x >= 0 {");

            // src/b.rs changes — must NOT be attributed to a.rs
            assert_eq!(b.added_lines.len(), 1);
            assert_eq!(b.removed_lines.len(), 1);
            assert_eq!(b.removed_lines[0].text, "    if x > 0 {");
            assert_eq!(b.added_lines[0].text, "    if x >= 0 {");

            // No phantom path-marker text in any added or removed line
            let phantom = a
                .added_lines
                .iter()
                .chain(a.removed_lines.iter())
                .find(|l| l.text.contains("src/b.rs") || l.text.contains("++ b/"));
            assert!(
                phantom.is_none(),
                "no probe should contain path-marker text, got: {phantom:?}"
            );
        } // end if let (Some(a), Some(b))
    }

    #[test]
    fn parser_is_robust_against_fuzz_like_inputs() {
        let mut seed = 0xC0FFEE_u64;

        for case in 0..4_096 {
            let text = if case % 2 == 0 {
                fuzz_case_as_raw_bytes(&mut seed)
            } else {
                fuzz_case_as_diff_like_lines(&mut seed)
            };
            assert_parser_invariants(&text);
        }
    }

    #[test]
    fn parser_is_robust_against_adversarial_diff_corpus() {
        let mut seed = 0xDEADBEEF_u64;
        for _ in 0..512 {
            let text = fuzz_case_as_adversarial_diff(&mut seed);
            assert_parser_invariants(&text);
        }
    }

    #[test]
    fn parser_preserves_invariants_for_structured_adversarial_regressions() {
        for text in structured_adversarial_diff_regressions() {
            assert_parser_invariants(&text);
        }
    }

    fn fuzz_case_as_raw_bytes(seed: &mut u64) -> String {
        let len = (next_u64(seed) % 768) as usize;
        let mut bytes = Vec::with_capacity(len);
        for _ in 0..len {
            bytes.push((next_u64(seed) & 0xFF) as u8);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn fuzz_case_as_diff_like_lines(seed: &mut u64) -> String {
        const PREFIXES: &[&str] = &[
            "diff --git a/src/lib.rs b/src/lib.rs",
            "--- a/src/lib.rs",
            "+++ b/src/lib.rs",
            "@@ -1,2 +1,2 @@",
            "@@ malformed @@",
            "+",
            "-",
            " ",
            "",
            "Binary files a/a and b/a differ",
        ];

        let line_count = (next_u64(seed) % 96 + 1) as usize;
        let mut out = String::new();
        for _ in 0..line_count {
            let prefix = PREFIXES[(next_u64(seed) % PREFIXES.len() as u64) as usize];
            out.push_str(prefix);
            let tail_len = (next_u64(seed) % 48) as usize;
            for _ in 0..tail_len {
                let ch = (next_u64(seed) & 0x7f) as u8;
                if ch != b'\n' {
                    out.push(ch as char);
                }
            }
            out.push('\n');
        }
        out
    }

    fn fuzz_case_as_adversarial_diff(seed: &mut u64) -> String {
        const FILE_PATHS: &[&str] = &[
            "src/lib.rs",
            "src/mod.rs",
            "src/nested/deep/file.rs",
            "src/unicode_named.rs",
            "src/contains spaces.rs",
        ];
        const HUNK_HEADERS: &[&str] = &[
            "@@ -1,1 +1,1 @@",
            "@@ -0,0 +1,99999999 @@",
            "@@ -99999999,1 +0,0 @@",
            "@@ -18446744073709551615,2 +18446744073709551615,2 @@",
            "@@ malformed @@",
            "@@ -x,y +q,z @@",
        ];
        const CONTENT_PREFIXES: &[&str] = &["+ ", "- ", "  ", "", "\\ No newline at end of file"];

        let file_count = (next_u64(seed) % 6 + 1) as usize;
        let mut out = String::new();
        for _ in 0..file_count {
            let path = FILE_PATHS[(next_u64(seed) % FILE_PATHS.len() as u64) as usize];
            out.push_str(&format!("diff --git a/{path} b/{path}\n"));
            out.push_str(&format!("--- a/{path}\n"));
            out.push_str(&format!("+++ b/{path}\n"));

            let hunk_count = (next_u64(seed) % 4 + 1) as usize;
            for _ in 0..hunk_count {
                let header = HUNK_HEADERS[(next_u64(seed) % HUNK_HEADERS.len() as u64) as usize];
                out.push_str(header);
                out.push('\n');

                let line_count = (next_u64(seed) % 20 + 1) as usize;
                for _ in 0..line_count {
                    let prefix =
                        CONTENT_PREFIXES[(next_u64(seed) % CONTENT_PREFIXES.len() as u64) as usize];
                    out.push_str(prefix);
                    let tail_len = (next_u64(seed) % 40) as usize;
                    for _ in 0..tail_len {
                        let byte = (next_u64(seed) & 0xFF) as u8;
                        if byte != b'\n' {
                            out.push(byte as char);
                        }
                    }
                    out.push('\n');
                }
            }
        }
        out
    }

    fn structured_adversarial_diff_regressions() -> Vec<String> {
        vec![
            format!(
                "diff --git a/{name} b/{name}\n--- a/{name}\n+++ b/{name}\n@@ -1,1 +1,1 @@\n-{removed}\n+{added}\n",
                name = "src/".to_string() + &"a".repeat(512) + ".rs",
                removed = "x".repeat(4096),
                added = "y".repeat(4096)
            ),
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,3 @@\n-a\r\n+b\r\n c\r\n".to_string(),
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,4 +1,4 @@\n-diff --git not a real header\n+@@ -999,999 +999,999 @@\n-+++ should stay payload\n+--- should stay payload\n".to_string(),
            "diff --git a/src/a.rs b/src/z.rs\nsimilarity index 80%\nrename from src/a.rs\nrename to src/z.rs\n--- a/src/a.rs\n+++ b/src/z.rs\n@@ malformed @@\n+line\n-dropped\ndiff --git a/src/b.rs b/src/b.rs\n--- a/src/b.rs\n+++ b/src/b.rs\n@@ -0,0 +1,1 @@\n+new\n".to_string(),
        ]
    }

    fn assert_parser_invariants(text: &str) {
        let files = parse_unified_diff(text);
        for file in files {
            assert!(!file.path.as_os_str().is_empty());
            assert!(
                file.added_lines
                    .iter()
                    .all(|line| !line.text.contains('\n'))
            );
            assert!(
                file.removed_lines
                    .iter()
                    .all(|line| !line.text.contains('\n'))
            );
        }
    }

    #[test]
    fn parser_handles_hunk_line_numbers_near_usize_max() {
        let diff = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -18446744073709551615,2 +18446744073709551615,2 @@\n-a\n+b\n c\n";
        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 1);
        let file = &files[0];
        assert_eq!(file.added_lines.len(), 1);
        assert_eq!(file.removed_lines.len(), 1);
        assert_eq!(file.added_lines[0].line, usize::MAX);
        assert_eq!(file.removed_lines[0].line, usize::MAX);
    }

    fn next_u64(seed: &mut u64) -> u64 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *seed
    }
}
