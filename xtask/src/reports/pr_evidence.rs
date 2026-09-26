use super::pr_causal_delta::write_canonical_delta;
use super::write_parented_file;
use crate::run::{
    capture_output_with_timeout, capture_process_output, run_output_owned,
    run_output_owned_with_timeout, tool_build_timeout,
};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
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
const PR_CHECK_JSON: &str = "target/ripr/pr/check.json";
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

fn reject_error_packet(repo: &Path) -> Result<(), String> {
    let packet_path = repo.join(PR_EVIDENCE_JSON);
    let packet_text = fs::read_to_string(&packet_path)
        .map_err(|err| format!("read generated {PR_EVIDENCE_JSON}: {err}"))?;
    let packet: Value = serde_json::from_str(&packet_text)
        .map_err(|err| format!("generated {PR_EVIDENCE_JSON} is not valid JSON: {err}"))?;
    if let Some(error) = ripr::reject_pr_evidence_error_packet(&packet) {
        return Err(error);
    }
    Ok(())
}

fn write_pr_evidence_with_runner(
    repo: &Path,
    options: &PrEvidenceOptions,
    run_check: impl FnOnce(&Path, &PrEvidenceOptions) -> Result<String, String>,
) -> Result<(), String> {
    remove_stale_check_artifact(repo)?;
    verify_revision(repo, &options.base)?;
    verify_revision(repo, &options.head)?;
    let changed_files = changed_files(repo, options)?;
    write_diff(repo, options)?;
    write_canonical_delta(
        repo,
        &options.base,
        &options.head,
        &changed_files,
        &options.root,
    )?;
    let result = match run_check(repo, options) {
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
    };
    result?;
    reject_error_packet(repo)
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
    let check_json_text = serde_json::to_string_pretty(&check_value)
        .map_err(|err| format!("serialize canonical check output: {err}"))?;

    write_parented_file(
        &repo.join(PR_CHECK_JSON),
        PR_CHECK_JSON,
        format!("{check_json_text}\n"),
    )?;

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

fn remove_stale_check_artifact(repo: &Path) -> Result<(), String> {
    match fs::remove_file(repo.join(PR_CHECK_JSON)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("remove stale {PR_CHECK_JSON} failed: {error}")),
    }
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
    if let Some(error) = ripr::reject_pr_evidence_error_packet(&packet) {
        return Err(error);
    }
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
    // Raw NUL-delimited inventory (#4004, #4006): `-z` output is never
    // C-quoted, so exotic names survive byte-exact; parsing rules come from
    // the shared authority below, not from line splitting here. The
    // `--diff-filter=ACMR` scope is the retained packet contract.
    let range = format!("{}...{}", options.base, options.head);
    let git_args = vec![
        "-C".to_string(),
        repo.display().to_string(),
        "diff".to_string(),
        "--name-only".to_string(),
        "-z".to_string(),
        "--diff-filter=ACMR".to_string(),
        range,
    ];
    let output = capture_process_output("git", &git_args, &[])
        .map_err(|error| format!("git diff --name-only -z inventory: {}", error.message))?;
    decode_changed_files(&output)
}

/// Decode raw `--name-only -z` bytes through the shared NUL path-record
/// authority (#4006). Strict: non-UTF-8, empty, or truncated records fail
/// loudly instead of collapsing through lossy conversion.
fn decode_changed_files(output: &[u8]) -> Result<Vec<String>, String> {
    ripr::analysis::parse_git_path_records(output)
        .map_err(|err| format!("PR evidence changed-file inventory: {err}"))
        .and_then(|paths| {
            paths
                .iter()
                .map(|path| {
                    path.to_str().map(str::to_string).ok_or_else(|| {
                        format!(
                            "PR evidence changed-file inventory: decoded path {} is not valid UTF-8",
                            path.display()
                        )
                    })
                })
                .collect()
        })
}

