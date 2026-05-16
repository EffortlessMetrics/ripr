use serde_json::Value;
use std::env;
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

pub(crate) fn ripr_annotations(args: &[String]) -> Result<(), String> {
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
            other => return Err(format!("unknown ripr-annotations argument {other:?}")),
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
        return Err(format!(
            "ripr-annotations {flag} requires a non-empty value"
        ));
    }
    Ok(value)
}

fn print_help() {
    println!("usage: cargo xtask ripr-annotations [--comments <path>] [--out <path>] [--check]");
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
        Err(format!(
            "{} is stale; run `cargo xtask ripr-annotations`",
            options.out
        ))
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
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().map(Path::to_path_buf).ok_or_else(|| {
        format!(
            "failed to resolve repo root from {}",
            manifest_dir.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
        assert_eq!(
            parse_options(&["--comments".to_string(), "".to_string()]),
            Err("ripr-annotations --comments requires a non-empty value".to_string())
        );
        Ok(())
    }

    #[test]
    fn missing_comments_artifact_is_advisory_noop() -> Result<(), String> {
        let repo = temp_repo("ripr-annotations-missing")?;
        let options = AnnotationOptions::default();
        let generated = render_annotations(&repo, &options)?;
        assert_eq!(generated.text, "");
        assert!(generated.comments_missing);
        check_annotations(&repo.join(&options.out), &generated, &options)?;
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))
    }

    #[test]
    fn comments_emit_warning_annotations_only() -> Result<(), String> {
        let repo = temp_repo("ripr-annotations-comments")?;
        write_json(
            &repo,
            DEFAULT_COMMENTS_JSON,
            &json!({
                "comments": [comment("src/lib.rs", 42)],
                "summary_only": [comment("src/other.rs", 8)]
            }),
        )?;
        let options = AnnotationOptions::default();
        let generated = render_annotations(&repo, &options)?;
        assert!(
            generated
                .text
                .contains("::warning file=src/lib.rs,line=42,title=ripr medium focused_test::")
        );
        assert!(
            generated
                .text
                .contains("Suggested test%3A Assert boundary behavior")
        );
        assert!(!generated.text.contains("src/other.rs"));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))
    }

    #[test]
    fn comments_without_safe_placement_fail_contract() -> Result<(), String> {
        let repo = temp_repo("ripr-annotations-bad-placement")?;
        let mut item = comment("src/lib.rs", 42);
        item.as_object_mut()
            .ok_or_else(|| "comment object".to_string())?
            .remove("placement");
        write_json(
            &repo,
            DEFAULT_COMMENTS_JSON,
            &json!({
                "comments": [item],
                "summary_only": []
            }),
        )?;
        let err = match render_annotations(&repo, &AnnotationOptions::default()) {
            Ok(_) => return Err("missing placement should fail".to_string()),
            Err(err) => err,
        };
        assert!(err.contains("missing placement object"));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))
    }

    #[test]
    fn check_rejects_stale_annotations() -> Result<(), String> {
        let repo = temp_repo("ripr-annotations-stale")?;
        write_json(
            &repo,
            DEFAULT_COMMENTS_JSON,
            &json!({
                "comments": [comment("src/lib.rs", 42)],
                "summary_only": []
            }),
        )?;
        let options = AnnotationOptions::default();
        let generated = render_annotations(&repo, &options)?;
        write_file(&repo, DEFAULT_ANNOTATIONS_TXT, "stale\n")?;
        let err = match check_annotations(&repo.join(&options.out), &generated, &options) {
            Ok(()) => return Err("stale annotations should fail".to_string()),
            Err(err) => err,
        };
        assert!(err.contains("target/ripr/review/annotations.txt is stale"));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))
    }

    fn comment(path: &str, line: u64) -> Value {
        json!({
            "id": "rec-1",
            "kind": "focused_test",
            "severity": "medium",
            "reason": "Changed boundary logic has broad evidence.",
            "suggested_test": {
                "intent": "Assert boundary behavior"
            },
            "placement": {
                "path": path,
                "line": line,
                "side": "RIGHT",
                "mode": "exact_seam_line"
            }
        })
    }

    fn temp_repo(name: &str) -> Result<PathBuf, String> {
        let unique = format!(
            "{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|err| format!("system clock before epoch: {err}"))?
                .as_nanos()
        );
        let path = env::temp_dir().join(unique);
        fs::create_dir_all(&path).map_err(|err| format!("create {}: {err}", path.display()))?;
        Ok(path)
    }

    fn write_json(repo: &Path, relative: &str, value: &Value) -> Result<(), String> {
        let text =
            serde_json::to_string_pretty(value).map_err(|err| format!("serialize: {err}"))?;
        write_file(repo, relative, &text)
    }

    fn write_file(repo: &Path, relative: &str, text: &str) -> Result<(), String> {
        let path = repo.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("create {}: {err}", parent.display()))?;
        }
        fs::write(&path, text).map_err(|err| format!("write {}: {err}", path.display()))
    }
}
