use super::render_helpers::{
    NO_REPAIR_TARGET_FOCUSED_TEST, no_repair_target_hand_step, push_markdown_recommendation,
    push_path_field, push_top_seam_json, yes_no,
};
use super::why_line;
use crate::analysis::ClassifiedSeam;
use crate::output::agent_seam_packets::{
    suggested_assertion_for_classified_seam, targeted_test_brief_outline_for_classified_seam,
};
use crate::output::json::escape as json_escape;
use crate::output::markdown::{PowershellForm, powershell_form};
use crate::output::path::{display_path, display_path_text};
use crate::output::pilot::commands::{
    PilotCommands, python_card_first_pr_command, repair_start_command,
};
use crate::output::pilot::ranking::{actionable_total, top_actionable_seams};
use crate::output::pilot::{
    PILOT_SUMMARY_SCHEMA_VERSION, PilotLanguageRoute, PilotLanguageRoutes, PilotPythonFirstUse,
    PilotSummaryContext,
};
use crate::output::python_repair_card::PythonRepairCard;

const PYTHON_PREVIEW_SUPPORTED_FEATURES: &[&str] = &[
    "project_detection",
    "diff_owner_mapping",
    "pytest_oracle_facts",
    "unittest_oracle_facts",
    "repair_cards",
];

const PYTHON_PREVIEW_DEFERRED_FEATURES: &[&str] = &[
    "outcome_receipts",
    "runtime_mutation_execution",
    "gate_authority",
    "generated_tests",
];

pub(crate) fn render_pilot_summary_json(
    classified: &[ClassifiedSeam],
    context: PilotSummaryContext<'_>,
) -> String {
    let actionable_total = actionable_total(classified);
    let top = top_actionable_seams(classified, context.max_seams);
    let commands = PilotCommands::new(context);

    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": \"{}\",\n",
        PILOT_SUMMARY_SCHEMA_VERSION
    ));
    out.push_str("  \"tool\": \"ripr\",\n");
    out.push_str("  \"scope\": \"repo\",\n");
    out.push_str("  \"status\": \"complete\",\n");
    out.push_str(&format!(
        "  \"root\": \"{}\",\n",
        json_escape(&display_path(context.root))
    ));
    out.push_str(&format!("  \"mode\": \"{}\",\n", context.mode.as_str()));
    out.push_str("  \"config\": {");
    match context.config_path {
        Some(path) => out.push_str(&format!(
            "\"state\": \"loaded\", \"path\": \"{}\"",
            json_escape(&display_path(path))
        )),
        None => out.push_str("\"state\": \"missing\", \"path\": null"),
    }
    out.push_str("},\n");

    out.push_str("  \"outputs\": {\n");
    push_path_field(
        &mut out,
        "repo_exposure_json",
        &context.artifacts.repo_exposure_json,
        true,
    );
    push_path_field(
        &mut out,
        "repo_exposure_md",
        &context.artifacts.repo_exposure_md,
        true,
    );
    push_path_field(
        &mut out,
        "agent_seam_packets_json",
        &context.artifacts.agent_seam_packets_json,
        true,
    );
    push_path_field(
        &mut out,
        "pilot_summary_json",
        &context.artifacts.pilot_summary_json,
        true,
    );
    push_path_field(
        &mut out,
        "pilot_summary_md",
        &context.artifacts.pilot_summary_md,
        false,
    );
    out.push_str("  },\n");

    out.push_str(&format!("  \"max_seams\": {},\n", context.max_seams));
    out.push_str(&format!("  \"timeout_ms\": {},\n", context.timeout_ms));
    out.push_str("  \"outputs_written\": [\n");
    out.push_str("    \"repo_exposure_json\",\n");
    out.push_str("    \"repo_exposure_md\",\n");
    out.push_str("    \"agent_seam_packets_json\",\n");
    out.push_str("    \"pilot_summary_json\",\n");
    out.push_str("    \"pilot_summary_md\"\n");
    out.push_str("  ],\n");
    out.push_str(&format!(
        "  \"actionable_seams_total\": {},\n",
        actionable_total
    ));
    out.push_str("  \"top_actionable_seams\": [");
    for (idx, entry) in top.iter().enumerate() {
        if idx == 0 {
            out.push('\n');
        }
        push_top_seam_json(&mut out, entry);
        if idx + 1 != top.len() {
            out.push_str(",\n");
        } else {
            out.push('\n');
        }
    }
    if !top.is_empty() {
        out.push_str("  ");
    }
    out.push_str("],\n");
    push_python_first_use_json(&mut out, context.python_first_use);
    push_language_routes_json(&mut out, context.language_routes);
    out.push_str("  \"next\": {\n");
    out.push_str(&format!(
        "    \"inspect_packet\": \"{}\",\n",
        json_escape(&display_path(&context.artifacts.agent_seam_packets_json))
    ));
    out.push_str(&format!(
        "    \"after_snapshot_command\": \"{}\",\n",
        json_escape(&commands.after_snapshot)
    ));
    out.push_str(&format!(
        "    \"outcome_command\": \"{}\",\n",
        json_escape(&commands.outcome)
    ));
    match top
        .first()
        .and_then(|entry| repair_start_command(context.root, entry))
    {
        Some(command) => out.push_str(&format!(
            "    \"repair_command\": \"{}\"\n",
            json_escape(&command)
        )),
        None => out.push_str("    \"repair_command\": null\n"),
    }
    out.push_str("  }\n");
    out.push_str("}\n");
    out
}

