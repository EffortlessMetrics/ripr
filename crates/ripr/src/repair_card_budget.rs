//! Detail/budget engine for `RepairCardV1` (RIPR-SPEC-0193, #4666).
//!
//! The engine lives at the crate root, not in `domain`, because byte
//! budgeting must measure the normalized serialized representation and domain
//! must not know JSON rendering (the same split as `repair_card_digest.rs`).
//!
//! The engine keeps the default wire card finite without discarding evidence:
//! every load-bearing detail family rides behind a stable typed reference
//! (digest + measured bytes + route, or an exact unavailable reason), and the
//! card's own compact prose fields are fail-closed against the inline budget.
//! Budgeting changes presentation/detail only — it never changes readiness,
//! target selection, actionability, or the semantic card identity.

use crate::domain::{
    RepairCardBudget, RepairCardDetailFamily, RepairCardDetailRef, RepairCardDetailState,
    RepairCardDetailSummary, RepairCardOmissionClass, RepairCardV1,
};

/// Producer-owned detail source for one evidence family. `content` is the
/// authority's own serialization; the engine only normalizes and measures it.
#[derive(Clone)]
pub(crate) struct RepairCardDetailSource {
    pub(crate) family: RepairCardDetailFamily,
    pub(crate) state: RepairCardDetailState,
    /// Stable portable retrieval route (`None` exactly when unavailable).
    pub(crate) route: Option<String>,
    /// Exact producer-owned reason (`Some` exactly when unavailable).
    pub(crate) unavailable_reason: Option<String>,
    /// Authority-owned content. `Null` exactly for Missing/Unavailable
    /// families; the engine never invents content.
    pub(crate) content: serde_json::Value,
}

impl RepairCardDetailSource {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "staged internal contract; #4667 connects the first producer-owned detail sources"
        )
    )]
    pub(crate) fn current(
        family: RepairCardDetailFamily,
        route: &str,
        content: serde_json::Value,
    ) -> Self {
        Self {
            family,
            state: RepairCardDetailState::Current,
            route: Some(route.to_string()),
            unavailable_reason: None,
            content,
        }
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "staged internal contract; #4667 connects the first producer-owned detail sources"
        )
    )]
    pub(crate) fn unavailable(family: RepairCardDetailFamily, reason: &str) -> Self {
        Self {
            family,
            state: RepairCardDetailState::Unavailable,
            route: None,
            unavailable_reason: Some(reason.to_string()),
            content: serde_json::Value::Null,
        }
    }
}

/// Apply the versioned item/byte budget to a freshly built card, attaching
/// detail references, measured accounting, and the complete-evidence digest.
/// The semantic id is minted here over the attached references for measurement
/// and evidence binding; the builder re-mints `repair_card_id` afterwards and
/// recomputes the identical value (the digest input excludes the id field,
/// the summary, and the complete-evidence digest).
pub(crate) fn apply_repair_card_budget(
    card: &mut RepairCardV1,
    sources: &[RepairCardDetailSource],
    budget: &RepairCardBudget,
) -> Result<(), String> {
    budget.validate()?;
    enforce_inline_budget(card, budget)?;

    if sources.len() > budget.max_detail_items {
        return Err(format!(
            "{} detail families exceed the reviewed item bound {}",
            sources.len(),
            budget.max_detail_items
        ));
    }

    let mut ordered = sources.to_vec_sorted_by_family()?;
    let mut references = Vec::with_capacity(ordered.len());
    let mut omitted_bytes = 0usize;
    let mut omission_classes = Vec::new();
    let mut identity_pairs = Vec::with_capacity(ordered.len());
    let mut unavailable_items = 0usize;

    for (ordinal, source) in ordered.drain(..).enumerate() {
        let class = omission_class(&source);
        let (digest, bytes) = normalize_source(&source)?;
        if source.state == RepairCardDetailState::Unavailable {
            unavailable_items += 1;
        } else {
            omitted_bytes = omitted_bytes.saturating_add(bytes);
        }
        identity_pairs.push((family_key(source.family), digest.clone()));
        omission_classes.push(class);
        references.push(RepairCardDetailRef {
            family: source.family,
            state: source.state,
            route: source.route.clone(),
            unavailable_reason: source.unavailable_reason.clone(),
            detail_digest: digest,
            omitted_bytes: bytes,
            ordinal,
            omission_class: class,
        });
    }

    omission_classes.sort();
    omission_classes.dedup();

    // Attach references with a zeroed summary first so the measured
    // `selected_bytes` is deterministic (the card serialized with a zeroed
    // self-reference), then record the real accounting.
    card.detail_references = references;
    card.detail_summary = RepairCardDetailSummary {
        referenced_items: card.detail_references.len() - unavailable_items,
        unavailable_items,
        selected_bytes: 0,
        omitted_bytes,
        complete_bytes: 0,
        omission_classes,
        ..RepairCardDetailSummary::default()
    };

    // Mint the semantic id over the attached references before measuring or
    // binding the complete evidence. The digest input excludes
    // `repair_card_id`, `detail_summary`, and `complete_evidence_digest`, so
    // the builder's later mint recomputes this identical id: the complete
    // digest binds card identity to evidence identity without a cycle.
    let semantic_id = crate::repair_card_digest::repair_card_semantic_digest(card)?;
    card.complete_evidence_digest = complete_digest(&semantic_id, &identity_pairs);

    // Measure the wire bound against the finalized card. The semantic id is
    // part of the serialized card and is not the summary self-reference the
    // spec zeroes, so the measured copy carries the real id with the summary
    // counts still zeroed.
    let mut measured = card.clone();
    measured.repair_card_id = semantic_id;
    let selected_bytes = normalized_len(&measured)?;
    if selected_bytes > budget.max_serialized_bytes {
        return Err(format!(
            "wire card needs {selected_bytes} normalized bytes, exceeding the reviewed byte bound {}",
            budget.max_serialized_bytes
        ));
    }
    card.detail_summary.selected_bytes = selected_bytes;
    card.detail_summary.complete_bytes = selected_bytes.saturating_add(omitted_bytes);
    Ok(())
}

