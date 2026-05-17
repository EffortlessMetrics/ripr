use serde_json::{Map, Value, json};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: &str = "0.1";
const DEFAULT_ROOT: &str = ".";
const DEFAULT_OUT_DIR: &str = "target/ripr/reports";
const DEFAULT_BASE: &str = "origin/main";
const DEFAULT_HEAD: &str = "HEAD";
const START_HERE_JSON: &str = "start-here.json";
const START_HERE_MD: &str = "start-here.md";
const DEFAULT_GAP_LEDGER: &str = "target/ripr/reports/gap-decision-ledger.json";
const DEFAULT_FIRST_ACTION: &str = "target/ripr/reports/first-useful-action.json";
const DEFAULT_REVIEW_COMMENTS: &str = "target/ripr/review/comments.json";
const DEFAULT_AGENT_PACKET: &str = "target/ripr/workflow/agent-packet.json";
const DEFAULT_GATE_DECISION: &str = "target/ripr/reports/gate-decision.json";
const DEFAULT_RECEIPTS_DIR: &str = "target/ripr/receipts";

#[derive(Clone, Debug, Eq, PartialEq)]
struct FirstPrOptions {
    root: String,
    base: String,
    head: String,
    gap_ledger: String,
    first_action: String,
    review_comments: String,
    agent_packet: String,
    gate_decision: String,
    receipts_dir: String,
    out_dir: String,
    check: bool,
}

impl Default for FirstPrOptions {
    fn default() -> Self {
        Self {
            root: DEFAULT_ROOT.to_string(),
            base: DEFAULT_BASE.to_string(),
            head: DEFAULT_HEAD.to_string(),
            gap_ledger: DEFAULT_GAP_LEDGER.to_string(),
            first_action: DEFAULT_FIRST_ACTION.to_string(),
            review_comments: DEFAULT_REVIEW_COMMENTS.to_string(),
            agent_packet: DEFAULT_AGENT_PACKET.to_string(),
            gate_decision: DEFAULT_GATE_DECISION.to_string(),
            receipts_dir: DEFAULT_RECEIPTS_DIR.to_string(),
            out_dir: DEFAULT_OUT_DIR.to_string(),
            check: false,
        }
    }
}

pub(crate) fn first_pr(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }
    let options = parse_options(args)?;
    let repo = repo_root()?;
    if options.check {
        check_first_pr(&repo, &options)
    } else {
        write_first_pr(&repo, &options)
    }
}

fn parse_options(args: &[String]) -> Result<FirstPrOptions, String> {
    let mut options = FirstPrOptions::default();
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
            "--gap-ledger" => {
                i += 1;
                options.gap_ledger = non_empty_arg(args, i, "--gap-ledger")?.to_string();
            }
            "--first-action" => {
                i += 1;
                options.first_action = non_empty_arg(args, i, "--first-action")?.to_string();
            }
            "--review-comments" => {
                i += 1;
                options.review_comments = non_empty_arg(args, i, "--review-comments")?.to_string();
            }
            "--agent-packet" => {
                i += 1;
                options.agent_packet = non_empty_arg(args, i, "--agent-packet")?.to_string();
            }
            "--gate-decision" => {
                i += 1;
                options.gate_decision = non_empty_arg(args, i, "--gate-decision")?.to_string();
            }
            "--receipts-dir" => {
                i += 1;
                options.receipts_dir = non_empty_arg(args, i, "--receipts-dir")?.to_string();
            }
            "--out-dir" => {
                i += 1;
                options.out_dir = non_empty_arg(args, i, "--out-dir")?.to_string();
            }
            "--check" => options.check = true,
            other => return Err(format!("unknown first-pr argument {other:?}")),
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
        return Err(format!("first-pr {flag} requires a non-empty value"));
    }
    Ok(value)
}

fn print_help() {
    println!(
        "Create the start-here packet for one PR from existing RIPR artifacts.\n\nusage: ripr first-pr [--root <path>] [--base <rev>] [--head <rev>] [--gap-ledger <path>] [--first-action <path>] [--review-comments <path>] [--agent-packet <path>] [--gate-decision <path>] [--receipts-dir <path>] [--out-dir <path>] [--check]"
    );
}

fn write_first_pr(repo: &Path, options: &FirstPrOptions) -> Result<(), String> {
    let root = resolve_path(repo, &options.root);
    let packet = render_start_here_packet(&root, options);
    let out_dir = resolve_path(&root, &options.out_dir);
    fs::create_dir_all(&out_dir)
        .map_err(|err| format!("failed to create {}: {err}", out_dir.display()))?;
    let json_path = out_dir.join(START_HERE_JSON);
    let markdown_path = out_dir.join(START_HERE_MD);
    let json_text = serde_json::to_string_pretty(&packet)
        .map_err(|err| format!("failed to serialize first-pr packet: {err}"))?;
    fs::write(&json_path, format!("{json_text}\n"))
        .map_err(|err| format!("failed to write {}: {err}", json_path.display()))?;
    fs::write(&markdown_path, render_start_here_markdown(&packet))
        .map_err(|err| format!("failed to write {}: {err}", markdown_path.display()))?;
    validate_start_here_packet(&json_path, &markdown_path)?;
    println!("Wrote {}", json_path.display());
    println!("Wrote {}", markdown_path.display());
    Ok(())
}

