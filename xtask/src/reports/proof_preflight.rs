//! `cargo xtask proof preflight` — execute the routed proof locally
//! (docs/PROOF_ROUTING.md, slice 4).
//!
//! The command reuses the `proof route` routing core (`route_proof`) to map
//! the files changed between a base and head revision onto the proof packs in
//! `policy/proof-packs.toml`, then runs each routed pack's REQUIRED commands
//! in manifest order, deduplicated across packs. Advisory commands are never
//! executed; they are listed in the receipt as `advisory_not_run`. The
//! receipt at `target/ripr/reports/proof-preflight.{json,md}` is a local,
//! advisory preflight receipt: it does not replace CI, and every CI lane
//! still runs as configured.

use super::proof_route::{
    ProofRoute, ProofRouteOptions, changed_files, load_ci_lanes, parse_route_options,
    resolve_commit_sha, route_proof,
};
use crate::policy::proof_packs::load_proof_packs;
use crate::run::capture_output_with_timeout;
use serde_json::{Value, json};
use std::time::Duration;

const PROOF_PREFLIGHT_SCHEMA_VERSION: &str = "0.1";
const PROOF_PREFLIGHT_JSON: &str = "proof-preflight.json";
const PROOF_PREFLIGHT_MD: &str = "proof-preflight.md";
const PROOF_PREFLIGHT_MODE: &str = "advisory-local-preflight";
/// Generous per-command ceiling; routed packs can include full workspace
/// builds and test runs.
const COMMAND_TIMEOUT: Duration = Duration::from_hours(2);
/// How much trailing output a failed command keeps in the receipt.
const OUTPUT_TAIL_LINES: usize = 40;

/// How preflight runs one command: tests inject a fake recording runner; the
/// real runner shells out through `crate::run`.
type CommandRunner<'a> = &'a mut dyn FnMut(&str, &[String]) -> Result<CommandOutcome, String>;

const STATUS_PASS: &str = "pass";
const STATUS_FAIL: &str = "fail";
const STATUS_NOT_RUN: &str = "not_run";

pub(super) fn proof_preflight(args: &[String]) -> Result<(), String> {
    let options = parse_route_options(args)?;
    let packs = load_proof_packs()?;
    let lanes = load_ci_lanes()?;
    let base_sha = resolve_commit_sha(&options.base)?;
    let head_sha = resolve_commit_sha(&options.head)?;
    let changed = changed_files(&options.base, &options.head)?;
    let route = route_proof(&packs, &lanes, &changed);
    let plan = preflight_plan(&route);

    println!(
        "proof preflight: {} routed pack(s), {} required command(s) to run, {} advisory command(s) not run",
        plan.routed_pack_ids.len(),
        plan.required.len(),
        plan.advisory_not_run.len()
    );

    let total = plan.required.len();
    let mut index = 0usize;
    let mut runner = |program: &str, command_args: &[String]| {
        index += 1;
        eprintln!("[{index}/{total}] $ {program} {}", command_args.join(" "));
        process_runner(program, command_args)
    };
    let results = execute_required(&plan.required, &mut runner);
    let receipt = assemble_receipt(&options, &base_sha, &head_sha, &plan, results);
    write_receipt(&receipt)?;

    match first_failure(&receipt.commands) {
        None => {
            println!(
                "proof preflight: {} ({} command(s) ran)",
                receipt.overall_status,
                receipt
                    .commands
                    .iter()
                    .filter(|result| result.status != STATUS_NOT_RUN)
                    .count()
            );
            Ok(())
        }
        Some(failed) => Err(format!(
            "proof preflight failed: pack(s) [{}] command `{}` exited with {}; see target/ripr/reports/{PROOF_PREFLIGHT_MD}; rerun: cargo xtask proof preflight --base {} --head {}",
            failed.packs.join(", "),
            failed.command,
            failed
                .exit_code
                .map_or_else(|| "no exit code".to_string(), |code| format!("code {code}")),
            options.base,
            options.head
        )),
    }
}

/// One deduplicated command in the execution plan, with every routed pack
/// that names it.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PlannedCommand {
    command: String,
    packs: Vec<String>,
}

