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
            new_line += 1;
        } else if let Some(text) = raw.strip_prefix('-') {
            file.removed_lines.push(ChangedLine {
                line: old_line,
                text: text.to_string(),
            });
            old_line += 1;
        } else if raw.starts_with(' ') || raw.is_empty() {
            old_line += 1;
            new_line += 1;
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
                assert!(file.added_lines.iter().all(|line| line.line >= 1));
                assert!(file.removed_lines.iter().all(|line| line.line >= 1));
            }
        }

        // Exercise parser behavior with grammar-shaped mutated inputs in addition
        // to arbitrary bytes. This increases the chance that fuzzing-like tests
        // hit stateful transitions between headers/hunks/content lines.
        for _case in 0..512 {
            let diff = generate_fuzz_like_diff(&mut seed);
            let files = parse_unified_diff(&diff);
            for file in files {
                assert!(!file.path.as_os_str().is_empty());
                assert!(file.added_lines.iter().all(|line| line.line >= 1));
                assert!(file.removed_lines.iter().all(|line| line.line >= 1));
            }
        }
    }

    fn next_u64(seed: &mut u64) -> u64 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *seed
    }

    fn generate_fuzz_like_diff(seed: &mut u64) -> String {
        let mut out = String::new();
        let file_count = ((next_u64(seed) % 4) + 1) as usize;

        for idx in 0..file_count {
            let mut base = format!("fuzz/path_{idx}_{}.rs", next_u64(seed) % 10);
            if next_u64(seed).is_multiple_of(7) {
                base.push_str("\0");
            }
            out.push_str(&format!("diff --git a/{base} b/{base}\n"));
            out.push_str(&format!("--- a/{base}\n"));
            out.push_str(&format!("+++ b/{base}\n"));

            let hunk_count = ((next_u64(seed) % 3) + 1) as usize;
            let mut old_line = ((next_u64(seed) % 50) + 1) as usize;
            let mut new_line = ((next_u64(seed) % 50) + 1) as usize;
            for _ in 0..hunk_count {
                out.push_str(&format!("@@ -{old_line},{} +{new_line},{} @@\n", 1, 1));
                let body_len = ((next_u64(seed) % 8) + 1) as usize;
                for _ in 0..body_len {
                    match next_u64(seed) % 5 {
                        0 => {
                            out.push_str("+added\n");
                            new_line += 1;
                        }
                        1 => {
                            out.push_str("-removed\n");
                            old_line += 1;
                        }
                        2 => {
                            out.push_str(" context\n");
                            old_line += 1;
                            new_line += 1;
                        }
                        3 => out.push_str("@@ malformed header @@\n"),
                        _ => out.push_str("\n"),
                    }
                }
            }
        }

        out
    }
}
