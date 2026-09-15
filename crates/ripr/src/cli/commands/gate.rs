//! Arg-parsing and dispatch for `ripr gate evaluate`.
//!
//! This is the CLI adapter layer only. Gate evaluation and report rendering
//! live in `crate::output::gate`. This module owns argv parsing, output
//! destination selection, and exit mapping for the gate command family.

use crate::cli::commands_options::GateOptions;
use crate::cli::help;
use crate::cli::parse::expect_value;
use crate::cli::suggest::unknown_argument;
use crate::output;
use std::io::IsTerminal;
use std::path::PathBuf;

use super::{non_empty_path_arg, non_empty_string_arg, write_text_file};

pub(in crate::cli) fn gate(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_gate_help();
        return Ok(());
    }
    // The sole subcommand is implied when the command runs bare:
    // `ripr <cmd>` behaves like `ripr <cmd> "evaluate"` (#2013).
    let (subcommand, rest) = match args.split_first() {
        Some((subcommand, rest)) => (subcommand.as_str(), rest),
        None => ("evaluate", &[][..]),
    };
    if subcommand != "evaluate" {
        return Err(format!(
            "unknown gate subcommand {subcommand:?}; expected `evaluate`"
        ));
    }

    let options = parse_gate_options(rest)?;
    if let Some(warning) =
        default_visible_only_warning(options.mode_explicit, !std::io::stderr().is_terminal())
    {
        eprintln!("{warning}");
    }
    // Surface the missing-input requirement directly instead of burying it
    // inside the gate-decision.json config_errors array (#2589 review).
    if options.input.pr_guidance.is_none()
        && options.input.gap_ledger.is_none()
        && options.input.repo_exposure.is_none()
    {
        return Err(
            "gate evaluate requires at least one of --pr-guidance <path>, --gap-ledger <path>, or --repo-exposure <path>; see `ripr gate --help` for all options".to_string(),
        );
    }
    let report = output::gate::build_gate_decision_report(&options.input)?;
    let rendered_json = output::gate::render_gate_decision_json(&report)?;
    let rendered_md = output::gate::render_gate_decision_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    if output::gate::gate_decision_should_fail(&report) {
        let detail = output::gate::gate_decision_inline_detail(&report);
        Err(format!(
            "ripr gate decision is {}{}; see {} for the full report",
            output::gate::gate_decision_status(&report),
            detail,
            options.out.display()
        ))
    } else {
        Ok(())
    }
}

fn parse_gate_options(args: &[String]) -> Result<GateOptions, String> {
    let mut root = PathBuf::from(".");
    let mut repo_exposure = None;
    let mut pr_guidance = None;
    let mut gap_ledger = None;
    let mut sarif_policy = None;
    let mut labels_json = None;
    let mut labels = Vec::new();
    let mut agent_verify = None;
    let mut agent_receipt = None;
    let mut recommendation_calibration = None;
    let mut mutation_calibration = None;
    let mut baseline = None;
    let mut exception_policy = None;
    let mut mode = output::gate::GateMode::VisibleOnly;
    let mut mode_explicit = false;
    let mut acknowledgement_labels = Vec::new();
    let mut out = PathBuf::from(output::gate::DEFAULT_GATE_OUT);
    let mut out_md = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_path_arg(args, i, "--root", "gate")?;
            }
            "--repo-exposure" => {
                i += 1;
                repo_exposure = Some(non_empty_path_arg(args, i, "--repo-exposure", "gate")?);
            }
            "--pr-guidance" => {
                i += 1;
                pr_guidance = Some(non_empty_path_arg(args, i, "--pr-guidance", "gate")?);
            }
            "--gap-ledger" => {
                i += 1;
                gap_ledger = Some(non_empty_path_arg(args, i, "--gap-ledger", "gate")?);
            }
            "--sarif-policy" => {
                i += 1;
                sarif_policy = Some(non_empty_path_arg(args, i, "--sarif-policy", "gate")?);
            }
            "--labels-json" => {
                i += 1;
                labels_json = Some(non_empty_path_arg(args, i, "--labels-json", "gate")?);
            }
            "--label" => {
                i += 1;
                labels.push(non_empty_string_arg(args, i, "--label", "gate")?);
            }
            "--agent-verify" => {
                i += 1;
                agent_verify = Some(non_empty_path_arg(args, i, "--agent-verify", "gate")?);
            }
            "--agent-receipt" => {
                i += 1;
                agent_receipt = Some(non_empty_path_arg(args, i, "--agent-receipt", "gate")?);
            }
            "--recommendation-calibration" => {
                i += 1;
                recommendation_calibration = Some(non_empty_path_arg(
                    args,
                    i,
                    "--recommendation-calibration",
                    "gate",
                )?);
            }
            "--mutation-calibration" => {
                i += 1;
                mutation_calibration = Some(non_empty_path_arg(
                    args,
                    i,
                    "--mutation-calibration",
                    "gate",
                )?);
            }
            "--baseline" => {
                i += 1;
                baseline = Some(non_empty_path_arg(args, i, "--baseline", "gate")?);
            }
            "--exception-policy" => {
                i += 1;
                exception_policy = Some(non_empty_path_arg(args, i, "--exception-policy", "gate")?);
            }
            "--mode" => {
                i += 1;
                mode_explicit = true;
                mode = output::gate::GateMode::parse(expect_value(args, i, "--mode")?)?;
            }
            "--acknowledgement-label" => {
                i += 1;
                acknowledgement_labels.push(non_empty_string_arg(
                    args,
                    i,
                    "--acknowledgement-label",
                    "gate",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "gate")?;
            }
            "--out-md" => {
                i += 1;
                out_md = Some(non_empty_path_arg(args, i, "--out-md", "gate")?);
            }
            other => return Err(unknown_argument("gate", other)),
        }
        i += 1;
    }

    let out_md = out_md.unwrap_or_else(|| output::gate::markdown_path_for(&out));
    Ok(GateOptions {
        input: output::gate::GateEvaluateInput {
            root,
            repo_exposure,
            pr_guidance,
            gap_ledger,
            sarif_policy,
            labels_json,
            labels,
            agent_verify,
            agent_receipt,
            recommendation_calibration,
            mutation_calibration,
            baseline,
            mode,
            acknowledgement_labels,
            exception_policy,
        },
        out,
        out_md,
        mode_explicit,
    })
}

