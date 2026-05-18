use super::write_parented_file;
use crate::run::{capture_output_with_timeout, run_output_owned};
use serde_json::{Map, Value, json};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEFAULT_ROOT: &str = ".";
const DEFAULT_BASE: &str = "origin/main";
const DEFAULT_HEAD: &str = "HEAD";
const PR_EVIDENCE_JSON: &str = "target/ripr/pr/repo-exposure.json";
const PR_EVIDENCE_MD: &str = "target/ripr/pr/repo-exposure.md";
const PR_DIFF: &str = "target/ripr/pr/pr.diff";
const DEFAULT_TOOL_TIMEOUT_SECS: u64 = 120;
const PR_EVIDENCE_TIMEOUT_ENV: &str = "RIPR_PR_EVIDENCE_TIMEOUT_SECS";

#[derive(Clone, Debug, Eq, PartialEq)]
struct PrEvidenceOptions {
    root: String,
    base: String,
    head: String,
    check: bool,
}

impl Default for PrEvidenceOptions {
    fn default() -> Self {
        Self {
            root: DEFAULT_ROOT.to_string(),
            base: DEFAULT_BASE.to_string(),
            head: DEFAULT_HEAD.to_string(),
            check: false,
        }
    }
}

pub(crate) fn ripr_pr(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }
    let options = parse_options(args)?;
    let repo = repo_root()?;
    if options.check {
        check_pr_evidence(&repo, &options)
    } else {
        write_pr_evidence(&repo, &options)
    }
}

fn parse_options(args: &[String]) -> Result<PrEvidenceOptions, String> {
    let mut options = PrEvidenceOptions::default();
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
            other => return Err(format!("unknown ripr-pr argument {other:?}")),
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
        return Err(format!("ripr-pr {flag} requires a non-empty value"));
    }
    Ok(value)
}

fn print_help() {
    println!("usage: cargo xtask ripr-pr [--base <rev>] [--head <rev>] [--root <path>] [--check]");
}

fn write_pr_evidence(repo: &Path, options: &PrEvidenceOptions) -> Result<(), String> {
    write_pr_evidence_with_runner(repo, options, run_ripr_check)
}

fn write_pr_evidence_with_runner(
    repo: &Path,
    options: &PrEvidenceOptions,
    run_check: impl FnOnce(&Path, &PrEvidenceOptions) -> Result<String, String>,
) -> Result<(), String> {
    verify_revision(repo, &options.base)?;
    verify_revision(repo, &options.head)?;
    let changed_files = changed_files(repo, options)?;
    write_diff(repo, options)?;
    match run_check(repo, options) {
        Ok(check_json) => {
            match write_pr_evidence_packet(repo, options, &changed_files, &check_json) {
                Ok(()) => Ok(()),
                Err(err) => write_pr_evidence_error_packet(
                    repo,
                    options,
                    &changed_files,
                    &format!("RIPR check output could not be converted into PR evidence: {err}"),
                ),
            }
        }
        Err(err) => write_pr_evidence_error_packet(repo, options, &changed_files, &err),
    }
}

#[cfg(test)]
fn write_pr_evidence_from_check_json(
    repo: &Path,
    options: &PrEvidenceOptions,
    check_json: &str,
) -> Result<(), String> {
    verify_revision(repo, &options.base)?;
    verify_revision(repo, &options.head)?;

    let changed_files = changed_files(repo, options)?;
    write_diff(repo, options)?;
    write_pr_evidence_packet(repo, options, &changed_files, check_json)
}

