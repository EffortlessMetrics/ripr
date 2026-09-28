//! Dispatch for `ripr agent` (including the legacy `ripr agent receipt`
//! alias).
//!
//! This is the CLI adapter layer only. Analysis, evaluation, and rendering
//! semantics live in `crate::app`, `crate::analysis`, and `crate::output`.
//! This module owns subcommand dispatch, output destination selection, and
//! exit mapping for the agent command family.

use crate::analysis;
use crate::app::agent_brief::{
    AgentBriefPolicy, AgentBriefResolvedWorkingSet, select_agent_brief_seams,
};
use crate::app::{self, OutputFormat};
use crate::cli::CommandError;
use crate::cli::agent::{
    AgentBriefOptions, AgentCommand, AgentPacketOptions, AgentReceiptOptions, AgentRepairOptions,
    AgentRepairPhase, AgentReviewSummaryOptions, AgentStartOptions, AgentStatusOptions,
    AgentVerifyExecuteOptions, AgentVerifyOptions, parse_agent_args,
};
use crate::cli::commands_agent_support::{
    build_agent_receipt_provenance, read_agent_verify_snapshot, resolve_agent_brief_working_set,
    validate_agent_receipt_verify_path, validate_agent_verify_snapshot_path,
};
use crate::cli::commands_context::{ensure_command_root, load_root_input_and_config};
use crate::config::load_for_root;
use crate::output;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::{
    fs::File,
    io::{BufWriter, Write},
};

use super::agent_dispatch;
use super::agent_gap_packet::render_agent_packet_from_gap_ledger;
use super::write_text_file;

/// Schema version of the `ripr agent repair --phase after` success envelope
/// (`kind: "repair_after_result"`). The envelope is its own versioned
/// contract: the agent verify 0.3 document rides unchanged under `verify`
/// and the agent status 0.1 document under `agent_status`, so every stdout
/// document's shape is identifiable from its `schema_version`. Refusals
/// after the verify render keep printing the bare agent verify 0.3 document
/// instead; typed refusals before it print `repair_after_refusal`.
const REPAIR_AFTER_RESULT_SCHEMA_VERSION: &str = "0.1";

/// Schema version of the `ripr agent repair --phase after` typed-refusal
/// document (`kind: "repair_after_refusal"`). A deliberate named refusal that
/// happens before any verify document exists (a diverged HEAD, drifted
/// analysis inputs, a no-movement verify refusal) exits 3 with this document
/// on stdout, so the phase keeps its one-stdout-document contract on every
/// exit-3 path. Its version is distinct from the success envelope's `0.1`
/// and the bare verify document's `0.3`, so every after-phase stdout shape
/// stays identifiable from `schema_version` alone.
const REPAIR_AFTER_REFUSAL_SCHEMA_VERSION: &str = "0.2";

pub(in crate::cli) fn agent(args: &[String]) -> Result<(), CommandError> {
    let command = parse_agent_args(args)?;
    if let Some(result) = agent_dispatch::run_agent_help_command(&command) {
        return result.map_err(CommandError::from);
    }

    match command {
        AgentCommand::Start(options) => run_agent_start(options).map_err(CommandError::from),
        AgentCommand::Brief(options) => run_agent_brief(options).map_err(CommandError::from),
        AgentCommand::Packet(options) => run_agent_packet(options).map_err(CommandError::from),
        // A deliberate named refusal (drifted analysis inputs, no movement)
        // maps to exit code 3, as it does inside `repair --phase after`.
        // Stdout stays empty on every refusal: the release negative corpus
        // (RIPR-SPEC-0134) treats a rendered stdout on rejection as a breach,
        // because the verify command's stdout is the verify artifact.
        AgentCommand::Verify(options) => run_agent_verify(options).map_err(|error| {
            if agent_verify_error_is_typed_refusal(&error) {
                CommandError::Decision(error)
            } else {
                CommandError::Failure(error)
            }
        }),
        // Typed refusals carry the Decision variant (exit code 3).
        AgentCommand::VerifyExecute(options) => run_agent_verify_execute(options),
        AgentCommand::Receipt(options) => run_agent_receipt(options).map_err(CommandError::from),
        AgentCommand::Status(options) => run_agent_status(options).map_err(CommandError::from),
        AgentCommand::ReviewSummary(options) => {
            run_agent_review_summary(options).map_err(CommandError::from)
        }
        // After-phase refusals carry the Decision variant (exit code 3).
        AgentCommand::Repair(options) => run_agent_repair(options),
        help_command @ (AgentCommand::Help
        | AgentCommand::StartHelp
        | AgentCommand::BriefHelp
        | AgentCommand::PacketHelp
        | AgentCommand::VerifyHelp
        | AgentCommand::VerifyExecuteHelp
        | AgentCommand::ReceiptHelp
        | AgentCommand::StatusHelp
        | AgentCommand::ReviewSummaryHelp
        | AgentCommand::RepairHelp) => agent_dispatch::run_agent_help_command(&help_command)
            .unwrap_or_else(|| Err("agent help command was not dispatched".to_string()))
            .map_err(CommandError::from),
    }
}

fn run_agent_start(options: AgentStartOptions) -> Result<(), String> {
    let json = options.json;
    let written = write_agent_start(options)?;
    if json {
        // Machine-readable mode: one JSON document with the same information
        // the prose lines carry. The prose output remains the default.
        let rendered = render_agent_start_json(&written)?;
        print!("{rendered}");
        return Ok(());
    }
    for line in agent_start_prose_lines(&written) {
        println!("{line}");
    }
    Ok(())
}

/// The default human output of `agent start`: one `Wrote <path>` line per
/// written workflow artifact, then the first missing-input command when one
/// exists.
fn agent_start_prose_lines(written: &AgentStartWritten) -> Vec<String> {
    let mut lines = written
        .paths
        .iter()
        .map(|path| format!("Wrote {}", path.display()))
        .collect::<Vec<_>>();
    if let Some(next) = &written.next_command {
        lines.push(format!("Next: {next}"));
    }
    lines
}

/// Render the `ripr agent start --json` document. Field names match the
/// workflow manifest's `outputs` block (`workflow_manifest`,
/// `commands_markdown`, `agent_brief`) and the agent status `next_command`
/// name; `next_command` is `null` when every workflow input is present.
fn render_agent_start_json(written: &AgentStartWritten) -> Result<String, String> {
    use crate::agent::loop_commands::{
        WORKFLOW_AGENT_BRIEF_ARTIFACT, WORKFLOW_COMMANDS_MARKDOWN_ARTIFACT,
        WORKFLOW_MANIFEST_ARTIFACT,
    };
    let paths = &written.paths;
    // Match each path by artifact identity, not write order: a future
    // reorder (or an added artifact) in `write_agent_start` must not
    // silently remap fields.
    let path_for = |artifact: &str| -> Option<String> {
        let file_name = std::path::Path::new(artifact).file_name()?;
        paths
            .iter()
            .find(|path| path.file_name() == Some(file_name))
            .map(|path| path.display().to_string())
    };
    let value = serde_json::json!({
        "schema_version": app::agent_workflow::AGENT_WORKFLOW_SCHEMA_VERSION,
        "tool": "ripr",
        "kind": "agent_start",
        "workflow": {
            "workflow_manifest": path_for(WORKFLOW_MANIFEST_ARTIFACT),
            "commands_markdown": path_for(WORKFLOW_COMMANDS_MARKDOWN_ARTIFACT),
            "agent_brief": path_for(WORKFLOW_AGENT_BRIEF_ARTIFACT),
        },
        "next_command": written.next_command,
    });
    output::json::render_pretty_with_newline(&value, "agent start")
}

/// Files written by `agent start` and the first missing-input command.
struct AgentStartWritten {
    paths: Vec<PathBuf>,
    next_command: Option<String>,
}

fn write_agent_start(options: AgentStartOptions) -> Result<AgentStartWritten, String> {
    ensure_command_root(&options.root, "agent start")?;
    let (input, config) = load_root_input_and_config(&options.root)?;

    let working_set = AgentBriefResolvedWorkingSet::seam_id(options.seam_id.clone());
    let (classified, _) =
        analysis::inventory_classified_seams_at_with_config(&input.root, &config)?;
    let selection = select_agent_brief_seams(
        &classified,
        &working_set,
        1,
        AgentBriefPolicy::from_config(&config),
    );
    if selection.top_seams.is_empty() {
        return Err(format!(
            "agent start seam_id {} was not found or is hidden by config. {}",
            options.seam_id,
            unknown_seam_id_hint(&options.root, &options.seam_id)
        ));
    }

    let out_dir = resolve_agent_start_out_dir(&input.root, &options.out_dir);
    std::fs::create_dir_all(&out_dir)
        .map_err(|err| format!("create {} failed: {err}", out_dir.display()))?;

    let agent_brief_json = output::agent_brief::render_agent_brief_json(
        &input.root,
        &input.mode,
        &config,
        &working_set,
        &selection,
    )?;
    let agent_brief_path = out_dir.join("agent-brief.json");
    write_text_file(&agent_brief_path, &agent_brief_json)?;

    let manifest = app::agent_workflow::build_agent_workflow_manifest(
        &input.root,
        &options.root,
        &input.mode,
        &out_dir,
        &options.seam_id,
        &agent_brief_json,
    )?;
    let workflow_json = output::agent_workflow::render_agent_workflow_json(&manifest)?;
    let commands_md = output::agent_workflow::render_agent_workflow_commands_md(&manifest);
    let workflow_path = out_dir.join("workflow.json");
    let commands_path = out_dir.join("commands.md");
    write_text_file(&workflow_path, &workflow_json)?;
    write_text_file(&commands_path, &commands_md)?;

    Ok(AgentStartWritten {
        paths: vec![workflow_path, commands_path, agent_brief_path],
        next_command: manifest
            .missing_inputs
            .first()
            .map(|next| next.command.clone()),
    })
}

fn run_agent_brief(options: AgentBriefOptions) -> Result<(), String> {
    ensure_command_root(&options.root, "agent brief")?;
    let (input, config) = load_root_input_and_config(&options.root)?;

    let working_set = resolve_agent_brief_working_set(&input.root, &options.working_set)?;
    let (classified, _) =
        analysis::inventory_classified_seams_at_with_config(&input.root, &config)?;
    let selection = select_agent_brief_seams(
        &classified,
        &working_set,
        options.max_seams,
        AgentBriefPolicy::from_config(&config),
    );
    let rendered = output::agent_brief::render_agent_brief_json(
        &input.root,
        &input.mode,
        &config,
        &working_set,
        &selection,
    )?;
    println!("{rendered}");
    Ok(())
}

fn run_agent_packet(options: AgentPacketOptions) -> Result<(), String> {
    ensure_command_root(&options.root, "agent packet")?;

    let rendered = render_agent_packet(&options)?;
    print!("{rendered}");
    Ok(())
}

