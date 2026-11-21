use std::fs;

use crate::{json_escape, write_json_string_array};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrReadyStep {
    pub(crate) id: &'static str,
    pub(crate) command: &'static str,
    pub(crate) report: &'static str,
    pub(crate) required: bool,
    pub(crate) status: String,
    pub(crate) summary: String,
}

pub(crate) fn run_readiness_step(
    id: &'static str,
    command: &'static str,
    report: &'static str,
    required: bool,
    run_step: fn() -> Result<(), String>,
) -> PrReadyStep {
    match run_step() {
        Ok(()) => {
            let (status, summary) = pr_ready_report_outcome(report)
                .unwrap_or_else(|| ("pass".to_string(), "completed".to_string()));
            PrReadyStep {
                id,
                command,
                report,
                required,
                status,
                summary,
            }
        }
        Err(err) => PrReadyStep {
            id,
            command,
            report,
            required,
            status: if required { "fail" } else { "needs_attention" }.to_string(),
            summary: first_error_line(&err),
        },
    }
}

fn pr_ready_report_outcome(report: &str) -> Option<(String, String)> {
    let contents = fs::read_to_string(report).ok()?;
    let status = markdown_status_value(&contents)?;
    pr_ready_status_from_report_status(status).map(|step_status| {
        (
            step_status.to_string(),
            format!("report status: {status}; see {report}"),
        )
    })
}

fn markdown_status_value(contents: &str) -> Option<&str> {
    contents.lines().find_map(|line| {
        line.strip_prefix("Status:")
            .map(str::trim)
            .filter(|status| !status.is_empty())
    })
}

pub(crate) fn pr_ready_status_from_report_status(status: &str) -> Option<&'static str> {
    match status {
        "pass" => None,
        "fail" | "error" => Some("fail"),
        "warn" | "actionable" | "incomplete" | "blocked" | "needs_attention" => {
            Some("needs_attention")
        }
        _ => None,
    }
}

fn first_error_line(err: &str) -> String {
    err.lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or(err)
        .trim()
        .to_string()
}

pub(crate) fn pr_ready_status(steps: &[PrReadyStep]) -> &'static str {
    if steps.iter().any(|step| step.status == "fail") {
        "fail"
    } else if steps.iter().any(|step| step.status != "pass") {
        "actionable"
    } else {
        "pass"
    }
}

pub(crate) fn pr_ready_next_action(steps: &[PrReadyStep]) -> &'static str {
    match pr_ready_status(steps) {
        "fail" => {
            "repair blocking generated-evidence or worktree hygiene issues before opening or updating a PR"
        }
        "actionable" => {
            "review the attention items, then run cargo xtask check-pr for full gate receipts"
        }
        _ => "run cargo xtask check-pr",
    }
}

pub(crate) fn pr_ready_markdown(steps: &[PrReadyStep]) -> String {
    let status = pr_ready_status(steps);
    let mut body = format!("# ripr PR Ready\n\nStatus: {status}\nMode: advisory\n\n");
    body.push_str("## Next Action\n\n");
    body.push_str("- ");
    body.push_str(pr_ready_next_action(steps));
    body.push_str("\n\n");

    let attention = steps
        .iter()
        .filter(|step| step.status != "pass")
        .collect::<Vec<_>>();
    body.push_str("## Current Risks\n\n");
    if attention.is_empty() {
        body.push_str("- none detected\n\n");
    } else {
        for step in attention {
            body.push_str(&format!(
                "- `{}`: {} ({})\n",
                step.id, step.summary, step.status
            ));
        }
        body.push('\n');
    }

    body.push_str("## Step Summary\n\n");
    body.push_str("| Step | Status | Required | Command | Report |\n");
    body.push_str("| --- | --- | --- | --- | --- |\n");
    for step in steps {
        body.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            step.id, step.status, step.required, step.command, step.report
        ));
    }

    body.push_str("\n## Safe Repairs\n\n");
    for repair in pr_ready_safe_repairs() {
        body.push_str("- ");
        body.push_str(repair);
        body.push('\n');
    }

    body.push_str("\n## Generated Only\n\n");
    for artifact in pr_ready_generated_only() {
        body.push_str("- `");
        body.push_str(artifact);
        body.push_str("`\n");
    }

    body.push_str("\n## Stop / Judgment Required\n\n");
    for item in pr_ready_judgment_required() {
        body.push_str("- ");
        body.push_str(item);
        body.push('\n');
    }

    push_start_here_language_section(&mut body);

    body.push_str("\n## Next Commands\n\n```bash\ncargo xtask check-pr\n```\n");
    body
}

