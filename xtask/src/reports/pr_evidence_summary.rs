use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const PR_EVIDENCE_JSON: &str = "target/ripr/pr/repo-exposure.json";
const PR_EVIDENCE_MD: &str = "target/ripr/pr/repo-exposure.md";
const REVIEW_COMMENTS_JSON: &str = "target/ripr/review/comments.json";
const REVIEW_COMMENTS_MD: &str = "target/ripr/review/comments.md";
const PR_SUMMARY_MD: &str = "target/ripr/pr/summary.md";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SummaryOptions {
    check: bool,
}

pub(crate) fn ripr_pr_summary(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }
    let options = parse_options(args)?;
    let repo = repo_root()?;
    let summary = render_pr_evidence_summary(&repo);
    let path = repo.join(PR_SUMMARY_MD);
    if options.check {
        check_summary(&path, &summary)
    } else {
        write_summary(&path, &summary)
    }
}

fn parse_options(args: &[String]) -> Result<SummaryOptions, String> {
    let mut check = false;
    for arg in args {
        match arg.as_str() {
            "--check" => check = true,
            other => return Err(format!("unknown ripr-pr-summary argument {other:?}")),
        }
    }
    Ok(SummaryOptions { check })
}

fn print_help() {
    println!("usage: cargo xtask ripr-pr-summary [--check]");
}

fn check_summary(path: &Path, expected: &str) -> Result<(), String> {
    let actual = fs::read_to_string(path)
        .map_err(|err| format!("missing or unreadable {PR_SUMMARY_MD}: {err}"))?;
    if actual == expected {
        println!("PR evidence summary contract ok: {PR_SUMMARY_MD}");
        Ok(())
    } else {
        Err(format!(
            "{PR_SUMMARY_MD} is stale; run `cargo xtask ripr-pr-summary`"
        ))
    }
}

fn write_summary(path: &Path, summary: &str) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err(format!("{PR_SUMMARY_MD} has no parent directory"));
    };
    fs::create_dir_all(parent)
        .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    fs::write(path, summary).map_err(|err| format!("failed to write {PR_SUMMARY_MD}: {err}"))?;
    println!("Wrote {PR_SUMMARY_MD}");
    Ok(())
}

fn render_pr_evidence_summary(repo: &Path) -> String {
    let pr_evidence = load_json(repo, PR_EVIDENCE_JSON);
    let review_comments = load_json(repo, REVIEW_COMMENTS_JSON);
    let pr_value = pr_evidence.value.as_ref();
    let review_value = review_comments.value.as_ref();

    let mut out = String::new();
    out.push_str("# PR Evidence Summary\n\n");
    render_fast_gate(
        &mut out,
        pr_value,
        review_value,
        &pr_evidence,
        &review_comments,
    );
    render_ripr(&mut out, pr_value, review_value);
    render_targeted_mutation(&mut out, pr_value);
    render_artifacts(&mut out, repo, &pr_evidence, &review_comments);
    out.push_str(
        "\n_This summary is generated from diff-scoped artifacts. Do not copy it into public badge state._\n",
    );
    out
}

fn render_fast_gate(
    out: &mut String,
    pr_value: Option<&Value>,
    review_value: Option<&Value>,
    pr_evidence: &JsonInput,
    review_comments: &JsonInput,
) {
    out.push_str("## Fast Gate\n\n");
    out.push_str(&format!("- PR evidence JSON: {}\n", pr_evidence.state));
    out.push_str(&format!(
        "- review guidance JSON: {}\n",
        review_comments.state
    ));
    out.push_str(&format!(
        "- PR evidence status: `{}`\n",
        string_field(pr_value, "status")
    ));
    out.push_str(&format!(
        "- review guidance status: `{}`\n",
        string_field(review_value, "status")
    ));
    out.push_str(&format!("- base: `{}`\n", string_field(pr_value, "base")));
    out.push_str(&format!("- head: `{}`\n", string_field(pr_value, "head")));
    out.push_str(&format!(
        "- changed files: {}\n\n",
        summary_u64(pr_value, "changed_files")
    ));
}