/// Fail closed when a compact prose field exceeds the inline budget: the full
/// content must live behind the owning family's detail reference instead of
/// being silently truncated or serialized unbounded. Fields are measured as
/// serialized JSON so escaping cannot smuggle extra wire bytes past the bound.
fn enforce_inline_budget(card: &RepairCardV1, budget: &RepairCardBudget) -> Result<(), String> {
    let fields: Vec<(&str, String)> = vec![
        (
            "changed_behavior",
            serde_json::to_string(&card.changed_behavior).map_err(|error| error.to_string())?,
        ),
        (
            "exact_blocker",
            serde_json::to_string(&card.exact_blocker).map_err(|error| error.to_string())?,
        ),
        (
            "assertion_goal_detail",
            serde_json::to_string(&card.assertion_goal_detail)
                .map_err(|error| error.to_string())?,
        ),
        (
            "candidate_value",
            serde_json::to_string(&card.candidate_value).map_err(|error| error.to_string())?,
        ),
        (
            "limitations",
            serde_json::to_string(&card.limitations).map_err(|error| error.to_string())?,
        ),
        (
            "stop_conditions",
            serde_json::to_string(&card.stop_conditions).map_err(|error| error.to_string())?,
        ),
        (
            "allowed_files",
            serde_json::to_string(&card.allowed_files).map_err(|error| error.to_string())?,
        ),
        (
            "forbidden_files",
            serde_json::to_string(&card.forbidden_files).map_err(|error| error.to_string())?,
        ),
    ];
    for (name, serialized) in fields {
        let bytes = serialized.len();
        if bytes > budget.max_inline_detail_bytes {
            return Err(format!(
                "compact field `{name}` needs {bytes} normalized bytes, exceeding the inline bound {}; move the full content behind its detail reference",
                budget.max_inline_detail_bytes
            ));
        }
    }
    Ok(())
}

fn omission_class(source: &RepairCardDetailSource) -> RepairCardOmissionClass {
    if source.state == RepairCardDetailState::Unavailable {
        return RepairCardOmissionClass::Unavailable;
    }
    if source.family == RepairCardDetailFamily::CanonicalPacket {
        return RepairCardOmissionClass::NotEmbeddable;
    }
    RepairCardOmissionClass::AuthorityOwnedDetail
}

/// Validate the source state/route/reason/content contract, then normalize the
/// content and return (content digest, byte count). An unavailable or missing
/// family hashes the normalized `null` and carries no omitted bytes.
fn normalize_source(source: &RepairCardDetailSource) -> Result<(String, usize), String> {
    let unavailable = source.state == RepairCardDetailState::Unavailable;
    if unavailable {
        if source.route.is_some() {
            return Err(format!(
                "unavailable family {} must not carry a retrieval route",
                family_key(source.family)
            ));
        }
        let reason = source
            .unavailable_reason
            .as_deref()
            .unwrap_or_default()
            .trim();
        if reason.is_empty() {
            return Err(format!(
                "unavailable family {} must record an exact unavailable reason",
                family_key(source.family)
            ));
        }
        if !source.content.is_null() {
            return Err(format!(
                "unavailable family {} must not claim content",
                family_key(source.family)
            ));
        }
    } else {
        if source.unavailable_reason.is_some() {
            return Err(format!(
                "family {} with state {:?} must not carry an unavailable reason",
                family_key(source.family),
                source.state
            ));
        }
        let route = source.route.as_deref().unwrap_or_default().trim();
        if route.is_empty() {
            return Err(format!(
                "family {} with state {:?} must carry a stable retrieval route",
                family_key(source.family),
                source.state
            ));
        }
        reject_root_specific_route(source.family, route)?;
        let expects_content = source.state != RepairCardDetailState::Missing;
        if expects_content && source.content.is_null() {
            return Err(format!(
                "family {} with state {:?} must carry authority-owned content",
                family_key(source.family),
                source.state
            ));
        }
        if !expects_content && !source.content.is_null() {
            return Err(format!(
                "missing family {} must not claim content",
                family_key(source.family)
            ));
        }
    }
    let normalized = serde_json::to_string(&source.content).map_err(|error| error.to_string())?;
    let digest = sha256_hex(normalized.as_bytes());
    let bytes = if unavailable || source.state == RepairCardDetailState::Missing {
        0
    } else {
        normalized.len()
    };
    Ok((digest, bytes))
}