/// The route-derived execution plan: required commands in manifest order,
/// deduplicated across packs, plus the advisory commands that stay unrun.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PreflightPlan {
    routed_pack_ids: Vec<String>,
    full_proof: bool,
    release_proof_required: bool,
    required: Vec<PlannedCommand>,
    advisory_not_run: Vec<PlannedCommand>,
}

/// Derive the execution plan from the routing decision. Routing itself stays
/// in `route_proof`; this only flattens the routed packs' command lists.
fn preflight_plan(route: &ProofRoute) -> PreflightPlan {
    let routed_pack_ids = route
        .routed_packs
        .iter()
        .map(|pack| pack.id.clone())
        .collect();
    let mut required: Vec<PlannedCommand> = Vec::new();
    for pack in &route.routed_packs {
        for command in &pack.required_commands {
            merge_planned_command(&mut required, command, &pack.id);
        }
    }
    let mut advisory_not_run: Vec<PlannedCommand> = Vec::new();
    for pack in &route.routed_packs {
        for command in &pack.advisory_commands {
            // A command required by any routed pack already runs; it is not
            // additionally listed as advisory.
            if required.iter().any(|entry| entry.command == *command) {
                continue;
            }
            merge_planned_command(&mut advisory_not_run, command, &pack.id);
        }
    }
    PreflightPlan {
        routed_pack_ids,
        full_proof: route.full_proof,
        release_proof_required: route.release_proof_required,
        required,
        advisory_not_run,
    }
}

fn merge_planned_command(plan: &mut Vec<PlannedCommand>, command: &str, pack_id: &str) {
    if let Some(existing) = plan.iter_mut().find(|entry| entry.command == command) {
        if !existing.packs.iter().any(|id| id == pack_id) {
            existing.packs.push(pack_id.to_string());
        }
    } else {
        plan.push(PlannedCommand {
            command: command.to_string(),
            packs: vec![pack_id.to_string()],
        });
    }
}

/// What running one command produced, independent of how it was run. Tests
/// inject a fake runner returning these; the real runner shells out through
/// `crate::run`.
#[derive(Clone, Debug, Eq, PartialEq)]
struct CommandOutcome {
    success: bool,
    exit_code: Option<i32>,
    output: String,
    duration_ms: u64,
}

/// One executed (or skipped) command in the receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
struct CommandResult {
    command: String,
    packs: Vec<String>,
    status: &'static str,
    duration_ms: u64,
    exit_code: Option<i32>,
    output_tail: Option<String>,
}

/// The sanctioned process path: `crate::run` is the repository command
/// runner (`policy/process_allowlist.txt`). Commands come from the manifest
/// as `cargo ...` strings, split on whitespace and run from the repo root
/// without shell interpretation.
fn process_runner(program: &str, args: &[String]) -> Result<CommandOutcome, String> {
    let context = format!("proof preflight command `{program} {}`", args.join(" "));
    let output = capture_output_with_timeout(program, args, &[], COMMAND_TIMEOUT, &context)?;
    let success = !output.timed_out && output.status.is_some_and(|status| status.success());
    let mut combined = output.stdout;
    combined.push_str(&output.stderr);
    if output.timed_out {
        combined.push_str(&format!(
            "\n(proof preflight terminated the command after {}s)",
            COMMAND_TIMEOUT.as_secs()
        ));
    }
    Ok(CommandOutcome {
        success,
        exit_code: output.status.and_then(|status| status.code()),
        output: combined,
        duration_ms: u64::try_from(output.duration.as_millis()).unwrap_or(u64::MAX),
    })
}

/// Run the planned required commands in order, failing fast: the first
/// failure stops execution and the remaining commands are recorded as
/// `not_run`. A runner error (for example a missing program) is recorded as
/// a failure of that command, not a silent skip.
fn execute_required(planned: &[PlannedCommand], runner: CommandRunner<'_>) -> Vec<CommandResult> {
    let mut results = Vec::new();
    let mut failed = false;
    for entry in planned {
        if failed {
            results.push(CommandResult {
                command: entry.command.clone(),
                packs: entry.packs.clone(),
                status: STATUS_NOT_RUN,
                duration_ms: 0,
                exit_code: None,
                output_tail: None,
            });
            continue;
        }
        let outcome = run_planned_command(entry, runner);
        let status = if outcome.success {
            STATUS_PASS
        } else {
            STATUS_FAIL
        };
        let output_tail = if outcome.success {
            None
        } else {
            Some(output_tail(&outcome.output))
        };
        if !outcome.success {
            failed = true;
        }
        results.push(CommandResult {
            command: entry.command.clone(),
            packs: entry.packs.clone(),
            status,
            duration_ms: outcome.duration_ms,
            exit_code: outcome.exit_code,
            output_tail,
        });
    }
    results
}

