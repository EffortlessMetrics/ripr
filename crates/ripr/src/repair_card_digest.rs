//! Semantic digest for `RepairCardV1` (RIPR-SPEC-0192, #4663; detail
//! references added under RIPR-SPEC-0193, #4666).
//!
//! The digest logic lives at the crate root, not in `domain`, because domain
//! must not know JSON rendering (see `command_spec_digest.rs` for the same
//! split). The scoped input below names every load-bearing card field except
//! the digest itself; presentation-only fields (`next_action.display`) never
//! enter identity.

use crate::domain::{
    FixInstructionSummary, RepairCardAssertionGoal, RepairCardAttempt, RepairCardDetailFamily,
    RepairCardDetailState, RepairCardDoneWhen, RepairCardReadinessFacts, RepairCardSnapshot,
    RepairCardSubject, RepairCardTarget, RepairCardV1,
};

/// The scoped semantic surface: every load-bearing field except the digest
/// itself. Presentation-only fields (`next_action.display`) and identity-free
/// envelope fields are excluded here.
#[derive(serde::Serialize)]
struct RepairCardDigestInput<'a> {
    schema_version: &'a str,
    snapshot: &'a RepairCardSnapshot,
    subject: &'a RepairCardSubject,
    instruction: &'a FixInstructionSummary,
    readiness: &'a RepairCardReadinessFacts,
    changed_behavior: &'a str,
    exact_blocker: &'a Option<String>,
    selected_target: &'a Option<RepairCardTarget>,
    assertion_goal: &'a Option<RepairCardAssertionGoal>,
    assertion_goal_detail: &'a Option<String>,
    candidate_value: &'a Option<String>,
    allowed_files: &'a [String],
    forbidden_files: &'a [String],
    done_when: &'a RepairCardDoneWhen,
    stop_conditions: &'a [String],
    next_action: Option<RepairCardDigestCommandRef<'a>>,
    selected_basis: &'a Option<String>,
    rejected_alternatives: &'a [crate::domain::RepairCardRejectedAlternative],
    attempt: &'a Option<RepairCardAttempt>,
    claim_boundary: &'a str,
    limitations: &'a [String],
    /// Routed detail identity: family, producer-reported state, and content
    /// digest. Retrieval routes, ordinals, byte counts, and the budget summary
    /// are presentation/detail and never enter identity (#4666).
    detail_references: Vec<RepairCardDigestDetail<'a>>,
}

#[derive(serde::Serialize)]
struct RepairCardDigestDetail<'a> {
    family: &'a RepairCardDetailFamily,
    state: &'a RepairCardDetailState,
    detail_digest: &'a str,
}

#[derive(serde::Serialize)]
struct RepairCardDigestCommandRef<'a> {
    command_id: &'a str,
    role: &'a str,
}