fn write_diff(repo: &Path, options: &PrEvidenceOptions) -> Result<(), String> {
    let out = repo.join(PR_DIFF);
    // Route the packet diff through the shared pinned Git assembly (#3930,
    // #4004): ambient textconv, color, external-diff, and context config must
    // not change what the packet analyzes. The assembly pins `-c
    // core.quotePath=true`, `--no-ext-diff`, `--no-textconv`, `--no-color`,
    // `--src-prefix=a/`, `--dst-prefix=b/`, `--binary`, and three-context
    // presentation.
    let diff = ripr::analysis::load_pr_evidence_diff_range(repo, &options.base, &options.head)?;
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
        "--base".to_string(),
        options.base.clone(),
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
            run_output_owned_with_timeout(
                "cargo",
                &build_args,
                tool_build_timeout()?,
                "cargo build of the ripr binary for PR evidence",
            )?;
            built_ripr_binary_path(repo)?.display().to_string()
        }
    };
    let timeout = Duration::from_secs(pr_evidence_timeout_secs()?);
    run_ripr_check_binary(&binary, ripr_args, options, timeout)
}

fn run_ripr_check_binary(
    binary: &str,
    ripr_args: Vec<String>,
    options: &PrEvidenceOptions,
    timeout: Duration,
) -> Result<String, String> {
    let output = capture_output_with_timeout(
        binary,
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
        Ok(value) => parse_positive_timeout_secs(PR_EVIDENCE_TIMEOUT_ENV, &value),
        Err(_) => Ok(DEFAULT_TOOL_TIMEOUT_SECS),
    }
}

fn parse_positive_timeout_secs(name: &str, value: &str) -> Result<u64, String> {
    let parsed = value
        .trim()
        .parse::<u64>()
        .map_err(|err| format!("{name} must be a positive integer: {err}"))?;
    if parsed > 0 {
        Ok(parsed)
    } else {
        Err(format!("{name} must be a positive integer"))
    }
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
    let targeted_mutation_route = targeted_mutation_route(check_value, ripr_severe_gap);

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
            "routing_reason": routing_reason,
            "targeted_mutation_route": targeted_mutation_route
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
            "routing_reason": null,
            "targeted_mutation_route": {
                "status": "not_required",
                "candidates": [],
                "limitations": []
            }
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

/// Mirrors `targeted_mutation_route` in `crates/ripr/src/app/pr_evidence.rs`
/// so the compatibility shim emits the same schema-required route the
/// `ripr pr-evidence` producer emits. Candidates derive only from
/// producer-owned probe facts on findings whose `source_currentness` is
/// `candidate_current` — a current head obligation, never base-side evidence.
/// Inputs that cannot yield a safe candidate produce honest limitations,
/// never invented candidates.
fn targeted_mutation_route(check_value: &Value, required: bool) -> Value {
    let mut candidates = Vec::new();
    let mut limitations = Vec::new();
    let mut seen = BTreeSet::new();
    for finding in check_value
        .get("findings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(classification) = finding.get("classification").and_then(Value::as_str) else {
            continue;
        };
        // Candidate-actionable eligibility (#3281): mutation candidates are
        // current obligations; base-side evidence never names a head target.
        if finding.get("source_currentness").and_then(Value::as_str) != Some("candidate_current") {
            continue;
        }
        if !matches!(
            classification,
            "weakly_exposed" | "reachable_unrevealed" | "no_static_path"
        ) {
            continue;
        }
        let Some(probe) = finding.get("probe").and_then(Value::as_object) else {
            limitations.push(json!({
                "kind": "no_safe_candidate",
                "message": "finding has no producer-owned probe facts from which to derive a safe mutation candidate"
            }));
            continue;
        };
        let family = probe
            .get("family")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let file = probe.get("file").and_then(Value::as_str);
        let line = probe.get("line").and_then(Value::as_u64);
        let expression = probe.get("expression").and_then(Value::as_str);
        let Some((from, to)) = (family == "predicate")
            .then(|| expression.and_then(predicate_operator_flip))
            .flatten()
        else {
            limitations.push(json!({
                "kind": "no_safe_candidate",
                "family": family,
                "message": format!("no safe concrete mutation candidate could be derived for {family} producer evidence")
            }));
            continue;
        };
        let Some(file) = file.filter(|file| !file.trim().is_empty()) else {
            limitations.push(json!({
                "kind": "no_safe_candidate",
                "family": family,
                "message": "predicate mutation candidate has no producer-owned source file"
            }));
            continue;
        };
        let Some(line) = line else {
            limitations.push(json!({
                "kind": "no_safe_candidate",
                "family": family,
                "message": "predicate mutation candidate has no unambiguous source line"
            }));
            continue;
        };
        let key = format!("{file}:{line}:{from}:{to}");
        if !seen.insert(key) {
            continue;
        }
        candidates.push(json!({
            "file": file,
            "line": line,
            "kind": "predicate_operator_flip",
            "from": from,
            "to": to,
            "command": format!("cargo mutants --file \"{}\"", file.replace('"', "\\\"")),
            "expected_observation": format!("the focused boundary test should observe the predicate change {from} -> {to}")
        }));
    }
    let status = if !required {
        "not_required"
    } else if candidates.is_empty() {
        "static_limitation"
    } else {
        "candidate"
    };
    json!({
        "status": status,
        "candidates": candidates,
        "limitations": limitations
    })
}

