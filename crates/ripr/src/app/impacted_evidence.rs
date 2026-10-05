//! `ripr impacted-evidence` — binary-first mutation-routing evidence (item 8e).
//!
//! Ports `cargo xtask impacted-evidence` into the `ripr` binary so downstream
//! consumers can route mutation mode without compiling their own xtask. Reads
//! `target/ripr/pr/repo-exposure.json` + PR labels and emits
//! `target/xtask/impacted-evidence/latest.{json,md}`.

use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::unknown_argument;
use crate::output::markdown::{code_span, inline_prose, table_cell_text, table_code_span};

const DEFAULT_PR_EVIDENCE_JSON: &str = "target/ripr/pr/repo-exposure.json";
const IMPACTED_JSON: &str = "target/xtask/impacted-evidence/latest.json";
const IMPACTED_MD: &str = "target/xtask/impacted-evidence/latest.md";

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImpactedEvidenceOptions {
    pr_evidence: String,
    labels: Vec<String>,
    check: bool,
}

impl Default for ImpactedEvidenceOptions {
    fn default() -> Self {
        Self {
            pr_evidence: DEFAULT_PR_EVIDENCE_JSON.to_string(),
            labels: labels_from_env(),
            check: false,
        }
    }
}

pub(crate) fn run_impacted_evidence(args: &[String]) -> Result<(), String> {
    run_impacted_evidence_at(&repo_root()?, args)
}

/// Shared entry point for `ripr impacted-evidence` (rooted at the working
/// directory) and the compatibility `cargo xtask impacted-evidence` route
/// (rooted at the xtask workspace), so refusal and routing logic has one owner.
pub fn run_impacted_evidence_at(repo: &Path, args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }
    let options = parse_options(args)?;
    // Taken before the evidence is read, so a refusal removes only outputs
    // that already existed when this run started (#5307).
    let previous = stamp_outputs(repo);
    let input = require_pr_evidence(repo, &options.pr_evidence)
        .map_err(|err| refuse_with_stale_cleanup(repo, err, options.check, &previous))?;
    let packet = packet_from_input(&options, &input);
    let json_text = serde_json::to_string_pretty(&packet)
        .map_err(|err| format!("serialize impacted evidence: {err}"))?;
    let markdown = render_impacted_evidence_markdown(&packet);
    if options.check {
        check_outputs(repo, &json_text, &markdown)
    } else {
        write_outputs(repo, &json_text, &markdown)
    }
}

fn parse_options(args: &[String]) -> Result<ImpactedEvidenceOptions, String> {
    let mut options = ImpactedEvidenceOptions::default();
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--pr-evidence" => {
                i += 1;
                options.pr_evidence = non_empty_arg(args, i, "--pr-evidence")?.to_string();
            }
            "--label" => {
                i += 1;
                options
                    .labels
                    .push(non_empty_arg(args, i, "--label")?.to_string());
            }
            "--labels" => {
                i += 1;
                options
                    .labels
                    .extend(split_labels(non_empty_arg(args, i, "--labels")?));
            }
            "--check" => options.check = true,
            other => return Err(unknown_argument("impacted-evidence", other)),
        }
        i += 1;
    }
    options.labels = normalize_labels(&options.labels);
    Ok(options)
}

fn non_empty_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    let Some(value) = args.get(index) else {
        return Err(format!("missing value for {flag}"));
    };
    if value.trim().is_empty() {
        return Err(format!(
            "impacted-evidence {flag} requires a non-empty value"
        ));
    }
    Ok(value)
}

fn print_help() {
    println!("{IMPACTED_EVIDENCE_HELP}");
}

/// Help body for `ripr impacted-evidence`. Also the flag source for
/// unknown-argument suggestions; keep accepted flags on option-list lines.
pub(crate) const IMPACTED_EVIDENCE_HELP: &str = "\
Route mutation mode from PR evidence and PR labels.

Usage: ripr impacted-evidence [--pr-evidence <path>] [--label <label>] [--labels <csv>] [--check]

Options:
  --pr-evidence <path>  Path to repo-exposure.json (default: target/ripr/pr/repo-exposure.json)
  --label <label>       Add a single PR label (repeatable)
  --labels <csv>        Add comma/newline/semicolon-separated PR labels
  --check               Verify outputs are up to date

Outputs:
  target/xtask/impacted-evidence/latest.json
  target/xtask/impacted-evidence/latest.md
";

#[cfg(test)]
fn impacted_evidence_packet(repo: &Path, options: &ImpactedEvidenceOptions) -> Value {
    packet_from_input(options, &load_pr_evidence(repo, &options.pr_evidence))
}

