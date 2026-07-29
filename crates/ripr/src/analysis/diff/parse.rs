use std::collections::BTreeMap;
use std::path::PathBuf;

use super::model::{ChangedFile, ChangedLine};
use super::path::{
    is_dev_null_new_path_marker, is_new_path_marker, parse_git_old_path, parse_new_path_marker,
    parse_old_path_for_confinement, parse_old_path_marker,
};

/// Default file-count limit for parsed diffs. Same default as the Rust adapter
/// (`analysis/language/rust.rs:DIFF_INDEX_FILE_LIMIT`); kept in sync so the
/// parser-level guard is consistent with the adapter-level guard (#2398).
const DEFAULT_DIFF_FILE_LIMIT: usize = 800;
const DIFF_FILE_LIMIT_ENV: &str = "RIPR_MAX_DIFF_INDEX_FILES";

pub(crate) fn parse_unified_diff_bounded_with_metadata(input: &str) -> Result<ParsedDiff, String> {
    let limit = diff_file_limit_from_env();
    parse_unified_diff_with_metadata_and_limit(input, limit)
}

/// Parse with an explicit file-count limit. Exposed for testing (#2398).
#[cfg(test)]
pub(crate) fn parse_unified_diff_with_limit(
    input: &str,
    limit: usize,
) -> Result<Vec<ChangedFile>, String> {
    Ok(parse_unified_diff_with_metadata_and_limit(input, limit)?.changed_files)
}

fn parse_unified_diff_with_metadata_and_limit(
    input: &str,
    limit: usize,
) -> Result<ParsedDiff, String> {
    let parsed = parse_unified_diff_with_metadata(input);
    if parsed.changed_files.len() > limit {
        return Err(format!(
            "diff_scope_oversized: {} changed files exceed the {DIFF_FILE_LIMIT_ENV} \
             limit ({limit}); analysis was not run to protect runner memory before \
             probe expansion. Repair route: reduce the diff scope, split the extraction \
             PR, run a narrower diff, or raise the limit via \
             {DIFF_FILE_LIMIT_ENV}=<number>.",
            parsed.changed_files.len()
        ));
    }
    Ok(parsed)
}

fn diff_file_limit_from_env() -> usize {
    std::env::var(DIFF_FILE_LIMIT_ENV)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_DIFF_FILE_LIMIT)
}

pub fn parse_unified_diff(input: &str) -> Vec<ChangedFile> {
    parse_unified_diff_with_metadata(input).changed_files
}

