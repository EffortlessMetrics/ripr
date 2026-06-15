//! `cargo xtask module-health` — advisory Rust source-file size report.
//!
//! Walks `crates/ripr/src/` and `xtask/src/` recursively, counts lines per
//! `*.rs` file, and writes a ranked report flagging files that exceed a
//! configurable threshold. The report is purely informational: it never fails,
//! never mutates anything, and is never wired into CI gates.
//!
//! Intended use: before starting a capability wave that touches an oversized
//! file, open a behaviour-preserving decomposition PR first so each new
//! feature lands in a focused module (zero golden drift = proof it is pure
//! structure). See `AGENTS.md` § Implementation Bias for the
//! refactor-before-extend principle.

use serde_json::json;
use std::path::{Path, PathBuf};

const MODULE_HEALTH_SCHEMA_VERSION: &str = "0.1";
const MODULE_HEALTH_STATE: &str = "advisory-report-only";
const MODULE_HEALTH_JSON: &str = "module-health.json";
const MODULE_HEALTH_MD: &str = "module-health.md";
const DEFAULT_THRESHOLD: usize = 2000;
const TOP_N: usize = 15;
const MODULE_HEALTH_USAGE: &str = "usage: cargo xtask module-health [--threshold <n>] [--help|-h]";

pub(crate) fn module_health(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{MODULE_HEALTH_USAGE}");
        println!();
        println!("Walks crates/ripr/src/ and xtask/src/ for *.rs files, counts lines per");
        println!("file, and writes an advisory ranked report to target/ripr/reports/.");
        println!("Always exits 0. Never mutates source. Never wired into CI gates.");
        println!();
        println!("Options:");
        println!(
            "  --threshold <n>   Lines-per-file threshold for flagging (default: {DEFAULT_THRESHOLD})"
        );
        return Ok(());
    }

    let threshold = parse_threshold(args)?;

    let root = std::env::current_dir()
        .map_err(|err| format!("failed to read current directory: {err}"))?;

    let mut files: Vec<FileEntry> = Vec::new();
    for search_root in &["crates/ripr/src", "xtask/src"] {
        let dir = root.join(search_root);
        if dir.exists() {
            collect_rs_files(&dir, &root, &mut files)?;
        }
    }

    // Sort by line count descending, then path ascending for stability.
    files.sort_by(|a, b| b.lines.cmp(&a.lines).then_with(|| a.path.cmp(&b.path)));

    let over_threshold: Vec<&FileEntry> = files.iter().filter(|f| f.lines >= threshold).collect();
    let over_threshold_count = over_threshold.len();
    let generated_files_count = files.len();

    // Build JSON report.
    let files_json: Vec<serde_json::Value> = files
        .iter()
        .map(|f| {
            json!({
                "path": f.path,
                "lines": f.lines,
                "over_threshold": f.lines >= threshold,
            })
        })
        .collect();

    let report_json = json!({
        "schema_version": MODULE_HEALTH_SCHEMA_VERSION,
        "state": MODULE_HEALTH_STATE,
        "threshold": threshold,
        "generated_files_count": generated_files_count,
        "over_threshold_count": over_threshold_count,
        "files": files_json,
    });

    let json_text = serde_json::to_string_pretty(&report_json)
        .map_err(|err| format!("serialize module-health report: {err}"))?;
    crate::write_report(MODULE_HEALTH_JSON, &format!("{json_text}\n"))?;
    crate::write_report(
        MODULE_HEALTH_MD,
        &render_markdown(threshold, &files, over_threshold_count, &over_threshold),
    )?;

    println!("Wrote target/ripr/reports/{MODULE_HEALTH_JSON}");
    println!("Wrote target/ripr/reports/{MODULE_HEALTH_MD}");
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileEntry {
    /// Forward-slash normalized path relative to repo root (platform-stable).
    path: String,
    lines: usize,
}

fn parse_threshold(args: &[String]) -> Result<usize, String> {
    let mut index = 0usize;
    let mut threshold = DEFAULT_THRESHOLD;
    while index < args.len() {
        match args[index].as_str() {
            "--threshold" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    format!("missing value for --threshold; {MODULE_HEALTH_USAGE}")
                })?;
                threshold = value.parse::<usize>().map_err(|err| {
                    format!("module-health --threshold must be a positive integer: {err}")
                })?;
                if threshold == 0 {
                    return Err("module-health --threshold must be greater than zero".to_string());
                }
            }
            other => {
                return Err(format!(
                    "unknown module-health argument `{other}`; {MODULE_HEALTH_USAGE}"
                ));
            }
        }
        index += 1;
    }
    Ok(threshold)
}

