use crate::analysis;
use crate::app::{self, CheckInput, Mode, OutputFormat};
use crate::cli::help;
use crate::cli::parse::{
    base_with_diff_conflict_error, disclose_attached_terminal_stdin_read, expect_value, parse_mode,
};
use crate::cli::suggest::{unknown_argument, unknown_value};
#[cfg(test)]
use crate::config::CONFIG_FILE_NAME;
use crate::config::{CheckInputExplicit, RiprConfig, apply_to_check_input, load_for_root};
use crate::output;
use std::path::{Path, PathBuf};

use crate::cli::commands_options::*;
use crate::cli::commands_timestamps::generated_at_unix_ms;

#[path = "commands/agent.rs"]
mod agent;
#[path = "commands/agent_card.rs"]
pub(crate) mod agent_card;
#[path = "commands/agent_dispatch.rs"]
mod agent_dispatch;
#[path = "commands/agent_gap_packet.rs"]
mod agent_gap_packet;
#[path = "commands/cache.rs"]
mod cache_command;
#[path = "commands/config.rs"]
mod config_command;
#[path = "commands/context.rs"]
mod context;
#[path = "commands/feedback.rs"]
mod feedback_command;
#[path = "commands/policy.rs"]
mod policy_commands;
#[path = "commands/receipt.rs"]
mod receipt_command;
#[path = "commands/swarm/mod.rs"]
mod swarm_command;

pub(super) use agent::{agent, before_phase_stdout, run_before_repair_with_identity};
pub(super) use context::context;
// Flag-documenting help bodies live beside their parsers so `cli::help`
// suggestions mine the same text `--help` prints.
pub(super) use feedback_command::{FEEDBACK_EXPORT_HELP, FEEDBACK_RECORD_HELP};
#[cfg(test)]
use policy_commands::{
    parse_policy_history_options, parse_policy_operations_options,
    parse_policy_preview_promotion_options, parse_policy_promotion_options,
    parse_policy_readiness_options, parse_policy_suppression_health_options,
    parse_policy_waiver_aging_options,
};
use policy_commands::{
    policy_history, policy_operations, policy_preview_promotion, policy_promotion,
    policy_readiness, policy_suppression_health, policy_waiver_aging,
};
pub(super) use receipt_command::{RECEIPT_CHECK_HELP, RECEIPT_WRITE_HELP};
// Cache help bodies live beside the cache parser but are also the flag source
// for `ripr cache status|clear` suggestions, so `cli::help` needs a path to
// them. Removing this re-export must fail to compile.
pub(super) use cache_command::{CACHE_CLEAR_HELP, CACHE_STATUS_HELP};

pub(super) fn receipt(args: &[String]) -> Result<(), String> {
    receipt_command::run_receipt(args)
}

pub(super) fn feedback(args: &[String]) -> Result<(), String> {
    feedback_command::run_feedback(args)
}

pub(super) fn swarm(args: &[String]) -> Result<(), String> {
    swarm_command::run(args)
}

pub(super) fn cache(args: &[String]) -> Result<(), String> {
    cache_command::run(args)
}

pub(super) use config_command::config;

fn write_text_file(path: &Path, rendered: &str) -> Result<(), String> {
    output::file_write::write(path, rendered.as_bytes()).map_err(|err| {
        format!(
            "write output {} failed: {err}",
            output::outcome::display_path(path)
        )
    })
}

fn append_jsonl_record(path: &Path, record: &str) -> Result<(), String> {
    output::file_write::append_line(path, record).map_err(|err| {
        format!(
            "append jsonl {} failed: {err}",
            output::outcome::display_path(path)
        )
    })
}

fn maybe_append_jsonl(path: Option<&Path>, record: &str) -> Result<(), String> {
    let Some(path) = path else {
        return Ok(());
    };
    append_jsonl_record(path, record)?;
    println!("Appended {}", path.display());
    Ok(())
}

#[path = "commands/baseline.rs"]
mod baseline;
pub(super) use baseline::baseline;

#[path = "commands/check.rs"]
mod check;
pub(super) use check::check;

#[path = "commands/review_comments.rs"]
mod review_comments;
pub(super) use review_comments::review_comments;

#[path = "commands/doctor.rs"]
mod doctor;
pub(super) use doctor::doctor;

#[path = "commands/gate.rs"]
mod gate;
pub(super) use gate::gate;

#[path = "commands/init.rs"]
mod init;
pub(super) use init::init;
#[cfg(test)]
use init::parse_init_options;

// The generated `ripr init --ci github` workflow template lives beside the
// init command; its tests here pin rendered placeholders against it.
#[path = "commands/init_workflow.rs"]
mod init_workflow;
#[cfg(test)]
use init_workflow::generated_github_actions_workflow;

#[path = "commands/pilot.rs"]
mod pilot;
pub(super) use pilot::pilot;

pub(super) fn outcome(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_outcome_help();
        return Ok(());
    }

    let options = parse_outcome_options(args)?;
    let before_json = crate::bounded_input::read_to_string(&options.before).map_err(|err| {
        format!(
            "read {} failed: {err}",
            output::outcome::display_path(&options.before)
        )
    })?;
    let after_json = crate::bounded_input::read_to_string(&options.after).map_err(|err| {
        format!(
            "read {} failed: {err}",
            output::outcome::display_path(&options.after)
        )
    })?;
    let report = output::outcome::targeted_test_outcome_report_from_json(
        &before_json,
        &after_json,
        output::outcome::display_path(&options.before),
        output::outcome::display_path(&options.after),
    )?;
    // Disclose on stderr (machine JSON output on stdout is unchanged) what
    // can actually be said about the two snapshots' provenance, so a user who
    // did not read the help knows what the movement report assumes. See
    // #1942; the line used to assert unconditionally that the artifacts carry
    // no head SHA, which is false for any snapshot written through the
    // artifact-identity path.
    eprintln!(
        "{}",
        output::outcome::head_provenance_disclosure(&before_json, &after_json)
    );
    let rendered = match options.format {
        OutcomeFormat::Markdown => output::outcome::render_targeted_test_outcome_md(&report),
        OutcomeFormat::Json => output::outcome::render_targeted_test_outcome_json(&report)?,
    };

    match options.out {
        Some(path) => write_text_file(&path, &rendered),
        None => {
            print!("{rendered}");
            Ok(())
        }
    }
}

pub(super) fn evidence_health(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_evidence_health_help();
        return Ok(());
    }

    let options = parse_evidence_health_options(args)?;
    if !options.root.is_dir() {
        return Err(format!(
            "evidence-health root {} is not a directory",
            options.root.display()
        ));
    }

    let config = load_for_root(&options.root)?;
    let (classified, _) =
        analysis::inventory_classified_seams_at_with_config(&options.root, &config)?;
    let calibration = match &options.mutation_calibration {
        Some(path) => {
            let contents = crate::bounded_input::read_to_string(path).map_err(|err| {
                format!(
                    "read evidence-health calibration context {} failed: {err}",
                    output::outcome::display_path(path)
                )
            })?;
            output::evidence_health::EvidenceHealthCalibration::from_json(
                output::outcome::display_path(path),
                &contents,
            )?
        }
        None => output::evidence_health::EvidenceHealthCalibration::not_provided(),
    };
    let report = output::evidence_health::build_evidence_health_report(
        &classified,
        output::outcome::display_path(&options.root),
        calibration,
    );
    let rendered_json = output::evidence_health::render_evidence_health_json(&report)?;
    let rendered_md = output::evidence_health::render_evidence_health_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

pub(super) fn zero(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_zero_help();
        return Ok(());
    }
    // The sole subcommand is implied when the command runs bare:
    // `ripr <cmd>` behaves like `ripr <cmd> "status"` (#2013).
    let (subcommand, rest) = match args.split_first() {
        Some((subcommand, rest)) => (subcommand.as_str(), rest),
        None => ("status", &[][..]),
    };
    if subcommand != "status" {
        return Err(format!(
            "unknown zero subcommand {subcommand:?}; expected `status`"
        ));
    }
    ripr_zero_status(rest)
}

pub(super) fn policy(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_policy_help();
        return Ok(());
    }
    let Some((subcommand, rest)) = args.split_first() else {
        return Err(
            "policy requires subcommand `readiness`, `operations`, `history`, `promote`, `preview-promote`, `waiver-aging`, or `suppression-health`"
                .to_string(),
        );
    };
    match subcommand.as_str() {
        "readiness" => policy_readiness(rest),
        "operations" => policy_operations(rest),
        "history" => policy_history(rest),
        "promote" => policy_promotion(rest),
        "preview-promote" => policy_preview_promotion(rest),
        "waiver-aging" => policy_waiver_aging(rest),
        "suppression-health" => policy_suppression_health(rest),
        _ => Err(format!(
            "unknown policy subcommand {subcommand:?}; expected `readiness`, `operations`, `history`, `promote`, `preview-promote`, `waiver-aging`, or `suppression-health`"
        )),
    }
}

pub(super) fn pr_ledger(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_pr_ledger_help();
        return Ok(());
    }
    // The sole subcommand is implied when the command runs bare:
    // `ripr <cmd>` behaves like `ripr <cmd> "record"` (#2013).
    let (subcommand, rest) = match args.split_first() {
        Some((subcommand, rest)) => (subcommand.as_str(), rest),
        None => ("record", &[][..]),
    };
    if subcommand != "record" {
        return Err(format!(
            "unknown pr-ledger subcommand {subcommand:?}; expected `record`"
        ));
    }
    pr_evidence_ledger_record(rest)
}

pub(super) fn pr_comments(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_pr_comments_help();
        return Ok(());
    }
    // The sole subcommand is implied when the command runs bare:
    // `ripr <cmd>` behaves like `ripr <cmd> "plan"` (#2013).
    let (subcommand, rest) = match args.split_first() {
        Some((subcommand, rest)) => (subcommand.as_str(), rest),
        None => ("plan", &[][..]),
    };
    if subcommand != "plan" {
        return Err(format!(
            "unknown pr-comments subcommand {subcommand:?}; expected `plan`"
        ));
    }
    pr_comments_plan(rest)
}

pub(super) fn pr_review(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_pr_review_help();
        return Ok(());
    }
    // The sole subcommand is implied when the command runs bare:
    // `ripr <cmd>` behaves like `ripr <cmd> "front-panel"` (#2013).
    let (subcommand, rest) = match args.split_first() {
        Some((subcommand, rest)) => (subcommand.as_str(), rest),
        None => ("front-panel", &[][..]),
    };
    if subcommand != "front-panel" {
        return Err(format!(
            "unknown pr-review subcommand {subcommand:?}; expected `front-panel`"
        ));
    }
    pr_review_front_panel(rest)
}

pub(super) fn coverage_grip(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_coverage_grip_help();
        return Ok(());
    }
    // The sole subcommand is implied when the command runs bare:
    // `ripr <cmd>` behaves like `ripr <cmd> "frontier"` (#2013).
    let (subcommand, rest) = match args.split_first() {
        Some((subcommand, rest)) => (subcommand.as_str(), rest),
        None => ("frontier", &[][..]),
    };
    if subcommand != "frontier" {
        return Err(format!(
            "unknown coverage-grip subcommand {subcommand:?}; expected `frontier`"
        ));
    }
    coverage_grip_frontier(rest)
}

pub(super) fn assistant_loop(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_assistant_loop_help();
        return Ok(());
    }
    let Some((subcommand, rest)) = args.split_first() else {
        return Err("assistant-loop requires subcommand `proof` or `health`".to_string());
    };
    match subcommand.as_str() {
        "proof" => assistant_loop_proof(rest),
        "health" => assistant_loop_health(rest),
        _ => Err(format!(
            "unknown assistant-loop subcommand {subcommand:?}; expected `proof` or `health`"
        )),
    }
}

pub(super) fn first_pr(args: &[String]) -> Result<(), String> {
    output::first_pr::first_pr(args)
}

pub(super) fn first_action(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_first_action_help();
        return Ok(());
    }

    let options = parse_first_action_options(args)?;
    let pr_guidance_path = options
        .pr_guidance
        .as_ref()
        .map(|path| output::first_useful_action::display_path(path));
    let assistant_proof_path = options
        .assistant_proof
        .as_ref()
        .map(|path| output::first_useful_action::display_path(path));
    let gap_ledger_path = options
        .gap_ledger
        .as_ref()
        .map(|path| output::first_useful_action::display_path(path));
    let ledger_path = options
        .ledger
        .as_ref()
        .map(|path| output::first_useful_action::display_path(path));
    let baseline_delta_path = options
        .baseline_delta
        .as_ref()
        .map(|path| output::first_useful_action::display_path(path));
    let receipt_path = options
        .receipt
        .as_ref()
        .map(|path| output::first_useful_action::display_path(path));
    let gate_decision_path = options
        .gate_decision
        .as_ref()
        .map(|path| output::first_useful_action::display_path(path));
    let coverage_frontier_path = options
        .coverage_frontier
        .as_ref()
        .map(|path| output::first_useful_action::display_path(path));
    let editor_context_path = options
        .editor_context
        .as_ref()
        .map(|path| output::first_useful_action::display_path(path));
    let input = output::first_useful_action::FirstUsefulActionInput {
        root: options.root,
        generated_at: first_action_generated_at()?,
        pr_guidance_path,
        assistant_proof_path,
        gap_ledger_path,
        ledger_path,
        baseline_delta_path,
        receipt_path,
        gate_decision_path,
        coverage_frontier_path,
        editor_context_path,
        pr_guidance_json: options
            .pr_guidance
            .as_ref()
            .map(|path| read_optional_text_for_report("PR guidance", path)),
        assistant_proof_json: options
            .assistant_proof
            .as_ref()
            .map(|path| read_optional_text_for_report("assistant proof", path)),
        gap_ledger_json: options
            .gap_ledger
            .as_ref()
            .map(|path| read_optional_text_for_report("gap decision ledger", path)),
        ledger_json: options
            .ledger
            .as_ref()
            .map(|path| read_optional_text_for_report("PR evidence ledger", path)),
        baseline_delta_json: options
            .baseline_delta
            .as_ref()
            .map(|path| read_optional_text_for_report("baseline debt delta", path)),
        receipt_json: options
            .receipt
            .as_ref()
            .map(|path| read_optional_text_for_report("receipt", path)),
        gate_decision_json: options
            .gate_decision
            .as_ref()
            .map(|path| read_optional_text_for_report("gate decision", path)),
        coverage_frontier_json: options
            .coverage_frontier
            .as_ref()
            .map(|path| read_optional_text_for_report("coverage/grip frontier", path)),
        editor_context_json: options
            .editor_context
            .as_ref()
            .map(|path| read_optional_text_for_report("editor context", path)),
    };
    let report = output::first_useful_action::build_first_useful_action_report(input);
    let rendered_json = output::first_useful_action::render_first_useful_action_json(&report)?;
    let rendered_md = output::first_useful_action::render_first_useful_action_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

pub(super) fn reports(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_reports_help();
        return Ok(());
    }
    let Some((subcommand, rest)) = args.split_first() else {
        return Err(
            "reports requires subcommand `index`, `gap-ledger`, `ts-limitations`, or `ts-false-actionable`"
                .to_string(),
        );
    };
    match subcommand.as_str() {
        "index" => report_packet_index(rest),
        "gap-ledger" => gap_decision_ledger(rest),
        "ts-limitations" => typescript_limitations(rest),
        "ts-false-actionable" => typescript_false_actionable(rest),
        _ => Err(format!(
            "unknown reports subcommand {subcommand:?}; expected `index`, `gap-ledger`, `ts-limitations`, or `ts-false-actionable`"
        )),
    }
}

fn report_packet_index(args: &[String]) -> Result<(), String> {
    let options = parse_report_packet_index_options(args)?;
    let input = output::report_packet_index::ReportPacketIndexInput {
        root: options.root,
        generated_at: report_packet_index_generated_at()?,
        reports_dir: options.reports_dir,
        review_dir: options.review_dir,
        receipts_dir: options.receipts_dir,
        workflow_dir: options.workflow_dir,
        agent_dir: options.agent_dir,
        pilot_dir: options.pilot_dir,
        ci_dir: options.ci_dir,
    };
    let report = output::report_packet_index::build_report_packet_index_report(input);
    let rendered_json = output::report_packet_index::render_report_packet_index_json(&report)?;
    let rendered_md = output::report_packet_index::render_report_packet_index_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

fn gap_decision_ledger(args: &[String]) -> Result<(), String> {
    let options = parse_gap_decision_ledger_options(args)?;
    let records_path = ledger_source_path(options.source.path())?;
    let input = output::gap_decision_ledger::GapDecisionLedgerInput {
        root: options.root,
        generated_at: gap_decision_ledger_generated_at()?,
        source_kind: options.source.kind(),
        records_path,
        records_json: read_optional_text_for_report(options.source.label(), options.source.path()),
    };
    let selected_root = PathBuf::from(&input.root);
    let mut report = output::gap_decision_ledger::build_gap_decision_ledger_report(input);
    output::gap_decision_ledger::stamp_gap_decision_ledger_source_subject(
        &mut report,
        &selected_root,
    )?;
    let rendered_json = output::gap_decision_ledger::render_gap_decision_ledger_json(&report)?;
    let rendered_md = output::gap_decision_ledger::render_gap_decision_ledger_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

fn ledger_source_path(path: &Path) -> Result<String, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("resolve gap-ledger source path failed: {error}"))?
            .join(path)
    };
    Ok(output::baseline_delta::display_path(&absolute))
}

fn typescript_limitations(args: &[String]) -> Result<(), String> {
    let options = parse_typescript_limitations_options(args)?;
    // An unreadable check output means the command could not complete:
    // surface the read failure (exit 2 per docs/EXIT_CODES.md) instead of
    // writing a `blocked` report and exiting 0 as if the run succeeded.
    let check_output_json = read_optional_text_for_report("check output", &options.check_output)?;
    let input = output::typescript_limitations::TypeScriptLimitationLeaderboardInput {
        root: options.root,
        generated_at: typescript_limitations_generated_at()?,
        check_output_path: output::baseline_delta::display_path(&options.check_output),
        check_output_json: Ok(check_output_json),
    };
    let report =
        output::typescript_limitations::build_typescript_limitation_leaderboard_report(input);
    let rendered_json =
        output::typescript_limitations::render_typescript_limitation_leaderboard_json(&report)?;
    let rendered_md =
        output::typescript_limitations::render_typescript_limitation_leaderboard_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

fn typescript_false_actionable(args: &[String]) -> Result<(), String> {
    let options = parse_typescript_false_actionable_options(args)?;
    // An unreadable corpus means the command could not complete: surface the
    // read failure (exit 2 per docs/EXIT_CODES.md) instead of writing a
    // `blocked` report and exiting 0 as if the run succeeded.
    let corpus_json =
        read_optional_text_for_report("TypeScript false-actionable audit corpus", &options.corpus)?;
    let input = output::typescript_false_actionable::TypeScriptFalseActionableAuditInput {
        root: options.root,
        generated_at: typescript_false_actionable_generated_at()?,
        corpus_path: output::baseline_delta::display_path(&options.corpus),
        corpus_json: Ok(corpus_json),
    };
    let report =
        output::typescript_false_actionable::build_typescript_false_actionable_audit_report(input);
    let rendered_json =
        output::typescript_false_actionable::render_typescript_false_actionable_audit_json(
            &report,
        )?;
    let rendered_md =
        output::typescript_false_actionable::render_typescript_false_actionable_audit_markdown(
            &report,
        );
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

fn ripr_zero_status(args: &[String]) -> Result<(), String> {
    let options = parse_ripr_zero_status_options(args)?;
    let baseline_path = options
        .baseline
        .as_ref()
        .map(|path| output::ripr_zero_status::display_path(path));
    let gate_path = options
        .gate
        .as_ref()
        .map(|path| output::ripr_zero_status::display_path(path));
    let gap_ledger_path = options
        .gap_ledger
        .as_ref()
        .map(|path| output::ripr_zero_status::display_path(path));
    let pr_guidance_path = options
        .pr_guidance
        .as_ref()
        .map(|path| output::ripr_zero_status::display_path(path));
    let recommendation_calibration_path = options
        .recommendation_calibration
        .as_ref()
        .map(|path| output::ripr_zero_status::display_path(path));
    let input = output::ripr_zero_status::RiprZeroStatusInput {
        root: ".".to_string(),
        generated_at: baseline_created_at()?,
        baseline_path,
        delta_path: output::ripr_zero_status::display_path(&options.delta),
        gap_ledger_path,
        gate_path,
        pr_guidance_path,
        recommendation_calibration_path,
        baseline_json: options
            .baseline
            .as_ref()
            .map(|path| read_optional_text_for_report("baseline", path)),
        delta_json: read_optional_text_for_report("baseline debt delta", &options.delta),
        gap_ledger_json: options
            .gap_ledger
            .as_ref()
            .map(|path| read_optional_text_for_report("gap decision ledger", path)),
        gate_json: options
            .gate
            .as_ref()
            .map(|path| read_optional_text_for_report("gate decision", path)),
        pr_guidance_json: options
            .pr_guidance
            .as_ref()
            .map(|path| read_optional_text_for_report("PR guidance", path)),
        recommendation_calibration_json: options
            .recommendation_calibration
            .as_ref()
            .map(|path| read_optional_text_for_report("recommendation calibration", path)),
    };
    let report = output::ripr_zero_status::build_ripr_zero_status_report(input);
    let rendered_json = output::ripr_zero_status::render_ripr_zero_status_json(&report)?;
    let rendered_md = output::ripr_zero_status::render_ripr_zero_status_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

fn pr_evidence_ledger_record(args: &[String]) -> Result<(), String> {
    let options = parse_pr_evidence_ledger_options(args)?;
    let gate_path = options
        .gate
        .as_ref()
        .map(|path| output::pr_evidence_ledger::display_path(path));
    let baseline_delta_path = options
        .baseline_delta
        .as_ref()
        .map(|path| output::pr_evidence_ledger::display_path(path));
    let zero_status_path = options
        .zero_status
        .as_ref()
        .map(|path| output::pr_evidence_ledger::display_path(path));
    let pr_guidance_path = options
        .pr_guidance
        .as_ref()
        .map(|path| output::pr_evidence_ledger::display_path(path));
    let gap_ledger_path = options
        .gap_ledger
        .as_ref()
        .map(|path| output::pr_evidence_ledger::display_path(path));
    let recommendation_calibration_path = options
        .recommendation_calibration
        .as_ref()
        .map(|path| output::pr_evidence_ledger::display_path(path));
    let agent_receipt_path = options
        .agent_receipt
        .as_ref()
        .map(|path| output::pr_evidence_ledger::display_path(path));
    let coverage_path = options
        .coverage
        .as_ref()
        .map(|path| output::pr_evidence_ledger::display_path(path));
    let history_path = options
        .history
        .as_ref()
        .map(|path| output::pr_evidence_ledger::display_path(path));
    let input = output::pr_evidence_ledger::PrEvidenceLedgerInput {
        root: ".".to_string(),
        generated_at: baseline_created_at()?,
        pr_number: options.pr_number,
        base: options.base,
        head: options.head,
        labels: options.labels,
        gate_path,
        baseline_delta_path,
        zero_status_path,
        pr_guidance_path,
        gap_ledger_path,
        recommendation_calibration_path,
        agent_receipt_path,
        coverage_path,
        history_path,
        gate_json: options
            .gate
            .as_ref()
            .map(|path| read_optional_text_for_report("gate decision", path)),
        baseline_delta_json: options
            .baseline_delta
            .as_ref()
            .map(|path| read_optional_text_for_report("baseline debt delta", path)),
        zero_status_json: options
            .zero_status
            .as_ref()
            .map(|path| read_optional_text_for_report("RIPR Zero status", path)),
        pr_guidance_json: options
            .pr_guidance
            .as_ref()
            .map(|path| read_optional_text_for_report("PR guidance", path)),
        gap_ledger_json: options
            .gap_ledger
            .as_ref()
            .map(|path| read_optional_text_for_report("gap decision ledger", path)),
        recommendation_calibration_json: options
            .recommendation_calibration
            .as_ref()
            .map(|path| read_optional_text_for_report("recommendation calibration", path)),
        agent_receipt_json: options
            .agent_receipt
            .as_ref()
            .map(|path| read_optional_text_for_report("agent receipt", path)),
        coverage_json: options
            .coverage
            .as_ref()
            .map(|path| read_optional_text_for_report("coverage", path)),
        history_json: options
            .history
            .as_ref()
            .map(|path| read_optional_text_for_report("history", path)),
    };
    let report = output::pr_evidence_ledger::build_pr_evidence_ledger_report(input);
    let rendered_json = output::pr_evidence_ledger::render_pr_evidence_ledger_json(&report)?;
    let rendered_md = output::pr_evidence_ledger::render_pr_evidence_ledger_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    maybe_append_jsonl(
        options.out_jsonl.as_deref(),
        &output::pr_evidence_ledger::render_pr_evidence_ledger_jsonl_record(&report)?,
    )?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

fn pr_comments_plan(args: &[String]) -> Result<(), String> {
    let options = parse_pr_comments_plan_options(args)?;
    let pr_guidance_path = options
        .pr_guidance
        .as_ref()
        .map(|path| output::pr_inline_comment_publish_plan::display_path(path));
    let existing_comments_path = options
        .existing_comments
        .as_ref()
        .map(|path| output::pr_inline_comment_publish_plan::display_path(path));
    let input = output::pr_inline_comment_publish_plan::CommentPublishPlanInput {
        root: options.root,
        generated_at: comment_publish_plan_generated_at()?,
        mode: options.mode,
        max_inline_comments: options.max_inline_comments,
        pr_guidance_path,
        pr_guidance_json: options
            .pr_guidance
            .as_ref()
            .map(|path| read_optional_text_for_report("PR guidance", path)),
        existing_comments_path,
        existing_comments_json: options
            .existing_comments
            .as_ref()
            .map(|path| read_optional_text_for_report("existing comments", path)),
        permission: output::pr_inline_comment_publish_plan::CommentPermissionContext {
            pull_request: options.pull_request,
            event_name: options.event_name,
            head_repo: options.head_repo,
            base_repo: options.base_repo,
            token_available: options.token_available,
            write_permission: options.write_permission,
        },
    };
    let report = output::pr_inline_comment_publish_plan::build_comment_publish_plan_report(input);
    let rendered_json =
        output::pr_inline_comment_publish_plan::render_comment_publish_plan_json(&report)?;
    let rendered_md =
        output::pr_inline_comment_publish_plan::render_comment_publish_plan_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

fn pr_review_front_panel(args: &[String]) -> Result<(), String> {
    let options = parse_pr_review_front_panel_options(args)?;
    let pr_guidance_path = options
        .pr_guidance
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let first_action_path = options
        .first_action
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let assistant_proof_path = options
        .assistant_proof
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let assistant_health_path = options
        .assistant_health
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let ledger_path = options
        .ledger
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let baseline_delta_path = options
        .baseline_delta
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let zero_status_path = options
        .zero_status
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let gate_decision_path = options
        .gate_decision
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let recommendation_calibration_path = options
        .recommendation_calibration
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let mutation_calibration_path = options
        .mutation_calibration
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let coverage_frontier_path = options
        .coverage_frontier
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let receipt_path = options
        .receipt
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let input = output::pr_review_front_panel::PrReviewFrontPanelInput {
        root: options.root,
        generated_at: pr_review_front_panel_generated_at()?,
        out_md_path: output::pr_review_front_panel::display_path(&options.out_md),
        pr_guidance_path,
        first_action_path,
        assistant_proof_path,
        assistant_health_path,
        ledger_path,
        baseline_delta_path,
        zero_status_path,
        gate_decision_path,
        recommendation_calibration_path,
        mutation_calibration_path,
        coverage_frontier_path,
        receipt_path,
        pr_guidance_json: options
            .pr_guidance
            .as_ref()
            .map(|path| read_optional_text_for_report("PR guidance", path)),
        first_action_json: options
            .first_action
            .as_ref()
            .map(|path| read_optional_text_for_report("first useful action", path)),
        assistant_proof_json: options
            .assistant_proof
            .as_ref()
            .map(|path| read_optional_text_for_report("assistant proof", path)),
        assistant_health_json: options
            .assistant_health
            .as_ref()
            .map(|path| read_optional_text_for_report("assistant loop health", path)),
        ledger_json: options
            .ledger
            .as_ref()
            .map(|path| read_optional_text_for_report("PR evidence ledger", path)),
        baseline_delta_json: options
            .baseline_delta
            .as_ref()
            .map(|path| read_optional_text_for_report("baseline debt delta", path)),
        zero_status_json: options
            .zero_status
            .as_ref()
            .map(|path| read_optional_text_for_report("RIPR Zero status", path)),
        gate_decision_json: options
            .gate_decision
            .as_ref()
            .map(|path| read_optional_text_for_report("gate decision", path)),
        recommendation_calibration_json: options
            .recommendation_calibration
            .as_ref()
            .map(|path| read_optional_text_for_report("recommendation calibration", path)),
        mutation_calibration_json: options
            .mutation_calibration
            .as_ref()
            .map(|path| read_optional_text_for_report("mutation calibration", path)),
        coverage_frontier_json: options
            .coverage_frontier
            .as_ref()
            .map(|path| read_optional_text_for_report("coverage/grip frontier", path)),
        receipt_json: options
            .receipt
            .as_ref()
            .map(|path| read_optional_text_for_report("receipt", path)),
    };
    let report = output::pr_review_front_panel::build_pr_review_front_panel_report(input);
    let rendered_json = output::pr_review_front_panel::render_pr_review_front_panel_json(&report)?;
    let rendered_md = output::pr_review_front_panel::render_pr_review_front_panel_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

fn coverage_grip_frontier(args: &[String]) -> Result<(), String> {
    let options = parse_coverage_grip_frontier_options(args)?;
    let coverage_path = options
        .coverage
        .as_ref()
        .map(|path| output::coverage_grip_frontier::display_path(path));
    let ledger_path = options
        .ledger
        .as_ref()
        .map(|path| output::coverage_grip_frontier::display_path(path));
    let baseline_delta_path = options
        .baseline_delta
        .as_ref()
        .map(|path| output::coverage_grip_frontier::display_path(path));
    let zero_status_path = options
        .zero_status
        .as_ref()
        .map(|path| output::coverage_grip_frontier::display_path(path));
    let input = output::coverage_grip_frontier::CoverageGripFrontierInput {
        root: ".".to_string(),
        generated_at: baseline_created_at()?,
        coverage_path,
        ledger_path,
        baseline_delta_path,
        zero_status_path,
        coverage_json: options
            .coverage
            .as_ref()
            .map(|path| read_optional_text_for_report("coverage", path)),
        ledger_json: options
            .ledger
            .as_ref()
            .map(|path| read_optional_text_for_report("PR evidence ledger", path)),
        baseline_delta_json: options
            .baseline_delta
            .as_ref()
            .map(|path| read_optional_text_for_report("baseline debt delta", path)),
        zero_status_json: options
            .zero_status
            .as_ref()
            .map(|path| read_optional_text_for_report("RIPR Zero status", path)),
    };
    let report = output::coverage_grip_frontier::build_coverage_grip_frontier_report(input);
    let rendered_json =
        output::coverage_grip_frontier::render_coverage_grip_frontier_json(&report)?;
    let rendered_md =
        output::coverage_grip_frontier::render_coverage_grip_frontier_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

fn assistant_loop_proof(args: &[String]) -> Result<(), String> {
    let options = parse_assistant_loop_proof_options(args)?;
    let pr_guidance_path = options
        .pr_guidance
        .as_ref()
        .map(|path| output::test_oracle_assistant_proof::display_path(path));
    let agent_packet_path = options
        .agent_packet
        .as_ref()
        .map(|path| output::test_oracle_assistant_proof::display_path(path));
    let before_path = options
        .before
        .as_ref()
        .map(|path| output::test_oracle_assistant_proof::display_path(path));
    let after_path = options
        .after
        .as_ref()
        .map(|path| output::test_oracle_assistant_proof::display_path(path));
    let receipt_path = options
        .receipt
        .as_ref()
        .map(|path| output::test_oracle_assistant_proof::display_path(path));
    let ledger_path = options
        .ledger
        .as_ref()
        .map(|path| output::test_oracle_assistant_proof::display_path(path));
    let coverage_frontier_path = options
        .coverage_frontier
        .as_ref()
        .map(|path| output::test_oracle_assistant_proof::display_path(path));
    let gate_decision_path = options
        .gate_decision
        .as_ref()
        .map(|path| output::test_oracle_assistant_proof::display_path(path));
    let input = output::test_oracle_assistant_proof::TestOracleAssistantProofInput {
        root: options.root,
        pr_guidance_path,
        agent_packet_path,
        before_path,
        after_path,
        receipt_path,
        ledger_path,
        coverage_frontier_path,
        gate_decision_path,
        pr_guidance_json: options
            .pr_guidance
            .as_ref()
            .map(|path| read_optional_text_for_report("PR guidance", path)),
        agent_packet_json: options
            .agent_packet
            .as_ref()
            .map(|path| read_optional_text_for_report("agent packet", path)),
        before_json: options
            .before
            .as_ref()
            .map(|path| read_optional_text_for_report("before evidence", path)),
        after_json: options
            .after
            .as_ref()
            .map(|path| read_optional_text_for_report("after evidence", path)),
        receipt_json: options
            .receipt
            .as_ref()
            .map(|path| read_optional_text_for_report("receipt", path)),
        ledger_json: options
            .ledger
            .as_ref()
            .map(|path| read_optional_text_for_report("PR evidence ledger", path)),
        coverage_frontier_json: options
            .coverage_frontier
            .as_ref()
            .map(|path| read_optional_text_for_report("coverage/grip frontier", path)),
        gate_decision_json: options
            .gate_decision
            .as_ref()
            .map(|path| read_optional_text_for_report("gate decision", path)),
    };
    let report =
        output::test_oracle_assistant_proof::build_test_oracle_assistant_proof_report(input);
    let rendered_json =
        output::test_oracle_assistant_proof::render_test_oracle_assistant_proof_json(&report)?;
    let rendered_md =
        output::test_oracle_assistant_proof::render_test_oracle_assistant_proof_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

fn assistant_loop_health(args: &[String]) -> Result<(), String> {
    let options = parse_assistant_loop_health_options(args)?;
    let proofs = options
        .proofs
        .iter()
        .map(|path| {
            let source_artifact = output::assistant_loop_health::display_path(path);
            let proof_json = read_optional_text_for_report("assistant proof", path);
            output::assistant_loop_health::AssistantLoopHealthProofInput {
                source_artifact,
                proof_json,
            }
        })
        .collect::<Vec<_>>();
    let report = output::assistant_loop_health::build_assistant_loop_health_report(
        output::assistant_loop_health::AssistantLoopHealthInput {
            root: options.root,
            generated_at: assistant_loop_health_generated_at()?,
            proofs,
        },
    );
    let rendered_json = output::assistant_loop_health::render_assistant_loop_health_json(&report)?;
    let rendered_md = output::assistant_loop_health::render_assistant_loop_health_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    println!(
        "Proofs: {}",
        output::assistant_loop_health::assistant_loop_health_proof_count(&report)
    );
    Ok(())
}

pub(super) fn calibrate(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_calibrate_help();
        return Ok(());
    }

    let Some((subcommand, rest)) = args.split_first() else {
        return Err("calibrate requires subcommand `cargo-mutants`".to_string());
    };
    if subcommand != "cargo-mutants" {
        return Err(format!(
            "unknown calibrate subcommand {subcommand:?}; expected `cargo-mutants`"
        ));
    }

    let options = parse_calibrate_cargo_mutants_options(rest)?;
    let repo_exposure_json = crate::bounded_input::read_to_string(&options.repo_exposure_json)
        .map_err(|err| {
            format!(
                "read {} failed: {err}",
                output::outcome::display_path(&options.repo_exposure_json)
            )
        })?;
    let mutants_json = read_calibration_mutants_json(&options.mutants_json)?;
    let report = output::mutation_calibration::mutation_calibration_report_from_json(
        &repo_exposure_json,
        &mutants_json,
    )?;
    let rendered = match options.format {
        CalibrateFormat::Markdown => {
            output::mutation_calibration::render_mutation_calibration_md(&report)
        }
        CalibrateFormat::Json => {
            output::mutation_calibration::render_mutation_calibration_json(&report)?
        }
    };

    match options.out {
        Some(path) => write_text_file(&path, &rendered),
        None => {
            print!("{rendered}");
            Ok(())
        }
    }
}

fn parse_calibrate_cargo_mutants_options(args: &[String]) -> Result<CalibrateOptions, String> {
    let mut mutants_json: Option<PathBuf> = None;
    let mut repo_exposure_json: Option<PathBuf> = None;
    let mut format = CalibrateFormat::Markdown;
    let mut out: Option<PathBuf> = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--mutants-json" | "--cargo-mutants-json" | "--input" => {
                i += 1;
                mutants_json = Some(PathBuf::from(expect_value(args, i, "--mutants-json")?));
            }
            "--repo-exposure-json" | "--static-json" => {
                i += 1;
                repo_exposure_json = Some(PathBuf::from(expect_value(
                    args,
                    i,
                    "--repo-exposure-json",
                )?));
            }
            "--format" => {
                i += 1;
                format = parse_calibrate_format(expect_value(args, i, "--format")?)?;
            }
            "--out" => {
                i += 1;
                out = Some(PathBuf::from(expect_value(args, i, "--out")?));
            }
            other => {
                return Err(unknown_argument("calibrate cargo-mutants", other));
            }
        }
        i += 1;
    }

    let mutants_json = mutants_json
        .ok_or_else(|| "calibrate cargo-mutants requires --mutants-json <path>".to_string())?;
    let repo_exposure_json = repo_exposure_json.ok_or_else(|| {
        "calibrate cargo-mutants requires --repo-exposure-json <path>".to_string()
    })?;
    Ok(CalibrateOptions {
        mutants_json,
        repo_exposure_json,
        format,
        out,
    })
}

fn parse_calibrate_format(value: &str) -> Result<CalibrateFormat, String> {
    const ACCEPTED: &[&str] = &["md", "markdown", "text", "json"];
    match value {
        "md" | "markdown" | "text" => Ok(CalibrateFormat::Markdown),
        "json" => Ok(CalibrateFormat::Json),
        _ => Err(unknown_value("calibrate format", value, ACCEPTED)),
    }
}

fn read_calibration_mutants_json(path: &Path) -> Result<String, String> {
    if path.is_dir() {
        let outcomes_path = path.join("outcomes.json");
        let mutants_path = path.join("mutants.json");
        let outcomes_exists = outcomes_path.exists();
        let mutants_exists = mutants_path.exists();

        if outcomes_exists && mutants_exists {
            let outcomes = read_json_value(&outcomes_path)?;
            let mutants = read_json_value(&mutants_path)?;
            return serde_json::to_string(&serde_json::Value::Array(vec![outcomes, mutants]))
                .map_err(|err| format!("failed to combine cargo-mutants directory JSON: {err}"));
        }

        if outcomes_exists {
            return read_calibration_text(&outcomes_path);
        }
        if mutants_exists {
            return read_calibration_text(&mutants_path);
        }
        return Err(format!(
            "{} is a directory but contains neither outcomes.json nor mutants.json",
            output::outcome::display_path(path)
        ));
    }
    read_calibration_text(path)
}

fn read_json_value(path: &Path) -> Result<serde_json::Value, String> {
    let text = read_calibration_text(path)?;
    serde_json::from_str(&text).map_err(|err| {
        format!(
            "failed to parse JSON from {}: {err}",
            output::outcome::display_path(path)
        )
    })
}

fn read_calibration_text(path: &Path) -> Result<String, String> {
    crate::bounded_input::read_to_string(path)
        .map_err(|err| format!("read {} failed: {err}", output::outcome::display_path(path)))
}

fn parse_outcome_options(args: &[String]) -> Result<OutcomeOptions, String> {
    let mut before: Option<PathBuf> = None;
    let mut after: Option<PathBuf> = None;
    let mut format = OutcomeFormat::Markdown;
    let mut out: Option<PathBuf> = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--before" => {
                i += 1;
                before = Some(PathBuf::from(expect_value(args, i, "--before")?));
            }
            "--after" => {
                i += 1;
                after = Some(PathBuf::from(expect_value(args, i, "--after")?));
            }
            "--format" => {
                i += 1;
                format = parse_outcome_format(expect_value(args, i, "--format")?)?;
            }
            "--out" => {
                i += 1;
                out = Some(PathBuf::from(expect_value(args, i, "--out")?));
            }
            other => return Err(unknown_argument("outcome", other)),
        }
        i += 1;
    }

    let before = before.ok_or_else(|| "outcome requires --before <path>".to_string())?;
    let after = after.ok_or_else(|| "outcome requires --after <path>".to_string())?;
    Ok(OutcomeOptions {
        before,
        after,
        format,
        out,
    })
}

fn parse_evidence_health_options(args: &[String]) -> Result<EvidenceHealthOptions, String> {
    let mut root = PathBuf::from(".");
    let mut out = PathBuf::from("target/ripr/reports/evidence-health.json");
    let mut out_md = PathBuf::from("target/ripr/reports/evidence-health.md");
    let mut mutation_calibration: Option<PathBuf> = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--out" => {
                i += 1;
                out = PathBuf::from(expect_value(args, i, "--out")?);
            }
            "--out-md" => {
                i += 1;
                out_md = PathBuf::from(expect_value(args, i, "--out-md")?);
            }
            "--mutation-calibration" => {
                i += 1;
                mutation_calibration = Some(PathBuf::from(expect_value(
                    args,
                    i,
                    "--mutation-calibration",
                )?));
            }
            other => return Err(unknown_argument("evidence-health", other)),
        }
        i += 1;
    }

    Ok(EvidenceHealthOptions {
        root,
        out,
        out_md,
        mutation_calibration,
    })
}

fn parse_ripr_zero_status_options(args: &[String]) -> Result<RiprZeroStatusOptions, String> {
    let mut baseline = None;
    let mut delta = None;
    let mut gap_ledger = None;
    let mut gate = None;
    let mut pr_guidance = None;
    let mut recommendation_calibration = None;
    let mut out = PathBuf::from(output::ripr_zero_status::DEFAULT_RIPR_ZERO_STATUS_OUT);
    let mut out_md = PathBuf::from(output::ripr_zero_status::DEFAULT_RIPR_ZERO_STATUS_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--baseline" => {
                i += 1;
                baseline = Some(non_empty_path_arg(args, i, "--baseline", "zero status")?);
            }
            "--delta" => {
                i += 1;
                delta = Some(non_empty_path_arg(args, i, "--delta", "zero status")?);
            }
            "--gap-ledger" => {
                i += 1;
                gap_ledger = Some(non_empty_path_arg(args, i, "--gap-ledger", "zero status")?);
            }
            "--gate" => {
                i += 1;
                gate = Some(non_empty_path_arg(args, i, "--gate", "zero status")?);
            }
            "--pr-guidance" => {
                i += 1;
                pr_guidance = Some(non_empty_path_arg(args, i, "--pr-guidance", "zero status")?);
            }
            "--recommendation-calibration" => {
                i += 1;
                recommendation_calibration = Some(non_empty_path_arg(
                    args,
                    i,
                    "--recommendation-calibration",
                    "zero status",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "zero status")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "zero status")?;
            }
            other => return Err(unknown_argument("zero status", other)),
        }
        i += 1;
    }

    Ok(RiprZeroStatusOptions {
        baseline,
        delta: delta.ok_or_else(|| "zero status requires --delta <path>".to_string())?,
        gap_ledger,
        gate,
        pr_guidance,
        recommendation_calibration,
        out,
        out_md,
    })
}

fn parse_pr_evidence_ledger_options(args: &[String]) -> Result<PrEvidenceLedgerOptions, String> {
    let mut pr_number = None;
    let mut base = None;
    let mut head = None;
    let mut labels = Vec::new();
    let mut gate = None;
    let mut baseline_delta = None;
    let mut zero_status = None;
    let mut pr_guidance = None;
    let mut gap_ledger = None;
    let mut recommendation_calibration = None;
    let mut agent_receipt = None;
    let mut coverage = None;
    let mut history = None;
    let mut out = PathBuf::from(output::pr_evidence_ledger::DEFAULT_PR_EVIDENCE_LEDGER_OUT);
    let mut out_md = PathBuf::from(output::pr_evidence_ledger::DEFAULT_PR_EVIDENCE_LEDGER_MD_OUT);
    let mut out_jsonl = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--pr-number" => {
                i += 1;
                pr_number = Some(non_empty_string_arg(
                    args,
                    i,
                    "--pr-number",
                    "pr-ledger record",
                )?);
            }
            "--base" => {
                i += 1;
                base = Some(non_empty_string_arg(args, i, "--base", "pr-ledger record")?);
            }
            "--head" => {
                i += 1;
                head = Some(non_empty_string_arg(args, i, "--head", "pr-ledger record")?);
            }
            "--label" => {
                i += 1;
                labels.push(non_empty_string_arg(
                    args,
                    i,
                    "--label",
                    "pr-ledger record",
                )?);
            }
            "--gate" => {
                i += 1;
                gate = Some(non_empty_path_arg(args, i, "--gate", "pr-ledger record")?);
            }
            "--baseline-delta" => {
                i += 1;
                baseline_delta = Some(non_empty_path_arg(
                    args,
                    i,
                    "--baseline-delta",
                    "pr-ledger record",
                )?);
            }
            "--zero-status" => {
                i += 1;
                zero_status = Some(non_empty_path_arg(
                    args,
                    i,
                    "--zero-status",
                    "pr-ledger record",
                )?);
            }
            "--pr-guidance" => {
                i += 1;
                pr_guidance = Some(non_empty_path_arg(
                    args,
                    i,
                    "--pr-guidance",
                    "pr-ledger record",
                )?);
            }
            "--gap-ledger" => {
                i += 1;
                gap_ledger = Some(non_empty_path_arg(
                    args,
                    i,
                    "--gap-ledger",
                    "pr-ledger record",
                )?);
            }
            "--recommendation-calibration" => {
                i += 1;
                recommendation_calibration = Some(non_empty_path_arg(
                    args,
                    i,
                    "--recommendation-calibration",
                    "pr-ledger record",
                )?);
            }
            "--agent-receipt" => {
                i += 1;
                agent_receipt = Some(non_empty_path_arg(
                    args,
                    i,
                    "--agent-receipt",
                    "pr-ledger record",
                )?);
            }
            "--coverage" => {
                i += 1;
                coverage = Some(non_empty_path_arg(
                    args,
                    i,
                    "--coverage",
                    "pr-ledger record",
                )?);
            }
            "--history" => {
                i += 1;
                history = Some(non_empty_path_arg(
                    args,
                    i,
                    "--history",
                    "pr-ledger record",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "pr-ledger record")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "pr-ledger record")?;
            }
            "--out-jsonl" => {
                i += 1;
                out_jsonl = Some(non_empty_path_arg(
                    args,
                    i,
                    "--out-jsonl",
                    "pr-ledger record",
                )?);
            }
            other => return Err(unknown_argument("pr-ledger record", other)),
        }
        i += 1;
    }

    if gate.is_none()
        && baseline_delta.is_none()
        && zero_status.is_none()
        && pr_guidance.is_none()
        && gap_ledger.is_none()
    {
        return Err(
            "pr-ledger record requires at least one of --gate, --baseline-delta, --zero-status, --pr-guidance, or --gap-ledger"
                .to_string(),
        );
    }

    if let Some(jsonl) = out_jsonl.as_ref() {
        let mut forbidden = vec![&out, &out_md];
        for input in [
            gate.as_ref(),
            baseline_delta.as_ref(),
            zero_status.as_ref(),
            pr_guidance.as_ref(),
            gap_ledger.as_ref(),
            recommendation_calibration.as_ref(),
            agent_receipt.as_ref(),
            coverage.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            forbidden.push(input);
        }
        if forbidden
            .iter()
            .any(|path| output::path::same_output_leaf(jsonl, path))
        {
            return Err(
                "pr-ledger record --out-jsonl must not be the same path as --out, --out-md, or an evidence input"
                    .to_string(),
            );
        }
    }

    Ok(PrEvidenceLedgerOptions {
        pr_number: pr_number
            .ok_or_else(|| "pr-ledger record requires --pr-number <value>".to_string())?,
        base: base.ok_or_else(|| "pr-ledger record requires --base <revision>".to_string())?,
        head: head.ok_or_else(|| "pr-ledger record requires --head <revision>".to_string())?,
        labels,
        gate,
        baseline_delta,
        zero_status,
        pr_guidance,
        gap_ledger,
        recommendation_calibration,
        agent_receipt,
        coverage,
        history,
        out,
        out_md,
        out_jsonl,
    })
}

fn parse_pr_comments_plan_options(args: &[String]) -> Result<PrCommentsPlanOptions, String> {
    let mut root = ".".to_string();
    let mut pr_guidance = None;
    let mut existing_comments = None;
    let mut mode = output::pr_inline_comment_publish_plan::CommentMode::Off;
    let mut pull_request = None;
    let mut event_name = None;
    let mut head_repo = None;
    let mut base_repo = None;
    let mut token_available = false;
    let mut write_permission = true;
    let mut max_inline_comments =
        output::pr_inline_comment_publish_plan::DEFAULT_MAX_INLINE_COMMENTS;
    let mut out =
        PathBuf::from(output::pr_inline_comment_publish_plan::DEFAULT_COMMENT_PUBLISH_PLAN_OUT);
    let mut out_md =
        PathBuf::from(output::pr_inline_comment_publish_plan::DEFAULT_COMMENT_PUBLISH_PLAN_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "pr-comments plan")?;
            }
            "--pr-guidance" => {
                i += 1;
                pr_guidance = Some(non_empty_path_arg(
                    args,
                    i,
                    "--pr-guidance",
                    "pr-comments plan",
                )?);
            }
            "--existing-comments" => {
                i += 1;
                existing_comments = Some(non_empty_path_arg(
                    args,
                    i,
                    "--existing-comments",
                    "pr-comments plan",
                )?);
            }
            "--mode" => {
                i += 1;
                mode = output::pr_inline_comment_publish_plan::CommentMode::parse(expect_value(
                    args, i, "--mode",
                )?)?;
            }
            "--pull-request" => {
                i += 1;
                let value = non_empty_string_arg(args, i, "--pull-request", "pr-comments plan")?;
                pull_request = Some(value.parse::<u64>().map_err(|err| {
                    format!("pr-comments plan --pull-request must be a positive integer: {err}")
                })?);
            }
            "--event-name" => {
                i += 1;
                event_name = Some(non_empty_string_arg(
                    args,
                    i,
                    "--event-name",
                    "pr-comments plan",
                )?);
            }
            "--head-repo" => {
                i += 1;
                head_repo = Some(non_empty_string_arg(
                    args,
                    i,
                    "--head-repo",
                    "pr-comments plan",
                )?);
            }
            "--base-repo" => {
                i += 1;
                base_repo = Some(non_empty_string_arg(
                    args,
                    i,
                    "--base-repo",
                    "pr-comments plan",
                )?);
            }
            "--token-available" => token_available = true,
            "--no-token" => token_available = false,
            "--write-permission" => write_permission = true,
            "--no-write-permission" => write_permission = false,
            "--max-inline-comments" => {
                i += 1;
                let value =
                    non_empty_string_arg(args, i, "--max-inline-comments", "pr-comments plan")?;
                max_inline_comments = value.parse::<usize>().map_err(|err| {
                    format!("pr-comments plan --max-inline-comments must be a number: {err}")
                })?;
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "pr-comments plan")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "pr-comments plan")?;
            }
            other => return Err(unknown_argument("pr-comments plan", other)),
        }
        i += 1;
    }

    if max_inline_comments == 0 {
        return Err("pr-comments plan --max-inline-comments must be greater than zero".to_string());
    }

    Ok(PrCommentsPlanOptions {
        root,
        pr_guidance,
        existing_comments,
        mode,
        pull_request,
        event_name,
        head_repo,
        base_repo,
        token_available,
        write_permission,
        max_inline_comments,
        out,
        out_md,
    })
}

fn parse_pr_review_front_panel_options(
    args: &[String],
) -> Result<PrReviewFrontPanelOptions, String> {
    let mut root = ".".to_string();
    let mut pr_guidance = None;
    let mut first_action = None;
    let mut assistant_proof = None;
    let mut assistant_health = None;
    let mut ledger = None;
    let mut baseline_delta = None;
    let mut zero_status = None;
    let mut gate_decision = None;
    let mut recommendation_calibration = None;
    let mut mutation_calibration = None;
    let mut coverage_frontier = None;
    let mut receipt = None;
    let mut out = PathBuf::from(output::pr_review_front_panel::DEFAULT_PR_REVIEW_FRONT_PANEL_OUT);
    let mut out_md =
        PathBuf::from(output::pr_review_front_panel::DEFAULT_PR_REVIEW_FRONT_PANEL_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "pr-review front-panel")?;
            }
            "--pr-guidance" => {
                i += 1;
                pr_guidance = Some(non_empty_path_arg(
                    args,
                    i,
                    "--pr-guidance",
                    "pr-review front-panel",
                )?);
            }
            "--first-action" => {
                i += 1;
                first_action = Some(non_empty_path_arg(
                    args,
                    i,
                    "--first-action",
                    "pr-review front-panel",
                )?);
            }
            "--assistant-proof" => {
                i += 1;
                assistant_proof = Some(non_empty_path_arg(
                    args,
                    i,
                    "--assistant-proof",
                    "pr-review front-panel",
                )?);
            }
            "--assistant-health" => {
                i += 1;
                assistant_health = Some(non_empty_path_arg(
                    args,
                    i,
                    "--assistant-health",
                    "pr-review front-panel",
                )?);
            }
            "--ledger" => {
                i += 1;
                ledger = Some(non_empty_path_arg(
                    args,
                    i,
                    "--ledger",
                    "pr-review front-panel",
                )?);
            }
            "--baseline-delta" => {
                i += 1;
                baseline_delta = Some(non_empty_path_arg(
                    args,
                    i,
                    "--baseline-delta",
                    "pr-review front-panel",
                )?);
            }
            "--zero-status" => {
                i += 1;
                zero_status = Some(non_empty_path_arg(
                    args,
                    i,
                    "--zero-status",
                    "pr-review front-panel",
                )?);
            }
            "--gate-decision" => {
                i += 1;
                gate_decision = Some(non_empty_path_arg(
                    args,
                    i,
                    "--gate-decision",
                    "pr-review front-panel",
                )?);
            }
            "--recommendation-calibration" => {
                i += 1;
                recommendation_calibration = Some(non_empty_path_arg(
                    args,
                    i,
                    "--recommendation-calibration",
                    "pr-review front-panel",
                )?);
            }
            "--mutation-calibration" => {
                i += 1;
                mutation_calibration = Some(non_empty_path_arg(
                    args,
                    i,
                    "--mutation-calibration",
                    "pr-review front-panel",
                )?);
            }
            "--coverage-frontier" => {
                i += 1;
                coverage_frontier = Some(non_empty_path_arg(
                    args,
                    i,
                    "--coverage-frontier",
                    "pr-review front-panel",
                )?);
            }
            "--receipt" => {
                i += 1;
                receipt = Some(non_empty_path_arg(
                    args,
                    i,
                    "--receipt",
                    "pr-review front-panel",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "pr-review front-panel")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "pr-review front-panel")?;
            }
            other => return Err(unknown_argument("pr-review front-panel", other)),
        }
        i += 1;
    }

    if pr_guidance.is_none()
        && first_action.is_none()
        && assistant_proof.is_none()
        && assistant_health.is_none()
        && ledger.is_none()
        && baseline_delta.is_none()
        && zero_status.is_none()
        && gate_decision.is_none()
        && recommendation_calibration.is_none()
        && mutation_calibration.is_none()
        && coverage_frontier.is_none()
        && receipt.is_none()
    {
        return Err(
            "pr-review front-panel requires at least one explicit artifact input".to_string(),
        );
    }

    Ok(PrReviewFrontPanelOptions {
        root,
        pr_guidance,
        first_action,
        assistant_proof,
        assistant_health,
        ledger,
        baseline_delta,
        zero_status,
        gate_decision,
        recommendation_calibration,
        mutation_calibration,
        coverage_frontier,
        receipt,
        out,
        out_md,
    })
}

fn parse_report_packet_index_options(args: &[String]) -> Result<ReportPacketIndexOptions, String> {
    let mut root = ".".to_string();
    let mut reports_dir = PathBuf::from("target/ripr/reports");
    let mut review_dir = PathBuf::from("target/ripr/review");
    let mut receipts_dir = PathBuf::from("target/ripr/receipts");
    let mut workflow_dir = PathBuf::from("target/ripr/workflow");
    let mut agent_dir = PathBuf::from("target/ripr/agent");
    let mut pilot_dir = PathBuf::from("target/ripr/pilot");
    let mut ci_dir = PathBuf::from("target/ci");
    let mut out = PathBuf::from(output::report_packet_index::DEFAULT_REPORT_PACKET_INDEX_OUT);
    let mut out_md = PathBuf::from(output::report_packet_index::DEFAULT_REPORT_PACKET_INDEX_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "reports index")?;
            }
            "--reports-dir" => {
                i += 1;
                reports_dir = non_empty_path_arg(args, i, "--reports-dir", "reports index")?;
            }
            "--review-dir" => {
                i += 1;
                review_dir = non_empty_path_arg(args, i, "--review-dir", "reports index")?;
            }
            "--receipts-dir" => {
                i += 1;
                receipts_dir = non_empty_path_arg(args, i, "--receipts-dir", "reports index")?;
            }
            "--workflow-dir" => {
                i += 1;
                workflow_dir = non_empty_path_arg(args, i, "--workflow-dir", "reports index")?;
            }
            "--agent-dir" => {
                i += 1;
                agent_dir = non_empty_path_arg(args, i, "--agent-dir", "reports index")?;
            }
            "--pilot-dir" => {
                i += 1;
                pilot_dir = non_empty_path_arg(args, i, "--pilot-dir", "reports index")?;
            }
            "--ci-dir" => {
                i += 1;
                ci_dir = non_empty_path_arg(args, i, "--ci-dir", "reports index")?;
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "reports index")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "reports index")?;
            }
            other => return Err(unknown_argument("reports index", other)),
        }
        i += 1;
    }

    Ok(ReportPacketIndexOptions {
        root,
        reports_dir,
        review_dir,
        receipts_dir,
        workflow_dir,
        agent_dir,
        pilot_dir,
        ci_dir,
        out,
        out_md,
    })
}

fn parse_gap_decision_ledger_options(args: &[String]) -> Result<GapDecisionLedgerOptions, String> {
    let mut root = ".".to_string();
    let mut records = None;
    let mut repo_exposure = None;
    let mut check_output = None;
    let mut out = PathBuf::from(output::gap_decision_ledger::DEFAULT_GAP_DECISION_LEDGER_OUT);
    let mut out_md = PathBuf::from(output::gap_decision_ledger::DEFAULT_GAP_DECISION_LEDGER_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "reports gap-ledger")?;
            }
            "--records" => {
                i += 1;
                records = Some(non_empty_path_arg(
                    args,
                    i,
                    "--records",
                    "reports gap-ledger",
                )?);
            }
            "--repo-exposure" => {
                i += 1;
                repo_exposure = Some(non_empty_path_arg(
                    args,
                    i,
                    "--repo-exposure",
                    "reports gap-ledger",
                )?);
            }
            "--check-output" => {
                i += 1;
                check_output = Some(non_empty_path_arg(
                    args,
                    i,
                    "--check-output",
                    "reports gap-ledger",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "reports gap-ledger")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "reports gap-ledger")?;
            }
            other => return Err(unknown_argument("reports gap-ledger", other)),
        }
        i += 1;
    }

    let supplied_sources =
        records.is_some() as u8 + repo_exposure.is_some() as u8 + check_output.is_some() as u8;
    if supplied_sources == 0 {
        return Err(
            "reports gap-ledger requires --records PATH, --repo-exposure PATH, or --check-output PATH"
                .to_string(),
        );
    }
    if supplied_sources > 1 {
        return Err(
            "reports gap-ledger accepts only one of --records, --repo-exposure, or --check-output"
                .to_string(),
        );
    }
    let source = if let Some(records) = records {
        GapDecisionLedgerSource::Records(records)
    } else if let Some(repo_exposure) = repo_exposure {
        GapDecisionLedgerSource::RepoExposure(repo_exposure)
    } else if let Some(check_output) = check_output {
        GapDecisionLedgerSource::CheckOutput(check_output)
    } else {
        return Err(
            "reports gap-ledger requires --records PATH, --repo-exposure PATH, or --check-output PATH"
                .to_string(),
        );
    };

    Ok(GapDecisionLedgerOptions {
        root,
        source,
        out,
        out_md,
    })
}

fn parse_typescript_limitations_options(
    args: &[String],
) -> Result<TypeScriptLimitationsOptions, String> {
    let mut root = ".".to_string();
    let mut check_output = None;
    let mut out = PathBuf::from(output::typescript_limitations::DEFAULT_TYPESCRIPT_LIMITATIONS_OUT);
    let mut out_md =
        PathBuf::from(output::typescript_limitations::DEFAULT_TYPESCRIPT_LIMITATIONS_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "reports ts-limitations")?;
            }
            "--check-output" => {
                i += 1;
                check_output = Some(non_empty_path_arg(
                    args,
                    i,
                    "--check-output",
                    "reports ts-limitations",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "reports ts-limitations")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "reports ts-limitations")?;
            }
            other => return Err(unknown_argument("reports ts-limitations", other)),
        }
        i += 1;
    }

    let Some(check_output) = check_output else {
        return Err("reports ts-limitations requires --check-output PATH".to_string());
    };

    Ok(TypeScriptLimitationsOptions {
        root,
        check_output,
        out,
        out_md,
    })
}

fn parse_typescript_false_actionable_options(
    args: &[String],
) -> Result<TypeScriptFalseActionableOptions, String> {
    let mut root = ".".to_string();
    let mut corpus = None;
    let mut out =
        PathBuf::from(output::typescript_false_actionable::DEFAULT_TYPESCRIPT_FALSE_ACTIONABLE_OUT);
    let mut out_md = PathBuf::from(
        output::typescript_false_actionable::DEFAULT_TYPESCRIPT_FALSE_ACTIONABLE_MD_OUT,
    );

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "reports ts-false-actionable")?;
            }
            "--corpus" => {
                i += 1;
                corpus = Some(non_empty_path_arg(
                    args,
                    i,
                    "--corpus",
                    "reports ts-false-actionable",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "reports ts-false-actionable")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "reports ts-false-actionable")?;
            }
            other => {
                return Err(unknown_argument("reports ts-false-actionable", other));
            }
        }
        i += 1;
    }

    let Some(corpus) = corpus else {
        return Err("reports ts-false-actionable requires --corpus PATH".to_string());
    };

    Ok(TypeScriptFalseActionableOptions {
        root,
        corpus,
        out,
        out_md,
    })
}

fn parse_coverage_grip_frontier_options(
    args: &[String],
) -> Result<CoverageGripFrontierOptions, String> {
    let mut coverage = None;
    let mut ledger = None;
    let mut baseline_delta = None;
    let mut zero_status = None;
    let mut out = PathBuf::from(output::coverage_grip_frontier::DEFAULT_COVERAGE_GRIP_FRONTIER_OUT);
    let mut out_md =
        PathBuf::from(output::coverage_grip_frontier::DEFAULT_COVERAGE_GRIP_FRONTIER_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--coverage" => {
                i += 1;
                coverage = Some(non_empty_path_arg(
                    args,
                    i,
                    "--coverage",
                    "coverage-grip frontier",
                )?);
            }
            "--ledger" => {
                i += 1;
                ledger = Some(non_empty_path_arg(
                    args,
                    i,
                    "--ledger",
                    "coverage-grip frontier",
                )?);
            }
            "--baseline-delta" => {
                i += 1;
                baseline_delta = Some(non_empty_path_arg(
                    args,
                    i,
                    "--baseline-delta",
                    "coverage-grip frontier",
                )?);
            }
            "--zero-status" => {
                i += 1;
                zero_status = Some(non_empty_path_arg(
                    args,
                    i,
                    "--zero-status",
                    "coverage-grip frontier",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "coverage-grip frontier")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "coverage-grip frontier")?;
            }
            other => return Err(unknown_argument("coverage-grip frontier", other)),
        }
        i += 1;
    }

    if ledger.is_none() && baseline_delta.is_none() && zero_status.is_none() {
        return Err(
            "coverage-grip frontier requires at least one of --ledger, --baseline-delta, or --zero-status"
                .to_string(),
        );
    }

    Ok(CoverageGripFrontierOptions {
        coverage,
        ledger,
        baseline_delta,
        zero_status,
        out,
        out_md,
    })
}

fn parse_assistant_loop_proof_options(
    args: &[String],
) -> Result<AssistantLoopProofOptions, String> {
    let mut root = ".".to_string();
    let mut pr_guidance = None;
    let mut agent_packet = None;
    let mut before = None;
    let mut after = None;
    let mut receipt = None;
    let mut ledger = None;
    let mut coverage_frontier = None;
    let mut gate_decision = None;
    let mut out =
        PathBuf::from(output::test_oracle_assistant_proof::DEFAULT_TEST_ORACLE_ASSISTANT_PROOF_OUT);
    let mut out_md = PathBuf::from(
        output::test_oracle_assistant_proof::DEFAULT_TEST_ORACLE_ASSISTANT_PROOF_MD_OUT,
    );

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "assistant-loop proof")?;
            }
            "--pr-guidance" => {
                i += 1;
                pr_guidance = Some(non_empty_path_arg(
                    args,
                    i,
                    "--pr-guidance",
                    "assistant-loop proof",
                )?);
            }
            "--agent-packet" => {
                i += 1;
                agent_packet = Some(non_empty_path_arg(
                    args,
                    i,
                    "--agent-packet",
                    "assistant-loop proof",
                )?);
            }
            "--before" => {
                i += 1;
                before = Some(non_empty_path_arg(
                    args,
                    i,
                    "--before",
                    "assistant-loop proof",
                )?);
            }
            "--after" => {
                i += 1;
                after = Some(non_empty_path_arg(
                    args,
                    i,
                    "--after",
                    "assistant-loop proof",
                )?);
            }
            "--receipt" => {
                i += 1;
                receipt = Some(non_empty_path_arg(
                    args,
                    i,
                    "--receipt",
                    "assistant-loop proof",
                )?);
            }
            "--ledger" => {
                i += 1;
                ledger = Some(non_empty_path_arg(
                    args,
                    i,
                    "--ledger",
                    "assistant-loop proof",
                )?);
            }
            "--coverage-frontier" => {
                i += 1;
                coverage_frontier = Some(non_empty_path_arg(
                    args,
                    i,
                    "--coverage-frontier",
                    "assistant-loop proof",
                )?);
            }
            "--gate-decision" => {
                i += 1;
                gate_decision = Some(non_empty_path_arg(
                    args,
                    i,
                    "--gate-decision",
                    "assistant-loop proof",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "assistant-loop proof")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "assistant-loop proof")?;
            }
            other => return Err(unknown_argument("assistant-loop proof", other)),
        }
        i += 1;
    }

    if pr_guidance.is_none()
        && agent_packet.is_none()
        && before.is_none()
        && after.is_none()
        && receipt.is_none()
        && ledger.is_none()
    {
        return Err(
            "assistant-loop proof requires at least one explicit artifact input".to_string(),
        );
    }

    Ok(AssistantLoopProofOptions {
        root,
        pr_guidance,
        agent_packet,
        before,
        after,
        receipt,
        ledger,
        coverage_frontier,
        gate_decision,
        out,
        out_md,
    })
}

fn parse_assistant_loop_health_options(
    args: &[String],
) -> Result<AssistantLoopHealthOptions, String> {
    let mut root = ".".to_string();
    let mut proofs = Vec::new();
    let mut out = PathBuf::from(output::assistant_loop_health::DEFAULT_ASSISTANT_LOOP_HEALTH_OUT);
    let mut out_md =
        PathBuf::from(output::assistant_loop_health::DEFAULT_ASSISTANT_LOOP_HEALTH_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "assistant-loop health")?;
            }
            "--proof" => {
                i += 1;
                proofs.push(non_empty_path_arg(
                    args,
                    i,
                    "--proof",
                    "assistant-loop health",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "assistant-loop health")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "assistant-loop health")?;
            }
            other => return Err(unknown_argument("assistant-loop health", other)),
        }
        i += 1;
    }

    if proofs.is_empty() {
        return Err("assistant-loop health requires at least one --proof path".to_string());
    }

    Ok(AssistantLoopHealthOptions {
        root,
        proofs,
        out,
        out_md,
    })
}

fn parse_first_action_options(args: &[String]) -> Result<FirstActionOptions, String> {
    let mut root = ".".to_string();
    let mut pr_guidance = None;
    let mut assistant_proof = None;
    let mut gap_ledger = None;
    let mut ledger = None;
    let mut baseline_delta = None;
    let mut receipt = None;
    let mut gate_decision = None;
    let mut coverage_frontier = None;
    let mut editor_context = None;
    let mut out = PathBuf::from(output::first_useful_action::DEFAULT_FIRST_USEFUL_ACTION_OUT);
    let mut out_md = PathBuf::from(output::first_useful_action::DEFAULT_FIRST_USEFUL_ACTION_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "first-action")?;
            }
            "--pr-guidance" => {
                i += 1;
                pr_guidance = Some(non_empty_path_arg(
                    args,
                    i,
                    "--pr-guidance",
                    "first-action",
                )?);
            }
            "--assistant-proof" => {
                i += 1;
                assistant_proof = Some(non_empty_path_arg(
                    args,
                    i,
                    "--assistant-proof",
                    "first-action",
                )?);
            }
            "--gap-ledger" => {
                i += 1;
                gap_ledger = Some(non_empty_path_arg(args, i, "--gap-ledger", "first-action")?);
            }
            "--ledger" => {
                i += 1;
                ledger = Some(non_empty_path_arg(args, i, "--ledger", "first-action")?);
            }
            "--baseline-delta" => {
                i += 1;
                baseline_delta = Some(non_empty_path_arg(
                    args,
                    i,
                    "--baseline-delta",
                    "first-action",
                )?);
            }
            "--receipt" => {
                i += 1;
                receipt = Some(non_empty_path_arg(args, i, "--receipt", "first-action")?);
            }
            "--gate-decision" => {
                i += 1;
                gate_decision = Some(non_empty_path_arg(
                    args,
                    i,
                    "--gate-decision",
                    "first-action",
                )?);
            }
            "--coverage-frontier" => {
                i += 1;
                coverage_frontier = Some(non_empty_path_arg(
                    args,
                    i,
                    "--coverage-frontier",
                    "first-action",
                )?);
            }
            "--editor-context" => {
                i += 1;
                editor_context = Some(non_empty_path_arg(
                    args,
                    i,
                    "--editor-context",
                    "first-action",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "first-action")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "first-action")?;
            }
            other => return Err(unknown_argument("first-action", other)),
        }
        i += 1;
    }

    if pr_guidance.is_none()
        && assistant_proof.is_none()
        && gap_ledger.is_none()
        && ledger.is_none()
        && baseline_delta.is_none()
        && receipt.is_none()
        && gate_decision.is_none()
        && coverage_frontier.is_none()
        && editor_context.is_none()
    {
        return Err("first-action requires at least one explicit artifact input".to_string());
    }

    Ok(FirstActionOptions {
        root,
        pr_guidance,
        assistant_proof,
        gap_ledger,
        ledger,
        baseline_delta,
        receipt,
        gate_decision,
        coverage_frontier,
        editor_context,
        out,
        out_md,
    })
}

fn baseline_created_at() -> Result<String, String> {
    generated_at_unix_ms()
}

fn first_action_generated_at() -> Result<String, String> {
    generated_at_unix_ms()
}

fn pr_review_front_panel_generated_at() -> Result<String, String> {
    generated_at_unix_ms()
}

fn comment_publish_plan_generated_at() -> Result<String, String> {
    generated_at_unix_ms()
}

fn report_packet_index_generated_at() -> Result<String, String> {
    generated_at_unix_ms()
}

fn gap_decision_ledger_generated_at() -> Result<String, String> {
    generated_at_unix_ms()
}

fn typescript_limitations_generated_at() -> Result<String, String> {
    generated_at_unix_ms()
}

fn typescript_false_actionable_generated_at() -> Result<String, String> {
    generated_at_unix_ms()
}

fn policy_readiness_generated_at() -> Result<String, String> {
    generated_at_unix_ms()
}

fn assistant_loop_health_generated_at() -> Result<String, String> {
    generated_at_unix_ms()
}

fn read_optional_text_for_report(label: &str, path: &Path) -> Result<String, String> {
    crate::bounded_input::read_to_string(path).map_err(|err| {
        format!(
            "read {label} {} failed: {err}",
            output::baseline_delta::display_path(path)
        )
    })
}

fn read_optional_manifest_for_report(
    root: &Path,
    manifest: &Path,
) -> Option<Result<String, String>> {
    let read_path = if manifest.is_absolute() {
        manifest.to_path_buf()
    } else {
        root.join(manifest)
    };
    match crate::bounded_input::read_to_string(&read_path) {
        Ok(text) => Some(Ok(text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => Some(Err(format!(
            "read suppression manifest {} failed: {err}",
            output::suppression_health::display_path(&read_path)
        ))),
    }
}

fn non_empty_path_arg(
    args: &[String],
    index: usize,
    flag: &str,
    command: &str,
) -> Result<PathBuf, String> {
    let value = non_empty_string_arg(args, index, flag, command)?;
    Ok(PathBuf::from(value))
}

fn non_empty_string_arg(
    args: &[String],
    index: usize,
    flag: &str,
    command: &str,
) -> Result<String, String> {
    let value = expect_value(args, index, flag)?;
    if value.trim().is_empty() {
        Err(format!("{command} {flag} requires a non-empty value"))
    } else {
        Ok(value.to_string())
    }
}

fn parse_outcome_format(value: &str) -> Result<OutcomeFormat, String> {
    const ACCEPTED: &[&str] = &["md", "markdown", "text", "json"];
    match value {
        "md" | "markdown" | "text" => Ok(OutcomeFormat::Markdown),
        "json" => Ok(OutcomeFormat::Json),
        _ => Err(unknown_value("outcome format", value, ACCEPTED)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiffReportFormat {
    Human,
    Json,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DiffOptions {
    root: PathBuf,
    /// `None` when `--base` was omitted; resolved by the loader's base
    /// authority at run time, never defaulted to a literal branch here.
    base: Option<String>,
    head: String,
    mode: Mode,
    format: DiffReportFormat,
    include_unchanged_tests: bool,
    explicit: CheckInputExplicit,
}

pub(super) fn diff(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_diff_help();
        return Ok(());
    }
    let options = parse_diff_options(args)?;
    let config = load_for_root(&options.root)?;
    // #3952 / RIPR-SPEC-0084: an omitted --base resolves the repository's
    // default branch through the same authority as `ripr check`, and an
    // explicit one is verified there, instead of assuming `origin/main`.
    let base = analysis::resolve_effective_base(&options.root, options.base.as_deref(), None)?;
    let diff_text = analysis::load_diff_range(&options.root, &base, &options.head)?;
    let changed_files = diff_changed_files_from_text(&diff_text);
    let diff_file = crate::app::temp_diff::write_temporary_diff_file(&diff_text)?;

    let check_result = run_diff_check_from_file(&options, &base, &config, &diff_file);
    let _ = std::fs::remove_file(&diff_file);
    // The temporary diff lives in a per-invocation private directory
    // (#2102); remove it too so runs do not accumulate empty dirs.
    if let Some(parent) = diff_file.parent() {
        let _ = std::fs::remove_dir(parent);
    }
    let output = check_result?;

    let report = output::diff_report::build_diff_report(
        &output,
        &base,
        &options.head,
        changed_files,
        diff_receipt_path(&base, &options.head),
    );
    match options.format {
        DiffReportFormat::Human => {
            print!("{}", output::diff_report::render_diff_report_human(&report))
        }
        DiffReportFormat::Json => {
            print!("{}", output::diff_report::render_diff_report_json(&report)?)
        }
    }
    Ok(())
}

fn parse_diff_options(args: &[String]) -> Result<DiffOptions, String> {
    let mut options = DiffOptions {
        root: PathBuf::from("."),
        base: None,
        head: "HEAD".to_string(),
        mode: Mode::Draft,
        format: DiffReportFormat::Human,
        include_unchanged_tests: true,
        explicit: CheckInputExplicit::default(),
    };

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                options.root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--base" => {
                i += 1;
                options.base = Some(expect_value(args, i, "--base")?.to_string());
            }
            "--head" => {
                i += 1;
                options.head = expect_value(args, i, "--head")?.to_string();
            }
            "--mode" => {
                i += 1;
                options.mode = parse_mode(expect_value(args, i, "--mode")?)?;
                options.explicit.mode = true;
            }
            "--format" => {
                i += 1;
                options.format = parse_diff_format(expect_value(args, i, "--format")?)?;
            }
            "--json" => options.format = DiffReportFormat::Json,
            "--no-unchanged-tests" => {
                options.include_unchanged_tests = false;
                options.explicit.include_unchanged_tests = true;
            }
            other => return Err(unknown_argument("diff", other)),
        }
        i += 1;
    }

    if options
        .base
        .as_deref()
        .is_some_and(|base| base.trim().is_empty())
    {
        return Err("diff --base requires a non-empty revision".to_string());
    }
    if options.head.trim().is_empty() {
        return Err("diff --head requires a non-empty revision".to_string());
    }

    Ok(options)
}

fn parse_diff_format(value: &str) -> Result<DiffReportFormat, String> {
    match value {
        "human" | "text" | "md" | "markdown" => Ok(DiffReportFormat::Human),
        "json" => Ok(DiffReportFormat::Json),
        _ => Err(format!(
            "unknown diff format {value:?}; expected `human`, `text`, `md`, `markdown`, or `json`"
        )),
    }
}

fn run_diff_check_from_file(
    options: &DiffOptions,
    base: &str,
    config: &RiprConfig,
    diff_file: &Path,
) -> Result<app::CheckOutput, String> {
    let mut input = CheckInput {
        root: options.root.clone(),
        base: Some(base.to_string()),
        diff_file: Some(diff_file.to_path_buf()),
        mode: options.mode.clone(),
        format: OutputFormat::Json,
        include_unchanged_tests: options.include_unchanged_tests,
        perl_facts_path: None,
        suppression_policy: None,
        git_timeout: None,
        git_candidate: None,
    };
    apply_to_check_input(&mut input, config, options.explicit);
    app::check_workspace_with_config(input, config)
}

fn diff_changed_files_from_text(diff_text: &str) -> Vec<output::diff_report::DiffChangedFile> {
    analysis::parse_unified_diff(diff_text)
        .into_iter()
        .map(|file| {
            let added_lines = file
                .added_lines
                .iter()
                .map(|line| line.line)
                .collect::<Vec<_>>();
            let removed_lines = file
                .removed_lines
                .iter()
                .map(|line| line.line)
                .collect::<Vec<_>>();
            output::diff_report::DiffChangedFile {
                // Rendering policy lives in the output layer; this stays an
                // adapter. The internal `PathBuf` remains platform-native for
                // filesystem joins.
                path: output::diff_report::portable_relative_path(&file.path),
                added_count: added_lines.len(),
                removed_count: removed_lines.len(),
                added_lines,
                removed_lines,
            }
        })
        .collect()
}

fn diff_receipt_path(base: &str, head: &str) -> String {
    format!(
        "target/ripr/receipts/diff-first-{}-{}.json",
        sanitize_ref_for_path(base),
        sanitize_ref_for_path(head)
    )
}

fn sanitize_ref_for_path(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() {
        "ref".to_string()
    } else {
        sanitized
    }
}

/// Whether a `-`-prefixed positional token is a `file:line` selector rather
/// than a mistyped flag.
///
/// Only consulted for tokens beginning with `-`, so a `probe:...` finding id
/// cannot reach here and is not checked — the caller's short-circuit already
/// accepts every non-`-` token. `app/selector.rs::selector_matches_file_line`
/// splits on the last `:` and imposes no constraint on the path, so the file
/// part may legitimately begin with `-`; only the line part must be numeric.
fn is_dash_prefixed_file_line_selector(value: &str) -> bool {
    value.rsplit_once(':').is_some_and(|(file, line)| {
        !file.is_empty() && !line.is_empty() && line.bytes().all(|byte| byte.is_ascii_digit())
    })
}

pub(super) fn explain(args: &[String]) -> Result<(), String> {
    let mut input = CheckInput::default();
    let mut explicit = CheckInputExplicit::default();
    let mut selector: Option<String> = None;
    // RIPR-SPEC-0140: `--from` loads a previously written check artifact
    // instead of re-running the pipeline. Scope flags passed alongside it
    // are assertions verified against the recording, not overrides.
    // `--mode` and `--no-unchanged-tests` feed the identity recomputation:
    // an artifact recorded with non-default values is only consumable when
    // the same values resolve here (flag or config).
    let mut from_artifact: Option<PathBuf> = None;
    let mut base_explicitly_provided = false;
    // `--worktree` matches `ripr check --worktree`: the finding may come
    // from uncommitted edits the committed-history diff never sees.
    let mut worktree = false;
    let mut root_explicitly_provided = false;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                input.root = PathBuf::from(expect_value(args, i, "--root")?);
                root_explicitly_provided = true;
            }
            "--base" => {
                i += 1;
                input.base = Some(expect_value(args, i, "--base")?.to_string());
                base_explicitly_provided = true;
            }
            "--diff" => {
                i += 1;
                input.diff_file = Some(PathBuf::from(expect_value(args, i, "--diff")?));
            }
            "--from" => {
                i += 1;
                from_artifact = Some(PathBuf::from(expect_value(args, i, "--from")?));
            }
            "--worktree" => worktree = true,
            "--mode" => {
                i += 1;
                input.mode = parse_mode(expect_value(args, i, "--mode")?)?;
                explicit.mode = true;
            }
            "--no-unchanged-tests" => {
                input.include_unchanged_tests = false;
                explicit.include_unchanged_tests = true;
            }
            "--perl-facts" => {
                i += 1;
                input.perl_facts_path = Some(PathBuf::from(expect_value(args, i, "--perl-facts")?));
            }
            "--suppression-policy" => {
                i += 1;
                input.suppression_policy = Some(PathBuf::from(expect_value(
                    args,
                    i,
                    "--suppression-policy",
                )?));
            }
            "--help" | "-h" => {
                help::print_explain_help();
                return Ok(());
            }
            // The selector is positional, so a mistyped flag used to be
            // accepted as one: analysis ran against `--fromm` as if it were a
            // finding id instead of reporting the typo. A `-` prefix alone
            // cannot decide that, though — `app/selector.rs` splits `file:line`
            // on the last `:` and accepts any path, so a file named
            // `-generated.rs` gives the valid selector `-generated.rs:42`.
            // Reject a `-`-prefixed token only when it is not selector-shaped.
            value
                if selector.is_none()
                    && (!value.starts_with('-') || is_dash_prefixed_file_line_selector(value)) =>
            {
                selector = Some(value.to_string());
            }
            other => return Err(unknown_argument("explain", other)),
        }
        i += 1;
    }
    // #4319: the synopsis reads `[--base REV|--diff PATH]` — alternatives —
    // but the loader gives `--diff` precedence and never validates `--base`
    // beside it, so both flags on one command line silently analyzed the
    // diff while appearing to assert the base. Fail at parse time, before
    // any pipeline run. Only the fresh path conflicts: beside `--from`, both
    // flags are assertions verified against the recording (RIPR-SPEC-0140),
    // so that verification path is intentionally left alone.
    if from_artifact.is_none() && base_explicitly_provided && input.diff_file.is_some() {
        return Err(base_with_diff_conflict_error("explain"));
    }
    worktree_scope_conflict(
        "explain",
        worktree,
        input.diff_file.is_some(),
        from_artifact.is_some(),
    )?;
    if selector.is_none() && !worktree {
        return Err(missing_selector_error(
            "missing finding selector",
            "ripr check --json",
        ));
    }
    resolve_worktree_root(&mut input, worktree, root_explicitly_provided)?;
    let config = load_for_root(&input.root)?;
    apply_to_check_input(&mut input, &config, explicit);
    let Some(selector) = selector else {
        return Err(missing_selector_error(
            "missing finding selector",
            &app::finding_navigation_with_worktree(&input, None, explicit.mode, worktree)
                .list_command(),
        ));
    };
    let asserted_base = if base_explicitly_provided {
        input.base.clone()
    } else {
        None
    };
    // #4319: `--diff -` reads the diff from stdin. On an attached terminal
    // that blocks until EOF with no visible sign of why, so the cli adapter
    // discloses the read before dispatching; the analysis loader itself
    // stays silent for library callers.
    let rendered = match from_artifact.as_deref() {
        Some(artifact_path) => app::explain_finding_from_artifact_with_navigation_mode(
            input,
            &selector,
            &config,
            artifact_path,
            asserted_base.as_deref(),
            explicit.mode,
        )?,
        None => {
            disclose_attached_terminal_stdin_read(input.diff_file.as_deref());
            app::explain_finding_with_config_and_navigation_mode(
                input,
                &selector,
                &config,
                explicit.mode,
                worktree,
            )?
        }
    };
    println!("{rendered}");
    Ok(())
}

/// The selector-less `explain`/`context` error. `listing` lists finding ids
/// for the same scope: a `--worktree` run names a worktree-scoped listing,
/// since plain `ripr check --json` omits findings only uncommitted edits
/// produce.
pub(super) fn missing_selector_error(what: &str, listing: &str) -> String {
    format!(
        "{what}; pass a finding id (e.g. `probe:src_lib.rs:error_path:abc123`) or `file:line`. Run `{listing}` to list finding ids"
    )
}

/// A `--worktree` drill-in without `--root` resolves the implicit project
/// root the same way `ripr check --worktree` does, so a manual invocation
/// from a project subdirectory analyzes the scope that listed the finding.
/// An explicit `--root` is kept as given.
pub(super) fn resolve_worktree_root(
    input: &mut CheckInput,
    worktree: bool,
    root_explicitly_provided: bool,
) -> Result<(), String> {
    if worktree && !root_explicitly_provided {
        check::resolve_implicit_workspace_root(input)?;
    }
    Ok(())
}

/// `--worktree` is its own diff source, like in `ripr check`: it cannot sit
/// beside `--diff`, and an artifact from `--from` already fixes the scope.
pub(super) fn worktree_scope_conflict(
    command: &str,
    worktree: bool,
    diff_file: bool,
    from_artifact: bool,
) -> Result<(), String> {
    if worktree && diff_file {
        return Err(format!(
            "{command} --worktree cannot be combined with --diff"
        ));
    }
    if worktree && from_artifact {
        return Err(format!(
            "{command} --worktree cannot be combined with --from: the artifact already records the diff it was written from"
        ));
    }
    Ok(())
}

pub(super) fn lsp(args: &[String]) -> Result<(), String> {
    for arg in args {
        match arg.as_str() {
            "--stdio" => {}
            "--version" | "-V" => {
                println!("ripr-lsp {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                help::print_lsp_help();
                return Ok(());
            }
            other => return Err(unknown_argument("lsp", other)),
        }
    }
    crate::lsp::serve()
}

/// `ripr pr-summary` — binary-first PR readiness summary (Campaign 31 item 8).
/// Composes existing RIPR artifacts into a PR evidence summary. Does NOT run
/// analysis or invoke Cargo. The canonical downstream replacement for
/// `cargo xtask ripr-pr-summary`.
pub(super) fn pr_summary(args: &[String]) -> Result<(), String> {
    crate::app::pr_summary::run_pr_summary(args)
}

/// `ripr annotations` — binary-first GitHub Actions annotations (item 8b).
/// Reads comments.json and emits `::warning` annotation lines. The canonical
/// downstream replacement for `cargo xtask ripr-annotations`.
pub(super) fn annotations(args: &[String]) -> Result<(), String> {
    crate::app::annotations::run_annotations(args)
}

/// `ripr pr-evidence` — binary-first PR evidence packet (Campaign 31 item 8c).
/// Writes the PR diff, runs an in-process RIPR check, and composes the result
/// into a PR evidence packet. The canonical downstream replacement for
/// `cargo xtask ripr-pr`. Unlike the xtask, it calls `check_workspace`
/// directly instead of shelling out to `cargo run -p ripr -- check`.
pub(super) fn pr_evidence(args: &[String]) -> Result<(), String> {
    crate::app::pr_evidence::run_pr_evidence(args)
}

/// `ripr impacted-evidence` — binary-first mutation-routing evidence (item 8e).
/// Reads PR evidence + labels and emits routing decision JSON + Markdown.
pub(super) fn impacted_evidence(args: &[String]) -> Result<(), String> {
    crate::app::impacted_evidence::run_impacted_evidence(args)
}

/// `ripr plus` — binary-first RIPR+ repo receipt (composition-only).
/// Composes the repo-wide RIPR+ quality-gate receipt from a pre-computed
/// `repo-exposure-summary-json` or `--gap-ledger` artifact. The canonical
/// downstream replacement for `cargo xtask ripr-plus`. Unlike the xtask, it
/// is artifact-composition-only and does not run an in-process full-repo scan.
pub(super) fn ripr_plus(args: &[String]) -> Result<(), String> {
    crate::app::ripr_plus::run_ripr_plus(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::agent_review_summary::NO_RECEIPT_BEFORE_REPAIR;
    use crate::output::first_pr::{
        MANUAL_RECEIPT_LABEL, MANUAL_VERIFY_LABEL, RECEIPT_AFTER_VERIFY_LABEL,
        REPAIR_AFTER_PHASE_LABEL, REPAIR_AFTER_PHASE_STEP, VERIFY_AFTER_EDIT_LABEL,
    };

    pub(super) fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    /// The adapter must route path rendering through the output-layer policy.
    ///
    /// This asserts the wiring only; `portable_relative_path`'s own behavior,
    /// including the platform-specific separator branch, is covered in
    /// `output::diff_report`.
    #[test]
    fn diff_changed_files_render_paths_through_the_output_policy() {
        let diff = "--- a/crates/ripr/src/lib.rs\n+++ b/crates/ripr/src/lib.rs\n@@ -1,1 +1,1 @@\n-old\n+new\n";
        let files = diff_changed_files_from_text(diff);
        assert_eq!(files.len(), 1, "expected one changed file: {files:?}");
        assert_eq!(
            files[0].path,
            output::diff_report::portable_relative_path(std::path::Path::new(
                "crates/ripr/src/lib.rs"
            ))
        );
        assert_eq!(files[0].path, "crates/ripr/src/lib.rs");
    }

    pub(super) fn unique_command_test_dir(label: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "ripr-command-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    pub(super) fn unique_repo_relative_test_dir(label: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        PathBuf::from("target/ripr").join(format!(
            "ripr-command-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    pub(super) fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    pub(super) fn copy_sample_workspace_to_temp(label: &str) -> Result<PathBuf, String> {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/sample");
        let dest = unique_command_test_dir(label);
        std::fs::create_dir_all(dest.join("src"))
            .map_err(|err| format!("failed to create temp sample src: {err}"))?;
        std::fs::create_dir_all(dest.join("tests"))
            .map_err(|err| format!("failed to create temp sample tests: {err}"))?;
        for relative in ["example.diff", "src/lib.rs", "tests/pricing.rs"] {
            std::fs::copy(source.join(relative), dest.join(relative))
                .map_err(|err| format!("failed to copy sample file {relative}: {err}"))?;
        }
        Ok(dest)
    }

    struct GeneratedWorkflowSmokeFixture<'a> {
        commands: &'a [&'a str],
        artifact_paths: &'a [&'a str],
        summary_sections: &'a [&'a str],
        non_blocking_steps: &'a [&'a str],
        gate_conditional_steps: &'a [&'a str],
        optional_sarif_steps: &'a [&'a str],
        forbidden_fragments: &'a [&'a str],
    }

    fn generated_workflow_smoke_fixture() -> GeneratedWorkflowSmokeFixture<'static> {
        GeneratedWorkflowSmokeFixture {
            commands: &[
                "ripr pilot",
                "ripr agent start",
                "ripr agent packet",
                "ripr check",
                "reports gap-ledger",
                "ripr review-comments",
                "gate evaluate",
                "ripr baseline diff",
                "zero status",
                "pr-ledger record",
                "policy waiver-aging",
                "policy suppression-health",
                "policy readiness",
                "policy operations",
                "policy history",
                "policy promote",
                "policy preview-promote",
                "assistant-loop proof",
                "assistant-loop health",
                "first-action",
                "pr-review front-panel",
                "first-pr",
                "reports index",
                "pr-comments plan",
                "ripr agent status",
                "ripr agent review-summary",
            ],
            artifact_paths: &[
                "target/ripr/pilot",
                "target/ripr/agent",
                "target/ripr/workflow",
                "target/ripr/reports",
                "target/ripr/review",
                "target/ripr/workflow/before.repo-exposure.json",
                "target/ripr/workflow/agent-packet.json",
                "target/ripr/workflow/agent-brief.json",
                "target/ripr/reports/agent-receipt.json",
                "target/ripr/workflow/agent-status.json",
                "target/ripr/workflow/agent-status.md",
                "target/ripr/workflow/agent-review-summary.json",
                "target/ripr/workflow/agent-review-summary.md",
                "target/ripr/reports/gap-decision-ledger.json",
                "target/ripr/reports/gap-decision-ledger.md",
                "target/ripr/reports/ripr-findings.sarif",
                "target/ripr/reports/ripr-seams.sarif",
                "target/ripr/reports/repo-ripr-badge.json",
                "target/ripr/reports/repo-ripr-badge-shields.json",
                "target/ripr/reports/gate-decision.json",
                "target/ripr/reports/gate-decision.md",
                "target/ripr/reports/baseline-debt-delta.json",
                "target/ripr/reports/baseline-debt-delta.md",
                "target/ripr/reports/ripr-zero-status.json",
                "target/ripr/reports/ripr-zero-status.md",
                "target/ripr/reports/pr-evidence-ledger.json",
                "target/ripr/reports/pr-evidence-ledger.md",
                "target/ripr/reports/waiver-aging.json",
                "target/ripr/reports/waiver-aging.md",
                "target/ripr/reports/suppression-health.json",
                "target/ripr/reports/suppression-health.md",
                "target/ripr/reports/policy-readiness.json",
                "target/ripr/reports/policy-readiness.md",
                "target/ripr/reports/policy-operations.json",
                "target/ripr/reports/policy-operations.md",
                "target/ripr/reports/policy-history.json",
                "target/ripr/reports/policy-history.md",
                "target/ripr/reports/policy-promotion-visible-only.json",
                "target/ripr/reports/policy-promotion-visible-only.md",
                "target/ripr/reports/policy-promotion-acknowledgeable.json",
                "target/ripr/reports/policy-promotion-acknowledgeable.md",
                "target/ripr/reports/policy-promotion-baseline-check.json",
                "target/ripr/reports/policy-promotion-baseline-check.md",
                "target/ripr/reports/policy-promotion-calibrated-gate.json",
                "target/ripr/reports/policy-promotion-calibrated-gate.md",
                "target/ripr/reports/preview-promotion-${language}-${class_label//_/-}.json",
                "target/ripr/reports/preview-promotion-${language}-${class_label//_/-}.md",
                "target/ripr/reports/preview-promotion-typescript-boundary-gap.md",
                "target/ripr/reports/preview-promotion-python-boundary-gap.md",
                "target/ripr/reports/test-oracle-assistant-proof.json",
                "target/ripr/reports/test-oracle-assistant-proof.md",
                "target/ripr/reports/assistant-loop-health.json",
                "target/ripr/reports/assistant-loop-health.md",
                "target/ripr/reports/first-useful-action.json",
                "target/ripr/reports/first-useful-action.md",
                "target/ripr/reports/pr-review-front-panel.json",
                "target/ripr/reports/pr-review-front-panel.md",
                "target/ripr/reports/start-here.json",
                "target/ripr/reports/start-here.md",
                "target/ripr/reports/index.json",
                "target/ripr/reports/index.md",
                "target/ripr/review/comments.json",
                "target/ripr/review/existing-comments.json",
                "target/ripr/review/comment-publish-plan.json",
                "target/ripr/review/comment-publish-plan.md",
                "target/ci/labels.json",
            ],
            summary_sections: &[
                "## RIPR advisory summary",
                "### Start here",
                "#### First-run status",
                "### Language preview grouping",
                "### PR review summary",
                "#### PR review at a glance",
                "### Recommended next test",
                "#### Recommended next test at a glance",
                "### Top recommendation",
                "### Agent review packet",
                "### Artifact packet",
                "### Uploaded review artifacts",
                "#### Uploaded artifacts at a glance",
                "### Gate decision",
                "#### Gate decision at a glance",
                "### Baseline debt delta",
                "#### Baseline debt movement",
                "### RIPR Zero status",
                "#### RIPR Zero at a glance",
                "### PR evidence ledger",
                "#### PR movement at a glance",
                "### Policy readiness",
                "#### Policy readiness at a glance",
                "### Policy operations",
                "#### Policy operations at a glance",
                "### Policy history",
                "#### Policy history at a glance",
                "### Policy promotion packets",
                "### Preview promotion packets",
                "### Waiver aging",
                "#### Waiver aging at a glance",
                "### Suppression health",
                "#### Suppression health at a glance",
                "### Test-oracle assistant proof",
                "#### Assistant proof at a glance",
                "### Agent proof status",
                "#### Agent proof status at a glance",
                "### SARIF and badge status",
                "### PR guidance annotations",
                "### PR inline comments",
                "### Known limits",
            ],
            non_blocking_steps: &[
                "Generate RIPR pilot packet",
                "Prepare RIPR editor-agent artifacts",
                "Generate RIPR agent loop artifacts",
                "Render RIPR repo badge artifacts",
                "Render RIPR baseline debt delta",
                "Render RIPR Zero status",
                "Render RIPR PR evidence ledger",
                "Render RIPR waiver aging",
                "Render RIPR suppression health",
                "Render RIPR policy readiness",
                "Render RIPR policy operations",
                "Render RIPR policy history",
                "Render RIPR policy promotion packets",
                "Render RIPR preview promotion packets",
                "Render RIPR test-oracle assistant proof",
                "Render RIPR assistant loop health",
                "Render RIPR first useful action",
                "Render RIPR PR review front panel",
                "Render RIPR report packet index",
                "Render RIPR LLM work-loop summaries",
                "Capture existing RIPR inline comments",
                "Plan RIPR inline comments",
                "Publish RIPR inline comments",
                "Capture RIPR gate labels",
                "Emit RIPR PR guidance annotations",
                "Check RIPR advisory artifacts",
                "Add RIPR advisory summary",
                "Upload RIPR report artifacts",
                // Upload infra is not analysis authority (#2009 review): a
                // CodeQL flake must not fail a gate the analysis passed.
                "Upload RIPR diff findings",
                "Upload RIPR repo seams",
            ],
            // Gate-critical analysis producers (#2009): advisory by default
            // but blocking when the operator opted into a blocking gate.
            gate_conditional_steps: &[
                "Run RIPR PR guidance report",
                "Render RIPR diff SARIF",
                "Render RIPR repo seam SARIF",
            ],
            optional_sarif_steps: &[
                "Render RIPR diff SARIF",
                "Render RIPR repo seam SARIF",
                "Upload RIPR diff findings",
                "Upload RIPR repo seams",
            ],
            forbidden_fragments: &[
                "fail-on-new-warning",
                "RIPR_PR_COMMENTS",
                "RIPR_GATE_MODE: \"acknowledgeable\"",
                "RIPR_GATE_MODE: \"baseline-check\"",
                "RIPR_GATE_MODE: \"calibrated-gate\"",
                // #3906: CI has no test edit between snapshots, so it
                // never runs the post-edit half of the loop.
                "ripr agent receipt",
                "ripr outcome",
                "> target/ripr/workflow/agent-verify.json",
                "> target/ripr/workflow/after.repo-exposure.json",
                "target/ripr/reports/targeted-test-outcome.json",
            ],
        }
    }

    fn workflow_step<'a>(workflow: &'a str, name: &str) -> &'a str {
        let marker = format!("      - name: {name}");
        let Some(start) = workflow.find(&marker) else {
            return "";
        };
        let rest = &workflow[start..];
        let end = rest.find("\n\n      - ").unwrap_or(rest.len());
        &rest[..end]
    }

    fn assert_contains_all(haystack: &str, label: &str, needles: &[&str]) {
        for needle in needles {
            assert!(
                haystack.contains(needle),
                "generated workflow missing {label} `{needle}`"
            );
        }
    }

    fn assert_step_before(workflow: &str, earlier: &str, later: &str) {
        let earlier_marker = format!("      - name: {earlier}");
        let later_marker = format!("      - name: {later}");
        assert!(
            workflow.contains(&earlier_marker),
            "generated workflow missing step `{earlier}`"
        );
        assert!(
            workflow.contains(&later_marker),
            "generated workflow missing step `{later}`"
        );
        let earlier_index = workflow.find(&earlier_marker).unwrap_or(usize::MAX);
        let later_index = workflow.find(&later_marker).unwrap_or(usize::MAX);
        assert!(
            earlier_index < later_index,
            "`{earlier}` must run before `{later}`"
        );
    }

    #[test]
    fn command_help_branches_return_ok() {
        assert_eq!(init(&args(&["--help"])), Ok(()));
        assert_eq!(config(&args(&["--help"])), Ok(()));
        assert_eq!(pilot(&args(&["--help"])), Ok(()));
        assert_eq!(review_comments(&args(&["--help"])), Ok(()));
        assert_eq!(gate(&args(&["--help"])), Ok(()));
        assert_eq!(calibrate(&args(&["--help"])), Ok(()));
        assert_eq!(agent(&args(&["--help"])), Ok(()));
        assert_eq!(agent(&args(&["start", "--help"])), Ok(()));
        assert_eq!(agent(&args(&["brief", "--help"])), Ok(()));
        assert_eq!(agent(&args(&["status", "--help"])), Ok(()));
        assert_eq!(check(&args(&["--help"])), Ok(()));
        assert_eq!(explain(&args(&["--help"])), Ok(()));
        assert_eq!(context(&args(&["--help"])), Ok(()));
        assert_eq!(reports(&args(&["--help"])), Ok(()));
        assert_eq!(doctor(&args(&["--help"])), Ok(()));
        assert_eq!(lsp(&args(&["--help"])), Ok(()));
    }

    #[test]
    fn reports_gap_ledger_requires_records_input() {
        assert_eq!(
            reports(&args(&["gap-ledger"])),
            Err(
                "reports gap-ledger requires --records PATH, --repo-exposure PATH, or --check-output PATH"
                    .to_string()
            )
        );
        assert_eq!(
            reports(&args(&["gap-ledger", "--records"])),
            Err("missing value for --records".to_string())
        );
        assert_eq!(
            reports(&args(&["gap-ledger", "--repo-exposure"])),
            Err("missing value for --repo-exposure".to_string())
        );
        assert_eq!(
            reports(&args(&["gap-ledger", "--check-output"])),
            Err("missing value for --check-output".to_string())
        );
        assert_eq!(
            reports(&args(&[
                "gap-ledger",
                "--records",
                "records.json",
                "--check-output",
                "check.json"
            ])),
            Err(
                "reports gap-ledger accepts only one of --records, --repo-exposure, or --check-output"
                    .to_string()
            )
        );
        assert_eq!(
            reports(&args(&["unknown"])),
            Err(
                "unknown reports subcommand \"unknown\"; expected `index`, `gap-ledger`, `ts-limitations`, or `ts-false-actionable`"
                    .to_string()
            )
        );
    }

    #[test]
    fn reports_ts_limitations_requires_check_output_input() {
        assert_eq!(
            reports(&args(&["ts-limitations"])),
            Err("reports ts-limitations requires --check-output PATH".to_string())
        );
        assert_eq!(
            reports(&args(&["ts-limitations", "--check-output"])),
            Err("missing value for --check-output".to_string())
        );
    }

    #[test]
    fn reports_ts_limitations_writes_json_and_markdown_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("ts-limitations");
        std::fs::create_dir_all(&dir)
            .map_err(|err| format!("create TypeScript limitations dir: {err}"))?;
        let check_output =
            repo_root().join("fixtures/typescript_static_limit_taxonomy/expected/check.json");
        let out = dir.join("typescript-limitations.json");
        let out_md = dir.join("typescript-limitations.md");

        reports(&args(&[
            "ts-limitations",
            "--check-output",
            &check_output.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text = std::fs::read_to_string(&out)
            .map_err(|err| format!("read TypeScript limitations JSON: {err}"))?;
        let value: serde_json::Value = serde_json::from_str(&json_text)
            .map_err(|err| format!("parse TypeScript limitations JSON: {err}"))?;
        assert_eq!(value["kind"], "typescript_limitation_leaderboard");
        assert_eq!(value["status"], "advisory");
        assert_eq!(value["summary"]["typescript_family_findings_total"], 4);
        assert_eq!(
            value["summary"]["top_limitation_kind"],
            "typescript_package_root_unresolved"
        );
        assert!(json_text.contains("typescript_import_graph_unresolved"));
        assert!(json_text.contains("static_limit_kind"));

        let markdown = std::fs::read_to_string(&out_md)
            .map_err(|err| format!("read TypeScript limitations Markdown: {err}"))?;
        assert!(markdown.contains("# RIPR TypeScript Limitation Leaderboard"));
        assert!(markdown.contains("typescript_package_root_unresolved"));
        assert!(markdown.contains("badge artifacts keep their existing authority"));

        std::fs::remove_dir_all(&dir)
            .map_err(|err| format!("remove TypeScript limitations dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn reports_ts_limitations_fails_closed_on_unreadable_check_output() -> Result<(), String> {
        // Per docs/EXIT_CODES.md an unreadable input means the command could
        // not complete: the CLI must return the read error (exit 2 at the
        // process boundary) instead of writing a `blocked` report and
        // exiting 0.
        let missing =
            unique_command_test_dir("ts-limitations-missing").join("definitely-missing-check.json");
        let result = reports(&args(&[
            "ts-limitations",
            "--check-output",
            &missing.display().to_string(),
        ]));
        let err = result.err().ok_or_else(|| {
            "unreadable check output must fail closed, not exit cleanly".to_string()
        })?;
        assert!(
            err.contains("read check output") && err.contains("failed"),
            "error must name the unreadable input, got: {err}"
        );
        Ok(())
    }

    #[test]
    fn reports_ts_false_actionable_requires_corpus_input() {
        assert_eq!(
            reports(&args(&["ts-false-actionable"])),
            Err("reports ts-false-actionable requires --corpus PATH".to_string())
        );
        assert_eq!(
            reports(&args(&["ts-false-actionable", "--corpus"])),
            Err("missing value for --corpus".to_string())
        );
    }

    #[test]
    fn reports_ts_false_actionable_writes_json_and_markdown_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("ts-false-actionable");
        std::fs::create_dir_all(&dir)
            .map_err(|err| format!("create TypeScript false-actionable dir: {err}"))?;
        let corpus =
            repo_root().join("fixtures/typescript-preview-false-actionable-audit/corpus.json");
        let out = dir.join("typescript-false-actionable-audit.json");
        let out_md = dir.join("typescript-false-actionable-audit.md");

        reports(&args(&[
            "ts-false-actionable",
            "--corpus",
            &corpus.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text = std::fs::read_to_string(&out)
            .map_err(|err| format!("read TypeScript false-actionable JSON: {err}"))?;
        let value: serde_json::Value = serde_json::from_str(&json_text)
            .map_err(|err| format!("parse TypeScript false-actionable JSON: {err}"))?;
        assert_eq!(value["kind"], "typescript_false_actionable_audit");
        assert_eq!(value["status"], "advisory");
        assert_eq!(value["summary"]["cases_total"], 15);
        assert_eq!(value["summary"]["false_actionable_total"], 0);
        assert_eq!(value["summary"]["false_actionable_rate"], 0.0);
        assert_eq!(value["summary"]["preview_boundary_violation_total"], 0);
        assert!(
            json_text.contains("This report does not edit source, generate tests, call providers")
        );

        let markdown = std::fs::read_to_string(&out_md)
            .map_err(|err| format!("read TypeScript false-actionable Markdown: {err}"))?;
        assert!(markdown.contains("# RIPR TypeScript False-Actionable Audit"));
        assert!(markdown.contains("False actionable: `0` / `15` (`0.000`)"));
        assert!(
            markdown.contains("Gate-decision and badge artifacts keep their existing authority")
        );

        std::fs::remove_dir_all(&dir)
            .map_err(|err| format!("remove TypeScript false-actionable dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn reports_ts_false_actionable_fails_closed_on_unreadable_corpus() -> Result<(), String> {
        // Per docs/EXIT_CODES.md an unreadable input means the command could
        // not complete: the CLI must return the read error (exit 2 at the
        // process boundary) instead of writing a `blocked` report and
        // exiting 0.
        let missing = unique_command_test_dir("ts-false-actionable-missing")
            .join("definitely-missing-corpus.json");
        let result = reports(&args(&[
            "ts-false-actionable",
            "--corpus",
            &missing.display().to_string(),
        ]));
        let err = result
            .err()
            .ok_or_else(|| "unreadable corpus must fail closed, not exit cleanly".to_string())?;
        assert!(
            err.contains("read TypeScript false-actionable audit corpus") && err.contains("failed"),
            "error must name the unreadable input, got: {err}"
        );
        Ok(())
    }

    #[test]
    fn reports_gap_ledger_writes_json_and_markdown_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("gap-ledger");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create gap ledger dir: {err}"))?;
        let records = repo_root().join("fixtures/gap-decision-ledger/corpus.json");
        let out = dir.join("gap-decision-ledger.json");
        let out_md = dir.join("gap-decision-ledger.md");

        reports(&args(&[
            "gap-ledger",
            "--records",
            &records.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text =
            std::fs::read_to_string(&out).map_err(|err| format!("read gap ledger JSON: {err}"))?;
        assert!(json_text.contains("\"kind\": \"gap_decision_ledger\""));
        assert!(json_text.contains("\"records_total\": 18"));
        let markdown = std::fs::read_to_string(&out_md)
            .map_err(|err| format!("read gap ledger Markdown: {err}"))?;
        assert!(markdown.contains("# RIPR Gap Decision Ledger"));
        assert!(markdown.contains("gate candidates=`1`"));

        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove gap ledger dir: {err}"))?;
        Ok(())
    }

    fn read_json_file(path: &Path) -> Result<serde_json::Value, String> {
        serde_json::from_str(
            &std::fs::read_to_string(path)
                .map_err(|err| format!("read {}: {err}", path.display()))?,
        )
        .map_err(|err| format!("parse {}: {err}", path.display()))
    }

    fn gap_ledger_from(
        root: &str,
        flag: &str,
        input: &Path,
        out: &Path,
    ) -> Result<serde_json::Value, String> {
        let out_md = out.with_extension("md");
        reports(&args(&[
            "gap-ledger",
            "--root",
            root,
            flag,
            &input.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;
        read_json_file(out)
    }

    fn pricing_gap_record(anchor_file: &str) -> serde_json::Value {
        serde_json::json!({
            "gap_id": "gap:pr:pricing",
            "canonical_gap_id": "gap:rust:pricing",
            "kind": "MissingBoundaryAssertion",
            "language": "rust",
            "language_status": "stable",
            "gap_state": "actionable",
            "repair_route": {
                "route_kind": "AddBoundaryAssertion",
                "related_test": "tests/pricing.rs::discount_threshold"
            },
            "anchor": {"file": anchor_file, "line": 1}
        })
    }

    /// #4544: the ledger writer never hashes the workspace. A records input
    /// without an analysis stamp yields no usable stamp (the LSP then reports
    /// `unverifiable_subject`), and re-rendering that unstamped ledger through
    /// `--records` does not mint a fresh stamp for the current bytes.
    #[test]
    fn reports_gap_ledger_without_an_input_stamp_is_unavailable_and_stays_so() -> Result<(), String>
    {
        use crate::output::gap_source_subject::{SourceSubjectCheck, check_source_subject};
        let dir = unique_command_test_dir("gap-ledger-source-subject-missing");
        std::fs::create_dir_all(dir.join("src")).map_err(|err| format!("create src: {err}"))?;
        std::fs::write(dir.join("src/pricing.rs"), "abc\n")
            .map_err(|err| format!("write anchor: {err}"))?;
        let records = dir.join("records.json");
        std::fs::write(
            &records,
            serde_json::json!({"records": [pricing_gap_record("src/pricing.rs")]}).to_string(),
        )
        .map_err(|err| format!("write records: {err}"))?;
        let root = dir.display().to_string();

        let ledger = gap_ledger_from(&root, "--records", &records, &dir.join("ledger.json"))?;
        assert_eq!(ledger.get("source_subject"), None);
        assert_eq!(
            ledger["source_subject_unavailable"],
            serde_json::json!("input_source_subject_missing")
        );
        let required = std::collections::BTreeSet::from([
            "src/pricing.rs".to_string(),
            "tests/pricing.rs".to_string(),
        ]);
        assert!(matches!(
            check_source_subject(&dir, ledger.get("source_subject"), &required),
            SourceSubjectCheck::Unverifiable(_)
        ));

        let rerendered = gap_ledger_from(
            &root,
            "--records",
            &dir.join("ledger.json"),
            &dir.join("rerendered.json"),
        )?;
        assert_eq!(rerendered.get("source_subject"), None);
        assert_eq!(
            rerendered["source_subject_unavailable"],
            serde_json::json!("input_source_subject_missing")
        );

        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove dir: {err}"))?;
        Ok(())
    }

    /// #4544: a stamped input's digests are copied, never recomputed: the
    /// ledger carries the input's digest even though the file changed before
    /// the ledger was written, so the LSP reports the ledger stale. The root is
    /// relative and the record names the anchor by absolute path, which must
    /// still resolve to the same repo-relative stamp entry.
    #[test]
    fn reports_gap_ledger_copies_the_input_stamp_for_absolute_paths_under_a_relative_root()
    -> Result<(), String> {
        use crate::output::gap_source_subject::{SourceSubjectCheck, check_source_subject};
        let dir = unique_repo_relative_test_dir("gap-ledger-source-subject-copy");
        assert!(dir.is_relative(), "{}", dir.display());
        std::fs::create_dir_all(dir.join("src")).map_err(|err| format!("create src: {err}"))?;
        std::fs::write(dir.join("src/pricing.rs"), "abc\n")
            .map_err(|err| format!("write anchor: {err}"))?;
        let absolute_anchor = std::env::current_dir()
            .map_err(|err| format!("cwd: {err}"))?
            .join(&dir)
            .join("src/pricing.rs");
        let root = dir.display().to_string();
        let analysis_stamp = serde_json::json!({
            "digest_algorithm": "sha256",
            "files": [
                {
                    "path": "src/pricing.rs",
                    "digest": "sha256:edeaaff3f1774ad2888673770c6d64097e391bc362d7d6fb34982ddf0efd18cb"
                },
                {"path": "tests/pricing.rs", "digest": null}
            ]
        });
        let records = dir.join("records.json");
        std::fs::write(
            &records,
            serde_json::json!({
                "root": root,
                "source_subject": analysis_stamp,
                "records": [pricing_gap_record(&absolute_anchor.display().to_string())]
            })
            .to_string(),
        )
        .map_err(|err| format!("write records: {err}"))?;
        // The workspace moves on after the analysis stamped it.
        std::fs::write(dir.join("src/pricing.rs"), "abd\n")
            .map_err(|err| format!("edit anchor: {err}"))?;

        let ledger = gap_ledger_from(&root, "--records", &records, &dir.join("ledger.json"))?;
        assert_eq!(ledger["source_subject"], analysis_stamp);
        assert_eq!(ledger.get("source_subject_unavailable"), None);
        let required = std::collections::BTreeSet::from([
            "src/pricing.rs".to_string(),
            "tests/pricing.rs".to_string(),
        ]);
        assert_eq!(
            check_source_subject(&dir, ledger.get("source_subject"), &required),
            SourceSubjectCheck::Stale("src/pricing.rs".to_string())
        );

        // Re-rendering the written ledger keeps the analysis stamp.
        let rerendered = gap_ledger_from(
            &root,
            "--records",
            &dir.join("ledger.json"),
            &dir.join("rerendered.json"),
        )?;
        assert_eq!(rerendered["source_subject"], analysis_stamp);

        // A stamp that omits a named file yields no usable stamp.
        std::fs::write(
            &records,
            serde_json::json!({
                "root": root,
                "source_subject": {"digest_algorithm": "sha256", "files": [
                    analysis_stamp["files"][0].clone()
                ]},
                "records": [pricing_gap_record("src/pricing.rs")]
            })
            .to_string(),
        )
        .map_err(|err| format!("rewrite records: {err}"))?;
        let incomplete =
            gap_ledger_from(&root, "--records", &records, &dir.join("incomplete.json"))?;
        assert_eq!(incomplete.get("source_subject"), None);
        assert_eq!(
            incomplete["source_subject_unavailable"],
            serde_json::json!("input_source_subject_incomplete")
        );

        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove dir: {err}"))?;
        Ok(())
    }

    /// #4544 regression: analysis, then an edit, then the ledger write. The
    /// repo-exposure artifact stamps the bytes the analysis run read; the
    /// ledger derived after the edit copies that stamp, so the LSP-side check
    /// reports the edited file stale instead of vouching for the new bytes.
    #[test]
    fn reports_gap_ledger_after_an_edit_keeps_the_analysis_time_stamp() -> Result<(), String> {
        use crate::output::gap_source_subject::{SourceSubjectCheck, check_source_subject};
        let dir = unique_command_test_dir("gap-ledger-source-subject-analysis");
        let fixture = repo_root().join("fixtures/boundary_gap/input");
        for file in ["Cargo.toml", "src/lib.rs", "tests/pricing.rs"] {
            let target = dir.join(file);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|err| format!("create {}: {err}", parent.display()))?;
            }
            std::fs::copy(fixture.join(file), &target)
                .map_err(|err| format!("copy {file}: {err}"))?;
        }
        let config = crate::config::RiprConfig::default();
        let (classified, limit_info) =
            crate::analysis::inventory_classified_seams_at_with_config(&dir, &config)?;
        let context = crate::agent::artifact::RepoExposureArtifactContext::for_repo_exposure(
            dir.clone(),
            "draft".to_string(),
            None,
            &config,
        )?;
        let repo_exposure_json =
            crate::output::repo_exposure::render_repo_exposure_json_with_context(
                &classified,
                limit_info.as_ref(),
                None,
                None,
                None,
                &context,
            )?;
        let repo_exposure = dir.join("repo-exposure.json");
        std::fs::write(&repo_exposure, &repo_exposure_json)
            .map_err(|err| format!("write repo exposure: {err}"))?;
        let analysis_stamp = read_json_file(&repo_exposure)?["source_subject"].clone();
        let stamped_paths = analysis_stamp["files"]
            .as_array()
            .map(|files| {
                files
                    .iter()
                    .filter_map(|file| file["path"].as_str().map(ToOwned::to_owned))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        assert_eq!(stamped_paths, ["src/lib.rs", "tests/pricing.rs"]);

        std::fs::write(
            dir.join("src/lib.rs"),
            format!(
                "{}\n// edited after analysis\n",
                std::fs::read_to_string(dir.join("src/lib.rs"))
                    .map_err(|err| format!("read lib: {err}"))?
            ),
        )
        .map_err(|err| format!("edit lib: {err}"))?;

        let ledger = gap_ledger_from(
            &dir.display().to_string(),
            "--repo-exposure",
            &repo_exposure,
            &dir.join("ledger.json"),
        )?;
        assert_eq!(ledger.get("source_subject_unavailable"), None);
        assert_eq!(ledger["source_subject"], analysis_stamp);
        let required = stamped_paths.into_iter().collect();
        assert_eq!(
            check_source_subject(&dir, ledger.get("source_subject"), &required),
            SourceSubjectCheck::Stale("src/lib.rs".to_string())
        );

        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn reports_gap_ledger_fails_closed_for_blank_and_mixed_verification_routes()
    -> Result<(), String> {
        for (case, commands) in [
            ("blank", serde_json::json!([" \t "])),
            ("mixed", serde_json::json!(["cargo test", "  "])),
        ] {
            let dir = unique_command_test_dir(&format!("gap-ledger-{case}-verify"));
            std::fs::create_dir_all(&dir)
                .map_err(|error| format!("create {case} gap-ledger dir: {error}"))?;
            let corpus_text = std::fs::read_to_string(
                repo_root().join("fixtures/gap-decision-ledger/corpus.json"),
            )
            .map_err(|error| format!("read gap-ledger corpus: {error}"))?;
            let corpus: serde_json::Value = serde_json::from_str(&corpus_text)
                .map_err(|error| format!("parse gap-ledger corpus: {error}"))?;
            let mut record = corpus
                .get("cases")
                .and_then(serde_json::Value::as_array)
                .and_then(|cases| cases.first())
                .and_then(|case| case.get("expected_gap_record"))
                .cloned()
                .ok_or_else(|| "gap-ledger corpus first expected record missing".to_string())?;
            record["verification_commands"] = commands;
            let records = dir.join("records.json");
            std::fs::write(
                &records,
                serde_json::json!({"records": [record]}).to_string(),
            )
            .map_err(|error| format!("write {case} gap-ledger records: {error}"))?;
            let out = dir.join("gap-decision-ledger.json");
            let out_md = dir.join("gap-decision-ledger.md");

            reports(&args(&[
                "gap-ledger",
                "--records",
                &records.display().to_string(),
                "--out",
                &out.display().to_string(),
                "--out-md",
                &out_md.display().to_string(),
            ]))?;

            let json_text = std::fs::read_to_string(&out)
                .map_err(|error| format!("read {case} gap-ledger JSON: {error}"))?;
            let value: serde_json::Value = serde_json::from_str(&json_text)
                .map_err(|error| format!("parse {case} gap-ledger JSON: {error}"))?;
            assert_eq!(value["summary"]["projection_pr_comment_eligible"], 0);
            assert_eq!(value["summary"]["projection_gate_candidate"], 0);
            assert_eq!(value["summary"]["projection_agent_packet_eligible"], 0);
            let markdown = std::fs::read_to_string(&out_md)
                .map_err(|error| format!("read {case} gap-ledger Markdown: {error}"))?;
            assert!(markdown.contains("Verify: `unavailable_incomplete_command_list`"));
            assert!(!markdown.contains("  - `cargo test`"));

            std::fs::remove_dir_all(&dir)
                .map_err(|error| format!("remove {case} gap-ledger dir: {error}"))?;
        }
        Ok(())
    }

    #[test]
    fn reports_gap_ledger_derives_output_contract_gap_from_check_output() -> Result<(), String> {
        let dir = unique_command_test_dir("gap-ledger-check-output");
        std::fs::create_dir_all(&dir)
            .map_err(|err| format!("create gap ledger check output dir: {err}"))?;
        let check_output = dir.join("check.json");
        let out = dir.join("gap-decision-ledger.json");
        let out_md = dir.join("gap-decision-ledger.md");
        std::fs::write(&check_output, check_output_with_presentation_text_gap())
            .map_err(|err| format!("write check output: {err}"))?;

        reports(&args(&[
            "gap-ledger",
            "--check-output",
            &check_output.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text = std::fs::read_to_string(&out)
            .map_err(|err| format!("read check-output gap ledger JSON: {err}"))?;
        assert!(json_text.contains("\"source_kind\": \"check_output\""));
        assert!(json_text.contains("\"kind\": \"MissingOutputContract\""));
        assert!(json_text.contains("\"route_kind\": \"AddOutputGolden\""));
        assert!(json_text.contains("cargo xtask goldens check"));
        assert!(json_text.contains("\"projection_pr_comment_eligible\": 1"));
        assert!(json_text.contains("\"projection_gate_candidate\": 0"));
        let markdown = std::fs::read_to_string(&out_md)
            .map_err(|err| format!("read check-output gap ledger Markdown: {err}"))?;
        assert!(markdown.contains("MissingOutputContract"));
        assert!(markdown.contains("AddOutputGolden"));

        std::fs::remove_dir_all(&dir)
            .map_err(|err| format!("remove gap ledger check output dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn reports_gap_ledger_derives_python_static_limit_as_report_only() -> Result<(), String> {
        let dir = unique_command_test_dir("gap-ledger-python-static-limit");
        std::fs::create_dir_all(&dir)
            .map_err(|err| format!("create python static-limit gap ledger dir: {err}"))?;
        let check_output =
            repo_root().join("fixtures/python_dynamic_dispatch_limit/expected/check.json");
        let out = dir.join("gap-decision-ledger.json");
        let out_md = dir.join("gap-decision-ledger.md");

        reports(&args(&[
            "gap-ledger",
            "--check-output",
            &check_output.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text = std::fs::read_to_string(&out)
            .map_err(|err| format!("read python static-limit gap ledger JSON: {err}"))?;
        let value: serde_json::Value = serde_json::from_str(&json_text)
            .map_err(|err| format!("parse python static-limit gap ledger JSON: {err}"))?;
        assert_eq!(value["status"], "advisory");
        assert_eq!(value["summary"]["records_total"], 1);
        assert_eq!(value["summary"]["static_limitation_total"], 1);
        assert_eq!(value["summary"]["projection_agent_packet_eligible"], 0);
        assert_eq!(value["records"][0]["kind"], "StaticLimitation");
        assert_eq!(value["records"][0]["repairability"], "analyzer_limitation");
        assert_eq!(value["records"][0]["static_limit_kind"], "dynamic_dispatch");
        assert_eq!(
            value["records"][0]["projection_eligibility"]["agent_packet"]["eligible"],
            false
        );
        assert!(value["records"][0].get("repair_route").is_none());

        std::fs::remove_dir_all(&dir)
            .map_err(|err| format!("remove python static-limit gap ledger dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn reports_gap_ledger_derives_repo_scoped_records_from_repo_exposure() -> Result<(), String> {
        let dir = unique_command_test_dir("gap-ledger-repo-exposure");
        std::fs::create_dir_all(&dir)
            .map_err(|err| format!("create gap ledger repo exposure dir: {err}"))?;
        let repo_exposure = dir.join("repo-exposure.json");
        let out = dir.join("gap-decision-ledger.json");
        let out_md = dir.join("gap-decision-ledger.md");
        std::fs::write(
            &repo_exposure,
            repo_exposure_with_actionable_evidence_record(),
        )
        .map_err(|err| format!("write repo exposure: {err}"))?;

        reports(&args(&[
            "gap-ledger",
            "--repo-exposure",
            &repo_exposure.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text = std::fs::read_to_string(&out)
            .map_err(|err| format!("read derived gap ledger JSON: {err}"))?;
        assert!(json_text.contains("\"source_kind\": \"repo_exposure\""));
        assert!(json_text.contains("\"records_total\": 1"));
        assert!(json_text.contains("\"kind\": \"MissingBoundaryAssertion\""));
        assert!(json_text.contains("\"scope\": \"repo_scoped\""));
        assert!(json_text.contains("\"route_kind\": \"AddBoundaryAssertion\""));
        assert!(json_text.contains("\"ripr_zero_count\":"));
        let markdown = std::fs::read_to_string(&out_md)
            .map_err(|err| format!("read derived gap ledger Markdown: {err}"))?;
        assert!(markdown.contains("AddBoundaryAssertion"));
        assert!(markdown.contains("repo_scoped"));

        std::fs::remove_dir_all(&dir)
            .map_err(|err| format!("remove gap ledger repo exposure dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn gap_ledger_source_path_is_bound_to_invocation_cwd() -> Result<(), String> {
        let rendered =
            ledger_source_path(Path::new("repo/target/ripr/reports/repo-exposure.json"))?;
        let expected_suffix = Path::new("repo/target/ripr/reports/repo-exposure.json")
            .to_string_lossy()
            .replace('\\', "/");
        assert!(rendered.ends_with(&expected_suffix), "{rendered}");
        assert!(Path::new(&rendered).is_absolute(), "{rendered}");
        Ok(())
    }

    fn repo_exposure_with_actionable_evidence_record() -> &'static str {
        r#"{
  "schema_version": "0.3",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-pricing-threshold",
      "file": "src/pricing.rs",
      "line": 88,
      "evidence_record": {
        "schema_version": "0.1",
        "seam_id": "seam-pricing-threshold",
        "canonical_gap_id": "gap:rust:pricing:threshold",
        "raw_findings": [
          {
            "file": "src/pricing.rs",
            "line": 88,
            "kind": "weakly_gripped",
            "expression": "amount >= discount_threshold",
            "probe_kind": "predicate_boundary",
            "source_id": "seam-pricing-threshold",
            "evidence_record_ref": "seam-pricing-threshold"
          }
        ],
        "canonical_item": {
          "canonical_gap_id": "gap:rust:pricing:threshold",
          "raw_group_size": 1,
          "canonical_item_kind": "gap",
          "evidence_class": "predicate_boundary",
          "gap_state": "actionable",
          "actionability": "upgrade_assertion",
          "group_reason": "same owner and missing discriminator",
          "why": "related tests reach the seam but miss the boundary discriminator",
          "recommended_repair": "Add an exact boundary assertion.",
          "related_test": {
            "name": "below_threshold_has_no_discount",
            "file": "tests/pricing_tests.rs",
            "line": 12,
            "reason": "direct owner call"
          },
          "verify_command": "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json",
          "confidence": {
            "basis": "static_only",
            "notes": ["no imported runtime calibration data"]
          }
        },
        "owner": "pricing::discounted_total",
        "location": {
          "file": "src/pricing.rs",
          "line": 88
        },
        "seam_kind": "predicate_boundary",
        "grip_class": "weakly_gripped",
        "headline_eligible": true,
        "recommendation": {
          "action": "write_targeted_test",
          "reason": "add the missing boundary assertion",
          "recommended_test": {
            "name": "discounts_at_threshold",
            "file": "tests/pricing_tests.rs",
            "reason": "nearest pricing test module"
          },
          "assertion_shape": {
            "kind": "exact_return_value",
            "example": "assert_eq!(discounted_total(100, 100), 90)"
          },
          "verify_command": "ripr agent verify --root . --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --json"
        },
        "actionability": {
          "class": "actionable_assertion_upgrade",
          "reason": "related tests reach the seam but still miss a concrete discriminator",
          "has_concrete_guidance": true
        }
      }
    }
  ]
}"#
    }

    fn check_output_with_presentation_text_gap() -> &'static str {
        r#"{
  "schema_version": "0.1",
  "tool": "ripr",
  "findings": [
    {"id": "help-label-decl", "source_currentness": "candidate_current"},
    {"id": "help-label-literal", "source_currentness": "candidate_current"}
  ],
  "finding_alignment": {
    "scope": "supported_classes",
    "items": [
      {
        "canonical_gap_id": "presentation_text::HELP_DEVICE_LABEL",
        "canonical_item_kind": "gap",
        "evidence_class": "presentation_text",
        "gap_state": "actionable",
        "actionability": "add_output_observer",
        "raw_group_size": 2,
        "group_reason": "declaration_and_literal_same_text_constant",
        "why": "Changed text flows to CLI help output and no supported output observer is found.",
        "recommended_repair": "Add or update a help-output snapshot assertion for HELP_DEVICE_LABEL.",
        "related_test": null,
        "verify_command": "cargo xtask evidence-quality-scorecard",
        "static_limitations": [],
        "confidence": {
          "basis": "fixture_backed",
          "notes": ["Visible unobserved presentation text is actionable only for supported sink patterns."]
        },
        "raw_findings": [
          {
            "file": "crates/ripr/src/cli/help.rs",
            "line": 42,
            "kind": "exposed",
            "expression": "pub const HELP_DEVICE_LABEL: &str =",
            "probe_kind": "field_construction",
            "source_id": "help-label-decl",
            "evidence_record_ref": "help-label-decl"
          },
          {
            "file": "crates/ripr/src/cli/help.rs",
            "line": 43,
            "kind": "static_unknown",
            "expression": "\"Device label\";",
            "probe_kind": "static_unknown",
            "source_id": "help-label-literal",
            "evidence_record_ref": "help-label-literal"
          }
        ],
        "presentation_text": {
          "constant_name": "HELP_DEVICE_LABEL",
          "text_literal": "Device label",
          "visibility": "user_visible",
          "observer": "none",
          "actionability": "add_output_observer",
          "source_kind": "const_decl",
          "canonical_group_reason": "declaration_and_literal_same_text_constant",
          "recommended_observer": "cli_help_output",
          "repair_kind": "output_observer",
          "target_test_type": "help_output_snapshot",
          "suggested_assertion": "Assert CLI help output includes the HELP_DEVICE_LABEL text."
        }
      }
    ]
  }
}"#
    }

    #[test]
    fn outcome_parses_required_paths_format_and_out() {
        assert_eq!(
            parse_outcome_options(&args(&[
                "--before",
                "before.json",
                "--after",
                "after.json",
                "--format",
                "json",
                "--out",
                "target/ripr/outcome/targeted-test-outcome.json",
            ])),
            Ok(OutcomeOptions {
                before: PathBuf::from("before.json"),
                after: PathBuf::from("after.json"),
                format: OutcomeFormat::Json,
                out: Some(PathBuf::from(
                    "target/ripr/outcome/targeted-test-outcome.json"
                )),
            })
        );
    }

    #[test]
    fn evidence_health_parses_default_and_full_option_surface() {
        assert_eq!(
            parse_evidence_health_options(&args(&[])),
            Ok(EvidenceHealthOptions {
                root: PathBuf::from("."),
                out: PathBuf::from("target/ripr/reports/evidence-health.json"),
                out_md: PathBuf::from("target/ripr/reports/evidence-health.md"),
                mutation_calibration: None,
            })
        );
        assert_eq!(
            parse_evidence_health_options(&args(&[
                "--root",
                "repo",
                "--out",
                "health.json",
                "--out-md",
                "health.md",
                "--mutation-calibration",
                "target/ripr/reports/mutation-calibration.json",
            ])),
            Ok(EvidenceHealthOptions {
                root: PathBuf::from("repo"),
                out: PathBuf::from("health.json"),
                out_md: PathBuf::from("health.md"),
                mutation_calibration: Some(PathBuf::from(
                    "target/ripr/reports/mutation-calibration.json"
                )),
            })
        );
    }

    #[test]
    fn evidence_health_rejects_unknown_arguments() {
        assert_eq!(
            parse_evidence_health_options(&args(&["--bad"])),
            Err(
                "unknown evidence-health argument \"--bad\". Run `ripr evidence-health --help`."
                    .to_string()
            )
        );
    }

    #[test]
    fn pr_review_bare_dispatches_to_front_panel() {
        // Bare `ripr pr-review` is the `front-panel` alias (#2013).
        assert_eq!(
            pr_review(&args(&[])),
            Err("pr-review front-panel requires at least one explicit artifact input".to_string())
        );
        assert_eq!(
            pr_review(&args(&["bogus"])),
            Err("unknown pr-review subcommand \"bogus\"; expected `front-panel`".to_string())
        );
    }

    #[test]
    fn ripr_zero_status_parses_option_surface() {
        assert_eq!(
            parse_ripr_zero_status_options(&args(&[
                "--baseline",
                ".ripr/gate-baseline.json",
                "--delta",
                "target/ripr/reports/baseline-debt-delta.json",
                "--gap-ledger",
                "target/ripr/reports/gap-decision-ledger.json",
                "--gate",
                "target/ripr/reports/gate-decision.json",
                "--pr-guidance",
                "target/ripr/review/comments.json",
                "--recommendation-calibration",
                "target/ripr/reports/recommendation-calibration.json",
                "--out",
                "target/ripr/reports/ripr-zero-status.json",
                "--out-md",
                "target/ripr/reports/ripr-zero-status.md",
            ])),
            Ok(RiprZeroStatusOptions {
                baseline: Some(PathBuf::from(".ripr/gate-baseline.json")),
                delta: PathBuf::from("target/ripr/reports/baseline-debt-delta.json"),
                gap_ledger: Some(PathBuf::from(
                    "target/ripr/reports/gap-decision-ledger.json"
                )),
                gate: Some(PathBuf::from("target/ripr/reports/gate-decision.json")),
                pr_guidance: Some(PathBuf::from("target/ripr/review/comments.json")),
                recommendation_calibration: Some(PathBuf::from(
                    "target/ripr/reports/recommendation-calibration.json",
                )),
                out: PathBuf::from("target/ripr/reports/ripr-zero-status.json"),
                out_md: PathBuf::from("target/ripr/reports/ripr-zero-status.md"),
            })
        );
    }

    #[test]
    fn ripr_zero_status_requires_inputs_and_rejects_unknown_args() {
        // Bare `ripr zero` is the `status` alias (#2013).
        assert_eq!(
            zero(&args(&[])),
            Err("zero status requires --delta <path>".to_string())
        );
        assert_eq!(
            zero(&args(&["unknown"])),
            Err("unknown zero subcommand \"unknown\"; expected `status`".to_string())
        );
        assert_eq!(
            parse_ripr_zero_status_options(&args(&[])),
            Err("zero status requires --delta <path>".to_string())
        );
        assert_eq!(
            parse_ripr_zero_status_options(&args(&["--delta", ""])),
            Err("zero status --delta requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_ripr_zero_status_options(&args(&["--bad"])),
            Err(
                "unknown zero status argument \"--bad\". Run `ripr zero status --help`."
                    .to_string()
            )
        );
    }

    #[test]
    fn policy_readiness_parses_option_surface() {
        assert_eq!(
            parse_policy_readiness_options(&args(&[
                "--root",
                ".",
                "--gate-decision",
                "target/ripr/reports/gate-decision.json",
                "--baseline-delta",
                "target/ripr/reports/baseline-debt-delta.json",
                "--recommendation-calibration",
                "target/ripr/reports/recommendation-calibration.json",
                "--mutation-calibration",
                "target/ripr/reports/mutation-calibration.json",
                "--waiver-aging",
                "target/ripr/reports/waiver-aging.json",
                "--suppression-health",
                "target/ripr/reports/suppression-health.json",
                "--repo-config",
                "target/ripr/reports/repo-config.json",
                "--previous-readiness",
                "target/ripr/reports/previous-policy-readiness.json",
                "--out",
                "target/ripr/reports/policy-readiness.json",
                "--out-md",
                "target/ripr/reports/policy-readiness.md",
            ])),
            Ok(PolicyReadinessOptions {
                root: ".".to_string(),
                gate_decision: Some(PathBuf::from("target/ripr/reports/gate-decision.json")),
                baseline_delta: Some(PathBuf::from(
                    "target/ripr/reports/baseline-debt-delta.json"
                )),
                recommendation_calibration: Some(PathBuf::from(
                    "target/ripr/reports/recommendation-calibration.json"
                )),
                mutation_calibration: Some(PathBuf::from(
                    "target/ripr/reports/mutation-calibration.json"
                )),
                waiver_aging: Some(PathBuf::from("target/ripr/reports/waiver-aging.json")),
                suppression_health: Some(PathBuf::from(
                    "target/ripr/reports/suppression-health.json"
                )),
                repo_config: Some(PathBuf::from("target/ripr/reports/repo-config.json")),
                previous_readiness: Some(PathBuf::from(
                    "target/ripr/reports/previous-policy-readiness.json"
                )),
                out: PathBuf::from("target/ripr/reports/policy-readiness.json"),
                out_md: PathBuf::from("target/ripr/reports/policy-readiness.md"),
            })
        );
    }

    #[test]
    fn policy_operations_parses_option_surface() {
        assert_eq!(
            parse_policy_operations_options(&args(&[
                "--root",
                ".",
                "--policy-readiness",
                "target/ripr/reports/policy-readiness.json",
                "--waiver-aging",
                "target/ripr/reports/waiver-aging.json",
                "--suppression-health",
                "target/ripr/reports/suppression-health.json",
                "--baseline-delta",
                "target/ripr/reports/baseline-debt-delta.json",
                "--gate-decision",
                "target/ripr/reports/gate-decision.json",
                "--recommendation-calibration",
                "target/ripr/reports/recommendation-calibration.json",
                "--mutation-calibration",
                "target/ripr/reports/mutation-calibration.json",
                "--preview-boundary",
                "target/ripr/reports/preview-boundary.json",
                "--out",
                "target/ripr/reports/policy-operations.json",
                "--out-md",
                "target/ripr/reports/policy-operations.md",
            ])),
            Ok(PolicyOperationsOptions {
                root: ".".to_string(),
                policy_readiness: Some(PathBuf::from("target/ripr/reports/policy-readiness.json")),
                waiver_aging: Some(PathBuf::from("target/ripr/reports/waiver-aging.json")),
                suppression_health: Some(PathBuf::from(
                    "target/ripr/reports/suppression-health.json"
                )),
                baseline_delta: Some(PathBuf::from(
                    "target/ripr/reports/baseline-debt-delta.json"
                )),
                gate_decision: Some(PathBuf::from("target/ripr/reports/gate-decision.json")),
                recommendation_calibration: Some(PathBuf::from(
                    "target/ripr/reports/recommendation-calibration.json"
                )),
                mutation_calibration: Some(PathBuf::from(
                    "target/ripr/reports/mutation-calibration.json"
                )),
                preview_boundary: Some(PathBuf::from("target/ripr/reports/preview-boundary.json")),
                out: PathBuf::from("target/ripr/reports/policy-operations.json"),
                out_md: PathBuf::from("target/ripr/reports/policy-operations.md"),
            })
        );
    }

    #[test]
    fn policy_readiness_rejects_unknown_args() {
        assert_eq!(
            policy(&args(&[])),
            Err(
                "policy requires subcommand `readiness`, `operations`, `history`, `promote`, `preview-promote`, `waiver-aging`, or `suppression-health`"
                    .to_string()
            )
        );
        assert_eq!(
            policy(&args(&["unknown"])),
            Err(
                "unknown policy subcommand \"unknown\"; expected `readiness`, `operations`, `history`, `promote`, `preview-promote`, `waiver-aging`, or `suppression-health`"
                    .to_string()
            )
        );
        assert_eq!(
            parse_policy_readiness_options(&args(&["--gate-decision", ""])),
            Err("policy readiness --gate-decision requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_policy_readiness_options(&args(&["--bad"])),
            Err(
                "unknown policy readiness argument \"--bad\". Run `ripr policy readiness --help`."
                    .to_string()
            )
        );
        assert_eq!(
            parse_policy_operations_options(&args(&[])),
            Err("policy operations requires --policy-readiness <path>".to_string())
        );
        assert_eq!(
            parse_policy_operations_options(&args(&["--policy-readiness", ""])),
            Err("policy operations --policy-readiness requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_policy_operations_options(&args(&["--bad"])),
            Err("unknown policy operations argument \"--bad\". Run `ripr policy operations --help`.".to_string())
        );
        assert_eq!(
            parse_policy_promotion_options(&args(&[])),
            Err("policy promote requires --to <mode>".to_string())
        );
        assert_eq!(
            parse_policy_promotion_options(&args(&["--to", "strict"])),
            Err(
                "unknown policy promotion target \"strict\"; expected `visible-only`, `acknowledgeable`, `baseline-check`, or `calibrated-gate`"
                    .to_string()
            )
        );
        assert_eq!(
            parse_policy_promotion_options(&args(&["--to", "visible-only"])),
            Err("policy promote requires --operations <path>".to_string())
        );
        assert_eq!(
            parse_policy_promotion_options(&args(&["--to", "visible-only", "--operations", ""])),
            Err("policy promote --operations requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_policy_promotion_options(&args(&["--bad"])),
            Err(
                "unknown policy promote argument \"--bad\". Run `ripr policy promote --help`."
                    .to_string()
            )
        );
        assert_eq!(
            parse_policy_preview_promotion_options(&args(&[])),
            Err("policy preview-promote requires --language <language>".to_string())
        );
        assert_eq!(
            parse_policy_preview_promotion_options(&args(&["--language", "ruby"])),
            Err(
                "unknown preview promotion language \"ruby\"; expected `typescript` or `python`"
                    .to_string()
            )
        );
        assert_eq!(
            parse_policy_preview_promotion_options(&args(&["--language", "typescript"])),
            Err("policy preview-promote requires --class <class>".to_string())
        );
        assert_eq!(
            parse_policy_preview_promotion_options(&args(&[
                "--language",
                "typescript",
                "--class",
                "",
            ])),
            Err("policy preview-promote --class requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_policy_preview_promotion_options(&args(&["--bad"])),
            Err("unknown policy preview-promote argument \"--bad\". Run `ripr policy preview-promote --help`.".to_string())
        );
    }

    #[test]
    fn policy_history_parses_option_surface() {
        assert_eq!(
            parse_policy_history_options(&args(&[
                "--root",
                ".",
                "--current",
                "target/ripr/reports/policy-operations.json",
                "--history",
                ".ripr/policy-history.jsonl",
                "--commit",
                "HEAD",
                "--pr-number",
                "123",
                "--out",
                "target/ripr/reports/policy-history.json",
                "--out-md",
                "target/ripr/reports/policy-history.md",
            ])),
            Ok(PolicyHistoryOptions {
                root: ".".to_string(),
                current: PathBuf::from("target/ripr/reports/policy-operations.json"),
                history: Some(PathBuf::from(".ripr/policy-history.jsonl")),
                commit: Some("HEAD".to_string()),
                pr_number: Some("123".to_string()),
                out: PathBuf::from("target/ripr/reports/policy-history.json"),
                out_md: PathBuf::from("target/ripr/reports/policy-history.md"),
                out_jsonl: None,
            })
        );
        assert_eq!(
            parse_policy_history_options(&args(&[])),
            Err("policy history requires --current <path>".to_string())
        );
        assert_eq!(
            parse_policy_history_options(&args(&["--current", ""])),
            Err("policy history --current requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_policy_history_options(&args(&["--current", "ops.json", "--bad"])),
            Err(
                "unknown policy history argument \"--bad\". Run `ripr policy history --help`."
                    .to_string()
            )
        );
        assert_eq!(
            parse_policy_history_options(&args(&[
                "--current",
                "ops.json",
                "--out-jsonl",
                ".ripr/policy-history.jsonl",
            ])),
            Ok(PolicyHistoryOptions {
                root: ".".to_string(),
                current: PathBuf::from("ops.json"),
                history: None,
                commit: None,
                pr_number: None,
                out: PathBuf::from(output::policy_history::DEFAULT_POLICY_HISTORY_OUT),
                out_md: PathBuf::from(output::policy_history::DEFAULT_POLICY_HISTORY_MD_OUT),
                out_jsonl: Some(PathBuf::from(".ripr/policy-history.jsonl")),
            })
        );
        assert_eq!(
            parse_policy_history_options(&args(&[
                "--current",
                "ops.json",
                "--out",
                "policy-history.json",
                "--out-jsonl",
                "policy-history.json",
            ])),
            Err(
                "policy history --out-jsonl must not be the same path as --out, --out-md, or --current"
                    .to_string()
            )
        );
        assert_eq!(
            parse_policy_history_options(&args(&[
                "--current",
                "ops.json",
                "--out",
                "./policy-history.json",
                "--out-jsonl",
                "policy-history.json",
            ])),
            Err(
                "policy history --out-jsonl must not be the same path as --out, --out-md, or --current"
                    .to_string()
            )
        );
        assert_eq!(
            parse_policy_history_options(&args(&[
                "--current",
                "ops.json",
                "--out-jsonl",
                "ops.json",
            ])),
            Err(
                "policy history --out-jsonl must not be the same path as --out, --out-md, or --current"
                    .to_string()
            )
        );
        assert_eq!(
            parse_policy_history_options(&args(&[
                "--current",
                "ops.json",
                "--history",
                ".ripr/policy-history.jsonl",
                "--out-jsonl",
                ".ripr/policy-history.jsonl",
            ]))
            .map(|options| options.out_jsonl),
            Ok(Some(PathBuf::from(".ripr/policy-history.jsonl")))
        );
    }

    #[test]
    fn policy_promotion_parses_option_surface() {
        assert_eq!(
            parse_policy_promotion_options(&args(&[
                "--root",
                ".",
                "--to",
                "baseline-check",
                "--operations",
                "target/ripr/reports/policy-operations.json",
                "--history",
                "target/ripr/reports/policy-history.json",
                "--out",
                "target/ripr/reports/policy-promotion-baseline-check.json",
                "--out-md",
                "target/ripr/reports/policy-promotion-baseline-check.md",
            ])),
            Ok(PolicyPromotionOptions {
                root: ".".to_string(),
                target_mode: "baseline-check".to_string(),
                operations: PathBuf::from("target/ripr/reports/policy-operations.json"),
                history: Some(PathBuf::from("target/ripr/reports/policy-history.json")),
                out: PathBuf::from("target/ripr/reports/policy-promotion-baseline-check.json"),
                out_md: PathBuf::from("target/ripr/reports/policy-promotion-baseline-check.md"),
            })
        );
        assert_eq!(
            parse_policy_promotion_options(&args(&[
                "--to",
                "visible-only",
                "--operations",
                "target/ripr/reports/policy-operations.json",
            ])),
            Ok(PolicyPromotionOptions {
                root: ".".to_string(),
                target_mode: "visible-only".to_string(),
                operations: PathBuf::from("target/ripr/reports/policy-operations.json"),
                history: None,
                out: PathBuf::from("target/ripr/reports/policy-promotion-visible-only.json"),
                out_md: PathBuf::from("target/ripr/reports/policy-promotion-visible-only.md"),
            })
        );
    }

    #[test]
    fn policy_preview_promotion_parses_option_surface() {
        assert_eq!(
            parse_policy_preview_promotion_options(&args(&[
                "--root",
                ".",
                "--language",
                "typescript",
                "--class",
                "boundary_gap",
                "--evidence",
                "target/ripr/reports/preview-promotion-evidence.json",
                "--out",
                "target/ripr/reports/preview-promotion-typescript-boundary-gap.json",
                "--out-md",
                "target/ripr/reports/preview-promotion-typescript-boundary-gap.md",
            ])),
            Ok(PolicyPreviewPromotionOptions {
                root: ".".to_string(),
                language: "typescript".to_string(),
                candidate_class: "boundary_gap".to_string(),
                evidence: Some(PathBuf::from(
                    "target/ripr/reports/preview-promotion-evidence.json"
                )),
                out: PathBuf::from(
                    "target/ripr/reports/preview-promotion-typescript-boundary-gap.json"
                ),
                out_md: PathBuf::from(
                    "target/ripr/reports/preview-promotion-typescript-boundary-gap.md"
                ),
            })
        );
        assert_eq!(
            parse_policy_preview_promotion_options(&args(&[
                "--language",
                "python",
                "--class",
                "boundary_gap",
            ])),
            Ok(PolicyPreviewPromotionOptions {
                root: ".".to_string(),
                language: "python".to_string(),
                candidate_class: "boundary_gap".to_string(),
                evidence: None,
                out: PathBuf::from(
                    "target/ripr/reports/preview-promotion-python-boundary-gap.json"
                ),
                out_md: PathBuf::from(
                    "target/ripr/reports/preview-promotion-python-boundary-gap.md"
                ),
            })
        );
    }

    #[test]
    fn policy_waiver_aging_parses_option_surface() {
        assert_eq!(
            parse_policy_waiver_aging_options(&args(&[
                "--root",
                ".",
                "--ledger",
                "target/ripr/reports/pr-evidence-ledger.json",
                "--history",
                ".ripr/pr-evidence-ledger.jsonl",
                "--out",
                "target/ripr/reports/waiver-aging.json",
                "--out-md",
                "target/ripr/reports/waiver-aging.md",
            ])),
            Ok(PolicyWaiverAgingOptions {
                root: ".".to_string(),
                ledger: Some(PathBuf::from("target/ripr/reports/pr-evidence-ledger.json")),
                history: Some(PathBuf::from(".ripr/pr-evidence-ledger.jsonl")),
                out: PathBuf::from("target/ripr/reports/waiver-aging.json"),
                out_md: PathBuf::from("target/ripr/reports/waiver-aging.md"),
            })
        );
    }

    #[test]
    fn policy_waiver_aging_rejects_unknown_args() {
        assert_eq!(
            parse_policy_waiver_aging_options(&args(&["--ledger", ""])),
            Err("policy waiver-aging --ledger requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_policy_waiver_aging_options(&args(&["--bad"])),
            Err("unknown policy waiver-aging argument \"--bad\". Run `ripr policy waiver-aging --help`.".to_string())
        );
    }

    #[test]
    fn policy_suppression_health_parses_option_surface() {
        assert_eq!(
            parse_policy_suppression_health_options(&args(&[
                "--root",
                ".",
                "--manifest",
                ".ripr/suppressions.toml",
                "--out",
                "target/ripr/reports/suppression-health.json",
                "--out-md",
                "target/ripr/reports/suppression-health.md",
            ])),
            Ok(PolicySuppressionHealthOptions {
                root: PathBuf::from("."),
                manifest: PathBuf::from(".ripr/suppressions.toml"),
                out: PathBuf::from("target/ripr/reports/suppression-health.json"),
                out_md: PathBuf::from("target/ripr/reports/suppression-health.md"),
            })
        );
    }

    #[test]
    fn policy_suppression_health_rejects_unknown_args() {
        assert_eq!(
            parse_policy_suppression_health_options(&args(&["--manifest", ""])),
            Err("policy suppression-health --manifest requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_policy_suppression_health_options(&args(&["--bad"])),
            Err("unknown policy suppression-health argument \"--bad\". Run `ripr policy suppression-health --help`.".to_string())
        );
    }

    #[test]
    fn policy_readiness_command_writes_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("policy-readiness");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create policy dir: {err}"))?;
        let gate = dir.join("gate-decision.json");
        let baseline = dir.join("baseline-debt-delta.json");
        let out = dir.join("policy-readiness.json");
        let out_md = dir.join("policy-readiness.md");
        std::fs::write(
            &gate,
            r#"{
              "schema_version": "0.1",
              "status": "advisory",
              "mode": "visible-only",
              "summary": {"blocking": 0, "acknowledged": 0, "advisory": 1, "suppressed": 0, "not_applicable": 0},
              "decisions": [{
                "decision": "advisory",
                "language": "typescript",
                "language_status": "preview",
                "static_limit_kind": "dynamic_dispatch"
              }]
            }"#,
        )
        .map_err(|err| format!("write gate: {err}"))?;
        std::fs::write(
            &baseline,
            r#"{
              "schema_version": "0.1",
              "kind": "baseline_debt_delta",
              "delta": {"still_present": 1, "resolved": 0, "new_policy_eligible": 0, "acknowledged": 0, "suppressed": 0, "stale_baseline_entry": 0, "invalid_baseline_entry": 0, "missing_current_input": 0}
            }"#,
        )
        .map_err(|err| format!("write baseline: {err}"))?;

        policy(&args(&[
            "readiness",
            "--gate-decision",
            &gate.display().to_string(),
            "--baseline-delta",
            &baseline.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text =
            std::fs::read_to_string(&out).map_err(|err| format!("read policy json: {err}"))?;
        let md_text =
            std::fs::read_to_string(&out_md).map_err(|err| format!("read policy md: {err}"))?;
        assert!(json_text.contains("\"status\": \"ready_for_baseline_check\""));
        assert!(json_text.contains("\"recommended_mode\": \"baseline-check\""));
        assert!(json_text.contains("\"preview_findings_gate_eligible\": 0"));
        assert!(json_text.contains("\"preview_findings_ripr_zero_blocking\": 0"));
        assert!(md_text.contains("# RIPR Policy Readiness"));
        assert!(md_text.contains("Recommended mode: baseline-check"));
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove policy dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn policy_operations_command_writes_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("policy-operations");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create operations dir: {err}"))?;
        let readiness = dir.join("policy-readiness.json");
        let waiver = dir.join("waiver-aging.json");
        let suppression = dir.join("suppression-health.json");
        let baseline = dir.join("baseline-debt-delta.json");
        let gate = dir.join("gate-decision.json");
        let out = dir.join("policy-operations.json");
        let out_md = dir.join("policy-operations.md");
        std::fs::write(
            &readiness,
            r#"{
              "schema_version": "0.1",
              "kind": "policy_readiness",
              "status": "ready_for_acknowledgeable",
              "next_policy_action": "Review baseline blockers before baseline-check.",
              "preview_evidence_boundary": {
                "state": "healthy",
                "preview_languages": ["typescript"],
                "preview_findings_visible": 1,
                "preview_findings_gate_eligible": 0,
                "preview_findings_ripr_zero_blocking": 0,
                "preview_findings_calibrated_confidence": 0,
                "missing_language_status": 0,
                "static_limits_seen": 1
              }
            }"#,
        )
        .map_err(|err| format!("write readiness: {err}"))?;
        std::fs::write(
            &waiver,
            r#"{"schema_version":"0.1","kind":"waiver_aging","status":"advisory","summary":{"waiver_count":1}}"#,
        )
        .map_err(|err| format!("write waiver: {err}"))?;
        std::fs::write(
            &suppression,
            r#"{"schema_version":"0.1","kind":"suppression_health","status":"healthy","summary":{"warnings":0,"config_errors":0}}"#,
        )
        .map_err(|err| format!("write suppression: {err}"))?;
        std::fs::write(
            &baseline,
            r#"{
              "schema_version": "0.1",
              "kind": "baseline_debt_delta",
              "delta": {"still_present": 1, "resolved": 0, "new_policy_eligible": 0, "acknowledged": 0, "suppressed": 0, "stale_baseline_entry": 1, "invalid_baseline_entry": 0, "missing_current_input": 0}
            }"#,
        )
        .map_err(|err| format!("write baseline: {err}"))?;
        std::fs::write(
            &gate,
            r#"{"schema_version":"0.1","kind":"gate_decision","status":"advisory","mode":"visible-only"}"#,
        )
        .map_err(|err| format!("write gate: {err}"))?;

        policy(&args(&[
            "operations",
            "--policy-readiness",
            &readiness.display().to_string(),
            "--waiver-aging",
            &waiver.display().to_string(),
            "--suppression-health",
            &suppression.display().to_string(),
            "--baseline-delta",
            &baseline.display().to_string(),
            "--gate-decision",
            &gate.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text =
            std::fs::read_to_string(&out).map_err(|err| format!("read operations json: {err}"))?;
        let md_text =
            std::fs::read_to_string(&out_md).map_err(|err| format!("read operations md: {err}"))?;
        assert!(json_text.contains("\"kind\": \"policy_operations\""));
        assert!(json_text.contains("\"current_policy_ceiling\": \"ready_for_acknowledgeable\""));
        assert!(json_text.contains("\"mode\": \"acknowledgeable\""));
        assert!(json_text.contains("\"mode\": \"baseline-check\""));
        assert!(json_text.contains("\"baseline_stale_entries\""));
        assert!(md_text.contains("# RIPR Policy Operations"));
        assert!(md_text.contains("Current ceiling: ready_for_acknowledgeable"));
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove operations dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn policy_history_command_writes_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("policy-history");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create history dir: {err}"))?;
        let current = dir.join("policy-operations.json");
        let history = dir.join("policy-history.jsonl");
        let out = dir.join("policy-history.json");
        let out_md = dir.join("policy-history.md");
        std::fs::write(
            &current,
            r#"{
              "schema_version": "0.1",
              "kind": "policy_operations",
              "generated_at": "unix_ms:10",
              "current_policy_ceiling": "ready_for_acknowledgeable",
              "safe_to_promote_to": [
                {"mode": "visible-only", "allowed_now": true, "reason": "ok", "source_artifacts": []},
                {"mode": "acknowledgeable", "allowed_now": true, "reason": "ok", "source_artifacts": []}
              ],
              "not_safe_to_promote_to": [],
              "promotion_blockers": [],
              "input_artifacts": [
                {"kind":"baseline_delta","path":"baseline.json","status":"read"},
                {"kind":"waiver_aging","path":"waiver.json","status":"read"},
                {"kind":"suppression_health","path":"suppression.json","status":"read"},
                {"kind":"recommendation_calibration","path":"recommendation.json","status":"omitted"}
              ],
              "current": {
                "new_policy_eligible_count": 1,
                "waiver_count": 2,
                "stale_suppression_count": 0,
                "baseline_still_present": 4,
                "baseline_resolved": 1
              }
            }"#,
        )
        .map_err(|err| format!("write current: {err}"))?;
        let history_text = r#"{"generated_at":"unix_ms:1","current_policy_ceiling":"ready_for_visible_only","recommended_mode":"visible-only","baseline_health":"healthy","waiver_health":"healthy","suppression_health":"healthy","calibration_health":"not_ready","preview_boundary_state":"healthy","new_policy_eligible_count":1,"waiver_count":2,"stale_suppression_count":0,"baseline_still_present":5,"baseline_resolved":0}
"#;
        std::fs::write(&history, history_text).map_err(|err| format!("write history: {err}"))?;

        policy(&args(&[
            "history",
            "--current",
            &current.display().to_string(),
            "--history",
            &history.display().to_string(),
            "--commit",
            "HEAD",
            "--pr-number",
            "123",
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text =
            std::fs::read_to_string(&out).map_err(|err| format!("read history json: {err}"))?;
        let md_text =
            std::fs::read_to_string(&out_md).map_err(|err| format!("read history md: {err}"))?;
        assert!(json_text.contains("\"kind\": \"policy_history\""));
        assert!(json_text.contains("\"readiness_improved\": true"));
        assert!(json_text.contains("\"example_append_record\""));
        assert!(md_text.contains("# RIPR Policy History"));
        assert!(md_text.contains("Readiness: improved"));
        assert_eq!(
            std::fs::read_to_string(&history).map_err(|err| format!("read history: {err}"))?,
            history_text
        );
        assert!(
            !dir.join("produced.jsonl").exists(),
            "default policy history must not write JSONL without --out-jsonl"
        );
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove history dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn policy_history_out_jsonl_is_opt_in_and_feeds_non_null_trend() -> Result<(), String> {
        let dir = unique_command_test_dir("policy-history-jsonl");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create history dir: {err}"))?;
        let current = dir.join("policy-operations.json");
        let jsonl = dir.join("policy-history.jsonl");
        let first_out = dir.join("first.json");
        let first_md = dir.join("first.md");
        let second_out = dir.join("second.json");
        let second_md = dir.join("second.md");
        std::fs::write(
            &current,
            r#"{
              "schema_version": "0.1",
              "kind": "policy_operations",
              "generated_at": "unix_ms:10",
              "current_policy_ceiling": "ready_for_acknowledgeable",
              "safe_to_promote_to": [
                {"mode": "visible-only", "allowed_now": true, "reason": "ok", "source_artifacts": []},
                {"mode": "acknowledgeable", "allowed_now": true, "reason": "ok", "source_artifacts": []}
              ],
              "not_safe_to_promote_to": [],
              "promotion_blockers": [],
              "input_artifacts": [
                {"kind":"baseline_delta","path":"baseline.json","status":"read"},
                {"kind":"waiver_aging","path":"waiver.json","status":"read"},
                {"kind":"suppression_health","path":"suppression.json","status":"read"},
                {"kind":"recommendation_calibration","path":"recommendation.json","status":"omitted"}
              ],
              "current": {
                "new_policy_eligible_count": 1,
                "waiver_count": 2,
                "stale_suppression_count": 0,
                "baseline_still_present": 4,
                "baseline_resolved": 1
              }
            }"#,
        )
        .map_err(|err| format!("write current: {err}"))?;

        policy(&args(&[
            "history",
            "--current",
            &current.display().to_string(),
            "--commit",
            "HEAD",
            "--pr-number",
            "1",
            "--out",
            &first_out.display().to_string(),
            "--out-md",
            &first_md.display().to_string(),
            "--out-jsonl",
            &jsonl.display().to_string(),
        ]))?;

        let first_json =
            std::fs::read_to_string(&first_out).map_err(|err| format!("read first json: {err}"))?;
        assert!(
            first_json.contains("\"history_not_supplied\"")
                || first_json.contains("\"direction\": \"unknown\""),
            "first snapshot without history stays unknown: {first_json}"
        );
        let produced =
            std::fs::read_to_string(&jsonl).map_err(|err| format!("read produced jsonl: {err}"))?;
        let first_line = produced
            .lines()
            .next()
            .ok_or_else(|| "produced jsonl must have one line".to_string())?;
        assert!(
            !first_line.contains("\"kind\""),
            "policy history jsonl is a snapshot, not the full report: {first_line}"
        );

        policy(&args(&[
            "history",
            "--current",
            &current.display().to_string(),
            "--history",
            &jsonl.display().to_string(),
            "--commit",
            "HEAD",
            "--pr-number",
            "2",
            "--out",
            &second_out.display().to_string(),
            "--out-md",
            &second_md.display().to_string(),
            "--out-jsonl",
            &jsonl.display().to_string(),
        ]))?;

        let second_json = std::fs::read_to_string(&second_out)
            .map_err(|err| format!("read second json: {err}"))?;
        assert!(
            second_json.contains("\"entries\": 2"),
            "history from --out-jsonl must populate entries: {second_json}"
        );
        assert!(
            second_json.contains("\"direction\": \"unchanged\""),
            "supplied history must populate trend, not unknown: {second_json}"
        );
        let appended =
            std::fs::read_to_string(&jsonl).map_err(|err| format!("read appended jsonl: {err}"))?;
        let lines: Vec<&str> = appended.lines().filter(|line| !line.is_empty()).collect();
        assert_eq!(lines.len(), 2, "second --out-jsonl must append: {appended}");
        assert_eq!(
            lines[0], first_line,
            "append must preserve the prior record"
        );
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove history dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn policy_history_out_jsonl_refuses_unavailable_current() -> Result<(), String> {
        let dir = unique_command_test_dir("policy-history-jsonl-refuse");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create history dir: {err}"))?;
        let current = dir.join("missing-policy-operations.json");
        let jsonl = dir.join("policy-history.jsonl");
        let out = dir.join("policy-history.json");
        let out_md = dir.join("policy-history.md");
        let error = match policy(&args(&[
            "history",
            "--current",
            &current.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
            "--out-jsonl",
            &jsonl.display().to_string(),
        ])) {
            Err(error) => error,
            Ok(()) => return Err("unavailable current must refuse --out-jsonl".to_string()),
        };
        assert!(
            error.contains("refuses to append"),
            "expected refuse error, got {error}"
        );
        assert!(
            !jsonl.exists(),
            "unavailable current must not produce a durable JSONL line"
        );
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove history dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn policy_promotion_command_writes_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("policy-promotion");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create promotion dir: {err}"))?;
        let operations = dir.join("policy-operations.json");
        let history = dir.join("policy-history.json");
        let out = dir.join("policy-promotion-baseline-check.json");
        let out_md = dir.join("policy-promotion-baseline-check.md");
        std::fs::write(
            &operations,
            r#"{
              "schema_version": "0.1",
              "kind": "policy_operations",
              "current_policy_ceiling": "ready_for_acknowledgeable",
              "safe_to_promote_to": [
                {"mode": "visible-only", "allowed_now": true, "reason": "visible ok", "source_artifacts": []},
                {"mode": "acknowledgeable", "allowed_now": true, "reason": "ack ok", "source_artifacts": []}
              ],
              "not_safe_to_promote_to": [
                {"mode": "baseline-check", "allowed_now": false, "reason": "baseline-check is blocked", "blockers": ["Review stale baseline entries."]}
              ],
              "promotion_blockers": [
                {
                  "kind": "baseline_stale_entries",
                  "severity": "warning",
                  "message": "Baseline contains stale entries.",
                  "target_modes": ["baseline-check"],
                  "source_artifact": "baseline-debt-delta.json",
                  "repair_action": "Run shrink-only baseline review."
                }
              ],
              "baseline_actions": ["Run shrink-only baseline review."],
              "waiver_actions": [],
              "suppression_actions": [],
              "calibration_actions": [],
              "preview_boundary_actions": ["Keep preview evidence advisory."],
              "warnings": [],
              "unknowns": [],
              "input_artifacts": []
            }"#,
        )
        .map_err(|err| format!("write operations: {err}"))?;
        std::fs::write(
            &history,
            r#"{"schema_version":"0.1","kind":"policy_history","history_summary":{"entries":1}}"#,
        )
        .map_err(|err| format!("write history: {err}"))?;

        policy(&args(&[
            "promote",
            "--to",
            "baseline-check",
            "--operations",
            &operations.display().to_string(),
            "--history",
            &history.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text =
            std::fs::read_to_string(&out).map_err(|err| format!("read promotion json: {err}"))?;
        let md_text =
            std::fs::read_to_string(&out_md).map_err(|err| format!("read promotion md: {err}"))?;
        assert!(json_text.contains("\"kind\": \"policy_promotion_packet\""));
        assert!(json_text.contains("\"target_mode\": \"baseline-check\""));
        assert!(json_text.contains("\"allowed_now\": false"));
        assert!(json_text.contains("Run shrink-only baseline review."));
        assert!(md_text.contains("# RIPR Policy Promotion Packet"));
        assert!(md_text.contains("Allowed now: no"));
        assert_eq!(
            std::fs::read_to_string(&history).map_err(|err| format!("read history: {err}"))?,
            r#"{"schema_version":"0.1","kind":"policy_history","history_summary":{"entries":1}}"#
        );
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove promotion dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn policy_preview_promotion_command_writes_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("policy-preview-promotion");
        std::fs::create_dir_all(&dir)
            .map_err(|err| format!("create preview promotion dir: {err}"))?;
        let evidence = dir.join("preview-promotion-evidence.json");
        let out = dir.join("preview-promotion-typescript-boundary-gap.json");
        let out_md = dir.join("preview-promotion-typescript-boundary-gap.md");
        let evidence_text = r#"{
          "language": "typescript",
          "language_status": "preview",
          "candidate_class": "boundary_gap",
          "supplied_evidence": ["fixture_corpus_coverage"],
          "static_limit_exclusions": true
        }"#;
        std::fs::write(&evidence, evidence_text)
            .map_err(|err| format!("write preview evidence: {err}"))?;

        policy(&args(&[
            "preview-promote",
            "--language",
            "typescript",
            "--class",
            "boundary_gap",
            "--evidence",
            &evidence.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text = std::fs::read_to_string(&out)
            .map_err(|err| format!("read preview promotion json: {err}"))?;
        let md_text = std::fs::read_to_string(&out_md)
            .map_err(|err| format!("read preview promotion md: {err}"))?;
        assert!(json_text.contains("\"kind\": \"preview_evidence_promotion_packet\""));
        assert!(json_text.contains("\"language_status\": \"preview\""));
        assert!(json_text.contains("\"allowed_now\": false"));
        assert!(json_text.contains("\"fixture_corpus_coverage\""));
        assert!(json_text.contains("\"recommendation_calibration\""));
        assert!(json_text.contains("\"may_fail_check\": false"));
        assert!(md_text.contains("# RIPR Preview Evidence Promotion Packet"));
        assert!(md_text.contains("Allowed now: no"));
        assert!(md_text.contains("may fail check: no"));
        assert_eq!(
            std::fs::read_to_string(&evidence)
                .map_err(|err| format!("read preview evidence: {err}"))?,
            evidence_text
        );
        std::fs::remove_dir_all(&dir)
            .map_err(|err| format!("remove preview promotion dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn policy_waiver_aging_command_writes_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("waiver-aging");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create waiver dir: {err}"))?;
        let ledger = dir.join("pr-evidence-ledger.json");
        let history = dir.join("pr-evidence-ledger.jsonl");
        let out = dir.join("waiver-aging.json");
        let out_md = dir.join("waiver-aging.md");
        let ledger_text = r#"{
          "schema_version": "0.1",
          "kind": "pr_evidence_ledger",
          "pr": {"number": "10"},
          "top_repair_route": {"seam_id": "seam-a", "path": "src/lib.rs"},
          "waivers": [{
            "label": "ripr-waive",
            "canonical_gap_id": "gap-a",
            "seam_id": "seam-a",
            "age_prs": 1,
            "age_days": 7,
            "reason": "accepted for this PR",
            "still_visible": true
          }]
        }"#;
        std::fs::write(&ledger, ledger_text).map_err(|err| format!("write ledger: {err}"))?;
        let history_text = serde_json::to_string(
            &serde_json::from_str::<serde_json::Value>(ledger_text)
                .map_err(|err| format!("parse ledger fixture: {err}"))?,
        )
        .map_err(|err| format!("compact history fixture: {err}"))?;
        std::fs::write(&history, format!("{history_text}\n"))
            .map_err(|err| format!("write history: {err}"))?;

        policy(&args(&[
            "waiver-aging",
            "--ledger",
            &ledger.display().to_string(),
            "--history",
            &history.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text =
            std::fs::read_to_string(&out).map_err(|err| format!("read waiver json: {err}"))?;
        let md_text =
            std::fs::read_to_string(&out_md).map_err(|err| format!("read waiver md: {err}"))?;
        assert!(json_text.contains("\"kind\": \"waiver_aging\""));
        assert!(json_text.contains("\"status\": \"advisory\""));
        assert!(json_text.contains("\"candidate_for_focused_test\": true"));
        assert!(json_text.contains("\"warnings\": []"));
        assert!(md_text.contains("# RIPR Waiver Aging"));
        assert!(md_text.contains("Repeated waiver is not a failure."));
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove waiver dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn policy_suppression_health_command_writes_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("suppression-health");
        let ripr_dir = dir.join(".ripr");
        std::fs::create_dir_all(&ripr_dir)
            .map_err(|err| format!("create suppression dir: {err}"))?;
        let manifest = ripr_dir.join("suppressions.toml");
        let out = dir.join("suppression-health.json");
        let out_md = dir.join("suppression-health.md");
        std::fs::write(
            &manifest,
            r#"schema_version = 1

[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:src/pricing.rs:88:predicate"
owner = "billing"
reason = "accepted durable policy exception"
scope = "seam:pricing::threshold"
created_at = "2026-01-01"
last_seen = "2026-05-01"
review_by = "2026-12-01"
expected_visibility = "suppressed_visible"
static_class = "weakly_exposed"
language = "rust"
"#,
        )
        .map_err(|err| format!("write suppression manifest: {err}"))?;

        policy(&args(&[
            "suppression-health",
            "--root",
            &dir.display().to_string(),
            "--manifest",
            ".ripr/suppressions.toml",
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text = std::fs::read_to_string(&out)
            .map_err(|err| format!("read suppression health json: {err}"))?;
        let md_text = std::fs::read_to_string(&out_md)
            .map_err(|err| format!("read suppression health md: {err}"))?;
        assert!(json_text.contains("\"kind\": \"suppression_health\""));
        assert!(json_text.contains("\"status\": \"healthy\""));
        assert!(json_text.contains("\"still_visible\": true"));
        assert!(md_text.contains("# RIPR Suppression Health"));
        assert!(md_text.contains("Suppressed findings remain visible"));
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove suppression dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn policy_suppression_health_command_treats_missing_manifest_as_no_suppressions()
    -> Result<(), String> {
        let dir = unique_command_test_dir("suppression-health-missing");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create suppression dir: {err}"))?;
        let out = dir.join("suppression-health.json");
        let out_md = dir.join("suppression-health.md");

        policy(&args(&[
            "suppression-health",
            "--root",
            &dir.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text = std::fs::read_to_string(&out)
            .map_err(|err| format!("read suppression health json: {err}"))?;
        assert!(json_text.contains("\"status\": \"no_suppressions\""));
        assert!(json_text.contains("\"suppressions\": 0"));
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove suppression dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn policy_suppression_health_rejects_missing_root_without_writing() -> Result<(), String> {
        let dir = unique_command_test_dir("suppression-health-no-root");
        let out = dir.join("suppression-health.json");
        let out_md = dir.join("suppression-health.md");
        let missing = dir.join("does-not-exist");

        let err = policy(&args(&[
            "suppression-health",
            "--root",
            &missing.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))
        .err()
        .ok_or_else(|| "missing --root must fail".to_string())?;

        assert!(err.contains("cannot be read"), "{err}");
        assert!(err.contains("does-not-exist"), "{err}");
        assert!(
            !out.exists() && !out_md.exists(),
            "no report may be written"
        );
        Ok(())
    }

    #[test]
    fn policy_suppression_health_rejects_file_root_without_writing() -> Result<(), String> {
        let dir = unique_command_test_dir("suppression-health-file-root");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create suppression dir: {err}"))?;
        let root_file = dir.join("root-file");
        std::fs::write(&root_file, "not a directory")
            .map_err(|err| format!("write root file: {err}"))?;
        let out = dir.join("suppression-health.json");
        let out_md = dir.join("suppression-health.md");

        let result = policy(&args(&[
            "suppression-health",
            "--root",
            &root_file.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]));
        let reports_written = out.exists() || out_md.exists();
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove suppression dir: {err}"))?;

        let err = result
            .err()
            .ok_or_else(|| "file --root must fail".to_string())?;
        assert!(err.contains("is not a directory"), "{err}");
        assert!(err.contains("root-file"), "{err}");
        assert!(!reports_written, "no report may be written");
        Ok(())
    }

    #[test]
    fn pr_evidence_ledger_parses_option_surface() {
        assert_eq!(
            parse_pr_evidence_ledger_options(&args(&[
                "--pr-number",
                "123",
                "--base",
                "base",
                "--head",
                "head",
                "--label",
                "ripr-waive",
                "--gate",
                "target/ripr/reports/gate-decision.json",
                "--baseline-delta",
                "target/ripr/reports/baseline-debt-delta.json",
                "--zero-status",
                "target/ripr/reports/ripr-zero-status.json",
                "--pr-guidance",
                "target/ripr/review/comments.json",
                "--gap-ledger",
                "target/ripr/reports/gap-decision-ledger.json",
                "--recommendation-calibration",
                "target/ripr/reports/recommendation-calibration.json",
                "--agent-receipt",
                "target/ripr/reports/agent-receipt.json",
                "--coverage",
                "target/ripr/reports/coverage-summary.json",
                "--history",
                ".ripr/pr-evidence-ledger.jsonl",
                "--out",
                "target/ripr/reports/pr-evidence-ledger.json",
                "--out-md",
                "target/ripr/reports/pr-evidence-ledger.md",
            ])),
            Ok(PrEvidenceLedgerOptions {
                pr_number: "123".to_string(),
                base: "base".to_string(),
                head: "head".to_string(),
                labels: vec!["ripr-waive".to_string()],
                gate: Some(PathBuf::from("target/ripr/reports/gate-decision.json")),
                baseline_delta: Some(PathBuf::from(
                    "target/ripr/reports/baseline-debt-delta.json"
                )),
                zero_status: Some(PathBuf::from("target/ripr/reports/ripr-zero-status.json")),
                pr_guidance: Some(PathBuf::from("target/ripr/review/comments.json")),
                gap_ledger: Some(PathBuf::from(
                    "target/ripr/reports/gap-decision-ledger.json"
                )),
                recommendation_calibration: Some(PathBuf::from(
                    "target/ripr/reports/recommendation-calibration.json"
                )),
                agent_receipt: Some(PathBuf::from("target/ripr/reports/agent-receipt.json")),
                coverage: Some(PathBuf::from("target/ripr/reports/coverage-summary.json")),
                history: Some(PathBuf::from(".ripr/pr-evidence-ledger.jsonl")),
                out: PathBuf::from("target/ripr/reports/pr-evidence-ledger.json"),
                out_md: PathBuf::from("target/ripr/reports/pr-evidence-ledger.md"),
                out_jsonl: None,
            })
        );
    }

    #[test]
    fn pr_evidence_ledger_requires_identity_and_evidence() {
        // Bare `ripr pr-ledger` is the `record` alias (#2013).
        assert_eq!(
            pr_ledger(&args(&[])),
            Err(
                "pr-ledger record requires at least one of --gate, --baseline-delta, --zero-status, --pr-guidance, or --gap-ledger"
                    .to_string()
            )
        );
        assert_eq!(
            pr_ledger(&args(&["unknown"])),
            Err("unknown pr-ledger subcommand \"unknown\"; expected `record`".to_string())
        );
        assert_eq!(
            parse_pr_evidence_ledger_options(&args(&[
                "--pr-number",
                "123",
                "--base",
                "base",
                "--head",
                "head"
            ])),
            Err(
                "pr-ledger record requires at least one of --gate, --baseline-delta, --zero-status, --pr-guidance, or --gap-ledger"
                    .to_string()
            )
        );
        assert_eq!(
            parse_pr_evidence_ledger_options(&args(&[
                "--pr-number",
                "123",
                "--base",
                "base",
                "--head",
                "head",
                "--gap-ledger",
                "gap-ledger.json",
            ]))
            .map(|options| options.gap_ledger),
            Ok(Some(PathBuf::from("gap-ledger.json")))
        );
        assert_eq!(
            parse_pr_evidence_ledger_options(&args(&[
                "--base",
                "base",
                "--head",
                "head",
                "--gate",
                "gate.json"
            ])),
            Err("pr-ledger record requires --pr-number <value>".to_string())
        );
        assert_eq!(
            parse_pr_evidence_ledger_options(&args(&[
                "--pr-number",
                "",
                "--base",
                "base",
                "--head",
                "head",
                "--gate",
                "gate.json"
            ])),
            Err("pr-ledger record --pr-number requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_pr_evidence_ledger_options(&args(&["--bad"])),
            Err(
                "unknown pr-ledger record argument \"--bad\". Run `ripr pr-ledger record --help`."
                    .to_string()
            )
        );
        assert_eq!(
            parse_pr_evidence_ledger_options(&args(&[
                "--pr-number",
                "123",
                "--base",
                "base",
                "--head",
                "head",
                "--gap-ledger",
                "gap-ledger.json",
                "--out-jsonl",
                ".ripr/pr-evidence-ledger.jsonl",
            ]))
            .map(|options| options.out_jsonl),
            Ok(Some(PathBuf::from(".ripr/pr-evidence-ledger.jsonl")))
        );
        assert_eq!(
            parse_pr_evidence_ledger_options(&args(&[
                "--pr-number",
                "123",
                "--base",
                "base",
                "--head",
                "head",
                "--gap-ledger",
                "gap-ledger.json",
                "--out",
                "ledger.json",
                "--out-jsonl",
                "ledger.json",
            ])),
            Err(
                "pr-ledger record --out-jsonl must not be the same path as --out, --out-md, or an evidence input"
                    .to_string()
            )
        );
        assert_eq!(
            parse_pr_evidence_ledger_options(&args(&[
                "--pr-number",
                "123",
                "--base",
                "base",
                "--head",
                "head",
                "--gap-ledger",
                "gap-ledger.json",
                "--out",
                "./ledger.json",
                "--out-jsonl",
                "ledger.json",
            ])),
            Err(
                "pr-ledger record --out-jsonl must not be the same path as --out, --out-md, or an evidence input"
                    .to_string()
            )
        );
        assert_eq!(
            parse_pr_evidence_ledger_options(&args(&[
                "--pr-number",
                "123",
                "--base",
                "base",
                "--head",
                "head",
                "--gap-ledger",
                "gap-ledger.json",
                "--out-jsonl",
                "gap-ledger.json",
            ])),
            Err(
                "pr-ledger record --out-jsonl must not be the same path as --out, --out-md, or an evidence input"
                    .to_string()
            )
        );
        assert_eq!(
            parse_pr_evidence_ledger_options(&args(&[
                "--pr-number",
                "123",
                "--base",
                "base",
                "--head",
                "head",
                "--gap-ledger",
                "gap-ledger.json",
                "--history",
                ".ripr/pr-evidence-ledger.jsonl",
                "--out-jsonl",
                ".ripr/pr-evidence-ledger.jsonl",
            ]))
            .map(|options| options.out_jsonl),
            Ok(Some(PathBuf::from(".ripr/pr-evidence-ledger.jsonl")))
        );
    }

    #[test]
    fn pr_comments_plan_parses_option_surface() {
        assert_eq!(
            parse_pr_comments_plan_options(&args(&[
                "--root",
                ".",
                "--pr-guidance",
                "target/ripr/review/comments.json",
                "--existing-comments",
                "target/ripr/review/existing-comments.json",
                "--mode",
                "inline",
                "--pull-request",
                "123",
                "--event-name",
                "pull_request",
                "--head-repo",
                "EffortlessMetrics/ripr",
                "--base-repo",
                "EffortlessMetrics/ripr",
                "--token-available",
                "--no-write-permission",
                "--max-inline-comments",
                "2",
                "--out",
                "target/ripr/review/comment-publish-plan.json",
                "--out-md",
                "target/ripr/review/comment-publish-plan.md",
            ])),
            Ok(PrCommentsPlanOptions {
                root: ".".to_string(),
                pr_guidance: Some(PathBuf::from("target/ripr/review/comments.json")),
                existing_comments: Some(PathBuf::from("target/ripr/review/existing-comments.json")),
                mode: output::pr_inline_comment_publish_plan::CommentMode::Inline,
                pull_request: Some(123),
                event_name: Some("pull_request".to_string()),
                head_repo: Some("EffortlessMetrics/ripr".to_string()),
                base_repo: Some("EffortlessMetrics/ripr".to_string()),
                token_available: true,
                write_permission: false,
                max_inline_comments: 2,
                out: PathBuf::from("target/ripr/review/comment-publish-plan.json"),
                out_md: PathBuf::from("target/ripr/review/comment-publish-plan.md"),
            })
        );
    }

    #[test]
    fn pr_comments_plan_rejects_bad_subcommands_and_options() {
        // Bare `ripr pr-comments` is the `plan` alias (#2013). The dispatch
        // writes the default plan files, so they are cleaned on both sides.
        for residue in [
            "target/ripr/review/comment-publish-plan.json",
            "target/ripr/review/comment-publish-plan.md",
        ] {
            let _ = std::fs::remove_file(residue);
        }
        let bare_result = pr_comments(&args(&[]));
        let json_path = "target/ripr/review/comment-publish-plan.json";
        let md_path = "target/ripr/review/comment-publish-plan.md";
        let json_text = std::fs::read_to_string(json_path);
        assert!(json_text.is_ok(), "bare dispatch must write {json_path}");
        let json_text = json_text.unwrap_or_default();
        let plan = serde_json::from_str::<serde_json::Value>(&json_text);
        assert!(
            plan.is_ok(),
            "plan output must be valid JSON: {}",
            plan.err().map(|err| err.to_string()).unwrap_or_default()
        );
        let plan = plan.unwrap_or_default();
        assert!(
            plan.get("schema_version").is_some() || plan.get("mode").is_some(),
            "plan output lost its contract shape: {json_text}"
        );
        assert!(
            std::path::Path::new(md_path).is_file(),
            "bare dispatch must write the markdown plan"
        );
        for residue in [json_path, md_path] {
            let _ = std::fs::remove_file(residue);
        }
        assert!(
            bare_result.is_ok(),
            "bare pr-comments must dispatch to plan with defaults"
        );
        assert_eq!(
            pr_comments(&args(&["publish"])),
            Err("unknown pr-comments subcommand \"publish\"; expected `plan`".to_string())
        );
        assert_eq!(
            parse_pr_comments_plan_options(&args(&["--mode", "post"])),
            Err(
                "unknown pr-comments plan mode \"post\"; expected `off`, `plan`, or `inline`"
                    .to_string()
            )
        );
        assert_eq!(
            parse_pr_comments_plan_options(&args(&["--pr-guidance", ""])),
            Err("pr-comments plan --pr-guidance requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_pr_comments_plan_options(&args(&["--max-inline-comments", "0"])),
            Err("pr-comments plan --max-inline-comments must be greater than zero".to_string())
        );
        assert_eq!(
            parse_pr_comments_plan_options(&args(&["--bad"])),
            Err(
                "unknown pr-comments plan argument \"--bad\". Run `ripr pr-comments plan --help`."
                    .to_string()
            )
        );
    }

    #[test]
    fn pr_comments_plan_writes_json_and_markdown_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("pr-comments-plan");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        let comments = dir.join("comments.json");
        let out = dir.join("comment-publish-plan.json");
        let out_md = dir.join("comment-publish-plan.md");
        std::fs::write(
            &comments,
            r#"{"comments":[{"id":"ripr-review-a","dedupe_key":"ripr:a","placement":{"path":"src/lib.rs","line":7,"side":"RIGHT","mode":"exact_seam_line"},"reason":"add focused assertion"}],"summary_only":[],"suppressed":[]}"#,
        )
        .map_err(|err| format!("write comments: {err}"))?;

        pr_comments(&args(&[
            "plan",
            "--pr-guidance",
            &comments.display().to_string(),
            "--mode",
            "plan",
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json = std::fs::read_to_string(&out)
            .map_err(|err| format!("read publish plan JSON: {err}"))?;
        let markdown = std::fs::read_to_string(&out_md)
            .map_err(|err| format!("read publish plan Markdown: {err}"))?;
        assert!(json.contains(r#""kind": "pr_inline_comment_publish_plan""#));
        assert!(json.contains(r#""planned_create": 1"#));
        assert!(markdown.contains("# RIPR Inline Comment Publish Plan"));
        assert!(markdown.contains("publishable comments: 1"));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn first_action_parses_option_surface() {
        assert_eq!(
            parse_first_action_options(&args(&[
                "--root",
                ".",
                "--pr-guidance",
                "target/ripr/review/comments.json",
                "--assistant-proof",
                "target/ripr/reports/test-oracle-assistant-proof.json",
                "--gap-ledger",
                "target/ripr/reports/gap-decision-ledger.json",
                "--ledger",
                "target/ripr/reports/pr-evidence-ledger.json",
                "--baseline-delta",
                "target/ripr/reports/baseline-debt-delta.json",
                "--receipt",
                "target/ripr/reports/agent-receipt.json",
                "--gate-decision",
                "target/ripr/reports/gate-decision.json",
                "--coverage-frontier",
                "target/ripr/reports/coverage-grip-frontier.json",
                "--editor-context",
                "target/ripr/workflow/evidence-context.json",
                "--out",
                "target/ripr/reports/first-useful-action.json",
                "--out-md",
                "target/ripr/reports/first-useful-action.md",
            ])),
            Ok(FirstActionOptions {
                root: ".".to_string(),
                pr_guidance: Some(PathBuf::from("target/ripr/review/comments.json")),
                assistant_proof: Some(PathBuf::from(
                    "target/ripr/reports/test-oracle-assistant-proof.json",
                )),
                gap_ledger: Some(PathBuf::from(
                    "target/ripr/reports/gap-decision-ledger.json"
                )),
                ledger: Some(PathBuf::from("target/ripr/reports/pr-evidence-ledger.json")),
                baseline_delta: Some(PathBuf::from(
                    "target/ripr/reports/baseline-debt-delta.json",
                )),
                receipt: Some(PathBuf::from("target/ripr/reports/agent-receipt.json")),
                gate_decision: Some(PathBuf::from("target/ripr/reports/gate-decision.json")),
                coverage_frontier: Some(PathBuf::from(
                    "target/ripr/reports/coverage-grip-frontier.json",
                )),
                editor_context: Some(PathBuf::from("target/ripr/workflow/evidence-context.json")),
                out: PathBuf::from("target/ripr/reports/first-useful-action.json"),
                out_md: PathBuf::from("target/ripr/reports/first-useful-action.md"),
            })
        );
    }

    #[test]
    fn first_action_requires_input_and_rejects_unknown_args() {
        assert_eq!(
            parse_first_action_options(&args(&[])),
            Err("first-action requires at least one explicit artifact input".to_string())
        );
        assert_eq!(
            parse_first_action_options(&args(&["--pr-guidance", ""])),
            Err("first-action --pr-guidance requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_first_action_options(&args(&["--bad"])),
            Err(
                "unknown first-action argument \"--bad\". Run `ripr first-action --help`."
                    .to_string()
            )
        );
    }

    #[test]
    fn first_action_cli_writes_gap_record_report() -> Result<(), String> {
        let dir = unique_command_test_dir("first-action-gap-record");
        std::fs::create_dir_all(&dir)
            .map_err(|err| format!("create first-action gap-record dir: {err}"))?;
        let gap_ledger = dir.join("gap-decision-ledger.json");
        let out = dir.join("first-useful-action.json");
        let out_md = dir.join("first-useful-action.md");
        std::fs::write(
            &gap_ledger,
            r#"{
  "kind": "gap_decision_ledger",
  "records": [
    {
      "gap_id": "gap:pr:pricing:threshold-boundary",
      "source_currentness": "candidate_current",
      "canonical_gap_id": "gap:rust:pricing:discount:threshold-boundary",
      "kind": "MissingBoundaryAssertion",
      "language": "rust",
      "language_status": "stable",
      "scope": "pr_local",
      "evidence_class": "predicate_boundary",
      "gap_state": "actionable",
      "policy_state": "new",
      "repairability": "repairable",
      "anchor": {
        "file": "src/pricing.rs",
        "line": 42,
        "dedupe_fingerprint": "gap:rust:pricing:discount:threshold-boundary"
      },
      "repair_route": {
        "route_kind": "AddBoundaryAssertion",
        "target_file": "tests/pricing.rs",
        "assertion_shape": "assert_eq!(discount(100, 100), 90)"
      },
      "verification_commands": [
        "cargo xtask fixtures boundary_gap"
      ]
    }
  ]
}"#,
        )
        .map_err(|err| format!("write gap decision ledger: {err}"))?;

        first_action(&args(&[
            "--gap-ledger",
            &gap_ledger.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json = std::fs::read_to_string(&out)
            .map_err(|err| format!("read first-action JSON: {err}"))?;
        assert!(json.contains(r#""source": "gap_ledger""#));
        assert!(json.contains(r#""repair_route": "AddBoundaryAssertion""#));
        let markdown = std::fs::read_to_string(&out_md)
            .map_err(|err| format!("read first-action Markdown: {err}"))?;
        assert!(markdown.contains("Repair MissingBoundaryAssertion via AddBoundaryAssertion"));

        std::fs::remove_dir_all(&dir)
            .map_err(|err| format!("remove first-action gap-record dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn ripr_zero_status_writes_json_and_markdown_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("ripr-zero-status");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create zero status dir: {err}"))?;
        let out = dir.join("ripr-zero-status.json");
        let out_md = dir.join("ripr-zero-status.md");
        let baseline = repo_root()
            .join("fixtures/boundary_gap/expected/baseline-debt-delta/mixed/baseline.json");
        let delta = repo_root().join(
            "fixtures/boundary_gap/expected/baseline-debt-delta/mixed/baseline-debt-delta.json",
        );

        zero(&args(&[
            "status",
            "--baseline",
            &baseline.display().to_string(),
            "--delta",
            &delta.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text =
            std::fs::read_to_string(&out).map_err(|err| format!("read zero json: {err}"))?;
        assert!(json_text.contains("\"kind\": \"ripr_zero_status\""));
        assert!(json_text.contains("\"status\": \"advisory\""));
        assert!(json_text.contains("\"baseline_debt_delta\""));

        let markdown =
            std::fs::read_to_string(&out_md).map_err(|err| format!("read zero md: {err}"))?;
        assert!(markdown.starts_with("# RIPR Zero Status"));
        assert!(markdown.contains("Visible unresolved gaps"));

        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove zero status dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn pr_evidence_ledger_writes_json_and_markdown_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("pr-evidence-ledger");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create ledger dir: {err}"))?;
        let out = dir.join("pr-evidence-ledger.json");
        let out_md = dir.join("pr-evidence-ledger.md");
        let gap_ledger = dir.join("gap-decision-ledger.json");
        let fixture = repo_root().join("fixtures/boundary_gap/expected/pr-evidence-ledger/mixed");
        std::fs::write(
            &gap_ledger,
            r#"{"gap_records":[{"gap_id":"gap:pr:cli","canonical_gap_id":"gap:rust:cli","kind":"MissingBoundaryAssertion","language":"rust","language_status":"stable","scope":"pr_local","gap_state":"actionable","policy_state":"new","repairability":"repairable","anchor":{"file":"src/cli.rs","line":7},"repair_route":{"route_kind":"AddBoundaryAssertion","assertion_shape":"assert!(cli())"},"verification_commands":["cargo xtask fixtures boundary_gap"]}]}"#,
        )
        .map_err(|err| format!("write gap ledger: {err}"))?;

        pr_ledger(&args(&[
            "record",
            "--pr-number",
            "123",
            "--base",
            "base",
            "--head",
            "head",
            "--gate",
            &fixture.join("gate-decision.json").display().to_string(),
            "--baseline-delta",
            &fixture
                .join("baseline-debt-delta.json")
                .display()
                .to_string(),
            "--zero-status",
            &fixture.join("ripr-zero-status.json").display().to_string(),
            "--pr-guidance",
            &fixture.join("comments.json").display().to_string(),
            "--gap-ledger",
            &gap_ledger.display().to_string(),
            "--agent-receipt",
            &fixture.join("agent-receipt.json").display().to_string(),
            "--history",
            &fixture.join("history.jsonl").display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text =
            std::fs::read_to_string(&out).map_err(|err| format!("read ledger json: {err}"))?;
        assert!(json_text.contains("\"kind\": \"pr_evidence_ledger\""));
        assert!(json_text.contains("\"baseline_resolved\": 3"));
        assert!(json_text.contains("\"source\": \"gap_decision_ledger\""));
        assert!(json_text.contains("\"gap_id\": \"gap:pr:cli\""));
        let md_text =
            std::fs::read_to_string(&out_md).map_err(|err| format!("read ledger md: {err}"))?;
        assert!(md_text.contains("# RIPR PR Evidence Ledger"));
        assert!(md_text.contains("Gate: acknowledgeable / acknowledged"));
        assert!(md_text.contains("Gap decision ledger:"));
        assert!(
            !dir.join("produced.jsonl").exists(),
            "default pr-ledger record must not write JSONL without --out-jsonl"
        );

        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove ledger dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn pr_evidence_ledger_out_jsonl_is_opt_in_and_feeds_non_null_history() -> Result<(), String> {
        let dir = unique_command_test_dir("pr-evidence-ledger-jsonl");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create ledger dir: {err}"))?;
        let out = dir.join("pr-evidence-ledger.json");
        let out_md = dir.join("pr-evidence-ledger.md");
        let jsonl = dir.join("pr-evidence-ledger.jsonl");
        let second_out = dir.join("second.json");
        let second_md = dir.join("second.md");
        let gap_ledger = dir.join("gap-decision-ledger.json");
        let fixture = repo_root().join("fixtures/boundary_gap/expected/pr-evidence-ledger/mixed");
        std::fs::write(
            &gap_ledger,
            r#"{"gap_records":[{"gap_id":"gap:pr:cli","canonical_gap_id":"gap:rust:cli","kind":"MissingBoundaryAssertion","language":"rust","language_status":"stable","scope":"pr_local","gap_state":"actionable","policy_state":"new","repairability":"repairable","anchor":{"file":"src/cli.rs","line":7},"repair_route":{"route_kind":"AddBoundaryAssertion","assertion_shape":"assert!(cli())"},"verification_commands":["cargo xtask fixtures boundary_gap"]}]}"#,
        )
        .map_err(|err| format!("write gap ledger: {err}"))?;

        pr_ledger(&args(&[
            "record",
            "--pr-number",
            "1",
            "--base",
            "base",
            "--head",
            "head",
            "--gate",
            &fixture.join("gate-decision.json").display().to_string(),
            "--baseline-delta",
            &fixture
                .join("baseline-debt-delta.json")
                .display()
                .to_string(),
            "--zero-status",
            &fixture.join("ripr-zero-status.json").display().to_string(),
            "--pr-guidance",
            &fixture.join("comments.json").display().to_string(),
            "--gap-ledger",
            &gap_ledger.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
            "--out-jsonl",
            &jsonl.display().to_string(),
        ]))?;

        let first_json =
            std::fs::read_to_string(&out).map_err(|err| format!("read first json: {err}"))?;
        assert!(
            first_json.contains("\"history\": null"),
            "first record without --history stays null: {first_json}"
        );
        let produced =
            std::fs::read_to_string(&jsonl).map_err(|err| format!("read produced jsonl: {err}"))?;
        let first_line = produced
            .lines()
            .next()
            .ok_or_else(|| "produced jsonl must have one line".to_string())?;
        assert!(
            first_line.contains("\"kind\":\"pr_evidence_ledger\""),
            "pr-ledger jsonl is the compact record object: {first_line}"
        );

        pr_ledger(&args(&[
            "record",
            "--pr-number",
            "2",
            "--base",
            "base",
            "--head",
            "head",
            "--gate",
            &fixture.join("gate-decision.json").display().to_string(),
            "--baseline-delta",
            &fixture
                .join("baseline-debt-delta.json")
                .display()
                .to_string(),
            "--zero-status",
            &fixture.join("ripr-zero-status.json").display().to_string(),
            "--pr-guidance",
            &fixture.join("comments.json").display().to_string(),
            "--gap-ledger",
            &gap_ledger.display().to_string(),
            "--history",
            &jsonl.display().to_string(),
            "--out",
            &second_out.display().to_string(),
            "--out-md",
            &second_md.display().to_string(),
            "--out-jsonl",
            &jsonl.display().to_string(),
        ]))?;

        let second_json = std::fs::read_to_string(&second_out)
            .map_err(|err| format!("read second json: {err}"))?;
        assert!(
            !second_json.contains("\"history\": null"),
            "history from --out-jsonl must be non-null: {second_json}"
        );
        assert!(
            second_json.contains("\"records\": 1"),
            "history must count the appended record: {second_json}"
        );
        assert!(
            second_json.contains("\"trend\": \"improving\"")
                || second_json.contains("\"trend\": \"regressing\"")
                || second_json.contains("\"trend\": \"stable\""),
            "trend must be populated from produced history: {second_json}"
        );
        let appended =
            std::fs::read_to_string(&jsonl).map_err(|err| format!("read appended jsonl: {err}"))?;
        let lines: Vec<&str> = appended.lines().filter(|line| !line.is_empty()).collect();
        assert_eq!(lines.len(), 2, "second --out-jsonl must append: {appended}");
        assert_eq!(
            lines[0], first_line,
            "append must preserve the prior record"
        );
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove ledger dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn coverage_grip_frontier_parses_option_surface() {
        assert_eq!(
            parse_coverage_grip_frontier_options(&args(&[
                "--coverage",
                "target/ripr/reports/coverage-summary.json",
                "--ledger",
                "target/ripr/reports/pr-evidence-ledger.json",
                "--baseline-delta",
                "target/ripr/reports/baseline-debt-delta.json",
                "--zero-status",
                "target/ripr/reports/ripr-zero-status.json",
                "--out",
                "target/ripr/reports/coverage-grip-frontier.json",
                "--out-md",
                "target/ripr/reports/coverage-grip-frontier.md",
            ])),
            Ok(CoverageGripFrontierOptions {
                coverage: Some(PathBuf::from("target/ripr/reports/coverage-summary.json")),
                ledger: Some(PathBuf::from("target/ripr/reports/pr-evidence-ledger.json")),
                baseline_delta: Some(PathBuf::from(
                    "target/ripr/reports/baseline-debt-delta.json"
                )),
                zero_status: Some(PathBuf::from("target/ripr/reports/ripr-zero-status.json")),
                out: PathBuf::from("target/ripr/reports/coverage-grip-frontier.json"),
                out_md: PathBuf::from("target/ripr/reports/coverage-grip-frontier.md"),
            })
        );
    }

    #[test]
    fn coverage_grip_frontier_requires_movement_input() {
        // Bare `ripr coverage-grip` is the `frontier` alias (#2013).
        assert_eq!(
            coverage_grip(&args(&[])),
            Err(
                "coverage-grip frontier requires at least one of --ledger, --baseline-delta, or --zero-status"
                    .to_string()
            )
        );
        assert_eq!(
            coverage_grip(&args(&["unknown"])),
            Err("unknown coverage-grip subcommand \"unknown\"; expected `frontier`".to_string())
        );
        assert_eq!(
            parse_coverage_grip_frontier_options(&args(&["--coverage", "coverage.json"])),
            Err(
                "coverage-grip frontier requires at least one of --ledger, --baseline-delta, or --zero-status"
                    .to_string()
            )
        );
        assert_eq!(
            parse_coverage_grip_frontier_options(&args(&["--bad"])),
            Err("unknown coverage-grip frontier argument \"--bad\". Run `ripr coverage-grip frontier --help`.".to_string())
        );
    }

    #[test]
    fn coverage_grip_frontier_writes_json_and_markdown_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("coverage-grip-frontier");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create frontier dir: {err}"))?;
        let coverage = dir.join("coverage-summary.json");
        let ledger = repo_root().join(
            "fixtures/boundary_gap/expected/pr-evidence-ledger/mixed/pr-evidence-ledger.json",
        );
        let out = dir.join("coverage-grip-frontier.json");
        let out_md = dir.join("coverage-grip-frontier.md");
        std::fs::write(
            &coverage,
            r#"{"coverage_delta_percent":0.0,"ripr_visible_unresolved_delta":-3}"#,
        )
        .map_err(|err| format!("write coverage: {err}"))?;

        coverage_grip(&args(&[
            "frontier",
            "--coverage",
            &coverage.display().to_string(),
            "--ledger",
            &ledger.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let rendered =
            std::fs::read_to_string(&out).map_err(|err| format!("read frontier JSON: {err}"))?;
        let markdown = std::fs::read_to_string(&out_md)
            .map_err(|err| format!("read frontier Markdown: {err}"))?;
        assert!(rendered.contains(r#""kind": "coverage_grip_frontier""#));
        assert!(rendered.contains("behavioral grip improved without line-coverage movement"));
        assert!(markdown.contains("# RIPR Coverage / Grip Frontier"));
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove frontier dir: {err}"))?;
        Ok(())
    }
    #[test]
    fn outcome_defaults_to_markdown_stdout_shape() {
        assert_eq!(
            parse_outcome_options(&args(
                &["--before", "before.json", "--after", "after.json",]
            )),
            Ok(OutcomeOptions {
                before: PathBuf::from("before.json"),
                after: PathBuf::from("after.json"),
                format: OutcomeFormat::Markdown,
                out: None,
            })
        );
    }

    #[test]
    fn outcome_requires_before_and_after() {
        assert_eq!(
            parse_outcome_options(&args(&["--after", "after.json"])),
            Err("outcome requires --before <path>".to_string())
        );
        assert_eq!(
            parse_outcome_options(&args(&["--before", "before.json"])),
            Err("outcome requires --after <path>".to_string())
        );
    }

    #[test]
    fn outcome_help_returns_ok() {
        assert_eq!(outcome(&args(&["--help"])), Ok(()));
    }

    /// Run the retained `ripr check` production path over the
    /// `ts_repair_packet_complete` fixture and return the rendered check JSON.
    #[cfg(feature = "lang-typescript")]
    fn ts_repair_packet_complete_check_json() -> Result<serde_json::Value, String> {
        let fixture = repo_root().join("fixtures/ts_repair_packet_complete");
        let mut input = crate::app::CheckInput {
            root: fixture.join("input"),
            diff_file: Some(fixture.join("diff.patch")),
            mode: crate::app::Mode::Draft,
            ..crate::app::CheckInput::default()
        };
        let config = crate::config::load_for_root(&input.root)?;
        crate::config::apply_to_check_input(
            &mut input,
            &config,
            crate::config::CheckInputExplicit::default(),
        );
        let output = crate::app::check_workspace_with_config(input, &config)?;
        let rendered = crate::output::json::render_with_config(&output, &config);
        serde_json::from_str(&rendered).map_err(|err| format!("check JSON did not parse: {err}"))
    }

    /// The single TypeScript repair packet the fixture's check JSON carries.
    #[cfg(feature = "lang-typescript")]
    fn ts_repair_packet(check: &serde_json::Value) -> Result<&serde_json::Value, String> {
        let packets: Vec<&serde_json::Value> = check["findings"]
            .as_array()
            .ok_or("check JSON must carry a findings array")?
            .iter()
            .filter_map(|finding| finding.get("typescript_repair_packet"))
            .collect();
        match packets.as_slice() {
            [packet] => Ok(packet),
            other => Err(format!(
                "expected exactly one TypeScript repair packet, got {}",
                other.len()
            )),
        }
    }

    #[cfg(feature = "lang-typescript")]
    fn ts_packet_receipt_command(packet: &serde_json::Value) -> Result<String, String> {
        packet["receipt_command"]
            .as_str()
            .map(ToString::to_string)
            .ok_or_else(|| format!("TypeScript repair packet has no receipt_command: {packet}"))
    }

    /// Split a copied bash command into argv the way a POSIX shell does for
    /// the quoting `shell_arg` emits: whitespace-separated words, single-quoted
    /// spans taken literally, and backslash escapes outside quotes. Any other
    /// shell-active character fails instead of being guessed at.
    fn posix_words(command: &str) -> Result<Vec<String>, String> {
        let mut words = Vec::new();
        let mut current: Option<String> = None;
        let mut chars = command.chars();
        while let Some(ch) = chars.next() {
            match ch {
                '\'' => {
                    let word = current.get_or_insert_with(String::new);
                    loop {
                        match chars.next() {
                            Some('\'') => break,
                            Some(inner) => word.push(inner),
                            None => return Err(format!("unterminated quote in `{command}`")),
                        }
                    }
                }
                '\\' => {
                    let escaped = chars
                        .next()
                        .ok_or_else(|| format!("dangling escape in `{command}`"))?;
                    current.get_or_insert_with(String::new).push(escaped);
                }
                '"' | '$' | '`' | ';' | '&' | '|' | '<' | '>' | '(' | ')' => {
                    return Err(format!("shell-active `{ch}` in `{command}`"));
                }
                ch if ch.is_whitespace() => {
                    if let Some(word) = current.take() {
                        words.push(word);
                    }
                }
                ch => current.get_or_insert_with(String::new).push(ch),
            }
        }
        if let Some(word) = current {
            words.push(word);
        }
        Ok(words)
    }

    /// #3906 / RIPR-SPEC-0079 `canonical_receipt_command` field rule: the
    /// receipt command the TypeScript preview packet emits is copied verbatim
    /// into a shell, so it must be the canonical `ripr receipt write` form and
    /// every argument it carries must be accepted by that command's parser.
    #[test]
    #[cfg(feature = "lang-typescript")]
    fn typescript_preview_receipt_command_parses_as_receipt_write() -> Result<(), String> {
        let check = ts_repair_packet_complete_check_json()?;
        let packet = ts_repair_packet(&check)?;
        let command = ts_packet_receipt_command(packet)?;
        let words = posix_words(&command)?;
        let rest = words
            .strip_prefix(&[
                "ripr".to_string(),
                "receipt".to_string(),
                "write".to_string(),
            ])
            .ok_or_else(|| format!("not a `ripr receipt write` command: {command}"))?;
        let options = receipt_command::parse_receipt_write_options(rest)
            .map_err(|err| format!("`{command}` is rejected by ripr receipt write: {err}"))?;
        assert_eq!(
            options.canonical_gap_id,
            "gap:typescript:typescript_preview:2396aec1"
        );
        assert_eq!(
            Some(options.canonical_gap_id.as_str()),
            packet["canonical_gap_id"].as_str(),
            "receipt must bind to the packet's canonical gap id"
        );
        assert_eq!(options.verify_command, "jest tests/discount.test.ts");
        assert_eq!(
            Some(options.verify_command.as_str()),
            packet["verify_command"].as_str(),
            "receipt must carry the packet's real verify command"
        );
        assert_eq!(options.verify_status, "not_run");
        assert_eq!(
            options.out,
            Some(PathBuf::from(
                "target/ripr/receipts/gap-typescript-typescript_preview-2396aec1.json"
            ))
        );
        Ok(())
    }

    /// RIPR-SPEC-0079 "`ripr outcome` is not a receipt command": the emitted
    /// TypeScript receipt command is not a movement invocation, `ripr outcome`
    /// rejects its arguments, and the retired movement form it replaced is not
    /// a receipt write either. The gap ledger also synthesizes the same string
    /// for the same gap when the packet supplies none, so both surfaces agree.
    #[test]
    #[cfg(feature = "lang-typescript")]
    fn typescript_preview_receipt_command_is_not_ripr_outcome() -> Result<(), String> {
        let check = ts_repair_packet_complete_check_json()?;
        let command = ts_packet_receipt_command(ts_repair_packet(&check)?)?;
        if command.contains("ripr outcome") {
            return Err(format!(
                "receipt_command must not be a ripr outcome invocation: {command}"
            ));
        }
        let words = posix_words(&command)?;
        if words.get(..3)
            != Some(
                &[
                    "ripr".to_string(),
                    "receipt".to_string(),
                    "write".to_string(),
                ][..],
            )
        {
            return Err(format!(
                "receipt_command must be a ripr receipt write invocation: {command}"
            ));
        }
        let receipt_args = words.get(3..).ok_or("receipt command has no arguments")?;
        if parse_outcome_options(receipt_args).is_ok() {
            return Err(format!(
                "ripr outcome must reject the receipt-write arguments of `{command}`"
            ));
        }
        let retired_movement_form = args(&[
            "--before",
            "<baseline>",
            "--after",
            "<repair>",
            "--out",
            "target/ripr/receipts/gap_typescript_typescript_preview_2396aec1.targeted-test-outcome.json",
        ]);
        if receipt_command::parse_receipt_write_options(&retired_movement_form).is_ok() {
            return Err("ripr receipt write must reject the retired movement form".to_string());
        }

        let mut without_packet_receipt = check.clone();
        if let Some(packet) = without_packet_receipt["findings"]
            .as_array_mut()
            .and_then(|findings| findings.first_mut())
            .and_then(|finding| finding.get_mut("typescript_repair_packet"))
            .and_then(serde_json::Value::as_object_mut)
        {
            packet.remove("receipt_command");
        }
        let ledger = crate::output::gap_decision_ledger::build_gap_decision_ledger_report(
            crate::output::gap_decision_ledger::GapDecisionLedgerInput {
                root: ".".to_string(),
                generated_at: "test".to_string(),
                source_kind:
                    crate::output::gap_decision_ledger::GapDecisionLedgerSourceKind::CheckOutput,
                records_path: "target/ripr/reports/check.json".to_string(),
                records_json: Ok(without_packet_receipt.to_string()),
            },
        );
        let ledger: serde_json::Value = serde_json::from_str(
            &crate::output::gap_decision_ledger::render_gap_decision_ledger_json(&ledger)?,
        )
        .map_err(|err| format!("gap ledger JSON did not parse: {err}"))?;
        let synthesized = match ledger["records"].as_array().map(Vec::as_slice) {
            Some([record]) => record["receipt_command"].as_str().map(ToString::to_string),
            other => {
                return Err(format!(
                    "expected one gap-ledger record, got {:?}",
                    other.map(<[serde_json::Value]>::len)
                ));
            }
        };
        assert_eq!(
            synthesized.as_deref(),
            Some(command.as_str()),
            "gap ledger and TypeScript packet must emit the same receipt command"
        );
        Ok(())
    }

    #[test]
    fn evidence_health_help_returns_ok() {
        assert_eq!(evidence_health(&args(&["--help"])), Ok(()));
    }

    #[test]
    fn outcome_command_writes_json_file() -> Result<(), String> {
        let dir = unique_command_test_dir("outcome");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        let before = dir.join("before.json");
        let after = dir.join("after.json");
        let out = dir.join("nested/targeted-test-outcome.json");
        std::fs::write(&before, outcome_before_json())
            .map_err(|err| format!("write before snapshot: {err}"))?;
        std::fs::write(&after, outcome_after_json())
            .map_err(|err| format!("write after snapshot: {err}"))?;

        outcome(&args(&[
            "--before",
            &before.display().to_string(),
            "--after",
            &after.display().to_string(),
            "--format",
            "json",
            "--out",
            &out.display().to_string(),
        ]))?;

        let rendered =
            std::fs::read_to_string(&out).map_err(|err| format!("read outcome output: {err}"))?;
        assert!(rendered.contains(r#""schema_version": "0.1""#));
        assert!(rendered.contains(r#""moved": 1"#));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn outcome_command_reports_read_failures() -> Result<(), String> {
        let dir = unique_command_test_dir("outcome-read");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        let before = dir.join("before.json");
        std::fs::write(&before, outcome_before_json())
            .map_err(|err| format!("write before snapshot: {err}"))?;

        let missing_before = outcome(&args(&[
            "--before",
            &dir.join("missing-before.json").display().to_string(),
            "--after",
            &dir.join("missing-after.json").display().to_string(),
        ]));
        assert!(matches!(missing_before, Err(message) if message.contains("read")));

        let missing_after = outcome(&args(&[
            "--before",
            &before.display().to_string(),
            "--after",
            &dir.join("missing-after.json").display().to_string(),
        ]));
        assert!(matches!(missing_after, Err(message) if message.contains("read")));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn calibrate_parses_required_inputs_format_and_out() {
        assert_eq!(
            parse_calibrate_cargo_mutants_options(&args(&[
                "--mutants-json",
                "target/mutants/outcomes.json",
                "--repo-exposure-json",
                "target/ripr/after.repo-exposure.json",
                "--format",
                "json",
                "--out",
                "target/ripr/calibration/mutation-calibration.json",
            ])),
            Ok(CalibrateOptions {
                mutants_json: PathBuf::from("target/mutants/outcomes.json"),
                repo_exposure_json: PathBuf::from("target/ripr/after.repo-exposure.json"),
                format: CalibrateFormat::Json,
                out: Some(PathBuf::from(
                    "target/ripr/calibration/mutation-calibration.json"
                )),
            })
        );
    }

    #[test]
    fn calibrate_requires_subcommand_and_inputs() {
        assert_eq!(
            calibrate(&args(&[])),
            Err("calibrate requires subcommand `cargo-mutants`".to_string())
        );
        assert_eq!(
            calibrate(&args(&["runtime"])),
            Err("unknown calibrate subcommand \"runtime\"; expected `cargo-mutants`".to_string())
        );
        assert_eq!(
            parse_calibrate_cargo_mutants_options(&args(&["--repo-exposure-json", "repo.json"])),
            Err("calibrate cargo-mutants requires --mutants-json <path>".to_string())
        );
        assert_eq!(
            parse_calibrate_cargo_mutants_options(&args(&["--mutants-json", "mutants.json"])),
            Err("calibrate cargo-mutants requires --repo-exposure-json <path>".to_string())
        );
    }

    #[test]
    fn calibrate_help_returns_ok() {
        assert_eq!(calibrate(&args(&["--help"])), Ok(()));
        assert_eq!(calibrate(&args(&["cargo-mutants", "--help"])), Ok(()));
    }

    #[test]
    fn calibrate_command_writes_json_file() -> Result<(), String> {
        let dir = unique_command_test_dir("calibrate");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        let repo = dir.join("repo-exposure.json");
        let mutants = dir.join("mutants.json");
        let out = dir.join("nested/mutation-calibration.json");
        std::fs::write(&repo, calibration_repo_json())
            .map_err(|err| format!("write repo exposure: {err}"))?;
        std::fs::write(&mutants, calibration_mutants_json())
            .map_err(|err| format!("write mutants: {err}"))?;

        calibrate(&args(&[
            "cargo-mutants",
            "--mutants-json",
            &mutants.display().to_string(),
            "--repo-exposure-json",
            &repo.display().to_string(),
            "--format",
            "json",
            "--out",
            &out.display().to_string(),
        ]))?;

        let rendered = std::fs::read_to_string(&out)
            .map_err(|err| format!("read calibration output: {err}"))?;
        assert!(rendered.contains(r#""schema_version": "0.1""#));
        assert!(rendered.contains(r#""static_gap_and_runtime_signal": 1"#));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn calibrate_reads_cargo_mutants_directory() -> Result<(), String> {
        let dir = unique_command_test_dir("calibrate-dir");
        let mutants_dir = dir.join("cargo-mutants");
        std::fs::create_dir_all(&mutants_dir)
            .map_err(|err| format!("create mutants dir: {err}"))?;
        std::fs::write(
            mutants_dir.join("mutants.json"),
            r#"{"mutants":[{"id":"m1","seam_id":"seam-a","operator":"replace"}]}"#,
        )
        .map_err(|err| format!("write mutants.json: {err}"))?;
        std::fs::write(
            mutants_dir.join("outcomes.json"),
            r#"{"outcomes":[{"id":"m1","outcome":"missed"}]}"#,
        )
        .map_err(|err| format!("write outcomes.json: {err}"))?;

        let combined = read_calibration_mutants_json(&mutants_dir)?;
        assert!(combined.contains("mutants"));
        assert!(combined.contains("outcomes"));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn init_requires_root_value() {
        assert_eq!(
            init(&args(&["--root"])),
            Err("missing value for --root".to_string())
        );
        assert_eq!(
            init(&args(&["--ci"])),
            Err("missing value for --ci".to_string())
        );
    }

    #[test]
    fn init_rejects_unknown_arguments() {
        assert_eq!(
            init(&args(&["--wat"])),
            Err("unknown init argument \"--wat\". Run `ripr init --help`.".to_string())
        );
        assert_eq!(
            init(&args(&["--ci", "gitlab"])),
            Err("unknown init --ci provider \"gitlab\"".to_string())
        );
    }

    #[test]
    fn init_parses_root_dry_run_and_force() {
        assert_eq!(
            parse_init_options(&args(&[
                "--root",
                "repo",
                "--dry-run",
                "--force",
                "--ci",
                "github",
            ])),
            Ok(InitOptions {
                root: PathBuf::from("repo"),
                dry_run: true,
                force: true,
                ci: Some(InitCi::Github),
            })
        );
    }

    /// The contiguous `#` comment lines immediately above `<name>:` in the
    /// generated workflow's `env:` block. Extracting the block is what makes
    /// the negative assertions below mean anything: a phrase deleted from this
    /// comment could still match somewhere else in a 2000-line workflow.
    fn generated_workflow_env_comment(workflow: &str, name: &str) -> Result<String, String> {
        let mut comment: Vec<&str> = Vec::new();
        for line in workflow.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with(&format!("{name}:")) {
                // An empty block means the comment moved away from the entry
                // (a blank line between them also lands here). Fail with that
                // as the reason rather than running `contains` on "".
                if comment.is_empty() {
                    return Err(format!("`{name}:` has no comment directly above it"));
                }
                return Ok(comment.join("\n"));
            }
            if let Some(text) = trimmed.strip_prefix("# ") {
                comment.push(text);
            } else if trimmed.starts_with('#') {
                comment.push("");
            } else {
                comment.clear();
            }
        }
        Err(format!("generated workflow has no `{name}:` env entry"))
    }

    /// The generated workflow documents the gate to whoever adopts it, and
    /// nothing else reads those comments, so they drift silently. Bind them to
    /// the two contracts they describe rather than to their own wording.
    #[test]
    fn generated_workflow_env_comments_match_the_gate_contract() -> Result<(), String> {
        let workflow = generated_github_actions_workflow();
        let gate_help = crate::cli::help::help_text_for("gate")
            .ok_or_else(|| "`ripr gate` has no help text".to_string())?;

        // `--baseline` takes a filesystem path, and the workflow feeds this
        // variable straight to that flag, so the comment cannot call it a ref.
        assert!(
            gate_help.contains("--baseline PATH"),
            "gate help no longer declares --baseline as a PATH:\n{gate_help}"
        );
        assert!(
            workflow.contains("--baseline \"$RIPR_GATE_BASELINE\""),
            "generated workflow no longer passes RIPR_GATE_BASELINE to --baseline"
        );
        let baseline = generated_workflow_env_comment(&workflow, "RIPR_GATE_BASELINE")?;
        // The path comes from `ripr baseline create`'s own default rather than
        // from a copy of the docs, so renaming the ledger fails here.
        assert!(
            baseline.contains(crate::output::baseline::DEFAULT_BASELINE_OUT),
            "RIPR_GATE_BASELINE comment does not name `{}`:\n{baseline}",
            crate::output::baseline::DEFAULT_BASELINE_OUT
        );
        for ref_wording in ["git ref", "tag, branch", "SHA"] {
            assert!(
                !baseline.contains(ref_wording),
                "RIPR_GATE_BASELINE comment describes a git ref, but the flag \
                 takes a path:\n{baseline}"
            );
        }

        // Every mode the CLI accepts is listed, derived from the help line
        // rather than from a second list kept here.
        let mode_line = gate_help
            .lines()
            .find(|line| line.trim_start().starts_with("--mode MODE"))
            .ok_or_else(|| format!("gate help does not document --mode:\n{gate_help}"))?;
        let modes: Vec<&str> = mode_line
            .split_once("MODE")
            .map(|(_, values)| values)
            .unwrap_or(mode_line)
            .split('.')
            .next()
            .unwrap_or_default()
            .split(&[',', ' '][..])
            .map(|token| token.trim())
            .filter(|token| {
                !token.is_empty()
                    && *token != "or"
                    && token
                        .chars()
                        .all(|character| character.is_ascii_lowercase() || character == '-')
            })
            .collect();
        // Guards the parse above, not the mode set: modes are only ever added,
        // so a short inventory means this stopped reading the help line.
        assert!(
            modes.len() >= 4,
            "parsed no usable mode inventory from: {mode_line}"
        );
        let mode_comment = generated_workflow_env_comment(&workflow, "RIPR_GATE_MODE")?;
        for mode in &modes {
            assert!(
                mode_comment.contains(mode),
                "generated workflow does not document gate mode `{mode}`:\n{mode_comment}"
            );
            // And each one is a mode the evaluator actually accepts, so a
            // documented mode that no longer parses fails here too.
            assert!(
                crate::output::gate::GateMode::parse(mode).is_ok(),
                "help and the workflow document `{mode}`, which `GateMode::parse` rejects"
            );
        }

        // calibrated-gate blocks a strict subset of what acknowledgeable
        // blocks: `output::gate` requires a new baseline identity, supporting
        // calibration evidence, and warning severity, where acknowledgeable
        // blocks every policy-eligible candidate.
        assert!(
            !mode_comment.contains("any actionable finding"),
            "calibrated-gate is described as the broadest mode:\n{mode_comment}"
        );
        assert!(
            mode_comment.contains("new") && mode_comment.contains("policy-eligible"),
            "calibrated-gate comment does not state what it narrows to:\n{mode_comment}"
        );

        Ok(())
    }

    #[test]
    fn init_generated_github_workflow_is_advisory() {
        let workflow = generated_github_actions_workflow();
        assert!(workflow.contains(
            "continue-on-error: ${{ vars.RIPR_GATE_MODE == '' || vars.RIPR_GATE_MODE == 'visible-only' }}"
        ));
        assert!(workflow.contains("github/codeql-action/upload-sarif@v4"));
        assert!(workflow.contains("actions/upload-artifact@v7"));
        assert!(workflow.contains("RIPR_UPLOAD_SARIF"));
        assert!(workflow.contains("RIPR_GATE_MODE: ${{ vars.RIPR_GATE_MODE || '' }}"));
        assert!(workflow.contains("RIPR_GATE_BASELINE: ${{ vars.RIPR_GATE_BASELINE || '' }}"));
        assert!(workflow.contains("RIPR_COMMENT_MODE: ${{ vars.RIPR_COMMENT_MODE || 'off' }}"));
        assert!(workflow.contains("pull-requests: write"));
        assert!(workflow.contains("--format sarif"));
        assert!(workflow.contains("--format repo-sarif"));
        assert!(workflow.contains("--format repo-badge-json"));
        assert!(workflow.contains("ripr pilot"));
        assert!(workflow.contains("ripr agent start"));
        assert!(workflow.contains("ripr agent packet"));
        assert!(workflow.contains("ripr agent status"));
        assert!(workflow.contains("ripr agent review-summary"));
        assert!(workflow.contains("target/ripr/workflow/agent-packet.json"));
        assert!(workflow.contains("target/ripr/workflow/agent-brief.json"));
        assert!(workflow.contains("target/ripr/workflow/agent-verify.json"));
        assert!(workflow.contains("target/ripr/reports/agent-receipt.json"));
        assert!(workflow.contains("target/ripr/workflow/agent-status.json"));
        assert!(workflow.contains("target/ripr/workflow/agent-status.md"));
        assert!(workflow.contains("target/ripr/workflow/agent-review-summary.json"));
        assert!(workflow.contains("target/ripr/workflow/agent-review-summary.md"));
        assert!(workflow.contains("target/ripr/agent/agent-packet.json"));
        assert!(workflow.contains("target/ripr/agent/agent-brief.json"));
        assert!(workflow.contains("target/ripr/reports/gate-decision.json"));
        assert!(workflow.contains("target/ripr/reports/gate-decision.md"));
        assert!(workflow.contains("target/ripr/reports/baseline-debt-delta.json"));
        assert!(workflow.contains("target/ripr/reports/baseline-debt-delta.md"));
        assert!(workflow.contains("target/ripr/reports/ripr-zero-status.json"));
        assert!(workflow.contains("target/ripr/reports/ripr-zero-status.md"));
        assert!(workflow.contains("target/ripr/reports/pr-evidence-ledger.json"));
        assert!(workflow.contains("target/ripr/reports/pr-evidence-ledger.md"));
        assert!(workflow.contains("target/ripr/reports/waiver-aging.json"));
        assert!(workflow.contains("target/ripr/reports/waiver-aging.md"));
        assert!(workflow.contains("target/ripr/reports/suppression-health.json"));
        assert!(workflow.contains("target/ripr/reports/suppression-health.md"));
        assert!(workflow.contains("target/ripr/reports/policy-readiness.json"));
        assert!(workflow.contains("target/ripr/reports/policy-readiness.md"));
        assert!(workflow.contains("target/ripr/reports/policy-operations.json"));
        assert!(workflow.contains("target/ripr/reports/policy-operations.md"));
        assert!(workflow.contains("target/ripr/reports/policy-history.json"));
        assert!(workflow.contains("target/ripr/reports/policy-history.md"));
        assert!(workflow.contains("target/ripr/reports/policy-promotion-visible-only.json"));
        assert!(workflow.contains("target/ripr/reports/policy-promotion-visible-only.md"));
        assert!(workflow.contains("target/ripr/reports/policy-promotion-acknowledgeable.json"));
        assert!(workflow.contains("target/ripr/reports/policy-promotion-acknowledgeable.md"));
        assert!(workflow.contains("target/ripr/reports/policy-promotion-baseline-check.json"));
        assert!(workflow.contains("target/ripr/reports/policy-promotion-baseline-check.md"));
        assert!(workflow.contains("target/ripr/reports/policy-promotion-calibrated-gate.json"));
        assert!(workflow.contains("target/ripr/reports/policy-promotion-calibrated-gate.md"));
        assert!(workflow.contains(
            "target/ripr/reports/preview-promotion-${language}-${class_label//_/-}.json"
        ));
        assert!(
            workflow.contains(
                "target/ripr/reports/preview-promotion-${language}-${class_label//_/-}.md"
            )
        );
        assert!(
            workflow.contains("target/ripr/reports/preview-promotion-typescript-boundary-gap.md")
        );
        assert!(workflow.contains("target/ripr/reports/preview-promotion-python-boundary-gap.md"));
        assert!(workflow.contains("target/ripr/reports/test-oracle-assistant-proof.json"));
        assert!(workflow.contains("target/ripr/reports/test-oracle-assistant-proof.md"));
        assert!(workflow.contains("target/ripr/reports/assistant-loop-health.json"));
        assert!(workflow.contains("target/ripr/reports/assistant-loop-health.md"));
        assert!(workflow.contains("target/ripr/reports/gap-decision-ledger.json"));
        assert!(workflow.contains("target/ripr/reports/gap-decision-ledger.md"));
        assert!(workflow.contains("target/ripr/reports/first-useful-action.json"));
        assert!(workflow.contains("target/ripr/reports/first-useful-action.md"));
        assert!(workflow.contains("target/ripr/reports/pr-review-front-panel.json"));
        assert!(workflow.contains("target/ripr/reports/pr-review-front-panel.md"));
        assert!(workflow.contains("target/ripr/reports/start-here.json"));
        assert!(workflow.contains("target/ripr/reports/start-here.md"));
        assert!(workflow.contains("target/ripr/reports/index.json"));
        assert!(workflow.contains("target/ripr/reports/index.md"));
        assert!(workflow.contains("target/ci/labels.json"));
        assert!(workflow.contains("target/ripr/review/comments.json"));
        assert!(workflow.contains("target/ripr/pr/check.json"));
        assert!(workflow.contains("ripr check \\"));
        assert!(workflow.contains("--check-output target/ripr/pr/check.json"));
        assert!(workflow.contains("target/ripr/review/existing-comments.json"));
        assert!(workflow.contains("target/ripr/review/comment-publish-plan.json"));
        assert!(workflow.contains("target/ripr/review/comment-publish-plan.md"));
        assert!(workflow.contains("target/ripr/review"));
        assert!(workflow.contains("target/ci"));
        assert!(workflow.contains("name: Capture existing RIPR inline comments"));
        assert!(workflow.contains("name: Plan RIPR inline comments"));
        assert!(workflow.contains("name: Publish RIPR inline comments"));
        assert!(workflow.contains("name: Capture RIPR gate labels"));
        assert!(workflow.contains("name: Evaluate RIPR gate decision"));
        assert!(workflow.contains("name: Render RIPR baseline debt delta"));
        assert!(workflow.contains("name: Emit RIPR PR guidance annotations"));
        assert!(workflow.contains("name: Render RIPR waiver aging"));
        assert!(workflow.contains("name: Render RIPR suppression health"));
        assert!(workflow.contains("name: Render RIPR policy readiness"));
        assert!(workflow.contains("name: Render RIPR policy operations"));
        assert!(workflow.contains("name: Render RIPR policy history"));
        assert!(workflow.contains("name: Render RIPR policy promotion packets"));
        assert!(workflow.contains("name: Render RIPR preview promotion packets"));
        assert!(workflow.contains("name: Render RIPR test-oracle assistant proof"));
        assert!(workflow.contains("name: Render RIPR assistant loop health"));
        assert!(workflow.contains("name: Render RIPR first useful action"));
        assert!(workflow.contains("name: Render RIPR PR review front panel"));
        assert!(workflow.contains("name: Render RIPR first-pr start-here"));
        assert!(workflow.contains("name: Render RIPR report packet index"));
        assert!(workflow.contains("def escape_data:"));
        assert!(workflow.contains("def escape_property:"));
        assert!(!workflow.contains("@tsv"));
        assert!(!workflow.contains("escape_github_property()"));
        assert!(workflow.contains(
            r#"::warning file=\(.placement.path | escape_property),line=\(.placement.line | tostring | escape_property)"#
        ));
        assert!(workflow.contains("title=RIPR targeted test guidance::"));
        assert!(workflow.contains("name: Add RIPR advisory summary"));
        assert!(workflow.contains("## RIPR advisory summary"));
        assert!(workflow.contains("### Start here"));
        assert!(workflow.contains("#### First-run status"));
        assert!(workflow.contains("Changed behavior"));
        assert!(workflow.contains("Current evidence strength"));
        assert!(workflow.contains("Missing discriminator"));
        assert!(workflow.contains("Focused proof intent"));
        assert!(workflow.contains("### Language preview grouping"));
        assert!(workflow.contains("### PR review summary"));
        assert!(workflow.contains("#### PR review at a glance"));
        assert!(workflow.contains("### Recommended next test"));
        assert!(workflow.contains("#### Recommended next test at a glance"));
        assert!(workflow.contains("### Top recommendation"));
        assert!(workflow.contains("### Artifact packet"));
        assert!(workflow.contains("### Uploaded review artifacts"));
        assert!(workflow.contains("#### Uploaded artifacts at a glance"));
        assert!(workflow.contains("### Gate decision"));
        assert!(workflow.contains("#### Gate decision at a glance"));
        assert!(workflow.contains("### Baseline debt delta"));
        assert!(workflow.contains("#### Baseline debt movement"));
        assert!(workflow.contains("### RIPR Zero status"));
        assert!(workflow.contains("#### RIPR Zero at a glance"));
        assert!(workflow.contains("### PR evidence ledger"));
        assert!(workflow.contains("#### PR movement at a glance"));
        assert!(workflow.contains("### Policy readiness"));
        assert!(workflow.contains("#### Policy readiness at a glance"));
        assert!(workflow.contains("### Policy operations"));
        assert!(workflow.contains("#### Policy operations at a glance"));
        assert!(workflow.contains("### Policy history"));
        assert!(workflow.contains("#### Policy history at a glance"));
        assert!(workflow.contains("### Policy promotion packets"));
        assert!(workflow.contains("### Preview promotion packets"));
        assert!(workflow.contains("### Waiver aging"));
        assert!(workflow.contains("#### Waiver aging at a glance"));
        assert!(workflow.contains("### Suppression health"));
        assert!(workflow.contains("#### Suppression health at a glance"));
        assert!(workflow.contains("### Test-oracle assistant proof"));
        assert!(workflow.contains("#### Assistant proof at a glance"));
        assert!(workflow.contains("### Agent proof status"));
        assert!(workflow.contains("#### Agent proof status at a glance"));
        assert!(workflow.contains("markdown_inline()"));
        assert!(workflow.contains("Active PR labels"));
        assert!(workflow.contains("Applied waiver label"));
        assert!(workflow.contains("Baseline artifact"));
        assert!(workflow.contains("Recommendation calibration"));
        assert!(workflow.contains("Mutation calibration"));
        assert!(workflow.contains("Blocking reason"));
        assert!(workflow.contains("Gate artifacts"));
        assert!(workflow.contains("Baseline delta artifacts"));
        assert!(workflow.contains("Policy readiness artifacts"));
        assert!(workflow.contains("Policy operations artifacts"));
        assert!(workflow.contains("Policy history artifacts"));
        assert!(workflow.contains("Promotion packet artifacts"));
        assert!(workflow.contains("Preview promotion artifacts"));
        assert!(workflow.contains("Waiver-aging artifacts"));
        assert!(workflow.contains("Suppression-health artifacts"));
        assert!(workflow.contains("Proof artifacts"));
        assert!(workflow.contains("Action artifacts"));
        assert!(workflow.contains("Front-panel artifacts"));
        assert!(workflow.contains("Index artifacts"));
        assert!(workflow.contains("### SARIF and badge status"));
        assert!(workflow.contains("### PR guidance annotations"));
        assert!(workflow.contains("### PR inline comments"));
        assert!(workflow.contains("### Known limits"));
        assert!(workflow.contains("Inline comments are disabled by default"));
        assert!(!workflow.contains("fail-on-new-warning"));
        assert!(!workflow.contains("pull_request_target"));
        assert!(!workflow.contains("RIPR_GATE_MODE: \"acknowledgeable\""));
        assert!(!workflow.contains("RIPR_GATE_MODE: \"baseline-check\""));
        assert!(!workflow.contains("RIPR_GATE_MODE: \"calibrated-gate\""));
    }

    #[test]
    fn init_generated_github_workflow_never_auto_refreshes_baseline() {
        let workflow = generated_github_actions_workflow();
        let baseline_delta = workflow_step(&workflow, "Render RIPR baseline debt delta");
        assert!(baseline_delta.contains("ripr baseline diff"));
        assert!(baseline_delta.contains("continue-on-error: true"));
        assert!(!workflow.contains("ripr baseline update"));
        assert!(!workflow.contains("--remove-resolved"));
        assert!(!workflow.contains("--adopt-new"));
        assert!(!workflow.contains("--out .ripr/gate-baseline.json"));
    }

    #[test]
    fn init_generated_github_workflow_uploads_reports_and_makes_sarif_optional() {
        let workflow = generated_github_actions_workflow();
        assert!(workflow.contains("name: RIPR advisory reports"));
        assert!(workflow.contains("target/ripr/pilot"));
        assert!(workflow.contains("target/ripr/agent"));
        assert!(workflow.contains("target/ripr/workflow"));
        assert!(workflow.contains("target/ripr/reports"));
        assert!(workflow.contains("target/ripr/review"));
        assert!(workflow.contains("target/ci"));
        assert!(workflow.contains("name: ripr-reports"));
        assert!(workflow.contains("RIPR_TOP_SEAM_ID"));
        assert!(workflow.contains(".top_actionable_seams[0].seam_id"));
        assert!(!workflow.contains(".top_seams[0].seam_id"));
        // An adopter repository has no xtask; the workflow must not call it.
        assert!(!workflow.contains("cargo xtask"));
        assert!(workflow.contains("cat target/ripr/pilot/pilot-summary.md"));
        assert!(workflow.contains("cat target/ripr/workflow/agent-review-summary.md"));
        assert!(workflow.contains("repo-ripr-badge.json"));
        assert!(workflow.contains("repo-ripr-badge-shields.json"));
        assert!(workflow.contains(".summary.comments // 0"));
        assert!(workflow.contains(".summary.summary_only // 0"));
        assert!(workflow.contains(".summary.suppressed // 0"));
        assert!(workflow.contains(".summary.unknown_confidence // 0"));
        assert!(workflow.contains(".inputs.labels // []"));
        assert!(workflow.contains(".policy.acknowledgement_labels // []"));
        assert!(workflow.contains(".policy.acknowledgement_label"));
        assert!(workflow.contains(".inputs.baseline // \"not supplied\""));
        assert!(workflow.contains(".inputs.recommendation_calibration // \"not supplied\""));
        assert!(workflow.contains(".inputs.mutation_calibration // \"not supplied\""));
        assert!(workflow.contains(".evidence.recommendation_calibration.confidence_effect"));
        assert!(workflow.contains(".evidence.mutation_calibration.confidence_effect"));
        assert!(workflow.contains(".gate_reason"));
        assert!(workflow.contains("blocking=\"$(markdown_inline \"$blocking\")\""));
        assert!(workflow.contains("Counts: blocking=\\`$blocking\\`"));
        assert!(workflow.contains(".delta.still_present // 0"));
        assert!(workflow.contains(".delta.resolved // 0"));
        assert!(workflow.contains(".delta.new_policy_eligible // 0"));
        assert!(workflow.contains(".delta.acknowledged // 0"));
        assert!(workflow.contains(".delta.suppressed // 0"));
        assert!(workflow.contains(".delta.stale_baseline_entry // 0"));
        assert!(workflow.contains(".delta.invalid_baseline_entry // 0"));
        assert!(workflow.contains(".delta.missing_current_input // 0"));
        assert!(workflow.contains(".delta.legacy_fallback_match // 0"));
        assert!(workflow.contains("legacy_fallback=\\`$legacy_fallback_match\\`"));
        assert!(workflow.contains("Counts: still_present=\\`$still_present\\`"));
        assert!(workflow.contains(".movement.new_policy_eligible // 0"));
        assert!(workflow.contains(".movement.baseline_still_present // 0"));
        assert!(workflow.contains(".movement.baseline_resolved // 0"));
        assert!(workflow.contains(".movement.acknowledged // 0"));
        assert!(workflow.contains(".movement.suppressed // 0"));
        assert!(workflow.contains(".movement.blocking_candidates // 0"));
        assert!(workflow.contains(".movement.visible_unresolved // 0"));
        assert!(workflow.contains(".coverage_grip_frontier.status // \"not_available\""));
        assert!(workflow.contains(".history.trend // \"not_available\""));
        assert!(workflow.contains("Counts: new_policy_eligible=\\`$ledger_new_policy_eligible\\`"));
        // F60-4: counts with no baseline delta or RIPR Zero status behind them
        // print as not measured, not as zeros.
        assert!(workflow.contains(".movement.count_source // \"unknown\""));
        assert!(
            workflow
                .contains("gap counts not measured (no baseline debt delta or RIPR Zero status)")
        );
        assert!(workflow.contains("sed 's/`/\\\\`/g'"));
        assert!(workflow.contains("Blocking reason (\\`$blocking\\`): \\`$blocking_reason\\`"));
        assert!(workflow.contains("Boundary: $limits_note"));
        assert!(workflow.contains("Pass/fail authority remains \\`ripr gate evaluate\\`"));
        assert!(workflow.contains("cat target/ripr/reports/pr-evidence-ledger.md"));
        assert!(workflow.contains("Set `RIPR_GATE_BASELINE`"));
        assert!(workflow.contains("RIPR_GATE_MODE"));
        assert!(workflow.contains("RIPR_GATE_BASELINE"));
        assert!(workflow.contains("RIPR_COMMENT_MODE"));
        assert!(workflow.contains("existing-comments.raw.json"));
        assert!(workflow.contains("<!-- ripr:dedupe="));
        assert!(workflow.contains("--mode \"$RIPR_COMMENT_MODE\""));
        assert!(workflow.contains("--existing-comments target/ripr/review/existing-comments.json"));
        assert!(workflow.contains("--token-available"));
        assert!(workflow.contains("--write-permission"));
        assert!(workflow.contains("jq -e '.summary.safe_to_publish == true'"));
        assert!(workflow.contains("gh api --method POST"));
        assert!(workflow.contains("gh api --method PATCH"));
        assert!(workflow.contains("assistant-loop proof"));
        assert!(workflow.contains("first-action"));
        assert!(workflow.contains("--pr-guidance target/ripr/review/comments.json"));
        assert!(workflow.contains("--agent-packet target/ripr/workflow/agent-brief.json"));
        assert!(workflow.contains("--before target/ripr/workflow/before.repo-exposure.json"));
        assert!(workflow.contains("--after target/ripr/workflow/after.repo-exposure.json"));
        assert!(workflow.contains("--receipt target/ripr/reports/agent-receipt.json"));
        assert!(workflow.contains("--ledger target/ripr/reports/pr-evidence-ledger.json"));
        assert!(
            workflow
                .contains("--coverage-frontier target/ripr/reports/coverage-grip-frontier.json")
        );
        assert!(workflow.contains("--gate-decision target/ripr/reports/gate-decision.json"));
        assert!(workflow.contains("pr-review front-panel"));
        assert!(workflow.contains("reports index"));
        assert!(workflow.contains("front_panel_has_input=true"));
        assert!(workflow.contains("--first-action target/ripr/reports/first-useful-action.json"));
        assert!(
            workflow.contains("--assistant-health target/ripr/reports/assistant-loop-health.json")
        );
        assert!(workflow.contains("--ledger target/ripr/reports/pr-evidence-ledger.json"));
        assert!(workflow.contains("--baseline-delta target/ripr/reports/baseline-debt-delta.json"));
        assert!(workflow.contains("--zero-status target/ripr/reports/ripr-zero-status.json"));
        assert!(
            workflow
                .contains("--mutation-calibration target/ripr/reports/mutation-calibration.json")
        );
        assert!(workflow.contains("--receipt target/ripr/reports/agent-receipt.json"));
        assert!(workflow.contains("ripr \"${gate_args[@]}\""));
        assert!(workflow.contains("ripr \"${proof_args[@]}\""));
        assert!(workflow.contains("ripr \"${first_action_args[@]}\""));
        assert!(workflow.contains("ripr \"${front_panel_args[@]}\""));
        assert!(workflow.contains("ripr reports index"));
        assert!(workflow.contains("index_has_input=true"));
        assert!(workflow.contains("Set `RIPR_GATE_MODE`"));
        assert!(workflow.contains("No runtime mutation execution is performed"));
        // The RIPR-source-tree-only cockpit step is gone from the adopter
        // workflow (F60-8).
        assert!(!workflow.contains("hashFiles('xtask/src/reports/operator.rs')"));
        assert!(workflow.contains("if: env.RIPR_UPLOAD_SARIF == 'true'"));
        assert!(workflow.contains(
            "if: env.RIPR_UPLOAD_SARIF == 'true' && github.event_name == 'pull_request'"
        ));
    }

    #[test]
    fn init_generated_github_workflow_names_cockpit_repair_commands() {
        let workflow = generated_github_actions_workflow();

        let first_action = workflow_step(&workflow, "Render RIPR first useful action");
        assert!(first_action.contains(
            "Safe next action: run `ripr first-action --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/first-useful-action.json --out-md target/ripr/reports/first-useful-action.md`"
        ));

        let front_panel = workflow_step(&workflow, "Render RIPR PR review front panel");
        assert!(front_panel.contains(
            "Safe next action: run `ripr pr-review front-panel --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/pr-review-front-panel.json --out-md target/ripr/reports/pr-review-front-panel.md`"
        ));

        let first_pr = workflow_step(&workflow, "Render RIPR first-pr start-here");
        assert!(first_pr.contains("ripr first-pr"));
        assert!(first_pr.contains("--gap-ledger target/ripr/reports/gap-decision-ledger.json"));
        assert!(first_pr.contains("--out-dir target/ripr/reports"));

        let packet_index = workflow_step(&workflow, "Render RIPR report packet index");
        assert!(packet_index.contains(
            "Regenerate command: `ripr reports index --root . --reports-dir target/ripr/reports --review-dir target/ripr/review --receipts-dir target/ripr/receipts --workflow-dir target/ripr/workflow --agent-dir target/ripr/agent --pilot-dir target/ripr/pilot --ci-dir target/ci --out target/ripr/reports/index.json --out-md target/ripr/reports/index.md`."
        ));

        let summary = workflow_step(&workflow, "Add RIPR advisory summary");
        assert!(summary.contains("### Start here"));
        assert!(summary.contains("Open `target/ripr/reports/start-here.md` first when it exists."));
        assert!(summary.contains(
            "Then open `target/ripr/reports/index.md` to navigate deeper evidence artifacts."
        ));
        assert!(summary.contains(
            "Safe next action: repair one named gap, regenerate missing or malformed artifacts"
        ));
        assert!(summary.contains(
            "Recovery states: missing artifact, stale evidence, wrong root, malformed artifact, no actionable gap, and preview-limited evidence"
        ));
        assert!(summary.contains(
            "Proof rail: the repair start, verify, receipt, and receipt path are static movement evidence only; verify and receipt run after the test edit."
        ));
        assert!(summary.contains(
            "Preview boundary: preview-limited evidence stays syntax-first and advisory"
        ));
        assert!(summary.contains(
            "Gate authority: `ripr gate evaluate` remains the pass/fail source only when `RIPR_GATE_MODE` is configured."
        ));
        assert!(summary.contains("Start-here artifact: `target/ripr/reports/start-here.md`"));
        assert!(summary.contains(".summary.start_here // \"not_available\""));
        assert!(summary.contains("Start-here artifact: \\`$start_here_path\\`"));
        assert!(
            summary.contains("Start-here artifact: `target/ripr/reports/pr-review-front-panel.md`")
        );
        assert!(summary.contains("Start-here artifact: `target/ripr/pilot/pilot-summary.md`"));
        assert!(summary.contains("#### First-run status"));
        assert!(summary.contains(".status // \"unknown\""));
        assert!(summary.contains(".selected.state // \"unknown\""));
        assert!(summary.contains(".selected.canonical_gap_id // .selected.gap_id"));
        assert!(summary.contains(".selected.language + \" (\""));
        assert!(summary.contains(".selected.changed_behavior // \"not_available\""));
        assert!(summary.contains(".selected.static_evidence_boundary // \"static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval.\""));
        assert!(summary.contains(
            ".selected.missing_discriminator // .selected.repair.suggested_assertion // \"not_available\""
        ));
        assert!(summary.contains(
            ".selected.focused_proof_intent // .selected.repair.suggested_assertion // \"not_available\""
        ));
        assert!(summary.contains(".selected.repair.target_file // \"not_available\""));
        assert!(summary.contains(".selected.repair.related_test // \"not_available\""));
        assert!(summary.contains(".selected.static_limit_kind"));
        assert!(summary.contains(".selected.receipt_path // \"not_available\""));
        assert!(summary.contains(".selected.receipt_state // \"receipt_missing\""));
        assert!(summary.contains("Canonical gap: \\`$start_gap\\`"));
        assert!(summary.contains("Language: \\`$start_language\\`"));
        assert!(summary.contains("Changed behavior: \\`$start_changed\\`"));
        assert!(summary.contains("Missing discriminator: \\`$start_missing\\`"));
        assert!(summary.contains("Focused proof intent: \\`$start_focused\\`"));
        assert!(summary.contains("Boundary: \\`$start_boundary\\`"));
        assert!(summary.contains("Repair target: \\`$start_target\\`"));
        assert!(summary.contains("Related test: \\`$start_related\\`"));
        assert!(summary.contains("Static limit: \\`$start_limit\\`"));
        // #3906 (F60-3, F60-14): a carried repair start leads the block and
        // the low-level pair becomes the manual alternative.
        assert!(summary.contains(".selected.repair_command // empty"));
        assert!(summary.contains("echo \"- Start repair: \\`$start_repair_command\\`\""));
        // The labels are the shared first-pr constants, substituted into
        // the workflow so one owner holds the text.
        assert!(summary.contains(&format!(
            "echo '- {REPAIR_AFTER_PHASE_LABEL}: {REPAIR_AFTER_PHASE_STEP}'"
        )));
        assert!(summary.contains(&format!("start_verify_label='{MANUAL_VERIFY_LABEL}'")));
        assert!(summary.contains(&format!("start_verify_label='{VERIFY_AFTER_EDIT_LABEL}'")));
        assert!(summary.contains(&format!(
            "start_receipt_label='{RECEIPT_AFTER_VERIFY_LABEL}'"
        )));
        assert!(
            !summary.contains("@RIPR_"),
            "unsubstituted workflow placeholder"
        );
        assert!(summary.contains("- $start_verify_label: \\`$start_verify\\`"));
        assert!(summary.contains("- $start_receipt_label: \\`$start_receipt\\`"));
        let lead = summary
            .find("echo \"- Start repair: \\`$start_repair_command\\`\"")
            .unwrap_or(usize::MAX);
        let status = summary
            .find("echo \"- Status: \\`$start_status\\`\"")
            .unwrap_or(0);
        assert!(
            lead < status,
            "the repair start must lead the first-run block"
        );
        assert!(summary.contains("Receipt path: \\`$start_receipt_path\\`"));
        assert!(summary.contains("Receipt state: \\`$start_receipt_state\\`"));
        assert!(summary.contains("Safe next action command: \\`$start_next\\`"));
        assert!(
            summary
                .contains(".selected.repair_command // .selected.next_command // .selected.regeneration_command // \"none\"")
        );
        assert!(summary.contains(".action_kind // \"unknown\""));
        assert!(summary.contains(".commands.context_packet // \"not_available\""));
        // #3906: the first-action summary surfaces the carried repair start.
        assert!(summary.contains(".commands.repair // \"not_available\""));
        assert!(summary.contains("Repair start: \\`$first_repair\\`"));
        assert!(summary.contains("Repair start: \\`$action_repair\\`"));
        // #3906 (F60-14): in both first-action blocks the carried repair
        // start leads, followed by its after phase; verify and receipt are
        // the manual alternative, else steps after the test edit.
        for (prefix, status_line) in [
            ("first", "echo \"- Status: \\`$first_status\\`\""),
            ("action", "echo \"- Status: \\`$action_status\\`\""),
        ] {
            let guard = format!(
                "if [ \"${prefix}_repair\" != not_available ] && [ \"${prefix}_repair\" != unknown ]; then"
            );
            let lead = format!("echo \"- Repair start: \\`${prefix}_repair\\`\"");
            assert!(summary.contains(&guard), "{prefix} guard");
            let lead_at = summary.find(&lead).unwrap_or(usize::MAX);
            let status_at = summary.find(status_line).unwrap_or(0);
            assert!(lead_at < status_at, "{prefix} repair start must lead");
            assert!(summary.contains(&format!("{prefix}_verify_label='{MANUAL_VERIFY_LABEL}'")));
            assert!(summary.contains(&format!("{prefix}_receipt_label='{MANUAL_RECEIPT_LABEL}'")));
            assert!(summary.contains(&format!(
                "{prefix}_verify_label='{VERIFY_AFTER_EDIT_LABEL}'"
            )));
            assert!(summary.contains(&format!(
                "{prefix}_receipt_label='{RECEIPT_AFTER_VERIFY_LABEL}'"
            )));
            assert!(summary.contains(&format!(
                "echo \"- ${prefix}_verify_label: \\`${prefix}_verify\\`\""
            )));
        }
        for prefix in ["panel", "ledger"] {
            assert!(summary.contains(&format!(
                "if [ \"${prefix}_repair\" != not_available ] && [ \"${prefix}_repair\" != unknown ]; then\n"
            )));
            assert!(summary.contains(&format!("echo \"- Repair start: \\`${prefix}_repair\\`\"")));
            assert!(summary.contains(&format!("{prefix}_verify_label='{MANUAL_VERIFY_LABEL}'")));
            assert!(summary.contains(&format!(
                "echo \"- ${prefix}_verify_label: \\`${prefix}_verify\\`\""
            )));
        }
        assert!(!summary.contains("Verify command: \\`$first_verify\\`"));
        assert!(!summary.contains("Verify command: \\`$action_verify\\`"));
        assert!(summary.contains("missing_start_here"));
        assert!(summary.contains("State: \\`missing_artifact\\`"));
        assert!(summary.contains(
            "Safe next action: run \\`ripr first-pr --root . --base origin/${{ github.base_ref || github.event.repository.default_branch }} --head HEAD --gap-ledger target/ripr/reports/gap-decision-ledger.json"
        ));
        assert!(summary.contains(
            "start-here is advisory first-run guidance only; gate decision remains separate pass/fail authority"
        ));
        assert!(summary.contains(
            "ripr first-pr --root . --base origin/${{ github.base_ref || github.event.repository.default_branch }} --head HEAD --gap-ledger target/ripr/reports/gap-decision-ledger.json"
        ));
        assert!(summary.contains(
            "Fallback safe next action: run \\`ripr first-action --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/first-useful-action.json --out-md target/ripr/reports/first-useful-action.md\\`"
        ));
        assert!(summary.contains(
            "Safe next action: run `ripr pr-review front-panel --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/pr-review-front-panel.json --out-md target/ripr/reports/pr-review-front-panel.md`"
        ));
        assert!(summary.contains(
            "Regenerate command: `ripr reports index --root . --reports-dir target/ripr/reports --review-dir target/ripr/review --receipts-dir target/ripr/receipts --workflow-dir target/ripr/workflow --agent-dir target/ripr/agent --pilot-dir target/ripr/pilot --ci-dir target/ci --out target/ripr/reports/index.json --out-md target/ripr/reports/index.md`."
        ));
    }

    #[test]
    fn init_generated_github_workflow_groups_preview_languages_only_when_configured() {
        let workflow = generated_github_actions_workflow();
        let summary = workflow_step(&workflow, "Add RIPR advisory summary");

        // The generated workflow consumes the typed doctor JSON surface
        // (#2072), not the human "Enabled languages:" line.
        assert!(summary.contains("ripr doctor --root . --json"));
        assert!(summary.contains("jq -r '.languages | join(\",\")'"));
        assert!(summary.contains("sed -n '/^typescript$/p; /^python$/p'"));
        assert!(!summary.contains("sed -n 's/^- Enabled languages: //p'"));
        assert!(summary.contains("|| true"));

        // The preview-promotion detection site uses the same typed surface
        // and discloses a doctor failure instead of claiming "none
        // configured" (#2182 review).
        let packets = workflow_step(&workflow, "Render RIPR preview promotion packets");
        assert!(packets.contains("ripr doctor --root . --json"));
        assert!(packets.contains("jq -r '.languages[]?'"));
        assert!(!packets.contains("sed -n 's/^- Enabled languages: //p'"));
        assert!(packets.contains("Language detection via `ripr doctor --json` failed"));
        assert!(!packets.contains("No TypeScript or Python preview languages are configured; preview promotion packets were not generated.'\n            fi") || packets.contains("if ripr doctor --root . --json > /dev/null 2>&1"));
        assert!(summary.contains("target/ripr/reports/repo-exposure.json"));
        assert!(summary.contains("target/ripr/pilot/repo-exposure.json"));
        assert!(summary.contains(".language_status? != \"preview\""));
        assert!(summary.contains("configured preview/advisory"));
        assert!(summary.contains("artifact_entries=\\`$artifact_entries\\`"));
        assert!(summary.contains("grouped_preview_languages=\"$preview_languages\""));
        assert!(
            summary.contains("grouped_preview_languages=\"$grouped_preview_languages javascript\"")
        );
        assert!(summary.contains("Grouped preview evidence languages"));
        assert!(summary.contains("for language in $grouped_preview_languages; do"));
        assert!(summary.contains("actionability_states"));
        assert!(summary.contains("actionability_categories"));
        assert!(summary.contains("repair_packet_ready_entries"));
        assert!(summary.contains("gate_impact=\\`none\\`"));
        assert!(summary.contains(
            "preview-language groups are advisory presentation only; \\`ripr gate evaluate\\` remains pass/fail authority"
        ));

        let guard = summary
            .find("if [ -n \"$preview_languages\" ]; then")
            .unwrap_or(usize::MAX);
        let grouping = summary
            .find("echo '### Language preview grouping'")
            .unwrap_or(usize::MAX);
        let pr_review = summary
            .find("echo '### PR review summary'")
            .unwrap_or(usize::MAX);
        assert_ne!(guard, usize::MAX, "missing language grouping guard");
        assert_ne!(grouping, usize::MAX, "missing language grouping heading");
        assert_ne!(pr_review, usize::MAX, "missing PR review heading");
        assert!(
            guard < grouping && grouping < pr_review,
            "language grouping must stay opt-in and before the PR review summary"
        );
    }

    #[test]
    fn init_generated_github_workflow_matches_smoke_fixture() {
        let workflow = generated_github_actions_workflow();
        let fixture = generated_workflow_smoke_fixture();

        assert!(workflow.contains("RIPR_UPLOAD_SARIF: \"true\""));
        // Install caching (#2008): the install names an exact version, so a
        // cached binary is reused only when it is that version.
        // Pinned to a SHA, not the mutable v2 tag (#2190 review).
        assert!(workflow.contains("Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32"));
        assert!(!workflow.contains("Swatinem/rust-cache@v2"));
        assert!(workflow.contains("shared-key: ripr-install"));
        assert!(workflow.contains("RIPR_GATE_MODE: ${{ vars.RIPR_GATE_MODE || '' }}"));
        assert!(workflow.contains("actions/upload-artifact@v7"));
        assert!(workflow.contains("github/codeql-action/upload-sarif@v4"));
        assert_contains_all(&workflow, "command", fixture.commands);
        assert_contains_all(&workflow, "artifact path", fixture.artifact_paths);
        assert_contains_all(&workflow, "summary section", fixture.summary_sections);

        // Workflow hardening: the job token is not persisted into the
        // checkout that PR-controlled code runs in.
        assert!(
            workflow.contains("          fetch-depth: 0\n          persist-credentials: false\n")
        );
        // Checked-in or cache-restored files under target/ripr and target/ci
        // are removed after the cache restore and before any RIPR step, so
        // gate inputs read "when present" come only from this run.
        let cleanup = workflow_step(&workflow, "Remove checked-in RIPR artifacts");
        assert!(cleanup.contains("run: rm -rf target/ripr target/ci"));
        let cache_at = workflow.find("Swatinem/rust-cache@").unwrap_or(usize::MAX);
        let cleanup_at = workflow
            .find("      - name: Remove checked-in RIPR artifacts")
            .unwrap_or(0);
        assert!(
            cache_at < cleanup_at,
            "cleanup must follow the cache restore"
        );
        assert_step_before(
            &workflow,
            "Remove checked-in RIPR artifacts",
            "Install ripr",
        );
        assert_step_before(
            &workflow,
            "Remove checked-in RIPR artifacts",
            "Generate RIPR pilot packet",
        );
        // Only comments the workflow itself posted count as existing RIPR
        // comments; a marker from another author cannot suppress or be
        // PATCHed.
        let capture = workflow_step(&workflow, "Capture existing RIPR inline comments");
        assert!(
            capture.contains(
                r#"| select(.user.login == "github-actions[bot]" and .user.type == "Bot")"#
            )
        );
        // Repository-derived text printed to the log folds CR/LF so it
        // cannot open a line GitHub parses as a workflow command.
        let publish = workflow_step(&workflow, "Publish RIPR inline comments");
        assert!(publish.contains(
            r#"jq -r '.blocked[]? | "- \(.blocked_reason): \(.message)" | gsub("[\r\n]"; " ")'"#
        ));
        assert!(publish.contains(
            r#"dedupe_key="$(jq -r '.dedupe_key | tostring | gsub("[\r\n]"; " ")' <<< "$operation")""#
        ));
        assert!(publish.contains(
            r#"select(.operation == "keep") | .dedupe_key | tostring | gsub("[\r\n]"; " ")'"#
        ));
        assert!(!publish.contains("jq -r '.dedupe_key' "));
        assert!(!publish.contains(r#"| .dedupe_key' "$publishable""#));

        let prepare = workflow_step(&workflow, "Prepare RIPR editor-agent artifacts");
        assert!(prepare.contains("RIPR_TOP_SEAM_ID"));
        // first-pr checks the review cards were built for its base; the
        // cards use the PR's base, so first-pr must too (F60-8).
        let first_pr = workflow_step(&workflow, "Render RIPR first-pr start-here");
        assert!(first_pr.contains(
            "--base \"origin/${{ github.base_ref || github.event.repository.default_branch }}\""
        ));
        assert!(prepare.contains(".top_actionable_seams[0].seam_id"));
        assert!(
            !prepare.contains(".top_seams[0].seam_id"),
            "top seam extraction must use pilot-summary top_actionable_seams"
        );

        let agent_loop = workflow_step(&workflow, "Generate RIPR agent loop artifacts");
        assert!(agent_loop.contains("ripr agent start"));
        assert!(agent_loop.contains("ripr agent packet"));
        assert!(agent_loop.contains("cp target/ripr/workflow/agent-packet.json"));
        assert!(agent_loop.contains("cp target/ripr/workflow/agent-brief.json"));
        // A failed packet render must not leave an empty JSON behind.
        assert!(agent_loop.contains("> \"$packet_tmp\""));
        assert!(agent_loop.contains("mv \"$packet_tmp\" target/ripr/workflow/agent-packet.json"));
        // #3906 (F60-1): before and after would both be this HEAD, so
        // verify has no movement to compare and exits 2 on every run.
        // CI writes the before side only; the repair's after phase writes
        // the rest where the test edit happens.
        for post_edit in [
            "ripr check",
            "ripr agent verify",
            "ripr agent receipt",
            "ripr outcome",
            "after.repo-exposure.json",
            "analysis-outcome.json",
            "agent-verify.json",
            "agent-receipt.json",
        ] {
            assert!(
                !agent_loop.contains(post_edit),
                "agent-loop step must not run the post-edit step `{post_edit}`:\n{agent_loop}"
            );
        }

        let guidance = workflow_step(&workflow, "Run RIPR PR guidance report");
        assert!(guidance.contains("github.event_name == 'pull_request'"));
        assert!(guidance.contains("mkdir -p target/ripr/pr target/ripr/review"));
        assert!(guidance.contains("check_status=0"));
        assert!(guidance.contains(r#"ripr check \"#));
        assert!(guidance.contains(r#"--base "origin/${{ github.base_ref }}"#));
        assert!(guidance.contains("--format json > target/ripr/pr/check.json"));
        assert!(guidance.contains("|| check_status=$?"));
        assert!(guidance.contains("target/ripr/pr/check.json"));
        assert!(guidance.contains("ripr review-comments"));
        assert!(guidance.contains("--base \"origin/${{ github.base_ref }}\""));
        assert!(guidance.contains("--head HEAD"));
        assert!(guidance.contains("--out target/ripr/review/comments.json"));

        let existing_comments = workflow_step(&workflow, "Capture existing RIPR inline comments");
        assert!(existing_comments.contains("env.RIPR_COMMENT_MODE != 'off'"));
        assert!(existing_comments.contains("GH_TOKEN: ${{ github.token }}"));
        assert!(existing_comments.contains("gh api --paginate --slurp"));
        assert!(
            existing_comments.contains("pulls/${{ github.event.pull_request.number }}/comments")
        );
        assert!(existing_comments.contains("target/ripr/review/existing-comments.json"));
        assert!(
            existing_comments
                .contains("capture(\"<!-- ripr:dedupe=(?<key>.*?)(?: presentation=[^ ]+)? -->\")")
        );

        let comment_plan = workflow_step(&workflow, "Plan RIPR inline comments");
        assert!(comment_plan.contains("env.RIPR_COMMENT_MODE != 'off'"));
        assert!(comment_plan.contains("hashFiles('target/ripr/review/comments.json')"));
        assert!(comment_plan.contains("pr-comments plan"));
        assert!(comment_plan.contains("--pr-guidance target/ripr/review/comments.json"));
        assert!(comment_plan.contains("--mode \"$RIPR_COMMENT_MODE\""));
        assert!(comment_plan.contains("--event-name \"${{ github.event_name }}\""));
        assert!(
            comment_plan.contains("--pull-request \"${{ github.event.pull_request.number }}\"")
        );
        assert!(
            comment_plan
                .contains("--head-repo \"${{ github.event.pull_request.head.repo.full_name }}\"")
        );
        assert!(comment_plan.contains("--base-repo \"${{ github.repository }}\""));
        assert!(comment_plan.contains("--out target/ripr/review/comment-publish-plan.json"));
        assert!(comment_plan.contains("--out-md target/ripr/review/comment-publish-plan.md"));
        assert!(
            comment_plan.contains("--existing-comments target/ripr/review/existing-comments.json")
        );
        assert!(comment_plan.contains("--token-available"));
        assert!(comment_plan.contains("--no-token"));
        assert!(comment_plan.contains("--write-permission"));

        let publish_comments = workflow_step(&workflow, "Publish RIPR inline comments");
        assert!(publish_comments.contains("env.RIPR_COMMENT_MODE == 'inline'"));
        assert!(
            publish_comments.contains("hashFiles('target/ripr/review/comment-publish-plan.json')")
        );
        assert!(publish_comments.contains("jq -e '.summary.safe_to_publish == true'"));
        assert!(publish_comments.contains("select(.safe_to_publish == true)"));
        assert!(publish_comments.contains("published_body: compact_body"));
        assert!(publish_comments.contains("github.event.pull_request.head.sha"));
        assert!(publish_comments.contains("gh api --method POST"));
        assert!(publish_comments.contains("gh api --method PATCH"));
        assert_step_before(
            &workflow,
            "Run RIPR PR guidance report",
            "Capture existing RIPR inline comments",
        );
        assert_step_before(
            &workflow,
            "Capture existing RIPR inline comments",
            "Plan RIPR inline comments",
        );
        assert_step_before(
            &workflow,
            "Plan RIPR inline comments",
            "Publish RIPR inline comments",
        );
        assert_step_before(
            &workflow,
            "Plan RIPR inline comments",
            "Evaluate RIPR gate decision",
        );
        assert_step_before(
            &workflow,
            "Capture RIPR gate labels",
            "Evaluate RIPR gate decision",
        );
        assert_step_before(
            &workflow,
            "Evaluate RIPR gate decision",
            "Render RIPR baseline debt delta",
        );
        assert_step_before(
            &workflow,
            "Render RIPR baseline debt delta",
            "Render RIPR Zero status",
        );
        assert_step_before(
            &workflow,
            "Render RIPR Zero status",
            "Render RIPR PR evidence ledger",
        );
        assert_step_before(
            &workflow,
            "Render RIPR PR evidence ledger",
            "Render RIPR waiver aging",
        );
        assert_step_before(
            &workflow,
            "Render RIPR waiver aging",
            "Render RIPR suppression health",
        );
        assert_step_before(
            &workflow,
            "Render RIPR suppression health",
            "Render RIPR policy readiness",
        );
        assert_step_before(
            &workflow,
            "Render RIPR policy readiness",
            "Render RIPR policy operations",
        );
        assert_step_before(
            &workflow,
            "Render RIPR policy operations",
            "Render RIPR policy history",
        );
        assert_step_before(
            &workflow,
            "Render RIPR policy history",
            "Render RIPR policy promotion packets",
        );
        assert_step_before(
            &workflow,
            "Render RIPR policy promotion packets",
            "Render RIPR preview promotion packets",
        );
        assert_step_before(
            &workflow,
            "Render RIPR preview promotion packets",
            "Render RIPR test-oracle assistant proof",
        );
        assert_step_before(
            &workflow,
            "Render RIPR test-oracle assistant proof",
            "Render RIPR assistant loop health",
        );
        assert_step_before(
            &workflow,
            "Render RIPR assistant loop health",
            "Render RIPR first useful action",
        );
        assert_step_before(
            &workflow,
            "Render RIPR first useful action",
            "Render RIPR PR review front panel",
        );
        assert_step_before(
            &workflow,
            "Render RIPR PR review front panel",
            "Render RIPR report packet index",
        );
        assert_step_before(
            &workflow,
            "Render RIPR report packet index",
            "Render RIPR LLM work-loop summaries",
        );
        assert_step_before(
            &workflow,
            "Render RIPR PR evidence ledger",
            "Emit RIPR PR guidance annotations",
        );
        assert_step_before(
            &workflow,
            "Run RIPR PR guidance report",
            "Add RIPR advisory summary",
        );

        let artifact_upload = workflow_step(&workflow, "Upload RIPR report artifacts");
        assert!(artifact_upload.contains("if-no-files-found: ignore"));
        for path in [
            "target/ripr/pilot",
            "target/ripr/agent",
            "target/ripr/workflow",
            "target/ripr/reports",
            "target/ripr/review",
            "target/ci",
        ] {
            assert!(
                artifact_upload.contains(path),
                "artifact upload must include {path}"
            );
        }

        let gate = workflow_step(&workflow, "Evaluate RIPR gate decision");
        assert!(gate.contains("env.RIPR_GATE_MODE != ''"));
        assert!(gate.contains("hashFiles('target/ripr/review/comments.json')"));
        assert!(gate.contains("gate evaluate"));
        assert!(gate.contains("--pr-guidance target/ripr/review/comments.json"));
        assert!(gate.contains("--mode \"$RIPR_GATE_MODE\""));
        assert!(gate.contains("--out target/ripr/reports/gate-decision.json"));
        assert!(gate.contains("--out-md target/ripr/reports/gate-decision.md"));
        assert!(gate.contains("--labels-json target/ci/labels.json"));
        assert!(gate.contains("--sarif-policy target/ripr/reports/sarif-policy.json"));
        assert!(gate.contains(
            "--recommendation-calibration target/ripr/reports/recommendation-calibration.json"
        ));
        assert!(
            gate.contains("--mutation-calibration target/ripr/reports/mutation-calibration.json")
        );
        assert!(gate.contains("--baseline \"$RIPR_GATE_BASELINE\""));
        assert!(!gate.contains("continue-on-error: true"));

        let baseline_delta = workflow_step(&workflow, "Render RIPR baseline debt delta");
        assert!(baseline_delta.contains("always() && env.RIPR_GATE_BASELINE != ''"));
        assert!(baseline_delta.contains("hashFiles('target/ripr/reports/gate-decision.json')"));
        assert!(baseline_delta.contains("continue-on-error: true"));
        assert!(baseline_delta.contains("ripr baseline diff"));
        assert!(baseline_delta.contains("--baseline \"$RIPR_GATE_BASELINE\""));
        assert!(baseline_delta.contains("--current target/ripr/reports/gate-decision.json"));
        assert!(baseline_delta.contains("--out target/ripr/reports/baseline-debt-delta.json"));
        assert!(baseline_delta.contains("--out-md target/ripr/reports/baseline-debt-delta.md"));

        let zero_status = workflow_step(&workflow, "Render RIPR Zero status");
        assert!(zero_status.contains("hashFiles('target/ripr/reports/baseline-debt-delta.json')"));
        assert!(zero_status.contains("continue-on-error: true"));
        assert!(zero_status.contains("zero status"));
        assert!(zero_status.contains("--delta target/ripr/reports/baseline-debt-delta.json"));
        assert!(zero_status.contains("--out target/ripr/reports/ripr-zero-status.json"));
        assert!(zero_status.contains("--out-md target/ripr/reports/ripr-zero-status.md"));
        assert!(zero_status.contains("--baseline \"$RIPR_GATE_BASELINE\""));
        assert!(zero_status.contains("--gate target/ripr/reports/gate-decision.json"));
        assert!(zero_status.contains("--pr-guidance target/ripr/review/comments.json"));
        assert!(zero_status.contains(
            "--recommendation-calibration target/ripr/reports/recommendation-calibration.json"
        ));

        let pr_ledger = workflow_step(&workflow, "Render RIPR PR evidence ledger");
        assert!(pr_ledger.contains("github.event_name == 'pull_request'"));
        assert!(pr_ledger.contains("hashFiles('target/ripr/review/comments.json')"));
        assert!(pr_ledger.contains("continue-on-error: true"));
        assert!(pr_ledger.contains("pr-ledger record"));
        assert!(pr_ledger.contains("--pr-number \"${{ github.event.pull_request.number }}\""));
        assert!(pr_ledger.contains("--base \"origin/${{ github.base_ref }}\""));
        assert!(pr_ledger.contains("--head HEAD"));
        assert!(pr_ledger.contains("--pr-guidance target/ripr/review/comments.json"));
        assert!(pr_ledger.contains("--gate target/ripr/reports/gate-decision.json"));
        assert!(
            pr_ledger.contains("--baseline-delta target/ripr/reports/baseline-debt-delta.json")
        );
        assert!(pr_ledger.contains("--zero-status target/ripr/reports/ripr-zero-status.json"));
        assert!(pr_ledger.contains(
            "--recommendation-calibration target/ripr/reports/recommendation-calibration.json"
        ));
        assert!(pr_ledger.contains("--agent-receipt target/ripr/reports/agent-receipt.json"));
        assert!(pr_ledger.contains("--coverage target/ripr/reports/coverage-summary.json"));
        assert!(pr_ledger.contains("--history .ripr/pr-evidence-ledger.jsonl"));
        assert!(!pr_ledger.contains("--out-jsonl"));
        assert!(pr_ledger.contains("ledger_args+=(--label \"$label\")"));
        assert!(pr_ledger.contains("ripr \"${ledger_args[@]}\""));

        let waiver_aging = workflow_step(&workflow, "Render RIPR waiver aging");
        assert!(waiver_aging.contains("hashFiles('target/ripr/reports/pr-evidence-ledger.json')"));
        assert!(waiver_aging.contains("continue-on-error: true"));
        assert!(waiver_aging.contains("policy waiver-aging"));
        assert!(waiver_aging.contains("--root ."));
        assert!(waiver_aging.contains("--ledger target/ripr/reports/pr-evidence-ledger.json"));
        assert!(waiver_aging.contains("--out target/ripr/reports/waiver-aging.json"));
        assert!(waiver_aging.contains("--out-md target/ripr/reports/waiver-aging.md"));
        assert!(waiver_aging.contains("--history .ripr/pr-evidence-ledger.jsonl"));
        assert!(waiver_aging.contains("ripr \"${waiver_args[@]}\""));

        let suppression_health = workflow_step(&workflow, "Render RIPR suppression health");
        assert!(suppression_health.contains("if: always()"));
        assert!(suppression_health.contains("continue-on-error: true"));
        assert!(suppression_health.contains("policy suppression-health"));
        assert!(suppression_health.contains("--root ."));
        assert!(suppression_health.contains("--out target/ripr/reports/suppression-health.json"));
        assert!(suppression_health.contains("--out-md target/ripr/reports/suppression-health.md"));
        assert!(suppression_health.contains("ripr \"${suppression_args[@]}\""));

        let policy_readiness = workflow_step(&workflow, "Render RIPR policy readiness");
        assert!(policy_readiness.contains("if: always()"));
        assert!(policy_readiness.contains("continue-on-error: true"));
        assert!(policy_readiness.contains("policy readiness"));
        assert!(policy_readiness.contains("--root ."));
        assert!(policy_readiness.contains("--out target/ripr/reports/policy-readiness.json"));
        assert!(policy_readiness.contains("--out-md target/ripr/reports/policy-readiness.md"));
        assert!(
            policy_readiness.contains("--gate-decision target/ripr/reports/gate-decision.json")
        );
        assert!(
            policy_readiness
                .contains("--baseline-delta target/ripr/reports/baseline-debt-delta.json")
        );
        assert!(policy_readiness.contains(
            "--recommendation-calibration target/ripr/reports/recommendation-calibration.json"
        ));
        assert!(
            policy_readiness
                .contains("--mutation-calibration target/ripr/reports/mutation-calibration.json")
        );
        assert!(policy_readiness.contains("--waiver-aging target/ripr/reports/waiver-aging.json"));
        assert!(
            policy_readiness
                .contains("--suppression-health target/ripr/reports/suppression-health.json")
        );
        assert!(policy_readiness.contains("ripr \"${policy_args[@]}\""));

        let policy_operations = workflow_step(&workflow, "Render RIPR policy operations");
        assert!(
            policy_operations.contains("hashFiles('target/ripr/reports/policy-readiness.json')")
        );
        assert!(policy_operations.contains("continue-on-error: true"));
        assert!(policy_operations.contains("policy operations"));
        assert!(policy_operations.contains("--root ."));
        assert!(
            policy_operations
                .contains("--policy-readiness target/ripr/reports/policy-readiness.json")
        );
        assert!(policy_operations.contains("--out target/ripr/reports/policy-operations.json"));
        assert!(policy_operations.contains("--out-md target/ripr/reports/policy-operations.md"));
        assert!(policy_operations.contains("--waiver-aging target/ripr/reports/waiver-aging.json"));
        assert!(
            policy_operations
                .contains("--suppression-health target/ripr/reports/suppression-health.json")
        );
        assert!(
            policy_operations
                .contains("--baseline-delta target/ripr/reports/baseline-debt-delta.json")
        );
        assert!(
            policy_operations.contains("--gate-decision target/ripr/reports/gate-decision.json")
        );
        assert!(policy_operations.contains(
            "--recommendation-calibration target/ripr/reports/recommendation-calibration.json"
        ));
        assert!(
            policy_operations
                .contains("--mutation-calibration target/ripr/reports/mutation-calibration.json")
        );
        assert!(
            policy_operations.contains("--preview-boundary target/ripr/reports/repo-exposure.json")
        );
        assert!(policy_operations.contains("ripr \"${operations_args[@]}\""));

        let policy_history = workflow_step(&workflow, "Render RIPR policy history");
        assert!(policy_history.contains("hashFiles('target/ripr/reports/policy-operations.json')"));
        assert!(policy_history.contains("continue-on-error: true"));
        assert!(policy_history.contains("policy history"));
        assert!(policy_history.contains("--current target/ripr/reports/policy-operations.json"));
        assert!(policy_history.contains("--commit \"$(git rev-parse HEAD)\""));
        assert!(policy_history.contains("--history .ripr/policy-history.jsonl"));
        assert!(policy_history.contains("--pr-number \"${{ github.event.number }}\""));
        assert!(policy_history.contains("--out target/ripr/reports/policy-history.json"));
        assert!(policy_history.contains("--out-md target/ripr/reports/policy-history.md"));
        assert!(!policy_history.contains("--out-jsonl"));
        assert!(policy_history.contains("ripr \"${history_args[@]}\""));

        let promotion_packets = workflow_step(&workflow, "Render RIPR policy promotion packets");
        assert!(
            promotion_packets.contains("hashFiles('target/ripr/reports/policy-operations.json')")
        );
        assert!(promotion_packets.contains("continue-on-error: true"));
        assert!(promotion_packets.contains(
            "for target_mode in visible-only acknowledgeable baseline-check calibrated-gate"
        ));
        assert!(promotion_packets.contains("policy promote"));
        assert!(promotion_packets.contains("--to \"$target_mode\""));
        assert!(
            promotion_packets.contains("--operations target/ripr/reports/policy-operations.json")
        );
        assert!(promotion_packets.contains("--history target/ripr/reports/policy-history.json"));
        assert!(
            promotion_packets.contains("target/ripr/reports/policy-promotion-${target_mode}.json")
        );
        assert!(promotion_packets.contains("ripr \"${promotion_args[@]}\""));

        let preview_packets = workflow_step(&workflow, "Render RIPR preview promotion packets");
        assert!(preview_packets.contains("if: always()"));
        assert!(preview_packets.contains("continue-on-error: true"));
        assert!(preview_packets.contains("ripr doctor --root ."));
        assert!(preview_packets.contains("policy preview-promote"));
        assert!(preview_packets.contains("--language \"$language\""));
        assert!(preview_packets.contains("--class \"$class_label\""));
        assert!(preview_packets.contains(
            "target/ripr/reports/preview-promotion-${language}-${class_label//_/-}.json"
        ));
        assert!(
            preview_packets
                .contains("--evidence target/ripr/reports/preview-promotion-evidence.json")
        );
        assert!(preview_packets.contains("TypeScript or Python preview languages are configured"));
        assert!(preview_packets.contains("ripr \"${preview_args[@]}\""));

        let assistant_proof = workflow_step(&workflow, "Render RIPR test-oracle assistant proof");
        assert!(assistant_proof.contains("hashFiles('target/ripr/review/comments.json')"));
        assert!(assistant_proof.contains("hashFiles('target/ripr/workflow/agent-brief.json')"));
        assert!(
            assistant_proof.contains("hashFiles('target/ripr/workflow/before.repo-exposure.json')")
        );
        assert!(
            assistant_proof.contains("hashFiles('target/ripr/workflow/after.repo-exposure.json')")
        );
        assert!(assistant_proof.contains("hashFiles('target/ripr/reports/agent-receipt.json')"));
        assert!(
            assistant_proof.contains("hashFiles('target/ripr/reports/pr-evidence-ledger.json')")
        );
        assert!(assistant_proof.contains("continue-on-error: true"));
        assert!(assistant_proof.contains("assistant-loop proof"));
        assert!(assistant_proof.contains("--root ."));
        assert!(assistant_proof.contains("--pr-guidance target/ripr/review/comments.json"));
        assert!(assistant_proof.contains("--agent-packet target/ripr/workflow/agent-brief.json"));
        assert!(
            assistant_proof.contains("--before target/ripr/workflow/before.repo-exposure.json")
        );
        assert!(assistant_proof.contains("--after target/ripr/workflow/after.repo-exposure.json"));
        assert!(assistant_proof.contains("--receipt target/ripr/reports/agent-receipt.json"));
        assert!(assistant_proof.contains("--ledger target/ripr/reports/pr-evidence-ledger.json"));
        assert!(
            assistant_proof.contains("--out target/ripr/reports/test-oracle-assistant-proof.json")
        );
        assert!(
            assistant_proof.contains("--out-md target/ripr/reports/test-oracle-assistant-proof.md")
        );
        assert!(
            assistant_proof
                .contains("--coverage-frontier target/ripr/reports/coverage-grip-frontier.json")
        );
        assert!(assistant_proof.contains("--gate-decision target/ripr/reports/gate-decision.json"));
        assert!(assistant_proof.contains("ripr \"${proof_args[@]}\""));

        let assistant_health = workflow_step(&workflow, "Render RIPR assistant loop health");
        assert!(
            assistant_health
                .contains("hashFiles('target/ripr/reports/test-oracle-assistant-proof.json')")
        );
        assert!(assistant_health.contains("continue-on-error: true"));
        assert!(assistant_health.contains("assistant-loop health"));
        assert!(assistant_health.contains("--root ."));
        assert!(
            assistant_health
                .contains("--proof target/ripr/reports/test-oracle-assistant-proof.json")
        );
        assert!(assistant_health.contains("--out target/ripr/reports/assistant-loop-health.json"));
        assert!(assistant_health.contains("--out-md target/ripr/reports/assistant-loop-health.md"));

        let gap_ledger = workflow_step(&workflow, "Render RIPR gap decision ledger");
        assert!(gap_ledger.contains("hashFiles('target/ripr/reports/repo-exposure.json')"));
        assert!(gap_ledger.contains("continue-on-error: true"));
        assert!(gap_ledger.contains("reports gap-ledger"));
        assert!(gap_ledger.contains("--root ."));
        assert!(gap_ledger.contains("--repo-exposure target/ripr/reports/repo-exposure.json"));
        assert!(gap_ledger.contains("--out target/ripr/reports/gap-decision-ledger.json"));
        assert!(gap_ledger.contains("--out-md target/ripr/reports/gap-decision-ledger.md"));

        let first_action = workflow_step(&workflow, "Render RIPR first useful action");
        assert!(first_action.contains("continue-on-error: true"));
        assert!(first_action.contains("first-action"));
        assert!(first_action.contains("--root ."));
        assert!(first_action.contains("--pr-guidance target/ripr/review/comments.json"));
        assert!(
            first_action
                .contains("--assistant-proof target/ripr/reports/test-oracle-assistant-proof.json")
        );
        assert!(first_action.contains("--ledger target/ripr/reports/pr-evidence-ledger.json"));
        assert!(
            first_action.contains("--baseline-delta target/ripr/reports/baseline-debt-delta.json")
        );
        assert!(first_action.contains("--receipt target/ripr/reports/agent-receipt.json"));
        assert!(first_action.contains("--gate-decision target/ripr/reports/gate-decision.json"));
        assert!(
            first_action
                .contains("--coverage-frontier target/ripr/reports/coverage-grip-frontier.json")
        );
        assert!(
            first_action.contains("--editor-context target/ripr/workflow/evidence-context.json")
        );
        assert!(first_action.contains("--out target/ripr/reports/first-useful-action.json"));
        assert!(first_action.contains("--out-md target/ripr/reports/first-useful-action.md"));
        assert!(first_action.contains("first_action_has_input=true"));
        assert!(first_action.contains("ripr \"${first_action_args[@]}\""));

        let front_panel = workflow_step(&workflow, "Render RIPR PR review front panel");
        assert!(front_panel.contains("continue-on-error: true"));
        assert!(front_panel.contains("pr-review front-panel"));
        assert!(front_panel.contains("--root ."));
        assert!(front_panel.contains("--pr-guidance target/ripr/review/comments.json"));
        assert!(
            front_panel.contains("--first-action target/ripr/reports/first-useful-action.json")
        );
        assert!(
            front_panel
                .contains("--assistant-proof target/ripr/reports/test-oracle-assistant-proof.json")
        );
        assert!(
            front_panel
                .contains("--assistant-health target/ripr/reports/assistant-loop-health.json")
        );
        assert!(front_panel.contains("--ledger target/ripr/reports/pr-evidence-ledger.json"));
        assert!(
            front_panel.contains("--baseline-delta target/ripr/reports/baseline-debt-delta.json")
        );
        assert!(front_panel.contains("--zero-status target/ripr/reports/ripr-zero-status.json"));
        assert!(front_panel.contains("--gate-decision target/ripr/reports/gate-decision.json"));
        assert!(front_panel.contains(
            "--recommendation-calibration target/ripr/reports/recommendation-calibration.json"
        ));
        assert!(
            front_panel
                .contains("--mutation-calibration target/ripr/reports/mutation-calibration.json")
        );
        assert!(
            front_panel
                .contains("--coverage-frontier target/ripr/reports/coverage-grip-frontier.json")
        );
        assert!(front_panel.contains("--receipt target/ripr/reports/agent-receipt.json"));
        assert!(front_panel.contains("--out target/ripr/reports/pr-review-front-panel.json"));
        assert!(front_panel.contains("--out-md target/ripr/reports/pr-review-front-panel.md"));
        assert!(front_panel.contains("front_panel_has_input=true"));
        assert!(front_panel.contains("ripr \"${front_panel_args[@]}\""));
        assert!(front_panel.contains("No RIPR PR review front-panel inputs were available."));

        let first_pr = workflow_step(&workflow, "Render RIPR first-pr start-here");
        assert!(first_pr.contains("continue-on-error: true"));
        assert!(first_pr.contains("ripr first-pr"));
        assert!(first_pr.contains("--root ."));
        assert!(first_pr.contains("--gap-ledger target/ripr/reports/gap-decision-ledger.json"));
        assert!(first_pr.contains("--first-action target/ripr/reports/first-useful-action.json"));
        assert!(first_pr.contains("--review-comments target/ripr/review/comments.json"));
        assert!(first_pr.contains("--agent-packet target/ripr/workflow/agent-packet.json"));
        assert!(first_pr.contains("--gate-decision target/ripr/reports/gate-decision.json"));
        assert!(first_pr.contains("--receipts-dir target/ripr/receipts"));
        assert!(first_pr.contains("--out-dir target/ripr/reports"));

        let packet_index = workflow_step(&workflow, "Render RIPR report packet index");
        assert!(packet_index.contains("continue-on-error: true"));
        assert!(packet_index.contains("reports index"));
        assert!(packet_index.contains("--reports-dir target/ripr/reports"));
        assert!(packet_index.contains("--review-dir target/ripr/review"));
        assert!(packet_index.contains("--receipts-dir target/ripr/receipts"));
        assert!(packet_index.contains("--workflow-dir target/ripr/workflow"));
        assert!(packet_index.contains("--agent-dir target/ripr/agent"));
        assert!(packet_index.contains("--pilot-dir target/ripr/pilot"));
        assert!(packet_index.contains("--ci-dir target/ci"));
        assert!(packet_index.contains("--out target/ripr/reports/index.json"));
        assert!(packet_index.contains("--out-md target/ripr/reports/index.md"));
        assert!(packet_index.contains("target/ripr/reports/start-here.md"));
        assert!(packet_index.contains("target/ripr/reports/pr-review-front-panel.md"));
        assert!(packet_index.contains("target/ripr/review/comments.json"));
        assert!(packet_index.contains("target/ripr/reports/policy-operations.md"));
        assert!(packet_index.contains("target/ripr/reports/policy-history.md"));
        assert!(packet_index.contains("target/ripr/reports/policy-promotion-baseline-check.md"));
        assert!(
            packet_index
                .contains("target/ripr/reports/preview-promotion-typescript-boundary-gap.md")
        );
        assert!(packet_index.contains("target/ripr/reports/gate-decision.md"));
        assert!(packet_index.contains("target/ripr/reports/agent-receipt.json"));
        assert!(packet_index.contains("index_has_input=true"));
        assert!(packet_index.contains("No RIPR report-packet index inputs were available."));

        let annotations = workflow_step(&workflow, "Emit RIPR PR guidance annotations");
        assert!(annotations.contains("hashFiles('target/ripr/review/comments.json')"));
        assert!(annotations.contains("def escape_data:"));
        assert!(annotations.contains("def escape_property:"));
        assert!(!annotations.contains("@tsv"));
        assert!(!annotations.contains("escape_github_message()"));
        assert!(annotations.contains("::warning file="));

        let summary = workflow_step(&workflow, "Add RIPR advisory summary");
        assert!(summary.contains("### PR review summary"));
        assert!(summary.contains("#### PR review at a glance"));
        assert!(summary.contains("target/ripr/reports/pr-review-front-panel.json"));
        assert!(summary.contains("target/ripr/reports/pr-review-front-panel.md"));
        assert!(summary.contains(".summary.headline // \"not_available\""));
        assert!(summary.contains(".summary.top_issue_state // \"unknown\""));
        assert!(summary.contains(".summary.policy_state // \"none\""));
        assert!(summary.contains(".summary.placement // \"not_available\""));
        assert!(summary.contains(".summary.movement_state // \"unknown\""));
        assert!(summary.contains(".summary.coverage_grip_state // \"not_available\""));
        assert!(summary.contains(".summary.new_policy_eligible // 0"));
        assert!(summary.contains(".summary.baseline_still_present // 0"));
        assert!(summary.contains(".summary.baseline_resolved // 0"));
        assert!(summary.contains(".summary.blocking_candidates // 0"));
        assert!(summary.contains(".top_issue.missing_discriminator // \"not_available\""));
        assert!(summary.contains(".top_issue.suggested_test // \"not_available\""));
        assert!(summary.contains(".top_issue.verify_command // \"not_available\""));
        assert!(summary.contains(".top_issue.agent_command // \"not_available\""));
        assert!(summary.contains(".top_issue.receipt.artifact // \"not_available\""));
        assert!(summary.contains(".policy.mode // \"not_available\""));
        assert!(summary.contains(".policy.decision // \"not_available\""));
        assert!(summary.contains("repo_relative < target/ripr/reports/pr-review-front-panel.md"));
        assert!(summary.contains("PR review summary was not generated"));
        assert!(summary.contains("### Recommended next test"));
        assert!(summary.contains("#### Recommended next test at a glance"));
        assert!(summary.contains("#### First-run status"));
        assert!(summary.contains("Open `target/ripr/reports/start-here.md` first"));
        assert!(summary.contains(
            "Recovery states: missing artifact, stale evidence, wrong root, malformed artifact, no actionable gap, and preview-limited evidence"
        ));
        assert!(summary.contains(
            "Proof rail: the repair start, verify, receipt, and receipt path are static movement evidence only; verify and receipt run after the test edit."
        ));
        assert!(summary.contains("Start-here artifact: `target/ripr/reports/start-here.md`"));
        assert!(summary.contains("start_json=target/ripr/reports/start-here.json"));
        assert!(summary.contains(".selected.state // \"unknown\""));
        assert!(summary.contains(".selected.kind // \"none\""));
        assert!(summary.contains(
            ".selected.repair.route // .selected.repair.suggested_assertion // \"not_available\""
        ));
        assert!(summary.contains(".selected.changed_behavior // \"not_available\""));
        assert!(summary.contains(".selected.static_evidence_boundary // \"static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval.\""));
        assert!(summary.contains(
            ".selected.missing_discriminator // .selected.repair.suggested_assertion // \"not_available\""
        ));
        assert!(summary.contains(
            ".selected.focused_proof_intent // .selected.repair.suggested_assertion // \"not_available\""
        ));
        assert!(summary.contains(".selected.verify_command // \"not_available\""));
        assert!(summary.contains(".selected.receipt_command // \"not_available\""));
        assert!(summary.contains(".selected.receipt_path // \"not_available\""));
        assert!(
            summary
                .contains(".selected.repair_command // .selected.next_command // .selected.regeneration_command // \"none\"")
        );
        assert!(summary.contains("cat target/ripr/reports/start-here.md"));
        assert!(summary.contains("Boundary: \\`$start_boundary\\`"));
        assert!(summary.contains("missing_start_here"));
        assert!(summary.contains(
            "ripr first-pr --root . --base origin/${{ github.base_ref || github.event.repository.default_branch }} --head HEAD --gap-ledger target/ripr/reports/gap-decision-ledger.json"
        ));
        assert!(summary.contains(".summary.start_here // \"not_available\""));
        assert!(
            summary.contains("Start-here artifact: `target/ripr/reports/pr-review-front-panel.md`")
        );
        assert!(summary.contains("Start-here artifact: `target/ripr/pilot/pilot-summary.md`"));
        assert!(summary.contains("target/ripr/workflow/agent-packet.json"));
        assert!(summary.contains("target/ripr/reports/first-useful-action.json"));
        assert!(summary.contains("target/ripr/reports/first-useful-action.md"));
        assert!(summary.contains(".action_kind // \"unknown\""));
        assert!(summary.contains("Safe next action: \\`$first_action_kind\\`"));
        assert!(summary.contains(".commands.verify // \"not_available\""));
        assert!(summary.contains(".commands.receipt // \"not_available\""));
        assert!(summary.contains(".fallback.kind // \"none\""));
        assert!(summary.contains("repo_relative < target/ripr/reports/first-useful-action.md"));
        assert!(summary.contains("Recommended next test was not generated"));
        assert!(summary.contains("cat target/ripr/pilot/pilot-summary.md"));
        assert!(summary.contains("cat target/ripr/workflow/agent-review-summary.md"));
        // #3906 (N5): before any test edit the review packet names the
        // missing receipt as expected and leads with the carried repair
        // start and its after phase, not the post-edit snapshot loop.
        let packet = summary
            .find("echo '### Agent review packet'")
            .unwrap_or(usize::MAX);
        let packet_block = summary.get(packet..).unwrap_or_default();
        let packet_end = packet_block
            .find("echo '### Artifact packet'")
            .unwrap_or(packet_block.len());
        let packet_block = packet_block.get(..packet_end).unwrap_or_default();
        let receipt = packet_block
            .find(&format!("echo '- Receipt: {NO_RECEIPT_BEFORE_REPAIR}'"))
            .unwrap_or(usize::MAX);
        let start = packet_block
            .find("echo \"- Start repair: \\`$review_repair_command\\`\"")
            .unwrap_or(usize::MAX);
        let after = packet_block
            .find(&format!(
                "echo '- {REPAIR_AFTER_PHASE_LABEL}: {REPAIR_AFTER_PHASE_STEP}'"
            ))
            .unwrap_or(usize::MAX);
        let full = packet_block
            .find("cat target/ripr/workflow/agent-review-summary.md")
            .unwrap_or(0);
        assert!(
            receipt < start && start < after && after < full,
            "{packet_block}"
        );
        assert!(packet_block.contains(".static_movement.state // empty"));
        assert!(packet_block.contains("[ \"$review_movement\" = missing_artifact ]"));
        assert!(packet_block.contains(".selected.repair_command // empty"));
        assert!(summary.contains("### Uploaded review artifacts"));
        assert!(summary.contains("#### Uploaded artifacts at a glance"));
        assert!(summary.contains("target/ripr/reports/index.json"));
        assert!(summary.contains("target/ripr/reports/index.md"));
        assert!(summary.contains(".summary.entries // 0"));
        assert!(summary.contains(".summary.available // 0"));
        assert!(summary.contains(".summary.missing_expected // 0"));
        assert!(summary.contains(".summary.start_here // \"not_available\""));
        assert!(summary.contains(".summary.gate_authority // \"not_available\""));
        assert!(summary.contains(".missing_expected[]?.label"));
        assert!(summary.contains(".warnings[]?.kind"));
        assert!(summary.contains("cat target/ripr/reports/index.md"));
        assert!(summary.contains("Uploaded review artifacts summary was not generated"));
        assert!(summary.contains("#### Gate decision at a glance"));
        assert!(summary.contains("markdown_inline()"));
        assert!(summary.contains("gate_status=\"$(jq -r '.status // \"unknown\"'"));
        assert!(summary.contains("gate_mode=\"$(jq -r '.mode // \"unknown\"'"));
        assert!(summary.contains(".summary.blocking // 0"));
        assert!(summary.contains(".summary.acknowledged // 0"));
        assert!(summary.contains(".summary.advisory // 0"));
        assert!(summary.contains(".summary.suppressed // 0"));
        assert!(summary.contains(".summary.not_applicable // 0"));
        assert!(summary.contains(".summary.unknown_confidence // 0"));
        assert!(summary.contains("blocking=\"$(markdown_inline \"$blocking\")\""));
        assert!(summary.contains("Counts: blocking=\\`$blocking\\`"));
        assert!(summary.contains("Active PR labels"));
        assert!(summary.contains("Acknowledgement labels"));
        assert!(summary.contains("Applied waiver label"));
        assert!(summary.contains("Baseline artifact"));
        assert!(summary.contains("Recommendation calibration"));
        assert!(summary.contains("Mutation calibration"));
        assert!(summary.contains("Blocking reason (\\`$blocking\\`): \\`$blocking_reason\\`"));
        assert!(summary.contains("target/ripr/reports/gate-decision.json"));
        assert!(summary.contains("target/ci/labels.json"));
        assert!(summary.contains("cat target/ripr/reports/gate-decision.md"));
        assert!(summary.contains("Gate decision was not run"));
        assert!(summary.contains("### Baseline debt delta"));
        assert!(summary.contains("#### Baseline debt movement"));
        assert!(summary.contains("target/ripr/reports/baseline-debt-delta.json"));
        assert!(summary.contains("target/ripr/reports/baseline-debt-delta.md"));
        assert!(summary.contains("cat target/ripr/reports/baseline-debt-delta.md"));
        assert!(summary.contains(".baseline.path // .inputs.baseline // \"unknown\""));
        assert!(summary.contains(".delta.still_present // 0"));
        assert!(summary.contains(".delta.resolved // 0"));
        assert!(summary.contains(".delta.new_policy_eligible // 0"));
        assert!(summary.contains(".delta.acknowledged // 0"));
        assert!(summary.contains(".delta.suppressed // 0"));
        assert!(summary.contains(".delta.stale_baseline_entry // 0"));
        assert!(summary.contains(".delta.invalid_baseline_entry // 0"));
        assert!(summary.contains(".delta.missing_current_input // 0"));
        assert!(summary.contains("Set `RIPR_GATE_BASELINE`"));
        assert!(summary.contains("Baseline debt delta was not run"));
        assert!(summary.contains("Baseline debt delta was not generated"));
        assert!(summary.contains("### RIPR Zero status"));
        assert!(summary.contains("#### RIPR Zero at a glance"));
        assert!(summary.contains("target/ripr/reports/ripr-zero-status.json"));
        assert!(summary.contains("target/ripr/reports/ripr-zero-status.md"));
        assert!(summary.contains(".ripr_zero.state // \"unknown\""));
        assert!(summary.contains(".ripr_zero.visible_unresolved // 0"));
        assert!(summary.contains(".ripr_zero.new_policy_eligible // 0"));
        assert!(summary.contains(".ripr_zero.blocking_candidates // 0"));
        assert!(summary.contains(".baseline.metadata.stale // 0"));
        assert!(summary.contains(".top_debt_areas[0].area // \"none\""));
        assert!(summary.contains("cat target/ripr/reports/ripr-zero-status.md"));
        assert!(summary.contains("RIPR Zero status was not run"));
        assert!(summary.contains("RIPR Zero status was not generated"));
        assert!(summary.contains("### PR evidence ledger"));
        assert!(summary.contains("#### PR movement at a glance"));
        assert!(summary.contains("target/ripr/reports/pr-evidence-ledger.json"));
        assert!(summary.contains("target/ripr/reports/pr-evidence-ledger.md"));
        assert!(summary.contains(".movement.new_policy_eligible // 0"));
        assert!(summary.contains(".movement.baseline_still_present // 0"));
        assert!(summary.contains(".movement.baseline_resolved // 0"));
        assert!(summary.contains(".movement.acknowledged // 0"));
        assert!(summary.contains(".movement.suppressed // 0"));
        assert!(summary.contains(".movement.blocking_candidates // 0"));
        assert!(summary.contains(".movement.visible_unresolved // 0"));
        assert!(summary.contains(".coverage_grip_frontier.status // \"not_available\""));
        assert!(summary.contains(".history.trend // \"not_available\""));
        assert!(summary.contains(".top_repair_route.verify_command // \"not_available\""));
        assert!(summary.contains(".top_repair_route.agent_command // \"not_available\""));
        assert!(summary.contains("Pass/fail authority remains \\`ripr gate evaluate\\`"));
        assert!(summary.contains("cat target/ripr/reports/pr-evidence-ledger.md"));
        assert!(summary.contains("PR evidence ledger was not generated"));
        assert!(summary.contains("PR evidence ledger was not run"));
        assert!(summary.contains("### Policy readiness"));
        assert!(summary.contains("#### Policy readiness at a glance"));
        assert!(summary.contains("target/ripr/reports/policy-readiness.json"));
        assert!(summary.contains("target/ripr/reports/policy-readiness.md"));
        assert!(summary.contains(".recommended_mode // \"unknown\""));
        assert!(summary.contains(".blocking_readiness.state // \"unknown\""));
        assert!(summary.contains(".baseline_health.state // \"unknown\""));
        assert!(summary.contains(".waiver_health.state // \"unknown\""));
        assert!(summary.contains(".suppression_health.state // \"unknown\""));
        assert!(summary.contains(".calibration_health.state // \"unknown\""));
        assert!(summary.contains(".preview_evidence_boundary.state // \"unknown\""));
        assert!(summary.contains("advisory readiness projection only"));
        assert!(summary.contains("cat target/ripr/reports/policy-readiness.md"));
        assert!(summary.contains("Policy readiness was not generated"));
        assert!(summary.contains("### Policy operations"));
        assert!(summary.contains("#### Policy operations at a glance"));
        assert!(summary.contains("target/ripr/reports/policy-operations.json"));
        assert!(summary.contains("target/ripr/reports/policy-operations.md"));
        assert!(summary.contains(".current_policy_ceiling // \"unknown\""));
        assert!(summary.contains(".recommended_next_action // \"not_available\""));
        assert!(summary.contains(".safe_to_promote_to // [] | length"));
        assert!(summary.contains(".not_safe_to_promote_to // [] | length"));
        assert!(summary.contains(".promotion_blockers // [] | length"));
        assert!(summary.contains("promotion requires manual review"));
        assert!(summary.contains("cat target/ripr/reports/policy-operations.md"));
        assert!(summary.contains("Policy operations was not generated"));
        assert!(summary.contains("### Policy history"));
        assert!(summary.contains("#### Policy history at a glance"));
        assert!(summary.contains("target/ripr/reports/policy-history.json"));
        assert!(summary.contains("target/ripr/reports/policy-history.md"));
        assert!(summary.contains(".current.current_policy_ceiling // \"unknown\""));
        assert!(summary.contains(".history_summary.entries // 0"));
        assert!(summary.contains(".trend.ceiling.direction // \"unknown\""));
        assert!(summary.contains(".trend.waiver_count.direction // \"unknown\""));
        assert!(summary.contains(".trend.preview_boundary_state.direction // \"unknown\""));
        assert!(
            summary.contains("never appends to \\`.ripr/policy-history.jsonl\\` automatically")
        );
        assert!(summary.contains("cat target/ripr/reports/policy-history.md"));
        assert!(summary.contains("Policy history was not generated"));
        assert!(summary.contains("### Policy promotion packets"));
        assert!(summary.contains("policy-promotion-visible-only.json"));
        assert!(summary.contains("policy-promotion-acknowledgeable.json"));
        assert!(summary.contains("policy-promotion-baseline-check.json"));
        assert!(summary.contains("policy-promotion-calibrated-gate.json"));
        assert!(summary.contains(".why_or_why_not // \"not_available\""));
        assert!(summary.contains("packets do not edit \\`ripr.toml\\`"));
        assert!(summary.contains("Policy promotion packets were not generated"));
        assert!(summary.contains("cat \"$promotion_md\""));
        assert!(summary.contains("### Preview promotion packets"));
        assert!(summary.contains("preview-promotion-*-*.json"));
        assert!(summary.contains(".candidate_class // \"unknown\""));
        assert!(summary.contains(".missing_evidence // [] | length"));
        assert!(summary.contains("preview evidence remains visible and non-gating"));
        assert!(summary.contains("Preview promotion packets were not generated"));
        assert!(summary.contains("cat \"$preview_md\""));
        assert!(summary.contains("### Waiver aging"));
        assert!(summary.contains("#### Waiver aging at a glance"));
        assert!(summary.contains("target/ripr/reports/waiver-aging.json"));
        assert!(summary.contains("target/ripr/reports/waiver-aging.md"));
        assert!(summary.contains(".summary.waiver_count // 0"));
        assert!(summary.contains(".summary.identity_count // 0"));
        assert!(summary.contains(".summary.repeated_seam_count // 0"));
        assert!(summary.contains(".summary.repeated_file_count // 0"));
        assert!(summary.contains(".summary.focused_test_candidates // 0"));
        assert!(summary.contains(".summary.durable_suppression_candidates // 0"));
        assert!(summary.contains("repeated waiver is a visible signal"));
        assert!(summary.contains("cat target/ripr/reports/waiver-aging.md"));
        assert!(summary.contains("Waiver aging was not generated"));
        assert!(summary.contains("### Suppression health"));
        assert!(summary.contains("#### Suppression health at a glance"));
        assert!(summary.contains("target/ripr/reports/suppression-health.json"));
        assert!(summary.contains("target/ripr/reports/suppression-health.md"));
        assert!(summary.contains(".summary.suppressions // 0"));
        assert!(summary.contains(".summary.healthy // 0"));
        assert!(summary.contains(".summary.missing_owner // 0"));
        assert!(summary.contains(".summary.missing_reason // 0"));
        assert!(summary.contains(".summary.stale // 0"));
        assert!(summary.contains(".summary.overbroad_scope // 0"));
        assert!(summary.contains(".summary.unknown_selector // 0"));
        assert!(summary.contains(".summary.preview_without_preview_label // 0"));
        assert!(summary.contains("suppressions remain visible durable exceptions"));
        assert!(summary.contains("cat target/ripr/reports/suppression-health.md"));
        assert!(summary.contains("Suppression health was not generated"));
        assert!(summary.contains("### Test-oracle assistant proof"));
        assert!(summary.contains("#### Assistant proof at a glance"));
        assert!(summary.contains("target/ripr/reports/test-oracle-assistant-proof.json"));
        assert!(summary.contains("target/ripr/reports/test-oracle-assistant-proof.md"));
        assert!(summary.contains(".seam.missing_discriminator // \"not_available\""));
        assert!(summary.contains(".recommendation.placement // \"not_available\""));
        assert!(summary.contains(".evidence_movement.state // \"unknown\""));
        assert!(summary.contains(".ci_projection.gate_decision // \"not_supplied\""));
        assert!(summary.contains(".ci_projection.coverage_frontier // \"not_supplied\""));
        assert!(summary.contains("cat target/ripr/reports/test-oracle-assistant-proof.md"));
        assert!(summary.contains("### Agent proof status"));
        assert!(summary.contains("#### Agent proof status at a glance"));
        assert!(summary.contains("target/ripr/reports/assistant-loop-health.json"));
        assert!(summary.contains("target/ripr/reports/assistant-loop-health.md"));
        assert!(summary.contains(".summary.proofs // 0"));
        assert!(summary.contains(".summary.complete // 0"));
        assert!(summary.contains(".summary.partial // 0"));
        assert!(summary.contains(".summary.missing_required_input // 0"));
        assert!(summary.contains(".summary.missing_optional_input // 0"));
        assert!(summary.contains(".summary.improved // 0"));
        assert!(summary.contains(".summary.unchanged // 0"));
        assert!(summary.contains(".summary.regressed // 0"));
        assert!(summary.contains(".summary.unknown_movement // 0"));
        assert!(summary.contains(".summary.repair_queue // 0"));
        assert!(summary.contains(".warning_summary[]?"));
        assert!(summary.contains(".repair_queue[]?.repair_kind"));
        assert!(summary.contains("cat target/ripr/reports/assistant-loop-health.md"));
        assert!(summary.contains("advisory static health over proof artifacts"));
        assert!(summary.contains(".summary.comments // 0"));
        assert!(summary.contains(".summary.summary_only // 0"));
        assert!(summary.contains(".summary.suppressed // 0"));
        assert!(summary.contains("No runtime mutation execution is performed"));

        for step in fixture.non_blocking_steps {
            let block = workflow_step(&workflow, step);
            assert!(
                block.contains("continue-on-error: true"),
                "`{step}` must remain advisory/non-blocking"
            );
        }

        for step in fixture.gate_conditional_steps {
            let block = workflow_step(&workflow, step);
            assert!(
                block.contains(
                    "continue-on-error: ${{ vars.RIPR_GATE_MODE == '' || vars.RIPR_GATE_MODE == 'visible-only' }}"
                ),
                "`{step}` must be conditional on RIPR_GATE_MODE, not unconditionally advisory"
            );
        }

        let artifact_check = workflow_step(&workflow, "Check RIPR advisory artifacts");
        assert!(artifact_check.contains("::warning::"));
        assert!(artifact_check.contains("target/ripr/reports/start-here.md"));
        assert!(artifact_check.contains("gate-decision.json (RIPR_GATE_MODE is set)"));

        for step in fixture.optional_sarif_steps {
            let block = workflow_step(&workflow, step);
            assert!(
                block.contains("env.RIPR_UPLOAD_SARIF == 'true'"),
                "`{step}` must stay gated by RIPR_UPLOAD_SARIF"
            );
        }

        for forbidden in fixture.forbidden_fragments {
            assert!(
                !workflow.contains(forbidden),
                "generated workflow must not enable `{forbidden}` by default"
            );
        }
    }

    #[test]
    fn init_ci_github_writes_workflow_and_preserves_existing_config() -> Result<(), String> {
        let dir = unique_command_test_dir("init-ci");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        let config = dir.join(CONFIG_FILE_NAME);
        std::fs::write(&config, "# existing policy\n")
            .map_err(|err| format!("write existing config: {err}"))?;

        init(&args(&[
            "--root",
            &dir.display().to_string(),
            "--ci",
            "github",
        ]))?;

        let config_text =
            std::fs::read_to_string(&config).map_err(|err| format!("read config: {err}"))?;
        let workflow_path = dir.join(".github/workflows/ripr.yml");
        let workflow = std::fs::read_to_string(&workflow_path)
            .map_err(|err| format!("read workflow: {err}"))?;
        assert_eq!(config_text, "# existing policy\n");
        assert!(workflow.contains("RIPR advisory reports"));
        assert!(workflow.contains("continue-on-error: true"));
        assert!(workflow.contains("actions/upload-artifact@v7"));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn init_ci_github_refuses_existing_workflow_without_force() -> Result<(), String> {
        let dir = unique_command_test_dir("init-ci-existing");
        let workflow_dir = dir.join(".github/workflows");
        std::fs::create_dir_all(&workflow_dir)
            .map_err(|err| format!("create workflow dir: {err}"))?;
        let workflow = workflow_dir.join("ripr.yml");
        std::fs::write(&workflow, "name: Existing\n")
            .map_err(|err| format!("write existing workflow: {err}"))?;

        let result = init(&args(&[
            "--root",
            &dir.display().to_string(),
            "--ci",
            "github",
        ]));
        assert!(matches!(result, Err(message) if message.contains("already exists")));
        assert!(!dir.join(CONFIG_FILE_NAME).exists());
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn init_force_replaces_symlink_without_clobbering_target() -> Result<(), String> {
        // #2101: a pre-placed ripr.toml symlink plus --force must not
        // clobber the symlink target; the link itself is replaced by a
        // regular config file.
        let dir = unique_command_test_dir("init-force-symlink");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        let target = dir.join("target.txt");
        std::fs::write(&target, "do not clobber\n")
            .map_err(|err| format!("write target: {err}"))?;
        let config = dir.join(CONFIG_FILE_NAME);
        std::os::unix::fs::symlink(&target, &config)
            .map_err(|err| format!("plant symlink: {err}"))?;

        init(&args(&["--root", &dir.display().to_string(), "--force"]))?;

        let target_text =
            std::fs::read_to_string(&target).map_err(|err| format!("read target: {err}"))?;
        assert_eq!(target_text, "do not clobber\n");
        let metadata =
            std::fs::symlink_metadata(&config).map_err(|err| format!("stat config: {err}"))?;
        assert!(metadata.file_type().is_file());
        let config_text =
            std::fs::read_to_string(&config).map_err(|err| format!("read config: {err}"))?;
        assert!(config_text.contains("[analysis]"));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn lsp_version_returns_ok() {
        assert_eq!(lsp(&args(&["--version"])), Ok(()));
    }

    #[test]
    fn lsp_rejects_unknown_arguments() {
        assert_eq!(
            lsp(&args(&["--bad"])),
            Err("unknown lsp argument \"--bad\". Run `ripr lsp --help`.".to_string())
        );
    }

    #[test]
    fn explain_requires_selector() {
        assert_eq!(
            explain(&args(&[])),
            Err("missing finding selector; pass a finding id (e.g. `probe:src_lib.rs:error_path:abc123`) or `file:line`. Run `ripr check --json` to list finding ids".to_string())
        );
    }

    /// The regression this guards is not a missing suggestion but a missing
    /// *error*: the positional-selector arm accepted any token, so a mistyped
    /// flag became the finding selector and `explain` ran a full analysis
    /// against `--fromm` as if it were a finding id. A flag-shaped token must
    /// be rejected as a flag, with the suggestion the parser now routes to.
    #[test]
    fn explain_suggests_the_nearest_flag_instead_of_taking_a_typo_as_the_selector() {
        assert_eq!(
            explain(&args(&["--fromm", "artifact.json"])),
            Err(
                "unknown explain argument \"--fromm\". Did you mean `--from`? \
                 Run `ripr explain --help`."
                    .to_string()
            )
        );
    }

    /// A `file:line` selector may legitimately begin with `-`, because
    /// `app/selector.rs` splits on the last `:` and accepts any path. Guarding
    /// the positional arm on the `-` prefix alone would reject
    /// `-generated.rs:42`, a previously usable selector, with no `--`
    /// end-of-options escape to recover it. The token must reach selector
    /// handling, while a genuine typo still gets the suggestion.
    #[test]
    fn explain_keeps_dash_prefixed_file_line_selectors() {
        let unknown_argument_error =
            "unknown explain argument \"-generated.rs:42\". Run `ripr explain --help`.";
        let result = explain(&args(&["-generated.rs:42"]));
        assert_ne!(
            result,
            Err(unknown_argument_error.to_string()),
            "a dash-prefixed file:line selector must not be treated as a flag"
        );
        assert!(
            !matches!(&result, Err(message) if message.contains("unknown explain argument")),
            "unexpected unknown-argument rejection: {result:?}"
        );

        assert_eq!(
            explain(&args(&["--fromm", "artifact.json"])),
            Err(
                "unknown explain argument \"--fromm\". Did you mean `--from`? \
                 Run `ripr explain --help`."
                    .to_string()
            )
        );
    }

    /// Also pins that the `!value.starts_with('-')` guard on the positional
    /// arm did not cost `explain` its selector: `probe:...` is still accepted
    /// there, so the rejected token is the trailing `"extra"`. Were the guard
    /// wrong, this error would name `probe:...` instead.
    #[test]
    fn explain_rejects_unexpected_argument_after_selector() {
        assert_eq!(
            explain(&args(&["probe:src_lib_rs:10:return_value", "extra"])),
            Err("unknown explain argument \"extra\". Run `ripr explain --help`.".to_string())
        );
    }

    #[test]
    fn explain_requires_values_for_value_flags() {
        assert_eq!(
            explain(&args(&["--root"])),
            Err("missing value for --root".to_string())
        );
        assert_eq!(
            explain(&args(&["--base"])),
            Err("missing value for --base".to_string())
        );
        assert_eq!(
            explain(&args(&["--diff"])),
            Err("missing value for --diff".to_string())
        );
        assert_eq!(
            explain(&args(&["--perl-facts"])),
            Err("missing value for --perl-facts".to_string())
        );
        assert_eq!(
            explain(&args(&["--suppression-policy"])),
            Err("missing value for --suppression-policy".to_string())
        );
    }

    /// #4319: the synopsis reads `[--base REV|--diff PATH]` — alternatives —
    /// but the loader gives `--diff` precedence and never validates `--base`
    /// beside it, so both flags on one command line silently analyzed the
    /// diff while appearing to assert the base. The conflict must fail at
    /// parse time (before any pipeline run), in either flag order, and before
    /// the selector requirement. Message pinned verbatim.
    #[test]
    fn explain_rejects_base_and_diff_together_at_parse_time() {
        let expected = Err(
            "explain --base cannot be combined with --diff: --base and --diff are alternative diff sources; pass one"
                .to_string(),
        );
        assert_eq!(
            explain(&args(&[
                "--diff",
                "sample.diff",
                "--base",
                "refs/heads/nope",
                "probe:src_lib.rs:error_path:abcd",
            ])),
            expected
        );
        assert_eq!(
            explain(&args(&[
                "--base",
                "refs/heads/nope",
                "--diff",
                "sample.diff"
            ])),
            expected,
            "the conflict must not depend on flag order or selector presence"
        );
    }

    /// `--from` scope flags are assertions verified against the recording
    /// (RIPR-SPEC-0140, `app/check_artifact.rs::verify_scope_assertions`),
    /// not alternative diff sources, so the fresh-run conflict must not fire
    /// on the reuse path. The parse proceeds past the gate and fails later,
    /// on the missing artifact — never with the conflict message.
    #[test]
    fn explain_keeps_base_and_diff_as_from_artifact_assertions() {
        let result = explain(&args(&[
            "--from",
            "does-not-exist.json",
            "--diff",
            "sample.diff",
            "--base",
            "refs/heads/nope",
            "probe:src_lib.rs:error_path:abcd",
        ]));
        assert!(
            !matches!(&result, Err(message) if message.contains("cannot be combined with --diff")),
            "`--from` + `--base` + `--diff` is the reuse-verification path, not a diff-source conflict: {result:?}"
        );
    }

    #[test]
    fn lsp_accepts_stdio_flag() {
        // lsp function doesn't reject --stdio, it just processes it
        assert_eq!(lsp(&args(&["--stdio"])), Ok(()));
    }

    #[test]
    fn lsp_version_returns_ok_with_short_flag() {
        assert_eq!(lsp(&args(&["-V"])), Ok(()));
    }

    pub(super) fn outcome_before_json() -> &'static str {
        r#"{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-a",
      "kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 42,
      "grip_class": "weakly_gripped",
      "related_tests": [
        {"oracle_kind": "exact_value", "oracle_strength": "weak"}
      ],
      "observed_values": ["50"],
      "missing_discriminators": [
        {"value": "threshold equality", "reason": "not observed"}
      ]
    }
  ]
}"#
    }

    pub(super) fn outcome_after_json() -> &'static str {
        r#"{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-a",
      "kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 42,
      "grip_class": "strongly_gripped",
      "related_tests": [
        {"oracle_kind": "exact_value", "oracle_strength": "strong"}
      ],
      "observed_values": ["50", "100"],
      "missing_discriminators": []
    }
  ]
}"#
    }

    fn calibration_repo_json() -> &'static str {
        r#"{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-a",
      "kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 42,
      "grip_class": "weakly_gripped",
      "related_tests": [],
      "observed_values": [],
      "missing_discriminators": []
    }
  ]
}"#
    }

    fn calibration_mutants_json() -> &'static str {
        r#"[{"id":"m1","seam_id":"seam-a","outcome":"missed","operator":"replace"}]"#
    }
}