pub(crate) fn render_pilot_summary_md(
    classified: &[ClassifiedSeam],
    context: PilotSummaryContext<'_>,
) -> String {
    let actionable_total = actionable_total(classified);
    let top = top_actionable_seams(classified, context.max_seams);
    let commands = PilotCommands::new(context);

    let mut out = String::new();
    out.push_str("# RIPR Pilot Summary\n\n");
    out.push_str("## What Was Inspected\n\n");
    out.push_str("- Status: `complete`\n");
    out.push_str(&format!("- Root: `{}`\n", display_path(context.root)));
    out.push_str(&format!("- Mode: `{}`\n", context.mode.as_str()));
    out.push_str(&format!("- Timeout: {} ms\n", context.timeout_ms));
    match context.config_path {
        Some(path) => out.push_str(&format!("- Config: loaded `{}`\n", display_path(path))),
        None => out.push_str("- Config: missing; using built-in defaults\n"),
    }
    out.push_str(&format!(
        "- Actionable seams: {} total, showing up to {}\n\n",
        actionable_total, context.max_seams
    ));

    let python_top = python_top_repair_card(context.python_first_use);
    if top.is_empty() {
        out.push_str("## Top Recommendation\n\n");
        if let Some(card) = python_top {
            push_python_repair_card_md(&mut out, card);
        } else if required_routes(context).is_some() {
            out.push_str(
                "Pilot ranks Rust seams and found none in this repository. This is not a clean result for the languages listed under Languages Outside The Rust Seam Scan.\n\n",
            );
        } else {
            out.push_str("No actionable seam was ranked by the default pilot policy.\n\n");
        }
    } else {
        out.push_str("## Top Recommendation\n\n");
        push_markdown_recommendation(&mut out, top[0]);
        out.push('\n');

        // A ranked seam is a gap worth reading, not a repair offer. When none
        // of them can start `ripr agent repair`, the heading must not call
        // them actionable (#4216 row 3).
        if top
            .iter()
            .any(|entry| repair_start_command(context.root, entry).is_some())
        {
            out.push_str("## Ranked Actionable Seams\n\n");
        } else {
            out.push_str("## Ranked Seams\n\n");
            out.push_str(
                "None of these seams can start a repair attempt (`ripr agent repair`); they are ranked for inspection by hand.\n\n",
            );
        }
        for (idx, entry) in top.iter().enumerate() {
            out.push_str(&format!(
                "{}. `{}` {} (`{}`) {}:{} `{}`\n",
                idx + 1,
                entry.seam.id().as_str(),
                entry.class.plain_label(),
                entry.class.as_str(),
                display_path(entry.seam.file()),
                entry.seam.display_line(),
                entry.seam.kind().as_str()
            ));
            out.push_str(&format!("   - Owner: `{}`\n", entry.seam.owner()));
            out.push_str(&format!("   - Why: {}\n", why_line(entry)));
            out.push_str(&format!(
                "   - Related test present: {}\n",
                yes_no(!entry.evidence.related_tests.is_empty())
            ));
            out.push_str(&format!(
                "   - Suggested assertion present: {}\n",
                yes_no(suggested_assertion_for_classified_seam(entry).is_some())
            ));
            out.push('\n');
        }
    }

    if let Some(first_use) = context.python_first_use {
        push_python_first_use_md(&mut out, first_use);
    }
    if let Some(routes) = required_routes(context) {
        push_language_routes_md(&mut out, routes);
    }

    out.push_str("## Outputs\n\n");
    out.push_str(&format!(
        "- Repo exposure JSON: `{}`\n",
        display_path(&context.artifacts.repo_exposure_json)
    ));
    out.push_str(&format!(
        "- Repo exposure Markdown: `{}`\n",
        display_path(&context.artifacts.repo_exposure_md)
    ));
    out.push_str(&format!(
        "- Agent seam packets: `{}`\n",
        display_path(&context.artifacts.agent_seam_packets_json)
    ));
    out.push_str(&format!(
        "- Pilot summary JSON: `{}`\n\n",
        display_path(&context.artifacts.pilot_summary_json)
    ));

    out.push_str("## Next Commands\n\n");
    let repair = top
        .first()
        .and_then(|entry| repair_start_command(context.root, entry));
    // One ordinary route (#3906): when the top seam can be repaired, the
    // repair transaction replaces the manual before/after snapshot pair.
    let routes = required_routes(context);
    let python_card = python_top_repair_card(context.python_first_use).filter(|_| top.is_empty());
    let python_first_pr = python_card_first_pr_command(context.root);
    let next_commands: Vec<&String> = match (repair.as_ref(), routes) {
        (Some(command), _) => {
            out.push_str(
                "Start the repair transaction for the top seam, add one focused test (test files only), then run the `--attempt ... --phase after` command it prints:\n\n",
            );
            vec![command]
        }
        // #3906: with no Rust seams, the repo-exposure snapshot pair would
        // only report that no seams moved. Route to the diff-first check that
        // analyzes the languages pilot did not rank.
        // rc rehearsal (py-pricing): a Python repair card is the top
        // recommendation, so the next commands follow its route; `ripr check`
        // would only lead back to pilot.
        (None, _) if python_card.is_some() => match python_card {
            Some(card) => {
                let edit = format!(
                    "{} `{}` in `{}` (test files only)",
                    capitalized(repair_action_label(&card.repair_action)),
                    card.suggested_test_name,
                    card.suggested_test_file
                );
                match card.receipt_command.as_ref() {
                    Some(receipt) => {
                        out.push_str(&format!(
                            "{edit}, then run the card's verify command and its receipt command:\n\n"
                        ));
                        vec![&card.verify_command, receipt]
                    }
                    None => {
                        out.push_str(&format!(
                            "Run `ripr first-pr` before the test edit: it names this gap's receipt command (run any regeneration command it prints first). Then {} `{}` in `{}` (test files only), run the card's verify command, and run that receipt command:\n\n",
                            repair_action_label(&card.repair_action),
                            card.suggested_test_name,
                            card.suggested_test_file
                        ));
                        vec![&python_first_pr, &card.verify_command]
                    }
                }
            }
            None => Vec::new(),
        },
        (None, Some(routes)) => {
            let commands = PilotLanguageRoutes::commands(routes);
            if commands.is_empty() {
                out.push_str(NO_LANGUAGE_ROUTE_COMMAND);
                out.push('\n');
                return out;
            }
            out.push_str(
                "Analyze the changed code in the languages outside the Rust seam scan with the diff-first check:\n\n",
            );
            commands
        }
        (None, None) => {
            match top.first() {
                Some(entry)
                    if targeted_test_brief_outline_for_classified_seam(entry)
                        .is_not_applicable() =>
                {
                    out.push_str(&format!(
                        "No repair attempt is available for the top seam. Next, {}, then rerun repo exposure and compare the snapshots:\n\n",
                        no_repair_target_hand_step(entry)
                    ));
                }
                _ => out.push_str(
                    "After adding one focused test, rerun repo exposure and compare the snapshots:\n\n",
                ),
            }
            vec![&commands.after_snapshot, &commands.outcome]
        }
    };
    out.push_str(super::COMMAND_SHELL_DISCLOSURE);
    out.push_str("```bash\n");
    for command in &next_commands {
        out.push_str(command);
        out.push('\n');
    }
    out.push_str("```\n");
    let mut unavailable: Vec<&String> = Vec::new();
    let mut translations: Vec<String> = Vec::new();
    let mut any_translated = false;
    for command in next_commands {
        match powershell_form(command) {
            PowershellForm::Translated(line) => {
                any_translated = true;
                translations.push(line);
            }
            PowershellForm::SameAsBash => translations.push(command.clone()),
            PowershellForm::Unavailable => unavailable.push(command),
        }
    }
    // Only fence translations that exist; a compound command under-emits to a
    // disclosure naming the bash form instead of an invalid translation. The
    // fence is the whole sequence, so it carries unchanged lines too, and is
    // omitted when every line runs unchanged in PowerShell.
    if any_translated {
        out.push_str("\n```powershell\n");
        for line in &translations {
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("\n```\n");
    }
    for command in unavailable {
        out.push_str(&format!(
            "{}: `{command}`\n",
            crate::output::markdown::POWERSHELL_UNAVAILABLE_DISCLOSURE
        ));
    }
    out
}

pub(crate) fn render_pilot_terminal(
    classified: &[ClassifiedSeam],
    context: PilotSummaryContext<'_>,
) -> String {
    let top = top_actionable_seams(classified, 1);
    let commands = PilotCommands::new(context);

    let mut out = String::new();
    out.push_str("RIPR pilot complete.\n\n");
    out.push_str("Inspected:\n");
    out.push_str(&format!("  root: {}\n", display_path(context.root)));
    out.push_str(&format!("  mode: {}\n", context.mode.as_str()));
    match context.config_path {
        Some(path) => out.push_str(&format!("  config: loaded {}\n", display_path(path))),
        None => out.push_str("  config: missing, using built-in defaults\n"),
    }
    out.push_str(&format!("  timeout: {} ms\n", context.timeout_ms));
    out.push('\n');

    let no_repair_target = if let Some(entry) = top.first() {
        let outline = targeted_test_brief_outline_for_classified_seam(entry);
        out.push_str("Top recommendation:\n");
        // The id leads the line, as it does in the Markdown sibling
        // (`render_helpers::push_markdown_recommendation`). Until this was
        // added, the terminal was the only one of the three pilot renderers
        // that dropped it, so a user who ran `ripr pilot` and read the screen
        // had no way to reach `ripr agent repair --seam-id <id>` — the step the
        // README names next — without opening a written artifact.
        out.push_str(&format!(
            "  inspected seam: {} {}:{} {} in {} ({})\n",
            entry.seam.id().as_str(),
            display_path(entry.seam.file()),
            entry.seam.display_line(),
            entry.seam.kind().as_str(),
            entry.seam.owner(),
            entry.class.human_label()
        ));
        out.push_str(&format!("  why it matters: {}\n", why_line(entry)));
        if outline.is_not_applicable() {
            out.push_str(&format!(
                "  focused test: {NO_REPAIR_TARGET_FOCUSED_TEST}\n"
            ));
        } else {
            out.push_str(&format!(
                "  focused test: add {} in {}\n",
                outline.suggested_name,
                display_path_text(&outline.suggested_file)
            ));
        }
        if let Some(value) = outline.candidate_value.as_ref() {
            out.push_str(&format!("  candidate value: {value}\n"));
        }
        out.push_str(&format!("  assertion: {}\n", outline.assertion_shape));
        // Only a seam that passes the fail-closed repair-packet flip gets the
        // paste-ready command. Route readiness alone is weaker: a ready seam
        // can still be ineligible, and offering a repair transaction there
        // would promise a target `agent repair` refuses. The closing block
        // uses the same builder, so the two lines cannot disagree (#3906).
        if let Some(command) = repair_start_command(context.root, entry) {
            out.push_str(&format!("  repair this seam: {command}\n"));
        }
        out.push('\n');
        outline.is_not_applicable()
    } else if let Some(card) = python_top_repair_card(context.python_first_use) {
        out.push_str("Top recommendation:\n");
        push_python_repair_card_terminal(&mut out, card);
        out.push('\n');
        false
    } else if required_routes(context).is_some() {
        out.push_str("Top recommendation:\n");
        out.push_str(
            "  none: pilot ranks Rust seams and found none here; see the languages below\n\n",
        );
        false
    } else {
        out.push_str("Top recommendation:\n");
        out.push_str("  none ranked by the default pilot policy\n\n");
        false
    };

    if let Some(first_use) = context.python_first_use {
        push_python_first_use_terminal(&mut out, first_use);
    }
    let routes = required_routes(context);
    if let Some(routes) = routes {
        push_language_routes_terminal(&mut out, routes);
    }

    out.push_str("Detailed brief:\n");
    out.push_str(&format!(
        "  {}\n",
        display_path(&context.artifacts.pilot_summary_md)
    ));
    out.push_str("Structured packet:\n");
    out.push_str(&format!(
        "  {}\n\n",
        display_path(&context.artifacts.agent_seam_packets_json)
    ));
    if let Some(command) = top
        .first()
        .and_then(|entry| repair_start_command(context.root, entry))
    {
        out.push_str("Next, in order:\n");
        out.push_str(&format!("  1. {command}\n"));
        out.push_str("  2. add the focused test named above (test files only)\n");
        out.push_str("  3. run the `--attempt ... --phase after` command that step 1 prints\n");
        out.push_str(
            "  (do not redirect these commands' output into the checkout, for example `> packet.json`: the edit cage counts that file as an edit; use target/ripr/ or a directory outside the repository)\n",
        );
        return out;
    }
    // rc rehearsal (py-pricing): with no Rust seam but a Python repair card,
    // the card is the top recommendation, so the closing block names the
    // card's route, not `ripr check`, which only leads back here.
    if top.is_empty()
        && let Some(card) = python_top_repair_card(context.python_first_use)
    {
        let edit = format!(
            "{} {} in {} (test files only): {}",
            repair_action_label(&card.repair_action),
            card.suggested_test_name,
            card.suggested_test_file,
            card.suggested_assertion
        );
        out.push_str("Next, in order:\n");
        if let Some(receipt) = card.receipt_command.as_deref() {
            out.push_str(&format!("  1. {edit}\n"));
            out.push_str(&format!("  2. {}\n", card.verify_command));
            out.push_str(&format!("  3. {receipt}\n"));
        } else {
            out.push_str(&format!(
                "  1. {} (names this gap's receipt command; run any regeneration command it prints first)\n",
                python_card_first_pr_command(context.root)
            ));
            out.push_str(&format!("  2. {edit}\n"));
            out.push_str(&format!("  3. {}\n", card.verify_command));
            out.push_str("  4. run the receipt command step 1 printed\n");
        }
        return out;
    }
    if let Some(routes) = routes {
        let commands = PilotLanguageRoutes::commands(routes);
        if commands.is_empty() {
            out.push_str(NO_LANGUAGE_ROUTE_COMMAND);
            out.push('\n');
        } else {
            out.push_str("Next, analyze the changed code in these languages:\n");
            for command in commands {
                out.push_str(&format!("  {command}\n"));
            }
        }
        return out;
    }
    if let Some(entry) = top.first().filter(|_| no_repair_target) {
        out.push_str(&format!(
            "Next, by hand: {}, then compare against this run:\n",
            no_repair_target_hand_step(entry)
        ));
    } else {
        out.push_str("Run after adding the focused test:\n");
    }
    out.push_str(&format!("  {}\n", commands.after_snapshot));
    out.push_str(&format!("  {}\n", commands.outcome));
    out
}

/// Closing line when every language pilot did not rank is unavailable in
/// this binary, so no runnable command exists.
const NO_LANGUAGE_ROUTE_COMMAND: &str =
    "No follow-up command applies: this ripr binary cannot analyze the languages listed above.";

/// Routes the human output must show: present only when pilot's Rust seam
/// scan produced no seams, so output for Rust seams stays unchanged.
fn required_routes<'a>(context: PilotSummaryContext<'a>) -> Option<&'a [PilotLanguageRoute]> {
    context
        .language_routes
        .and_then(PilotLanguageRoutes::required)
}

