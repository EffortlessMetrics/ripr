use super::{ensure_parent_dir, write_parented_file};
use crate::run::{
    capture_output_with_timeout, run_output_owned, run_output_owned_with_timeout,
    tool_build_timeout,
};
use crate::verification_contracts::validate_json_file_against_schema;
use serde_json::Value;
use sha2::{Digest, Sha256};
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
const REVIEW_COMMENTS_RECEIPT: &str = "target/ripr/review/run-receipt.json";
const REVIEW_COMMENTS_SCHEMA: &str = "schemas/ripr/review-comments.schema.json";
const DEFAULT_TOOL_TIMEOUT_SECS: u64 = 120;

#[derive(Debug)]
struct ReviewCommentsRunError {
    message: String,
    timed_out: bool,
}

impl ReviewCommentsRunError {
    fn timed_out(message: String) -> Self {
        Self {
            message,
            timed_out: true,
        }
    }
}

impl From<String> for ReviewCommentsRunError {
    fn from(message: String) -> Self {
        Self {
            message,
            timed_out: false,
        }
    }
}

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

fn write_review_comments_with_runner<E>(
    repo: &Path,
    options: &ReviewCommentsOptions,
    run_producer: impl FnOnce(&Path, &ReviewCommentsOptions) -> Result<(), E>,
) -> Result<(), String>
where
    E: Into<ReviewCommentsRunError>,
{
    verify_revision(repo, &options.base)?;
    verify_revision(repo, &options.head)?;
    remove_stale_review_artifacts(repo)?;
    if !has_changed_paths(repo, &options.base, &options.head)? {
        write_empty_review_comments(repo, options)?;
    } else {
        match run_producer(repo, options).map_err(Into::into) {
            Ok(()) => ensure_review_comments_receipt(repo, options, "complete", None)?,
            Err(err) => {
                let status = if err.timed_out {
                    "limited_timeout"
                } else {
                    "failed"
                };
                let receipt = review_comments_receipt(repo, options, status, Some(&err.message));
                write_receipt_file(repo, &receipt)?;
                write_error_review_comments(repo, options, &err.message, &receipt)?;
            }
        }
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
    validate_run_receipt(packet, repo, options, &mut violations);

    for key in ["comments", "summary_only", "suppressed", "warnings"] {
        if !packet.get(key).is_some_and(Value::is_array) {
            violations.push(format!("{key} is missing or not an array"));
        }
    }
    if let Some(warnings) = packet.get("warnings").and_then(Value::as_array) {
        for (index, warning) in warnings.iter().enumerate() {
            if !warning.is_object() {
                violations.push(format!("warnings[{index}] is not an object"));
            }
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

fn validate_run_receipt(
    packet: &Value,
    repo: &Path,
    options: &ReviewCommentsOptions,
    violations: &mut Vec<String>,
) {
    let Some(receipt) = packet.get("run_receipt").and_then(Value::as_object) else {
        violations.push("run_receipt is missing or not an object".to_string());
        return;
    };
    for key in [
        "schema_version",
        "status",
        "root_identity",
        "base_sha",
        "head_sha",
        "last_completed_phase",
        "active_phase",
        "reusable_cache_identity",
        "atomic_write_status",
    ] {
        if !receipt.contains_key(key) {
            violations.push(format!("run_receipt.{key} is missing"));
        }
    }
    expect_string_value(
        receipt.get("schema_version"),
        "0.1",
        "run_receipt.schema_version",
        violations,
    );
    let receipt_root = PathBuf::from(command_root_arg(repo, &options.root));
    let expected_base = resolve_revision_identity(&receipt_root, &options.base);
    let expected_head = resolve_revision_identity(&receipt_root, &options.head);
    expect_string_value(
        receipt.get("base_sha"),
        &expected_base,
        "run_receipt.base_sha",
        violations,
    );
    expect_string_value(
        receipt.get("head_sha"),
        &expected_head,
        "run_receipt.head_sha",
        violations,
    );
    let receipt_status = receipt.get("status").and_then(Value::as_str);
    match receipt_status {
        Some("in_progress" | "complete" | "limited_timeout" | "failed") => {}
        Some(other) => violations.push(format!(
            "run_receipt.status {other:?} is not contract-valid"
        )),
        None => violations.push("run_receipt.status is missing or not a string".to_string()),
    }
    if receipt
        .get("configured_timeout_ms")
        .is_none_or(|value| value.as_u64().is_none_or(|timeout| timeout == 0))
    {
        violations.push("run_receipt.configured_timeout_ms is missing or invalid".to_string());
    }
    for key in [
        "completed_artifacts",
        "missing_artifacts",
        "limitations",
        "non_claims",
    ] {
        if !receipt.get(key).is_some_and(Value::is_array) {
            violations.push(format!("run_receipt.{key} is missing or not an array"));
        }
    }
    match receipt_status {
        Some("complete") => {
            if receipt
                .get("last_completed_phase")
                .is_none_or(|value| value.as_str() != Some("artifact_io"))
            {
                violations.push(
                    "complete run_receipt.last_completed_phase must be artifact_io".to_string(),
                );
            }
            if !receipt.get("active_phase").is_some_and(Value::is_null) {
                violations.push("complete run_receipt.active_phase must be null".to_string());
            }
            if receipt
                .get("completed_artifacts")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
            {
                violations
                    .push("complete run_receipt.completed_artifacts must not be empty".to_string());
            }
            if receipt
                .get("missing_artifacts")
                .and_then(Value::as_array)
                .is_some_and(|artifacts| !artifacts.is_empty())
            {
                violations.push("complete run_receipt.missing_artifacts must be empty".to_string());
            }
        }
        Some("limited_timeout" | "failed") => {
            if receipt
                .get("active_phase")
                .and_then(Value::as_str)
                .is_none_or(|phase| phase.trim().is_empty())
            {
                violations.push(format!(
                    "{receipt_status:?} run_receipt.active_phase must name the interrupted phase"
                ));
            }
            if receipt
                .get("missing_artifacts")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
            {
                violations.push(format!(
                    "{receipt_status:?} run_receipt.missing_artifacts must not be empty"
                ));
            }
        }
        Some("in_progress") | None => {}
        Some(_) => {}
    }
}

fn expect_string_value(
    actual: Option<&Value>,
    expected: &str,
    key: &str,
    violations: &mut Vec<String>,
) {
    match actual.and_then(Value::as_str) {
        Some(value) if value == expected => {}
        Some(value) => violations.push(format!("{key} is {value:?}, expected {expected:?}")),
        None => violations.push(format!("{key} is missing or not a string")),
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

fn run_ripr_review_comments(
    repo: &Path,
    options: &ReviewCommentsOptions,
) -> Result<(), ReviewCommentsRunError> {
    let out = repo.join(REVIEW_COMMENTS_JSON);
    ensure_parent_dir(&out, REVIEW_COMMENTS_JSON)?;

    let root_arg = command_root_arg(repo, &options.root);
    let out_arg = out.display().to_string();
    let timeout_ms = review_comments_timeout_ms()?;
    let ripr_args = vec![
        "review-comments".to_string(),
        "--root".to_string(),
        root_arg,
        "--base".to_string(),
        options.base.clone(),
        "--head".to_string(),
        options.head.clone(),
        "--timeout-ms".to_string(),
        timeout_ms.to_string(),
        "--out".to_string(),
        out_arg,
    ];
    let binary = match env::var("RIPR_BIN") {
        Ok(binary) => {
            if binary.trim().is_empty() {
                return Err("RIPR_BIN is set but empty".to_string().into());
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
            run_output_owned_with_timeout(
                "cargo",
                &build_args,
                tool_build_timeout()?,
                "cargo build of the ripr binary for review-comments",
            )?;
            built_ripr_binary_path(repo)?.display().to_string()
        }
    };
    let timeout = Duration::from_secs(review_comments_timeout_secs()?);
    let output =
        capture_output_with_timeout(&binary, &ripr_args, &[], timeout, "ripr review-comments")?;
    if output.timed_out {
        return Err(ReviewCommentsRunError::timed_out(format!(
            "ripr review-comments timed out after {} seconds",
            output.duration.as_secs()
        )));
    }
    if output.status.is_some_and(|status| status.success()) {
        Ok(())
    } else {
        Err(ReviewCommentsRunError::from(format!(
            "ripr review-comments failed\nstdout:\n{}\nstderr:\n{}",
            output.stdout.trim(),
            output.stderr.trim()
        )))
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

fn review_comments_timeout_ms() -> Result<u64, String> {
    let seconds = review_comments_timeout_secs()?;
    if seconds == 0 {
        return Err("RIPR_REVIEW_COMMENTS_TIMEOUT_SECS must be a positive integer".to_string());
    }
    seconds
        .checked_mul(1_000)
        .ok_or_else(|| "RIPR_REVIEW_COMMENTS_TIMEOUT_SECS is too large".to_string())
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
    receipt: &Value,
) -> Result<(), String> {
    let packet = error_review_comments_packet(repo, options, error, receipt);
    let json_text = serde_json::to_string_pretty(&packet)
        .map_err(|err| format!("serialize review comments error packet: {err}"))?;
    let markdown = render_error_review_comments_markdown(&packet);
    write_review_comments_artifacts(repo, &json_text, &markdown)
}

fn write_empty_review_comments(repo: &Path, options: &ReviewCommentsOptions) -> Result<(), String> {
    let receipt = review_comments_receipt(repo, options, "complete", None);
    write_receipt_file(repo, &receipt)?;
    let packet = empty_review_comments_packet(repo, options, &receipt);
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
    receipt: &Value,
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
        "limits_note": "Review guidance generation is advisory. The producer did not complete, so no comments are emitted.",
        "run_receipt": receipt
    })
}

fn empty_review_comments_packet(
    repo: &Path,
    options: &ReviewCommentsOptions,
    receipt: &Value,
) -> Value {
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
        "limits_note": "No changed paths were detected, so no changed-line review guidance is emitted.",
        "run_receipt": receipt
    })
}

fn remove_stale_review_artifacts(repo: &Path) -> Result<(), String> {
    for relative in [
        REVIEW_COMMENTS_JSON,
        REVIEW_COMMENTS_MD,
        REVIEW_COMMENTS_RECEIPT,
    ] {
        let path = repo.join(relative);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(format!("remove stale {relative} failed: {err}")),
        }
    }
    Ok(())
}

fn ensure_review_comments_receipt(
    repo: &Path,
    options: &ReviewCommentsOptions,
    status: &str,
    error: Option<&str>,
) -> Result<(), String> {
    let packet_path = repo.join(REVIEW_COMMENTS_JSON);
    if !packet_path.is_file() {
        return Ok(());
    }
    let text = fs::read_to_string(&packet_path).map_err(|err| {
        format!("read {REVIEW_COMMENTS_JSON} for receipt attachment failed: {err}")
    })?;
    let mut packet: Value = serde_json::from_str(&text).map_err(|err| {
        format!("parse {REVIEW_COMMENTS_JSON} for receipt attachment failed: {err}")
    })?;
    let mut receipt = packet
        .get("run_receipt")
        .cloned()
        .unwrap_or_else(|| review_comments_receipt(repo, options, status, error));
    if status == "complete"
        && let Some(object) = receipt.as_object_mut()
    {
        object.insert("status".to_string(), Value::String("complete".to_string()));
        object.insert(
            "last_completed_phase".to_string(),
            Value::String("artifact_io".to_string()),
        );
        object.insert("active_phase".to_string(), Value::Null);
        object.insert(
            "completed_artifacts".to_string(),
            serde_json::json!([REVIEW_COMMENTS_JSON, REVIEW_COMMENTS_MD]),
        );
        object.insert("missing_artifacts".to_string(), serde_json::json!([]));
    }
    write_receipt_file(repo, &receipt)?;
    if let Some(object) = packet.as_object_mut() {
        object.insert("run_receipt".to_string(), receipt);
    }
    let rendered = serde_json::to_string_pretty(&packet)
        .map_err(|err| format!("serialize {REVIEW_COMMENTS_JSON} with receipt failed: {err}"))?;
    write_parented_file(&packet_path, REVIEW_COMMENTS_JSON, format!("{rendered}\n"))
}

fn write_receipt_file(repo: &Path, receipt: &Value) -> Result<(), String> {
    let path = repo.join(REVIEW_COMMENTS_RECEIPT);
    let rendered = serde_json::to_string_pretty(receipt)
        .map_err(|err| format!("serialize review-comments receipt failed: {err}"))?;
    write_parented_file(&path, REVIEW_COMMENTS_RECEIPT, format!("{rendered}\n"))
}

fn review_comments_receipt(
    repo: &Path,
    options: &ReviewCommentsOptions,
    status: &str,
    error: Option<&str>,
) -> Value {
    let receipt_path = repo.join(REVIEW_COMMENTS_RECEIPT);
    let receipt_root = PathBuf::from(command_root_arg(repo, &options.root));
    let root_identity = canonical_root_identity(&receipt_root);
    let base_sha = resolve_revision_identity(&receipt_root, &options.base);
    let head_sha = resolve_revision_identity(&receipt_root, &options.head);
    let cache_identity = reusable_cache_identity(&root_identity, &base_sha, &head_sha);
    let existing_receipt = fs::read_to_string(&receipt_path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(|receipt| {
            receipt.get("base_sha").and_then(Value::as_str) == Some(base_sha.as_str())
                && receipt.get("head_sha").and_then(Value::as_str) == Some(head_sha.as_str())
        });
    let mut receipt = existing_receipt.unwrap_or_else(|| {
        serde_json::json!({
            "schema_version": "0.1",
            "root_identity": root_identity,
            "base_sha": base_sha,
            "head_sha": head_sha,
            "configured_timeout_ms": review_comments_timeout_ms().unwrap_or(120_000),
            "last_completed_phase": Value::Null,
            "active_phase": "review_comments_process",
            "completed_artifacts": [],
            "missing_artifacts": [REVIEW_COMMENTS_JSON, REVIEW_COMMENTS_MD],
            "reusable_cache_identity": cache_identity,
            "limitations": [],
            "non_claims": [],
            "atomic_write_status": "committed"
        })
    });
    if let Some(object) = receipt.as_object_mut() {
        object.insert("status".to_string(), Value::String(status.to_string()));
        if status == "complete" {
            object.insert(
                "last_completed_phase".to_string(),
                Value::String("artifact_io".to_string()),
            );
            object.insert("active_phase".to_string(), Value::Null);
            object.insert(
                "completed_artifacts".to_string(),
                serde_json::json!([REVIEW_COMMENTS_JSON, REVIEW_COMMENTS_MD]),
            );
            object.insert("missing_artifacts".to_string(), serde_json::json!([]));
        } else if status == "limited_timeout" {
            object.insert(
                "active_phase".to_string(),
                object
                    .get("active_phase")
                    .cloned()
                    .unwrap_or_else(|| Value::String("review_comments_process".to_string())),
            );
            object.insert(
                "limitations".to_string(),
                serde_json::json!([{
                    "category": "analysis_timeout",
                    "repair_route": "perf/review-comments-phase-budget"
                }]),
            );
            object.insert(
                "non_claims".to_string(),
                serde_json::json!(["no complete route inventory", "no all-clear"]),
            );
        } else if status == "failed" {
            object.insert(
                "limitations".to_string(),
                serde_json::json!([{
                    "category": "review_comments_failure",
                    "repair_route": "analysis/review-comments-error-diagnostics"
                }]),
            );
            object.insert(
                "non_claims".to_string(),
                serde_json::json!([format!(
                    "failure: {}",
                    first_line(error.unwrap_or("unknown failure"))
                )]),
            );
        }
    }
    receipt
}

fn canonical_root_identity(root: &Path) -> String {
    let normalized = root
        .canonicalize()
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/");
    normalized
        .strip_prefix("//?/")
        .unwrap_or(&normalized)
        .to_string()
}

fn reusable_cache_identity(root: &str, base: &str, head: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"ripr-review-comments\0");
    hasher.update(root.as_bytes());
    hasher.update([0]);
    hasher.update(base.as_bytes());
    hasher.update([0]);
    hasher.update(head.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
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

fn resolve_revision_identity(repo: &Path, revision: &str) -> String {
    let object = format!("{revision}^{{commit}}");
    run_git_output(repo, &["rev-parse", "--verify", object.as_str()])
        .map(|value| value.trim().to_string())
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| revision.to_string())
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
    fn validation_rejects_incomplete_complete_receipt() -> Result<(), String> {
        let packet = valid_packet(&options());
        let mut object = packet
            .as_object()
            .cloned()
            .ok_or_else(|| "packet should be an object".to_string())?;
        let mut receipt = object
            .get("run_receipt")
            .and_then(Value::as_object)
            .cloned()
            .ok_or_else(|| "receipt should be an object".to_string())?;
        receipt.insert("active_phase".to_string(), json!("artifact_io"));
        receipt.insert("completed_artifacts".to_string(), json!([]));
        object.insert("run_receipt".to_string(), Value::Object(receipt));

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
                .any(|violation| violation.contains("active_phase must be null"))
        );
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("completed_artifacts must not be empty"))
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
        let receipt = review_comments_receipt(
            &repo_root_for_display(),
            &options(),
            "failed",
            Some("synthetic failure"),
        );
        let packet = error_review_comments_packet(
            &repo_root_for_display(),
            &options(),
            "synthetic failure",
            &receipt,
        );
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
                .map_err(|err| format!("write success Markdown: {err}"))?;
            Ok::<(), ReviewCommentsRunError>(())
        })?;

        let packet = read_packet(&repo)?;
        assert_eq!(packet["status"], "advisory");
        assert_eq!(packet["run_receipt"]["status"], "complete");
        assert_eq!(packet["run_receipt"]["active_phase"], Value::Null);
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn write_wrapper_converts_producer_failure_to_error_packet() -> Result<(), String> {
        let (repo, options) = prepared_review_repo("ripr-review-comments-error")?;
        write_review_comments_with_runner(&repo, &options, |_repo, _options| {
            Err(ReviewCommentsRunError::from(
                "synthetic producer failure\nsecond line".to_string(),
            ))
        })?;

        let packet = read_packet(&repo)?;
        assert_eq!(packet["status"], "error");
        assert_eq!(
            packet["warnings"][0]["message"],
            "synthetic producer failure"
        );
        assert_eq!(packet["run_receipt"]["status"], "failed");
        assert!(repo.join(REVIEW_COMMENTS_RECEIPT).is_file());
        let markdown = fs::read_to_string(repo.join(REVIEW_COMMENTS_MD))
            .map_err(|err| format!("read error Markdown: {err}"))?;
        assert!(markdown.contains("No review guidance was generated."));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn write_wrapper_does_not_infer_timeout_from_error_text() -> Result<(), String> {
        let (repo, options) = prepared_review_repo("ripr-review-comments-text-timeout")?;
        write_review_comments_with_runner(&repo, &options, |_repo, _options| {
            Err("a non-timeout failure mentions timed out in its context".to_string())
        })?;

        let packet = read_packet(&repo)?;
        assert_eq!(packet["run_receipt"]["status"], "failed");
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn write_wrapper_preserves_typed_timeout_receipt() -> Result<(), String> {
        let (repo, options) = prepared_review_repo("ripr-review-comments-timeout")?;
        write_review_comments_with_runner(&repo, &options, |_repo, _options| {
            Err(ReviewCommentsRunError::timed_out(
                "ripr review-comments timed out after 1 seconds".to_string(),
            ))
        })?;

        let packet = read_packet(&repo)?;
        assert_eq!(packet["run_receipt"]["status"], "limited_timeout");
        assert_eq!(
            packet["run_receipt"]["limitations"][0]["category"],
            "analysis_timeout"
        );
        let standalone = fs::read_to_string(repo.join(REVIEW_COMMENTS_RECEIPT))
            .map_err(|err| format!("read timeout receipt: {err}"))?;
        let standalone: Value = serde_json::from_str(&standalone)
            .map_err(|err| format!("parse timeout receipt: {err}"))?;
        assert_eq!(standalone["status"], "limited_timeout");
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn write_wrapper_skips_producer_for_empty_diff() -> Result<(), String> {
        let (repo, mut options) = prepared_review_repo("ripr-review-comments-empty")?;
        options.base = "HEAD".to_string();
        options.head = "HEAD".to_string();

        write_review_comments_with_runner(&repo, &options, |_repo, _options| {
            Err(ReviewCommentsRunError::from(
                "producer should not run for an empty diff".to_string(),
            ))
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
        let receipt_root = PathBuf::from(command_root_arg(repo, &options.root));
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
            "limits_note": "Comments are capped and advisory; summary-only items never annotate.",
            "run_receipt": {
                "schema_version": "0.1",
                "status": "complete",
                "root_identity": normalize_path_text(&command_root_arg(repo, &options.root)),
                "base_sha": resolve_revision_identity(&receipt_root, &options.base),
                "head_sha": resolve_revision_identity(&receipt_root, &options.head),
                "configured_timeout_ms": 120000,
                "last_completed_phase": "artifact_io",
                "active_phase": null,
                "completed_artifacts": ["comments.json", "comments.md"],
                "missing_artifacts": [],
                "reusable_cache_identity": "review-comments|fixture",
                "limitations": [],
                "non_claims": ["static review guidance is advisory evidence only"],
                "atomic_write_status": "committed"
            }
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
