use crate::agent::command_specs::report_regeneration_command_spec_from_display;
use crate::agent::loop_commands::{
    anchored_redirect_target, check_repo_exposure_command, display_path, shell_arg,
};
use crate::app::agent_status::pilot_select_command;
use crate::config::detect_python_project;
use crate::domain::CommandSpec;
use crate::output::gap_decision_ledger::projection_eligible_from_value;
use crate::output::receipt_lifecycle::receipt_lifecycle_state;
use crate::output::receipt_write::receipt_write_command;
use crate::output::review_comments::SUMMARY_REASON_NO_SAFE_PLACEMENT;
use crate::output::start_here_state::{
    START_HERE_PREVIEW_LIMITED, normalize_start_here_output_state, start_here_output_state_is_known,
};
#[cfg(test)]
use crate::testing::cwd_placeholder::{project_cwd_text, project_renderer_cwd};
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
/// `selected.repair.route` for a review-card selection: the route is the
/// carried `agent repair` transaction rather than a ledger route kind.
const REVIEW_CARD_REPAIR_ROUTE: &str = "AgentRepairTransaction";
pub(crate) const STATIC_EVIDENCE_BOUNDARY: &str = "static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval.";

// Human labels for the proof path (#3906). A low-level verify compares
// snapshots taken around a test edit and a receipt reads that verify, so
// neither can run before the edit; the labels say when each one runs.
// When a repair start is present, its after phase already runs verify and
// writes the receipt, so the low-level pair is only the manual alternative.
// JSON fields keep their names; only the human rendering changes.
pub(crate) const VERIFY_AFTER_EDIT_LABEL: &str = "Verify after the test edit";
pub(crate) const RECEIPT_AFTER_VERIFY_LABEL: &str = "Receipt after verify";
pub(crate) const REPAIR_AFTER_PHASE_LABEL: &str = "After the test edit";
pub(crate) const REPAIR_AFTER_PHASE_STEP: &str = "run the `--attempt ... --phase after` command the before phase prints; it verifies movement and writes the receipt.";
// The manual pair names its prerequisites (F60-2(c)): the low-level verify
// reads a before snapshot taken before the test edit and an after snapshot
// taken after it, so on a checkout without them it fails as printed. The
// repair's before and after phases write both snapshots themselves.
pub(crate) const MANUAL_VERIFY_LABEL: &str = "Manual verify without a repair attempt (needs before and after snapshots taken around the test edit)";
pub(crate) const MANUAL_RECEIPT_LABEL: &str =
    "Manual receipt without a repair attempt (after the manual verify)";

/// Label for the line that follows a receipt command recording `not_run`.
pub(crate) const RECEIPT_STATUS_LABEL: &str = "Receipt status";
/// What to pass as `--status` once verify has run. The printed receipt
/// command records `--status not_run` so it stays runnable and true as
/// printed; only the reader knows the verify outcome, so the line names the
/// values that carry it instead of the command claiming one.
pub(crate) const RECEIPT_STATUS_STEP: &str = "the command records `--status not_run` as printed; after the verify command runs, change it to `--status passed` if verify exited 0 or `--status failed` if it did not.";

/// Label for the command that re-checks the selected gap's static evidence
/// after verify, on the check-output route that has no repair after phase.
pub(crate) const STATIC_RECHECK_LABEL: &str = "Static re-check after verify";
/// Label for the line that says what the receipt does not show.
pub(crate) const RECEIPT_BOUNDARY_LABEL: &str = "Receipt boundary";
/// A `ripr receipt write` receipt records the verify status it is given and
/// nothing else, so nothing on that route shows whether the gap moved (MCP
/// agent walk, 2026-09-29). The static re-check reads the uncommitted test
/// edit and compares it with the check report the gap came from.
pub(crate) const RECEIPT_BOUNDARY_STEP: &str = "the receipt records the verify status you pass; it does not re-check the gap. The static re-check reads the test edit even before it is committed (`--worktree`) and compares it with the check report this gap came from; find this gap's line: under `Moved` or `Removed` its static evidence changed, under `Unchanged` it did not. Static movement is not a runtime or mutation result.";

/// The `Receipt status` step for a receipt command that records `not_run`,
/// `None` for any other command (a `ripr outcome` receipt, or one that already
/// carries an outcome).
pub(crate) fn receipt_status_step(receipt_command: &str) -> Option<&'static str> {
    let words = ripr_command_words(receipt_command)?;
    (is_receipt_write(&words) && ripr_flag_value(&words, "--status") == Some("not_run"))
        .then_some(RECEIPT_STATUS_STEP)
}

/// Whether `command` is `ripr receipt write`, whose receipt records the
/// verify status it is given and re-checks nothing; a `ripr outcome` receipt
/// compares snapshots itself.
fn is_receipt_write_command(command: &str) -> bool {
    ripr_command_words(command).is_some_and(|words| is_receipt_write(&words))
}

fn is_receipt_write(words: &[(String, bool)]) -> bool {
    words.get(1).map(|(word, _)| word.as_str()) == Some("receipt")
        && words.get(2).map(|(word, _)| word.as_str()) == Some("write")
}

/// The one selector for the low-level verify and receipt labels (#3906).
///
/// With a carried repair start, its after phase runs verify and writes the
/// receipt, so the pair is the manual alternative; without one, both are
/// steps that run after the focused test edit. Every human surface takes
/// its labels from here, so the transaction reads the same everywhere.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProofPathLabels {
    pub(crate) verify: &'static str,
    pub(crate) receipt: &'static str,
}

impl ProofPathLabels {
    pub(crate) fn for_repair_start(has_repair_start: bool) -> Self {
        if has_repair_start {
            Self {
                verify: MANUAL_VERIFY_LABEL,
                receipt: MANUAL_RECEIPT_LABEL,
            }
        } else {
            Self {
                verify: VERIFY_AFTER_EDIT_LABEL,
                receipt: RECEIPT_AFTER_VERIFY_LABEL,
            }
        }
    }
}

mod options;
mod preflight;
mod rendering;
mod validation;

pub(crate) use options::FIRST_PR_HELP;
use options::{FirstPrOptions, parse_options, print_help};
use preflight::{FirstPrPreflight, first_pr_preflight};
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

    let mut options = parse_options(args)?;
    print_side_effect_disclosure(&options);

    let repo = repo_root()?;
    let resolution = omitted_base_resolution(&repo, &options);
    // A missing packet is answered before a base failure (#4285): `--check`
    // never diffs, so a repository where no default base resolves still gets
    // the recovery, and that recovery demands the `--base` its write needs.
    if options.check {
        let root = resolve_path(&repo, &options.root);
        let root_recovery = root_preflight_recovery(&root, &options).is_some();
        let base_error = resolution.as_ref().and_then(|result| result.as_ref().err());
        check_packet_paths(&repo, &root, root_recovery, base_error, &options)?;
    }
    match resolution {
        Some(Ok(base)) => options.base = base,
        Some(Err(err)) => return Err(format!("first-pr: {err}")),
        None => {}
    }
    if options.check {
        check_first_pr(&repo, &options)
    } else {
        write_first_pr(&repo, &options)
    }
}

/// Resolve an omitted `--base` through the diff loader's authority (#3952,
/// RIPR-SPEC-0084) instead of assuming `origin/main`, which need not exist:
/// in a repository without `origin` the packet used to record a base that
/// was never analyzed.
///
/// Only a root that is a Git work tree is resolved. A missing root or a
/// non-repository keeps the placeholder so the root and Git preflights still
/// write their own recovery packets; those block before the base is used.
/// When the root is a work tree and nothing resolves, the result is the
/// loader's named error: the run fails with it rather than recording a
/// guessed base. `None` means nothing is resolved (explicit base, or a root
/// the preflights handle).
fn omitted_base_resolution(
    repo: &Path,
    options: &FirstPrOptions,
) -> Option<Result<String, String>> {
    if options.base_explicit {
        return None;
    }
    let root = resolve_path(repo, &options.root);
    if !root.is_dir() || git_worktree_available_with_ceiling(&root, None) != Ok(true) {
        return None;
    }
    Some(crate::analysis::resolve_effective_base(&root, None, None))
}

/// Print the side-effect and cost disclosure for the *resolved* invocation, so
/// `--check` (validate-only) and custom `--out-dir` runs report their actual
/// write behavior rather than always claiming the defaults. Rendered after
/// `parse_options` and before any filesystem write.
fn print_side_effect_disclosure(options: &FirstPrOptions) {
    println!("ripr first-pr - side effects and cost disclosure");
    // first-pr composes existing artifacts; it runs no analysis, so it has
    // no analysis cache or diff-sized runtime to disclose.
    println!("  cost class:      artifact composition only; runs no analysis");
    if options.check {
        println!("  writes to:       none (--check validates an existing start-here packet)");
    } else {
        println!(
            "  writes to:       {}/",
            options.out_dir.trim_end_matches('/')
        );
    }
    println!("  cache location:  none");
    println!("  git reads:       yes (base and head preflight)");
    println!("  network:         none");
    println!("  runtime hint:    seconds\n");
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
    crate::output::file_write::create_output_dir(&out_dir, "--out-dir")?;
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
    println!("Wrote {}", display_path(&json_path));
    println!("Wrote {}", display_path(&markdown_path));
    Ok(())
}

/// The packet `--check` validates lives under the root, or under the
/// invocation directory when the root failed its preflight (where the write
/// run puts that recovery packet). A missing packet file is the validate-only
/// recovery error, never a later preflight or base failure; `base_error` is
/// the default-base failure the suggested write would otherwise hit.
fn check_packet_paths(
    repo: &Path,
    root: &Path,
    root_recovery: bool,
    base_error: Option<&String>,
    options: &FirstPrOptions,
) -> Result<(PathBuf, PathBuf), String> {
    let output_root = if root_recovery { repo } else { root };
    let out_dir = resolve_path(output_root, &options.out_dir);
    let json_path = out_dir.join(START_HERE_JSON);
    let markdown_path = out_dir.join(START_HERE_MD);
    if !json_path.exists() || !markdown_path.exists() {
        return Err(first_pr_missing_packet_recovery_error(
            &json_path,
            &markdown_path,
            options,
            &out_dir,
            base_error.map(String::as_str),
        ));
    }
    Ok((json_path, markdown_path))
}

fn check_first_pr(repo: &Path, options: &FirstPrOptions) -> Result<(), String> {
    let root = resolve_path(repo, &options.root);
    let root_recovery = root_preflight_recovery(&root, options);
    let (json_path, markdown_path) =
        check_packet_paths(repo, &root, root_recovery.is_some(), None, options)?;
    let preflight_recovery = root_recovery.or_else(|| git_preflight_recovery(&root, options));
    let packet = validate_start_here_packet(&json_path, &markdown_path)?;
    validate_current_preflight_recovery(&packet, &root, options, preflight_recovery)?;
    print!(
        "{}",
        start_here_cli_summary(&packet, &json_path, &markdown_path)
    );
    println!("First PR start-here packet ok: {}", json_path.display());
    Ok(())
}

fn first_pr_missing_packet_recovery_error(
    json_path: &Path,
    markdown_path: &Path,
    options: &FirstPrOptions,
    out_dir: &Path,
    base_error: Option<&str>,
) -> String {
    let missing = if !json_path.exists() {
        json_path
    } else {
        markdown_path
    };
    // Render the already-resolved locations: the suggested command must
    // reproduce the exact directory `--check` validated, even when pasted
    // from a different working directory, and paths render with stable
    // separators on every host.
    let mut message = format!(
        "first-pr --check validates an existing start-here packet; it does not create one.\n\nMissing:\n  {}\n\nCreate and validate it with:\n  {}",
        display_path(missing),
        first_pr_write_command(options, out_dir, base_error.is_some())
    );
    // Without a resolvable default base the write needs an explicit one; the
    // placeholder is never filled with a guess (#4285).
    if let Some(err) = base_error {
        message.push_str(&format!(
            "\n\nReplace <ref> with the branch or commit this PR is based on: {err}"
        ));
    }
    message
}

fn first_pr_write_command(options: &FirstPrOptions, out_dir: &Path, base_required: bool) -> String {
    let mut parts = vec![
        "ripr".to_string(),
        "first-pr".to_string(),
        "--root".to_string(),
        shell_arg(&options.command_root()),
    ];
    // An omitted base stays omitted when the write run can resolve it through
    // the diff loader (#3952); rendering a default here would be a guess. When
    // nothing resolves, the command names the `--base` the user must supply.
    if options.base_explicit {
        parts.push("--base".to_string());
        parts.push(shell_arg(&options.base));
    } else if base_required {
        parts.push("--base".to_string());
        parts.push("<ref>".to_string());
    }
    parts.push("--head".to_string());
    parts.push(shell_arg(&options.head));
    if let Some(check_output) = &options.check_output {
        parts.push("--check-output".to_string());
        parts.push(shell_arg(check_output));
    }
    if options.gap_ledger_explicit {
        parts.push("--gap-ledger".to_string());
        parts.push(shell_arg(&options.gap_ledger));
    }
    parts.push("--out-dir".to_string());
    parts.push(shell_arg(&display_path(out_dir)));
    parts.join(" ")
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
        shell_arg(&options.command_root()),
        options.base,
        options.head
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
                shell_arg(&options.command_root()),
                shell_arg(&options.base),
                options.anchored_arg(check_output)
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
                shell_arg(&options.command_root()),
                options.anchored_arg(&options.gap_ledger),
                options.anchored_arg(&options.first_action),
                options.anchored_arg(&with_extension(&options.first_action, "md"))
            )),
        ),
        artifact_status(
            root,
            "review_comments",
            "PR repair cards",
            &options.review_comments,
            Some(review_comments_regeneration_command(root, options)),
        ),
        artifact_status(
            root,
            "agent_packet",
            "Agent repair packet",
            &options.agent_packet,
            selection.agent_packet_command(),
        ),
        artifact_status(
            root,
            "gate_decision",
            "Gate decision",
            &options.gate_decision,
            Some(format!(
                "ripr gate evaluate --gap-ledger {} --out {} --out-md {}",
                options.anchored_arg(&options.gap_ledger),
                options.anchored_arg(&options.gate_decision),
                options.anchored_arg(&with_extension(&options.gate_decision, "md"))
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

    let mut selected = selection.to_json();
    // The repo-exposure report and gap ledger are both prerequisites for the
    // first Rust start-here selection. Keep the primary recovery route, but
    // disclose the second missing input now instead of making the operator
    // discover it on the next invocation. Other artifact rows are optional
    // until a particular selection needs them.
    if selected["artifact"]["id"] == "repo_exposure"
        && let Some(ledger) = artifacts
            .iter()
            .find(|artifact| artifact["id"] == "gap_ledger" && artifact["status"] == "missing")
    {
        selected["also_missing"] = json!([{
            "id": "gap_ledger",
            "label": "Gap decision ledger",
            "path": options.gap_ledger,
            "regeneration_command": ledger["regeneration_command"]
        }]);
    }

    let mut packet = json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "kind": "first_pr_start_here",
        "status": selection.status(),
        "posture": "advisory",
        "root": options.root,
        "inputs": inputs,
        "selected": selected,
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
    let mut report = crate::output::gap_decision_ledger::build_gap_decision_ledger_report(
        crate::output::gap_decision_ledger::GapDecisionLedgerInput {
            root: options.root.clone(),
            generated_at: "first-pr-check-output".to_string(),
            source_kind:
                crate::output::gap_decision_ledger::GapDecisionLedgerSourceKind::CheckOutput,
            records_path: check_output.to_string(),
            records_json: Ok(contents),
        },
    );
    crate::output::gap_decision_ledger::stamp_gap_decision_ledger_source_subject(
        &mut report,
        root,
    )?;
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

fn missing_base_command(options: &FirstPrOptions) -> String {
    options.base.strip_prefix("origin/")
        .filter(|branch| !branch.trim().is_empty())
        .map(|branch| {
            format!(
                "git fetch origin {branch}; then rerun `ripr first-pr --root {} --base {} --head {}`.",
                shell_arg(&options.command_root()), options.base, options.head
            )
        })
        .unwrap_or_else(|| {
            format!(
                "Fetch or choose a local base ref, then rerun `ripr first-pr --root {} --base {} --head {}`.",
                shell_arg(&options.command_root()), options.base, options.head
            )
        })
}

fn git_args(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_string()).collect()
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
            Some(doctor_command(&options.command_root())),
        ));
    }
    if !root.join("Cargo.toml").is_file() {
        if detect_python_project(root) || detect_typescript_project(root) {
            return None;
        }
        // A Go or Java repository is the right root; sending it to `--root`
        // and doctor loops. Name the languages ripr cannot analyze instead.
        // Rust or preview source below the root (a nested crate) means the
        // root really is wrong, so that case keeps `wrong_root`.
        let analyzable_below = !crate::analysis::workspace_rust_files(root).is_empty()
            || !crate::analysis::workspace_preview_language_files(root).is_empty();
        let unanalyzed = crate::analysis::workspace_unanalyzed_source_languages(root);
        if !analyzable_below && !unanalyzed.is_empty() {
            let found = unanalyzed
                .iter()
                .map(|(language, count)| format!("{language} ({count} file(s))"))
                .collect::<Vec<_>>()
                .join(", ");
            return Some(Selection::no_action(
                "no_action",
                format!(
                    "The first-pr root `{}` has {found} source and no Rust, Python or TypeScript project. ripr does not analyze these languages, so there is no gap to assign; review their changes with their own tests.",
                    options.root
                ),
                0,
            ));
        }
        return Some(Selection::blocked(
            "wrong_root",
            format!(
                "The first-pr root `{}` is not a Rust/Cargo workspace because Cargo.toml is missing, and no Python or TypeScript project markers were found. Pass the repository root with `--root` before assigning repair work.",
                options.root
            ),
            Some(doctor_command(&options.command_root())),
        ));
    }
    None
}