fn route_status_label(route: &PilotLanguageRoute) -> &'static str {
    match (route.available, route.enabled) {
        (false, _) => "not available in this build",
        (true, true) => "preview, diff-first",
        (true, false) => "preview, diff-first; not enabled in ripr.toml [languages]",
    }
}

fn file_count_label(count: usize) -> String {
    if count == 1 {
        "1 file".to_string()
    } else {
        format!("{count} files")
    }
}

fn push_language_routes_terminal(out: &mut String, routes: &[PilotLanguageRoute]) {
    out.push_str("Languages outside pilot's Rust seam scan:\n");
    for route in routes {
        out.push_str(&format!(
            "  {}: {} ({})\n",
            route.language.as_str(),
            file_count_label(route.file_count),
            route_status_label(route)
        ));
        // One label for every language: the guidance category id stays in
        // `pilot-summary.json` and the Markdown guidance line, not the label.
        if let Some(command) = route.command.as_deref() {
            out.push_str(&format!("    route: {command}\n"));
        } else if let Some(guidance) = route.guidance.as_deref() {
            out.push_str(&format!("    {guidance}\n"));
        }
    }
    out.push('\n');
}

fn push_language_routes_md(out: &mut String, routes: &[PilotLanguageRoute]) {
    out.push_str("## Languages Outside The Rust Seam Scan\n\n");
    for route in routes {
        out.push_str(&format!(
            "- `{}`: {} ({})\n",
            route.language.as_str(),
            file_count_label(route.file_count),
            route_status_label(route)
        ));
        if let Some(command) = route.command.as_deref() {
            out.push_str(&format!("  - Route: `{command}`\n"));
        }
        match (route.guidance_category, route.guidance.as_deref()) {
            (Some(category), Some(guidance)) => {
                out.push_str(&format!("  - `{category}`: {guidance}\n"));
            }
            (None, Some(guidance)) => out.push_str(&format!("  - {guidance}\n")),
            _ => {}
        }
    }
    out.push('\n');
}

