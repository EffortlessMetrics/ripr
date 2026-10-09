//! Blind-journey execution consumer (#4604, RIPR-SPEC-0205).
//!
//! The named execution consumer of the RIPR-SPEC-0200 blind-journey contract
//! (#4603): a deterministic, offline executor that turns one scripted journey
//! (the reviewed operator-visible prompt, the separately stored answer key and
//! the ordered observable operator/evaluator actions) into one stamped,
//! validator-accepted `BlindJourneyPacketV1`.
//!
//! The executor owns three fail-closed duties the contract deliberately left
//! to this slice:
//!
//! 1. **Stamping**: every emitted packet passes through
//!    `stamp_blind_journey_packet`; the executor never hand-writes a digest,
//!    review binding or contamination result, and a stale review binding or
//!    divergent candidate identity refuses the run.
//! 2. **Per-kind digest presence**: the RIPR-SPEC-0200 receipt defers
//!    per-kind input/output digest presence to this execution slice. The
//!    executor requires `file_edit`, `project_verification_execution`,
//!    `static_analysis_execution` and `receipt_execution` events to carry both
//!    input and output bytes, and `product_command_invocation` events to
//!    carry input bytes; the executor digests the recorded bytes itself, so a
//!    script can never misrecord a digest. Observation bindings follow the
//!    same law: a declared verification exit without a verification
//!    execution, or a declared static/receipt movement without the matching
//!    execution, refuses the journey. Each execution kind also owns one
//!    canonical output record (`exit:<n>`, `static:<movement>`,
//!    `receipt:<state>`); the last execution of the kind governs the axis and
//!    a declared observation that disagrees with the recorded bytes refuses
//!    the journey, so no script can declare an exit the transcript did not
//!    record. A retained accepted review must likewise arrive with both digest
//!    bindings present; blank bindings would let stamping bind changed content
//!    to an old accepted verdict.
//! 3. **Derived terminals**: the receipt terminal is never claimed by the
//!    script. The executor derives it in one fixed precedence from the closed
//!    intervention taxonomy and the answer-key comparison, derives every
//!    evidence axis from the transcript, stamps the packet and then requires
//!    the live RIPR-SPEC-0200 validator to accept the derived receipt. A
//!    derived receipt the validator rejects — a contaminated prompt, a
//!    missing accepted review, an answer-key leak — refuses the run with the
//!    validator's own reasons.
//!
//! The executor launches no candidate, runs no process and decides no release
//! verdict; it makes the #4516/#4518/#4519 literal journeys executable as
//! scripts without pre-implementing them, while the real candidate-bound run
//! waits on the #4510 harness and the frozen #1609 candidate.

use serde::{Deserialize, Serialize};

use crate::blind_journey::{
    BLIND_JOURNEY_ANSWER_KEY_SCHEMA_VERSION, BLIND_JOURNEY_PROMPT_SCHEMA_VERSION,
    BLIND_JOURNEY_RECEIPT_SCHEMA_VERSION, BlindJourneyAnswerKeyV1, BlindJourneyAssessmentV1,
    BlindJourneyAssistanceStateV1, BlindJourneyCandidateRefV1, BlindJourneyContaminationResultV1,
    BlindJourneyCurrentnessV1, BlindJourneyEditCageVerdictV1, BlindJourneyEventKindV1,
    BlindJourneyEventV1, BlindJourneyEvidenceAxesV1, BlindJourneyInterventionV1,
    BlindJourneyPacketV1, BlindJourneyPromptReviewV1, BlindJourneyPromptReviewVerdictV1,
    BlindJourneyPromptV1, BlindJourneyReceiptStatusV1, BlindJourneyReceiptV1, BlindJourneyResultV1,
    BlindJourneySelectionCorrectnessV1, BlindJourneyStaticMovementV1,
    BlindJourneyVerificationStatusV1, assess_blind_journey_packet, sha256_hex,
    stamp_blind_journey_packet,
};

pub(crate) const BLIND_JOURNEY_JOURNEY_SCHEMA_VERSION: &str = "blind_journey_journey.v1";
pub(crate) const BLIND_JOURNEY_EXECUTE_CORPUS_SCHEMA_VERSION: &str =
    "blind_journey_execute_corpus.v1";
pub(crate) const BLIND_JOURNEY_EXECUTE_CLAIM_BOUNDARY: &str = "Static blind-journey execution \
 consumer: executor success defines that one scripted journey deterministically produces one \
 stamped, validator-accepted blind-journey receipt; it claims no installed usefulness, no \
 candidate qualification, no blind acceptance and no release verdict.";
pub(crate) const BLIND_JOURNEY_EXECUTE_DECISION: &str = "RIPR-SPEC-0205";

/// One scripted observable action. The script records what was observed
/// (subject and, where the per-kind presence rule requires it, the exact
/// input/output bytes); the executor assigns the sequence chain, digests the
/// bytes and classifies interventions.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlindJourneyActionV1 {
    pub kind: BlindJourneyEventKindV1,
    /// Exact subject: argv, path, doc id, option id or observation id.
    pub subject: String,
    /// Exact input bytes the executor digests; never a precomputed digest.
    pub input_bytes: Option<String>,
    /// Exact output bytes the executor digests; never a precomputed digest.
    pub output_bytes: Option<String>,
    pub operator_visible: bool,
    /// Closed intervention classification for every non-product action.
    pub intervention: Option<BlindJourneyInterventionV1>,
    /// Required when `intervention` is present.
    pub actor: Option<String>,
    /// Required for disqualifying interventions; must name the exact basis.
    pub reason: Option<String>,
}

/// Evaluator-side axis observations for one journey. An observation may never
/// claim more than the transcript enforces: the executor refuses a
/// verification exit without a verification execution and a declared static
/// or receipt movement without the matching execution.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlindJourneyObservedAxesV1 {
    /// Candidate currentness is an admission observation (#4510 in real
    /// runs); offline it is a declared observation the executor records.
    pub candidate_currentness: BlindJourneyCurrentnessV1,
    /// Observed project-verification exit; `None` means not executed.
    pub verification_exit: Option<i64>,
    /// Declared static movement; `unknown` without a static execution.
    pub static_movement: BlindJourneyStaticMovementV1,
    /// Declared receipt state; `unknown`/`receipt_not_applicable` without a
    /// receipt execution.
    pub receipt_status: BlindJourneyReceiptStatusV1,
    pub external_runtime_mutation_evidence: Option<String>,
}

/// One scripted blind journey: the reviewed prompt surface, the separately
/// stored answer key, the ordered actions and the axis observations. Digests
/// are always empty here; stamping binds them.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlindJourneyJourneyV1 {
    pub schema_version: String,
    pub candidate: BlindJourneyCandidateRefV1,
    pub operator_goal: String,
    pub public_inputs: Vec<String>,
    pub ordinary_permissions: Vec<String>,
    pub prohibited_hints: Vec<String>,
    pub prompt_bytes: String,
    /// The retained reviewer verdict; an accepted verdict must carry both
    /// digest bindings, and a present divergent binding refuses the run.
    pub review: BlindJourneyPromptReviewV1,
    pub answer_key: BlindJourneyAnswerKeyV1,
    pub actions: Vec<BlindJourneyActionV1>,
    pub observations: BlindJourneyObservedAxesV1,
    pub limitations: Vec<String>,
    pub non_claims: Vec<String>,
}