pub(crate) fn parse_unified_diff_with_metadata(input: &str) -> ParsedDiff {
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

        // Binary file sentinel: `Binary files a/x and b/x differ` (or
        // `/dev/null` variants) signals that this file has no textual hunks
        // and produces no analyzable line-level probes. Treat it as a hunk
        // closer so a following textual file-section is not mis-attributed to
        // the binary file's still-open hunk, and so the parser does not fall
        // through and consume the literal `Binary files ...` line as hunk
        // payload.
        if state.handle_binary_files_sentinel(raw) {
            state.record_binary_deletion(raw);
            i += 1;
            continue;
        }

        if state.handle_submodule_mode(raw) {
            i += 1;
            continue;
        }

        if state.handle_submodule_index(raw) {
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
                .is_some_and(|next| is_new_path_marker(next))
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

    ParsedDiff {
        changed_files: files.into_values().collect(),
        deleted_file_count: state.deleted_file_count(),
        submodule_file_count: state.submodule_file_count(),
    }
}

#[derive(Debug, Default)]
pub(crate) struct ParsedDiff {
    pub(crate) changed_files: Vec<ChangedFile>,
    pub(crate) deleted_file_count: usize,
    pub(crate) submodule_file_count: usize,
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
        ChangedFile, ChangedLine, is_dev_null_new_path_marker, is_new_path_marker,
        parse_git_old_path, parse_hunk_header, parse_new_path_marker,
        parse_old_path_for_confinement, parse_old_path_marker,
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
        section_old_path: Option<PathBuf>,
        deletion_section: bool,
        deletion_counted: bool,
        deleted_file_count: usize,
        submodule_section: bool,
        submodule_counted: bool,
        submodule_new_file: bool,
        submodule_file_count: usize,
    }

    impl ParserState {
        /// Whether the parser is currently inside a hunk body.
        pub(super) fn in_hunk(&self) -> bool {
            self.in_hunk
        }

        pub(super) fn deleted_file_count(&self) -> usize {
            self.deleted_file_count
        }

        pub(super) fn submodule_file_count(&self) -> usize {
            self.submodule_file_count
        }

        pub(super) fn handle_submodule_mode(&mut self, raw: &str) -> bool {
            if self.in_hunk {
                return false;
            }
            if matches!(raw, "new file mode 160000" | "deleted file mode 160000") {
                self.submodule_section = true;
                self.deletion_section = raw == "deleted file mode 160000";
                return true;
            }
            false
        }

        pub(super) fn handle_submodule_index(&mut self, raw: &str) -> bool {
            if self.in_hunk {
                return false;
            }
            let mut fields = raw.split_whitespace();
            let is_gitlink = fields.next() == Some("index")
                && fields.next().is_some_and(|range| range.contains(".."))
                && fields.next() == Some("160000")
                && fields.next().is_none();
            if is_gitlink {
                self.submodule_section = true;
            }
            is_gitlink
        }

        fn record_deleted_file_if_ready(&mut self) {
            if self.deletion_section && !self.deletion_counted && self.section_old_path.is_some() {
                self.deleted_file_count = self.deleted_file_count.saturating_add(1);
                self.deletion_counted = true;
            }
        }

        pub(super) fn record_binary_deletion(&mut self, raw: &str) {
            if raw.starts_with("Binary files ") && raw.ends_with(" and /dev/null differ") {
                self.deletion_section = true;
                self.record_deleted_file_if_ready();
            }
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

            if raw.starts_with("deleted file mode ") {
                self.deletion_section = true;
                self.record_deleted_file_if_ready();
                return true;
            }

            if parse_old_path_marker(raw) {
                self.saw_old_path_marker = true;
                self.section_old_path = parse_old_path_for_confinement(raw);
                self.submodule_new_file = raw == "--- /dev/null";
                self.record_deleted_file_if_ready();
                if self.submodule_section
                    && self.deletion_section
                    && !self.submodule_counted
                    && self.section_old_path.is_some()
                {
                    self.submodule_file_count = self.submodule_file_count.saturating_add(1);
                    self.submodule_counted = true;
                }
                return true;
            }

            let Some(path) = parse_new_path_marker(raw) else {
                if self.saw_old_path_marker && is_dev_null_new_path_marker(raw) {
                    self.deletion_section = true;
                    self.record_deleted_file_if_ready();
                }
                // A syntactically valid `+++` marker whose path was rejected
                // by confinement (or `/dev/null`): consume the marker and
                // clear the current file so the following hunk lines cannot
                // be mis-attributed to the previous file. consume_hunk_line
                // ignores hunk lines while current_path is None (#2099).
                if is_new_path_marker(raw) {
                    self.current_path = None;
                    self.saw_old_path_marker = false;
                    return true;
                }
                return false;
            };
            if self.submodule_section
                && !self.submodule_counted
                && (self.submodule_new_file || self.section_old_path.as_ref() == Some(&path))
            {
                self.submodule_file_count = self.submodule_file_count.saturating_add(1);
                self.submodule_counted = true;
            }
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
            // Recognise every form of `git diff` file-section boundary:
            //   diff --git a/x b/x      (the common two-way diff)
            //   diff --cc a/x b/x       (combined diff for a merge commit)
            //   diff --combined a/x b/x (explicit --combined flag)
            // Without the `--cc`/`--combined` cases, a merge-commit diff would
            // fall through: the `diff ` prefix is consumed by no handler, the
            // following `--- `/`+++ ` markers may be mis-attributed to the
            // previous file, and the parser silently produces zero or wrong
            // probes for the merge. Treat all three as the same boundary so
            // the parser closes any open hunk and resets file context.
            let is_boundary = raw.strip_prefix("diff --").is_some_and(|rest| {
                rest.starts_with("git ") || rest.starts_with("cc ") || rest.starts_with("combined ")
            });
            if !is_boundary {
                return false;
            }
            self.current_path = None;
            self.in_hunk = false;
            self.saw_old_path_marker = false;
            self.section_old_path = parse_git_old_path(raw);
            self.deletion_section = false;
            self.deletion_counted = false;
            self.submodule_section = false;
            self.submodule_counted = false;
            self.submodule_new_file = false;
            true
        }

        pub(super) fn handle_hunk_header(&mut self, raw: &str) -> bool {
            if !raw.starts_with("@@") {
                return false;
            }
            self.saw_old_path_marker = false;
            if let Some((old_start, new_start)) = parse_hunk_header(raw) {
                // Overflow guard: if either start coordinate is at usize::MAX,
                // the counter cannot advance and every line in this hunk would
                // be tagged with a meaningless line number. usize::MAX is never
                // a real source line, and downstream consumers (classifier,
                // probe generator) trust `line` unconditionally — emitting a
                // probe at usize::MAX produces a finding that points nowhere.
                // Drop the entire hunk (including the first line) by refusing
                // to enter it. The earlier fail-closed variant recorded the
                // first line at usize::MAX and then closed; this stricter
                // variant drops even that first line, because usize::MAX is
                // not an honest coordinate for any line. See the post-merge
                // review of #2050.
                if old_start == usize::MAX || new_start == usize::MAX {
                    self.in_hunk = false;
                    return true;
                }
                self.old_line = old_start;
                self.new_line = new_start;
                self.in_hunk = true;
            } else {
                self.in_hunk = false;
            }
            true
        }

        /// Detect the `Binary files a/x and b/x differ` sentinel git emits in
        /// place of a textual hunk when a file's bytes differ but cannot be
        /// shown as text. Returns `true` (and closes any open hunk) so the
        /// caller advances past the line; `false` otherwise.
        pub(super) fn handle_binary_files_sentinel(&mut self, raw: &str) -> bool {
            // `Binary files ` is git's literal prefix; the full line shape is:
            //   Binary files a/<path> and b/<path> differ
            //   Binary files a/<path> and /dev/null differ
            //   Binary files /dev/null and b/<path> differ
            // We do not try to register the path: ripr cannot extract line
            // probes from a binary blob, so the file is correctly treated as
            // having no changed lines. We only need to ensure that any open
            // hunk is closed so a later textual section is not mis-attributed.
            if !raw.starts_with("Binary files ") || !raw.ends_with(" differ") {
                return false;
            }
            self.in_hunk = false;
            self.saw_old_path_marker = false;
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
                // Fail closed on overflow: if the new-side counter is already
                // at usize::MAX (from a malicious or malformed @@ header), it
                // cannot advance. Earlier behaviour silently emitted every
                // subsequent line in this hunk tagged `line: usize::MAX`,
                // producing ownerless probes that masqueraded as a long run of
                // changes. Close the hunk instead so only the first overflowed
                // line is recorded (and the rest are dropped as ambiguous).
                if let Some(next) = self.new_line.checked_add(1) {
                    self.new_line = next;
                } else {
                    self.close_hunk();
                }
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
                if let Some(next) = self.old_line.checked_add(1) {
                    self.old_line = next;
                } else {
                    self.close_hunk();
                }
            } else if raw.starts_with(' ') || raw.is_empty() {
                if let (Some(o), Some(n)) =
                    (self.old_line.checked_add(1), self.new_line.checked_add(1))
                {
                    self.old_line = o;
                    self.new_line = n;
                } else {
                    // Either counter saturated: stop emitting misleading
                    // line numbers for the rest of this hunk.
                    self.close_hunk();
                }
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
    fn metadata_counts_deleted_file_sections_without_registering_new_files() {
        let deleted = "diff --git a/src/old.rs b/src/old.rs
deleted file mode 100644
--- a/src/old.rs
+++ /dev/null
@@ -1,2 +0,0 @@
-old
-lines
";
        let parsed = parse_unified_diff_with_metadata(deleted);
        assert!(parsed.changed_files.is_empty());
        assert_eq!(parsed.deleted_file_count, 1);

        let empty = "diff --git a/src/empty.rs b/src/empty.rs\ndeleted file mode 100644\n";
        let parsed = parse_unified_diff_with_metadata(empty);
        assert!(parsed.changed_files.is_empty());
        assert_eq!(parsed.deleted_file_count, 1);

        let binary = "diff --git a/src/blob.bin b/src/blob.bin\nBinary files a/src/blob.bin and /dev/null differ\n";
        let parsed = parse_unified_diff_with_metadata(binary);
        assert!(parsed.changed_files.is_empty());
        assert_eq!(parsed.deleted_file_count, 1);

        let changed = parse_unified_diff_with_metadata(
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,1 +1,1 @@\n-old\n+new\n",
        );
        assert_eq!(changed.deleted_file_count, 0);
        assert_eq!(changed.changed_files.len(), 1);
    }

    #[test]
    fn metadata_does_not_count_null_or_traversal_old_paths_as_deletions() {
        let new_file = "diff --git a/src/new.rs b/src/new.rs\nnew file mode 100644\n--- /dev/null\n+++ b/src/new.rs\n";
        assert_eq!(
            parse_unified_diff_with_metadata(new_file).deleted_file_count,
            0
        );

        let traversal = "diff --git a/src/../escape.rs b/src/../escape.rs\ndeleted file mode 100644\n--- a/../../../etc/passwd\n+++ /dev/null\n";
        assert_eq!(
            parse_unified_diff_with_metadata(traversal).deleted_file_count,
            0
        );
    }

    #[test]
    fn metadata_counts_confined_submodule_pointer_changes() {
        let submodule = "diff --git a/vendor/lib b/vendor/lib\nindex 1111111..2222222 160000\n--- a/vendor/lib\n+++ b/vendor/lib\n@@ -1 +1 @@\n-Subproject commit 1111111\n+Subproject commit 2222222\n";
        let parsed = parse_unified_diff_with_metadata(submodule);

        assert_eq!(parsed.submodule_file_count, 1);
        assert_eq!(parsed.changed_files.len(), 1);
        assert_eq!(parsed.changed_files[0].path, PathBuf::from("vendor/lib"));
    }

    #[test]
    fn metadata_counts_gitlink_additions_and_deletions() {
        let addition = "diff --git a/vendor/new b/vendor/new\nnew file mode 160000\nindex 0000000..2222222\n--- /dev/null\n+++ b/vendor/new\n";
        let parsed = parse_unified_diff_with_metadata(addition);
        assert_eq!(parsed.submodule_file_count, 1);
        assert_eq!(parsed.changed_files[0].path, PathBuf::from("vendor/new"));

        let deletion = "diff --git a/vendor/old b/vendor/old\ndeleted file mode 160000\nindex 1111111..0000000\n--- a/vendor/old\n+++ /dev/null\n";
        let parsed = parse_unified_diff_with_metadata(deletion);
        assert_eq!(parsed.submodule_file_count, 1);
        assert!(parsed.changed_files.is_empty());
    }

    #[test]
    fn metadata_does_not_count_non_gitlink_or_unconfined_submodule_markers() {
        let regular = "diff --git a/src/lib.rs b/src/lib.rs\nindex 1111111..2222222 100644\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n";
        assert_eq!(
            parse_unified_diff_with_metadata(regular).submodule_file_count,
            0
        );

        let traversal = "diff --git a/../escape b/../escape\nindex 1111111..2222222 160000\n--- a/../escape\n+++ b/../escape\n";
        let parsed = parse_unified_diff_with_metadata(traversal);
        assert_eq!(parsed.submodule_file_count, 0);
        assert!(parsed.changed_files.is_empty());
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
    fn parser_handles_hunk_line_numbers_at_usize_max() {
        // A hunk header whose start coordinate is usize::MAX cannot produce
        // any honest line number: usize::MAX is never a real source line,
        // and the counter cannot advance. The parser refuses to enter the
        // hunk at all, so NO changed line is recorded — not even the first.
        // This is stricter than the earlier fail-closed variant (which
        // recorded the first line at usize::MAX and then closed); the
        // stricter behaviour avoids emitting a probe at usize::MAX that
        // downstream consumers (classifier, probe generator) would trust
        // unconditionally. See the post-merge review of #2050.
        let diff = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -18446744073709551615,2 +18446744073709551615,2 @@\n+a\n-b\n c\n";
        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 1);
        let file = &files[0];
        // The entire hunk is dropped: no added, no removed lines.
        assert_eq!(file.added_lines.len(), 0);
        assert_eq!(file.removed_lines.len(), 0);
    }

    #[test]
    fn parser_treats_diff_cc_combined_diff_header_as_boundary() {
        // `diff --cc` is the combined diff git emits for merge commits. Before
        // the boundary recognition fix, the `diff ` prefix matched no handler,
        // the following `--- a/...`/`+++ b/...` markers were mis-attributed to
        // the previous file's still-open hunk, and the parser produced probes
        // against the wrong file (or zero probes for the merge). We now treat
        // `diff --cc` exactly like `diff --git`: it closes any open file/hunk
        // and resets context so the next `--- `/`+++ ` pair opens a fresh
        // section.
        let diff = "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,1 +1,1 @@\n-old\n+new\ndiff --cc a/src/merged.rs b/src/merged.rs\n--- a/src/merged.rs\n+++ b/src/merged.rs\n@@@ -1,1 -1,1 +1,1 @@@\n-old\n+new\n";
        let files = parse_unified_diff(diff);
        // The first file's textual hunk is parsed normally. The `diff --cc`
        // file's combined hunk header (`@@@ ... @@@`) is not a `@@` prefix, so
        // it produces no line probes — but the boundary close means the
        // following `--- a/src/merged.rs`/`+++ b/src/merged.rs` markers are
        // not mis-attributed to `src/a.rs`.
        let a = files
            .iter()
            .find(|f| f.path == std::path::Path::new("src/a.rs"));
        let merged = files
            .iter()
            .find(|f| f.path == std::path::Path::new("src/merged.rs"));
        assert!(a.is_some(), "src/a.rs registered: {files:?}");
        assert!(merged.is_some(), "src/merged.rs registered: {files:?}");
        if let (Some(a), Some(merged)) = (a, merged) {
            // The merged.rs path is registered via its `+++` marker (so it
            // appears in the file list) but carries no analyzable changed
            // lines.
            assert!(merged.added_lines.is_empty());
            assert!(merged.removed_lines.is_empty());
            // The a.rs hunk is intact.
            assert_eq!(a.added_lines.len(), 1);
            assert_eq!(a.removed_lines.len(), 1);
        }
    }

    #[test]
    fn parser_treats_diff_combined_explicit_header_as_boundary() {
        // `diff --combined` is the explicit form of `diff --cc`. Same boundary
        // treatment.
        let diff = "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,1 +1,1 @@\n-old\n+new\ndiff --combined a/src/merged.rs\n--- a/src/merged.rs\n+++ b/src/merged.rs\n";
        let files = parse_unified_diff(diff);
        assert!(
            files
                .iter()
                .any(|f| f.path == std::path::Path::new("src/a.rs"))
        );
        assert!(
            files
                .iter()
                .any(|f| f.path == std::path::Path::new("src/merged.rs"))
        );
    }

    #[test]
    fn parser_handles_binary_files_sentinel_and_closes_open_hunk() {
        // git emits `Binary files a/x and b/x differ` in place of a textual
        // hunk when a file's bytes differ. ripr cannot extract line probes
        // from a binary blob, so the file is correctly recorded with zero
        // changed lines. We must also ensure the sentinel closes any open hunk
        // so a following textual file is not mis-attributed.
        let diff = "diff --git a/binary.dat b/binary.dat\nBinary files a/binary.dat and b/binary.dat differ\ndiff --git a/src/text.rs b/src/text.rs\n--- a/src/text.rs\n+++ b/src/text.rs\n@@ -3,1 +3,1 @@\n-old\n+new\n";
        let files = parse_unified_diff(diff);
        // binary.dat is not registered: the `Binary files` line is consumed as
        // a sentinel and produces no `+++ b/binary.dat` marker that would
        // create a ChangedFile. (Even if git emitted a path marker, the file
        // would correctly carry no analyzable lines.)
        assert!(
            !files
                .iter()
                .any(|f| f.path == std::path::Path::new("binary.dat")),
            "binary sentinel must not register a textual ChangedFile: {files:?}"
        );
        // The following textual file is parsed normally — the sentinel closed
        // any open state from the previous file.
        let text = files
            .iter()
            .find(|f| f.path == std::path::Path::new("src/text.rs"));
        assert!(text.is_some(), "src/text.rs registered: {files:?}");
        if let Some(text) = text {
            assert_eq!(text.added_lines.len(), 1);
            assert_eq!(text.added_lines[0].line, 3);
            assert_eq!(text.removed_lines.len(), 1);
            assert_eq!(text.removed_lines[0].line, 3);
        }
    }

    #[test]
    fn parser_handles_binary_files_sentinel_with_dev_null() {
        // `/dev/null` variants: a newly-added or removed binary file.
        let diff = "diff --git a/new_blob.dat b/new_blob.dat\nnew file mode 100644\nBinary files /dev/null and b/new_blob.dat differ\ndiff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,1 +1,1 @@\n-old\n+new\n";
        let files = parse_unified_diff(diff);
        let lib = files
            .iter()
            .find(|f| f.path == std::path::Path::new("src/lib.rs"));
        assert!(lib.is_some(), "src/lib.rs registered: {files:?}");
        if let Some(lib) = lib {
            assert_eq!(lib.added_lines.len(), 1);
        }
    }

    #[test]
    fn parser_drops_all_lines_after_usize_max_overflow_in_hunk() {
        // Secondary overflow defense: a hunk header whose start is NEAR but
        // not AT usize::MAX (here usize::MAX - 2) enters the hunk normally,
        // but after the first few lines the counter saturates and the parser
        // closes the hunk fail-closed. This test exercises the checked_add
        // close-on-overflow path in consume_hunk_line (the primary defense
        // is in handle_hunk_header, tested by parser_handles_hunk_line_numbers_at_usize_max).
        //
        // Start at usize::MAX - 2 = 18446744073709551613. The first `+first`
        // line is recorded at that line number (a valid coordinate). The
        // second `+second` advances to usize::MAX - 1 (valid). The third
        // `+third` advances to usize::MAX (valid). The fourth context line
        // ` fourth` cannot advance (usize::MAX + 1 overflows), so the hunk
        // closes and `-fifth` is dropped.
        let diff = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -18446744073709551613,5 +18446744073709551613,5 @@\n+first\n+second\n+third\n fourth\n-fifth\n";
        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 1);
        let file = &files[0];
        // Three added lines recorded (at usize::MAX-2, usize::MAX-1, usize::MAX);
        // the context line ` fourth` triggers the close; `-fifth` is dropped.
        assert_eq!(file.added_lines.len(), 3);
        assert_eq!(file.added_lines[0].text, "first");
        assert_eq!(file.added_lines[0].line, usize::MAX - 2);
        assert_eq!(file.added_lines[1].text, "second");
        assert_eq!(file.added_lines[1].line, usize::MAX - 1);
        assert_eq!(file.added_lines[2].text, "third");
        assert_eq!(file.added_lines[2].line, usize::MAX);
        // The fifth line (`-fifth`) is dropped fail-closed.
        assert_eq!(file.removed_lines.len(), 0);
    }

    #[test]
    fn rejects_new_path_marker_with_parent_dir_traversal() {
        assert_eq!(parse_new_path_marker("+++ b/../../../etc/passwd"), None);
    }

    #[test]
    fn rejects_new_path_marker_with_embedded_parent_dir() {
        assert_eq!(parse_new_path_marker("+++ b/src/../../../etc/passwd"), None);
    }

    #[test]
    fn rejects_new_path_marker_with_absolute_path() {
        assert_eq!(parse_new_path_marker("+++ b//etc/passwd"), None);
        assert_eq!(parse_new_path_marker("+++ /etc/passwd"), None);
    }

    #[test]
    fn old_path_marker_accepts_traversal_as_boundary_signal() {
        // #2402: parse_old_path_marker is a boundary detector, not a path
        // validator. It must accept traversal paths so the parser recognizes
        // the `---` line as a file-section boundary and clears the current
        // file (preventing payload mis-attribution, #2099). The path itself
        // is confined separately by parse_old_path_for_confinement.
        assert!(parse_old_path_marker("--- a/../../../etc/passwd"));
    }

    #[test]
    fn old_path_for_confinement_rejects_traversal() {
        // #2402: the confined old-path extractor rejects traversal paths
        // symmetrically with parse_new_path_marker.
        use super::super::path::parse_old_path_for_confinement;
        assert_eq!(
            parse_old_path_for_confinement("--- a/../../../etc/passwd"),
            None
        );
        assert_eq!(parse_old_path_for_confinement("--- /etc/passwd"), None);
    }

    #[test]
    fn old_path_for_confinement_accepts_normal_path() {
        use super::super::path::parse_old_path_for_confinement;
        assert_eq!(
            parse_old_path_for_confinement("--- a/src/lib.rs"),
            Some(PathBuf::from("src/lib.rs"))
        );
    }

    #[test]
    fn normalizes_new_path_marker_with_cur_dir_components() {
        assert_eq!(
            parse_new_path_marker("+++ b/./src/lib.rs"),
            Some(PathBuf::from("src/lib.rs"))
        );
    }

    #[test]
    fn diff_with_traversal_path_registers_no_file() {
        // A crafted diff whose `+++` marker escapes the workspace must not
        // register a ChangedFile at all: rejection is treated like
        // `/dev/null`, so no SourceLocation can escape the root (#2099).
        let diff = "diff --git a/../../../etc/passwd b/../../../etc/passwd\n--- a/../../../etc/passwd\n+++ b/../../../etc/passwd\n@@ -1,1 +1,1 @@\n-root\n+root\n";
        let files = parse_unified_diff(diff);
        assert!(files.is_empty());
    }

    #[test]
    fn plain_diff_rejected_traversal_marker_does_not_corrupt_previous_file() {
        // Plain unified diff (no `diff --git` separators): the RANK-2
        // boundary detector must treat a confinement-rejected `+++` marker as
        // a file-section boundary, and the parser must clear the current
        // file — otherwise the crafted marker, hunk header, and payload lines
        // are consumed as changes to the previous in-workspace file (#2099
        // review).
        let diff = "--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,1 +1,1 @@\n-old\n+new\n--- a/../../../etc/passwd\n+++ b/../../../etc/passwd\n@@ -1,1 +1,1 @@\n-root\n+root\n";
        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, PathBuf::from("src/a.rs"));
        assert_eq!(files[0].added_lines.len(), 1);
        assert_eq!(files[0].added_lines[0].text, "new");
        assert_eq!(files[0].removed_lines.len(), 1);
        assert_eq!(files[0].removed_lines[0].text, "old");
    }

    #[test]
    fn hunk_payload_resembling_markers_with_spaces_stays_payload() {
        // A removed line whose text is `-- token` followed by an added line
        // whose text is `++ token with spaces` must not be misread as a
        // file-section boundary: an unquoted marker path with whitespace is
        // implausible, so the hunk stays open and both lines attach to the
        // current file (#2099 review).
        let diff = "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,3 +1,3 @@\n ctx\n--- token\n+++ token with spaces\n ctx2\n";
        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, PathBuf::from("src/a.rs"));
        assert_eq!(files[0].removed_lines.len(), 1);
        assert_eq!(files[0].removed_lines[0].text, "-- token");
        assert_eq!(files[0].added_lines.len(), 1);
        assert_eq!(files[0].added_lines[0].text, "++ token with spaces");
    }

    fn next_u64(seed: &mut u64) -> u64 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *seed
    }

    #[test]
    fn bounded_parser_accepts_normal_diff() -> Result<(), String> {
        let diff = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,1 +1,1 @@\n-old\n+new\n";
        let files = parse_unified_diff_bounded_with_metadata(diff)
            .map(|parsed| parsed.changed_files)
            .map_err(|e| format!("bounded parser should accept a normal diff: {e}"))?;
        assert_eq!(files.len(), 1);
        Ok(())
    }

    #[test]
    fn bounded_parser_rejects_oversized_diff() -> Result<(), String> {
        // #2398: a diff with more files than the limit must fail closed
        // with the diff_scope_oversized prefix that is_diff_scope_oversized
        // matches.
        let mut diff = String::new();
        for i in 0..10 {
            diff.push_str(&format!(
                "diff --git a/src/file{i}.rs b/src/file{i}.rs\n--- a/src/file{i}.rs\n+++ b/src/file{i}.rs\n@@ -1,1 +1,1 @@\n-old\n+new\n"
            ));
        }
        let result = parse_unified_diff_with_limit(&diff, 5);
        let Err(err) = result else {
            return Err("oversized diff should be rejected".to_string());
        };
        assert!(
            err.starts_with("diff_scope_oversized"),
            "error must use diff_scope_oversized prefix: {err}"
        );
        assert!(
            err.contains("10 changed files"),
            "error must name the file count: {err}"
        );
        Ok(())
    }
}