fn push_language_routes_json(out: &mut String, routes: Option<&PilotLanguageRoutes>) {
    out.push_str("  \"language_routes\": ");
    let Some(routes) = routes else {
        out.push_str("null,\n");
        return;
    };
    out.push_str("{\n");
    json_string_field(out, 4, "state", routes.state.as_str(), true);
    out.push_str("    \"routes\": [");
    for (idx, route) in routes.routes.iter().enumerate() {
        out.push_str(if idx == 0 { "\n" } else { ",\n" });
        out.push_str("      {\n");
        json_string_field(out, 8, "language", route.language.as_str(), true);
        out.push_str(&format!("        \"file_count\": {},\n", route.file_count));
        json_string_field(out, 8, "language_status", route.language_status(), true);
        out.push_str(&format!("        \"enabled\": {},\n", route.enabled));
        json_string_field(out, 8, "route", route.route(), true);
        json_optional_string_field(out, 8, "command", route.command.as_deref(), true);
        json_optional_string_field(out, 8, "guidance_category", route.guidance_category, true);
        json_optional_string_field(out, 8, "guidance", route.guidance.as_deref(), false);
        out.push_str("      }");
    }
    if !routes.routes.is_empty() {
        out.push_str("\n    ");
    }
    out.push_str("]\n");
    out.push_str("  },\n");
}

