//! App-side builder that projects the shared repair authorities into a
//! [`RepairCardV1`] (RIPR-SPEC-0192, #4663).
//!
//! [`build_repair_card`] is the only production entry point. It is a pure
//! projection: every card field names the authority it was copied from, no
//! classification vocabulary is invented here, and route exposure stays
//! fail-closed — a card never presents a runnable route that the shared
//! instruction vocabulary and repair-route readiness gate would not expose.

use crate::analysis::repair_route::{RepairRouteReadiness, RepairTargetSelection};
use crate::domain::{
    CommandSpec, FixInstructionSummary, REPAIR_CARD_CLAIM_BOUNDARY, REPAIR_CARD_SCHEMA_VERSION,
    RepairCardAssertionGoal, RepairCardAttempt, RepairCardBudget, RepairCardCommandRef,
    RepairCardDoneWhen, RepairCardProposedTestKind, RepairCardReadinessFacts,
    RepairCardRejectedAlternative, RepairCardSnapshot, RepairCardSubject, RepairCardTarget,
    RepairCardTestKind, RepairCardV1, repair_card_route_exposable,
};
use crate::repair_card_budget::{RepairCardDetailSource, apply_repair_card_budget};
use crate::repair_card_digest::repair_card_semantic_digest;

/// Bounded input selected by the caller (CLI projection wiring lands in #4667,
/// agent ratification in #4669); the builder itself stays pure and performs no
/// IO. Every `Option` is producer-owned: the builder never fabricates a
/// blocker, assertion text, or candidate value.
pub(crate) struct RepairCardInput<'a> {
    pub(crate) snapshot: RepairCardSnapshot,
    pub(crate) subject: RepairCardSubject,
    pub(crate) instruction: &'a FixInstructionSummary,
    pub(crate) readiness: &'a RepairRouteReadiness,
    pub(crate) changed_behavior: String,
    /// Producer-owned exact blocker (for example the first witness limitation
    /// detail or missing-discriminator text); the card copies it verbatim.
    pub(crate) exact_blocker: Option<String>,
    /// Producer-owned witness fact: the expected observation routes through
    /// an observer setup rather than a direct sink.
    pub(crate) has_observer_setup: bool,
    /// Producer-owned suggested-assertion text or observer description.
    pub(crate) assertion_goal_detail: Option<String>,
    /// Producer-owned candidate input/value when the route supplies one.
    pub(crate) candidate_value: Option<String>,
    /// Producer-owned repair-packet eligibility flip
    /// (`RepairPacketEligibility::eligible()`): the packet authority's
    /// fail-closed decision that this seam is safe for a targeted-test repair
    /// route. A card must not present a runnable route the packet authority
    /// would not expose.
    pub(crate) packet_eligible: bool,
    pub(crate) next_command: Option<&'a CommandSpec>,
    pub(crate) allowed_files: Vec<String>,
    pub(crate) forbidden_files: Vec<String>,
    pub(crate) done_when: RepairCardDoneWhen,
    pub(crate) stop_conditions: Vec<String>,
    /// Producer-owned basis for the selected target; falls back to the
    /// selected target's own relation basis (Existing) or owner (Proposed).
    pub(crate) selected_basis: Option<String>,
    pub(crate) rejected_alternatives: Vec<RepairCardRejectedAlternative>,
    pub(crate) attempt: Option<&'a crate::app::repair_attempt::RepairAttemptManifest>,
    pub(crate) limitations: Vec<String>,
    /// Producer-owned detail sources for the load-bearing evidence families
    /// (#4666): the card routes to them instead of embedding their content.
    pub(crate) detail_sources: Vec<RepairCardDetailSource>,
    /// Versioned item/byte budget applied to the wire card. The default is
    /// provisional-but-versioned; #4669 ratifies the numbers.
    pub(crate) budget: RepairCardBudget,
}