pub(crate) fn pr_ready_json(steps: &[PrReadyStep]) -> String {
    let mut body = "{\n".to_string();
    body.push_str("  \"schema_version\": \"0.1\",\n");
    body.push_str("  \"mode\": \"advisory\",\n");
    body.push_str(&format!(
        "  \"status\": \"{}\",\n",
        json_escape(pr_ready_status(steps))
    ));
    body.push_str(&format!(
        "  \"next_action\": \"{}\",\n",
        json_escape(pr_ready_next_action(steps))
    ));
    body.push_str("  \"steps\": [\n");
    for (index, step) in steps.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!("      \"id\": \"{}\",\n", json_escape(step.id)));
        body.push_str(&format!(
            "      \"command\": \"{}\",\n",
            json_escape(step.command)
        ));
        body.push_str(&format!(
            "      \"status\": \"{}\",\n",
            json_escape(&step.status)
        ));
        body.push_str(&format!("      \"required\": {},\n", step.required));
        body.push_str(&format!(
            "      \"report\": \"{}\",\n",
            json_escape(step.report)
        ));
        body.push_str(&format!(
            "      \"summary\": \"{}\"\n",
            json_escape(&step.summary)
        ));
        body.push_str("    }");
    }
    body.push_str("\n  ],\n");
    body.push_str("  \"safe_repairs\": [");
    write_json_string_array_from_strs(&mut body, pr_ready_safe_repairs());
    body.push_str("],\n");
    body.push_str("  \"generated_only\": [");
    write_json_string_array_from_strs(&mut body, pr_ready_generated_only());
    body.push_str("],\n");
    body.push_str("  \"judgment_required\": [");
    write_json_string_array_from_strs(&mut body, pr_ready_judgment_required());
    body.push_str("],\n");
    body.push_str("  \"next_commands\": [\"cargo xtask check-pr\"]\n");
    body.push_str("}\n");
    body
}

fn write_json_string_array_from_strs(body: &mut String, values: &[&str]) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            body.push_str(", ");
        }
        body.push('"');
        body.push_str(&json_escape(value));
        body.push('"');
    }
}

fn pr_ready_safe_repairs() -> &'static [&'static str] {
    &[
        "run cargo xtask fix-pr",
        "restore generated badge endpoint residue in ordinary PRs",
        "review target/ripr/reports/suggested-fixes.patch before applying deterministic fixes",
    ]
}

fn pr_ready_generated_only() -> &'static [&'static str] {
    &[
        "badges/*.json",
        "target/ripr/**",
        "crates/ripr/examples/sample/target/**",
        "target/ripr/receipts/**",
    ]
}

fn pr_ready_judgment_required() -> &'static [&'static str] {
    &[
        "badge endpoint refresh",
        "golden blessing",
        "suppression",
        "baseline adoption",
        "dependency exception",
        "branch protection",
        "policy authority change",
    ]
}

fn start_here_language_terms() -> &'static [&'static str] {
    &[
        "start here: open target/ripr/reports/start-here.md first when it exists",
        "safe next action: repair one named gap, regenerate missing evidence, or stop on no-action",
        "missing artifact / stale evidence / wrong root / malformed artifact: fail closed before repair work",
        "no actionable gap: advisory no-action, not runtime adequacy or mutation proof",
        "preview-limited evidence: syntax-first and advisory, with static limits before repair language",
        "verify command / receipt command / receipt path: static movement proof rail",
    ]
}

fn push_start_here_language_section(body: &mut String) {
    body.push_str("\n## Start-Here Language\n\n");
    for term in start_here_language_terms() {
        body.push_str("- ");
        body.push_str(term);
        body.push('\n');
    }
}

fn cockpit_next_action(steps: &[PrReadyStep]) -> &'static str {
    match pr_ready_status(steps) {
        "fail" => "repair blocking repo-ops issues before merging or opening more work",
        "actionable" => {
            "review the action queue, then run cargo xtask pr-ready or cargo xtask check-pr for the active PR"
        }
        _ => {
            "use cargo xtask pr-ready for the active PR or cargo xtask gh-pr-status --pr <number> before merging"
        }
    }
}

fn cockpit_action_queue(steps: &[PrReadyStep]) -> Vec<String> {
    let mut actions = Vec::new();
    for step in steps.iter().filter(|step| step.status != "pass") {
        let action = match step.id {
            "worktree_doctor" => "repair or acknowledge local worktree hygiene findings",
            "command_catalog_check" => {
                "update the command mutability catalog before adding new xtask commands"
            }
            "spec_numbering" => "repair spec numbering or README/traceability references",
            "campaign_status" => {
                "review campaign/source-of-truth drift before continuing broad work"
            }
            "pr_triage" => {
                "review stale, duplicate, behind, policy-sensitive, or generated-artifact PRs"
            }
            "generated_clean" => "remove generated residue before ordinary PR work",
            "badge_diff_policy" => "move badge endpoint JSON changes to a badge refresh PR",
            _ => "review the linked repo-ops report",
        };
        actions.push(action.to_string());
    }
    if actions.is_empty() {
        actions.push("run cargo xtask pr-ready for the active PR".to_string());
        actions.push("run cargo xtask gh-pr-status --pr <number> before merging".to_string());
    }
    actions
}

