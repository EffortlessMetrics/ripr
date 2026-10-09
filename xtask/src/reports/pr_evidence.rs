use super::pr_causal_delta::write_canonical_delta;
use super::write_parented_file;
use crate::run::{
    capture_output_with_timeout, capture_process_output, run_output_owned,
    run_output_owned_with_timeout, tool_build_timeout,
};
use ripr::review_input::{
    REVIEW_INPUT_PROJECTION_LIMIT, REVIEW_INPUT_SCHEMA_VERSION, REVIEW_INPUT_SELECTION_POLICY,
    REVIEW_INPUT_SELECTION_POLICY_VERSION, ReviewInputV1, canonical_finding_index,
    canonical_projection,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
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
const PR_CANONICAL_DIFF: &str = "target/ripr/pr/check.diff";
const REVIEW_INPUT_MAX_BYTES: usize = 128 * 1024;
const DEFAULT_TOOL_TIMEOUT_SECS: u64 = 120;
const PR_EVIDENCE_TIMEOUT_ENV: &str = "RIPR_PR_EVIDENCE_TIMEOUT_SECS";
const DIFF_SCOPE_OVERSIZED_GUARD: &str = "diff_scope_oversized:";
const CHILD_DIAGNOSTIC_MAX_CHARS: usize = 2048;
const CHILD_DIAGNOSTIC_MAX_LINES: usize = 12;
const TOOL_ERROR_MAX_CHARS: usize = 4096;
const TOOL_ERROR_MAX_LINES: usize = 24;
const TRUNCATED_MARKER: &str = "… [truncated]";

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
            reject_error_packet(repo)
        }
    }
}

#[cfg(test)]
fn write_pr_evidence_from_check_json(
    repo: &Path,
    options: &PrEvidenceOptions,
    check_json: &str,
) -> Result<(), String> {
    remove_stale_check_artifact(repo)?;
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
    let root = repo
        .join(&options.root)
        .canonicalize()
        .map_err(|err| format!("resolve review input root failed: {err}"))?;
    let config = ripr::config::load_for_root(&root)
        .map_err(|err| format!("load review input config: {err}"))?;
    let canonical_diff = fs::read(repo.join(PR_CANONICAL_DIFF))
        .map_err(|err| format!("read canonical diff for check subject binding: {err}"))?;
    let mut subject = json!({
        "schema_version": "ripr.pr_check_subject.v1",
        "root_identity": ripr::review_input::canonical_root_identity(&root),
        "base_sha": resolve_revision(repo, &options.base, "commit")?,
        "head_sha": resolve_revision(repo, &options.head, "commit")?,
        "head_tree": resolve_revision(repo, &options.head, "tree")?,
        "check_sha256": format!("sha256:{:x}", Sha256::digest(check_json_text.as_bytes())),
        "check_byte_count": check_json_text.len(),
        "check_schema": check_value.get("schema_version").cloned().unwrap_or(Value::Null),
        "mode": check_value.get("mode").cloned().unwrap_or(Value::Null),
        "canonical_diff_sha256": format!("sha256:{:x}", Sha256::digest(&canonical_diff)),
        "configuration_fingerprint": ripr::config::repo_exposure_config_identity_hash(&config),
        "analyzer_generation": ripr::review_input::REVIEW_ANALYZER_GENERATION,
        "analysis_outcome": check_value.get("analysis_outcome").cloned().unwrap_or(Value::Null),
    });
    let findings = check_value
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| "ripr check output findings must be an array".to_string())?;
    let (index, index_byte_count) = canonical_finding_index(findings, &root)?;
    subject["canonical_finding_index"] = serde_json::to_value(index)
        .map_err(|error| format!("serialize canonical finding index: {error}"))?;
    subject["canonical_finding_index_entry_count"] = json!(findings.len());
    subject["canonical_finding_index_byte_count"] = json!(index_byte_count);
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
    for field in [
        "projected_finding_count",
        "projection_limit",
        "projection_truncated",
        "projection_selection_policy",
        "projection_selection_policy_version",
    ] {
        subject[field] = review_input[field].clone();
    }
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

    // The saved status check is fallible, so it must precede authority.
    reject_error_packet(repo)?;
    println!("Wrote {PR_EVIDENCE_JSON}");
    println!("Wrote {PR_EVIDENCE_MD}");
    // Commit admission authority only after every fallible producer operation.
    publish_check_subject(repo, &subject_text)
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
    let canonical_diff = fs::read(repo.join(PR_CANONICAL_DIFF))
        .map_err(|err| format!("read canonical diff for review input binding: {err}"))?;
    if projection_bytes.len() > REVIEW_INPUT_MAX_BYTES {
        return Err(format!(
            "producer review input exceeds {REVIEW_INPUT_MAX_BYTES} byte limit"
        ));
    }
    let input = json!({
        "schema_version": REVIEW_INPUT_SCHEMA_VERSION,
        "mode": check["mode"],
        "root_identity": ripr::review_input::canonical_root_identity(&root),
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
    // The subject is admission authority. Remove it before touching subordinate
    // artifacts or attempting setup/analysis; only missing files are harmless.
    for relative in [PR_CHECK_SUBJECT_JSON, PR_CHECK_JSON, PR_REVIEW_INPUT_JSON] {
        match fs::remove_file(repo.join(relative)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("remove stale {relative} failed: {error}")),
        }
    }
    Ok(())
}

fn publish_check_subject(repo: &Path, contents: &str) -> Result<(), String> {
    use std::io::Write;

    static STAGE_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    let destination = repo.join(PR_CHECK_SUBJECT_JSON);
    let parent = destination.parent().ok_or("check subject has no parent")?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("stage check subject clock: {error}"))?
        .as_nanos();
    let sequence = STAGE_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let stage = parent.join(format!(
        ".ripr-pr-subject-{}-{nanos}-{sequence}.tmp",
        std::process::id()
    ));
    // Exclusive creation cannot truncate an existing file or follow its link.
    // An open failure leaves that unowned path untouched.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&stage)
        .map_err(|error| format!("create staged check subject failed: {error}"))?;
    let result = (|| {
        file.write_all(contents.as_bytes())
            .map_err(|error| format!("write staged check subject failed: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("sync staged check subject failed: {error}"))?;
        drop(file);
        fs::rename(&stage, &destination)
            .map_err(|error| format!("failed to finalize {PR_CHECK_SUBJECT_JSON}: {error}"))
    })();
    if result.is_err() {
        // Only this invocation's exclusively created stage is ours to remove.
        // Preserve the primary failure if cleanup also fails.
        let _ = fs::remove_file(&stage);
    }
    result
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
            "root_identity",
            ripr::review_input::canonical_root_identity(&repo.join(&options.root)),
        ),
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
    match digest_file(&repo.join(PR_CANONICAL_DIFF)) {
        Ok((digest, _)) => {
            if subject.get("canonical_diff_sha256").and_then(Value::as_str) != Some(digest.as_str())
            {
                violations.push(format!(
                    "{PR_CHECK_SUBJECT_JSON} canonical_diff_sha256 does not match {PR_CANONICAL_DIFF}"
                ));
            }
            match load_canonical_check_diff(repo, options) {
                Ok(expected) => {
                    if digest != format!("sha256:{:x}", Sha256::digest(expected.as_bytes())) {
                        violations.push(format!(
                            "{PR_CANONICAL_DIFF} does not match the requested canonical base/head diff"
                        ));
                    }
                }
                Err(error) => {
                    violations.push(format!("cannot reconstruct {PR_CANONICAL_DIFF}: {error}"))
                }
            }
        }
        Err(error) => violations.push(format!(
            "missing or unreadable {PR_CANONICAL_DIFF}: {error}"
        )),
    }
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
                    if Some(review.root_identity.as_str())
                        != subject.get("root_identity").and_then(Value::as_str)
                    {
                        violations.push(format!(
                            "{PR_REVIEW_INPUT_JSON} root_identity does not match the current PR evidence subject"
                        ));
                    }
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
    // not change the packet presentation. The assembly pins `-c
    // core.quotePath=true`, `--no-ext-diff`, `--no-textconv`, `--no-color`,
    // `--src-prefix=a/`, `--dst-prefix=b/`, `--binary`, and three-context
    // presentation.
    let diff = ripr::analysis::load_pr_evidence_diff_range(repo, &options.base, &options.head)?;
    write_parented_file(&out, PR_DIFF, diff)?;
    let canonical = load_canonical_check_diff(repo, options)?;
    write_parented_file(&repo.join(PR_CANONICAL_DIFF), PR_CANONICAL_DIFF, canonical)
}

fn load_canonical_check_diff(repo: &Path, options: &PrEvidenceOptions) -> Result<String, String> {
    ripr::analysis::load_canonical_pr_evidence_diff_range(repo, &options.base, &options.head)
}

fn run_ripr_check(repo: &Path, options: &PrEvidenceOptions) -> Result<String, String> {
    let diff_path = repo.join(PR_CANONICAL_DIFF);
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
        // Review admission requires the complete finding set. The interactive
        // JSON default may retain only a disclosed prefix; never reuse that
        // bounded document as complete producer evidence. Keep timed capture
        // and the canonical-index/projection bounds unchanged.
        &[("RIPR_CHECK_FINDINGS_BYTES", "0")],
        timeout,
        "ripr check for PR evidence",
    )?;
    if output.timed_out {
        let (shell, label) = native_retry_shell();
        return Err(format!(
            "ripr check for PR evidence timed out after {} seconds; retry command ({label}): {}",
            timeout.as_secs(),
            pr_evidence_retry_command(options, shell)
        ));
    }
    if output.status.is_some_and(|status| status.success()) {
        Ok(output.stdout)
    } else {
        Err(format_ripr_check_child_failure(
            output.status,
            &output.stdout,
            &output.stderr,
        ))
    }
}

