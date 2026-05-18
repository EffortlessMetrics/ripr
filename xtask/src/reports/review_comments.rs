use super::{ensure_parent_dir, write_parented_file};
use crate::run::{capture_output_with_timeout, run_output_owned};
use crate::verification_contracts::validate_json_file_against_schema;
use serde_json::Value;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEFAULT_ROOT: &str = ".";
const DEFAULT_BASE: &str = "origin/main";
const DEFAULT_HEAD: &str = "HEAD";
const REVIEW_COMMENTS_JSON: &str = "target/ripr/review/comments.json";
const REVIEW_COMMENTS_MD: &str = "target/ripr/review/comments.md";
const REVIEW_COMMENTS_SCHEMA: &str = "schemas/ripr/review-comments.schema.json";
const DEFAULT_TOOL_TIMEOUT_SECS: u64 = 120;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReviewCommentsOptions {
    root: String,
    base: String,
    head: String,
    check: bool,
}

impl Default for ReviewCommentsOptions {
    fn default() -> Self {
        Self {
            root: DEFAULT_ROOT.to_string(),
            base: DEFAULT_BASE.to_string(),
            head: DEFAULT_HEAD.to_string(),
            check: false,
        }
    }
}

pub(crate) fn ripr_review_comments(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }
    let options = parse_options(args)?;
    let repo = repo_root()?;
    if options.check {
        check_review_comments(&repo, &options)
    } else {
        write_review_comments(&repo, &options)
    }
}

fn parse_options(args: &[String]) -> Result<ReviewCommentsOptions, String> {
    let mut options = ReviewCommentsOptions::default();
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                options.root = non_empty_arg(args, i, "--root")?.to_string();
            }
            "--base" => {
                i += 1;
                options.base = non_empty_arg(args, i, "--base")?.to_string();
            }
            "--head" => {
                i += 1;
                options.head = non_empty_arg(args, i, "--head")?.to_string();
            }
            "--check" => options.check = true,
            other => return Err(format!("unknown ripr-review-comments argument {other:?}")),
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
            "ripr-review-comments {flag} requires a non-empty value"
        ));
    }
    Ok(value)
}

fn print_help() {
    println!(
        "usage: cargo xtask ripr-review-comments [--base <rev>] [--head <rev>] [--root <path>] [--check]"
    );
}

fn write_review_comments(repo: &Path, options: &ReviewCommentsOptions) -> Result<(), String> {
    write_review_comments_with_runner(repo, options, run_ripr_review_comments)
}

fn write_review_comments_with_runner(
    repo: &Path,
    options: &ReviewCommentsOptions,
    run_producer: impl FnOnce(&Path, &ReviewCommentsOptions) -> Result<(), String>,
) -> Result<(), String> {
    verify_revision(repo, &options.base)?;
    verify_revision(repo, &options.head)?;
    if !has_changed_paths(repo, &options.base, &options.head)? {
        write_empty_review_comments(repo, options)?;
    } else if let Err(err) = run_producer(repo, options) {
        write_error_review_comments(repo, options, &err)?;
    }
    validate_review_comments(repo, options, true)?;
    println!("Wrote {REVIEW_COMMENTS_JSON}");
    println!("Wrote {REVIEW_COMMENTS_MD}");
    Ok(())
}

fn check_review_comments(repo: &Path, options: &ReviewCommentsOptions) -> Result<(), String> {
    verify_revision(repo, &options.base)?;
    verify_revision(repo, &options.head)?;
    validate_review_comments(repo, options, true)?;
    println!("Review comments contract ok: {REVIEW_COMMENTS_JSON}");
    Ok(())
}