fn render_ripr(out: &mut String, pr_value: Option<&Value>, review_value: Option<&Value>) {
    out.push_str("## RIPR\n\n");
    out.push_str(&format!(
        "- changed-line comments: {}\n",
        summary_u64(review_value.or(pr_value), "comments")
    ));
    out.push_str(&format!(
        "- summary-only guidance: {}\n",
        summary_u64(review_value.or(pr_value), "summary_only")
    ));
    out.push_str(&format!(
        "- suppressed guidance: {}\n",
        summary_u64(review_value.or(pr_value), "suppressed")
    ));
    out.push_str(&format!(
        "- weakly_exposed: {}\n",
        summary_u64(pr_value, "weakly_exposed")
    ));
    out.push_str(&format!(
        "- reachable_unrevealed: {}\n",
        summary_u64(pr_value, "reachable_unrevealed")
    ));
    out.push_str(&format!(
        "- no_static_path: {}\n",
        summary_u64(pr_value, "no_static_path")
    ));
    out.push_str(&format!(
        "- severe gaps: {}\n\n",
        summary_u64(pr_value, "severe_gaps")
    ));
}

fn render_targeted_mutation(out: &mut String, pr_value: Option<&Value>) {
    out.push_str("## Targeted Mutation\n\n");
    out.push_str(&format!(
        "- requires_targeted_mutation: {}\n",
        summary_bool(pr_value, "requires_targeted_mutation")
    ));
    out.push_str(&format!(
        "- ripr_severe_gap: {}\n",
        summary_bool(pr_value, "ripr_severe_gap")
    ));
    out.push_str(&format!(
        "- routing_reason: `{}`\n\n",
        summary_string_or_null(pr_value, "routing_reason")
    ));
}

fn render_artifacts(
    out: &mut String,
    repo: &Path,
    pr_evidence: &JsonInput,
    review_comments: &JsonInput,
) {
    out.push_str("## Artifacts\n\n");
    out.push_str("| Artifact | Path | State |\n");
    out.push_str("| --- | --- | --- |\n");
    out.push_str(&format!(
        "| PR evidence JSON | `{}` | {} |\n",
        PR_EVIDENCE_JSON, pr_evidence.state
    ));
    out.push_str(&format!(
        "| PR evidence Markdown | `{}` | {} |\n",
        PR_EVIDENCE_MD,
        file_state(repo, PR_EVIDENCE_MD)
    ));
    out.push_str(&format!(
        "| Review guidance JSON | `{}` | {} |\n",
        REVIEW_COMMENTS_JSON, review_comments.state
    ));
    out.push_str(&format!(
        "| Review guidance Markdown | `{}` | {} |\n",
        REVIEW_COMMENTS_MD,
        file_state(repo, REVIEW_COMMENTS_MD)
    ));
    out.push_str(&format!(
        "| PR evidence summary Markdown | `{}` | generated |\n",
        PR_SUMMARY_MD
    ));
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct JsonInput {
    state: InputState,
    value: Option<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum InputState {
    Present,
    Missing,
    Invalid(String),
}

impl std::fmt::Display for InputState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Present => f.write_str("present"),
            Self::Missing => f.write_str("missing"),
            Self::Invalid(err) => write!(f, "invalid: {}", md_escape(err)),
        }
    }
}

fn load_json(repo: &Path, relative: &str) -> JsonInput {
    let path = repo.join(relative);
    let Ok(text) = fs::read_to_string(&path) else {
        return JsonInput {
            state: InputState::Missing,
            value: None,
        };
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(value) => JsonInput {
            state: InputState::Present,
            value: Some(value),
        },
        Err(err) => JsonInput {
            state: InputState::Invalid(first_line(&err.to_string())),
            value: None,
        },
    }
}

fn file_state(repo: &Path, relative: &str) -> &'static str {
    if repo.join(relative).exists() {
        "present"
    } else {
        "missing"
    }
}

fn string_field(value: Option<&Value>, key: &str) -> String {
    value
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .map(md_escape)
        .unwrap_or_else(|| "not_available".to_string())
}

fn summary_u64(value: Option<&Value>, key: &str) -> String {
    value
        .and_then(|value| value.get("summary"))
        .and_then(|summary| summary.get(key))
        .and_then(Value::as_u64)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "not_available".to_string())
}

fn summary_bool(value: Option<&Value>, key: &str) -> String {
    value
        .and_then(|value| value.get("summary"))
        .and_then(|summary| summary.get(key))
        .and_then(Value::as_bool)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "not_available".to_string())
}