fn run_planned_command(entry: &PlannedCommand, runner: CommandRunner<'_>) -> CommandOutcome {
    let mut parts = entry.command.split_whitespace().map(str::to_string);
    let Some(program) = parts.next() else {
        return failed_outcome(format!("proof pack command is empty: `{}`", entry.command));
    };
    let args: Vec<String> = parts.collect();
    match runner(&program, &args) {
        Ok(outcome) => outcome,
        Err(err) => failed_outcome(err),
    }
}

fn failed_outcome(message: String) -> CommandOutcome {
    CommandOutcome {
        success: false,
        exit_code: None,
        output: message,
        duration_ms: 0,
    }
}

fn output_tail(output: &str) -> String {
    let lines: Vec<&str> = output.lines().collect();
    let start = lines.len().saturating_sub(OUTPUT_TAIL_LINES);
    lines
        .get(start..)
        .unwrap_or_default()
        .join("\n")
        .trim_end()
        .to_string()
}

fn first_failure(results: &[CommandResult]) -> Option<&CommandResult> {
    results.iter().find(|result| result.status == STATUS_FAIL)
}

/// The preflight receipt. Both the JSON and the markdown render from this
/// one struct so they cannot disagree.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PreflightReceipt {
    base_rev: String,
    base_sha: String,
    head_rev: String,
    head_sha: String,
    routed_pack_ids: Vec<String>,
    full_proof: bool,
    release_proof_required: bool,
    commands: Vec<CommandResult>,
    advisory_not_run: Vec<PlannedCommand>,
    overall_status: &'static str,
}

fn assemble_receipt(
    options: &ProofRouteOptions,
    base_sha: &str,
    head_sha: &str,
    plan: &PreflightPlan,
    commands: Vec<CommandResult>,
) -> PreflightReceipt {
    let overall_status = if first_failure(&commands).is_some() {
        STATUS_FAIL
    } else {
        STATUS_PASS
    };
    PreflightReceipt {
        base_rev: options.base.clone(),
        base_sha: base_sha.to_string(),
        head_rev: options.head.clone(),
        head_sha: head_sha.to_string(),
        routed_pack_ids: plan.routed_pack_ids.clone(),
        full_proof: plan.full_proof,
        release_proof_required: plan.release_proof_required,
        commands,
        advisory_not_run: plan.advisory_not_run.clone(),
        overall_status,
    }
}

fn write_receipt(receipt: &PreflightReceipt) -> Result<(), String> {
    let json_text = serde_json::to_string_pretty(&preflight_json(receipt))
        .map_err(|err| format!("serialize proof preflight receipt: {err}"))?;
    crate::write_report(PROOF_PREFLIGHT_JSON, &format!("{json_text}\n"))?;
    crate::write_report(PROOF_PREFLIGHT_MD, &preflight_markdown(receipt))?;
    println!("Wrote target/ripr/reports/{PROOF_PREFLIGHT_JSON}");
    println!("Wrote target/ripr/reports/{PROOF_PREFLIGHT_MD}");
    Ok(())
}

fn preflight_json(receipt: &PreflightReceipt) -> Value {
    let commands: Vec<Value> = receipt
        .commands
        .iter()
        .map(|result| {
            json!({
                "command": result.command,
                "packs": result.packs,
                "status": result.status,
                "duration_ms": result.duration_ms,
                "exit_code": result.exit_code,
                "output_tail": result.output_tail,
            })
        })
        .collect();
    let advisory_not_run: Vec<Value> = receipt
        .advisory_not_run
        .iter()
        .map(|entry| {
            json!({
                "command": entry.command,
                "packs": entry.packs,
            })
        })
        .collect();
    json!({
        "schema_version": PROOF_PREFLIGHT_SCHEMA_VERSION,
        "mode": PROOF_PREFLIGHT_MODE,
        "base": {"rev": receipt.base_rev, "sha": receipt.base_sha},
        "head": {"rev": receipt.head_rev, "sha": receipt.head_sha},
        "routed_packs": receipt.routed_pack_ids,
        "full_proof": receipt.full_proof,
        "release_proof_required": receipt.release_proof_required,
        "overall_status": receipt.overall_status,
        "commands": commands,
        "advisory_not_run": advisory_not_run,
    })
}

