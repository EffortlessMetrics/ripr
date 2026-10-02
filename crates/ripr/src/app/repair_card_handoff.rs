//! Production producer for the compact repair-card handoff (RIPR-SPEC-0194,
//! #4667): `ripr agent card` assembles the same typed authorities the packet
//! and editor surfaces consume — seam inventory, repair-packet eligibility,
//! the check finding set and its witness, the recommended-test/edit-cage
//! derivation, the repair-attempt inventory, and the typed command catalog —
//! into one bounded [`RepairCardV1`]. Nothing is re-derived and no
//! classification vocabulary is invented here.
//!
//! The complete canonical packet stays behind the card's explicit
//! `canonical_packet` detail route (`ripr agent packet --seam-id ... --json`);
//! the packet render is unchanged and remains the compatibility path.

use std::path::Path;

use crate::agent::artifact::git_output;
use crate::agent::command_specs::{AgentArtifactRoute, agent_inspection_command_spec};
use crate::analysis::ClassifiedSeam;
use crate::analysis::repair_route::{
    RepairRouteReadiness, RepairTargetSelection, repair_packet_eligibility,
};
use crate::config::RiprConfig;
use crate::domain::{
    CardCurrentnessGoal, CommandSpec, DiagnosticWitness, EditCageGoal, FindingCanonicalGap,
    FixInstructionSummary, FocusedExecutionGoal, MutationConfirmationGoal, RepairCardBudget,
    RepairCardDetailFamily, RepairCardDetailState, RepairCardDoneWhen, RepairCardSnapshot,
    RepairCardSnapshotCurrentness, RepairCardSubject, RepairCardV1, StaticMovementGoal,
    repair_card_route_exposable,
};
use crate::output::agent_seam_packets::{
    EDIT_CAGE_PRODUCTION_STATEMENT, EDIT_CAGE_TERMINALITY_WARNING, PacketCommandContext,
    TASK_WRITE_TARGETED_TEST, recommended_test_for, render_agent_seam_packet_json_with_context,
    task_for,
};
use crate::output::path::display_path;
use crate::repair_card_budget::RepairCardDetailSource;

use super::repair_attempt::{
    RepairAttemptInventoryEntry, RepairAttemptManifest, inventory_repair_attempts,
};
use super::repair_card::{RepairCardInput, build_repair_card};
use super::{CheckInput, check_workspace_with_config};

/// Producer-gathered facts for one seam's repair card. The pure assembly
/// [`assemble_repair_card`] takes these directly so the projection contract is
/// unit-testable without git or analysis IO; [`repair_card_for_entry`] gathers
/// them from the live authorities.
pub(crate) struct SeamCardFacts<'a> {
    pub(crate) entry: &'a ClassifiedSeam,
    /// The witness of the check finding that names this seam's canonical gap,
    /// when one exists in the current analysis.
    pub(crate) witness: Option<&'a DiagnosticWitness>,
    /// The finding ID the witness was projected from (the `ripr explain`
    /// route names it).
    pub(crate) finding_id: Option<&'a str>,
    /// The latest recorded repair attempt for this seam, when one exists.
    pub(crate) attempt: Option<&'a RepairAttemptManifest>,
    /// The rendered complete canonical packet envelope this card routes to.
    pub(crate) packet_json: &'a str,
    pub(crate) repository_head: &'a str,
    pub(crate) workspace_identity: &'a str,
    /// Producer-observed working-tree currentness for the files this card's
    /// evidence binds: `Current` on a clean scope, `AcceptedDirtyDraft` when
    /// an uncommitted change touches them. Both currentness axes project this
    /// fact verbatim; the assembly never mints an unchecked `current` claim.
    pub(crate) currentness: RepairCardSnapshotCurrentness,
    /// The typed inspection route (`ripr agent packet ... --json`) offered as
    /// the card's one next action when the route gate is open.
    pub(crate) next_command: Option<CommandSpec>,
}

/// Gather the live authorities for one classified seam and assemble its card.
pub(crate) fn repair_card_for_entry(
    entry: &ClassifiedSeam,
    root: &Path,
    config: &RiprConfig,
) -> Result<RepairCardV1, String> {
    let eligibility = repair_packet_eligibility(entry);
    let witness_pack = witness_for_seam(
        root,
        config,
        entry,
        eligibility.readiness.canonical_gap_id.as_deref(),
    )?;
    let (finding_id, witness) = match witness_pack {
        Some((id, witness)) => (Some(id), Some(witness)),
        None => (None, None),
    };
    let repository_head = git_output(root, &["rev-parse", "HEAD"])
        .map_err(|error| format!("agent card could not resolve the repository head: {error}"))?
        .trim()
        .to_string();
    let workspace_identity = workspace_identity_for(entry, &eligibility.readiness)?;
    let seam_id = entry.seam.id().as_str().to_string();
    let attempt = latest_attempt_for_seam(root, &seam_id)?;
    let currentness = evidence_tree_currentness(root, entry)?;
    let packet_json = card_packet_json(entry);
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
        currentness,
        next_command: Some(next_command),
    })
}

