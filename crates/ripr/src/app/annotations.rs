//! `ripr annotations` — binary-first GitHub Actions annotations (item 8b).
//!
//! Ports `cargo xtask ripr-annotations` into the `ripr` binary so downstream
//! consumers can generate GitHub Actions annotations without compiling their
//! own xtask. Reads `target/ripr/review/comments.json` and emits
//! `::warning` annotation lines to `target/ripr/review/annotations.txt`.
//! Supports `--comments <path>`, `--out <path>`, `--check`, `--help`.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_COMMENTS_JSON: &str = "target/ripr/review/comments.json";
const DEFAULT_ANNOTATIONS_TXT: &str = "target/ripr/review/annotations.txt";

#[derive(Clone, Debug, Eq, PartialEq)]
struct AnnotationOptions {
    comments: String,
    out: String,
    check: bool,
}

impl Default for AnnotationOptions {
    fn default() -> Self {
        Self {
            comments: DEFAULT_COMMENTS_JSON.to_string(),
            out: DEFAULT_ANNOTATIONS_TXT.to_string(),
            check: false,
        }
    }
}

pub(crate) fn run_annotations(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }
    let options = parse_options(args)?;
    let repo = repo_root()?;
    let generated = render_annotations(&repo, &options)?;
    let out = repo.join(&options.out);
    if options.check {
        check_annotations(&out, &generated, &options)
    } else {
        write_annotations(&out, &generated, &options)
    }
}

fn parse_options(args: &[String]) -> Result<AnnotationOptions, String> {
    let mut options = AnnotationOptions::default();
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--comments" => {
                i += 1;
                options.comments = non_empty_arg(args, i, "--comments")?.to_string();
            }
            "--out" => {
                i += 1;
                options.out = non_empty_arg(args, i, "--out")?.to_string();
            }
            "--check" => options.check = true,
            other => return Err(format!("unknown annotations argument `{other}`")),
        }
        i += 1;
    }
    Ok(options)
}

fn non_empty_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    let Some(value) = args.get(index) else {
        return Err(format!("missing value for {flag}"));
    };
    if value.trim().is_empty() {
        return Err(format!("annotations {flag} requires a non-empty value"));
    }
    Ok(value)
}

fn print_help() {
    println!("usage: ripr annotations [--comments <path>] [--out <path>] [--check]");
    println!();
    println!("Options:");
    println!("  --comments <path>  Path to comments.json (default: {DEFAULT_COMMENTS_JSON})");
    println!("  --out <path>       Output annotations path (default: {DEFAULT_ANNOTATIONS_TXT})");
    println!("  --check            Verify the existing annotations are up to date.");
}

fn render_annotations(
    repo: &Path,
    options: &AnnotationOptions,
) -> Result<AnnotationOutput, String> {
    let comments_path = repo.join(&options.comments);
    if !comments_path.exists() {
        return Ok(AnnotationOutput {
            text: String::new(),
            comments_missing: true,
        });
    }
    let text = fs::read_to_string(&comments_path)
        .map_err(|err| format!("failed to read {}: {err}", options.comments))?;
    let packet: Value = serde_json::from_str(&text)
        .map_err(|err| format!("{} is not valid JSON: {err}", options.comments))?;
    let comments = packet
        .get("comments")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{} is missing comments[]", options.comments))?;

    let mut out = String::new();
    for item in comments {
        let annotation = annotation_from_comment(item)?;
        out.push_str(&annotation);
        out.push('\n');
    }
    Ok(AnnotationOutput {
        text: out,
        comments_missing: false,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AnnotationOutput {
    text: String,
    comments_missing: bool,
}

fn annotation_from_comment(item: &Value) -> Result<String, String> {
    let placement = item
        .get("placement")
        .and_then(Value::as_object)
        .ok_or_else(|| "comments[] item is missing placement object".to_string())?;
    let path = string_key(placement, "path")?;
    let line = placement
        .get("line")
        .and_then(Value::as_u64)
        .ok_or_else(|| "comments[] placement.line is missing or not an integer".to_string())?;
    let mode = string_key(placement, "mode")?;
    if !matches!(
        mode.as_str(),
        "exact_seam_line" | "owner_function_changed_line" | "same_file_changed_line"
    ) {
        return Err(format!(
            "comments[] placement mode {mode:?} is not annotation-safe"
        ));
    }
    let severity = item
        .get("severity")
        .and_then(Value::as_str)
        .unwrap_or("advisory");
    let kind = item
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("focused_test");
    let reason = item
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("RIPR review guidance");
    let intent = item
        .get("suggested_test")
        .and_then(|test| test.get("intent"))
        .and_then(Value::as_str);
    let mut message = reason.to_string();
    if let Some(intent) = intent {
        message.push_str(" Suggested test: ");
        message.push_str(intent);
    }
    let title = format!("ripr {severity} {kind}");
    Ok(format!(
        "::warning file={},line={},title={}::{}",
        escape_cmd(&path),
        line,
        escape_cmd(&title),
        escape_cmd(&message)
    ))
}

fn string_key(object: &serde_json::Map<String, Value>, key: &str) -> Result<String, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("comments[] placement.{key} is missing or empty"))
}

fn check_annotations(
    path: &Path,
    generated: &AnnotationOutput,
    options: &AnnotationOptions,
) -> Result<(), String> {
    if generated.comments_missing && !path.exists() {
        println!("RIPR annotations skipped: {} is missing", options.comments);
        return Ok(());
    }
    let actual = fs::read_to_string(path)
        .map_err(|err| format!("missing or unreadable {}: {err}", options.out))?;
    if actual == generated.text {
        println!("RIPR annotations contract ok: {}", options.out);
        Ok(())
    } else {
        Err(format!("{} is stale; run `ripr annotations`", options.out))
    }
}

fn write_annotations(
    path: &Path,
    generated: &AnnotationOutput,
    options: &AnnotationOptions,
) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err(format!("{} has no parent directory", options.out));
    };
    fs::create_dir_all(parent)
        .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    fs::write(path, &generated.text)
        .map_err(|err| format!("failed to write {}: {err}", options.out))?;
    if generated.comments_missing {
        println!("RIPR annotations skipped: {} is missing", options.comments);
    } else if generated.text.is_empty() {
        println!("RIPR annotations: no comments[] guidance to emit");
    } else {
        print!("{}", generated.text);
    }
    println!("Wrote {}", options.out);
    Ok(())
}

fn escape_cmd(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
        .replace(',', "%2C")
        .replace(':', "%3A")
}

fn repo_root() -> Result<PathBuf, String> {
    std::env::current_dir().map_err(|err| format!("failed to determine working directory: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_supports_paths_and_check() -> Result<(), String> {
        let parsed = parse_options(&[
            "--comments".to_string(),
            "comments.json".to_string(),
            "--out".to_string(),
            "annotations.txt".to_string(),
            "--check".to_string(),
        ])?;
        assert_eq!(parsed.comments, "comments.json");
        assert_eq!(parsed.out, "annotations.txt");
        assert!(parsed.check);
        match parse_options(&["--comments".to_string(), "".to_string()]) {
            Err(msg) if msg.contains("non-empty") => Ok(()),
            other => Err(format!("expected non-empty error, got {other:?}")),
        }
    }

    #[test]
    fn parse_rejects_unknown_arg() -> Result<(), String> {
        match parse_options(&["--bogus".to_string()]) {
            Err(msg) if msg.contains("--bogus") => Ok(()),
            other => Err(format!("expected unknown-arg error, got {other:?}")),
        }
    }
}
