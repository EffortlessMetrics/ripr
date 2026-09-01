use super::{ensure_parent_dir, write_parented_file};
use crate::run::{
    capture_output_with_timeout, run_output_owned, run_output_owned_with_timeout,
    tool_build_timeout,
};
use crate::verification_contracts::validate_json_file_against_schema;
use ripr::review_input::stream_producer_check;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Read;
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
const DEFAULT_REVIEW_MODE: &str = "draft";

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
    check_output: Option<String>,
    check: bool,
}

impl Default for ReviewCommentsOptions {
    fn default() -> Self {
        Self {
            root: DEFAULT_ROOT.to_string(),
            base: DEFAULT_BASE.to_string(),
            head: DEFAULT_HEAD.to_string(),
            check_output: None,
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
            "--check-output" => {
                i += 1;
                options.check_output = Some(non_empty_arg(args, i, "--check-output")?.to_string());
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
        "usage: cargo xtask ripr-review-comments [--base <rev>] [--head <rev>] [--root <path>] [--check-output <path>] [--check]"
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
    let mut primary_failure: Option<(String, String)> = None;
    if !has_changed_paths(repo, &options.base, &options.head)? && options.check_output.is_none() {
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
                if let Err(secondary) = write_receipt_file(repo, &receipt) {
                    return Err(primary_and_secondary(status, &err.message, &secondary));
                }
                if let Err(secondary) =
                    write_error_review_comments(repo, options, &err.message, &receipt)
                {
                    return Err(primary_and_secondary(status, &err.message, &secondary));
                }
                primary_failure = Some((status.to_string(), err.message));
            }
        }
    }
    if let Err(secondary) = validate_review_comments(repo, options, true) {
        if let Some((status, message)) = primary_failure {
            let retained = append_secondary_diagnostic(repo, &secondary)
                .err()
                .map_or_else(String::new, |error| {
                    format!("; diagnostic retention failed: {error}")
                });
            return Err(format!(
                "{}{}",
                primary_and_secondary(&status, &message, &secondary),
                retained
            ));
        }
        return Err(secondary);
    }
    println!("Wrote {REVIEW_COMMENTS_JSON}");
    println!("Wrote {REVIEW_COMMENTS_MD}");
    Ok(())
}

fn primary_and_secondary(status: &str, primary: &str, secondary: &str) -> String {
    format!(
        "primary review-comments failure: {status}: {}; secondary packet diagnostic: {secondary}",
        first_line(primary)
    )
}

fn check_review_comments(repo: &Path, options: &ReviewCommentsOptions) -> Result<(), String> {
    verify_revision(repo, &options.base)?;
    verify_revision(repo, &options.head)?;
    validate_review_comments(repo, options, true)?;
    validate_review_health(repo)?;
    println!("Review comments contract ok: {REVIEW_COMMENTS_JSON}");
    Ok(())
}

fn validate_review_health(repo: &Path) -> Result<(), String> {
    let path = repo.join(REVIEW_COMMENTS_JSON);
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("missing or unreadable {REVIEW_COMMENTS_JSON}: {err}"))?;
    let packet: Value = serde_json::from_str(&text)
        .map_err(|err| format!("{REVIEW_COMMENTS_JSON} is not valid JSON: {err}"))?;
    let status = packet.get("status").and_then(Value::as_str);
    let receipt_status = packet
        .get("run_receipt")
        .and_then(|receipt| receipt.get("status"))
        .and_then(Value::as_str);
    if status == Some("advisory") && receipt_status == Some("complete") {
        return Ok(());
    }
    let primary = packet
        .pointer("/run_receipt/primary_failure")
        .and_then(|value| {
            let phase = value.get("phase")?.as_str()?;
            let category = value.get("category")?.as_str()?;
            Some(format!("; primary={phase}/{category}"))
        })
        .unwrap_or_default();
    Err(format!(
        "review guidance is not healthy: status={}, run_receipt.status={}{}",
        status.unwrap_or("missing"),
        receipt_status.unwrap_or("missing"),
        primary
    ))
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
    validate_check_output_against_packet(packet, repo, options, &mut violations);

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

fn validate_check_output_against_packet(
    packet: &Value,
    repo: &Path,
    options: &ReviewCommentsOptions,
    violations: &mut Vec<String>,
) {
    let Some(check_output) = &options.check_output else {
        return;
    };
    let path = Path::new(check_output);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo.join(path)
    };
    let check_sha256 = match digest_file(&path) {
        Ok(digest) => digest,
        Err(error) => {
            violations.push(format!(
                "--check-output {} is unreadable: {error}",
                path.display()
            ));
            return;
        }
    };
    let subject_path = path.with_extension("subject.json");
    let subject_text = match fs::read_to_string(&subject_path) {
        Ok(text) => text,
        Err(error) => {
            violations.push(format!(
                "--check-output subject receipt {} is unreadable: {error}",
                subject_path.display()
            ));
            return;
        }
    };
    let subject: Value = match serde_json::from_str(&subject_text) {
        Ok(value) => value,
        Err(error) => {
            violations.push(format!(
                "--check-output subject receipt {} is invalid JSON: {error}",
                subject_path.display()
            ));
            return;
        }
    };
    let receipt_root = PathBuf::from(command_root_arg(repo, &options.root));
    let expected_subject = [
        ("schema_version", "ripr.pr_check_subject.v1".to_string()),
        (
            "base_sha",
            resolve_revision_identity(&receipt_root, &options.base),
        ),
        (
            "head_sha",
            resolve_revision_identity(&receipt_root, &options.head),
        ),
        (
            "head_tree",
            resolve_tree_identity(&receipt_root, &options.head),
        ),
        ("check_sha256", check_sha256),
    ];
    for (field, expected) in expected_subject {
        if subject.get(field).and_then(Value::as_str) != Some(expected.as_str()) {
            violations.push(format!(
                "--check-output subject receipt {} {field} does not match the current subject or check bytes",
                subject_path.display()
            ));
        }
    }
    let producer_file = match File::open(&path) {
        Ok(file) => file,
        Err(error) => {
            violations.push(format!(
                "--check-output {} is unreadable: {error}",
                path.display()
            ));
            return;
        }
    };
    let producer = match stream_producer_check(producer_file, &receipt_root) {
        Ok(producer) => producer,
        Err(error) => {
            violations.push(format!(
                "--check-output {} is invalid: {error}",
                path.display()
            ));
            return;
        }
    };
    if producer.tool.trim().is_empty() {
        violations.push(format!(
            "--check-output {} is missing producer field tool",
            path.display()
        ));
    }
    if producer.tool != "ripr" {
        violations.push(format!(
            "--check-output {} producer tool must be ripr",
            path.display()
        ));
    }
    if producer.schema_version != "0.2" {
        violations.push(format!(
            "--check-output {} producer schema_version does not match expected \"0.2\"",
            path.display()
        ));
    }
    let packet_root = packet.get("root").and_then(Value::as_str).unwrap_or("");
    for (field, actual, expected) in [
        ("root", producer.root.as_str(), packet_root),
        (
            "base",
            producer.base.as_str(),
            packet.get("base").and_then(Value::as_str).unwrap_or(""),
        ),
        (
            "mode",
            producer.mode.as_str(),
            packet.get("mode").and_then(Value::as_str).unwrap_or(""),
        ),
    ] {
        if actual != expected {
            violations.push(format!(
                "--check-output {} producer {field} does not match expected {expected:?}",
                path.display()
            ));
        }
    }
    if !producer.summary.is_object() || !producer.analysis_outcome.is_object() {
        violations.push(format!(
            "--check-output {} requires producer summary, findings and analysis_outcome",
            path.display()
        ));
    }
    if producer.analysis_outcome
        != packet
            .get("analysis_outcome")
            .cloned()
            .unwrap_or(Value::Null)
    {
        violations.push(format!(
            "--check-output {} analysis_outcome does not match rendered review packet",
            path.display()
        ));
    }
}

