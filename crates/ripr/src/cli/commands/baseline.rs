//! Arg-parsing and dispatch for `ripr baseline create / diff / update`.
//!
//! This is the CLI adapter layer only. Baseline report construction and
//! rendering live in `crate::output::baseline`, `crate::output::baseline_delta`,
//! and `crate::output::baseline_update`. This module owns argv parsing, output
//! destination selection, and exit mapping for the baseline command family.

use crate::cli::commands_options::{
    BaselineCreateOptions, BaselineDiffOptions, BaselineUpdateOptions,
};
use crate::cli::help;
use crate::output;
use std::path::PathBuf;

use super::{
    baseline_created_at, non_empty_path_arg, read_optional_text_for_report, write_text_file,
};

pub(in crate::cli) fn baseline(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_baseline_help();
        return Ok(());
    }
    let Some((subcommand, rest)) = args.split_first() else {
        return Err("baseline requires subcommand `create`, `diff`, or `update`".to_string());
    };
    match subcommand.as_str() {
        "create" => baseline_create(rest),
        "diff" => baseline_diff(rest),
        "update" => baseline_update(rest),
        _ => Err(format!(
            "unknown baseline subcommand {subcommand:?}; expected `create`, `diff`, or `update`"
        )),
    }
}

fn baseline_create(args: &[String]) -> Result<(), String> {
    let options = parse_baseline_create_options(args)?;
    let gate_decision_json = std::fs::read_to_string(&options.from).map_err(|err| {
        format!(
            "read baseline create source {} failed: {err}",
            output::baseline::display_path(&options.from)
        )
    })?;
    let created_at = baseline_created_at()?;
    let source_report = output::baseline::display_path(&options.from);
    let report = output::baseline::baseline_create_report_from_gate_decision_json(
        &source_report,
        &created_at,
        &gate_decision_json,
    )?;
    let rendered = output::baseline::render_baseline_create_json(&report)?;
    if options.dry_run {
        print!("{rendered}");
        return Ok(());
    }
    if options.out.exists() && !options.force {
        return Err(format!(
            "{} already exists; rerun `ripr baseline create --force` to overwrite it",
            options.out.display()
        ));
    }
    write_text_file(&options.out, &rendered)?;
    println!("Wrote {}", options.out.display());
    println!(
        "Entries: {}",
        output::baseline::baseline_entry_count(&report)
    );
    Ok(())
}

fn baseline_diff(args: &[String]) -> Result<(), String> {
    let options = parse_baseline_diff_options(args)?;
    let baseline_path = output::baseline_delta::display_path(&options.baseline);
    let current_path = output::baseline_delta::display_path(&options.current);
    let baseline_json = read_optional_text_for_report("baseline", &options.baseline);
    let current_json = read_optional_text_for_report("current gate-decision", &options.current);
    let report = output::baseline_delta::build_baseline_delta_report(
        output::baseline_delta::BaselineDeltaInput {
            root: ".".to_string(),
            baseline_path,
            current_gate_decision_path: current_path,
            baseline_json,
            current_gate_decision_json: current_json,
        },
    );
    let rendered_json = output::baseline_delta::render_baseline_delta_json(&report)?;
    let rendered_md = output::baseline_delta::render_baseline_delta_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    println!(
        "Items: {}",
        output::baseline_delta::baseline_delta_item_count(&report)
    );
    Ok(())
}