fn python_top_repair_card(first_use: Option<&PilotPythonFirstUse>) -> Option<&PythonRepairCard> {
    first_use.and_then(|first_use| first_use.top_repair_card.as_ref())
}

fn push_python_first_use_json(out: &mut String, first_use: Option<&PilotPythonFirstUse>) {
    out.push_str("  \"python_first_use\": ");
    let Some(first_use) = first_use else {
        out.push_str("null,\n");
        return;
    };

    out.push_str("{\n");
    json_string_field(out, 4, "status", first_use.status.as_str(), true);
    json_string_field(out, 4, "language", "python", true);
    json_string_field(out, 4, "language_status", "preview", true);
    json_string_field(out, 4, "authority_boundary", "preview_advisory_only", true);
    out.push_str(&format!(
        "    \"findings_total\": {},\n",
        first_use.findings_total
    ));
    out.push_str(&format!(
        "    \"repair_cards_total\": {},\n",
        first_use.repair_cards_total
    ));
    out.push_str(&format!(
        "    \"limitation_count\": {},\n",
        first_use.limitation_count
    ));
    json_optional_string_field(
        out,
        4,
        "analysis_error",
        first_use.analysis_error.as_deref(),
        true,
    );
    json_string_array_field(
        out,
        4,
        "supported_features",
        PYTHON_PREVIEW_SUPPORTED_FEATURES,
        true,
    );
    json_string_array_field(
        out,
        4,
        "deferred_features",
        PYTHON_PREVIEW_DEFERRED_FEATURES,
        true,
    );
    out.push_str("    \"top_repair_card\": ");
    if let Some(card) = first_use.top_repair_card.as_ref() {
        push_python_repair_card_json(out, card, 4);
        out.push('\n');
    } else {
        out.push_str("null\n");
    }
    out.push_str("  },\n");
}