fn summary_string_or_null(value: Option<&Value>, key: &str) -> String {
    let Some(value) = value
        .and_then(|value| value.get("summary"))
        .and_then(|summary| summary.get(key))
    else {
        return "not_available".to_string();
    };
    if value.is_null() {
        "none".to_string()
    } else {
        value
            .as_str()
            .map(md_escape)
            .unwrap_or_else(|| "invalid".to_string())
    }
}

fn md_escape(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn first_line(value: &str) -> String {
    value.lines().next().unwrap_or(value).trim().to_string()
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
    fn parse_accepts_check_only() {
        assert_eq!(
            parse_options(&["--check".to_string()]),
            Ok(SummaryOptions { check: true })
        );
        assert_eq!(
            parse_options(&["--bad".to_string()]),
            Err("unknown ripr-pr-summary argument \"--bad\"".to_string())
        );
    }

    #[test]
    fn summary_renders_from_machine_readable_artifacts() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-summary-valid")?;
        write_json(
            &repo,
            PR_EVIDENCE_JSON,
            &json!({
                "status": "advisory",
                "base": "origin/main",
                "head": "HEAD",
                "summary": {
                    "changed_files": 2,
                    "weakly_exposed": 1,
                    "reachable_unrevealed": 0,
                    "no_static_path": 0,
                    "severe_gaps": 1,
                    "requires_targeted_mutation": true,
                    "ripr_severe_gap": true,
                    "routing_reason": "ripr severe gap"
                }
            }),
        )?;
        write_json(
            &repo,
            REVIEW_COMMENTS_JSON,
            &json!({
                "status": "advisory",
                "summary": {
                    "comments": 1,
                    "summary_only": 2,
                    "suppressed": 3
                }
            }),
        )?;
        fs::write(repo.join(PR_EVIDENCE_MD), "# PR Evidence\n")
            .map_err(|err| format!("write PR md: {err}"))?;
        fs::write(repo.join(REVIEW_COMMENTS_MD), "# Guidance\n")
            .map_err(|err| format!("write review md: {err}"))?;

        let summary = render_pr_evidence_summary(&repo);
        assert!(summary.contains("# PR Evidence Summary"));
        assert!(summary.contains("## Fast Gate"));
        assert!(summary.contains("## RIPR"));
        assert!(summary.contains("## Targeted Mutation"));
        assert!(summary.contains("- changed-line comments: 1"));
        assert!(summary.contains("- routing_reason: `ripr severe gap`"));
        assert!(summary.contains("target/ripr/pr/repo-exposure.json"));
        assert!(summary.contains("target/ripr/review/comments.json"));

        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))
    }

    #[test]
    fn summary_makes_missing_artifacts_explicit() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-summary-missing")?;
        let summary = render_pr_evidence_summary(&repo);
        assert!(summary.contains("- PR evidence JSON: missing"));
        assert!(summary.contains("- review guidance JSON: missing"));
        assert!(summary.contains("- changed files: not_available"));
        assert!(
            summary.contains(
                "| Review guidance Markdown | `target/ripr/review/comments.md` | missing |"
            )
        );
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))
    }

    #[test]
    fn summary_handles_error_packets_and_invalid_json() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-summary-error")?;
        write_json(
            &repo,
            PR_EVIDENCE_JSON,
            &json!({
                "status": "error",
                "base": "main",
                "head": "HEAD",
                "summary": {
                    "changed_files": 0,
                    "requires_targeted_mutation": false,
                    "ripr_severe_gap": false,
                    "routing_reason": null
                }
            }),
        )?;
        write_file(&repo, REVIEW_COMMENTS_JSON, "{not json")?;
        let summary = render_pr_evidence_summary(&repo);
        assert!(summary.contains("- PR evidence status: `error`"));
        assert!(summary.contains("- routing_reason: `none`"));
        assert!(summary.contains("- review guidance JSON: invalid:"));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))
    }

    #[test]
    fn check_rejects_stale_summary() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-summary-stale")?;
        let path = repo.join(PR_SUMMARY_MD);
        fs::create_dir_all(path.parent().ok_or_else(|| "summary parent".to_string())?)
            .map_err(|err| format!("create summary parent: {err}"))?;
        fs::write(&path, "stale\n").map_err(|err| format!("write stale summary: {err}"))?;
        let expected = render_pr_evidence_summary(&repo);
        let err = match check_summary(&path, &expected) {
            Ok(()) => return Err("stale summary should fail".to_string()),
            Err(err) => err,
        };
        assert!(err.contains("target/ripr/pr/summary.md is stale"));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))
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