/// Render the canonical-packet envelope whose bytes feed the card's
/// `canonical_packet` content digest. The content-bound render always uses
/// the portable root (`.`), never the bound checkout spelling: the packet's
/// `next`-block command strings embed that root, and `detail_digest` /
/// `complete_evidence_digest` are load-bearing identity — equivalent roots
/// must hash identically (RIPR-SPEC-0192 authority 4; the #3166
/// equivalent-roots acceptance). The human-facing packet command keeps
/// binding the concrete root (#4000's display contract is untouched:
/// display is presentation and never enters identity).
pub(crate) fn card_packet_json(entry: &ClassifiedSeam) -> String {
    render_agent_seam_packet_json_with_context(
        entry,
        PacketCommandContext::Standalone { root: "." },
    )
}

/// One bounded dirty-state probe over the files this card's evidence binds:
/// the seam file (the forbidden edit surface), every related-test file (the
/// related-test-candidates evidence family binds them all), and, when the
/// packet task is the targeted-test task, the recommended test file (the
/// allowed edit surface). Any uncommitted change in that scope — tracked
/// modification, untracked draft, or ignored edit, all of which the analysis
/// reads from working-tree bytes while `snapshot.repository_head` names the
/// committed head — projects `AcceptedDirtyDraft`. A clean scope projects
/// `Current`. A probe failure fails closed: the gather errors, no card is
/// minted, and no unchecked `current` claim escapes.
pub(crate) fn evidence_tree_currentness(
    root: &Path,
    entry: &ClassifiedSeam,
) -> Result<RepairCardSnapshotCurrentness, String> {
    let paths = evidence_probe_paths(entry);
    let mut args: Vec<String> = vec![
        // Evidence filenames are literal paths, never patterns: a seam file
        // named `foo*bar.rs` must not glob-match unrelated files.
        "--literal-pathspecs".to_string(),
        "status".to_string(),
        "--porcelain".to_string(),
        "--ignored".to_string(),
        "--".to_string(),
    ];
    for path in &paths {
        args.push(path.to_string_lossy().into_owned());
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = git_output(root, &arg_refs)
        .map_err(|error| format!("agent card could not probe the working-tree state: {error}"))?;
    if output.trim().is_empty() {
        Ok(RepairCardSnapshotCurrentness::Current)
    } else {
        Ok(RepairCardSnapshotCurrentness::AcceptedDirtyDraft)
    }
}

/// The probe pathspecs in their filesystem spelling. `display_path` is a
/// presentation renderer (it percent-encodes `%` and rewrites separators);
/// feeding a display spelling to git would probe a path that does not exist
/// and silently read a dirty scope as clean.
fn evidence_probe_paths(entry: &ClassifiedSeam) -> Vec<std::path::PathBuf> {
    let mut paths: Vec<std::path::PathBuf> = vec![entry.seam.file().to_path_buf()];
    for related in &entry.evidence.related_tests {
        if !paths.contains(&related.file) {
            paths.push(related.file.clone());
        }
    }
    if task_for(entry) == TASK_WRITE_TARGETED_TEST {
        let recommended = recommended_test_for(entry);
        if recommended.file != "not_applicable" {
            let recommended_path = std::path::PathBuf::from(&recommended.file);
            if !paths.contains(&recommended_path) {
                paths.push(recommended_path);
            }
        }
    }
    paths
}

/// Assemble one [`RepairCardV1`] from producer-owned facts. Pure projection:
/// every field names the authority it was copied from, the route gate stays
/// fail-closed, and the complete packet rides behind its detail reference.
pub(crate) fn assemble_repair_card(facts: &SeamCardFacts<'_>) -> Result<RepairCardV1, String> {
    let entry = facts.entry;
    let eligibility = repair_packet_eligibility(entry);
    let readiness = &eligibility.readiness;
    let instruction = facts
        .witness
        .map(FixInstructionSummary::from_witness)
        .unwrap_or_else(FixInstructionSummary::unavailable);

    // Edit cage: the exact derivation the seam packet states (#4330), so the
    // card cannot promise a different edit surface than the attempt enforces.
    let actionable = task_for(entry) == TASK_WRITE_TARGETED_TEST;
    let recommended = recommended_test_for(entry);
    // Admission is a ceiling on a statically ready route. A seam already
    // blocked by missing/static evidence has no edit to admit; retain that
    // more relevant diagnosis instead of replacing it with a cage error.
    let edit_cage_refusal = if readiness.is_repair_ready() {
        super::repair_attempt::edit_cage_policy_from_packet(
            facts.packet_json,
            entry.seam.id().as_str(),
        )
        .err()
    } else {
        None
    };
    let production_file = display_path(entry.seam.file());
    let allowed_files: Vec<String> = if actionable && recommended.file != "not_applicable" {
        vec![recommended.file.clone()]
    } else {
        Vec::new()
    };
    let forbidden_files: Vec<String> = if allowed_files
        .first()
        .is_some_and(|allowed| allowed == &production_file)
    {
        Vec::new()
    } else {
        vec![production_file]
    };

    let done_when = RepairCardDoneWhen {
        static_movement: StaticMovementGoal::ClosedBySelectedRoute,
        focused_test_execution: if actionable {
            FocusedExecutionGoal::VerifiedPass
        } else {
            FocusedExecutionGoal::ExplicitlyNotRun
        },
        edit_cage: EditCageGoal::Compliant,
        mutation_confirmation: MutationConfirmationGoal::NotRequested,
        // Both currentness axes name the one producer-observed tree fact; a
        // dirty evidence scope stays an accepted dirty draft, never a silent
        // `current` (#5008).
        currentness: match facts.currentness {
            RepairCardSnapshotCurrentness::Current => CardCurrentnessGoal::Current,
            RepairCardSnapshotCurrentness::AcceptedDirtyDraft => {
                CardCurrentnessGoal::AcceptedDirtyDraft
            }
        },
    };
    let mut stop_conditions = vec![EDIT_CAGE_PRODUCTION_STATEMENT.to_string()];
    if !allowed_files.is_empty() {
        stop_conditions.push(format!(
            "{EDIT_CAGE_TERMINALITY_WARNING} (allowed: {})",
            allowed_files.join(", ")
        ));
    }

    let changed_behavior = facts
        .witness
        .map(|witness| witness.changed_expression.trim())
        .filter(|expression| !expression.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| entry.seam.expression().to_string());
    let exact_blocker = facts
        .witness
        .and_then(|witness| witness.missing_discriminators.first())
        .map(|fact| format!("{}: {}", fact.value, fact.reason));
    let has_observer_setup = facts
        .witness
        .is_some_and(|witness| witness.expected_sink.is_some());
    let assertion_goal_detail = facts
        .witness
        .and_then(|witness| witness.suggested_assertion.clone());
    let limitations: Vec<String> = facts
        .witness
        .map(|witness| {
            witness
                .limitations
                .iter()
                .map(|limitation| limitation.detail.trim())
                .filter(|detail| !detail.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    // The card never presents a runnable route the shared instruction
    // vocabulary and repair-route readiness gate would not expose.
    let route_exposed = eligibility.eligible()
        && edit_cage_refusal.is_none()
        && repair_card_route_exposable(instruction.state, readiness.is_repair_ready());
    let next_command = if route_exposed {
        facts.next_command.as_ref()
    } else {
        None
    };

    let seam_id = entry.seam.id().as_str().to_string();
    let packet_route = format!("ripr agent packet --seam-id {seam_id} --json");
    let detail_sources = detail_sources_for(facts, &packet_route, &done_when)?;

    build_repair_card(&RepairCardInput {
        snapshot: RepairCardSnapshot {
            workspace_identity: facts.workspace_identity.to_string(),
            repository_head: facts.repository_head.to_string(),
            currentness: facts.currentness,
        },
        subject: RepairCardSubject {
            seam_id,
            canonical_gap_id: readiness.canonical_gap_id.clone(),
            finding_id: facts.finding_id.map(str::to_string),
        },
        instruction: &instruction,
        readiness,
        changed_behavior,
        exact_blocker,
        has_observer_setup,
        assertion_goal_detail,
        candidate_value: None,
        packet_eligible: eligibility.eligible(),
        edit_cage_refusal,
        next_command,
        allowed_files,
        forbidden_files,
        done_when,
        stop_conditions,
        selected_basis: None,
        rejected_alternatives: Vec::new(),
        attempt: facts.attempt,
        limitations,
        detail_sources,
        budget: RepairCardBudget::default(),
    })
}

/// Whether the rendered packet envelope actually surfaces this seam. The
/// envelope's `packets` array honors the shared queue policy; an entry whose
/// `seam_id` names this seam is the only current-surface proof.
fn packet_surfaces_seam(packet_content: &serde_json::Value, seam_id: &str) -> bool {
    packet_content
        .get("packets")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|packets| {
            packets.iter().any(|packet| {
                packet.get("seam_id").and_then(serde_json::Value::as_str) == Some(seam_id)
            })
        })
}

/// Every load-bearing evidence family rides behind a typed reference. The
/// canonical packet and the static-movement axis name the packet route; the
/// witness-derived families name the `ripr explain` route for the same
/// finding; families with no producer on this surface record an exact
/// unavailable reason instead of inventing content.
fn detail_sources_for(
    facts: &SeamCardFacts<'_>,
    packet_route: &str,
    done_when: &RepairCardDoneWhen,
) -> Result<Vec<RepairCardDetailSource>, String> {
    let packet_content: serde_json::Value =
        serde_json::from_str(facts.packet_json).map_err(|error| {
            format!("agent card could not bind the canonical packet content: {error}")
        })?;
    // The packet envelope honors the shared queue policy: a seam the queue
    // does not surface renders with no matching entry in `packets`. Crediting
    // the card with a current packet route for a seam the packet itself omits
    // would overstate the evidence, so the family records an exact unavailable
    // reason instead (fail-closed, like every other omitted family).
    let mut sources = Vec::new();
    if packet_surfaces_seam(&packet_content, facts.entry.seam.id().as_str()) {
        sources.push(RepairCardDetailSource::current(
            RepairCardDetailFamily::CanonicalPacket,
            packet_route,
            packet_content,
        ));
    } else {
        sources.push(RepairCardDetailSource::unavailable(
            RepairCardDetailFamily::CanonicalPacket,
            "the canonical packet does not surface this seam under the current packet queue policy",
        ));
    }
    match (facts.witness, facts.finding_id) {
        (Some(witness), Some(finding_id)) => {
            let explain_route = format!("ripr explain {finding_id}");
            sources.push(RepairCardDetailSource::current(
                RepairCardDetailFamily::FixInstruction,
                &explain_route,
                serde_json::to_value(witness)
                    .map_err(|error| format!("agent card witness content failed: {error}"))?,
            ));
            sources.push(RepairCardDetailSource::current(
                RepairCardDetailFamily::WitnessStageEvidence,
                "ripr check --json",
                serde_json::to_value(&facts.entry.evidence).map_err(|error| {
                    format!("agent card stage-evidence content failed: {error}")
                })?,
            ));
            sources.push(RepairCardDetailSource::current(
                RepairCardDetailFamily::LimitationDetail,
                &explain_route,
                serde_json::to_value(&witness.limitations)
                    .map_err(|error| format!("agent card limitation content failed: {error}"))?,
            ));
        }
        _ => {
            let reason = "no witness-producing finding names this seam in the current analysis";
            sources.push(RepairCardDetailSource::unavailable(
                RepairCardDetailFamily::FixInstruction,
                reason,
            ));
            sources.push(RepairCardDetailSource::unavailable(
                RepairCardDetailFamily::WitnessStageEvidence,
                reason,
            ));
            sources.push(RepairCardDetailSource::unavailable(
                RepairCardDetailFamily::LimitationDetail,
                reason,
            ));
        }
    }
    sources.push(RepairCardDetailSource::current(
        RepairCardDetailFamily::RelatedTestCandidates,
        packet_route,
        serde_json::to_value(&facts.entry.evidence.related_tests)
            .map_err(|error| format!("agent card related-test content failed: {error}"))?,
    ));
    sources.push(RepairCardDetailSource::current(
        RepairCardDetailFamily::StaticMovement,
        packet_route,
        serde_json::to_value(done_when)
            .map_err(|error| format!("agent card done-when content failed: {error}"))?,
    ));
    match facts.attempt {
        Some(manifest) => {
            // The attempt manifest is bound to the repository head it was
            // recorded at; an attempt recorded against another head is stale
            // evidence for this snapshot, not current status.
            let mut source = RepairCardDetailSource::current(
                RepairCardDetailFamily::RepairAttemptStatus,
                "ripr agent status --json",
                serde_json::to_value(manifest)
                    .map_err(|error| format!("agent card attempt content failed: {error}"))?,
            );
            if manifest.repository_head != facts.repository_head {
                source.state = RepairCardDetailState::Stale;
            }
            sources.push(source);
        }
        None => sources.push(RepairCardDetailSource::unavailable(
            RepairCardDetailFamily::RepairAttemptStatus,
            "no repair attempt is recorded for this seam",
        )),
    }
    sources.push(RepairCardDetailSource::unavailable(
        RepairCardDetailFamily::FocusedProofReceipt,
        "focused-proof receipts are written by the repair journey; none is recorded for this seam",
    ));
    sources.push(RepairCardDetailSource::unavailable(
        RepairCardDetailFamily::MutationCalibration,
        "mutation calibration is optional and never required to close a card; none is produced for this seam",
    ));
    Ok(sources)
}

/// Run the check pipeline and project the witness for the finding that names
/// this seam's canonical gap, when one exists. The witness is the single
/// instruction authority (`FixInstructionSummary::from_witness`); no second
/// derivation happens here.
fn witness_for_seam(
    root: &Path,
    config: &RiprConfig,
    entry: &ClassifiedSeam,
    canonical_gap_id: Option<&str>,
) -> Result<Option<(String, DiagnosticWitness)>, String> {
    // The expensive workspace check only feeds the canonical-gap match; with
    // no gap id the match cannot bind, so skip the analysis entirely.
    if canonical_gap_id.is_none() {
        return Ok(None);
    }
    let output = check_workspace_with_config(
        CheckInput {
            root: root.to_path_buf(),
            git_timeout: Some(super::default_cli_git_timeout()),
            ..Default::default()
        },
        config,
    )
    .map_err(|error| format!("agent card could not run the witness analysis: {error}"))?;
    Ok(witness_from_findings(
        &output.findings,
        entry,
        canonical_gap_id,
    ))
}

/// Project the witness for the finding that names this seam's canonical gap
/// from an already-completed finding set. The owner-match rule is the single
/// seam↔witness binding authority: consumers that already hold a completed
/// analysis (the LSP snapshot, #4668) bind the witness here instead of
/// re-running the check pipeline, and no consumer may re-derive the match.
pub(crate) fn witness_from_findings(
    findings: &[crate::domain::Finding],
    entry: &ClassifiedSeam,
    canonical_gap_id: Option<&str>,
) -> Option<(String, DiagnosticWitness)> {
    let gap_id = canonical_gap_id?;
    findings
        .iter()
        .find(|finding| {
            finding
                .canonical_gap
                .as_ref()
                .is_some_and(|gap| gap_names_seam(gap, gap_id, entry.seam.owner()))
        })
        .and_then(|finding| {
            DiagnosticWitness::from_finding(finding).map(|witness| (finding.id.clone(), witness))
        })
}

/// A canonical gap id is content-derived and excludes the source location, so
/// sibling seams can share one id; the id match alone would credit this seam
/// with another seam's witness. The owner is the discriminating identity and
/// must match this seam's owner.
fn gap_names_seam(gap: &FindingCanonicalGap, gap_id: &str, owner: &str) -> bool {
    gap.id == gap_id && gap.owner == owner
}

/// The portable workspace identity is producer-owned through the admitted
/// test-target evidence (the `TestTargetEvidence` precedent). When nothing on
/// this seam names it, the card refuses to mint one: identity is never
/// fabricated from a checkout path. Exposed crate-internally so the LSP
/// editor projection (#4668) consumes the exact same derivation.
pub(crate) fn workspace_identity_for(
    entry: &ClassifiedSeam,
    readiness: &RepairRouteReadiness,
) -> Result<String, String> {
    if let RepairTargetSelection::Existing(target) = &readiness.target_selection {
        return Ok(target.workspace_identity().to_string());
    }
    for test in &entry.evidence.related_tests {
        if let Some(target) = &test.test_target {
            return Ok(target.workspace_identity().to_string());
        }
    }
    Err(format!(
        "agent card cannot name the portable workspace identity for seam `{}`: no admitted test-target evidence names it; run `ripr agent packet --seam-id {} --json` for the full evidence packet",
        entry.seam.id().as_str(),
        entry.seam.id().as_str()
    ))
}

/// The most recently created recorded attempt for this seam, if any. Attempt
/// ids are content hashes (not time-ordered), so recency is the manifest's
/// own `created_unix_ms`. Exposed crate-internally so the LSP editor
/// projection (#4668) binds attempt state through the same inventory.
pub(crate) fn latest_attempt_for_seam(
    root: &Path,
    seam_id: &str,
) -> Result<Option<RepairAttemptManifest>, String> {
    let attempts = inventory_repair_attempts(root)?;
    Ok(attempts
        .into_iter()
        .filter_map(|entry| match entry {
            RepairAttemptInventoryEntry::Valid(manifest) if manifest.seam_id == seam_id => {
                Some(*manifest)
            }
            _ => None,
        })
        .max_by_key(|manifest| manifest.created_unix_ms))
}

#[cfg(test)]
mod tests {
    use super::super::repair_attempt::{
        REPAIR_ATTEMPT_SCHEMA_VERSION, RepairAttemptId, RepairAttemptState,
    };
    use super::*;
    use crate::analysis::seams::{
        ExpectedSink, RepoSeam, RequiredDiscriminator, SeamGripClass, SeamKind,
    };
    use crate::analysis::test_grip_evidence::TestGripEvidence;
    use crate::domain::{Confidence, FixInstructionState, StageEvidence, StageState};

    fn stage(state: StageState) -> StageEvidence {
        StageEvidence::new(state, Confidence::Medium, "test stage")
    }

    fn boundary_seam() -> RepoSeam {
        RepoSeam::new(
            "src/pricing.rs",
            "pricing::discounted_total",
            SeamKind::PredicateBoundary,
            42,
            88,
            "amount >= discount_threshold",
            RequiredDiscriminator::BoundaryValue {
                description: "amount >= discount_threshold".to_string(),
            },
            ExpectedSink::ReturnValue,
        )
    }

    fn weakly_gripped_entry() -> ClassifiedSeam {
        let seam = boundary_seam();
        let seam_id = seam.id().clone();
        ClassifiedSeam {
            seam,
            evidence: TestGripEvidence {
                seam_id,
                related_tests: Vec::new(),
                reach: stage(StageState::Yes),
                activate: stage(StageState::Yes),
                propagate: stage(StageState::Yes),
                observe: stage(StageState::Yes),
                discriminate: stage(StageState::No),
                observed_values: Vec::new(),
                missing_discriminators: Vec::new(),
                new_test_target: None,
            },
            class: SeamGripClass::WeaklyGripped,
        }
    }

    /// The rendered packet envelope for this seam: `packets` names the seam,
    /// so the canonical packet family projects as current.
    fn packet_for(entry: &ClassifiedSeam) -> String {
        format!(
            r#"{{"schema_version":"0.5","packets":[{{"seam_id":"{}"}}]}}"#,
            entry.seam.id().as_str()
        )
    }

    fn facts_for<'a>(entry: &'a ClassifiedSeam, packet_json: &'a str) -> SeamCardFacts<'a> {
        SeamCardFacts {
            entry,
            witness: None,
            finding_id: None,
            attempt: None,
            packet_json,
            repository_head: "abc123",
            workspace_identity: "workspace:demo",
            currentness: RepairCardSnapshotCurrentness::Current,
            next_command: None,
        }
    }

    #[test]
    fn repair_card_assembly_projects_every_family_with_packet_route() -> Result<(), String> {
        let entry = weakly_gripped_entry();
        let packet = packet_for(&entry);
        let card = assemble_repair_card(&facts_for(&entry, &packet))?;
        if card.schema_version != crate::domain::REPAIR_CARD_SCHEMA_VERSION {
            return Err("card does not name the repair card schema".to_string());
        }
        if card.subject.seam_id != entry.seam.id().as_str() {
            return Err("card subject does not name the seam".to_string());
        }
        if card.detail_references.len() != 9 {
            return Err(format!(
                "expected nine detail references, got {}",
                card.detail_references.len()
            ));
        }
        let packet_ref = card
            .detail_references
            .iter()
            .find(|reference| reference.family == RepairCardDetailFamily::CanonicalPacket)
            .ok_or_else(|| "canonical packet reference missing".to_string())?;
        if packet_ref.state != crate::domain::RepairCardDetailState::Current
            || !packet_ref
                .route
                .as_deref()
                .is_some_and(|route| route.contains("ripr agent packet --seam-id"))
        {
            return Err("canonical packet must ride behind the packet route".to_string());
        }
        for family in [
            RepairCardDetailFamily::FixInstruction,
            RepairCardDetailFamily::WitnessStageEvidence,
            RepairCardDetailFamily::LimitationDetail,
        ] {
            let reference = card
                .detail_references
                .iter()
                .find(|reference| reference.family == family)
                .ok_or_else(|| format!("reference missing for {family:?}"))?;
            if reference.state != crate::domain::RepairCardDetailState::Unavailable
                || reference
                    .unavailable_reason
                    .as_deref()
                    .is_none_or(str::is_empty)
            {
                return Err(format!(
                    "{family:?} must record an exact unavailable reason"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn repair_card_assembly_preserves_static_limitations_without_cage_admission()
    -> Result<(), String> {
        let entry = weakly_gripped_entry();
        let readiness = repair_packet_eligibility(&entry).readiness;
        assert!(!readiness.is_repair_ready());
        assert!(!readiness.missing_evidence.is_empty());
        let packet = packet_for(&entry);
        assert_eq!(
            super::super::repair_attempt::edit_cage_policy_from_packet(
                &packet,
                entry.seam.id().as_str()
            )
            .err()
            .as_deref(),
            Some("repair packet is missing allowed_edit_surface")
        );
        let card = assemble_repair_card(&facts_for(&entry, &packet))?;
        assert!(!card.readiness.repair_ready);
        assert_eq!(card.readiness.missing_evidence, readiness.missing_evidence);
        assert!(card.exact_blocker.is_none());
        assert!(card.next_action.is_none());
        Ok(())
    }

    #[test]
    fn repair_card_assembly_omits_next_action_when_route_gate_is_closed() -> Result<(), String> {
        let entry = weakly_gripped_entry();
        let command = agent_inspection_command_spec(
            AgentArtifactRoute::Packet,
            "src",
            entry.seam.id().as_str(),
        );
        let packet = packet_for(&entry);
        let mut facts = facts_for(&entry, &packet);
        facts.next_command = Some(command);
        let card = assemble_repair_card(&facts)?;
        if card.next_action.is_some() {
            return Err(
                "a card without witness instruction must not present a next action".to_string(),
            );
        }
        if card.instruction.state != FixInstructionState::Unavailable {
            return Err("missing witness must project an unavailable instruction".to_string());
        }
        Ok(())
    }

    #[test]
    fn repair_card_assembly_stays_within_the_default_bounds() -> Result<(), String> {
        let entry = weakly_gripped_entry();
        let packet = packet_for(&entry);
        let card = assemble_repair_card(&facts_for(&entry, &packet))?;
        if card.detail_references.len() > crate::domain::DEFAULT_REPAIR_CARD_MAX_DETAIL_ITEMS {
            return Err("card exceeded the default item bound".to_string());
        }
        if card.detail_summary.selected_bytes
            > crate::domain::DEFAULT_REPAIR_CARD_MAX_SERIALIZED_BYTES
        {
            return Err("card exceeded the default byte bound".to_string());
        }
        Ok(())
    }

    #[test]
    fn gap_names_seam_requires_owner_match() -> Result<(), String> {
        let gap = FindingCanonicalGap {
            id: "gap-1".to_string(),
            language: "rust".to_string(),
            file: "src/pricing.rs".to_string(),
            owner: "pricing::discounted_total".to_string(),
            behavior_kind: "predicate_boundary".to_string(),
            probe_kind: "predicate_boundary".to_string(),
            normalized_discriminator: "amount >= discount_threshold".to_string(),
        };
        if !gap_names_seam(&gap, "gap-1", "pricing::discounted_total") {
            return Err("the seam's own owner must name the seam".to_string());
        }
        if gap_names_seam(&gap, "gap-1", "pricing::other_total") {
            return Err("a gap id shared with another owner must not name the seam".to_string());
        }
        Ok(())
    }

    #[test]
    fn packet_queue_omission_marks_canonical_packet_unavailable() -> Result<(), String> {
        let entry = weakly_gripped_entry();
        let empty_packet = r#"{"schema_version":"0.5","packets":[]}"#;
        let card = assemble_repair_card(&facts_for(&entry, empty_packet))?;
        let reference = card
            .detail_references
            .iter()
            .find(|reference| reference.family == RepairCardDetailFamily::CanonicalPacket)
            .ok_or_else(|| "canonical packet reference missing".to_string())?;
        if reference.state != RepairCardDetailState::Unavailable {
            return Err(
                "a seam the packet queue omits must not project a current canonical packet"
                    .to_string(),
            );
        }
        if reference
            .unavailable_reason
            .as_deref()
            .is_none_or(str::is_empty)
        {
            return Err("the omitted packet must record an exact unavailable reason".to_string());
        }
        Ok(())
    }

    fn attempt_at(repository_head: &str, seam_id: &str) -> Result<RepairAttemptManifest, String> {
        Ok(RepairAttemptManifest {
            schema_version: REPAIR_ATTEMPT_SCHEMA_VERSION.to_string(),
            kind: "repair_attempt".to_string(),
            repair_attempt_id: RepairAttemptId::parse("repair-attempt-0123456789abcdef01234567")
                .map_err(|error| format!("test attempt id: {error}"))?,
            state: RepairAttemptState::AwaitingEdit,
            root: ".".to_string(),
            repository_head: repository_head.to_string(),
            producer_version: "test".to_string(),
            seam_id: seam_id.to_string(),
            created_unix_ms: 0,
            artifacts: Vec::new(),
            next_command: "ripr agent repair --root . --seam-id seam-a --phase after".to_string(),
            limitations: Vec::new(),
            non_claims: Vec::new(),
            after: None,
            last_after_refusal: None,
            terminal_artifacts: Vec::new(),
            store: None,
        })
    }

    #[test]
    fn attempt_recorded_at_another_head_is_stale_not_current() -> Result<(), String> {
        let entry = weakly_gripped_entry();
        let attempt = attempt_at("deadbeef", entry.seam.id().as_str())?;
        let packet = packet_for(&entry);
        let mut facts = facts_for(&entry, &packet);
        facts.attempt = Some(&attempt);
        let card = assemble_repair_card(&facts)?;
        let reference = card
            .detail_references
            .iter()
            .find(|reference| reference.family == RepairCardDetailFamily::RepairAttemptStatus)
            .ok_or_else(|| "repair attempt reference missing".to_string())?;
        if reference.state != RepairCardDetailState::Stale {
            return Err(
                "an attempt recorded at another head must project stale, not current".to_string(),
            );
        }
        if reference.route.as_deref().is_none() {
            return Err("a stale attempt keeps its typed status route".to_string());
        }
        Ok(())
    }

    #[test]
    fn dirty_evidence_tree_projects_accepted_dirty_draft_on_both_axes() -> Result<(), String> {
        let entry = weakly_gripped_entry();
        let packet = packet_for(&entry);
        let mut facts = facts_for(&entry, &packet);
        facts.currentness = RepairCardSnapshotCurrentness::AcceptedDirtyDraft;
        let card = assemble_repair_card(&facts)?;
        if card.snapshot.currentness != RepairCardSnapshotCurrentness::AcceptedDirtyDraft {
            return Err(
                "a dirty evidence scope must project accepted_dirty_draft on the snapshot axis"
                    .to_string(),
            );
        }
        if card.done_when.currentness != CardCurrentnessGoal::AcceptedDirtyDraft {
            return Err(
                "a dirty evidence scope must project accepted_dirty_draft on the done_when axis"
                    .to_string(),
            );
        }
        // The dirty-draft state is load-bearing identity, not presentation.
        let clean = assemble_repair_card(&facts_for(&entry, &packet))?;
        if clean.repair_card_id == card.repair_card_id {
            return Err("the currentness projection must remint the semantic identity".to_string());
        }
        Ok(())
    }

    #[test]
    fn clean_evidence_tree_projects_current_on_both_axes() -> Result<(), String> {
        let entry = weakly_gripped_entry();
        let packet = packet_for(&entry);
        let card = assemble_repair_card(&facts_for(&entry, &packet))?;
        if card.snapshot.currentness != RepairCardSnapshotCurrentness::Current {
            return Err(
                "a clean evidence scope must project current on the snapshot axis".to_string(),
            );
        }
        if card.done_when.currentness != CardCurrentnessGoal::Current {
            return Err(
                "a clean evidence scope must project current on the done_when axis".to_string(),
            );
        }
        Ok(())
    }

    /// #5005: the packet bytes the card content-hashes must carry no absolute
    /// checkout spelling. Binding the checkout the way the pre-fix producer
    /// did (`bound_root`) would embed the absolutized working directory in
    /// every `next`-block command string and leak it into
    /// `detail_digest`.
    #[test]
    fn card_packet_json_binds_no_checkout_spelling() -> Result<(), String> {
        let entry = weakly_gripped_entry();
        let first = card_packet_json(&entry);
        let second = card_packet_json(&entry);
        if first != second {
            return Err("the content-bound packet render is not deterministic".to_string());
        }
        let bound_cwd = crate::agent::loop_commands::bound_root(".");
        if bound_cwd == "." {
            return Err("the test working directory must resolve to an absolute root".to_string());
        }
        if first.contains(&bound_cwd) {
            return Err(
                "the content-bound packet render leaks the absolute checkout spelling".to_string(),
            );
        }
        Ok(())
    }

    /// #5005: equivalent evidence assembled through the producer's own
    /// content-bound render mints one card identity — the packet content
    /// carries no root spelling to differ on.
    #[test]
    fn equivalent_evidence_mints_one_card_identity_through_the_portable_render()
    -> Result<(), String> {
        let entry = weakly_gripped_entry();
        let first = card_packet_json(&entry);
        let second = card_packet_json(&entry);
        let first_card = assemble_repair_card(&facts_for(&entry, &first))?;
        let second_card = assemble_repair_card(&facts_for(&entry, &second))?;
        if first_card.repair_card_id != second_card.repair_card_id {
            return Err("equivalent evidence minted different card identities".to_string());
        }
        if first_card.complete_evidence_digest != second_card.complete_evidence_digest {
            return Err(
                "equivalent evidence minted different complete evidence identities".to_string(),
            );
        }
        Ok(())
    }

    /// #5008 review: the probe pathspecs keep the filesystem spelling.
    /// `display_path` percent-encodes `%` for presentation; feeding that
    /// spelling to git would probe a path that does not exist and silently
    /// read a dirty scope as clean.
    #[test]
    fn evidence_probe_paths_keep_the_filesystem_spelling() -> Result<(), String> {
        let seam = RepoSeam::new(
            "src/price%off.rs",
            "pricing::discounted_total",
            SeamKind::PredicateBoundary,
            42,
            88,
            "amount >= discount_threshold",
            RequiredDiscriminator::BoundaryValue {
                description: "amount >= discount_threshold".to_string(),
            },
            ExpectedSink::ReturnValue,
        );
        let seam_id = seam.id().clone();
        let entry = ClassifiedSeam {
            seam,
            evidence: TestGripEvidence {
                seam_id,
                related_tests: Vec::new(),
                reach: stage(StageState::Yes),
                activate: stage(StageState::Yes),
                propagate: stage(StageState::Yes),
                observe: stage(StageState::Yes),
                discriminate: stage(StageState::No),
                observed_values: Vec::new(),
                missing_discriminators: Vec::new(),
                new_test_target: None,
            },
            class: SeamGripClass::WeaklyGripped,
        };
        let paths = evidence_probe_paths(&entry);
        if paths != vec![std::path::PathBuf::from("src/price%off.rs")] {
            return Err(format!(
                "the probe must use the filesystem spelling, got {paths:?}"
            ));
        }
        Ok(())
    }

    /// #5008: a probe failure fails closed — no card, hence no `current`
    /// claim. A `.git` file with a dangling `gitdir:` pointer is fatal in
    /// every git version (unlike an invalid `.git` directory, which some
    /// versions skip during discovery), so the probe errors exactly as it
    /// would outside a repo, independent of where the test temp directory
    /// lives.
    #[test]
    fn evidence_tree_currentness_probe_failure_fails_closed() -> Result<(), String> {
        let probe_root = std::env::temp_dir().join(format!(
            "ripr-card-currentness-probe-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| format!("clock error: {error}"))?
                .as_nanos()
        ));
        std::fs::create_dir_all(&probe_root)
            .map_err(|error| format!("probe fixture directory failed: {error}"))?;
        std::fs::write(probe_root.join(".git"), "gitdir: /nonexistent/ripr-probe\n")
            .map_err(|error| format!("probe fixture gitfile failed: {error}"))?;
        let entry = weakly_gripped_entry();
        let outcome = evidence_tree_currentness(&probe_root, &entry);
        std::fs::remove_dir_all(&probe_root)
            .map_err(|error| format!("probe fixture cleanup failed: {error}"))?;
        let Err(_message) = outcome else {
            return Err(
                "a dirty-state probe failure must fail closed, not default to current".to_string(),
            );
        };
        Ok(())
    }
}