fn format_ripr_check_child_failure(
    status: Option<ExitStatus>,
    stdout: &str,
    stderr: &str,
) -> String {
    let status = describe_native_status(status);
    let child = actionable_child_reason(stdout, stderr);
    if child.is_empty() {
        format!("ripr check for PR evidence failed ({status})")
    } else {
        format!("ripr check for PR evidence failed ({status})\n{child}")
    }
}

fn describe_native_status(status: Option<ExitStatus>) -> String {
    let Some(status) = status else {
        return "native status: unknown".to_string();
    };
    if let Some(code) = status.code() {
        return format!("native status: exit {code}");
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return format!("native status: signal {signal}");
        }
    }
    "native status: unknown".to_string()
}

fn actionable_child_reason(stdout: &str, stderr: &str) -> String {
    let stderr = stderr.trim();
    if !stderr.is_empty() {
        return bounded_text(
            &prefer_named_guard(stderr),
            CHILD_DIAGNOSTIC_MAX_CHARS,
            CHILD_DIAGNOSTIC_MAX_LINES,
        );
    }
    let stdout = stdout.trim();
    if stdout.is_empty() || looks_like_json_payload(stdout) {
        return String::new();
    }
    bounded_text(
        &prefer_named_guard(stdout),
        CHILD_DIAGNOSTIC_MAX_CHARS,
        CHILD_DIAGNOSTIC_MAX_LINES,
    )
}

fn looks_like_json_payload(text: &str) -> bool {
    matches!(text.trim_start().as_bytes().first(), Some(b'{' | b'['))
}

fn prefer_named_guard(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if let Some(index) = lines.iter().position(|line| is_named_guard_line(line)) {
        lines[index..].join("\n")
    } else {
        text.to_string()
    }
}

fn is_named_guard_line(line: &str) -> bool {
    let trimmed = line.trim();
    let without_reporter = trimmed.strip_prefix("ripr: ").unwrap_or(trimmed);
    without_reporter.starts_with(DIFF_SCOPE_OVERSIZED_GUARD)
}

fn bounded_text(text: &str, max_chars: usize, max_lines: usize) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut lines = Vec::new();
    let mut truncated = false;
    for (index, line) in trimmed.lines().enumerate() {
        if index >= max_lines {
            truncated = true;
            break;
        }
        lines.push(line);
    }
    let mut joined = lines.join("\n");
    if joined.chars().count() > max_chars {
        truncated = true;
        joined = joined.chars().take(max_chars).collect();
        joined = joined.trim_end().to_string();
    }
    if truncated {
        if joined.is_empty() {
            TRUNCATED_MARKER.to_string()
        } else {
            format!("{joined}{TRUNCATED_MARKER}")
        }
    } else {
        joined
    }
}

fn bounded_tool_error_message(error: &str) -> String {
    let trimmed = error.trim();
    if trimmed.is_empty() {
        return "RIPR PR evidence generation did not complete.".to_string();
    }
    let bounded = bounded_text(trimmed, TOOL_ERROR_MAX_CHARS, TOOL_ERROR_MAX_LINES);
    if bounded.is_empty() {
        "RIPR PR evidence generation did not complete.".to_string()
    } else {
        bounded
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

#[derive(Clone, Copy)]
enum RetryShell {
    Bash,
    PowerShell,
}

fn native_retry_shell() -> (RetryShell, &'static str) {
    if cfg!(windows) {
        (RetryShell::PowerShell, "PowerShell")
    } else {
        (RetryShell::Bash, "Bash")
    }
}

fn pr_evidence_retry_command(options: &PrEvidenceOptions, shell: RetryShell) -> String {
    let quote = |value: &str| match shell {
        RetryShell::Bash => bash_retry_arg(value),
        RetryShell::PowerShell => powershell_retry_arg(value),
    };
    format!(
        "cargo xtask ripr-pr --base {} --head {} --root {}",
        quote(&options.base),
        quote(&options.head),
        quote(&options.root)
    )
}

// Match the product command renderer's safe bare set; Bash otherwise needs
// close-escape-reopen, while PowerShell doubles quotes inside a literal.
fn retry_arg_is_plain(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '/' | '_' | '-' | ':'))
}

fn bash_retry_arg(value: &str) -> String {
    if retry_arg_is_plain(value) {
        value.to_string()
    } else if value.contains('\n') || value.contains('\r') {
        // `first_line` retains only one physical warning line in the packet.
        // ANSI-C quoting keeps valid Unix roots with newlines copyable while
        // still making backslashes and apostrophes literal Bash data.
        let mut escaped = String::new();
        for ch in value.chars() {
            match ch {
                '\\' => escaped.push_str("\\\\"),
                '\'' => escaped.push_str("\\'"),
                '\n' => escaped.push_str("\\n"),
                '\r' => escaped.push_str("\\r"),
                _ => escaped.push(ch),
            }
        }
        format!("$'{escaped}'")
    } else {
        format!("'{}'", value.replace('\'', r"'\''"))
    }
}

fn powershell_retry_arg(value: &str) -> String {
    if retry_arg_is_plain(value) {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "''"))
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

