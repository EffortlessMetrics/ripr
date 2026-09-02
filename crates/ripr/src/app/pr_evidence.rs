//! `ripr pr-evidence` — binary-first PR evidence packet (Campaign 31 item 8c).
//!
//! Ports `cargo xtask ripr-pr` into the `ripr` binary so downstream consumers
//! (e.g. perl-lsp-swarm) can generate their PR evidence packet without
//! compiling their own `xtask`. The xtask wrapper remains as a compatibility
//! shim until downstream consumers migrate.
//!
//! Unlike the xtask, this command does NOT shell out to `cargo run -p ripr --
//! check ...`. It calls [`crate::check_workspace`] directly and renders the
//! resulting [`crate::CheckOutput`] as JSON via [`crate::app::render_check`].
//! This avoids recompilation and keeps the analysis in-process.

use crate::app::{CheckInput, Mode, OutputFormat, check_workspace, render_check};
use crate::config::{load_for_root, repo_exposure_config_identity_hash};
use crate::review_input::{
    CanonicalFindingIndexV1, REVIEW_INDEX_MAX_BYTES, REVIEW_INDEX_MAX_ENTRIES,
    REVIEW_INDEX_SCHEMA_VERSION, REVIEW_INPUT_PROJECTION_LIMIT, REVIEW_INPUT_SCHEMA_VERSION,
    REVIEW_INPUT_SELECTION_POLICY, REVIEW_INPUT_SELECTION_POLICY_VERSION, ReviewInputV1,
    canonical_projection, canonical_projection_all, canonical_projection_from_index,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Entry point for `ripr pr-evidence`. Generates the PR diff, runs an
/// in-process RIPR check over that diff, and composes the result into a PR
/// evidence packet (`repo-exposure.{json,md}`). Writes:
/// - `target/ripr/pr/pr.diff` (analyzed diff)
/// - `target/ripr/pr/repo-exposure.json` (PR evidence JSON)
/// - `target/ripr/pr/repo-exposure.md` (PR evidence Markdown)
///
/// When the check fails, an `error` packet is still written so downstream
/// consumers see a contract-valid, actionable artifact rather than a gap.
pub(crate) fn run_pr_evidence(args: &[String]) -> Result<(), String> {
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
            other => return Err(format!("unknown pr-evidence argument `{other}`")),
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
        return Err(format!("pr-evidence {flag} requires a non-empty value"));
    }
    Ok(value)
}