/// Recovery for an unknown `--seam-id`. Cold agents most often pass the
/// `probe:...` finding ID that `ripr check` prints; the error must name where
/// seam IDs come from instead of leaving "not found" as a dead end. The
/// pilot command names the same root the failing call used.
fn unknown_seam_id_hint(root: &Path, seam_id: &str) -> String {
    // `bound_root` over the raw path, like the LSP routes: `display_path`
    // would turn a literal backslash in a Unix path into a separator (#4287).
    let root = crate::agent::loop_commands::shell_arg(&crate::agent::loop_commands::bound_root(
        &root.to_string_lossy(),
    ));
    let pilot = format!(
        "`ripr pilot --root {root}` to list current seam IDs with their exact `ripr agent repair` commands."
    );
    if seam_id.starts_with("probe:") {
        format!("`probe:...` is a `ripr check` finding ID, not a seam ID; run {pilot}")
    } else {
        format!("Run {pilot}")
    }
}

fn render_agent_packet(options: &AgentPacketOptions) -> Result<String, String> {
    if let (Some(gap_ledger), Some(gap_id)) = (&options.gap_ledger, &options.gap_id) {
        return render_agent_packet_from_gap_ledger(&options.root, gap_ledger, gap_id);
    }

    let seam_id = options.seam_id.as_deref().ok_or_else(|| {
        "agent packet requires --seam-id or --gap-ledger with --gap-id".to_string()
    })?;
    let config = load_for_root(&options.root)?;
    let (classified, _) =
        analysis::inventory_classified_seams_at_with_config(&options.root, &config)?;
    let entry = classified
        .iter()
        .find(|entry| entry.seam.id().as_str() == seam_id)
        .ok_or_else(|| {
            format!(
                "agent packet seam_id {seam_id} was not found. {}",
                unknown_seam_id_hint(&options.root, seam_id)
            )
        })?;

    let policy = AgentBriefPolicy::from_config(&config);
    if let Some(reason) = policy.omission_reason_for_class(entry.class) {
        return Err(format!("agent packet seam_id {seam_id} {reason}"));
    }

    Ok(output::agent_seam_packets::render_agent_seam_packet_json(
        entry,
    ))
}

fn run_agent_verify(options: AgentVerifyOptions) -> Result<(), String> {
    let rendered = render_agent_verify(&options)?;
    print!("{rendered}");
    Ok(())
}

fn render_agent_verify(options: &AgentVerifyOptions) -> Result<String, String> {
    let before_path =
        validate_agent_verify_snapshot_path(&options.root, &options.before, "--before")?;
    let after_path = validate_agent_verify_snapshot_path(&options.root, &options.after, "--after")?;
    let before_json = read_agent_verify_snapshot(&before_path, "before")?;
    let after_json = read_agent_verify_snapshot(&after_path, "after")?;
    let before_identity = crate::agent::artifact::validate_repo_exposure_artifact(
        &options.root,
        &before_json,
        "before",
    )?;
    let after_identity = crate::agent::artifact::validate_repo_exposure_artifact(
        &options.root,
        &after_json,
        "after",
    )?;
    crate::agent::artifact::validate_comparable_pair(&before_identity, &after_identity)
        .map_err(|error| format!("agent verify artifacts are incomparable: {error}"))?;
    crate::agent::artifact::validate_pair_lineage(&options.root, &before_identity, &after_identity)
        .map_err(|error| format!("agent verify artifacts are incomparable: {error}"))?;
    crate::agent::artifact::validate_verify_movement(&before_identity, &after_identity)
        .map_err(|error| format!("agent verify {error}"))?;
    let artifact_currentness = crate::agent::artifact::pair_currentness_label(
        &before_identity.currentness,
        &after_identity.currentness,
    );
    let report = output::outcome::targeted_test_outcome_report_from_json(
        &before_json,
        &after_json,
        output::outcome::display_path(&options.before),
        output::outcome::display_path(&options.after),
    )?;
    // Bind the verify result to the exact artifact bytes it compared (#2922
    // PR B): the validated content commitments ride in canonical output so a
    // later byte change to either artifact is detectable downstream.
    let binding = output::outcome::AgentVerifyArtifactBinding {
        before_content_sha256: before_identity.content_sha256,
        after_content_sha256: after_identity.content_sha256,
    };
    output::outcome::render_agent_verify_json_with_currentness(
        &report,
        Some(artifact_currentness),
        &binding,
    )
}

/// Exact `render_agent_verify` error for drifted analysis inputs.
const AGENT_VERIFY_INPUT_DRIFT_ERROR: &str =
    "agent verify artifacts are incomparable: analysis input identities differ";

/// Prefix of the `render_agent_verify` error for a no-movement refusal.
const AGENT_VERIFY_NO_MOVEMENT_PREFIX: &str =
    "agent verify no repository movement between before and after artifacts";

/// True when a `render_agent_verify` error is a deliberate named refusal
/// (input-identity drift or a no-movement verify refusal) rather than an
/// operational failure. Matched on the whole message or its leading clause,
/// never a substring: path-bearing operational errors echo user-supplied
/// paths, and a path must not turn an unreadable input into a refusal.
fn agent_verify_error_is_typed_refusal(error: &str) -> bool {
    error == AGENT_VERIFY_INPUT_DRIFT_ERROR || error.starts_with(AGENT_VERIFY_NO_MOVEMENT_PREFIX)
}

fn run_agent_verify_execute(options: AgentVerifyExecuteOptions) -> Result<(), CommandError> {
    ensure_command_root(&options.root, "agent verify-execute")?;
    let outcome = app::verification_execution::execute_verify_packet(
        &options.root,
        &options.packet,
        &options.result_json,
        options.authorize,
        options.cancel_after_ms,
    );
    // The typed disposition is the contract, so it reaches stdout on every
    // terminal state — including refusals. A typed refusal is a successfully
    // rendered blocking answer: it maps to the decision exit code 3 so an
    // orchestrator can branch on `0` executed, `3` refused (read the stdout
    // JSON), `2` could not complete. Only an uncommitted observation
    // (`verification_result_write_failed`) remains a Failure.
    print!("{}", outcome.rendered);
    if outcome.refused {
        return Err(CommandError::Decision(outcome.disposition.to_string()));
    }
    if outcome.failed {
        return Err(CommandError::from(outcome.disposition.to_string()));
    }
    Ok(())
}

fn run_agent_receipt(options: AgentReceiptOptions) -> Result<(), String> {
    run_agent_receipt_for_attempt(options, None, None)
}

fn run_agent_receipt_for_attempt(
    options: AgentReceiptOptions,
    attempt_id: Option<&str>,
    attempt_packet_path: Option<&Path>,
) -> Result<(), String> {
    ensure_command_root(&options.root, "agent receipt")?;

    let verify_path = validate_agent_receipt_verify_path(&options.root, &options.verify_json)?;
    let verify_json = std::fs::read_to_string(&verify_path).map_err(|err| {
        format!(
            "read agent receipt verify JSON {} failed: {err}",
            output::outcome::display_path(&verify_path)
        )
    })?;
    let validated =
        app::agent_receipt::validate_agent_receipt_verify_json(&options.root, &verify_json)?;
    if let Some(attempt_id) = attempt_id {
        let before_sha256 = validated
            .verify
            .get("inputs")
            .and_then(|inputs| inputs.get("before_content_sha256"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "agent verify JSON is missing before_content_sha256".to_string())?;
        crate::app::repair_attempt::validate_verify_binding(
            &options.root,
            attempt_id,
            &validated.input_paths.before,
            before_sha256,
        )?;
    }
    let compatibility_packet_path = options.root.join("target/ripr/workflow/agent-packet.json");
    let packet_path = attempt_packet_path.unwrap_or(&compatibility_packet_path);
    let attempts_root = options
        .root
        .join(crate::app::repair_attempt::REPAIR_ATTEMPT_DIRECTORY);
    let repair_attempt_binding = if packet_path.exists() || attempts_root.exists() {
        Some(crate::app::repair_attempt::receipt_binding(
            &options.root,
            &options.seam_id,
            packet_path,
            attempt_id,
        )?)
    } else {
        None
    };
    let input_paths = &validated.input_paths;
    let provenance = build_agent_receipt_provenance(
        &options.root,
        &options.verify_json,
        &verify_path,
        input_paths,
    )?;
    let root_display = output::outcome::display_path(&options.root);
    let analysis_outcome_path =
        app::analysis_outcome_artifact::analysis_outcome_artifact_path_for_verify(&verify_path)?;
    let analysis_outcome = match app::analysis_outcome_artifact::read_analysis_outcome_artifact_at(
        &options.root,
        &root_display,
        &analysis_outcome_path,
    ) {
        Ok(outcome) => {
            output::agent_receipt::AgentReceiptAnalysisOutcome::Present(Box::new(outcome))
        }
        Err(error) => {
            let status = match &error {
                app::analysis_outcome_artifact::AnalysisOutcomeArtifactError::Missing(_) => {
                    output::agent_receipt::AgentReceiptUnavailableStatus::Missing
                }
                app::analysis_outcome_artifact::AnalysisOutcomeArtifactError::Invalid(_) => {
                    output::agent_receipt::AgentReceiptUnavailableStatus::Invalid
                }
            };
            output::agent_receipt::AgentReceiptAnalysisOutcome::Unavailable {
                status,
                reason: error.to_string(),
            }
        }
    };
    let rendered = output::agent_receipt::render_agent_receipt_value_json(
        &validated.verify,
        output::outcome::display_path(&options.verify_json),
        &options.seam_id,
        options.test_changed.as_deref(),
        &options.commands_run,
        provenance,
        analysis_outcome,
    )?;
    let mut receipt: serde_json::Value = serde_json::from_str(&rendered)
        .map_err(|error| format!("parse rendered agent receipt failed: {error}"))?;
    if let Some(binding) = repair_attempt_binding {
        receipt["repair_attempt"] = binding;
    }
    let rendered = serde_json::to_string_pretty(&receipt)
        .map_err(|error| format!("serialize bound agent receipt failed: {error}"))?
        + "\n";

    match options.out {
        Some(path) => {
            let path = resolve_agent_receipt_out_path(&options.root, &path)?;
            super::write_text_file(&path, &rendered)
        }
        None => {
            print!("{rendered}");
            Ok(())
        }
    }
}

/// Anchor a relative `agent receipt --out` at the resolved `--root` (issue
/// #3967): the product renders `--out` relative next to an absolute
/// `--root`, so resolving it at the paste-site process CWD wrote genuine
/// receipts outside the selected root. An absolute `--out` passes through;
/// a relative one joins `--root`, resolved against the process working
/// directory exactly as `--root` itself resolves (mirrors the input-side
/// `validate_agent_receipt_artifact_path` root join and the #3872 redirect
/// anchor rule). Sibling `--out` surfaces keep their own contracts.
fn resolve_agent_receipt_out_path(root: &Path, out: &Path) -> Result<PathBuf, String> {
    if out.is_absolute() {
        return Ok(out.to_path_buf());
    }
    let base = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|err| format!("resolve agent receipt --out root failed: {err}"))?
            .join(root)
    };
    Ok(base.join(out))
}