/// Portable identities only: an absolute checkout spelling would make
/// equivalent roots mint different references, so the engine refuses to carry
/// one. A producer that observes a wrong-root route reports that state
/// instead; the card shows it visibly and never repairs it silently.
fn reject_root_specific_route(family: RepairCardDetailFamily, route: &str) -> Result<(), String> {
    let root_specific = route.starts_with('/')
        || route
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_alphabetic())
            && route.as_bytes().get(1) == Some(&b':')
            && matches!(route.as_bytes().get(2), Some(b'/') | Some(b'\\'));
    if root_specific {
        return Err(format!(
            "family {} route is a root-specific spelling, not a portable route",
            family_key(family)
        ));
    }
    Ok(())
}

/// Deterministic family ordering key. The spelling is pinned against the
/// serde snake_case wire name by the `detail_vocabulary_round_trips_snake_case`
/// round-trip test, so the static table cannot drift from the wire spelling.
fn family_key(family: RepairCardDetailFamily) -> &'static str {
    match family {
        RepairCardDetailFamily::FixInstruction => "fix_instruction",
        RepairCardDetailFamily::WitnessStageEvidence => "witness_stage_evidence",
        RepairCardDetailFamily::RelatedTestCandidates => "related_test_candidates",
        RepairCardDetailFamily::LimitationDetail => "limitation_detail",
        RepairCardDetailFamily::CanonicalPacket => "canonical_packet",
        RepairCardDetailFamily::RepairAttemptStatus => "repair_attempt_status",
        RepairCardDetailFamily::FocusedProofReceipt => "focused_proof_receipt",
        RepairCardDetailFamily::StaticMovement => "static_movement",
        RepairCardDetailFamily::MutationCalibration => "mutation_calibration",
    }
}

/// The identity of the complete evidence: the semantic card id joined with
/// every routed family's content digest in deterministic family order, so the
/// digest binds the card's repair facts to its routed evidence. Budget-independent.
fn complete_digest(semantic_id: &str, identity_pairs: &[(&str, String)]) -> String {
    let mut joined = String::with_capacity(semantic_id.len() + identity_pairs.len() * 80 + 1);
    joined.push_str(semantic_id);
    joined.push('|');
    for (index, (family, digest)) in identity_pairs.iter().enumerate() {
        if index > 0 {
            joined.push(',');
        }
        joined.push_str(family);
        joined.push('=');
        joined.push_str(digest);
    }
    sha256_hex(joined.as_bytes())
}

