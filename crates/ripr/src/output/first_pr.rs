use crate::agent::loop_commands::{
    WORKFLOW_AFTER_SNAPSHOT_ARTIFACT, WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
    check_repo_exposure_command, display_path, outcome_command, shell_arg,
};
use crate::config::{CONFIG_FILE_NAME, detect_python_project};
use crate::output::receipt_lifecycle::receipt_lifecycle_state;
use crate::output::start_here_state::{
    START_HERE_PREVIEW_LIMITED, normalize_start_here_output_state, start_here_output_state_is_known,
};
use serde_json::{Map, Value, json};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SCHEMA_VERSION: &str = "0.1";
const DEFAULT_ROOT: &str = ".";
const DEFAULT_OUT_DIR: &str = "target/ripr/reports";
const DEFAULT_BASE: &str = "origin/main";
const DEFAULT_HEAD: &str = "HEAD";
const START_HERE_JSON: &str = "start-here.json";
const START_HERE_MD: &str = "start-here.md";
const DEFAULT_REPO_EXPOSURE: &str = "target/ripr/reports/repo-exposure.json";
const DEFAULT_REPO_EXPOSURE_LATENCY_JSON: &str = "target/ripr/reports/repo-exposure-latency.json";
const DEFAULT_REPO_EXPOSURE_LATENCY_REPORT: &str = "target/ripr/reports/repo-exposure-latency.md";
const DEFAULT_CHECK_OUTPUT: &str = "target/ripr/reports/check.json";
const DEFAULT_GAP_LEDGER: &str = "target/ripr/reports/gap-decision-ledger.json";
const DEFAULT_FIRST_ACTION: &str = "target/ripr/reports/first-useful-action.json";
const DEFAULT_REVIEW_COMMENTS: &str = "target/ripr/review/comments.json";
const DEFAULT_AGENT_PACKET: &str = "target/ripr/workflow/agent-packet.json";
const DEFAULT_GATE_DECISION: &str = "target/ripr/reports/gate-decision.json";
const DEFAULT_RECEIPTS_DIR: &str = "target/ripr/receipts";
const REPO_EXPOSURE_LATENCY_REPORT_COMMAND: &str = "cargo xtask repo-exposure-latency-report";
pub(crate) const STATIC_EVIDENCE_BOUNDARY: &str = "static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval.";

mod options;
mod rendering;
mod validation;

#[cfg(test)]
use options::first_pr_help_text;
use options::{FirstPrOptions, parse_options, print_help};
#[cfg(test)]
use rendering::markdown_code_or_text;
use rendering::{render_start_here_markdown, start_here_cli_summary};
#[cfg(test)]
use validation::validate_selected_state;
use validation::validate_start_here_packet;

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

fn write_first_pr(repo: &Path, options: &FirstPrOptions) -> Result<(), String> {
    let root = resolve_path(repo, &options.root);
    let root_recovery = root_preflight_recovery(&root, options);
    let preflight_recovery = root_recovery
        .clone()
        .or_else(|| git_preflight_recovery(&root, options));
    let output_root = if root_recovery.is_some() { repo } else { &root };
    if preflight_recovery.is_none() {
        materialize_check_output_gap_ledger(&root, options)?;
    }
    let packet = match preflight_recovery {
        Some(selection) => render_start_here_recovery_packet(&root, options, selection),
        None => render_start_here_packet(&root, options),
    };
    let out_dir = resolve_path(output_root, &options.out_dir);
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
    print!(
        "{}",
        start_here_cli_summary(&packet, &json_path, &markdown_path)
    );
    println!("Wrote {}", json_path.display());
    println!("Wrote {}", markdown_path.display());
    Ok(())
}

fn check_first_pr(repo: &Path, options: &FirstPrOptions) -> Result<(), String> {
    let root = resolve_path(repo, &options.root);
    let root_recovery = root_preflight_recovery(&root, options);
    let preflight_recovery = root_recovery
        .clone()
        .or_else(|| git_preflight_recovery(&root, options));
    let output_root = if root_recovery.is_some() { repo } else { &root };
    let out_dir = resolve_path(output_root, &options.out_dir);
    let json_path = out_dir.join(START_HERE_JSON);
    let markdown_path = out_dir.join(START_HERE_MD);
    let packet = validate_start_here_packet(&json_path, &markdown_path)?;
    validate_current_preflight_recovery(&packet, &root, options, preflight_recovery)?;
    print!(
        "{}",
        start_here_cli_summary(&packet, &json_path, &markdown_path)
    );
    println!("First PR start-here packet ok: {}", json_path.display());
    Ok(())
}

fn validate_current_preflight_recovery(
    packet: &Value,
    root: &Path,
    options: &FirstPrOptions,
    preflight_recovery: Option<Selection>,
) -> Result<(), String> {
    let Some(selection) = preflight_recovery else {
        return Ok(());
    };
    let expected = render_start_here_recovery_packet(root, options, selection);
    if packet.get("status") == expected.get("status")
        && packet.get("selected") == expected.get("selected")
    {
        return Ok(());
    }
    Err(format!(
        "first-pr start-here packet is stale for current root/git preflight; rerun `ripr first-pr --root {} --base {} --head {}` before relying on it",
        options.root, options.base, options.head
    ))
}

fn render_start_here_packet(root: &Path, options: &FirstPrOptions) -> Value {
    let gap_path = resolve_path(root, &options.gap_ledger);
    let mut warnings = Vec::new();
    let selection = match read_json(&gap_path) {
        Ok(gap_ledger) => select_from_gap_ledger(&gap_ledger, root, options),
        Err(ArtifactReadError::Missing) => missing_gap_ledger_selection(root, options),
        Err(ArtifactReadError::Malformed(message)) => Selection::blocked(
            "malformed_artifact",
            format!("The gap decision ledger could not be parsed: {message}"),
            Some(format!(
                "Regenerate the gap ledger with `{}` before assigning repair work.",
                regenerate_gap_ledger_command(root, options)
            )),
        ),
    };
    if let Some(warning) = selection.warning() {
        warnings.push(warning);
    }
    let preflight = options.preflight.then(|| first_pr_preflight(root, options));
    if let Some(preflight) = &preflight {
        warnings.extend(preflight.warnings());
    }

    render_start_here_packet_with_selection(root, options, selection, warnings, preflight)
}

fn render_start_here_recovery_packet(
    root: &Path,
    options: &FirstPrOptions,
    selection: Selection,
) -> Value {
    let mut warnings = selection.warning().into_iter().collect::<Vec<_>>();
    let preflight = options.preflight.then(|| first_pr_preflight(root, options));
    if let Some(preflight) = &preflight {
        warnings.extend(preflight.warnings());
    }
    render_start_here_packet_with_selection(root, options, selection, warnings, preflight)
}

fn render_start_here_packet_with_selection(
    root: &Path,
    options: &FirstPrOptions,
    selection: Selection,
    warnings: Vec<String>,
    preflight: Option<FirstPrPreflight>,
) -> Value {
    let mut artifacts = Vec::new();
    if let Some(check_output) = options.check_output.as_deref() {
        artifacts.push(artifact_status(
            root,
            "check_output",
            "Check output",
            check_output,
            Some(format!(
                "ripr check --root {} --base {} --json > {}",
                options.root, options.base, check_output
            )),
        ));
    }
    artifacts.extend([
        artifact_status(
            root,
            "gap_ledger",
            "Gap decision ledger",
            &options.gap_ledger,
            Some(regenerate_gap_ledger_command(root, options)),
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
    ]);

    let mut inputs = json!({
        "gap_ledger": options.gap_ledger,
        "base": options.base,
        "head": options.head,
        "first_action": options.first_action,
        "review_comments": options.review_comments,
        "agent_packet": options.agent_packet,
        "gate_decision": options.gate_decision,
        "receipts_dir": options.receipts_dir
    });
    if let Some(check_output) = options.check_output.as_deref()
        && let Some(inputs) = inputs.as_object_mut()
    {
        inputs.insert(
            "check_output".to_string(),
            Value::String(check_output.to_string()),
        );
    }

    let mut packet = json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "kind": "first_pr_start_here",
        "status": selection.status(),
        "posture": "advisory",
        "root": options.root,
        "inputs": inputs,
        "selected": selection.to_json(),
        "commands": selection.commands_json(root, options),
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
    });
    if let Some(preflight) = preflight {
        packet["preflight"] = preflight.to_json();
    }
    packet
}