fn validate_review_comments(
    repo: &Path,
    options: &ReviewCommentsOptions,
    markdown_required: bool,
) -> Result<(), String> {
    validate_json_file_against_schema(repo, REVIEW_COMMENTS_JSON, REVIEW_COMMENTS_SCHEMA)?;
    let json_path = repo.join(REVIEW_COMMENTS_JSON);
    let markdown_path = repo.join(REVIEW_COMMENTS_MD);
    let text = fs::read_to_string(&json_path)
        .map_err(|err| format!("missing or unreadable {REVIEW_COMMENTS_JSON}: {err}"))?;
    let packet: Value = serde_json::from_str(&text)
        .map_err(|err| format!("{REVIEW_COMMENTS_JSON} is not valid JSON: {err}"))?;
    let violations =
        validate_packet_value(&packet, repo, options, markdown_required, &markdown_path);
    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "review comments contract violations:\n{}",
            violations
                .iter()
                .map(|violation| format!("- {violation}"))
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }
}

fn validate_packet_value(
    packet: &Value,
    repo: &Path,
    options: &ReviewCommentsOptions,
    markdown_required: bool,
    markdown_path: &Path,
) -> Vec<String> {
    let mut violations = Vec::new();
    expect_string(packet, "schema_version", "0.1", &mut violations);
    expect_string(packet, "tool", "ripr", &mut violations);
    expect_string(packet, "base", options.base.as_str(), &mut violations);
    expect_string(packet, "head", options.head.as_str(), &mut violations);

    let expected_root = normalize_path_text(&command_root_arg(repo, &options.root));
    expect_string(packet, "root", expected_root.as_str(), &mut violations);

    match packet.get("status").and_then(Value::as_str) {
        Some("advisory" | "incomplete" | "error") => {}
        Some(other) => violations.push(format!("status {other:?} is not contract-valid")),
        None => violations.push("status is missing or not a string".to_string()),
    }
    match packet.get("mode").and_then(Value::as_str) {
        Some("instant" | "draft" | "fast" | "deep" | "ready") => {}
        Some(other) => violations.push(format!("mode {other:?} is not contract-valid")),
        None => violations.push("mode is missing or not a string".to_string()),
    }

    validate_rendering_limits(packet, &mut violations);
    validate_summary_counts(packet, &mut violations);

    for key in ["comments", "summary_only", "suppressed", "warnings"] {
        if !packet.get(key).is_some_and(Value::is_array) {
            violations.push(format!("{key} is missing or not an array"));
        }
    }
    if !packet.get("limits_note").is_some_and(non_empty_string) {
        violations.push("limits_note is missing or empty".to_string());
    }
    if markdown_required && !markdown_path.exists() {
        violations.push(format!("{REVIEW_COMMENTS_MD} is missing"));
    }
    violations
}

fn validate_rendering_limits(packet: &Value, violations: &mut Vec<String>) {
    let Some(limits) = packet.get("rendering_limits").and_then(Value::as_object) else {
        violations.push("rendering_limits is missing or not an object".to_string());
        return;
    };
    for key in ["max_inline_comments", "max_summary_items"] {
        if !limits.get(key).is_some_and(Value::is_u64) {
            violations.push(format!(
                "rendering_limits.{key} is missing or not an integer"
            ));
        }
    }
}

fn validate_summary_counts(packet: &Value, violations: &mut Vec<String>) {
    let Some(summary) = packet.get("summary").and_then(Value::as_object) else {
        violations.push("summary is missing or not an object".to_string());
        return;
    };

    for key in ["comments", "summary_only", "suppressed"] {
        match summary.get(key).and_then(Value::as_u64) {
            Some(value) if value == array_len(packet, key) as u64 => {}
            Some(value) => violations.push(format!(
                "summary.{key} is {value}, expected {}",
                array_len(packet, key)
            )),
            None => violations.push(format!(
                "summary.{key} is missing or not a non-negative integer"
            )),
        }
    }
    if !summary
        .get("unchanged_tests")
        .is_some_and(Value::is_boolean)
    {
        violations.push("summary.unchanged_tests is missing or not a boolean".to_string());
    }
}