fn push_python_repair_card_json(out: &mut String, card: &PythonRepairCard, indent: usize) {
    let sp = " ".repeat(indent);
    out.push_str("{\n");
    json_string_field(out, indent + 2, "card_version", &card.card_version, true);
    json_string_field(out, indent + 2, "source", &card.source, true);
    json_string_field(
        out,
        indent + 2,
        "canonical_gap_id",
        &card.canonical_gap_id,
        true,
    );
    json_string_field(out, indent + 2, "language", &card.language, true);
    json_string_field(
        out,
        indent + 2,
        "language_status",
        &card.language_status,
        true,
    );
    json_string_field(
        out,
        indent + 2,
        "authority_boundary",
        &card.authority_boundary,
        true,
    );
    json_string_field(out, indent + 2, "repair_action", &card.repair_action, true);
    json_string_field(out, indent + 2, "changed_owner", &card.changed_owner, true);
    json_string_field(
        out,
        indent + 2,
        "changed_behavior",
        &card.changed_behavior,
        true,
    );
    json_string_field(
        out,
        indent + 2,
        "current_test_evidence",
        &card.current_test_evidence,
        true,
    );
    json_string_field(
        out,
        indent + 2,
        "missing_discriminator",
        &card.missing_discriminator,
        true,
    );
    json_string_field(
        out,
        indent + 2,
        "recommended_test_shape",
        &card.recommended_test_shape,
        true,
    );
    json_string_field(
        out,
        indent + 2,
        "suggested_assertion",
        &card.suggested_assertion,
        true,
    );
    json_string_field(
        out,
        indent + 2,
        "suggested_test_file",
        &card.suggested_test_file,
        true,
    );
    json_string_field(
        out,
        indent + 2,
        "suggested_test_name",
        &card.suggested_test_name,
        true,
    );
    json_optional_string_field(
        out,
        indent + 2,
        "suggested_test_node_id",
        card.suggested_test_node_id.as_deref(),
        true,
    );
    json_string_field(
        out,
        indent + 2,
        "verify_command",
        &card.verify_command,
        true,
    );
    json_string_field(
        out,
        indent + 2,
        "verify_command_confidence",
        &card.verify_command_confidence,
        true,
    );
    json_optional_string_field(
        out,
        indent + 2,
        "receipt_command",
        card.receipt_command.as_deref(),
        true,
    );
    json_string_field(
        out,
        indent + 2,
        "receipt_status",
        &card.receipt_status,
        true,
    );
    json_string_field(
        out,
        indent + 2,
        "receipt_guidance",
        &card.receipt_guidance,
        true,
    );
    json_string_array_field_refs(
        out,
        indent + 2,
        "stop_conditions",
        &card.stop_conditions,
        true,
    );
    json_string_array_field_refs(out, indent + 2, "limits", &card.limits, false);
    out.push_str(&format!("{sp}}}"));
}