fn normalized_len(card: &RepairCardV1) -> Result<usize, String> {
    let serialized = serde_json::to_string(card).map_err(|error| error.to_string())?;
    Ok(serialized.len())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    use std::fmt::Write as _;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

trait SortByFamily {
    fn to_vec_sorted_by_family(&self) -> Result<Vec<RepairCardDetailSource>, String>;
}

impl SortByFamily for [RepairCardDetailSource] {
    fn to_vec_sorted_by_family(&self) -> Result<Vec<RepairCardDetailSource>, String> {
        let mut ordered = self.to_vec();
        ordered.sort_by_key(|source| family_key(source.family));
        for window in ordered.windows(2) {
            if window[0].family == window[1].family {
                return Err(format!(
                    "duplicate detail family {} in one card",
                    family_key(window[0].family)
                ));
            }
        }
        Ok(ordered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        CardCurrentnessGoal, EditCageGoal, FixInstructionState, FixInstructionSummary,
        FocusedExecutionGoal, MutationConfirmationGoal, REPAIR_CARD_CLAIM_BOUNDARY,
        REPAIR_CARD_SCHEMA_VERSION, RepairCardDoneWhen, RepairCardReadinessFacts,
        RepairCardSnapshot, RepairCardSnapshotCurrentness, RepairCardSubject, StaticMovementGoal,
    };

    fn minimal_card() -> RepairCardV1 {
        RepairCardV1 {
            schema_version: REPAIR_CARD_SCHEMA_VERSION.to_string(),
            repair_card_id: String::new(),
            snapshot: RepairCardSnapshot {
                workspace_identity: "workspace:demo".to_string(),
                repository_head: "abc123".to_string(),
                currentness: RepairCardSnapshotCurrentness::Current,
            },
            subject: RepairCardSubject {
                seam_id: "seam:demo".to_string(),
                canonical_gap_id: None,
                finding_id: None,
            },
            instruction: FixInstructionSummary {
                state: FixInstructionState::FixSiteReady,
                has_fix_site: true,
                has_suggested_assertion: false,
                limitation_kinds: Vec::new(),
            },
            readiness: RepairCardReadinessFacts {
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
            done_when: RepairCardDoneWhen {
                static_movement: StaticMovementGoal::ClosedBySelectedRoute,
                focused_test_execution: FocusedExecutionGoal::VerifiedPass,
                edit_cage: EditCageGoal::Compliant,
                mutation_confirmation: MutationConfirmationGoal::NotRequested,
                currentness: CardCurrentnessGoal::Current,
            },
            stop_conditions: Vec::new(),
            next_action: None,
            selected_basis: None,
            rejected_alternatives: Vec::new(),
            attempt: None,
            claim_boundary: REPAIR_CARD_CLAIM_BOUNDARY.to_string(),
            limitations: Vec::new(),
            detail_references: Vec::new(),
            detail_summary: RepairCardDetailSummary::default(),
            complete_evidence_digest: String::new(),
        }
    }

    fn json(text: &str) -> serde_json::Value {
        serde_json::json!({ "note": text })
    }

    fn all_family_sources() -> Vec<RepairCardDetailSource> {
        vec![
            RepairCardDetailSource::current(
                RepairCardDetailFamily::WitnessStageEvidence,
                "workspace:demo/probe/witness",
                json("witness"),
            ),
            RepairCardDetailSource::current(
                RepairCardDetailFamily::FixInstruction,
                "workspace:demo/instruction/full",
                json("instruction"),
            ),
            RepairCardDetailSource::unavailable(
                RepairCardDetailFamily::MutationCalibration,
                "mutation calibration authority does not serve this seam",
            ),
            RepairCardDetailSource::current(
                RepairCardDetailFamily::CanonicalPacket,
                "workspace:demo/packet/canonical",
                json("packet"),
            ),
            RepairCardDetailSource {
                family: RepairCardDetailFamily::RelatedTestCandidates,
                state: RepairCardDetailState::Stale,
                route: Some("workspace:demo/tests/related".to_string()),
                unavailable_reason: None,
                content: json("related"),
            },
            RepairCardDetailSource {
                family: RepairCardDetailFamily::LimitationDetail,
                state: RepairCardDetailState::Malformed,
                route: Some("workspace:demo/limitation/detail".to_string()),
                unavailable_reason: None,
                content: json("limitation"),
            },
            RepairCardDetailSource {
                family: RepairCardDetailFamily::RepairAttemptStatus,
                state: RepairCardDetailState::WrongRoot,
                route: Some("workspace:other/attempt/status".to_string()),
                unavailable_reason: None,
                content: json("attempt"),
            },
            RepairCardDetailSource {
                family: RepairCardDetailFamily::FocusedProofReceipt,
                state: RepairCardDetailState::Missing,
                route: Some("workspace:demo/receipt/focused".to_string()),
                unavailable_reason: None,
                content: serde_json::Value::Null,
            },
            RepairCardDetailSource::current(
                RepairCardDetailFamily::StaticMovement,
                "workspace:demo/movement/detail",
                json("movement"),
            ),
        ]
    }

    #[test]
    fn default_budget_routes_every_family_in_deterministic_order() -> Result<(), String> {
        let mut card = minimal_card();
        let sources = all_family_sources();
        apply_repair_card_budget(&mut card, &sources, &RepairCardBudget::default())?;

        let keys = card
            .detail_references
            .iter()
            .map(|reference| family_key(reference.family))
            .collect::<Vec<_>>();
        let mut sorted = keys.clone();
        sorted.sort();
        if keys != sorted {
            return Err(format!(
                "references are not in deterministic family order: {keys:?}"
            ));
        }
        for (ordinal, reference) in card.detail_references.iter().enumerate() {
            if reference.ordinal != ordinal {
                return Err(format!(
                    "reference {} carries ordinal {}, expected {ordinal}",
                    family_key(reference.family),
                    reference.ordinal
                ));
            }
        }
        if card.detail_summary.referenced_items != 8 || card.detail_summary.unavailable_items != 1 {
            return Err(format!(
                "unexpected accounting: {} referenced, {} unavailable",
                card.detail_summary.referenced_items, card.detail_summary.unavailable_items
            ));
        }
        if card.detail_summary.budget_version != crate::domain::REPAIR_CARD_BUDGET_VERSION {
            return Err("summary does not name the budget version".to_string());
        }
        let packet = card
            .detail_references
            .iter()
            .find(|reference| reference.family == RepairCardDetailFamily::CanonicalPacket)
            .ok_or_else(|| "canonical packet reference missing".to_string())?;
        if packet.omission_class != RepairCardOmissionClass::NotEmbeddable {
            return Err("canonical packet must stay not-embeddable".to_string());
        }
        let calibration = card
            .detail_references
            .iter()
            .find(|reference| reference.family == RepairCardDetailFamily::MutationCalibration)
            .ok_or_else(|| "mutation calibration reference missing".to_string())?;
        if calibration.omission_class != RepairCardOmissionClass::Unavailable {
            return Err("unavailable family lost its omission class".to_string());
        }
        if calibration.route.is_some() || calibration.unavailable_reason.is_none() {
            return Err("unavailable family must carry a reason and no route".to_string());
        }
        let mut classes = card.detail_summary.omission_classes.clone();
        classes.sort();
        classes.dedup();
        if classes != card.detail_summary.omission_classes {
            return Err("omission classes are not sorted and deduplicated".to_string());
        }
        if card.detail_summary.selected_bytes
            > crate::domain::DEFAULT_REPAIR_CARD_MAX_SERIALIZED_BYTES
        {
            return Err("card exceeded the reviewed byte bound".to_string());
        }
        Ok(())
    }

    #[test]
    fn byte_accounting_measures_the_actual_normalized_representation() -> Result<(), String> {
        let mut card = minimal_card();
        let content = json("abc");
        let expected = serde_json::to_string(&content)
            .map_err(|error| error.to_string())?
            .len();
        apply_repair_card_budget(
            &mut card,
            &[RepairCardDetailSource::current(
                RepairCardDetailFamily::WitnessStageEvidence,
                "workspace:demo/probe/witness",
                content,
            )],
            &RepairCardBudget::default(),
        )?;
        let reference = &card.detail_references[0];
        if reference.omitted_bytes != expected {
            return Err(format!(
                "omitted bytes {} do not match the normalized representation {expected}",
                reference.omitted_bytes
            ));
        }
        if card.detail_summary.omitted_bytes != expected {
            return Err("summary omitted bytes do not match the reference".to_string());
        }
        if card.detail_summary.complete_bytes != card.detail_summary.selected_bytes + expected {
            return Err("complete bytes are not selected + omitted".to_string());
        }
        Ok(())
    }

    #[test]
    fn oversized_single_detail_stays_bounded_and_visible() -> Result<(), String> {
        let mut card = minimal_card();
        let large = "x".repeat(1024 * 1024 + 7);
        let content = json(&large);
        let expected = serde_json::to_string(&content)
            .map_err(|error| error.to_string())?
            .len();
        apply_repair_card_budget(
            &mut card,
            &[RepairCardDetailSource::current(
                RepairCardDetailFamily::WitnessStageEvidence,
                "workspace:demo/probe/witness",
                content,
            )],
            &RepairCardBudget::default(),
        )?;
        if card.detail_references.len() != 1 {
            return Err("oversized detail disappeared".to_string());
        }
        let reference = &card.detail_references[0];
        if reference.omitted_bytes != expected {
            return Err("oversized detail bytes were not measured".to_string());
        }
        if reference.detail_digest.is_empty() {
            return Err("oversized detail lost its content identity".to_string());
        }
        if card.detail_summary.selected_bytes
            > crate::domain::DEFAULT_REPAIR_CARD_MAX_SERIALIZED_BYTES
        {
            return Err("oversized detail forced unbounded card serialization".to_string());
        }
        if card.detail_summary.complete_bytes != card.detail_summary.selected_bytes + expected {
            return Err("complete bytes must include the oversized detail".to_string());
        }
        Ok(())
    }

    #[test]
    fn equivalent_facts_keep_identity_regardless_of_source_order() -> Result<(), String> {
        let mut forward = minimal_card();
        let sources = all_family_sources();
        apply_repair_card_budget(&mut forward, &sources, &RepairCardBudget::default())?;

        let mut reversed = minimal_card();
        let mut flipped = all_family_sources();
        flipped.reverse();
        apply_repair_card_budget(&mut reversed, &flipped, &RepairCardBudget::default())?;

        if forward.detail_references != reversed.detail_references {
            return Err("source order leaked into the references".to_string());
        }
        if forward.complete_evidence_digest != reversed.complete_evidence_digest {
            return Err("source order leaked into the complete evidence identity".to_string());
        }
        Ok(())
    }

    #[test]
    fn route_spelling_is_presentation_not_identity() -> Result<(), String> {
        let mut first = minimal_card();
        apply_repair_card_budget(
            &mut first,
            &[RepairCardDetailSource::current(
                RepairCardDetailFamily::WitnessStageEvidence,
                "workspace:demo/probe/witness",
                json("same"),
            )],
            &RepairCardBudget::default(),
        )?;
        let mut second = minimal_card();
        apply_repair_card_budget(
            &mut second,
            &[RepairCardDetailSource::current(
                RepairCardDetailFamily::WitnessStageEvidence,
                "workspace:equivalent/probe/witness",
                json("same"),
            )],
            &RepairCardBudget::default(),
        )?;
        if first.detail_references[0].detail_digest != second.detail_references[0].detail_digest {
            return Err("equivalent roots minted different content identities".to_string());
        }
        if first.complete_evidence_digest != second.complete_evidence_digest {
            return Err("route spelling entered the complete evidence identity".to_string());
        }
        Ok(())
    }

    #[test]
    fn unavailable_family_requires_exact_reason_and_no_content() {
        let cases = vec![
            RepairCardDetailSource {
                family: RepairCardDetailFamily::MutationCalibration,
                state: RepairCardDetailState::Unavailable,
                route: Some("workspace:demo/calibration".to_string()),
                unavailable_reason: Some("authority declined".to_string()),
                content: serde_json::Value::Null,
            },
            RepairCardDetailSource {
                family: RepairCardDetailFamily::MutationCalibration,
                state: RepairCardDetailState::Unavailable,
                route: None,
                unavailable_reason: Some("   ".to_string()),
                content: serde_json::Value::Null,
            },
            RepairCardDetailSource {
                family: RepairCardDetailFamily::MutationCalibration,
                state: RepairCardDetailState::Unavailable,
                route: None,
                unavailable_reason: Some("authority declined".to_string()),
                content: json("claimed anyway"),
            },
        ];
        for case in cases {
            let mut card = minimal_card();
            assert!(matches!(
                apply_repair_card_budget(&mut card, &[case], &RepairCardBudget::default()),
                Err(_message)
            ));
        }
    }

    #[test]
    fn routed_states_require_portable_routes_and_content() {
        let base = RepairCardDetailSource::current(
            RepairCardDetailFamily::WitnessStageEvidence,
            "workspace:demo/probe/witness",
            json("witness"),
        );
        let mut without_route = base.clone();
        without_route.route = None;
        let mut null_content = base.clone();
        null_content.content = serde_json::Value::Null;
        let mut with_reason = base.clone();
        with_reason.unavailable_reason = Some("no reason allowed".to_string());
        let mut missing_with_content = base.clone();
        missing_with_content.state = RepairCardDetailState::Missing;
        missing_with_content.content = json("present after all");
        let mut absolute = base.clone();
        absolute.route = Some("/srv/ripr/probe".to_string());
        let mut drive = base.clone();
        drive.route = Some(concat!("C", ":/checkout/probe").to_string());
        for case in [
            without_route,
            null_content,
            with_reason,
            missing_with_content,
            absolute,
            drive,
        ] {
            let mut card = minimal_card();
            assert!(matches!(
                apply_repair_card_budget(&mut card, &[case], &RepairCardBudget::default()),
                Err(_message)
            ));
        }
    }

    #[test]
    fn stale_malformed_wrongroot_missing_states_stay_visible() -> Result<(), String> {
        let mut card = minimal_card();
        apply_repair_card_budget(
            &mut card,
            &all_family_sources(),
            &RepairCardBudget::default(),
        )?;
        let state_of = |family: RepairCardDetailFamily| {
            card.detail_references
                .iter()
                .find(|reference| reference.family == family)
                .map(|reference| reference.state)
        };
        if state_of(RepairCardDetailFamily::RelatedTestCandidates)
            != Some(RepairCardDetailState::Stale)
        {
            return Err("stale evidence was not kept visibly stale".to_string());
        }
        if state_of(RepairCardDetailFamily::LimitationDetail)
            != Some(RepairCardDetailState::Malformed)
        {
            return Err("malformed evidence was not kept visibly malformed".to_string());
        }
        if state_of(RepairCardDetailFamily::RepairAttemptStatus)
            != Some(RepairCardDetailState::WrongRoot)
        {
            return Err("wrong-root evidence was not kept visibly wrong-root".to_string());
        }
        let missing = card
            .detail_references
            .iter()
            .find(|reference| reference.family == RepairCardDetailFamily::FocusedProofReceipt)
            .ok_or_else(|| "missing family lost its reference".to_string())?;
        if missing.state != RepairCardDetailState::Missing || missing.omitted_bytes != 0 {
            return Err("missing family must keep its route with zero omitted bytes".to_string());
        }
        Ok(())
    }

    #[test]
    fn budgeting_never_strengthens_card_state() -> Result<(), String> {
        let mut card = minimal_card();
        let stale = RepairCardDetailSource {
            family: RepairCardDetailFamily::WitnessStageEvidence,
            state: RepairCardDetailState::Stale,
            route: Some("workspace:demo/probe/witness".to_string()),
            unavailable_reason: None,
            content: json("stale witness"),
        };
        apply_repair_card_budget(&mut card, &[stale], &RepairCardBudget::default())?;
        if !card.readiness.repair_ready {
            return Err("budgeting weakened the readiness flip".to_string());
        }
        if card.instruction.state != FixInstructionState::FixSiteReady {
            return Err("budgeting changed the instruction state".to_string());
        }
        Ok(())
    }

    #[test]
    fn duplicate_family_fails_closed() {
        let mut card = minimal_card();
        let one = RepairCardDetailSource::current(
            RepairCardDetailFamily::WitnessStageEvidence,
            "workspace:demo/probe/a",
            json("a"),
        );
        let two = RepairCardDetailSource::current(
            RepairCardDetailFamily::WitnessStageEvidence,
            "workspace:demo/probe/b",
            json("b"),
        );
        assert!(matches!(
            apply_repair_card_budget(&mut card, &[one, two], &RepairCardBudget::default()),
            Err(_message)
        ));
    }

    #[test]
    fn oversized_compact_field_fails_closed_naming_the_field() -> Result<(), String> {
        let mut card = minimal_card();
        card.assertion_goal_detail = Some("y".repeat(4097));
        let sources = [RepairCardDetailSource::current(
            RepairCardDetailFamily::FixInstruction,
            "workspace:demo/instruction/full",
            json("full instruction"),
        )];
        let Err(message) =
            apply_repair_card_budget(&mut card, &sources, &RepairCardBudget::default())
        else {
            return Err("oversized compact field was not refused".to_string());
        };
        if !message.contains("assertion_goal_detail") {
            return Err(format!("error did not name the field: {message}"));
        }
        Ok(())
    }

    #[test]
    fn item_bound_is_enforced() {
        let mut card = minimal_card();
        let sources = all_family_sources();
        let budget = RepairCardBudget {
            max_detail_items: 2,
            ..RepairCardBudget::default()
        };
        assert!(matches!(
            apply_repair_card_budget(&mut card, &sources, &budget),
            Err(_message)
        ));
    }

    #[test]
    fn budget_numbers_are_presentation_not_identity() -> Result<(), String> {
        let sources = all_family_sources();
        let mut first = minimal_card();
        apply_repair_card_budget(&mut first, &sources, &RepairCardBudget::default())?;
        let mut second = minimal_card();
        let tighter = RepairCardBudget {
            max_detail_items: 9,
            max_serialized_bytes: 32 * 1024,
            max_inline_detail_bytes: 2 * 1024,
        };
        apply_repair_card_budget(&mut second, &sources, &tighter)?;
        if first.detail_references != second.detail_references {
            return Err("budget numbers changed the references".to_string());
        }
        if first.complete_evidence_digest != second.complete_evidence_digest {
            return Err("budget numbers changed the complete evidence identity".to_string());
        }
        Ok(())
    }

    #[test]
    fn empty_projection_stays_finite_and_stable() -> Result<(), String> {
        let mut first = minimal_card();
        apply_repair_card_budget(&mut first, &[], &RepairCardBudget::default())?;
        if !first.detail_references.is_empty() || first.detail_summary.referenced_items != 0 {
            return Err("empty projection grew references".to_string());
        }
        if first.detail_summary.selected_bytes == 0 {
            return Err("selected bytes were not measured".to_string());
        }
        let mut second = minimal_card();
        apply_repair_card_budget(&mut second, &[], &RepairCardBudget::default())?;
        if first.complete_evidence_digest != second.complete_evidence_digest {
            return Err("empty projection identity is unstable".to_string());
        }
        Ok(())
    }

    #[test]
    fn complete_digest_binds_card_identity_to_evidence() -> Result<(), String> {
        let sources = [RepairCardDetailSource::current(
            RepairCardDetailFamily::WitnessStageEvidence,
            "workspace:demo/probe/witness",
            json("witness"),
        )];
        let mut first = minimal_card();
        apply_repair_card_budget(&mut first, &sources, &RepairCardBudget::default())?;
        let mut different_facts = minimal_card();
        different_facts.changed_behavior = "other changed behavior".to_string();
        apply_repair_card_budget(&mut different_facts, &sources, &RepairCardBudget::default())?;
        if first.complete_evidence_digest == different_facts.complete_evidence_digest {
            return Err("complete digest ignored the card identity".to_string());
        }
        let mut equivalent = minimal_card();
        apply_repair_card_budget(&mut equivalent, &sources, &RepairCardBudget::default())?;
        if first.complete_evidence_digest != equivalent.complete_evidence_digest {
            return Err("equivalent cards minted different complete digests".to_string());
        }
        Ok(())
    }

    #[test]
    fn wire_bound_is_measured_with_the_finalized_card_id() -> Result<(), String> {
        let mut measured = minimal_card();
        apply_repair_card_budget(&mut measured, &[], &RepairCardBudget::default())?;
        let selected = measured.detail_summary.selected_bytes;
        let mut exact = minimal_card();
        apply_repair_card_budget(
            &mut exact,
            &[],
            &RepairCardBudget {
                max_serialized_bytes: selected,
                ..RepairCardBudget::default()
            },
        )?;
        let mut one_under = minimal_card();
        let Err(_message) = apply_repair_card_budget(
            &mut one_under,
            &[],
            &RepairCardBudget {
                max_serialized_bytes: selected - 1,
                ..RepairCardBudget::default()
            },
        ) else {
            return Err("wire bound ignored the finalized card size".to_string());
        };
        Ok(())
    }

    #[test]
    fn escapes_count_toward_the_inline_budget() {
        let mut escaped = minimal_card();
        escaped.changed_behavior = "\n".repeat(3000);
        assert!(matches!(
            apply_repair_card_budget(&mut escaped, &[], &RepairCardBudget::default()),
            Err(_message)
        ));
        let mut plain = minimal_card();
        plain.changed_behavior = "x".repeat(3000);
        assert!(matches!(
            apply_repair_card_budget(&mut plain, &[], &RepairCardBudget::default()),
            Ok(())
        ));
    }

    #[test]
    fn detail_vocabulary_round_trips_snake_case() -> Result<(), String> {
        let families = [
            ("fix_instruction", RepairCardDetailFamily::FixInstruction),
            (
                "witness_stage_evidence",
                RepairCardDetailFamily::WitnessStageEvidence,
            ),
            (
                "related_test_candidates",
                RepairCardDetailFamily::RelatedTestCandidates,
            ),
            (
                "limitation_detail",
                RepairCardDetailFamily::LimitationDetail,
            ),
            ("canonical_packet", RepairCardDetailFamily::CanonicalPacket),
            (
                "repair_attempt_status",
                RepairCardDetailFamily::RepairAttemptStatus,
            ),
            (
                "focused_proof_receipt",
                RepairCardDetailFamily::FocusedProofReceipt,
            ),
            ("static_movement", RepairCardDetailFamily::StaticMovement),
            (
                "mutation_calibration",
                RepairCardDetailFamily::MutationCalibration,
            ),
        ];
        for (expected, family) in families {
            let json = serde_json::to_string(&family).map_err(|error| error.to_string())?;
            if json != format!("\"{expected}\"") {
                return Err(format!("family serializes as {json}, expected {expected}"));
            }
            let round_tripped: RepairCardDetailFamily =
                serde_json::from_str(&json).map_err(|error| error.to_string())?;
            assert_eq!(family, round_tripped);
        }
        let states = [
            ("current", RepairCardDetailState::Current),
            ("stale", RepairCardDetailState::Stale),
            ("malformed", RepairCardDetailState::Malformed),
            ("wrong_root", RepairCardDetailState::WrongRoot),
            ("missing", RepairCardDetailState::Missing),
            ("unavailable", RepairCardDetailState::Unavailable),
        ];
        for (expected, state) in states {
            let json = serde_json::to_string(&state).map_err(|error| error.to_string())?;
            if json != format!("\"{expected}\"") {
                return Err(format!("state serializes as {json}, expected {expected}"));
            }
            let round_tripped: RepairCardDetailState =
                serde_json::from_str(&json).map_err(|error| error.to_string())?;
            assert_eq!(state, round_tripped);
        }
        let classes = [
            ("not_embeddable", RepairCardOmissionClass::NotEmbeddable),
            (
                "authority_owned_detail",
                RepairCardOmissionClass::AuthorityOwnedDetail,
            ),
            ("unavailable", RepairCardOmissionClass::Unavailable),
        ];
        for (expected, class) in classes {
            let json = serde_json::to_string(&class).map_err(|error| error.to_string())?;
            if json != format!("\"{expected}\"") {
                return Err(format!("class serializes as {json}, expected {expected}"));
            }
            let round_tripped: RepairCardOmissionClass =
                serde_json::from_str(&json).map_err(|error| error.to_string())?;
            assert_eq!(class, round_tripped);
        }
        Ok(())
    }
}