fn baseline_update(args: &[String]) -> Result<(), String> {
    let options = parse_baseline_update_options(args)?;
    if !options.remove_resolved {
        return Err(
            "baseline update requires --remove-resolved; adopting new debt is not supported"
                .to_string(),
        );
    }
    let baseline_path = output::baseline_update::display_path(&options.baseline);
    let current_path = output::baseline_update::display_path(&options.current);
    let baseline_json = std::fs::read_to_string(&options.baseline).map_err(|err| {
        format!(
            "read baseline update baseline {} failed: {err}",
            output::baseline_update::display_path(&options.baseline)
        )
    })?;
    let current_json = std::fs::read_to_string(&options.current).map_err(|err| {
        format!(
            "read baseline update current gate-decision {} failed: {err}",
            output::baseline_update::display_path(&options.current)
        )
    })?;
    let report = output::baseline_update::build_baseline_update_remove_resolved(
        output::baseline_update::BaselineUpdateInput {
            baseline_path,
            current_gate_decision_path: current_path,
            baseline_json,
            current_gate_decision_json: current_json,
        },
    )?;
    let rendered = output::baseline_update::render_baseline_update_json(&report)?;
    let out = options.out.unwrap_or_else(|| options.baseline.clone());
    write_text_file(&out, &rendered)?;
    println!("Wrote {}", out.display());
    println!(
        "Entries: {} -> {}",
        output::baseline_update::baseline_update_before_entry_count(&report),
        output::baseline_update::baseline_update_after_entry_count(&report)
    );
    println!(
        "Removed resolved: {}",
        output::baseline_update::baseline_update_removed_resolved_count(&report)
    );
    println!(
        "Ignored new current: {}",
        output::baseline_update::baseline_update_ignored_new_current_count(&report)
    );
    if output::baseline_update::baseline_update_warning_count(&report) > 0 {
        println!(
            "Warnings: {}",
            output::baseline_update::baseline_update_warning_count(&report)
        );
    }
    Ok(())
}

fn parse_baseline_create_options(args: &[String]) -> Result<BaselineCreateOptions, String> {
    let mut parse = BaselineCreateParseState::default();

    let mut i = 0usize;
    while i < args.len() {
        parse.apply_arg(args, &mut i)?;
        i += 1;
    }

    parse.into_options()
}

#[derive(Debug)]
struct BaselineCreateParseState {
    from: Option<PathBuf>,
    out: PathBuf,
    dry_run: bool,
    force: bool,
}

impl Default for BaselineCreateParseState {
    fn default() -> Self {
        Self {
            from: None,
            out: PathBuf::from(output::baseline::DEFAULT_BASELINE_OUT),
            dry_run: false,
            force: false,
        }
    }
}

impl BaselineCreateParseState {
    fn apply_arg(&mut self, args: &[String], i: &mut usize) -> Result<(), String> {
        match args[*i].as_str() {
            "--from" => self.parse_from(args, i),
            "--out" => self.parse_out(args, i),
            "--dry-run" => {
                self.dry_run = true;
                Ok(())
            }
            "--force" => {
                self.force = true;
                Ok(())
            }
            other => Err(format!("unknown baseline create argument {other:?}")),
        }
    }

    fn parse_from(&mut self, args: &[String], i: &mut usize) -> Result<(), String> {
        *i += 1;
        self.from = Some(non_empty_path_arg(args, *i, "--from", "baseline create")?);
        Ok(())
    }

    fn parse_out(&mut self, args: &[String], i: &mut usize) -> Result<(), String> {
        *i += 1;
        self.out = non_empty_path_arg(args, *i, "--out", "baseline create")?;
        Ok(())
    }

    fn into_options(self) -> Result<BaselineCreateOptions, String> {
        Ok(BaselineCreateOptions {
            from: self
                .from
                .ok_or_else(|| "baseline create requires --from <path>".to_string())?,
            out: self.out,
            dry_run: self.dry_run,
            force: self.force,
        })
    }
}

fn parse_baseline_diff_options(args: &[String]) -> Result<BaselineDiffOptions, String> {
    let mut baseline = None;
    let mut current = None;
    let mut out = PathBuf::from(output::baseline_delta::DEFAULT_BASELINE_DELTA_OUT);
    let mut out_md = PathBuf::from(output::baseline_delta::DEFAULT_BASELINE_DELTA_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--baseline" => {
                i += 1;
                baseline = Some(non_empty_path_arg(args, i, "--baseline", "baseline diff")?);
            }
            "--current" => {
                i += 1;
                current = Some(non_empty_path_arg(args, i, "--current", "baseline diff")?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "baseline diff")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "baseline diff")?;
            }
            other => return Err(format!("unknown baseline diff argument {other:?}")),
        }
        i += 1;
    }

    Ok(BaselineDiffOptions {
        baseline: baseline.ok_or_else(|| "baseline diff requires --baseline <path>".to_string())?,
        current: current.ok_or_else(|| "baseline diff requires --current <path>".to_string())?,
        out,
        out_md,
    })
}