fn check_first_pr(repo: &Path, options: &FirstPrOptions) -> Result<(), String> {
    let root = resolve_path(repo, &options.root);
    let out_dir = resolve_path(&root, &options.out_dir);
    let json_path = out_dir.join(START_HERE_JSON);
    let markdown_path = out_dir.join(START_HERE_MD);
    validate_start_here_packet(&json_path, &markdown_path)?;
    println!("First PR start-here packet ok: {}", json_path.display());
    Ok(())
}

fn render_start_here_packet(root: &Path, options: &FirstPrOptions) -> Value {
    let gap_path = resolve_path(root, &options.gap_ledger);
    let mut warnings = Vec::new();
    let selection = match read_json(&gap_path) {
        Ok(gap_ledger) => select_from_gap_ledger(&gap_ledger, root, options),
        Err(ArtifactReadError::Missing) => Selection::missing_artifact(
            "gap_ledger",
            "Gap decision ledger",
            &options.gap_ledger,
            regenerate_gap_ledger_command(&options.gap_ledger),
        ),
        Err(ArtifactReadError::Malformed(message)) => Selection::blocked(
            "malformed_artifact",
            format!("The gap decision ledger could not be parsed: {message}"),
            Some(format!(
                "Regenerate the gap ledger with `{}` before assigning repair work.",
                regenerate_gap_ledger_command(&options.gap_ledger)
            )),
        ),
    };
    if let Some(warning) = selection.warning() {
        warnings.push(warning);
    }

    let artifacts = vec![
        artifact_status(
            root,
            "gap_ledger",
            "Gap decision ledger",
            &options.gap_ledger,
            Some(regenerate_gap_ledger_command(&options.gap_ledger)),
        ),
        artifact_status(
            root,
            "first_action",
            "First useful action",
            &options.first_action,
            Some(format!(
                "ripr first-action --root {} --gap-ledger {} --out {} --out-md {}",
                options.root,
                options.gap_ledger,
                options.first_action,
                with_extension(&options.first_action, "md")
            )),
        ),
        artifact_status(
            root,
            "review_comments",
            "PR repair cards",
            &options.review_comments,
            Some(format!(
                "ripr review-comments --root {} --base {} --head {} --gap-ledger {} --out {}",
                options.root,
                options.base,
                options.head,
                options.gap_ledger,
                options.review_comments
            )),
        ),
        artifact_status(
            root,
            "agent_packet",
            "Agent repair packet",
            &options.agent_packet,
            selection.agent_packet_command().or_else(|| {
                Some(format!(
                    "ripr agent packet --root {} --gap-ledger {} --gap-id <gap-id> --json > {}",
                    options.root, options.gap_ledger, options.agent_packet
                ))
            }),
        ),
        artifact_status(
            root,
            "gate_decision",
            "Gate decision",
            &options.gate_decision,
            Some(format!(
                "ripr gate evaluate --gap-ledger {} --out {} --out-md {}",
                options.gap_ledger,
                options.gate_decision,
                with_extension(&options.gate_decision, "md")
            )),
        ),
    ];

    json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "kind": "first_pr_start_here",
        "status": selection.status(),
        "posture": "advisory",
        "root": options.root,
        "inputs": {
            "gap_ledger": options.gap_ledger,
            "base": options.base,
            "head": options.head,
            "first_action": options.first_action,
            "review_comments": options.review_comments,
            "agent_packet": options.agent_packet,
            "gate_decision": options.gate_decision,
            "receipts_dir": options.receipts_dir
        },
        "selected": selection.to_json(),
        "commands": selection.commands_json(options),
        "artifacts": artifacts,
        "authority": {
            "status": "advisory",
            "gate_decision": options.gate_decision,
            "boundary": "Pass/fail authority remains with explicit gate-decision artifacts when configured; this first-run packet does not gate."
        },
        "warnings": warnings,
        "limits": [
            "Composes explicit RIPR artifacts only.",
            "Does not run hidden analysis.",
            "Does not edit source or generate tests.",
            "Does not run mutation testing.",
            "Does not change CI blocking or gate policy."
        ]
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Selection {
    TopGap(Box<TopGapSelection>),
    MissingArtifact {
        id: String,
        label: String,
        path: String,
        regeneration_command: String,
    },
    Blocked {
        state: String,
        message: String,
        next_command: Option<String>,
    },
    NoAction {
        state: String,
        reason: String,
        records_total: usize,
    },
}

impl Selection {
    fn missing_artifact(id: &str, label: &str, path: &str, regeneration_command: String) -> Self {
        Self::MissingArtifact {
            id: id.to_string(),
            label: label.to_string(),
            path: path.to_string(),
            regeneration_command,
        }
    }

    fn blocked(state: &str, message: String, next_command: Option<String>) -> Self {
        Self::Blocked {
            state: state.to_string(),
            message,
            next_command,
        }
    }

    fn no_action(state: &str, reason: String, records_total: usize) -> Self {
        Self::NoAction {
            state: state.to_string(),
            reason,
            records_total,
        }
    }

    fn status(&self) -> &'static str {
        match self {
            Self::TopGap(_) => "actionable",
            Self::MissingArtifact { .. } | Self::Blocked { .. } => "blocked",
            Self::NoAction { .. } => "no_action",
        }
    }

    fn warning(&self) -> Option<String> {
        match self {
            Self::MissingArtifact { label, path, .. } => {
                Some(format!("{label} is missing: {path}"))
            }
            Self::Blocked { message, .. } => Some(message.clone()),
            Self::TopGap(_) | Self::NoAction { .. } => None,
        }
    }

    fn agent_packet_command(&self) -> Option<String> {
        let Self::TopGap(top_gap) = self else {
            return None;
        };
        Some(top_gap.agent_packet_command.clone())
    }

    fn commands_json(&self, options: &FirstPrOptions) -> Value {
        let mut commands = Map::new();
        commands.insert(
            "regenerate_gap_ledger".to_string(),
            Value::String(regenerate_gap_ledger_command(&options.gap_ledger)),
        );
        match self {
            Self::TopGap(top_gap) => {
                commands.insert(
                    "agent_packet".to_string(),
                    Value::String(top_gap.agent_packet_command.clone()),
                );
                commands.insert(
                    "verify".to_string(),
                    Value::String(top_gap.verify_command.clone()),
                );
                if let Some(receipt_command) = &top_gap.receipt_command {
                    commands.insert(
                        "receipt".to_string(),
                        Value::String(receipt_command.clone()),
                    );
                }
            }
            Self::MissingArtifact {
                regeneration_command,
                ..
            } => {
                commands.insert(
                    "next".to_string(),
                    Value::String(regeneration_command.clone()),
                );
            }
            Self::Blocked { next_command, .. } => {
                if let Some(command) = next_command {
                    commands.insert("next".to_string(), Value::String(command.clone()));
                }
            }
            Self::NoAction { .. } => {}
        }
        Value::Object(commands)
    }

    fn to_json(&self) -> Value {
        match self {
            Self::TopGap(top_gap) => top_gap.to_json(),
            Self::MissingArtifact {
                id,
                label,
                path,
                regeneration_command,
            } => json!({
                "state": "missing_artifact",
                "artifact": {
                    "id": id,
                    "label": label,
                    "path": path
                },
                "next_action": "regenerate_missing_artifact",
                "regeneration_command": regeneration_command
            }),
            Self::Blocked {
                state,
                message,
                next_command,
            } => json!({
                "state": state,
                "message": message,
                "next_command": next_command
            }),
            Self::NoAction {
                state,
                reason,
                records_total,
            } => json!({
                "state": state,
                "reason": reason,
                "records_total": records_total
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TopGapSelection {
    gap_id: String,
    canonical_gap_id: Option<String>,
    kind: String,
    source_artifact: String,
    changed_behavior: Option<String>,
    why: String,
    repair_route: String,
    target_file: Option<String>,
    related_test: Option<String>,
    suggested_assertion: Option<String>,
    anchor_file: Option<String>,
    anchor_line: Option<u64>,
    anchor_owner: Option<String>,
    dedupe_fingerprint: Option<String>,
    verify_command: String,
    receipt_command: Option<String>,
    agent_packet_command: String,
}

impl TopGapSelection {
    fn to_json(&self) -> Value {
        json!({
            "state": "top_gap",
            "gap_id": self.gap_id,
            "canonical_gap_id": self.canonical_gap_id,
            "kind": self.kind,
            "source_artifact": self.source_artifact,
            "changed_behavior": self.changed_behavior,
            "why": self.why,
            "repair": {
                "route": self.repair_route,
                "target_file": self.target_file,
                "related_test": self.related_test,
                "suggested_assertion": self.suggested_assertion
            },
            "anchor": {
                "file": self.anchor_file,
                "line": self.anchor_line,
                "owner": self.anchor_owner,
                "dedupe_fingerprint": self.dedupe_fingerprint
            },
            "verify_command": self.verify_command,
            "receipt_command": self.receipt_command,
            "agent_packet_command": self.agent_packet_command
        })
    }
}

fn select_from_gap_ledger(gap_ledger: &Value, root: &Path, options: &FirstPrOptions) -> Selection {
    let records = gap_records(gap_ledger);
    if ledger_reports_timeout(gap_ledger) {
        return Selection::blocked(
            "timeout",
            "The gap decision ledger reports a timeout; refresh the first-run evidence before assigning repair work.".to_string(),
            Some(regenerate_gap_ledger_command(&options.gap_ledger)),
        );
    }
    if ledger_reports_stale(gap_ledger) {
        return Selection::blocked(
            "stale_artifact",
            "The gap decision ledger is stale; refresh the first-run evidence before assigning repair work.".to_string(),
            Some(regenerate_gap_ledger_command(&options.gap_ledger)),
        );
    }
    if let Some(observed_root) = string_path(gap_ledger, &["root"])
        && root_mismatch(root, &options.root, &observed_root)
    {
        return Selection::blocked(
            "wrong_root",
            format!(
                "The gap decision ledger was generated for root `{observed_root}`, but first-pr is running for `{}`.",
                options.root
            ),
            Some(regenerate_gap_ledger_command(&options.gap_ledger)),
        );
    }
    if ledger_reports_blocked(gap_ledger) {
        let message = first_string_array_item(gap_ledger, &["warnings"]).map_or_else(
            || {
                "The gap decision ledger is blocked; refresh the first-run evidence before assigning repair work.".to_string()
            },
            |warning| {
                format!(
                    "The gap decision ledger is blocked: {warning}. Refresh the first-run evidence before assigning repair work."
                )
            },
        );
        return Selection::blocked(
            "blocked_artifact",
            message,
            Some(regenerate_gap_ledger_command(&options.gap_ledger)),
        );
    }
    if ledger_reports_empty_diff(gap_ledger) {
        return Selection::no_action(
            "empty_diff",
            "The PR diff is empty, so no repairable Rust gap was selected.".to_string(),
            records.len(),
        );
    }
    if let Some(record) = records.iter().copied().find(is_first_run_repairable_gap) {
        return Selection::TopGap(Box::new(top_gap_from_record(record, options)));
    }
    Selection::no_action(
        "no_action",
        "No repairable PR-local stable Rust gap was selected from the gap decision ledger."
            .to_string(),
        records.len(),
    )
}

fn ledger_reports_timeout(value: &Value) -> bool {
    matches!(
        string_path(value, &["status"])
            .or_else(|| string_path(value, &["state"]))
            .as_deref(),
        Some("timeout" | "timed_out")
    ) || matches!(bool_path(value, &["timeout"]), Some(true))
}

fn ledger_reports_stale(value: &Value) -> bool {
    matches!(
        string_path(value, &["status"])
            .or_else(|| string_path(value, &["state"]))
            .as_deref(),
        Some("stale" | "analysis_stale")
    ) || matches!(bool_path(value, &["stale"]), Some(true))
}

fn ledger_reports_empty_diff(value: &Value) -> bool {
    matches!(
        string_path(value, &["status"])
            .or_else(|| string_path(value, &["state"]))
            .or_else(|| string_path(value, &["reason"]))
            .as_deref(),
        Some("empty_diff")
    )
}

fn ledger_reports_blocked(value: &Value) -> bool {
    matches!(
        string_path(value, &["status"])
            .or_else(|| string_path(value, &["state"]))
            .as_deref(),
        Some("blocked")
    )
}

fn root_mismatch(expected_root: &Path, expected_arg: &str, observed_root: &str) -> bool {
    let observed = observed_root.trim();
    if observed.is_empty() || observed == "." || observed == expected_arg {
        return false;
    }
    let observed_path = Path::new(observed);
    if observed_path.is_absolute() {
        return normalized_path(observed_path) != normalized_path(expected_root);
    }
    true
}

fn is_first_run_repairable_gap(record: &&Value) -> bool {
    string_path(record, &["language"]).is_some_and(|value| value == "rust")
        && string_path(record, &["language_status"]).is_some_and(|value| value == "stable")
        && string_path(record, &["scope"]).is_some_and(|value| value == "pr_local")
        && string_path(record, &["gap_state"]).is_some_and(|value| value == "actionable")
        && string_path(record, &["repairability"]).is_some_and(|value| value == "repairable")
        && string_path(record, &["policy_state"])
            .is_some_and(|value| value == "new" || value == "reintroduced")
        && record.get("repair_route").is_some()
        && first_string_array_item(record, &["verification_commands"]).is_some()
}

fn top_gap_from_record(record: &Value, options: &FirstPrOptions) -> TopGapSelection {
    let repair_route = record.get("repair_route");
    let anchor = record.get("anchor");
    let gap_id = string_path(record, &["gap_id"]).unwrap_or_else(|| "unknown-gap".to_string());
    let kind = string_path(record, &["kind"]).unwrap_or_else(|| "Unknown".to_string());
    let changed_behavior = string_from_sources(&[
        (repair_route, &["changed_behavior"]),
        (Some(record), &["changed_behavior"]),
    ]);
    let verify_command = first_string_array_item(record, &["verification_commands"])
        .unwrap_or_else(|| regenerate_gap_ledger_command(&options.gap_ledger));
    let receipt_command = string_path(record, &["receipt_command"]);
    TopGapSelection {
        gap_id: gap_id.clone(),
        canonical_gap_id: string_path(record, &["canonical_gap_id"]),
        kind: kind.clone(),
        source_artifact: options.gap_ledger.clone(),
        changed_behavior,
        why: why_for_gap(&kind),
        repair_route: string_from_sources(&[(repair_route, &["route_kind"])])
            .unwrap_or_else(|| "RepairRouteUnavailable".to_string()),
        target_file: string_from_sources(&[(repair_route, &["target_file"])]),
        related_test: string_from_sources(&[(repair_route, &["related_test"])]),
        suggested_assertion: string_from_sources(&[(repair_route, &["assertion_shape"])]),
        anchor_file: string_from_sources(&[(anchor, &["file"])]),
        anchor_line: u64_from_sources(&[(anchor, &["line"])]),
        anchor_owner: string_from_sources(&[(anchor, &["owner"])]),
        dedupe_fingerprint: string_from_sources(&[(anchor, &["dedupe_fingerprint"])]),
        verify_command,
        receipt_command,
        agent_packet_command: format!(
            "ripr agent packet --root {} --gap-ledger {} --gap-id {} --json > {}",
            options.root, options.gap_ledger, gap_id, options.agent_packet
        ),
    }
}

fn why_for_gap(kind: &str) -> String {
    match kind {
        "MissingBoundaryAssertion" => {
            "A related Rust test reaches this change, but no equality-boundary assertion was found for the changed behavior.".to_string()
        }
        "MissingOutputContract" => {
            "User-facing output changed, but the gap ledger did not find checked output or golden evidence for the changed text.".to_string()
        }
        "MissingValueAssertion" => {
            "A related Rust test reaches this change, but no exact value assertion was found for the changed behavior.".to_string()
        }
        "MissingErrorDiscriminator" => {
            "A related Rust test reaches this error path, but no error discriminator was found for the changed behavior.".to_string()
        }
        _ => "The gap ledger marked this PR-local stable Rust gap as repairable and policy-targeted.".to_string(),
    }
}

fn artifact_status(
    root: &Path,
    id: &str,
    label: &str,
    path: &str,
    regeneration_command: Option<String>,
) -> Value {
    let resolved = resolve_path(root, path);
    let status = if resolved.exists() {
        "present"
    } else {
        "missing"
    };
    json!({
        "id": id,
        "label": label,
        "path": path,
        "status": status,
        "regeneration_command": regeneration_command
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ArtifactReadError {
    Missing,
    Malformed(String),
}

fn read_json(path: &Path) -> Result<Value, ArtifactReadError> {
    let text = fs::read_to_string(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ArtifactReadError::Missing
        } else {
            ArtifactReadError::Malformed(format!("read failed: {error}"))
        }
    })?;
    serde_json::from_str(&text).map_err(|err| ArtifactReadError::Malformed(err.to_string()))
}

fn gap_records(value: &Value) -> Vec<&Value> {
    if let Some(records) = value.as_array() {
        return records.iter().collect();
    }
    if let Some(records) = value.get("records").and_then(Value::as_array) {
        return records.iter().collect();
    }
    if let Some(records) = value.get("gap_records").and_then(Value::as_array) {
        return records.iter().collect();
    }
    value
        .get("cases")
        .and_then(Value::as_array)
        .map(|cases| {
            cases
                .iter()
                .filter_map(|case| case.get("expected_gap_record"))
                .collect()
        })
        .unwrap_or_default()
}

fn path_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn string_path(value: &Value, path: &[&str]) -> Option<String> {
    path_value(value, path)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn string_from_sources(sources: &[(Option<&Value>, &[&str])]) -> Option<String> {
    sources
        .iter()
        .filter_map(|(value, path)| value.and_then(|value| string_path(value, path)))
        .find(|value| !value.trim().is_empty())
}

fn u64_from_sources(sources: &[(Option<&Value>, &[&str])]) -> Option<u64> {
    sources
        .iter()
        .filter_map(|(value, path)| {
            let value = value.and_then(|value| path_value(value, path))?;
            value.as_u64()
        })
        .next()
}

fn first_string_array_item(value: &Value, path: &[&str]) -> Option<String> {
    path_value(value, path)?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .find(|item| !item.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn bool_path(value: &Value, path: &[&str]) -> Option<bool> {
    path_value(value, path)?.as_bool()
}

fn regenerate_gap_ledger_command(out: &str) -> String {
    format!(
        "ripr reports gap-ledger --repo-exposure target/ripr/reports/repo-exposure.json --out {out} --out-md {}",
        with_extension(out, "md")
    )
}

fn with_extension(path: &str, extension: &str) -> String {
    let mut path = PathBuf::from(path);
    path.set_extension(extension);
    path.display().to_string().replace('\\', "/")
}

fn render_start_here_markdown(packet: &Value) -> String {
    let selected = packet.get("selected").unwrap_or(&Value::Null);
    let state = string_path(selected, &["state"]).unwrap_or_else(|| "unknown".to_string());
    let mut out = String::new();
    out.push_str("# RIPR First PR Start Here\n\n");
    out.push_str("Status: advisory\n");
    out.push_str(&format!(
        "State: {}\n\n",
        packet
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
    ));

    match state.as_str() {
        "top_gap" => render_top_gap_markdown(selected, &mut out),
        "missing_artifact" => render_missing_artifact_markdown(selected, &mut out),
        "empty_diff" | "no_action" => render_no_action_markdown(selected, &mut out),
        _ => render_blocked_markdown(selected, &mut out),
    }

    out.push_str("\n## Artifacts\n\n");
    if let Some(artifacts) = packet.get("artifacts").and_then(Value::as_array) {
        for artifact in artifacts {
            let label = artifact
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("artifact");
            let path = artifact.get("path").and_then(Value::as_str).unwrap_or("");
            let status = artifact
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            out.push_str(&format!("- {label}: `{path}` ({status})\n"));
        }
    }

    out.push_str("\n## Authority\n\n");
    out.push_str(
        "This packet is advisory. Pass/fail authority remains with explicit gate-decision artifacts when configured.\n",
    );

    out.push_str("\n## Limits\n\n");
    if let Some(limits) = packet.get("limits").and_then(Value::as_array) {
        for limit in limits.iter().filter_map(Value::as_str) {
            out.push_str(&format!("- {limit}\n"));
        }
    }
    out
}

fn render_top_gap_markdown(selected: &Value, out: &mut String) {
    out.push_str("## Top Gap\n\n");
    let kind = selected
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("gap");
    out.push_str(&format!("ripr gap: {}\n\n", sentence_case(kind)));
    if let Some(changed) = selected.get("changed_behavior").and_then(Value::as_str) {
        out.push_str("Changed behavior:\n");
        out.push_str(&format!("`{}`\n\n", changed.trim()));
    }
    if let Some(why) = selected.get("why").and_then(Value::as_str) {
        out.push_str("Why this matters:\n");
        out.push_str(why);
        out.push_str("\n\n");
    }
    if let Some(repair) = selected.get("repair").and_then(Value::as_object) {
        out.push_str("Repair:\n");
        if let Some(route) = repair.get("route").and_then(Value::as_str) {
            out.push_str(&format!("- Route: `{route}`\n"));
        }
        if let Some(target) = repair.get("target_file").and_then(Value::as_str) {
            out.push_str(&format!("- Target: `{target}`\n"));
        }
        if let Some(assertion) = repair.get("suggested_assertion").and_then(Value::as_str) {
            out.push_str(&format!("- Assertion: `{assertion}`\n"));
        }
        out.push('\n');
    }
    if let Some(command) = selected.get("verify_command").and_then(Value::as_str) {
        out.push_str("Verify:\n");
        out.push_str(&format!("`{command}`\n\n"));
    }
    if let Some(command) = selected.get("agent_packet_command").and_then(Value::as_str) {
        out.push_str("Agent packet:\n");
        out.push_str(&format!("`{command}`\n"));
    }
}

fn render_missing_artifact_markdown(selected: &Value, out: &mut String) {
    out.push_str("## Next\n\n");
    let artifact = selected.get("artifact").unwrap_or(&Value::Null);
    let label = artifact
        .get("label")
        .and_then(Value::as_str)
        .unwrap_or("required artifact");
    let path = artifact.get("path").and_then(Value::as_str).unwrap_or("");
    out.push_str(&format!("Regenerate missing {label}: `{path}`.\n\n"));
    if let Some(command) = selected.get("regeneration_command").and_then(Value::as_str) {
        out.push_str("Run:\n");
        out.push_str(&format!("`{command}`\n"));
    }
}

fn render_no_action_markdown(selected: &Value, out: &mut String) {
    out.push_str("## No Action\n\n");
    let reason = selected
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("No repairable PR-local Rust gap was selected.");
    out.push_str(reason);
    out.push_str("\n\nNo-action is not a runtime, coverage, or mutation adequacy claim.\n");
}

fn render_blocked_markdown(selected: &Value, out: &mut String) {
    out.push_str("## Blocked\n\n");
    let message = selected
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("First-run packet is blocked by unavailable evidence.");
    out.push_str(message);
    out.push_str("\n\n");
    if let Some(command) = selected.get("next_command").and_then(Value::as_str) {
        out.push_str("Next:\n");
        out.push_str(&format!("`{command}`\n"));
    }
}

fn validate_start_here_packet(json_path: &Path, markdown_path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(json_path)
        .map_err(|err| format!("missing or unreadable {}: {err}", json_path.display()))?;
    let packet: Value = serde_json::from_str(&text)
        .map_err(|err| format!("{} is not valid JSON: {err}", json_path.display()))?;
    let mut violations = Vec::new();
    expect_string(&packet, "schema_version", SCHEMA_VERSION, &mut violations);
    expect_string(&packet, "tool", "ripr", &mut violations);
    expect_string(&packet, "kind", "first_pr_start_here", &mut violations);
    match packet.get("status").and_then(Value::as_str) {
        Some("actionable" | "blocked" | "no_action") => {}
        Some(status) => violations.push(format!("status {status:?} is not contract-valid")),
        None => violations.push("status is missing or not a string".to_string()),
    }
    expect_string(&packet, "posture", "advisory", &mut violations);
    if let Some(selected) = packet.get("selected").filter(|value| value.is_object()) {
        validate_selected_state(
            packet
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            selected,
            &mut violations,
        );
    } else {
        violations.push("selected is missing or not an object".to_string());
    }
    if !packet.get("commands").is_some_and(Value::is_object) {
        violations.push("commands is missing or not an object".to_string());
    }
    if !packet.get("artifacts").is_some_and(Value::is_array) {
        violations.push("artifacts is missing or not an array".to_string());
    }
    if !markdown_path.exists() {
        violations.push(format!("{} is missing", markdown_path.display()));
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "first-pr start-here contract violations:\n{}",
            violations
                .iter()
                .map(|violation| format!("- {violation}"))
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }
}

fn validate_selected_state(status: &str, selected: &Value, violations: &mut Vec<String>) {
    let Some(state) = selected.get("state").and_then(Value::as_str) else {
        violations.push("selected.state is missing or not a string".to_string());
        return;
    };
    let expected_status = match state {
        "top_gap" => "actionable",
        "missing_artifact" | "malformed_artifact" | "stale_artifact" | "wrong_root"
        | "blocked_artifact" | "timeout" => "blocked",
        "empty_diff" | "no_action" => "no_action",
        other => {
            violations.push(format!("selected.state {other:?} is not contract-valid"));
            return;
        }
    };
    if status != expected_status {
        violations.push(format!(
            "selected.state {state:?} requires status {expected_status:?}, found {status:?}"
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

fn sentence_case(value: &str) -> String {
    let mut out = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index > 0 && ch.is_uppercase() {
            out.push(' ');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

fn resolve_path(root: &Path, path: &str) -> PathBuf {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    }
}

fn normalized_path(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

fn repo_root() -> Result<PathBuf, String> {
    env::current_dir().map_err(|err| format!("failed to resolve current directory: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parse_accepts_artifact_paths_and_check() -> Result<(), String> {
        let parsed = parse_options(&[
            "--root".to_string(),
            "repo".to_string(),
            "--base".to_string(),
            "origin/main".to_string(),
            "--head".to_string(),
            "HEAD".to_string(),
            "--gap-ledger".to_string(),
            "gap.json".to_string(),
            "--out-dir".to_string(),
            "out".to_string(),
            "--check".to_string(),
        ])?;
        assert_eq!(parsed.root, "repo");
        assert_eq!(parsed.base, "origin/main");
        assert_eq!(parsed.head, "HEAD");
        assert_eq!(parsed.gap_ledger, "gap.json");
        assert_eq!(parsed.out_dir, "out");
        assert!(parsed.check);
        assert_eq!(
            parse_options(&["--gap-ledger".to_string(), "".to_string()]),
            Err("first-pr --gap-ledger requires a non-empty value".to_string())
        );
        Ok(())
    }

    #[test]
    fn selects_repairable_rust_gap_from_ledger() -> Result<(), String> {
        let repo = temp_repo("first-pr-select")?;
        let ledger = repo.join(DEFAULT_GAP_LEDGER);
        write_json(&ledger, ledger_with_repairable_gap())?;
        let options = FirstPrOptions::default();
        let packet = render_start_here_packet(&repo, &options);
        assert_eq!(packet["status"], "actionable");
        assert_eq!(packet["selected"]["state"], "top_gap");
        assert_eq!(
            packet["selected"]["gap_id"],
            "gap:pr:pricing:threshold-boundary"
        );
        assert_eq!(
            packet["selected"]["repair"]["route"],
            "AddBoundaryAssertion"
        );
        assert!(
            packet["commands"]["agent_packet"].as_str().is_some_and(
                |command| command.contains("--gap-id gap:pr:pricing:threshold-boundary")
            )
        );
        cleanup(&repo)
    }

    #[test]
    fn missing_gap_ledger_writes_recovery_packet() -> Result<(), String> {
        let repo = temp_repo("first-pr-missing")?;
        let options = FirstPrOptions::default();
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "missing_artifact");
        assert!(
            packet["selected"]["regeneration_command"]
                .as_str()
                .is_some_and(|command| command.contains("ripr reports gap-ledger"))
        );
        check_first_pr(&repo, &options)?;
        cleanup(&repo)
    }

    #[test]
    fn malformed_gap_ledger_writes_blocked_packet() -> Result<(), String> {
        let repo = temp_repo("first-pr-malformed")?;
        let ledger = repo.join(DEFAULT_GAP_LEDGER);
        let parent = ledger
            .parent()
            .ok_or_else(|| "ledger path has no parent".to_string())?;
        fs::create_dir_all(parent).map_err(|err| format!("mkdir {}: {err}", parent.display()))?;
        fs::write(&ledger, "{not-json")
            .map_err(|err| format!("write {}: {err}", ledger.display()))?;
        let options = FirstPrOptions::default();
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "malformed_artifact");
        cleanup(&repo)
    }

    #[test]
    fn stale_gap_ledger_suppresses_repair_selection() -> Result<(), String> {
        let repo = temp_repo("first-pr-stale")?;
        let ledger = repo.join(DEFAULT_GAP_LEDGER);
        let mut value = ledger_with_repairable_gap();
        value["status"] = json!("stale");
        write_json(&ledger, value)?;
        let packet = render_start_here_packet(&repo, &FirstPrOptions::default());
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "stale_artifact");
        assert!(
            packet["selected"]["next_command"]
                .as_str()
                .is_some_and(|command| command.contains("ripr reports gap-ledger"))
        );
        cleanup(&repo)
    }

    #[test]
    fn wrong_root_gap_ledger_suppresses_repair_selection() -> Result<(), String> {
        let repo = temp_repo("first-pr-wrong-root")?;
        let ledger = repo.join(DEFAULT_GAP_LEDGER);
        let mut value = ledger_with_repairable_gap();
        value["root"] = json!("other-workspace");
        write_json(&ledger, value)?;
        let packet = render_start_here_packet(&repo, &FirstPrOptions::default());
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "wrong_root");
        assert!(
            packet["selected"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("other-workspace"))
        );
        cleanup(&repo)
    }

    #[test]
    fn timeout_gap_ledger_writes_retry_packet() -> Result<(), String> {
        let repo = temp_repo("first-pr-timeout")?;
        let ledger = repo.join(DEFAULT_GAP_LEDGER);
        let mut value = ledger_with_repairable_gap();
        value["status"] = json!("timeout");
        write_json(&ledger, value)?;
        let packet = render_start_here_packet(&repo, &FirstPrOptions::default());
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "timeout");
        assert!(
            packet["selected"]["next_command"]
                .as_str()
                .is_some_and(|command| command.contains("ripr reports gap-ledger"))
        );
        cleanup(&repo)
    }

    #[test]
    fn blocked_gap_ledger_writes_retry_packet() -> Result<(), String> {
        let repo = temp_repo("first-pr-blocked-ledger")?;
        let ledger = repo.join(DEFAULT_GAP_LEDGER);
        write_json(
            &ledger,
            json!({
                "schema_version": "0.1",
                "kind": "gap_decision_ledger",
                "status": "blocked",
                "warnings": ["read missing.json failed: not found"],
                "summary": {"records_total": 0},
                "records": []
            }),
        )?;
        let packet = render_start_here_packet(&repo, &FirstPrOptions::default());
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "blocked_artifact");
        assert!(
            packet["selected"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("read missing.json failed"))
        );
        assert!(
            packet["selected"]["next_command"]
                .as_str()
                .is_some_and(|command| command.contains("ripr reports gap-ledger"))
        );
        cleanup(&repo)
    }

    #[test]
    fn empty_diff_gap_ledger_is_schema_valid_no_action() -> Result<(), String> {
        let repo = temp_repo("first-pr-empty-diff")?;
        let ledger = repo.join(DEFAULT_GAP_LEDGER);
        write_json(
            &ledger,
            json!({
                "schema_version": "0.1",
                "kind": "gap_decision_ledger",
                "status": "empty_diff",
                "summary": {"records_total": 0},
                "records": []
            }),
        )?;
        let options = FirstPrOptions::default();
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        let markdown = fs::read_to_string(repo.join(DEFAULT_OUT_DIR).join(START_HERE_MD))
            .map_err(|err| format!("read start-here markdown: {err}"))?;
        assert_eq!(packet["status"], "no_action");
        assert_eq!(packet["selected"]["state"], "empty_diff");
        assert_eq!(packet["selected"]["records_total"], 0);
        assert!(markdown.contains("## No Action"));
        assert!(!markdown.contains("## Blocked"));
        cleanup(&repo)
    }

    #[test]
    fn no_repairable_gap_is_advisory_no_action() -> Result<(), String> {
        let repo = temp_repo("first-pr-no-action")?;
        let ledger = repo.join(DEFAULT_GAP_LEDGER);
        write_json(
            &ledger,
            json!({
                "schema_version": "0.1",
                "records": [
                    {
                        "gap_id": "gap:report-only",
                        "language": "rust",
                        "language_status": "stable",
                        "scope": "pr_local",
                        "gap_state": "report_only",
                        "policy_state": "not_policy_targeted",
                        "repairability": "analyzer_limitation"
                    }
                ]
            }),
        )?;
        let packet = render_start_here_packet(&repo, &FirstPrOptions::default());
        assert_eq!(packet["status"], "no_action");
        assert_eq!(packet["selected"]["state"], "no_action");
        cleanup(&repo)
    }

    #[test]
    fn first_successful_pr_fixture_corpus_matches_expected_outputs() -> Result<(), String> {
        let corpus = fixture_repo_root()?.join("fixtures/first_successful_pr");
        let manifest = read_packet(&corpus.join("corpus.json"))?;
        let cases = manifest
            .get("cases")
            .and_then(Value::as_array)
            .ok_or_else(|| "first_successful_pr corpus is missing cases".to_string())?;
        for case in cases {
            let case_id = string_path(case, &["id"])
                .ok_or_else(|| "first_successful_pr case is missing id".to_string())?;
            assert_first_successful_pr_case(&corpus, &case_id)?;
        }
        Ok(())
    }

    fn ledger_with_repairable_gap() -> Value {
        json!({
            "schema_version": "0.1",
            "kind": "gap_decision_ledger",
            "records": [
                {
                    "gap_id": "gap:preview",
                    "kind": "MissingBoundaryAssertion",
                    "language": "typescript",
                    "language_status": "preview",
                    "scope": "pr_local",
                    "gap_state": "actionable",
                    "policy_state": "new",
                    "repairability": "repairable",
                    "repair_route": {
                        "route_kind": "AddBoundaryAssertion"
                    },
                    "verification_commands": ["cargo xtask fixtures"]
                },
                {
                    "gap_id": "gap:pr:pricing:threshold-boundary",
                    "canonical_gap_id": "gap:rust:pricing:discount:threshold-boundary",
                    "kind": "MissingBoundaryAssertion",
                    "language": "rust",
                    "language_status": "stable",
                    "scope": "pr_local",
                    "gap_state": "actionable",
                    "policy_state": "new",
                    "repairability": "repairable",
                    "anchor": {
                        "file": "src/pricing.rs",
                        "line": 42,
                        "owner": "pricing::discount",
                        "dedupe_fingerprint": "gap:rust:pricing:discount:threshold-boundary"
                    },
                    "repair_route": {
                        "route_kind": "AddBoundaryAssertion",
                        "target_file": "tests/pricing.rs",
                        "assertion_shape": "assert_eq!(discount(100, 100), 90)"
                    },
                    "verification_commands": [
                        "cargo xtask fixtures boundary_gap",
                        "cargo xtask goldens check"
                    ]
                }
            ]
        })
    }

    fn write_json(path: &Path, value: Value) -> Result<(), String> {
        let parent = path
            .parent()
            .ok_or_else(|| format!("{} has no parent", path.display()))?;
        fs::create_dir_all(parent).map_err(|err| format!("mkdir {}: {err}", parent.display()))?;
        let text =
            serde_json::to_string_pretty(&value).map_err(|err| format!("serialize json: {err}"))?;
        fs::write(path, text).map_err(|err| format!("write {}: {err}", path.display()))
    }

    fn read_packet(path: &Path) -> Result<Value, String> {
        let text =
            fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
        serde_json::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
    }

    fn assert_first_successful_pr_case(corpus: &Path, case_id: &str) -> Result<(), String> {
        let case = corpus.join(case_id);
        let options = FirstPrOptions {
            root: format!("fixtures/first_successful_pr/{case_id}"),
            gap_ledger: "inputs/reports/gap-decision-ledger.json".to_string(),
            ..FirstPrOptions::default()
        };
        let actual_json = render_start_here_packet(&case, &options);
        let expected_json = read_packet(&case.join("expected/start-here.json"))?;
        assert_eq!(
            actual_json, expected_json,
            "start-here JSON drift in {case_id}"
        );

        let actual_md = render_start_here_markdown(&actual_json);
        let expected_md = fs::read_to_string(case.join("expected/start-here.md"))
            .map_err(|err| format!("read expected start-here markdown for {case_id}: {err}"))?;
        assert_eq!(
            actual_md.replace("\r\n", "\n"),
            expected_md.replace("\r\n", "\n"),
            "start-here Markdown drift in {case_id}"
        );
        Ok(())
    }

    fn temp_repo(name: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock error: {err}"))?
            .as_nanos();
        let path = env::temp_dir().join(format!("ripr-{name}-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&path).map_err(|err| format!("mkdir {}: {err}", path.display()))?;
        Ok(path)
    }

    fn cleanup(path: &Path) -> Result<(), String> {
        if path.exists() {
            fs::remove_dir_all(path).map_err(|err| format!("cleanup {}: {err}", path.display()))?;
        }
        Ok(())
    }

    fn fixture_repo_root() -> Result<PathBuf, String> {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .ok_or_else(|| "failed to resolve fixture repo root".to_string())
    }
}