fn collect_rs_files(dir: &Path, repo_root: &Path, out: &mut Vec<FileEntry>) -> Result<(), String> {
    let read_dir = std::fs::read_dir(dir)
        .map_err(|err| format!("failed to read directory {}: {err}", dir.display()))?;
    for entry in read_dir {
        let entry = entry.map_err(|err| format!("failed to read directory entry: {err}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| format!("failed to stat {}: {err}", path.display()))?;
        if file_type.is_dir() {
            collect_rs_files(&path, repo_root, out)?;
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            let lines = count_lines(&path)?;
            let rel = relative_path_normalized(&path, repo_root);
            out.push(FileEntry { path: rel, lines });
        }
    }
    Ok(())
}

fn count_lines(path: &Path) -> Result<usize, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    // Count newline-terminated lines: number of '\n' characters.
    Ok(text.chars().filter(|&c| c == '\n').count())
}

/// Return the path relative to `root`, with backslashes replaced by forward
/// slashes (platform-stable for Windows-generated output).
fn relative_path_normalized(path: &Path, root: &Path) -> String {
    let rel: PathBuf = path.strip_prefix(root).unwrap_or(path).to_path_buf();
    rel.to_string_lossy().replace('\\', "/")
}

fn render_markdown(
    threshold: usize,
    all_files: &[FileEntry],
    over_threshold_count: usize,
    over_threshold: &[&FileEntry],
) -> String {
    let mut body = String::from("# ripr module-health report\n\n");
    body.push_str("Status: advisory\n");
    body.push_str("State: advisory-report-only\n\n");
    body.push_str(
        "This report flags oversized Rust source files as a refactoring guide. \
         It is advisory-only: it never fails CI, never mutates anything, and \
         changes no CI behavior. Use it to identify monoliths before starting a \
         capability wave — decompose first so each new feature lands in a focused module.\n\n",
    );
    body.push_str(&format!("Threshold: {threshold} lines\n"));
    body.push_str(&format!("Files scanned: {}\n", all_files.len()));
    body.push_str(&format!("Files over threshold: {over_threshold_count}\n\n"));

    body.push_str("## Files over threshold (candidates for decomposition)\n\n");
    if over_threshold.is_empty() {
        body.push_str(&format!(
            "No files exceed the {threshold}-line threshold.\n\n"
        ));
    } else {
        body.push_str("| File | Lines |\n| --- | --- |\n");
        for f in over_threshold {
            body.push_str(&format!("| `{}` | {} |\n", f.path, f.lines));
        }
        body.push('\n');
    }

    let top_n = TOP_N.min(all_files.len());
    body.push_str(&format!("## Top {top_n} largest files overall\n\n"));
    if all_files.is_empty() {
        body.push_str("(no files found)\n\n");
    } else {
        body.push_str("| # | File | Lines | Over threshold |\n| --- | --- | --- | --- |\n");
        for (rank, f) in all_files.iter().take(top_n).enumerate() {
            let flag = if f.lines >= threshold { "yes" } else { "no" };
            body.push_str(&format!(
                "| {} | `{}` | {} | {} |\n",
                rank + 1,
                f.path,
                f.lines,
                flag
            ));
        }
        body.push('\n');
    }

    body.push_str("Reproduce locally:\n\n");
    body.push_str("```\n");
    body.push_str(&format!(
        "cargo xtask module-health --threshold {threshold}\n"
    ));
    body.push_str("```\n");
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_threshold_default_is_2000() -> Result<(), String> {
        assert_eq!(parse_threshold(&[])?, DEFAULT_THRESHOLD);
        Ok(())
    }

    #[test]
    fn parse_threshold_explicit_value() -> Result<(), String> {
        let args = ["--threshold".to_string(), "500".to_string()];
        assert_eq!(parse_threshold(&args)?, 500);
        Ok(())
    }

    #[test]
    fn parse_threshold_zero_rejected() -> Result<(), String> {
        let args = ["--threshold".to_string(), "0".to_string()];
        if parse_threshold(&args).is_ok() {
            return Err("expected error for threshold=0 but got Ok".to_string());
        }
        Ok(())
    }

    #[test]
    fn parse_threshold_unknown_arg_rejected() -> Result<(), String> {
        let args = ["--unknown".to_string()];
        if parse_threshold(&args).is_ok() {
            return Err("expected error for unknown arg but got Ok".to_string());
        }
        Ok(())
    }

    #[test]
    fn count_lines_counts_newlines() -> Result<(), String> {
        let tmp = tempfile_path();
        // 3 lines = 3 newlines
        std::fs::write(&tmp, "line1\nline2\nline3\n").map_err(|err| format!("write tmp: {err}"))?;
        let result = count_lines(&tmp);
        let _ = std::fs::remove_file(&tmp);
        assert_eq!(result?, 3);
        Ok(())
    }

    fn tempfile_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "module_health_test_{}.rs",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn over_threshold_flag_correct() {
        let files = [
            FileEntry {
                path: "a.rs".to_string(),
                lines: 3000,
            },
            FileEntry {
                path: "b.rs".to_string(),
                lines: 100,
            },
        ];
        let over: Vec<&FileEntry> = files.iter().filter(|f| f.lines >= 2000).collect();
        assert_eq!(over.len(), 1);
        assert_eq!(over[0].path, "a.rs");
    }

    #[test]
    fn relative_path_normalized_replaces_backslashes() {
        // Build the backslash-separated path at runtime so no literal Windows
        // path appears in source (keeps check-local-context clean) while still
        // exercising the backslash -> forward-slash normalization on both
        // platforms (on Windows strip_prefix splits components; on Linux the
        // separator is a plain char and the whole string is normalized).
        let bs = "\\";
        let root = PathBuf::from("root");
        let path = PathBuf::from(format!("root{bs}sub{bs}file.rs"));
        let result = relative_path_normalized(&path, &root);
        assert!(!result.contains('\\'), "backslash found in: {result}");
        assert!(result.contains('/'), "forward slash missing in: {result}");
    }

    #[test]
    fn markdown_contains_advisory_note_and_threshold() {
        let files = [
            FileEntry {
                path: "big.rs".to_string(),
                lines: 5000,
            },
            FileEntry {
                path: "small.rs".to_string(),
                lines: 50,
            },
        ];
        let over: Vec<&FileEntry> = files.iter().filter(|f| f.lines >= 2000).collect();
        let md = render_markdown(2000, &files, over.len(), &over);
        assert!(md.contains("advisory-only"));
        assert!(md.contains("2000"));
        assert!(md.contains("big.rs"));
        assert!(md.contains("5000"));
    }
}