fn array_len(packet: &Value, key: &str) -> usize {
    packet
        .get(key)
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

fn expect_string(packet: &Value, key: &str, expected: &str, violations: &mut Vec<String>) {
    match packet.get(key).and_then(Value::as_str) {
        Some(actual) if actual == expected => {}
        Some(actual) => violations.push(format!("{key} is {actual:?}, expected {expected:?}")),
        None => violations.push(format!("{key} is missing or not a string")),
    }
}

fn non_empty_string(value: &Value) -> bool {
    value.as_str().is_some_and(|text| !text.trim().is_empty())
}

fn run_ripr_review_comments(repo: &Path, options: &ReviewCommentsOptions) -> Result<(), String> {
    let out = repo.join(REVIEW_COMMENTS_JSON);
    ensure_parent_dir(&out, REVIEW_COMMENTS_JSON)?;

    let root_arg = command_root_arg(repo, &options.root);
    let out_arg = out.display().to_string();
    let ripr_args = vec![
        "review-comments".to_string(),
        "--root".to_string(),
        root_arg,
        "--base".to_string(),
        options.base.clone(),
        "--head".to_string(),
        options.head.clone(),
        "--out".to_string(),
        out_arg,
    ];
    let binary = match env::var("RIPR_BIN") {
        Ok(binary) => {
            if binary.trim().is_empty() {
                return Err("RIPR_BIN is set but empty".to_string());
            }
            binary
        }
        Err(_) => {
            let build_args = [
                "build".to_string(),
                "--manifest-path".to_string(),
                repo.join("Cargo.toml").display().to_string(),
                "-p".to_string(),
                "ripr".to_string(),
                "--quiet".to_string(),
            ];
            run_output_owned("cargo", &build_args)?;
            built_ripr_binary_path(repo)?.display().to_string()
        }
    };
    let timeout = Duration::from_secs(review_comments_timeout_secs()?);
    let output =
        capture_output_with_timeout(&binary, &ripr_args, &[], timeout, "ripr review-comments")?;
    if output.timed_out {
        return Err(format!(
            "ripr review-comments timed out after {} seconds",
            output.duration.as_secs()
        ));
    }
    if output.status.is_some_and(|status| status.success()) {
        Ok(())
    } else {
        Err(format!(
            "ripr review-comments failed\nstdout:\n{}\nstderr:\n{}",
            output.stdout.trim(),
            output.stderr.trim()
        ))
    }
}

fn review_comments_timeout_secs() -> Result<u64, String> {
    match env::var("RIPR_REVIEW_COMMENTS_TIMEOUT_SECS") {
        Ok(value) => value.trim().parse::<u64>().map_err(|err| {
            format!("RIPR_REVIEW_COMMENTS_TIMEOUT_SECS must be a positive integer: {err}")
        }),
        Err(_) => Ok(DEFAULT_TOOL_TIMEOUT_SECS),
    }
}

fn ripr_exe_name() -> &'static str {
    if cfg!(windows) { "ripr.exe" } else { "ripr" }
}

fn built_ripr_binary_path(repo: &Path) -> Result<PathBuf, String> {
    let cwd = env::current_dir().map_err(|err| format!("resolve current directory: {err}"))?;
    Ok(built_ripr_binary_path_from_target_dir(
        repo,
        &cwd,
        env::var_os("CARGO_TARGET_DIR").as_deref(),
    ))
}

fn built_ripr_binary_path_from_target_dir(
    repo: &Path,
    cwd: &Path,
    target_dir: Option<&OsStr>,
) -> PathBuf {
    cargo_target_dir(repo, cwd, target_dir)
        .join("debug")
        .join(ripr_exe_name())
}

fn cargo_target_dir(repo: &Path, cwd: &Path, target_dir: Option<&OsStr>) -> PathBuf {
    match target_dir {
        Some(value) if !value.is_empty() => target_dir_from_value(repo, cwd, &PathBuf::from(value)),
        _ => repo.join("target"),
    }
}

fn target_dir_from_value(repo: &Path, cwd: &Path, value: &Path) -> PathBuf {
    if value.is_absolute() {
        value.to_path_buf()
    } else if cwd.is_absolute() {
        cwd.join(value)
    } else {
        repo.join(value)
    }
}