fn git_preflight_recovery(root: &Path, options: &FirstPrOptions) -> Option<Selection> {
    match git_worktree_available_with_ceiling(root, options.git_ceiling.as_deref()) {
        Ok(true) => {}
        Ok(false) => {
            return Some(Selection::blocked(
                "blocked_artifact",
                format!(
                    "The first-pr root `{}` is not a git worktree. Run setup checks before assigning repair work.",
                    options.root
                ),
                Some(doctor_command(&options.command_root())),
            ));
        }
        Err(message) => {
            return Some(Selection::blocked(
                "blocked_artifact",
                format!(
                    "The first-pr git preflight could not run for root `{}`: {message}. Run setup checks before assigning repair work.",
                    options.root
                ),
                Some(doctor_command(&options.command_root())),
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
                Some(doctor_command(&options.command_root())),
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
                Some(doctor_command(&options.command_root())),
            ));
        }
    }

    if let Err(message) = git_diff_range_valid(root, &options.base, &options.head) {
        // #4538: a range with no merge base names its cause (shallow clone or
        // unrelated histories) like `ripr check` does, and a shallow clone's
        // next command is the unshallow repair, not the same failing diff.
        if message.contains("no merge base") {
            let (diagnosis, shallow) =
                crate::analysis::no_merge_base_diagnosis(root, &options.base, &options.head, None);
            let command = if shallow {
                unshallow_command(options)
            } else {
                diff_range_command(options)
            };
            return Some(Selection::blocked(
                "blocked_artifact",
                format!(
                    "The first-pr diff range `{}...{}` has no merge base: {diagnosis}",
                    options.base, options.head
                ),
                Some(command),
            ));
        }
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
        // Boxed to keep the variant small: CommandSpec is a wide struct
        // and `Selection` is matched by value on the render paths.
        command_spec: Option<Box<CommandSpec>>,
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
    fn missing_artifact(
        id: &str,
        label: &str,
        path: &str,
        regeneration_command: String,
        command_spec: Option<CommandSpec>,
    ) -> Self {
        Self::MissingArtifact {
            id: id.to_string(),
            label: label.to_string(),
            path: path.to_string(),
            regeneration_command,
            command_spec: command_spec.map(Box::new),
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
        top_gap.agent_packet_command.clone()
    }

    fn commands_json(&self, root: &Path, options: &FirstPrOptions) -> Value {
        let mut commands = Map::new();
        commands.insert(
            "regenerate_gap_ledger".to_string(),
            Value::String(regenerate_gap_ledger_command(root, options)),
        );
        match self {
            Self::TopGap(top_gap) => {
                if let Some(command) = &top_gap.agent_packet_command {
                    commands.insert("agent_packet".to_string(), Value::String(command.clone()));
                }
                // The review card's repair start stays on `selected` only: the
                // editor's first-pr projection fails the whole packet closed on
                // a `commands` value outside its allowlist, and `agent repair`
                // is not on it.
                // The carried analysis-outcome step also stays in selected
                // metadata and Markdown: `ripr check` is outside that editor
                // allowlist. Do not make an otherwise usable packet unsafe.
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
                command_spec,
            } => {
                // FIX #1617 slice 2: the typed spec is additive beside the
                // legacy string; the `commands` map stays string-only.
                let mut value = json!({
                    "state": "missing_artifact",
                    "output_state": normalize_start_here_output_state("missing_artifact"),
                    "artifact": {
                        "id": id,
                        "label": label,
                        "path": path
                    },
                    "next_action": "regenerate_missing_artifact",
                    "regeneration_command": regeneration_command
                });
                if let Some(spec) = command_spec
                    && let Ok(spec_value) = serde_json::to_value(spec)
                {
                    value["regeneration_command_spec"] = spec_value;
                }
                value
            }
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
    analysis_outcome_command: Option<String>,
    verify_command: String,
    receipt_command: String,
    /// `None` for a review-card selection: its carried receipt command names
    /// its own output, and first-pr does not parse commands for paths.
    receipt_path: Option<String>,
    receipt_command_source: String,
    receipt_state: Option<String>,
    static_limit_kind: Option<String>,
    static_limit_detail: Option<String>,
    /// `None` for a review-card selection: `agent packet --gap-id` resolves
    /// ledger gaps, and the card is not one of them.
    agent_packet_command: Option<String>,
    /// The review card's `llm_guidance.repair_command`, carried unchanged
    /// (#3906). Ledger selections never carry one: first-pr does not build
    /// `agent repair` from a gap id, probe id, or seam id.
    repair_command: Option<String>,
    /// The static re-check after verify on the check-output route: rerun
    /// check over the working tree and compare it with the check report the
    /// gap came from. `None` when that report is absent or a repair start
    /// carries its own after phase.
    static_recheck_command: Option<String>,
}

impl TopGapSelection {
    fn to_json(&self) -> Value {
        let mut value = json!({
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
        });
        // Present only when carried, like the card field it projects; ledger
        // selections keep their existing shape.
        if let Some(command) = &self.repair_command {
            value["repair_command"] = Value::String(command.clone());
        }
        if let Some(command) = &self.analysis_outcome_command {
            value["analysis_outcome_command"] = Value::String(command.clone());
        }
        if let Some(command) = &self.static_recheck_command {
            value["static_recheck_command"] = Value::String(command.clone());
        }
        value
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
            Some(regenerate_blocked_gap_ledger_command(root, options)),
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
        let top_gap = top_gap_from_record(record, gap_ledger, root, options);
        if let Some(edited) = check_output_evidence_predates_edit(root, options, &top_gap) {
            return Selection::blocked(
                "stale_artifact",
                format!(
                    "The gap decision ledger predates the last edit to `{edited}`, so its repair instruction may already be done; refresh the evidence, then rerun first-pr to see whether the gap is still open."
                ),
                // The freshness check reads the check output's timestamp, so
                // the recovery must rewrite it, not only the ledger.
                Some(rerun_check_output_gap_ledger_command(options)),
            );
        }
        return Selection::TopGap(Box::new(top_gap));
    }
    match review_card_repair_start(root, options) {
        Ok(top_gap) => Selection::TopGap(Box::new(top_gap)),
        Err(CardFallback::Recover(selection)) => selection,
        Err(CardFallback::NoCard(note)) => {
            let mut reason = "No repairable PR-local stable Rust or preview Python/TypeScript gap was selected from the gap decision ledger."
                .to_string();
            if let Some(limitation) = records
                .iter()
                .find_map(|record| static_limitation_note(record))
            {
                reason.push(' ');
                reason.push_str(&limitation);
            }
            if let Some(note) = note {
                reason.push(' ');
                reason.push_str(&note);
            }
            Selection::no_action("no_action", reason, records.len())
        }
    }
}

/// Second source for a start (#3906): when the ledger yields no top gap, the
/// first review card that carries `llm_guidance.repair_command` becomes the
/// selection. Generated CI builds the ledger from repo-exposure, whose records
/// are all repo-scoped, so without this a PR with an eligible seam card never
/// reaches `agent repair`.
///
/// The card's command is carried, never rebuilt: the card producer offers it
/// only past the fail-closed repair-packet flip and only for a test-surface
/// target (`evidence_record::repair_start_command_for`). Every field of the
/// selection comes from that one card, never mixed with a ledger record.
///
/// Why no review card became the selection.
///
/// `Recover` means the cards could not be read at all (missing, unreadable,
/// or for another root or range): first-pr stops on that input with its
/// regeneration command, because "no actionable gap" would be a verdict on
/// cards it never saw (F60-10). `NoCard` means current cards were read and
/// none carries a repair start; its note is appended to the no-action reason.
enum CardFallback {
    Recover(Selection),
    NoCard(Option<String>),
}

fn review_card_repair_start(
    root: &Path,
    options: &FirstPrOptions,
) -> Result<TopGapSelection, CardFallback> {
    let path = &options.review_comments;
    let seam_route = seam_review_comments_command(options);
    let report = match read_json(&resolve_path(root, path)) {
        Ok(report) => report,
        // Seam cards come from Rust seam analysis; a preview-language root
        // has no seam-level route to name.
        Err(ArtifactReadError::Missing) if uses_check_output_gap_ledger(root) => {
            return Err(CardFallback::NoCard(None));
        }
        Err(ArtifactReadError::Missing) => {
            let command_spec = report_regeneration_command_spec_from_display(
                &seam_route,
                display_selected_root(options),
            );
            return Err(CardFallback::Recover(Selection::missing_artifact(
                "review_comments",
                "PR repair cards",
                path,
                seam_route,
                command_spec,
            )));
        }
        Err(ArtifactReadError::Malformed(message)) => {
            return Err(CardFallback::Recover(Selection::blocked(
                "malformed_artifact",
                format!(
                    "The review cards at `{path}` could not be read ({message}); regenerate them before first-pr can say whether this PR has a repair start."
                ),
                Some(seam_route),
            )));
        }
    };
    if let Some((state, problem)) = review_comments_currentness_problem(&report, root, options) {
        return Err(CardFallback::Recover(Selection::blocked(
            state,
            format!(
                "The review cards at `{path}` were not used because {problem}; regenerate them before first-pr can say whether this PR has a repair start."
            ),
            Some(seam_route),
        )));
    }
    let cards = ["comments", "summary_only"]
        .into_iter()
        .filter_map(|bucket| report.get(bucket).and_then(Value::as_array))
        .flatten();
    let mut outside_diff_starts = 0usize;
    for card in cards {
        // A card with no safe changed-line placement is a seam whose line and
        // owner sit outside every hunk. It is a repository repair, not this
        // PR's changed behavior, so it never becomes the PR's top gap.
        if string_path(card, &["summary_reason"]).as_deref()
            == Some(SUMMARY_REASON_NO_SAFE_PLACEMENT)
        {
            if top_gap_from_review_card(card, options).is_some() {
                outside_diff_starts += 1;
            }
            continue;
        }
        if let Some(top_gap) = top_gap_from_review_card(card, options) {
            return Ok(top_gap);
        }
    }
    if outside_diff_starts > 0 {
        return Err(CardFallback::NoCard(Some(format!(
            "{outside_diff_starts} review card(s) in `{path}` carry a repair start for code outside this PR's changed lines; `{}` ranks repository-wide repairs.",
            pilot_select_command(&options.root)
        ))));
    }
    // Gap-ledger-scoped cards never carry a repair start; the seam-level
    // report is the route that can.
    if string_path(&report, &["analysis_scope", "scope"]).as_deref() == Some("gap_ledger_artifact")
    {
        return Err(CardFallback::NoCard(Some(format!(
            "No review card in `{path}` carries a repair start (`llm_guidance.repair_command`) because the cards were rendered from the gap ledger; for a seam-level repair start, run `{seam_route}`."
        ))));
    }
    Err(CardFallback::NoCard(Some(format!(
        "No review card in `{path}` carries a repair start (`llm_guidance.repair_command`)."
    ))))
}

/// Fail closed on a review-comments report that is not a complete, current
/// RIPR report for this invocation: a card for another root or range must not
/// become this PR's repair start.
fn review_comments_currentness_problem(
    report: &Value,
    root: &Path,
    options: &FirstPrOptions,
) -> Option<(&'static str, String)> {
    if string_path(report, &["tool"]).as_deref() != Some("ripr") {
        return Some((
            "malformed_artifact",
            "the file is not a RIPR review-comments report".to_string(),
        ));
    }
    match string_path(report, &["status"]) {
        Some(status) if status == "advisory" => {}
        Some(status) => {
            return Some((
                "blocked_artifact",
                format!("the report status is `{status}`"),
            ));
        }
        None => {
            return Some((
                "malformed_artifact",
                "the report does not record a status".to_string(),
            ));
        }
    }
    match string_path(report, &["root"]) {
        Some(observed) if root_mismatch(root, &options.root, &observed) => {
            return Some((
                "wrong_root",
                format!(
                    "they were generated for root `{observed}`, not `{}`",
                    options.root
                ),
            ));
        }
        Some(_) => {}
        None => {
            return Some((
                "malformed_artifact",
                "the report does not record its root".to_string(),
            ));
        }
    }
    for (field, expected) in [("base", &options.base), ("head", &options.head)] {
        match string_path(report, &[field]) {
            Some(observed) if observed == *expected => {}
            Some(observed) => {
                return Some((
                    "stale_artifact",
                    format!("they were generated for {field} `{observed}`, not `{expected}`"),
                ));
            }
            None => {
                return Some((
                    "malformed_artifact",
                    format!("the report does not record its {field}"),
                ));
            }
        }
    }
    None
}

/// How to regenerate the review cards first-pr reads. A Rust root needs the
/// seam-level cards, the only ones that can carry a repair start; cards
/// rendered from the gap ledger never do, so offering that form sent a
/// fresh checkout to cards that could not help it. A preview-language root
/// has no seam analysis, so its cards come from the gap ledger.
fn review_comments_regeneration_command(root: &Path, options: &FirstPrOptions) -> String {
    if uses_check_output_gap_ledger(root) {
        return format!(
            "ripr review-comments --root {} --base {} --head {} --gap-ledger {} --out {}",
            shell_arg(&options.command_root()),
            shell_arg(&options.base),
            shell_arg(&options.head),
            options.anchored_arg(&options.gap_ledger),
            options.anchored_arg(&options.review_comments)
        );
    }
    seam_review_comments_command(options)
}

/// The seam-level review-comments command: without `--gap-ledger`, so the
/// cards come from working-set seam analysis, the only cards that can carry a
/// repair start.
fn seam_review_comments_command(options: &FirstPrOptions) -> String {
    format!(
        "ripr review-comments --root {} --base {} --head {} --out {}",
        shell_arg(&options.command_root()),
        shell_arg(&options.base),
        shell_arg(&options.head),
        options.anchored_arg(&options.review_comments)
    )
}

/// Project one review card into the start-here top gap, or `None` when it
/// carries no repair start or lacks a field the top-gap contract requires.
fn top_gap_from_review_card(card: &Value, options: &FirstPrOptions) -> Option<TopGapSelection> {
    if string_path(card, &["gap_state"]).as_deref() != Some("actionable") {
        return None;
    }
    let repair_command = string_path(card, &["llm_guidance", "repair_command"])?;
    let verify_command = string_path(card, &["llm_guidance", "verify_command"])?;
    let receipt_command = string_path(card, &["receipt_command"])?;
    let changed_behavior = string_path(card, &["seam", "expression"])?;
    let missing_discriminator = string_path(card, &["missing_discriminator"])?;
    let canonical_gap_id = string_path(card, &["canonical_gap_id"]);
    let gap_id = canonical_gap_id
        .clone()
        .or_else(|| string_path(card, &["id"]))?;
    let suggested_test = card.get("suggested_test");
    let target_file = string_from_sources(&[(suggested_test, &["recommended_file"])]);
    let related_test = string_from_sources(&[(suggested_test, &["related_test", "name"])]);
    let suggested_assertion = string_from_sources(&[(suggested_test, &["assertion_shape"])]);
    let repair_route = REVIEW_CARD_REPAIR_ROUTE.to_string();
    Some(TopGapSelection {
        gap_id,
        canonical_gap_id,
        // The card producer builds a repair start only from a classified
        // Rust seam; preview languages have no `agent repair` transaction.
        language: Some("rust".to_string()),
        language_status: Some("stable".to_string()),
        kind: string_path(card, &["kind"]).unwrap_or_else(|| "Unknown".to_string()),
        source_artifact: options.review_comments.clone(),
        changed_behavior: Some(changed_behavior),
        current_evidence_strength: current_evidence_strength_for_card(card),
        missing_discriminator,
        focused_proof_intent: focused_proof_intent(
            &repair_route,
            target_file.as_deref(),
            suggested_assertion.as_deref(),
            related_test.as_deref(),
        ),
        why: string_path(card, &["reason"]).unwrap_or_else(|| {
            "The review card names a missing discriminator for this changed seam.".to_string()
        }),
        repair_route,
        target_file,
        related_test,
        suggested_assertion,
        anchor_file: string_path(card, &["seam", "file"])
            .or_else(|| string_path(card, &["placement", "path"])),
        anchor_line: u64_from_sources(&[
            (Some(card), &["seam", "line"]),
            (Some(card), &["placement", "line"]),
        ]),
        anchor_owner: string_path(card, &["owner"]),
        dedupe_fingerprint: string_path(card, &["dedupe_key"]),
        analysis_outcome_command: string_path(card, &["llm_guidance", "analysis_outcome_command"]),
        verify_command,
        receipt_command,
        receipt_path: None,
        receipt_command_source: "review_comments.receipt_command".to_string(),
        receipt_state: None,
        static_limit_kind: None,
        static_limit_detail: None,
        agent_packet_command: None,
        repair_command: Some(repair_command),
        static_recheck_command: None,
    })
}

fn current_evidence_strength_for_card(card: &Value) -> String {
    let grip = string_path(card, &["grip_class"]).unwrap_or_else(|| "unknown".to_string());
    let oracle = match (
        string_path(card, &["oracle_strength"]),
        string_path(card, &["oracle_kind"]),
    ) {
        (Some(strength), Some(kind)) => {
            format!(" with a `{strength}` `{kind}` related-test oracle")
        }
        _ => String::new(),
    };
    format!(
        "Static evidence classifies this seam as `{grip}`{oracle}; the review card names the missing discriminator."
    )
}

fn missing_gap_ledger_selection(root: &Path, options: &FirstPrOptions) -> Selection {
    if uses_check_output_gap_ledger(root) {
        return missing_gap_ledger_artifact(
            "gap_ledger",
            "Gap decision ledger",
            &options.gap_ledger,
            root,
            options,
        );
    }

    let repo_exposure = resolve_path(root, DEFAULT_REPO_EXPOSURE);
    if !repo_exposure.exists() {
        return missing_repo_exposure_selection(root, options);
    }
    missing_gap_ledger_artifact(
        "gap_ledger",
        "Gap decision ledger",
        &options.gap_ledger,
        root,
        options,
    )
}

/// FIX #1617 slice 2: attach the typed spec recovered from the display. The
/// compound `&&` form fails the exact-shape recovery and stays
/// legacy-string-only.
fn missing_gap_ledger_artifact(
    id: &str,
    label: &str,
    path: &str,
    root: &Path,
    options: &FirstPrOptions,
) -> Selection {
    let regeneration_command = regenerate_gap_ledger_command(root, options);
    let command_spec = report_regeneration_command_spec_from_display(
        &regeneration_command,
        display_selected_root(options),
    );
    Selection::missing_artifact(id, label, path, regeneration_command, command_spec)
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
    let regeneration_command = regenerate_repo_exposure_command(&options.command_root());
    let command_spec = report_regeneration_command_spec_from_display(
        &regeneration_command,
        display_selected_root(options),
    );
    Selection::missing_artifact(
        "repo_exposure",
        "Repo exposure report",
        DEFAULT_REPO_EXPOSURE,
        regeneration_command,
        command_spec,
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
    // 0.2 (#3864) only added the file-fact cache receipt; the run status and
    // trace fields read here are unchanged, so both versions stay usable.
    if !matches!(
        string_path(&report, &["schema_version"]).as_deref(),
        Some("0.1" | "0.2")
    ) || string_path(&report, &["tool"]).as_deref() != Some("ripr")
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

/// A check-output ledger (Python or TypeScript root) records no head or
/// file identity, so after the operator edits the named test, first-pr would
/// repeat the same repair from the old ledger on every run (onboarding
/// Python walk, #4227). The selection is stale when the test file or changed
/// source it names was modified after the evidence was written: the
/// `--check-output` report when one is given (first-pr re-materializes the
/// ledger from it on every run), otherwise the ledger itself. A missing file
/// or unreadable timestamp is not treated as an edit.
fn check_output_evidence_predates_edit(
    root: &Path,
    options: &FirstPrOptions,
    top_gap: &TopGapSelection,
) -> Option<String> {
    if !uses_check_output_gap_ledger(root) {
        return None;
    }
    let evidence = options
        .check_output
        .as_deref()
        .unwrap_or(&options.gap_ledger);
    let evidence_written = fs::metadata(resolve_path(root, evidence))
        .and_then(|metadata| metadata.modified())
        .ok()?;
    [&top_gap.target_file, &top_gap.anchor_file]
        .into_iter()
        .flatten()
        .find(|file| {
            fs::metadata(resolve_path(root, file))
                .and_then(|metadata| metadata.modified())
                .is_ok_and(|edited| edited > evidence_written)
        })
        .cloned()
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

/// Name a PR-local static limitation the ledger recorded (#4224), so a
/// finding whose repair packet failed closed reads as that limitation instead
/// of a bare "no action". The detail and target shape are the ledger record's
/// own fields; nothing here makes the record delegatable.
fn static_limitation_note(record: &Value) -> Option<String> {
    if string_path(record, &["scope"]).as_deref() != Some("pr_local")
        || string_path(record, &["gap_state"]).as_deref() != Some("static_limitation")
    {
        return None;
    }
    let detail = string_path(record, &["static_limit_detail"])?;
    let kind = string_path(record, &["static_limit_kind"])
        .unwrap_or_else(|| "static_limitation".to_string());
    let location = match (
        string_path(record, &["anchor", "file"]),
        record
            .get("anchor")
            .and_then(|anchor| anchor.get("line"))
            .and_then(Value::as_u64),
    ) {
        (Some(file), Some(line)) => format!(" at `{file}:{line}`"),
        (Some(file), None) => format!(" at `{file}`"),
        _ => String::new(),
    };
    let mut note = format!("Static limitation `{kind}`{location}: {detail}.");
    let shape = record
        .get("static_limits")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|limit| {
            limit.get("kind").and_then(Value::as_str) == Some("not_delegatable_target_shape")
        })
        .and_then(|limit| limit.get("detail").and_then(Value::as_str))
        .filter(|shape| !shape.trim().is_empty());
    if let Some(shape) = shape {
        note.push_str(&format!(" Target shape (not delegatable): {shape}"));
    }
    Some(note)
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
        && (!matches!(
            (
                string_path(record, &["language"]).as_deref(),
                string_path(record, &["language_status"]).as_deref(),
            ),
            (Some("python"), Some("preview"))
        ) || projection_eligible_from_value(record, "agent_packet"))
}

fn first_pr_language_is_supported(record: &Value) -> bool {
    matches!(
        (
            string_path(record, &["language"]).as_deref(),
            string_path(record, &["language_status"]).as_deref(),
        ),
        (Some("rust"), Some("stable"))
            | (Some("python"), Some("preview"))
            | (Some("typescript"), Some("preview"))
    )
}

fn top_gap_from_record(
    record: &Value,
    gap_ledger: &Value,
    root: &Path,
    options: &FirstPrOptions,
) -> TopGapSelection {
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
    let ledger_receipt_command = string_path(record, &["receipt_command"]);
    let ledger_receipt_or_path_command = command_like_path(record, &["receipt_command_or_path"]);
    let ledger_command = ledger_receipt_command
        .map(|command| (command, "gap_ledger.receipt_command"))
        .or_else(|| {
            ledger_receipt_or_path_command
                .map(|command| (command, "gap_ledger.receipt_command_or_path"))
        });
    let receipt_path = selected_receipt_path(
        record,
        ledger_command.as_ref().map(|(command, _)| command.as_str()),
    )
    .unwrap_or_else(|| first_pr_receipt_path(&options.receipts_dir, &gap_id));
    let canonical_gap_id_for_receipt =
        string_path(record, &["canonical_gap_id"]).unwrap_or_else(|| gap_id.clone());
    let (receipt_command, receipt_command_source) = match ledger_command {
        Some((command, source)) => (command, source.to_string()),
        None => (
            receipt_write_command(
                &canonical_gap_id_for_receipt,
                &verify_command,
                Some(&receipt_path),
            ),
            "first_pr.default_receipt_write_command".to_string(),
        ),
    };
    let static_recheck_command = if is_receipt_write_command(&receipt_command) {
        static_recheck_command(gap_ledger, root, options)
    } else {
        None
    };
    let repair_route_kind = string_from_sources(&[(repair_route, &["route_kind"])])
        .unwrap_or_else(|| "RepairRouteUnavailable".to_string());
    let target_file = string_from_sources(&[(repair_route, &["target_file"])]);
    let related_test = string_from_sources(&[(repair_route, &["related_test"])]);
    let suggested_assertion = string_from_sources(&[(repair_route, &["assertion_shape"])]);
    let missing_discriminator = if matches!(language.as_deref(), Some("python" | "typescript")) {
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
        analysis_outcome_command: None,
        verify_command,
        receipt_command,
        receipt_path: Some(receipt_path),
        receipt_command_source,
        receipt_state: string_path(record, &["receipt", "state"])
            .or_else(|| string_path(record, &["receipt", "movement"]))
            .map(|state| receipt_lifecycle_state(Some(&state))),
        static_limit_kind: string_path(record, &["static_limit_kind"]),
        static_limit_detail: string_path(record, &["static_limit_detail"]),
        repair_command: None,
        static_recheck_command,
        agent_packet_command: Some(format!(
            "ripr agent packet --root {} --gap-ledger {} --gap-id {} --json > {}",
            shell_arg(&options.command_root()),
            options.anchored_arg(&options.gap_ledger),
            shell_arg(&gap_id),
            // Issue #3872: the shell redirect anchors at --root like every
            // other funnel redirect, so the pasted packet command reproduces
            // the validated write location from any working directory (and
            // the derived PowerShell WriteAllText form inherits the anchor).
            shell_arg(&anchored_redirect_target(
                &options.command_root(),
                &options.agent_packet
            ))
        )),
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
        (Some("typescript"), "MissingBoundaryAssertion")
        | (Some("typescript"), "MissingValueAssertion")
        | (Some("typescript"), "MissingErrorDiscriminator") => {
            "Static evidence found related TypeScript test context, but the current proof is weak because the discriminator is missing.".to_string()
        }
        (Some("typescript"), "MissingSideEffectObserver") => {
            "Static evidence found related TypeScript test context, but the current proof does not observe the changed output or side effect.".to_string()
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
                Some("typescript") => "preview TypeScript",
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
        (Some("typescript"), "MissingBoundaryAssertion") => {
            "A related TypeScript test reaches this change, but no boundary discriminator was found for the changed behavior.".to_string()
        }
        (Some("typescript"), "MissingValueAssertion") => {
            "A related TypeScript test reaches this change, but no exact value assertion was found for the changed behavior.".to_string()
        }
        (Some("typescript"), "MissingErrorDiscriminator") => {
            "A related TypeScript test reaches this error path, but no error discriminator was found for the changed behavior.".to_string()
        }
        (Some("typescript"), "MissingSideEffectObserver") => {
            "A related TypeScript test reaches this change, but no output or side-effect observer was found for the changed behavior.".to_string()
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
                Some("typescript") => "preview TypeScript",
                Some("rust") | None => "stable Rust",
                Some(other) => other,
            };
            format!(
                "The gap ledger marked this PR-local {language_label} gap as repairable and policy-targeted."
            )
        }
    }
}

/// Shell words of a single `ripr ...` command, read with POSIX quoting (the
/// `shell_arg` form every receipt command is rendered in), each with whether
/// any part of it was quoted. `None` for a command this reader cannot take literally:
/// unbalanced quotes, a trailing escape, another program, or an unquoted
/// shell operator (a redirect or a chained command writes elsewhere).
fn ripr_command_words(command: &str) -> Option<Vec<(String, bool)>> {
    let mut words: Vec<(String, bool)> = Vec::new();
    let mut current = String::new();
    let mut quoted_word = false;
    let mut in_word = false;
    let mut quote: Option<char> = None;
    let mut chars = command.chars();
    while let Some(character) = chars.next() {
        match (quote, character) {
            (Some(active), value) if value == active => quote = None,
            (Some('"'), '\\') => current.push(chars.next()?),
            (Some(_), value) => current.push(value),
            (None, '\'' | '"') => {
                quote = Some(character);
                quoted_word = true;
                in_word = true;
            }
            (None, '\\') => {
                current.push(chars.next()?);
                in_word = true;
            }
            (None, value) if value.is_whitespace() => {
                if in_word {
                    words.push((std::mem::take(&mut current), quoted_word));
                }
                quoted_word = false;
                in_word = false;
            }
            (None, value) => {
                current.push(value);
                in_word = true;
            }
        }
    }
    if quote.is_some() {
        return None;
    }
    if in_word {
        words.push((current, quoted_word));
    }
    if words.first().map(|(word, _)| word.as_str()) != Some("ripr") {
        return None;
    }
    let operator = words.iter().any(|(word, quoted)| {
        !quoted && matches!(word.as_str(), ">" | ">>" | "|" | "&&" | "||" | ";")
    });
    (!operator).then_some(words)
}

/// Value of the last unquoted `flag` in a ripr command's words.
fn ripr_flag_value<'a>(words: &'a [(String, bool)], flag: &str) -> Option<&'a str> {
    words
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, (word, quoted))| !quoted && word == flag)
        .find_map(|(index, _)| words.get(index + 1).map(|(value, _)| value.as_str()))
}

/// The file a printed `ripr receipt write` command writes: its `--out`, or
/// the receipt writer's own default for its `--gap` when it names no
/// `--out`. `None` when the command cannot be read literally.
fn receipt_command_out_path(command: &str) -> Option<String> {
    let words = ripr_command_words(command)?;
    if words
        .iter()
        .any(|(word, quoted)| !quoted && word == "--out")
    {
        // A flag with no usable value fails in the CLI; name no path for it.
        return ripr_flag_value(&words, "--out")
            .filter(|out| !out.trim().is_empty())
            .map(str::to_string);
    }
    let is_receipt_write = words.get(1).map(|(word, _)| word.as_str()) == Some("receipt")
        && words.get(2).map(|(word, _)| word.as_str()) == Some("write");
    let gap = ripr_flag_value(&words, "--gap").filter(|gap| !gap.trim().is_empty())?;
    is_receipt_write.then(|| {
        crate::app::receipt::receipt_default_path(gap)
            .to_string_lossy()
            .replace('\\', "/")
    })
}

/// One source of truth for where the receipt lands: the file the printed
/// ledger receipt command writes (its `--out`, or the receipt writer's
/// default for its `--gap`), else a recorded path, else `None` so the
/// caller uses the first-pr default that its synthesized command then
/// writes. A path chosen independently of the printed command named a file
/// that command never writes (Python preview: `gap-pr-...targeted-test-outcome.json`
/// beside a `--out gap-python-....json`), so the command's own `--out` wins
/// even over a recorded path that disagrees with it.
fn selected_receipt_path(record: &Value, ledger_command: Option<&str>) -> Option<String> {
    ledger_command
        .and_then(receipt_command_out_path)
        .or_else(|| string_path(record, &["receipt_path"]))
        .or_else(|| string_path(record, &["receipt", "path"]))
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
    if uses_check_output_gap_ledger(root) {
        return regenerate_check_output_gap_ledger_command(options);
    }
    regenerate_repo_exposure_gap_ledger_command(options)
}

/// A blocked ledger could not use its input, so regenerating the ledger
/// alone reproduces the same state (onboarding re-walk 4: an empty
/// repo-exposure file looped first-pr on the ledger command). A Rust root
/// regenerates the repo-exposure input first; the check-output form already
/// reruns `ripr check` unless the caller supplied its own check output.
fn regenerate_blocked_gap_ledger_command(root: &Path, options: &FirstPrOptions) -> String {
    if uses_check_output_gap_ledger(root) {
        return regenerate_check_output_gap_ledger_command(options);
    }
    // Every path in the compound anchors at --root, like the redirect in its
    // first half: a cwd-relative read or write would split the retry across
    // directories when first-pr runs with a --root other than the cwd.
    format!(
        "{} && {}",
        regenerate_repo_exposure_command(&options.command_root()),
        regenerate_repo_exposure_gap_ledger_command(options)
    )
}

fn uses_check_output_gap_ledger(root: &Path) -> bool {
    !root.join("Cargo.toml").is_file()
        && (detect_python_project(root) || detect_typescript_project(root))
}

/// `ripr reports gap-ledger` resolves `--repo-exposure`, `--out` and
/// `--out-md` against its working directory, so each anchors at the bound
/// root that `--root` names (#3948, #4287).
fn regenerate_repo_exposure_gap_ledger_command(options: &FirstPrOptions) -> String {
    format!(
        "ripr reports gap-ledger --root {} --repo-exposure {} --out {} --out-md {}",
        shell_arg(&options.command_root()),
        options.anchored_arg(DEFAULT_REPO_EXPOSURE),
        options.anchored_arg(&options.gap_ledger),
        options.anchored_arg(&with_extension(&options.gap_ledger, "md"))
    )
}

fn regenerate_check_output_gap_ledger_command(options: &FirstPrOptions) -> String {
    check_output_gap_ledger_command(options, options.check_output.is_none(), false)
}

/// Rewrites the check output (the supplied `--check-output` path when there
/// is one) before rebuilding the ledger from it. This is the stale-evidence
/// refresh after a test or source edit, and that edit is usually still
/// uncommitted: plain `ripr check` reads each file as committed at HEAD, so
/// the refresh reads the working tree (`--worktree`), or it would select the
/// gap the edit just closed again (MCP agent walk, 2026-09-29).
fn rerun_check_output_gap_ledger_command(options: &FirstPrOptions) -> String {
    check_output_gap_ledger_command(options, true, true)
}

/// The static re-check after verify (MCP agent walk, 2026-09-29): on the
/// check-output route the receipt records only the verify status it is given,
/// so this reruns check over the working tree, where the test edit usually
/// still is, and compares it with the check report the gap came from.
///
/// That report is the one the ledger names as its input
/// (`inputs.records` with `inputs.source_kind == "check_output"`), never a
/// default path that merely exists: comparing against a report that did not
/// produce the selected gap would show movement that is not the edit's. The
/// command is omitted when the ledger names no check-output input, when it
/// disagrees with a supplied `--check-output`, or when the report is absent.
fn static_recheck_command(
    gap_ledger: &Value,
    root: &Path,
    options: &FirstPrOptions,
) -> Option<String> {
    if !uses_check_output_gap_ledger(root) {
        return None;
    }
    if string_path(gap_ledger, &["inputs", "source_kind"]).as_deref() != Some("check_output") {
        return None;
    }
    let before_raw =
        string_path(gap_ledger, &["inputs", "records"]).filter(|path| !path.trim().is_empty())?;
    let before_path = resolve_path(root, &before_raw);
    if let Some(supplied) = options.check_output.as_deref()
        && resolve_path(root, supplied) != before_path
    {
        return None;
    }
    if !before_path.is_file() {
        return None;
    }
    let after_raw = match before_raw.strip_suffix(".json") {
        Some(stem) => format!("{stem}.after.json"),
        None => format!("{before_raw}.after.json"),
    };
    let before = options.anchored_arg(&before_raw);
    let after = options.anchored_arg(&after_raw);
    Some(format!(
        "ripr check --root {} --base {} --worktree --json > {after} && ripr outcome --before {before} --after {after}",
        shell_arg(&options.command_root()),
        shell_arg(&options.base),
    ))
}

fn check_output_gap_ledger_command(
    options: &FirstPrOptions,
    rerun_check: bool,
    worktree: bool,
) -> String {
    let root = shell_arg(&options.command_root());
    let base = shell_arg(&options.base);
    let check_output_raw = options
        .check_output
        .as_deref()
        .unwrap_or(DEFAULT_CHECK_OUTPUT);
    let check_output = options.anchored_arg(check_output_raw);
    let out = options.anchored_arg(&options.gap_ledger);
    let out_md = options.anchored_arg(&with_extension(&options.gap_ledger, "md"));
    let ledger = format!(
        "ripr reports gap-ledger --check-output {check_output} --root {root} --out {out} --out-md {out_md}"
    );
    if !rerun_check {
        return ledger;
    }
    // The shell redirect target anchors at --root (issue #3872) so the
    // pasted compound reproduces the validated write location from any
    // working directory. The paired --check-output read names the same
    // anchored file: a relative read next to an absolute write would
    // split the compound across directories when pasted elsewhere.
    let worktree = if worktree { " --worktree" } else { "" };
    format!("ripr check --root {root} --base {base}{worktree} --json > {check_output} && {ledger}")
}

fn detect_typescript_project(root: &Path) -> bool {
    ["package.json", "tsconfig.json", "jsconfig.json"]
        .iter()
        .any(|marker| root.join(marker).is_file())
        || ["src", "tests"]
            .iter()
            .any(|marker| dir_contains_typescript_source(&root.join(marker)))
}

fn dir_contains_typescript_source(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if matches!(name, ".git" | "target" | "node_modules" | "dist" | "build") {
                continue;
            }
            if dir_contains_typescript_source(&path) {
                return true;
            }
        } else if file_type.is_file() && is_typescript_source_file(&path) {
            return true;
        }
    }
    false
}