/// Compute the portable semantic digest of a card. Equivalent roots and
/// presentation-only changes (command display strings) preserve identity;
/// any load-bearing field move remints it.
pub(crate) fn repair_card_semantic_digest(card: &RepairCardV1) -> Result<String, String> {
    let input = RepairCardDigestInput {
        schema_version: &card.schema_version,
        snapshot: &card.snapshot,
        subject: &card.subject,
        instruction: &card.instruction,
        readiness: &card.readiness,
        changed_behavior: &card.changed_behavior,
        exact_blocker: &card.exact_blocker,
        selected_target: &card.selected_target,
        assertion_goal: &card.assertion_goal,
        assertion_goal_detail: &card.assertion_goal_detail,
        candidate_value: &card.candidate_value,
        allowed_files: &card.allowed_files,
        forbidden_files: &card.forbidden_files,
        done_when: &card.done_when,
        stop_conditions: &card.stop_conditions,
        next_action: card
            .next_action
            .as_ref()
            .map(|command| RepairCardDigestCommandRef {
                command_id: &command.command_id,
                role: &command.role,
            }),
        selected_basis: &card.selected_basis,
        rejected_alternatives: &card.rejected_alternatives,
        attempt: &card.attempt,
        claim_boundary: &card.claim_boundary,
        limitations: &card.limitations,
        detail_references: card
            .detail_references
            .iter()
            .map(|reference| RepairCardDigestDetail {
                family: &reference.family,
                state: &reference.state,
                detail_digest: &reference.detail_digest,
            })
            .collect(),
    };
    let serialized = serde_json::to_string(&input)
        .map_err(|error| format!("repair card digest serialization failed: {error}"))?;
    Ok(sha256_hex(serialized.as_bytes()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        FixInstructionState, REPAIR_CARD_CLAIM_BOUNDARY, REPAIR_CARD_SCHEMA_VERSION,
        RepairCardCommandRef, RepairCardDetailSummary, RepairCardProposedTestKind,
        RepairCardSnapshotCurrentness, RepairCardTestKind,
    };

    fn snapshot() -> RepairCardSnapshot {
        RepairCardSnapshot {
            workspace_identity: "workspace:demo".to_string(),
            repository_head: "abc123".to_string(),
            currentness: RepairCardSnapshotCurrentness::Current,
        }
    }

    fn subject() -> RepairCardSubject {
        RepairCardSubject {
            seam_id: "seam:demo".to_string(),
            canonical_gap_id: Some("gap:demo".to_string()),
            finding_id: Some("probe:demo".to_string()),
        }
    }

    fn ready_instruction() -> FixInstructionSummary {
        FixInstructionSummary {
            state: FixInstructionState::FixSiteReady,
            has_fix_site: true,
            has_suggested_assertion: true,
            limitation_kinds: Vec::new(),
        }
    }

    fn readiness_ready() -> RepairCardReadinessFacts {
        RepairCardReadinessFacts {
            repair_ready: true,
            required_evidence: vec!["owner".to_string()],
            present_evidence: vec!["owner".to_string()],
            missing_evidence: Vec::new(),
        }
    }

    fn done_when() -> RepairCardDoneWhen {
        RepairCardDoneWhen {
            static_movement: crate::domain::StaticMovementGoal::ClosedBySelectedRoute,
            focused_test_execution: crate::domain::FocusedExecutionGoal::VerifiedPass,
            edit_cage: crate::domain::EditCageGoal::Compliant,
            mutation_confirmation: crate::domain::MutationConfirmationGoal::NotRequested,
            currentness: crate::domain::CardCurrentnessGoal::Current,
        }
    }

    fn next_action() -> RepairCardCommandRef {
        RepairCardCommandRef {
            command_id: "cmd:verify".to_string(),
            role: "verify".to_string(),
            display: "cargo test --package demo".to_string(),
        }
    }

    fn card_fixture() -> RepairCardV1 {
        RepairCardV1 {
            schema_version: REPAIR_CARD_SCHEMA_VERSION.to_string(),
            repair_card_id: String::new(),
            snapshot: snapshot(),
            subject: subject(),
            instruction: ready_instruction(),
            readiness: readiness_ready(),
            changed_behavior: "expr".to_string(),
            exact_blocker: None,
            selected_target: Some(RepairCardTarget::Existing {
                symbol_id: "symbol:test:demo".to_string(),
                file: "tests/demo.rs".to_string(),
                line: 12,
                test_kind: RepairCardTestKind::Integration,
                relation: "direct_owner_call".to_string(),
                workspace_identity: "workspace:demo".to_string(),
            }),
            assertion_goal: Some(RepairCardAssertionGoal::SuggestedAssertion),
            assertion_goal_detail: Some("assert_eq!(price(0), 0)".to_string()),
            candidate_value: None,
            allowed_files: vec!["tests/demo.rs".to_string()],
            forbidden_files: vec!["src/lib.rs".to_string()],
            done_when: done_when(),
            stop_conditions: vec!["snapshot_stale".to_string()],
            next_action: Some(next_action()),
            selected_basis: Some("direct_owner_call".to_string()),
            rejected_alternatives: vec![crate::domain::RepairCardRejectedAlternative {
                target: "tests/other.rs::name_match".to_string(),
                reason: "weak_token_substring".to_string(),
            }],
            attempt: Some(RepairCardAttempt {
                attempt_id: "ra-0123456789abcdef".to_string(),
                state: "awaiting_edit".to_string(),
            }),
            claim_boundary: REPAIR_CARD_CLAIM_BOUNDARY.to_string(),
            limitations: Vec::new(),
            detail_references: Vec::new(),
            detail_summary: RepairCardDetailSummary::default(),
            complete_evidence_digest: String::new(),
        }
    }

    #[test]
    fn digest_is_stable_across_presentation_only_changes() -> Result<(), String> {
        let mut card = card_fixture();
        let original = repair_card_semantic_digest(&card)?;

        // Command display is presentation: reminting it must not move identity.
        let Some(action) = card.next_action.as_mut() else {
            return Err("fixture carries no next action".to_string());
        };
        action.display = "a different rendered string".to_string();
        let reminted = repair_card_semantic_digest(&card)?;
        if reminted != original {
            return Err(format!(
                "command display entered the semantic identity surface: {reminted}"
            ));
        }

        // The digest field itself never feeds the digest.
        card.repair_card_id = "garbage".to_string();
        let reminted = repair_card_semantic_digest(&card)?;
        if reminted != original {
            return Err("repair_card_id fed its own digest".to_string());
        }
        Ok(())
    }

    #[test]
    fn every_load_bearing_field_move_remints_the_digest() -> Result<(), String> {
        let base = card_fixture();
        let base_digest = repair_card_semantic_digest(&base)?;

        let mut moved = base.clone();
        moved.changed_behavior = "different".to_string();
        if repair_card_semantic_digest(&moved)? == base_digest {
            return Err("changed_behavior is not load-bearing".to_string());
        }

        let mut moved = base.clone();
        moved.instruction.state = FixInstructionState::Stale;
        if repair_card_semantic_digest(&moved)? == base_digest {
            return Err("instruction state is not load-bearing".to_string());
        }

        let mut moved = base.clone();
        moved.readiness.repair_ready = false;
        if repair_card_semantic_digest(&moved)? == base_digest {
            return Err("readiness flip is not load-bearing".to_string());
        }

        let mut moved = base.clone();
        moved.selected_target = None;
        if repair_card_semantic_digest(&moved)? == base_digest {
            return Err("selected target is not load-bearing".to_string());
        }

        let mut moved = base.clone();
        moved.done_when.static_movement =
            crate::domain::StaticMovementGoal::ImprovedAccordingToSelectedRoute;
        if repair_card_semantic_digest(&moved)? == base_digest {
            return Err("done_when static movement axis is not load-bearing".to_string());
        }

        let mut moved = base.clone();
        moved.allowed_files = vec!["tests/other.rs".to_string()];
        if repair_card_semantic_digest(&moved)? == base_digest {
            return Err("allowed files are not load-bearing".to_string());
        }

        let mut moved = base.clone();
        moved.attempt = None;
        if repair_card_semantic_digest(&moved)? == base_digest {
            return Err("attempt state is not load-bearing".to_string());
        }

        let mut moved = base.clone();
        moved.rejected_alternatives.clear();
        if repair_card_semantic_digest(&moved)? == base_digest {
            return Err("rejected alternatives are not load-bearing".to_string());
        }

        let mut moved = base.clone();
        moved.snapshot.currentness = RepairCardSnapshotCurrentness::AcceptedDirtyDraft;
        if repair_card_semantic_digest(&moved)? == base_digest {
            return Err("snapshot currentness is not load-bearing".to_string());
        }

        let mut moved = base.clone();
        moved.snapshot.workspace_identity = "workspace:other".to_string();
        if repair_card_semantic_digest(&moved)? == base_digest {
            return Err("snapshot workspace identity is not load-bearing".to_string());
        }

        let mut moved = base.clone();
        moved.exact_blocker = Some("blocker".to_string());
        if repair_card_semantic_digest(&moved)? == base_digest {
            return Err("exact blocker is not load-bearing".to_string());
        }

        let mut moved = base.clone();
        moved.assertion_goal = None;
        if repair_card_semantic_digest(&moved)? == base_digest {
            return Err("assertion goal is not load-bearing".to_string());
        }

        let mut moved = base.clone();
        moved.assertion_goal_detail = Some("other assertion".to_string());
        if repair_card_semantic_digest(&moved)? == base_digest {
            return Err("assertion goal detail is not load-bearing".to_string());
        }

        let mut moved = base.clone();
        moved.forbidden_files = vec!["src/other.rs".to_string()];
        if repair_card_semantic_digest(&moved)? == base_digest {
            return Err("forbidden files are not load-bearing".to_string());
        }

        let mut moved = base.clone();
        moved.stop_conditions = vec!["other_stop".to_string()];
        if repair_card_semantic_digest(&moved)? == base_digest {
            return Err("stop conditions are not load-bearing".to_string());
        }

        let mut moved = base.clone();
        moved.selected_basis = Some("other basis".to_string());
        if repair_card_semantic_digest(&moved)? == base_digest {
            return Err("selected basis is not load-bearing".to_string());
        }

        let mut moved = base.clone();
        moved.limitations = vec!["limitation".to_string()];
        if repair_card_semantic_digest(&moved)? == base_digest {
            return Err("limitations are not load-bearing".to_string());
        }

        let mut moved = base.clone();
        moved.done_when.focused_test_execution =
            crate::domain::FocusedExecutionGoal::ExplicitlyNotRun;
        if repair_card_semantic_digest(&moved)? == base_digest {
            return Err("done_when focused execution axis is not load-bearing".to_string());
        }
        Ok(())
    }

    #[test]
    fn proposed_and_existing_targets_stay_distinct() -> Result<(), String> {
        let existing = RepairCardTarget::Existing {
            symbol_id: "symbol:test:a".to_string(),
            file: "tests/a.rs".to_string(),
            line: 1,
            test_kind: RepairCardTestKind::InlineUnit,
            relation: "direct_owner_call".to_string(),
            workspace_identity: "workspace:demo".to_string(),
        };
        let proposed = RepairCardTarget::Proposed {
            file: "tests/new.rs".to_string(),
            owner: "src/lib.rs".to_string(),
            proposal_kind: RepairCardProposedTestKind::InlineUnit,
        };
        if existing == proposed {
            return Err("existing and proposed targets collapsed into one variant".to_string());
        }
        let existing_json = serde_json::to_string(&existing).map_err(|error| error.to_string())?;
        let proposed_json = serde_json::to_string(&proposed).map_err(|error| error.to_string())?;
        if !existing_json.contains("\"kind\":\"existing\"") {
            return Err(format!(
                "existing target lost its kind tag: {existing_json}"
            ));
        }
        if !proposed_json.contains("\"kind\":\"proposed\"") {
            return Err(format!(
                "proposed target lost its kind tag: {proposed_json}"
            ));
        }
        Ok(())
    }

    #[test]
    fn done_when_keeps_all_five_axes_separate() -> Result<(), String> {
        let card = card_fixture();
        let json = serde_json::to_string(&card.done_when).map_err(|error| error.to_string())?;
        for axis in [
            "static_movement",
            "focused_test_execution",
            "edit_cage",
            "mutation_confirmation",
            "currentness",
        ] {
            if !json.contains(axis) {
                return Err(format!("done_when lost the `{axis}` axis: {json}"));
            }
        }
        let round_tripped: RepairCardDoneWhen =
            serde_json::from_str(&json).map_err(|error| error.to_string())?;
        if round_tripped != card.done_when {
            return Err("done_when axes did not round trip independently".to_string());
        }
        Ok(())
    }

    #[test]
    fn detail_references_enter_identity_without_retrieval_spelling() -> Result<(), String> {
        fn detail_ref(
            route: &str,
            digest: &str,
            bytes: usize,
            ordinal: usize,
            state: crate::domain::RepairCardDetailState,
        ) -> crate::domain::RepairCardDetailRef {
            crate::domain::RepairCardDetailRef {
                family: crate::domain::RepairCardDetailFamily::WitnessStageEvidence,
                state,
                route: Some(route.to_string()),
                unavailable_reason: None,
                detail_digest: digest.to_string(),
                omitted_bytes: bytes,
                ordinal,
                omission_class: crate::domain::RepairCardOmissionClass::AuthorityOwnedDetail,
            }
        }

        let base = card_fixture();
        let base_digest = repair_card_semantic_digest(&base)?;

        let mut moved = base.clone();
        moved.detail_references.push(detail_ref(
            "workspace:demo/probe/witness",
            "digest-a",
            10,
            0,
            crate::domain::RepairCardDetailState::Current,
        ));
        let with_ref = repair_card_semantic_digest(&moved)?;
        if with_ref == base_digest {
            return Err("detail reference did not enter the semantic identity".to_string());
        }

        let mut respelled = moved.clone();
        let Some(reference) = respelled.detail_references.get_mut(0) else {
            return Err("fixture reference missing".to_string());
        };
        reference.route = Some("workspace:equivalent/probe/witness".to_string());
        if repair_card_semantic_digest(&respelled)? != with_ref {
            return Err("route spelling entered the semantic identity".to_string());
        }

        let mut reaccounted = moved.clone();
        let Some(reference) = reaccounted.detail_references.get_mut(0) else {
            return Err("fixture reference missing".to_string());
        };
        reference.ordinal = 9;
        reference.omitted_bytes = 999;
        if repair_card_semantic_digest(&reaccounted)? != with_ref {
            return Err("ordinal or byte accounting entered the semantic identity".to_string());
        }

        let mut rederived = moved.clone();
        rederived.complete_evidence_digest = "garbage".to_string();
        rederived.detail_summary.selected_bytes = 123_456;
        if repair_card_semantic_digest(&rederived)? != with_ref {
            return Err("budget accounting entered the semantic identity".to_string());
        }

        let mut restated = moved.clone();
        let Some(reference) = restated.detail_references.get_mut(0) else {
            return Err("fixture reference missing".to_string());
        };
        reference.detail_digest = "digest-b".to_string();
        if repair_card_semantic_digest(&restated)? == with_ref {
            return Err("detail content digest is not load-bearing".to_string());
        }

        let mut stale = moved.clone();
        let Some(reference) = stale.detail_references.get_mut(0) else {
            return Err("fixture reference missing".to_string());
        };
        reference.state = crate::domain::RepairCardDetailState::Stale;
        if repair_card_semantic_digest(&stale)? == with_ref {
            return Err("detail state is not load-bearing".to_string());
        }
        Ok(())
    }
}