fn write_error_review_comments(
    repo: &Path,
    options: &ReviewCommentsOptions,
    error: &str,
) -> Result<(), String> {
    let packet = error_review_comments_packet(repo, options, error);
    let json_text = serde_json::to_string_pretty(&packet)
        .map_err(|err| format!("serialize review comments error packet: {err}"))?;
    let markdown = render_error_review_comments_markdown(&packet);
    write_review_comments_artifacts(repo, &json_text, &markdown)
}

fn write_empty_review_comments(repo: &Path, options: &ReviewCommentsOptions) -> Result<(), String> {
    let packet = empty_review_comments_packet(repo, options);
    let json_text = serde_json::to_string_pretty(&packet)
        .map_err(|err| format!("serialize review comments empty packet: {err}"))?;
    let markdown = render_empty_review_comments_markdown(&packet);
    write_review_comments_artifacts(repo, &json_text, &markdown)
}

fn write_review_comments_artifacts(
    repo: &Path,
    json_text: &str,
    markdown: &str,
) -> Result<(), String> {
    write_parented_file(
        &repo.join(REVIEW_COMMENTS_JSON),
        REVIEW_COMMENTS_JSON,
        format!("{json_text}\n"),
    )?;
    write_parented_file(&repo.join(REVIEW_COMMENTS_MD), REVIEW_COMMENTS_MD, markdown)
}

fn error_review_comments_packet(
    repo: &Path,
    options: &ReviewCommentsOptions,
    error: &str,
) -> Value {
    serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "status": "error",
        "root": normalize_path_text(&command_root_arg(repo, &options.root)),
        "base": options.base,
        "head": options.head,
        "mode": "fast",
        "rendering_limits": {
            "max_inline_comments": 0,
            "max_summary_items": 0
        },
        "summary": {
            "comments": 0,
            "summary_only": 0,
            "suppressed": 0,
            "unchanged_tests": true
        },
        "comments": [],
        "summary_only": [],
        "suppressed": [],
        "warnings": [
            {
                "kind": "tool_error",
                "message": first_line(error),
                "path": null
            }
        ],
        "limits_note": "Review guidance generation is advisory. The producer did not complete, so no comments are emitted."
    })
}

fn empty_review_comments_packet(repo: &Path, options: &ReviewCommentsOptions) -> Value {
    serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "status": "advisory",
        "root": normalize_path_text(&command_root_arg(repo, &options.root)),
        "base": options.base,
        "head": options.head,
        "mode": "fast",
        "rendering_limits": {
            "max_inline_comments": 0,
            "max_summary_items": 0
        },
        "summary": {
            "comments": 0,
            "summary_only": 0,
            "suppressed": 0,
            "unchanged_tests": true
        },
        "comments": [],
        "summary_only": [],
        "suppressed": [],
        "warnings": [],
        "limits_note": "No changed paths were detected, so no changed-line review guidance is emitted."
    })
}

fn render_error_review_comments_markdown(packet: &Value) -> String {
    let warning = packet
        .get("warnings")
        .and_then(Value::as_array)
        .and_then(|warnings| warnings.first())
        .and_then(|warning| warning.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("review guidance generation did not complete");
    format!(
        "# RIPR PR Guidance\n\n- status: error\n- base: `{}`\n- head: `{}`\n- line annotations: 0\n- summary-only recommendations: 0\n- suppressed recommendations: 0\n\nNo review guidance was generated.\n\n## Warnings\n\n- tool_error: {}\n",
        packet
            .get("base")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_BASE),
        packet
            .get("head")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_HEAD),
        md_escape(warning)
    )
}

fn render_empty_review_comments_markdown(packet: &Value) -> String {
    format!(
        "# RIPR PR Guidance\n\n- status: advisory\n- base: `{}`\n- head: `{}`\n- line annotations: 0\n- summary-only recommendations: 0\n- suppressed recommendations: 0\n\nNo changed paths were detected.\n",
        packet
            .get("base")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_BASE),
        packet
            .get("head")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_HEAD)
    )
}

fn first_line(value: &str) -> String {
    value
        .lines()
        .next()
        .unwrap_or("ripr review-comments failed")
        .trim()
        .to_string()
}