fn materialize_check_output_gap_ledger(
    root: &Path,
    options: &FirstPrOptions,
) -> Result<(), String> {
    let Some(check_output) = options.check_output.as_deref() else {
        return Ok(());
    };
    let check_output_path = resolve_path(root, check_output);
    let contents = fs::read_to_string(&check_output_path).map_err(|err| {
        format!(
            "first-pr --check-output {} is invalid: read failed: {err}",
            check_output_path.display()
        )
    })?;
    let report = crate::output::gap_decision_ledger::build_gap_decision_ledger_report(
        crate::output::gap_decision_ledger::GapDecisionLedgerInput {
            root: options.root.clone(),
            generated_at: "first-pr-check-output".to_string(),
            source_kind:
                crate::output::gap_decision_ledger::GapDecisionLedgerSourceKind::CheckOutput,
            records_path: check_output.to_string(),
            records_json: Ok(contents),
        },
    );
    let json = crate::output::gap_decision_ledger::render_gap_decision_ledger_json(&report)?;
    let markdown = crate::output::gap_decision_ledger::render_gap_decision_ledger_markdown(&report);
    let gap_ledger_path = resolve_path(root, &options.gap_ledger);
    if let Some(parent) = gap_ledger_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    fs::write(&gap_ledger_path, json)
        .map_err(|err| format!("failed to write {}: {err}", gap_ledger_path.display()))?;
    let gap_ledger_markdown = resolve_path(root, &with_extension(&options.gap_ledger, "md"));
    if let Some(parent) = gap_ledger_markdown.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    fs::write(&gap_ledger_markdown, markdown)
        .map_err(|err| format!("failed to write {}: {err}", gap_ledger_markdown.display()))?;
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FirstPrPreflight {
    status: &'static str,
    mode: &'static str,
    root: String,
    resolved_root: String,
    base: String,
    head: String,
    next_command: Option<String>,
    checks: Vec<PreflightCheck>,
}

impl FirstPrPreflight {
    fn warnings(&self) -> impl Iterator<Item = String> + '_ {
        self.checks
            .iter()
            .filter(|check| {
                check.status != "ok" && check.status != "defaulted" && check.status != "will_create"
            })
            .map(|check| check.message.clone())
    }

    fn to_json(&self) -> Value {
        json!({
            "status": self.status,
            "mode": self.mode,
            "root": self.root,
            "resolved_root": self.resolved_root,
            "base": self.base,
            "head": self.head,
            "next_command": self.next_command,
            "checks": self.checks.iter().map(PreflightCheck::to_json).collect::<Vec<_>>()
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreflightCheck {
    id: &'static str,
    label: &'static str,
    status: &'static str,
    message: String,
    path: Option<String>,
    next_command: Option<String>,
}

impl PreflightCheck {
    fn ok(id: &'static str, label: &'static str, message: impl Into<String>) -> Self {
        Self {
            id,
            label,
            status: "ok",
            message: message.into(),
            path: None,
            next_command: None,
        }
    }

    fn defaulted(id: &'static str, label: &'static str, message: impl Into<String>) -> Self {
        Self {
            id,
            label,
            status: "defaulted",
            message: message.into(),
            path: None,
            next_command: None,
        }
    }

    fn needs_attention(
        id: &'static str,
        label: &'static str,
        message: impl Into<String>,
        next_command: Option<String>,
    ) -> Self {
        Self {
            id,
            label,
            status: "needs_attention",
            message: message.into(),
            path: None,
            next_command,
        }
    }

    fn no_action(
        id: &'static str,
        label: &'static str,
        message: impl Into<String>,
        next_command: Option<String>,
    ) -> Self {
        Self {
            id,
            label,
            status: "no_action",
            message: message.into(),
            path: None,
            next_command,
        }
    }

    fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "label": self.label,
            "status": self.status,
            "message": self.message,
            "path": self.path,
            "next_command": self.next_command
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CommandOutput {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

impl CommandOutput {
    fn success(&self) -> bool {
        matches!(self.code, Some(0))
    }
}

fn first_pr_preflight(root: &Path, options: &FirstPrOptions) -> FirstPrPreflight {
    let mut checks = Vec::new();
    let resolved_root = root.display().to_string();
    checks.push(preflight_root_check(root, options));
    let git_available = matches!(checks.last().map(|check| check.status), Some("ok"))
        && preflight_git_repo_check(root, &mut checks);
    let mut base_ok = false;
    let mut head_ok = false;
    if git_available {
        base_ok = preflight_git_ref_check(
            root,
            &mut checks,
            "git_base",
            "Git base",
            &options.base,
            Some(missing_base_command(options)),
        );
        head_ok = preflight_git_ref_check(
            root,
            &mut checks,
            "git_head",
            "Git head",
            &options.head,
            Some(format!(
                "Check --head `{}` or fetch the branch, then rerun `ripr first-pr --root {} --base {} --head {}`.",
                options.head, options.root, options.base, options.head
            )),
        );
    }
    if git_available && base_ok && head_ok {
        preflight_diff_check(root, options, &mut checks);
    }
    checks.push(preflight_project_check(root));
    checks.push(preflight_config_check(root));
    checks.push(preflight_output_check(root, options));
    checks.push(PreflightCheck::ok(
        "mode",
        "Mode",
        if options.check {
            "Check mode validates the existing start-here packet without rewriting it."
        } else {
            "Write mode composes start-here.json and start-here.md from explicit artifacts."
        },
    ));
    let next_command = checks.iter().find_map(|check| check.next_command.clone());
    let status = if checks
        .iter()
        .any(|check| check.status == "needs_attention" || check.status == "no_action")
    {
        "needs_attention"
    } else {
        "ready"
    };
    FirstPrPreflight {
        status,
        mode: if options.check { "check" } else { "write" },
        root: options.root.clone(),
        resolved_root,
        base: options.base.clone(),
        head: options.head.clone(),
        next_command,
        checks,
    }
}

fn preflight_root_check(root: &Path, options: &FirstPrOptions) -> PreflightCheck {
    if root.is_dir() {
        PreflightCheck::ok(
            "root",
            "Workspace root",
            format!("Workspace root `{}` exists.", options.root),
        )
        .with_path(root.display().to_string())
    } else {
        PreflightCheck::needs_attention(
            "root",
            "Workspace root",
            format!(
                "Workspace root `{}` does not exist or is not a directory.",
                options.root
            ),
            Some("Run from a repository root or pass --root <path>.".to_string()),
        )
        .with_path(root.display().to_string())
    }
}

fn preflight_git_repo_check(root: &Path, checks: &mut Vec<PreflightCheck>) -> bool {
    match run_git(root, &git_args(&["rev-parse", "--is-inside-work-tree"])) {
        Ok(output) if output.success() && output.stdout.trim() == "true" => {
            checks.push(PreflightCheck::ok(
                "git_repo",
                "Git repository",
                "The root is inside a Git worktree.",
            ));
            true
        }
        Ok(output) => {
            checks.push(PreflightCheck::needs_attention(
                "git_repo",
                "Git repository",
                command_problem(
                    "The root is not a Git worktree.",
                    &output,
                    "Run from a Git worktree or pass --root <repo>.",
                ),
                Some("Run from a Git worktree or pass --root <repo>.".to_string()),
            ));
            false
        }
        Err(message) => {
            checks.push(PreflightCheck::needs_attention(
                "git_repo",
                "Git repository",
                format!("Could not run git preflight: {message}."),
                Some(
                    "Install git or run first-pr from an environment where git is available."
                        .to_string(),
                ),
            ));
            false
        }
    }
}

fn preflight_git_ref_check(
    root: &Path,
    checks: &mut Vec<PreflightCheck>,
    id: &'static str,
    label: &'static str,
    rev: &str,
    next_command: Option<String>,
) -> bool {
    let commit = format!("{rev}^{{commit}}");
    match run_git(
        root,
        &[
            "rev-parse".to_string(),
            "--verify".to_string(),
            "--quiet".to_string(),
            commit,
        ],
    ) {
        Ok(output) if output.success() => {
            checks.push(PreflightCheck::ok(
                id,
                label,
                format!("Resolved `{rev}` to a commit."),
            ));
            true
        }
        Ok(output) => {
            checks.push(PreflightCheck::needs_attention(
                id,
                label,
                command_problem(
                    &format!("Could not resolve `{rev}` to a commit."),
                    &output,
                    "Fetch the missing ref or pass a resolvable --base/--head.",
                ),
                next_command,
            ));
            false
        }
        Err(message) => {
            checks.push(PreflightCheck::needs_attention(
                id,
                label,
                format!("Could not run git ref preflight for `{rev}`: {message}."),
                next_command,
            ));
            false
        }
    }
}

fn preflight_diff_check(root: &Path, options: &FirstPrOptions, checks: &mut Vec<PreflightCheck>) {
    let range = format!("{}..{}", options.base, options.head);
    match run_git(
        root,
        &[
            "diff".to_string(),
            "--quiet".to_string(),
            range.clone(),
            "--".to_string(),
        ],
    ) {
        Ok(output) if matches!(output.code, Some(0)) => {
            checks.push(PreflightCheck::no_action(
                "git_diff",
                "Git diff",
                format!("No file diff was found for `{range}`."),
                Some(format!(
                    "Choose a head with changes or rerun after committing PR work: `ripr first-pr --root {} --base {} --head {}`.",
                    options.root, options.base, options.head
                )),
            ));
        }
        Ok(output) if matches!(output.code, Some(1)) => {
            checks.push(PreflightCheck::ok(
                "git_diff",
                "Git diff",
                format!("Found a file diff for `{range}`."),
            ));
        }
        Ok(output) => {
            checks.push(PreflightCheck::needs_attention(
                "git_diff",
                "Git diff",
                command_problem(
                    &format!("Could not inspect diff range `{range}`."),
                    &output,
                    "Check --base and --head, then rerun first-pr.",
                ),
                Some(format!(
                    "Check --base and --head, then rerun `ripr first-pr --root {} --base {} --head {}`.",
                    options.root, options.base, options.head
                )),
            ));
        }
        Err(message) => {
            checks.push(PreflightCheck::needs_attention(
                "git_diff",
                "Git diff",
                format!("Could not run git diff preflight: {message}."),
                Some(
                    "Install git or rerun from an environment where git is available.".to_string(),
                ),
            ));
        }
    }
}

fn preflight_project_check(root: &Path) -> PreflightCheck {
    let manifest = root.join("Cargo.toml");
    if manifest.is_file() {
        PreflightCheck::ok(
            "cargo_workspace",
            "Cargo workspace",
            "Cargo.toml was found at the workspace root.",
        )
        .with_path(manifest.display().to_string())
    } else if detect_python_project(root) {
        PreflightCheck::ok(
            "python_project",
            "Python project",
            "Python project markers were found; first-pr can consume Python preview gap-ledger records.",
        )
        .with_path(root.display().to_string())
    } else {
        PreflightCheck::needs_attention(
            "cargo_workspace",
            "Cargo workspace",
            "No Cargo.toml was found at the workspace root.",
            Some(
                "Run from a Rust/Cargo workspace, a Python project root, or pass --root <repo>."
                    .to_string(),
            ),
        )
        .with_path(manifest.display().to_string())
    }
}

fn preflight_config_check(root: &Path) -> PreflightCheck {
    let config = root.join(CONFIG_FILE_NAME);
    if config.is_file() {
        PreflightCheck::ok(
            "ripr_config",
            "RIPR config",
            format!("{CONFIG_FILE_NAME} was found."),
        )
        .with_path(config.display().to_string())
    } else {
        PreflightCheck::defaulted(
            "ripr_config",
            "RIPR config",
            format!("No {CONFIG_FILE_NAME} was found; built-in advisory defaults apply."),
        )
        .with_path(config.display().to_string())
    }
}

fn preflight_output_check(root: &Path, options: &FirstPrOptions) -> PreflightCheck {
    let out_dir = resolve_path(root, &options.out_dir);
    if out_dir.exists() && !out_dir.is_dir() {
        return PreflightCheck::needs_attention(
            "output_dir",
            "Output directory",
            format!(
                "Output path `{}` exists but is not a directory.",
                options.out_dir
            ),
            Some("Choose a directory for --out-dir, then rerun first-pr.".to_string()),
        )
        .with_path(out_dir.display().to_string());
    }
    if out_dir.is_dir() {
        PreflightCheck::ok(
            "output_dir",
            "Output directory",
            format!("Output directory `{}` exists.", options.out_dir),
        )
        .with_path(out_dir.display().to_string())
    } else {
        PreflightCheck {
            id: "output_dir",
            label: "Output directory",
            status: "will_create",
            message: format!(
                "Output directory `{}` will be created if needed.",
                options.out_dir
            ),
            path: Some(out_dir.display().to_string()),
            next_command: None,
        }
    }
}

fn missing_base_command(options: &FirstPrOptions) -> String {
    options.base.strip_prefix("origin/")
        .filter(|branch| !branch.trim().is_empty())
        .map(|branch| {
            format!(
                "git fetch origin {branch}; then rerun `ripr first-pr --root {} --base {} --head {}`.",
                options.root, options.base, options.head
            )
        })
        .unwrap_or_else(|| {
            format!(
                "Fetch or choose a local base ref, then rerun `ripr first-pr --root {} --base {} --head {}`.",
                options.root, options.base, options.head
            )
        })
}

fn git_args(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_string()).collect()
}

fn run_git(root: &Path, args: &[String]) -> Result<CommandOutput, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|err| err.to_string())?;
    Ok(CommandOutput {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

fn command_problem(summary: &str, output: &CommandOutput, fallback: &str) -> String {
    let detail = output
        .stderr
        .trim()
        .lines()
        .next()
        .or_else(|| output.stdout.trim().lines().next())
        .filter(|line| !line.trim().is_empty());
    match detail {
        Some(detail) => format!("{summary} {detail}"),
        None => fallback.to_string(),
    }
}

fn root_preflight_recovery(root: &Path, options: &FirstPrOptions) -> Option<Selection> {
    if !root.is_dir() {
        return Some(Selection::blocked(
            "wrong_root",
            format!(
                "The first-pr root `{}` is not a directory. Pass an existing Rust/Cargo workspace with `--root` before assigning repair work.",
                options.root
            ),
            Some(doctor_command(&options.root)),
        ));
    }
    if !root.join("Cargo.toml").is_file() {
        if detect_python_project(root) {
            return None;
        }
        return Some(Selection::blocked(
            "wrong_root",
            format!(
                "The first-pr root `{}` is not a Rust/Cargo workspace because Cargo.toml is missing, and no Python project markers were found. Pass the repository root with `--root` before assigning repair work.",
                options.root
            ),
            Some(doctor_command(&options.root)),
        ));
    }
    None
}

fn git_preflight_recovery(root: &Path, options: &FirstPrOptions) -> Option<Selection> {
    match git_worktree_available(root) {
        Ok(true) => {}
        Ok(false) => {
            return Some(Selection::blocked(
                "blocked_artifact",
                format!(
                    "The first-pr root `{}` is not a git worktree. Run setup checks before assigning repair work.",
                    options.root
                ),
                Some(doctor_command(&options.root)),
            ));
        }
        Err(message) => {
            return Some(Selection::blocked(
                "blocked_artifact",
                format!(
                    "The first-pr git preflight could not run for root `{}`: {message}. Run setup checks before assigning repair work.",
                    options.root
                ),
                Some(doctor_command(&options.root)),
            ));
        }
    }

    match git_rev_exists(root, &options.base) {
        Ok(true) => {}
        Ok(false) => {
            return Some(Selection::blocked(
                "blocked_artifact",
                format!(
                    "The first-pr base `{}` does not resolve to a commit. Fetch the base ref or pass a valid `--base` before assigning repair work.",
                    options.base
                ),
                Some(fetch_base_command(options)),
            ));
        }
        Err(message) => {
            return Some(Selection::blocked(
                "blocked_artifact",
                format!(
                    "The first-pr base `{}` could not be checked: {message}. Run setup checks before assigning repair work.",
                    options.base
                ),
                Some(doctor_command(&options.root)),
            ));
        }
    }

    match git_rev_exists(root, &options.head) {
        Ok(true) => {}
        Ok(false) => {
            return Some(Selection::blocked(
                "blocked_artifact",
                format!(
                    "The first-pr head `{}` does not resolve to a commit. Pass a valid `--head` before assigning repair work.",
                    options.head
                ),
                Some(verify_ref_command(options, &options.head)),
            ));
        }
        Err(message) => {
            return Some(Selection::blocked(
                "blocked_artifact",
                format!(
                    "The first-pr head `{}` could not be checked: {message}. Run setup checks before assigning repair work.",
                    options.head
                ),
                Some(doctor_command(&options.root)),
            ));
        }
    }

    if let Err(message) = git_diff_range_valid(root, &options.base, &options.head) {
        return Some(Selection::blocked(
            "blocked_artifact",
            format!(
                "The first-pr diff range `{}...{}` could not be checked: {message}. Refresh the base/head inputs before assigning repair work.",
                options.base, options.head
            ),
            Some(diff_range_command(options)),
        ));
    }

    None
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

    fn commands_json(&self, root: &Path, options: &FirstPrOptions) -> Value {
        let mut commands = Map::new();
        commands.insert(
            "regenerate_gap_ledger".to_string(),
            Value::String(regenerate_gap_ledger_command(root, options)),
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
                commands.insert(
                    "receipt".to_string(),
                    Value::String(top_gap.receipt_command.clone()),
                );
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
                "output_state": normalize_start_here_output_state("missing_artifact"),
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
                "output_state": normalize_start_here_output_state(state),
                "message": message,
                "next_command": next_command
            }),
            Self::NoAction {
                state,
                reason,
                records_total,
            } => json!({
                "state": state,
                "output_state": normalize_start_here_output_state(state),
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
    language: Option<String>,
    language_status: Option<String>,
    kind: String,
    source_artifact: String,
    changed_behavior: Option<String>,
    current_evidence_strength: String,
    missing_discriminator: String,
    focused_proof_intent: String,
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
    receipt_command: String,
    receipt_path: String,
    receipt_command_source: String,
    receipt_state: Option<String>,
    static_limit_kind: Option<String>,
    static_limit_detail: Option<String>,
    agent_packet_command: String,
}

impl TopGapSelection {
    fn to_json(&self) -> Value {
        json!({
            "state": "top_gap",
            "output_state": self.output_state(),
            "gap_id": self.gap_id,
            "canonical_gap_id": self.canonical_gap_id,
            "language": self.language,
            "language_status": self.language_status,
            "kind": self.kind,
            "source_artifact": self.source_artifact,
            "changed_behavior": self.changed_behavior,
            "current_evidence_strength": self.current_evidence_strength,
            "missing_discriminator": self.missing_discriminator,
            "focused_proof_intent": self.focused_proof_intent,
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
            "receipt_path": self.receipt_path,
            "receipt_command_source": self.receipt_command_source,
            "receipt_state": self.receipt_state,
            "static_limit_kind": self.static_limit_kind,
            "static_limit_detail": self.static_limit_detail,
            "static_evidence_boundary": STATIC_EVIDENCE_BOUNDARY,
            "agent_packet_command": self.agent_packet_command
        })
    }

    fn output_state(&self) -> &'static str {
        if self.language_status.as_deref() == Some("preview") || self.static_limit_kind.is_some() {
            START_HERE_PREVIEW_LIMITED
        } else {
            normalize_start_here_output_state("top_gap")
        }
    }
}

fn select_from_gap_ledger(gap_ledger: &Value, root: &Path, options: &FirstPrOptions) -> Selection {
    let records = gap_records(gap_ledger);
    if ledger_reports_timeout(gap_ledger) {
        return Selection::blocked(
            "timeout",
            "The gap decision ledger reports a timeout; refresh the first-run evidence before assigning repair work.".to_string(),
            Some(regenerate_gap_ledger_command(root, options)),
        );
    }
    if ledger_reports_stale(gap_ledger) {
        return Selection::blocked(
            "stale_artifact",
            "The gap decision ledger is stale; refresh the first-run evidence before assigning repair work.".to_string(),
            Some(regenerate_gap_ledger_command(root, options)),
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
            Some(regenerate_gap_ledger_command(root, options)),
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
            Some(regenerate_gap_ledger_command(root, options)),
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
        return Selection::TopGap(Box::new(top_gap_from_record(record, root, options)));
    }
    Selection::no_action(
        "no_action",
        "No repairable PR-local stable Rust gap was selected from the gap decision ledger."
            .to_string(),
        records.len(),
    )
}

fn missing_gap_ledger_selection(root: &Path, options: &FirstPrOptions) -> Selection {
    if uses_python_check_output_gap_ledger(root) {
        return Selection::missing_artifact(
            "gap_ledger",
            "Gap decision ledger",
            &options.gap_ledger,
            regenerate_gap_ledger_command(root, options),
        );
    }

    let repo_exposure = resolve_path(root, DEFAULT_REPO_EXPOSURE);
    if !repo_exposure.exists() {
        return missing_repo_exposure_selection(root, options);
    }
    Selection::missing_artifact(
        "gap_ledger",
        "Gap decision ledger",
        &options.gap_ledger,
        regenerate_gap_ledger_command(root, options),
    )
}

fn missing_repo_exposure_selection(root: &Path, options: &FirstPrOptions) -> Selection {
    if repo_exposure_latency_report_available(root) {
        if let Some(summary) = repo_exposure_latency_report_summary(root) {
            return Selection::blocked(
                summary.selection_state(),
                summary.message(),
                Some(repo_exposure_latency_report_command(&options.root)),
            );
        }
        return Selection::blocked(
            "blocked_artifact",
            format!(
                "Repo exposure report is missing at `{DEFAULT_REPO_EXPOSURE}`; run the bounded repo-exposure latency report before assigning repair work."
            ),
            Some(repo_exposure_latency_report_command(&options.root)),
        );
    }
    Selection::missing_artifact(
        "repo_exposure",
        "Repo exposure report",
        DEFAULT_REPO_EXPOSURE,
        regenerate_repo_exposure_command(&options.root),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RepoExposureLatencySummary {
    format: String,
    status: String,
    trace_phase: Option<String>,
    trace_status: Option<String>,
}

impl RepoExposureLatencySummary {
    fn selection_state(&self) -> &'static str {
        match self.status.as_str() {
            "timeout" => "timeout",
            _ => "blocked_artifact",
        }
    }

    fn message(&self) -> String {
        let mut message = format!(
            "Repo exposure report is missing at `{DEFAULT_REPO_EXPOSURE}`; bounded latency JSON `{DEFAULT_REPO_EXPOSURE_LATENCY_JSON}` shows `{}` `{}`",
            self.format, self.status
        );
        if let (Some(phase), Some(status)) = (&self.trace_phase, &self.trace_status) {
            message.push_str(&format!(" at `{phase}` `{status}`"));
        }
        message.push_str(&format!(
            ". Inspect `{DEFAULT_REPO_EXPOSURE_LATENCY_REPORT}` before assigning repair work."
        ));
        message
    }
}

fn repo_exposure_latency_report_summary(root: &Path) -> Option<RepoExposureLatencySummary> {
    let report = read_json(&resolve_path(root, DEFAULT_REPO_EXPOSURE_LATENCY_JSON)).ok()?;
    if string_path(&report, &["schema_version"]).as_deref() != Some(SCHEMA_VERSION)
        || string_path(&report, &["tool"]).as_deref() != Some("ripr")
        || string_path(&report, &["report"]).as_deref() != Some("repo-exposure-latency")
    {
        return None;
    }
    let run = report
        .get("runs")?
        .as_array()?
        .iter()
        .find(|run| string_path(run, &["format"]).as_deref() == Some("repo-exposure-json"))?;
    let status = string_path(run, &["status"])?;
    if !matches!(status.as_str(), "timeout" | "fail") {
        return None;
    }
    let last_trace = run
        .get("trace")
        .and_then(Value::as_array)
        .and_then(|trace| trace.last());
    Some(RepoExposureLatencySummary {
        format: "repo-exposure-json".to_string(),
        status,
        trace_phase: last_trace.and_then(|trace| string_path(trace, &["phase"])),
        trace_status: last_trace.and_then(|trace| string_path(trace, &["status"])),
    })
}

fn repo_exposure_latency_report_available(root: &Path) -> bool {
    fs::read_to_string(root.join("xtask/src/command.rs"))
        .is_ok_and(|text| text.contains("\"repo-exposure-latency-report\""))
}

fn repo_exposure_latency_report_command(root: &str) -> String {
    if root == DEFAULT_ROOT {
        return REPO_EXPOSURE_LATENCY_REPORT_COMMAND.to_string();
    }
    let manifest_path = display_path(&Path::new(root).join("Cargo.toml"));
    format!(
        "cargo run --manifest-path {} -p xtask -- repo-exposure-latency-report",
        shell_arg(&manifest_path)
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
    first_pr_language_is_supported(record)
        && string_path(record, &["scope"]).is_some_and(|value| value == "pr_local")
        && string_path(record, &["gap_state"]).is_some_and(|value| value == "actionable")
        && string_path(record, &["repairability"]).is_some_and(|value| value == "repairable")
        && string_path(record, &["policy_state"])
            .is_some_and(|value| value == "new" || value == "reintroduced")
        && record.get("repair_route").is_some()
        && first_string_array_item(record, &["verification_commands"]).is_some()
}

fn first_pr_language_is_supported(record: &Value) -> bool {
    matches!(
        (
            string_path(record, &["language"]).as_deref(),
            string_path(record, &["language_status"]).as_deref(),
        ),
        (Some("rust"), Some("stable")) | (Some("python"), Some("preview"))
    )
}

fn top_gap_from_record(record: &Value, root: &Path, options: &FirstPrOptions) -> TopGapSelection {
    let repair_route = record.get("repair_route");
    let anchor = record.get("anchor");
    let gap_id = string_path(record, &["gap_id"]).unwrap_or_else(|| "unknown-gap".to_string());
    let kind = string_path(record, &["kind"]).unwrap_or_else(|| "Unknown".to_string());
    let language = string_path(record, &["language"]);
    let language_status = string_path(record, &["language_status"]);
    let changed_behavior = string_from_sources(&[
        (repair_route, &["changed_behavior"]),
        (Some(record), &["changed_behavior"]),
    ]);
    let verify_command = first_string_array_item(record, &["verification_commands"])
        .unwrap_or_else(|| regenerate_gap_ledger_command(root, options));
    let receipt_path = string_path(record, &["receipt_path"])
        .or_else(|| string_path(record, &["receipt", "path"]))
        .unwrap_or_else(|| first_pr_receipt_path(&options.receipts_dir, &gap_id));
    let ledger_receipt_command = string_path(record, &["receipt_command"]);
    let ledger_receipt_or_path_command = command_like_path(record, &["receipt_command_or_path"]);
    let (receipt_command, receipt_command_source) = if let Some(command) = ledger_receipt_command {
        (command, "gap_ledger.receipt_command".to_string())
    } else if let Some(command) = ledger_receipt_or_path_command {
        (command, "gap_ledger.receipt_command_or_path".to_string())
    } else {
        (
            outcome_command(
                WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
                WORKFLOW_AFTER_SNAPSHOT_ARTIFACT,
                Some(&receipt_path),
            ),
            "first_pr.default_outcome_command".to_string(),
        )
    };
    let repair_route_kind = string_from_sources(&[(repair_route, &["route_kind"])])
        .unwrap_or_else(|| "RepairRouteUnavailable".to_string());
    let target_file = string_from_sources(&[(repair_route, &["target_file"])]);
    let related_test = string_from_sources(&[(repair_route, &["related_test"])]);
    let suggested_assertion = string_from_sources(&[(repair_route, &["assertion_shape"])]);
    let missing_discriminator = if language.as_deref() == Some("python") {
        string_from_sources(&[
            (repair_route, &["missing_discriminator"]),
            (Some(record), &["missing_discriminator"]),
        ])
    } else {
        None
    }
    .unwrap_or_else(|| missing_discriminator_for_gap(&kind, suggested_assertion.as_deref()));
    TopGapSelection {
        gap_id: gap_id.clone(),
        canonical_gap_id: string_path(record, &["canonical_gap_id"]),
        language: language.clone(),
        language_status: language_status.clone(),
        kind: kind.clone(),
        source_artifact: options.gap_ledger.clone(),
        changed_behavior,
        current_evidence_strength: current_evidence_strength_for_gap(&kind, language.as_deref()),
        missing_discriminator,
        focused_proof_intent: focused_proof_intent(
            &repair_route_kind,
            target_file.as_deref(),
            suggested_assertion.as_deref(),
            related_test.as_deref(),
        ),
        why: why_for_gap(&kind, language.as_deref()),
        repair_route: repair_route_kind,
        target_file,
        related_test,
        suggested_assertion,
        anchor_file: string_from_sources(&[(anchor, &["file"])]),
        anchor_line: u64_from_sources(&[(anchor, &["line"])]),
        anchor_owner: string_from_sources(&[(anchor, &["owner"])]),
        dedupe_fingerprint: string_from_sources(&[(anchor, &["dedupe_fingerprint"])]),
        verify_command,
        receipt_command,
        receipt_path,
        receipt_command_source,
        receipt_state: string_path(record, &["receipt", "state"])
            .or_else(|| string_path(record, &["receipt", "movement"]))
            .map(|state| receipt_lifecycle_state(Some(&state))),
        static_limit_kind: string_path(record, &["static_limit_kind"]),
        static_limit_detail: string_path(record, &["static_limit_detail"]),
        agent_packet_command: format!(
            "ripr agent packet --root {} --gap-ledger {} --gap-id {} --json > {}",
            options.root, options.gap_ledger, gap_id, options.agent_packet
        ),
    }
}

fn current_evidence_strength_for_gap(kind: &str, language: Option<&str>) -> String {
    match (language, kind) {
        (Some("python"), "MissingBoundaryAssertion")
        | (Some("python"), "MissingValueAssertion")
        | (Some("python"), "MissingErrorDiscriminator") => {
            "Static evidence found related Python test context, but the current proof is weak because the discriminator is missing.".to_string()
        }
        (Some("python"), "MissingSideEffectObserver") => {
            "Static evidence found related Python test context, but the current proof does not observe the changed output or side effect.".to_string()
        }
        (_, "MissingBoundaryAssertion")
        | (_, "MissingValueAssertion")
        | (_, "MissingErrorDiscriminator") => {
            "Static evidence found related Rust test context, but the current proof is weak because the discriminator is missing.".to_string()
        }
        (_, "MissingOutputContract") => {
            "Static evidence found changed user-facing output, but no checked output or golden proof is attached.".to_string()
        }
        _ => {
            let language_label = match language {
                Some("python") => "preview Python",
                Some("rust") | None => "stable Rust",
                Some(other) => other,
            };
            format!(
                "The gap ledger marked this PR-local {language_label} gap as actionable and repairable; no runtime proof is claimed."
            )
        }
    }
}

fn missing_discriminator_for_gap(kind: &str, suggested_assertion: Option<&str>) -> String {
    match kind {
        "MissingBoundaryAssertion" => {
            "Equality-boundary assertion for the changed behavior.".to_string()
        }
        "MissingOutputContract" => {
            "Checked output or golden proof for the changed text.".to_string()
        }
        "MissingValueAssertion" => "Exact value assertion for the changed behavior.".to_string(),
        "MissingErrorDiscriminator" => {
            "Error discriminator assertion for the changed behavior.".to_string()
        }
        _ => suggested_assertion
            .map(|assertion| format!("Assertion or output proof shaped as `{assertion}`."))
            .unwrap_or_else(|| {
                "Specific assertion or output proof that observes the changed behavior.".to_string()
            }),
    }
}

fn focused_proof_intent(
    repair_route: &str,
    target_file: Option<&str>,
    suggested_assertion: Option<&str>,
    related_test: Option<&str>,
) -> String {
    let target = target_file
        .or(related_test)
        .map(|target| format!(" in `{target}`"))
        .unwrap_or_default();
    match repair_route {
        "AddOutputGolden" => suggested_assertion
            .map(|assertion| format!("Add or update the output proof{target} so `{assertion}`."))
            .unwrap_or_else(|| format!("Add or update the output proof{target}.")),
        "AddBoundaryAssertion" => suggested_assertion
            .map(|assertion| format!("Add a focused boundary assertion{target}: `{assertion}`."))
            .unwrap_or_else(|| format!("Add a focused boundary assertion{target}.")),
        "StrengthenExistingTest" => suggested_assertion
            .map(|assertion| {
                format!("Strengthen the existing related test{target}: `{assertion}`.")
            })
            .unwrap_or_else(|| format!("Strengthen the existing related test{target}.")),
        "AddValueAssertion" => suggested_assertion
            .map(|assertion| format!("Add a focused value assertion{target}: `{assertion}`."))
            .unwrap_or_else(|| format!("Add a focused value assertion{target}.")),
        "AddErrorDiscriminator" => suggested_assertion
            .map(|assertion| format!("Add a focused error-path assertion{target}: `{assertion}`."))
            .unwrap_or_else(|| format!("Add a focused error-path assertion{target}.")),
        _ => suggested_assertion
            .map(|assertion| format!("Add the focused proof{target}: `{assertion}`."))
            .unwrap_or_else(|| format!("Add the focused proof{target}.")),
    }
}

fn why_for_gap(kind: &str, language: Option<&str>) -> String {
    match (language, kind) {
        (Some("python"), "MissingBoundaryAssertion") => {
            "A related Python test reaches this change, but no boundary discriminator was found for the changed behavior.".to_string()
        }
        (Some("python"), "MissingValueAssertion") => {
            "A related Python test reaches this change, but no exact value assertion was found for the changed behavior.".to_string()
        }
        (Some("python"), "MissingErrorDiscriminator") => {
            "A related Python test reaches this error path, but no error discriminator was found for the changed behavior.".to_string()
        }
        (Some("python"), "MissingSideEffectObserver") => {
            "A related Python test reaches this change, but no output or side-effect observer was found for the changed behavior.".to_string()
        }
        (_, "MissingBoundaryAssertion") => {
            "A related Rust test reaches this change, but no equality-boundary assertion was found for the changed behavior.".to_string()
        }
        (_, "MissingOutputContract") => {
            "User-facing output changed, but the gap ledger did not find checked output or golden evidence for the changed text.".to_string()
        }
        (_, "MissingValueAssertion") => {
            "A related Rust test reaches this change, but no exact value assertion was found for the changed behavior.".to_string()
        }
        (_, "MissingErrorDiscriminator") => {
            "A related Rust test reaches this error path, but no error discriminator was found for the changed behavior.".to_string()
        }
        _ => {
            let language_label = match language {
                Some("python") => "preview Python",
                Some("rust") | None => "stable Rust",
                Some(other) => other,
            };
            format!(
                "The gap ledger marked this PR-local {language_label} gap as repairable and policy-targeted."
            )
        }
    }
}

fn first_pr_receipt_path(receipts_dir: &str, gap_id: &str) -> String {
    let directory = receipts_dir.trim_end_matches(['/', '\\']);
    let file_name = format!("{}.targeted-test-outcome.json", slugify_gap_id(gap_id));
    if directory.is_empty() {
        file_name
    } else {
        format!("{directory}/{file_name}")
    }
}

fn slugify_gap_id(gap_id: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in gap_id.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "gap".to_string()
    } else {
        trimmed.to_string()
    }
}

fn command_like_path(value: &Value, path: &[&str]) -> Option<String> {
    string_path(value, path).filter(|text| text.trim_start().starts_with("ripr "))
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

fn regenerate_gap_ledger_command(root: &Path, options: &FirstPrOptions) -> String {
    if uses_python_check_output_gap_ledger(root) {
        return regenerate_python_gap_ledger_command(options);
    }
    regenerate_repo_exposure_gap_ledger_command(&options.gap_ledger)
}

fn uses_python_check_output_gap_ledger(root: &Path) -> bool {
    detect_python_project(root) && !root.join("Cargo.toml").is_file()
}

fn regenerate_repo_exposure_gap_ledger_command(out: &str) -> String {
    format!(
        "ripr reports gap-ledger --repo-exposure target/ripr/reports/repo-exposure.json --out {out} --out-md {}",
        with_extension(out, "md")
    )
}

fn regenerate_python_gap_ledger_command(options: &FirstPrOptions) -> String {
    let root = shell_arg(&options.root);
    let base = shell_arg(&options.base);
    let check_output = shell_arg(
        options
            .check_output
            .as_deref()
            .unwrap_or(DEFAULT_CHECK_OUTPUT),
    );
    let out = shell_arg(&options.gap_ledger);
    let out_md = shell_arg(&with_extension(&options.gap_ledger, "md"));
    if options.check_output.is_some() {
        format!(
            "ripr reports gap-ledger --check-output {check_output} --root {root} --out {out} --out-md {out_md}"
        )
    } else {
        format!(
            "ripr check --root {root} --base {base} --json > {check_output} && ripr reports gap-ledger --check-output {check_output} --root {root} --out {out} --out-md {out_md}"
        )
    }
}

fn regenerate_repo_exposure_command(root: &str) -> String {
    check_repo_exposure_command(root, "instant", DEFAULT_REPO_EXPOSURE)
}

fn git_worktree_available(root: &Path) -> Result<bool, String> {
    git_success(root, &["rev-parse", "--is-inside-work-tree"])
}

fn git_rev_exists(root: &Path, rev: &str) -> Result<bool, String> {
    let commit = format!("{rev}^{{commit}}");
    git_success(root, &["rev-parse", "--verify", "--quiet", &commit])
}

fn git_diff_range_valid(root: &Path, base: &str, head: &str) -> Result<(), String> {
    let range = format!("{base}...{head}");
    let output = Command::new("git")
        .arg("diff")
        .arg("--name-only")
        .arg("--no-ext-diff")
        .arg(&range)
        .current_dir(root)
        .output()
        .map_err(|err| format!("failed to run git diff: {err}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        Err("git diff failed without stderr".to_string())
    } else {
        Err(stderr)
    }
}

fn git_success(root: &Path, args: &[&str]) -> Result<bool, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|err| format!("failed to run git: {err}"))?;
    Ok(output.status.success())
}

fn fetch_base_command(options: &FirstPrOptions) -> String {
    if let Some(branch) = options.base.strip_prefix("origin/") {
        format!(
            "git -C {} fetch origin {}",
            shell_arg(&options.root),
            shell_arg(branch)
        )
    } else {
        format!("git -C {} fetch --all --prune", shell_arg(&options.root))
    }
}

fn verify_ref_command(options: &FirstPrOptions, rev: &str) -> String {
    format!(
        "git -C {} rev-parse --verify {}",
        shell_arg(&options.root),
        shell_arg(&format!("{rev}^{{commit}}"))
    )
}

fn diff_range_command(options: &FirstPrOptions) -> String {
    format!(
        "git -C {} diff --name-only --no-ext-diff {}",
        shell_arg(&options.root),
        shell_arg(&format!("{}...{}", options.base, options.head))
    )
}

fn doctor_command(root: &str) -> String {
    format!("ripr doctor --root {}", shell_arg(root))
}

fn with_extension(path: &str, extension: &str) -> String {
    let mut path = PathBuf::from(path);
    path.set_extension(extension);
    path.display().to_string().replace('\\', "/")
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
            "--check-output".to_string(),
            "check.json".to_string(),
            "--gap-ledger".to_string(),
            "gap.json".to_string(),
            "--out-dir".to_string(),
            "out".to_string(),
            "--check".to_string(),
        ])?;
        assert_eq!(parsed.root, "repo");
        assert_eq!(parsed.base, "origin/main");
        assert_eq!(parsed.head, "HEAD");
        assert_eq!(parsed.check_output.as_deref(), Some("check.json"));
        assert_eq!(parsed.gap_ledger, "gap.json");
        assert_eq!(parsed.out_dir, "out");
        assert!(parsed.check);
        assert!(parsed.preflight);
        assert!(!FirstPrOptions::default().preflight);
        assert_eq!(
            parse_options(&["--gap-ledger".to_string(), "".to_string()]),
            Err("first-pr --gap-ledger requires a non-empty value".to_string())
        );
        Ok(())
    }

    #[test]
    fn first_pr_help_pins_start_here_language() {
        let help = first_pr_help_text();
        assert!(help.contains("ripr first-pr|start-here"));
        assert!(help.contains("--check-output <path>"));
        assert!(help.contains("Start-here language:"));
        assert!(help.contains("safe next action"));
        assert!(
            help.contains("missing artifact / stale evidence / wrong root / malformed artifact")
        );
        assert!(help.contains("no actionable gap"));
        assert!(help.contains("preview-limited evidence"));
        assert!(help.contains("verify command / receipt command / receipt path"));
    }

    #[test]
    fn markdown_command_rendering_preserves_embedded_code_spans() {
        assert_eq!(markdown_code_or_text("git status"), "`git status`");
        assert_eq!(
            markdown_code_or_text(
                "Choose a head with changes or rerun after committing PR work: `ripr first-pr --root . --base origin/main --head HEAD`."
            ),
            "Choose a head with changes or rerun after committing PR work: `ripr first-pr --root . --base origin/main --head HEAD`."
        );
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
        assert_eq!(packet["selected"]["output_state"], "actionable_gap");
        assert_eq!(
            packet["selected"]["gap_id"],
            "gap:pr:pricing:threshold-boundary"
        );
        assert_eq!(
            packet["selected"]["repair"]["route"],
            "AddBoundaryAssertion"
        );
        assert_eq!(
            packet["selected"]["current_evidence_strength"],
            "Static evidence found related Rust test context, but the current proof is weak because the discriminator is missing."
        );
        assert_eq!(
            packet["selected"]["missing_discriminator"],
            "Equality-boundary assertion for the changed behavior."
        );
        assert_eq!(
            packet["selected"]["focused_proof_intent"],
            "Add a focused boundary assertion in `tests/pricing.rs`: `assert_eq!(discount(100, 100), 90)`."
        );
        assert_eq!(
            packet["selected"]["static_evidence_boundary"],
            STATIC_EVIDENCE_BOUNDARY
        );
        assert!(
            packet["commands"]["receipt"]
                .as_str()
                .is_some_and(|command| command.contains("ripr outcome --before"))
        );
        assert!(
            packet["commands"]["agent_packet"].as_str().is_some_and(
                |command| command.contains("--gap-id gap:pr:pricing:threshold-boundary")
            )
        );
        let summary = start_here_cli_summary(
            &packet,
            Path::new("target/ripr/reports/start-here.json"),
            Path::new("target/ripr/reports/start-here.md"),
        );
        assert!(summary.contains("Top actionable gap: missing boundary assertion"));
        assert!(summary.contains("Changed behavior: `amount >= threshold`"));
        assert!(summary.contains(
            "Current evidence strength: Static evidence found related Rust test context"
        ));
        assert!(summary.contains(
            "Missing discriminator: Equality-boundary assertion for the changed behavior."
        ));
        assert!(summary.contains(
            "Focused proof intent: Add a focused boundary assertion in `tests/pricing.rs`"
        ));
        assert!(summary.contains(
            "Why this matters: A related Rust test reaches this change, but no equality-boundary assertion was found for the changed behavior."
        ));
        let markdown = render_start_here_markdown(&packet);
        assert!(markdown.contains(
            "- Why this matters: A related Rust test reaches this change, but no equality-boundary assertion was found for the changed behavior."
        ));
        cleanup(&repo)
    }

    #[test]
    fn top_gap_contract_requires_changed_behavior() {
        let selected = json!({
            "state": "top_gap",
            "output_state": "actionable_gap",
            "kind": "MissingBoundaryAssertion",
            "why": "A related Rust test reaches this change.",
            "current_evidence_strength": "Static evidence found related Rust test context.",
            "missing_discriminator": "Equality-boundary assertion.",
            "focused_proof_intent": "Add one focused boundary assertion.",
            "verify_command": "cargo xtask fixtures boundary_gap",
            "receipt_path": "target/ripr/receipts/gap.json",
            "static_evidence_boundary": STATIC_EVIDENCE_BOUNDARY,
        });
        let mut violations = Vec::new();
        validate_selected_state("actionable", &selected, &mut violations);

        assert_eq!(
            violations,
            vec!["selected top_gap must name changed behavior"]
        );
    }

    #[test]
    fn missing_repo_exposure_blocks_before_gap_ledger() -> Result<(), String> {
        let repo = temp_repo("first-pr-missing-repo-exposure")?;
        let options = FirstPrOptions::default();
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "missing_artifact");
        assert_eq!(packet["selected"]["output_state"], "missing_artifacts");
        assert_eq!(packet["selected"]["artifact"]["id"], "repo_exposure");
        assert_eq!(
            packet["selected"]["artifact"]["path"],
            DEFAULT_REPO_EXPOSURE
        );
        assert!(
            packet["selected"]["regeneration_command"]
                .as_str()
                .is_some_and(|command| command
                    == "ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/repo-exposure.json")
        );
        let summary = start_here_cli_summary(
            &packet,
            Path::new("target/ripr/reports/start-here.json"),
            Path::new("target/ripr/reports/start-here.md"),
        );
        assert!(summary.contains(
            "Missing artifact: Repo exposure report at `target/ripr/reports/repo-exposure.json`"
        ));
        assert!(summary.contains("Regeneration command: `ripr check --root . --mode instant"));
        check_first_pr(&repo, &options)?;
        cleanup(&repo)
    }

    #[test]
    fn missing_repo_exposure_uses_bounded_latency_report_when_available() -> Result<(), String> {
        let repo = temp_repo("first-pr-missing-repo-exposure-latency")?;
        fs::create_dir_all(repo.join("xtask/src"))
            .map_err(|err| format!("mkdir xtask src: {err}"))?;
        fs::write(
            repo.join("xtask/src/command.rs"),
            "\"repo-exposure-latency-report\"",
        )
        .map_err(|err| format!("write xtask command catalog: {err}"))?;
        let options = FirstPrOptions::default();
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "blocked_artifact");
        assert_eq!(packet["selected"]["output_state"], "missing_artifacts");
        assert!(
            packet["selected"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("Repo exposure report is missing"))
        );
        assert_eq!(
            packet["selected"]["next_command"],
            REPO_EXPOSURE_LATENCY_REPORT_COMMAND
        );
        let summary = start_here_cli_summary(
            &packet,
            Path::new("target/ripr/reports/start-here.json"),
            Path::new("target/ripr/reports/start-here.md"),
        );
        assert!(summary.contains("Recovery reason: Repo exposure report is missing"));
        assert!(summary.contains("Next command: `cargo xtask repo-exposure-latency-report`"));
        check_first_pr(&repo, &options)?;
        cleanup(&repo)
    }

    #[test]
    fn missing_repo_exposure_ignores_latency_report_when_xtask_command_is_unavailable()
    -> Result<(), String> {
        let repo = temp_repo("first-pr-existing-latency-without-xtask")?;
        write_json(
            &repo.join(DEFAULT_REPO_EXPOSURE_LATENCY_JSON),
            json!({
                "schema_version": "0.1",
                "tool": "ripr",
                "report": "repo-exposure-latency",
                "status": "warn",
                "runs": [
                    {
                        "format": "repo-exposure-json",
                        "status": "timeout",
                        "duration_ms": 30000
                    }
                ]
            }),
        )?;
        let options = FirstPrOptions::default();
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "missing_artifact");
        assert_eq!(packet["selected"]["output_state"], "missing_artifacts");
        assert_eq!(packet["selected"]["artifact"]["id"], "repo_exposure");
        assert!(
            packet["selected"]["regeneration_command"]
                .as_str()
                .is_some_and(|command| command
                    == "ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/repo-exposure.json")
        );
        cleanup(&repo)
    }

    #[test]
    fn missing_repo_exposure_uses_existing_latency_report_before_rerun() -> Result<(), String> {
        let repo = temp_repo("first-pr-existing-latency-timeout")?;
        fs::create_dir_all(repo.join("xtask/src"))
            .map_err(|err| format!("mkdir xtask src: {err}"))?;
        fs::write(
            repo.join("xtask/src/command.rs"),
            "\"repo-exposure-latency-report\"",
        )
        .map_err(|err| format!("write xtask command catalog: {err}"))?;
        write_json(
            &repo.join(DEFAULT_REPO_EXPOSURE_LATENCY_JSON),
            json!({
                "schema_version": "0.1",
                "tool": "ripr",
                "report": "repo-exposure-latency",
                "status": "warn",
                "runs": [
                    {
                        "format": "repo-exposure-json",
                        "status": "timeout",
                        "duration_ms": 30000,
                        "trace": [
                            {
                                "phase": "evidence_for_seams",
                                "status": "start_seams_40692",
                                "duration_ms": 0
                            },
                            {
                                "phase": "evidence_for_seams_progress",
                                "status": "processed_2500_of_40692",
                                "duration_ms": 24056
                            }
                        ]
                    }
                ]
            }),
        )?;
        let options = FirstPrOptions::default();
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "timeout");
        assert_eq!(packet["selected"]["output_state"], "timeout_partial");
        let message = packet["selected"]["message"]
            .as_str()
            .ok_or_else(|| "selected message missing".to_string())?;
        assert!(message.contains(DEFAULT_REPO_EXPOSURE_LATENCY_REPORT));
        assert!(message.contains("repo-exposure-json"));
        assert!(message.contains("timeout"));
        assert!(message.contains("evidence_for_seams_progress"));
        assert!(message.contains("processed_2500_of_40692"));
        let summary = start_here_cli_summary(
            &packet,
            Path::new("target/ripr/reports/start-here.json"),
            Path::new("target/ripr/reports/start-here.md"),
        );
        assert!(summary.contains("Recovery reason: Repo exposure report is missing"));
        assert!(summary.contains("evidence_for_seams_progress"));
        check_first_pr(&repo, &options)?;
        cleanup(&repo)
    }

    #[test]
    fn missing_repo_exposure_keeps_failed_latency_report_blocked_not_timeout() -> Result<(), String>
    {
        let repo = temp_repo("first-pr-existing-latency-fail")?;
        fs::create_dir_all(repo.join("xtask/src"))
            .map_err(|err| format!("mkdir xtask src: {err}"))?;
        fs::write(
            repo.join("xtask/src/command.rs"),
            "\"repo-exposure-latency-report\"",
        )
        .map_err(|err| format!("write xtask command catalog: {err}"))?;
        write_json(
            &repo.join(DEFAULT_REPO_EXPOSURE_LATENCY_JSON),
            json!({
                "schema_version": "0.1",
                "tool": "ripr",
                "report": "repo-exposure-latency",
                "status": "fail",
                "runs": [
                    {
                        "format": "repo-exposure-json",
                        "status": "fail",
                        "duration_ms": 1200,
                        "exit_code": 101,
                        "trace": []
                    }
                ]
            }),
        )?;
        let options = FirstPrOptions::default();
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "blocked_artifact");
        assert_eq!(packet["selected"]["output_state"], "missing_artifacts");
        let message = packet["selected"]["message"]
            .as_str()
            .ok_or_else(|| "selected message missing".to_string())?;
        assert!(message.contains(DEFAULT_REPO_EXPOSURE_LATENCY_JSON));
        assert!(message.contains(DEFAULT_REPO_EXPOSURE_LATENCY_REPORT));
        assert!(message.contains("fail"));
        check_first_pr(&repo, &options)?;
        cleanup(&repo)
    }

    #[test]
    fn missing_repo_exposure_ignores_wrong_identity_latency_report() -> Result<(), String> {
        let repo = temp_repo("first-pr-existing-latency-wrong-identity")?;
        fs::create_dir_all(repo.join("xtask/src"))
            .map_err(|err| format!("mkdir xtask src: {err}"))?;
        fs::write(
            repo.join("xtask/src/command.rs"),
            "\"repo-exposure-latency-report\"",
        )
        .map_err(|err| format!("write xtask command catalog: {err}"))?;
        write_json(
            &repo.join(DEFAULT_REPO_EXPOSURE_LATENCY_JSON),
            json!({
                "schema_version": "0.1",
                "tool": "other-tool",
                "report": "other-report",
                "runs": [
                    {
                        "format": "repo-exposure-json",
                        "status": "timeout",
                        "duration_ms": 30000,
                        "trace": [
                            {
                                "phase": "untrusted_phase",
                                "status": "untrusted_status",
                                "duration_ms": 1
                            }
                        ]
                    }
                ]
            }),
        )?;
        let options = FirstPrOptions::default();
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "blocked_artifact");
        assert_eq!(packet["selected"]["output_state"], "missing_artifacts");
        let message = packet["selected"]["message"]
            .as_str()
            .ok_or_else(|| "selected message missing".to_string())?;
        assert!(message.contains("run the bounded repo-exposure latency report"));
        assert!(!message.contains("untrusted_phase"));
        assert!(!message.contains("untrusted_status"));
        check_first_pr(&repo, &options)?;
        cleanup(&repo)
    }

    #[test]
    fn missing_repo_exposure_roots_bounded_latency_report_for_custom_root() -> Result<(), String> {
        let invocation_root = temp_repo("first-pr-invocation-root")?;
        let repo = temp_repo("first-pr custom-root latency")?;
        fs::create_dir_all(repo.join("xtask/src"))
            .map_err(|err| format!("mkdir xtask src: {err}"))?;
        fs::write(
            repo.join("xtask/src/command.rs"),
            "\"repo-exposure-latency-report\"",
        )
        .map_err(|err| format!("write xtask command catalog: {err}"))?;
        let root_arg = display_path(&repo);
        let options = FirstPrOptions {
            root: root_arg,
            ..FirstPrOptions::default()
        };
        write_first_pr(&invocation_root, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        let manifest_path = display_path(&repo.join("Cargo.toml"));
        assert_eq!(
            packet["selected"]["next_command"],
            format!(
                "cargo run --manifest-path {} -p xtask -- repo-exposure-latency-report",
                shell_arg(&manifest_path)
            )
        );
        check_first_pr(&invocation_root, &options)?;
        cleanup(&repo)?;
        cleanup(&invocation_root)
    }

    #[test]
    fn missing_gap_ledger_writes_recovery_packet_after_repo_exposure_exists() -> Result<(), String>
    {
        let repo = temp_repo("first-pr-missing-gap-ledger")?;
        fs::create_dir_all(repo.join("target/ripr/reports"))
            .map_err(|err| format!("mkdir reports dir: {err}"))?;
        fs::write(repo.join(DEFAULT_REPO_EXPOSURE), "{}")
            .map_err(|err| format!("write repo exposure: {err}"))?;
        let options = FirstPrOptions::default();
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "missing_artifact");
        assert_eq!(packet["selected"]["output_state"], "missing_artifacts");
        assert_eq!(packet["selected"]["artifact"]["id"], "gap_ledger");
        assert!(
            packet["selected"]["regeneration_command"]
                .as_str()
                .is_some_and(|command| command.contains("ripr reports gap-ledger"))
        );
        let summary = start_here_cli_summary(
            &packet,
            Path::new("target/ripr/reports/start-here.json"),
            Path::new("target/ripr/reports/start-here.md"),
        );
        assert!(summary.contains(
            "Missing artifact: Gap decision ledger at `target/ripr/reports/gap-decision-ledger.json`"
        ));
        assert!(summary.contains("Regeneration command: `ripr reports gap-ledger"));
        check_first_pr(&repo, &options)?;
        cleanup(&repo)
    }

    #[test]
    fn missing_python_gap_ledger_uses_check_output_bridge() -> Result<(), String> {
        let repo = temp_python_repo("first-pr-python-missing-gap-ledger")?;
        let options = FirstPrOptions::default();
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;

        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "missing_artifact");
        assert_eq!(packet["selected"]["output_state"], "missing_artifacts");
        assert_eq!(packet["selected"]["artifact"]["id"], "gap_ledger");
        assert_eq!(packet["selected"]["artifact"]["path"], DEFAULT_GAP_LEDGER);
        let command = packet["selected"]["regeneration_command"]
            .as_str()
            .ok_or_else(|| "selected regeneration command missing".to_string())?;
        assert!(command.contains(
            "ripr check --root . --base origin/main --json > target/ripr/reports/check.json"
        ));
        assert!(command.contains(
            "ripr reports gap-ledger --check-output target/ripr/reports/check.json --root . --out target/ripr/reports/gap-decision-ledger.json --out-md target/ripr/reports/gap-decision-ledger.md"
        ));
        assert!(!command.contains("--repo-exposure"));
        assert_eq!(packet["commands"]["regenerate_gap_ledger"], command);
        assert_eq!(packet["artifacts"][0]["regeneration_command"], command);
        check_first_pr(&repo, &options)?;
        cleanup(&repo)
    }

    #[test]
    fn missing_root_writes_recovery_packet_without_creating_root() -> Result<(), String> {
        let repo = temp_repo("first-pr-missing-root")?;
        let options = FirstPrOptions {
            root: "missing-workspace".to_string(),
            ..FirstPrOptions::default()
        };
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "wrong_root");
        assert_eq!(packet["selected"]["output_state"], "wrong_root");
        assert!(
            packet["selected"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("is not a directory"))
        );
        assert_eq!(
            packet["selected"]["next_command"],
            "ripr doctor --root missing-workspace"
        );
        assert!(
            !repo.join("missing-workspace").exists(),
            "first-pr must not create a typo root while writing a recovery packet"
        );
        check_first_pr(&repo, &options)?;
        cleanup(&repo)
    }

    #[test]
    fn non_cargo_root_writes_workspace_recovery_packet_to_invocation_root() -> Result<(), String> {
        let repo = temp_repo("first-pr-not-cargo-root")?;
        let non_workspace = repo.join("not-workspace");
        fs::create_dir_all(&non_workspace)
            .map_err(|err| format!("mkdir {}: {err}", non_workspace.display()))?;
        let options = FirstPrOptions {
            root: "not-workspace".to_string(),
            ..FirstPrOptions::default()
        };
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "wrong_root");
        assert_eq!(packet["selected"]["output_state"], "wrong_root");
        assert!(
            packet["selected"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("Cargo.toml is missing"))
        );
        assert_eq!(
            packet["selected"]["next_command"],
            "ripr doctor --root not-workspace"
        );
        assert!(
            !non_workspace.join(DEFAULT_OUT_DIR).exists(),
            "first-pr must not write recovery artifacts under a non-Cargo root"
        );
        check_first_pr(&repo, &options)?;
        cleanup(&repo)
    }

    #[test]
    fn non_git_root_writes_recovery_packet() -> Result<(), String> {
        let repo = temp_cargo_root_outside_repo("first-pr-not-git")?;
        write_json(&repo.join(DEFAULT_GAP_LEDGER), ledger_with_repairable_gap())?;
        let options = FirstPrOptions::default();
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "blocked_artifact");
        assert!(
            packet["selected"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("not a git worktree"))
        );
        assert_eq!(packet["selected"]["next_command"], "ripr doctor --root .");
        cleanup(&repo)
    }

    #[test]
    fn missing_git_base_writes_recovery_packet() -> Result<(), String> {
        let repo = temp_repo("first-pr-missing-base")?;
        write_json(&repo.join(DEFAULT_GAP_LEDGER), ledger_with_repairable_gap())?;
        let options = FirstPrOptions {
            base: "origin/missing-base".to_string(),
            ..FirstPrOptions::default()
        };
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "blocked_artifact");
        assert!(
            packet["selected"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("origin/missing-base"))
        );
        assert_eq!(
            packet["selected"]["next_command"],
            "git -C . fetch origin missing-base"
        );
        cleanup(&repo)
    }

    #[test]
    fn check_first_pr_rejects_stale_git_preflight_packet() -> Result<(), String> {
        let repo = temp_repo("first-pr-check-stale-git-preflight")?;
        write_json(&repo.join(DEFAULT_GAP_LEDGER), ledger_with_repairable_gap())?;
        let options = FirstPrOptions::default();
        write_first_pr(&repo, &options)?;
        run_git_setup(&repo, &["update-ref", "-d", "refs/remotes/origin/main"])?;
        let err = match check_first_pr(&repo, &options) {
            Ok(()) => return Err("check mode accepted stale git preflight state".to_string()),
            Err(err) => err,
        };
        assert!(
            err.contains("stale for current root/git preflight"),
            "unexpected check error: {err}"
        );
        cleanup(&repo)
    }

    #[test]
    fn missing_plain_git_base_writes_fetch_all_recovery_packet() -> Result<(), String> {
        let repo = temp_repo("first-pr-missing-plain-base")?;
        write_json(&repo.join(DEFAULT_GAP_LEDGER), ledger_with_repairable_gap())?;
        let options = FirstPrOptions {
            base: "missing-base".to_string(),
            ..FirstPrOptions::default()
        };
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "blocked_artifact");
        assert!(
            packet["selected"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("missing-base"))
        );
        assert_eq!(
            packet["selected"]["next_command"],
            "git -C . fetch --all --prune"
        );
        cleanup(&repo)
    }

    #[test]
    fn missing_git_head_writes_recovery_packet() -> Result<(), String> {
        let repo = temp_repo("first-pr-missing-head")?;
        write_json(&repo.join(DEFAULT_GAP_LEDGER), ledger_with_repairable_gap())?;
        let options = FirstPrOptions {
            head: "missing-head".to_string(),
            ..FirstPrOptions::default()
        };
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "blocked_artifact");
        assert!(
            packet["selected"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("missing-head"))
        );
        assert_eq!(
            packet["selected"]["next_command"],
            "git -C . rev-parse --verify \"missing-head^{commit}\""
        );
        cleanup(&repo)
    }

    #[test]
    fn unrelated_git_range_writes_recovery_packet() -> Result<(), String> {
        let repo = temp_repo("first-pr-unrelated-range")?;
        write_json(&repo.join(DEFAULT_GAP_LEDGER), ledger_with_repairable_gap())?;
        run_git_setup(&repo, &["checkout", "--orphan", "unrelated"])?;
        run_git_setup(&repo, &["commit", "--allow-empty", "-m", "unrelated"])?;
        let options = FirstPrOptions {
            head: "unrelated".to_string(),
            ..FirstPrOptions::default()
        };
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "blocked_artifact");
        assert!(
            packet["selected"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("origin/main...unrelated"))
        );
        assert_eq!(
            packet["selected"]["next_command"],
            "git -C . diff --name-only --no-ext-diff origin/main...unrelated"
        );
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
        assert_eq!(packet["selected"]["output_state"], "malformed_artifact");
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
        assert_eq!(packet["selected"]["output_state"], "stale_evidence");
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
        assert_eq!(packet["selected"]["output_state"], "wrong_root");
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
        assert_eq!(packet["selected"]["output_state"], "timeout_partial");
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
        assert_eq!(packet["selected"]["output_state"], "missing_artifacts");
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
        assert_eq!(packet["selected"]["output_state"], "clean");
        assert_eq!(packet["selected"]["records_total"], 0);
        let summary = start_here_cli_summary(
            &packet,
            Path::new("target/ripr/reports/start-here.json"),
            Path::new("target/ripr/reports/start-here.md"),
        );
        assert!(
            summary
                .contains("Reason: The PR diff is empty, so no repairable Rust gap was selected.")
        );
        assert!(summary.contains("Verify command: `not_applicable`"));
        assert!(markdown.contains("## Start Here"));
        assert!(markdown.contains("- State: `empty_diff`"));
        assert!(markdown.contains("- Safe next action: stop on no-action"));
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
        assert_eq!(packet["selected"]["output_state"], "no_actionable_gap");
        cleanup(&repo)
    }

    #[test]
    fn python_preview_gap_ledger_is_selected_for_start_here() -> Result<(), String> {
        let repo = temp_python_repo("first-pr-python-preview")?;
        fs::create_dir_all(repo.join("app")).map_err(|err| format!("mkdir app: {err}"))?;
        fs::write(
            repo.join("app/pricing.py"),
            "def calculate_discount(amount, threshold):\n    return amount >= threshold\n",
        )
        .map_err(|err| format!("write app/pricing.py: {err}"))?;
        run_git_setup(&repo, &["add", "app/pricing.py"])?;
        run_git_setup(&repo, &["commit", "-m", "change pricing"])?;
        write_json(
            &repo.join(DEFAULT_GAP_LEDGER),
            ledger_with_python_repairable_gap(),
        )?;

        let options = FirstPrOptions {
            preflight: true,
            ..FirstPrOptions::default()
        };
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "actionable");
        assert_eq!(packet["selected"]["state"], "top_gap");
        assert_eq!(
            packet["selected"]["output_state"],
            START_HERE_PREVIEW_LIMITED
        );
        assert_eq!(packet["selected"]["language"], "python");
        assert_eq!(packet["selected"]["language_status"], "preview");
        assert_eq!(
            packet["selected"]["missing_discriminator"],
            "amount == threshold"
        );
        assert_eq!(
            packet["selected"]["verify_command"],
            "pytest tests/test_pricing.py::test_calculate_discount_smoke"
        );
        assert_eq!(
            packet["selected"]["receipt_command_source"],
            "gap_ledger.receipt_command"
        );
        let python_project = preflight_check(&packet, "python_project")?;
        assert_eq!(python_project["status"], "ok");
        let summary = start_here_cli_summary(
            &packet,
            Path::new("target/ripr/reports/start-here.json"),
            Path::new("target/ripr/reports/start-here.md"),
        );
        assert!(summary.contains("Safe next action: repair one named gap `gap:python:app/pricing.py:calculate_discount:predicate_boundary:amount>=threshold`"));
        assert!(summary.contains(
            "Verify command: `pytest tests/test_pricing.py::test_calculate_discount_smoke`"
        ));
        check_first_pr(&repo, &options)?;
        cleanup(&repo)
    }

    #[test]
    fn python_check_output_materializes_gap_ledger_for_start_here() -> Result<(), String> {
        let repo = temp_python_repo("first-pr-python-check-output")?;
        fs::create_dir_all(repo.join("app")).map_err(|err| format!("mkdir app: {err}"))?;
        fs::write(
            repo.join("app/pricing.py"),
            "def calculate_discount(amount, threshold):\n    return amount >= threshold\n",
        )
        .map_err(|err| format!("write app/pricing.py: {err}"))?;
        run_git_setup(&repo, &["add", "app/pricing.py"])?;
        run_git_setup(&repo, &["commit", "-m", "change pricing"])?;
        write_json(
            &repo.join(DEFAULT_CHECK_OUTPUT),
            check_output_with_python_repair_card(),
        )?;

        let options = FirstPrOptions {
            check_output: Some(DEFAULT_CHECK_OUTPUT.to_string()),
            preflight: true,
            ..FirstPrOptions::default()
        };
        write_first_pr(&repo, &options)?;

        let ledger = read_packet(&repo.join(DEFAULT_GAP_LEDGER))?;
        assert_eq!(ledger["inputs"]["source_kind"], "check_output");
        assert_eq!(ledger["inputs"]["records"], DEFAULT_CHECK_OUTPUT);
        assert_eq!(
            ledger["records"][0]["receipt_command"],
            "ripr outcome --before target/ripr/reports/check.json --after target/ripr/reports/after-check.json --format json --out target/ripr/receipts/gap-python-app-pricing.py-calculate_discount-predicate_boundary-amount-threshold.json"
        );

        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "actionable");
        assert_eq!(packet["inputs"]["check_output"], DEFAULT_CHECK_OUTPUT);
        assert_eq!(packet["selected"]["source_artifact"], DEFAULT_GAP_LEDGER);
        assert_eq!(
            packet["selected"]["receipt_command_source"],
            "gap_ledger.receipt_command"
        );
        assert_eq!(
            packet["selected"]["receipt_command"],
            "ripr outcome --before target/ripr/reports/check.json --after target/ripr/reports/after-check.json --format json --out target/ripr/receipts/gap-python-app-pricing.py-calculate_discount-predicate_boundary-amount-threshold.json"
        );
        assert_eq!(
            packet["selected"]["agent_packet_command"],
            "ripr agent packet --root . --gap-ledger target/ripr/reports/gap-decision-ledger.json --gap-id gap:pr:gap:python:app/pricing.py:calculate_discount:predicate_boundary:amount>=threshold --json > target/ripr/workflow/agent-packet.json"
        );
        assert!(repo.join(DEFAULT_GAP_LEDGER).is_file());
        assert!(
            repo.join(with_extension(DEFAULT_GAP_LEDGER, "md"))
                .is_file()
        );
        check_first_pr(&repo, &options)?;
        cleanup(&repo)
    }

    #[test]
    fn preflight_reports_missing_git_base_and_config_defaults() -> Result<(), String> {
        let repo = temp_repo("first-pr-preflight-missing-base")?;
        fs::write(repo.join("Cargo.toml"), "[workspace]\n")
            .map_err(|err| format!("write Cargo.toml: {err}"))?;
        run_git_ok(&repo, &["init"])?;
        let options = FirstPrOptions {
            base: "origin/missing-base".to_string(),
            preflight: true,
            ..FirstPrOptions::default()
        };
        let packet = render_start_here_packet(&repo, &options);
        assert_eq!(packet["preflight"]["status"], "needs_attention");
        assert_eq!(packet["preflight"]["mode"], "write");
        let base = preflight_check(&packet, "git_base")?;
        assert_eq!(base["status"], "needs_attention");
        assert!(
            base["next_command"]
                .as_str()
                .is_some_and(|command| command.contains("git fetch origin missing-base"))
        );
        let config = preflight_check(&packet, "ripr_config")?;
        assert_eq!(config["status"], "defaulted");
        assert!(
            config["message"]
                .as_str()
                .is_some_and(|message| message.contains("built-in advisory defaults"))
        );
        cleanup(&repo)
    }

    #[test]
    fn preflight_reports_output_path_that_is_not_a_directory() -> Result<(), String> {
        let repo = temp_repo("first-pr-preflight-output-file")?;
        fs::write(repo.join("Cargo.toml"), "[workspace]\n")
            .map_err(|err| format!("write Cargo.toml: {err}"))?;
        fs::write(repo.join("start-here.out"), "not a directory")
            .map_err(|err| format!("write output placeholder: {err}"))?;
        let options = FirstPrOptions {
            out_dir: "start-here.out".to_string(),
            preflight: true,
            ..FirstPrOptions::default()
        };
        let packet = render_start_here_packet(&repo, &options);
        let output = preflight_check(&packet, "output_dir")?;
        assert_eq!(output["status"], "needs_attention");
        assert!(
            output["message"]
                .as_str()
                .is_some_and(|message| message.contains("is not a directory"))
        );
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
                    "changed_behavior": "amount >= threshold",
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

    fn ledger_with_python_repairable_gap() -> Value {
        json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "kind": "gap_decision_ledger",
            "status": "advisory",
            "records": [
                {
                    "gap_id": "gap:pr:gap:python:app/pricing.py:calculate_discount:predicate_boundary:amount>=threshold",
                    "canonical_gap_id": "gap:python:app/pricing.py:calculate_discount:predicate_boundary:amount>=threshold",
                    "kind": "MissingBoundaryAssertion",
                    "language": "python",
                    "language_status": "preview",
                    "scope": "pr_local",
                    "current_evidence_strength": "weakly_exposed",
                    "changed_behavior": "if amount >= threshold:",
                    "missing_discriminator": "amount == threshold",
                    "gap_state": "actionable",
                    "policy_state": "new",
                    "repairability": "repairable",
                    "static_limit_kind": "python_preview",
                    "static_limit_detail": "Python repair cards are preview advisory evidence.",
                    "anchor": {
                        "file": "app/pricing.py",
                        "line": 2,
                        "owner": "python:app/pricing.py::calculate_discount",
                        "dedupe_fingerprint": "gap:python:app/pricing.py:calculate_discount:predicate_boundary:amount>=threshold"
                    },
                    "repair_route": {
                        "route_kind": "StrengthenExistingTest",
                        "target_file": "tests/test_pricing.py",
                        "related_test": "test_calculate_discount_smoke",
                        "assertion_shape": "assert calculate_discount(amount=threshold, threshold=threshold) == expected_discount",
                        "missing_discriminator": "amount == threshold",
                        "changed_behavior": "if amount >= threshold:",
                        "stop_conditions": [
                            "import cannot be resolved",
                            "expected value is ambiguous",
                            "production code edit appears necessary"
                        ]
                    },
                    "verification_commands": [
                        "pytest tests/test_pricing.py::test_calculate_discount_smoke"
                    ],
                    "receipt_command": "ripr outcome --before .ripr/before.json --after .ripr/after.json --format json --out .ripr/receipts/python-threshold.json"
                }
            ]
        })
    }

    fn check_output_with_python_repair_card() -> Value {
        json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "findings": [
                {
                    "id": "probe:app_pricing.py:2:python_preview",
                    "classification": "weakly_exposed",
                    "probe": {
                        "file": "app/pricing.py",
                        "line": 2,
                        "family": "predicate_boundary"
                    },
                    "canonical_gap": {
                        "id": "gap:python:app/pricing.py:calculate_discount:predicate_boundary:amount>=threshold",
                        "file": "app/pricing.py",
                        "owner": "python:app/pricing.py::calculate_discount",
                        "behavior_kind": "predicate_boundary"
                    },
                    "related_tests": [
                        {
                            "file": "tests/test_pricing.py",
                            "line": 4,
                            "name": "test_calculate_discount"
                        }
                    ],
                    "python_repair_card": {
                        "language": "python",
                        "language_status": "preview",
                        "canonical_gap_id": "gap:python:app/pricing.py:calculate_discount:predicate_boundary:amount>=threshold",
                        "changed_owner": "python:app/pricing.py::calculate_discount",
                        "changed_behavior": "if amount >= threshold:",
                        "current_test_evidence": [
                            "tests/test_pricing.py reaches calculate_discount",
                            "existing test asserts broad success"
                        ],
                        "missing_discriminator": "amount == threshold",
                        "repair_action": "strengthen_existing_test",
                        "test_shape": "pytest exact boundary assertion",
                        "suggested_assertion": "assert calculate_discount(amount=threshold, threshold=threshold) == expected_discount",
                        "suggested_location": {
                            "source_file": "app/pricing.py",
                            "test_file": "tests/test_pricing.py",
                            "test_name": "test_calculate_discount_smoke"
                        },
                        "verify": {
                            "command": "pytest tests/test_pricing.py::test_calculate_discount_smoke",
                            "confidence": "high"
                        },
                        "receipt": {
                            "status": "unavailable_until_saved_check_output"
                        },
                        "stop_conditions": [
                            "Stop if imports, fixtures, or test setup cannot call the changed owner.",
                            "Stop if the expected value for the missing discriminator is ambiguous.",
                            "Stop if adding the test appears to require a production-code edit."
                        ],
                        "limits": [
                            "static advisory evidence only"
                        ],
                        "authority_boundary": "preview_advisory_only"
                    }
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
        let path = temp_cargo_root(name)?;
        init_git_repo(&path)?;
        Ok(path)
    }

    fn temp_python_repo(name: &str) -> Result<PathBuf, String> {
        let path = write_temp_root(&env::temp_dir(), name)?;
        fs::write(
            path.join("pyproject.toml"),
            "[project]\nname = \"first-pr-python-test\"\nversion = \"0.0.0\"\n",
        )
        .map_err(|err| format!("write temp pyproject.toml: {err}"))?;
        init_git_repo_with_initial_files(&path, &["pyproject.toml"])?;
        Ok(path)
    }

    fn temp_cargo_root_outside_repo(name: &str) -> Result<PathBuf, String> {
        let repo_root = fixture_repo_root()?;
        let parent = repo_root
            .parent()
            .ok_or_else(|| format!("{} has no parent", repo_root.display()))?;
        write_temp_cargo_root(parent, name)
    }

    fn temp_cargo_root(name: &str) -> Result<PathBuf, String> {
        write_temp_cargo_root(&env::temp_dir(), name)
    }

    fn write_temp_cargo_root(parent: &Path, name: &str) -> Result<PathBuf, String> {
        let path = write_temp_root(parent, name)?;
        fs::write(
            path.join("Cargo.toml"),
            "[package]\nname = \"first-pr-test\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write temp Cargo.toml: {err}"))?;
        Ok(path)
    }

    fn write_temp_root(parent: &Path, name: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock error: {err}"))?
            .as_nanos();
        let path = parent.join(format!("ripr-{name}-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&path).map_err(|err| format!("mkdir {}: {err}", path.display()))?;
        Ok(path)
    }

    fn init_git_repo(path: &Path) -> Result<(), String> {
        init_git_repo_with_initial_files(path, &["Cargo.toml"])
    }

    fn init_git_repo_with_initial_files(path: &Path, files: &[&str]) -> Result<(), String> {
        run_git_setup(path, &["init"])?;
        run_git_setup(path, &["config", "user.email", "ripr@example.invalid"])?;
        run_git_setup(path, &["config", "user.name", "RIPR Test"])?;
        for file in files {
            run_git_setup(path, &["add", file])?;
        }
        run_git_setup(path, &["commit", "-m", "init"])?;
        run_git_setup(path, &["update-ref", "refs/remotes/origin/main", "HEAD"])
    }

    fn run_git_setup(path: &Path, args: &[&str]) -> Result<(), String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .map_err(|err| format!("failed to run git {args:?}: {err}"))?;
        if output.status.success() {
            return Ok(());
        }
        Err(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }

    fn cleanup(path: &Path) -> Result<(), String> {
        if path.exists() {
            fs::remove_dir_all(path).map_err(|err| format!("cleanup {}: {err}", path.display()))?;
        }
        Ok(())
    }

    fn run_git_ok(root: &Path, args: &[&str]) -> Result<(), String> {
        let output = run_git(root, &git_args(args))?;
        if output.success() {
            Ok(())
        } else {
            Err(command_problem(
                "git test command failed.",
                &output,
                "git command failed",
            ))
        }
    }

    fn preflight_check<'a>(packet: &'a Value, id: &str) -> Result<&'a Value, String> {
        let checks = packet
            .get("preflight")
            .and_then(|value| value.get("checks"))
            .and_then(Value::as_array)
            .ok_or_else(|| "packet is missing preflight checks".to_string())?;
        checks
            .iter()
            .find(|check| string_path(check, &["id"]).is_some_and(|value| value == id))
            .ok_or_else(|| format!("missing preflight check {id}"))
    }

    fn fixture_repo_root() -> Result<PathBuf, String> {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .ok_or_else(|| "failed to resolve fixture repo root".to_string())
    }
}
