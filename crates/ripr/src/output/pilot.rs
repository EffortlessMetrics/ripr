//! Render the first-run pilot packet summary.
//!
//! `ripr pilot` joins existing repo-exposure and agent packet artifacts with
//! one small operator summary. It does not change classification semantics or
//! run additional analysis.

use crate::agent::loop_commands::{self, display_path};
use crate::analysis::ClassifiedSeam;
use crate::analysis::seams::SeamGripClass;
use crate::app::Mode;
use crate::output::agent_seam_packets::{
    suggested_assertion_for_classified_seam, targeted_test_brief_for_classified_seam,
    targeted_test_brief_outline_for_classified_seam,
};
use crate::output::json::escape as json_escape;
use crate::output::path::display_path_text;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

pub(crate) const PILOT_SUMMARY_SCHEMA_VERSION: &str = "0.2";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PilotArtifacts {
    pub(crate) repo_exposure_json: PathBuf,
    pub(crate) repo_exposure_md: PathBuf,
    pub(crate) agent_seam_packets_json: PathBuf,
    pub(crate) pilot_summary_json: PathBuf,
    pub(crate) pilot_summary_md: PathBuf,
}

#[derive(Clone, Copy)]
pub(crate) struct PilotSummaryContext<'a> {
    pub(crate) root: &'a Path,
    pub(crate) mode: &'a Mode,
    pub(crate) config_path: Option<&'a Path>,
    pub(crate) max_seams: usize,
    pub(crate) timeout_ms: u64,
    pub(crate) artifacts: &'a PilotArtifacts,
}

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

pub(crate) fn top_actionable_seams(
    classified: &[ClassifiedSeam],
    max_seams: usize,
) -> Vec<&ClassifiedSeam> {
    let mut actionable = classified
        .iter()
        .filter(|entry| class_rank(entry.class).is_some())
        .collect::<Vec<_>>();
    actionable.sort_by(|left, right| compare_ranked_seams(left, right));
    actionable.truncate(max_seams);
    actionable
}

fn actionable_total(classified: &[ClassifiedSeam]) -> usize {
    classified
        .iter()
        .filter(|entry| class_rank(entry.class).is_some())
        .count()
}

fn compare_ranked_seams(left: &ClassifiedSeam, right: &ClassifiedSeam) -> Ordering {
    class_rank(left.class)
        .cmp(&class_rank(right.class))
        .then(
            bool_rank(!left.evidence.missing_discriminators.is_empty()).cmp(&bool_rank(
                !right.evidence.missing_discriminators.is_empty(),
            )),
        )
        .then(
            bool_rank(!left.evidence.related_tests.is_empty())
                .cmp(&bool_rank(!right.evidence.related_tests.is_empty())),
        )
        .then(
            bool_rank(suggested_assertion_for_classified_seam(left).is_some()).cmp(&bool_rank(
                suggested_assertion_for_classified_seam(right).is_some(),
            )),
        )
        .then(display_path(left.seam.file()).cmp(&display_path(right.seam.file())))
        .then(left.seam.display_line().cmp(&right.seam.display_line()))
        .then(left.seam.kind().as_str().cmp(right.seam.kind().as_str()))
        .then(left.seam.id().as_str().cmp(right.seam.id().as_str()))
}

fn class_rank(class: SeamGripClass) -> Option<u8> {
    Some(match class {
        SeamGripClass::WeaklyGripped => 0,
        SeamGripClass::Ungripped => 1,
        SeamGripClass::ReachableUnrevealed => 2,
        SeamGripClass::ActivationUnknown
        | SeamGripClass::PropagationUnknown
        | SeamGripClass::ObservationUnknown
        | SeamGripClass::DiscriminationUnknown => 3,
        SeamGripClass::Opaque => 4,
        SeamGripClass::StronglyGripped | SeamGripClass::Intentional | SeamGripClass::Suppressed => {
            return None;
        }
    })
}

