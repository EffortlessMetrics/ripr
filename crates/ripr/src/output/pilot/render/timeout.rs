use super::render_helpers::push_path_field;
use crate::output::json::escape as json_escape;
use crate::output::path::display_path;
use crate::output::pilot::commands::PilotCommands;
use crate::output::pilot::{PILOT_SUMMARY_SCHEMA_VERSION, PilotSummaryContext};

pub(crate) fn render_pilot_timeout_summary_json(context: PilotSummaryContext<'_>) -> String {
    let commands = PilotCommands::new(context);

    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": \"{}\",\n",
        PILOT_SUMMARY_SCHEMA_VERSION
    ));
    out.push_str("  \"tool\": \"ripr\",\n");
    out.push_str("  \"scope\": \"repo\",\n");
    out.push_str("  \"status\": \"partial\",\n");
    out.push_str("  \"reason\": \"timeout\",\n");
    out.push_str(&format!("  \"timeout_ms\": {},\n", context.timeout_ms));
    out.push_str("  \"completed_phases\": [],\n");
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

    out.push_str("  \"outputs_written\": [\n");
    out.push_str("    \"pilot_summary_json\",\n");
    out.push_str("    \"pilot_summary_md\"\n");
    out.push_str("  ],\n");
    out.push_str(&format!("  \"max_seams\": {},\n", context.max_seams));
    out.push_str("  \"actionable_seams_total\": null,\n");
    out.push_str("  \"top_actionable_seams\": [],\n");
    out.push_str("  \"next\": {\n");
    out.push_str(&format!(
        "    \"retry_command\": \"{}\"\n",
        json_escape(&commands.retry)
    ));
    out.push_str("  }\n");
    out.push_str("}\n");
    out
}

pub(crate) fn render_pilot_timeout_summary_md(context: PilotSummaryContext<'_>) -> String {
    let commands = PilotCommands::new(context);

    let mut out = String::new();
    out.push_str("# RIPR Pilot Summary\n\n");
    out.push_str("## Scope\n\n");
    out.push_str("- Status: `partial`\n");
    out.push_str(&format!(
        "- Reason: analysis timed out after {} ms\n",
        context.timeout_ms
    ));
    out.push_str(&format!("- Root: `{}`\n", display_path(context.root)));
    out.push_str(&format!("- Mode: `{}`\n", context.mode.as_str()));
    match context.config_path {
        Some(path) => out.push_str(&format!("- Config: loaded `{}`\n\n", display_path(path))),
        None => out.push_str("- Config: missing; using built-in defaults\n\n"),
    }

    out.push_str("## Outputs\n\n");
    out.push_str("Analysis did not finish within the pilot budget, so repo exposure and agent seam packet files were not written.\n\n");
    out.push_str(&format!(
        "- Pilot summary JSON: `{}`\n",
        display_path(&context.artifacts.pilot_summary_json)
    ));
    out.push_str(&format!(
        "- Pilot summary Markdown: `{}`\n\n",
        display_path(&context.artifacts.pilot_summary_md)
    ));

    out.push_str("## Next Command\n\n");
    out.push_str("Rerun with a larger explicit budget:\n\n");
    out.push_str("```bash\n");
    out.push_str(&commands.retry);
    out.push_str("\n```\n");
    out
}

pub(crate) fn render_pilot_timeout_terminal(context: PilotSummaryContext<'_>) -> String {
    let commands = PilotCommands::new(context);

    let mut out = String::new();
    out.push_str("RIPR pilot partial.\n\n");
    out.push_str("Reason:\n");
    out.push_str(&format!(
        "  analysis timed out after {} ms\n\n",
        context.timeout_ms
    ));
    out.push_str("Config:\n");
    match context.config_path {
        Some(path) => out.push_str(&format!("  loaded: {}\n", display_path(path))),
        None => out.push_str("  missing: using built-in defaults\n"),
    }
    out.push('\n');
    out.push_str("Written:\n");
    out.push_str(&format!(
        "  {}\n",
        display_path(&context.artifacts.pilot_summary_json)
    ));
    out.push_str(&format!(
        "  {}\n\n",
        display_path(&context.artifacts.pilot_summary_md)
    ));
    out.push_str("Next:\n");
    out.push_str(&format!("  {}\n", commands.retry));
    out
}
