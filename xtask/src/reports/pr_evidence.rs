use super::pr_causal_delta::write_canonical_delta;
use super::write_parented_file;
use crate::run::{
    capture_output_with_timeout, run_output_owned, run_output_owned_with_timeout,
    tool_build_timeout,
};
use ripr::review_input::{
    CanonicalFindingIndexV1, REVIEW_INDEX_MAX_BYTES, REVIEW_INDEX_MAX_ENTRIES,
    REVIEW_INDEX_SCHEMA_VERSION, REVIEW_INPUT_PROJECTION_LIMIT, REVIEW_INPUT_SCHEMA_VERSION,
    REVIEW_INPUT_SELECTION_POLICY, REVIEW_INPUT_SELECTION_POLICY_VERSION, ReviewInputV1,
    canonical_projection, canonical_projection_all,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEFAULT_ROOT: &str = ".";
const DEFAULT_BASE: &str = "origin/main";
const DEFAULT_HEAD: &str = "HEAD";
const PR_EVIDENCE_JSON: &str = "target/ripr/pr/repo-exposure.json";
const PR_EVIDENCE_MD: &str = "target/ripr/pr/repo-exposure.md";
const PR_CHECK_JSON: &str = "target/ripr/pr/check.json";
const PR_CHECK_SUBJECT_JSON: &str = "target/ripr/pr/check.subject.json";
const PR_REVIEW_INPUT_JSON: &str = "target/ripr/pr/review-input.json";
const PR_DIFF: &str = "target/ripr/pr/pr.diff";
const REVIEW_INPUT_MAX_BYTES: usize = 128 * 1024;
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
    match run_check(repo, options) {
        Ok(check_json) => {
            match write_pr_evidence_packet(repo, options, &changed_files, &check_json) {
                Ok(()) => Ok(()),
                Err(err) => {
                    let diagnostic =
                        format!("RIPR check output could not be converted into PR evidence: {err}");
                    write_pr_evidence_error_packet(repo, options, &changed_files, &diagnostic)?;
                    Err(diagnostic)
                }
            }
        }
        Err(err) => {
            write_pr_evidence_error_packet(repo, options, &changed_files, &err)?;
            Err(err)
        }
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
    if !check_value.is_object() {
        return Err("ripr check output must be a JSON object".to_string());
    }
    let packet = pr_evidence_packet(options, changed_files, &check_value);
    let json_text = serde_json::to_string_pretty(&packet)
        .map_err(|err| format!("serialize PR evidence packet: {err}"))?;
    let markdown = render_pr_evidence_markdown(&packet);
    let check_json_text = format!(
        "{}\n",
        serde_json::to_string_pretty(&check_value)
            .map_err(|err| format!("serialize canonical check output: {err}"))?
    );
    let mut subject = json!({
        "schema_version": "ripr.pr_check_subject.v1",
        "base_sha": resolve_revision(repo, &options.base, "commit")?,
        "head_sha": resolve_revision(repo, &options.head, "commit")?,
        "head_tree": resolve_revision(repo, &options.head, "tree")?,
        "check_sha256": format!("sha256:{:x}", Sha256::digest(check_json_text.as_bytes())),
        "check_byte_count": check_json_text.len(),
        "check_schema": check_value.get("schema_version").cloned().unwrap_or(Value::Null),
        "mode": check_value.get("mode").cloned().unwrap_or(Value::Null),
        "analysis_outcome": check_value.get("analysis_outcome").cloned().unwrap_or(Value::Null),
    });
    let root = repo
        .join(&options.root)
        .canonicalize()
        .map_err(|err| format!("resolve review input root failed: {err}"))?;
    let findings = check_value
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| "ripr check output findings must be an array".to_string())?;
    let entries = canonical_projection_all(findings, &root)
        .map_err(|error| format!("derive canonical finding index: {error}"))?;
    if entries.len() > REVIEW_INDEX_MAX_ENTRIES {
        return Err("canonical finding index exceeds entry limit".to_string());
    }
    let encoded_entries = serde_json::to_vec(&entries)
        .map_err(|error| format!("serialize canonical finding index: {error}"))?;
    if encoded_entries.len() > REVIEW_INDEX_MAX_BYTES {
        return Err("canonical finding index exceeds byte limit".to_string());
    }
    let index = CanonicalFindingIndexV1 {
        schema_version: REVIEW_INDEX_SCHEMA_VERSION.to_string(),
        total_finding_count: entries.len() as u64,
        index_sha256: format!("sha256:{:x}", Sha256::digest(&encoded_entries)),
        entries,
    };
    subject["canonical_finding_index"] = serde_json::to_value(index)
        .map_err(|error| format!("serialize canonical finding index: {error}"))?;
    subject["canonical_finding_index_entry_count"] = json!(findings.len());
    subject["canonical_finding_index_byte_count"] = json!(encoded_entries.len());
    let review_input = producer_review_input(&check_value, repo, options, &subject)?;
    let review_input_text = format!(
        "{}\n",
        serde_json::to_string_pretty(&review_input)
            .map_err(|err| format!("serialize producer review input: {err}"))?
    );
    subject["review_input_sha256"] = json!(format!(
        "sha256:{:x}",
        Sha256::digest(review_input_text.as_bytes())
    ));
    subject["review_input_byte_count"] = json!(review_input_text.len());
    let subject_text = format!(
        "{}\n",
        serde_json::to_string_pretty(&subject)
            .map_err(|err| format!("serialize check subject receipt: {err}"))?
    );

    write_parented_file(&repo.join(PR_CHECK_JSON), PR_CHECK_JSON, check_json_text)?;
    write_parented_file(
        &repo.join(PR_REVIEW_INPUT_JSON),
        PR_REVIEW_INPUT_JSON,
        review_input_text,
    )?;
    write_parented_file(
        &repo.join(PR_CHECK_SUBJECT_JSON),
        PR_CHECK_SUBJECT_JSON,
        subject_text,
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

fn producer_review_input(
    check: &Value,
    repo: &Path,
    options: &PrEvidenceOptions,
    subject: &Value,
) -> Result<Value, String> {
    let findings = check
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| "ripr check output findings must be an array".to_string())?;
    let root = repo
        .join(&options.root)
        .canonicalize()
        .map_err(|err| format!("resolve review input root failed: {err}"))?;
    let projected = canonical_projection(findings, &root)
        .map_err(|error| format!("derive review input projection: {error}"))?;
    let projected_count = projected.len();
    let total_finding_count = findings.len();
    let analysis_complete = check
        .get("analysis_outcome")
        .and_then(|outcome| outcome.get("analysis_complete"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let projection_truncated = projected_count < total_finding_count;
    let findings_value = serde_json::to_value(projected)
        .map_err(|error| format!("serialize review input projection: {error}"))?;
    let projection_bytes = serde_json::to_vec(&findings_value)
        .map_err(|err| format!("serialize producer review input digest: {err}"))?;
    let canonical_diff = fs::read(repo.join(PR_DIFF))
        .map_err(|err| format!("read canonical diff for review input binding: {err}"))?;
    if projection_bytes.len() > REVIEW_INPUT_MAX_BYTES {
        return Err(format!(
            "producer review input exceeds {REVIEW_INPUT_MAX_BYTES} byte limit"
        ));
    }
    let input = json!({
        "schema_version": REVIEW_INPUT_SCHEMA_VERSION,
        "mode": check["mode"],
        "root_identity": root.display().to_string().replace('\\', "/"),
        "base_sha": subject["base_sha"],
        "head_sha": subject["head_sha"],
        "head_tree": subject["head_tree"],
        "check_sha256": subject["check_sha256"],
        "canonical_diff_sha256": format!("sha256:{:x}", Sha256::digest(canonical_diff)),
        "analysis_complete": analysis_complete,
        "total_finding_count": total_finding_count,
        "projected_finding_count": projected_count,
        "projection_limit": REVIEW_INPUT_PROJECTION_LIMIT,
        "projection_truncated": projection_truncated,
        "projection_selection_policy": REVIEW_INPUT_SELECTION_POLICY,
        "projection_selection_policy_version": REVIEW_INPUT_SELECTION_POLICY_VERSION,
        "reviewed_count": projected_count,
        "projection_sha256": format!("sha256:{:x}", Sha256::digest(&projection_bytes)),
        "findings": findings_value,
    });
    let typed: ReviewInputV1 = serde_json::from_value(input)
        .map_err(|error| format!("producer review input does not match ReviewInputV1: {error}"))?;
    serde_json::to_value(typed)
        .map_err(|error| format!("serialize typed producer review input: {error}"))
}

fn remove_stale_check_artifact(repo: &Path) -> Result<(), String> {
    for relative in [PR_CHECK_JSON, PR_CHECK_SUBJECT_JSON, PR_REVIEW_INPUT_JSON] {
        match fs::remove_file(repo.join(relative)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("remove stale {relative} failed: {error}")),
        }
    }
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
    let mut violations = validate_packet_value(
        &packet,
        options,
        changed_files.len(),
        markdown_path.exists(),
    );
    if packet.get("status").and_then(Value::as_str) != Some("error") {
        violations.extend(check_subject_violations(repo, options));
    }
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

fn check_subject_violations(repo: &Path, options: &PrEvidenceOptions) -> Vec<String> {
    let check_path = repo.join(PR_CHECK_JSON);
    let (check_sha256, check_byte_count) = match digest_file(&check_path) {
        Ok(identity) => identity,
        Err(error) => return vec![format!("missing or unreadable {PR_CHECK_JSON}: {error}")],
    };
    let subject_text = match fs::read_to_string(repo.join(PR_CHECK_SUBJECT_JSON)) {
        Ok(text) => text,
        Err(error) => {
            return vec![format!(
                "missing or unreadable {PR_CHECK_SUBJECT_JSON}: {error}"
            )];
        }
    };
    let subject: Value = match serde_json::from_str(&subject_text) {
        Ok(value) => value,
        Err(error) => {
            return vec![format!(
                "{PR_CHECK_SUBJECT_JSON} is not valid JSON: {error}"
            )];
        }
    };
    let expected = [
        ("schema_version", "ripr.pr_check_subject.v1".to_string()),
        (
            "base_sha",
            resolve_revision(repo, &options.base, "commit").unwrap_or_default(),
        ),
        (
            "head_sha",
            resolve_revision(repo, &options.head, "commit").unwrap_or_default(),
        ),
        (
            "head_tree",
            resolve_revision(repo, &options.head, "tree").unwrap_or_default(),
        ),
        ("check_sha256", check_sha256),
    ];
    let mut violations: Vec<String> = expected
        .into_iter()
        .filter_map(|(field, expected)| {
            (subject.get(field).and_then(Value::as_str) != Some(expected.as_str())).then(|| {
                format!(
                    "{PR_CHECK_SUBJECT_JSON} {field} does not match the current PR evidence subject"
                )
            })
        })
        .collect();
    if subject.get("check_byte_count").and_then(Value::as_u64) != Some(check_byte_count) {
        violations.push(format!(
            "{PR_CHECK_SUBJECT_JSON} check_byte_count does not match check.json"
        ));
    }
    if subject
        .get("canonical_finding_index_entry_count")
        .and_then(Value::as_u64)
        != subject
            .get("canonical_finding_index")
            .and_then(|value| value.get("entries"))
            .and_then(Value::as_array)
            .map(|entries| entries.len() as u64)
    {
        violations.push(format!(
            "{PR_CHECK_SUBJECT_JSON} canonical finding index entry count is contradictory"
        ));
    }
    let index = subject
        .get("canonical_finding_index")
        .cloned()
        .and_then(|value| {
            serde_json::from_value::<ripr::review_input::CanonicalFindingIndexV1>(value).ok()
        });
    let Some(index) = index else {
        violations.push(format!(
            "{PR_CHECK_SUBJECT_JSON} canonical_finding_index is missing or malformed"
        ));
        return violations;
    };
    if let Err(error) = ripr::review_input::canonical_projection_from_index(&index) {
        violations.push(format!(
            "{PR_CHECK_SUBJECT_JSON} canonical finding index invalid: {error}"
        ));
    }
    let review_path = repo.join(PR_REVIEW_INPUT_JSON);
    match fs::read(&review_path) {
        Ok(review_bytes) => {
            let actual = format!("sha256:{:x}", Sha256::digest(&review_bytes));
            if subject.get("review_input_sha256").and_then(Value::as_str) != Some(actual.as_str()) {
                violations.push(format!(
                    "{PR_CHECK_SUBJECT_JSON} review_input_sha256 does not match review-input.json"
                ));
            }
            if subject
                .get("review_input_byte_count")
                .and_then(Value::as_u64)
                != Some(review_bytes.len() as u64)
            {
                violations.push(format!(
                    "{PR_CHECK_SUBJECT_JSON} review_input_byte_count does not match review-input.json"
                ));
            }
            match serde_json::from_slice::<ripr::review_input::ReviewInputV1>(&review_bytes) {
                Ok(review) => {
                    if let Ok(expected_projection) =
                        ripr::review_input::canonical_projection_from_index(&index)
                        && review.findings != expected_projection
                    {
                        violations.push(format!(
                            "{PR_REVIEW_INPUT_JSON} is not derived from the canonical finding index"
                        ));
                    }
                }
                Err(error) => {
                    violations.push(format!("{PR_REVIEW_INPUT_JSON} is invalid: {error}"))
                }
            }
        }
        Err(error) => violations.push(format!(
            "missing or unreadable {PR_REVIEW_INPUT_JSON}: {error}"
        )),
    }
    violations
}

fn digest_file(path: &Path) -> Result<(String, u64), String> {
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(file);
    let mut digest = Sha256::new();
    let mut byte_count = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("read {} failed: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        byte_count = byte_count
            .checked_add(read as u64)
            .ok_or_else(|| format!("byte count overflow for {}", path.display()))?;
    }
    Ok((format!("sha256:{:x}", digest.finalize()), byte_count))
}

fn verify_revision(repo: &Path, rev: &str) -> Result<(), String> {
    let commit = format!("{rev}^{{commit}}");
    run_git_output(repo, &["rev-parse", "--verify", commit.as_str()])
        .map(|_| ())
        .map_err(|err| format!("bad base/head revision {rev:?}: {err}"))
}

fn resolve_revision(repo: &Path, rev: &str, object_kind: &str) -> Result<String, String> {
    let object = format!("{rev}^{{{object_kind}}}");
    run_git_output(repo, &["rev-parse", "--verify", object.as_str()])
        .map(|value| value.trim().to_string())
        .and_then(|value| {
            if value.is_empty() {
                Err(format!(
                    "resolved {object_kind} identity for {rev:?} is empty"
                ))
            } else {
                Ok(value)
            }
        })
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
    let diff = run_git_output(
        repo,
        &["diff", "--unified=0", "--no-ext-diff", range.as_str()],
    )?;
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
        "--no-unchanged-tests".to_string(),
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
    let mut diagnostic = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("\n");
    if diagnostic.is_empty() {
        diagnostic = "RIPR PR evidence generation did not complete.".to_string();
    }
    diagnostic.truncate(4096);
    diagnostic
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
    use ripr::review_input::projection_summary;

    fn options() -> PrEvidenceOptions {
        PrEvidenceOptions {
            root: ".".to_string(),
            base: "origin/main".to_string(),
            head: "HEAD".to_string(),
            check: false,
        }
    }

    #[test]
    fn projection_summary_is_non_empty_for_renderer_admission() {
        assert_eq!(
            projection_summary(&json!({"suggested_next_action": "  "})),
            "Inspect the producer-owned review finding."
        );
        assert_eq!(
            projection_summary(&json!({
                "suggested_next_action": "",
                "recommended_next_step": "Strengthen the assertion."
            })),
            "Strengthen the assertion."
        );
        assert_eq!(
            projection_summary(&json!({"suggested_next_action": "Escalate."})),
            "Escalate."
        );
    }

    #[test]
    fn producer_review_input_projects_and_prioritizes_findings() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-review-input-projection")?;
        write_repo_file(&repo, "src/lib.rs", "pub fn value() -> u8 { 1 }\n")?;
        let mut findings = Vec::new();
        for index in 0..12 {
            let severity = match index % 4 {
                0 => "note",
                1 => "warning",
                2 => "error",
                _ => "critical",
            };
            findings.push(json!({
                "id": format!("finding-{index}"),
                "severity": severity,
                "classification": "exposed",
                "probe": {"file": "src/lib.rs", "line": index + 1},
                "suggested_next_action": if index == 0 { "  " } else { "Act." },
                "recommended_next_step": if index == 0 { "Fallback." } else { "" },
                "related_tests": [{"name": "value_is_one", "file": "src/lib.rs", "line": 1}],
            }));
        }
        let check = json!({
            "mode": "draft",
            "findings": findings,
            "analysis_outcome": {"analysis_complete": true}
        });
        let subject = json!({
            "base_sha": "base",
            "head_sha": "head",
            "head_tree": "tree",
            "check_sha256": "check"
        });
        fs::create_dir_all(repo.join("target/ripr/pr"))
            .map_err(|err| format!("create review input directory: {err}"))?;
        fs::write(repo.join(PR_DIFF), "diff --git a/src/lib.rs b/src/lib.rs\n")
            .map_err(|err| format!("write canonical diff: {err}"))?;
        let projected = producer_review_input(&check, &repo, &options(), &subject)?;
        assert_eq!(projected["total_finding_count"], 12);
        assert_eq!(projected["projected_finding_count"], 10);
        assert_eq!(projected["reviewed_count"], 10);
        assert_eq!(projected["projection_truncated"], true);
        assert_eq!(projected["findings"][0]["severity"], "critical");
        assert_eq!(
            projected["findings"][0]["related_test"]["name"],
            "value_is_one"
        );
        assert!(projected["findings"].as_array().is_some_and(|findings| {
            findings
                .iter()
                .any(|finding| finding["summary"] == "Fallback.")
        }));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
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
        let error = write_pr_evidence_with_runner(&repo, &options, |_repo, _options| {
            Err("ripr check for PR evidence timed out after 120 seconds; retry command: cargo xtask ripr-pr --base HEAD~1 --head HEAD --root .".to_string())
        })
        .err()
        .ok_or_else(|| "producer failure must fail the command after writing its error packet".to_string())?;
        assert!(error.contains("timed out after 120 seconds"));
        check_pr_evidence(&repo, &options)?;

        let packet_text = fs::read_to_string(repo.join(PR_EVIDENCE_JSON))
            .map_err(|err| format!("read packet: {err}"))?;
        let packet: Value =
            serde_json::from_str(&packet_text).map_err(|err| format!("parse packet: {err}"))?;
        assert_eq!(packet["status"], "error");
        assert_eq!(packet["warnings"][0]["kind"], "tool_error");
        assert!(
            packet["warnings"][0]["message"]
                .as_str()
                .is_some_and(|message| message.contains("timed out after 120 seconds"))
        );
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
        let subject_text = fs::read_to_string(repo.join(PR_CHECK_SUBJECT_JSON))
            .map_err(|err| format!("read check subject receipt: {err}"))?;
        let subject: Value = serde_json::from_str(&subject_text)
            .map_err(|err| format!("parse check subject receipt: {err}"))?;
        assert_eq!(subject["schema_version"], "ripr.pr_check_subject.v1");
        assert_eq!(
            subject["base_sha"],
            resolve_revision(&repo, "HEAD~1", "commit")?
        );
        assert_eq!(
            subject["head_sha"],
            resolve_revision(&repo, "HEAD", "commit")?
        );
        assert_eq!(
            subject["head_tree"],
            resolve_revision(&repo, "HEAD", "tree")?
        );
        assert_eq!(
            subject["check_sha256"],
            format!("sha256:{:x}", Sha256::digest(check_text.as_bytes()))
        );
        assert!(check_value["findings"].is_array());
        assert_eq!(check_value["analysis_outcome"]["analysis_complete"], true);
        assert_eq!(
            check_value["analysis_outcome"]["outcome"]["kind"],
            "no_behavioral_candidates"
        );
        let review_input_text = fs::read_to_string(repo.join(PR_REVIEW_INPUT_JSON))
            .map_err(|err| format!("read review input: {err}"))?;
        let review_input: Value = serde_json::from_str(&review_input_text)
            .map_err(|err| format!("parse review input: {err}"))?;
        let diff_bytes =
            fs::read(repo.join(PR_DIFF)).map_err(|err| format!("read canonical diff: {err}"))?;
        assert_eq!(
            review_input["canonical_diff_sha256"],
            format!("sha256:{:x}", Sha256::digest(diff_bytes))
        );
        assert_eq!(review_input["mode"], "draft");

        run_git(
            &repo,
            &[
                "commit",
                "--amend",
                "--no-gpg-sign",
                "-m",
                "amended same tree",
            ],
        )?;
        let stale_error = check_pr_evidence(&repo, &options)
            .err()
            .ok_or_else(|| "same-tree amended head must reject stale evidence".to_string())?;
        assert!(stale_error.contains("head_sha does not match"));

        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn pr_evidence_diff_matches_review_comments_input_contract() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-diff-contract")?;
        run_git(&repo, &["init"])?;
        run_git(&repo, &["config", "user.email", "ripr-pr@example.invalid"])?;
        run_git(&repo, &["config", "user.name", "RIPR PR Test"])?;
        write_repo_file(&repo, "src/lib.rs", "pub fn value() -> u8 { 1 }\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "initial"])?;
        write_repo_file(&repo, "src/lib.rs", "pub fn value() -> u8 { 2 }\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "change value"])?;

        let options = PrEvidenceOptions {
            base: "HEAD~1".to_string(),
            head: "HEAD".to_string(),
            ..options()
        };
        write_diff(&repo, &options)?;

        let actual = fs::read_to_string(repo.join(PR_DIFF))
            .map_err(|err| format!("read produced diff: {err}"))?;
        let expected = run_git_output(
            &repo,
            &["diff", "--unified=0", "--no-ext-diff", "HEAD~1...HEAD"],
        )?;
        assert!(!actual.is_empty());
        assert_eq!(actual, expected);

        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

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