fn write_pr_evidence_packet(
    repo: &Path,
    options: &PrEvidenceOptions,
    changed_files: &[String],
    check_json: &str,
) -> Result<(), String> {
    let check_value: Value = serde_json::from_str(check_json)
        .map_err(|err| format!("ripr check output was not valid JSON: {err}"))?;
    let packet = pr_evidence_packet(options, changed_files, &check_value);
    let json_text = serde_json::to_string_pretty(&packet)
        .map_err(|err| format!("serialize PR evidence packet: {err}"))?;
    let markdown = render_pr_evidence_markdown(&packet);

    write_parented_file(
        &repo.join(PR_EVIDENCE_JSON),
        PR_EVIDENCE_JSON,
        format!("{json_text}\n"),
    )?;
    write_parented_file(&repo.join(PR_EVIDENCE_MD), PR_EVIDENCE_MD, markdown)?;

    let violations = validate_packet_value(&packet, options, changed_files.len(), true);
    if !violations.is_empty() {
        return Err(format!(
            "generated PR evidence failed contract validation:\n{}",
            violations
                .iter()
                .map(|violation| format!("- {violation}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    println!("Wrote {PR_EVIDENCE_JSON}");
    println!("Wrote {PR_EVIDENCE_MD}");
    Ok(())
}

fn write_pr_evidence_error_packet(
    repo: &Path,
    options: &PrEvidenceOptions,
    changed_files: &[String],
    error: &str,
) -> Result<(), String> {
    let packet = pr_evidence_error_packet(options, changed_files, error);
    let json_text = serde_json::to_string_pretty(&packet)
        .map_err(|err| format!("serialize PR evidence error packet: {err}"))?;
    let markdown = render_pr_evidence_markdown(&packet);

    write_parented_file(
        &repo.join(PR_EVIDENCE_JSON),
        PR_EVIDENCE_JSON,
        format!("{json_text}\n"),
    )?;
    write_parented_file(&repo.join(PR_EVIDENCE_MD), PR_EVIDENCE_MD, markdown)?;

    let violations = validate_packet_value(&packet, options, changed_files.len(), true);
    if !violations.is_empty() {
        return Err(format!(
            "generated PR evidence error packet failed contract validation:\n{}",
            violations
                .iter()
                .map(|violation| format!("- {violation}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    println!("Wrote {PR_EVIDENCE_JSON}");
    println!("Wrote {PR_EVIDENCE_MD}");
    Ok(())
}

fn check_pr_evidence(repo: &Path, options: &PrEvidenceOptions) -> Result<(), String> {
    verify_revision(repo, &options.base)?;
    verify_revision(repo, &options.head)?;
    let changed_files = changed_files(repo, options)?;
    let json_path = repo.join(PR_EVIDENCE_JSON);
    let markdown_path = repo.join(PR_EVIDENCE_MD);
    let text = fs::read_to_string(&json_path)
        .map_err(|err| format!("missing or unreadable {PR_EVIDENCE_JSON}: {err}"))?;
    let packet: Value = serde_json::from_str(&text)
        .map_err(|err| format!("{PR_EVIDENCE_JSON} is not valid JSON: {err}"))?;
    let violations = validate_packet_value(
        &packet,
        options,
        changed_files.len(),
        markdown_path.exists(),
    );
    if violations.is_empty() {
        println!("PR evidence contract ok: {PR_EVIDENCE_JSON}");
        return Ok(());
    }

    Err(format!(
        "PR evidence contract violations:\n{}",
        violations
            .iter()
            .map(|violation| format!("- {violation}"))
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

fn verify_revision(repo: &Path, rev: &str) -> Result<(), String> {
    let commit = format!("{rev}^{{commit}}");
    run_git_output(repo, &["rev-parse", "--verify", commit.as_str()])
        .map(|_| ())
        .map_err(|err| format!("bad base/head revision {rev:?}: {err}"))
}

fn changed_files(repo: &Path, options: &PrEvidenceOptions) -> Result<Vec<String>, String> {
    let range = format!("{}...{}", options.base, options.head);
    let output = run_git_output(
        repo,
        &["diff", "--name-only", "--diff-filter=ACMR", range.as_str()],
    )?;
    Ok(output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

fn write_diff(repo: &Path, options: &PrEvidenceOptions) -> Result<(), String> {
    let out = repo.join(PR_DIFF);
    let range = format!("{}...{}", options.base, options.head);
    let diff = run_git_output(repo, &["diff", "--binary", "--no-ext-diff", range.as_str()])?;
    write_parented_file(&out, PR_DIFF, diff)
}

fn run_ripr_check(repo: &Path, options: &PrEvidenceOptions) -> Result<String, String> {
    let diff_path = repo.join(PR_DIFF);
    let diff_arg = diff_path.display().to_string();
    let root_arg = command_root_arg(repo, &options.root);
    let ripr_args = vec![
        "check".to_string(),
        "--root".to_string(),
        root_arg,
        "--diff".to_string(),
        diff_arg,
        "--format".to_string(),
        "json".to_string(),
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
    let timeout = Duration::from_secs(pr_evidence_timeout_secs()?);
    let output = capture_output_with_timeout(
        &binary,
        &ripr_args,
        &[],
        timeout,
        "ripr check for PR evidence",
    )?;
    if output.timed_out {
        return Err(format!(
            "ripr check for PR evidence timed out after {} seconds; retry command: {}",
            timeout.as_secs(),
            pr_evidence_retry_command(options)
        ));
    }
    if output.status.is_some_and(|status| status.success()) {
        Ok(output.stdout)
    } else {
        Err(format!(
            "ripr check for PR evidence failed\nstdout:\n{}\nstderr:\n{}",
            output.stdout.trim(),
            output.stderr.trim()
        ))
    }
}

fn pr_evidence_timeout_secs() -> Result<u64, String> {
    match env::var(PR_EVIDENCE_TIMEOUT_ENV) {
        Ok(value) => value
            .trim()
            .parse::<u64>()
            .map_err(|err| format!("{PR_EVIDENCE_TIMEOUT_ENV} must be a positive integer: {err}")),
        Err(_) => Ok(DEFAULT_TOOL_TIMEOUT_SECS),
    }
    .and_then(|value| {
        if value > 0 {
            Ok(value)
        } else {
            Err(format!(
                "{PR_EVIDENCE_TIMEOUT_ENV} must be a positive integer"
            ))
        }
    })
}

fn pr_evidence_retry_command(options: &PrEvidenceOptions) -> String {
    format!(
        "cargo xtask ripr-pr --base {} --head {} --root {}",
        options.base, options.head, options.root
    )
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

fn command_root_arg(repo: &Path, root: &str) -> String {
    let root_path = Path::new(root);
    if root_path.is_absolute() {
        return root.to_string();
    }
    repo.join(root_path).display().to_string()
}

fn run_git_output(repo: &Path, args: &[&str]) -> Result<String, String> {
    let mut git_args = vec!["-C".to_string(), repo.display().to_string()];
    git_args.extend(args.iter().map(|arg| (*arg).to_string()));
    run_output_owned("git", &git_args)
}

fn pr_evidence_packet(
    options: &PrEvidenceOptions,
    changed_files: &[String],
    check_value: &Value,
) -> Value {
    let check_summary = check_value.get("summary").and_then(Value::as_object);
    let weakly_exposed = count_field(check_summary, "weakly_exposed");
    let reachable_unrevealed = count_field(check_summary, "reachable_unrevealed");
    let no_static_path = count_field(check_summary, "no_static_path");
    let severe_gaps = weakly_exposed + reachable_unrevealed + no_static_path;
    let ripr_severe_gap = severe_gaps > 0;
    let mut warnings = Vec::new();
    if check_summary.is_none() {
        warnings.push(json!({
            "kind": "invalid_json",
            "message": "RIPR check output did not include a summary object.",
            "path": null
        }));
    }

    let routing_reason = if ripr_severe_gap {
        json!("ripr severe gap")
    } else {
        Value::Null
    };

    json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "kind": "pr_evidence",
        "scope": "diff",
        "status": if warnings.is_empty() { "advisory" } else { "incomplete" },
        "root": options.root.as_str(),
        "base": options.base.as_str(),
        "head": options.head.as_str(),
        "summary": {
            "changed_files": changed_files.len(),
            "comments": 0,
            "summary_only": 0,
            "suppressed": 0,
            "weakly_exposed": weakly_exposed,
            "reachable_unrevealed": reachable_unrevealed,
            "no_static_path": no_static_path,
            "severe_gaps": severe_gaps,
            "requires_targeted_mutation": ripr_severe_gap,
            "ripr_severe_gap": ripr_severe_gap,
            "routing_reason": routing_reason
        },
        "artifacts": [
            {
                "label": "PR evidence JSON",
                "path": PR_EVIDENCE_JSON,
                "kind": "json",
                "scope": "diff",
                "available": true,
                "required": true
            },
            {
                "label": "PR evidence Markdown",
                "path": PR_EVIDENCE_MD,
                "kind": "markdown",
                "scope": "diff",
                "available": true
            },
            {
                "label": "Analyzed PR diff",
                "path": PR_DIFF,
                "kind": "other",
                "scope": "diff",
                "available": true
            }
        ],
        "warnings": warnings,
        "advisory_limits": [
            "RIPR evidence is static and advisory by default.",
            "This packet does not post review comments or execute mutation.",
            "Public badge state must not be derived from this diff-scoped packet."
        ]
    })
}

fn pr_evidence_error_packet(
    options: &PrEvidenceOptions,
    changed_files: &[String],
    error: &str,
) -> Value {
    json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "kind": "pr_evidence",
        "scope": "diff",
        "status": "error",
        "root": options.root.as_str(),
        "base": options.base.as_str(),
        "head": options.head.as_str(),
        "summary": {
            "changed_files": changed_files.len(),
            "comments": 0,
            "summary_only": 0,
            "suppressed": 0,
            "weakly_exposed": 0,
            "reachable_unrevealed": 0,
            "no_static_path": 0,
            "severe_gaps": 0,
            "requires_targeted_mutation": false,
            "ripr_severe_gap": false,
            "routing_reason": null
        },
        "artifacts": [
            {
                "label": "PR evidence JSON",
                "path": PR_EVIDENCE_JSON,
                "kind": "json",
                "scope": "diff",
                "available": true,
                "required": true
            },
            {
                "label": "PR evidence Markdown",
                "path": PR_EVIDENCE_MD,
                "kind": "markdown",
                "scope": "diff",
                "available": true
            },
            {
                "label": "Analyzed PR diff",
                "path": PR_DIFF,
                "kind": "other",
                "scope": "diff",
                "available": true
            }
        ],
        "warnings": [
            {
                "kind": "tool_error",
                "message": first_line(error),
                "path": null
            }
        ],
        "advisory_limits": [
            "RIPR evidence is static and advisory by default.",
            "This packet does not post review comments or execute mutation.",
            "Public badge state must not be derived from this diff-scoped packet.",
            "PR evidence generation did not complete, so this packet must not be treated as proof of no gaps."
        ]
    })
}

fn first_line(text: &str) -> String {
    text.lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .unwrap_or("RIPR PR evidence generation did not complete.")
        .to_string()
}

fn count_field(summary: Option<&Map<String, Value>>, key: &str) -> usize {
    summary
        .and_then(|summary| summary.get(key))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
}

fn validate_packet_value(
    packet: &Value,
    options: &PrEvidenceOptions,
    expected_changed_files: usize,
    markdown_exists: bool,
) -> Vec<String> {
    let mut violations = Vec::new();
    expect_string(packet, "schema_version", "0.1", &mut violations);
    expect_string(packet, "tool", "ripr", &mut violations);
    expect_string(packet, "kind", "pr_evidence", &mut violations);
    expect_string(packet, "scope", "diff", &mut violations);
    expect_string(packet, "root", options.root.as_str(), &mut violations);
    expect_string(packet, "base", options.base.as_str(), &mut violations);
    expect_string(packet, "head", options.head.as_str(), &mut violations);

    match packet.get("status").and_then(Value::as_str) {
        Some("advisory" | "incomplete" | "error") => {}
        Some(other) => violations.push(format!("status {other:?} is not contract-valid")),
        None => violations.push("status is missing or not a string".to_string()),
    }

    let summary = packet.get("summary").and_then(Value::as_object);
    let Some(summary) = summary else {
        violations.push("summary is missing or not an object".to_string());
        return violations;
    };
    for key in [
        "comments",
        "summary_only",
        "suppressed",
        "weakly_exposed",
        "reachable_unrevealed",
        "no_static_path",
        "severe_gaps",
    ] {
        if !summary.get(key).is_some_and(Value::is_u64) {
            violations.push(format!(
                "summary.{key} is missing or not a non-negative integer"
            ));
        }
    }
    match summary.get("changed_files").and_then(Value::as_u64) {
        Some(value) if value == expected_changed_files as u64 => {}
        Some(value) => violations.push(format!(
            "summary.changed_files is {value}, expected {expected_changed_files}"
        )),
        None => violations
            .push("summary.changed_files is missing or not a non-negative integer".to_string()),
    }
    for key in ["requires_targeted_mutation", "ripr_severe_gap"] {
        if !summary.get(key).is_some_and(Value::is_boolean) {
            violations.push(format!("summary.{key} is missing or not a boolean"));
        }
    }
    if !(summary.get("routing_reason").is_some_and(Value::is_string)
        || summary.get("routing_reason").is_some_and(Value::is_null))
    {
        violations.push("summary.routing_reason is missing or not string/null".to_string());
    }

    validate_artifacts(packet, &mut violations);
    if !markdown_exists {
        violations.push(format!("{PR_EVIDENCE_MD} is missing"));
    }
    if !packet.get("warnings").is_some_and(Value::is_array) {
        violations.push("warnings is missing or not an array".to_string());
    }
    match packet.get("advisory_limits").and_then(Value::as_array) {
        Some(limits) if !limits.is_empty() => {}
        Some(_) => violations.push("advisory_limits is empty".to_string()),
        None => violations.push("advisory_limits is missing or not an array".to_string()),
    }
    violations
}

fn expect_string(packet: &Value, key: &str, expected: &str, violations: &mut Vec<String>) {
    match packet.get(key).and_then(Value::as_str) {
        Some(actual) if actual == expected => {}
        Some(actual) => violations.push(format!("{key} is {actual:?}, expected {expected:?}")),
        None => violations.push(format!("{key} is missing or not a string")),
    }
}

fn validate_artifacts(packet: &Value, violations: &mut Vec<String>) {
    let Some(artifacts) = packet.get("artifacts").and_then(Value::as_array) else {
        violations.push("artifacts is missing or not an array".to_string());
        return;
    };
    for required_path in [PR_EVIDENCE_JSON, PR_EVIDENCE_MD] {
        if !artifacts.iter().any(|artifact| {
            artifact.get("path").and_then(Value::as_str) == Some(required_path)
                && artifact.get("scope").and_then(Value::as_str) == Some("diff")
                && artifact
                    .get("available")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
        }) {
            violations.push(format!(
                "artifacts[] is missing available diff artifact {required_path}"
            ));
        }
    }
}

fn render_pr_evidence_markdown(packet: &Value) -> String {
    let summary = packet.get("summary").and_then(Value::as_object);
    let changed_files = count_field(summary, "changed_files");
    let comments = count_field(summary, "comments");
    let summary_only = count_field(summary, "summary_only");
    let suppressed = count_field(summary, "suppressed");
    let weakly_exposed = count_field(summary, "weakly_exposed");
    let reachable_unrevealed = count_field(summary, "reachable_unrevealed");
    let no_static_path = count_field(summary, "no_static_path");
    let severe_gaps = count_field(summary, "severe_gaps");
    let requires_targeted_mutation = bool_field(summary, "requires_targeted_mutation");
    let routing_reason = summary
        .and_then(|summary| summary.get("routing_reason"))
        .and_then(Value::as_str)
        .unwrap_or("none");

    let mut out = String::new();
    out.push_str("# PR Evidence Summary\n\n");
    out.push_str("## Fast Gate\n\n");
    out.push_str(&format!(
        "- status: {}\n",
        string_field(packet, "status", "unknown")
    ));
    out.push_str(&format!(
        "- root: `{}`\n",
        md_escape(string_field(packet, "root", "."))
    ));
    out.push_str(&format!(
        "- base: `{}`\n",
        md_escape(string_field(packet, "base", DEFAULT_BASE))
    ));
    out.push_str(&format!(
        "- head: `{}`\n",
        md_escape(string_field(packet, "head", DEFAULT_HEAD))
    ));
    out.push_str(&format!("- changed files: {changed_files}\n\n"));

    out.push_str("## RIPR\n\n");
    out.push_str(&format!("- changed-line comments: {comments}\n"));
    out.push_str(&format!("- summary-only guidance: {summary_only}\n"));
    out.push_str(&format!("- suppressed guidance: {suppressed}\n"));
    out.push_str(&format!("- weakly_exposed: {weakly_exposed}\n"));
    out.push_str(&format!("- reachable_unrevealed: {reachable_unrevealed}\n"));
    out.push_str(&format!("- no_static_path: {no_static_path}\n"));
    out.push_str(&format!("- severe gaps: {severe_gaps}\n\n"));

    out.push_str("## Targeted Mutation\n\n");
    out.push_str(&format!(
        "- requires_targeted_mutation: {requires_targeted_mutation}\n"
    ));
    out.push_str(&format!(
        "- routing_reason: `{}`\n\n",
        md_escape(routing_reason)
    ));

    out.push_str("## Artifacts\n\n");
    out.push_str("| Artifact | Path | Scope | Available |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    if let Some(artifacts) = packet.get("artifacts").and_then(Value::as_array) {
        for artifact in artifacts {
            out.push_str(&format!(
                "| {} | `{}` | {} | {} |\n",
                md_escape(string_field(artifact, "label", "artifact")),
                md_escape(string_field(artifact, "path", "unknown")),
                md_escape(string_field(artifact, "scope", "unknown")),
                artifact
                    .get("available")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            ));
        }
    }

    if let Some(warnings) = packet.get("warnings").and_then(Value::as_array)
        && !warnings.is_empty()
    {
        out.push_str("\n## Warnings\n\n");
        for warning in warnings {
            out.push_str(&format!(
                "- {}: {}\n",
                md_escape(string_field(warning, "kind", "warning")),
                md_escape(string_field(
                    warning,
                    "message",
                    "PR evidence generation warning"
                ))
            ));
        }
    }

    out.push_str(
        "\n_This packet is diff-scoped and advisory. Do not copy it into public badge state._\n",
    );
    out
}

fn bool_field(summary: Option<&Map<String, Value>>, key: &str) -> bool {
    summary
        .and_then(|summary| summary.get(key))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn string_field<'a>(packet: &'a Value, key: &str, fallback: &'a str) -> &'a str {
    packet.get(key).and_then(Value::as_str).unwrap_or(fallback)
}

fn md_escape(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
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

    fn options() -> PrEvidenceOptions {
        PrEvidenceOptions {
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
            Err("unknown ripr-pr argument \"--bad\"".to_string())
        );
        assert_eq!(
            parse_options(&["--base".into(), "".into()]),
            Err("ripr-pr --base requires a non-empty value".to_string())
        );
    }

    #[test]
    fn packet_maps_check_summary_to_routing_fields() {
        let check = json!({
            "summary": {
                "weakly_exposed": 2,
                "reachable_unrevealed": 1,
                "no_static_path": 0
            }
        });
        let changed = vec!["src/lib.rs".to_string(), "tests/lib.rs".to_string()];
        let packet = pr_evidence_packet(&options(), &changed, &check);
        assert_eq!(packet["summary"]["changed_files"], 2);
        assert_eq!(packet["summary"]["weakly_exposed"], 2);
        assert_eq!(packet["summary"]["reachable_unrevealed"], 1);
        assert_eq!(packet["summary"]["severe_gaps"], 3);
        assert_eq!(packet["summary"]["requires_targeted_mutation"], true);
        assert_eq!(packet["summary"]["routing_reason"], "ripr severe gap");
    }

    #[test]
    fn packet_without_check_summary_is_incomplete_and_warns() {
        let packet = pr_evidence_packet(&options(), &[], &json!({}));
        assert_eq!(packet["status"], "incomplete");
        assert_eq!(packet["warnings"][0]["kind"], "invalid_json");
    }

    #[test]
    fn error_packet_is_contract_valid_and_actionable() {
        let changed = vec!["src/lib.rs".to_string()];
        let packet = pr_evidence_error_packet(
            &options(),
            &changed,
            "ripr check for PR evidence timed out after 120 seconds; retry command: cargo xtask ripr-pr --base origin/main --head HEAD --root .",
        );
        assert_eq!(packet["status"], "error");
        assert_eq!(packet["summary"]["changed_files"], 1);
        assert_eq!(packet["summary"]["severe_gaps"], 0);
        assert_eq!(packet["summary"]["ripr_severe_gap"], false);
        assert_eq!(packet["warnings"][0]["kind"], "tool_error");
        assert!(
            packet["warnings"][0]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("retry command")
        );
        let violations = validate_packet_value(&packet, &options(), 1, true);
        assert_eq!(violations, Vec::<String>::new());
    }

    #[test]
    fn validation_rejects_changed_file_drift() {
        let packet = pr_evidence_packet(
            &options(),
            &["src/lib.rs".to_string()],
            &json!({
                "summary": {
                    "weakly_exposed": 0,
                    "reachable_unrevealed": 0,
                    "no_static_path": 0
                }
            }),
        );
        let violations = validate_packet_value(&packet, &options(), 2, true);
        assert!(
            violations
                .iter()
                .any(|violation| { violation.contains("summary.changed_files is 1, expected 2") })
        );
    }

    #[test]
    fn validation_requires_markdown_artifact() {
        let packet = pr_evidence_packet(
            &options(),
            &[],
            &json!({
                "summary": {
                    "weakly_exposed": 0,
                    "reachable_unrevealed": 0,
                    "no_static_path": 0
                }
            }),
        );
        let violations = validate_packet_value(&packet, &options(), 0, false);
        assert!(violations.contains(&format!("{PR_EVIDENCE_MD} is missing")));
    }

    #[test]
    fn markdown_renders_stable_summary_sections() {
        let packet = pr_evidence_packet(
            &options(),
            &["src/lib.rs".to_string()],
            &json!({
                "summary": {
                    "weakly_exposed": 1,
                    "reachable_unrevealed": 0,
                    "no_static_path": 0
                }
            }),
        );
        let markdown = render_pr_evidence_markdown(&packet);
        assert!(markdown.contains("# PR Evidence Summary"));
        assert!(markdown.contains("## Fast Gate"));
        assert!(markdown.contains("## RIPR"));
        assert!(markdown.contains("## Targeted Mutation"));
        assert!(markdown.contains("target/ripr/pr/repo-exposure.json"));
    }

    #[test]
    fn markdown_renders_error_warnings() {
        let packet = pr_evidence_error_packet(
            &options(),
            &["src/lib.rs".to_string()],
            "ripr check for PR evidence failed; retry command: cargo xtask ripr-pr --base origin/main --head HEAD --root .",
        );
        let markdown = render_pr_evidence_markdown(&packet);
        assert!(markdown.contains("## Warnings"));
        assert!(markdown.contains("tool_error"));
        assert!(markdown.contains("retry command"));
    }

    #[test]
    fn write_pr_evidence_writes_error_packet_when_check_fails() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-error-packet")?;
        run_git(&repo, &["init"])?;
        run_git(&repo, &["config", "user.email", "ripr-pr@example.invalid"])?;
        run_git(&repo, &["config", "user.name", "RIPR PR Test"])?;
        write_repo_file(&repo, "README.md", "# sample\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "initial"])?;
        write_repo_file(&repo, "src/lib.rs", "pub fn value() -> u8 { 1 }\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "add rust"])?;

        let options = PrEvidenceOptions {
            base: "HEAD~1".to_string(),
            head: "HEAD".to_string(),
            ..options()
        };
        write_pr_evidence_with_runner(&repo, &options, |_repo, _options| {
            Err("ripr check for PR evidence timed out after 120 seconds; retry command: cargo xtask ripr-pr --base HEAD~1 --head HEAD --root .".to_string())
        })?;
        check_pr_evidence(&repo, &options)?;

        let packet_text = fs::read_to_string(repo.join(PR_EVIDENCE_JSON))
            .map_err(|err| format!("read packet: {err}"))?;
        let packet: Value =
            serde_json::from_str(&packet_text).map_err(|err| format!("parse packet: {err}"))?;
        assert_eq!(packet["status"], "error");
        assert_eq!(packet["warnings"][0]["kind"], "tool_error");
        assert!(repo.join(PR_DIFF).exists());
        assert!(repo.join(PR_EVIDENCE_MD).exists());

        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn write_and_check_packet_in_git_repo() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-packet")?;
        run_git(&repo, &["init"])?;
        run_git(&repo, &["config", "user.email", "ripr-pr@example.invalid"])?;
        run_git(&repo, &["config", "user.name", "RIPR PR Test"])?;
        write_repo_file(&repo, "README.md", "# sample\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "initial"])?;
        write_repo_file(&repo, "src/lib.rs", "pub fn value() -> u8 { 1 }\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "add rust"])?;

        let options = PrEvidenceOptions {
            base: "HEAD~1".to_string(),
            head: "HEAD".to_string(),
            ..options()
        };
        let check_json = r#"{
          "summary": {
            "weakly_exposed": 1,
            "reachable_unrevealed": 0,
            "no_static_path": 0
          }
        }"#;
        write_pr_evidence_from_check_json(&repo, &options, check_json)?;
        check_pr_evidence(&repo, &options)?;

        let packet_text = fs::read_to_string(repo.join(PR_EVIDENCE_JSON))
            .map_err(|err| format!("read packet: {err}"))?;
        let packet: Value =
            serde_json::from_str(&packet_text).map_err(|err| format!("parse packet: {err}"))?;
        assert_eq!(packet["summary"]["changed_files"], 1);
        assert_eq!(packet["summary"]["weakly_exposed"], 1);
        assert_eq!(packet["summary"]["requires_targeted_mutation"], true);
        assert!(repo.join(PR_DIFF).exists());
        assert!(repo.join(PR_EVIDENCE_MD).exists());

        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
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

    #[test]
    fn built_binary_path_honors_absolute_target_dir() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-target-dir")?;
        let cwd = repo.join("subdir");
        let target = repo.join("custom-target");
        let expected = target.join("debug").join(ripr_exe_name());
        assert_eq!(
            built_ripr_binary_path_from_target_dir(&repo, &cwd, Some(target.as_os_str())),
            expected
        );
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }
}