fn is_typescript_source_file(path: &Path) -> bool {
    // #4116: consume the shared TS/JS extension authority so .mts/.cts
    // first-use sources are detected exactly like .ts/.tsx.
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            crate::analysis::ts_js_source_kind(extension)
                == Some(crate::analysis::TsJsSourceKind::TypeScript)
        })
}

fn regenerate_repo_exposure_command(root: &str) -> String {
    check_repo_exposure_command(root, "instant", DEFAULT_REPO_EXPOSURE)
}

fn git_worktree_available_with_ceiling(
    root: &Path,
    ceiling: Option<&Path>,
) -> Result<bool, String> {
    git_success_with_ceiling(root, &["rev-parse", "--is-inside-work-tree"], ceiling)
}

fn git_rev_exists(root: &Path, rev: &str) -> Result<bool, String> {
    let commit = format!("{rev}^{{commit}}");
    git_success(root, &["rev-parse", "--verify", "--quiet", &commit])
}

fn git_diff_range_valid(root: &Path, base: &str, head: &str) -> Result<(), String> {
    // Validity probe only (#4006): the exit status is the entire signal —
    // stdout is never parsed, so no path identity flows from this call. It
    // still passes `-z` so the grammar is unambiguous and no C-quoted
    // rendering is ever produced on this route.
    let range = format!("{base}...{head}");
    let output = Command::new("git")
        .arg("diff")
        .arg("--name-only")
        .arg("-z")
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
    git_success_with_ceiling(root, args, None)
}

