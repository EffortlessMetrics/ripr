#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(|s| s.as_str()) {
        Some("ci-fast") => ci_fast(),
        Some("ci-full") => ci_full(),
        Some("check-static-language") => check_static_language(),
        Some("check-no-panic-family") => check_no_panic_family(),
        Some("package") => run("cargo", &["package", "-p", "ripr", "--list"]).map(|_| ()),
        Some("publish-dry-run") => {
            run("cargo", &["publish", "-p", "ripr", "--dry-run"]).map(|_| ())
        }
        Some("help") | None => {
            print_help();
            Ok(())
        }
        Some(other) => Err(format!("unknown xtask command {other}")),
    };
    if let Err(err) = result {
        eprintln!("xtask: {err}");
        std::process::exit(1);
    }
}

fn ci_fast() -> Result<(), String> {
    run("cargo", &["fmt", "--check"])?;
    run("cargo", &["check", "--workspace", "--all-targets"])?;
    run("cargo", &["test", "--workspace"])?;
    check_static_language()?;
    check_no_panic_family()
}

fn ci_full() -> Result<(), String> {
    ci_fast()?;
    run(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run("cargo", &["doc", "--workspace", "--no-deps"])?;
    run("cargo", &["package", "-p", "ripr", "--list"])?;
    run("cargo", &["publish", "-p", "ripr", "--dry-run"]).map(|_| ())
}

fn run(program: &str, args: &[&str]) -> Result<ExitStatus, String> {
    eprintln!("$ {} {}", program, args.join(" "));
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|err| format!("failed to run {program}: {err}"))?;
    if status.success() {
        Ok(status)
    } else {
        Err(format!("{program} {} failed with {status}", args.join(" ")))
    }
}

fn print_help() {
    println!(
        "xtask commands:\n  ci-fast\n  ci-full\n  check-static-language\n  check-no-panic-family\n  package\n  publish-dry-run"
    );
}

fn check_static_language() -> Result<(), String> {
    let allowed = read_path_allowlist(".ripr/static-language-allowlist.txt")?;
    let forbidden = forbidden_static_terms();
    let mut violations = Vec::new();

    for path in collect_files(Path::new("."))? {
        let normalized = normalize_path(&path);
        if !is_static_language_candidate(&normalized) || allowed.contains(&normalized) {
            continue;
        }
        let text = read_text_lossy(&path)?;
        for (line_number, line) in text.lines().enumerate() {
            let lower = line.to_ascii_lowercase();
            for term in &forbidden {
                if contains_word(&lower, term) {
                    violations.push(format!(
                        "{normalized}:{} contains prohibited static-language term `{term}`",
                        line_number + 1
                    ));
                }
            }
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "static language check failed:\n{}",
            violations.join("\n")
        ))
    }
}

fn check_no_panic_family() -> Result<(), String> {
    let allowlist = read_count_allowlist(".ripr/no-panic-allowlist.txt")?;
    let roots = [
        Path::new("crates/ripr/src"),
        Path::new("crates/ripr/tests"),
        Path::new("xtask/src"),
    ];
    let patterns = forbidden_panic_patterns();
    let mut counts = BTreeMap::<(String, String), usize>::new();

    for root in roots {
        if !root.exists() {
            continue;
        }
        for path in collect_files(root)? {
            if path.extension().and_then(|value| value.to_str()) != Some("rs") {
                continue;
            }
            let normalized = normalize_path(&path);
            let text = read_text_lossy(&path)?;
            for pattern in &patterns {
                let count = text.matches(pattern).count();
                if count > 0 {
                    counts.insert((normalized.clone(), pattern.clone()), count);
                }
            }
        }
    }

    let mut violations = Vec::new();
    for ((path, pattern), count) in &counts {
        let allowed = allowlist
            .get(&(path.clone(), pattern.clone()))
            .copied()
            .unwrap_or(0);
        if *count > allowed {
            violations.push(format!(
                "{path} contains `{pattern}` {count} time(s), allowed {allowed}"
            ));
        }
    }

    for ((path, pattern), allowed) in &allowlist {
        let actual = counts
            .get(&(path.clone(), pattern.clone()))
            .copied()
            .unwrap_or(0);
        if actual > *allowed {
            violations.push(format!(
                "{path} contains `{pattern}` {actual} time(s), allowed {allowed}"
            ));
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "panic-family check failed:\n{}",
            violations.join("\n")
        ))
    }
}