/// Project the shared repair authorities into a versioned repair card.
pub(crate) fn build_repair_card(input: &RepairCardInput<'_>) -> Result<RepairCardV1, String> {
    let selected_target = project_target(&input.readiness.target_selection);
    if input.next_command.is_some()
        && !(input.packet_eligible
            && repair_card_route_exposable(
                input.instruction.state,
                input.readiness.is_repair_ready(),
            ))
    {
        return Err(
            "repair card route gate is closed; the card must not present a runnable route"
                .to_string(),
        );
    }
    if input.instruction.has_suggested_assertion && input.assertion_goal_detail.is_none() {
        return Err(
            "producer claims a suggested assertion but supplied no assertion text".to_string(),
        );
    }
    if input.subject.seam_id != input.readiness.seam_id {
        return Err("subject seam and readiness seam do not identify one repair".to_string());
    }
    if let (Some(subject_gap), Some(readiness_gap)) = (
        &input.subject.canonical_gap_id,
        &input.readiness.canonical_gap_id,
    ) && subject_gap != readiness_gap
    {
        return Err("subject gap and readiness gap do not identify one repair".to_string());
    }
    if input.selected_basis.is_some() && selected_target.is_none() {
        return Err("selected basis without a selected target".to_string());
    }
    if input.rejected_alternatives.len() > crate::domain::MAX_REPAIR_CARD_REJECTED_ALTERNATIVES {
        return Err(
            "repair card rejected_alternatives exceed the compact-card boundary".to_string(),
        );
    }
    let selected_basis = input
        .selected_basis
        .clone()
        .or_else(|| selected_basis_from_target(selected_target.as_ref()));

    let mut card = RepairCardV1 {
        schema_version: REPAIR_CARD_SCHEMA_VERSION.to_string(),
        repair_card_id: String::new(),
        snapshot: input.snapshot.clone(),
        subject: input.subject.clone(),
        instruction: input.instruction.clone(),
        readiness: RepairCardReadinessFacts {
            repair_ready: input.readiness.is_repair_ready(),
            required_evidence: input.readiness.required_evidence.clone(),
            present_evidence: input.readiness.present_evidence.clone(),
            missing_evidence: input.readiness.missing_evidence.clone(),
        },
        changed_behavior: input.changed_behavior.clone(),
        exact_blocker: input.exact_blocker.clone(),
        selected_target,
        assertion_goal: project_assertion_goal(input),
        assertion_goal_detail: input.assertion_goal_detail.clone(),
        candidate_value: input.candidate_value.clone(),
        allowed_files: input.allowed_files.clone(),
        forbidden_files: input.forbidden_files.clone(),
        done_when: input.done_when,
        stop_conditions: input.stop_conditions.clone(),
        next_action: input.next_command.map(project_command_ref),
        selected_basis,
        rejected_alternatives: input.rejected_alternatives.clone(),
        attempt: input.attempt.map(project_attempt),
        claim_boundary: REPAIR_CARD_CLAIM_BOUNDARY.to_string(),
        limitations: input.limitations.clone(),
        detail_references: Vec::new(),
        detail_summary: crate::domain::RepairCardDetailSummary::default(),
        complete_evidence_digest: String::new(),
    };
    apply_repair_card_budget(&mut card, &input.detail_sources, &input.budget)?;
    card.repair_card_id = repair_card_semantic_digest(&card)?;
    Ok(card)
}

fn project_assertion_goal(input: &RepairCardInput<'_>) -> Option<RepairCardAssertionGoal> {
    if input.instruction.has_suggested_assertion {
        return Some(RepairCardAssertionGoal::SuggestedAssertion);
    }
    if input.has_observer_setup {
        return Some(RepairCardAssertionGoal::ObserverSetup);
    }
    None
}