const DEFAULT_VISIBLE_ONLY_WARNING: &str = "ripr: gate evaluate is using default mode visible-only; it records advisory evidence and never blocks. Pass --mode acknowledgeable, --mode baseline-check, or --mode calibrated-gate to opt into blocking policy.";

fn default_visible_only_warning(
    mode_explicit: bool,
    non_interactive: bool,
) -> Option<&'static str> {
    if non_interactive && !mode_explicit {
        Some(DEFAULT_VISIBLE_ONLY_WARNING)
    } else {
        None
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::tests::{args, repo_root, unique_command_test_dir};
    use super::*;

    #[test]
    fn gate_parses_full_option_surface() {
        let options = parse_gate_options(&args(&[
            "--root",
            "repo",
            "--repo-exposure",
            "target/ripr/reports/repo-exposure.json",
            "--pr-guidance",
            "target/ripr/review/comments.json",
            "--gap-ledger",
            "target/ripr/reports/gap-decision-ledger.json",
            "--sarif-policy",
            "target/ripr/reports/sarif-policy.json",
            "--labels-json",
            "target/ci/labels.json",
            "--label",
            "ripr-waive",
            "--agent-verify",
            "target/ripr/workflow/agent-verify.json",
            "--agent-receipt",
            "target/ripr/reports/agent-receipt.json",
            "--recommendation-calibration",
            "target/ripr/reports/recommendation-calibration.json",
            "--mutation-calibration",
            "target/ripr/reports/mutation-calibration.json",
            "--baseline",
            "target/ripr/reports/gate-baseline.json",
            "--mode",
            "calibrated-gate",
            "--acknowledgement-label",
            "custom-waive",
            "--out",
            "target/ripr/reports/gate-decision.json",
        ]));

        assert_eq!(
            options,
            Ok(GateOptions {
                input: output::gate::GateEvaluateInput {
                    root: PathBuf::from("repo"),
                    repo_exposure: Some(PathBuf::from("target/ripr/reports/repo-exposure.json")),
                    pr_guidance: Some(PathBuf::from("target/ripr/review/comments.json")),
                    gap_ledger: Some(PathBuf::from(
                        "target/ripr/reports/gap-decision-ledger.json"
                    )),
                    sarif_policy: Some(PathBuf::from("target/ripr/reports/sarif-policy.json")),
                    labels_json: Some(PathBuf::from("target/ci/labels.json")),
                    labels: vec!["ripr-waive".to_string()],
                    agent_verify: Some(PathBuf::from("target/ripr/workflow/agent-verify.json")),
                    agent_receipt: Some(PathBuf::from("target/ripr/reports/agent-receipt.json")),
                    recommendation_calibration: Some(PathBuf::from(
                        "target/ripr/reports/recommendation-calibration.json"
                    )),
                    mutation_calibration: Some(PathBuf::from(
                        "target/ripr/reports/mutation-calibration.json"
                    )),
                    baseline: Some(PathBuf::from("target/ripr/reports/gate-baseline.json")),
                    mode: output::gate::GateMode::CalibratedGate,
                    acknowledgement_labels: vec!["custom-waive".to_string()],
                    exception_policy: None,
                },
                out: PathBuf::from("target/ripr/reports/gate-decision.json"),
                out_md: PathBuf::from("target/ripr/reports/gate-decision.md"),
                mode_explicit: true,
            })
        );
    }

    #[test]
    fn gate_rejects_bad_surface_and_unknown_args() {
        // Bare `ripr gate` is the `evaluate` alias (#2013): it dispatches
        // instead of erroring on a missing subcommand. The full path writes
        // the default report files, so they are cleaned on both sides.
        for residue in [
            "target/ripr/reports/gate-decision.json",
            "target/ripr/reports/gate-decision.md",
        ] {
            let _ = std::fs::remove_file(residue);
        }
        let bare_result = gate(&args(&[]));
        for residue in [
            "target/ripr/reports/gate-decision.json",
            "target/ripr/reports/gate-decision.md",
        ] {
            let _ = std::fs::remove_file(residue);
        }
        assert_eq!(
            bare_result,
            Err(
                "gate evaluate requires at least one of --pr-guidance <path>, --gap-ledger <path>, or --repo-exposure <path>; see `ripr gate --help` for all options"
                    .to_string()
            )
        );
        assert_eq!(
            gate(&args(&["inspect"])),
            Err("unknown gate subcommand \"inspect\"; expected `evaluate`".to_string())
        );
        assert_eq!(
            parse_gate_options(&args(&["--mode", "strict"])),
            Err("unknown gate mode `strict`; expected `visible-only`, `acknowledgeable`, `baseline-check`, or `calibrated-gate`".to_string())
        );
        assert_eq!(
            parse_gate_options(&args(&["--out", ""])),
            Err("gate --out requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_gate_options(&args(&["--bad"])),
            Err("unknown gate argument \"--bad\". Run `ripr gate --help`.".to_string())
        );
        assert_eq!(
            parse_gate_options(&args(&[])),
            Ok(GateOptions {
                input: output::gate::GateEvaluateInput {
                    root: PathBuf::from("."),
                    repo_exposure: None,
                    pr_guidance: None,
                    gap_ledger: None,
                    sarif_policy: None,
                    labels_json: None,
                    labels: Vec::new(),
                    agent_verify: None,
                    agent_receipt: None,
                    recommendation_calibration: None,
                    mutation_calibration: None,
                    baseline: None,
                    mode: output::gate::GateMode::VisibleOnly,
                    acknowledgement_labels: Vec::new(),
                    exception_policy: None,
                },
                out: PathBuf::from(output::gate::DEFAULT_GATE_OUT),
                out_md: PathBuf::from("target/ripr/reports/gate-decision.md"),
                mode_explicit: false,
            })
        );
    }

    #[test]
    fn default_visible_only_warning_requires_omitted_mode_and_non_interactive_use() {
        assert_eq!(
            default_visible_only_warning(false, true),
            Some(DEFAULT_VISIBLE_ONLY_WARNING)
        );
        assert_eq!(default_visible_only_warning(false, false), None);
        assert_eq!(default_visible_only_warning(true, true), None);
        assert_eq!(
            parse_gate_options(&args(&["--label", "--mode"])).map(|options| options.mode_explicit),
            Ok(false)
        );
        assert_eq!(
            parse_gate_options(&args(&["--pr-guidance", "--mode"]))
                .map(|options| options.mode_explicit),
            Ok(false)
        );
    }

    #[test]
    fn gate_command_writes_visible_only_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("gate-visible");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create gate dir: {err}"))?;
        let out = dir.join("gate-decision.json");
        let out_md = dir.join("gate-decision.md");
        gate(&args(&[
            "evaluate",
            "--root",
            &repo_root().display().to_string(),
            "--pr-guidance",
            "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text =
            std::fs::read_to_string(&out).map_err(|err| format!("read gate json: {err}"))?;
        let md_text =
            std::fs::read_to_string(&out_md).map_err(|err| format!("read gate md: {err}"))?;
        assert!(json_text.contains("\"status\": \"advisory\""));
        assert!(json_text.contains("\"mode\": \"visible-only\""));
        assert!(md_text.contains("# RIPR Gate Decision"));
        assert!(md_text.contains("Decision: advisory"));
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove gate dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn gate_command_writes_blocked_report_before_error() -> Result<(), String> {
        let dir = unique_command_test_dir("gate-blocked");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create gate dir: {err}"))?;
        let out = dir.join("gate-decision.json");
        let result = gate(&args(&[
            "evaluate",
            "--root",
            &repo_root().display().to_string(),
            "--pr-guidance",
            "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
            "--mode",
            "acknowledgeable",
            "--out",
            &out.display().to_string(),
        ]));

        assert!(matches!(result, Err(message) if message.contains("blocked")));
        let json_text =
            std::fs::read_to_string(&out).map_err(|err| format!("read gate json: {err}"))?;
        assert!(json_text.contains("\"status\": \"blocked\""));
        assert!(json_text.contains("\"decision\": \"blocking\""));
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove gate dir: {err}"))?;
        Ok(())
    }
}
