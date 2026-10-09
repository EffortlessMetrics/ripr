//! `ripr pr-evidence` — binary-first PR evidence packet (Campaign 31 item 8c).
//!
//! Ports `cargo xtask ripr-pr` into the `ripr` binary so downstream consumers
//! (e.g. perl-lsp-swarm) can generate their PR evidence packet without
//! compiling their own `xtask`. The xtask wrapper remains as a compatibility
//! shim until downstream consumers migrate.
//!
//! Unlike the xtask, this command does NOT shell out to `cargo run -p ripr --
//! check ...`. It calls [`crate::app::check_workspace_with_config`] directly and renders the
//! resulting [`crate::CheckOutput`] as JSON via [`crate::app::render_check_json_unbounded`].
//! This avoids recompilation and keeps the analysis in-process.

use crate::app::{CheckInput, Mode, OutputFormat, check_workspace_with_config};
use crate::cli::unknown_argument;
use crate::config::{
    CheckInputExplicit, apply_to_check_input, load_for_root, repo_exposure_config_identity_hash,
};
use crate::output::markdown::{code_span, inline_prose, table_cell_text, table_code_span};
use crate::review_input::{
    CanonicalFindingIndexV1, REVIEW_INPUT_PROJECTION_LIMIT, REVIEW_INPUT_SCHEMA_VERSION,
    REVIEW_INPUT_SELECTION_POLICY, REVIEW_INPUT_SELECTION_POLICY_VERSION, ReviewInputV1,
    canonical_finding_index, canonical_projection, canonical_projection_from_index,
    canonical_root_identity,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
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
const PR_CANONICAL_DIFF: &str = "target/ripr/pr/check.diff";
const REVIEW_INPUT_MAX_BYTES: usize = 128 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
struct PrEvidenceOptions {
    root: String,
    base: String,
    /// `false` when `--base` was omitted: `base` then holds a placeholder
    /// until [`run_pr_evidence`] resolves the repository's default branch.
    base_explicit: bool,
    head: String,
    check: bool,
}

impl Default for PrEvidenceOptions {
    fn default() -> Self {
        Self {
            root: DEFAULT_ROOT.to_string(),
            base: DEFAULT_BASE.to_string(),
            base_explicit: false,
            head: DEFAULT_HEAD.to_string(),
            check: false,
        }
    }
}

/// Entry point for `ripr pr-evidence`. Generates the PR diff, runs an
/// in-process RIPR check over that diff, and composes the result into a PR
/// evidence packet (`repo-exposure.{json,md}`). Writes:
/// - `target/ripr/pr/pr.diff` (three-context PR presentation diff)
/// - `target/ripr/pr/check.diff` (canonical zero-context analysis input)
/// - `target/ripr/pr/check.json`, `check.subject.json`, and `review-input.json`
///   (the check result and exact reusable producer binding)
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
    let mut options = parse_options(args)?;
    let repo = repo_root()?;
    ensure_root_inside_invocation_repo(&repo, &options.root)?;
    if !options.base_explicit {
        // #3952 / RIPR-SPEC-0084: resolve the repository's default branch
        // through the diff loader's authority instead of assuming
        // `origin/main`, which need not exist; nothing resolving is a named
        // failure, never a guessed base recorded in the packet.
        options.base =
            crate::analysis::resolve_effective_base(&repo, None, Some(PR_EVIDENCE_GIT_DEADLINE))
                .map_err(|err| format!("pr-evidence: {err}"))?;
    }
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
                options.base_explicit = true;
            }
            "--head" => {
                i += 1;
                options.head = non_empty_arg(args, i, "--head")?.to_string();
            }
            "--check" => options.check = true,
            other => return Err(unknown_argument("pr-evidence", other)),
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
    println!("{PR_EVIDENCE_HELP}");
}

/// Help body for `ripr pr-evidence`. Also the flag source for unknown-argument
/// suggestions; keep accepted flags on option-list lines.
pub(crate) const PR_EVIDENCE_HELP: &str = "\
Write the diff-scoped PR evidence packet for one base and head.

Usage: ripr pr-evidence [--base <rev>] [--head <rev>] [--root <path>] [--check]

Options:
  --base <rev>   PR base revision. When omitted, resolved like `ripr check`:
                 origin/HEAD, then origin/main, origin/master, main, master.
  --head <rev>   PR head revision. Defaults to HEAD.
  --root <path>  Workspace to analyze, inside the repository of the current
                 directory (Git history and outputs stay there). Defaults to
                 the current directory.
  --check        Verify the existing PR evidence packet is contract-valid.

Outputs:
  target/ripr/pr/repo-exposure.json  — PR evidence JSON packet
  target/ripr/pr/repo-exposure.md   — PR evidence Markdown panel
  target/ripr/pr/pr.diff          — three-context PR presentation diff
  target/ripr/pr/check.diff       — canonical zero-context analysis input
  target/ripr/pr/check.json       — complete check result
  target/ripr/pr/check.subject.json — exact producer subject receipt
  target/ripr/pr/review-input.json — bounded reusable findings projection