fn run_agent_status(options: AgentStatusOptions) -> Result<(), String> {
    ensure_command_root(&options.root, "agent status")?;

    // The report inspects the fixed `target/ripr/workflow` and
    // `target/ripr/reports` artifact paths under `--root`, and its second
    // argument is the root as the user should retype it in the printed next
    // commands. Passing the workflow directory there rendered
    // `--root ./target/ripr/workflow`, a command that cannot run (#3893).
    // A non-default `--out` is refused rather than silently ignored until
    // the inspected paths can follow it.
    if let Some(dir) = &options.out_dir {
        let requested = resolve_agent_start_out_dir(&options.root, dir);
        let default_dir = options.root.join("target/ripr/workflow");
        if requested != default_dir {
            return Err(format!(
                "agent status --out {} is not supported: agent status reads the workflow artifacts under {}",
                dir.display(),
                default_dir.display()
            ));
        }
    }
    let report = app::agent_status::build_agent_status_report(&options.root, &options.root);
    if options.json {
        let rendered = app::agent_status::render_agent_status_json(&report)?;
        print!("{rendered}");
    } else {
        let rendered = app::agent_status::render_agent_status_markdown(&report);
        print!("{rendered}");
    }
    Ok(())
}

fn run_agent_review_summary(options: AgentReviewSummaryOptions) -> Result<(), String> {
    ensure_command_root(&options.root, "agent review-summary")?;

    let report =
        app::agent_review_summary::build_agent_review_summary_report(&options.root, &options.root);
    if options.json {
        let rendered = app::agent_review_summary::render_agent_review_summary_json(&report)?;
        print!("{rendered}");
    } else {
        let rendered = app::agent_review_summary::render_agent_review_summary_markdown(&report);
        print!("{rendered}");
    }
    Ok(())
}

/// Two-phase agent repair loop (#2443). Composes the existing 7 subcommands
/// into 2 phases:
/// - `--phase before`: runs before-snapshot + packet (the agent then edits in
///   the workspace between phases) and publishes a durable attempt ID.
/// - `--phase after`: consumes that exact attempt's retained inputs, then runs
///   after-snapshot + verify + receipt + status.
///
/// This reduces the 7-command loop to 2 while preserving the agent's control
/// over the edit step and the transaction's identity across fresh sessions.
///
/// When the before phase was bound to a Python repair-trust selection
/// (RIPR-SPEC-0176, #3568), the after phase re-verifies the retained binding
/// by digest before recording the applied edit, and publishes the apply-phase
/// record. The driver records no verification result, no static movement, and
/// no closure; #3570 owns the verification phase.
///
/// When an after phase refuses after it selected its attempt, the refusal is
/// recorded on that attempt through the attempt authority, so `ripr agent
/// status` reports it instead of repeating the refused command unannotated.
fn run_agent_repair(options: AgentRepairOptions) -> Result<(), CommandError> {
    let mut refusal = AfterPhaseRefusalContext::default();
    let result = run_agent_repair_phase(options, &mut refusal);
    if let (Err(error), Some((root, attempt_id))) = (&result, &refusal.selected_attempt)
        && let Err(record_error) = crate::app::repair_attempt::record_repair_attempt_after_refusal(
            root,
            attempt_id,
            &after_refusal_reason(error, &refusal.narration),
        )
    {
        eprintln!(
            "ripr: could not record the after-phase refusal on attempt `{}`: {record_error}",
            attempt_id.as_str()
        );
    }
    // A deliberate named refusal once the after phase selected its attempt
    // (typed) is recorded above and maps to the decision exit code 3.
    // Operational errors after selection stay ordinary failures: exit 2.
    if refusal.typed
        && !refusal.stdout_document_printed
        && let (Err(error), Some((_, attempt_id))) = (&result, &refusal.selected_attempt)
    {
        // Exit 3 means "read the stdout JSON document for the answer"
        // (docs/EXIT_CODES.md): a typed refusal before any verify document
        // exists still prints exactly one document, naming the cause and the
        // recovery the narration gave on stderr.
        match render_repair_after_refusal_json(attempt_id.as_str(), error, &refusal.narration) {
            Ok(rendered) => print!("{rendered}"),
            Err(render_error) => eprintln!("ripr: {render_error}"),
        }
    }
    if refusal.selected_attempt.is_some() && refusal.typed {
        result.map_err(CommandError::Decision)
    } else {
        result.map_err(CommandError::Failure)
    }
}

/// What an after phase that refuses leaves for the attempt record: the
/// attempt it selected, the narration it printed before the final error
/// (the named cause and the recovery), so `ripr agent status` can repeat the
/// same explanation instead of only the terse final error, and whether the
/// refusal was a deliberate named refusal rather than an operational error.
#[derive(Default)]
struct AfterPhaseRefusalContext {
    selected_attempt: Option<(PathBuf, crate::app::repair_attempt::RepairAttemptId)>,
    narration: Vec<String>,
    /// Set only at a deliberate named refusal (diverged HEAD, drifted
    /// analysis inputs, a no-movement verify refusal, or a replaced
    /// trust-binding manifest): a refusal the command narrates with its
    /// cause and recovery before returning. Only this maps to the decision
    /// exit code 3; operational errors after attempt selection (an
    /// unreadable retained packet or manifest, a failed snapshot write,
    /// failed receipt or apply-record publication) stay exit code 2.
    typed: bool,
    /// Set once the phase printed its one stdout document (the bare verify
    /// document on a post-verify refusal), so a refusal never prints a
    /// second document.
    stdout_document_printed: bool,
}

/// The typed-refusal stdout document of an after phase that refused with a
/// named cause before any verify document existed: the attempt, the final
/// error, and the narrated cause and recovery lines (the same lines stderr
/// carries and the attempt record keeps).
fn render_repair_after_refusal_json(
    attempt_id: &str,
    error: &str,
    narration: &[String],
) -> Result<String, String> {
    let document = serde_json::json!({
        "schema_version": REPAIR_AFTER_REFUSAL_SCHEMA_VERSION,
        "kind": "repair_after_refusal",
        "attempt_id": attempt_id,
        "error": error.trim(),
        "narration": narration,
    });
    serde_json::to_string_pretty(&document)
        .map(|rendered| format!("{rendered}\n"))
        .map_err(|error| format!("serialize after-phase refusal document failed: {error}"))
}

impl AfterPhaseRefusalContext {
    /// Prints one narration line and keeps it for the refusal record.
    fn narrate(&mut self, line: String) {
        eprintln!("ripr: {line}");
        self.narration.push(line);
    }
}

/// The recorded refusal: the final error, then the narration that named its
/// cause and recovery, as sentences. The attempt authority bounds its length.
fn after_refusal_reason(error: &str, narration: &[String]) -> String {
    let mut reason = error.trim().trim_end_matches('.').to_string();
    for line in narration {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        reason.push_str(". ");
        let mut chars = line.chars();
        if let Some(first) = chars.next() {
            reason.extend(first.to_uppercase());
            reason.push_str(chars.as_str().trim_end_matches('.'));
        }
    }
    reason
}