fn predicate_operator_flip(expression: &str) -> Option<(&'static str, &'static str)> {
    [
        (">=", ">"),
        ("<=", "<"),
        ("==", "!="),
        ("!=", "=="),
        (">", ">="),
        ("<", "<="),
    ]
    .into_iter()
    .find_map(|(from, to)| expression.contains(from).then_some((from, to)))
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
    validate_targeted_mutation_route(summary, &mut violations);

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

/// Presence and eligibility check for the schema-required
/// `summary.targeted_mutation_route`. The app producer
/// (`crates/ripr/src/app/pr_evidence.rs`) pairs `status: "not_required"`
/// with `requires_targeted_mutation: false` and never derives candidates
/// from base-side evidence, so a packet whose route disagrees with the
/// eligibility rule is producer drift, not a softer state.
fn validate_targeted_mutation_route(summary: &Map<String, Value>, violations: &mut Vec<String>) {
    let Some(route) = summary
        .get("targeted_mutation_route")
        .and_then(Value::as_object)
    else {
        violations.push("summary.targeted_mutation_route is missing or not an object".to_string());
        return;
    };
    let status = route.get("status").and_then(Value::as_str);
    match status {
        Some("not_required" | "candidate" | "static_limitation") => {}
        Some(other) => violations.push(format!(
            "summary.targeted_mutation_route.status {other:?} is not contract-valid"
        )),
        None => violations
            .push("summary.targeted_mutation_route.status is missing or not a string".to_string()),
    }
    for key in ["candidates", "limitations"] {
        if !route.get(key).is_some_and(Value::is_array) {
            violations.push(format!(
                "summary.targeted_mutation_route.{key} is missing or not an array"
            ));
        }
    }
    let required = summary
        .get("requires_targeted_mutation")
        .is_some_and(|value| value == &Value::Bool(true));
    if let Some(status) = status
        && (status == "not_required") == required
    {
        violations.push(format!(
            "summary.targeted_mutation_route.status {status:?} disagrees with \
             summary.requires_targeted_mutation {required}"
        ));
    }
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
    fn packet_derives_targeted_mutation_route_from_candidate_current_findings() {
        let check = json!({
            "summary": {
                "weakly_exposed": 1,
                "reachable_unrevealed": 0,
                "no_static_path": 0
            },
            "findings": [
                {
                    "classification": "weakly_exposed",
                    "source_currentness": "candidate_current",
                    "probe": {
                        "family": "predicate",
                        "file": "src/pricing.rs",
                        "line": 42,
                        "expression": "amount >= threshold"
                    }
                },
                {
                    "classification": "weakly_exposed",
                    "source_currentness": "base_only",
                    "probe": {
                        "family": "predicate",
                        "file": "src/base.rs",
                        "line": 7,
                        "expression": "count >= limit"
                    }
                }
            ]
        });
        let packet = pr_evidence_packet(&options(), &["src/pricing.rs".to_string()], &check);
        let route = &packet["summary"]["targeted_mutation_route"];
        assert_eq!(route["status"], "candidate");
        assert_eq!(route["candidates"].as_array().map(Vec::len), Some(1));
        assert_eq!(route["candidates"][0]["file"], "src/pricing.rs");
        assert_eq!(route["candidates"][0]["from"], ">=");
        assert_eq!(route["candidates"][0]["to"], ">");
        let candidates = route["candidates"].as_array();
        assert!(
            candidates.is_some_and(|candidates| candidates
                .iter()
                .all(|candidate| candidate["file"] != "src/base.rs")),
            "base-side evidence must never name a head mutation target"
        );
        assert_eq!(route["limitations"], json!([]));
        let violations = validate_packet_value(&packet, &options(), 1, true);
        assert_eq!(violations, Vec::<String>::new());
    }

    #[test]
    fn packet_marks_required_route_static_limitation_without_safe_candidate() {
        let check = json!({
            "summary": {
                "weakly_exposed": 1,
                "reachable_unrevealed": 0,
                "no_static_path": 0
            },
            "findings": [
                {
                    "classification": "weakly_exposed",
                    "source_currentness": "candidate_current"
                }
            ]
        });
        let packet = pr_evidence_packet(&options(), &["src/lib.rs".to_string()], &check);
        let route = &packet["summary"]["targeted_mutation_route"];
        assert_eq!(route["status"], "static_limitation");
        assert_eq!(route["candidates"], json!([]));
        assert_eq!(route["limitations"][0]["kind"], "no_safe_candidate");
        let violations = validate_packet_value(&packet, &options(), 1, true);
        assert_eq!(violations, Vec::<String>::new());
    }

    #[test]
    fn validation_rejects_route_status_that_disagrees_with_eligibility() {
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
        let mut drifted = packet.clone();
        drifted["summary"]["targeted_mutation_route"]["status"] = "candidate".into();
        let violations = validate_packet_value(&drifted, &options(), 1, true);
        assert!(
            violations
                .iter()
                .any(|violation| violation
                    .contains("disagrees with summary.requires_targeted_mutation")),
            "a not_required route claiming candidates must be rejected: {violations:?}"
        );

        let mut missing = packet;
        assert!(
            matches!(missing.get_mut("summary"), Some(Value::Object(_))),
            "summary must be an object for the removal test"
        );
        if let Some(summary) = missing.get_mut("summary").and_then(Value::as_object_mut) {
            summary.remove("targeted_mutation_route");
        }
        let violations = validate_packet_value(&missing, &options(), 1, true);
        assert!(
            violations.iter().any(|violation| {
                violation.contains("summary.targeted_mutation_route is missing")
            }),
            "an absent route must be rejected: {violations:?}"
        );
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
    fn timeout_parser_rejects_non_positive_and_invalid_values() -> Result<(), String> {
        assert_eq!(
            parse_positive_timeout_secs("RIPR_TEST_TIMEOUT", "120"),
            Ok(120)
        );
        assert_eq!(
            parse_positive_timeout_secs("RIPR_TEST_TIMEOUT", "0"),
            Err("RIPR_TEST_TIMEOUT must be a positive integer".to_string())
        );
        let err = match parse_positive_timeout_secs("RIPR_TEST_TIMEOUT", "abc") {
            Ok(value) => return Err(format!("invalid timeout should fail, got {value}")),
            Err(err) => err,
        };
        assert!(err.contains("RIPR_TEST_TIMEOUT"));
        assert!(err.contains("positive integer"));
        Ok(())
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
        write_parented_file(
            &repo.join(PR_CHECK_JSON),
            PR_CHECK_JSON,
            "{\"stale\":true}\n",
        )?;
        let producer_result = write_pr_evidence_with_runner(&repo, &options, |_repo, _options| {
            Err("ripr check for PR evidence timed out after 120 seconds; retry command: cargo xtask ripr-pr --base HEAD~1 --head HEAD --root .".to_string())
        });
        let producer_error = producer_result
            .err()
            .ok_or_else(|| "producer error packet unexpectedly passed generation".to_string())?;
        assert!(producer_error.contains("review-comments must not run"));

        let packet_text = fs::read_to_string(repo.join(PR_EVIDENCE_JSON))
            .map_err(|err| format!("read packet: {err}"))?;
        let packet: Value =
            serde_json::from_str(&packet_text).map_err(|err| format!("parse packet: {err}"))?;
        assert_eq!(packet["status"], "error");
        assert_eq!(packet["warnings"][0]["kind"], "tool_error");
        assert!(repo.join(PR_DIFF).exists());
        assert!(repo.join(PR_EVIDENCE_MD).exists());
        assert!(!repo.join(PR_CHECK_JSON).exists());
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn run_ripr_check_uses_fake_binary_success_output() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-fake-success")?;
        let fake = fake_ripr_invocation(
            &repo,
            "fake-ripr-success",
            r#"{"summary":{"weakly_exposed":1,"reachable_unrevealed":0,"no_static_path":0}}"#,
            "",
            0,
            None,
        )?;
        let result =
            run_ripr_check_binary(&fake.binary, fake.args, &options(), Duration::from_secs(30))?;
        assert!(result.contains(r#""weakly_exposed":1"#));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn run_ripr_check_reports_fake_binary_failure() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-fake-failure")?;
        let fake = fake_ripr_invocation(&repo, "fake-ripr-failure", "", "bad diff", 7, None)?;
        let err = match run_ripr_check_binary(
            &fake.binary,
            fake.args,
            &options(),
            Duration::from_secs(30),
        ) {
            Ok(output) => return Err(format!("fake failure should fail, got {output}")),
            Err(err) => err,
        };
        assert!(err.contains("ripr check for PR evidence failed"));
        assert!(err.contains("bad diff"));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn run_ripr_check_reports_fake_binary_timeout() -> Result<(), String> {
        #[cfg(not(windows))]
        let repo = temp_repo("ripr-pr-fake-timeout")?;
        #[cfg(windows)]
        let (binary, args) = (
            "powershell".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                "Start-Sleep -Seconds 30".to_string(),
            ],
        );
        #[cfg(not(windows))]
        let (binary, args) = {
            let fake = fake_ripr_invocation(&repo, "fake-ripr-timeout", "", "", 0, Some(30))?;
            (fake.binary, fake.args)
        };
        let err = match run_ripr_check_binary(&binary, args, &options(), Duration::from_secs(1)) {
            Ok(output) => return Err(format!("fake timeout should fail, got {output}")),
            Err(err) => err,
        };
        assert!(err.contains("timed out after 1 seconds"));
        assert!(err.contains("retry command: cargo xtask ripr-pr"));
        #[cfg(not(windows))]
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
          "schema_version": "0.2",
          "tool": "ripr",
          "mode": "draft",
          "root": ".",
          "base": "HEAD~1",
          "summary": {
            "weakly_exposed": 1,
            "reachable_unrevealed": 0,
            "no_static_path": 0
          },
          "findings": [],
          "analysis_outcome": {
            "analysis_complete": true,
            "outcome": {
              "schema_version": "0.1",
              "kind": "no_behavioral_candidates",
              "identity": {
                "repository_identity": null,
                "root_identity": null,
                "config_identity": null,
                "base_revision": "HEAD~1",
                "input_identity": "sha256:fixture",
                "snapshot_identity": null
              },
              "counts": {
                "changed_file_count": 1,
                "changed_line_count": 1,
                "candidate_line_count": 0,
                "probe_count": 0,
                "finding_count": 0
              },
              "limitations": [],
              "claim_boundary": "Static analysis outcome only; no correctness, test-adequacy, runtime-execution, or merge-readiness claim."
            }
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
        let check_text = fs::read_to_string(repo.join(PR_CHECK_JSON))
            .map_err(|err| format!("read canonical check output: {err}"))?;
        let check_value: Value = serde_json::from_str(&check_text)
            .map_err(|err| format!("parse canonical check output: {err}"))?;
        assert_eq!(check_value["summary"]["weakly_exposed"], 1);
        assert_eq!(check_value["schema_version"], "0.2");
        assert_eq!(check_value["tool"], "ripr");
        assert_eq!(check_value["mode"], "draft");
        assert_eq!(check_value["root"], ".");
        assert_eq!(check_value["base"], "HEAD~1");
        assert!(check_value["findings"].is_array());
        assert_eq!(check_value["analysis_outcome"]["analysis_complete"], true);
        assert_eq!(
            check_value["analysis_outcome"]["outcome"]["kind"],
            "no_behavioral_candidates"
        );

        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn pinned_diff_keeps_edit_under_hostile_color_config() -> Result<(), String> {
        // Discriminates the pinned `--no-color` assembly: repo-local
        // `color.diff=always` must not move packet bytes.
        let repo = temp_repo("ripr-pr-color")?;
        run_git(&repo, &["init"])?;
        run_git(&repo, &["config", "user.email", "ripr-pr@example.invalid"])?;
        run_git(&repo, &["config", "user.name", "RIPR PR Test"])?;
        run_git(&repo, &["config", "color.diff", "always"])?;
        let body = (1..=9).map(|n| format!("line {n}\n")).collect::<String>();
        write_repo_file(&repo, "notes.txt", &body)?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "initial"])?;
        let changed = body.replace("line 5\n", "line FIVE\n");
        write_repo_file(&repo, "notes.txt", &changed)?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "edit"])?;

        let options = PrEvidenceOptions {
            base: "HEAD~1".to_string(),
            head: "HEAD".to_string(),
            ..options()
        };
        write_pr_evidence_from_check_json(&repo, &options, MINIMAL_CHECK_JSON)?;
        check_pr_evidence(&repo, &options)?;

        let diff =
            fs::read(repo.join(PR_DIFF)).map_err(|err| format!("read {}: {err}", PR_DIFF))?;
        if diff.contains(&0x1b) {
            return Err("hostile color.diff config leaked ANSI escapes into pr.diff".to_string());
        }
        let text = String::from_utf8(diff).map_err(|err| format!("pr.diff not UTF-8: {err}"))?;
        if !text.contains("+line FIVE") {
            return Err("pr.diff lost the edited line".to_string());
        }
        // Three-context presentation pin: an unchanged line three away from
        // the edit must survive, distinguishing the packet view from a
        // zero-context diff.
        if !text.contains(" line 2") || !text.contains(" line 8") {
            return Err("pr.diff lost three-context presentation lines".to_string());
        }

        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn pinned_diff_survives_textconv_driver() -> Result<(), String> {
        // Discriminates the pinned `--no-textconv` assembly: a repository
        // textconv driver that censors content must not hide the edit.
        let repo = temp_repo("ripr-pr-textconv")?;
        run_git(&repo, &["init"])?;
        run_git(&repo, &["config", "user.email", "ripr-pr@example.invalid"])?;
        run_git(&repo, &["config", "user.name", "RIPR PR Test"])?;
        write_repo_file(&repo, ".gitattributes", "*.txt diff=riprcensor\n")?;
        run_git(
            &repo,
            &["config", "diff.riprcensor.textconv", "echo CENSORED"],
        )?;
        write_repo_file(&repo, "secret.txt", "alpha\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "initial"])?;
        write_repo_file(&repo, "secret.txt", "alpha\nbravo\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "edit"])?;

        let options = PrEvidenceOptions {
            base: "HEAD~1".to_string(),
            head: "HEAD".to_string(),
            ..options()
        };
        write_pr_evidence_from_check_json(&repo, &options, MINIMAL_CHECK_JSON)?;
        check_pr_evidence(&repo, &options)?;

        let diff =
            fs::read(repo.join(PR_DIFF)).map_err(|err| format!("read {}: {err}", PR_DIFF))?;
        let text = String::from_utf8(diff).map_err(|err| format!("pr.diff not UTF-8: {err}"))?;
        if !text.contains("+bravo") {
            return Err("textconv driver hid the edited line from pr.diff".to_string());
        }

        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn written_diff_matches_pinned_authority() -> Result<(), String> {
        // Parity/currentness pin (#4004 item 5): the bytes the xtask route
        // writes must stay identical to the shared authority's output for the
        // same repository and range. A deliberate divergence in either route
        // fails here.
        let repo = temp_repo("ripr-pr-parity")?;
        run_git(&repo, &["init"])?;
        run_git(&repo, &["config", "user.email", "ripr-pr@example.invalid"])?;
        run_git(&repo, &["config", "user.name", "RIPR PR Test"])?;
        write_repo_file(&repo, "src/lib.rs", "pub fn value() -> u8 { 1 }\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "initial"])?;
        write_repo_file(&repo, "src/lib.rs", "pub fn value() -> u8 { 2 }\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "edit"])?;

        let options = PrEvidenceOptions {
            base: "HEAD~1".to_string(),
            head: "HEAD".to_string(),
            ..options()
        };
        write_pr_evidence_from_check_json(&repo, &options, MINIMAL_CHECK_JSON)?;

        let written =
            fs::read(repo.join(PR_DIFF)).map_err(|err| format!("read {}: {err}", PR_DIFF))?;
        let authority = ripr::analysis::load_pr_evidence_diff_range(&repo, "HEAD~1", "HEAD")
            .map_err(|err| format!("pinned authority: {err}"))?;
        if written != authority.as_bytes() {
            return Err("xtask pr.diff diverged from the pinned diff authority".to_string());
        }

        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn nul_inventory_survives_exotic_names() -> Result<(), String> {
        // Discriminates NUL-delimited inventory: space, non-ASCII, and rename
        // records must decode exact; the old line parser C-quoted or split
        // them. Asserts through the real `changed_files` production path.
        let repo = temp_repo("ripr-pr-names")?;
        run_git(&repo, &["init"])?;
        run_git(&repo, &["config", "user.email", "ripr-pr@example.invalid"])?;
        run_git(&repo, &["config", "user.name", "RIPR PR Test"])?;
        write_repo_file(&repo, "base.txt", "base\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "initial"])?;
        write_repo_file(&repo, "sp ace.txt", "spaces\n")?;
        write_repo_file(&repo, "uni-\u{e9}.txt", "unicode\n")?;
        fs::remove_file(repo.join("base.txt")).map_err(|err| format!("remove base.txt: {err}"))?;
        write_repo_file(&repo, "renamed.txt", "base\n")?;
        run_git(&repo, &["add", "-A"])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "exotic"])?;

        let options = PrEvidenceOptions {
            base: "HEAD~1".to_string(),
            head: "HEAD".to_string(),
            ..options()
        };
        let mut files = changed_files(&repo, &options)?;
        files.sort();
        let expected = vec![
            "renamed.txt".to_string(),
            "sp ace.txt".to_string(),
            "uni-\u{e9}.txt".to_string(),
        ];
        if files != expected {
            return Err(format!(
                "exotic inventory mismatch: got {files:?}, want {expected:?}"
            ));
        }
        write_pr_evidence_from_check_json(&repo, &options, MINIMAL_CHECK_JSON)?;
        check_pr_evidence(&repo, &options)?;

        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn strict_inventory_rejects_non_utf8() -> Result<(), String> {
        // The strict-failure side of the NUL authority at the xtask decode
        // boundary: non-UTF-8 records fail loudly instead of collapsing
        // through lossy conversion. Live non-UTF-8 git names are impractical
        // on Windows runners, so this pins the mapping directly; the
        // end-to-end byte path is covered by `nul_inventory_survives_exotic_names`.
        let err = match decode_changed_files(b"ok.txt\0\xffbad\0") {
            Err(err) => err,
            Ok(files) => {
                return Err(format!("non-UTF-8 inventory must fail, decoded {files:?}"));
            }
        };
        if !err.contains("not valid UTF-8") {
            return Err(format!("unexpected strict-decode error: {err}"));
        }
        Ok(())
    }

    const MINIMAL_CHECK_JSON: &str = r#"{
      "schema_version": "0.2",
      "tool": "ripr",
      "mode": "draft",
      "root": ".",
      "summary": {
        "weakly_exposed": 0,
        "reachable_unrevealed": 0,
        "no_static_path": 0
      },
      "findings": []
    }"#;

    #[test]
    fn stale_check_artifact_is_removed_before_revision_setup_failure() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-stale-before-setup-failure")?;
        write_parented_file(
            &repo.join(PR_CHECK_JSON),
            PR_CHECK_JSON,
            "{\"stale\":true}\n",
        )?;
        let options = PrEvidenceOptions {
            base: "missing-base".to_string(),
            ..options()
        };

        let mut runner_called = false;
        let _error = write_pr_evidence_with_runner(&repo, &options, |_repo, _options| {
            runner_called = true;
            Err("runner must not execute after revision setup failure".to_string())
        })
        .err()
        .ok_or_else(|| "invalid revision should fail before the runner".to_string())?;

        assert!(!runner_called);
        assert!(!repo.join(PR_CHECK_JSON).exists());
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

    fn fake_ripr_args() -> Vec<String> {
        vec![
            "check".to_string(),
            "--root".to_string(),
            ".".to_string(),
            "--base".to_string(),
            "origin/main".to_string(),
            "--diff".to_string(),
            "target/ripr/pr/pr.diff".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ]
    }

    struct FakeRiprInvocation {
        binary: String,
        args: Vec<String>,
    }

    fn fake_ripr_invocation(
        repo: &Path,
        name: &str,
        stdout: &str,
        stderr: &str,
        exit_code: i32,
        sleep_seconds: Option<u64>,
    ) -> Result<FakeRiprInvocation, String> {
        let fake = fake_ripr_binary(repo, name, stdout, stderr, exit_code, sleep_seconds)?;
        #[cfg(windows)]
        {
            Ok(FakeRiprInvocation {
                binary: fake.display().to_string(),
                args: fake_ripr_args(),
            })
        }
        #[cfg(not(windows))]
        {
            let mut args = vec![fake.display().to_string()];
            args.extend(fake_ripr_args());
            Ok(FakeRiprInvocation {
                binary: "/bin/sh".to_string(),
                args,
            })
        }
    }

    fn fake_ripr_binary(
        repo: &Path,
        name: &str,
        stdout: &str,
        stderr: &str,
        exit_code: i32,
        sleep_seconds: Option<u64>,
    ) -> Result<PathBuf, String> {
        let path = repo.join(fake_ripr_name(name));
        #[cfg(windows)]
        {
            let mut script = String::from("@echo off\r\n");
            if let Some(seconds) = sleep_seconds {
                script.push_str(&format!(
                    "powershell -NoProfile -Command Start-Sleep -Seconds {seconds}\r\n"
                ));
            }
            if !stdout.is_empty() {
                script.push_str(&format!("echo {}\r\n", stdout));
            }
            if !stderr.is_empty() {
                script.push_str(&format!("echo {} 1>&2\r\n", stderr));
            }
            script.push_str(&format!("exit /b {exit_code}\r\n"));
            fs::write(&path, script).map_err(|err| format!("write {}: {err}", path.display()))?;
        }
        #[cfg(not(windows))]
        {
            let temp_path = path.with_extension("tmp");
            let mut script = String::from("#!/bin/sh\n");
            if let Some(seconds) = sleep_seconds {
                script.push_str(&format!("sleep {seconds}\n"));
            }
            if !stdout.is_empty() {
                script.push_str(&format!("printf '%s\\n' '{}'\n", sh_single_quote(stdout)));
            }
            if !stderr.is_empty() {
                script.push_str(&format!(
                    "printf '%s\\n' '{}' >&2\n",
                    sh_single_quote(stderr)
                ));
            }
            script.push_str(&format!("exit {exit_code}\n"));
            fs::write(&temp_path, script)
                .map_err(|err| format!("write {}: {err}", temp_path.display()))?;
            let mut permissions = fs::metadata(&temp_path)
                .map_err(|err| format!("metadata {}: {err}", temp_path.display()))?
                .permissions();
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(0o755);
            fs::set_permissions(&temp_path, permissions)
                .map_err(|err| format!("chmod {}: {err}", temp_path.display()))?;
            fs::rename(&temp_path, &path).map_err(|err| {
                format!(
                    "rename {} to {}: {err}",
                    temp_path.display(),
                    path.display()
                )
            })?;
        }
        Ok(path)
    }

    fn fake_ripr_name(name: &str) -> String {
        if cfg!(windows) {
            format!("{name}.cmd")
        } else {
            name.to_string()
        }
    }

    #[cfg(not(windows))]
    fn sh_single_quote(value: &str) -> String {
        value.replace('\'', "'\\''")
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