fn json_string_field(out: &mut String, indent: usize, name: &str, value: &str, trailing: bool) {
    out.push_str(&format!(
        "{}\"{}\": \"{}\"{}\n",
        " ".repeat(indent),
        name,
        json_escape(value),
        if trailing { "," } else { "" }
    ));
}

fn json_optional_string_field(
    out: &mut String,
    indent: usize,
    name: &str,
    value: Option<&str>,
    trailing: bool,
) {
    let sp = " ".repeat(indent);
    match value {
        Some(value) => out.push_str(&format!(
            "{sp}\"{name}\": \"{}\"{}\n",
            json_escape(value),
            if trailing { "," } else { "" }
        )),
        None => out.push_str(&format!(
            "{sp}\"{name}\": null{}\n",
            if trailing { "," } else { "" }
        )),
    }
}

fn json_string_array_field(
    out: &mut String,
    indent: usize,
    name: &str,
    values: &[&str],
    trailing: bool,
) {
    let owned = values
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    json_string_array_field_refs(out, indent, name, &owned, trailing);
}

fn json_string_array_field_refs(
    out: &mut String,
    indent: usize,
    name: &str,
    values: &[String],
    trailing: bool,
) {
    let sp = " ".repeat(indent);
    out.push_str(&format!("{sp}\"{name}\": ["));
    for (idx, value) in values.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&format!("\"{}\"", json_escape(value)));
    }
    out.push_str(&format!("]{}\n", if trailing { "," } else { "" }));
}

fn push_python_first_use_md(out: &mut String, first_use: &PilotPythonFirstUse) {
    out.push_str("## Python Preview First Use\n\n");
    out.push_str(&format!("- Status: `{}`\n", first_use.status.as_str()));
    out.push_str("- Language: `python` (`preview`)\n");
    out.push_str("- Boundary: `preview_advisory_only`\n");
    out.push_str(&format!(
        "- Python findings: `{}`\n",
        first_use.findings_total
    ));
    out.push_str(&format!(
        "- Repair cards: `{}`\n",
        first_use.repair_cards_total
    ));
    out.push_str(&format!(
        "- Limitations: `{}`\n",
        first_use.limitation_count
    ));
    if let Some(error) = first_use.analysis_error.as_deref() {
        out.push_str(&format!("- Analysis note: `{}`\n", error));
    }
    if let Some(card) = first_use.top_repair_card.as_ref() {
        out.push('\n');
        push_python_repair_card_md(out, card);
    } else {
        out.push_str("\nNo Python repair card was selected for this run.\n\n");
    }
}

