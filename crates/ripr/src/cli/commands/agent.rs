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
use std::path::{Path, PathBuf};
use std::{
    fs::File,
    io::{BufWriter, Write},
};

use super::agent_dispatch;
use super::agent_gap_packet::render_agent_packet_from_gap_ledger;
use super::write_text_file;

pub(in crate::cli) fn agent(args: &[String]) -> Result<(), String> {
    let command = parse_agent_args(args)?;
    if let Some(result) = agent_dispatch::run_agent_help_command(&command) {
        return result;
    }

    match command {
        AgentCommand::Start(options) => run_agent_start(options),
        AgentCommand::Brief(options) => run_agent_brief(options),
        AgentCommand::Packet(options) => run_agent_packet(options),
        AgentCommand::Verify(options) => run_agent_verify(options),
        AgentCommand::VerifyExecute(options) => run_agent_verify_execute(options),
        AgentCommand::Receipt(options) => run_agent_receipt(options),
        AgentCommand::Status(options) => run_agent_status(options),
        AgentCommand::ReviewSummary(options) => run_agent_review_summary(options),
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
            .unwrap_or_else(|| Err("agent help command was not dispatched".to_string())),
    }
}

fn run_agent_start(options: AgentStartOptions) -> Result<(), String> {
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
            "agent start seam_id {} was not found or is hidden by config",
            options.seam_id
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

    println!("Wrote {}", workflow_path.display());
    println!("Wrote {}", commands_path.display());
    println!("Wrote {}", agent_brief_path.display());
    if let Some(next) = manifest.missing_inputs.first() {
        println!("Next: {}", next.command);
    }
    Ok(())
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
        .ok_or_else(|| format!("agent packet seam_id {seam_id} was not found"))?;

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

fn run_agent_verify_execute(options: AgentVerifyExecuteOptions) -> Result<(), String> {
    ensure_command_root(&options.root, "agent verify-execute")?;
    let outcome = app::verification_execution::execute_verify_packet(
        &options.root,
        &options.packet,
        &options.result_json,
        options.authorize,
        options.cancel_after_ms,
    );
    // The typed disposition is the contract, so it reaches stdout on every
    // terminal state — including refusals. The exit status only distinguishes
    // "RIPR committed a bounded observation" from "it could not".
    print!("{}", outcome.rendered);
    if outcome.failed {
        return Err(outcome.disposition.to_string());
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
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent)
                    .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
            }
            std::fs::write(&path, rendered).map_err(|err| {
                format!(
                    "write {} failed: {err}",
                    output::outcome::display_path(&path)
                )
            })
        }
        None => {
            print!("{rendered}");
            Ok(())
        }
    }
}