fn parse_baseline_update_options(args: &[String]) -> Result<BaselineUpdateOptions, String> {
    let mut baseline = None;
    let mut current = None;
    let mut out = None;
    let mut remove_resolved = false;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--baseline" => {
                i += 1;
                baseline = Some(non_empty_path_arg(
                    args,
                    i,
                    "--baseline",
                    "baseline update",
                )?);
            }
            "--current" => {
                i += 1;
                current = Some(non_empty_path_arg(args, i, "--current", "baseline update")?);
            }
            "--out" => {
                i += 1;
                out = Some(non_empty_path_arg(args, i, "--out", "baseline update")?);
            }
            "--remove-resolved" => remove_resolved = true,
            other => return Err(format!("unknown baseline update argument {other:?}")),
        }
        i += 1;
    }

    Ok(BaselineUpdateOptions {
        baseline: baseline
            .ok_or_else(|| "baseline update requires --baseline <path>".to_string())?,
        current: current.ok_or_else(|| "baseline update requires --current <path>".to_string())?,
        out,
        remove_resolved,
    })
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::tests::{args, repo_root, unique_command_test_dir};
    use super::*;

    #[test]
    fn baseline_create_parses_option_surface() {
        assert_eq!(
            parse_baseline_create_options(&args(&[
                "--from",
                "target/ripr/reports/gate-decision.json",
                "--out",
                ".ripr/gate-baseline.json",
                "--dry-run",
                "--force",
            ])),
            Ok(BaselineCreateOptions {
                from: PathBuf::from("target/ripr/reports/gate-decision.json"),
                out: PathBuf::from(".ripr/gate-baseline.json"),
                dry_run: true,
                force: true,
            })
        );
        assert_eq!(
            parse_baseline_create_options(&args(&["--from", "gate.json"])),
            Ok(BaselineCreateOptions {
                from: PathBuf::from("gate.json"),
                out: PathBuf::from(".ripr/gate-baseline.json"),
                dry_run: false,
                force: false,
            })
        );
    }

    #[test]
    fn baseline_create_requires_source_and_rejects_unknown_args() {
        assert_eq!(
            baseline(&args(&[])),
            Err("baseline requires subcommand `create`, `diff`, or `update`".to_string())
        );
        assert_eq!(
            baseline(&args(&["unknown"])),
            Err(
                "unknown baseline subcommand \"unknown\"; expected `create`, `diff`, or `update`"
                    .to_string()
            )
        );
        assert_eq!(
            parse_baseline_create_options(&args(&[])),
            Err("baseline create requires --from <path>".to_string())
        );
        assert_eq!(
            parse_baseline_create_options(&args(&["--from", ""])),
            Err("baseline create --from requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_baseline_create_options(&args(&["--bad"])),
            Err("unknown baseline create argument \"--bad\"".to_string())
        );
    }

    #[test]
    fn baseline_diff_parses_option_surface() {
        assert_eq!(
            parse_baseline_diff_options(&args(&[
                "--baseline",
                ".ripr/gate-baseline.json",
                "--current",
                "target/ripr/reports/gate-decision.json",
                "--out",
                "target/ripr/reports/baseline-debt-delta.json",
                "--out-md",
                "target/ripr/reports/baseline-debt-delta.md",
            ])),
            Ok(BaselineDiffOptions {
                baseline: PathBuf::from(".ripr/gate-baseline.json"),
                current: PathBuf::from("target/ripr/reports/gate-decision.json"),
                out: PathBuf::from("target/ripr/reports/baseline-debt-delta.json"),
                out_md: PathBuf::from("target/ripr/reports/baseline-debt-delta.md"),
            })
        );
    }

    #[test]
    fn baseline_diff_requires_inputs_and_rejects_unknown_args() {
        assert_eq!(
            parse_baseline_diff_options(&args(&[])),
            Err("baseline diff requires --baseline <path>".to_string())
        );
        assert_eq!(
            parse_baseline_diff_options(&args(&["--baseline", ".ripr/gate-baseline.json"])),
            Err("baseline diff requires --current <path>".to_string())
        );
        assert_eq!(
            parse_baseline_diff_options(&args(&["--baseline", ""])),
            Err("baseline diff --baseline requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_baseline_diff_options(&args(&["--bad"])),
            Err("unknown baseline diff argument \"--bad\"".to_string())
        );
    }

    #[test]
    fn baseline_update_parses_option_surface() {
        assert_eq!(
            parse_baseline_update_options(&args(&[
                "--baseline",
                ".ripr/gate-baseline.json",
                "--current",
                "target/ripr/reports/gate-decision.json",
                "--remove-resolved",
                "--out",
                ".ripr/gate-baseline.updated.json",
            ])),
            Ok(BaselineUpdateOptions {
                baseline: PathBuf::from(".ripr/gate-baseline.json"),
                current: PathBuf::from("target/ripr/reports/gate-decision.json"),
                out: Some(PathBuf::from(".ripr/gate-baseline.updated.json")),
                remove_resolved: true,
            })
        );
        assert_eq!(
            parse_baseline_update_options(&args(&[
                "--baseline",
                ".ripr/gate-baseline.json",
                "--current",
                "target/ripr/reports/gate-decision.json",
            ])),
            Ok(BaselineUpdateOptions {
                baseline: PathBuf::from(".ripr/gate-baseline.json"),
                current: PathBuf::from("target/ripr/reports/gate-decision.json"),
                out: None,
                remove_resolved: false,
            })
        );
    }

    #[test]
    fn baseline_update_requires_inputs_remove_resolved_and_rejects_unknown_args() {
        assert_eq!(
            parse_baseline_update_options(&args(&[])),
            Err("baseline update requires --baseline <path>".to_string())
        );
        assert_eq!(
            parse_baseline_update_options(&args(&["--baseline", ".ripr/gate-baseline.json"])),
            Err("baseline update requires --current <path>".to_string())
        );
        assert_eq!(
            parse_baseline_update_options(&args(&["--baseline", ""])),
            Err("baseline update --baseline requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_baseline_update_options(&args(&["--bad"])),
            Err("unknown baseline update argument \"--bad\"".to_string())
        );
        assert_eq!(
            parse_baseline_update_options(&args(&["--adopt-new"])),
            Err("unknown baseline update argument \"--adopt-new\"".to_string())
        );
        assert_eq!(
            baseline(&args(&[
                "update",
                "--baseline",
                ".ripr/gate-baseline.json",
                "--current",
                "target/ripr/reports/gate-decision.json",
            ])),
            Err(
                "baseline update requires --remove-resolved; adopting new debt is not supported"
                    .to_string()
            )
        );
    }

    #[test]
    fn baseline_create_writes_baseline_without_overwriting_by_default() -> Result<(), String> {
        let dir = unique_command_test_dir("baseline-create");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create baseline dir: {err}"))?;
        let out = dir.join("gate-baseline.json");
        let from = repo_root().join(
            "fixtures/boundary_gap/expected/calibrated-gate/visible-only-advisory/gate-decision.json",
        );
        baseline(&args(&[
            "create",
            "--from",
            &from.display().to_string(),
            "--out",
            &out.display().to_string(),
        ]))?;

        let json_text =
            std::fs::read_to_string(&out).map_err(|err| format!("read baseline json: {err}"))?;
        assert!(json_text.contains("\"kind\": \"gate_baseline\""));
        assert!(json_text.contains("\"reviewed\": false"));
        assert!(json_text.contains("\"source_report\""));
        assert!(json_text.contains("\"seam_id\": \"8f7fa8644fd12280\""));
        assert!(json_text.contains("\"entries\": 1"));

        let second = baseline(&args(&[
            "create",
            "--from",
            &from.display().to_string(),
            "--out",
            &out.display().to_string(),
        ]));
        assert!(matches!(second, Err(message) if message.contains("--force")));

        baseline(&args(&[
            "create",
            "--from",
            &from.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--force",
        ]))?;

        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove baseline dir: {err}"))?;
        Ok(())
    }
}