fn push_python_repair_card_md(out: &mut String, card: &PythonRepairCard) {
    out.push_str("- Top Python repairable gap:\n");
    out.push_str(&format!("  - Gap: `{}`\n", card.canonical_gap_id));
    out.push_str(&format!("  - Repair action: `{}`\n", card.repair_action));
    out.push_str(&format!("  - Changed owner: `{}`\n", card.changed_owner));
    out.push_str(&format!(
        "  - Changed behavior: {}\n",
        card.changed_behavior
    ));
    out.push_str(&format!(
        "  - Current test evidence: {}\n",
        card.current_test_evidence
    ));
    out.push_str(&format!(
        "  - Missing discriminator: `{}`\n",
        card.missing_discriminator
    ));
    out.push_str(&format!(
        "  - Recommended test shape: {}\n",
        card.recommended_test_shape
    ));
    out.push_str(&format!(
        "  - Suggested assertion: {}\n",
        card.suggested_assertion
    ));
    out.push_str(&format!(
        "  - Suggested test target: `{}` in `{}`\n",
        card.suggested_test_name, card.suggested_test_file
    ));
    out.push_str(&format!("  - Verify: `{}`\n", card.verify_command));
    if let Some(command) = card.receipt_command.as_deref() {
        out.push_str(&format!("  - Receipt: `{command}`\n"));
    } else {
        out.push_str(&format!("  - Receipt status: `{}`\n", card.receipt_status));
    }
    out.push_str(&format!(
        "  - Receipt guidance: {}\n",
        card.receipt_guidance
    ));
    out.push('\n');
}

fn push_python_first_use_terminal(out: &mut String, first_use: &PilotPythonFirstUse) {
    out.push_str("Python preview:\n");
    out.push_str(&format!("  status: {}\n", first_use.status.as_str()));
    out.push_str("  language: python (preview)\n");
    out.push_str(&format!("  findings: {}\n", first_use.findings_total));
    out.push_str(&format!(
        "  repair cards: {}\n",
        first_use.repair_cards_total
    ));
    out.push_str(&format!("  limitations: {}\n", first_use.limitation_count));
    if let Some(error) = first_use.analysis_error.as_deref() {
        out.push_str(&format!("  analysis note: {error}\n"));
    }
    if first_use.top_repair_card.is_none() {
        out.push_str("  top repair card: none\n");
    }
    out.push('\n');
}

fn push_python_repair_card_terminal(out: &mut String, card: &PythonRepairCard) {
    out.push_str("  language: python (preview)\n");
    out.push_str(&format!("  gap: {}\n", card.canonical_gap_id));
    out.push_str(&format!("  repair action: {}\n", card.repair_action));
    out.push_str(&format!("  changed owner: {}\n", card.changed_owner));
    out.push_str(&format!("  changed behavior: {}\n", card.changed_behavior));
    out.push_str(&format!(
        "  current test evidence: {}\n",
        card.current_test_evidence
    ));
    out.push_str(&format!(
        "  missing discriminator: {}\n",
        card.missing_discriminator
    ));
    out.push_str(&format!(
        "  recommended repair: {} {} in {}\n",
        repair_action_label(&card.repair_action),
        card.suggested_test_name,
        card.suggested_test_file
    ));
    out.push_str(&format!("  test shape: {}\n", card.recommended_test_shape));
    out.push_str(&format!("  assertion: {}\n", card.suggested_assertion));
    out.push_str(&format!("  verify: {}\n", card.verify_command));
    if let Some(command) = card.receipt_command.as_deref() {
        out.push_str(&format!("  receipt: {command}\n"));
    } else {
        out.push_str(&format!("  receipt status: {}\n", card.receipt_status));
    }
    out.push_str(&format!("  receipt guidance: {}\n", card.receipt_guidance));
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

fn repair_action_label(action: &str) -> &'static str {
    match action {
        "strengthen_existing_test" => "strengthen",
        _ => "add or strengthen",
    }
}