/// Keep the legacy diagnostic entry point on the live packet path while using
/// the bounded UTF-8-safe formatter and its explicit truncation marker.
fn first_line(text: &str) -> String {
    bounded_tool_error_message(text)
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
                md_warning(string_field(
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

fn md_warning(value: &str) -> String {
    // Warnings are list prose, not table cells. A retry must be code: Markdown
    // consumes backslashes before punctuation and interprets entities, tags,
    // and emphasis in ordinary prose, changing copied shell arguments.
    let flattened = value.replace(['\r', '\n'], " ");
    if flattened.starts_with("ripr check for PR evidence timed out after ")
        && let Some((context, tail)) = flattened.split_once("; retry command (")
        && let Some((shell, command)) = tail.split_once("): ")
        && matches!(shell, "Bash" | "PowerShell")
    {
        return format!(
            "{context}; retry command ({shell}): {}",
            md_code_span(command)
        );
    }
    flattened
}

fn md_code_span(command: &str) -> String {
    let mut run = 0usize;
    let mut longest = 0usize;
    for ch in command.chars() {
        if ch == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let delimiter = "`".repeat(longest + 1);
    format!("{delimiter}{command}{delimiter}")
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
    use ripr::review_input::{REVIEW_INDEX_MAX_BYTES, REVIEW_INDEX_MAX_ENTRIES};


    const CONFIGURATION_A: &str = "[analysis]\ninclude_unchanged_tests = true\n";
    const CONFIGURATION_B: &str = "[analysis]\ninclude_unchanged_tests = false\n";

    fn with_configuration_fixture(
        name: &str,
        initial_config: Option<&str>,
        test: impl FnOnce(
            &Path,
            &PrEvidenceOptions,
            &str,
            &dyn Fn(&Path, &PrEvidenceOptions) -> Result<String, String>,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        let _cwd_guard = crate::acquire_test_cwd_read_guard();
        crate::reports::fixtures::ripr_fixture_binary()?;
        let binary = built_ripr_binary_path(&repo_root()?)?.display().to_string();
        let repo = temp_repo(name)?;
        let result = (|| {
            run_git(
                &repo,
                &["-c", "init.templateDir=", "init", "--quiet", "-b", "trunk"],
            )?;
            run_git(
                &repo,
                &["config", "user.email", "config-fixture@example.invalid"],
            )?;
            run_git(&repo, &["config", "user.name", "RIPR Config Fixture"])?;
            run_git(&repo, &["config", "commit.gpgSign", "false"])?;
            write_repo_file(&repo, ".gitignore", "target/\n")?;
            write_repo_file(
                &repo,
                "Cargo.toml",
                "[package]\nname = \"config-probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            )?;
            if let Some(config) = initial_config {
                write_repo_file(&repo, "ripr.toml", config)?;
            }
            write_repo_file(
                &repo,
                "src/lib.rs",
                "pub fn eligible(value: i32) -> bool { value > 0 }\n",
            )?;
            // This unchanged evidence is present in the whole head. Preserve
            // each adapter's existing choice about selecting it.
            write_repo_file(
                &repo,
                "tests/eligible.rs",
                "#[test]\nfn boundary() { assert!(!config_probe::eligible(1)); }\n",
            )?;
            run_git(&repo, &["add", "-A"])?;
            run_git(&repo, &["commit", "--quiet", "-m", "base"])?;
            write_repo_file(
                &repo,
                "src/lib.rs",
                "pub fn eligible(value: i32) -> bool { value > 1 }\n",
            )?;
            run_git(&repo, &["commit", "--quiet", "-a", "-m", "predicate"])?;
            let options = PrEvidenceOptions {
                base: resolve_revision(&repo, "HEAD~1", "commit")?,
                head: resolve_revision(&repo, "HEAD", "commit")?,
                ..options()
            };
            let run_check = |repo: &Path, options: &PrEvidenceOptions| {
                run_ripr_check_binary(
                    &binary,
                    vec![
                        "check".into(),
                        "--root".into(),
                        command_root_arg(repo, &options.root),
                        "--base".into(),
                        options.base.clone(),
                        "--diff".into(),
                        repo.join(PR_CANONICAL_DIFF).display().to_string(),
                        "--no-unchanged-tests".into(),
                        "--format".into(),
                        "json".into(),
                    ],
                    options,
                    Duration::from_mins(2),
                )
            };
            write_pr_evidence_with_runner(&repo, &options, |repo, options| {
                run_check(repo, options)
            })?;
            let check = fs::read_to_string(repo.join(PR_CHECK_JSON))
                .map_err(|error| error.to_string())?;
            let value: Value = serde_json::from_str(&check).map_err(|error| error.to_string())?;
            if value
                .pointer("/analysis_outcome/analysis_complete")
                .and_then(Value::as_bool)
                != Some(true)
                || value
                    .get("findings")
                    .and_then(Value::as_array)
                    .is_none_or(Vec::is_empty)
            {
                return Err("configuration fixture must execute complete nonempty analysis".into());
            }
            check_pr_evidence(&repo, &options)?;
            configuration_fixture_review(&repo, &options)?;
            test(&repo, &options, &check, &run_check)
        })();
        let cleanup = fs::remove_dir_all(&repo)
            .map_err(|error| format!("cleanup {}: {error}", repo.display()));
        result.and(cleanup)
    }

    fn configuration_fixture_review(
        repo: &Path,
        options: &PrEvidenceOptions,
    ) -> Result<(), String> {
        ripr::cli::run(vec![
            "ripr".into(),
            "review-comments".into(),
            "--root".into(),
            repo.display().to_string(),
            "--base".into(),
            options.base.clone(),
            "--head".into(),
            options.head.clone(),
            "--check-output".into(),
            repo.join(PR_CHECK_JSON).display().to_string(),
            "--out".into(),
            repo.join("target/config-review.json").display().to_string(),
        ])
        .map_err(|error| error.message().to_string())
    }

    #[test]
    fn producer_configuration_drift_is_rejected_after_actual_check() -> Result<(), String> {
        with_configuration_fixture(
            "ripr-pr-config-drift",
            Some(CONFIGURATION_A),
            |repo, options, check, run_check| {
                let first: Value =
                    serde_json::from_str(check).map_err(|error| error.to_string())?;
                let recorded = first
                    .pointer("/analysis_outcome/outcome/identity/config_identity")
                    .and_then(Value::as_str)
                    .ok_or("actual configured check must carry a text identity")?;
                let config_a = ripr::config::load_for_root(repo)?;
                write_repo_file(repo, "ripr.toml", CONFIGURATION_B)?;
                let config_b = ripr::config::load_for_root(repo)?;
                if ripr::config::repo_exposure_config_identity_hash(&config_a)
                    != ripr::config::repo_exposure_config_identity_hash(&config_b)
                {
                    return Err("fixture must preserve the seven-field seam fingerprint".into());
                }
                write_repo_file(repo, "ripr.toml", CONFIGURATION_A)?;
                let result = write_pr_evidence_with_runner(repo, options, |repo, options| {
                    let generated = run_check(repo, options)?;
                    let value: Value =
                        serde_json::from_str(&generated).map_err(|error| error.to_string())?;
                    if value
                        .pointer("/analysis_outcome/outcome/identity/config_identity")
                        .and_then(Value::as_str)
                        != Some(recorded)
                    {
                        return Err("fresh runner did not analyze configuration A".into());
                    }
                    // Change only live configuration after this actual check,
                    // before the existing packet writer reloads it.
                    write_repo_file(repo, "ripr.toml", CONFIGURATION_B)?;
                    Ok(generated)
                });
                let failure = result.err().ok_or_else(|| {
                    format!("configuration drift published old analysis {recorded}")
                })?;
                if !failure.contains("config_identity") || repo.join(PR_CHECK_SUBJECT_JSON).exists()
                {
                    return Err(format!(
                        "wrong drift refusal or retained authority: {failure}"
                    ));
                }
                let packet: Value = serde_json::from_slice(
                    &fs::read(repo.join(PR_EVIDENCE_JSON)).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                if packet["status"] != "error" {
                    return Err("configuration drift did not publish an error packet".into());
                }
                write_repo_file(repo, "ripr.toml", CONFIGURATION_A)?;
                write_pr_evidence_with_runner(repo, options, |_, _| Ok(check.to_string()))?;
                check_pr_evidence(repo, options)?;
                configuration_fixture_review(repo, options)
            },
        )
    }

    #[test]
    fn stale_configuration_is_rejected_by_saved_check_and_review() -> Result<(), String> {
        with_configuration_fixture(
            "ripr-pr-config-stale",
            Some(CONFIGURATION_A),
            |repo, options, _, _| {
                for config in [Some(CONFIGURATION_B), Some(""), None] {
                    match config {
                        Some(text) => write_repo_file(repo, "ripr.toml", text)?,
                        None => fs::remove_file(repo.join("ripr.toml"))
                            .map_err(|error| error.to_string())?,
                    }
                    let saved = check_pr_evidence(repo, options)
                        .err()
                        .ok_or("saved check admitted stale loaded configuration")?;
                    if !saved.contains("config_identity") {
                        return Err(format!("wrong saved configuration refusal: {saved}"));
                    }
                    let review = configuration_fixture_review(repo, options)
                        .err()
                        .ok_or("direct review admitted stale loaded configuration")?;
                    if !review.contains("config_identity") {
                        return Err(format!("wrong review configuration refusal: {review}"));
                    }
                    write_repo_file(repo, "ripr.toml", CONFIGURATION_A)?;
                    check_pr_evidence(repo, options)?;
                    configuration_fixture_review(repo, options)?;
                }
                Ok(())
            },
        )
    }

    #[test]
    fn copied_configuration_identity_requires_present_string_or_null() -> Result<(), String> {
        for initial in [Some(CONFIGURATION_A), Some(""), None] {
            with_configuration_fixture(
                "ripr-pr-config-shape",
                initial,
                |repo, options, _, _| {
                    let path = repo.join(PR_CHECK_SUBJECT_JSON);
                    let original = fs::read(&path).map_err(|error| error.to_string())?;
                    let subject: Value =
                        serde_json::from_slice(&original).map_err(|error| error.to_string())?;
                    let actual = subject
                        .pointer("/analysis_outcome/outcome/identity/config_identity")
                        .ok_or("actual producer did not carry config_identity")?;
                    if initial.is_some() != actual.is_string()
                        || initial.is_none() != actual.is_null()
                    {
                        return Err(
                            "loaded-empty config and no config lost their distinction".into(),
                        );
                    }
                    for wrong in [
                        None,
                        Some(json!(7)),
                        Some(json!(false)),
                        Some(json!([])),
                        Some(json!({})),
                        Some(json!("fnv1a64:foreign")),
                    ] {
                        let mut mutated = subject.clone();
                        let identity = mutated
                            .pointer_mut("/analysis_outcome/outcome/identity")
                            .and_then(Value::as_object_mut)
                            .ok_or("actual outcome identity is not an object")?;
                        match wrong {
                            Some(value) => {
                                identity.insert("config_identity".into(), value);
                            }
                            None => {
                                identity.remove("config_identity");
                            }
                        }
                        fs::write(
                            &path,
                            serde_json::to_vec(&mutated).map_err(|error| error.to_string())?,
                        )
                        .map_err(|error| error.to_string())?;
                        let saved = check_pr_evidence(repo, options)
                            .err()
                            .ok_or("saved check admitted missing or malformed config identity")?;
                        if !saved.contains("config_identity") {
                            return Err(format!("wrong saved shape refusal: {saved}"));
                        }
                        let review = configuration_fixture_review(repo, options)
                            .err()
                            .ok_or("review admitted missing or malformed config identity")?;
                        if !review.contains("config_identity") {
                            return Err(format!("wrong review shape refusal: {review}"));
                        }
                        fs::write(&path, &original).map_err(|error| error.to_string())?;
                        check_pr_evidence(repo, options)?;
                        configuration_fixture_review(repo, options)?;
                    }
                    Ok(())
                },
            )?;
        }
        Ok(())
    }

    fn options() -> PrEvidenceOptions {
        PrEvidenceOptions {
            root: ".".to_string(),
            base: "origin/main".to_string(),
            head: "HEAD".to_string(),
            check: false,
        }
    }

    const OVERSIZED_DIFF_CHILD: &str = "diff_scope_oversized: 16651 changed Rust lines across 18 Rust files exceed the RIPR_MAX_DIFF_CHANGED_RUST_LINES limit (10000); analysis was not run to protect runner memory before probe expansion. Repair route: reduce the diff scope, split the extraction PR, run a narrower diff, or raise the limit via RIPR_MAX_DIFF_CHANGED_RUST_LINES=<number>.";

    fn synthetic_exit_status(code: i32) -> ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            ExitStatus::from_raw(code << 8)
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            ExitStatus::from_raw(code.cast_unsigned())
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
        fs::write(
            repo.join(PR_CANONICAL_DIFF),
            "diff --git a/src/lib.rs b/src/lib.rs\n",
        )
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
    fn retry_keeps_default_arguments_and_regeneration_semantics() -> Result<(), String> {
        let defaults = options();
        let expected = "cargo xtask ripr-pr --base origin/main --head HEAD --root .";
        for shell in [RetryShell::Bash, RetryShell::PowerShell] {
            let actual = pr_evidence_retry_command(&defaults, shell);
            if actual != expected {
                return Err(format!("default retry changed: {actual:?}"));
            }
        }
        let check_mode = PrEvidenceOptions {
            check: true,
            ..defaults
        };
        let actual = pr_evidence_retry_command(&check_mode, RetryShell::Bash);
        if actual != expected {
            return Err(format!("check-mode recovery must regenerate: {actual:?}"));
        }
        Ok(())
    }

    fn hostile_retry_options() -> PrEvidenceOptions {
        PrEvidenceOptions {
            base: "topic$(touch-marker)".to_string(),
            head: "topic'|`name".to_string(),
            root: if cfg!(windows) {
                "root\\[dir]`a&b with spaces".to_string()
            } else {
                "root\\'$(touch-marker)<tag>*\nwith spaces".to_string()
            },
            check: false,
        }
    }

    fn expected_retry_args(options: &PrEvidenceOptions) -> Vec<String> {
        vec![
            "xtask".to_string(),
            "ripr-pr".to_string(),
            "--base".to_string(),
            options.base.clone(),
            "--head".to_string(),
            options.head.clone(),
            "--root".to_string(),
            options.root.clone(),
        ]
    }

    #[cfg(not(windows))]
    fn execute_retry_command(command: &str) -> Result<Vec<String>, String> {
        let script = format!(
            "touch-marker() {{ printf INJECTED; }}; cargo() {{ printf '%s\\0' \"$@\"; }}; {command}"
        );
        let output = std::process::Command::new("bash")
            .args(["-c", &script])
            .output()
            .map_err(|err| format!("execute Bash retry fixture: {err}"))?;
        if !output.status.success() {
            return Err(format!(
                "Bash retry failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        if output
            .stdout
            .windows(b"INJECTED".len())
            .any(|part| part == b"INJECTED")
        {
            return Err("Bash retry ran command substitution".to_string());
        }
        output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|arg| !arg.is_empty())
            .map(|arg| String::from_utf8(arg.to_vec()).map_err(|err| err.to_string()))
            .collect::<Result<_, _>>()
    }

    #[cfg(not(windows))]
    #[test]
    fn bash_retry_executes_with_literal_arguments() -> Result<(), String> {
        let options = hostile_retry_options();
        for revision in [&options.base, &options.head] {
            let status = std::process::Command::new("git")
                .args(["check-ref-format", "--branch", revision])
                .status()
                .map_err(|err| format!("validate hostile ref: {err}"))?;
            if !status.success() {
                return Err(format!("hostile fixture ref is not valid: {revision:?}"));
            }
        }
        let command = pr_evidence_retry_command(&options, RetryShell::Bash);
        let actual = execute_retry_command(&command)?;
        let expected = expected_retry_args(&options);
        if actual != expected {
            return Err(format!("Bash retry argv changed: {actual:?}"));
        }
        let raw = format!(
            "cargo xtask ripr-pr --base {} --head {} --root {}",
            options.base, options.head, options.root
        );
        if execute_retry_command(&raw).is_ok_and(|args| args == expected) {
            return Err("raw Bash interpolation unexpectedly preserved argv".to_string());
        }
        Ok(())
    }

    #[cfg(windows)]
    fn execute_retry_command(command: &str) -> Result<Vec<String>, String> {
        let script = format!(
            "function touch-marker {{ $global:marker = $true }}; function cargo {{ $global:seen = @($args) }}; {command}; if ($global:marker) {{ exit 71 }}; ConvertTo-Json -InputObject $global:seen -Compress"
        );
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
            .map_err(|err| format!("execute PowerShell retry fixture: {err}"))?;
        if !output.status.success() {
            return Err(format!(
                "PowerShell retry failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        serde_json::from_slice(&output.stdout)
            .map_err(|err| format!("parse PowerShell argv: {err}"))
    }

    #[cfg(windows)]
    #[test]
    fn powershell_retry_executes_with_literal_arguments() -> Result<(), String> {
        let options = hostile_retry_options();
        for revision in [&options.base, &options.head] {
            let status = std::process::Command::new("git")
                .args(["check-ref-format", "--branch", revision])
                .status()
                .map_err(|err| format!("validate hostile ref: {err}"))?;
            if !status.success() {
                return Err(format!("hostile fixture ref is not valid: {revision:?}"));
            }
        }
        let command = pr_evidence_retry_command(&options, RetryShell::PowerShell);
        let actual = execute_retry_command(&command)?;
        let expected = expected_retry_args(&options);
        if actual != expected {
            return Err(format!("PowerShell retry argv changed: {actual:?}"));
        }
        let raw = format!(
            "cargo xtask ripr-pr --base {} --head {} --root {}",
            options.base, options.head, options.root
        );
        if execute_retry_command(&raw).is_ok_and(|args| args == expected) {
            return Err("raw PowerShell interpolation unexpectedly preserved argv".to_string());
        }
        Ok(())
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
    fn error_packet_retains_multiline_child_failure_and_native_status() {
        let packet = pr_evidence_error_packet(
            &options(),
            &["src/lib.rs".to_string()],
            &(format_ripr_check_child_failure(
                Some(synthetic_exit_status(2)),
                "",
                OVERSIZED_DIFF_CHILD,
            )),
        );
        let message = packet["warnings"][0]["message"]
            .as_str()
            .unwrap_or_default();
        assert_eq!(packet["status"], "error");
        assert_eq!(packet["warnings"][0]["kind"], "tool_error");
        assert!(
            message.contains("native status: exit 2"),
            "packet dropped native status: {message}"
        );
        assert!(
            message.contains("diff_scope_oversized:"),
            "packet dropped the named guard: {message}"
        );
        assert!(
            message.contains("16651 changed Rust lines across 18 Rust files"),
            "packet dropped observed scope: {message}"
        );
        assert!(
            message.contains("RIPR_MAX_DIFF_CHANGED_RUST_LINES limit (10000)"),
            "packet dropped configured scope: {message}"
        );
        assert!(
            message.contains("Repair route: reduce the diff scope"),
            "packet dropped recovery text: {message}"
        );
        assert!(
            packet.get("analysis_scope").is_none(),
            "child prose must not become typed analysis authority"
        );
        let violations = validate_packet_value(&packet, &options(), 1, true);
        assert_eq!(violations, Vec::<String>::new());
        let console = ripr::reject_pr_evidence_error_packet(&packet).unwrap_or_default();
        assert!(console.contains("review-comments must not run"));
        assert!(console.contains("diff_scope_oversized:"));
        assert!(console.contains("native status: exit 2"));
        let markdown = render_pr_evidence_markdown(&packet);
        assert!(markdown.contains("diff_scope_oversized:"));
        assert!(markdown.contains("Repair route"));
    }

    #[test]
    fn error_packet_truncates_oversized_tool_error_explicitly() {
        let huge = format!(
            "ripr check for PR evidence failed (native status: exit 1)\n{}",
            "x".repeat(10_000)
        );
        let packet = pr_evidence_error_packet(&options(), &[], &huge);
        let message = packet["warnings"][0]["message"]
            .as_str()
            .unwrap_or_default();
        assert!(
            message.contains(TRUNCATED_MARKER),
            "oversized diagnostic must mark truncation: {message}"
        );
        assert!(
            message.chars().count() <= TOOL_ERROR_MAX_CHARS + TRUNCATED_MARKER.chars().count(),
            "truncated packet message still too large: {}",
            message.chars().count()
        );
        assert!(
            !message.contains(&"x".repeat(9000)),
            "packet dumped unlimited child output"
        );
        assert_eq!(packet["status"], "error");
    }

    #[test]
    fn error_packet_preserves_utf8_and_bounds_extra_diagnostic_lines() {
        let unicode = "€".repeat(TOOL_ERROR_MAX_CHARS + 1);
        let packet = pr_evidence_error_packet(&options(), &[], &unicode);
        let message = packet["warnings"][0]["message"]
            .as_str()
            .unwrap_or_default();
        assert_eq!(
            message,
            format!("{}{TRUNCATED_MARKER}", "€".repeat(TOOL_ERROR_MAX_CHARS)),
            "UTF-8 diagnostics must remain complete characters and mark truncation"
        );

        let lines = (0..=TOOL_ERROR_MAX_LINES)
            .map(|index| format!("diagnostic line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let packet = pr_evidence_error_packet(&options(), &[], &lines);
        let message = packet["warnings"][0]["message"]
            .as_str()
            .unwrap_or_default();
        assert!(message.contains(&format!("diagnostic line {}", TOOL_ERROR_MAX_LINES - 1)));
        assert!(!message.contains(&format!("diagnostic line {TOOL_ERROR_MAX_LINES}")));
        assert!(message.ends_with(TRUNCATED_MARKER));
        assert_eq!(packet["status"], "error");
    }

    #[cfg(unix)]
    #[test]
    fn native_status_reports_unix_signal_without_inventing_an_exit_code() {
        use std::os::unix::process::ExitStatusExt;
        let status = ExitStatus::from_raw(15);
        assert_eq!(
            describe_native_status(Some(status)),
            "native status: signal 15"
        );
        assert!(!describe_native_status(Some(status)).contains("exit "));
    }

    #[test]
    fn native_status_stays_unknown_when_process_status_is_missing() {
        assert_eq!(describe_native_status(None), "native status: unknown");
        let message = format_ripr_check_child_failure(None, "", "");
        assert!(message.contains("native status: unknown"));
        assert!(!message.contains("exit "));
        assert!(!message.contains("signal "));
        assert!(!message.contains("PATH="));
        assert!(!message.contains("RIPR_BIN"));
    }

    #[test]
    fn child_failure_prefers_named_guard_and_ignores_json_stdout() {
        let noisy = format!("noise before the guard\nripr: {OVERSIZED_DIFF_CHILD}\n");
        let reason = actionable_child_reason("{\"summary\":{\"weakly_exposed\":1}}", &noisy);
        assert!(reason.starts_with("ripr: diff_scope_oversized:"));
        assert!(!reason.contains("noise before the guard"));
        assert!(!reason.contains("weakly_exposed"));

        let empty_stderr = format_ripr_check_child_failure(
            Some(synthetic_exit_status(3)),
            "{\"summary\":{\"weakly_exposed\":1}}",
            "",
        );
        assert!(empty_stderr.contains("native status: exit 3"));
        assert!(
            !empty_stderr.contains("weakly_exposed"),
            "JSON stdout must not be copied into the diagnostic: {empty_stderr}"
        );
        assert!(
            !empty_stderr.contains("stdout:"),
            "empty stderr must not dump stdout labels: {empty_stderr}"
        );
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
        let options = hostile_retry_options();
        fs::create_dir_all(repo.join(&options.root))
            .map_err(|err| format!("create hostile root: {err}"))?;
        run_git(&repo, &["branch", &options.base, "HEAD~1"])?;
        // Windows cannot store `|` in a loose-ref filename even though Git
        // accepts it as a ref name. A packed ref exercises the real revision
        // consumer with the same valid hostile name on every host.
        let tip = run_git_output(&repo, &["rev-parse", "HEAD"])?;
        fs::write(
            repo.join(".git/packed-refs"),
            format!(
                "# pack-refs with: peeled fully-peeled sorted\n{} refs/heads/{}\n",
                tip.trim(),
                options.head
            ),
        )
        .map_err(|err| format!("write hostile packed ref: {err}"))?;
        run_git(&repo, &["rev-parse", "--verify", &options.head])?;
        write_parented_file(
            &repo.join(PR_CHECK_JSON),
            PR_CHECK_JSON,
            "{\"stale\":true}\n",
        )?;
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
            let fake =
                fake_ripr_invocation(&repo, "fake-ripr-packet-timeout", "", "", 0, Some(30))?;
            (fake.binary, fake.args)
        };
        let timeout = Duration::from_secs(1);
        let producer_result = write_pr_evidence_with_runner(&repo, &options, |_repo, options| {
            run_ripr_check_binary(&binary, args, options, timeout)
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
        assert!(
            packet["warnings"][0]["message"]
                .as_str()
                .is_some_and(|message| message
                    .contains(&format!("timed out after {} seconds", timeout.as_secs())))
        );
        let warning = packet["warnings"][0]["message"]
            .as_str()
            .ok_or_else(|| "missing timeout warning".to_string())?;
        let (shell, label) = native_retry_shell();
        let marker = format!("retry command ({label}): ");
        let warning_command = warning
            .split_once(&marker)
            .map(|(_, command)| command)
            .ok_or_else(|| format!("error packet lost retry guidance: {warning}"))?;
        let expected_command = pr_evidence_retry_command(&options, shell);
        if warning_command != expected_command {
            return Err(format!("error packet retry changed: {warning_command:?}"));
        }
        let markdown = fs::read_to_string(repo.join(PR_EVIDENCE_MD))
            .map_err(|err| format!("read Markdown packet: {err}"))?;
        let markdown_span = markdown
            .split_once(&marker)
            .and_then(|(_, tail)| tail.lines().next())
            .ok_or_else(|| "Markdown packet lost retry guidance".to_string())?;
        let delimiter_len = markdown_span
            .bytes()
            .take_while(|byte| *byte == b'`')
            .count();
        if delimiter_len == 0 || markdown_span.len() <= delimiter_len * 2 {
            return Err(format!(
                "Markdown retry is not a code span: {markdown_span:?}"
            ));
        }
        let delimiter = "`".repeat(delimiter_len);
        let markdown_command = markdown_span
            .strip_suffix(&delimiter)
            .and_then(|span| span.get(delimiter_len..))
            .ok_or_else(|| format!("Markdown retry span is malformed: {markdown_span:?}"))?;
        if markdown_command.contains(&delimiter) {
            return Err("Markdown retry delimiter collides with command data".to_string());
        }
        if markdown_command != expected_command {
            return Err(format!("Markdown retry changed: {markdown_command:?}"));
        }
        let expected_args = expected_retry_args(&options);
        for command in [warning_command, markdown_command] {
            let actual_args = execute_retry_command(command)?;
            if actual_args != expected_args {
                return Err(format!("packet retry argv changed: {actual_args:?}"));
            }
        }
        assert!(repo.join(PR_DIFF).exists());
        assert!(repo.join(PR_EVIDENCE_MD).exists());
        assert!(!repo.join(PR_CHECK_JSON).exists());
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn run_ripr_check_requests_complete_findings_from_actual_child() -> Result<(), String> {
        #[cfg(windows)]
        let (binary, args) = (
            "powershell".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                "[Console]::Out.Write($env:RIPR_CHECK_FINDINGS_BYTES)".to_string(),
            ],
        );
        #[cfg(not(windows))]
        let (binary, args) = (
            "/bin/sh".to_string(),
            vec![
                "-c".to_string(),
                "printf '%s' \"$RIPR_CHECK_FINDINGS_BYTES\"".to_string(),
            ],
        );
        let result = run_ripr_check_binary(&binary, args, &options(), Duration::from_secs(30))?;
        if result != "0" {
            return Err(format!(
                "PR evidence child inherited a bounded findings budget: {result:?}"
            ));
        }
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
        assert!(err.contains("native status: exit 7"));
        assert!(err.contains("bad diff"));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn run_ripr_check_reports_multiline_stderr_and_empty_stderr() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-fake-multiline")?;
        let fake = fake_ripr_invocation(
            &repo,
            "fake-ripr-multiline",
            "",
            "first noise\nneeded detail on a later line\n",
            2,
            None,
        )?;
        let err = match run_ripr_check_binary(
            &fake.binary,
            fake.args,
            &options(),
            Duration::from_secs(30),
        ) {
            Ok(output) => return Err(format!("multiline failure should fail, got {output}")),
            Err(err) => err,
        };
        assert!(err.contains("native status: exit 2"));
        assert!(err.contains("needed detail on a later line"));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;

        let repo = temp_repo("ripr-pr-fake-empty-stderr")?;
        let fake = fake_ripr_invocation(&repo, "fake-ripr-empty-stderr", "", "", 3, None)?;
        let err = match run_ripr_check_binary(
            &fake.binary,
            fake.args,
            &options(),
            Duration::from_secs(30),
        ) {
            Ok(output) => return Err(format!("empty-stderr failure should fail, got {output}")),
            Err(err) => err,
        };
        assert!(err.contains("native status: exit 3"));
        assert!(!err.contains("stdout:"));
        assert!(!err.contains("stderr:"));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn run_ripr_check_truncates_oversized_child_output() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-fake-oversized-output")?;
        let stderr = (0..80)
            .map(|index| format!("child line {index:02} {}", "y".repeat(60)))
            .collect::<Vec<_>>()
            .join("\n");
        let fake = fake_ripr_invocation(&repo, "fake-ripr-oversized-output", "", &stderr, 4, None)?;
        let err = match run_ripr_check_binary(
            &fake.binary,
            fake.args,
            &options(),
            Duration::from_secs(30),
        ) {
            Ok(output) => return Err(format!("oversized child should fail, got {output}")),
            Err(err) => err,
        };
        assert!(err.contains("native status: exit 4"));
        assert!(
            err.contains(TRUNCATED_MARKER),
            "oversized child output must mark truncation: {err}"
        );
        assert!(
            err.chars().count() < stderr.chars().count(),
            "bounded diagnostic was larger than the child output"
        );
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
        let (_, label) = native_retry_shell();
        if !err.contains(&format!("retry command ({label}): cargo xtask ripr-pr")) {
            return Err(format!("timeout retry guidance missing: {err}"));
        }
        if err.contains("snapshot timed out") {
            return Err(format!(
                "main-analysis timeout reused snapshot-timeout wording: {err}"
            ));
        }
        if err.contains("native status:") {
            return Err(format!(
                "timeout diagnostic must stay a timeout, not an exit/signal status: {err}"
            ));
        }
        #[cfg(not(windows))]
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn run_ripr_check_reports_signal_status_on_unix() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-fake-signal")?;
        let script = repo.join("fake-ripr-signal");
        fs::write(&script, "#!/bin/sh\nkill -s TERM $$\n")
            .map_err(|err| format!("write signal fake: {err}"))?;
        let err = match run_ripr_check_binary(
            "/bin/sh",
            vec![script.display().to_string()],
            &options(),
            Duration::from_secs(30),
        ) {
            Ok(output) => return Err(format!("signal fake should fail, got {output}")),
            Err(err) => err,
        };
        if !err.contains("native status: signal 15") {
            return Err(format!("signal diagnostic missing TERM/15: {err}"));
        }
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn write_pr_evidence_keeps_oversized_diff_child_in_error_packet() -> Result<(), String> {
        let (repo, options) = two_commit_repo("ripr-pr-child-guard")?;
        write_parented_file(
            &repo.join(PR_CHECK_JSON),
            PR_CHECK_JSON,
            "{\"stale\":true}\n",
        )?;
        let limited_json = r#"{"schema_version":"0.2","summary":{"weakly_exposed":0},"analysis_scope":{"run_status":"diff_scope_oversized","downstream_consumable":false}}"#;
        let fake = fake_ripr_invocation(
            &repo,
            "fake-ripr-oversized-diff",
            limited_json,
            &format!("warning noise\nripr: {OVERSIZED_DIFF_CHILD}\n"),
            2,
            None,
        )?;
        let binary = fake.binary;
        let args = fake.args;
        let producer_error =
            match write_pr_evidence_with_runner(&repo, &options, |_repo, options| {
                run_ripr_check_binary(&binary, args, options, Duration::from_secs(30))
            }) {
                Ok(()) => {
                    return Err("oversized-diff child unexpectedly produced evidence".to_string());
                }
                Err(err) => err,
            };
        assert!(producer_error.contains("review-comments must not run"));
        assert!(producer_error.contains("diff_scope_oversized:"));
        assert!(producer_error.contains("native status: exit 2"));
        assert!(producer_error.contains("16651 changed Rust lines across 18 Rust files"));
        assert!(producer_error.contains("RIPR_MAX_DIFF_CHANGED_RUST_LINES limit (10000)"));
        assert!(producer_error.contains("Repair route: reduce the diff scope"));

        let packet_text = fs::read_to_string(repo.join(PR_EVIDENCE_JSON))
            .map_err(|err| format!("read packet: {err}"))?;
        let packet: Value =
            serde_json::from_str(&packet_text).map_err(|err| format!("parse packet: {err}"))?;
        assert_eq!(packet["status"], "error");
        assert_eq!(packet["warnings"][0]["kind"], "tool_error");
        let warning = packet["warnings"][0]["message"]
            .as_str()
            .ok_or_else(|| "missing child-failure warning".to_string())?;
        if !warning.contains("diff_scope_oversized:") || !warning.contains("native status: exit 2")
        {
            return Err(format!("error packet lost child failure: {warning}"));
        }
        if warning.contains("weakly_exposed") {
            return Err("limited JSON stdout leaked into the error packet".to_string());
        }
        let markdown = fs::read_to_string(repo.join(PR_EVIDENCE_MD))
            .map_err(|err| format!("read Markdown packet: {err}"))?;
        if !markdown.contains("diff_scope_oversized:") || !markdown.contains("Repair route") {
            return Err(format!("Markdown packet lost child recovery: {markdown}"));
        }
        assert!(!repo.join(PR_CHECK_JSON).exists());
        assert!(repo.join(PR_DIFF).exists());
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
        let diff_bytes = fs::read(repo.join(PR_CANONICAL_DIFF))
            .map_err(|err| format!("read canonical diff: {err}"))?;
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
        write_repo_file(&repo, "src/lib.rs", "pub fn value() -> u8 {\n    1\n}\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "initial"])?;
        write_repo_file(&repo, "src/lib.rs", "pub fn value() -> u8 {\n    2\n}\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "change value"])?;

        let options = PrEvidenceOptions {
            base: "HEAD~1".to_string(),
            head: "HEAD".to_string(),
            ..options()
        };
        write_diff(&repo, &options)?;

        let actual = fs::read_to_string(repo.join(PR_CANONICAL_DIFF))
            .map_err(|err| format!("read produced diff: {err}"))?;
        let expected = run_git_output(
            &repo,
            &["diff", "--unified=0", "--no-ext-diff", "HEAD~1...HEAD"],
        )?;
        assert!(!actual.is_empty());
        assert_eq!(actual, expected);
        let presentation = fs::read_to_string(repo.join(PR_DIFF))
            .map_err(|err| format!("read presentation diff: {err}"))?;
        assert_ne!(actual, presentation);
        assert_eq!(
            presentation,
            ripr::analysis::load_pr_evidence_diff_range(&repo, &options.base, &options.head)?
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

    #[test]
    fn failed_producer_rerun_does_not_replay_same_subject() -> Result<(), String> {
        let _cwd_guard = crate::acquire_test_cwd_read_guard();
        crate::reports::fixtures::ripr_fixture_binary()?;
        let repo = temp_repo("ripr-pr-subject-transaction")?;
        let result = (|| {
            run_git(
                &repo,
                &["-c", "init.templateDir=", "init", "--quiet", "-b", "trunk"],
            )?;
            run_git(
                &repo,
                &["config", "user.email", "subject-fixture@example.invalid"],
            )?;
            run_git(&repo, &["config", "user.name", "RIPR Subject Fixture"])?;
            run_git(&repo, &["config", "commit.gpgSign", "false"])?;
            write_repo_file(&repo, ".gitignore", "target/\n")?;
            write_repo_file(
                &repo,
                "Cargo.toml",
                "[package]\nname = \"subject-probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            )?;
            write_repo_file(
                &repo,
                "src/lib.rs",
                "pub fn eligible(value: i32) -> bool { value > 0 }\n",
            )?;
            run_git(&repo, &["add", "-A"])?;
            run_git(&repo, &["commit", "--quiet", "-m", "base"])?;
            write_repo_file(
                &repo,
                "src/lib.rs",
                "pub fn eligible(value: i32) -> bool { value > 1 }\n",
            )?;
            run_git(&repo, &["commit", "--quiet", "-a", "-m", "predicate"])?;
            let options = PrEvidenceOptions {
                base: resolve_revision(&repo, "HEAD~1", "commit")?,
                head: resolve_revision(&repo, "HEAD", "commit")?,
                ..options()
            };
            let binary = built_ripr_binary_path(&repo_root()?)?.display().to_string();
            let mut check = String::new();
            write_pr_evidence_with_runner(&repo, &options, |repo, options| {
                let generated = run_ripr_check_binary(
                    &binary,
                    vec![
                        "check".into(),
                        "--root".into(),
                        command_root_arg(repo, &options.root),
                        "--base".into(),
                        options.base.clone(),
                        "--diff".into(),
                        repo.join(PR_CANONICAL_DIFF).display().to_string(),
                        "--no-unchanged-tests".into(),
                        "--format".into(),
                        "json".into(),
                    ],
                    options,
                    Duration::from_mins(2),
                )?;
                check.clone_from(&generated);
                Ok(generated)
            })?;
            let value: Value = serde_json::from_str(&check).map_err(|error| error.to_string())?;
            assert_eq!(value["schema_version"], "0.2");
            assert_eq!(value["analysis_outcome"]["analysis_complete"], true);
            assert!(
                value["findings"]
                    .as_array()
                    .is_some_and(|findings| !findings.is_empty())
            );
            let review = || {
                ripr::cli::run(vec![
                    "ripr".into(),
                    "review-comments".into(),
                    "--root".into(),
                    repo.display().to_string(),
                    "--base".into(),
                    options.base.clone(),
                    "--head".into(),
                    options.head.clone(),
                    "--check-output".into(),
                    repo.join(PR_CHECK_JSON).display().to_string(),
                    "--out".into(),
                    repo.join("target/review.json").display().to_string(),
                ])
                .map_err(|error| error.message().to_string())
            };
            let baseline = || -> Result<(), String> {
                write_pr_evidence_with_runner(&repo, &options, |_, _| Ok(check.clone()))?;
                check_pr_evidence(&repo, &options)?;
                review()?;
                let rendered: Value = serde_json::from_slice(
                    &fs::read(repo.join("target/review.json"))
                        .map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                assert_eq!(
                    rendered["analysis_scope"]["basis"],
                    "producer_check_projection"
                );
                assert!(
                    rendered["analysis_scope"]["classified_seams_considered"]
                        .as_u64()
                        .is_some_and(|count| count > 0)
                );
                Ok(())
            };
            let refused = |label: &str| -> Result<(), String> {
                let error = review()
                    .err()
                    .ok_or_else(|| format!("{label}: stale same-identity producer was admitted"))?;
                assert!(
                    error.contains("missing_producer") || error.contains("malformed_producer"),
                    "{label}: unexpected review refusal: {error}"
                );
                assert!(
                    check_pr_evidence(&repo, &options).is_err(),
                    "{label}: saved check admitted failure"
                );
                Ok(())
            };
            let mut oversized = value.clone();
            oversized["findings"] = Value::Array(vec![
                value["findings"][0].clone();
                REVIEW_INDEX_MAX_ENTRIES + 1
            ]);
            let oversized = serde_json::to_string(&oversized).map_err(|error| error.to_string())?;
            let mut excess_bytes = value.clone();
            excess_bytes["findings"] = Value::Array(
                (0..REVIEW_INDEX_MAX_ENTRIES)
                    .map(|i| {
                        let mut finding = value["findings"][0].clone();
                        finding["id"] = json!(format!("index-byte-{i:04}"));
                        finding["suggested_next_action"] = json!("x".repeat(600));
                        finding
                    })
                    .collect(),
            );
            let byte_findings = excess_bytes["findings"]
                .as_array()
                .ok_or_else(|| "byte fixture findings are missing".to_string())?;
            let entries = ripr::review_input::canonical_projection_all(byte_findings, &repo)?;
            let legacy = serde_json::to_vec(&entries).map_err(|error| error.to_string())?;
            assert!(legacy.len() > REVIEW_INDEX_MAX_BYTES);
            let selected = ripr::review_input::canonical_projection(byte_findings, &repo)?;
            assert!(
                serde_json::to_vec(&selected)
                    .map_err(|error| error.to_string())?
                    .len()
                    < REVIEW_INPUT_MAX_BYTES
            );
            let excess_bytes =
                serde_json::to_string(&excess_bytes).map_err(|error| error.to_string())?;
            let mut malformed_oversized = value.clone();
            malformed_oversized["findings"] =
                Value::Array(vec![Value::Null; REVIEW_INDEX_MAX_ENTRIES + 1]);
            let malformed_oversized =
                serde_json::to_string(&malformed_oversized).map_err(|error| error.to_string())?;
            for (label, replacement) in [
                ("runner failure", None),
                ("malformed conversion", Some("{")),
                ("oversized conversion", Some(oversized.as_str())),
                ("index byte limit", Some(excess_bytes.as_str())),
                (
                    "entry guard before projection",
                    Some(malformed_oversized.as_str()),
                ),
            ] {
                baseline()?;
                let failure = write_pr_evidence_with_runner(&repo, &options, |_, _| {
                    replacement.map_or_else(
                        || Err("injected runner failure".to_string()),
                        |text| Ok(text.to_string()),
                    )
                })
                .err()
                .ok_or_else(|| format!("{label}: producer returned success"))?;
                let expected = match label {
                    "runner failure" => "injected runner failure",
                    "malformed conversion" => "not valid JSON",
                    "oversized conversion" | "entry guard before projection" => {
                        "exceeds entry limit"
                    }
                    "index byte limit" => "exceeds byte limit",
                    _ => return Err(format!("unknown failure control: {label}")),
                };
                assert!(
                    failure.contains(expected),
                    "{label}: wrong failure: {failure}"
                );
                refused(label)?;
            }
            // This late failure distinguishes subject-last publication from
            // merely deleting a previous receipt before the runner.
            baseline()?;
            let markdown = repo.join(PR_EVIDENCE_MD);
            fs::remove_file(&markdown).map_err(|error| error.to_string())?;
            fs::create_dir(&markdown).map_err(|error| error.to_string())?;
            let failure = write_pr_evidence_with_runner(&repo, &options, |_, _| Ok(check.clone()))
                .err()
                .ok_or_else(|| "Markdown write failure returned success".to_string())?;
            assert!(failure.contains(PR_EVIDENCE_MD), "wrong failure: {failure}");
            refused("Markdown write failure")?;
            fs::remove_dir(&markdown).map_err(|error| error.to_string())?;

            baseline()?;
            let subject = repo.join(PR_CHECK_SUBJECT_JSON);
            let failure = write_pr_evidence_with_runner(&repo, &options, |_, _| {
                match fs::remove_file(&subject) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.to_string()),
                }
                fs::create_dir(&subject).map_err(|error| error.to_string())?;
                Ok(check.clone())
            })
            .err()
            .ok_or_else(|| "blocked subject publication returned success".to_string())?;
            assert!(
                failure.contains("finalize"),
                "wrong publication failure: {failure}"
            );
            refused("subject rename failure")?;
            for entry in fs::read_dir(subject.parent().ok_or("subject has no parent")?)
                .map_err(|error| error.to_string())?
            {
                let entry = entry.map_err(|error| error.to_string())?;
                assert!(
                    !entry.file_name().to_string_lossy().ends_with(".tmp"),
                    "owned stage was stranded"
                );
            }
            fs::remove_dir(&subject).map_err(|error| error.to_string())?;

            baseline()?;
            let retained_check =
                fs::read(repo.join(PR_CHECK_JSON)).map_err(|error| error.to_string())?;
            fs::remove_file(&subject).map_err(|error| error.to_string())?;
            fs::create_dir(&subject).map_err(|error| error.to_string())?;
            let mut runner_called = false;
            let failure = write_pr_evidence_with_runner(&repo, &options, |_, _| {
                runner_called = true;
                Err("runner must not execute after invalidation failure".into())
            })
            .err()
            .ok_or_else(|| "invalidation failure returned success".to_string())?;
            assert!(!runner_called);
            assert!(
                failure.contains("remove stale"),
                "wrong invalidation failure: {failure}"
            );
            assert_eq!(
                fs::read(repo.join(PR_CHECK_JSON)).map_err(|error| error.to_string())?,
                retained_check,
                "subject authority must be invalidated before subordinate files"
            );
            refused("invalidation failure")?;
            fs::remove_dir(&subject).map_err(|error| error.to_string())?;
            for artifact in [PR_CHECK_JSON, PR_REVIEW_INPUT_JSON] {
                fs::remove_file(repo.join(artifact)).map_err(|error| error.to_string())?;
            }
            let mut runner_called = false;
            let failure = write_pr_evidence_with_runner(&repo, &options, |_, _| {
                runner_called = true;
                Err("injected failure with no old artifacts".into())
            });
            assert!(
                runner_called,
                "missing old artifacts must not block the runner"
            );
            assert!(failure.is_err());
            refused("missing old artifacts")?;
            baseline()?;
            Ok(())
        })();
        let cleanup = fs::remove_dir_all(&repo)
            .map_err(|error| format!("cleanup {}: {error}", repo.display()));
        result.and(cleanup)
    }

    #[test]
    fn real_producer_root_identity_is_admitted_but_not_replayed() -> Result<(), String> {
        let _cwd_guard = crate::acquire_test_cwd_read_guard();
        crate::reports::fixtures::ripr_fixture_binary()?;
        let binary = built_ripr_binary_path(&repo_root()?)?.display().to_string();
        let parent = temp_repo("ripr-pr-root-identity")?;
        let repo = parent.join("producer");
        let other = parent.join("other-root");
        let result = (|| {
            fs::create_dir_all(&repo).map_err(|error| error.to_string())?;
            run_git(&repo, &["init"])?;
            run_git(&repo, &["config", "user.email", "ripr-pr@example.invalid"])?;
            run_git(&repo, &["config", "user.name", "RIPR PR Test"])?;
            write_repo_file(&repo, ".gitignore", "target/\n")?;
            write_repo_file(
                &repo,
                "Cargo.toml",
                "[package]\nname = \"root-identity-probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            )?;
            write_repo_file(
                &repo,
                "src/lib.rs",
                "pub fn eligible(value: i32) -> bool { value > 0 }\n",
            )?;
            run_git(&repo, &["add", "."])?;
            run_git(&repo, &["commit", "--no-gpg-sign", "-m", "initial"])?;
            write_repo_file(
                &repo,
                "src/lib.rs",
                "pub fn eligible(value: i32) -> bool { value > 1 }\n",
            )?;
            run_git(&repo, &["add", "."])?;
            run_git(
                &repo,
                &["commit", "--no-gpg-sign", "-m", "change predicate"],
            )?;
            let options = PrEvidenceOptions {
                base: "HEAD~1".into(),
                head: "HEAD".into(),
                ..options()
            };
            let check = run_ripr_check_binary(
                &binary,
                vec![
                    "check".into(),
                    "--root".into(),
                    repo.display().to_string(),
                    "--base".into(),
                    options.base.clone(),
                    "--no-unchanged-tests".into(),
                    "--format".into(),
                    "json".into(),
                ],
                &options,
                Duration::from_mins(2),
            )?;
            let value: Value = serde_json::from_str(&check).map_err(|error| error.to_string())?;
            if value
                .pointer("/analysis_outcome/analysis_complete")
                .and_then(Value::as_bool)
                != Some(true)
                || value
                    .get("findings")
                    .and_then(Value::as_array)
                    .is_none_or(|findings| findings.is_empty())
            {
                return Err("fixture producer must generate complete nonempty findings".into());
            }
            write_pr_evidence_from_check_json(&repo, &options, &check)?;
            check_pr_evidence(&repo, &options)?;
            run_git(
                &parent,
                &[
                    "clone",
                    "--no-hardlinks",
                    &repo.display().to_string(),
                    &other.display().to_string(),
                ],
            )?;
            let canonical = repo.canonicalize().map_err(|error| error.to_string())?;
            for public_producer in [false, true] {
                if public_producer {
                    for artifact in [
                        PR_EVIDENCE_JSON,
                        PR_EVIDENCE_MD,
                        PR_CHECK_JSON,
                        PR_CHECK_SUBJECT_JSON,
                        PR_REVIEW_INPUT_JSON,
                    ] {
                        fs::remove_file(repo.join(artifact)).map_err(|error| {
                            format!("remove compatibility producer artifact {artifact}: {error}")
                        })?;
                    }
                    for verify in [false, true] {
                        let mut producer_args =
                            vec!["pr-evidence".into(), "--base".into(), options.base.clone()];
                        if verify {
                            producer_args.push("--check".into());
                        }
                        let producer = crate::run::capture_output_in_dir_with_timeout_bounded(
                            Path::new(&binary),
                            &producer_args,
                            &[],
                            &repo,
                            Duration::from_mins(2),
                            64 * 1024,
                            "public PR evidence producer",
                        )?;
                        if producer.timed_out
                            || !producer.status.is_some_and(|status| status.success())
                        {
                            return Err(format!("public producer failed: {}", producer.stderr));
                        }
                    }
                }
                for artifact in [
                    PR_EVIDENCE_JSON,
                    PR_EVIDENCE_MD,
                    PR_CHECK_JSON,
                    PR_CHECK_SUBJECT_JSON,
                    PR_REVIEW_INPUT_JSON,
                    PR_CANONICAL_DIFF,
                    PR_DIFF,
                ] {
                    let destination = other.join(artifact);
                    if let Some(parent) = destination.parent() {
                        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                    }
                    fs::copy(repo.join(artifact), destination)
                        .map_err(|error| error.to_string())?;
                }
                for root in [&repo, &canonical, &repo.join("."), &other] {
                    let admitted = root != &other;
                    let validation = check_pr_evidence(root, &options);
                    if admitted {
                        validation?;
                    } else {
                        match validation {
                            Err(error) if error.contains("root_identity") => {}
                            result => {
                                return Err(format!(
                                    "compatibility checker accepted replay or rejected for wrong reason: {result:?}"
                                ));
                            }
                        }
                    }
                    let out = parent.join("review.json");
                    let args = vec![
                        "review-comments".into(),
                        "--root".into(),
                        root.display().to_string(),
                        "--base".into(),
                        options.base.clone(),
                        "--head".into(),
                        options.head.clone(),
                        "--check-output".into(),
                        repo.join(PR_CHECK_JSON).display().to_string(),
                        "--out".into(),
                        out.display().to_string(),
                    ];
                    let review = capture_output_with_timeout(
                        &binary,
                        &args,
                        &[],
                        Duration::from_mins(2),
                        "real root identity admission",
                    )?;
                    if review.timed_out
                        || review.status.is_some_and(|status| status.success()) != admitted
                    {
                        return Err(format!(
                            "unexpected root admission for {}: {}\n{}",
                            root.display(),
                            review.stdout,
                            review.stderr
                        ));
                    }
                    if admitted {
                        let rendered =
                            fs::read_to_string(&out).map_err(|error| error.to_string())?;
                        let rendered: Value =
                            serde_json::from_str(&rendered).map_err(|error| error.to_string())?;
                        if rendered
                            .pointer("/analysis_scope/basis")
                            .and_then(Value::as_str)
                            != Some("producer_check_projection")
                            || rendered
                                .pointer("/analysis_scope/classified_seams_considered")
                                .and_then(Value::as_u64)
                                .is_none_or(|count| count == 0)
                        {
                            return Err(
                                "consumer did not review the nonempty producer projection".into()
                            );
                        }
                    } else if !review.stderr.contains("producer_identity_mismatch")
                        || !review.stderr.contains("root_identity")
                    {
                        return Err(format!(
                            "replay did not fail at root identity admission: {}",
                            review.stderr
                        ));
                    }
                }
            }
            for selected_root in ["src".to_string(), repo.join("src").display().to_string()] {
                let rooted_options = PrEvidenceOptions {
                    root: selected_root,
                    ..options.clone()
                };
                write_pr_evidence_from_check_json(&repo, &rooted_options, &check)?;
                check_pr_evidence(&repo, &rooted_options)?;
            }
            write_pr_evidence_from_check_json(&repo, &options, &check)?;
            let review_path = repo.join(PR_REVIEW_INPUT_JSON);
            let mut review: Value =
                serde_json::from_slice(&fs::read(&review_path).map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
            let review_object = review
                .as_object_mut()
                .ok_or("review input must be an object")?;
            review_object.insert(
                "root_identity".into(),
                Value::String("different-root".into()),
            );
            fs::write(
                &review_path,
                serde_json::to_vec(&review).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            let subject_path = repo.join(PR_CHECK_SUBJECT_JSON);
            let mut subject: Value = serde_json::from_slice(
                &fs::read(&subject_path).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            let (digest, bytes) = digest_file(&review_path)?;
            let subject_object = subject.as_object_mut().ok_or("subject must be an object")?;
            subject_object.insert("review_input_sha256".into(), Value::String(digest));
            subject_object.insert("review_input_byte_count".into(), Value::from(bytes));
            fs::write(
                subject_path,
                serde_json::to_vec(&subject).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            let violations = check_subject_violations(&repo, &options);
            if violations.len() != 1
                || !violations
                    .iter()
                    .any(|error| error.contains("review-input.json root_identity"))
            {
                return Err(format!(
                    "review identity mismatch must be the sole violation: {violations:?}"
                ));
            }
            Ok(())
        })();
        let cleanup = fs::remove_dir_all(&parent)
            .map_err(|error| format!("cleanup {}: {error}", parent.display()));
        result.and(cleanup)
    }

    #[test]
    fn repository_language_policy_admits_real_mixed_language_producer() -> Result<(), String> {
        let _cwd_guard = crate::acquire_test_cwd_read_guard();
        // Cargo runs unit tests from the xtask package directory. Retain the
        // fixture builder's freshness guarantee, but resolve the workspace
        // binary through the same path owner as the production PR producer.
        crate::reports::fixtures::ripr_fixture_binary()?;
        let binary = built_ripr_binary_path(&repo_root()?)?.display().to_string();
        let policy = match fs::read_to_string(repo_root()?.join("ripr.toml")) {
            Ok(policy) => policy,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(format!("read repository language policy: {error}")),
        };
        let repo = temp_repo("ripr-pr-real-language-policy")?;
        let result = (|| {
            run_git(&repo, &["init"])?;
            run_git(&repo, &["config", "user.email", "ripr-pr@example.invalid"])?;
            run_git(&repo, &["config", "user.name", "RIPR PR Test"])?;
            write_repo_file(&repo, ".gitignore", "target/\n")?;
            write_repo_file(
                &repo,
                "Cargo.toml",
                "[package]\nname = \"mixed-producer\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            )?;
            write_repo_file(
                &repo,
                "src/lib.rs",
                "pub fn eligible(value: i32) -> bool { value > 0 }\n",
            )?;
            write_repo_file(
                &repo,
                "src/eligible.ts",
                "export function eligible(value: number): boolean { return value > 0; }\n",
            )?;
            write_repo_file(
                &repo,
                "tests/eligible.test.ts",
                "import { eligible } from '../src/eligible';\ntest('positive value is eligible', () => { expect(eligible(2)).toBe(true); });\n",
            )?;
            write_repo_file(
                &repo,
                "src/eligible.py",
                "def eligible(value):\n    return value > 0\n",
            )?;
            write_repo_file(
                &repo,
                "tests/test_eligible.py",
                "from eligible import eligible\ndef test_positive_value():\n    assert eligible(2)\n",
            )?;
            run_git(&repo, &["add", "."])?;
            run_git(&repo, &["commit", "--no-gpg-sign", "-m", "initial"])?;
            let base = run_git_output(&repo, &["rev-parse", "HEAD"])?;
            write_repo_file(
                &repo,
                "src/lib.rs",
                "pub fn eligible(value: i32) -> bool { value > 1 }\n",
            )?;
            write_repo_file(
                &repo,
                "src/eligible.ts",
                "export function eligible(value: number): boolean { return value > 1; }\n",
            )?;
            write_repo_file(
                &repo,
                "src/eligible.py",
                "def eligible(value):\n    return value > 1\n",
            )?;
            for (config, complete) in [
                (policy.as_str(), true),
                ("[languages]\nenabled = [\"rust\"]\n", false),
                ("[languages]\nenabled = [\"rust\", \"typescript\"]\n", false),
            ] {
                write_repo_file(&repo, "ripr.toml", config)?;
                run_git(&repo, &["add", "."])?;
                run_git(
                    &repo,
                    &["commit", "--no-gpg-sign", "-m", "producer policy case"],
                )?;
                let options = PrEvidenceOptions {
                    root: ".".to_string(),
                    base: base.trim().to_string(),
                    head: run_git_output(&repo, &["rev-parse", "HEAD"])?
                        .trim()
                        .to_string(),
                    check: false,
                };
                let args = vec![
                    "check".into(),
                    "--root".into(),
                    repo.display().to_string(),
                    "--base".into(),
                    options.base.clone(),
                    "--no-unchanged-tests".into(),
                    "--format".into(),
                    "json".into(),
                ];
                let check = run_ripr_check_binary(&binary, args, &options, Duration::from_mins(2))?;
                let value: Value =
                    serde_json::from_str(&check).map_err(|error| error.to_string())?;
                if value
                    .pointer("/analysis_outcome/analysis_complete")
                    .and_then(Value::as_bool)
                    != Some(complete)
                {
                    return Err(format!(
                        "repository policy expected complete={complete}, got {}",
                        value
                            .get("analysis_outcome")
                            .ok_or("missing analysis outcome")?
                    ));
                }
                let findings = value
                    .get("findings")
                    .and_then(Value::as_array)
                    .ok_or("missing producer findings")?;
                if !findings
                    .iter()
                    .any(|finding| finding.get("language").and_then(Value::as_str) == Some("rust"))
                {
                    return Err("real Rust finding missing".into());
                }
                if complete
                    && !findings.iter().any(|finding| {
                        finding.get("language").and_then(Value::as_str) == Some("typescript")
                            && finding.get("language_status").and_then(Value::as_str)
                                == Some("preview")
                    })
                {
                    return Err("real preview TypeScript finding missing".into());
                }
                if complete
                    && !findings.iter().any(|finding| {
                        finding.get("language").and_then(Value::as_str) == Some("python")
                            && finding.get("language_status").and_then(Value::as_str)
                                == Some("preview")
                    })
                {
                    return Err("real preview Python finding missing".into());
                }
                write_pr_evidence_from_check_json(&repo, &options, &check)?;
                check_pr_evidence(&repo, &options)?;
                if complete {
                    let presentation =
                        fs::read(repo.join(PR_DIFF)).map_err(|error| error.to_string())?;
                    let canonical = fs::read(repo.join(PR_CANONICAL_DIFF))
                        .map_err(|error| error.to_string())?;
                    assert_ne!(
                        presentation, canonical,
                        "fixture must distinguish diff recipes"
                    );
                    fs::write(repo.join(PR_CANONICAL_DIFF), &presentation)
                        .map_err(|error| error.to_string())?;
                    let violations = check_subject_violations(&repo, &options);
                    assert!(
                        violations
                            .iter()
                            .any(|error| error.contains("canonical_diff_sha256"))
                    );
                    assert!(
                        violations
                            .iter()
                            .any(|error| error.contains("requested canonical base/head diff"))
                    );
                    fs::write(repo.join(PR_CANONICAL_DIFF), &canonical)
                        .map_err(|error| error.to_string())?;
                    check_pr_evidence(&repo, &options)?;
                }
                let args = vec![
                    "review-comments".into(),
                    "--root".into(),
                    repo.display().to_string(),
                    "--base".into(),
                    options.base.clone(),
                    "--head".into(),
                    options.head.clone(),
                    "--check-output".into(),
                    repo.join(PR_CHECK_JSON).display().to_string(),
                    "--out".into(),
                    repo.join("target/ripr/review/comments.json")
                        .display()
                        .to_string(),
                ];
                let review = capture_output_with_timeout(
                    &binary,
                    &args,
                    &[],
                    Duration::from_mins(2),
                    "real producer review admission",
                )?;
                if review.timed_out
                    || review.status.is_some_and(|status| status.success()) != complete
                {
                    return Err(format!(
                        "unexpected review admission complete={complete}: {}\n{}",
                        review.stdout, review.stderr
                    ));
                }
                if !complete
                    && !format!("{}{}", review.stdout, review.stderr)
                        .contains("producer analysis is not complete")
                {
                    return Err(
                        "disabled adapter was not rejected for incomplete producer evidence".into(),
                    );
                }
                if complete {
                    let rendered =
                        fs::read_to_string(repo.join("target/ripr/review/comments.json"))
                            .map_err(|error| format!("read admitted review output: {error}"))?;
                    let rendered: Value = serde_json::from_str(&rendered)
                        .map_err(|error| format!("parse admitted review output: {error}"))?;
                    if rendered
                        .pointer("/analysis_scope/basis")
                        .and_then(Value::as_str)
                        != Some("producer_check_projection")
                        || rendered
                            .pointer("/analysis_scope/classified_seams_considered")
                            .and_then(Value::as_u64)
                            .is_none_or(|count| count == 0)
                    {
                        return Err(
                            "consumer did not review the nonempty producer projection".into()
                        );
                    }
                }
            }
            Ok(())
        })();
        let cleanup = fs::remove_dir_all(&repo)
            .map_err(|error| format!("cleanup {}: {error}", repo.display()));
        result.and(cleanup)
    }

    fn two_commit_repo(name: &str) -> Result<(PathBuf, PrEvidenceOptions), String> {
        let repo = temp_repo(name)?;
        run_git(&repo, &["init"])?;
        run_git(&repo, &["config", "user.email", "ripr-pr@example.invalid"])?;
        run_git(&repo, &["config", "user.name", "RIPR PR Test"])?;
        write_repo_file(&repo, "README.md", "# sample\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "initial"])?;
        write_repo_file(&repo, "src/lib.rs", "pub fn value() -> u8 { 1 }\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "add rust"])?;
        Ok((
            repo,
            PrEvidenceOptions {
                base: "HEAD~1".to_string(),
                head: "HEAD".to_string(),
                ..options()
            },
        ))
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
                let stdout_path = path.with_extension("stdout.txt");
                fs::write(&stdout_path, stdout)
                    .map_err(|err| format!("write {}: {err}", stdout_path.display()))?;
                script.push_str("type \"%~dpn0.stdout.txt\"\r\n");
            }
            if !stderr.is_empty() {
                let stderr_path = path.with_extension("stderr.txt");
                fs::write(&stderr_path, stderr)
                    .map_err(|err| format!("write {}: {err}", stderr_path.display()))?;
                script.push_str("type \"%~dpn0.stderr.txt\" 1>&2\r\n");
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