/// Runs one repair phase. `refusal` receives the attempt the after phase
/// selected and the narration it printed, so a refusal past that point can
/// be recorded on the attempt with its named cause and recovery.
fn run_agent_repair_phase(
    options: AgentRepairOptions,
    refusal: &mut AfterPhaseRefusalContext,
) -> Result<(), String> {
    let AgentRepairOptions {
        root,
        seam_id,
        attempt_id,
        phase,
        // The before-phase half runs the workflow only; the digest-bound
        // binding is produced in `cli::run` once the workflow artifacts exist
        // (see `persist_before_repair_attempt`).
        python_repair_trust: _,
        edit_authorization,
        verify_authorization,
        verify_rollback,
    } = options;

    match phase {
        AgentRepairPhase::Before => {
            let seam_id = seam_id.ok_or_else(|| {
                "agent repair --phase before lost its parsed seam identity".to_string()
            })?;
            ensure_command_root(&root, "agent repair --phase before")?;
            eprintln!(
                "ripr: agent repair --phase before for seam `{seam_id}` at {}",
                root.display()
            );

            // Render and admit the packet first (F15-12): a seam whose repair
            // packet names no test file ripr may edit refuses here, before any
            // workflow artifact is written, so neither this phase nor
            // `ripr agent status` reads as a started repair.
            let packet = render_agent_packet(&AgentPacketOptions {
                root: root.clone(),
                seam_id: Some(seam_id.clone()),
                gap_ledger: None,
                gap_id: None,
                json: true,
            })?;
            let policy =
                crate::app::repair_attempt::edit_cage_policy_from_packet(&packet, &seam_id)
                    .map_err(|error| before_phase_refusal(&seam_id, &error))?;
            crate::edit_cage::validate_build_output_precondition(&root, &policy).map_err(
                |error| {
                    format!("{error} No workflow was prepared and no repair attempt was started.")
                },
            )?;

            // Compose existing commands: start (creates workflow + brief) +
            // packet. The start step's `Next: ripr check ...` hint is dropped
            // because this phase writes that before snapshot itself; printing
            // it sent users to redo it.
            let started = write_agent_start(AgentStartOptions {
                root: root.clone(),
                seam_id: seam_id.clone(),
                out_dir: std::path::PathBuf::from("target/ripr/workflow"),
                json: false,
            })?;
            for path in &started.paths {
                eprintln!("ripr: wrote {}", path.display());
            }

            let before = root.join("target/ripr/workflow/before.repo-exposure.json");
            write_agent_repo_exposure_snapshot(&root, &before)?;

            let packet_path = root.join("target/ripr/workflow/agent-packet.json");
            write_text_file(&packet_path, &packet)?;
            eprintln!("ripr: wrote {}", packet_path.display());
            // Stdout carries the packet JSON when it is piped or redirected,
            // which is how agents and scripts read it. A terminal reader gets
            // a short summary instead of ~13 KB of JSON (F15-7); the packet
            // file above is the same bytes either way.
            print!(
                "{}",
                before_phase_stdout(
                    &packet,
                    "target/ripr/workflow/agent-packet.json",
                    std::io::stdout().is_terminal(),
                )
            );
            // "Complete" and the next step are printed once the attempt is
            // published (`cli::persist_before_repair_attempt`), so a refusal
            // there is never preceded by a completion line.
            Ok(())
        }
        AgentRepairPhase::After => {
            ensure_command_root(&root, "agent repair --phase after")?;
            let attempt = crate::app::repair_attempt::resolve_awaiting_repair_attempt(
                &root,
                attempt_id.as_deref(),
                seam_id.as_deref(),
            )?;
            refusal.selected_attempt = Some((root.clone(), attempt.attempt_id.clone()));
            eprintln!(
                "ripr: agent repair --phase after for attempt `{}` (seam `{}`) at {}",
                attempt.attempt_id.as_str(),
                attempt.seam_id,
                root.display()
            );
            eprintln!(
                "ripr: consuming attempt manifest {}",
                attempt.manifest_path.display()
            );

            // The retained before snapshot and packet are the transaction's
            // authority. The repository-global after, verify, receipt, and
            // status paths remain compatibility projections for existing
            // review and cockpit consumers.
            let before = attempt.before_snapshot_path.clone();
            let after = root.join("target/ripr/workflow/after.repo-exposure.json");
            write_agent_repo_exposure_snapshot(&root, &after)?;

            let packet_path = attempt.packet_path.clone();
            let packet_bytes = std::fs::read(&packet_path).map_err(|error| {
                format!(
                    "read retained repair packet {} failed: {error}",
                    packet_path.display()
                )
            })?;
            let packet_text = String::from_utf8(packet_bytes.clone())
                .map_err(|error| format!("retained repair packet is not UTF-8: {error}"))?;
            let cage_policy = crate::app::repair_attempt::edit_cage_policy_from_packet(
                &packet_text,
                &attempt.seam_id,
            )?;

            // The trust binding, when the attempt carries one, is re-verified
            // by digest immediately before the applied edit is recorded.
            let retained_binding = crate::app::python_repair_binding::load_retained_binding(
                &root,
                &attempt.attempt_id,
            )?;
            let verified_binding = match &retained_binding {
                Some(binding) => Some(crate::app::python_repair_binding::reverify_for_apply(
                    &attempt.seam_id,
                    &cage_policy,
                    &packet_bytes,
                    binding,
                    &edit_authorization,
                )?),
                None => {
                    if edit_authorization.authorized {
                        return Err(
                            "agent repair --edit-authorized/--edit-authority require an attempt whose before phase recorded a python repair-trust binding; this attempt is not trust-bound"
                                .to_string(),
                        );
                    }
                    None
                }
            };

            // An ordinary attempt admits commits made on top of its prepared
            // head (a committed focused test) and the edit cage evaluates
            // every committed path. A HEAD that no longer descends from the
            // prepared head is refused here, before anything is finished, so
            // the attempt stays awaiting the edit and the recovery below can
            // restore the prepared head. A trust-bound attempt keeps its
            // exact-head rule and its typed `stale` record. The rule is the
            // attempt authority's, which `ripr agent status` reads too.
            let head_movement = match crate::app::repair_attempt::after_phase_head_admission_by_id(
                &root,
                &attempt.attempt_id,
            )? {
                crate::app::repair_attempt::AfterPhaseHeadAdmission::Current { movement } => {
                    movement
                }
                crate::app::repair_attempt::AfterPhaseHeadAdmission::FinishesStale { .. } => {
                    crate::edit_cage::HeadMovement::RequireBaselineHead
                }
                crate::app::repair_attempt::AfterPhaseHeadAdmission::RefusedDiverged {
                    current_head,
                } => {
                    for line in crate::app::repair_attempt::diverged_head_recovery(
                        &crate::agent::loop_commands::display_path(&root),
                        attempt.attempt_id.as_str(),
                        &attempt.seam_id,
                        &attempt.repository_head,
                        &current_head,
                    )
                    .lines()
                    {
                        refusal.narrate(line);
                    }
                    // A deliberate named refusal: narrated cause and recovery
                    // above, so it maps to the decision exit code 3.
                    refusal.typed = true;
                    return Err(format!(
                        "repair attempt `{}` cannot finish: HEAD {current_head} does not descend from its before-phase head {}",
                        attempt.attempt_id.as_str(),
                        attempt.repository_head
                    ));
                }
            };

            // Review summaries consume the canonical diff-scoped producer
            // outcome. Generate it from the same current root before issuing
            // the receipt so the built-in repair route cannot report a clean
            // review packet without completeness evidence.
            write_agent_analysis_outcome(&root)?;

            let verify_options = AgentVerifyOptions {
                root: root.clone(),
                before,
                after,
                json: true,
            };

            let verify_json = root.join("target/ripr/workflow/agent-verify.json");
            let rendered_verify = match render_agent_verify(&verify_options) {
                Ok(rendered) => rendered,
                Err(error) => {
                    // Only deliberate named refusals are typed (exit 3):
                    // drifted analysis inputs and a no-movement verify
                    // refusal. Snapshot canonicalization, reads, artifact
                    // validation, and rendering failures are operational
                    // (exit 2) even though the attempt was already selected.
                    if agent_verify_error_is_typed_refusal(&error) {
                        if error == AGENT_VERIFY_INPUT_DRIFT_ERROR {
                            for line in repair_after_input_drift_lines(&root, &attempt) {
                                refusal.narrate(line);
                            }
                        }
                        refusal.typed = true;
                    }
                    return Err(error);
                }
            };
            write_text_file(&verify_json, &rendered_verify)?;

            // Stdout carries exactly one JSON document on every path, like
            // every other agent command. The verify outcome is held until the
            // tail below settles: on success the single document is the
            // repair-after-result envelope carrying the verify result under
            // `verify` and the status report under `agent_status`; when the
            // tail refuses, the verify document alone is printed — the
            // refusal bytes this phase always produced, and still one document
            // an orchestrator can parse with one JSON.parse call.
            let after_tail = |refusal: &mut AfterPhaseRefusalContext| -> Result<String, String> {
                use crate::app::python_repair_binding::{
                    ManifestConfirmationError, confirm_manifest_unchanged, write_apply_record,
                };
                use crate::app::repair_attempt::{
                    finish_repair_attempt, restore_repair_attempt_to_awaiting_edit,
                };

                // The retained binding's manifest bytes are confirmed again
                // immediately before the durable finish: the apply
                // verification ran before several expensive operations, and a
                // manifest replaced inside that window must refuse instead of
                // silently advancing the attempt against replaced trust data.
                // This is a deliberate named refusal: it maps to exit code 3.
                if let Some(binding) = &retained_binding
                    && let Err(error) = confirm_manifest_unchanged(binding)
                {
                    // Only a manifest replaced inside the late window is a
                    // deliberate named refusal (exit 3); a manifest or
                    // retained binding that cannot be read or validated is
                    // operational (exit 2).
                    if matches!(error, ManifestConfirmationError::Replaced(_)) {
                        refusal.typed = true;
                    }
                    return Err(error.into());
                }

                // Finish only after all command-owned after artifacts exist.
                // This makes the durable delta the exact delta the receipt
                // binds, while the receipt itself remains outside the measured
                // edit window.
                let cage_after =
                    finish_repair_attempt(&root, &attempt.attempt_id, &packet_path, head_movement)?;
                eprintln!(
                    "ripr: edit-cage verdict for attempt `{}`: {:?}",
                    cage_after.attempt_id.as_str(),
                    cage_after.verdict.status
                );
                for line in repair_after_cage_recovery_lines(
                    &root,
                    &attempt.seam_id,
                    &attempt.repository_head,
                    &cage_after,
                ) {
                    eprintln!("ripr: {line}");
                }

                // The receipt can refuse (for example an escape verdict is not
                // receipt-ready). The refusal must not swallow the typed apply
                // evidence, so the outcome is carried to the end and the apply
                // record is published either way.
                let test_changed = authored_test_changed(&cage_policy, &cage_after.verdict);
                let receipt_result = run_agent_receipt_for_attempt(
                    AgentReceiptOptions {
                        root: root.clone(),
                        verify_json: verify_json.clone(),
                        seam_id: attempt.seam_id.clone(),
                        test_changed,
                        commands_run: Vec::new(),
                        json: true,
                        out: Some(root.join("target/ripr/reports/agent-receipt.json")),
                    },
                    Some(cage_after.attempt_id.as_str()),
                    Some(&packet_path),
                );

                // The status report the finished after phase embeds in its
                // single stdout document. Built here it reads exactly what
                // `ripr agent status --json` would print at this point: after
                // the finish and the receipt write, before the apply record.
                let status_report = app::agent_status::build_agent_status_report(&root, &root);
                let status_rendered = app::agent_status::render_agent_status_json(&status_report)?;

                // The apply record is published last: the receipt re-evaluates
                // the edit cage over the exact delta finish measured, so no
                // artifact write may land between finish and the receipt
                // binding.
                // The retained binding's manifest bytes are confirmed once
                // more immediately before the record write: the earlier
                // confirmation ran before the durable finish, so a manifest
                // replaced inside that finalize window must refuse here
                // instead of publishing an apply record against replaced trust
                // data. The refusal restores the attempt to awaiting_edit, so
                // the identical retry re-verifies everything.
                let mut apply_record_result: Result<(), String> = Ok(());
                let apply_inputs = (&retained_binding, &verified_binding);
                if let (Some(binding), Some(verified)) = apply_inputs {
                    // A manifest replaced inside the finalize window is a
                    // deliberate named refusal (exit code 3); a manifest or
                    // retained binding that cannot be read or validated, and
                    // a record write failure after a successful confirmation,
                    // are operational (exit code 2).
                    if let Err(error) = confirm_manifest_unchanged(binding) {
                        apply_record_result = Err(error.message().to_string());
                        if matches!(error, ManifestConfirmationError::Replaced(_)) {
                            refusal.typed = true;
                        }
                    } else {
                        let record_outcome = write_apply_record(
                            &root,
                            &attempt.attempt_id,
                            &binding.artifact_sha256,
                            verified,
                            edit_authorization.authority.as_deref().unwrap_or_default(),
                            &cage_after,
                        );
                        match record_outcome {
                            Ok(apply_record_path) => {
                                eprintln!(
                                    "ripr: python repair-trust apply record: {}",
                                    apply_record_path.display()
                                );
                            }
                            Err(error) => {
                                // Finish already advanced the durable state, so a
                                // failed record publication must restore the
                                // attempt to awaiting_edit: the identical retry is
                                // otherwise rejected and the record could never be
                                // recreated.
                                match restore_repair_attempt_to_awaiting_edit(
                                    &root,
                                    &attempt.attempt_id,
                                ) {
                                    Ok(()) => {
                                        eprintln!(
                                            "ripr: apply record publication failed; the attempt was restored to awaiting_edit for a retry"
                                        );
                                        apply_record_result = Err(error);
                                    }
                                    Err(restore_error) => {
                                        apply_record_result = Err(format!(
                                            "{error}; rolling the attempt back for a retry also failed: {restore_error}"
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
                receipt_result?;
                apply_record_result?;

                Ok(status_rendered)
            };
            let status_rendered = match after_tail(&mut *refusal) {
                Ok(status_rendered) => status_rendered,
                Err(error) => {
                    print!("{rendered_verify}");
                    refusal.stdout_document_printed = true;
                    return Err(error);
                }
            };
            let document: serde_json::Value = serde_json::from_str(&rendered_verify)
                .map_err(|error| format!("parse rendered agent verify JSON failed: {error}"))?;
            let status_document: serde_json::Value = serde_json::from_str(&status_rendered)
                .map_err(|error| format!("parse rendered agent status JSON failed: {error}"))?;
            // The success output is its own versioned envelope
            // (`repair_after_result`), not a mutated verify document: the
            // verify outcome already owns the top-level `status` name
            // (`advisory`), so splicing `agent_status` into the 0.3 document
            // would leave two same-version documents with different shapes.
            // The verify document keeps every field, name, and value under
            // `verify`, and the status report rides beside it under
            // `agent_status`.
            let envelope = serde_json::json!({
                "schema_version": REPAIR_AFTER_RESULT_SCHEMA_VERSION,
                "kind": "repair_after_result",
                "verify": document,
                "agent_status": status_document,
            });
            let combined = serde_json::to_string_pretty(&envelope).map_err(|error| {
                format!("serialize after-phase result document failed: {error}")
            })?;
            println!("{combined}");

            let receipt_path = root.join("target/ripr/reports/agent-receipt.json");
            for line in repair_after_summary_lines(&receipt_path) {
                eprintln!("ripr: {line}");
            }
            eprintln!(
                "ripr: after phase complete. Receipt: {}",
                receipt_path.display()
            );
            Ok(())
        }
        AgentRepairPhase::Verify => {
            ensure_command_root(&root, "agent repair --phase verify")?;
            let attempt_id = attempt_id.as_deref().ok_or_else(|| {
                "agent repair --phase verify lost its parsed attempt identity".to_string()
            })?;
            eprintln!(
                "ripr: agent repair --phase verify for attempt `{attempt_id}` at {}",
                root.display()
            );
            let receipt_path = app::python_repair_verification::run_verification_phase(
                app::python_repair_verification::VerificationOptions {
                    root: &root,
                    attempt_id,
                    authorization: app::python_repair_verification::VerifyAuthorization {
                        authorized: verify_authorization.authorized,
                        authority: verify_authorization.authority.clone(),
                    },
                    rollback: verify_rollback,
                },
            )?;
            let rendered = std::fs::read_to_string(&receipt_path).map_err(|error| {
                format!(
                    "read verification receipt {} failed: {error}",
                    receipt_path.display()
                )
            })?;
            print!("{rendered}");
            eprintln!(
                "ripr: verification receipt: {} (immutable; execution and static movement are separate observations)",
                receipt_path.display()
            );
            Ok(())
        }
    }
}

fn write_agent_analysis_outcome(root: &Path) -> Result<(), String> {
    let (mut input, config) = load_root_input_and_config(root)?;
    // Match the bare `ripr check --root <root> --format json` route used by
    // generated workflows: default-base resolution remains owned by the
    // analysis loader rather than being frozen to origin/main here.
    input.base = None;
    input.format = OutputFormat::Json;
    input.git_timeout = Some(app::default_cli_git_timeout());
    let output = app::check_workspace_with_config(input, &config)?;
    let rendered = app::render_check_with_config(&output, &OutputFormat::Json, &config)?;
    write_text_file(
        &root.join(crate::agent::loop_commands::WORKFLOW_ANALYSIS_OUTCOME_ARTIFACT),
        &rendered,
    )
}

fn write_agent_repo_exposure_snapshot(root: &Path, path: &Path) -> Result<(), String> {
    let config = load_for_root(root)?;
    let (classified, limit_info) =
        analysis::inventory_classified_seams_at_with_config(root, &config)?;
    let ts_guidance = output::render::detect_ts_full_repo_guidance_pub(root, &classified);
    let python_guidance =
        output::render::detect_python_repo_exposure_guidance_pub(root, &classified);
    let context = crate::agent::artifact::RepoExposureArtifactContext::for_repo_exposure(
        root.to_path_buf(),
        "ready".to_string(),
        None,
        &config,
    )?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    let temporary_path = path.with_extension(format!("json.tmp-{}", std::process::id()));
    let write_result = (|| -> Result<(), String> {
        let file = File::create(&temporary_path)
            .map_err(|err| format!("create {} failed: {err}", temporary_path.display()))?;
        let mut writer = BufWriter::new(file);
        output::repo_exposure::write_repo_exposure_json_with_context(
            &classified,
            limit_info.as_ref(),
            ts_guidance.as_ref(),
            python_guidance.as_ref(),
            &context,
            &mut writer,
        )?;
        writer
            .flush()
            .map_err(|err| format!("flush {} failed: {err}", temporary_path.display()))?;
        Ok(())
    })();
    if let Err(err) = write_result {
        let _ = std::fs::remove_file(&temporary_path);
        return Err(err);
    }

    // Publish only a complete snapshot. Unix rename replaces atomically; on
    // Windows the existing command-owned target must be unlinked first.
    #[cfg(windows)]
    if path.exists()
        && let Err(err) = std::fs::remove_file(path)
    {
        let _ = std::fs::remove_file(&temporary_path);
        return Err(format!("remove {} failed: {err}", path.display()));
    }
    if let Err(err) = std::fs::rename(&temporary_path, path) {
        let _ = std::fs::remove_file(&temporary_path);
        return Err(format!("publish {} failed: {err}", path.display()));
    }
    Ok(())
}

/// Narration for the repair after phase: the receipt's summary, or a line
/// naming the receipt that could not be read so the gap is visible rather
/// than silently empty.
fn repair_after_summary_lines(receipt_path: &Path) -> Vec<String> {
    match std::fs::read_to_string(receipt_path) {
        Ok(receipt) => repair_receipt_summary_lines(&receipt),
        Err(error) => vec![format!(
            "could not read {} to summarize the result: {error}",
            receipt_path.display()
        )],
    }
}

/// Upper bound on cage violations the after-phase narration lists; the attempt
/// manifest retains all of them.
const CAGE_RECOVERY_MAX_VIOLATIONS: usize = 10;

/// The receipt's `test_changed` for an after phase: the attempt's selected
/// test file, named only when the edit cage measured it changing and found
/// nothing else wrong. The value is the cage's own normalized path; no test
/// name is inferred.
fn authored_test_changed(
    policy: &crate::edit_cage::EditCagePolicy,
    verdict: &crate::edit_cage::EditCageVerdict,
) -> Option<String> {
    if verdict.status == crate::edit_cage::EditCageVerdictStatus::Compliant {
        let target = policy.selected_target.path();
        if verdict.changed_paths.iter().any(|path| path == target) {
            return Some(target.to_string());
        }
    }
    None
}

/// Recovery narration for an after phase whose attempt did not finish
/// compliant and current. Such an attempt is terminal: re-running it, or the
/// receipt command `agent status` projects from the workflow artifacts,
/// refuses. The lines name each cage violation (bounded) and the one route
/// that recovers: a new attempt prepared while the gap still exists.
fn repair_after_cage_recovery_lines(
    root: &Path,
    seam_id: &str,
    before_head: &str,
    after: &crate::app::repair_attempt::RepairAttemptAfter,
) -> Vec<String> {
    use crate::agent::loop_commands::{display_path, shell_arg};
    use crate::edit_cage::EditCageVerdictStatus;

    if after.current && after.verdict.status == EditCageVerdictStatus::Compliant {
        return Vec::new();
    }
    let attempt_id = after.attempt_id.as_str();
    let mut lines = Vec::new();
    if !after.current {
        lines.push(format!(
            "attempt `{attempt_id}` is stale: repository HEAD moved from {} to {} after its before phase.",
            short_head(before_head),
            short_head(&after.repository_head)
        ));
    }
    let violations = &after.verdict.violations;
    if !violations.is_empty() {
        lines.push(format!(
            "the edit cage refused {} path(s); only the packet's allowed test surface may change:",
            violations.len()
        ));
        for violation in violations.iter().take(CAGE_RECOVERY_MAX_VIOLATIONS) {
            lines.push(format!(
                "  {} ({:?}): {}",
                violation.path, violation.kind, violation.reason
            ));
        }
        if violations.len() > CAGE_RECOVERY_MAX_VIOLATIONS {
            lines.push(format!(
                "  ... and {} more; see the `after` block of the attempt manifest.",
                violations.len() - CAGE_RECOVERY_MAX_VIOLATIONS
            ));
        }
        lines.extend(untracked_output_hints(root, after));
    }
    let root_arg = shell_arg(&display_path(root));
    let seam_arg = shell_arg(seam_id);
    lines.push(format!(
        "attempt `{attempt_id}` is terminal and cannot produce a receipt; re-running it or `ripr agent receipt` will refuse."
    ));
    // A committed edit cannot be stashed: when HEAD moved, the recovery
    // first uncommits it so the rest of the route applies unchanged.
    let uncommit = if after.repository_head == before_head {
        ""
    } else {
        "if you committed the test edit or a refused change, uncommit it first (for example `git reset --soft HEAD~1` when it is the last commit; the changes stay in the worktree), "
    };
    lines.push(format!(
        "to recover: {uncommit}undo the refused changes, set your test edit aside (for example `git stash`), run `ripr agent repair --root {root_arg} --seam-id {seam_arg} --phase before` while the gap still exists, restore the test edit (`git stash pop`), then run the new --attempt command it prints."
    ));
    lines
}

/// Names refused untracked paths with recovery guidance for the declared
/// build directory and possible shell redirects elsewhere in the checkout.
/// The cage cannot attribute a path to a particular writer, so this narration
/// leaves its verdict unchanged. An unreadable baseline yields no hint.
fn untracked_output_hints(
    root: &Path,
    after: &crate::app::repair_attempt::RepairAttemptAfter,
) -> Vec<String> {
    use crate::edit_cage::EditCageViolationKind;

    let Ok(baseline) = crate::app::repair_attempt::load_edit_cage_baseline(root, &after.attempt_id)
    else {
        return Vec::new();
    };
    let build_output = baseline.policy().ignored_build_output.as_ref();
    let (build_paths, other_paths): (Vec<_>, Vec<_>) = after
        .verdict
        .violations
        .iter()
        .filter(|violation| violation.kind == EditCageViolationKind::OutsideAllowedSurface)
        .filter(|violation| baseline.index_entry(&violation.path).is_none())
        .take(CAGE_RECOVERY_MAX_VIOLATIONS)
        .partition(|violation| build_output.is_some_and(|rule| rule.matches(&violation.path)));
    let mut hints = Vec::new();
    if let Some(rule) = build_output.filter(|_| !build_paths.is_empty()) {
        let paths = build_paths
            .iter()
            .map(|violation| format!("`{}`", violation.path))
            .collect::<Vec<_>>();
        hints.push(format!(
            "Refused untracked paths under Cargo's declared build directory `{}/`: {}. Check the effective Git ignore rules: generated build output must be ignored before starting a new repair attempt (for example `/{}/` in .gitignore). Tracked and untracked-but-not-ignored files remain subject to the edit cage; these paths are not attributed to redirected ripr output.",
            rule.path(), paths.join(", "), rule.path()
        ));
    }
    let untracked = other_paths
        .iter()
        .map(|violation| format!("`{}`", violation.path))
        .collect::<Vec<_>>();
    if untracked.is_empty() {
        return hints;
    }
    hints.push(format!(
        "{} {} not tracked by Git when the before phase ran. If {} a file you redirected ripr output into (for example `> packet.json` or `2> before.err`), the edit cage counts it as an edit: delete it, and redirect `agent repair` output outside the checkout or under target/ripr/ (the before phase already writes the packet to target/ripr/workflow/agent-packet.json).",
        untracked.join(", "),
        if untracked.len() == 1 { "was" } else { "were" },
        if untracked.len() == 1 {
            "it is"
        } else {
            "one is"
        }
    ));
    hints
}

/// Recovery narration for an after phase refused because the analysis input
/// identity moved between the phases. The attempt is not finished, so it is
/// still awaiting the edit: the lines name the changed inputs and the rerun.
fn repair_after_input_drift_lines(
    root: &Path,
    attempt: &crate::app::repair_attempt::ResolvedRepairAttempt,
) -> Vec<String> {
    use crate::agent::loop_commands::{display_path, shell_arg};

    let root_arg = shell_arg(&display_path(root));
    let attempt_arg = shell_arg(attempt.attempt_id.as_str());
    let seam_arg = shell_arg(&attempt.seam_id);
    let mut lines = Vec::new();
    match crate::app::repair_attempt::analysis_input_changes(root, &attempt.attempt_id) {
        Ok(paths) if !paths.is_empty() => {
            lines.push(format!(
                "analysis inputs changed after the before phase: {}. The before and after snapshots must analyze the same Cargo manifests, Git-tracked Cargo.lock files, and ripr.toml (an untracked Cargo.lock that the build writes does not count).",
                paths.join(", ")
            ));
            lines.push(format!(
                "attempt `{}` was not finished and is still awaiting the focused test edit.",
                attempt.attempt_id.as_str()
            ));
            // A committed input change cannot be restored in the worktree
            // alone: the edit cage also reads the committed range.
            let uncommit = match crate::app::repair_attempt::attempt_head_lineage(
                root,
                &attempt.repository_head,
            ) {
                Ok(crate::app::repair_attempt::AttemptHeadLineage::Prepared) => String::new(),
                _ => format!(
                    "if a commit made after the before phase changed them, first run `git reset --soft {}` (the changes stay in the worktree); then ",
                    attempt.repository_head
                ),
            };
            // Name the untrack route only when a lockfile is among the
            // changed inputs; a manifest-only change has no lockfile to
            // untrack, and the advice would send the reader after the wrong
            // file.
            let untrack = paths
                .iter()
                .find(|path| Path::new(path.as_str()).file_name() == Some("Cargo.lock".as_ref()))
                .map(|path| {
                    format!(", or `git rm --cached {path}` for a lockfile that became tracked")
                })
                .unwrap_or_default();
            lines.push(format!(
                "to recover: {uncommit}restore those files to their before-phase state (for example `git checkout {} -- <path>`{untrack}), then rerun `ripr agent repair --root {root_arg} --attempt {attempt_arg} --phase after`. To keep the change, set your test edit aside, run `ripr agent repair --root {root_arg} --seam-id {seam_arg} --phase before`, restore the edit, then run the new --attempt command it prints.",
                attempt.repository_head
            ));
        }
        Ok(_) => {
            lines.push(
                "no Cargo manifest, Git-tracked Cargo.lock, or ripr.toml changed after the before phase, so the analyzer build or its configuration differs (for example ripr was reinstalled between the phases)."
                    .to_string(),
            );
            lines.push(format!(
                "to recover: set your test edit aside, run `ripr agent repair --root {root_arg} --seam-id {seam_arg} --phase before` with the ripr you will use for the after phase, restore the edit, then run the new --attempt command it prints."
            ));
        }
        Err(error) => lines.push(format!(
            "could not determine which analysis inputs changed: {error}"
        )),
    }
    lines
}

/// A before phase refused because the seam's repair packet cannot bound a
/// test-only edit. Names the seam and the reason in plain words, says that
/// nothing was started, and points at the surfaces that only offer a repair
/// start for seams that pass this check.
fn before_phase_refusal(seam_id: &str, error: &str) -> String {
    format!(
        "seam `{seam_id}` has no test file ripr can route a repair to, so no repair attempt was started. Pick a seam whose `ripr pilot` output or review card shows a repair start. Cause: {error}"
    )
}

/// What the before phase prints on stdout: the packet JSON for a pipe or
/// file, a short summary for a terminal. A packet the summary cannot read
/// falls back to the JSON, so nothing is hidden.
fn before_phase_stdout(packet: &str, packet_path: &str, terminal: bool) -> String {
    if terminal && let Some(summary) = before_phase_summary(packet, packet_path) {
        return summary;
    }
    packet.to_string()
}

fn before_phase_summary(packet: &str, packet_path: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(packet).ok()?;
    let item = value.get("packets")?.as_array()?.first()?;
    let text = |pointer: &str| {
        item.pointer(pointer)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
    };
    let seam_id = text("/seam_id")?;
    let test_file = text("/recommended_test/file")?;
    let mut lines = Vec::new();
    let location = match (
        text("/file"),
        item.get("line").and_then(serde_json::Value::as_u64),
    ) {
        (Some(file), Some(line)) => format!(" at {file}:{line}"),
        (Some(file), None) => format!(" at {file}"),
        _ => String::new(),
    };
    let owner = text("/owner")
        .map(|owner| format!(" in {owner}"))
        .unwrap_or_default();
    lines.push(format!(
        "Repair prepared for seam {seam_id}{location}{owner}."
    ));
    if let Some(expression) = text("/changed_expression") {
        lines.push(format!("  changed behavior: {expression}"));
    }
    if let Some(missing) = text("/missing_discriminators/0/value") {
        lines.push(format!("  missing discriminator: {missing}"));
    }
    match text("/recommended_test/name") {
        Some(name) => lines.push(format!(
            "  edit one test file: {test_file} (suggested test `{name}`); leave production code unchanged"
        )),
        None => lines.push(format!(
            "  edit one test file: {test_file}; leave production code unchanged"
        )),
    }
    if let Some(assertion) = text("/suggested_assertions/0") {
        lines.push(format!("  assertion shape: {assertion}"));
    }
    lines.push(format!(
        "  full repair packet (JSON): {packet_path}; stdout carries it when piped"
    ));
    Some(lines.join("\n") + "\n")
}

fn short_head(head: &str) -> &str {
    head.get(..12).unwrap_or(head)
}

/// One-line human result for the repair after phase, read from the receipt
/// that owns it. The JSON streams on stdout carry the full evidence; this
/// names the movement so a person does not have to parse them. A receipt
/// missing these fields yields no summary rather than a guessed one.
fn repair_receipt_summary_lines(receipt: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(receipt) else {
        return Vec::new();
    };
    let text = |pointer: &str| value.pointer(pointer).and_then(serde_json::Value::as_str);
    let mut lines = Vec::new();
    if let (Some(seam_id), Some(before), Some(after), Some(movement)) = (
        text("/seam/seam_id"),
        text("/provenance/before_class"),
        text("/provenance/after_class"),
        text("/provenance/movement"),
    ) {
        let summary = text("/summary/next_action/summary")
            .map(|summary| format!(" {summary}"))
            .unwrap_or_default();
        lines.push(format!(
            "result for seam `{seam_id}`: {before} -> {after} ({movement}).{summary}"
        ));
    }
    // Static movement is not a test result: a failing focused test still
    // reads `improved`. Say so before the next step, not only inside it.
    if text("/verification/status") == Some(crate::output::agent_receipt::VERIFICATION_NOT_RUN) {
        lines.push(
            "test run: none recorded. This receipt compares static evidence only; a failing test can still show `improved`."
                .to_string(),
        );
    }
    // The receipt producer owns which next step fits its status: only an
    // `advisory` receipt recommends including it in review, and any other
    // status states that it is not review evidence and how to recover. A
    // receipt without a status is not vouched for, so nothing is forwarded.
    if text("/status").is_some()
        && let Some(action) = text("/summary/next_action/recommended_action")
    {
        lines.push(format!("next: {action}"));
    }
    lines
}

fn resolve_agent_start_out_dir(root: &Path, out_dir: &Path) -> PathBuf {
    if out_dir.is_absolute() {
        out_dir.to_path_buf()
    } else {
        root.join(out_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{
        args, outcome_after_json, outcome_before_json, unique_command_test_dir,
        unique_repo_relative_test_dir,
    };
    use super::*;
    use crate::app::agent_brief::AgentBriefLine;
    use crate::cli::agent::AgentBriefWorkingSet;
    use crate::cli::commands_agent_support::{
        agent_brief_lines_from_diff, agent_brief_owners_for_lines, normalize_agent_brief_path,
    };

    /// Issue #3967: a relative `agent receipt --out` anchors at the resolved
    /// `--root` instead of the paste-site working directory, so the
    /// product-rendered command writes under the selected root from any
    /// CWD. An absolute `--out` passes through untouched.
    #[test]
    fn agent_receipt_out_path_anchors_relative_out_at_root() -> Result<(), String> {
        let cwd = std::env::current_dir()
            .map_err(|err| format!("read test working directory failed: {err}"))?;
        let absolute_root = cwd.join("receipt-anchor-root");
        let anchored = resolve_agent_receipt_out_path(
            &absolute_root,
            Path::new("target/ripr/reports/agent-receipt.json"),
        )?;
        let expected = absolute_root.join("target/ripr/reports/agent-receipt.json");
        if anchored != expected {
            return Err(format!(
                "relative --out must anchor at --root: {}",
                anchored.display()
            ));
        }
        let absolute_out = cwd.join("elsewhere").join("receipt.json");
        let passthrough = resolve_agent_receipt_out_path(&absolute_root, &absolute_out)?;
        if passthrough != absolute_out {
            return Err(format!(
                "absolute --out must pass through: {}",
                passthrough.display()
            ));
        }
        let relative_root = Path::new("some-checkout");
        let anchored_relative =
            resolve_agent_receipt_out_path(relative_root, Path::new("out.json"))?;
        let expected_relative = cwd.join(relative_root).join("out.json");
        if anchored_relative != expected_relative {
            return Err(format!(
                "relative --out under a relative --root must resolve through the process working directory: {}",
                anchored_relative.display()
            ));
        }
        Ok(())
    }

    #[test]
    fn agent_rejects_unknown_subcommands() {
        assert_eq!(
            agent(&args(&["unknown"])),
            Err(CommandError::Failure(
                "unknown agent subcommand \"unknown\"; expected `start`, `brief`, `packet`, `verify`, `verify-execute`, `receipt`, `status`, `review-summary`, or `repair`"
                    .to_string()
            ))
        );
    }

    #[test]
    fn agent_verify_execute_refusal_maps_to_decision_exit_code() -> Result<(), String> {
        let dir = unique_command_test_dir("agent-verify-execute-refusal");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        // A missing packet is a typed refusal (`verification_rejected_policy`),
        // printed as the stdout JSON document: the command ran successfully
        // and declined, so it maps to the decision exit code 3.
        let result = agent(&args(&[
            "verify-execute",
            "--root",
            &dir.display().to_string(),
            "--packet",
            &dir.join("missing-packet.json").display().to_string(),
            "--result-json",
            &dir.join("result.json").display().to_string(),
            "--json",
        ]));
        let Err(error) = result else {
            return Err("expected a typed refusal, got Ok".to_string());
        };
        assert!(
            matches!(
                &error,
                CommandError::Decision(message)
                    if message.contains("verification_rejected_policy")
            ),
            "typed refusal must carry the Decision variant: {error:?}"
        );
        assert_eq!(error.exit_code(), 3);
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn agent_repair_failure_before_attempt_selection_stays_a_failure() {
        // A missing root refuses before the after phase selects its attempt,
        // so it is an ordinary Failure (exit code 2), not a Decision: only
        // errors after attempt selection are recorded refusals.
        assert!(matches!(
            agent(&args(&[
                "repair",
                "--root",
                "target/ripr/missing-agent-repair-root",
                "--seam-id",
                "seam-a",
                "--phase",
                "after",
            ])),
            Err(CommandError::Failure(message)) if message.contains("is not a directory")
        ));
    }

    #[test]
    fn agent_start_rejects_missing_root_before_analysis() {
        assert_eq!(
            agent(&args(&[
                "start",
                "--root",
                "target/ripr/missing-agent-start-root",
                "--seam-id",
                "f3c9e4d21a0b7c88",
            ])),
            Err(CommandError::Failure(
                "agent start root target/ripr/missing-agent-start-root is not a directory"
                    .to_string()
            ))
        );
    }

    #[test]
    fn agent_status_rejects_missing_root_before_reading_artifacts() {
        assert_eq!(
            agent(&args(&[
                "status",
                "--root",
                "target/ripr/missing-agent-status-root",
                "--json",
            ])),
            Err(CommandError::Failure(
                "agent status root target/ripr/missing-agent-status-root is not a directory"
                    .to_string()
            ))
        );
    }

    #[test]
    fn agent_review_summary_rejects_missing_root_before_reading_artifacts() {
        assert_eq!(
            agent(&args(&[
                "review-summary",
                "--root",
                "target/ripr/missing-agent-review-summary-root",
                "--json",
            ])),
            Err(CommandError::Failure(
                "agent review-summary root target/ripr/missing-agent-review-summary-root is not a directory"
                    .to_string()
            ))
        );
    }

    #[test]
    fn agent_packet_rejects_missing_root_before_analysis() {
        assert_eq!(
            agent(&args(&[
                "packet",
                "--root",
                "target/ripr/missing-agent-packet-root",
                "--seam-id",
                "f3c9e4d21a0b7c88",
                "--json",
            ])),
            Err(CommandError::Failure(
                "agent packet root target/ripr/missing-agent-packet-root is not a directory"
                    .to_string()
            ))
        );
    }

    #[test]
    fn agent_verify_reports_read_failures() -> Result<(), String> {
        let dir = unique_command_test_dir("agent-verify-read");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        let before = dir.join("before.json");
        std::fs::write(&before, outcome_before_json())
            .map_err(|err| format!("write before snapshot: {err}"))?;

        let missing_before = agent(&args(&[
            "verify",
            "--root",
            &dir.display().to_string(),
            "--before",
            &dir.join("missing-before.json").display().to_string(),
            "--after",
            &dir.join("missing-after.json").display().to_string(),
            "--json",
        ]));
        assert!(matches!(
            missing_before,
            Err(CommandError::Failure(message))
                if message.contains("canonicalize agent verify --before")
        ));

        let missing_after = agent(&args(&[
            "verify",
            "--root",
            &dir.display().to_string(),
            "--before",
            before.to_string_lossy().as_ref(),
            "--after",
            &dir.join("missing-after.json").display().to_string(),
            "--json",
        ]));
        assert!(matches!(
            missing_after,
            Err(CommandError::Failure(message))
                if message.contains("canonicalize agent verify --after")
        ));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn agent_verify_rejects_snapshots_outside_root() -> Result<(), String> {
        let root = unique_command_test_dir("agent-verify-root");
        let outside = unique_command_test_dir("agent-verify-outside");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root dir: {err}"))?;
        std::fs::create_dir_all(&outside).map_err(|err| format!("create outside dir: {err}"))?;
        let before = outside.join("before.json");
        let after = root.join("after.json");
        std::fs::write(&before, outcome_before_json())
            .map_err(|err| format!("write before snapshot: {err}"))?;
        std::fs::write(&after, outcome_after_json())
            .map_err(|err| format!("write after snapshot: {err}"))?;

        let result = agent(&args(&[
            "verify",
            "--root",
            &root.display().to_string(),
            "--before",
            before.to_string_lossy().as_ref(),
            "--after",
            after.to_string_lossy().as_ref(),
            "--json",
        ]));

        assert!(matches!(
            result,
            Err(CommandError::Failure(message)) if message.contains("must stay under root")
        ));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
        Ok(())
    }

    #[test]
    fn agent_receipt_reports_read_failures() -> Result<(), String> {
        let dir = unique_command_test_dir("agent-receipt-read");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;

        let missing = agent(&args(&[
            "receipt",
            "--root",
            &dir.display().to_string(),
            "--verify-json",
            &dir.join("missing-agent-verify.json").display().to_string(),
            "--seam-id",
            "seam-a",
            "--json",
        ]));
        assert!(matches!(
            missing,
            Err(CommandError::Failure(message))
                if message.contains("canonicalize agent receipt --verify-json")
        ));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn agent_receipt_rejects_verify_json_outside_root() -> Result<(), String> {
        let root = unique_command_test_dir("agent-receipt-root");
        let outside = unique_command_test_dir("agent-receipt-outside");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root dir: {err}"))?;
        std::fs::create_dir_all(&outside).map_err(|err| format!("create outside dir: {err}"))?;
        let verify = outside.join("agent-verify.json");
        std::fs::write(&verify, "{}").map_err(|err| format!("write verify JSON: {err}"))?;

        let result = agent(&args(&[
            "receipt",
            "--root",
            &root.display().to_string(),
            "--verify-json",
            &verify.display().to_string(),
            "--seam-id",
            "seam-a",
            "--json",
        ]));

        assert!(matches!(
            result,
            Err(CommandError::Failure(message)) if message.contains("must stay under root")
        ));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
        Ok(())
    }

    #[test]
    fn agent_brief_rejects_missing_root_before_analysis() {
        assert_eq!(
            agent(&args(&[
                "brief",
                "--root",
                "target/ripr/missing-agent-brief-root",
                "--diff",
                "change.diff",
                "--json",
            ])),
            Err(CommandError::Failure(
                "agent brief root target/ripr/missing-agent-brief-root is not a directory"
                    .to_string()
            ))
        );
    }

    #[test]
    fn agent_brief_diff_lines_are_normalized_to_requested_root() {
        let diff = "diff --git a/crates/ripr/examples/sample/src/lib.rs b/crates/ripr/examples/sample/src/lib.rs\n--- a/crates/ripr/examples/sample/src/lib.rs\n+++ b/crates/ripr/examples/sample/src/lib.rs\n@@ -8,1 +8,1 @@\n-old\n+new\n";
        let lines = agent_brief_lines_from_diff(Path::new("crates/ripr/examples/sample"), diff);

        assert_eq!(
            lines,
            vec![AgentBriefLine::new(PathBuf::from("src/lib.rs"), 8)]
        );
    }

    #[test]
    fn agent_brief_owner_lines_are_resolved_from_changed_lines() -> Result<(), String> {
        let root = unique_command_test_dir("agent-brief-owner-lines");
        std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn discounted_total(amount: i32) -> i32 {\n    let discount = 10;\n    amount - discount\n}\n",
        )
        .map_err(|err| format!("write src/lib.rs: {err}"))?;
        let lines = vec![AgentBriefLine::new(PathBuf::from("src/lib.rs"), 3)];

        let owners = agent_brief_owners_for_lines(&root, &lines);

        assert_eq!(owners.len(), 1);
        assert_eq!(owners[0].line, 3);
        assert!(owners[0].owner.ends_with("discounted_total"));
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_brief_owner_lines_are_best_effort_for_missing_files() -> Result<(), String> {
        let root = unique_command_test_dir("agent-brief-owner-missing");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        let lines = vec![AgentBriefLine::new(PathBuf::from("src/missing.rs"), 3)];

        let owners = agent_brief_owners_for_lines(&root, &lines);

        assert!(owners.is_empty());
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_brief_normalizes_absolute_diff_paths_against_relative_root() -> Result<(), String> {
        let root = unique_repo_relative_test_dir("agent-brief-normalize");
        let src = root.join("src");
        std::fs::create_dir_all(&src).map_err(|err| format!("create src dir: {err}"))?;
        let absolute_file = std::env::current_dir()
            .map_err(|err| format!("read current dir: {err}"))?
            .join(&root)
            .join("src/lib.rs");

        assert_eq!(
            normalize_agent_brief_path(&root, &absolute_file),
            PathBuf::from("src/lib.rs")
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_brief_files_reject_parent_dir_escape() -> Result<(), String> {
        let root = unique_command_test_dir("agent-brief-files-escape");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;

        let result = resolve_agent_brief_working_set(
            &root,
            &AgentBriefWorkingSet::Files(vec![PathBuf::from("../../secret")]),
        );

        let Err(message) = result else {
            return Err("expected a confinement error, got Ok".to_string());
        };
        assert!(
            message.contains("must stay under root"),
            "unexpected error: {message}"
        );
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_brief_files_reject_absolute_path_outside_root() -> Result<(), String> {
        let root = unique_command_test_dir("agent-brief-files-abs");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        let outside = unique_command_test_dir("agent-brief-files-outside");

        let result = resolve_agent_brief_working_set(
            &root,
            &AgentBriefWorkingSet::Files(vec![outside.join("secret.rs")]),
        );

        let Err(message) = result else {
            return Err("expected a confinement error, got Ok".to_string());
        };
        assert!(
            message.contains("must stay under root"),
            "unexpected error: {message}"
        );
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_brief_files_accept_relative_and_absolute_under_root() -> Result<(), String> {
        let root = unique_repo_relative_test_dir("agent-brief-files-ok");
        let src = root.join("src");
        std::fs::create_dir_all(&src).map_err(|err| format!("create src dir: {err}"))?;
        let absolute_under_root = std::env::current_dir()
            .map_err(|err| format!("read current dir: {err}"))?
            .join(&root)
            .join("src/lib.rs");

        let resolved = resolve_agent_brief_working_set(
            &root,
            &AgentBriefWorkingSet::Files(vec![PathBuf::from("src/lib.rs"), absolute_under_root]),
        );

        let Ok(resolved) = resolved else {
            return Err(format!("expected confinement to accept, got {resolved:?}"));
        };
        // Output contract: both spellings resolve to the same confined
        // repo-relative path — not merely any non-empty path (#2100 review).
        assert_eq!(
            resolved.files,
            vec![PathBuf::from("src/lib.rs"), PathBuf::from("src/lib.rs")]
        );
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_brief_diff_path_must_stay_under_root() -> Result<(), String> {
        let root = unique_command_test_dir("agent-brief-root");
        let outside = unique_command_test_dir("agent-brief-outside");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        std::fs::create_dir_all(&outside).map_err(|err| format!("create outside: {err}"))?;
        let outside_diff = outside.join("change.diff");
        std::fs::write(&outside_diff, "diff --git a/src/lib.rs b/src/lib.rs\n")
            .map_err(|err| format!("write outside diff: {err}"))?;

        let result = resolve_agent_brief_working_set(
            &root,
            &AgentBriefWorkingSet::Diff(outside_diff.clone()),
        );
        let err = match result {
            Ok(_) => return Err("outside diff path should be rejected".to_string()),
            Err(err) => err,
        };

        assert!(
            err.contains("must stay under root"),
            "unexpected error: {err}"
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        std::fs::remove_dir_all(&outside).map_err(|err| format!("remove outside: {err}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod repair_summary_tests {
    use super::{repair_after_summary_lines, repair_receipt_summary_lines};

    #[test]
    fn repair_after_summary_names_a_receipt_it_could_not_read() {
        let missing = std::env::temp_dir()
            .join(format!("ripr-missing-receipt-{}", std::process::id()))
            .join("agent-receipt.json");
        let lines = repair_after_summary_lines(&missing);
        assert_eq!(lines.len(), 1, "expected one read-failure line: {lines:?}");
        let expected_prefix = format!(
            "could not read {} to summarize the result: ",
            missing.display()
        );
        assert!(
            lines
                .first()
                .is_some_and(|line| line.starts_with(&expected_prefix)),
            "read failure must name the receipt path: {lines:?}"
        );
    }

    #[test]
    fn repair_summary_names_movement_and_next_action_from_the_receipt() {
        let receipt = r#"{
            "status": "advisory",
            "seam": {"seam_id": "67fc764ba37d77bd"},
            "provenance": {"before_class": "weakly_gripped", "after_class": "strongly_gripped", "movement": "improved"},
            "summary": {"next_action": {"summary": "Static grip improved.", "recommended_action": "Run the focused test and keep it only if it passes; ripr did not run it. Then include this receipt in review."}}
        }"#;
        assert_eq!(
            repair_receipt_summary_lines(receipt),
            vec![
                "result for seam `67fc764ba37d77bd`: weakly_gripped -> strongly_gripped (improved). Static grip improved.".to_string(),
                "next: Run the focused test and keep it only if it passes; ripr did not run it. Then include this receipt in review.".to_string(),
            ]
        );
    }

    #[test]
    fn repair_summary_reports_unchanged_movement_verbatim() {
        let receipt = r#"{
            "seam": {"seam_id": "s"},
            "provenance": {"before_class": "weakly_gripped", "after_class": "weakly_gripped", "movement": "unchanged"}
        }"#;
        assert_eq!(
            repair_receipt_summary_lines(receipt),
            vec!["result for seam `s`: weakly_gripped -> weakly_gripped (unchanged).".to_string()]
        );
    }

    #[test]
    fn repair_summary_forwards_the_receipt_owned_next_step_for_a_non_advisory_receipt() {
        let movement = r#""seam": {"seam_id": "s"},
            "provenance": {"before_class": "weakly_gripped", "after_class": "strongly_gripped", "movement": "improved"}"#;
        let result =
            "result for seam `s`: weakly_gripped -> strongly_gripped (improved). Static grip improved."
                .to_string();
        let step = "This receipt is not review evidence because its status is `invalid` (Analysis outcome artifact base does not match its typed identity); do not include it in review.";
        let invalid = format!(
            r#"{{"status": "invalid", {movement}, "summary": {{"next_action": {{"summary": "Static grip improved.", "recommended_action": "{step}"}}}}}}"#
        );
        assert_eq!(
            repair_receipt_summary_lines(&invalid),
            vec![result.clone(), format!("next: {step}")]
        );
        // Without a status nothing vouches for the receipt, so its
        // recommendation is not forwarded.
        let unstated = format!(
            r#"{{{movement}, "summary": {{"next_action": {{"summary": "Static grip improved.", "recommended_action": "Run the focused test and keep it only if it passes; ripr did not run it. Then include this receipt in review."}}}}}}"#
        );
        assert_eq!(repair_receipt_summary_lines(&unstated), vec![result]);
    }

    #[test]
    fn repair_summary_is_empty_when_the_receipt_lacks_movement() {
        assert!(repair_receipt_summary_lines(r#"{"seam": {"seam_id": "s"}}"#).is_empty());
        assert!(repair_receipt_summary_lines("not json").is_empty());
    }
}

#[cfg(test)]
mod before_phase_stdout_tests {
    use super::before_phase_stdout;

    const PACKET: &str = r#"{
  "schema_version": "0.1",
  "packets": [
    {
      "seam_id": "0d196886bad1b124",
      "owner": "src/lib.rs::discounted_total",
      "file": "src/lib.rs",
      "line": 11,
      "changed_expression": "amount >= DISCOUNT_THRESHOLD",
      "recommended_test": {
        "name": "discounted_total_boundary_discriminator",
        "file": "tests/pricing.rs"
      },
      "missing_discriminators": [
        { "value": "DISCOUNT_THRESHOLD (equality boundary)" }
      ],
      "suggested_assertions": ["assert_eq!(discounted_total(10_000), 9_000)"]
    }
  ]
}"#;

    /// F15-7: a pipe or file gets the packet JSON byte for byte, so agents
    /// and scripts that parse stdout keep their contract.
    #[test]
    fn piped_stdout_is_the_packet_json_unchanged() {
        assert_eq!(
            before_phase_stdout(PACKET, "target/ripr/workflow/agent-packet.json", false),
            PACKET
        );
    }

    /// F15-7: a terminal gets a short summary naming the seam, the one test
    /// file to edit, and where the full packet is, instead of the JSON.
    #[test]
    fn terminal_stdout_is_a_short_summary_of_the_same_packet() {
        let summary = before_phase_stdout(PACKET, "target/ripr/workflow/agent-packet.json", true);
        assert!(!summary.contains('{'), "{summary}");
        assert!(summary.lines().count() <= 8, "{summary}");
        for expected in [
            "Repair prepared for seam 0d196886bad1b124 at src/lib.rs:11 in src/lib.rs::discounted_total.",
            "  changed behavior: amount >= DISCOUNT_THRESHOLD",
            "  missing discriminator: DISCOUNT_THRESHOLD (equality boundary)",
            "  edit one test file: tests/pricing.rs (suggested test `discounted_total_boundary_discriminator`); leave production code unchanged",
            "  assertion shape: assert_eq!(discounted_total(10_000), 9_000)",
            "  full repair packet (JSON): target/ripr/workflow/agent-packet.json; stdout carries it when piped",
        ] {
            assert!(
                summary.lines().any(|line| line == expected),
                "missing `{expected}` in:\n{summary}"
            );
        }
    }

    /// A packet the summary cannot read (no test file) falls back to the JSON
    /// rather than hiding it behind an empty summary.
    #[test]
    fn unreadable_packet_falls_back_to_the_json_on_a_terminal() {
        let packet = r#"{"packets":[{"seam_id":"x"}]}"#;
        assert_eq!(before_phase_stdout(packet, "p", true), packet);
    }
}

#[cfg(test)]
mod start_output_tests {
    use super::{AgentStartWritten, agent_start_prose_lines, render_agent_start_json};
    use std::path::PathBuf;

    fn written(next_command: Option<String>) -> AgentStartWritten {
        AgentStartWritten {
            paths: vec![
                PathBuf::from("target/ripr/workflow/workflow.json"),
                PathBuf::from("target/ripr/workflow/commands.md"),
                PathBuf::from("target/ripr/workflow/agent-brief.json"),
            ],
            next_command,
        }
    }

    #[test]
    fn agent_start_json_carries_workflow_paths_and_next_command() -> Result<(), String> {
        let rendered = render_agent_start_json(&written(Some(
            "ripr check --root . --format json > target/ripr/workflow/before.repo-exposure.json"
                .to_string(),
        )))?;
        let value: serde_json::Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("agent start JSON must parse: {err}"))?;
        assert_eq!(value["schema_version"], "0.1");
        assert_eq!(value["kind"], "agent_start");
        assert_eq!(
            value["workflow"]["workflow_manifest"],
            "target/ripr/workflow/workflow.json"
        );
        assert_eq!(
            value["workflow"]["commands_markdown"],
            "target/ripr/workflow/commands.md"
        );
        assert_eq!(
            value["workflow"]["agent_brief"],
            "target/ripr/workflow/agent-brief.json"
        );
        assert_eq!(
            value["next_command"],
            "ripr check --root . --format json > target/ripr/workflow/before.repo-exposure.json"
        );
        Ok(())
    }

    #[test]
    fn agent_start_json_next_command_is_null_when_every_input_is_present() -> Result<(), String> {
        let rendered = render_agent_start_json(&written(None))?;
        let value: serde_json::Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("agent start JSON must parse: {err}"))?;
        assert_eq!(value["next_command"], serde_json::Value::Null);
        assert!(value["workflow"]["agent_brief"].is_string());
        Ok(())
    }

    #[test]
    fn agent_start_prose_default_names_every_written_path() {
        let lines = agent_start_prose_lines(&written(Some("ripr pilot --root .".to_string())));
        assert_eq!(
            lines,
            vec![
                "Wrote target/ripr/workflow/workflow.json".to_string(),
                "Wrote target/ripr/workflow/commands.md".to_string(),
                "Wrote target/ripr/workflow/agent-brief.json".to_string(),
                "Next: ripr pilot --root .".to_string(),
            ]
        );
        let without_next = agent_start_prose_lines(&written(None));
        assert_eq!(without_next.len(), 3);
        assert!(!without_next.iter().any(|line| line.starts_with("Next:")));
    }
}