fn git_success_with_ceiling(
    root: &Path,
    args: &[&str],
    ceiling: Option<&Path>,
) -> Result<bool, String> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(root);
    if let Some(c) = ceiling {
        cmd.env("GIT_CEILING_DIRECTORIES", c);
    }
    let output = cmd
        .output()
        .map_err(|err| format!("failed to run git: {err}"))?;
    Ok(output.status.success())
}

fn fetch_base_command(options: &FirstPrOptions) -> String {
    if let Some(branch) = options.base.strip_prefix("origin/") {
        format!(
            "git -C {} fetch origin {}",
            shell_arg(&options.command_root()),
            shell_arg(branch)
        )
    } else {
        format!(
            "git -C {} fetch --all --prune",
            shell_arg(&options.command_root())
        )
    }
}

fn verify_ref_command(options: &FirstPrOptions, rev: &str) -> String {
    format!(
        "git -C {} rev-parse --verify {}",
        shell_arg(&options.command_root()),
        shell_arg(&format!("{rev}^{{commit}}"))
    )
}

/// Human-facing suggested command (#4006 named non-claim): this string is
/// rendered into a blocked-selection message for the operator to read and
/// run — it is never executed by ripr and its output is never machine-parsed,
/// so no path identity flows through it. It intentionally stays
/// human-readable (no `-z`): NUL-delimited output is a machine grammar, and
/// suggesting it to a human reader would degrade the message it lives in.
fn diff_range_command(options: &FirstPrOptions) -> String {
    format!(
        "git -C {} diff --name-only --no-ext-diff {}",
        shell_arg(&options.command_root()),
        shell_arg(&format!("{}...{}", options.base, options.head))
    )
}