fn read_path_allowlist(path: &str) -> Result<BTreeSet<String>, String> {
    let mut allowed = BTreeSet::new();
    let text = read_text_lossy(Path::new(path))?;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        allowed.insert(normalize_slashes(trimmed));
    }
    Ok(allowed)
}

fn read_count_allowlist(path: &str) -> Result<BTreeMap<(String, String), usize>, String> {
    let mut allowed = BTreeMap::new();
    let text = read_text_lossy(Path::new(path))?;
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts = trimmed.split('|').collect::<Vec<_>>();
        if parts.len() != 4 {
            return Err(format!(
                "{path}:{} expected path|pattern|max_count|reason",
                line_number + 1
            ));
        }
        let max_count = parts[2]
            .parse::<usize>()
            .map_err(|err| format!("{path}:{} invalid max_count: {err}", line_number + 1))?;
        allowed.insert(
            (normalize_slashes(parts[0]), parts[1].to_string()),
            max_count,
        );
    }
    Ok(allowed)
}

fn collect_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_files_inner(root, &mut files)?;
    Ok(files)
}

fn collect_files_inner(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let normalized = normalize_path(path);
    if should_skip_path(&normalized) {
        return Ok(());
    }
    let metadata =
        fs::metadata(path).map_err(|err| format!("failed to inspect {normalized}: {err}"))?;
    if metadata.is_file() {
        files.push(path.to_path_buf());
        return Ok(());
    }
    if metadata.is_dir() {
        for entry in
            fs::read_dir(path).map_err(|err| format!("failed to read {normalized}: {err}"))?
        {
            let entry = entry.map_err(|err| format!("failed to read {normalized}: {err}"))?;
            collect_files_inner(&entry.path(), files)?;
        }
    }
    Ok(())
}

fn should_skip_path(path: &str) -> bool {
    path == ".git"
        || path.starts_with(".git/")
        || path == "target"
        || path.starts_with("target/")
        || path.ends_with("/node_modules")
        || path.contains("/node_modules/")
        || path.ends_with("/dist")
        || path.contains("/dist/")
}

fn is_static_language_candidate(path: &str) -> bool {
    let extensions = [".md", ".rs", ".txt", ".json", ".toml", ".yml", ".yaml"];
    extensions.iter().any(|extension| path.ends_with(extension))
}

fn read_text_lossy(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn normalize_path(path: &Path) -> String {
    normalize_slashes(&path.to_string_lossy())
        .trim_start_matches("./")
        .to_string()
}

fn normalize_slashes(value: &str) -> String {
    value.replace('\\', "/")
}

fn forbidden_static_terms() -> Vec<String> {
    ["killed", "survived", "untested", "proven", "adequate"]
        .iter()
        .map(|value| value.to_string())
        .collect()
}

fn forbidden_panic_patterns() -> Vec<String> {
    [
        concat!("unwrap", "("),
        concat!("expect", "("),
        concat!("panic", "!"),
        concat!("todo", "!"),
        concat!("unimplemented", "!"),
        concat!("unreachable", "!"),
    ]
    .iter()
    .map(|value| value.to_string())
    .collect()
}

fn contains_word(text: &str, word: &str) -> bool {
    let mut start = 0usize;
    while let Some(offset) = text[start..].find(word) {
        let idx = start + offset;
        let before = text[..idx].chars().next_back();
        let after = text[idx + word.len()..].chars().next();
        if !is_word_char(before) && !is_word_char(after) {
            return true;
        }
        start = idx + word.len();
    }
    false
}

fn is_word_char(value: Option<char>) -> bool {
    value
        .map(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        .unwrap_or(false)
}