fn run_agent_status(options: AgentStatusOptions) -> Result<(), String> {
    ensure_command_root(&options.root, "agent status")?;

    let workflow_dir = match &options.out_dir {
        Some(dir) => resolve_agent_start_out_dir(&options.root, dir),
        None => options.root.join("target/ripr/workflow"),
    };
    let report = app::agent_status::build_agent_status_report(&options.root, &workflow_dir);
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
fn run_agent_repair(options: AgentRepairOptions) -> Result<(), String> {
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

            // Compose existing commands: start (creates workflow + brief) + packet.
            run_agent_start(AgentStartOptions {
                root: root.clone(),
                seam_id: seam_id.clone(),
                out_dir: std::path::PathBuf::from("target/ripr/workflow"),
            })?;

            let before = root.join("target/ripr/workflow/before.repo-exposure.json");
            write_agent_repo_exposure_snapshot(&root, &before)?;

            let packet = render_agent_packet(&AgentPacketOptions {
                root: root.clone(),
                seam_id: Some(seam_id),
                gap_ledger: None,
                gap_id: None,
                json: true,
            })?;
            let packet_path = root.join("target/ripr/workflow/agent-packet.json");
            write_text_file(&packet_path, &packet)?;
            print!("{packet}");

            eprintln!("ripr: before phase complete. Next:");
            eprintln!("  1. Edit the source code to add or strengthen the discriminator.");
            eprintln!(
                "  2. Run the exact --attempt command printed after the attempt manifest is published."
            );
            Ok(())
        }
        AgentRepairPhase::After => {
            ensure_command_root(&root, "agent repair --phase after")?;
            let attempt = crate::app::repair_attempt::resolve_awaiting_repair_attempt(
                &root,
                attempt_id.as_deref(),
                seam_id.as_deref(),
            )?;
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
            let rendered_verify = render_agent_verify(&verify_options)?;
            write_text_file(&verify_json, &rendered_verify)?;
            print!("{rendered_verify}");

            // The retained binding's manifest bytes are confirmed again
            // immediately before the durable finish: the apply verification
            // ran before several expensive operations, and a manifest
            // replaced inside that window must refuse instead of silently
            // advancing the attempt against replaced trust data.
            if let Some(binding) = &retained_binding {
                crate::app::python_repair_binding::confirm_manifest_unchanged(binding)?;
            }

            // Finish only after all command-owned after artifacts exist. This
            // makes the durable delta the exact delta the receipt binds, while
            // the receipt itself remains outside the measured edit window.
            let cage_after = crate::app::repair_attempt::finish_repair_attempt(
                &root,
                &attempt.attempt_id,
                &packet_path,
            )?;
            eprintln!(
                "ripr: edit-cage verdict for attempt `{}`: {:?}",
                cage_after.attempt_id.as_str(),
                cage_after.verdict.status
            );

            // The receipt can refuse (for example an escape verdict is not
            // receipt-ready). The refusal must not swallow the typed apply
            // evidence, so the outcome is carried to the end and the apply
            // record is published either way.
            let receipt_result = run_agent_receipt_for_attempt(
                AgentReceiptOptions {
                    root: root.clone(),
                    verify_json: verify_json.clone(),
                    seam_id: attempt.seam_id,
                    test_changed: None,
                    commands_run: Vec::new(),
                    json: true,
                    out: Some(root.join("target/ripr/reports/agent-receipt.json")),
                },
                Some(cage_after.attempt_id.as_str()),
                Some(&packet_path),
            );

            run_agent_status(AgentStatusOptions {
                root: root.clone(),
                json: true,
                out_dir: Some(std::path::PathBuf::from("target/ripr/workflow")),
            })?;

            // The apply record is published last: the receipt re-evaluates the
            // edit cage over the exact delta finish measured, so no artifact
            // write may land between finish and the receipt binding.
            // The retained binding's manifest bytes are confirmed once more
            // immediately before the record write: the earlier confirmation
            // ran before the durable finish, so a manifest replaced inside
            // that finalize window must refuse here instead of publishing an
            // apply record against replaced trust data. The refusal restores
            // the attempt to awaiting_edit, so the identical retry
            // re-verifies everything.
            let mut apply_record_result = Ok(());
            if let (Some(binding), Some(verified)) = (&retained_binding, &verified_binding) {
                let record_outcome =
                    crate::app::python_repair_binding::confirm_manifest_unchanged(binding)
                        .and_then(|()| {
                            crate::app::python_repair_binding::write_apply_record(
                                &root,
                                &attempt.attempt_id,
                                &binding.artifact_sha256,
                                verified,
                                edit_authorization.authority.as_deref().unwrap_or_default(),
                                &cage_after,
                            )
                        });
                match record_outcome {
                    Ok(apply_record_path) => {
                        eprintln!(
                            "ripr: python repair-trust apply record: {}",
                            apply_record_path.display()
                        );
                    }
                    Err(error) => {
                        // Finish already advanced the durable state, so a
                        // failed record publication must restore the attempt
                        // to awaiting_edit: the identical retry is otherwise
                        // rejected and the record could never be recreated.
                        match crate::app::repair_attempt::restore_repair_attempt_to_awaiting_edit(
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
            receipt_result?;
            apply_record_result?;

            eprintln!("ripr: after phase complete. Review the receipt and status output.");
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

    #[test]
    fn agent_rejects_unknown_subcommands() {
        assert_eq!(
            agent(&args(&["unknown"])),
            Err(
                "unknown agent subcommand \"unknown\"; expected `start`, `brief`, `packet`, `verify`, `verify-execute`, `receipt`, `status`, `review-summary`, or `repair`"
                    .to_string()
            )
        );
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
            Err(
                "agent start root target/ripr/missing-agent-start-root is not a directory"
                    .to_string()
            )
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
            Err(
                "agent status root target/ripr/missing-agent-status-root is not a directory"
                    .to_string()
            )
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
            Err(
                "agent review-summary root target/ripr/missing-agent-review-summary-root is not a directory"
                    .to_string()
            )
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
            Err(
                "agent packet root target/ripr/missing-agent-packet-root is not a directory"
                    .to_string()
            )
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
        assert!(
            matches!(missing_before, Err(message) if message.contains("canonicalize agent verify --before"))
        );

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
        assert!(
            matches!(missing_after, Err(message) if message.contains("canonicalize agent verify --after"))
        );
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

        assert!(matches!(result, Err(message) if message.contains("must stay under root")));
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
        assert!(
            matches!(missing, Err(message) if message.contains("canonicalize agent receipt --verify-json"))
        );
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

        assert!(matches!(result, Err(message) if message.contains("must stay under root")));
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
            Err(
                "agent brief root target/ripr/missing-agent-brief-root is not a directory"
                    .to_string()
            )
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