fn preflight_markdown(receipt: &PreflightReceipt) -> String {
    let mut body = String::from("# ripr proof preflight receipt\n\n");
    body.push_str(&format!("Status: {}\n", receipt.overall_status));
    body.push_str(&format!("Mode: {PROOF_PREFLIGHT_MODE}\n"));
    body.push_str(&format!(
        "Release proof required: {}\n\n",
        receipt.release_proof_required
    ));
    body.push_str(
        "This is a local preflight receipt and advisory evidence only: it runs the routed \
         packs' required proof commands on the contributor machine. It does not replace CI; \
         every CI lane still runs as configured.\n\n",
    );
    body.push_str("Range:\n\n");
    body.push_str(&format!(
        "- base: `{}` (`{}`)\n",
        receipt.base_rev, receipt.base_sha
    ));
    body.push_str(&format!(
        "- head: `{}` (`{}`)\n\n",
        receipt.head_rev, receipt.head_sha
    ));

    body.push_str(&format!(
        "## Routed packs ({})\n\n",
        receipt.routed_pack_ids.len()
    ));
    if receipt.routed_pack_ids.is_empty() {
        body.push_str("No proof pack routed for this change; nothing to run.\n\n");
    } else {
        for pack_id in &receipt.routed_pack_ids {
            body.push_str(&format!("- {pack_id}\n"));
        }
        body.push('\n');
    }
    if receipt.full_proof {
        body.push_str(
            "An unknown surface routed this change to the full proof: every pack's required \
             commands are in the plan.\n\n",
        );
    }

    body.push_str(&format!(
        "## Required commands ({})\n\n",
        receipt.commands.len()
    ));
    if receipt.commands.is_empty() {
        body.push_str("No required commands were planned.\n\n");
    } else {
        body.push_str("| Command | Packs | Status | Duration (ms) |\n");
        body.push_str("| --- | --- | --- | --- |\n");
        for result in &receipt.commands {
            body.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                result.command,
                result.packs.join(", "),
                result.status,
                result.duration_ms
            ));
        }
        body.push('\n');
    }

    if let Some(failed) = first_failure(&receipt.commands) {
        body.push_str("## Failure\n\n");
        body.push_str(&format!("- pack(s): {}\n", failed.packs.join(", ")));
        body.push_str(&format!("- command: `{}`\n", failed.command));
        body.push_str(&format!(
            "- exit code: {}\n\n",
            failed
                .exit_code
                .map_or_else(|| "(none)".to_string(), |code| code.to_string())
        ));
        if let Some(tail) = &failed.output_tail {
            body.push_str(&format!("Last output lines:\n\n```text\n{tail}\n```\n\n"));
        }
    }

    body.push_str(&format!(
        "## Advisory commands not run ({})\n\n",
        receipt.advisory_not_run.len()
    ));
    if receipt.advisory_not_run.is_empty() {
        body.push_str("No advisory commands were routed.\n\n");
    } else {
        body.push_str(
            "Advisory commands stay visible without blocking; the preflight never executes \
             them.\n\n",
        );
        body.push_str("| Command | Packs |\n");
        body.push_str("| --- | --- |\n");
        for entry in &receipt.advisory_not_run {
            body.push_str(&format!(
                "| `{}` | {} |\n",
                entry.command,
                entry.packs.join(", ")
            ));
        }
        body.push('\n');
    }

    body.push_str(&format!(
        "Rerun: `cargo xtask proof preflight --base {} --head {}`\n",
        receipt.base_rev, receipt.head_rev
    ));
    body
}

#[cfg(test)]
mod tests {
    use super::super::proof_route::test_support::route_for;
    use super::*;