fn digest_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
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
        "requested_mode",
        "analysis_identity",
        "phase_evidence",
        "primary_failure",
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
        "0.2",
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
        "phase_evidence",
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
    let mut ripr_args = vec![
        "review-comments".to_string(),
        "--root".to_string(),
        root_arg,
        "--base".to_string(),
        options.base.clone(),
        "--head".to_string(),
        options.head.clone(),
    ];
    if let Some(check_output) = &options.check_output {
        ripr_args.push("--check-output".to_string());
        ripr_args.push(check_output.clone());
    }
    ripr_args.extend([
        "--timeout-ms".to_string(),
        timeout_ms.to_string(),
        "--out".to_string(),
        out_arg,
    ]);
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
        let receipt_detail = fs::read_to_string(repo.join("target/ripr/review/run-receipt.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|receipt| receipt.get("primary_failure").cloned())
            .and_then(|failure| {
                failure
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
        let detail = receipt_detail
            .filter(|detail| detail != "ripr review-comments failed")
            .or_else(|| {
                output
                    .stderr
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .map(str::trim)
                    .map(str::to_owned)
            })
            .unwrap_or_default();
        Err(ReviewCommentsRunError::from(format!(
            "ripr review-comments failed{}\nstdout:\n{}\nstderr:\n{}",
            if detail.is_empty() {
                String::new()
            } else {
                format!(": child diagnostic: {}", detail.replace(['\r', '\n'], " "))
            },
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
    let mode = receipt
        .get("analysis_identity")
        .and_then(|identity| identity.get("mode"))
        .and_then(Value::as_str)
        .or_else(|| receipt.get("requested_mode").and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_default();
    let mut packet = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "status": "error",
        "root": normalize_path_text(&command_root_arg(repo, &options.root)),
        "base": options.base,
        "head": options.head,
        "mode": mode,
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
    });
    if let Some(analysis_outcome) = producer_analysis_outcome(repo, options) {
        packet["analysis_outcome"] = analysis_outcome;
        packet["limits_note"] = serde_json::json!(
            "Review guidance generation did not complete; no comments are emitted, but the producer analysis outcome is retained and bound to this error packet."
        );
    }
    packet
}

fn producer_analysis_outcome(repo: &Path, options: &ReviewCommentsOptions) -> Option<Value> {
    let check_output = options.check_output.as_deref()?;
    let path = Path::new(check_output);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo.join(path)
    };
    let text = fs::read_to_string(path).ok()?;
    let producer: Value = serde_json::from_str(&text).ok()?;
    producer
        .get("analysis_outcome")
        .filter(|outcome| !outcome.is_null())
        .cloned()
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

fn append_secondary_diagnostic(repo: &Path, diagnostic: &str) -> Result<(), String> {
    let receipt_path = repo.join(REVIEW_COMMENTS_RECEIPT);
    let mut receipt: Value = serde_json::from_str(
        &fs::read_to_string(&receipt_path)
            .map_err(|error| format!("read {REVIEW_COMMENTS_RECEIPT}: {error}"))?,
    )
    .map_err(|error| format!("parse {REVIEW_COMMENTS_RECEIPT}: {error}"))?;
    let diagnostics = receipt
        .pointer_mut("/primary_failure/secondary_diagnostics")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "primary failure has no secondary diagnostics array".to_string())?;
    if !diagnostics
        .iter()
        .any(|value| value.as_str() == Some(diagnostic))
    {
        diagnostics.push(Value::String(diagnostic.to_string()));
    }
    write_receipt_file(repo, &receipt)?;

    let packet_path = repo.join(REVIEW_COMMENTS_JSON);
    let mut packet: Value = serde_json::from_str(
        &fs::read_to_string(&packet_path)
            .map_err(|error| format!("read {REVIEW_COMMENTS_JSON}: {error}"))?,
    )
    .map_err(|error| format!("parse {REVIEW_COMMENTS_JSON}: {error}"))?;
    packet["run_receipt"] = receipt;
    let rendered = serde_json::to_string_pretty(&packet)
        .map_err(|error| format!("serialize {REVIEW_COMMENTS_JSON}: {error}"))?;
    write_parented_file(&packet_path, REVIEW_COMMENTS_JSON, format!("{rendered}\n"))
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
            receipt.get("schema_version").and_then(Value::as_str) == Some("0.2")
                && receipt.get("base_sha").and_then(Value::as_str) == Some(base_sha.as_str())
                && receipt.get("head_sha").and_then(Value::as_str) == Some(head_sha.as_str())
        });
    let mut receipt = existing_receipt.unwrap_or_else(|| {
        serde_json::json!({
            "schema_version": "0.2",
            "root_identity": root_identity,
            "base_sha": base_sha,
            "head_sha": head_sha,
            "requested_mode": requested_review_mode(repo, options),
            "analysis_identity": Value::Null,
            "phase_evidence": [],
            "primary_failure": Value::Null,
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
        if !object.contains_key("requested_mode") {
            object.insert(
                "requested_mode".to_string(),
                Value::String(requested_review_mode(repo, options)),
            );
        }
        object.insert("status".to_string(), Value::String(status.to_string()));
        if status == "complete" {
            if object
                .get("phase_evidence")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
            {
                object.insert(
                    "phase_evidence".to_string(),
                    serde_json::json!([{
                        "phase": "canonical_diff",
                        "duration_ms": 0,
                        "reused": false,
                        "subject_count": 0,
                        "stop_reason": "no_changed_paths",
                        "input_identity_digest": null
                    }]),
                );
            }
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
            let phase = object
                .get("active_phase")
                .and_then(Value::as_str)
                .filter(|phase| !phase.trim().is_empty())
                .unwrap_or("review_comments_process")
                .to_string();
            object.insert(
                "primary_failure".to_string(),
                serde_json::json!({
                    "phase": phase,
                    "category": "limited_timeout",
                    "message": first_line(error.unwrap_or("review-comments timed out")),
                    "secondary_diagnostics": []
                }),
            );
            object.insert("active_phase".to_string(), Value::String(phase));
            object.insert(
                "missing_artifacts".to_string(),
                serde_json::json!([REVIEW_COMMENTS_JSON, REVIEW_COMMENTS_MD]),
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
            let phase = object
                .get("active_phase")
                .and_then(Value::as_str)
                .filter(|phase| !phase.trim().is_empty())
                .unwrap_or("review_comments_process")
                .to_string();
            object.insert("active_phase".to_string(), Value::String(phase.clone()));
            object.insert(
                "missing_artifacts".to_string(),
                serde_json::json!([REVIEW_COMMENTS_JSON, REVIEW_COMMENTS_MD]),
            );
            object.insert(
                "primary_failure".to_string(),
                serde_json::json!({
                    "phase": phase,
                    "category": "instrument_failure",
                    "message": first_line(error.unwrap_or("review-comments failed")),
                    "secondary_diagnostics": []
                }),
            );
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

fn requested_review_mode(repo: &Path, options: &ReviewCommentsOptions) -> String {
    let root = PathBuf::from(command_root_arg(repo, &options.root));
    let configured = root.ancestors().find_map(|ancestor| {
        let path = ancestor.join("ripr.toml");
        let text = fs::read_to_string(path).ok()?;
        let value: toml::Value = toml::from_str(&text).ok()?;
        value
            .get("analysis")?
            .get("mode")?
            .as_str()
            .map(str::to_string)
    });
    match configured.as_deref() {
        Some(mode @ ("instant" | "draft" | "fast" | "deep" | "ready")) => mode.to_string(),
        _ => DEFAULT_REVIEW_MODE.to_string(),
    }
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

fn resolve_tree_identity(repo: &Path, revision: &str) -> String {
    let object = format!("{revision}^{{tree}}");
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
            check_output: None,
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
    fn check_output_missing_artifact_is_not_accepted() {
        let mut options = options();
        options.check_output = Some("target/missing-check-output.json".to_string());
        let violations = validate_packet_value(
            &valid_packet(&options),
            &repo_root_for_display(),
            &options,
            false,
            Path::new(REVIEW_COMMENTS_MD),
        );
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("is unreadable")),
            "{violations:#?}"
        );
    }

    #[test]
    fn check_output_outcome_mismatch_is_not_accepted() -> Result<(), String> {
        let repo = temp_repo("ripr-review-comments-check-mismatch")?;
        let mut options = options();
        options.check_output = Some("target/check-output.json".to_string());
        let packet = valid_packet_for_repo(&repo, &options);
        let producer = json!({
            "schema_version": "0.2",
            "tool": "ripr",
            "mode": "draft",
            "root": repo.display().to_string(),
            "base": options.base.clone(),
            "summary": {},
            "findings": [],
            "analysis_outcome": {"analysis_complete": true}
        });
        fs::create_dir_all(repo.join("target")).map_err(|err| format!("create target: {err}"))?;
        write_check_output(&repo, &options, &producer)?;
        let violations = validate_packet_value(
            &packet,
            &repo,
            &options,
            false,
            Path::new(REVIEW_COMMENTS_MD),
        );
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("does not match rendered review packet")),
            "{violations:#?}"
        );
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn check_output_identity_mismatches_are_not_accepted() -> Result<(), String> {
        let repo = temp_repo("ripr-review-comments-check-identity")?;
        let mut options = options();
        options.check_output = Some("target/check-output.json".to_string());
        let mut packet = valid_packet_for_repo(&repo, &options);
        packet["analysis_outcome"] = json!({"analysis_complete": true});
        let producer = json!({
            "schema_version": "0.2",
            "tool": "ripr",
            "mode": packet["mode"].clone(),
            "root": packet["root"].clone(),
            "base": packet["base"].clone(),
            "summary": {},
            "findings": [],
            "analysis_outcome": packet["analysis_outcome"].clone()
        });
        fs::create_dir_all(repo.join("target")).map_err(|err| format!("create target: {err}"))?;
        for (field, wrong) in [
            ("schema_version", "9.9"),
            ("mode", "ready"),
            ("root", "wrong-root"),
            ("base", "wrong-base"),
        ] {
            let mut mutated = producer.clone();
            mutated[field] = json!(wrong);
            write_check_output(&repo, &options, &mutated)?;
            let violations = validate_packet_value(
                &packet,
                &repo,
                &options,
                false,
                Path::new(REVIEW_COMMENTS_MD),
            );
            assert!(
                violations
                    .iter()
                    .any(|violation| violation.contains(&format!("producer {field}"))),
                "{field}: {violations:#?}"
            );
        }
        write_check_output(&repo, &options, &producer)?;
        let subject_path = repo.join("target/check-output.subject.json");
        for field in ["head_sha", "head_tree", "check_sha256"] {
            let mut subject: Value = serde_json::from_slice(
                &fs::read(&subject_path).map_err(|err| format!("read subject: {err}"))?,
            )
            .map_err(|err| format!("parse subject: {err}"))?;
            subject[field] = json!("wrong");
            fs::write(
                &subject_path,
                serde_json::to_vec(&subject).map_err(|err| format!("serialize subject: {err}"))?,
            )
            .map_err(|err| format!("write subject mutation: {err}"))?;
            let violations = validate_packet_value(
                &packet,
                &repo,
                &options,
                false,
                Path::new(REVIEW_COMMENTS_MD),
            );
            assert!(
                violations.iter().any(|violation| {
                    violation.contains("subject receipt") && violation.contains(field)
                }),
                "{field}: {violations:#?}"
            );
            write_check_output(&repo, &options, &producer)?;
        }
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn error_packet_is_contract_valid() {
        let mut receipt = review_comments_receipt(
            &repo_root_for_display(),
            &options(),
            "failed",
            Some("synthetic failure"),
        );
        receipt["requested_mode"] = json!("fast");
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
    fn timeout_packet_retains_resolved_draft_and_fast_identity() {
        for mode in ["draft", "fast"] {
            let receipt = json!({
                "requested_mode": mode,
                "analysis_identity": { "mode": mode },
                "primary_failure": {
                    "phase": "canonical_analysis",
                    "category": "limited_timeout",
                    "message": "timed out",
                    "secondary_diagnostics": []
                }
            });
            let packet = error_review_comments_packet(
                &repo_root_for_display(),
                &options(),
                "timed out",
                &receipt,
            );
            assert_eq!(packet["mode"], mode);
            assert_eq!(
                packet["run_receipt"]["primary_failure"]["phase"],
                "canonical_analysis"
            );
            assert_eq!(
                packet["run_receipt"]["primary_failure"]["category"],
                "limited_timeout"
            );
        }
    }

    #[test]
    fn error_packet_retains_check_output_analysis_outcome() -> Result<(), String> {
        let repo = temp_repo("ripr-review-comments-error-outcome")?;
        let mut options = options();
        options.check_output = Some("target/check-output.json".to_string());
        let producer = json!({
            "schema_version": "0.2",
            "tool": "ripr",
            "mode": "draft",
            "root": normalize_path_text(&command_root_arg(&repo, &options.root)),
            "base": options.base.clone(),
            "summary": {},
            "findings": [],
            "analysis_outcome": {
                "analysis_complete": true,
                "outcome": {
                    "schema_version": "0.1",
                    "kind": "complete_no_findings",
                    "identity": {
                        "repository_identity": null,
                        "root_identity": null,
                        "config_identity": null,
                        "base_revision": options.base.clone(),
                        "input_identity": null,
                        "snapshot_identity": null
                    },
                    "counts": {
                        "changed_file_count": 0,
                        "changed_line_count": 0,
                        "candidate_line_count": 0,
                        "probe_count": 0,
                        "finding_count": 0
                    },
                    "limitations": [],
                    "claim_boundary": "Static analysis outcome only; no correctness, test-adequacy, runtime-execution, or merge-readiness claim."
                }
            }
        });
        fs::create_dir_all(repo.join("target")).map_err(|err| format!("create target: {err}"))?;
        write_check_output(&repo, &options, &producer)?;

        let receipt = review_comments_receipt(
            &repo,
            &options,
            "limited_timeout",
            Some("ripr review-comments timed out after 60 seconds"),
        );
        let packet = error_review_comments_packet(
            &repo,
            &options,
            "ripr review-comments timed out after 60 seconds",
            &receipt,
        );
        assert_eq!(packet["status"], "error");
        assert_eq!(packet["analysis_outcome"], producer["analysis_outcome"]);
        let violations = validate_packet_value(
            &packet,
            &repo,
            &options,
            false,
            Path::new(REVIEW_COMMENTS_MD),
        );
        assert!(violations.is_empty(), "{violations:#?}");

        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
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
        write_review_comments_with_runner(&repo, &options, |repo, options| {
            write_active_test_receipt(repo, options, "draft", "input_validation")?;
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
        let health_error = check_review_comments(&repo, &options)
            .err()
            .ok_or_else(|| "status:error review guidance must not pass --check".to_string())?;
        assert!(health_error.contains("review guidance is not healthy"));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn write_wrapper_does_not_infer_timeout_from_error_text() -> Result<(), String> {
        let (repo, options) = prepared_review_repo("ripr-review-comments-text-timeout")?;
        write_review_comments_with_runner(&repo, &options, |repo, options| {
            write_active_test_receipt(repo, options, "draft", "input_validation")?;
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
        write_review_comments_with_runner(&repo, &options, |repo, options| {
            write_active_test_receipt(repo, options, "draft", "canonical_analysis")?;
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
    fn secondary_packet_defect_does_not_replace_primary_timeout() -> Result<(), String> {
        let (repo, options) = prepared_review_repo("ripr-review-comments-secondary")?;
        let error = write_review_comments_with_runner(&repo, &options, |repo, options| {
            write_active_test_receipt(repo, options, "invalid-mode", "canonical_analysis")?;
            Err(ReviewCommentsRunError::timed_out(
                "ripr review-comments timed out after 1 seconds".to_string(),
            ))
        })
        .err()
        .ok_or_else(|| "secondary packet defect must remain non-green".to_string())?;
        assert!(error.starts_with("primary review-comments failure: limited_timeout"));
        assert!(error.contains("secondary packet diagnostic"));
        let receipt: Value = serde_json::from_str(
            &fs::read_to_string(repo.join(REVIEW_COMMENTS_RECEIPT))
                .map_err(|err| format!("read timeout receipt: {err}"))?,
        )
        .map_err(|err| format!("parse timeout receipt: {err}"))?;
        assert_eq!(receipt["primary_failure"]["phase"], "canonical_analysis");
        assert_eq!(receipt["primary_failure"]["category"], "limited_timeout");
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
    fn write_wrapper_runs_producer_for_empty_diff_with_check_output() -> Result<(), String> {
        let (repo, mut options) = prepared_review_repo("ripr-review-comments-empty-check")?;
        options.base = "HEAD".to_string();
        options.head = "HEAD".to_string();
        options.check_output = Some("target/check-output.json".to_string());
        let mut producer_called = false;

        let result = write_review_comments_with_runner(&repo, &options, |_repo, _options| {
            producer_called = true;
            Err(ReviewCommentsRunError::from(
                "producer received explicit check output for an empty diff".to_string(),
            ))
        });

        assert!(producer_called);
        let error = result
            .err()
            .ok_or_else(|| "missing check-output must fail closed".to_string())?;
        assert!(error.contains("--check-output"), "{error}");
        let packet = read_packet(&repo)?;
        assert_eq!(packet["status"], "error");
        assert_eq!(packet["run_receipt"]["status"], "failed");
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
            "mode": "draft",
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
                "schema_version": "0.2",
                "status": "complete",
                "root_identity": normalize_path_text(&command_root_arg(repo, &options.root)),
                "base_sha": resolve_revision_identity(&receipt_root, &options.base),
                "head_sha": resolve_revision_identity(&receipt_root, &options.head),
                "requested_mode": "draft",
                "analysis_identity": null,
                "phase_evidence": [{
                    "phase": "artifact_io",
                    "duration_ms": 0,
                    "reused": false,
                    "subject_count": null,
                    "stop_reason": "complete",
                    "input_identity_digest": null
                }],
                "primary_failure": null,
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
                check_output: None,
                check: false,
            },
        ))
    }

    fn write_check_output(
        repo: &Path,
        options: &ReviewCommentsOptions,
        producer: &Value,
    ) -> Result<(), String> {
        let path = repo.join("target/check-output.json");
        let bytes = serde_json::to_vec(producer).map_err(|err| format!("serialize: {err}"))?;
        fs::create_dir_all(repo.join("target")).map_err(|err| format!("create target: {err}"))?;
        fs::write(&path, &bytes).map_err(|err| format!("write producer: {err}"))?;
        let subject = json!({
            "schema_version": "ripr.pr_check_subject.v1",
            "base_sha": resolve_revision_identity(repo, &options.base),
            "head_sha": resolve_revision_identity(repo, &options.head),
            "head_tree": resolve_tree_identity(repo, &options.head),
            "check_sha256": format!("sha256:{:x}", Sha256::digest(&bytes)),
        });
        fs::write(
            path.with_extension("subject.json"),
            serde_json::to_vec(&subject).map_err(|err| format!("serialize subject: {err}"))?,
        )
        .map_err(|err| format!("write check subject receipt: {err}"))
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

    fn write_active_test_receipt(
        repo: &Path,
        options: &ReviewCommentsOptions,
        mode: &str,
        phase: &str,
    ) -> Result<(), String> {
        let mut receipt = review_comments_receipt(
            repo,
            options,
            "failed",
            Some("synthetic pre-output failure"),
        );
        receipt["status"] = json!("in_progress");
        receipt["requested_mode"] = json!(mode);
        receipt["active_phase"] = json!(phase);
        if let Some(object) = receipt.as_object_mut() {
            object.remove("primary_failure");
        }
        write_receipt_file(repo, &receipt)
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
