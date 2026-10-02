//! RepairCard projection through standard LSP (#4668, RIPR-SPEC-0198).
//!
//! The editor adapter never re-derives card facts: witness binding, portable
//! workspace identity, attempt recency, packet rendering, and card assembly
//! all delegate to the same `app::repair_card_handoff` authorities the CLI
//! `ripr agent card` handoff consumes (#4667). The adapter owns framing only —
//! one bounded copy action ([`seam_repair_card_action`]) and one bounded hover
//! section ([`repair_card_hover_lines`]) — and fails closed (omits the
//! projection) whenever a producer fact cannot be bound. Capability and budget
//! may omit detail on the wire; neither the action nor the hover ever
//! strengthens readiness, actionability, or currentness.

use super::state::AnalysisSnapshot;
use crate::agent::artifact::git_output;
use crate::agent::command_specs::{AgentArtifactRoute, agent_inspection_command_spec};
use crate::agent::loop_commands;
use crate::analysis::ClassifiedSeam;
use crate::analysis::repair_route::repair_packet_eligibility;
use crate::app::repair_card_handoff::{
    SeamCardFacts, assemble_repair_card, latest_attempt_for_seam, witness_from_findings,
    workspace_identity_for,
};
use crate::domain::{FixInstructionState, RepairCardDetailState, RepairCardV1};
use crate::output::agent_seam_packets::{
    PacketCommandContext, render_agent_seam_packet_json_with_context,
};

/// Assemble the same [`RepairCardV1`] the CLI `ripr agent card` handoff
/// builds, from the completed snapshot's own authorities instead of re-running
/// the check pipeline. Returns `None` (fail-closed omission) when any producer
/// fact cannot be bound — the editor surface never reconstructs a card from
/// weaker evidence and never invents the fields the producers refused.
pub(super) fn seam_repair_card(
    entry: &ClassifiedSeam,
    snapshot: &AnalysisSnapshot,
) -> Option<RepairCardV1> {
    let readiness = &repair_packet_eligibility(entry).readiness;
    let (finding_id, witness) = match witness_from_findings(
        &snapshot.findings,
        entry,
        readiness.canonical_gap_id.as_deref(),
    ) {
        Some((finding_id, witness)) => (Some(finding_id), Some(witness)),
        None => (None, None),
    };
    let root = snapshot.root.as_path();
    let seam_id = entry.seam.id().as_str().to_string();
    // The card's semantic identity binds the live repository head (the digest
    // input carries the snapshot block), so the editor must resolve the same
    // head the CLI producer would, not a snapshot-cached spelling.
    let repository_head = git_output(root, &["rev-parse", "HEAD"])
        .ok()?
        .trim()
        .to_string();
    let workspace_identity = workspace_identity_for(entry, readiness).ok()?;
    let attempt = latest_attempt_for_seam(root, &seam_id).ok()?;
    let packet_root = loop_commands::bound_root(&root.to_string_lossy());
    let packet_json = render_agent_seam_packet_json_with_context(
        entry,
        PacketCommandContext::Standalone {
            root: packet_root.as_str(),
        },
    );
    let next_command = agent_inspection_command_spec(
        AgentArtifactRoute::Packet,
        &root.to_string_lossy(),
        &seam_id,
    );
    assemble_repair_card(&SeamCardFacts {
        entry,
        witness: witness.as_ref(),
        finding_id: finding_id.as_deref(),
        attempt: attempt.as_ref(),
        packet_json: &packet_json,
        repository_head: &repository_head,
        workspace_identity: &workspace_identity,
        next_command: Some(next_command),
    })
    .ok()
}