/// The committed executor expectation for one scenario.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlindJourneyExecuteOutcomeV1 {
    /// The executor emits one validator-accepted receipt.
    Accepted,
    /// The executor refuses the journey; `reason_contains` must appear in
    /// the refusal.
    Refused,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlindJourneyExecuteExpectationV1 {
    pub outcome: BlindJourneyExecuteOutcomeV1,
    pub terminal: Option<BlindJourneyResultV1>,
    pub reason_contains: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlindJourneyExecuteScenarioV1 {
    pub id: String,
    pub expected: BlindJourneyExecuteExpectationV1,
    pub journey: BlindJourneyJourneyV1,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlindJourneyExecuteCorpusV1 {
    pub schema_version: String,
    pub scenarios: Vec<BlindJourneyExecuteScenarioV1>,
}

/// The executor fixture scenarios RIPR-SPEC-0205 requires; the committed
/// corpus must cover all of them and the live executor decides each outcome
/// independently of the committed expectation.
pub(crate) const REQUIRED_BLIND_JOURNEY_EXECUTE_SCENARIO_IDS: [&str; 61] = [
    "executor_positive_journey_emits_receipt",
    "executor_positive_second_eligible_item_selects_b",
    "executor_instrument_watchdog_stays_positive",
    "executor_verification_failure_visible_emits_receipt",
    "executor_verification_not_run_visible_emits_receipt",
    "executor_honest_limitation_emits_receipt",
    "executor_discoverability_failure_emits_receipt",
    "executor_private_hint_records_hidden_assistance",
    "executor_manual_plumbing_records_own_terminal",
    "executor_workspace_binary_substitution_terminal",
    "executor_unsafe_action_records_unsafe_or_wrong_edit",
    "executor_quiet_neighbor_records_wrong_or_stale_subject",
    "executor_forbidden_edit_records_unsafe_or_wrong_edit",
    "executor_refuses_file_edit_without_output_digest",
    "executor_refuses_verification_without_input_digest",
    "executor_refuses_verification_exit_without_execution",
    "executor_refuses_contaminated_prompt",
    "executor_refuses_positive_without_accepted_review",
    "executor_refuses_stale_review_binding",
    "executor_refuses_honest_limitation_without_limitations",
    "executor_refuses_contradictory_verification_output",
    "executor_refuses_contradictory_static_output",
    "executor_refuses_receipt_not_applicable_with_execution",
    "executor_refuses_unbound_accepted_review",
    "installed_rust_positive_journey_emits_receipt",
    "installed_rust_no_ignore_rule_stops_before_edit",
    "installed_rust_notes_file_edit_refused",
    "installed_rust_production_edit_refused",
    "installed_rust_failing_focused_test_visible",
    "installed_rust_verification_skipped_visible",
    "installed_rust_deleted_before_artifact_names_recovery",
    "installed_rust_workspace_binary_rejected",
    "installed_rust_interrupted_receipt_write_recovers",
    "installed_rust_second_root_portable_identity",
    "installed_python_positive_journey_emits_receipt",
    "installed_python_bare_pytest_import_failure_visible",
    "installed_python_no_environment_stops_before_edit",
    "installed_python_production_edit_refused",
    "installed_python_receipt_path_mismatch_refused",
    "installed_python_failing_focused_test_visible",
    "installed_python_verification_skipped_visible",
    "installed_python_deleted_before_artifact_names_recovery",
    "installed_python_historical_bare_form_accepted_at_documented_strength",
    "installed_python_decoy_interpreter_rejected",
    "installed_python_interrupted_receipt_write_recovers",
    "installed_typescript_positive_journey_emits_receipt",
    "installed_typescript_imported_constant_variant_unresolved",
    "installed_typescript_let_binding_variant_unresolved",
    "installed_typescript_reassignment_variant_unresolved",
    "installed_typescript_shadowing_variant_unresolved",
    "installed_typescript_object_namespace_write_variant_unresolved",
    "installed_typescript_enum_computed_initializer_variant_unresolved",
    "installed_typescript_nonliteral_arithmetic_variant_unresolved",
    "installed_typescript_reach_barrier_variant_unresolved",
    "installed_typescript_failing_vitest_visible",
    "installed_typescript_verification_skipped_visible",
    "installed_typescript_cyclic_route_discoverability_failure",
    "installed_typescript_deleted_before_artifact_names_recovery",
    "installed_typescript_workspace_binary_rejected",
    "installed_typescript_interrupted_receipt_write_recovers",
    "installed_typescript_second_root_portable_identity",
];

/// Required scenario ids absent from one committed executor corpus id set.
pub(crate) fn missing_blind_journey_execute_required_scenarios<'a>(
    present: impl IntoIterator<Item = &'a str>,
) -> Vec<&'static str> {
    let present: std::collections::BTreeSet<&str> = present.into_iter().collect();
    REQUIRED_BLIND_JOURNEY_EXECUTE_SCENARIO_IDS
        .iter()
        .filter(|required| !present.contains(**required))
        .copied()
        .collect()
}

