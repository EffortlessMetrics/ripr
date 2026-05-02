use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Default)]
pub struct ChangedFile {
    pub path: PathBuf,
    pub added_lines: Vec<ChangedLine>,
    pub removed_lines: Vec<ChangedLine>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedLine {
    pub line: usize,
    pub text: String,
}

pub fn load_diff(
    root: &Path,
    base: Option<&str>,
    diff_file: Option<&PathBuf>,
) -> Result<String, String> {
    if let Some(diff_file) = diff_file {
        return std::fs::read_to_string(diff_file)
            .map_err(|err| format!("failed to read diff file {}: {err}", diff_file.display()));
    }

    let base = base.unwrap_or("origin/main");
    let output = Command::new("git")
        .arg("diff")
        .arg(format!("{base}...HEAD"))
        .current_dir(root)
        .output()
        .map_err(|err| format!("failed to run git diff: {err}"))?;

    if !output.status.success() {
        return Err(format!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn parse_unified_diff(input: &str) -> Vec<ChangedFile> {
    let mut files: BTreeMap<PathBuf, ChangedFile> = BTreeMap::new();
    let mut current_path: Option<PathBuf> = None;
    let mut old_line = 0usize;
    let mut new_line = 0usize;

    for raw in input.lines() {
        if let Some(path) = raw.strip_prefix("+++ b/") {
            let path = PathBuf::from(path.trim());
            current_path = Some(path.clone());
            files.entry(path.clone()).or_insert_with(|| ChangedFile {
                path,
                ..ChangedFile::default()
            });
            continue;
        }

        if raw.starts_with("diff --git ") {
            current_path = None;
            continue;
        }

        if raw.starts_with("@@") {
            if let Some((old_start, new_start)) = parse_hunk_header(raw) {
                old_line = old_start;
                new_line = new_start;
            }
            continue;
        }

        let Some(path) = current_path.clone() else {
            continue;
        };
        let Some(file) = files.get_mut(&path) else {
            continue;
        };

        if raw.starts_with("+++") || raw.starts_with("---") {
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
    fn parser_is_robust_against_fuzz_like_inputs() {
        let mut seed = 0xC0FFEE_u64;
        for _case in 0..512 {
            let len = (next_u64(&mut seed) % 512) as usize;
            let mut bytes = Vec::with_capacity(len);
            for _ in 0..len {
                bytes.push((next_u64(&mut seed) & 0xFF) as u8);
            }
            let text = String::from_utf8_lossy(&bytes);
            let files = parse_unified_diff(&text);
            for file in files {
                assert!(!file.path.as_os_str().is_empty());
                assert!(file.added_lines.windows(2).all(|w| w[0].line <= w[1].line));
                assert!(
                    file.removed_lines
                        .windows(2)
                        .all(|w| w[0].line <= w[1].line)
                );
            }
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