/// The wire spelling of a typed state enum. Presentation only: the enum
/// remains the authority and this never re-interprets a value.
fn state_wire_name<T: serde::Serialize>(state: &T) -> String {
    serde_json::to_value(state)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Bounded hover section for one assembled card: canonical identity,
/// instruction state, next-action presence, and per-state detail-family
/// availability counts. Verbatim typed fields only — the card itself, with
/// its stable detail references, rides behind the copy action.
pub(super) fn repair_card_hover_lines(card: &RepairCardV1) -> Vec<String> {
    let mut current = 0usize;
    let mut stale = 0usize;
    let mut unavailable = 0usize;
    let mut other = 0usize;
    for reference in &card.detail_references {
        match reference.state {
            RepairCardDetailState::Current => current += 1,
            RepairCardDetailState::Stale => stale += 1,
            RepairCardDetailState::Unavailable => unavailable += 1,
            RepairCardDetailState::Malformed
            | RepairCardDetailState::WrongRoot
            | RepairCardDetailState::Missing => other += 1,
        }
    }
    let instruction = state_wire_name(&card.instruction.state);
    let mut lines = vec![
        String::new(),
        "## Repair card".to_string(),
        format!("Card: `{}`", card.repair_card_id),
        format!("Instruction: `{instruction}`"),
    ];
    if card.instruction.state == FixInstructionState::Unavailable {
        lines.push("Next action: none (no producer-owned route)".to_string());
    } else {
        match &card.next_action {
            Some(action) => lines.push(format!("Next action: `{}`", action.display)),
            None => lines.push("Next action: none (route gate closed)".to_string()),
        }
    }
    lines.push(format!(
        "Detail: {current} current · {stale} stale · {unavailable} unavailable · {other} other of {} families",
        card.detail_references.len()
    ));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::REPAIR_CARD_SCHEMA_VERSION;

    fn digest_fixture_card() -> RepairCardV1 {
        // Reuse the pure assembly fixture shape through the shared builder so
        // the hover projection is tested against a real card, not a hand-drawn
        // shell. Facts come from `app::repair_card_handoff`'s own test
        // helpers via a minimal local replica: only the fields the hover
        // section reads are exercised here.
        RepairCardV1 {
            schema_version: REPAIR_CARD_SCHEMA_VERSION.to_string(),
            repair_card_id: "card-digest".to_string(),
            snapshot: crate::domain::RepairCardSnapshot {
                workspace_identity: "workspace:demo".to_string(),
                repository_head: "abc123".to_string(),
                currentness: crate::domain::RepairCardSnapshotCurrentness::Current,
            },
            subject: crate::domain::RepairCardSubject {
                seam_id: "seam:demo".to_string(),
                canonical_gap_id: Some("gap:demo".to_string()),
                finding_id: Some("probe:demo".to_string()),
            },
            instruction: crate::domain::FixInstructionSummary {
                state: crate::domain::FixInstructionState::FixSiteReady,
                has_fix_site: true,
                has_suggested_assertion: false,
                limitation_kinds: Vec::new(),
            },
            readiness: crate::domain::RepairCardReadinessFacts {
                repair_ready: true,
                required_evidence: Vec::new(),
                present_evidence: Vec::new(),
                missing_evidence: Vec::new(),
            },
            changed_behavior: "expr".to_string(),
            exact_blocker: None,
            selected_target: None,
            assertion_goal: None,
            assertion_goal_detail: None,
            candidate_value: None,
            allowed_files: Vec::new(),
            forbidden_files: Vec::new(),
            done_when: crate::domain::RepairCardDoneWhen {
                static_movement: crate::domain::StaticMovementGoal::ClosedBySelectedRoute,
                focused_test_execution: crate::domain::FocusedExecutionGoal::ExplicitlyNotRun,
                edit_cage: crate::domain::EditCageGoal::Compliant,
                mutation_confirmation: crate::domain::MutationConfirmationGoal::NotRequested,
                currentness: crate::domain::CardCurrentnessGoal::Current,
            },
            stop_conditions: Vec::new(),
            next_action: Some(crate::domain::RepairCardCommandRef {
                command_id: "ripr:agent:packet".to_string(),
                role: "inspection".to_string(),
                display: "ripr agent packet --root . --seam-id seam:demo --json".to_string(),
            }),
            selected_basis: None,
            rejected_alternatives: Vec::new(),
            attempt: None,
            claim_boundary: crate::domain::REPAIR_CARD_CLAIM_BOUNDARY.to_string(),
            limitations: Vec::new(),
            detail_references: vec![
                crate::domain::RepairCardDetailRef {
                    family: crate::domain::RepairCardDetailFamily::CanonicalPacket,
                    state: RepairCardDetailState::Current,
                    route: Some("ripr agent packet --seam-id seam:demo --json".to_string()),
                    unavailable_reason: None,
                    detail_digest: "digest-a".to_string(),
                    omitted_bytes: 100,
                    ordinal: 0,
                    omission_class: crate::domain::RepairCardOmissionClass::AuthorityOwnedDetail,
                },
                crate::domain::RepairCardDetailRef {
                    family: crate::domain::RepairCardDetailFamily::RepairAttemptStatus,
                    state: RepairCardDetailState::Stale,
                    route: Some("ripr agent status --json".to_string()),
                    unavailable_reason: None,
                    detail_digest: "digest-b".to_string(),
                    omitted_bytes: 50,
                    ordinal: 1,
                    omission_class: crate::domain::RepairCardOmissionClass::AuthorityOwnedDetail,
                },
                crate::domain::RepairCardDetailRef {
                    family: crate::domain::RepairCardDetailFamily::MutationCalibration,
                    state: RepairCardDetailState::Unavailable,
                    route: None,
                    unavailable_reason: Some("none produced".to_string()),
                    detail_digest: String::new(),
                    omitted_bytes: 0,
                    ordinal: 2,
                    omission_class: crate::domain::RepairCardOmissionClass::Unavailable,
                },
            ],
            detail_summary: crate::domain::RepairCardDetailSummary::default(),
            complete_evidence_digest: String::new(),
        }
    }

    #[test]
    fn hover_lines_project_identity_state_next_action_and_detail_counts() -> Result<(), String> {
        let card = digest_fixture_card();
        let lines = repair_card_hover_lines(&card);
        let text = lines.join("\n");
        if !text.contains("## Repair card") {
            return Err("hover section missing its heading".to_string());
        }
        if !text.contains("Card: `card-digest`") {
            return Err("hover section must name the canonical card identity".to_string());
        }
        if !text.contains("Instruction: `fix_site_ready`") {
            return Err("hover section must project the typed instruction state".to_string());
        }
        if !text.contains("ripr agent packet --root . --seam-id seam:demo --json") {
            return Err("hover section must name the card's next action".to_string());
        }
        if !text.contains("1 current · 1 stale · 1 unavailable · 0 other of 3 families") {
            return Err("hover section must count detail availability per state".to_string());
        }
        Ok(())
    }

    #[test]
    fn unavailable_instruction_projects_no_route_and_no_next_action() -> Result<(), String> {
        let mut card = digest_fixture_card();
        card.instruction.state = crate::domain::FixInstructionState::Unavailable;
        card.next_action = None;
        let lines = repair_card_hover_lines(&card);
        let text = lines.join("\n");
        if !text.contains("Instruction: `unavailable`") {
            return Err("unavailable instruction must keep its typed state".to_string());
        }
        if !text.contains("Next action: none (no producer-owned route)") {
            return Err("unavailable instruction must not present a next action".to_string());
        }
        Ok(())
    }

    #[test]
    fn limited_route_gate_projects_closed_next_action_without_strengthening() -> Result<(), String>
    {
        let mut card = digest_fixture_card();
        // A ready instruction whose route gate closed keeps the state honest
        // and names the closure instead of inventing a route.
        card.next_action = None;
        let lines = repair_card_hover_lines(&card);
        let text = lines.join("\n");
        if !text.contains("Instruction: `fix_site_ready`") {
            return Err("a closed route gate must not weaken the instruction state".to_string());
        }
        if !text.contains("Next action: none (route gate closed)") {
            return Err("a closed route gate must be named as closed".to_string());
        }
        Ok(())
    }

    #[test]
    fn state_wire_name_serializes_without_reinterpretation() -> Result<(), String> {
        if state_wire_name(&FixInstructionState::FixSiteReady) != "fix_site_ready" {
            return Err("fix_site_ready spelling drifted".to_string());
        }
        if state_wire_name(&RepairCardDetailState::Unavailable) != "unavailable" {
            return Err("unavailable spelling drifted".to_string());
        }
        Ok(())
    }
}
