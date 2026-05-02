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
        for _case in 0..1024 {
            let len = (next_u64(&mut seed) % 512) as usize;
            let mut bytes = Vec::with_capacity(len);
            for _ in 0..len {
                bytes.push((next_u64(&mut seed) & 0xFF) as u8);
            }
            let text = String::from_utf8_lossy(&bytes);
            let files = parse_unified_diff(&text);
            for file in files {
                assert!(file.added_lines.iter().all(|line| !line.text.contains('\n')));
                assert!(file.removed_lines.iter().all(|line| !line.text.contains('\n')));
            }
        }

        // Structured fuzzing: perturb valid-ish patch fragments so the parser
        // explores realistic line-oriented states while still receiving
        // malformed hunks and headers.
        let mut grammar_seed = 0xDEADBEEF_u64;
        for _case in 0..512 {
            let text = generate_fuzz_diff(&mut grammar_seed);
            let files = parse_unified_diff(&text);
            for file in files {
                assert!(file.added_lines.iter().all(|line| !line.text.contains('\n')));
                assert!(file.removed_lines.iter().all(|line| !line.text.contains('\n')));
            }
        }
    }

    fn generate_fuzz_diff(seed: &mut u64) -> String {
        const TOKENS: &[&str] = &[
            "diff --git a/src/lib.rs b/src/lib.rs\n",
            "diff --git a/src/main.rs b/src/main.rs\n",
            "--- a/src/lib.rs\n",
            "+++ b/src/lib.rs\n",
            "--- a/src/main.rs\n",
            "+++ b/src/main.rs\n",
            "@@ -1,3 +1,3 @@\n",
            "@@ -0,0 +1,1 @@\n",
            "@@ -999999999999999999,1 +1,1 @@\n",
            "+added line\n",
            "-removed line\n",
            " context\n",
            "+++ b/\n",
            "@@ not a hunk @@\n",
            "\n",
        ];

        let mut out = String::new();
        let n = (next_u64(seed) % 64) as usize;
        for _ in 0..n {
            let idx = (next_u64(seed) as usize) % TOKENS.len();
            out.push_str(TOKENS[idx]);

            if next_u64(seed).is_multiple_of(7) {
                // Inject arbitrary bytes between valid-ish lines.
                let mut noise = [0u8; 8];
                for byte in &mut noise {
                    *byte = (next_u64(seed) & 0xFF) as u8;
                }
                out.push_str(&String::from_utf8_lossy(&noise));
                out.push('\n');
            }
        }
        out
    }

    fn next_u64(seed: &mut u64) -> u64 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *seed
    }
}
