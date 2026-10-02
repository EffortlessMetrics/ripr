//! `ripr agent card` (#4667): the compact repair-card handoff.
//!
//! This is the CLI adapter layer only. The producer lives in
//! `crate::app::repair_card_handoff`; the card schema and its budget live in
//! `crate::domain` / `crate::repair_card_budget`. This module owns entry
//! resolution, policy omission checks, and the two output shapes: the
//! versioned `RepairCardV1` JSON document (`--json`) and the compact human
//! summary (default), which presents typed fields verbatim and never
//! re-derives or enhances them.

use crate::analysis;
use crate::app::agent_brief::AgentBriefPolicy;
use crate::cli::agent::AgentCardOptions;
use crate::cli::commands_context::ensure_command_root;
use crate::config::load_for_root;
use crate::domain::{RepairCardTarget, RepairCardV1};
use crate::output;

use super::agent::unknown_seam_id_hint;

pub(super) fn run_agent_card(options: AgentCardOptions) -> Result<(), String> {
    ensure_command_root(&options.root, "agent card")?;
    let card = render_agent_card(&options)?;
    if options.json {
        let rendered = output::json::render_pretty_with_newline(&card, "agent card")?;
        print!("{rendered}");
        return Ok(());
    }
    for line in agent_card_prose_lines(&card) {
        println!("{line}");
    }
    Ok(())
}

fn render_agent_card(options: &AgentCardOptions) -> Result<RepairCardV1, String> {
    let config = load_for_root(&options.root)?;
    let (classified, _) =
        analysis::inventory_classified_seams_at_with_config(&options.root, &config)?;
    let entry = classified
        .iter()
        .find(|entry| entry.seam.id().as_str() == options.seam_id)
        .ok_or_else(|| {
            format!(
                "agent card seam_id {} was not found. {}",
                options.seam_id,
                unknown_seam_id_hint(&options.root, &options.seam_id)
            )
        })?;

    let policy = AgentBriefPolicy::from_config(&config);
    if let Some(reason) = policy.omission_reason_for_class(entry.class) {
        // Mirror `agent packet` (#4332): a policy-omitted seam is a dead end
        // without the listing route; name it like the not-found refusals do.
        return Err(format!(
            "agent card seam_id {} {reason}. {}",
            options.seam_id,
            unknown_seam_id_hint(&options.root, &options.seam_id)
        ));
    }

    crate::app::repair_card_handoff::repair_card_for_entry(entry, &options.root, &config)
}

/// The default human output: the card's typed fields, in card order,
/// presented verbatim. The renderer never re-derives values, never reorders
/// evidence, and routes every omitted family to its typed reference. The one
/// enhancement over a raw field dump is the explicit non-actionable reason
/// when the card carries no next action, because a silent absence there reads
/// as a bug, not a gate decision.
///
/// Crate-visible for the #4669 usability measurement, which counts the human
/// presentation separately from the JSON wire shape (RIPR-SPEC-0196).
/// The wire spelling of a typed value, presented verbatim in the human
/// summary (the enum remains the authority; the renderer never re-interprets
/// a value and never prints a `{:?}` debug spelling).
fn wire_name<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

pub(crate) fn agent_card_prose_lines(card: &RepairCardV1) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("Repair card {}", card.repair_card_id));
    lines.push(format!("  schema: {}", card.schema_version));
    let mut subject = format!("  seam: {}", card.subject.seam_id);
    if let Some(finding_id) = &card.subject.finding_id {
        subject.push_str(&format!(" (finding {finding_id})"));
    }
    lines.push(subject);
    lines.push(format!(
        "  snapshot: {} @ {} ({})",
        card.snapshot.workspace_identity,
        card.snapshot.repository_head,
        wire_name(&card.snapshot.currentness)
    ));
    lines.push(format!(
        "  instruction: {:?} (fix site: {}, suggested assertion: {})",
        card.instruction.state,
        yes_no(card.instruction.has_fix_site),
        yes_no(card.instruction.has_suggested_assertion)
    ));
    let missing = if card.readiness.missing_evidence.is_empty() {
        "none".to_string()
    } else {
        card.readiness.missing_evidence.join("; ")
    };
    lines.push(format!(
        "  readiness: repair_ready={} (missing evidence: {missing})",
        card.readiness.repair_ready
    ));
    lines.push(format!("  changed behavior: {}", card.changed_behavior));
    lines.push(format!(
        "  blocker: {}",
        card.exact_blocker.as_deref().unwrap_or("-")
    ));
    if let Some(goal) = &card.assertion_goal {
        lines.push(format!(
            "  assertion goal: {goal:?}: {}",
            card.assertion_goal_detail.as_deref().unwrap_or("-")
        ));
    }
    if let Some(target) = &card.selected_target {
        let rendered = match target {
            RepairCardTarget::Existing {
                symbol_id,
                file,
                line,
                test_kind,
                relation,
                workspace_identity: _,
            } => format!(
                "  selected target: existing {symbol_id} {file}:{line} ({test_kind:?}, relation {relation})"
            ),
            RepairCardTarget::Proposed {
                file,
                owner,
                proposal_kind,
            } => format!("  selected target: proposed {owner} in {file} ({proposal_kind:?})"),
        };
        lines.push(rendered);
    }
    match &card.next_action {
        Some(action) => {
            lines.push(format!("  next action: {} ({})", action.display, action.command_id));
        }
        None => lines.push(format!(
            "  next action: none (instruction {:?} with repair_ready={} exposes no bounded route; inspect the detail references below)",
            card.instruction.state, card.readiness.repair_ready
        )),
    }
    if !card.allowed_files.is_empty() || !card.forbidden_files.is_empty() {
        lines.push(format!(
            "  edit cage: allowed=[{}] forbidden=[{}]",
            card.allowed_files.join(", "),
            card.forbidden_files.join(", ")
        ));
    }
    lines.push(format!(
        "  done when: static movement {:?}, focused test {:?}, edit cage {:?}, mutation {:?}, currentness {}",
        card.done_when.static_movement,
        card.done_when.focused_test_execution,
        card.done_when.edit_cage,
        card.done_when.mutation_confirmation,
        wire_name(&card.done_when.currentness)
    ));
    for stop in &card.stop_conditions {
        lines.push(format!("  stop: {stop}"));
    }
    for reference in &card.detail_references {
        match &reference.route {
            Some(route) => lines.push(format!("  detail [{:?}]: {route}", reference.family)),
            None => lines.push(format!(
                "  detail [{:?}]: unavailable ({})",
                reference.family,
                reference
                    .unavailable_reason
                    .as_deref()
                    .unwrap_or("no reason recorded")
            )),
        }
    }
    if !card.limitations.is_empty() {
        lines.push(format!("  limitations: {}", card.limitations.join("; ")));
    }
    if let Some(attempt) = &card.attempt {
        lines.push(format!(
            "  attempt: {} ({})",
            attempt.attempt_id, attempt.state
        ));
    }
    // The packet command is presented the way the card actually exposes it:
    // when the typed next action is open, its display already binds the
    // portable root and is directly executable; without a next action the
    // packet family is a route reference and stays rootless like every other
    // detail route (#4666 portability contract).
    match &card.next_action {
        Some(action) => lines.push(format!("  full packet: {}", action.display)),
        None => lines.push(format!(
            "  full packet: ripr agent packet --seam-id {} --json",
            card.subject.seam_id
        )),
    }
    lines
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