fn project_target(selection: &RepairTargetSelection) -> Option<RepairCardTarget> {
    match selection {
        RepairTargetSelection::Existing(target) => Some(RepairCardTarget::Existing {
            symbol_id: target.symbol_id().0.clone(),
            file: target.file().display().to_string(),
            line: target.line(),
            test_kind: match target.test_kind() {
                crate::analysis::test_grip_evidence::TestKind::InlineUnit => {
                    RepairCardTestKind::InlineUnit
                }
                crate::analysis::test_grip_evidence::TestKind::Integration => {
                    RepairCardTestKind::Integration
                }
            },
            relation: target.relation().as_str().to_string(),
            workspace_identity: target.workspace_identity().to_string(),
        }),
        RepairTargetSelection::Proposed(proposal) => Some(RepairCardTarget::Proposed {
            file: proposal.file.display().to_string(),
            owner: proposal.owner.clone(),
            proposal_kind: match proposal.kind {
                crate::analysis::new_test_target::NewTestKind::InlineUnit => {
                    RepairCardProposedTestKind::InlineUnit
                }
                crate::analysis::new_test_target::NewTestKind::Integration => {
                    RepairCardProposedTestKind::Integration
                }
            },
        }),
        RepairTargetSelection::Missing => None,
    }
}

fn selected_basis_from_target(target: Option<&RepairCardTarget>) -> Option<String> {
    match target {
        Some(RepairCardTarget::Existing { relation, .. }) => Some(relation.clone()),
        Some(RepairCardTarget::Proposed { owner, .. }) => Some(owner.clone()),
        None => None,
    }
}

fn project_command_ref(command: &CommandSpec) -> RepairCardCommandRef {
    RepairCardCommandRef {
        command_id: command.command_id.clone(),
        role: serde_role(&command.role),
        display: command.display.clone(),
    }
}

fn project_attempt(
    manifest: &crate::app::repair_attempt::RepairAttemptManifest,
) -> RepairCardAttempt {
    RepairCardAttempt {
        attempt_id: manifest.repair_attempt_id.as_str().to_string(),
        // Copied through the manifest's own serde representation so this
        // projection cannot drift from the attempt authority's vocabulary.
        state: serde_state(&manifest.state),
    }
}

fn serde_state(state: &crate::app::repair_attempt::RepairAttemptState) -> String {
    serde_json::to_string(state)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string()
}