fn md_escape(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn verify_revision(repo: &Path, rev: &str) -> Result<(), String> {
    let commit = format!("{rev}^{{commit}}");
    run_git_output(repo, &["rev-parse", "--verify", commit.as_str()])
        .map(|_| ())
        .map_err(|err| format!("bad base/head revision {rev:?}: {err}"))
}

fn run_git_output(repo: &Path, args: &[&str]) -> Result<String, String> {
    let mut git_args = vec!["-C".to_string(), repo.display().to_string()];
    git_args.extend(args.iter().map(|arg| (*arg).to_string()));
    run_output_owned("git", &git_args)
}

fn has_changed_paths(repo: &Path, base: &str, head: &str) -> Result<bool, String> {
    let range = format!("{base}..{head}");
    let output = run_git_output(repo, &["diff", "--name-only", range.as_str()])?;
    Ok(output.lines().any(|line| !line.trim().is_empty()))
}

fn command_root_arg(repo: &Path, root: &str) -> String {
    let root_path = Path::new(root);
    if root_path.is_absolute() {
        return root.to_string();
    }
    repo.join(root_path).display().to_string()
}

fn normalize_path_text(value: &str) -> String {
    value.replace('\\', "/")
}

#[cfg(test)]
fn repo_root_for_display() -> PathBuf {
    repo_root().unwrap_or_else(|_| PathBuf::from("."))
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

    fn options() -> ReviewCommentsOptions {
        ReviewCommentsOptions {
            root: ".".to_string(),
            base: "origin/main".to_string(),
            head: "HEAD".to_string(),
            check: false,
        }
    }

    #[test]
    fn parse_defaults_and_check_mode() -> Result<(), String> {
        assert_eq!(parse_options(&[])?, options());
        let parsed = parse_options(&["--base".into(), "main".into(), "--check".into()])?;
        assert_eq!(parsed.base, "main");
        assert!(parsed.check);
        Ok(())
    }

    #[test]
    fn parse_rejects_unknown_or_empty_args() {
        assert_eq!(
            parse_options(&["--bad".into()]),
            Err("unknown ripr-review-comments argument \"--bad\"".to_string())
        );
        assert_eq!(
            parse_options(&["--head".into(), "".into()]),
            Err("ripr-review-comments --head requires a non-empty value".to_string())
        );
    }

    #[test]
    fn validation_rejects_summary_count_drift() -> Result<(), String> {
        let packet = valid_packet(&options());
        let mut object = packet
            .as_object()
            .cloned()
            .ok_or_else(|| "packet should be an object".to_string())?;
        let mut summary = object
            .get("summary")
            .and_then(Value::as_object)
            .cloned()
            .ok_or_else(|| "summary should be an object".to_string())?;
        summary.insert("comments".to_string(), json!(99));
        object.insert("summary".to_string(), Value::Object(summary));

        let violations = validate_packet_value(
            &Value::Object(object),
            &repo_root_for_display(),
            &options(),
            false,
            Path::new(REVIEW_COMMENTS_MD),
        );
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("summary.comments is 99, expected 0")),
            "{violations:#?}"
        );
        Ok(())
    }

    #[test]
    fn validation_requires_markdown_artifact() {
        let violations = validate_packet_value(
            &valid_packet(&options()),
            &repo_root_for_display(),
            &options(),
            true,
            Path::new("missing-comments.md"),
        );
        assert!(violations.contains(&format!("{REVIEW_COMMENTS_MD} is missing")));
    }

    #[test]
    fn error_packet_is_contract_valid() {
        let packet =
            error_review_comments_packet(&repo_root_for_display(), &options(), "synthetic failure");
        let violations = validate_packet_value(
            &packet,
            &repo_root_for_display(),
            &options(),
            false,
            Path::new(REVIEW_COMMENTS_MD),
        );
        assert!(violations.is_empty(), "{violations:#?}");
        assert_eq!(packet["status"], "error");
        assert_eq!(packet["warnings"][0]["kind"], "tool_error");
    }

    #[test]
    fn write_and_check_packet_in_git_repo() -> Result<(), String> {
        let (repo, options) = prepared_review_repo("ripr-review-comments")?;
        let packet = valid_packet_for_repo(&repo, &options);
        fs::write(
            repo.join(REVIEW_COMMENTS_JSON),
            serde_json::to_string_pretty(&packet).map_err(|err| format!("serialize: {err}"))?,
        )
        .map_err(|err| format!("write comments JSON: {err}"))?;
        fs::write(repo.join(REVIEW_COMMENTS_MD), "# RIPR PR Guidance\n")
            .map_err(|err| format!("write comments Markdown: {err}"))?;

        check_review_comments(&repo, &options)?;
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn write_wrapper_accepts_successful_producer() -> Result<(), String> {
        let (repo, options) = prepared_review_repo("ripr-review-comments-success")?;
        write_review_comments_with_runner(&repo, &options, |repo, options| {
            let packet = valid_packet_for_repo(repo, options);
            fs::write(
                repo.join(REVIEW_COMMENTS_JSON),
                serde_json::to_string_pretty(&packet)
                    .map_err(|err| format!("serialize success packet: {err}"))?,
            )
            .map_err(|err| format!("write success JSON: {err}"))?;
            fs::write(repo.join(REVIEW_COMMENTS_MD), "# RIPR PR Guidance\n")
                .map_err(|err| format!("write success Markdown: {err}"))
        })?;

        let packet = read_packet(&repo)?;
        assert_eq!(packet["status"], "advisory");
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn write_wrapper_converts_producer_failure_to_error_packet() -> Result<(), String> {
        let (repo, options) = prepared_review_repo("ripr-review-comments-error")?;
        write_review_comments_with_runner(&repo, &options, |_repo, _options| {
            Err("synthetic producer failure\nsecond line".to_string())
        })?;

        let packet = read_packet(&repo)?;
        assert_eq!(packet["status"], "error");
        assert_eq!(
            packet["warnings"][0]["message"],
            "synthetic producer failure"
        );
        let markdown = fs::read_to_string(repo.join(REVIEW_COMMENTS_MD))
            .map_err(|err| format!("read error Markdown: {err}"))?;
        assert!(markdown.contains("No review guidance was generated."));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn write_wrapper_skips_producer_for_empty_diff() -> Result<(), String> {
        let (repo, mut options) = prepared_review_repo("ripr-review-comments-empty")?;
        options.base = "HEAD".to_string();
        options.head = "HEAD".to_string();

        write_review_comments_with_runner(&repo, &options, |_repo, _options| {
            Err("producer should not run for an empty diff".to_string())
        })?;

        let packet = read_packet(&repo)?;
        assert_eq!(packet["status"], "advisory");
        assert_eq!(packet["summary"]["comments"], 0);
        assert_eq!(
            packet["warnings"].as_array().map_or(usize::MAX, Vec::len),
            0
        );
        let markdown = fs::read_to_string(repo.join(REVIEW_COMMENTS_MD))
            .map_err(|err| format!("read empty Markdown: {err}"))?;
        assert!(markdown.contains("No changed paths were detected."));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn changed_path_detection_distinguishes_empty_and_non_empty_diffs() -> Result<(), String> {
        let (repo, options) = prepared_review_repo("ripr-review-comments-diff")?;

        assert!(has_changed_paths(&repo, &options.base, &options.head)?);
        assert!(!has_changed_paths(&repo, "HEAD", "HEAD")?);
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn built_path_resolves_to_debug_ripr_binary() -> Result<(), String> {
        let repo = env::temp_dir().join("ripr-review-repo");
        let path = built_ripr_binary_path(&repo)?;

        assert_eq!(path.file_name(), Some(OsStr::new(ripr_exe_name())));
        assert_eq!(
            path.parent().and_then(Path::file_name),
            Some(OsStr::new("debug"))
        );
        Ok(())
    }

    #[test]
    fn target_dir_honors_default_absolute_and_relative_cargo_target_dir() {
        let repo = env::temp_dir().join("ripr-review-repo");
        let cwd = env::temp_dir().join("ripr-review-cwd");
        let absolute_target = env::temp_dir().join("ripr-review-target");

        assert_eq!(
            built_ripr_binary_path_from_target_dir(&repo, &cwd, None),
            repo.join("target").join("debug").join(ripr_exe_name())
        );
        assert_eq!(
            built_ripr_binary_path_from_target_dir(&repo, &cwd, Some(absolute_target.as_os_str())),
            absolute_target.join("debug").join(ripr_exe_name())
        );
        assert_eq!(
            target_dir_from_value(&repo, &cwd, &absolute_target),
            absolute_target
        );
        assert_eq!(
            target_dir_from_value(&repo, &cwd, Path::new("target-alt")),
            cwd.join("target-alt")
        );
        assert_eq!(
            target_dir_from_value(&repo, Path::new("relative-cwd"), Path::new("target-alt")),
            repo.join("target-alt")
        );
    }

    fn valid_packet(options: &ReviewCommentsOptions) -> Value {
        valid_packet_for_repo(&repo_root_for_display(), options)
    }

    fn valid_packet_for_repo(repo: &Path, options: &ReviewCommentsOptions) -> Value {
        json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "status": "advisory",
            "root": normalize_path_text(&command_root_arg(repo, &options.root)),
            "base": options.base,
            "head": options.head,
            "mode": "fast",
            "rendering_limits": {
                "max_inline_comments": 3,
                "max_summary_items": 10
            },
            "summary": {
                "comments": 0,
                "summary_only": 0,
                "suppressed": 0,
                "unchanged_tests": true
            },
            "comments": [],
            "summary_only": [],
            "suppressed": [],
            "warnings": [],
            "limits_note": "Comments are capped and advisory; summary-only items never annotate."
        })
    }

    fn prepared_review_repo(name: &str) -> Result<(PathBuf, ReviewCommentsOptions), String> {
        let repo = temp_repo(name)?;
        run_git(&repo, &["init"])?;
        run_git(
            &repo,
            &["config", "user.email", "ripr-review@example.invalid"],
        )?;
        run_git(&repo, &["config", "user.name", "RIPR Review Test"])?;
        write_repo_file(&repo, "README.md", "# sample\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "initial"])?;
        write_repo_file(&repo, "src/lib.rs", "pub fn value() -> u8 { 1 }\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "add rust"])?;
        fs::create_dir_all(repo.join("target/ripr/review"))
            .map_err(|err| format!("create out dir: {err}"))?;
        copy_review_comments_schema(&repo)?;
        Ok((
            repo,
            ReviewCommentsOptions {
                root: ".".to_string(),
                base: "HEAD~1".to_string(),
                head: "HEAD".to_string(),
                check: false,
            },
        ))
    }

    fn copy_review_comments_schema(repo: &Path) -> Result<(), String> {
        let schema_path = repo.join(REVIEW_COMMENTS_SCHEMA);
        fs::create_dir_all(
            schema_path
                .parent()
                .ok_or_else(|| "review comments schema path has no parent".to_string())?,
        )
        .map_err(|err| format!("create schema dir: {err}"))?;
        fs::copy(
            repo_root_for_display().join(REVIEW_COMMENTS_SCHEMA),
            &schema_path,
        )
        .map_err(|err| format!("copy review comments schema: {err}"))?;
        Ok(())
    }

    fn read_packet(repo: &Path) -> Result<Value, String> {
        let text = fs::read_to_string(repo.join(REVIEW_COMMENTS_JSON))
            .map_err(|err| format!("read packet: {err}"))?;
        serde_json::from_str(&text).map_err(|err| format!("parse packet: {err}"))
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

    fn write_repo_file(repo: &Path, relative: &str, text: &str) -> Result<(), String> {
        let path = repo.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("create {}: {err}", parent.display()))?;
        }
        fs::write(&path, text).map_err(|err| format!("write {}: {err}", path.display()))
    }

    fn run_git(repo: &Path, args: &[&str]) -> Result<(), String> {
        run_git_output(repo, args).map(|_| ())
    }
}
