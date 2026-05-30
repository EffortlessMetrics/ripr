use super::render_helpers::{
    push_markdown_recommendation, push_path_field, push_top_seam_json, yes_no,
};
use super::why_line;
use crate::analysis::ClassifiedSeam;
use crate::output::agent_seam_packets::{
    suggested_assertion_for_classified_seam, targeted_test_brief_outline_for_classified_seam,
};
use crate::output::json::escape as json_escape;
use crate::output::path::{display_path, display_path_text};
use crate::output::pilot::commands::PilotCommands;
use crate::output::pilot::ranking::{actionable_total, top_actionable_seams};
use crate::output::pilot::{
    PILOT_SUMMARY_SCHEMA_VERSION, PilotPythonFirstUse, PilotSummaryContext,
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

    let python_top = python_top_repair_card(context.python_first_use);
    if top.is_empty() {
        out.push_str("## Top Recommendation\n\n");
        if let Some(card) = python_top {
            push_python_repair_card_md(&mut out, card);
        } else {
            out.push_str("No actionable seam was ranked by the default pilot policy.\n\n");
        }
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

    if let Some(first_use) = context.python_first_use {
        push_python_first_use_md(&mut out, first_use);
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
    } else if let Some(card) = python_top_repair_card(context.python_first_use) {
        out.push_str("Top recommendation:\n");
        push_python_repair_card_terminal(&mut out, card);
        out.push('\n');
    } else {
        out.push_str("Top recommendation:\n");
        out.push_str("  none ranked by the default pilot policy\n\n");
    }

    if let Some(first_use) = context.python_first_use {
        push_python_first_use_terminal(&mut out, first_use);
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
        "  - Suggested test: `{}` in `{}`\n",
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
        "  recommended test: add {} in {}\n",
        card.suggested_test_name, card.suggested_test_file
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