fn serde_role(role: &crate::domain::CommandRole) -> String {
    serde_json::to_string(role)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::new_test_target::{
        NewTestKind, NewTestProposalProvenance, NewTestTargetProposal,
    };
    use crate::analysis::repair_route::{
        RepairRouteReadiness, RepairRouteState, RepairTargetSelection,
    };
    use crate::analysis::test_grip_evidence::TestTargetEvidence;
    use crate::app::repair_attempt::{RepairAttemptId, RepairAttemptManifest, RepairAttemptState};
    use crate::domain::CommandRole;
    use crate::domain::{
        CardCurrentnessGoal, EditCageGoal, FixInstructionState, FocusedExecutionGoal,
        MutationConfirmationGoal, RepairCardDetailFamily, RepairCardDetailState,
        StaticMovementGoal,
    };
    use std::path::{Path, PathBuf};

    fn snapshot() -> RepairCardSnapshot {
        RepairCardSnapshot {
            workspace_identity: "workspace:demo".to_string(),
            repository_head: "abc123".to_string(),
            currentness: crate::domain::RepairCardSnapshotCurrentness::Current,
        }
    }

    fn subject() -> RepairCardSubject {
        RepairCardSubject {
            seam_id: "seam:demo".to_string(),
            canonical_gap_id: Some("gap:demo".to_string()),
            finding_id: Some("probe:demo".to_string()),
        }
    }

    fn instruction(state: FixInstructionState) -> FixInstructionSummary {
        FixInstructionSummary {
            state,
            has_fix_site: state == FixInstructionState::FixSiteReady,
            has_suggested_assertion: true,
            limitation_kinds: Vec::new(),
        }
    }

    fn readiness(
        state: RepairRouteState,
        target_selection: RepairTargetSelection,
    ) -> RepairRouteReadiness {
        RepairRouteReadiness {
            state,
            seam_id: "seam:demo".to_string(),
            canonical_gap_id: Some("gap:demo".to_string()),
            required_evidence: vec!["owner".to_string()],
            present_evidence: vec!["owner".to_string()],
            missing_evidence: Vec::new(),
            target_selection,
            test_target: None,
            proposed_oracle: None,
            current_oracle: None,
            authority_boundary: "static-only",
        }
    }

    fn ready_readiness() -> RepairRouteReadiness {
        readiness(
            RepairRouteState::Ready,
            RepairTargetSelection::Existing(TestTargetEvidence::fixture(
                "case",
                Path::new("tests/demo.rs"),
                3,
            )),
        )
    }

    fn done_when() -> RepairCardDoneWhen {
        RepairCardDoneWhen {
            static_movement: StaticMovementGoal::ClosedBySelectedRoute,
            focused_test_execution: FocusedExecutionGoal::VerifiedPass,
            edit_cage: EditCageGoal::Compliant,
            mutation_confirmation: MutationConfirmationGoal::NotRequested,
            currentness: CardCurrentnessGoal::Current,
        }
    }

    fn verify_command() -> CommandSpec {
        CommandSpec {
            schema_version: "1".to_string(),
            command_id: "cmd:verify:demo".to_string(),
            role: CommandRole::Verify,
            execution_mode: crate::domain::CommandExecutionMode::Direct,
            program: "cargo".to_string(),
            args: vec!["test".to_string(), "-p".to_string(), "demo".to_string()],
            cwd: ".".to_string(),
            env_set: Vec::new(),
            env_passthrough: Vec::new(),
            environment_policy: crate::domain::EnvironmentPolicy::Declared,
            stdin: crate::domain::StdinPolicy::Null,
            timeout_ms: 120_000,
            cancellation: crate::domain::CancellationPolicy::Allowed,
            network_policy: crate::domain::NetworkPolicy::Forbidden,
            expected_result_parser: crate::domain::ExpectedResultParser::ExitCode,
            expected_exit_codes: vec![0],
            expected_writes: Vec::new(),
            cost_class: crate::domain::CommandCostClass::CompileOrTest,
            platforms: vec![crate::domain::CommandPlatform::Linux],
            display: "cargo test -p demo".to_string(),
            authority_boundary: crate::domain::CommandAuthorityBoundary::VerificationRouteOnly,
        }
    }

    fn base_input<'a>(
        instruction: &'a FixInstructionSummary,
        readiness: &'a RepairRouteReadiness,
    ) -> RepairCardInput<'a> {
        RepairCardInput {
            snapshot: snapshot(),
            subject: subject(),
            instruction,
            readiness,
            changed_behavior: "expr".to_string(),
            exact_blocker: None,
            has_observer_setup: false,
            assertion_goal_detail: Some("assert_eq!(price(0), 0)".to_string()),
            candidate_value: None,
            packet_eligible: true,
            next_command: None,
            allowed_files: vec!["tests/demo.rs".to_string()],
            forbidden_files: vec!["src/lib.rs".to_string()],
            done_when: done_when(),
            stop_conditions: vec!["snapshot_stale".to_string()],
            selected_basis: None,
            rejected_alternatives: Vec::new(),
            attempt: None,
            limitations: Vec::new(),
            detail_sources: Vec::new(),
            budget: RepairCardBudget::default(),
        }
    }

    #[test]
    fn ready_card_projects_authority_facts_verbatim() -> Result<(), String> {
        let instruction = instruction(FixInstructionState::FixSiteReady);
        let readiness = ready_readiness();
        let command = verify_command();
        let mut input = base_input(&instruction, &readiness);
        input.next_command = Some(&command);

        let card = build_repair_card(&input)?;
        if card.schema_version != REPAIR_CARD_SCHEMA_VERSION {
            return Err(format!(
                "unexpected schema version: {}",
                card.schema_version
            ));
        }
        if card.repair_card_id.is_empty() {
            return Err("card id was not minted".to_string());
        }
        if !card.readiness.repair_ready {
            return Err("ready card did not copy the readiness flip".to_string());
        }
        if card.instruction.state != FixInstructionState::FixSiteReady {
            return Err("instruction state was not copied verbatim".to_string());
        }
        let action = card
            .next_action
            .as_ref()
            .ok_or_else(|| "ready card lost its next action".to_string())?;
        if action.command_id != command.command_id {
            return Err("next action did not copy the command identity".to_string());
        }
        if action.role != "verify" {
            return Err(format!("next action role drifted: {}", action.role));
        }
        if card.assertion_goal != Some(RepairCardAssertionGoal::SuggestedAssertion) {
            return Err("suggested assertion was not projected".to_string());
        }
        if card.selected_basis.as_deref() != Some("direct_owner_call") {
            return Err("selected basis did not fall back to the target relation".to_string());
        }
        match card.selected_target {
            Some(RepairCardTarget::Existing {
                ref relation,
                ref workspace_identity,
                ..
            }) => {
                if relation != "direct_owner_call" {
                    return Err("existing target relation was not copied".to_string());
                }
                if workspace_identity != "fixture" {
                    return Err("existing target identity was not copied".to_string());
                }
            }
            ref other => return Err(format!("expected an existing target, found {other:?}")),
        }
        Ok(())
    }

    #[test]
    fn stale_card_omits_route_and_keeps_instruction() -> Result<(), String> {
        let instruction = instruction(FixInstructionState::Stale);
        let readiness = readiness(
            RepairRouteState::StaticLimitation,
            RepairTargetSelection::Missing,
        );
        let input = base_input(&instruction, &readiness);

        let card = build_repair_card(&input)?;
        if card.readiness.repair_ready {
            return Err("stale card reported a ready route".to_string());
        }
        if card.next_action.is_some() {
            return Err("stale card exposed a route".to_string());
        }
        if card.instruction.state != FixInstructionState::Stale {
            return Err("stale instruction state was not copied".to_string());
        }
        Ok(())
    }

    #[test]
    fn not_ready_with_command_fails_closed() {
        let instruction = instruction(FixInstructionState::StaticLimitation);
        let readiness = readiness(
            RepairRouteState::StaticLimitation,
            RepairTargetSelection::Missing,
        );
        let command = verify_command();
        let mut input = base_input(&instruction, &readiness);
        input.next_command = Some(&command);

        assert!(matches!(build_repair_card(&input), Err(_message)));
    }

    #[test]
    fn limited_card_surfaces_producer_blocker_and_missing_evidence() -> Result<(), String> {
        let mut instruction = instruction(FixInstructionState::StaticLimitation);
        instruction.has_suggested_assertion = false;
        instruction.limitation_kinds = vec!["missing_discriminator_unavailable".to_string()];
        let mut readiness = readiness(
            RepairRouteState::StaticLimitation,
            RepairTargetSelection::Missing,
        );
        readiness.missing_evidence = vec!["owner".to_string()];
        readiness.present_evidence = Vec::new();
        let mut input = base_input(&instruction, &readiness);
        input.exact_blocker = Some("no live coverage authority".to_string());

        let card = build_repair_card(&input)?;
        if card.readiness.repair_ready {
            return Err("limited card reported readiness".to_string());
        }
        if card.assertion_goal.is_some() {
            return Err("limited card invented an assertion goal".to_string());
        }
        if card.readiness.missing_evidence != ["owner".to_string()] {
            return Err("missing evidence was not copied verbatim".to_string());
        }
        if card.exact_blocker.as_deref() != Some("no live coverage authority") {
            return Err("producer blocker was not copied verbatim".to_string());
        }
        Ok(())
    }

    #[test]
    fn ineligible_packet_with_command_fails_closed() {
        let instruction = instruction(FixInstructionState::FixSiteReady);
        let readiness = ready_readiness();
        let command = verify_command();
        let mut input = base_input(&instruction, &readiness);
        input.packet_eligible = false;
        input.next_command = Some(&command);

        assert!(matches!(build_repair_card(&input), Err(_message)));
    }

    #[test]
    fn mismatched_seam_identity_fails_closed() {
        let instruction = instruction(FixInstructionState::FixSiteReady);
        let readiness = ready_readiness();
        let mut input = base_input(&instruction, &readiness);
        input.subject.seam_id = "seam:other".to_string();

        assert!(matches!(build_repair_card(&input), Err(_message)));
    }

    #[test]
    fn mismatched_gap_identity_fails_closed() {
        let instruction = instruction(FixInstructionState::FixSiteReady);
        let readiness = ready_readiness();
        let mut input = base_input(&instruction, &readiness);
        input.subject.canonical_gap_id = Some("gap:other".to_string());

        assert!(matches!(build_repair_card(&input), Err(_message)));
    }

    #[test]
    fn basis_without_target_fails_closed() {
        let instruction = instruction(FixInstructionState::InspectOnly);
        let readiness = readiness(
            RepairRouteState::PolicyExcluded,
            RepairTargetSelection::Missing,
        );
        let mut input = base_input(&instruction, &readiness);
        input.selected_basis = Some("RIPR-0001 observed".to_string());

        assert!(matches!(build_repair_card(&input), Err(_message)));
    }

    #[test]
    fn suggested_assertion_without_text_fails_closed() {
        let instruction = instruction(FixInstructionState::FixSiteReady);
        let readiness = ready_readiness();
        let mut input = base_input(&instruction, &readiness);
        input.assertion_goal_detail = None;

        assert!(matches!(build_repair_card(&input), Err(_message)));
    }

    #[test]
    fn inspect_only_card_is_advisory_only() -> Result<(), String> {
        let mut instruction = instruction(FixInstructionState::InspectOnly);
        instruction.has_suggested_assertion = false;
        let readiness = readiness(
            RepairRouteState::PolicyExcluded,
            RepairTargetSelection::Missing,
        );
        let input = base_input(&instruction, &readiness);

        let card = build_repair_card(&input)?;
        if card.instruction.state != FixInstructionState::InspectOnly {
            return Err("inspect-only state was not copied".to_string());
        }
        if card.selected_target.is_some()
            || card.next_action.is_some()
            || card.selected_basis.is_some()
            || card.assertion_goal.is_some()
        {
            return Err("inspect-only card carried an actionable surface".to_string());
        }
        Ok(())
    }

    #[test]
    fn unavailable_card_has_no_target_and_no_action() -> Result<(), String> {
        let instruction = instruction(FixInstructionState::Unavailable);
        let readiness = readiness(
            RepairRouteState::PolicyExcluded,
            RepairTargetSelection::Missing,
        );
        let input = base_input(&instruction, &readiness);

        let card = build_repair_card(&input)?;
        if card.selected_target.is_some() {
            return Err("unavailable card invented a target".to_string());
        }
        if card.next_action.is_some() {
            return Err("unavailable card exposed a route".to_string());
        }
        if card.selected_basis.is_some() {
            return Err("unavailable card invented a basis".to_string());
        }
        Ok(())
    }

    #[test]
    fn proposed_target_stays_proposed() -> Result<(), String> {
        let readiness = readiness(
            RepairRouteState::Ready,
            RepairTargetSelection::Proposed(NewTestTargetProposal {
                kind: NewTestKind::Integration,
                file: PathBuf::from("tests/new.rs"),
                owner: "src/lib.rs".to_string(),
                provenance: NewTestProposalProvenance::ProducerOwned,
            }),
        );
        let instruction = instruction(FixInstructionState::FixSiteReady);
        let input = base_input(&instruction, &readiness);

        let card = build_repair_card(&input)?;
        match card.selected_target {
            Some(RepairCardTarget::Proposed {
                ref file,
                ref owner,
                proposal_kind,
            }) => {
                if file != "tests/new.rs" || owner != "src/lib.rs" {
                    return Err("proposal fields were not copied verbatim".to_string());
                }
                if proposal_kind != RepairCardProposedTestKind::Integration {
                    return Err("proposal kind was not projected".to_string());
                }
            }
            ref other => return Err(format!("expected a proposed target, found {other:?}")),
        }
        if card.selected_basis.as_deref() != Some("src/lib.rs") {
            return Err("proposed basis did not fall back to the owner".to_string());
        }
        Ok(())
    }

    #[test]
    fn over_boundary_rejected_alternatives_fail_closed() {
        let instruction = instruction(FixInstructionState::FixSiteReady);
        let readiness = ready_readiness();
        let mut input = base_input(&instruction, &readiness);
        input.rejected_alternatives = (0..crate::domain::MAX_REPAIR_CARD_REJECTED_ALTERNATIVES + 1)
            .map(|index| RepairCardRejectedAlternative {
                target: format!("tests/other{index}.rs::name_match"),
                reason: "weak_token_substring".to_string(),
            })
            .collect();

        assert!(matches!(build_repair_card(&input), Err(_message)));
    }

    #[test]
    fn wrong_owner_shape_records_basis_and_rejections() -> Result<(), String> {
        let instruction = instruction(FixInstructionState::FixSiteReady);
        let readiness = ready_readiness();
        let mut input = base_input(&instruction, &readiness);
        input.selected_basis = Some("RIPR-0001 observed".to_string());
        input.rejected_alternatives = vec![
            RepairCardRejectedAlternative {
                target: "tests/other.rs::right_string_wrong_receiver".to_string(),
                reason: "credits another owner".to_string(),
            },
            RepairCardRejectedAlternative {
                target: "tests/other.rs::name_match".to_string(),
                reason: "weak_token_substring".to_string(),
            },
        ];

        let card = build_repair_card(&input)?;
        if card.selected_basis.as_deref() != Some("RIPR-0001 observed") {
            return Err("explicit basis was not kept".to_string());
        }
        if card.rejected_alternatives.len() != 2 {
            return Err("rejected alternatives were not copied".to_string());
        }
        Ok(())
    }

    #[test]
    fn effect_observer_shape_projects_observer_goal_and_improved_axis() -> Result<(), String> {
        let mut instruction = instruction(FixInstructionState::FixSiteReady);
        instruction.has_suggested_assertion = false;
        let readiness = ready_readiness();
        let mut input = base_input(&instruction, &readiness);
        input.has_observer_setup = true;
        input.assertion_goal_detail = Some("observer now sees the effect".to_string());
        input.done_when.static_movement = StaticMovementGoal::ImprovedAccordingToSelectedRoute;

        let card = build_repair_card(&input)?;
        if card.assertion_goal != Some(RepairCardAssertionGoal::ObserverSetup) {
            return Err("observer setup was not projected".to_string());
        }
        if card.done_when.static_movement != StaticMovementGoal::ImprovedAccordingToSelectedRoute {
            return Err("improved axis was not projected".to_string());
        }
        Ok(())
    }

    #[test]
    fn attempt_state_copies_without_upgrade() -> Result<(), String> {
        let manifest = RepairAttemptManifest {
            schema_version: "0.1".to_string(),
            kind: "repair".to_string(),
            repair_attempt_id: RepairAttemptId::parse("repair-attempt-0123456789abcdef01234567")
                .map_err(|error| error.to_string())?,
            state: RepairAttemptState::AwaitingEdit,
            root: ".".to_string(),
            repository_head: "abc123".to_string(),
            producer_version: "test".to_string(),
            seam_id: "seam:demo".to_string(),
            created_unix_ms: 0,
            artifacts: Vec::new(),
            next_command: "cargo test".to_string(),
            limitations: Vec::new(),
            non_claims: Vec::new(),
            after: None,
            last_after_refusal: None,
            terminal_artifacts: Vec::new(),
            store: None,
        };
        let instruction = instruction(FixInstructionState::FixSiteReady);
        let readiness = ready_readiness();
        let mut input = base_input(&instruction, &readiness);
        input.attempt = Some(&manifest);

        let card = build_repair_card(&input)?;
        let attempt = card
            .attempt
            .as_ref()
            .ok_or_else(|| "attempt section was not projected".to_string())?;
        if attempt.attempt_id != "repair-attempt-0123456789abcdef01234567" {
            return Err("attempt id was not copied".to_string());
        }
        if attempt.state != "awaiting_edit" {
            return Err(format!("attempt state drifted: {}", attempt.state));
        }
        Ok(())
    }

    #[test]
    fn ready_card_with_detail_sources_carries_typed_references() -> Result<(), String> {
        let instruction = instruction(FixInstructionState::FixSiteReady);
        let readiness = ready_readiness();
        let command = verify_command();
        let mut input = base_input(&instruction, &readiness);
        input.next_command = Some(&command);
        input.detail_sources = vec![
            RepairCardDetailSource::current(
                RepairCardDetailFamily::CanonicalPacket,
                "workspace:demo/packet/canonical",
                serde_json::json!({ "packet": true }),
            ),
            RepairCardDetailSource::unavailable(
                RepairCardDetailFamily::MutationCalibration,
                "mutation calibration authority does not serve this seam",
            ),
        ];

        let card = build_repair_card(&input)?;
        if card.detail_references.len() != 2 {
            return Err(format!(
                "expected two typed references, found {}",
                card.detail_references.len()
            ));
        }
        if card.detail_references[0].family != RepairCardDetailFamily::CanonicalPacket {
            return Err("references are not in deterministic family order".to_string());
        }
        if card.detail_references[1].family != RepairCardDetailFamily::MutationCalibration {
            return Err("unavailable family lost its reference".to_string());
        }
        if card.detail_summary.referenced_items != 1 || card.detail_summary.unavailable_items != 1 {
            return Err(
                "detail accounting did not count referenced/unavailable families".to_string(),
            );
        }
        if card.complete_evidence_digest.is_empty() {
            return Err("complete evidence identity was not minted".to_string());
        }
        if !card.readiness.repair_ready || card.next_action.is_none() {
            return Err("budgeting changed readiness or actionability".to_string());
        }
        Ok(())
    }

    #[test]
    fn detail_content_move_remints_card_identity() -> Result<(), String> {
        let instruction = instruction(FixInstructionState::FixSiteReady);
        let readiness = ready_readiness();

        let mut first_input = base_input(&instruction, &readiness);
        first_input.detail_sources = vec![RepairCardDetailSource::current(
            RepairCardDetailFamily::WitnessStageEvidence,
            "workspace:demo/probe/witness",
            serde_json::json!({ "note": "before" }),
        )];
        let first = build_repair_card(&first_input)?;

        let mut second_input = base_input(&instruction, &readiness);
        second_input.detail_sources = vec![RepairCardDetailSource::current(
            RepairCardDetailFamily::WitnessStageEvidence,
            "workspace:demo/probe/witness",
            serde_json::json!({ "note": "after" }),
        )];
        let second = build_repair_card(&second_input)?;

        if first.repair_card_id == second.repair_card_id {
            return Err("detail content move did not remint the semantic identity".to_string());
        }
        Ok(())
    }

    #[test]
    fn stale_detail_evidence_does_not_strengthen_or_weaken_route() -> Result<(), String> {
        let instruction = instruction(FixInstructionState::FixSiteReady);
        let readiness = ready_readiness();
        let command = verify_command();
        let mut input = base_input(&instruction, &readiness);
        input.next_command = Some(&command);
        input.detail_sources = vec![RepairCardDetailSource {
            family: RepairCardDetailFamily::WitnessStageEvidence,
            state: RepairCardDetailState::Stale,
            route: Some("workspace:demo/probe/witness".to_string()),
            unavailable_reason: None,
            content: serde_json::json!({ "note": "stale witness" }),
        }];

        let card = build_repair_card(&input)?;
        if card.detail_references[0].state != RepairCardDetailState::Stale {
            return Err("stale evidence state was not projected verbatim".to_string());
        }
        if !card.readiness.repair_ready {
            return Err("stale detail evidence weakened the readiness flip".to_string());
        }
        if card.next_action.is_none() {
            return Err("stale detail evidence stripped the exposed route".to_string());
        }
        Ok(())
    }
}