fn packet_from_input(options: &ImpactedEvidenceOptions, input: &PrEvidenceInput) -> Value {
    let ripr_severe_gap = input
        .value
        .as_ref()
        .and_then(|value| value.pointer("/summary/ripr_severe_gap"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let pr_requires_targeted = input
        .value
        .as_ref()
        .and_then(|value| value.pointer("/summary/requires_targeted_mutation"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let targeted_mutation_route = input
        .value
        .as_ref()
        .and_then(|value| value.pointer("/summary/targeted_mutation_route"))
        .cloned()
        .unwrap_or_else(|| {
            json!({
                "status": "static_limitation",
                "candidates": [],
                "limitations": [{
                    "kind": "route_unavailable",
                    "message": "PR evidence did not provide a targeted mutation route"
                }]
            })
        });
    let decision = routing_decision(&options.labels, ripr_severe_gap || pr_requires_targeted);
    let warnings = input.warning(&options.pr_evidence);

    json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "kind": "impacted_evidence",
        "scope": "diff",
        "status": if warnings.is_empty() { "advisory" } else { "incomplete" },
        "inputs": {
            "pr_evidence": options.pr_evidence,
            "labels": options.labels
        },
        "summary": {
            "mutation_mode": decision.mode,
            "requires_targeted_mutation": decision.requires_targeted_mutation,
            "requires_full_owner_mutation": decision.requires_full_owner_mutation,
            "ripr_severe_gap": ripr_severe_gap,
            "routing_reason": decision.reason,
            "targeted_mutation_route": targeted_mutation_route
        },
        "artifacts": [
            {
                "label": "impacted evidence JSON",
                "path": IMPACTED_JSON,
                "kind": "json",
                "scope": "diff",
                "available": true,
                "required": true
            },
            {
                "label": "impacted evidence Markdown",
                "path": IMPACTED_MD,
                "kind": "markdown",
                "scope": "diff",
                "available": true
            },
            {
                "label": "PR evidence JSON",
                "path": options.pr_evidence,
                "kind": "json",
                "scope": "diff",
                "available": input.value.is_some()
            }
        ],
        "warnings": warnings,
        "advisory_limits": [
            "Impacted evidence routes mutation; it does not execute mutation.",
            "Full-owner mutation requires an explicit mutation/full-owner label."
        ]
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RoutingDecision {
    mode: &'static str,
    requires_targeted_mutation: bool,
    requires_full_owner_mutation: bool,
    reason: Value,
}

fn routing_decision(labels: &[String], ripr_routes_targeted: bool) -> RoutingDecision {
    if has_any_label(labels, &["mutation/full-owner"]) {
        return RoutingDecision {
            mode: "full_owner",
            requires_targeted_mutation: false,
            requires_full_owner_mutation: true,
            reason: json!("mutation/full-owner label"),
        };
    }
    if has_any_label(labels, &["mutation", "mutation/targeted"]) {
        return targeted("mutation label");
    }
    if has_any_label(labels, &["release-risk"]) {
        return targeted("release-risk label");
    }
    if ripr_routes_targeted {
        return targeted("ripr severe gap");
    }
    RoutingDecision {
        mode: "fast_only",
        requires_targeted_mutation: false,
        requires_full_owner_mutation: false,
        reason: Value::Null,
    }
}

fn has_any_label(labels: &[String], needles: &[&str]) -> bool {
    needles
        .iter()
        .any(|needle| labels.iter().any(|label| label == needle))
}

fn targeted(reason: &'static str) -> RoutingDecision {
    RoutingDecision {
        mode: "targeted",
        requires_targeted_mutation: true,
        requires_full_owner_mutation: false,
        reason: json!(reason),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PrEvidenceInput {
    value: Option<Value>,
    state: InputState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum InputState {
    Present,
    Missing,
    Invalid(String),
}

impl PrEvidenceInput {
    fn warning(&self, path: &str) -> Vec<Value> {
        match &self.state {
            InputState::Present => Vec::new(),
            InputState::Missing => vec![json!({
                "kind": "missing_artifact",
                "message": "PR evidence JSON is missing; mutation routing uses labels only.",
                "path": path
            })],
            InputState::Invalid(err) => vec![json!({
                "kind": "invalid_json",
                "message": format!("PR evidence JSON is invalid: {err}"),
                "path": path
            })],
        }
    }
}

/// Summary fields the producer always writes; absent ones must not default to
/// "no mutation needed".
const ROUTING_FIELDS: [&str; 2] = ["ripr_severe_gap", "requires_targeted_mutation"];

/// Refuses to route mutation from labels alone. A missing or non-JSON PR
/// evidence file would otherwise yield `fast_only`, which reads as "no mutation
/// needed" when the real state is "evidence not seen". Fails before any output
/// is written so a stale `latest.*` cannot be mistaken for this run.
/// Returns the loaded input so the packet is built from the exact bytes that
/// were validated.
fn require_pr_evidence(repo: &Path, relative: &str) -> Result<PrEvidenceInput, String> {
    let input = load_pr_evidence(repo, relative);
    match &input.state {
        InputState::Present => {
            let missing: Vec<&str> = ROUTING_FIELDS
                .into_iter()
                .filter(|field| {
                    !input
                        .value
                        .as_ref()
                        .and_then(|value| value.pointer(&format!("/summary/{field}")))
                        .is_some_and(Value::is_boolean)
                })
                .collect();
            if missing.is_empty() {
                Ok(input)
            } else {
                Err(format!(
                    "impacted-evidence: PR evidence {relative} lacks boolean summary.{}; a packet without routing fields would read as \"no mutation needed\". Regenerate it with `ripr pr-evidence` (`cargo xtask ripr-pr` in the ripr repository).",
                    missing.join(" and summary.")
                ))
            }
        }
        InputState::Missing => Err(format!(
            "impacted-evidence: PR evidence {relative} is missing or unreadable; refusing to route mutation from labels alone. \
             Run `ripr pr-evidence` (`cargo xtask ripr-pr` in the ripr repository) first or pass --pr-evidence <path>."
        )),
        InputState::Invalid(err) => Err(format!(
            "impacted-evidence: PR evidence {relative} is not valid JSON ({err}); \
             regenerate it with `ripr pr-evidence` (`cargo xtask ripr-pr` in the ripr repository) or pass --pr-evidence <path>."
        )),
    }
}

const OUTPUTS: [&str; 2] = [IMPACTED_JSON, IMPACTED_MD];

/// Size and modification time of one output file. An output whose stamp
/// changed was rewritten after the stamp was taken, so it belongs to another
/// run.
#[derive(Clone, Debug, PartialEq, Eq)]
struct OutputStamp {
    len: u64,
    modified: Option<std::time::SystemTime>,
}

/// What a metadata read saw for one output. Only `NotFound` means absent; any
/// other error is kept so cleanup fails loudly instead of silently skipping
/// (or misreporting) an output it could not inspect.
#[derive(Clone, Debug, PartialEq, Eq)]
enum OutputState {
    Absent,
    Present(OutputStamp),
    Unreadable(String),
}

fn stamp_output(repo: &Path, relative: &str) -> OutputState {
    match fs::symlink_metadata(repo.join(relative)) {
        Ok(metadata) => OutputState::Present(OutputStamp {
            len: metadata.len(),
            modified: metadata.modified().ok(),
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => OutputState::Absent,
        Err(err) => OutputState::Unreadable(err.to_string()),
    }
}

fn stamp_outputs(repo: &Path) -> [OutputState; 2] {
    OUTPUTS.map(|relative| stamp_output(repo, relative))
}

/// What a refusal did to each output.
#[derive(Default)]
struct StaleCleanup {
    removed: Vec<&'static str>,
    failed: Vec<String>,
    /// Written by a concurrent run after this run started, or the unchanged
    /// partner of such an output; left in place.
    left: Vec<&'static str>,
}

/// Removes a previous run's outputs so a failed run cannot leave a stale
/// `latest.*` that a later reader mistakes for this run's routing. An output
/// that appeared or changed since `previous` was taken was written by a
/// concurrent run sharing the target directory, so it and its partner are
/// left in place (#5307). The stamp check and the removal are not atomic: a write landing
/// between them can still be removed, which narrows the race to that window.
/// The stamp is size plus modification time, so on a filesystem with coarse
/// timestamps (FAT, some network mounts) a same-size rewrite within one tick,
/// or any same-size rewrite where the platform reports no mtime, also reads
/// as unchanged and is removed.
fn discard_stale_outputs(repo: &Path, previous: &[OutputState; 2]) -> StaleCleanup {
    let mut cleanup = StaleCleanup::default();
    let current = stamp_outputs(repo);
    // The JSON and Markdown are one receipt. A concurrent run can publish both
    // between this run's two start reads, so the start pair may mix an old
    // stamp with a new one; when either output is newer, the pair belongs to
    // another run and neither is removed.
    let newer_generation = current.iter().zip(previous).any(|pair| match pair {
        (OutputState::Present(_), OutputState::Absent) => true,
        (OutputState::Present(now), OutputState::Present(before)) => now != before,
        _ => false,
    });
    for ((relative, now), before) in OUTPUTS.into_iter().zip(current).zip(previous) {
        match (now, before) {
            (OutputState::Absent, _) => {}
            (OutputState::Unreadable(err), _) => cleanup
                .failed
                .push(format!("{relative}: could not read its metadata: {err}")),
            // Without a start stamp the run cannot tell its own stale output
            // from a concurrent run's, so it neither deletes nor claims the
            // output is newer.
            (OutputState::Present(_), OutputState::Unreadable(err)) => cleanup.failed.push(
                format!("{relative}: could not read its metadata when this run started ({err})"),
            ),
            (OutputState::Present(_), _) if newer_generation => cleanup.left.push(relative),
            (OutputState::Present(_), _) => match fs::remove_file(repo.join(relative)) {
                Ok(()) => cleanup.removed.push(relative),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => cleanup.failed.push(format!("{relative}: {err}")),
            },
        }
    }
    cleanup
}

fn refuse_with_stale_cleanup(
    repo: &Path,
    err: String,
    check: bool,
    previous: &[OutputState; 2],
) -> String {
    if check {
        return err;
    }
    let StaleCleanup {
        removed,
        failed,
        left,
    } = discard_stale_outputs(repo, previous);
    let mut message = err;
    if !removed.is_empty() {
        message.push_str(&format!(" Removed stale {}.", removed.join(" and ")));
    }
    if !left.is_empty() {
        // The pair rule can leave an unchanged partner beside the newer
        // output, so the reason names the concurrent write, not each file.
        let (pronoun, verb) = if left.len() == 1 {
            ("it", "does")
        } else {
            ("they", "do")
        };
        message.push_str(&format!(
            " Left {} in place: another run wrote output after this run started, so {pronoun} {verb} not describe this refused run.",
            left.join(" and "),
        ));
    }
    if !failed.is_empty() {
        message.push_str(&format!(
            " Could not remove stale output, so it may be out of date: {}.",
            failed.join("; ")
        ));
    }
    message
}

fn load_pr_evidence(repo: &Path, relative: &str) -> PrEvidenceInput {
    let path = repo.join(relative);
    let Ok(text) = fs::read_to_string(&path) else {
        return PrEvidenceInput {
            value: None,
            state: InputState::Missing,
        };
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(value) => PrEvidenceInput {
            value: Some(value),
            state: InputState::Present,
        },
        Err(err) => PrEvidenceInput {
            value: None,
            state: InputState::Invalid(first_line(&err.to_string())),
        },
    }
}

fn render_impacted_evidence_markdown(packet: &Value) -> String {
    let summary = packet.get("summary").and_then(Value::as_object);
    let inputs = packet.get("inputs").and_then(Value::as_object);
    let labels = inputs
        .and_then(|inputs| inputs.get("labels"))
        .and_then(Value::as_array)
        .map(|labels| {
            labels
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|labels| !labels.is_empty())
        .unwrap_or_else(|| "none".to_string());

    let mut out = String::new();
    out.push_str("# Impacted Evidence\n\n");
    out.push_str("## Routing\n\n");
    out.push_str(&format!(
        "- status: {}\n",
        code_span(
            packet
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        )
    ));
    out.push_str(&format!(
        "- mutation_mode: {}\n",
        code_span(&summary_string(summary, "mutation_mode", "unknown"))
    ));
    out.push_str(&format!(
        "- requires_targeted_mutation: {}\n",
        summary_bool(summary, "requires_targeted_mutation")
    ));
    out.push_str(&format!(
        "- requires_full_owner_mutation: {}\n",
        summary_bool(summary, "requires_full_owner_mutation")
    ));
    out.push_str(&format!(
        "- ripr_severe_gap: {}\n",
        summary_bool(summary, "ripr_severe_gap")
    ));
    out.push_str(&format!(
        "- routing_reason: {}\n\n",
        code_span(&summary_string_or_null(summary, "routing_reason"))
    ));
    if let Some(route) = summary
        .and_then(|summary| summary.get("targeted_mutation_route"))
        .and_then(Value::as_object)
    {
        out.push_str(&format!(
            "- targeted_mutation_route: {}\n",
            code_span(
                route
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            )
        ));
        if let Some(candidate) = route
            .get("candidates")
            .and_then(Value::as_array)
            .and_then(|candidates| candidates.first())
            .and_then(Value::as_object)
        {
            out.push_str(&format!(
                "- candidate: {}:{} {} -> {}\n- command: {}\n",
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
                )
            ));
        }
        if let Some(limitation) = route
            .get("limitations")
            .and_then(Value::as_array)
            .and_then(|limitations| limitations.first())
            .and_then(Value::as_object)
        {
            out.push_str(&format!(
                "- limitation: {}\n",
                code_span(
                    limitation
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                )
            ));
        }
        out.push('\n');
    }

    out.push_str("## Inputs\n\n");
    out.push_str(&format!(
        "- PR evidence: {}\n",
        code_span(
            inputs
                .and_then(|inputs| inputs.get("pr_evidence"))
                .and_then(Value::as_str)
                .unwrap_or("not_available")
        )
    ));
    out.push_str(&format!("- labels: {}\n\n", code_span(&labels)));

    out.push_str("## Artifacts\n\n");
    out.push_str("| Artifact | Path | Available |\n");
    out.push_str("| --- | --- | --- |\n");
    if let Some(artifacts) = packet.get("artifacts").and_then(Value::as_array) {
        for artifact in artifacts {
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                table_cell_text(string_field(artifact, "label", "artifact")),
                table_code_span(string_field(artifact, "path", "unknown")),
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
                inline_prose(string_field(warning, "message", "unknown warning"))
            ));
        }
    }

    out.push_str("\n_This receipt routes verification work. It does not execute mutation._\n");
    out
}

fn summary_string(
    summary: Option<&serde_json::Map<String, Value>>,
    key: &str,
    fallback: &str,
) -> String {
    summary
        .and_then(|summary| summary.get(key))
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_string()
}

fn summary_bool(summary: Option<&serde_json::Map<String, Value>>, key: &str) -> String {
    summary
        .and_then(|summary| summary.get(key))
        .and_then(Value::as_bool)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "not_available".to_string())
}

fn summary_string_or_null(summary: Option<&serde_json::Map<String, Value>>, key: &str) -> String {
    let Some(value) = summary.and_then(|summary| summary.get(key)) else {
        return "not_available".to_string();
    };
    if value.is_null() {
        "none".to_string()
    } else {
        value.as_str().unwrap_or("invalid").to_string()
    }
}

fn string_field<'a>(value: &'a Value, key: &str, fallback: &'a str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or(fallback)
}

fn check_outputs(repo: &Path, json_text: &str, markdown: &str) -> Result<(), String> {
    let json_path = repo.join(IMPACTED_JSON);
    let md_path = repo.join(IMPACTED_MD);
    let actual_json = fs::read_to_string(&json_path)
        .map_err(|err| format!("missing or unreadable {IMPACTED_JSON}: {err}"))?;
    let actual_md = fs::read_to_string(&md_path)
        .map_err(|err| format!("missing or unreadable {IMPACTED_MD}: {err}"))?;
    if actual_json == format!("{json_text}\n") && actual_md == markdown {
        println!("Impacted evidence contract ok: {IMPACTED_JSON}");
        Ok(())
    } else {
        Err("impacted evidence is stale; run `ripr impacted-evidence`".to_string())
    }
}

fn write_outputs(repo: &Path, json_text: &str, markdown: &str) -> Result<(), String> {
    let json_path = repo.join(IMPACTED_JSON);
    let md_path = repo.join(IMPACTED_MD);
    if let Some(parent) = json_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create impacted evidence dir: {err}"))?;
    }
    crate::output::file_write::write(&json_path, format!("{json_text}\n").as_bytes())
        .map_err(|err| format!("failed to write {IMPACTED_JSON}: {err}"))?;
    crate::output::file_write::write(&md_path, markdown.as_bytes())
        .map_err(|err| format!("failed to write {IMPACTED_MD}: {err}"))?;
    println!("Wrote {IMPACTED_JSON}");
    println!("Wrote {IMPACTED_MD}");
    Ok(())
}

fn labels_from_env() -> Vec<String> {
    env::var("GITHUB_PR_LABELS")
        .or_else(|_| env::var("PR_LABELS"))
        .map(|labels| normalize_labels(&split_labels(&labels)))
        .unwrap_or_default()
}

fn split_labels(labels: &str) -> Vec<String> {
    labels
        .split([',', '\n', ';'])
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .map(str::to_string)
        .collect()
}

fn normalize_labels(labels: &[String]) -> Vec<String> {
    labels
        .iter()
        .map(|label| label.trim().to_ascii_lowercase())
        .filter(|label| !label.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn first_line(value: &str) -> String {
    value.lines().next().unwrap_or(value).trim().to_string()
}

fn repo_root() -> Result<PathBuf, String> {
    std::env::current_dir().map_err(|err| format!("failed to determine working directory: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rejects_blank_label_values() -> Result<(), String> {
        let err = match parse_options(&["--label".to_string(), "   ".to_string()]) {
            Ok(_) => return Err("blank --label should fail".to_string()),
            Err(err) => err,
        };
        assert_eq!(
            err,
            "impacted-evidence --label requires a non-empty value".to_string()
        );
        Ok(())
    }

    #[test]
    fn parse_accepts_labels_and_check() -> Result<(), String> {
        let parsed = parse_options(&[
            "--label".to_string(),
            "release-risk".to_string(),
            "--labels".to_string(),
            "mutation, docs".to_string(),
            "--check".to_string(),
        ])?;
        assert_eq!(
            parsed.labels,
            vec![
                "docs".to_string(),
                "mutation".to_string(),
                "release-risk".to_string()
            ]
        );
        assert!(parsed.check);
        Ok(())
    }

    #[test]
    fn impacted_evidence_preserves_targeted_mutation_route() -> Result<(), String> {
        let repo = env::temp_dir().join(format!(
            "ripr-impacted-evidence-route-{}",
            std::process::id()
        ));
        let input = repo.join("target/ripr/pr/repo-exposure.json");
        if repo.exists() {
            fs::remove_dir_all(&repo).map_err(|err| format!("remove {}: {err}", repo.display()))?;
        }
        fs::create_dir_all(
            input
                .parent()
                .ok_or_else(|| "missing input parent".to_string())?,
        )
        .map_err(|err| format!("create {}: {err}", repo.display()))?;
        let value = json!({
            "summary": {
                "ripr_severe_gap": true,
                "requires_targeted_mutation": true,
                "targeted_mutation_route": {
                    "status": "candidate",
                    "candidates": [{
                        "file": "src/lib.rs",
                        "line": 8,
                        "kind": "predicate_operator_flip",
                        "from": ">=",
                        "to": ">",
                        "command": "cargo mutants --file \"src/lib.rs\"",
                        "expected_observation": "boundary test observes operator flip"
                    }],
                    "limitations": [{
                        "kind": "no_safe_candidate",
                        "message": "helper dispatch remains static limitation"
                    }]
                }
            }
        });
        fs::write(
            &input,
            serde_json::to_vec(&value).map_err(|err| err.to_string())?,
        )
        .map_err(|err| format!("write {}: {err}", input.display()))?;
        let packet = impacted_evidence_packet(
            &repo,
            &ImpactedEvidenceOptions {
                pr_evidence: "target/ripr/pr/repo-exposure.json".to_string(),
                labels: Vec::new(),
                check: false,
            },
        );
        assert_eq!(
            packet["summary"]["targeted_mutation_route"]["candidates"][0]["command"],
            "cargo mutants --file \"src/lib.rs\""
        );
        let markdown = render_impacted_evidence_markdown(&packet);
        assert!(markdown.contains("cargo mutants --file"));
        assert!(markdown.contains("helper dispatch remains static limitation"));
        fs::write(
            &input,
            br#"{"summary":{"ripr_severe_gap":true,"requires_targeted_mutation":true}}"#,
        )
        .map_err(|err| format!("rewrite {}: {err}", input.display()))?;
        let fallback = impacted_evidence_packet(
            &repo,
            &ImpactedEvidenceOptions {
                pr_evidence: "target/ripr/pr/repo-exposure.json".to_string(),
                labels: Vec::new(),
                check: false,
            },
        );
        assert_eq!(
            fallback["summary"]["targeted_mutation_route"]["status"],
            "static_limitation"
        );
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn missing_or_invalid_pr_evidence_is_refused_with_an_actionable_message() -> Result<(), String>
    {
        let repo = env::temp_dir().join(format!(
            "ripr-impacted-evidence-refuse-{}",
            std::process::id()
        ));
        if repo.exists() {
            fs::remove_dir_all(&repo).map_err(|err| format!("remove {}: {err}", repo.display()))?;
        }
        fs::create_dir_all(&repo).map_err(|err| format!("create {}: {err}", repo.display()))?;

        let missing = require_pr_evidence(&repo, "nope.json")
            .err()
            .ok_or_else(|| "missing evidence must be refused".to_string())?;
        assert!(
            missing.contains("nope.json") && missing.contains("missing"),
            "{missing}"
        );
        assert!(missing.contains("ripr pr-evidence"), "{missing}");

        fs::write(repo.join("bad.json"), "not json")
            .map_err(|err| format!("write bad.json: {err}"))?;
        let invalid = require_pr_evidence(&repo, "bad.json")
            .err()
            .ok_or_else(|| "invalid evidence must be refused".to_string())?;
        assert!(invalid.contains("not valid JSON"), "{invalid}");

        fs::write(repo.join("empty.json"), "{}")
            .map_err(|err| format!("write empty.json: {err}"))?;
        let empty = require_pr_evidence(&repo, "empty.json")
            .err()
            .ok_or_else(|| "evidence without routing fields must be refused".to_string())?;
        assert!(empty.contains("summary.ripr_severe_gap"), "{empty}");
        assert!(
            empty.contains("summary.requires_targeted_mutation"),
            "{empty}"
        );

        fs::write(
            repo.join("ok.json"),
            r#"{"summary":{"ripr_severe_gap":false,"requires_targeted_mutation":false}}"#,
        )
        .map_err(|err| format!("write ok.json: {err}"))?;
        require_pr_evidence(&repo, "ok.json")?;
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn markdown_states_packet_status() {
        let packet = json!({"status": "incomplete", "summary": {}, "inputs": {}});
        assert!(render_impacted_evidence_markdown(&packet).contains("- status: `incomplete`"));
    }

    #[test]
    fn refusal_discards_previous_outputs_but_check_does_not() -> Result<(), String> {
        let repo = env::temp_dir().join(format!(
            "ripr-impacted-evidence-stale-{}",
            std::process::id()
        ));
        if repo.exists() {
            fs::remove_dir_all(&repo).map_err(|err| format!("remove {}: {err}", repo.display()))?;
        }
        fs::create_dir_all(repo.join("target/xtask/impacted-evidence"))
            .map_err(|err| format!("create {}: {err}", repo.display()))?;
        fs::write(repo.join(IMPACTED_JSON), "{}").map_err(|err| err.to_string())?;
        fs::write(repo.join(IMPACTED_MD), "old").map_err(|err| err.to_string())?;

        let previous = stamp_outputs(&repo);
        let kept = refuse_with_stale_cleanup(&repo, "boom.".to_string(), true, &previous);
        assert_eq!(kept, "boom.");
        assert!(repo.join(IMPACTED_JSON).exists(), "--check must not delete");

        let cleaned = refuse_with_stale_cleanup(&repo, "boom.".to_string(), false, &previous);
        assert!(cleaned.contains("Removed stale"), "{cleaned}");
        assert!(!repo.join(IMPACTED_JSON).exists() && !repo.join(IMPACTED_MD).exists());
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    #[test]
    fn failed_stale_removal_is_reported() -> Result<(), String> {
        let repo = env::temp_dir().join(format!(
            "ripr-impacted-evidence-undeletable-{}",
            std::process::id()
        ));
        if repo.exists() {
            fs::remove_dir_all(&repo).map_err(|err| format!("remove {}: {err}", repo.display()))?;
        }
        // A directory where the file belongs makes remove_file fail without NotFound.
        fs::create_dir_all(repo.join(IMPACTED_JSON))
            .map_err(|err| format!("create {}: {err}", repo.display()))?;
        let previous = stamp_outputs(&repo);
        let message = refuse_with_stale_cleanup(&repo, "boom.".to_string(), false, &previous);
        assert!(
            message.contains("Could not remove stale output"),
            "{message}"
        );
        assert!(message.contains(IMPACTED_JSON), "{message}");
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }

    /// #5307: a refusing run must not delete outputs a concurrent run wrote
    /// after the refusing run started, whether they replaced older outputs or
    /// appeared where none existed.
    #[test]
    fn refusal_leaves_outputs_a_concurrent_run_wrote() -> Result<(), String> {
        let repo = env::temp_dir().join(format!(
            "ripr-impacted-evidence-concurrent-{}",
            std::process::id()
        ));
        if repo.exists() {
            fs::remove_dir_all(&repo).map_err(|err| format!("remove {}: {err}", repo.display()))?;
        }
        fs::create_dir_all(repo.join("target/xtask/impacted-evidence"))
            .map_err(|err| format!("create {}: {err}", repo.display()))?;
        fs::write(repo.join(IMPACTED_JSON), "{}").map_err(|err| err.to_string())?;
        // This run starts: the JSON exists, the Markdown does not.
        let previous = stamp_outputs(&repo);
        // A concurrent run with valid evidence then writes both outputs.
        fs::write(repo.join(IMPACTED_JSON), r#"{"status":"concurrent"}"#)
            .map_err(|err| err.to_string())?;
        fs::write(repo.join(IMPACTED_MD), "concurrent").map_err(|err| err.to_string())?;

        let message = refuse_with_stale_cleanup(&repo, "boom.".to_string(), false, &previous);
        let json = fs::read_to_string(repo.join(IMPACTED_JSON));
        let markdown = fs::read_to_string(repo.join(IMPACTED_MD));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;

        assert_eq!(
            json.map_err(|err| err.to_string())?,
            r#"{"status":"concurrent"}"#
        );
        assert_eq!(markdown.map_err(|err| err.to_string())?, "concurrent");
        assert!(!message.contains("Removed stale"), "{message}");
        assert!(
            message.contains(&format!("Left {IMPACTED_JSON} and {IMPACTED_MD} in place")),
            "{message}"
        );
        Ok(())
    }

    fn fresh_repo(tag: &str) -> Result<std::path::PathBuf, String> {
        let repo = env::temp_dir().join(format!(
            "ripr-impacted-evidence-{tag}-{}",
            std::process::id()
        ));
        if repo.exists() {
            fs::remove_dir_all(&repo).map_err(|err| format!("remove {}: {err}", repo.display()))?;
        }
        Ok(repo)
    }

    /// The production refusal path stamps before it reads evidence and hands
    /// that stamp to cleanup, so outputs from before the run are removed.
    #[test]
    fn refusing_run_removes_outputs_that_predate_it() -> Result<(), String> {
        let repo = fresh_repo("run-refusal")?;
        fs::create_dir_all(repo.join("target/xtask/impacted-evidence"))
            .map_err(|err| format!("create {}: {err}", repo.display()))?;
        fs::write(repo.join(IMPACTED_JSON), "{}").map_err(|err| err.to_string())?;
        fs::write(repo.join(IMPACTED_MD), "old").map_err(|err| err.to_string())?;
        // No PR evidence exists, so the run refuses.
        let result = run_impacted_evidence_at(&repo, &[]);
        let json_left = repo.join(IMPACTED_JSON).exists();
        let md_left = repo.join(IMPACTED_MD).exists();
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        let message = match result {
            Ok(()) => return Err("a run without PR evidence must refuse".to_string()),
            Err(message) => message,
        };
        assert!(message.contains("is missing or unreadable"), "{message}");
        assert!(
            message.contains(&format!("Removed stale {IMPACTED_JSON} and {IMPACTED_MD}")),
            "{message}"
        );
        assert!(!json_left && !md_left);
        Ok(())
    }

    /// The outputs are one receipt: when a concurrent run's Markdown appears,
    /// the JSON beside it is not removed even though its stamp is unchanged.
    #[test]
    fn a_newer_markdown_keeps_its_unchanged_json_partner() -> Result<(), String> {
        let repo = fresh_repo("newer-partner")?;
        fs::create_dir_all(repo.join("target/xtask/impacted-evidence"))
            .map_err(|err| format!("create {}: {err}", repo.display()))?;
        fs::write(repo.join(IMPACTED_JSON), "{}").map_err(|err| err.to_string())?;
        let previous = stamp_outputs(&repo);
        fs::write(repo.join(IMPACTED_MD), "concurrent").map_err(|err| err.to_string())?;
        let message = refuse_with_stale_cleanup(&repo, "boom.".to_string(), false, &previous);
        let json_left = repo.join(IMPACTED_JSON).exists();
        let md_left = repo.join(IMPACTED_MD).exists();
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        assert!(json_left && md_left, "{message}");
        assert!(!message.contains("Removed stale"), "{message}");
        assert!(
            message.contains(&format!("Left {IMPACTED_JSON} and {IMPACTED_MD} in place")),
            "{message}"
        );
        assert!(
            message.contains(
                "in place: another run wrote output after this run started, so they do not describe this refused run."
            ),
            "{message}"
        );
        Ok(())
    }

    /// Codex interleaving: a concurrent run publishes both outputs between the
    /// two start reads, so the start pair holds the old JSON stamp and the new
    /// Markdown stamp. Neither current output is removed.
    #[test]
    fn mixed_start_pair_from_an_interleaved_publish_keeps_both() -> Result<(), String> {
        let repo = fresh_repo("interleaved")?;
        fs::create_dir_all(repo.join("target/xtask/impacted-evidence"))
            .map_err(|err| format!("create {}: {err}", repo.display()))?;
        fs::write(repo.join(IMPACTED_JSON), "{}").map_err(|err| err.to_string())?;
        let old_json = stamp_output(&repo, IMPACTED_JSON);
        fs::write(repo.join(IMPACTED_JSON), r#"{"status":"concurrent"}"#)
            .map_err(|err| err.to_string())?;
        fs::write(repo.join(IMPACTED_MD), "concurrent").map_err(|err| err.to_string())?;
        let new_md = stamp_output(&repo, IMPACTED_MD);
        let message =
            refuse_with_stale_cleanup(&repo, "boom.".to_string(), false, &[old_json, new_md]);
        let json_left = repo.join(IMPACTED_JSON).exists();
        let md_left = repo.join(IMPACTED_MD).exists();
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        assert!(json_left && md_left, "{message}");
        assert!(!message.contains("Removed stale"), "{message}");
        Ok(())
    }

    /// A single output written by a concurrent run, with no partner on disk,
    /// is named with singular wording.
    #[test]
    fn one_concurrent_output_is_left_with_singular_wording() -> Result<(), String> {
        let repo = fresh_repo("concurrent-one")?;
        fs::create_dir_all(repo.join("target/xtask/impacted-evidence"))
            .map_err(|err| format!("create {}: {err}", repo.display()))?;
        let previous = stamp_outputs(&repo);
        fs::write(repo.join(IMPACTED_MD), "concurrent").map_err(|err| err.to_string())?;
        let message = refuse_with_stale_cleanup(&repo, "boom.".to_string(), false, &previous);
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        assert!(
            message.contains(&format!(
                "Left {IMPACTED_MD} in place: another run wrote output after this run started, so it does not describe this refused run."
            )),
            "{message}"
        );
        Ok(())
    }

    /// A metadata error other than `NotFound` is reported as a cleanup
    /// failure, never read as an absent output. Unix only: Windows reports a
    /// file in a path's directory position as `NotFound`.
    #[cfg(unix)]
    #[test]
    fn unreadable_output_metadata_is_reported_not_skipped() -> Result<(), String> {
        let repo = fresh_repo("unreadable-now")?;
        fs::create_dir_all(repo.join("target/xtask"))
            .map_err(|err| format!("create {}: {err}", repo.display()))?;
        // A file where the outputs directory belongs makes every output
        // lookup fail with a not-a-directory error, not `NotFound`.
        fs::write(repo.join("target/xtask/impacted-evidence"), "blocker")
            .map_err(|err| err.to_string())?;
        let previous = stamp_outputs(&repo);
        let message = refuse_with_stale_cleanup(&repo, "boom.".to_string(), false, &previous);
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        assert!(
            previous
                .iter()
                .all(|state| matches!(state, OutputState::Unreadable(_))),
            "{previous:?}"
        );
        assert!(
            message.contains("Could not remove stale output"),
            "{message}"
        );
        assert!(
            message.contains(&format!("{IMPACTED_JSON}: could not read its metadata")),
            "{message}"
        );
        Ok(())
    }

    /// An output whose start stamp could not be read is neither deleted nor
    /// claimed as a concurrent run's.
    #[test]
    fn output_with_unreadable_start_stamp_is_kept_and_reported() -> Result<(), String> {
        let repo = fresh_repo("unreadable-start")?;
        fs::create_dir_all(repo.join("target/xtask/impacted-evidence"))
            .map_err(|err| format!("create {}: {err}", repo.display()))?;
        fs::write(repo.join(IMPACTED_JSON), "{}").map_err(|err| err.to_string())?;
        let previous = [
            OutputState::Unreadable("permission denied".to_string()),
            OutputState::Absent,
        ];
        let message = refuse_with_stale_cleanup(&repo, "boom.".to_string(), false, &previous);
        let json_left = repo.join(IMPACTED_JSON).exists();
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        assert!(
            json_left,
            "an output the run could not stamp must not be deleted"
        );
        assert!(!message.contains("Left "), "{message}");
        assert!(
            message.contains(&format!(
                "{IMPACTED_JSON}: could not read its metadata when this run started (permission denied)"
            )),
            "{message}"
        );
        Ok(())
    }

    #[test]
    fn explicit_root_owns_evidence_and_outputs() -> Result<(), String> {
        let repo = env::temp_dir().join(format!(
            "ripr-impacted-evidence-root-{}",
            std::process::id()
        ));
        if repo.exists() {
            fs::remove_dir_all(&repo).map_err(|err| format!("remove {}: {err}", repo.display()))?;
        }
        fs::create_dir_all(repo.join("target/ripr/pr"))
            .map_err(|err| format!("create {}: {err}", repo.display()))?;
        fs::write(
            repo.join(DEFAULT_PR_EVIDENCE_JSON),
            r#"{"summary":{"ripr_severe_gap":false,"requires_targeted_mutation":false}}"#,
        )
        .map_err(|err| err.to_string())?;
        run_impacted_evidence_at(&repo, &[])?;
        assert!(
            repo.join(IMPACTED_JSON).exists(),
            "outputs land under the given root"
        );
        assert!(repo.join(IMPACTED_MD).exists());
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))?;
        Ok(())
    }
}