    fn plan_for(files: &[&str]) -> Result<PreflightPlan, String> {
        Ok(preflight_plan(&route_for(files)?))
    }

    fn plan_commands(plan: &PreflightPlan) -> Vec<&str> {
        plan.required
            .iter()
            .map(|entry| entry.command.as_str())
            .collect()
    }

    fn advisory_commands(plan: &PreflightPlan) -> Vec<&str> {
        plan.advisory_not_run
            .iter()
            .map(|entry| entry.command.as_str())
            .collect()
    }

    /// Fake runner: records every invocation and fails on a chosen command.
    struct FakeRunner {
        invocations: Vec<String>,
        fail_on: Option<String>,
    }

    impl FakeRunner {
        fn passing() -> Self {
            Self {
                invocations: Vec::new(),
                fail_on: None,
            }
        }

        fn failing_on(command: &str) -> Self {
            Self {
                invocations: Vec::new(),
                fail_on: Some(command.to_string()),
            }
        }

        fn run(&mut self, program: &str, args: &[String]) -> Result<CommandOutcome, String> {
            let command = if args.is_empty() {
                program.to_string()
            } else {
                format!("{program} {}", args.join(" "))
            };
            self.invocations.push(command.clone());
            let fails = self.fail_on.as_deref() == Some(command.as_str());
            Ok(CommandOutcome {
                success: !fails,
                exit_code: Some(if fails { 101 } else { 0 }),
                output: if fails {
                    "line one\nline two: something broke".to_string()
                } else {
                    "ok".to_string()
                },
                duration_ms: 5,
            })
        }
    }

    fn execute_with(
        plan: &PreflightPlan,
        fake: &mut FakeRunner,
    ) -> Result<Vec<CommandResult>, String> {
        let mut runner = |program: &str, args: &[String]| -> Result<CommandOutcome, String> {
            fake.run(program, args)
        };
        Ok(execute_required(&plan.required, &mut runner))
    }

    #[test]
    fn docs_only_plan_contains_only_docs_pack_commands() -> Result<(), String> {
        let plan = plan_for(&["docs/specs/SPEC-0001-example.md"])?;
        assert_eq!(plan.routed_pack_ids, vec!["docs-spec"]);
        assert!(!plan.full_proof);
        assert_eq!(
            plan_commands(&plan),
            vec![
                "cargo xtask check-spec-format",
                "cargo xtask check-spec-numbering",
                "cargo xtask check-doc-index",
                "cargo xtask check-static-language",
                "cargo xtask check-local-context",
            ]
        );
        for entry in &plan.required {
            assert_eq!(entry.packs, vec!["docs-spec"]);
        }
        assert_eq!(
            advisory_commands(&plan),
            vec!["cargo xtask markdown-links", "cargo xtask check-doc-roles",]
        );
        Ok(())
    }

    #[test]
    fn docs_only_plan_excludes_release_commands() -> Result<(), String> {
        // The release boundary: release-package commands enter the plan only
        // when the release pack is routed (matched or full proof). A docs-only
        // diff routes neither.
        let plan = plan_for(&["docs/specs/SPEC-0001-example.md"])?;
        assert!(!plan.release_proof_required);
        let commands = plan_commands(&plan);
        for release_command in [
            "cargo xtask check-pr",
            "cargo package -p ripr --list",
            "cargo publish -p ripr --dry-run",
        ] {
            assert!(
                !commands.contains(&release_command),
                "docs-only plan should not contain `{release_command}`: {commands:?}"
            );
        }
        let mut fake = FakeRunner::passing();
        execute_with(&plan, &mut fake)?;
        assert!(
            !fake
                .invocations
                .iter()
                .any(|invocation| invocation.contains("publish") || invocation.contains("package")),
            "docs-only preflight must not execute release commands: {:?}",
            fake.invocations
        );
        Ok(())
    }

    #[test]
    fn release_file_plan_includes_release_commands() -> Result<(), String> {
        let plan = plan_for(&["Cargo.toml"])?;
        assert_eq!(plan.routed_pack_ids, vec!["release-package"]);
        assert!(plan.release_proof_required);
        assert_eq!(
            plan_commands(&plan),
            vec![
                "cargo test --workspace",
                "cargo clippy --workspace --all-targets -- -D warnings",
                "cargo xtask check-pr",
                "cargo package -p ripr --list",
                "cargo publish -p ripr --dry-run",
                "cargo xtask release-readiness",
            ]
        );
        Ok(())
    }