fn unshallow_command(options: &FirstPrOptions) -> String {
    format!(
        "git -C {} fetch --unshallow",
        shell_arg(&options.command_root())
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

/// Join a possibly relative path onto the command root, without carrying the
/// root's own `.` into every path this command prints.
///
/// `ripr first-pr --root .` makes `root` end in a `CurDir` component, so a
/// plain `root.join(...)` renders as `/repo/./target/ripr/reports/start-here.md`
/// in the `Start here:`, `Artifacts:` and `Wrote` lines — a path nobody would
/// type, on the command whose whole job is handing a reader a file to open.
/// Collecting the components drops the interior `CurDir` while keeping both a
/// leading `./`, which other surfaces already render, and every `ParentDir`:
/// dropping a `..` would name a different directory. The filesystem target is
/// unchanged either way.
fn resolve_path(root: &Path, path: &str) -> PathBuf {
    let candidate = Path::new(path);
    let joined = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };
    let resolved: PathBuf = joined.components().collect();
    // A path made only of `.` components keeps one `CurDir` and collects back
    // to `.`, so the fallback is for the empty-path case alone, which would
    // name nothing at all.
    if resolved.as_os_str().is_empty() {
        return PathBuf::from(".");
    }
    resolved
}

fn normalized_path(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

/// The selected root first-pr displays are rendered against: `--root` as
/// the invocation spelled it, which `loop_commands::bound_root` resolves
/// once against this process's working directory — the same directory
/// `repo_root` resolved it against. Typed recovery binds to the same root so
/// a display and its spec cannot name different repositories (#3999).
fn display_selected_root(options: &FirstPrOptions) -> &Path {
    Path::new(&options.root)
}

fn repo_root() -> Result<PathBuf, String> {
    env::current_dir().map_err(|err| format!("failed to resolve current directory: {err}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn is_typescript_source_file_recognizes_modern_ts_extensions() {
        // #4116: first-use TypeScript-source detection consumes the shared
        // extension authority; .mts/.cts join .ts/.tsx, JavaScript-family
        // and near-miss extensions stay excluded, and `.d.ts` keeps its
        // accepted declaration-file routing via the "ts" extension.
        let cases = [
            ("src/a.ts", true),
            ("src/a.tsx", true),
            ("src/a.mts", true),
            ("src/a.cts", true),
            ("src/a.d.ts", true),
            ("src/a.js", false),
            ("src/a.jsx", false),
            ("src/a.mjs", false),
            ("src/a.cjs", false),
            ("src/a.mt", false),
            ("src/a.mjsx", false),
            ("src/a.ctsx", false),
            ("Makefile", false),
        ];

        for (path, expected) in cases {
            assert_eq!(
                super::is_typescript_source_file(Path::new(path)),
                expected,
                "{path}"
            );
        }
    }

    #[test]
    fn resolve_path_drops_the_roots_own_cur_dir() {
        // `--root .` used to render every emitted artifact as
        // `/repo/./target/...`. The rendered path is now the one a reader
        // would type, and names the same file.
        assert_eq!(
            resolve_path(Path::new("/repo/."), "target/ripr/reports/start-here.md"),
            PathBuf::from("/repo/target/ripr/reports/start-here.md")
        );
        // A leading `./` is kept: `Components` only drops interior `CurDir`,
        // so the existing relative rendering that other surfaces already emit
        // is untouched and this change stays confined to the joined case.
        assert_eq!(
            resolve_path(Path::new("."), "target/ripr/reports"),
            PathBuf::from("./target/ripr/reports")
        );
        // An absolute argument still wins over the root, unchanged.
        assert_eq!(
            resolve_path(Path::new("/repo/."), "/elsewhere/out"),
            PathBuf::from("/elsewhere/out")
        );
        // Discriminator: `..` is not a `.`. Dropping it would name a different
        // directory, so it survives.
        assert_eq!(
            resolve_path(Path::new("/repo/."), "../shared/out"),
            PathBuf::from("/repo/../shared/out")
        );
    }

    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// The selected root as first-pr binds it into generated commands (#4000).
    fn bound_arg(root: &str) -> String {
        shell_arg(&crate::agent::loop_commands::bound_root(root))
    }

    #[test]
    fn first_pr_check_missing_packet_error_explains_validate_only_mode() {
        let mut options = FirstPrOptions {
            check_output: Some("target/ripr/foo/check.json".to_string()),
            out_dir: "target/ripr/foo/reports".to_string(),
            ..FirstPrOptions::default()
        };
        options.check = true;
        let err = first_pr_missing_packet_recovery_error(
            Path::new("/repo/target/ripr/foo/reports/start-here.json"),
            Path::new("/repo/target/ripr/foo/reports/start-here.md"),
            &options,
            Path::new("/repo/target/ripr/foo/reports"),
            None,
        );

        assert!(err.contains("first-pr --check validates an existing start-here packet"));
        assert!(err.contains("it does not create one"));
        assert!(err.contains("Missing:\n  /repo/target/ripr/foo/reports/start-here.json"));
        assert!(err.contains("--check-output target/ripr/foo/check.json"));
        assert!(err.contains("--out-dir /repo/target/ripr/foo/reports"));
        assert!(!err.contains("--out-dir target/ripr/foo/reports"));
        assert!(!err.trim_end().ends_with(" --check"));
    }

    #[test]
    fn first_pr_write_command_renders_only_an_explicit_base() {
        // #4285: an omitted base is resolved by the write run itself.
        let out = Path::new("target/ripr/reports");
        assert!(!first_pr_write_command(&FirstPrOptions::default(), out, false).contains("--base"));
        let explicit = FirstPrOptions {
            base: "upstream/trunk".to_string(),
            base_explicit: true,
            ..FirstPrOptions::default()
        };
        assert!(
            first_pr_write_command(&explicit, out, false).contains("--base upstream/trunk --head")
        );
        // Nothing resolves: the command demands a base instead of guessing one.
        assert!(
            first_pr_write_command(&FirstPrOptions::default(), out, true)
                .contains("--base <ref> --head")
        );
    }

    #[test]
    fn first_pr_write_command_preserves_explicit_gap_ledger_only() {
        let implicit = FirstPrOptions::default();
        assert!(
            !first_pr_write_command(&implicit, Path::new("target/ripr/reports"), false)
                .contains("--gap-ledger")
        );

        let explicit = FirstPrOptions {
            gap_ledger: "target/custom/gaps.json".to_string(),
            gap_ledger_explicit: true,
            ..FirstPrOptions::default()
        };
        assert!(
            first_pr_write_command(&explicit, Path::new("target/ripr/reports"), false)
                .contains("--gap-ledger target/custom/gaps.json")
        );
    }

    #[test]
    fn first_pr_write_command_renders_resolved_out_dir() {
        let options = FirstPrOptions::default();
        // Mixed-case anchored path: proves the resolved directory renders
        // verbatim (no CWD-relative fallback, no separator or case folding).
        let rendered = first_pr_write_command(&options, Path::new("/Repo/out/Reports"), false);
        assert!(rendered.contains("--out-dir /Repo/out/Reports"));
        assert!(!rendered.contains("--out-dir target/ripr/reports"));
    }

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
        assert!(parsed.base_explicit);
        assert!(!FirstPrOptions::default().base_explicit);
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
        match parse_options(&["--gap-ledgr".to_string()]) {
            Err(unknown)
                if unknown.contains("Did you mean `--gap-ledger`?")
                    && unknown.contains("Run `ripr first-pr --help`.") => {}
            other => {
                return Err(format!(
                    "first-pr typo must suggest --gap-ledger from its help body, got {other:?}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn first_pr_help_pins_start_here_language() {
        let help = FIRST_PR_HELP;
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
                .is_some_and(|command| command.starts_with("ripr receipt write --gap "))
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

    /// A receipt command printed as the step after verify records
    /// `--status not_run`; the line after it says which value carries the
    /// verify outcome, on the CLI summary and in the start-here markdown.
    #[test]
    fn receipt_after_verify_names_the_status_that_carries_the_verify_outcome() {
        let command = "ripr receipt write --gap 'gap:python:p.py:f' --verify-command 'python -m pytest tests/test_p.py::test_f' --status not_run --out target/ripr/receipts/g.json";
        let packet = json!({
            "status": "actionable",
            "selected": {
                "state": "top_gap",
                "kind": "MissingBoundaryAssertion",
                "verify_command": "python -m pytest tests/test_p.py::test_f",
                "receipt_command": command,
                "receipt_path": "target/ripr/receipts/g.json",
            },
        });
        let step = format!("{RECEIPT_STATUS_LABEL}: {RECEIPT_STATUS_STEP}\n");
        let summary = start_here_cli_summary(
            &packet,
            Path::new("target/ripr/reports/start-here.json"),
            Path::new("target/ripr/reports/start-here.md"),
        );
        assert!(
            summary.contains(&format!("Receipt after verify: `{command}`\n{step}")),
            "{summary}"
        );
        let markdown = render_start_here_markdown(&packet);
        assert!(
            markdown.contains(&format!("- Receipt after verify: `{command}`\n- {step}")),
            "{markdown}"
        );
        assert!(RECEIPT_STATUS_STEP.contains("`--status passed`"));
        assert!(RECEIPT_STATUS_STEP.contains("`--status failed`"));

        for other in [
            "ripr outcome --before b.json --after a.json --format json --out o.json",
            "ripr receipt write --gap g --verify-command x --status passed",
            "ripr receipt write --gap 'x --status not_run' --verify-command x --status failed",
            // Quoted text is a value, not the flag (Devin review on #4485).
            "ripr receipt write --gap 'x --status not_run ' --verify-command x --status failed",
            "ripr receipt write --gap \"a --status not_run b\" --status passed",
        ] {
            assert_eq!(receipt_status_step(other), None, "{other}");
        }
    }

    /// The start-here markdown must present the receipt command for both
    /// shells: the bash form stays byte-identical, and the PowerShell form
    /// round-trips an embedded quote through PowerShell's doubled-quote idiom
    /// via the shared `powershell_command` translation (#2628).
    #[test]
    fn start_here_markdown_offers_receipt_command_powershell_variant() -> Result<(), String> {
        let packet = json!({
            "status": "actionable",
            "selected": {
                "state": "top_gap",
                "kind": "MissingBoundaryAssertion",
                "receipt_command": "ripr receipt write --gap 'it'\\''s' --verify-command 'cargo test' --status not_run",
            },
        });
        let markdown = render_start_here_markdown(&packet);

        let bash_form = "Receipt after verify:\n`ripr receipt write --gap 'it'\\''s' --verify-command 'cargo test' --status not_run`\n\n";
        assert!(
            markdown.contains(bash_form),
            "bash receipt command drifted:\n{markdown}"
        );
        let powershell_form = "Receipt after verify (PowerShell):\n`ripr receipt write --gap 'it''s' --verify-command 'cargo test' --status not_run`";
        assert!(
            markdown.contains(powershell_form),
            "powershell receipt command missing or drifted:\n{markdown}"
        );
        let bash_label = markdown
            .find("Receipt after verify:\n")
            .ok_or_else(|| format!("bash receipt label must exist: {markdown}"))?;
        let powershell_label = markdown
            .find("Receipt after verify (PowerShell):\n")
            .ok_or_else(|| format!("powershell receipt label must exist: {markdown}"))?;
        assert!(
            bash_label < powershell_label,
            "bash form must be presented before the PowerShell variant"
        );
        assert!(
            markdown.contains("The first form is written for Bash; cmd.exe is not supported."),
            "receipt presentation must state the cmd.exe boundary:\n{markdown}"
        );
        Ok(())
    }

    /// The start-here markdown must present the verify and agent packet
    /// commands for both shells (#2628): the bash forms stay byte-identical,
    /// the PowerShell forms round-trip an embedded quote (verify) and the
    /// packet redirect (agent packet) through the shared `powershell_command`
    /// translation, and each block states the cmd.exe boundary.
    #[test]
    fn start_here_markdown_offers_verify_and_agent_packet_powershell_variants() -> Result<(), String>
    {
        let packet = json!({
            "status": "actionable",
            "selected": {
                "state": "top_gap",
                "kind": "MissingBoundaryAssertion",
                "verify_command": "cargo test 'it'\\''s'",
                "agent_packet_command": "ripr agent packet --root 'repo root' --gap-id gap:pr:pricing --json > target/ripr/workflow/agent-packet.json",
            },
        });
        let markdown = render_start_here_markdown(&packet);

        let bash_verify = "Verify after the test edit:\n`cargo test 'it'\\''s'`\n\n";
        assert!(
            markdown.contains(bash_verify),
            "bash verify command drifted:\n{markdown}"
        );
        let powershell_verify = "Verify after the test edit (PowerShell):\n`cargo test 'it''s'`";
        assert!(
            markdown.contains(powershell_verify),
            "powershell verify command missing or drifted:\n{markdown}"
        );
        let bash_packet = "Agent packet command:\n`ripr agent packet --root 'repo root' --gap-id gap:pr:pricing --json > target/ripr/workflow/agent-packet.json`\n\n";
        assert!(
            markdown.contains(bash_packet),
            "bash agent packet command drifted:\n{markdown}"
        );
        let powershell_packet = "Agent packet command (PowerShell):\n`$riprEncoding = [Console]::OutputEncoding; try { [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false) } catch {}; try { $ripr = ((ripr agent packet --root 'repo root' --gap-id gap:pr:pricing --json) | Out-String) } finally { try { [Console]::OutputEncoding = $riprEncoding } catch {} }; if ($LASTEXITCODE -eq 0) { [System.IO.File]::WriteAllText($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath('target/ripr/workflow/agent-packet.json'), $ripr.Replace(\"`r`n\", \"`n\"), [System.Text.UTF8Encoding]::new($false)) } else { throw \"ripr exited with code $LASTEXITCODE\" }`";
        assert!(
            markdown.contains(powershell_packet),
            "powershell agent packet command missing or drifted:\n{markdown}"
        );
        let bash_verify_at = markdown
            .find(bash_verify)
            .ok_or_else(|| format!("bash verify label must exist: {markdown}"))?;
        let powershell_verify_at = markdown
            .find(powershell_verify)
            .ok_or_else(|| format!("powershell verify label must exist: {markdown}"))?;
        let bash_packet_at = markdown
            .find(bash_packet)
            .ok_or_else(|| format!("bash agent packet label must exist: {markdown}"))?;
        let powershell_packet_at = markdown
            .find(powershell_packet)
            .ok_or_else(|| format!("powershell agent packet label must exist: {markdown}"))?;
        assert!(
            bash_verify_at < powershell_verify_at && bash_packet_at < powershell_packet_at,
            "bash form must be presented before the PowerShell variant:\n{markdown}"
        );
        assert_eq!(
            markdown
                .matches("The first form is written for Bash; cmd.exe is not supported.")
                .count(),
            2,
            "each presented block must state the cmd.exe boundary:\n{markdown}"
        );
        Ok(())
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
        // Issue #3872: the redirect target anchors at the resolved --root, so
        // the expectation builds the same anchored path instead of pinning a
        // machine directory.
        let expected_regeneration = format!(
            "ripr check --root {} --mode instant --format repo-exposure-json > {}",
            shell_arg(&crate::agent::loop_commands::bound_root(".")),
            shell_arg(&anchored_redirect_target(".", DEFAULT_REPO_EXPOSURE))
        );
        assert!(
            packet["selected"]["regeneration_command"]
                .as_str()
                .is_some_and(|command| command == expected_regeneration)
        );
        let summary = start_here_cli_summary(
            &packet,
            Path::new("target/ripr/reports/start-here.json"),
            Path::new("target/ripr/reports/start-here.md"),
        );
        assert!(summary.contains(
            "Missing artifact: Repo exposure report at `target/ripr/reports/repo-exposure.json`"
        ));
        assert!(summary.contains(&format!(
            "Regeneration command: `ripr check --root {} --mode instant",
            shell_arg(&crate::agent::loop_commands::bound_root("."))
        )));
        assert_eq!(packet["selected"]["also_missing"][0]["id"], "gap_ledger");
        assert_eq!(
            packet["selected"]["also_missing"][0]["path"],
            DEFAULT_GAP_LEDGER
        );
        assert!(
            packet["selected"]["also_missing"][0]["regeneration_command"]
                .as_str()
                .is_some_and(|command| command.contains("ripr reports gap-ledger"))
        );
        assert!(summary.contains(
            "Also missing: Gap decision ledger at `target/ripr/reports/gap-decision-ledger.json`"
        ));
        assert!(summary.contains("Then run: `ripr reports gap-ledger"));
        assert!(
            summary.find("Regeneration command:") < summary.find("Also missing:"),
            "repo exposure recovery must precede the dependent ledger: {summary}"
        );
        let markdown = render_start_here_markdown(&packet);
        assert!(markdown.contains("- Also missing: Gap decision ledger"));
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
        let expected_regeneration = format!(
            "ripr check --root {} --mode instant --format repo-exposure-json > {}",
            shell_arg(&crate::agent::loop_commands::bound_root(".")),
            shell_arg(&anchored_redirect_target(".", DEFAULT_REPO_EXPOSURE))
        );
        assert!(
            packet["selected"]["regeneration_command"]
                .as_str()
                .is_some_and(|command| command == expected_regeneration)
        );
        cleanup(&repo)
    }

    #[test]
    fn missing_repo_exposure_uses_existing_latency_report_before_rerun() -> Result<(), String> {
        // 0.2 is what `repo-exposure-latency-report` writes since #3864.
        for schema_version in ["0.1", "0.2"] {
            existing_latency_timeout_report_is_used(schema_version)?;
        }
        Ok(())
    }

    fn existing_latency_timeout_report_is_used(schema_version: &str) -> Result<(), String> {
        let repo = temp_repo(&format!(
            "first-pr-existing-latency-timeout-{schema_version}"
        ))?;
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
                "schema_version": schema_version,
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
        assert!(packet["selected"].get("also_missing").is_none());
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
        // Issue #3872: both the redirect target and the paired --check-output
        // read name the same anchored file.
        let bound = bound_arg(".");
        let anchored_check = shell_arg(&anchored_redirect_target(".", DEFAULT_CHECK_OUTPUT));
        assert!(command.contains(&format!(
            "ripr check --root {bound} --base origin/main --json > {anchored_check}"
        )));
        // #4287: the ledger outputs anchor at --root as well.
        assert!(command.contains(&format!(
            "ripr reports gap-ledger --check-output {anchored_check} --root {bound} --out {} --out-md {}",
            shell_arg(&anchored_redirect_target(".", DEFAULT_GAP_LEDGER)),
            shell_arg(&anchored_redirect_target(
                ".",
                &with_extension(DEFAULT_GAP_LEDGER, "md")
            ))
        )));
        assert!(!command.contains("--repo-exposure"));
        assert_eq!(packet["commands"]["regenerate_gap_ledger"], command);
        assert_eq!(packet["artifacts"][0]["regeneration_command"], command);
        check_first_pr(&repo, &options)?;
        cleanup(&repo)
    }

    /// FIX #1617 slice 2: a missing-artifact selection carries the typed
    /// regeneration spec beside the legacy string only when the display is
    /// a simple canonical route; the compound `&&` route stays
    /// legacy-string-only, and the `commands` map stays string-only.
    #[test]
    fn missing_gap_ledger_selection_carries_typed_spec_only_for_simple_routes() -> Result<(), String>
    {
        let repo = temp_python_repo("first-pr-python-ledger-spec-recovery")?;

        let simple_options = FirstPrOptions {
            check_output: Some(DEFAULT_CHECK_OUTPUT.to_string()),
            ..FirstPrOptions::default()
        };
        let simple = missing_gap_ledger_selection(&repo, &simple_options);
        let simple_json = simple.to_json();
        let command = simple_json["regeneration_command"]
            .as_str()
            .ok_or("missing-artifact selection must keep the legacy regeneration command")?;
        assert!(
            command.starts_with("ripr reports gap-ledger --check-output"),
            "unexpected simple bridge command: {command}"
        );
        let spec = simple_json
            .get("regeneration_command_spec")
            .ok_or("a simple route display must carry a typed regeneration spec")?;
        assert_eq!(spec["command_id"], "ripr:reports:gap-ledger");
        assert_eq!(spec["role"], "regeneration");
        assert_eq!(spec["execution_mode"], "direct");
        assert_eq!(spec["expected_writes"][0], DEFAULT_GAP_LEDGER);
        let commands = simple.commands_json(&repo, &simple_options);
        assert!(
            commands["next"].is_string(),
            "the commands map must stay legacy-string-only"
        );

        let compound = missing_gap_ledger_selection(&repo, &FirstPrOptions::default());
        let compound_json = compound.to_json();
        assert!(
            compound_json.get("regeneration_command_spec").is_none(),
            "a compound && route must stay legacy-string-only"
        );
        assert!(
            compound_json["regeneration_command"]
                .as_str()
                .is_some_and(|command| command.contains(" && ")),
            "expected the compound default bridge command"
        );

        let repo_exposure = missing_repo_exposure_selection(&repo, &FirstPrOptions::default());
        let repo_exposure_json = repo_exposure.to_json();
        let spec = repo_exposure_json
            .get("regeneration_command_spec")
            .ok_or("the repo-exposure route display must carry a typed spec")?;
        assert_eq!(spec["command_id"], "ripr:check:repo-exposure");
        assert_eq!(spec["execution_mode"], "shell_required");
        assert_eq!(
            spec["expected_writes"][0],
            "target/ripr/reports/repo-exposure.json"
        );

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
            format!("ripr doctor --root {}", bound_arg("missing-workspace"))
        );
        assert!(
            !repo.join("missing-workspace").exists(),
            "first-pr must not create a typo root while writing a recovery packet"
        );
        check_first_pr(&repo, &options)?;
        cleanup(&repo)
    }

    #[test]
    fn unsupported_language_root_is_no_action_not_wrong_root() -> Result<(), String> {
        // A Go repository is the right root: sending it to `--root` and
        // doctor was a loop with no exit.
        let repo = temp_repo("first-pr-go-root")?;
        let go_root = repo.join("go-service");
        fs::create_dir_all(go_root.join("pkg"))
            .map_err(|err| format!("mkdir {}: {err}", go_root.display()))?;
        fs::write(go_root.join("go.mod"), "module example.com/svc\n")
            .map_err(|err| format!("write go.mod: {err}"))?;
        fs::write(go_root.join("pkg/calc.go"), "package pkg\n")
            .map_err(|err| format!("write calc.go: {err}"))?;
        let options = FirstPrOptions {
            root: "go-service".to_string(),
            ..FirstPrOptions::default()
        };
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "no_action", "{packet}");
        assert_eq!(
            packet["selected"]["output_state"], "no_actionable_gap",
            "{packet}"
        );
        let text = packet.to_string();
        assert!(text.contains("Go (1 file(s))"), "{text}");
        assert!(!text.contains("Pass the repository root"), "{text}");

        // A nested Cargo crate below the Go root means `--root` should point
        // at that crate, so the recovery stays `wrong_root`.
        let nested = go_root.join("rust-core");
        fs::create_dir_all(nested.join("src"))
            .map_err(|err| format!("mkdir {}: {err}", nested.display()))?;
        fs::write(
            nested.join("Cargo.toml"),
            "[package]\nname = \"core\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write nested Cargo.toml: {err}"))?;
        fs::write(nested.join("src/lib.rs"), "pub fn f() -> i32 { 1 }\n")
            .map_err(|err| format!("write nested lib.rs: {err}"))?;
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked", "{packet}");
        assert_eq!(packet["selected"]["state"], "wrong_root", "{packet}");
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
            format!("ripr doctor --root {}", bound_arg("not-workspace"))
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
        let ceiling = repo.parent().map(Path::to_path_buf);
        let options = FirstPrOptions {
            git_ceiling: ceiling,
            ..FirstPrOptions::default()
        };
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "blocked_artifact");
        assert!(
            packet["selected"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("not a git worktree"))
        );
        assert_eq!(
            packet["selected"]["next_command"],
            format!("ripr doctor --root {}", bound_arg("."))
        );
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
            format!("git -C {} fetch origin missing-base", bound_arg("."))
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
            format!("git -C {} fetch --all --prune", bound_arg("."))
        );
        cleanup(&repo)
    }

    #[test]
    fn shallow_range_without_merge_base_points_at_unshallow() -> Result<(), String> {
        // #4538: a PR-style shallow fetch has both tips but no merge base.
        // The next command must be the unshallow repair, not the same
        // failing `git diff`.
        let origin = temp_repo("first-pr-shallow-origin")?;
        run_git_setup(&origin, &["checkout", "-q", "-b", "feature"])?;
        run_git_setup(&origin, &["commit", "-q", "--allow-empty", "-m", "feature"])?;
        run_git_setup(&origin, &["checkout", "-q", "-"])?;
        run_git_setup(
            &origin,
            &["commit", "-q", "--allow-empty", "-m", "main moves"],
        )?;
        let shallow = write_temp_root(&env::temp_dir(), "first-pr-shallow-clone")?;
        // `file://` keeps `--depth` honored (a plain path uses the local
        // transport); a Windows drive path needs the third slash.
        let origin_path = origin.display().to_string().replace('\\', "/");
        let url = if origin_path.starts_with('/') {
            format!("file://{origin_path}")
        } else {
            format!("file:///{origin_path}")
        };
        run_git_setup(&shallow, &["init", "-q"])?;
        run_git_setup(&shallow, &["remote", "add", "origin", &url])?;
        run_git_setup(
            &shallow,
            &[
                "fetch",
                "-q",
                "--depth=1",
                "origin",
                "+refs/heads/feature:refs/remotes/origin/feature",
                "+HEAD:refs/remotes/origin/main",
            ],
        )?;
        run_git_setup(&shallow, &["checkout", "-q", "--detach", "origin/feature"])?;
        write_json(
            &shallow.join(DEFAULT_GAP_LEDGER),
            ledger_with_repairable_gap(),
        )?;
        let options = FirstPrOptions::default();
        write_first_pr(&shallow, &options)?;
        let packet = read_packet(&shallow.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["selected"]["state"], "blocked_artifact");
        let message = packet["selected"]["message"].as_str().unwrap_or_default();
        assert!(
            message.contains("no merge base")
                && message.contains("shallow clone")
                && message.contains("fetch-depth: 0"),
            "unexpected message: {message}"
        );
        assert_eq!(
            packet["selected"]["next_command"],
            format!("git -C {} fetch --unshallow", bound_arg("."))
        );
        cleanup(&shallow)?;
        cleanup(&origin)
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
            format!(
                "git -C {} rev-parse --verify 'missing-head^{{commit}}'",
                bound_arg(".")
            )
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
            packet["selected"]["message"].as_str().is_some_and(
                |message| message.contains("origin/main...unrelated")
                    // #4538: the cause is named, and a full clone is not
                    // diagnosed as shallow.
                    && message.contains("unrelated histories")
                    && !message.contains("shallow clone")
            )
        );
        assert_eq!(
            packet["selected"]["next_command"],
            format!(
                "git -C {} diff --name-only --no-ext-diff origin/main...unrelated",
                bound_arg(".")
            )
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
        // The ledger could not use its input, so the retry regenerates that
        // input first; rerunning the ledger alone reproduced the blocked state
        // (onboarding re-walk 4, an empty repo-exposure file).
        let next = packet["selected"]["next_command"]
            .as_str()
            .unwrap_or_default();
        let (input, ledger) = next.split_once(" && ").unwrap_or_default();
        // #4287: both halves bind the same root, so `--root` never stays
        // `.` beside anchored paths.
        let exposure = anchored_redirect_target(".", DEFAULT_REPO_EXPOSURE);
        let bound = bound_arg(".");
        assert!(
            input
                == format!(
                    "ripr check --root {bound} --mode instant --format repo-exposure-json > {exposure}"
                ),
            "{next}"
        );
        // The ledger half reads the file the first half wrote and writes where
        // first-pr resolves the ledger, both anchored at --root.
        assert!(
            ledger
                == format!(
                    "ripr reports gap-ledger --root {bound} --repo-exposure {exposure} --out {} --out-md {}",
                    anchored_redirect_target(".", DEFAULT_GAP_LEDGER),
                    anchored_redirect_target(".", &with_extension(DEFAULT_GAP_LEDGER, "md"))
                ),
            "{next}"
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
        // Current review cards were read and none carries a repair start, so
        // no-action is a verdict on evidence first-pr actually saw.
        write_json(
            &repo.join(DEFAULT_REVIEW_COMMENTS),
            review_comments_report(Vec::new()),
        )?;
        let packet = render_start_here_packet(&repo, &FirstPrOptions::default());
        assert_eq!(packet["status"], "no_action");
        assert_eq!(packet["selected"]["state"], "no_action");
        assert_eq!(packet["selected"]["output_state"], "no_actionable_gap");
        cleanup(&repo)
    }

    /// #4224: a TypeScript finding whose repair packet failed closed used to
    /// leave the ledger empty (`blocked`), and first-pr looped on "refresh
    /// the first-run evidence". The ledger built from real check output now
    /// carries a non-delegatable static-limitation record, and first-pr names
    /// that limitation and target shape as advisory no-action.
    #[test]
    fn fail_closed_typescript_packet_names_limitation_instead_of_blocked() -> Result<(), String> {
        use crate::output::gap_decision_ledger::{
            GapDecisionLedgerInput, GapDecisionLedgerSourceKind, build_gap_decision_ledger_report,
            render_gap_decision_ledger_json,
        };
        let report = build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: ".".to_string(),
            generated_at: "test".to_string(),
            source_kind: GapDecisionLedgerSourceKind::CheckOutput,
            records_path: "check.json".to_string(),
            records_json: Ok(include_str!(
                "../../../../fixtures/ts_repair_packet_boundary_unreachable/expected/check.json"
            )
            .to_string()),
        });
        let ledger_json = render_gap_decision_ledger_json(&report)?;
        let repo = temp_repo("first-pr-ts-fail-closed")?;
        let ledger_value: Value =
            serde_json::from_str(&ledger_json).map_err(|err| format!("parse ledger: {err}"))?;
        assert_eq!(ledger_value["status"], "advisory", "{ledger_value}");
        write_json(&repo.join(DEFAULT_GAP_LEDGER), ledger_value)?;
        write_json(
            &repo.join(DEFAULT_REVIEW_COMMENTS),
            review_comments_report(Vec::new()),
        )?;
        let packet = render_start_here_packet(&repo, &FirstPrOptions::default());
        assert_eq!(packet["status"], "no_action", "{packet}");
        assert_eq!(packet["selected"]["output_state"], "no_actionable_gap");
        let reason = packet["selected"]["reason"].as_str().unwrap_or_default();
        assert!(
            reason.contains(
                "Static limitation `typescript_repair_packet_not_delegatable` at `src/auth.ts:2`"
            ),
            "{reason}"
        );
        assert!(
            reason.contains("does not reach the missing discriminator `user.length == 3`"),
            "{reason}"
        );
        assert!(
            reason.contains(
                "Target shape (not delegatable): Add an exact boundary assertion for `user.length == 3`."
            ),
            "{reason}"
        );
        assert!(
            !reason.contains("refresh the first-run evidence"),
            "{reason}"
        );
        assert!(packet["commands"].get("agent_packet").is_none(), "{packet}");
        cleanup(&repo)
    }

    /// #4216: a weakly exposed Python finding without a repair card used to
    /// leave first-pr at a bare generic "no actionable gap". The ledger now
    /// carries a non-delegatable static-limitation record, and first-pr names
    /// why no card exists and the manual step, as advisory no-action.
    #[test]
    fn python_finding_without_repair_card_names_limitation_and_manual_step() -> Result<(), String> {
        use crate::output::gap_decision_ledger::{
            GapDecisionLedgerInput, GapDecisionLedgerSourceKind, build_gap_decision_ledger_report,
            render_gap_decision_ledger_json,
        };
        let report = build_gap_decision_ledger_report(GapDecisionLedgerInput {
            root: ".".to_string(),
            generated_at: "test".to_string(),
            source_kind: GapDecisionLedgerSourceKind::CheckOutput,
            records_path: "check.json".to_string(),
            records_json: Ok(include_str!(
                "../../../../fixtures/python_rebound_constant_boundary_limit/expected/check.json"
            )
            .to_string()),
        });
        let ledger_json = render_gap_decision_ledger_json(&report)?;
        let repo = temp_repo("first-pr-python-no-card")?;
        let ledger_value: Value =
            serde_json::from_str(&ledger_json).map_err(|err| format!("parse ledger: {err}"))?;
        write_json(&repo.join(DEFAULT_GAP_LEDGER), ledger_value)?;
        write_json(
            &repo.join(DEFAULT_REVIEW_COMMENTS),
            review_comments_report(Vec::new()),
        )?;
        let packet = render_start_here_packet(&repo, &FirstPrOptions::default());
        assert_eq!(packet["status"], "no_action", "{packet}");
        assert_eq!(packet["selected"]["output_state"], "no_actionable_gap");
        let reason = packet["selected"]["reason"].as_str().unwrap_or_default();
        assert!(
            reason.contains(
                "Static limitation `python_repair_card_unavailable` at `src/pricing.py:10`: this Python preview finding has no repair card (static evidence names no concrete missing discriminator)"
            ),
            "{reason}"
        );
        assert!(
            reason.contains("add or strengthen a test by hand, then rerun `ripr check`"),
            "{reason}"
        );
        assert!(packet["commands"].get("agent_packet").is_none(), "{packet}");
        cleanup(&repo)
    }

    /// A carried repair start that no producer builds from the card's seam
    /// id: the non-default `--root` makes any rebuilt command differ.
    const CARD_REPAIR_COMMAND: &str =
        "ripr agent repair --root crates/pricing --seam-id seam-b --phase before";

    /// A ledger whose only actionable record is repo-scoped, as generated CI
    /// builds it from repo-exposure: first-pr selects no top gap from it.
    fn ledger_with_repo_scoped_gap_only() -> Value {
        json!({
            "schema_version": "0.1",
            "kind": "gap_decision_ledger",
            "records": [
                {
                    "gap_id": "gap:repo:pricing",
                    "language": "rust",
                    "language_status": "stable",
                    "scope": "repo_scoped",
                    "gap_state": "actionable",
                    "policy_state": "new",
                    "repairability": "repairable",
                    "repair_route": {"route_kind": "AddBoundaryAssertion"},
                    "verification_commands": ["cargo test -p pricing"]
                }
            ]
        })
    }

    /// One working-set review card; `repair_command: None` models a card
    /// that failed the repair-packet flip (the producer omits the field).
    fn review_card(seam: &str, repair_command: Option<&str>) -> Value {
        let mut guidance = json!({
            "command": format!("ripr agent brief --root . --seam-id {seam} --json"),
            "prompt": "Write one focused Rust test.",
            "verify_command": format!("ripr agent verify --root . --seam {seam} --json"),
        });
        if let Some(command) = repair_command {
            guidance["repair_command"] = json!(command);
        }
        json!({
            "id": format!("ripr-review-{seam}"),
            "canonical_gap_id": format!("gap:{seam}"),
            "seam_id": seam,
            "gap_state": "actionable",
            "kind": "predicate_boundary",
            "grip_class": "weakly_gripped",
            "oracle_kind": "exact_value",
            "oracle_strength": "strong",
            "owner": format!("pricing::{seam}"),
            "dedupe_key": format!("ripr:{seam}:src/pricing.rs:88"),
            "missing_discriminator": format!("{seam} == threshold"),
            "reason": format!("Static evidence names missing discriminator `{seam} == threshold`."),
            "receipt_command": format!("ripr agent receipt --root . --seam-id {seam} --json"),
            "seam": {"expression": format!("{seam} >= threshold"), "file": "src/pricing.rs", "line": 88},
            "suggested_test": {
                "assertion_shape": format!("assert_eq!(discounted_total({seam}), 90)"),
                "recommended_file": "tests/pricing.rs",
                "related_test": {"file": "tests/pricing.rs", "line": 12, "name": "above_threshold_gets_discount"}
            },
            "llm_guidance": guidance
        })
    }

    fn review_comments_report(comments: Vec<Value>) -> Value {
        json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "status": "advisory",
            "root": ".",
            "base": "origin/main",
            "head": "HEAD",
            "comments": comments,
            "summary_only": [],
            "suppressed": []
        })
    }

    #[test]
    fn review_card_selection_carries_optional_outcome_without_inventing_it() -> Result<(), String> {
        let options = FirstPrOptions::default();
        let command = "ripr check --root . --mode draft --format json > target/ripr/workflow/analysis-outcome.json";
        for supplied in [None, Some(command)] {
            let mut card = review_card(
                "seam-a",
                Some("ripr agent repair --root . --seam-id seam-a --phase before"),
            );
            if let Some(command) = supplied {
                card["llm_guidance"]["analysis_outcome_command"] = json!(command);
            }
            let top_gap = top_gap_from_review_card(&card, &options)
                .ok_or("old and new cards must retain top-gap selection")?;
            let selected = top_gap.to_json();
            let commands =
                Selection::TopGap(Box::new(top_gap)).commands_json(Path::new("."), &options);
            let markdown = render_start_here_markdown(&json!({"selected": selected}));
            match supplied {
                Some(command)
                    if selected
                        .get("analysis_outcome_command")
                        .and_then(Value::as_str)
                        == Some(command)
                        && commands.get("analysis_outcome").is_none()
                        && markdown.contains(command) => {}
                None if selected.get("analysis_outcome_command").is_none()
                    && commands.get("analysis_outcome").is_none()
                    && !markdown.contains(command) => {}
                _ => {
                    return Err(format!(
                        "optional outcome was lost or invented: {selected}, {commands}"
                    ));
                }
            }
        }
        Ok(())
    }

    /// Preconditions shared by the review-card tests: the ledger really
    /// yields no top gap, so any selection must come from the cards.
    fn write_no_top_gap_ledger(repo: &Path) -> Result<(), String> {
        let ledger = ledger_with_repo_scoped_gap_only();
        if gap_records(&ledger)
            .into_iter()
            .any(|record| is_first_run_repairable_gap(&record))
        {
            return Err("fixture ledger must not yield a top gap".to_string());
        }
        write_json(&repo.join(DEFAULT_GAP_LEDGER), ledger)
    }

    fn position(haystack: &str, needle: &str) -> Result<usize, String> {
        haystack
            .find(needle)
            .ok_or_else(|| format!("missing {needle:?} in:\n{haystack}"))
    }

    /// #3906: with no ledger top gap, the first card carrying a repair start
    /// becomes the selection. Its command is carried byte-for-byte into
    /// start-here and on into pr-summary, every field comes from that card
    /// (not the earlier card without a start), and it leads the proof path.
    /// Onboarding re-walk 4: after the PR's own gap was repaired, first-pr
    /// promoted a summary-only card for an unchanged function (`with_shipping`,
    /// outside every hunk) to "Top actionable gap" with "Changed behavior".
    /// A card with no safe changed-line placement is a repository repair, so
    /// it never becomes the PR's top gap; a summary-only card that is on a
    /// changed line (inline cap reached) still does.
    #[test]
    fn review_card_outside_the_diff_is_not_the_pr_top_gap() -> Result<(), String> {
        use crate::output::review_comments::SUMMARY_REASON_INLINE_CAP_REACHED;
        let summary_only_card = |reason: &str| {
            let mut card = review_card("seam-b", Some(CARD_REPAIR_COMMAND));
            card["summary_reason"] = json!(reason);
            card
        };
        let packet_for = |card: Value| -> Result<Value, String> {
            let repo = temp_repo("first-pr-card-outside-diff")?;
            write_no_top_gap_ledger(&repo)?;
            let mut report = review_comments_report(Vec::new());
            report["summary_only"] = json!([card]);
            write_json(&repo.join(DEFAULT_REVIEW_COMMENTS), report)?;
            let packet = render_start_here_packet(&repo, &FirstPrOptions::default());
            cleanup(&repo)?;
            Ok(packet)
        };

        let outside = packet_for(summary_only_card(SUMMARY_REASON_NO_SAFE_PLACEMENT))?;
        assert_eq!(outside["status"], "no_action", "{outside}");
        assert_eq!(outside["selected"]["output_state"], "no_actionable_gap");
        assert!(
            outside["selected"].get("repair_command").is_none(),
            "{outside}"
        );
        let reason = outside["selected"]["reason"].as_str().unwrap_or_default();
        assert!(
            reason.contains(&format!("1 review card(s) in `target/ripr/review/comments.json` carry a repair start for code outside this PR's changed lines; `{}` ranks repository-wide repairs.", pilot_select_command("."))),
            "{reason}"
        );

        let capped = packet_for(summary_only_card(SUMMARY_REASON_INLINE_CAP_REACHED))?;
        assert_eq!(capped["status"], "actionable", "{capped}");
        assert_eq!(capped["selected"]["repair_command"], CARD_REPAIR_COMMAND);
        Ok(())
    }

    #[test]
    fn review_card_repair_start_is_carried_when_the_ledger_selects_nothing() -> Result<(), String> {
        let repo = temp_repo("first-pr-card-repair-start")?;
        write_no_top_gap_ledger(&repo)?;
        let without = review_card("seam-a", None);
        let with = review_card("seam-b", Some(CARD_REPAIR_COMMAND));
        if without.pointer("/llm_guidance/repair_command").is_some()
            || with.pointer("/llm_guidance/repair_command") != Some(&json!(CARD_REPAIR_COMMAND))
        {
            return Err("fixture cards must differ only in the carried start".to_string());
        }
        write_json(
            &repo.join(DEFAULT_REVIEW_COMMENTS),
            review_comments_report(vec![without, with]),
        )?;

        write_first_pr(&repo, &FirstPrOptions::default())?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        let markdown = fs::read_to_string(repo.join(DEFAULT_OUT_DIR).join(START_HERE_MD))
            .map_err(|err| format!("read start-here markdown: {err}"))?;
        cleanup(&repo)?;

        let selected = &packet["selected"];
        assert_eq!(packet["status"], "actionable");
        assert_eq!(selected["state"], "top_gap");
        assert_eq!(selected["output_state"], "actionable_gap");
        assert_eq!(selected["repair_command"], CARD_REPAIR_COMMAND);
        assert_eq!(selected["source_artifact"], DEFAULT_REVIEW_COMMENTS);
        assert_eq!(selected["canonical_gap_id"], "gap:seam-b");
        assert_eq!(selected["changed_behavior"], "seam-b >= threshold");
        assert_eq!(selected["missing_discriminator"], "seam-b == threshold");
        assert_eq!(
            selected["verify_command"],
            "ripr agent verify --root . --seam seam-b --json"
        );
        assert_eq!(
            selected["receipt_command"],
            "ripr agent receipt --root . --seam-id seam-b --json"
        );
        assert_eq!(selected["anchor"]["owner"], "pricing::seam-b");
        // The card is not a ledger gap: no `agent packet --gap-id`, and the
        // start stays off the editor-allowlisted `commands` map.
        assert!(selected["agent_packet_command"].is_null());
        assert!(packet["commands"].get("agent_packet").is_none());
        assert!(packet["commands"].get("repair").is_none());
        assert_eq!(
            packet["commands"]["verify"],
            "ripr agent verify --root . --seam seam-b --json"
        );

        let summary = start_here_cli_summary(
            &packet,
            Path::new("target/ripr/reports/start-here.json"),
            Path::new("target/ripr/reports/start-here.md"),
        );
        // #3906 (F60-14): the start leads, the after phase follows it as
        // the next step, and the low-level verify and receipt render as the
        // manual alternative, not as peer steps of the transaction.
        let after_phase = format!("{REPAIR_AFTER_PHASE_LABEL}: {REPAIR_AFTER_PHASE_STEP}\n");
        let start = position(
            &summary,
            &format!("Start repair: `{CARD_REPAIR_COMMAND}`\n{after_phase}"),
        )?;
        assert!(start < position(&summary, &format!("{MANUAL_VERIFY_LABEL}: `"))?);
        assert!(start < position(&summary, &format!("{MANUAL_RECEIPT_LABEL}: `"))?);
        assert!(!summary.contains("Verify command:"), "{summary}");
        assert!(!summary.contains("Receipt command:"), "{summary}");
        let bullet = position(
            &markdown,
            &format!("- Start repair: `{CARD_REPAIR_COMMAND}`\n- {after_phase}"),
        )?;
        assert!(bullet < position(&markdown, &format!("- {MANUAL_VERIFY_LABEL}: `"))?);
        let block = position(
            &markdown,
            &format!("Start repair:\n`{CARD_REPAIR_COMMAND}`\n"),
        )?;
        let block_after_phase = position(&markdown, &format!("\n{after_phase}\n"))?;
        assert!(block < block_after_phase);
        assert!(block_after_phase < position(&markdown, &format!("{MANUAL_VERIFY_LABEL}:\n`"))?);
        assert!(block_after_phase < position(&markdown, &format!("{MANUAL_RECEIPT_LABEL}:\n`"))?);
        assert!(!markdown.contains("Verify command"), "{markdown}");
        assert!(!markdown.contains("Receipt command"), "{markdown}");

        // pr-summary carries start-here's command unchanged and leads the
        // local reproduction commands with it.
        let pr_summary = crate::app::pr_summary::build_pr_evidence_summary(
            Some(&packet),
            None,
            None,
            None,
            None,
            None,
        );
        let summary_json: Value = serde_json::from_str(
            &crate::app::pr_summary::render_pr_evidence_summary_json(&pr_summary),
        )
        .map_err(|err| format!("parse pr-summary json: {err}"))?;
        assert_eq!(
            summary_json["top_repair"]["repair_command"],
            CARD_REPAIR_COMMAND
        );
        assert_eq!(
            summary_json["local_reproduction_commands"][0],
            CARD_REPAIR_COMMAND
        );
        let summary_md = crate::app::pr_summary::render_evidence_summary_md(&pr_summary);
        let line = position(
            &summary_md,
            &format!("- start repair: `{CARD_REPAIR_COMMAND}`\n"),
        )?;
        assert!(
            line < position(
                &summary_md,
                &format!("- {}: `", MANUAL_VERIFY_LABEL.to_lowercase())
            )?
        );
        assert!(!summary_md.contains("- verify: `"), "{summary_md}");
        Ok(())
    }

    /// #3906 negative: a card without a repair start (it failed the flip)
    /// under-emits. First-pr never builds `agent repair` from its seam id;
    /// the no-action reason names why.
    #[test]
    fn review_card_without_repair_start_stays_no_action() -> Result<(), String> {
        let repo = temp_repo("first-pr-card-no-start")?;
        write_no_top_gap_ledger(&repo)?;
        let card = review_card("seam-a", None);
        if card["seam_id"] != "seam-a" || card["gap_state"] != "actionable" {
            return Err("fixture card must be actionable with a known seam id".to_string());
        }
        write_json(
            &repo.join(DEFAULT_REVIEW_COMMENTS),
            review_comments_report(vec![card]),
        )?;
        write_first_pr(&repo, &FirstPrOptions::default())?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        let markdown = fs::read_to_string(repo.join(DEFAULT_OUT_DIR).join(START_HERE_MD))
            .map_err(|err| format!("read start-here markdown: {err}"))?;
        cleanup(&repo)?;

        assert_eq!(packet["status"], "no_action");
        assert_eq!(packet["selected"]["state"], "no_action");
        assert!(
            packet["selected"]["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains(
                    "No review card in `target/ripr/review/comments.json` carries a repair start"
                ))
        );
        let text = packet.to_string();
        assert!(!text.contains("agent repair"), "{text}");
        assert!(!markdown.contains("Start repair"), "{markdown}");
        Ok(())
    }

    /// #3906 negative: a ledger top gap keeps its verify/receipt/agent-packet
    /// route and carries no repair start, even when an eligible card exists.
    #[test]
    fn ledger_top_gap_keeps_its_route_beside_an_eligible_card() -> Result<(), String> {
        let repo = temp_repo("first-pr-ledger-beside-card")?;
        write_json(&repo.join(DEFAULT_GAP_LEDGER), ledger_with_repairable_gap())?;
        write_json(
            &repo.join(DEFAULT_REVIEW_COMMENTS),
            review_comments_report(vec![review_card("seam-b", Some(CARD_REPAIR_COMMAND))]),
        )?;
        write_first_pr(&repo, &FirstPrOptions::default())?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        let markdown = fs::read_to_string(repo.join(DEFAULT_OUT_DIR).join(START_HERE_MD))
            .map_err(|err| format!("read start-here markdown: {err}"))?;
        cleanup(&repo)?;

        let selected = &packet["selected"];
        assert_eq!(selected["state"], "top_gap");
        assert_eq!(selected["gap_id"], "gap:pr:pricing:threshold-boundary");
        assert_eq!(selected["source_artifact"], DEFAULT_GAP_LEDGER);
        assert!(selected.get("repair_command").is_none());
        assert!(
            packet["commands"]["agent_packet"].as_str().is_some_and(
                |command| command.contains("--gap-id gap:pr:pricing:threshold-boundary")
            )
        );
        assert!(!packet.to_string().contains("agent repair"));
        assert!(!markdown.contains("Start repair"), "{markdown}");

        let pr_summary = crate::app::pr_summary::build_pr_evidence_summary(
            Some(&packet),
            None,
            None,
            None,
            None,
            None,
        );
        let summary_json: Value = serde_json::from_str(
            &crate::app::pr_summary::render_pr_evidence_summary_json(&pr_summary),
        )
        .map_err(|err| format!("parse pr-summary json: {err}"))?;
        assert!(summary_json["top_repair"].get("repair_command").is_none());
        assert_eq!(
            summary_json["local_reproduction_commands"][0], "ripr check --base origin/main",
            "pr-summary replays the base the start-here packet recorded"
        );
        Ok(())
    }

    /// #3906: a card from another range, root, or an incomplete report must
    /// not become this PR's repair start. F60-10: nor may first-pr call the
    /// PR "no actionable gap" on cards it could not use; it stops on the
    /// review-card input with the seam-level command that regenerates it.
    #[test]
    fn review_card_repair_start_fails_closed_on_stale_or_missing_reports() -> Result<(), String> {
        let eligible = || review_card("seam-b", Some(CARD_REPAIR_COMMAND));
        let mut other_base = review_comments_report(vec![eligible()]);
        other_base["base"] = json!("origin/release");
        let mut incomplete = review_comments_report(vec![eligible()]);
        incomplete["status"] = json!("incomplete");
        let mut other_root = review_comments_report(vec![eligible()]);
        other_root["root"] = json!("crates/other");
        let mut other_head = review_comments_report(vec![eligible()]);
        other_head["head"] = json!("feature-tip");
        // The malformed branches: another tool's report, and reports that
        // do not record their status, root, or head.
        let mut other_tool = review_comments_report(vec![eligible()]);
        other_tool["tool"] = json!("not-ripr");
        let unrecorded = |field: &str| {
            let mut report = review_comments_report(vec![eligible()]);
            if let Some(object) = report.as_object_mut() {
                object.remove(field);
            }
            report
        };
        let seam_route = format!(
            "ripr review-comments --root {} --base origin/main --head HEAD --out {}",
            bound_arg("."),
            shell_arg(&anchored_redirect_target(".", DEFAULT_REVIEW_COMMENTS))
        );
        for (name, report, state, needle) in [
            (
                "base",
                Some(other_base),
                "stale_artifact",
                "generated for base `origin/release`, not `origin/main`",
            ),
            (
                "status",
                Some(incomplete),
                "blocked_artifact",
                "the report status is `incomplete`",
            ),
            (
                "root",
                Some(other_root),
                "wrong_root",
                "generated for root `crates/other`, not `.`",
            ),
            (
                "head",
                Some(other_head),
                "stale_artifact",
                "generated for head `feature-tip`, not `HEAD`",
            ),
            (
                "tool",
                Some(other_tool),
                "malformed_artifact",
                "the file is not a RIPR review-comments report",
            ),
            (
                "unrecorded status",
                Some(unrecorded("status")),
                "malformed_artifact",
                "the report does not record a status",
            ),
            (
                "unrecorded root",
                Some(unrecorded("root")),
                "malformed_artifact",
                "the report does not record its root",
            ),
            (
                "unrecorded head",
                Some(unrecorded("head")),
                "malformed_artifact",
                "the report does not record its head",
            ),
            (
                "missing",
                None,
                "missing_artifact",
                "PR repair cards is missing: target/ripr/review/comments.json",
            ),
        ] {
            let repo = temp_repo(&format!("first-pr-card-{name}"))?;
            write_no_top_gap_ledger(&repo)?;
            if let Some(report) = report {
                write_json(&repo.join(DEFAULT_REVIEW_COMMENTS), report)?;
            }
            let packet = render_start_here_packet(&repo, &FirstPrOptions::default());
            cleanup(&repo)?;
            assert_eq!(packet["status"], "blocked", "{name}: {packet}");
            assert_eq!(packet["selected"]["state"], state, "{name}");
            assert_eq!(packet["commands"]["next"], seam_route, "{name}");
            assert!(
                packet.to_string().contains(needle),
                "{name}: {needle} not in {packet}"
            );
            assert!(!packet.to_string().contains("agent repair"), "{name}");
        }

        // Cards rendered from the gap ledger were read and are current; they
        // cannot carry a repair start, so this stays no-action and names the
        // seam-level route.
        let mut gap_ledger_scoped = review_comments_report(vec![review_card("seam-a", None)]);
        gap_ledger_scoped["analysis_scope"] = json!({"scope": "gap_ledger_artifact"});
        let repo = temp_repo("first-pr-card-scope")?;
        write_no_top_gap_ledger(&repo)?;
        write_json(&repo.join(DEFAULT_REVIEW_COMMENTS), gap_ledger_scoped)?;
        let packet = render_start_here_packet(&repo, &FirstPrOptions::default());
        cleanup(&repo)?;
        assert_eq!(packet["status"], "no_action", "{packet}");
        let reason = packet["selected"]["reason"].as_str().unwrap_or_default();
        assert!(
            reason.contains(&format!(
                "rendered from the gap ledger; for a seam-level repair start, run `{seam_route}`"
            )),
            "{reason}"
        );
        Ok(())
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
        // The receipt path is the file the printed receipt command writes.
        assert_eq!(
            packet["selected"]["receipt_path"],
            ".ripr/receipts/python-threshold.json"
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
            "Verify after the test edit: `pytest tests/test_pricing.py::test_calculate_discount_smoke`"
        ));
        check_first_pr(&repo, &options)?;
        cleanup(&repo)
    }

    #[test]
    fn python_ledger_older_than_the_named_test_edit_is_stale() -> Result<(), String> {
        let repo = temp_python_repo("first-pr-python-stale-ledger")?;
        fs::create_dir_all(repo.join("app")).map_err(|err| format!("mkdir app: {err}"))?;
        fs::create_dir_all(repo.join("tests")).map_err(|err| format!("mkdir tests: {err}"))?;
        fs::write(
            repo.join("app/pricing.py"),
            "def calculate_discount(amount, threshold):\n    return amount >= threshold\n",
        )
        .map_err(|err| format!("write app/pricing.py: {err}"))?;
        let test_file = repo.join("tests/test_pricing.py");
        fs::write(
            &test_file,
            "def test_calculate_discount_smoke():\n    pass\n",
        )
        .map_err(|err| format!("write tests/test_pricing.py: {err}"))?;
        run_git_setup(&repo, &["add", "app/pricing.py", "tests/test_pricing.py"])?;
        run_git_setup(&repo, &["commit", "-m", "change pricing"])?;
        let ledger_path = repo.join(DEFAULT_GAP_LEDGER);
        write_json(&ledger_path, ledger_with_python_repairable_gap())?;
        let ledger_written = fs::metadata(&ledger_path)
            .and_then(|metadata| metadata.modified())
            .map_err(|err| format!("ledger mtime: {err}"))?;
        let set_mtime = |path: &Path, at: std::time::SystemTime| -> Result<(), String> {
            fs::File::options()
                .write(true)
                .open(path)
                .and_then(|file| file.set_modified(at))
                .map_err(|err| format!("set mtime {}: {err}", path.display()))
        };
        let options = FirstPrOptions::default();

        // The named test predates the ledger: the repair is still selected.
        set_mtime(
            &test_file,
            ledger_written - std::time::Duration::from_mins(1),
        )?;
        set_mtime(
            &repo.join("app/pricing.py"),
            ledger_written - std::time::Duration::from_mins(1),
        )?;
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["selected"]["state"], "top_gap");

        // The operator edits the named test after the ledger: first-pr stops
        // repeating the old repair and routes to the refresh command.
        set_mtime(
            &test_file,
            ledger_written + std::time::Duration::from_mins(1),
        )?;
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "blocked");
        assert_eq!(packet["selected"]["state"], "stale_artifact");
        let message = packet["selected"]["message"].as_str().unwrap_or("");
        assert!(
            message.contains("predates the last edit to `tests/test_pricing.py`"),
            "stale message must name the edited test: {message}"
        );
        let next = packet["selected"]["next_command"].as_str().unwrap_or("");
        assert!(
            next.starts_with("ripr check --root ") && next.contains("ripr reports gap-ledger"),
            "stale selection must route to the check-output refresh: {next}"
        );
        // The edit that made the evidence stale is usually uncommitted; a
        // refresh that reads HEAD selects the gap it just closed again.
        assert!(
            next.contains(" --base origin/main --worktree --json > "),
            "stale refresh must read the working tree: {next}"
        );
        assert!(packet["selected"].get("verify_command").is_none());
        check_first_pr(&repo, &options)?;
        cleanup(&repo)
    }

    /// Devin on #4429: with a supplied `--check-output`, the stale recovery
    /// must rewrite that file, because the freshness check reads its
    /// timestamp; a ledger-only command would block again on the next run.
    #[test]
    fn stale_recovery_rewrites_a_supplied_check_output() {
        let options = FirstPrOptions {
            check_output: Some("saved/check.json".to_string()),
            ..FirstPrOptions::default()
        };
        let ledger_only = regenerate_check_output_gap_ledger_command(&options);
        assert!(
            ledger_only.starts_with("ripr reports gap-ledger"),
            "{ledger_only}"
        );
        let rerun = rerun_check_output_gap_ledger_command(&options);
        let (check, ledger) = rerun.split_once(" && ").unwrap_or((rerun.as_str(), ""));
        assert!(check.starts_with("ripr check --root "), "{rerun}");
        assert!(check.ends_with("saved/check.json"), "{rerun}");
        assert!(check.contains(" --worktree --json > "), "{rerun}");
        assert_eq!(ledger, ledger_only);
    }

    /// The stale branch itself must route a supplied `--check-output` to the
    /// rewriting command, not the ledger-only one.
    #[test]
    fn stale_supplied_check_output_routes_to_a_check_rerun() -> Result<(), String> {
        let repo = temp_python_repo("first-pr-python-stale-check-output")?;
        fs::create_dir_all(repo.join("app")).map_err(|err| format!("mkdir app: {err}"))?;
        let source = repo.join("app/pricing.py");
        fs::write(
            &source,
            "def calculate_discount(amount, threshold):\n    return amount >= threshold\n",
        )
        .map_err(|err| format!("write app/pricing.py: {err}"))?;
        run_git_setup(&repo, &["add", "app/pricing.py"])?;
        run_git_setup(&repo, &["commit", "-m", "change pricing"])?;
        let check_output = repo.join(DEFAULT_CHECK_OUTPUT);
        write_json(&check_output, check_output_with_python_repair_card())?;
        let written = fs::metadata(&check_output)
            .and_then(|metadata| metadata.modified())
            .map_err(|err| format!("check output mtime: {err}"))?;
        fs::File::options()
            .write(true)
            .open(&source)
            .and_then(|file| file.set_modified(written + std::time::Duration::from_mins(1)))
            .map_err(|err| format!("set mtime: {err}"))?;

        let options = FirstPrOptions {
            check_output: Some(DEFAULT_CHECK_OUTPUT.to_string()),
            ..FirstPrOptions::default()
        };
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["selected"]["state"], "stale_artifact");
        let next = packet["selected"]["next_command"].as_str().unwrap_or("");
        let (check, _) = next.split_once(" && ").unwrap_or((next, ""));
        assert!(
            check.starts_with("ripr check --root ") && check.ends_with(DEFAULT_CHECK_OUTPUT),
            "stale recovery must rewrite the supplied check output: {next}"
        );
        cleanup(&repo)
    }

    /// MCP agent walk (2026-09-29): on the Python route the printed
    /// receipt records only the verify status it is given, and nothing on
    /// the route showed whether the gap moved after the test edit. With the
    /// check report the gap came from on disk, first-pr names the static
    /// re-check: check over the working tree, then `ripr outcome` against
    /// that report. Without the report there is nothing to compare, so no
    /// re-check is offered.
    #[test]
    fn python_top_gap_names_the_static_recheck_after_verify() -> Result<(), String> {
        let repo = temp_python_repo("first-pr-python-static-recheck")?;
        fs::create_dir_all(repo.join("app")).map_err(|err| format!("mkdir app: {err}"))?;
        fs::write(
            repo.join("app/pricing.py"),
            "def calculate_discount(amount, threshold):\n    return amount >= threshold\n",
        )
        .map_err(|err| format!("write app/pricing.py: {err}"))?;
        run_git_setup(&repo, &["add", "app/pricing.py"])?;
        run_git_setup(&repo, &["commit", "-m", "change pricing"])?;
        let options = FirstPrOptions::default();

        let mut ledger = ledger_with_python_repairable_gap();
        ledger["records"][0]["receipt_command"] = json!(
            "ripr receipt write --gap g --verify-command 'pytest tests/test_pricing.py' --status not_run --out target/ripr/receipts/g.json"
        );
        ledger["inputs"] = json!({
            "source_kind": "check_output",
            "records": DEFAULT_CHECK_OUTPUT,
        });

        // No check report on disk: nothing to compare against.
        write_json(&repo.join(DEFAULT_GAP_LEDGER), ledger.clone())?;
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["selected"]["state"], "top_gap");
        assert!(
            packet["selected"].get("static_recheck_command").is_none(),
            "{packet}"
        );

        // A `ripr outcome` receipt compares snapshots itself: no re-check.
        write_json(
            &repo.join(DEFAULT_CHECK_OUTPUT),
            check_output_with_python_repair_card(),
        )?;
        write_json(
            &repo.join(DEFAULT_GAP_LEDGER),
            ledger_with_python_repairable_gap(),
        )?;
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["selected"]["state"], "top_gap");
        assert!(
            packet["selected"].get("static_recheck_command").is_none(),
            "{packet}"
        );

        // A `receipt write` receipt with the check report on disk.
        write_json(&repo.join(DEFAULT_GAP_LEDGER), ledger.clone())?;
        write_first_pr(&repo, &options)?;
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["selected"]["state"], "top_gap");
        let before = options.anchored_arg(DEFAULT_CHECK_OUTPUT);
        let after = options.anchored_arg("target/ripr/reports/check.after.json");
        let expected = format!(
            "ripr check --root {} --base origin/main --worktree --json > {after} && ripr outcome --before {before} --after {after}",
            bound_arg(".")
        );
        assert_eq!(packet["selected"]["static_recheck_command"], expected);
        assert!(
            before.ends_with("target/ripr/reports/check.json"),
            "{before}"
        );
        assert!(
            after.ends_with("target/ripr/reports/check.after.json"),
            "{after}"
        );

        let summary = start_here_cli_summary(
            &packet,
            Path::new("start-here.json"),
            Path::new("start-here.md"),
        );
        let status_line = format!("{RECEIPT_STATUS_LABEL}: {RECEIPT_STATUS_STEP}\n");
        let recheck_line = format!("{STATIC_RECHECK_LABEL}: `{expected}`\n");
        let boundary_line = format!("{RECEIPT_BOUNDARY_LABEL}: {RECEIPT_BOUNDARY_STEP}\n");
        let status_at = summary.find(&status_line).ok_or(summary.clone())?;
        let recheck_at = summary.find(&recheck_line).ok_or(summary.clone())?;
        let boundary_at = summary.find(&boundary_line).ok_or(summary.clone())?;
        assert!(
            status_at < recheck_at && recheck_at < boundary_at,
            "{summary}"
        );
        assert!(
            summary.contains(&format!("{STATIC_RECHECK_LABEL} (PowerShell 2/2): `ripr outcome --before {before} --after {after}`")),
            "{summary}"
        );
        assert!(
            RECEIPT_BOUNDARY_STEP.contains("does not re-check the gap")
                && RECEIPT_BOUNDARY_STEP.contains("not a runtime or mutation result"),
            "{RECEIPT_BOUNDARY_STEP}"
        );
        let markdown = render_start_here_markdown(&packet);
        assert!(
            markdown.contains(&format!("- {STATIC_RECHECK_LABEL}: `{expected}`\n"))
                && markdown.contains(&format!(
                    "- {RECEIPT_BOUNDARY_LABEL}: {RECEIPT_BOUNDARY_STEP}\n"
                )),
            "{markdown}"
        );
        check_first_pr(&repo, &options)?;

        // Provenance: the before report is the one the ledger was built from,
        // never the default path merely because it exists.
        let recheck_for = |ledger: &Value, options: &FirstPrOptions| -> Result<Value, String> {
            write_json(&repo.join(DEFAULT_GAP_LEDGER), ledger.clone())?;
            write_first_pr(&repo, options)?;
            let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
            assert_eq!(packet["selected"]["state"], "top_gap", "{packet}");
            Ok(packet["selected"]["static_recheck_command"].clone())
        };
        let mut unnamed = ledger.clone();
        unnamed
            .as_object_mut()
            .ok_or("ledger fixture must be an object")?
            .remove("inputs");
        assert_eq!(recheck_for(&unnamed, &options)?, Value::Null);
        let mut records_source = ledger.clone();
        records_source["inputs"]["source_kind"] = json!("records");
        assert_eq!(recheck_for(&records_source, &options)?, Value::Null);
        let mut saved = ledger.clone();
        saved["inputs"]["records"] = json!("saved/check.json");
        assert_eq!(
            recheck_for(&saved, &options)?,
            Value::Null,
            "an absent named report must not fall back to the default check.json"
        );
        write_json(
            &repo.join("saved/check.json"),
            check_output_with_python_repair_card(),
        )?;
        let saved_before = options.anchored_arg("saved/check.json");
        let saved_after = options.anchored_arg("saved/check.after.json");
        assert_eq!(
            recheck_for(&saved, &options)?,
            json!(format!(
                "ripr check --root {} --base origin/main --worktree --json > {saved_after} && ripr outcome --before {saved_before} --after {saved_after}",
                bound_arg(".")
            ))
        );
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
            ledger["records"][0]["projection_eligibility"]["agent_packet"]["eligible"],
            true
        );
        let ledger_receipt_cmd = ledger["records"][0]["receipt_command"]
            .as_str()
            .unwrap_or("");
        assert!(
            ledger_receipt_cmd.starts_with("ripr receipt write --gap "),
            "ledger receipt_command must be canonical ripr receipt write, got: {ledger_receipt_cmd}"
        );
        assert!(
            !ledger_receipt_cmd.contains("ripr outcome"),
            "ledger receipt_command must not contain ripr outcome, got: {ledger_receipt_cmd}"
        );

        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "actionable");
        assert_eq!(packet["inputs"]["check_output"], DEFAULT_CHECK_OUTPUT);
        assert_eq!(packet["selected"]["source_artifact"], DEFAULT_GAP_LEDGER);
        assert_eq!(
            packet["selected"]["receipt_command_source"],
            "gap_ledger.receipt_command"
        );
        let packet_receipt_cmd = packet["selected"]["receipt_command"].as_str().unwrap_or("");
        assert!(
            packet_receipt_cmd.starts_with("ripr receipt write --gap "),
            "packet receipt_command must be canonical ripr receipt write, got: {packet_receipt_cmd}"
        );
        assert!(
            !packet_receipt_cmd.contains("ripr outcome"),
            "packet receipt_command must not contain ripr outcome, got: {packet_receipt_cmd}"
        );
        // The gap id carries shell metacharacters (`>=`), so both presented
        // forms must quote it: unquoted, bash truncates the argument at `>`
        // and redirects to a file literally named `=threshold` (PR #3625
        // review round 3, coderabbit).
        // Issue #3872: the packet redirect anchors at the resolved --root.
        assert_eq!(
            packet["selected"]["agent_packet_command"],
            format!(
                "ripr agent packet --root {} --gap-ledger {} --gap-id 'gap:pr:gap:python:app/pricing.py:calculate_discount:predicate_boundary:amount>=threshold' --json > {}",
                bound_arg("."),
                shell_arg(&anchored_redirect_target(".", DEFAULT_GAP_LEDGER)),
                shell_arg(&anchored_redirect_target(
                    ".",
                    "target/ripr/workflow/agent-packet.json"
                ))
            )
        );
        let quoted_id = "gap:pr:gap:python:app/pricing.py:calculate_discount:predicate_boundary:amount>=threshold";
        let markdown = render_start_here_markdown(&packet);
        assert!(
            markdown.contains(&format!("--gap-id '{quoted_id}' --json >")),
            "rendered bash form must quote the gap id:\n{markdown}"
        );
        assert!(
            markdown.contains(&format!("--gap-id '{quoted_id}' --json) | Out-String")),
            "rendered powershell form must quote the gap id:\n{markdown}"
        );
        let packet_artifact = packet["artifacts"]
            .as_array()
            .and_then(|artifacts| {
                artifacts
                    .iter()
                    .find(|artifact| artifact["id"] == "agent_packet")
            })
            .ok_or_else(|| "agent-packet artifact missing".to_string())?;
        assert_eq!(
            packet_artifact["regeneration_command"],
            packet["selected"]["agent_packet_command"]
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
    fn python_wrong_owner_check_output_is_not_selected_for_start_here() -> Result<(), String> {
        let repo = temp_python_repo("first-pr-python-wrong-owner")?;
        write_json(
            &repo.join(DEFAULT_CHECK_OUTPUT),
            serde_json::from_str(include_str!(
                "../../../../fixtures/python_adversarial_same_method_other_class/expected/check.json"
            ))
            .map_err(|error| format!("parse wrong-owner Python fixture: {error}"))?,
        )?;

        let options = FirstPrOptions {
            check_output: Some(DEFAULT_CHECK_OUTPUT.to_string()),
            ..FirstPrOptions::default()
        };
        write_first_pr(&repo, &options)?;

        let ledger = read_packet(&repo.join(DEFAULT_GAP_LEDGER))?;
        assert_eq!(ledger["records"].as_array().map(Vec::len), Some(1));
        assert_eq!(ledger["records"][0]["language"], "python");
        assert_eq!(ledger["records"][0]["language_status"], "preview");
        assert_eq!(
            ledger["records"][0]["projection_eligibility"]["agent_packet"]["eligible"],
            false
        );
        assert!(ledger["records"][0]["receipt_command"].is_null());
        assert_eq!(ledger["summary"]["projection_agent_packet_eligible"], 0);

        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        assert_eq!(packet["status"], "no_action");
        assert_eq!(packet["selected"]["state"], "no_action");
        assert_eq!(packet["selected"]["output_state"], "no_actionable_gap");
        assert!(packet["selected"]["gap_id"].is_null());
        assert!(packet["selected"]["receipt_command"].is_null());
        assert!(packet["selected"]["agent_packet_command"].is_null());
        let packet_artifact = packet["artifacts"]
            .as_array()
            .and_then(|artifacts| {
                artifacts
                    .iter()
                    .find(|artifact| artifact["id"] == "agent_packet")
            })
            .ok_or_else(|| "agent-packet artifact missing".to_string())?;
        assert!(packet_artifact["regeneration_command"].is_null());
        assert!(packet["commands"]["agent_packet"].is_null());
        assert!(packet["commands"]["verify"].is_null());
        assert!(packet["commands"]["receipt"].is_null());
        let markdown = fs::read_to_string(repo.join(DEFAULT_OUT_DIR).join(START_HERE_MD))
            .map_err(|error| format!("read wrong-owner start-here Markdown: {error}"))?;
        assert!(!markdown.contains("ripr agent packet"));
        assert!(!markdown.contains("ripr receipt write"));
        cleanup(&repo)
    }

    #[test]
    fn python_no_strong_oracle_check_output_is_selected_for_start_here() -> Result<(), String> {
        let repo = temp_python_repo("first-pr-python-no-strong-oracle")?;
        write_json(
            &repo.join(DEFAULT_CHECK_OUTPUT),
            serde_json::from_str(include_str!(
                "../../../../fixtures/python_boundary_gap/expected/check.json"
            ))
            .map_err(|error| format!("parse boundary Python fixture: {error}"))?,
        )?;
        let options = FirstPrOptions {
            check_output: Some(DEFAULT_CHECK_OUTPUT.to_string()),
            ..FirstPrOptions::default()
        };
        write_first_pr(&repo, &options)?;

        let ledger = read_packet(&repo.join(DEFAULT_GAP_LEDGER))?;
        if ledger["records"][0]["projection_eligibility"]["agent_packet"]["eligible"] != true
            || ledger["summary"]["projection_agent_packet_eligible"] != 1
            || ledger["records"][0]["receipt_command"].is_null()
        {
            return Err(
                "boundary fixture did not materialize eligible receipt-backed ledger record"
                    .to_string(),
            );
        }
        let packet = read_packet(&repo.join(DEFAULT_OUT_DIR).join(START_HERE_JSON))?;
        if packet["status"] != "actionable"
            || packet["selected"]["state"] != "top_gap"
            || packet["commands"]["agent_packet"].is_null()
            || packet["commands"]["verify"].is_null()
            || packet["commands"]["receipt"].is_null()
        {
            return Err(
                "boundary fixture did not produce a concrete first-PR packet route".to_string(),
            );
        }
        cleanup(&repo)
    }

    #[test]
    fn python_preview_projection_eligibility_fails_closed() {
        let mut record = ledger_with_python_repairable_gap()["records"][0].clone();
        let record_ref = &record;
        assert!(is_first_run_repairable_gap(&record_ref));

        record["projection_eligibility"]["agent_packet"]["eligible"] = json!(false);
        let record_ref = &record;
        assert!(!is_first_run_repairable_gap(&record_ref));

        if let Some(object) = record.as_object_mut() {
            object.remove("projection_eligibility");
        }
        let record_ref = &record;
        assert!(!is_first_run_repairable_gap(&record_ref));

        record["projection_eligibility"] = json!("malformed");
        let record_ref = &record;
        assert!(!is_first_run_repairable_gap(&record_ref));
    }

    #[test]
    fn typescript_preview_gap_ledger_is_selected_for_start_here() -> Result<(), String> {
        let repo = temp_typescript_repo("first-pr-typescript-preview")?;
        fs::create_dir_all(repo.join("src")).map_err(|err| format!("mkdir src: {err}"))?;
        fs::write(
            repo.join("src/discount.ts"),
            "export function applyDiscount(amount: number, threshold: number) { return amount >= threshold ? 50 : 0; }\n",
        )
        .map_err(|err| format!("write src/discount.ts: {err}"))?;
        run_git_setup(&repo, &["add", "src/discount.ts"])?;
        run_git_setup(&repo, &["commit", "-m", "change discount"])?;
        write_json(
            &repo.join(DEFAULT_GAP_LEDGER),
            ledger_with_typescript_repairable_gap(),
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
        assert_eq!(packet["selected"]["language"], "typescript");
        assert_eq!(packet["selected"]["language_status"], "preview");
        assert_eq!(
            packet["selected"]["missing_discriminator"],
            "amount == threshold"
        );
        assert_eq!(
            packet["selected"]["verify_command"],
            "jest tests/discount.test.ts"
        );
        let typescript_project = preflight_check(&packet, "typescript_project")?;
        assert_eq!(typescript_project["status"], "ok");
        let summary = start_here_cli_summary(
            &packet,
            Path::new("target/ripr/reports/start-here.json"),
            Path::new("target/ripr/reports/start-here.md"),
        );
        assert!(summary.contains(
            "Safe next action: repair one named gap `gap:typescript:typescript_preview:2396aec1`"
        ));
        assert!(summary.contains("Verify after the test edit: `jest tests/discount.test.ts`"));
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

    fn ledger_with_typescript_repairable_gap() -> Value {
        json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "kind": "gap_decision_ledger",
            "status": "advisory",
            "records": [
                {
                    "gap_id": "gap:pr:gap:typescript:typescript_preview:2396aec1",
                    "canonical_gap_id": "gap:typescript:typescript_preview:2396aec1",
                    "kind": "MissingBoundaryAssertion",
                    "language": "typescript",
                    "language_status": "preview",
                    "scope": "pr_local",
                    "current_evidence_strength": "weakly_exposed",
                    "changed_behavior": "amount == threshold",
                    "missing_discriminator": "amount == threshold",
                    "gap_state": "actionable",
                    "policy_state": "new",
                    "repairability": "repairable",
                    "static_limit_kind": "typescript_preview",
                    "static_limit_detail": "TypeScript repair packets are preview advisory evidence.",
                    "anchor": {
                        "file": "src/discount.ts",
                        "line": 2,
                        "owner": "applyDiscount",
                        "dedupe_fingerprint": "gap:typescript:typescript_preview:2396aec1"
                    },
                    "repair_route": {
                        "route_kind": "AddBoundaryAssertion",
                        "target_file": "tests/discount.test.ts",
                        "related_test": "tests/discount.test.ts::applyDiscount applies discount when amount meets threshold",
                        "assertion_shape": "expect(result).toBe(50)",
                        "missing_discriminator": "amount == threshold",
                        "changed_behavior": "amount == threshold",
                        "stop_conditions": [
                            "Stop if the TypeScript preview packet loses repair_packet_ready=true.",
                            "Stop if the verification command cannot run from this workspace.",
                            "Stop if the repair appears to require a production-code edit."
                        ]
                    },
                    "verification_commands": [
                        "jest tests/discount.test.ts"
                    ],
                    "receipt_command": "ripr receipt write --gap gap:typescript:typescript_preview:2396aec1 --verify-cmd 'jest tests/discount.test.ts' --out target/ripr/receipts/gap-typescript-typescript-preview-2396aec1.targeted-test-outcome.json"
                }
            ]
        })
    }

    #[test]
    fn receipt_path_is_the_out_path_the_printed_receipt_command_writes() {
        // The shape the gap ledger prints for a Python preview gap: the gap
        // id is single-quoted because it carries `>=` and `/`.
        let command = "ripr receipt write --gap 'gap:python:pricing/__init__.py:discounted_total:predicate_boundary:predicate:amount>=discount_threshold' --verify-command 'python -m pytest tests/test_pricing.py::test_discount_far_above_threshold' --status not_run --out target/ripr/receipts/gap-python-pricing-__init__.py-discounted_total-predicate_boundary-predicate-amount-discount_threshold.json";
        assert_eq!(
            receipt_command_out_path(command).as_deref(),
            Some(
                "target/ripr/receipts/gap-python-pricing-__init__.py-discounted_total-predicate_boundary-predicate-amount-discount_threshold.json"
            )
        );
        assert_eq!(
            receipt_command_out_path(
                "ripr receipt write --gap g --out 'target/my receipts/o'\\''k.json'"
            )
            .as_deref(),
            Some("target/my receipts/o'k.json")
        );
        // A quoted `--out` is the gap's value, not the flag: the command
        // writes the default for that gap id.
        assert_eq!(
            receipt_command_out_path("ripr receipt write --gap '--out' --verify-command x"),
            Some(
                crate::app::receipt::receipt_default_path("--out")
                    .to_string_lossy()
                    .replace('\\', "/")
            )
        );
        // Without `--out`, the command writes the receipt writer's default
        // for its gap (Devin review on #4485).
        assert_eq!(
            receipt_command_out_path(
                "ripr receipt write --gap gap:test:aabbccdd --verify-command x --status not_run"
            )
            .as_deref(),
            Some(
                crate::app::receipt::receipt_default_path("gap:test:aabbccdd")
                    .to_string_lossy()
                    .replace('\\', "/")
                    .as_str()
            )
        );
        assert!(
            receipt_command_out_path("ripr receipt write --gap gap:test:aabbccdd")
                .is_some_and(|path| path.starts_with("target/ripr/receipts/"))
        );
        for command in [
            "ripr receipt write --verify-command x --status not_run",
            "ripr outcome --gap g --format json",
            "ripr receipt write --gap g --out",
            "ripr receipt write --gap g --out ''",
            "ripr outcome --json > target/o.json --out other.json",
            "ripr receipt write --gap 'unterminated --out x.json",
            "cargo test --out x.json",
        ] {
            assert_eq!(receipt_command_out_path(command), None, "{command}");
        }
    }

    #[test]
    fn printed_receipt_command_out_wins_over_a_recorded_receipt_path() {
        let record = json!({"receipt_path": "target/ripr/receipts/recorded.json"});
        let command = "ripr receipt write --gap g --verify-command x --status not_run --out target/ripr/receipts/written.json";
        assert_eq!(
            selected_receipt_path(&record, Some(command)).as_deref(),
            Some("target/ripr/receipts/written.json"),
            "the path shown must be the file the shown command writes"
        );
        assert_eq!(
            selected_receipt_path(&record, Some("ripr receipt write --gap g")),
            receipt_command_out_path("ripr receipt write --gap g"),
            "a command without --out writes the writer's default for its gap, not the recorded path"
        );
        assert_eq!(
            selected_receipt_path(&record, Some("ripr outcome --json")).as_deref(),
            Some("target/ripr/receipts/recorded.json"),
            "a command that names no receipt file falls back to the recorded path"
        );
        let nested = json!({"receipt": {"path": "target/ripr/receipts/nested.json"}});
        assert_eq!(
            selected_receipt_path(&nested, None).as_deref(),
            Some("target/ripr/receipts/nested.json")
        );
        assert_eq!(selected_receipt_path(&json!({}), None), None);
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
                    "source_currentness": "candidate_current",
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
                    "projection_eligibility": {
                        "agent_packet": {
                            "eligible": true,
                            "reason": "direct Python oracle alignment is eligible for preview packet projection"
                        }
                    },
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
                    "source_currentness": "candidate_current",
                    "classification": "weakly_exposed",
                    "source_currentness": "candidate_current",
                    "oracle_alignment": "direct",
                    "alignment_reason": "strong_oracle_observes_owner_name",
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
        // The checked-in ledger stands for evidence generated just now. A
        // checkout writes files in no fixed order, so without this a test file
        // can look newer than the ledger and trip the stale-evidence guard.
        let ledger = case.join(&options.gap_ledger);
        fs::File::options()
            .write(true)
            .open(&ledger)
            .and_then(|file| file.set_modified(std::time::SystemTime::now()))
            .map_err(|err| format!("refresh ledger mtime {}: {err}", ledger.display()))?;
        let actual_json = render_start_here_packet(&case, &options);
        let actual_md = render_start_here_markdown(&actual_json);
        // Issue #3872: funnel redirect targets anchor at the resolved --root,
        // so the machine prefix projects to `<cwd>/` before comparing against
        // the checked-in expectation (placeholder rule: loop_commands).
        let mut normalized_json = actual_json;
        project_renderer_cwd(&mut normalized_json);
        let normalized_md = project_cwd_text(&actual_md);
        let expected_json = read_packet(&case.join("expected/start-here.json"))?;
        assert_eq!(
            normalized_json, expected_json,
            "start-here JSON drift in {case_id}"
        );

        let expected_md = fs::read_to_string(case.join("expected/start-here.md"))
            .map_err(|err| format!("read expected start-here markdown for {case_id}: {err}"))?;
        assert_eq!(
            normalized_md.replace("\r\n", "\n"),
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

    fn temp_typescript_repo(name: &str) -> Result<PathBuf, String> {
        let path = write_temp_root(&env::temp_dir(), name)?;
        fs::write(
            path.join("package.json"),
            r#"{"name":"first-pr-typescript-test","private":true,"devDependencies":{"jest":"^30.0.0","typescript":"^5.8.0"}}"#,
        )
        .map_err(|err| format!("write temp package.json: {err}"))?;
        init_git_repo_with_initial_files(&path, &["package.json"])?;
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

    fn first_pr_args(root: &str, out: &str) -> Vec<String> {
        vec![
            "--root".to_string(),
            root.to_string(),
            "--base".to_string(),
            "HEAD".to_string(),
            "--out-dir".to_string(),
            out.to_string(),
        ]
    }

    #[cfg(unix)]
    #[test]
    fn unwritable_out_dir_names_out_dir_not_out() -> Result<(), String> {
        use crate::testing::unwritable_output::OutputDirFixture;

        let env = OutputDirFixture::unwritable("first-pr-ro", "reports")?;
        let root = OutputDirFixture::path_arg(&env.root)?;
        let out = OutputDirFixture::path_arg(&env.target)?;
        let error = match first_pr(&first_pr_args(root, out)) {
            Err(error) => error,
            Ok(()) => {
                return Err(
                    "unwritable --out-dir must fail while creating the output directory"
                        .to_string(),
                );
            }
        };
        assert!(error.contains(&format!("create {out} failed:")), "{error}");
        assert!(
            error.contains("write elsewhere with --out-dir PATH"),
            "first-pr must name --out-dir PATH, got {error}"
        );
        assert!(
            !error.contains("write elsewhere with --out PATH"),
            "first-pr must not name pilot's flag, got {error}"
        );
        Ok(())
    }

    #[test]
    fn occupying_file_out_dir_does_not_name_the_relocate_flag() -> Result<(), String> {
        use crate::testing::unwritable_output::OutputDirFixture;

        let env = OutputDirFixture::occupying_file("first-pr-file", "reports")?;
        let root = OutputDirFixture::path_arg(&env.root)?;
        let out = OutputDirFixture::path_arg(&env.target)?;
        let error = match first_pr(&first_pr_args(root, out)) {
            Err(error) => error,
            Ok(()) => return Err("file occupying --out-dir must fail".to_string()),
        };
        assert!(error.contains(&format!("create {out} failed:")), "{error}");
        assert!(
            !error.contains("write elsewhere"),
            "a file occupying --out-dir is not a not-writable tree: {error}"
        );
        Ok(())
    }
}