fn print_help() {
    println!("usage: ripr pr-evidence [--base <rev>] [--head <rev>] [--root <path>] [--check]");
    println!();
    println!("Options:");
    println!("  --base <rev>   PR base revision. Defaults to {DEFAULT_BASE}.");
    println!("  --head <rev>   PR head revision. Defaults to {DEFAULT_HEAD}.");
    println!("  --root <path>  Workspace root label. Defaults to current directory.");
    println!("  --check        Verify the existing PR evidence packet is contract-valid.");
    println!();
    println!("Outputs:");
    println!("  {PR_EVIDENCE_JSON}  — PR evidence JSON packet");
    println!("  {PR_EVIDENCE_MD}   — PR evidence Markdown panel");
    println!("  {PR_DIFF}          — analyzed PR diff");
    println!();
    println!("This packet is diff-scoped and advisory. It does not post review");
    println!("comments, edit source, or change gate semantics.");
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
    let config = load_for_root(&root)?;
    let canonical_diff = fs::read(repo.join(PR_DIFF))
        .map_err(|err| format!("read canonical diff for check subject binding: {err}"))?;
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
        return Err(format!(
            "canonical finding index exceeds byte limit ({} > {})",
            encoded_entries.len(),
            REVIEW_INDEX_MAX_BYTES
        ));
    }
    let index = CanonicalFindingIndexV1 {
        schema_version: REVIEW_INDEX_SCHEMA_VERSION.to_string(),
        total_finding_count: entries.len() as u64,
        index_sha256: format!("sha256:{:x}", Sha256::digest(&encoded_entries)),
        entries,
    };
    let mut subject = json!({
        "schema_version": "ripr.pr_check_subject.v1",
        "root_identity": root.display().to_string().replace('\\', "/"),
        "base_sha": resolve_revision(repo, &options.base, "commit")?,
        "head_sha": resolve_revision(repo, &options.head, "commit")?,
        "head_tree": resolve_revision(repo, &options.head, "tree")?,
        "check_sha256": format!("sha256:{:x}", Sha256::digest(check_json_text.as_bytes())),
        "check_byte_count": check_json_text.len(),
        "check_schema": check_value.get("schema_version").cloned().unwrap_or(Value::Null),
        "mode": check_value.get("mode").cloned().unwrap_or(Value::Null),
        "canonical_diff_sha256": format!("sha256:{:x}", Sha256::digest(&canonical_diff)),
        "configuration_fingerprint": repo_exposure_config_identity_hash(&config),
        "analyzer_generation": crate::review_input::REVIEW_ANALYZER_GENERATION,
        "analysis_outcome": check_value.get("analysis_outcome").cloned().unwrap_or(Value::Null),
        "canonical_finding_index": serde_json::to_value(&index)
            .map_err(|error| format!("serialize canonical finding index: {error}"))?,
        "canonical_finding_index_entry_count": findings.len(),
        "canonical_finding_index_byte_count": encoded_entries.len(),
    });
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
    if !violations.is_empty() {
        return Err(format!(
            "PR evidence contract violations:\n{}",
            violations
                .iter()
                .map(|violation| format!("- {violation}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    validate_producer_artifacts(repo, options)?;
    println!("PR evidence contract ok: {PR_EVIDENCE_JSON}");
    Ok(())
}

fn validate_producer_artifacts(repo: &Path, _options: &PrEvidenceOptions) -> Result<(), String> {
    let check_path = repo.join(PR_CHECK_JSON);
    let subject_path = repo.join(PR_CHECK_SUBJECT_JSON);
    let review_input_path = repo.join(PR_REVIEW_INPUT_JSON);
    let (check_digest, check_byte_count) = digest_file(&check_path)
        .map_err(|error| format!("missing or unreadable {PR_CHECK_JSON}: {error}"))?;
    let subject_bytes = fs::read(&subject_path)
        .map_err(|error| format!("missing or unreadable {PR_CHECK_SUBJECT_JSON}: {error}"))?;
    let review_input_bytes = fs::read(&review_input_path)
        .map_err(|error| format!("missing or unreadable {PR_REVIEW_INPUT_JSON}: {error}"))?;
    let subject: Value = serde_json::from_slice(&subject_bytes)
        .map_err(|error| format!("{PR_CHECK_SUBJECT_JSON} is not valid JSON: {error}"))?;
    let review_input: ReviewInputV1 = serde_json::from_slice(&review_input_bytes)
        .map_err(|error| format!("{PR_REVIEW_INPUT_JSON} is not valid ReviewInputV1: {error}"))?;
    if subject.get("check_sha256").and_then(Value::as_str) != Some(&check_digest) {
        return Err(format!(
            "{PR_CHECK_SUBJECT_JSON} check_sha256 does not match {PR_CHECK_JSON}"
        ));
    }
    if subject.get("check_byte_count").and_then(Value::as_u64) != Some(check_byte_count) {
        return Err(format!(
            "{PR_CHECK_SUBJECT_JSON} check_byte_count does not match {PR_CHECK_JSON}"
        ));
    }
    let index: CanonicalFindingIndexV1 = subject
        .get("canonical_finding_index")
        .cloned()
        .ok_or_else(|| format!("{PR_CHECK_SUBJECT_JSON} is missing canonical_finding_index"))
        .and_then(|value| {
            serde_json::from_value(value).map_err(|error| {
                format!("{PR_CHECK_SUBJECT_JSON} canonical_finding_index is invalid: {error}")
            })
        })?;
    let expected_projection = canonical_projection_from_index(&index)
        .map_err(|error| format!("validate canonical finding index: {error}"))?;
    let actual_projection = review_input.findings.clone();
    if actual_projection != expected_projection {
        return Err(format!(
            "{PR_REVIEW_INPUT_JSON} is not the canonical projection"
        ));
    }
    if subject.get("review_input_sha256").and_then(Value::as_str)
        != Some(&format!("sha256:{:x}", Sha256::digest(&review_input_bytes)))
    {
        return Err(format!(
            "{PR_CHECK_SUBJECT_JSON} review_input_sha256 does not match {PR_REVIEW_INPUT_JSON}"
        ));
    }
    if subject
        .get("review_input_byte_count")
        .and_then(Value::as_u64)
        != Some(review_input_bytes.len() as u64)
    {
        return Err(format!(
            "{PR_CHECK_SUBJECT_JSON} review_input_byte_count does not match {PR_REVIEW_INPUT_JSON}"
        ));
    }
    Ok(())
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

/// Run the RIPR check in-process and return the JSON rendering. This replaces
/// the xtask's `cargo run -p ripr -- check ...` subprocess with a direct call
/// to [`crate::check_workspace`], avoiding recompilation. The diff written by
/// [`write_diff`] is passed via `--diff`, matching the xtask's scope.
fn run_ripr_check(repo: &Path, options: &PrEvidenceOptions) -> Result<String, String> {
    let diff_path = repo.join(PR_DIFF);
    let root_path = command_root_path(repo, &options.root);
    let input = CheckInput {
        root: root_path,
        base: None,
        diff_file: Some(diff_path),
        mode: Mode::Draft,
        format: OutputFormat::Json,
        include_unchanged_tests: true,
        perl_facts_path: None,
        suppression_policy: None,
        git_timeout: None,
    };
    let output = check_workspace(input)?;
    render_check(&output, &OutputFormat::Json)
}

fn command_root_path(repo: &Path, root: &str) -> PathBuf {
    let root_path = Path::new(root);
    if root_path.is_absolute() {
        root_path.to_path_buf()
    } else {
        repo.join(root_path)
    }
}

fn resolve_revision(repo: &Path, revision: &str, object: &str) -> Result<String, String> {
    let expression = format!("{revision}^{{{object}}}");
    run_git_output(repo, &["rev-parse", "--verify", expression.as_str()])
        .map(|output| output.trim().to_string())
}

fn run_git_output(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run git {args:?}: {err}"))?;
    if output.status.success() {
        String::from_utf8(output.stdout)
            .map_err(|err| format!("git {args:?} produced non-UTF-8 output: {err}"))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(format!(
            "git {args:?} failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
        ))
    }
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

fn validate_targeted_mutation_route(summary: &Map<String, Value>, violations: &mut Vec<String>) {
    let Some(route) = summary
        .get("targeted_mutation_route")
        .and_then(Value::as_object)
    else {
        violations.push("summary.targeted_mutation_route is missing or not an object".to_string());
        return;
    };
    match route.get("status").and_then(Value::as_str) {
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
    render_targeted_mutation_route(
        &mut out,
        summary.and_then(|summary| summary.get("targeted_mutation_route")),
    );

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

fn render_targeted_mutation_route(out: &mut String, route: Option<&Value>) {
    let Some(route) = route.and_then(Value::as_object) else {
        return;
    };
    let status = route
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    out.push_str(&format!("- route: `{status}`\n"));
    if let Some(candidates) = route.get("candidates").and_then(Value::as_array) {
        for candidate in candidates {
            let Some(candidate) = candidate.as_object() else {
                continue;
            };
            out.push_str(&format!(
                "- candidate: `{}`:{} {} -> {}\n- command: `{}`\n- expected: {}\n",
                md_escape(
                    candidate
                        .get("file")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                ),
                candidate.get("line").and_then(Value::as_u64).unwrap_or(0),
                candidate.get("from").and_then(Value::as_str).unwrap_or("?"),
                candidate.get("to").and_then(Value::as_str).unwrap_or("?"),
                md_escape(
                    candidate
                        .get("command")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                ),
                md_escape(
                    candidate
                        .get("expected_observation")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                )
            ));
        }
    }
    if let Some(limitations) = route.get("limitations").and_then(Value::as_array) {
        for limitation in limitations {
            out.push_str(&format!(
                "- limitation: `{}`\n",
                md_escape(
                    limitation
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("no safe candidate")
                )
            ));
        }
    }
    out.push('\n');
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

/// Resolve the repo root. In the ripr binary, this is the current working
/// directory (the user runs `ripr pr-evidence` from the repo root). The xtask
/// used `CARGO_MANIFEST_DIR` but the binary should not assume a build-system
/// location.
fn repo_root() -> Result<PathBuf, String> {
    std::env::current_dir().map_err(|err| format!("failed to determine working directory: {err}"))
}

fn write_parented_file(path: &Path, label: &str, contents: impl AsRef<[u8]>) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create parent dir for {label}: {err}"))?;
    }
    fs::write(path, contents).map_err(|err| format!("failed to write {label}: {err}"))
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
    fn parse_rejects_unknown_or_empty_args() -> Result<(), String> {
        match parse_options(&["--bad".into()]) {
            Err(msg) if msg.contains("--bad") => Ok(()),
            other => Err(format!("expected unknown-arg error, got {other:?}")),
        }?;
        match parse_options(&["--base".into(), "".into()]) {
            Err(msg) if msg.contains("non-empty") => Ok(()),
            other => Err(format!("expected non-empty error, got {other:?}")),
        }
    }

    #[test]
    fn packet_maps_check_summary_to_routing_fields() {
        let check = json!({
            "summary": {
                "weakly_exposed": 2,
                "reachable_unrevealed": 1,
                "no_static_path": 0
            },
            "findings": [{
                "classification": "weakly_exposed",
                "probe": {
                    "family": "predicate",
                    "file": "src/lib.rs",
                    "line": 8,
                    "expression": "amount >= threshold"
                }
            }]
        });
        let changed = vec!["src/lib.rs".to_string(), "tests/lib.rs".to_string()];
        let packet = pr_evidence_packet(&options(), &changed, &check);
        assert_eq!(packet["summary"]["changed_files"], 2);
        assert_eq!(packet["summary"]["weakly_exposed"], 2);
        assert_eq!(packet["summary"]["reachable_unrevealed"], 1);
        assert_eq!(packet["summary"]["severe_gaps"], 3);
        assert_eq!(packet["summary"]["requires_targeted_mutation"], true);
        assert_eq!(packet["summary"]["routing_reason"], "ripr severe gap");
        assert_eq!(
            packet["summary"]["targeted_mutation_route"]["status"],
            "candidate"
        );
        assert_eq!(
            packet["summary"]["targeted_mutation_route"]["candidates"][0]["from"],
            ">="
        );
        assert_eq!(
            packet["summary"]["targeted_mutation_route"]["candidates"][0]["to"],
            ">"
        );
    }

    #[test]
    fn packet_names_limitation_when_severe_finding_has_no_safe_candidate() {
        let packet = pr_evidence_packet(
            &options(),
            &["src/lib.rs".to_string()],
            &json!({
                "summary": {"weakly_exposed": 1, "reachable_unrevealed": 0, "no_static_path": 0},
                "findings": [{
                    "classification": "weakly_exposed",
                    "probe": {"family": "call_presence", "file": "src/lib.rs", "line": 8, "expression": "publish(event)"}
                }]
            }),
        );
        assert_eq!(
            packet["summary"]["targeted_mutation_route"]["status"],
            "static_limitation"
        );
        assert_eq!(
            packet["summary"]["targeted_mutation_route"]["candidates"]
                .as_array()
                .map(Vec::len),
            Some(0)
        );
        assert_eq!(
            packet["summary"]["targeted_mutation_route"]["limitations"][0]["kind"],
            "no_safe_candidate"
        );
        assert!(validate_packet_value(&packet, &options(), 1, true).is_empty());
    }

    #[test]
    fn targeted_mutation_route_covers_operator_variants_and_fail_closed_inputs() {
        let mut findings = vec![
            (">=", ">"),
            ("<=", "<"),
            ("==", "!="),
            ("!=", "=="),
            (">", ">="),
            ("<", "<=")
        ]
        .into_iter()
        .map(|(operator, _)| {
            json!({
                "classification": "weakly_exposed",
                "probe": {"family": "predicate", "file": "src/lib.rs", "line": 8, "expression": format!("value {operator} limit")}
            })
        })
        .collect::<Vec<_>>();
        findings.push(json!({
            "classification": "weakly_exposed",
            "probe": {"family": "predicate", "file": "src/lib.rs", "line": 8, "expression": "value >= limit"}
        }));
        findings.push(json!({
            "classification": "weakly_exposed",
            "probe": {"family": "call_presence", "file": "src/lib.rs", "line": 9, "expression": "publish(value)"}
        }));
        findings.push(json!({"classification": "weakly_exposed"}));
        findings.push(json!({
            "classification": "weakly_exposed",
            "probe": {"family": "predicate", "line": 10, "expression": "value >= limit"}
        }));
        findings.push(json!({
            "classification": "weakly_exposed",
            "probe": {"family": "predicate", "file": "src/lib.rs", "expression": "value >= limit"}
        }));
        findings.push(json!({
            "classification": "weakly_exposed",
            "probe": {"family": "predicate", "file": "src/lib.rs", "line": 11, "expression": "value + limit"}
        }));
        let route = targeted_mutation_route(&json!({"findings": findings}), true);
        assert_eq!(route["status"], "candidate");
        assert_eq!(route["candidates"].as_array().map(Vec::len), Some(6));
        assert_eq!(route["limitations"].as_array().map(Vec::len), Some(5));
    }

    #[test]
    fn markdown_renders_targeted_mutation_candidate_and_limitation() {
        let packet = pr_evidence_packet(
            &options(),
            &["src/lib.rs".to_string()],
            &json!({
                "summary": {"weakly_exposed": 1, "reachable_unrevealed": 0, "no_static_path": 0},
                "findings": [
                    {"classification": "weakly_exposed", "probe": {"family": "predicate", "file": "src/lib.rs", "line": 8, "expression": "value >= limit"}},
                    {"classification": "weakly_exposed", "probe": {"family": "call_presence", "file": "src/lib.rs", "line": 9, "expression": "publish(value)"}}
                ]
            }),
        );
        let markdown = render_pr_evidence_markdown(&packet);
        assert!(markdown.contains("route: `candidate`"));
        assert!(markdown.contains("cargo mutants --file"));
        assert!(markdown.contains("no safe concrete mutation candidate"));
    }

    #[test]
    fn packet_without_check_summary_is_incomplete_and_warns() {
        let packet = pr_evidence_packet(&options(), &[], &json!({}));
        assert_eq!(packet["status"], "incomplete");
        assert_eq!(packet["warnings"][0]["kind"], "invalid_json");
        assert_eq!(
            packet["summary"]["targeted_mutation_route"]["status"],
            "not_required"
        );
    }

    #[test]
    fn error_packet_is_contract_valid_and_actionable() {
        let changed = vec!["src/lib.rs".to_string()];
        let packet = pr_evidence_error_packet(
            &options(),
            &changed,
            "ripr check for PR evidence failed; retry command: ripr pr-evidence --base origin/main --head HEAD --root .",
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
            "ripr check for PR evidence failed; retry command: ripr pr-evidence --base origin/main --head HEAD --root .",
        );
        let markdown = render_pr_evidence_markdown(&packet);
        assert!(markdown.contains("## Warnings"));
        assert!(markdown.contains("tool_error"));
        assert!(markdown.contains("retry command"));
    }

    #[test]
    fn write_pr_evidence_writes_error_packet_when_check_fails() -> Result<(), String> {
        let repo = temp_repo("ripr-pr-evidence-error-packet")?;
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
            Err("ripr check for PR evidence failed; retry command: ripr pr-evidence --base HEAD~1 --head HEAD --root .".to_string())
        })?;
        let check_error = match check_pr_evidence(&repo, &options) {
            Ok(()) => {
                return Err("an error packet without producer artifacts passed --check".into());
            }
            Err(error) => error,
        };
        if !check_error.contains("missing or unreadable target/ripr/pr/check.json") {
            return Err(format!(
                "unexpected producer artifact validation error: {check_error}"
            ));
        }

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
        let repo = temp_repo("ripr-pr-evidence-packet")?;
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
          "schema_version": "ripr.check.v1",
          "mode": "draft",
          "analysis_outcome": {"analysis_complete": true},
          "findings": [],
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

        let subject_path = repo.join(PR_CHECK_SUBJECT_JSON);
        let review_input_path = repo.join(PR_REVIEW_INPUT_JSON);
        let subject_bytes =
            fs::read(&subject_path).map_err(|err| format!("read subject: {err}"))?;
        let review_input_bytes =
            fs::read(&review_input_path).map_err(|err| format!("read review input: {err}"))?;
        let subject = serde_json::from_slice::<Value>(&subject_bytes)
            .map_err(|err| format!("parse subject: {err}"))?;
        let review_input = serde_json::from_slice::<Value>(&review_input_bytes)
            .map_err(|err| format!("parse review input: {err}"))?;

        let mut mutated_subject = subject.clone();
        mutated_subject["check_sha256"] = json!("sha256:wrong");
        fs::write(
            &subject_path,
            serde_json::to_vec(&mutated_subject).map_err(|err| err.to_string())?,
        )
        .map_err(|err| format!("write subject digest mutation: {err}"))?;
        if check_pr_evidence(&repo, &options).is_ok() {
            return Err("subject check digest mutation must fail".to_string());
        }
        fs::write(&subject_path, &subject_bytes)
            .map_err(|err| format!("restore subject: {err}"))?;

        let mut mutated_subject = subject.clone();
        mutated_subject["check_byte_count"] = json!(0);
        fs::write(
            &subject_path,
            serde_json::to_vec(&mutated_subject).map_err(|err| err.to_string())?,
        )
        .map_err(|err| format!("write subject size mutation: {err}"))?;
        if check_pr_evidence(&repo, &options).is_ok() {
            return Err("subject check size mutation must fail".to_string());
        }
        fs::write(&subject_path, &subject_bytes)
            .map_err(|err| format!("restore subject: {err}"))?;

        let mut mutated_subject = subject.clone();
        mutated_subject["canonical_finding_index"] = json!("invalid");
        fs::write(
            &subject_path,
            serde_json::to_vec(&mutated_subject).map_err(|err| err.to_string())?,
        )
        .map_err(|err| format!("write index mutation: {err}"))?;
        if check_pr_evidence(&repo, &options).is_ok() {
            return Err("invalid canonical index must fail".to_string());
        }
        fs::write(&subject_path, &subject_bytes)
            .map_err(|err| format!("restore subject: {err}"))?;

        let plausible_finding = json!({
            "stable_id": "substituted",
            "file": "src/lib.rs",
            "line": 1,
            "severity": "warning",
            "finding_class": "exposed",
            "summary": "substituted",
            "evidence_digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "related_test": null
        });
        let mut mutated_review_input = review_input.clone();
        mutated_review_input["findings"] = json!([plausible_finding]);
        mutated_review_input["reviewed_count"] = json!(1);
        mutated_review_input["projected_finding_count"] = json!(1);
        fs::write(
            &review_input_path,
            serde_json::to_vec(&mutated_review_input).map_err(|err| err.to_string())?,
        )
        .map_err(|err| format!("write projection mutation: {err}"))?;
        if check_pr_evidence(&repo, &options).is_ok() {
            return Err("substituted projection must fail".to_string());
        }
        fs::write(&review_input_path, &review_input_bytes)
            .map_err(|err| format!("restore review input: {err}"))?;

        let mut mutated_subject = subject.clone();
        mutated_subject["review_input_sha256"] = json!("sha256:wrong");
        fs::write(
            &subject_path,
            serde_json::to_vec(&mutated_subject).map_err(|err| err.to_string())?,
        )
        .map_err(|err| format!("write review digest mutation: {err}"))?;
        if check_pr_evidence(&repo, &options).is_ok() {
            return Err("review input digest mutation must fail".to_string());
        }
        fs::write(&subject_path, &subject_bytes)
            .map_err(|err| format!("restore subject: {err}"))?;

        let mut mutated_subject = subject.clone();
        mutated_subject["review_input_byte_count"] = json!(0);
        fs::write(
            &subject_path,
            serde_json::to_vec(&mutated_subject).map_err(|err| err.to_string())?,
        )
        .map_err(|err| format!("write review size mutation: {err}"))?;
        if check_pr_evidence(&repo, &options).is_ok() {
            return Err("review input size mutation must fail".to_string());
        }
        fs::write(&subject_path, &subject_bytes)
            .map_err(|err| format!("restore subject: {err}"))?;

        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn command_root_path_resolves_relative_to_repo() {
        let repo = Path::new("/repo");
        assert_eq!(command_root_path(repo, "."), Path::new("/repo/."));
        assert_eq!(command_root_path(repo, "/abs/root"), Path::new("/abs/root"));
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
        let path = std::env::temp_dir().join(unique);
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