pub(crate) fn cockpit_markdown(steps: &[PrReadyStep]) -> String {
    let status = pr_ready_status(steps);
    let mut body = format!("# ripr Repo Cockpit\n\nStatus: {status}\nMode: advisory\n\n");
    body.push_str("## Next Action\n\n");
    body.push_str("- ");
    body.push_str(cockpit_next_action(steps));
    body.push_str("\n\n");

    body.push_str("## Action Queue\n\n");
    for action in cockpit_action_queue(steps) {
        body.push_str(&format!("- {action}\n"));
    }
    body.push('\n');

    let attention = steps
        .iter()
        .filter(|step| step.status != "pass")
        .collect::<Vec<_>>();
    body.push_str("## Current Risks\n\n");
    if attention.is_empty() {
        body.push_str("- none detected\n\n");
    } else {
        for step in attention {
            body.push_str(&format!(
                "- `{}`: {} ({})\n",
                step.id, step.summary, step.status
            ));
        }
        body.push('\n');
    }

    body.push_str("## Step Summary\n\n");
    body.push_str("| Step | Status | Required | Report |\n");
    body.push_str("| --- | --- | --- | --- |\n");
    for step in steps {
        body.push_str(&format!(
            "| `{}` | `{}` | {} | `{}` |\n",
            step.id, step.status, step.required, step.report
        ));
    }
    body.push('\n');

    body.push_str("## Safe Repairs\n\n");
    for item in pr_ready_safe_repairs() {
        body.push_str(&format!("- {item}\n"));
    }
    body.push('\n');

    body.push_str("## Generated Only\n\n");
    for item in pr_ready_generated_only() {
        body.push_str(&format!("- `{item}`\n"));
    }
    body.push('\n');

    body.push_str("## Stop / Judgment Required\n\n");
    for item in pr_ready_judgment_required() {
        body.push_str(&format!("- {item}\n"));
    }

    push_start_here_language_section(&mut body);

    body.push_str("## Next Commands\n\n```bash\ncargo xtask pr-ready\ncargo xtask check-pr\n```\n");
    body
}

pub(crate) fn cockpit_json(steps: &[PrReadyStep]) -> String {
    let action_queue = cockpit_action_queue(steps);
    let mut body = "{\n".to_string();
    body.push_str("  \"schema_version\": \"0.1\",\n");
    body.push_str("  \"mode\": \"advisory\",\n");
    body.push_str(&format!(
        "  \"status\": \"{}\",\n",
        json_escape(pr_ready_status(steps))
    ));
    body.push_str(&format!(
        "  \"next_action\": \"{}\",\n",
        json_escape(cockpit_next_action(steps))
    ));
    body.push_str("  \"action_queue\": [");
    write_json_string_array(&mut body, &action_queue);
    body.push_str("],\n");
    body.push_str("  \"steps\": [\n");
    for (index, step) in steps.iter().enumerate() {
        if index > 0 {
            body.push_str(",\n");
        }
        body.push_str("    {\n");
        body.push_str(&format!("      \"id\": \"{}\",\n", json_escape(step.id)));
        body.push_str(&format!(
            "      \"command\": \"{}\",\n",
            json_escape(step.command)
        ));
        body.push_str(&format!(
            "      \"status\": \"{}\",\n",
            json_escape(&step.status)
        ));
        body.push_str(&format!("      \"required\": {},\n", step.required));
        body.push_str(&format!(
            "      \"report\": \"{}\",\n",
            json_escape(step.report)
        ));
        body.push_str(&format!(
            "      \"summary\": \"{}\"\n",
            json_escape(&step.summary)
        ));
        body.push_str("    }");
    }
    body.push_str("\n  ],\n");
    body.push_str("  \"safe_repairs\": [");
    write_json_string_array_from_strs(&mut body, pr_ready_safe_repairs());
    body.push_str("],\n");
    body.push_str("  \"generated_only\": [");
    write_json_string_array_from_strs(&mut body, pr_ready_generated_only());
    body.push_str("],\n");
    body.push_str("  \"judgment_required\": [");
    write_json_string_array_from_strs(&mut body, pr_ready_judgment_required());
    body.push_str("],\n");
    body.push_str("  \"next_commands\": [\"cargo xtask pr-ready\", \"cargo xtask check-pr\"]\n");
    body.push_str("}\n");
    body
}