/// Load and parse one committed executor fixture corpus.
pub(crate) fn load_blind_journey_execute_corpus(
    body: &str,
) -> Result<BlindJourneyExecuteCorpusV1, String> {
    let corpus: BlindJourneyExecuteCorpusV1 = serde_json::from_str(body)
        .map_err(|error| format!("parse blind journey execute corpus: {error}"))?;
    if corpus.schema_version != BLIND_JOURNEY_EXECUTE_CORPUS_SCHEMA_VERSION {
        return Err(format!(
            "unsupported blind journey execute corpus schema `{}`",
            corpus.schema_version
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for scenario in &corpus.scenarios {
        if !ids.insert(scenario.id.clone()) {
            return Err(format!(
                "duplicate blind journey execute scenario id `{}`",
                scenario.id
            ));
        }
    }
    Ok(corpus)
}

fn digest_of(bytes: &Option<String>) -> Option<String> {
    bytes.as_ref().map(|value| sha256_hex(value.as_bytes()))
}

fn build_events(journey: &BlindJourneyJourneyV1) -> Result<Vec<BlindJourneyEventV1>, String> {
    let mut events = Vec::with_capacity(journey.actions.len());
    let mut previous_sequence: Option<u64> = None;
    for (index, action) in journey.actions.iter().enumerate() {
        let sequence = index as u64 + 1;
        if action.subject.trim().is_empty() {
            return Err(format!(
                "action_subject_missing:action {sequence} has no exact subject"
            ));
        }
        // Per-kind digest presence (deferred to this slice by the
        // RIPR-SPEC-0200 receipt): execution-bearing kinds must carry the
        // bytes the executor digests; a command invocation must carry its
        // argv input bytes.
        let (input_required, output_required) = match action.kind {
            BlindJourneyEventKindV1::FileEdit
            | BlindJourneyEventKindV1::ProjectVerificationExecution
            | BlindJourneyEventKindV1::StaticAnalysisExecution
            | BlindJourneyEventKindV1::ReceiptExecution => (true, true),
            BlindJourneyEventKindV1::ProductCommandInvocation => (true, false),
            BlindJourneyEventKindV1::PublicDocumentationLookup
            | BlindJourneyEventKindV1::ProductOutputFollowed
            | BlindJourneyEventKindV1::OrdinaryTargetSourceRead
            | BlindJourneyEventKindV1::OperatorQuestion
            | BlindJourneyEventKindV1::OperatorProductOptionSelection
            | BlindJourneyEventKindV1::ProcessCancellationRestartCleanup
            | BlindJourneyEventKindV1::HarnessIntervention => (false, false),
        };
        if input_required && action.input_bytes.is_none() {
            return Err(format!(
                "digest_presence_violation:{:?}:event {} requires input bytes",
                action.kind, sequence
            ));
        }
        if output_required && action.output_bytes.is_none() {
            return Err(format!(
                "digest_presence_violation:{:?}:event {} requires output bytes",
                action.kind, sequence
            ));
        }
        // Closed intervention classification at construction: a harness or
        // process-control action must carry one taxonomy class, every
        // classified action names its actor, every disqualifying action names
        // its exact basis, and instrument-only observation stays invisible.
        if matches!(
            action.kind,
            BlindJourneyEventKindV1::HarnessIntervention
                | BlindJourneyEventKindV1::ProcessCancellationRestartCleanup
        ) && action.intervention.is_none()
        {
            return Err(format!(
                "intervention_unclassified:non-product event {} has no closed classification",
                sequence
            ));
        }
        if let Some(intervention) = action.intervention {
            if intervention == BlindJourneyInterventionV1::InstrumentOnlyNotOperatorVisible
                && action.operator_visible
            {
                return Err(format!(
                    "instrument_visibility_violation:instrument-only event {} was operator visible",
                    sequence
                ));
            }
            if action.actor.as_deref().is_none_or(str::is_empty) {
                return Err(format!(
                    "intervention_actor_missing:event {} records no actor",
                    sequence
                ));
            }
            if intervention.disqualifying_result().is_some()
                && action.reason.as_deref().is_none_or(str::is_empty)
            {
                return Err(format!(
                    "intervention_reason_missing:non-product event {} records no exact reason",
                    sequence
                ));
            }
        }
        events.push(BlindJourneyEventV1 {
            sequence,
            predecessor_sequence: previous_sequence,
            kind: action.kind,
            subject: action.subject.clone(),
            input_digest: digest_of(&action.input_bytes),
            output_digest: digest_of(&action.output_bytes),
            operator_visible: action.operator_visible,
            intervention: action.intervention,
            actor: action.actor.clone(),
            reason: action.reason.clone(),
        });
        previous_sequence = Some(sequence);
    }
    Ok(events)
}

fn kind_count(events: &[BlindJourneyEventV1], kind: BlindJourneyEventKindV1) -> usize {
    events.iter().filter(|event| event.kind == kind).count()
}

fn last_subject(events: &[BlindJourneyEventV1], kind: BlindJourneyEventKindV1) -> Option<String> {
    events
        .iter()
        .rev()
        .find(|event| event.kind == kind)
        .map(|event| event.subject.clone())
}

/// The exact recorded output bytes of the last action of one kind; the script
/// records bytes, never digests, and the per-kind presence rule already
/// guaranteed the bytes exist for execution-bearing kinds.
fn last_recorded_output(
    journey: &BlindJourneyJourneyV1,
    kind: BlindJourneyEventKindV1,
) -> Option<&str> {
    journey
        .actions
        .iter()
        .rev()
        .find(|action| action.kind == kind)
        .and_then(|action| action.output_bytes.as_deref())
}

/// Canonical `static:<movement>` record of one static analysis execution.
fn parse_recorded_static_movement(bytes: &str) -> Option<BlindJourneyStaticMovementV1> {
    match bytes.strip_prefix("static:")?.trim() {
        "improved" => Some(BlindJourneyStaticMovementV1::Improved),
        "unchanged" => Some(BlindJourneyStaticMovementV1::Unchanged),
        "regressed" => Some(BlindJourneyStaticMovementV1::Regressed),
        _ => None,
    }
}

/// Canonical `receipt:<state>` record of one receipt execution.
fn parse_recorded_receipt_status(bytes: &str) -> Option<BlindJourneyReceiptStatusV1> {
    match bytes.strip_prefix("receipt:")?.trim() {
        "movement-improved" => Some(BlindJourneyReceiptStatusV1::ReceiptMovementImproved),
        "movement-unchanged" => Some(BlindJourneyReceiptStatusV1::ReceiptMovementUnchanged),
        "found" => Some(BlindJourneyReceiptStatusV1::ReceiptFound),
        "gap-mismatch" => Some(BlindJourneyReceiptStatusV1::ReceiptGapMismatch),
        "missing" => Some(BlindJourneyReceiptStatusV1::ReceiptMissing),
        "not-applicable" => Some(BlindJourneyReceiptStatusV1::ReceiptNotApplicable),
        _ => None,
    }
}

fn derive_axes(
    journey: &BlindJourneyJourneyV1,
    events: &[BlindJourneyEventV1],
) -> Result<BlindJourneyEvidenceAxesV1, String> {
    // A field must not claim more than the transcript enforces: the declared
    // observations bind to the recorded execution outputs, not only to the
    // presence of an execution event. Each execution kind owns one canonical
    // output record (`exit:<n>`, `static:<movement>`, `receipt:<state>`); the
    // last execution of the kind governs the axis, and a declared observation
    // that disagrees with the recorded bytes refuses the journey before any
    // receipt exists.
    let verification_executions = kind_count(
        events,
        BlindJourneyEventKindV1::ProjectVerificationExecution,
    );
    let project_verification_status = match (
        verification_executions,
        journey.observations.verification_exit,
    ) {
        (0, None) => BlindJourneyVerificationStatusV1::NotRun,
        (0, Some(_exit)) => {
            return Err(
                "observation_unbound:verification_exit is recorded without a project \
                 verification execution"
                    .to_string(),
            );
        }
        (count, None) => {
            return Err(format!(
                "observation_unbound:{count} project verification executions recorded no \
                 exit observation"
            ));
        }
        (_count, Some(declared)) => {
            let recorded = last_recorded_output(
                journey,
                BlindJourneyEventKindV1::ProjectVerificationExecution,
            )
            .and_then(|bytes| bytes.strip_prefix("exit:"))
            .and_then(|code| code.trim().parse::<i64>().ok());
            match recorded {
                None => {
                    return Err(
                        "observation_unbound:project verification output is not a canonical \
                         `exit:<n>` record"
                            .to_string(),
                    );
                }
                Some(recorded) if recorded != declared => {
                    return Err(format!(
                        "observation_unbound:verification_exit {declared} disagrees with the \
                         recorded exit {recorded}"
                    ));
                }
                Some(_) => {}
            }
            if declared == 0 {
                BlindJourneyVerificationStatusV1::Passed
            } else {
                BlindJourneyVerificationStatusV1::Failed
            }
        }
    };
    let static_executions = kind_count(events, BlindJourneyEventKindV1::StaticAnalysisExecution);
    if static_executions > 0 {
        let recorded =
            last_recorded_output(journey, BlindJourneyEventKindV1::StaticAnalysisExecution)
                .and_then(parse_recorded_static_movement);
        let recorded = match recorded {
            Some(recorded) => recorded,
            None => {
                return Err(
                    "observation_unbound:static analysis output is not a canonical \
                     `static:<movement>` record"
                        .to_string(),
                );
            }
        };
        if journey.observations.static_movement == BlindJourneyStaticMovementV1::Unknown {
            return Err(
                "observation_unbound:a static analysis execution recorded no movement observation"
                    .to_string(),
            );
        }
        if journey.observations.static_movement != recorded {
            return Err(format!(
                "observation_unbound:static_movement {:?} disagrees with the recorded \
                 static output",
                journey.observations.static_movement
            ));
        }
    } else if journey.observations.static_movement != BlindJourneyStaticMovementV1::Unknown {
        return Err(
            "observation_unbound:static_movement is recorded without a static analysis execution"
                .to_string(),
        );
    }
    let receipt_executions = kind_count(events, BlindJourneyEventKindV1::ReceiptExecution);
    if receipt_executions > 0 {
        let recorded = last_recorded_output(journey, BlindJourneyEventKindV1::ReceiptExecution)
            .and_then(parse_recorded_receipt_status);
        match recorded {
            None => {
                return Err(
                    "observation_unbound:receipt output is not a canonical `receipt:<state>` \
                     record"
                        .to_string(),
                );
            }
            Some(BlindJourneyReceiptStatusV1::ReceiptNotApplicable) => {
                return Err("observation_unbound:a receipt execution cannot record \
                     receipt_not_applicable; no applicable receipt observation exists"
                    .to_string());
            }
            Some(recorded) => {
                if journey.observations.receipt_status == BlindJourneyReceiptStatusV1::Unknown {
                    return Err(
                        "observation_unbound:a receipt execution recorded no receipt state \
                         observation"
                            .to_string(),
                    );
                }
                if journey.observations.receipt_status != recorded {
                    return Err(format!(
                        "observation_unbound:receipt_status {:?} disagrees with the recorded \
                         receipt output",
                        journey.observations.receipt_status
                    ));
                }
            }
        }
    } else {
        let receipt_observation_requires_execution = matches!(
            journey.observations.receipt_status,
            BlindJourneyReceiptStatusV1::ReceiptFound
                | BlindJourneyReceiptStatusV1::ReceiptGapMismatch
                | BlindJourneyReceiptStatusV1::ReceiptMissing
                | BlindJourneyReceiptStatusV1::ReceiptMovementImproved
                | BlindJourneyReceiptStatusV1::ReceiptMovementUnchanged
        );
        if receipt_observation_requires_execution {
            return Err(
                "observation_unbound:receipt_status is recorded without a receipt execution"
                    .to_string(),
            );
        }
    }
    let key = &journey.answer_key;
    let selection_correctness = match last_subject(
        events,
        BlindJourneyEventKindV1::OperatorProductOptionSelection,
    ) {
        None => BlindJourneySelectionCorrectnessV1::Unknown,
        Some(chosen) if key.quiet_neighbors.contains(&chosen) => {
            BlindJourneySelectionCorrectnessV1::QuietNeighborSelected
        }
        Some(chosen) if key.eligible_items.contains(&chosen) => {
            BlindJourneySelectionCorrectnessV1::CorrectActionable
        }
        Some(_chosen) => BlindJourneySelectionCorrectnessV1::NotActionable,
    };
    let edit_subjects: Vec<&str> = events
        .iter()
        .filter(|event| event.kind == BlindJourneyEventKindV1::FileEdit)
        .map(|event| event.subject.as_str())
        .collect();
    let edit_cage_verdict = if edit_subjects.is_empty() {
        BlindJourneyEditCageVerdictV1::Unknown
    } else if edit_subjects.iter().any(|subject| {
        key.forbidden_edits.iter().any(|path| path == subject)
            || !key.expected_edit_cage.iter().any(|path| path == subject)
    }) {
        BlindJourneyEditCageVerdictV1::Violation
    } else {
        BlindJourneyEditCageVerdictV1::WithinCage
    };
    let operator_assistance_state = if events.iter().any(|event| {
        event
            .intervention
            .and_then(BlindJourneyInterventionV1::disqualifying_result)
            .is_some()
    }) {
        BlindJourneyAssistanceStateV1::AssistanceObserved
    } else {
        BlindJourneyAssistanceStateV1::NoneObserved
    };
    Ok(BlindJourneyEvidenceAxesV1 {
        candidate_currentness: journey.observations.candidate_currentness,
        selection_correctness,
        edit_cage_verdict,
        project_verification_status,
        static_movement: journey.observations.static_movement,
        receipt_status: journey.observations.receipt_status,
        operator_assistance_state,
        external_runtime_mutation_evidence: journey
            .observations
            .external_runtime_mutation_evidence
            .clone(),
    })
}

fn derive_terminal(
    journey: &BlindJourneyJourneyV1,
    events: &[BlindJourneyEventV1],
    axes: &BlindJourneyEvidenceAxesV1,
) -> BlindJourneyResultV1 {
    // Fixed precedence; the terminal is derived, never claimed, so no script
    // can restate a machine result into a stronger state.
    // 1. Assistance disqualifiers in event order.
    for event in events {
        if let Some(intervention) = event.intervention
            && let Some(result) = intervention.disqualifying_result()
        {
            return result;
        }
    }
    // 2. Answer-key comparison: the selection, then the edit cage.
    if let Some(chosen) = last_subject(
        events,
        BlindJourneyEventKindV1::OperatorProductOptionSelection,
    ) && (journey.answer_key.quiet_neighbors.contains(&chosen)
        || !journey.answer_key.eligible_items.contains(&chosen))
    {
        return BlindJourneyResultV1::WrongOrStaleSubject;
    }
    if axes.edit_cage_verdict == BlindJourneyEditCageVerdictV1::Violation {
        return BlindJourneyResultV1::UnsafeOrWrongEdit;
    }
    if kind_count(
        events,
        BlindJourneyEventKindV1::OperatorProductOptionSelection,
    ) == 0
        && kind_count(events, BlindJourneyEventKindV1::OperatorQuestion) > 0
    {
        return BlindJourneyResultV1::ProductDiscoverabilityFailure;
    }
    // 3-4. Project verification stays its own axis.
    if axes.project_verification_status == BlindJourneyVerificationStatusV1::Failed {
        return BlindJourneyResultV1::VerificationFailureVisible;
    }
    if axes.project_verification_status == BlindJourneyVerificationStatusV1::NotRun {
        return BlindJourneyResultV1::VerificationNotRunVisible;
    }
    // 6. Any non-positive-grade axis keeps an honest, non-positive receipt.
    let all_axes_positive = axes.candidate_currentness == BlindJourneyCurrentnessV1::Current
        && axes.selection_correctness == BlindJourneySelectionCorrectnessV1::CorrectActionable
        && axes.edit_cage_verdict == BlindJourneyEditCageVerdictV1::WithinCage
        && axes.project_verification_status == BlindJourneyVerificationStatusV1::Passed
        && axes.static_movement == BlindJourneyStaticMovementV1::Improved
        && axes.receipt_status == BlindJourneyReceiptStatusV1::ReceiptMovementImproved
        && axes.operator_assistance_state == BlindJourneyAssistanceStateV1::NoneObserved;
    if !all_axes_positive {
        return BlindJourneyResultV1::HonestLimitation;
    }
    BlindJourneyResultV1::PassedBlindJourney
}

fn require_exact_limitations(
    journey: &BlindJourneyJourneyV1,
    terminal: BlindJourneyResultV1,
) -> Result<(), String> {
    let requires_limitations = matches!(
        terminal,
        BlindJourneyResultV1::HonestLimitation
            | BlindJourneyResultV1::ProductDiscoverabilityFailure
    );
    if !requires_limitations {
        return Ok(());
    }
    if journey.limitations.is_empty()
        || journey
            .limitations
            .iter()
            .any(|limitation| limitation.trim().is_empty())
    {
        return Err(
            "honest_terminal_without_exact_limitations:an honest terminal requires \
             exact limitations"
                .to_string(),
        );
    }
    if journey.non_claims.is_empty()
        || journey
            .non_claims
            .iter()
            .any(|non_claim| non_claim.trim().is_empty())
    {
        return Err(
            "honest_terminal_without_exact_limitations:an honest terminal requires exact non-claims"
                .to_string(),
        );
    }
    Ok(())
}

/// Execute one scripted journey into one stamped, validator-accepted packet.
/// Every refusal is a plain `Err(String)` naming the exact violated rule.
pub(crate) fn execute_blind_journey(
    journey: &BlindJourneyJourneyV1,
) -> Result<BlindJourneyPacketV1, String> {
    if journey.schema_version != BLIND_JOURNEY_JOURNEY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported_schema:journey:{}",
            journey.schema_version
        ));
    }
    if journey.answer_key.schema_version != BLIND_JOURNEY_ANSWER_KEY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported_schema:answer_key:{}",
            journey.answer_key.schema_version
        ));
    }
    // A retained accepted review describes a human judgment over exact prompt
    // and answer-key content. Blank digest bindings would let stamping bind
    // today's content to a previously accepted verdict, so an accepted review
    // must arrive with both bindings present; stamping then refuses any
    // binding that does not match the exact content.
    if journey.review.verdict == BlindJourneyPromptReviewVerdictV1::Accepted
        && (journey.review.prompt_digest.is_empty() || journey.review.answer_key_digest.is_empty())
    {
        return Err(
            "review_binding_missing:an accepted review must bind the exact reviewed prompt \
             and answer key digests"
                .to_string(),
        );
    }
    let events = build_events(journey)?;
    let axes = derive_axes(journey, &events)?;
    let terminal = derive_terminal(journey, &events, &axes);
    require_exact_limitations(journey, terminal)?;
    let prompt = BlindJourneyPromptV1 {
        schema_version: BLIND_JOURNEY_PROMPT_SCHEMA_VERSION.to_string(),
        candidate: journey.candidate.clone(),
        operator_goal: journey.operator_goal.clone(),
        public_inputs: journey.public_inputs.clone(),
        ordinary_permissions: journey.ordinary_permissions.clone(),
        prohibited_hints: journey.prohibited_hints.clone(),
        prompt_bytes: journey.prompt_bytes.clone(),
        prompt_digest: String::new(),
        review: BlindJourneyPromptReviewV1 {
            reviewer: journey.review.reviewer.clone(),
            verdict: journey.review.verdict,
            prompt_digest: journey.review.prompt_digest.clone(),
            answer_key_digest: journey.review.answer_key_digest.clone(),
        },
        contamination_result: BlindJourneyContaminationResultV1::MechanicallyClean,
    };
    let receipt = BlindJourneyReceiptV1 {
        schema_version: BLIND_JOURNEY_RECEIPT_SCHEMA_VERSION.to_string(),
        candidate: journey.candidate.clone(),
        prompt_digest: None,
        answer_key_digest: None,
        review: prompt.review.clone(),
        selected_item: last_subject(
            events.as_slice(),
            BlindJourneyEventKindV1::OperatorProductOptionSelection,
        ),
        selected_edit: last_subject(events.as_slice(), BlindJourneyEventKindV1::FileEdit),
        events,
        axes,
        terminal_result: terminal,
        limitations: journey.limitations.clone(),
        non_claims: journey.non_claims.clone(),
    };
    let packet = stamp_blind_journey_packet(prompt, journey.answer_key.clone(), receipt)?;
    let assessment: BlindJourneyAssessmentV1 = assess_blind_journey_packet(&packet);
    if !assessment.accepted {
        return Err(format!(
            "executor_refusal:the derived receipt failed the blind-journey contract: {:?}",
            assessment.rejection_reasons
        ));
    }
    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blind_journey::BLIND_JOURNEY_ANSWER_KEY_SCHEMA_VERSION as ANSWER_KEY_SCHEMA;

    fn candidate_ref() -> BlindJourneyCandidateRefV1 {
        BlindJourneyCandidateRefV1 {
            repository: "https://example.invalid/operator/target".to_string(),
            base: "base-commit-sha".to_string(),
            head: "head-commit-sha".to_string(),
            tree: "tree-object-sha".to_string(),
            root: "/srv/executor-journey".to_string(),
            package: "ripr".to_string(),
            binary: "ripr".to_string(),
            binary_digest: "abababababababababababababababababababababababababababababababab"
                .to_string(),
        }
    }

    fn clean_prompt_bytes() -> String {
        "You are given an installed analysis tool and a target repository. \
         Improve the tests for one behavior you can reach through the product. \
         You may read the public docs and the ordinary target source. Do not \
         ask anyone for the intended change."
            .to_string()
    }

    fn answer_key() -> BlindJourneyAnswerKeyV1 {
        BlindJourneyAnswerKeyV1 {
            schema_version: ANSWER_KEY_SCHEMA.to_string(),
            eligible_items: vec!["item-a".to_string(), "item-b".to_string()],
            quiet_neighbors: vec!["quiet-neighbor".to_string()],
            expected_edit_cage: vec!["src/lib.rs".to_string()],
            discriminator_family: "boundary-constant".to_string(),
            known_product_limitations: vec!["no-network".to_string()],
            forbidden_edits: vec!["src/main.rs".to_string()],
            answer_key_digest: String::new(),
        }
    }

    fn action(
        kind: BlindJourneyEventKindV1,
        subject: &str,
        input_bytes: Option<&str>,
        output_bytes: Option<&str>,
    ) -> BlindJourneyActionV1 {
        BlindJourneyActionV1 {
            kind,
            subject: subject.to_string(),
            input_bytes: input_bytes.map(str::to_string),
            output_bytes: output_bytes.map(str::to_string),
            operator_visible: true,
            intervention: None,
            actor: None,
            reason: None,
        }
    }

    fn positive_actions() -> Vec<BlindJourneyActionV1> {
        vec![
            action(
                BlindJourneyEventKindV1::PublicDocumentationLookup,
                "docs:cli-check",
                None,
                None,
            ),
            action(
                BlindJourneyEventKindV1::OrdinaryTargetSourceRead,
                "src/lib.rs",
                None,
                None,
            ),
            action(
                BlindJourneyEventKindV1::ProductCommandInvocation,
                "ripr check",
                Some("argv:ripr check"),
                None,
            ),
            action(
                BlindJourneyEventKindV1::OperatorProductOptionSelection,
                "item-a",
                None,
                None,
            ),
            action(
                BlindJourneyEventKindV1::FileEdit,
                "src/lib.rs",
                Some("fn lib() -> bool { false }\n"),
                Some("fn lib() -> bool { true }\n"),
            ),
            action(
                BlindJourneyEventKindV1::ProjectVerificationExecution,
                "project verification command",
                Some("argv:project verification"),
                Some("exit:0"),
            ),
            action(
                BlindJourneyEventKindV1::StaticAnalysisExecution,
                "ripr check",
                Some("argv:ripr check"),
                Some("static:improved"),
            ),
            action(
                BlindJourneyEventKindV1::ReceiptExecution,
                "ripr receipts",
                Some("argv:ripr receipts"),
                Some("receipt:movement-improved"),
            ),
        ]
    }

    fn positive_observations() -> BlindJourneyObservedAxesV1 {
        BlindJourneyObservedAxesV1 {
            candidate_currentness: BlindJourneyCurrentnessV1::Current,
            verification_exit: Some(0),
            static_movement: BlindJourneyStaticMovementV1::Improved,
            receipt_status: BlindJourneyReceiptStatusV1::ReceiptMovementImproved,
            external_runtime_mutation_evidence: None,
        }
    }

    fn journey_with(
        actions: Vec<BlindJourneyActionV1>,
        observations: BlindJourneyObservedAxesV1,
    ) -> BlindJourneyJourneyV1 {
        let key = answer_key();
        let mut journey = BlindJourneyJourneyV1 {
            schema_version: BLIND_JOURNEY_JOURNEY_SCHEMA_VERSION.to_string(),
            candidate: candidate_ref(),
            operator_goal: "improve one test through the installed product".to_string(),
            public_inputs: vec!["docs/cli.md".to_string()],
            ordinary_permissions: vec!["run public product entrypoints".to_string()],
            prohibited_hints: vec!["no intended change hints".to_string()],
            prompt_bytes: clean_prompt_bytes(),
            review: BlindJourneyPromptReviewV1 {
                reviewer: "reviewer-1".to_string(),
                verdict: crate::blind_journey::BlindJourneyPromptReviewVerdictV1::Accepted,
                prompt_digest: String::new(),
                answer_key_digest: String::new(),
            },
            answer_key: key,
            actions,
            observations,
            limitations: Vec::new(),
            non_claims: Vec::new(),
        };
        bind_review(&mut journey);
        journey
    }

    /// Bind the review digests to the exact journey content, as a retained
    /// accepted review must. The answer-key digest serialization of this
    /// struct cannot fail; a failure would leave the binding blank and every
    /// positive test below would refuse with `review_binding_missing`.
    fn bind_review(journey: &mut BlindJourneyJourneyV1) {
        let prompt = BlindJourneyPromptV1 {
            schema_version: BLIND_JOURNEY_PROMPT_SCHEMA_VERSION.to_string(),
            candidate: journey.candidate.clone(),
            operator_goal: journey.operator_goal.clone(),
            public_inputs: journey.public_inputs.clone(),
            ordinary_permissions: journey.ordinary_permissions.clone(),
            prohibited_hints: journey.prohibited_hints.clone(),
            prompt_bytes: journey.prompt_bytes.clone(),
            prompt_digest: String::new(),
            review: BlindJourneyPromptReviewV1 {
                reviewer: journey.review.reviewer.clone(),
                verdict: journey.review.verdict,
                prompt_digest: String::new(),
                answer_key_digest: String::new(),
            },
            contamination_result: BlindJourneyContaminationResultV1::MechanicallyClean,
        };
        journey.review.prompt_digest = crate::blind_journey::prompt_digest(&prompt);
        if let Ok(digest) =
            crate::blind_journey::blind_journey_answer_key_digest(&journey.answer_key)
        {
            journey.review.answer_key_digest = digest;
        }
    }

    fn positive_journey() -> BlindJourneyJourneyV1 {
        journey_with(positive_actions(), positive_observations())
    }

    fn refused_reason(
        result: Result<BlindJourneyPacketV1, String>,
        needle: &str,
    ) -> Result<(), String> {
        match result {
            Err(message) if message.contains(needle) => Ok(()),
            Err(message) => Err(format!(
                "expected a refusal containing `{needle}`, got: {message}"
            )),
            Ok(_packet) => Err(format!(
                "expected a refusal containing `{needle}`, got an emitted packet"
            )),
        }
    }

    #[test]
    fn positive_journey_emits_a_stamped_validated_receipt() -> Result<(), String> {
        let journey = positive_journey();
        let packet = execute_blind_journey(&journey)?;
        let assessment = assess_blind_journey_packet(&packet);
        if !assessment.positive() {
            return Err(format!(
                "the emitted receipt must validate positive, got {:?}",
                assessment.rejection_reasons
            ));
        }
        if packet.receipt.terminal_result != BlindJourneyResultV1::PassedBlindJourney {
            return Err("the derived terminal must be passed_blind_journey".to_string());
        }
        if packet.receipt.events.iter().any(|event| {
            event.input_digest.is_none() && matches!(event.kind, BlindJourneyEventKindV1::FileEdit)
        }) {
            return Err("the executor must digest the recorded edit input bytes".to_string());
        }
        Ok(())
    }

    #[test]
    fn file_edit_without_output_bytes_refuses_before_any_receipt() -> Result<(), String> {
        let mut journey = positive_journey();
        for step in &mut journey.actions {
            if step.kind == BlindJourneyEventKindV1::FileEdit {
                step.output_bytes = None;
            }
        }
        refused_reason(
            execute_blind_journey(&journey),
            "digest_presence_violation:FileEdit",
        )
    }

    #[test]
    fn verification_execution_without_input_bytes_refuses() -> Result<(), String> {
        let mut journey = positive_journey();
        for step in &mut journey.actions {
            if step.kind == BlindJourneyEventKindV1::ProjectVerificationExecution {
                step.input_bytes = None;
            }
        }
        refused_reason(
            execute_blind_journey(&journey),
            "digest_presence_violation:ProjectVerificationExecution",
        )
    }

    #[test]
    fn command_invocation_without_argv_input_bytes_refuses() -> Result<(), String> {
        let mut journey = positive_journey();
        for step in &mut journey.actions {
            if step.kind == BlindJourneyEventKindV1::ProductCommandInvocation {
                step.input_bytes = None;
            }
        }
        refused_reason(
            execute_blind_journey(&journey),
            "digest_presence_violation:ProductCommandInvocation",
        )
    }

    #[test]
    fn verification_exit_without_an_execution_refuses() -> Result<(), String> {
        let mut journey = positive_journey();
        journey
            .actions
            .retain(|step| step.kind != BlindJourneyEventKindV1::ProjectVerificationExecution);
        refused_reason(execute_blind_journey(&journey), "observation_unbound")
    }

    #[test]
    fn declared_static_movement_without_an_execution_refuses() -> Result<(), String> {
        let mut journey = positive_journey();
        journey
            .actions
            .retain(|step| step.kind != BlindJourneyEventKindV1::StaticAnalysisExecution);
        refused_reason(execute_blind_journey(&journey), "observation_unbound")
    }

    #[test]
    fn hidden_hint_derives_hidden_assistance_not_a_positive_row() -> Result<(), String> {
        let mut journey = positive_journey();
        journey.actions.push(BlindJourneyActionV1 {
            kind: BlindJourneyEventKindV1::HarnessIntervention,
            subject: "evaluator supplied the intended edit after the failure".to_string(),
            input_bytes: None,
            output_bytes: None,
            operator_visible: false,
            intervention: Some(BlindJourneyInterventionV1::PrivateHarnessHint),
            actor: Some("evaluator".to_string()),
            reason: Some("named the intended edit after the printed command failed".to_string()),
        });
        let packet = execute_blind_journey(&journey)?;
        if packet.receipt.terminal_result != BlindJourneyResultV1::HiddenOperatorAssistance {
            return Err(format!(
                "a private hint must derive hidden_operator_assistance, got {:?}",
                packet.receipt.terminal_result
            ));
        }
        if packet.receipt.axes.operator_assistance_state
            != BlindJourneyAssistanceStateV1::AssistanceObserved
        {
            return Err("the assistance axis must record assistance_observed".to_string());
        }
        Ok(())
    }

    #[test]
    fn passing_axes_cannot_hide_a_hint() -> Result<(), String> {
        // The axes pass everywhere; the hint still owns the terminal.
        let mut journey = positive_journey();
        journey.actions.push(BlindJourneyActionV1 {
            kind: BlindJourneyEventKindV1::HarnessIntervention,
            subject: "evaluator supplied the intended edit".to_string(),
            input_bytes: None,
            output_bytes: None,
            operator_visible: false,
            intervention: Some(BlindJourneyInterventionV1::HiddenOperatorKnowledge),
            actor: Some("evaluator".to_string()),
            reason: Some("the operator already knew the intended change".to_string()),
        });
        let packet = execute_blind_journey(&journey)
            .map_err(|error| format!("the derived receipt must stay accepted: {error}"))?;
        if packet.receipt.terminal_result != BlindJourneyResultV1::HiddenOperatorAssistance {
            return Err(
                "passing verification and static axes must not hide hidden operator knowledge"
                    .to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn quiet_neighbor_selection_derives_wrong_or_stale_subject() -> Result<(), String> {
        let mut journey = positive_journey();
        for step in &mut journey.actions {
            if step.kind == BlindJourneyEventKindV1::OperatorProductOptionSelection {
                step.subject = "quiet-neighbor".to_string();
            }
        }
        let packet = execute_blind_journey(&journey)?;
        if packet.receipt.terminal_result != BlindJourneyResultV1::WrongOrStaleSubject {
            return Err(format!(
                "a quiet-neighbor selection must derive wrong_or_stale_subject, got {:?}",
                packet.receipt.terminal_result
            ));
        }
        if packet.receipt.axes.selection_correctness
            != BlindJourneySelectionCorrectnessV1::QuietNeighborSelected
        {
            return Err("the selection axis must record quiet_neighbor_selected".to_string());
        }
        Ok(())
    }

    #[test]
    fn forbidden_edit_derives_unsafe_or_wrong_edit() -> Result<(), String> {
        let mut journey = positive_journey();
        for step in &mut journey.actions {
            if step.kind == BlindJourneyEventKindV1::FileEdit {
                step.subject = "src/main.rs".to_string();
            }
        }
        let packet = execute_blind_journey(&journey)?;
        if packet.receipt.terminal_result != BlindJourneyResultV1::UnsafeOrWrongEdit {
            return Err(format!(
                "a forbidden edit must derive unsafe_or_wrong_edit, got {:?}",
                packet.receipt.terminal_result
            ));
        }
        if packet.receipt.axes.edit_cage_verdict != BlindJourneyEditCageVerdictV1::Violation {
            return Err("the edit-cage axis must record violation".to_string());
        }
        Ok(())
    }

    #[test]
    fn failed_verification_derives_verification_failure_visible() -> Result<(), String> {
        let mut journey = positive_journey();
        journey.observations.verification_exit = Some(17);
        for step in &mut journey.actions {
            if step.kind == BlindJourneyEventKindV1::ProjectVerificationExecution {
                step.output_bytes = Some("exit:17".to_string());
            }
        }
        let packet = execute_blind_journey(&journey)?;
        if packet.receipt.terminal_result != BlindJourneyResultV1::VerificationFailureVisible {
            return Err(format!(
                "a recorded failing exit must derive verification_failure_visible, got {:?}",
                packet.receipt.terminal_result
            ));
        }
        if packet.receipt.axes.project_verification_status
            != BlindJourneyVerificationStatusV1::Failed
        {
            return Err("the verification axis must record failed".to_string());
        }
        Ok(())
    }

    #[test]
    fn skipped_verification_derives_not_run_visible() -> Result<(), String> {
        let mut journey = positive_journey();
        journey
            .actions
            .retain(|step| step.kind != BlindJourneyEventKindV1::ProjectVerificationExecution);
        journey.observations.verification_exit = None;
        journey.observations.static_movement = BlindJourneyStaticMovementV1::Unknown;
        journey
            .actions
            .retain(|step| step.kind != BlindJourneyEventKindV1::StaticAnalysisExecution);
        journey.observations.receipt_status = BlindJourneyReceiptStatusV1::Unknown;
        journey
            .actions
            .retain(|step| step.kind != BlindJourneyEventKindV1::ReceiptExecution);
        let packet = execute_blind_journey(&journey)?;
        if packet.receipt.terminal_result != BlindJourneyResultV1::VerificationNotRunVisible {
            return Err(format!(
                "an absent verification must derive verification_not_run_visible, got {:?}",
                packet.receipt.terminal_result
            ));
        }
        Ok(())
    }

    #[test]
    fn honest_limitation_requires_exact_limitations() -> Result<(), String> {
        let mut journey = positive_journey();
        journey.observations.static_movement = BlindJourneyStaticMovementV1::Unchanged;
        for step in &mut journey.actions {
            if step.kind == BlindJourneyEventKindV1::StaticAnalysisExecution {
                step.output_bytes = Some("static:unchanged".to_string());
            }
        }
        refused_reason(
            execute_blind_journey(&journey),
            "honest_terminal_without_exact_limitations",
        )
    }

    #[test]
    fn honest_limitation_with_limitations_emits_a_receipt() -> Result<(), String> {
        let mut journey = positive_journey();
        journey.observations.static_movement = BlindJourneyStaticMovementV1::Unchanged;
        for step in &mut journey.actions {
            if step.kind == BlindJourneyEventKindV1::StaticAnalysisExecution {
                step.output_bytes = Some("static:unchanged".to_string());
            }
        }
        journey.limitations = vec![
            "the installed product exposed no static movement for the selected \
             repair on this repository"
                .to_string(),
        ];
        journey.non_claims = vec![
            "no completed repair is claimed; recovery requires an actionable static route"
                .to_string(),
        ];
        let packet = execute_blind_journey(&journey)?;
        if packet.receipt.terminal_result != BlindJourneyResultV1::HonestLimitation {
            return Err(format!(
                "unchanged static movement must derive honest_limitation, got {:?}",
                packet.receipt.terminal_result
            ));
        }
        let assessment = assess_blind_journey_packet(&packet);
        if !assessment.accepted {
            return Err(format!(
                "the honest receipt must stay accepted: {:?}",
                assessment.rejection_reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn contaminated_prompt_refuses_with_the_validator_reason() -> Result<(), String> {
        let mut journey = positive_journey();
        journey.prompt_bytes = "Improve the tests for gap-042 as intended.".to_string();
        // The review binds the exact reviewed content, including the
        // contaminated bytes; the contamination scan, not the binding, refuses.
        bind_review(&mut journey);
        refused_reason(execute_blind_journey(&journey), "mechanically_contaminated")
    }

    #[test]
    fn positive_without_an_accepted_review_refuses() -> Result<(), String> {
        let mut journey = positive_journey();
        journey.review.verdict =
            crate::blind_journey::BlindJourneyPromptReviewVerdictV1::NotReviewed;
        refused_reason(
            execute_blind_journey(&journey),
            "positive_without_accepted_review",
        )
    }

    #[test]
    fn stale_review_binding_refuses_at_stamping() -> Result<(), String> {
        let mut journey = positive_journey();
        journey.review.prompt_digest = "0".repeat(64);
        refused_reason(
            execute_blind_journey(&journey),
            "prompt review binds different prompt bytes",
        )
    }

    #[test]
    fn unclassified_harness_action_refuses_before_any_receipt() -> Result<(), String> {
        let mut journey = positive_journey();
        journey.actions.push(BlindJourneyActionV1 {
            kind: BlindJourneyEventKindV1::HarnessIntervention,
            subject: "evaluator did something".to_string(),
            input_bytes: None,
            output_bytes: None,
            operator_visible: false,
            intervention: None,
            actor: None,
            reason: None,
        });
        refused_reason(execute_blind_journey(&journey), "intervention_unclassified")
    }

    #[test]
    fn instrument_only_watchdog_stays_positive() -> Result<(), String> {
        let mut journey = positive_journey();
        journey.actions.push(BlindJourneyActionV1 {
            kind: BlindJourneyEventKindV1::ProcessCancellationRestartCleanup,
            subject: "watchdog observed process bounds".to_string(),
            input_bytes: None,
            output_bytes: None,
            operator_visible: false,
            intervention: Some(BlindJourneyInterventionV1::InstrumentOnlyNotOperatorVisible),
            actor: Some("harness".to_string()),
            reason: Some("watchdog inventory only; no operator path change".to_string()),
        });
        let packet = execute_blind_journey(&journey)?;
        let assessment = assess_blind_journey_packet(&packet);
        if !assessment.positive() {
            return Err(format!(
                "an instrument-only watchdog must not change a positive run: {:?}",
                assessment.rejection_reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn manual_plumbing_derives_its_own_terminal() -> Result<(), String> {
        let mut journey = positive_journey();
        journey.actions.push(BlindJourneyActionV1 {
            kind: BlindJourneyEventKindV1::HarnessIntervention,
            subject: "evaluator placed a missing artifact".to_string(),
            input_bytes: None,
            output_bytes: None,
            operator_visible: false,
            intervention: Some(BlindJourneyInterventionV1::ManualArtifactPlumbing),
            actor: Some("evaluator".to_string()),
            reason: Some("placed an artifact the product did not emit".to_string()),
        });
        let packet = execute_blind_journey(&journey)?;
        if packet.receipt.terminal_result != BlindJourneyResultV1::ManualArtifactPlumbing {
            return Err(format!(
                "manual plumbing must derive manual_artifact_plumbing, got {:?}",
                packet.receipt.terminal_result
            ));
        }
        Ok(())
    }

    #[test]
    fn derived_terminal_is_never_script_claimable() -> Result<(), String> {
        // No field on the journey carries a terminal: the only way to assert
        // one is the emitted receipt, so strengthening would require forging
        // the validator, not restating a field.
        let journey = positive_journey();
        let packet = execute_blind_journey(&journey)?;
        let assessment = assess_blind_journey_packet(&packet);
        if !assessment.positive() {
            return Err(format!(
                "the executor loop must emit only validator-accepted receipts: {:?}",
                assessment.rejection_reasons
            ));
        }
        Ok(())
    }

    #[test]
    fn static_and_receipt_executions_require_both_digests() -> Result<(), String> {
        for kind in [
            BlindJourneyEventKindV1::StaticAnalysisExecution,
            BlindJourneyEventKindV1::ReceiptExecution,
        ] {
            for drop_output in [false, true] {
                let mut journey = positive_journey();
                for step in &mut journey.actions {
                    if step.kind == kind {
                        if drop_output {
                            step.output_bytes = None;
                        } else {
                            step.input_bytes = None;
                        }
                    }
                }
                let needle = format!("digest_presence_violation:{kind:?}");
                refused_reason(execute_blind_journey(&journey), &needle)?;
            }
        }
        Ok(())
    }

    #[test]
    fn contradictory_verification_output_refuses() -> Result<(), String> {
        // exit:1 was recorded; declaring exit 0 must not derive a passed axis.
        let mut journey = positive_journey();
        for step in &mut journey.actions {
            if step.kind == BlindJourneyEventKindV1::ProjectVerificationExecution {
                step.output_bytes = Some("exit:1".to_string());
            }
        }
        refused_reason(
            execute_blind_journey(&journey),
            "observation_unbound:verification_exit 0 disagrees",
        )
    }

    #[test]
    fn non_canonical_verification_output_refuses() -> Result<(), String> {
        let mut journey = positive_journey();
        for step in &mut journey.actions {
            if step.kind == BlindJourneyEventKindV1::ProjectVerificationExecution {
                step.output_bytes = Some("all tests passed".to_string());
            }
        }
        refused_reason(execute_blind_journey(&journey), "observation_unbound")
    }

    #[test]
    fn contradictory_static_output_refuses() -> Result<(), String> {
        let mut journey = positive_journey();
        for step in &mut journey.actions {
            if step.kind == BlindJourneyEventKindV1::StaticAnalysisExecution {
                step.output_bytes = Some("static:unchanged".to_string());
            }
        }
        refused_reason(execute_blind_journey(&journey), "observation_unbound")
    }

    #[test]
    fn contradictory_receipt_output_refuses() -> Result<(), String> {
        let mut journey = positive_journey();
        for step in &mut journey.actions {
            if step.kind == BlindJourneyEventKindV1::ReceiptExecution {
                step.output_bytes = Some("receipt:missing".to_string());
            }
        }
        refused_reason(execute_blind_journey(&journey), "observation_unbound")
    }

    #[test]
    fn receipt_not_applicable_with_an_execution_refuses() -> Result<(), String> {
        let mut journey = positive_journey();
        journey.observations.receipt_status = BlindJourneyReceiptStatusV1::ReceiptNotApplicable;
        for step in &mut journey.actions {
            if step.kind == BlindJourneyEventKindV1::ReceiptExecution {
                step.output_bytes = Some("receipt:not-applicable".to_string());
            }
        }
        refused_reason(execute_blind_journey(&journey), "observation_unbound")
    }

    #[test]
    fn unbound_accepted_review_refuses() -> Result<(), String> {
        let mut journey = positive_journey();
        journey.review.prompt_digest = String::new();
        journey.review.answer_key_digest = String::new();
        refused_reason(execute_blind_journey(&journey), "review_binding_missing")
    }

    #[test]
    fn changed_answer_key_with_a_retained_binding_refuses() -> Result<(), String> {
        // The review bound the old answer key; changing the key afterwards
        // must refuse at stamping instead of re-binding the old verdict.
        let mut journey = positive_journey();
        journey.answer_key.eligible_items.push("item-c".to_string());
        refused_reason(
            execute_blind_journey(&journey),
            "prompt review binds a different answer key",
        )
    }
}
