use crate::analysis::ClassifiedSeam;
use crate::output::agent_seam_packets::{
    suggested_assertion_for_classified_seam, targeted_test_brief_for_classified_seam,
    targeted_test_brief_outline_for_classified_seam,
};
use crate::output::json::escape as json_escape;
use crate::output::path::{display_path, display_path_text};
use crate::output::pilot::commands::PilotCommands;
use crate::output::pilot::ranking::{actionable_total, top_actionable_seams};
use crate::output::pilot::{PILOT_SUMMARY_SCHEMA_VERSION, PilotSummaryContext};
use std::path::Path;

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
        "    \"outcome_command\": \"{}\"\n",
        json_escape(&commands.outcome)
    ));
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

    if top.is_empty() {
        out.push_str("## Top Recommendation\n\n");
        out.push_str("No actionable seam was ranked by the default pilot policy.\n\n");
    } else {
        out.push_str("## Top Recommendation\n\n");
        push_markdown_recommendation(&mut out, top[0]);
        out.push('\n');

        out.push_str("## Ranked Actionable Seams\n\n");
        for (idx, entry) in top.iter().enumerate() {
            out.push_str(&format!(
                "{}. `{}` `{}` {}:{} `{}`\n",
                idx + 1,
                entry.seam.id().as_str(),
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
    out.push_str(
        "After adding one focused test, rerun repo exposure and compare the snapshots:\n\n",
    );
    out.push_str("```bash\n");
    out.push_str(&commands.after_snapshot);
    out.push('\n');
    out.push_str(&commands.outcome);
    out.push_str("\n```\n");
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

    if let Some(entry) = top.first() {
        let outline = targeted_test_brief_outline_for_classified_seam(entry);
        out.push_str("Top recommendation:\n");
        out.push_str(&format!(
            "  inspected seam: {}:{} {} in {} ({})\n",
            display_path(entry.seam.file()),
            entry.seam.display_line(),
            entry.seam.kind().as_str(),
            entry.seam.owner(),
            entry.class.as_str()
        ));
        out.push_str(&format!("  why it matters: {}\n", why_line(entry)));
        out.push_str(&format!(
            "  focused test: add {} in {}\n",
            outline.suggested_name,
            display_path_text(&outline.suggested_file)
        ));
        if let Some(value) = outline.candidate_value.as_ref() {
            out.push_str(&format!("  candidate value: {value}\n"));
        }
        out.push_str(&format!("  assertion: {}\n\n", outline.assertion_shape));
    } else {
        out.push_str("Top recommendation:\n");
        out.push_str("  none ranked by the default pilot policy\n\n");
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
    out.push_str("Run after adding the focused test:\n");
    out.push_str(&format!("  {}\n", commands.after_snapshot));
    out.push_str(&format!("  {}\n", commands.outcome));
    out
}

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

fn push_top_seam_json(out: &mut String, entry: &ClassifiedSeam) {
    out.push_str("    {\n");
    out.push_str(&format!(
        "      \"seam_id\": \"{}\",\n",
        json_escape(entry.seam.id().as_str())
    ));
    out.push_str(&format!(
        "      \"file\": \"{}\",\n",
        json_escape(&display_path(entry.seam.file()))
    ));
    out.push_str(&format!("      \"line\": {},\n", entry.seam.display_line()));
    out.push_str(&format!(
        "      \"kind\": \"{}\",\n",
        entry.seam.kind().as_str()
    ));
    out.push_str(&format!(
        "      \"owner\": \"{}\",\n",
        json_escape(entry.seam.owner())
    ));
    out.push_str(&format!(
        "      \"grip_class\": \"{}\",\n",
        entry.class.as_str()
    ));
    out.push_str(&format!(
        "      \"why\": \"{}\",\n",
        json_escape(&why_line(entry))
    ));
    out.push_str("      \"missing_discriminator\": ");
    if let Some(missing) = entry.evidence.missing_discriminators.first() {
        out.push_str(&format!(
            "{{\"value\": \"{}\", \"reason\": \"{}\"}}",
            json_escape(&missing.value),
            json_escape(&missing.reason)
        ));
    } else {
        out.push_str("null");
    }
    out.push_str(",\n");
    out.push_str(&format!(
        "      \"related_test_present\": {},\n",
        !entry.evidence.related_tests.is_empty()
    ));
    out.push_str(&format!(
        "      \"suggested_assertion_present\": {},\n",
        suggested_assertion_for_classified_seam(entry).is_some()
    ));
    out.push_str(&format!(
        "      \"targeted_test_brief\": \"{}\"\n",
        json_escape(&targeted_test_brief_for_classified_seam(entry))
    ));
    out.push_str("    }");
}

fn push_markdown_recommendation(out: &mut String, entry: &ClassifiedSeam) {
    let outline = targeted_test_brief_outline_for_classified_seam(entry);
    out.push_str(&format!(
        "- Inspected seam: `{}` {}:{} `{}` in `{}` (`{}`)\n",
        entry.seam.id().as_str(),
        display_path(entry.seam.file()),
        entry.seam.display_line(),
        entry.seam.kind().as_str(),
        entry.seam.owner(),
        entry.class.as_str()
    ));
    out.push_str(&format!("- Why it matters: {}\n", why_line(entry)));
    out.push_str(&format!(
        "- Focused test: add `{}` in `{}`\n",
        outline.suggested_name,
        display_path_text(&outline.suggested_file)
    ));
    if let Some(value) = outline.candidate_value.as_ref() {
        out.push_str(&format!("- Candidate value: `{value}`\n"));
    }
    out.push_str(&format!(
        "- Assertion shape: `{}`\n",
        outline.assertion_shape
    ));
    out.push_str("- Detailed work order:\n\n");
    out.push_str("```text\n");
    out.push_str(&targeted_test_brief_for_classified_seam(entry));
    out.push_str("```\n");
}

fn push_path_field(out: &mut String, name: &str, path: &Path, trailing: bool) {
    out.push_str(&format!(
        "    \"{}\": \"{}\"{}\n",
        name,
        json_escape(&display_path(path)),
        if trailing { "," } else { "" }
    ));
}

pub(super) fn why_line(entry: &ClassifiedSeam) -> String {
    if let Some(missing) = entry.evidence.missing_discriminators.first() {
        return format!(
            "missing discriminator: {} ({})",
            missing.value, missing.reason
        );
    }
    let summary = entry.evidence.discriminate.summary.trim();
    if !summary.is_empty() {
        return format!("static discriminator summary: {summary}");
    }
    format!("{} static seam evidence", entry.class.as_str())
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