    #[test]
    fn unknown_file_plan_is_the_conservative_full_plan() -> Result<(), String> {
        let plan = plan_for(&["scripts/unknown-surface.bin"])?;
        assert!(plan.full_proof);
        assert!(plan.release_proof_required);
        // Every pack routes, in manifest order.
        assert_eq!(
            plan.routed_pack_ids,
            vec![
                "docs-spec",
                "static-language",
                "output-contracts",
                "traceability-capabilities",
                "xtask-report",
                "analysis-fixture",
                "editor-lsp",
                "release-package",
            ]
        );
        let commands = plan_commands(&plan);
        for expected in [
            "cargo xtask check-spec-format",
            "cargo xtask check-output-contracts",
            "cargo test --workspace",
            "cargo xtask vscode-compile",
            "cargo publish -p ripr --dry-run",
        ] {
            assert!(
                commands.contains(&expected),
                "full plan should contain `{expected}`: {commands:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn plan_deduplicates_commands_across_packs() -> Result<(), String> {
        // crates/ripr/src/output/** is in both static-language and
        // output-contracts; both require check-output-contracts.
        let plan = plan_for(&["crates/ripr/src/output/human.rs"])?;
        let commands = plan_commands(&plan);
        let occurrences = commands
            .iter()
            .filter(|command| **command == "cargo xtask check-output-contracts")
            .count();
        assert_eq!(
            occurrences, 1,
            "shared command should appear once: {commands:?}"
        );
        let shared = plan
            .required
            .iter()
            .find(|entry| entry.command == "cargo xtask check-output-contracts")
            .ok_or("plan should contain the shared command")?;
        assert_eq!(shared.packs, vec!["static-language", "output-contracts"]);
        // One execution per distinct command string.
        let mut fake = FakeRunner::passing();
        execute_with(&plan, &mut fake)?;
        let executed = fake
            .invocations
            .iter()
            .filter(|invocation| *invocation == "cargo xtask check-output-contracts")
            .count();
        assert_eq!(executed, 1, "invocations: {:?}", fake.invocations);
        Ok(())
    }

    #[test]
    fn advisory_commands_are_listed_but_never_executed() -> Result<(), String> {
        let plan = plan_for(&["docs/specs/SPEC-0001-example.md"])?;
        let advisory = advisory_commands(&plan);
        assert!(advisory.contains(&"cargo xtask markdown-links"));
        let mut fake = FakeRunner::passing();
        execute_with(&plan, &mut fake)?;
        for advisory_command in advisory {
            assert!(
                !fake
                    .invocations
                    .iter()
                    .any(|invocation| invocation == advisory_command),
                "advisory command `{advisory_command}` must not be executed: {:?}",
                fake.invocations
            );
        }
        Ok(())
    }

    #[test]
    fn advisory_command_required_elsewhere_runs_as_required_only() -> Result<(), String> {
        // analysis-fixture requires `cargo xtask goldens check` and lists
        // `cargo xtask golden-drift` as advisory; output-contracts lists
        // golden-drift advisory too. A command required by any routed pack
        // never appears as advisory.
        let plan = plan_for(&[
            "crates/ripr/src/output/human.rs",
            "fixtures/sample/input.diff",
        ])?;
        let commands = plan_commands(&plan);
        assert!(commands.contains(&"cargo xtask goldens check"));
        assert!(
            !advisory_commands(&plan).contains(&"cargo xtask goldens check"),
            "required command must not be listed advisory: {:?}",
            plan.advisory_not_run
        );
        Ok(())
    }

    #[test]
    fn failure_stops_execution_and_marks_the_rest_not_run() -> Result<(), String> {
        let plan = plan_for(&["docs/specs/SPEC-0001-example.md"])?;
        let mut fake = FakeRunner::failing_on("cargo xtask check-spec-numbering");
        let results = execute_with(&plan, &mut fake)?;
        assert_eq!(results.len(), plan.required.len());
        assert_eq!(results[0].status, STATUS_PASS);
        assert_eq!(results[1].status, STATUS_FAIL);
        assert_eq!(results[1].exit_code, Some(101));
        let tail = results[1]
            .output_tail
            .as_deref()
            .ok_or("failed command should record an output tail")?;
        assert!(tail.contains("something broke"));
        for result in results.iter().skip(2) {
            assert_eq!(result.status, STATUS_NOT_RUN);
            assert_eq!(result.exit_code, None);
        }
        // Fail fast: nothing after the failing command was invoked.
        assert_eq!(fake.invocations.len(), 2);
        Ok(())
    }

    #[test]
    fn receipt_json_and_markdown_render_the_same_struct() -> Result<(), String> {
        let plan = plan_for(&["docs/specs/SPEC-0001-example.md"])?;
        let mut fake = FakeRunner::failing_on("cargo xtask check-doc-index");
        let results = execute_with(&plan, &mut fake)?;
        let options = ProofRouteOptions {
            base: "origin/main".to_string(),
            head: "HEAD".to_string(),
        };
        let receipt = assemble_receipt(&options, "base-sha", "head-sha", &plan, results);
        assert_eq!(receipt.overall_status, STATUS_FAIL);

        let json_value = preflight_json(&receipt);
        assert_eq!(
            json_value.get("schema_version").and_then(Value::as_str),
            Some(PROOF_PREFLIGHT_SCHEMA_VERSION)
        );
        assert_eq!(
            json_value.get("mode").and_then(Value::as_str),
            Some(PROOF_PREFLIGHT_MODE)
        );
        assert_eq!(
            json_value
                .get("release_proof_required")
                .and_then(Value::as_bool),
            Some(receipt.release_proof_required)
        );
        assert_eq!(
            json_value.get("overall_status").and_then(Value::as_str),
            Some(STATUS_FAIL)
        );
        let json_commands = json_value
            .get("commands")
            .and_then(Value::as_array)
            .ok_or("receipt json should carry `commands`")?;
        assert_eq!(json_commands.len(), receipt.commands.len());

        let markdown = preflight_markdown(&receipt);
        assert!(markdown.contains("Status: fail"));
        assert!(markdown.contains(&format!("Mode: {PROOF_PREFLIGHT_MODE}")));
        assert!(markdown.contains("local preflight receipt"));
        assert!(markdown.contains("does not replace CI"));
        for result in &receipt.commands {
            assert!(
                markdown.contains(&format!("| `{}` |", result.command)),
                "markdown should list `{}`",
                result.command
            );
        }
        assert!(markdown.contains("## Failure"));
        assert!(markdown.contains("- command: `cargo xtask check-doc-index`"));
        assert!(
            markdown
                .contains("Rerun: `cargo xtask proof preflight --base origin/main --head HEAD`")
        );
        Ok(())
    }

    #[test]
    fn passing_run_produces_a_pass_receipt_with_release_flag_from_route() -> Result<(), String> {
        let plan = plan_for(&["Cargo.toml"])?;
        let mut fake = FakeRunner::passing();
        let results = execute_with(&plan, &mut fake)?;
        let options = ProofRouteOptions {
            base: "origin/main".to_string(),
            head: "HEAD".to_string(),
        };
        let receipt = assemble_receipt(&options, "base-sha", "head-sha", &plan, results);
        assert_eq!(receipt.overall_status, STATUS_PASS);
        assert!(receipt.release_proof_required);
        assert!(receipt.commands.iter().all(|r| r.status == STATUS_PASS));
        let json_value = preflight_json(&receipt);
        assert_eq!(
            json_value
                .get("release_proof_required")
                .and_then(Value::as_bool),
            Some(true)
        );
        Ok(())
    }

    #[test]
    fn output_tail_keeps_only_the_last_lines() {
        let long: Vec<String> = (0..100).map(|index| format!("line {index}")).collect();
        let tail = output_tail(&long.join("\n"));
        let lines: Vec<&str> = tail.lines().collect();
        assert_eq!(lines.len(), OUTPUT_TAIL_LINES);
        assert_eq!(lines[0], "line 60");
        assert_eq!(lines[OUTPUT_TAIL_LINES - 1], "line 99");
        let short = output_tail("only line");
        assert_eq!(short, "only line");
    }
}