fn bool_rank(value: bool) -> u8 {
    if value { 0 } else { 1 }
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

fn why_line(entry: &ClassifiedSeam) -> String {
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

struct PilotCommands {
    after_snapshot: String,
    outcome: String,
    retry: String,
}

impl PilotCommands {
    fn new(context: PilotSummaryContext<'_>) -> Self {
        let out_dir = context
            .artifacts
            .pilot_summary_json
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let after_path = context
            .artifacts
            .pilot_summary_json
            .parent()
            .map(|dir| dir.join("after.repo-exposure.json"))
            .unwrap_or_else(|| PathBuf::from("after.repo-exposure.json"));
        let after_snapshot = loop_commands::check_repo_exposure_command(
            &display_path(context.root),
            context.mode.as_str(),
            &loop_commands::shell_path(&after_path),
        );
        let outcome = loop_commands::outcome_command(
            &loop_commands::shell_path(&context.artifacts.repo_exposure_json),
            &loop_commands::shell_path(&after_path),
            None,
        );
        let retry_timeout_ms = context.timeout_ms.saturating_mul(4).max(120_000);
        let retry = format!(
            "ripr pilot --root {} --out {} --mode {} --max-seams {} --timeout-ms {}",
            loop_commands::shell_path(context.root),
            loop_commands::shell_path(&out_dir),
            context.mode.as_str(),
            context.max_seams,
            retry_timeout_ms
        );
        Self {
            after_snapshot,
            outcome,
            retry,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamKind};
    use crate::analysis::test_grip_evidence::{
        RelatedTestGrip, RelationConfidence, RelationReason, TestGripEvidence,
    };
    use crate::domain::{
        Confidence, MissingDiscriminatorFact, OracleKind, OracleStrength, StageEvidence,
        StageState, ValueFact,
    };

    fn seam(file: &str, line: usize, expression: &str) -> RepoSeam {
        RepoSeam::new(
            file,
            "pricing::discounted_total",
            SeamKind::PredicateBoundary,
            line * 10,
            line,
            expression,
            RequiredDiscriminator::BoundaryValue {
                description: expression.to_string(),
            },
            ExpectedSink::ReturnValue,
        )
    }

    fn stage(state: StageState) -> StageEvidence {
        StageEvidence::new(state, Confidence::Medium, "stage summary")
    }

    fn missing() -> MissingDiscriminatorFact {
        MissingDiscriminatorFact {
            value: "discount_threshold equality boundary".to_string(),
            reason: "observed values do not include the equality-boundary case".to_string(),
            flow_sink: None,
        }
    }

    fn related_test() -> RelatedTestGrip {
        RelatedTestGrip {
            test_name: "below_threshold_has_no_discount".to_string(),
            file: PathBuf::from("tests/pricing.rs"),
            line: 12,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            evidence_summary: "exact value assertion".to_string(),
            relation_reason: RelationReason::DirectOwnerCall,
            relation_confidence: RelationConfidence::High,
        }
    }

    fn pilot_artifacts() -> PilotArtifacts {
        PilotArtifacts {
            repo_exposure_json: PathBuf::from("target/ripr/pilot/repo-exposure.json"),
            repo_exposure_md: PathBuf::from("target/ripr/pilot/repo-exposure.md"),
            agent_seam_packets_json: PathBuf::from("target/ripr/pilot/agent-seam-packets.json"),
            pilot_summary_json: PathBuf::from("target/ripr/pilot/pilot-summary.json"),
            pilot_summary_md: PathBuf::from("target/ripr/pilot/pilot-summary.md"),
        }
    }

    fn pilot_context(artifacts: &PilotArtifacts) -> PilotSummaryContext<'_> {
        PilotSummaryContext {
            root: Path::new("."),
            mode: &Mode::Draft,
            config_path: Some(Path::new("ripr.toml")),
            max_seams: 5,
            timeout_ms: 30_000,
            artifacts,
        }
    }

    fn classified_with(
        class: SeamGripClass,
        file: &str,
        line: usize,
        missing_discriminators: Vec<MissingDiscriminatorFact>,
        related_tests: Vec<RelatedTestGrip>,
    ) -> ClassifiedSeam {
        let seam = seam(file, line, "amount >= discount_threshold");
        ClassifiedSeam {
            evidence: TestGripEvidence {
                seam_id: seam.id().clone(),
                related_tests,
                reach: stage(StageState::Yes),
                activate: stage(StageState::Yes),
                propagate: stage(StageState::Yes),
                observe: stage(StageState::Yes),
                discriminate: stage(StageState::Weak),
                observed_values: Vec::<ValueFact>::new(),
                missing_discriminators,
            },
            seam,
            class,
        }
    }

    #[test]
    fn pilot_ranking_prefers_actionable_class_order_before_tie_breakers() {
        let ungripped = classified_with(
            SeamGripClass::Ungripped,
            "src/a.rs",
            10,
            vec![missing()],
            vec![related_test()],
        );
        let weak = classified_with(
            SeamGripClass::WeaklyGripped,
            "src/z.rs",
            99,
            Vec::new(),
            Vec::new(),
        );

        let entries = [ungripped, weak];
        let ranked = top_actionable_seams(&entries, 5);
        assert_eq!(ranked[0].class, SeamGripClass::WeaklyGripped);
        assert_eq!(ranked[1].class, SeamGripClass::Ungripped);
    }

    #[test]
    fn pilot_ranking_uses_evidence_tie_breakers_then_stable_location() {
        let no_missing = classified_with(
            SeamGripClass::WeaklyGripped,
            "src/a.rs",
            10,
            Vec::new(),
            vec![related_test()],
        );
        let with_missing = classified_with(
            SeamGripClass::WeaklyGripped,
            "src/b.rs",
            10,
            vec![missing()],
            Vec::new(),
        );
        let stable_first = classified_with(
            SeamGripClass::WeaklyGripped,
            "src/c.rs",
            10,
            Vec::new(),
            Vec::new(),
        );
        let stable_second = classified_with(
            SeamGripClass::WeaklyGripped,
            "src/d.rs",
            10,
            Vec::new(),
            Vec::new(),
        );

        let entries = [stable_second, stable_first, no_missing, with_missing];
        let ranked = top_actionable_seams(&entries, 5);
        assert_eq!(display_path(ranked[0].seam.file()), "src/b.rs");
        assert_eq!(display_path(ranked[1].seam.file()), "src/a.rs");
        assert_eq!(display_path(ranked[2].seam.file()), "src/c.rs");
        assert_eq!(display_path(ranked[3].seam.file()), "src/d.rs");
    }

    #[test]
    fn pilot_ranking_excludes_solved_governed_classes() {
        let strong = classified_with(
            SeamGripClass::StronglyGripped,
            "src/strong.rs",
            1,
            Vec::new(),
            Vec::new(),
        );
        let intentional = classified_with(
            SeamGripClass::Intentional,
            "src/intentional.rs",
            2,
            Vec::new(),
            Vec::new(),
        );
        let suppressed = classified_with(
            SeamGripClass::Suppressed,
            "src/suppressed.rs",
            3,
            Vec::new(),
            Vec::new(),
        );
        let opaque = classified_with(
            SeamGripClass::Opaque,
            "src/opaque.rs",
            4,
            Vec::new(),
            Vec::new(),
        );

        let entries = [strong, intentional, suppressed, opaque];
        let ranked = top_actionable_seams(&entries, 5);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].class, SeamGripClass::Opaque);
    }

    #[test]
    fn pilot_summary_json_contains_config_state_artifacts_and_next_commands() {
        let entry = classified_with(
            SeamGripClass::WeaklyGripped,
            "src/pricing.rs",
            88,
            vec![missing()],
            vec![related_test()],
        );
        let artifacts = PilotArtifacts {
            repo_exposure_json: PathBuf::from("target/ripr/pilot/repo-exposure.json"),
            repo_exposure_md: PathBuf::from("target/ripr/pilot/repo-exposure.md"),
            agent_seam_packets_json: PathBuf::from("target/ripr/pilot/agent-seam-packets.json"),
            pilot_summary_json: PathBuf::from("target/ripr/pilot/pilot-summary.json"),
            pilot_summary_md: PathBuf::from("target/ripr/pilot/pilot-summary.md"),
        };
        let context = PilotSummaryContext {
            root: Path::new("."),
            mode: &Mode::Draft,
            config_path: Some(Path::new("ripr.toml")),
            max_seams: 5,
            timeout_ms: 30_000,
            artifacts: &artifacts,
        };

        let json = render_pilot_summary_json(&[entry], context);
        assert!(json.contains(r#""schema_version": "0.2""#));
        assert!(json.contains(r#""status": "complete""#));
        assert!(json.contains(r#""state": "loaded""#));
        assert!(json.contains(r#""top_actionable_seams""#));
        assert!(json.contains(r#""missing_discriminator""#));
        assert!(json.contains("ripr outcome --before target/ripr/pilot/repo-exposure.json"));
    }

    #[test]
    fn pilot_summary_md_spells_out_first_screen_recommendation() {
        let entry = classified_with(
            SeamGripClass::WeaklyGripped,
            "src/pricing.rs",
            88,
            vec![missing()],
            vec![related_test()],
        );
        let artifacts = pilot_artifacts();
        let md = render_pilot_summary_md(&[entry], pilot_context(&artifacts));

        for needle in [
            "## What Was Inspected",
            "## Top Recommendation",
            "- Inspected seam:",
            "- Why it matters: missing discriminator: discount_threshold equality boundary",
            "- Focused test: add `discounted_total_boundary_discriminator` in `tests/pricing.rs`",
            "- Candidate value: `discount_threshold equality boundary`",
            "Target seam:",
            "Add a targeted test:",
            "## Next Commands",
            "ripr outcome --before target/ripr/pilot/repo-exposure.json",
        ] {
            assert!(md.contains(needle), "missing markdown needle: {needle}");
        }
    }

    #[test]
    fn pilot_terminal_prints_top_test_and_follow_up_commands() {
        let entry = classified_with(
            SeamGripClass::WeaklyGripped,
            "src/pricing.rs",
            88,
            vec![missing()],
            vec![related_test()],
        );
        let artifacts = pilot_artifacts();
        let terminal = render_pilot_terminal(&[entry], pilot_context(&artifacts));

        for needle in [
            "Inspected:",
            "root: .",
            "mode: draft",
            "config: loaded ripr.toml",
            "Top recommendation:",
            "inspected seam: src/pricing.rs:88 predicate_boundary in pricing::discounted_total (weakly_gripped)",
            "why it matters: missing discriminator: discount_threshold equality boundary",
            "focused test: add discounted_total_boundary_discriminator in tests/pricing.rs",
            "candidate value: discount_threshold equality boundary",
            "assertion: assert_eq!(discounted_total(/* discount_threshold equality boundary */), /* expected */)",
            "Detailed brief:",
            "target/ripr/pilot/pilot-summary.md",
            "Structured packet:",
            "target/ripr/pilot/agent-seam-packets.json",
            "Run after adding the focused test:",
            "ripr check --root . --mode draft --format repo-exposure-json > target/ripr/pilot/after.repo-exposure.json",
            "ripr outcome --before target/ripr/pilot/repo-exposure.json",
        ] {
            assert!(
                terminal.contains(needle),
                "missing terminal needle: {needle}"
            );
        }
    }

    #[test]
    fn timeout_summary_json_is_partial_and_points_to_retry() {
        let artifacts = PilotArtifacts {
            repo_exposure_json: PathBuf::from("target/ripr/pilot/repo-exposure.json"),
            repo_exposure_md: PathBuf::from("target/ripr/pilot/repo-exposure.md"),
            agent_seam_packets_json: PathBuf::from("target/ripr/pilot/agent-seam-packets.json"),
            pilot_summary_json: PathBuf::from("target/ripr/pilot/pilot-summary.json"),
            pilot_summary_md: PathBuf::from("target/ripr/pilot/pilot-summary.md"),
        };
        let context = PilotSummaryContext {
            root: Path::new("."),
            mode: &Mode::Draft,
            config_path: None,
            max_seams: 5,
            timeout_ms: 1,
            artifacts: &artifacts,
        };

        let json = render_pilot_timeout_summary_json(context);
        assert!(json.contains(r#""schema_version": "0.2""#));
        assert!(json.contains(r#""status": "partial""#));
        assert!(json.contains(r#""reason": "timeout""#));
        assert!(json.contains(r#""actionable_seams_total": null"#));
        assert!(json.contains("ripr pilot --root . --out target/ripr/pilot --mode draft"));
        assert!(json.contains("--timeout-ms 120000"));
    }
}
