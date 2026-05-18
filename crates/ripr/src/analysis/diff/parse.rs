use std::collections::BTreeMap;
use std::path::PathBuf;

use super::model::{ChangedFile, ChangedLine};

pub fn parse_unified_diff(input: &str) -> Vec<ChangedFile> {
    let mut files: BTreeMap<PathBuf, ChangedFile> = BTreeMap::new();
    let mut current_path: Option<PathBuf> = None;
    let mut old_line = 0usize;
    let mut new_line = 0usize;
    let mut in_hunk = false;

    for raw in input.lines() {
        if !in_hunk && let Some(path) = parse_new_path_marker(raw) {
            current_path = Some(path.clone());
            files.entry(path.clone()).or_insert_with(|| ChangedFile {
                path,
                ..ChangedFile::default()
            });
            continue;
        }

        if raw.starts_with("diff --git ") {
            current_path = None;
            in_hunk = false;
            continue;
        }

        if raw.starts_with("@@") {
            if let Some((old_start, new_start)) = parse_hunk_header(raw) {
                old_line = old_start;
                new_line = new_start;
                in_hunk = true;
            }
            continue;
        }

        let Some(path) = current_path.clone() else {
            continue;
        };
        let Some(file) = files.get_mut(&path) else {
            continue;
        };

        if !in_hunk && (raw.starts_with("+++") || raw.starts_with("---")) {
            continue;
        }

        if let Some(text) = raw.strip_prefix('+') {
            file.added_lines.push(ChangedLine {
                line: new_line,
                text: text.to_string(),
            });
            new_line = new_line.saturating_add(1);
        } else if let Some(text) = raw.strip_prefix('-') {
            file.removed_lines.push(ChangedLine {
                line: old_line,
                text: text.to_string(),
            });
            old_line = old_line.saturating_add(1);
        } else if raw.starts_with(' ') || raw.is_empty() {
            old_line = old_line.saturating_add(1);
            new_line = new_line.saturating_add(1);
        }
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

fn parse_new_path_marker(raw: &str) -> Option<PathBuf> {
    let marker = raw.strip_prefix("+++ ")?;
    let path = parse_diff_path_token(marker)?;
    if path == "/dev/null" {
        return None;
    }
    let path = path.strip_prefix("b/").unwrap_or(&path);
    Some(PathBuf::from(path))
}

fn parse_diff_path_token(raw: &str) -> Option<String> {
    let raw = raw.trim_end_matches('\r');
    if let Some(quoted) = raw.strip_prefix('"') {
        return parse_c_quoted_path(quoted);
    }

    let token = raw.split_once('\t').map_or(raw, |(path, _metadata)| path);
    Some(token.trim_end().to_string()).filter(|path| !path.is_empty())
}

fn parse_c_quoted_path(raw: &str) -> Option<String> {
    let mut path = String::new();
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Some(path),
            '\\' => path.push(parse_c_escape(&mut chars)),
            _ => path.push(ch),
        }
    }

    None
}

fn parse_c_escape<I>(chars: &mut std::iter::Peekable<I>) -> char
where
    I: Iterator<Item = char>,
{
    let Some(ch) = chars.next() else {
        return '\\';
    };

    match ch {
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        '\\' => '\\',
        '"' => '"',
        '0'..='7' => parse_octal_escape(ch, chars),
        _ => ch,
    }
}

fn parse_octal_escape<I>(first: char, chars: &mut std::iter::Peekable<I>) -> char
where
    I: Iterator<Item = char>,
{
    let mut value = first.to_digit(8).unwrap_or(0);

    for _ in 0..2 {
        let Some(next) = chars.peek().copied() else {
            break;
        };
        let Some(digit) = next.to_digit(8) else {
            break;
        };
        let _ = chars.next();
        value = value.saturating_mul(8).saturating_add(digit);
    }

    char::from_u32(value).unwrap_or('\u{FFFD}')
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
        assert_eq!(file.removed_lines[0].line, 0);
        assert_eq!(file.added_lines[0].line, 0);
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
                text: "current".to_string()
            }]
        );
        assert_eq!(
            files[0].removed_lines,
            vec![ChangedLine {
                line: 1,
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