This packet is diff-scoped and advisory. It does not post review
comments, edit source, or change gate semantics.
";

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
    let config = load_for_root(&root)?;
    let canonical_diff = fs::read(repo.join(PR_CANONICAL_DIFF))
        .map_err(|err| format!("read canonical diff for check subject binding: {err}"))?;
    let findings = check_value
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| "ripr check output findings must be an array".to_string())?;
    let (index, index_byte_count) = canonical_finding_index(findings, &root)?;
    let mut subject = json!({
        "schema_version": "ripr.pr_check_subject.v1",
        "root_identity": canonical_root_identity(&root),
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
        "canonical_finding_index_byte_count": index_byte_count,
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
    // Commit admission authority only after every fallible producer operation.
    crate::atomic_file::write(
        &repo.join(PR_CHECK_SUBJECT_JSON),
        subject_text.as_bytes(),
        PR_CHECK_SUBJECT_JSON,
    )
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
        "root_identity": canonical_root_identity(&root),
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
    Err(reject_pr_evidence_error_packet(&packet).unwrap_or_else(|| {
        "PR evidence producer failed; review-comments must not run: unknown producer error"
            .to_string()
    }))
}

/// Return the fail-closed error for a producer error packet, if present.
///
/// This is the single status-level actionability authority shared by the
/// public `ripr pr-evidence` command and the xtask compatibility wrapper.
pub fn reject_pr_evidence_error_packet(packet: &Value) -> Option<String> {
    (packet.get("status").and_then(Value::as_str) == Some("error")).then(|| {
        format!(
            "PR evidence producer failed; review-comments must not run: {}",
            packet["warnings"]
                .as_array()
                .and_then(|warnings| warnings.first())
                .and_then(|warning| warning["message"].as_str())
                .unwrap_or("unknown producer error")
        )
    })
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
    if let Some(error) = reject_pr_evidence_error_packet(&packet) {
        return Err(error);
    }
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

fn validate_producer_artifacts(repo: &Path, options: &PrEvidenceOptions) -> Result<(), String> {
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
    if subject.get("schema_version").and_then(Value::as_str) != Some("ripr.pr_check_subject.v1") {
        return Err(format!(
            "{PR_CHECK_SUBJECT_JSON} schema_version is unsupported"
        ));
    }
    let expected_root = repo
        .join(&options.root)
        .canonicalize()
        .map_err(|error| format!("resolve producer evidence root: {error}"))?;
    let config = load_for_root(&expected_root)?;
    let mut expected_input = CheckInput {
        root: expected_root.clone(),
        ..CheckInput::default()
    };
    apply_to_check_input(&mut expected_input, &config, CheckInputExplicit::default());
    let expected_config = repo_exposure_config_identity_hash(&config);
    let (canonical_diff_digest, _) = digest_file(&repo.join(PR_CANONICAL_DIFF))
        .map_err(|error| format!("missing or unreadable {PR_CANONICAL_DIFF}: {error}"))?;
    let expected_diff = load_canonical_check_diff(repo, options)?;
    if canonical_diff_digest != format!("sha256:{:x}", Sha256::digest(expected_diff.as_bytes())) {
        return Err(format!(
            "{PR_CANONICAL_DIFF} does not match the requested canonical base/head diff"
        ));
    }
    let expected_root = canonical_root_identity(&expected_root);
    let expected_base_sha = resolve_revision(repo, &options.base, "commit")?;
    let expected_head_sha = resolve_revision(repo, &options.head, "commit")?;
    let expected_head_tree = resolve_revision(repo, &options.head, "tree")?;
    for (field, actual, expected) in [
        (
            "root_identity",
            subject.get("root_identity").and_then(Value::as_str),
            Some(expected_root.as_str()),
        ),
        (
            "base_sha",
            subject.get("base_sha").and_then(Value::as_str),
            Some(expected_base_sha.as_str()),
        ),
        (
            "head_sha",
            subject.get("head_sha").and_then(Value::as_str),
            Some(expected_head_sha.as_str()),
        ),
        (
            "head_tree",
            subject.get("head_tree").and_then(Value::as_str),
            Some(expected_head_tree.as_str()),
        ),
        (
            "canonical_diff_sha256",
            subject.get("canonical_diff_sha256").and_then(Value::as_str),
            Some(canonical_diff_digest.as_str()),
        ),
        (
            "mode",
            subject.get("mode").and_then(Value::as_str),
            Some(expected_input.mode.as_str()),
        ),
        (
            "configuration_fingerprint",
            subject
                .get("configuration_fingerprint")
                .and_then(Value::as_str),
            Some(expected_config.as_str()),
        ),
    ] {
        if actual != expected {
            return Err(format!(
                "{PR_CHECK_SUBJECT_JSON} {field} does not match the requested analysis identity"
            ));
        }
    }
    for (field, subject_value, review_value) in [
        (
            "root_identity",
            subject.get("root_identity").and_then(Value::as_str),
            review_input.root_identity.as_str(),
        ),
        (
            "base_sha",
            subject.get("base_sha").and_then(Value::as_str),
            review_input.base_sha.as_str(),
        ),
        (
            "head_sha",
            subject.get("head_sha").and_then(Value::as_str),
            review_input.head_sha.as_str(),
        ),
        (
            "head_tree",
            subject.get("head_tree").and_then(Value::as_str),
            review_input.head_tree.as_str(),
        ),
        (
            "check_sha256",
            subject.get("check_sha256").and_then(Value::as_str),
            review_input.check_sha256.as_str(),
        ),
        (
            "canonical_diff_sha256",
            subject.get("canonical_diff_sha256").and_then(Value::as_str),
            review_input.canonical_diff_sha256.as_str(),
        ),
        (
            "mode",
            subject.get("mode").and_then(Value::as_str),
            review_input.mode.as_str(),
        ),
    ] {
        if subject_value != Some(review_value) {
            return Err(format!(
                "{PR_CHECK_SUBJECT_JSON} {field} contradicts {PR_REVIEW_INPUT_JSON}"
            ));
        }
    }
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
    if subject
        .get("canonical_finding_index_entry_count")
        .and_then(Value::as_u64)
        != Some(index.entries.len() as u64)
    {
        return Err(format!(
            "{PR_CHECK_SUBJECT_JSON} canonical finding index entry count is contradictory"
        ));
    }
    let encoded_index = serde_json::to_vec(&index.entries)
        .map_err(|error| format!("serialize canonical finding index: {error}"))?;
    if subject
        .get("canonical_finding_index_byte_count")
        .and_then(Value::as_u64)
        != Some(encoded_index.len() as u64)
    {
        return Err(format!(
            "{PR_CHECK_SUBJECT_JSON} canonical finding index byte count is contradictory"
        ));
    }
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
    // Raw NUL-delimited inventory (#4006): `-z` output is never C-quoted,
    // so exotic names survive byte-exact; parsing rules come from the
    // shared authority in `decode_changed_files`, not from line splitting
    // here. The `--diff-filter=ACMR` scope contract is unchanged.
    let range = format!("{}...{}", options.base, options.head);
    let output = run_git_output_bytes(
        repo,
        &[
            "diff",
            "--name-only",
            "-z",
            "--diff-filter=ACMR",
            range.as_str(),
        ],
    )?;
    decode_changed_files(&output)
}

/// Decode raw `--name-only -z` bytes through the shared NUL path-record
/// authority (#4006). Strict: non-UTF-8 or empty records fail loudly
/// instead of collapsing through lossy conversion.
fn decode_changed_files(output: &[u8]) -> Result<Vec<String>, String> {
    crate::analysis::parse_git_path_records(output)
        .map_err(|err| format!("pr-evidence changed-file inventory: {err}"))
        .and_then(|paths| {
            paths
                .iter()
                .map(|path| {
                    path.to_str().map(str::to_string).ok_or_else(|| {
                        format!(
                            "pr-evidence changed-file inventory: decoded path {} is not valid UTF-8",
                            path.display()
                        )
                    })
                })
                .collect()
        })
}

fn write_diff(repo: &Path, options: &PrEvidenceOptions) -> Result<(), String> {
    let out = repo.join(PR_DIFF);
    // Route the packet diff through the shared pinned Git assembly (issue
    // #3930) rather than restating flags: ambient presentation helpers
    // (external diff, textconv, color) must not change what the packet
    // records. `--binary` stays the caller extra and the evidence path
    // selects three context lines (the pre-#3930 presentation); the
    // assembly pins `-c core.quotePath=true`, `--no-ext-diff`,
    // `--no-textconv`, `--no-color`, `--src-prefix=a/`,
    // `--dst-prefix=b/`, and `--inter-hunk-context=0`.
    let diff = crate::analysis::load_pr_evidence_diff_range(repo, &options.base, &options.head)?;
    write_parented_file(&out, PR_DIFF, diff)?;
    let canonical = load_canonical_check_diff(repo, options)?;
    write_parented_file(&repo.join(PR_CANONICAL_DIFF), PR_CANONICAL_DIFF, canonical)
}

/// Keep the analysis/identity bytes on the same pinned range assembly as
/// review-comments (zero context, no external diff/textconv, short submodules).
/// The caller retains the packet diff's existing five-minute full-diff ceiling.
fn load_canonical_check_diff(repo: &Path, options: &PrEvidenceOptions) -> Result<String, String> {
    crate::analysis::load_canonical_pr_evidence_diff_range(repo, &options.base, &options.head)
}

/// Run the RIPR check in-process over the exact zero-context canonical input.
/// The loaded repository configuration drives both analysis and the producer
/// identity, matching the ordinary check CLI and review-comments consumer.
fn run_ripr_check(repo: &Path, options: &PrEvidenceOptions) -> Result<String, String> {
    let diff_path = repo.join(PR_CANONICAL_DIFF);
    let root_path = command_root_path(repo, &options.root);
    let config = load_for_root(&root_path)?;
    let mut input = CheckInput {
        root: root_path,
        base: None,
        diff_file: Some(diff_path),
        mode: Mode::Draft,
        format: OutputFormat::Json,
        include_unchanged_tests: true,
        perl_facts_path: None,
        suppression_policy: None,
        git_timeout: None,
        git_candidate: None,
    };
    apply_to_check_input(&mut input, &config, CheckInputExplicit::default());
    let output = check_workspace_with_config(input, &config)?;
    // #5203: the internal packet input renders unbounded. Routing counts
    // the full finding set; the external findings-array byte budget must
    // not silently truncate the counts this packet routes from (Codex P1:
    // a bounded prefix under-counted severe gaps with no disclosure).
    Ok(crate::app::render_check_json_unbounded(&output))
}

/// `pr-evidence` reads Git history and writes `target/ripr/pr/` from the
/// invocation directory, and analyzes the `--root` workspace with that diff.
/// A `--root` outside the invocation repository would pair one repository's
/// diff with another's source and stamp the packet with the selected root,
/// so a clean-looking packet could describe a change it never read. Refuse
/// a missing, file-typed, or foreign root before any Git read or write.
fn ensure_root_inside_invocation_repo(repo: &Path, root: &str) -> Result<(), String> {
    let root_path = command_root_path(repo, root);
    if !root_path.is_dir() {
        return Err(format!(
            "pr-evidence root {root} is not a directory; pass `--root` naming an existing \
             directory inside the repository you run `ripr pr-evidence` from"
        ));
    }
    let same_directory = match (fs::canonicalize(repo), fs::canonicalize(&root_path)) {
        (Ok(repo), Ok(root)) => repo == root,
        _ => false,
    };
    if same_directory {
        return Ok(());
    }
    let toplevel =
        |dir: &Path| crate::git::discovered_work_tree_toplevel(dir, PR_EVIDENCE_GIT_DEADLINE);
    match (toplevel(repo), toplevel(&root_path)) {
        (Some(invocation), Some(selected)) if invocation == selected => Ok(()),
        _ => Err(format!(
            "pr-evidence root {root} is not inside the Git work tree of the current directory \
             ({}); pr-evidence reads history and writes target/ripr/pr/ from the current \
             directory, so run it from the repository that contains the root \
             (for example `cd <repository> && ripr pr-evidence --root <member>`)",
            repo.display()
        )),
    }
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
    String::from_utf8(run_git_output_bytes(repo, args)?)
        .map_err(|err| format!("git {args:?} produced non-UTF-8 output: {err}"))
}

/// Cooperative deadline for every git probe in this file (#2303, #4363).
/// PR evidence answers bounded revision and path-inventory questions; a hung
/// git must not pin the command. One minute matches the other bounded git
/// consumers.
const PR_EVIDENCE_GIT_DEADLINE: Duration = Duration::from_mins(1);

/// Capture raw git stdout bytes through this file's single git call site
/// (#4006). Path inventories decode through the shared NUL authority at
/// the call site; other callers keep the strict UTF-8 wrapper above. The
/// spawn goes through the shared `crate::git` deadline and process-owner
/// authority (#4363).
fn run_git_output_bytes(repo: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    run_git_output_bytes_within(repo, args, PR_EVIDENCE_GIT_DEADLINE)
}

/// [`run_git_output_bytes`] with the deadline as a parameter, so a test can
/// prove the deadline reaches the git runner.
fn run_git_output_bytes_within(
    repo: &Path,
    args: &[&str],
    deadline: Duration,
) -> Result<Vec<u8>, String> {
    let output = crate::git::run_git_output_with_deadline(repo, args, Some(deadline))
        .map_err(|err| format!("failed to run git {args:?}: {err}"))?;
    if output.status.success() {
        Ok(output.stdout)
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
    // Candidate-actionable eligibility (#3281): PR routing counts current
    // candidate-side obligations, not base-side evidence. Recompute the
    // three severe-gap classes from the findings array when it is present;
    // fall back to the classification summary only when it is not.
    let candidate_current_counts =
        check_value
            .get("findings")
            .and_then(Value::as_array)
            .map(|findings| {
                let mut weakly_exposed = 0;
                let mut reachable_unrevealed = 0;
                let mut no_static_path = 0;
                for finding in findings {
                    if finding.get("source_currentness").and_then(Value::as_str)
                        != Some("candidate_current")
                    {
                        continue;
                    }
                    match finding.get("classification").and_then(Value::as_str) {
                        Some("weakly_exposed") => weakly_exposed += 1,
                        Some("reachable_unrevealed") => reachable_unrevealed += 1,
                        Some("no_static_path") => no_static_path += 1,
                        _ => {}
                    }
                }
                (weakly_exposed, reachable_unrevealed, no_static_path)
            });
    let (weakly_exposed, reachable_unrevealed, no_static_path) = candidate_current_counts
        .unwrap_or_else(|| {
            (
                count_field(check_summary, "weakly_exposed"),
                count_field(check_summary, "reachable_unrevealed"),
                count_field(check_summary, "no_static_path"),
            )
        });
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
        "- root: {}\n",
        code_span(string_field(packet, "root", "."))
    ));
    out.push_str(&format!(
        "- base: {}\n",
        code_span(string_field(packet, "base", DEFAULT_BASE))
    ));
    out.push_str(&format!(
        "- head: {}\n",
        code_span(string_field(packet, "head", DEFAULT_HEAD))
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
        "- routing_reason: {}\n\n",
        code_span(routing_reason)
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
                "| {} | {} | {} | {} |\n",
                table_cell_text(string_field(artifact, "label", "artifact")),
                table_code_span(string_field(artifact, "path", "unknown")),
                table_cell_text(string_field(artifact, "scope", "unknown")),
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
                inline_prose(string_field(warning, "kind", "warning")),
                inline_prose(string_field(
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
    out.push_str(&format!("- route: {}\n", code_span(status)));
    if let Some(candidates) = route.get("candidates").and_then(Value::as_array) {
        for candidate in candidates {
            let Some(candidate) = candidate.as_object() else {
                continue;
            };
            out.push_str(&format!(
                "- candidate: {}:{} {} -> {}\n- command: {}\n- expected: {}\n",
                code_span(
                    candidate
                        .get("file")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                ),
                candidate.get("line").and_then(Value::as_u64).unwrap_or(0),
                inline_prose(candidate.get("from").and_then(Value::as_str).unwrap_or("?")),
                inline_prose(candidate.get("to").and_then(Value::as_str).unwrap_or("?")),
                code_span(
                    candidate
                        .get("command")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                ),
                inline_prose(
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
                "- limitation: {}\n",
                code_span(
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
    use crate::review_input::{REVIEW_INDEX_MAX_BYTES, REVIEW_INDEX_MAX_ENTRIES};

    fn options() -> PrEvidenceOptions {
        PrEvidenceOptions {
            root: ".".to_string(),
            base: "origin/main".to_string(),
            base_explicit: true,
            head: "HEAD".to_string(),
            check: false,
        }
    }

    #[test]
    fn run_git_output_bytes_forwards_its_deadline_to_git() -> Result<(), String> {
        // #4363: a zero deadline is refused before spawn with the named
        // timeout error. Dropping the deadline would instead report the
        // missing root as a spawn failure, which this rejects.
        let missing = std::env::temp_dir().join(format!(
            "ripr-pr-evidence-deadline-missing-{}",
            std::process::id()
        ));
        let bounded =
            run_git_output_bytes_within(&missing, &["rev-parse", "HEAD"], Duration::from_mins(1));
        match bounded {
            Err(err) if !err.contains(crate::git::GIT_INVOCATION_TIMEOUT_PREFIX) => {}
            other => return Err(format!("control: expected a spawn failure, got {other:?}")),
        }
        match run_git_output_bytes_within(&missing, &["rev-parse", "HEAD"], Duration::ZERO) {
            Err(err) if err.contains(crate::git::GIT_INVOCATION_TIMEOUT_PREFIX) => Ok(()),
            other => Err(format!("zero deadline must be refused, got {other:?}")),
        }
    }

    /// Issue #3930: the PR-evidence diff rides the same pinned Git
    /// presentation as the analysis loaders. On an ordinary repository the
    /// packet diff must be byte-identical to the pre-repair argv output;
    /// non-UTF-8 git output must fail with a named error rather than
    /// record replacement characters (the pre-#3930 contract); and a
    /// textconv driver that hides source must not reach the packet
    /// artifact: the pre-repair argv is the control (an empty patch proves
    /// the fixture hides the edit), and `write_diff` must retain the
    /// source edit.
    #[test]
    fn write_diff_ignores_textconv_like_analysis_loaders() -> Result<(), String> {
        use crate::testing::fixture_git::{fixture_git_ok, remove_fixture_tree};
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};

        /// Scoped cleanup: the unique temporary repository is removed on
        /// every exit, including early `?` returns and panics (Windows
        /// readonly Git objects included).
        struct FixtureGuard<'a> {
            repo: &'a Path,
        }
        impl Drop for FixtureGuard<'_> {
            fn drop(&mut self) {
                let _ = remove_fixture_tree(self.repo);
            }
        }

        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system time before unix epoch: {error}"))?
            .as_nanos();
        let repo = std::env::temp_dir().join(format!(
            "ripr-pr-evidence-textconv-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _guard = FixtureGuard { repo: &repo };
        fs::create_dir_all(repo.join("src"))
            .map_err(|error| format!("create fixture src failed: {error}"))?;
        fixture_git_ok(&repo, &["init", "--initial-branch=main"])?;
        for (key, value) in [
            ("user.name", "PR Evidence"),
            ("user.email", "pr-evidence@example.com"),
            ("commit.gpgsign", "false"),
            ("core.autocrlf", "false"),
        ] {
            fixture_git_ok(&repo, &["config", "--local", key, value])?;
        }
        fs::write(repo.join(".gitattributes"), "src/lib.rs diff=audit\n")
            .map_err(|error| format!("write gitattributes failed: {error}"))?;
        // Seven lines with the edit on line 4: three context lines exist
        // on each side, so `--unified=0` and `--unified=3` produce
        // different bytes and the byte-identity pin below discriminates
        // the evidence-path context selection.
        fs::write(
            repo.join("src/lib.rs"),
            "pub const A: u32 = 1;\n\
             pub const B: u32 = 2;\n\
             pub const C: u32 = 3;\n\
             pub const VALUE: u32 = 1;\n\
             pub const D: u32 = 4;\n\
             pub const E: u32 = 5;\n\
             pub const F: u32 = 6;\n",
        )
        .map_err(|error| format!("write base source failed: {error}"))?;
        fixture_git_ok(&repo, &["add", "."])?;
        fixture_git_ok(&repo, &["commit", "--quiet", "-m", "base"])?;
        fixture_git_ok(&repo, &["tag", "evidence-base"])?;
        fs::write(
            repo.join("src/lib.rs"),
            "pub const A: u32 = 1;\n\
             pub const B: u32 = 2;\n\
             pub const C: u32 = 3;\n\
             pub const VALUE: u32 = 2;\n\
             pub const D: u32 = 4;\n\
             pub const E: u32 = 5;\n\
             pub const F: u32 = 6;\n",
        )
        .map_err(|error| format!("write edited source failed: {error}"))?;
        fixture_git_ok(&repo, &["add", "src/lib.rs"])?;
        fixture_git_ok(&repo, &["commit", "--quiet", "-m", "edit"])?;
        let range = "evidence-base...HEAD";
        let options = PrEvidenceOptions {
            root: ".".to_string(),
            base: "evidence-base".to_string(),
            base_explicit: true,
            head: "HEAD".to_string(),
            check: false,
        };
        // Byte-compatibility: with no ambient diff configuration yet, the
        // routed packet diff must be byte-identical to the pre-repair argv
        // output (three context lines, the pre-#3930 presentation). The
        // baseline states context and color explicitly so ambient
        // `diff.context` / color configuration cannot move it while the
        // packet command stays pinned; on ordinary repositories this is
        // exactly what the pre-repair argv emitted.
        let baseline = &[
            "diff",
            "--binary",
            "--no-ext-diff",
            "--unified=3",
            "--no-color",
            range,
        ];
        let raw = run_git_output(&repo, baseline)?;
        write_diff(&repo, &options)?;
        let pinned = fs::read(repo.join(PR_DIFF))
            .map_err(|error| format!("read packet diff failed: {error}"))?;
        assert_eq!(
            pinned,
            raw.as_bytes(),
            "packet diff must stay byte-identical to the pre-repair argv on ordinary repositories"
        );
        // Strictness: a tracked text file with non-UTF-8 bytes must fail
        // the packet diff with a named error (the pre-#3930 contract),
        // never record replacement characters. No textconv driver is
        // configured yet, so the raw bytes reach the decode. The edit is
        // restored afterwards for the textconv phases below.
        fs::write(
            repo.join("src/lib.rs"),
            b"pub const VALUE: u32 = 1;\nlatin1: \xe9\n",
        )
        .map_err(|error| format!("write non-UTF-8 source failed: {error}"))?;
        fixture_git_ok(&repo, &["add", "src/lib.rs"])?;
        fixture_git_ok(&repo, &["commit", "--quiet", "-m", "non-utf8"])?;
        match write_diff(&repo, &options) {
            Ok(()) => {
                return Err("packet diff must reject non-UTF-8 git output".to_string());
            }
            Err(err) => assert!(
                err.contains("UTF-8"),
                "unexpected strict-decode error: {err}"
            ),
        }
        fs::write(
            repo.join("src/lib.rs"),
            "pub const A: u32 = 1;\n\
             pub const B: u32 = 2;\n\
             pub const C: u32 = 3;\n\
             pub const VALUE: u32 = 2;\n\
             pub const D: u32 = 4;\n\
             pub const E: u32 = 5;\n\
             pub const F: u32 = 6;\n",
        )
        .map_err(|error| format!("restore edited source failed: {error}"))?;
        fixture_git_ok(&repo, &["add", "src/lib.rs"])?;
        fixture_git_ok(&repo, &["commit", "--quiet", "-m", "restore"])?;
        // Git itself is the constant-output helper on Unix and
        // Windows; no shell script, executable permission, or global
        // environment mutation.
        fixture_git_ok(
            &repo,
            &["config", "--local", "diff.audit.textconv", "git --version"],
        )?;
        // Control: the pre-repair argv lets the textconv hide the source
        // edit. Without this control a broken fixture could let the
        // regression pass. Same isolated baseline as above; the textconv
        // driver still comes from the fixture-local configuration, so the
        // control keeps its meaning under ambient git configuration.
        let raw = run_git_output(&repo, baseline)?;
        assert!(
            raw.trim().is_empty(),
            "the constant textconv must hide the source edit"
        );
        write_diff(&repo, &options)?;
        let diff = fs::read_to_string(repo.join(PR_DIFF))
            .map_err(|error| format!("read packet diff failed: {error}"))?;
        assert!(
            diff.contains("pub const VALUE"),
            "packet diff must retain the source edit despite textconv"
        );
        assert!(
            diff.contains("pub const C: u32 = 3;") && diff.contains("pub const D: u32 = 4;"),
            "packet diff must keep the three-line presentation around the edit"
        );
        assert!(
            !diff.contains('\u{1b}'),
            "packet diff must not contain color"
        );
        Ok(())
    }

    #[test]
    fn parse_defaults_and_check_mode() -> Result<(), String> {
        assert_eq!(
            parse_options(&[])?,
            PrEvidenceOptions {
                base_explicit: false,
                ..options()
            }
        );
        let parsed = parse_options(&["--base".into(), "main".into(), "--check".into()])?;
        assert_eq!(parsed.base, "main");
        assert!(parsed.base_explicit);
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
            "findings": [
                {
                    "classification": "weakly_exposed",
                    "source_currentness": "candidate_current",
                    "probe": {
                        "family": "predicate",
                        "file": "src/lib.rs",
                        "line": 8,
                        "expression": "amount >= threshold"
                    }
                },
                {
                    "classification": "weakly_exposed",
                    "source_currentness": "candidate_current",
                    "probe": {
                        "family": "predicate",
                        "file": "src/other.rs",
                        "line": 3,
                        "expression": "other >= bound"
                    }
                },
                {
                    "classification": "reachable_unrevealed",
                    "source_currentness": "candidate_current",
                    "probe": {
                        "family": "side_effect",
                        "file": "src/lib.rs",
                        "line": 12,
                        "expression": "publish(event)"
                    }
                }
            ]
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
                    "source_currentness": "candidate_current",
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
                "source_currentness": "candidate_current",
                "probe": {"family": "predicate", "file": "src/lib.rs", "line": 8, "expression": format!("value {operator} limit")}
            })
        })
        .collect::<Vec<_>>();
        findings.push(json!({
            "classification": "weakly_exposed",
            "source_currentness": "candidate_current",
            "probe": {"family": "predicate", "file": "src/lib.rs", "line": 8, "expression": "value >= limit"}
        }));
        findings.push(json!({
            "classification": "weakly_exposed",
            "source_currentness": "candidate_current",
            "probe": {"family": "call_presence", "file": "src/lib.rs", "line": 9, "expression": "publish(value)"}
        }));
        findings.push(
            json!({"classification": "weakly_exposed", "source_currentness": "candidate_current"}),
        );
        findings.push(json!({
            "classification": "weakly_exposed",
            "source_currentness": "candidate_current",
            "probe": {"family": "predicate", "line": 10, "expression": "value >= limit"}
        }));
        findings.push(json!({
            "classification": "weakly_exposed",
            "source_currentness": "candidate_current",
            "probe": {"family": "predicate", "file": "src/lib.rs", "expression": "value >= limit"}
        }));
        findings.push(json!({
            "classification": "weakly_exposed",
            "source_currentness": "candidate_current",
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
                    {"classification": "weakly_exposed", "source_currentness": "candidate_current", "probe": {"family": "predicate", "file": "src/lib.rs", "line": 8, "expression": "value >= limit"}},
                    {"classification": "weakly_exposed", "source_currentness": "candidate_current", "probe": {"family": "call_presence", "file": "src/lib.rs", "line": 9, "expression": "publish(value)"}}
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
        let generation_error = match write_pr_evidence_with_runner(
            &repo,
            &options,
            |_repo, _options| {
                Err("ripr check for PR evidence failed; retry command: ripr pr-evidence --base HEAD~1 --head HEAD --root .".to_string())
            },
        ) {
            Ok(()) => return Err("producer failure did not fail closed".to_string()),
            Err(error) => error,
        };
        assert!(generation_error.contains("producer failed"));

        let check_error = match check_pr_evidence(&repo, &options) {
            Ok(()) => return Err("standalone check accepted preserved error packet".to_string()),
            Err(error) => error,
        };
        assert!(check_error.contains("producer failed"));
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

    #[cfg(feature = "lang-rust")]
    #[test]
    fn failed_producer_rerun_does_not_replay_same_subject() -> Result<(), String> {
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
            let mut check = String::new();
            write_pr_evidence_with_runner(&repo, &options, |repo, options| {
                let generated = run_ripr_check(repo, options)?;
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
                crate::cli::run(vec![
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
            let entries = crate::review_input::canonical_projection_all(byte_findings, &repo)?;
            let legacy = serde_json::to_vec(&entries).map_err(|error| error.to_string())?;
            assert!(legacy.len() > REVIEW_INDEX_MAX_BYTES);
            let selected = crate::review_input::canonical_projection(byte_findings, &repo)?;
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

    /// The actual producer must be reusable by the strict consumer, including
    /// a nondefault effective mode. Wrong inputs cannot become admitted simply
    /// because nearby JSON or an internally consistent presentation diff exists.
    #[cfg(feature = "lang-rust")]
    #[test]
    fn actual_pr_producer_preserves_config_and_canonical_review_identity() -> Result<(), String> {
        use crate::testing::fixture_git::fixture_git_ok;

        struct OwnedProducerFixture(PathBuf);
        impl Drop for OwnedProducerFixture {
            fn drop(&mut self) {
                let _ = crate::testing::fixture_git::remove_fixture_tree(&self.0);
            }
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let repo = std::env::temp_dir().join(format!(
            "ripr-pr-producer-identity-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&repo).map_err(|error| error.to_string())?;
        let _fixture = OwnedProducerFixture(repo.clone());
        fixture_git_ok(
            &repo,
            &["-c", "init.templateDir=", "init", "--quiet", "-b", "trunk"],
        )?;
        fixture_git_ok(&repo, &["config", "user.name", "RIPR Producer Fixture"])?;
        fixture_git_ok(
            &repo,
            &["config", "user.email", "producer-fixture@example.invalid"],
        )?;
        fixture_git_ok(&repo, &["config", "commit.gpgSign", "false"])?;
        write_repo_file(
            &repo,
            "Cargo.toml",
            "[package]\nname = \"pricing\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )?;
        let original = "pub const DISCOUNT_THRESHOLD: u64 = 10_000;\n\npub fn discounted_total(amount: u64) -> u64 {\n    if amount > DISCOUNT_THRESHOLD {\n        amount - amount / 10\n    } else {\n        amount\n    }\n}\n";
        write_repo_file(&repo, "src/lib.rs", original)?;
        write_repo_file(
            &repo,
            "tests/pricing.rs",
            "use pricing::discounted_total;\n#[test]\nfn below_threshold() { assert_eq!(discounted_total(5_000), 5_000); }\n",
        )?;
        fixture_git_ok(&repo, &["add", "-A"])?;
        fixture_git_ok(&repo, &["commit", "--quiet", "-m", "base"])?;
        write_repo_file(
            &repo,
            "src/lib.rs",
            &original.replace(
                "amount > DISCOUNT_THRESHOLD",
                "amount >= DISCOUNT_THRESHOLD",
            ),
        )?;
        fixture_git_ok(&repo, &["commit", "--quiet", "-a", "-m", "boundary"])?;
        let options = PrEvidenceOptions {
            base: "HEAD~1".to_string(),
            head: "HEAD".to_string(),
            ..options()
        };
        let canonical = crate::analysis::load_diff_range_with_deadline_core(
            &repo,
            &options.base,
            &options.head,
            Some(Duration::from_mins(1)),
        )
        .map_err(|error| error.to_string())?;
        assert!(canonical.contains("+    if amount >= DISCOUNT_THRESHOLD"));
        let check_path = repo.join(PR_CHECK_JSON);
        for mode in ["draft", "fast"] {
            if mode == "fast" {
                write_repo_file(
                    &repo,
                    "ripr.toml",
                    "[analysis]\nmode = \"fast\"\n[languages]\nenabled = [\"rust\"]\n",
                )?;
            }
            let config = load_for_root(&repo)?;
            let mut input = CheckInput {
                root: repo.clone(),
                ..CheckInput::default()
            };
            apply_to_check_input(&mut input, &config, CheckInputExplicit::default());
            assert_eq!(input.mode.as_str(), mode);
            write_pr_evidence(&repo, &options)?;
            check_pr_evidence(&repo, &options)?;
            assert_eq!(
                fs::read(repo.join(PR_CANONICAL_DIFF)).map_err(|e| e.to_string())?,
                canonical.as_bytes()
            );
            let presentation = fs::read(repo.join(PR_DIFF)).map_err(|e| e.to_string())?;
            assert_ne!(
                presentation,
                canonical.as_bytes(),
                "the fixture must distinguish three-context presentation from canonical analysis input"
            );
            let subject_bytes =
                fs::read(repo.join(PR_CHECK_SUBJECT_JSON)).map_err(|e| e.to_string())?;
            let subject: Value =
                serde_json::from_slice(&subject_bytes).map_err(|e| e.to_string())?;
            let check_bytes = fs::read(&check_path).map_err(|e| e.to_string())?;
            let check: Value = serde_json::from_slice(&check_bytes).map_err(|e| e.to_string())?;
            assert_eq!(check["mode"], mode);
            assert_eq!(subject["mode"], mode);
            assert_eq!(
                subject["check_sha256"],
                json!(format!("sha256:{:x}", Sha256::digest(&check_bytes)))
            );
            assert_eq!(
                subject["canonical_diff_sha256"],
                json!(format!("sha256:{:x}", Sha256::digest(canonical.as_bytes())))
            );
            let admitted = crate::app::review_comments::admit_producer_evidence(
                &check_path,
                &input,
                &config,
                &options.base,
                &options.head,
                &canonical,
            )
            .map_err(|error| format!("actual producer was not admitted: {}", error.message))?;
            assert_eq!(admitted.identity.mode, mode);
            assert!(admitted.outcome.counts.finding_count > 0);
            assert!(!admitted.producer_projection.is_empty());
            assert_eq!(
                admitted.identity.configuration_fingerprint,
                repo_exposure_config_identity_hash(&config)
            );

            // An ordinary redirected check JSON without its subject is not a
            // producer packet, even when that JSON and review input are real.
            fs::remove_file(repo.join(PR_CHECK_SUBJECT_JSON)).map_err(|e| e.to_string())?;
            let missing = crate::app::review_comments::admit_producer_evidence(
                &check_path,
                &input,
                &config,
                &options.base,
                &options.head,
                &canonical,
            )
            .err()
            .ok_or_else(|| "bare check JSON was admitted".to_string())?;
            assert_eq!(missing.category, "missing_producer");
            fs::write(repo.join(PR_CHECK_SUBJECT_JSON), &subject_bytes)
                .map_err(|e| e.to_string())?;
            let wrong_head = crate::app::review_comments::admit_producer_evidence(
                &check_path,
                &input,
                &config,
                &options.base,
                "HEAD~1",
                &canonical,
            )
            .err()
            .ok_or_else(|| "wrong head was admitted".to_string())?;
            assert_eq!(wrong_head.category, "producer_identity_mismatch");
            assert!(wrong_head.message.contains("head_sha"));
            let wrong_diff = crate::app::review_comments::admit_producer_evidence(
                &check_path,
                &input,
                &config,
                &options.base,
                &options.head,
                std::str::from_utf8(&presentation).map_err(|e| e.to_string())?,
            )
            .err()
            .ok_or_else(|| "presentation diff was admitted as canonical input".to_string())?;
            assert_eq!(wrong_diff.category, "producer_identity_mismatch");
            assert!(wrong_diff.message.contains("canonical_diff_sha256"));
            fs::write(repo.join(PR_CANONICAL_DIFF), &presentation).map_err(|e| e.to_string())?;
            let invalid_input = check_pr_evidence(&repo, &options).err().ok_or_else(|| {
                "standalone check accepted the wrong canonical input file".to_string()
            })?;
            assert!(invalid_input.contains("requested canonical base/head diff"));
            fs::write(repo.join(PR_CANONICAL_DIFF), canonical.as_bytes())
                .map_err(|e| e.to_string())?;
            check_pr_evidence(&repo, &options)?;
        }
        // Same mode, changed configuration fingerprint must fail reuse and
        // standalone validation; a changed effective mode gets its own refusal.
        let bound_config = load_for_root(&repo)?;
        let bound_fingerprint = repo_exposure_config_identity_hash(&bound_config);
        write_repo_file(
            &repo,
            "ripr.toml",
            "[analysis]\nmode = \"fast\"\n[languages]\nenabled = [\"rust\"]\n[oracles]\nsnapshot_strength = \"strong\"\n",
        )?;
        let drift = load_for_root(&repo)?;
        assert_ne!(
            repo_exposure_config_identity_hash(&drift),
            bound_fingerprint
        );
        let mut input = CheckInput {
            root: repo.clone(),
            ..CheckInput::default()
        };
        apply_to_check_input(&mut input, &drift, CheckInputExplicit::default());
        let stale_config = crate::app::review_comments::admit_producer_evidence(
            &check_path,
            &input,
            &drift,
            &options.base,
            &options.head,
            &canonical,
        )
        .err()
        .ok_or_else(|| "changed configuration was admitted".to_string())?;
        assert_eq!(stale_config.category, "producer_identity_mismatch");
        assert!(stale_config.message.contains("configuration_fingerprint"));
        assert!(
            check_pr_evidence(&repo, &options)
                .err()
                .is_some_and(|e| e.contains("configuration_fingerprint"))
        );
        input.mode = Mode::Deep;
        let wrong_mode = crate::app::review_comments::admit_producer_evidence(
            &check_path,
            &input,
            &drift,
            &options.base,
            &options.head,
            &canonical,
        )
        .err()
        .ok_or_else(|| "wrong mode was admitted".to_string())?;
        assert_eq!(wrong_mode.category, "producer_mode_mismatch");
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
        // Candidate-currentness contract: the packet recomputes severe-gap
        // counts from candidate_current findings, not from the check summary.
        let check_json = r#"{
          "schema_version": "ripr.check.v1",
          "mode": "draft",
          "analysis_outcome": {"analysis_complete": true},
          "findings": [
            {
              "id": "probe:src_lib_rs:value:00000000",
              "classification": "weakly_exposed",
              "severity": "medium",
              "source_currentness": "candidate_current",
              "probe": {"family": "predicate", "file": "src/lib.rs", "line": 1}
            }
          ],
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
        let review_input = serde_json::from_slice::<Value>(&review_input_bytes)
            .map_err(|err| format!("parse review input: {err}"))?;

        for (field, mutation) in [
            ("check_sha256", json!("sha256:wrong")),
            ("check_byte_count", json!(0)),
            ("schema_version", json!("ripr.pr_check_subject.v0")),
            ("root_identity", json!("/another/repository")),
            ("base_sha", json!("sha256:other")),
            ("head_sha", json!("sha256:other")),
            ("head_tree", json!("sha256:other")),
            ("canonical_finding_index", json!("invalid")),
            ("review_input_sha256", json!("sha256:wrong")),
            ("canonical_finding_index_entry_count", json!(2)),
            ("canonical_finding_index_byte_count", json!(1)),
            ("review_input_byte_count", json!(0)),
        ] {
            reject_subject_mutation(
                &repo,
                &options,
                &subject_path,
                &subject_bytes,
                field,
                mutation,
            )?;
        }

        let mut substituted = review_input.clone();
        substituted["findings"] = json!([{
            "stable_id": "substituted", "file": "src/lib.rs", "line": 1,
            "severity": "warning", "finding_class": "exposed", "summary": "substituted",
            "evidence_digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "related_test": null
        }]);
        substituted["reviewed_count"] = json!(1);
        substituted["projected_finding_count"] = json!(1);
        reject_review_mutation(
            &repo,
            &options,
            &review_input_path,
            &review_input_bytes,
            substituted,
            "projection",
        )?;

        let mut mode = review_input.clone();
        mode["mode"] = json!("fast");
        reject_review_mutation(
            &repo,
            &options,
            &review_input_path,
            &review_input_bytes,
            mode,
            "mode",
        )?;

        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    fn reject_subject_mutation(
        repo: &Path,
        options: &PrEvidenceOptions,
        path: &Path,
        original: &[u8],
        field: &str,
        mutation: Value,
    ) -> Result<(), String> {
        let mut value: Value = serde_json::from_slice(original).map_err(|err| err.to_string())?;
        value[field] = mutation;
        fs::write(
            path,
            serde_json::to_vec(&value).map_err(|err| err.to_string())?,
        )
        .map_err(|err| format!("write subject {field} mutation: {err}"))?;
        let rejected = check_pr_evidence(repo, options).is_err();
        fs::write(path, original).map_err(|err| format!("restore subject: {err}"))?;
        if rejected {
            Ok(())
        } else {
            Err(format!("subject {field} mutation must fail"))
        }
    }

    fn reject_review_mutation(
        repo: &Path,
        options: &PrEvidenceOptions,
        path: &Path,
        original: &[u8],
        value: Value,
        label: &str,
    ) -> Result<(), String> {
        fs::write(
            path,
            serde_json::to_vec(&value).map_err(|err| err.to_string())?,
        )
        .map_err(|err| format!("write review {label} mutation: {err}"))?;
        let rejected = check_pr_evidence(repo, options).is_err();
        fs::write(path, original).map_err(|err| format!("restore review input: {err}"))?;
        if rejected {
            Ok(())
        } else {
            Err(format!("review {label} mutation must fail"))
        }
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

    #[test]
    fn strict_changed_file_inventory_rejects_non_utf8() -> Result<(), String> {
        // The strict-failure side of the NUL authority at the product
        // pr-evidence decode boundary: non-UTF-8 records fail loudly
        // instead of collapsing through lossy conversion.
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

    #[test]
    fn changed_files_decode_exotic_names_exact() -> Result<(), String> {
        // Discriminates NUL-delimited inventory (#4006): space and
        // non-ASCII names must decode byte-exact; the old line parser kept
        // git's C-quoted octal form. Asserts through the real
        // `changed_files` production path.
        let repo = names_repo("ripr-pr-evidence-names")?;
        run_git(&repo, &["init"])?;
        run_git(&repo, &["config", "user.email", "ripr@example.invalid"])?;
        run_git(&repo, &["config", "user.name", "RIPR Test"])?;
        write_repo_file(&repo, "base.txt", "base\n")?;
        run_git(&repo, &["add", "."])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "initial"])?;
        write_repo_file(&repo, "sp ace.txt", "spaces\n")?;
        write_repo_file(&repo, "uni-\u{e9}.txt", "unicode\n")?;
        run_git(&repo, &["add", "-A"])?;
        run_git(&repo, &["commit", "--no-gpg-sign", "-m", "exotic"])?;

        let options = PrEvidenceOptions {
            base: "HEAD~1".to_string(),
            head: "HEAD".to_string(),
            ..PrEvidenceOptions::default()
        };
        let mut files = changed_files(&repo, &options)?;
        files.sort();
        let expected = vec!["sp ace.txt".to_string(), "uni-\u{e9}.txt".to_string()];
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        if files != expected {
            return Err(format!(
                "exotic changed-file inventory mismatch: got {files:?}, want {expected:?}"
            ));
        }
        Ok(())
    }

    fn names_repo(name: &str) -> Result<PathBuf, String> {
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

    #[test]
    fn experimental_complete_generation_cannot_be_laundered_through_status_ok() {
        for generation in [Value::Null, json!({"coverage":"complete","production_admission":true})] {
            let mut packet = json!({"status":"ok","analysis_complete":true});
            packet["experimental_complete_execution"] = generation;
            assert!(reject_pr_evidence_error_packet(&packet).is_some(),
                "experimental generation is not qualified for production admission");
        }
    }
}
