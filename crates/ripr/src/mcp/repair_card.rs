//! MCP RepairCard projection (#4668, RIPR-SPEC-0215).
//!
//! `ripr_get_repair_card` / `ripr://repair-card/{canonical_item_id}` project
//! the same versioned [`crate::domain::RepairCardV1`] the CLI
//! `ripr agent card` handoff (#4667) and the standard-LSP projection
//! (RIPR-SPEC-0198) consume, for one canonical item of the committed
//! snapshot. The adapter owns framing only: every card fact is bound through
//! the shared `crate::app::repair_card_handoff` authorities, and the card
//! itself is assembled by the same pure [`assemble_repair_card`] projection —
//! the MCP layer never re-derives readiness, identity, currentness, or route
//! state and never reconstructs a card from weaker evidence.
//!
//! Two transport facts are snapshot-bound instead of request-bound, because
//! ADR 0022 forbids the adapter from launching git per read: the repository
//! head and the evidence-scope dirty-state probe are captured once when
//! `ripr_refresh` commits the snapshot (the one bounded analysis attempt),
//! alongside the shared diff-scoped seam inventory the card producer
//! consumes. A card therefore binds the *analyzed* head of its snapshot, and
//! post-refresh edits stay invisible until the next refresh — the session's
//! existing freshness contract. The durable repair-attempt store, by
//! contrast, is plain filesystem state and is re-read live at card-read time
//! through the same inventory the CLI and LSP card producers run.

use super::gaps::GapItem;
use super::workspace::{
    AttemptFailure, CODE_ANALYSIS_FAILED, CODE_ITEM_NOT_FOUND, CODE_NO_SNAPSHOT,
    CODE_WORKSPACE_UNAVAILABLE, Snapshot, WorkspaceSession, bounded_document,
};
use crate::agent::artifact::git_output;
use crate::agent::command_specs::{AgentArtifactRoute, agent_inspection_command_spec};
use crate::analysis::ClassifiedSeam;
use crate::analysis::inventory_diff_scoped_classified_seams_at_with_config;
use crate::analysis::repair_route::repair_packet_eligibility;
use crate::app::repair_card_handoff::{
    AgentCardError, SeamCardFacts, assemble_repair_card, card_packet_json, latest_attempt_for_seam,
    witness_from_findings, workspace_identity_for,
};
use crate::config::RiprConfig;
use crate::domain::{AgentCardRefusalKind, RepairCardSnapshotCurrentness};
use serde_json::{Value, json};
use std::path::Path;

pub(crate) const REPAIR_CARD_SCHEMA_VERSION: &str = "ripr-mcp-repair-card-v1";

/// `ripr_get_repair_card` — the tool name travels in one const so the
/// descriptor, the dispatch, and the help text cannot drift apart.
pub(crate) const GET_REPAIR_CARD_TOOL_NAME: &str = "ripr_get_repair_card";

/// `ripr://repair-card/{canonical_item_id}` — the resource template routes
/// canonical item ids exactly like `ripr://gap/{canonical_item_id}`.
pub(crate) const REPAIR_CARD_TEMPLATE: &str = "ripr://repair-card/{canonical_item_id}";

/// Typed failure codes this slice adds to the shared wire vocabulary. The
/// spellings are the [`AgentCardRefusalKind`] wire spellings the CLI
/// `agent_card_refusal` envelope already pins, so one machine state names
/// one refusal on every transport. `policy_omitted` and
/// `witness_unavailable` stay named for wire stability even though this
/// slice cannot reach them (project-local policy is never loaded and the
/// witness binds from committed findings without a failing producer call).
pub(crate) const CODE_SEAM_NOT_FOUND: &str = "seam_not_found";
pub(crate) const CODE_POLICY_OMITTED: &str = "policy_omitted";
pub(crate) const CODE_WITNESS_UNAVAILABLE: &str = "witness_unavailable";
pub(crate) const CODE_IDENTITY_UNNAMEABLE: &str = "identity_unnameable";
pub(crate) const CODE_BUDGET_OVERFLOW: &str = "budget_overflow";

/// The repair-card producer facts bound when one bounded analysis attempt
/// commits the snapshot. `ripr_refresh` is the only analysis the adapter ever
/// runs (ADR 0022); capturing the card producers there keeps every card read
/// a pure projection over committed state plus the live durable attempt
/// store.
#[derive(Clone, Debug)]
pub(crate) struct SnapshotCardProducers {
    /// The analyzed repository head (`git rev-parse HEAD` at commit time).
    /// The card's semantic identity binds this head; the adapter never
    /// re-resolves git state per read.
    pub(crate) repository_head: String,
    /// The seams that owner-discriminated bind a snapshot canonical item,
    /// each with its commit-time evidence-scope currentness probe.
    pub(crate) bindings: Vec<SeamCardBinding>,
}

/// One seam bound to one snapshot canonical item at commit time.
#[derive(Clone, Debug)]
pub(crate) struct SeamCardBinding {
    /// The snapshot canonical item id this seam's readiness gap names.
    pub(crate) item_canonical_id: String,
    pub(crate) seam: ClassifiedSeam,
    /// The bounded dirty-state probe over the files this card's evidence
    /// binds, observed while the committing attempt ran.
    pub(crate) currentness: RepairCardSnapshotCurrentness,
}

/// Extract the canonical item id from a `ripr://repair-card/{id}` resource
/// URI. The grammar matches the gap template exactly: one non-empty path
/// segment with no further separators.
pub(crate) fn repair_card_resource_id(uri: &str) -> Option<&str> {
    uri.strip_prefix("ripr://repair-card/")
        .filter(|id| !id.is_empty() && !id.contains('/'))
}

/// Bind the repair-card producers into one committing snapshot. This runs
/// inside the refresh attempt (the adapter's single allowed analysis), after
/// the shared check authority completed: the diff-scoped seam inventory over
/// the snapshot's own item files supplies the same classified seams the CLI
/// card producer consumes, the analyzed head is resolved once, and each
/// owner-discriminated binding records its evidence-scope currentness probe.
/// Any failure fails the attempt: the snapshot commits complete or not at
/// all, so the card surface can never silently vanish under a served
/// snapshot identity.
pub(crate) fn bind_snapshot_card_producers(
    root: &Path,
    snapshot: &mut Snapshot,
) -> Result<(), AttemptFailure> {
    let repository_head = git_output(root, &["rev-parse", "HEAD"])
        .map_err(|error| {
            AttemptFailure::new(
                CODE_ANALYSIS_FAILED,
                format!("the analyzed repository head could not be resolved: {error}"),
                "retry with ripr_refresh",
            )
        })?
        .trim()
        .to_string();
    let mut bindings = Vec::new();
    if !snapshot.items.is_empty() {
        let config = RiprConfig::default();
        let changed_files = snapshot
            .items
            .iter()
            .map(|item| std::path::PathBuf::from(item.file.as_str()))
            .collect::<Vec<_>>();
        let changed_owner_names = snapshot
            .findings
            .iter()
            .filter_map(|finding| finding.canonical_gap.as_ref().map(|gap| gap.owner.clone()))
            .collect::<Vec<_>>();
        let inventory = inventory_diff_scoped_classified_seams_at_with_config(
            root,
            &config,
            &changed_files,
            &changed_owner_names,
        )
        .map_err(|error| {
            AttemptFailure::new(
                CODE_ANALYSIS_FAILED,
                format!("the seam inventory authority failed during refresh: {error}"),
                "retry with ripr_refresh; if the failure persists, run `ripr agent packet` in the repository",
            )
        })?;
        for entry in inventory.classified {
            let Some(item) = item_bound_by_seam(snapshot, &entry) else {
                continue;
            };
            let currentness = crate::app::repair_card_handoff::evidence_tree_currentness(
                root, &entry,
            )
            .map_err(|error| {
                AttemptFailure::new(
                    CODE_ANALYSIS_FAILED,
                    format!("the evidence currentness probe failed during refresh: {error}"),
                    "retry with ripr_refresh",
                )
            })?;
            bindings.push(SeamCardBinding {
                item_canonical_id: item.canonical_id.clone(),
                seam: entry,
                currentness,
            });
        }
    }
    snapshot.card_producers = Some(SnapshotCardProducers {
        repository_head,
        bindings,
    });
    Ok(())
}

/// The snapshot canonical item one classified seam owner-discriminated
/// binds, if any. The binding rule is the shared card producer's own: the
/// seam's readiness canonical gap id must name the item, and the gap's
/// producer-recorded owner must be this seam's owner — a gap id shared with
/// a sibling owner never credits this seam with another seam's item.
fn item_bound_by_seam<'a>(snapshot: &'a Snapshot, entry: &ClassifiedSeam) -> Option<&'a GapItem> {
    let readiness = repair_packet_eligibility(entry).readiness;
    let gap_id = readiness.canonical_gap_id.as_deref()?;
    let item = snapshot
        .items
        .iter()
        .find(|item| item.canonical_id == gap_id)?;
    let owner_names_seam = snapshot.findings.iter().any(|finding| {
        finding
            .canonical_gap
            .as_ref()
            .is_some_and(|gap| gap.id == gap_id && gap.owner == entry.seam.owner())
    });
    owner_names_seam.then_some(item)
}

/// Map one card-assembly failure onto the typed wire vocabulary. Refusal
/// kinds travel under their pinned spellings so a loop driver branches on
/// the same machine state on every transport; operational failures stay
/// `analysis_failed`, matching the other evidence tools.
fn agent_card_failure(error: AgentCardError) -> AttemptFailure {
    match error {
        AgentCardError::Refusal { kind, message } => {
            let recovery: &'static str = match kind {
                AgentCardRefusalKind::SeamNotFound => {
                    "list the current canonical ids with ripr_list_gaps, then retry"
                }
                AgentCardRefusalKind::PolicyOmitted => {
                    "project-local policy is not loaded by this server; run `ripr agent card` in the repository for the policy-aware card"
                }
                AgentCardRefusalKind::WitnessUnavailable => {
                    "run `ripr check --format json` in the repository, then re-run ripr_refresh"
                }
                AgentCardRefusalKind::IdentityUnnameable => {
                    "run `ripr agent packet --seam-id <seam> --json` in the repository for the full evidence packet"
                }
                AgentCardRefusalKind::BudgetOverflow => {
                    "run `ripr agent packet --seam-id <seam> --json` in the repository for the unbounded packet"
                }
            };
            AttemptFailure::new(kind.as_str(), message, recovery)
        }
        AgentCardError::Operational(message) => AttemptFailure::new(
            CODE_ANALYSIS_FAILED,
            message,
            "retry with ripr_refresh; if the failure persists, run `ripr agent card` in the repository",
        ),
    }
}

impl WorkspaceSession {
    /// `ripr_get_repair_card` / `ripr://repair-card/{canonical_item_id}`:
    /// the same versioned repair card the CLI and standard-LSP handoffs
    /// project, bound to the committed snapshot and one canonical item. Read
    /// paths fail closed with the typed vocabulary; no fact is re-derived or
    /// upgraded, and a producer refusal never ships a weakened card.
    pub(crate) fn repair_card_document(
        &self,
        gap_id: &str,
        requested: Option<&str>,
        root: Option<&Path>,
    ) -> Result<Value, AttemptFailure> {
        let snapshot = self.active_snapshot(requested)?;
        let Some(item) = snapshot.item(gap_id) else {
            return Err(AttemptFailure::new(
                CODE_ITEM_NOT_FOUND,
                format!("no canonical item {gap_id} exists in the current snapshot"),
                "list the current canonical ids with ripr_list_gaps, then retry",
            ));
        };
        let Some(producers) = &snapshot.card_producers else {
            return Err(AttemptFailure::new(
                CODE_NO_SNAPSHOT,
                "the current snapshot carries no repair-card producers; it was committed before the card projection existed",
                "re-run ripr_refresh to commit a complete snapshot, then retry",
            ));
        };
        let Some(binding) = producers
            .bindings
            .iter()
            .find(|binding| binding.item_canonical_id == item.canonical_id)
        else {
            return Err(AttemptFailure::new(
                CODE_SEAM_NOT_FOUND,
                format!(
                    "no classified seam owner-discriminated binds canonical item {} in this snapshot",
                    item.canonical_id
                ),
                "read the item's evidence with ripr_get_gap, or run `ripr agent card` in the repository",
            ));
        };
        let entry = &binding.seam;
        // The bind-time owner discrimination is re-verified against the
        // committed findings on every read: the retained binding is honored
        // only while one finding still names this item's gap with this
        // seam's owner. A stale or owner-mismatched binding credits the seam
        // with another owner's item, so the read fails closed exactly like
        // an unbound item instead of projecting a mis-attributed card.
        let owner_names_seam = snapshot.findings.iter().any(|finding| {
            finding
                .canonical_gap
                .as_ref()
                .is_some_and(|gap| gap.id == item.canonical_id && gap.owner == entry.seam.owner())
        });
        if !owner_names_seam {
            return Err(AttemptFailure::new(
                CODE_SEAM_NOT_FOUND,
                format!(
                    "no committed finding names canonical item {} with the bound seam's owner in this snapshot",
                    item.canonical_id
                ),
                "read the item's evidence with ripr_get_gap, or run `ripr agent card` in the repository",
            ));
        }
        let Some(root) = root else {
            return Err(AttemptFailure::new(
                CODE_WORKSPACE_UNAVAILABLE,
                "the workspace root is unavailable, so the durable attempt store cannot be read for this card",
                "restart the server with `ripr mcp --stdio --root <repository>` and retry",
            ));
        };
        let readiness = repair_packet_eligibility(entry).readiness;
        let seam_id = entry.seam.id().as_str().to_string();
        // The witness and finding id bind through the shared
        // owner-discriminated matcher over the committed findings — the same
        // projection the CLI gather path and the LSP consume. A missing
        // witness stays a missing witness: the card assembles its typed
        // unavailable instruction instead of inventing one.
        let (finding_id, witness) = match witness_from_findings(
            &snapshot.findings,
            entry,
            readiness.canonical_gap_id.as_deref(),
        ) {
            Some((finding_id, witness)) => (Some(finding_id), Some(witness)),
            None => (None, None),
        };
        let workspace_identity = workspace_identity_for(entry, &readiness).map_err(|message| {
            AttemptFailure::new(
                CODE_IDENTITY_UNNAMEABLE,
                message,
                "run `ripr agent packet --seam-id <seam> --json` in the repository for the full evidence packet",
            )
        })?;
        // The durable attempt store is filesystem state, not analysis: the
        // shared inventory re-runs live so the card's attempt block matches
        // what `ripr agent status` would report at this moment. An
        // unreadable store fails the read closed rather than projecting a
        // wrongly empty attempt state.
        let attempt = latest_attempt_for_seam(root, &seam_id).map_err(|error| {
            AttemptFailure::new(
                CODE_ANALYSIS_FAILED,
                format!("the durable attempt store could not be inventoried: {error}"),
                "run `ripr agent status` in the repository for the full diagnostic",
            )
        })?;
        let packet_json = card_packet_json(entry);
        // The display binds the portable root: the host-local checkout path
        // is intentionally not projected (ADR 0022). The display string is
        // presentation only and never execution authority.
        let next_command = agent_inspection_command_spec(AgentArtifactRoute::Packet, ".", &seam_id);
        let card = assemble_repair_card(&SeamCardFacts {
            entry,
            witness: witness.as_ref(),
            finding_id: finding_id.as_deref(),
            attempt: attempt.as_ref(),
            packet_json: &packet_json,
            repository_head: &producers.repository_head,
            workspace_identity: &workspace_identity,
            currentness: binding.currentness,
            next_command: Some(next_command),
        })
        .map_err(agent_card_failure)?;
        let card_value = serde_json::to_value(&card).map_err(|error| {
            AttemptFailure::new(
                CODE_ANALYSIS_FAILED,
                format!("serialize the repair card: {error}"),
                "retry with ripr_refresh",
            )
        })?;
        let snapshot_id = snapshot.snapshot_id.clone();
        let item_canonical_id = item.canonical_id.clone();
        let item_finding_id = item.finding_id.clone();
        let repository_head = producers.repository_head.clone();
        // The attempt link binds the durable store only: in-memory session
        // transactions never ride a card (stated in the limitations below)
        // and stay reachable through ripr_prepare_repair /
        // ripr_get_repair_attempt.
        let (durable_attempt, receipt) = match &card.attempt {
            Some(attempt) => (
                Some(format!("ripr://repair-attempt/{}", attempt.attempt_id)),
                Some(format!("ripr://receipt/{}", attempt.attempt_id)),
            ),
            None => (None, None),
        };
        let document = json!({
            "schema_version": REPAIR_CARD_SCHEMA_VERSION,
            "snapshot_id": snapshot_id,
            "requested_snapshot_id": requested,
            "item": {
                "canonical_id": item_canonical_id,
                "finding_id": item_finding_id,
            },
            "card": card_value,
            "currentness": {
                "repository_head": repository_head,
                "basis": "the analyzed head and the evidence-scope dirty-state probe bound when ripr_refresh committed this snapshot; the adapter never re-resolves git state per read, so edits after the refresh are visible only after the next one",
            },
            "claim_boundary": "One bounded repair card for one canonical item of one committed snapshot, assembled by the same shared application authority as the CLI and standard-LSP handoffs. This projection edits nothing, executes nothing, and never upgrades readiness, actionability, or currentness.",
            "limitations": [
                "the card binds the analyzed repository head and commit-time currentness of its snapshot; a HEAD move or edit after ripr_refresh changes what the CLI would bind live, so refresh again before comparing card identities across transports",
                "the next-action display binds the portable root `.` and is presentation only; the host-local root path is intentionally not projected and the display is never execution authority",
                "attempt state is re-read from the durable store at card-read time; in-memory session transactions never ride a card and stay reachable through ripr_prepare_repair / ripr_get_repair_attempt",
                "the seam inventory and the evidence facts both ran with built-in defaults; project-local configuration stays detected-not-loaded",
            ],
            "links": {
                "snapshot": format!("ripr://snapshot/{snapshot_id}"),
                "gap": format!("ripr://gap/{item_canonical_id}"),
                "repair_attempt": durable_attempt,
                "receipt": receipt,
            },
        });
        bounded_document(document)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::seams::{
        ExpectedSink, RepoSeam, RequiredDiscriminator, SeamGripClass, SeamKind,
    };
    use crate::analysis::test_grip_evidence::{
        RelatedTestGrip, TestGripEvidence, TestTargetEvidence,
    };
    use crate::analysis_outcome::{AnalysisOutcome, AnalysisOutcomeCounts, AnalysisOutcomeKind};
    use crate::domain::{Confidence, FindingCanonicalGap, OracleKind, OracleStrength};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn stage(state: crate::domain::StageState) -> crate::domain::StageEvidence {
        crate::domain::StageEvidence::new(state, Confidence::Medium, "test stage")
    }

    /// The same weakly-gripped predicate-boundary fixture shape the shared
    /// handoff tests build: candidate-current grip with a failed
    /// discriminate stage, so the readiness gate stays closed. One
    /// directly-related strong test carries fixture test-target evidence so
    /// the portable workspace identity resolves exactly like a live seam.
    fn weakly_gripped_entry() -> ClassifiedSeam {
        let seam = RepoSeam::new(
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
        );
        let seam_id = seam.id().clone();
        ClassifiedSeam {
            seam,
            evidence: TestGripEvidence {
                seam_id,
                related_tests: vec![RelatedTestGrip {
                    test_name: "discounted_total_boundary".to_string(),
                    file: PathBuf::from("tests/pricing.rs"),
                    line: 12,
                    test_target: Some(TestTargetEvidence::fixture(
                        "discounted_total_boundary",
                        Path::new("tests/pricing.rs"),
                        12,
                    )),
                    oracle_kind: OracleKind::ExactValue,
                    oracle_strength: OracleStrength::Strong,
                    evidence_summary: "asserts the discounted total".to_string(),
                    relation_reason: crate::domain::RelationReason::DirectOwnerCall,
                    relation_confidence: crate::domain::RelationConfidence::High,
                }],
                reach: stage(crate::domain::StageState::Yes),
                activate: stage(crate::domain::StageState::Yes),
                propagate: stage(crate::domain::StageState::Yes),
                observe: stage(crate::domain::StageState::Yes),
                discriminate: stage(crate::domain::StageState::No),
                observed_values: Vec::new(),
                missing_discriminators: Vec::new(),
                new_test_target: None,
            },
            class: SeamGripClass::WeaklyGripped,
        }
    }

    fn output(findings: &[crate::domain::Finding]) -> Result<crate::app::CheckOutput, String> {
        let outcome = AnalysisOutcome::new(
            AnalysisOutcomeKind::CompleteWithFindings,
            Default::default(),
            AnalysisOutcomeCounts {
                changed_file_count: 1,
                changed_line_count: 2,
                candidate_line_count: 2,
                probe_count: 2,
                finding_count: findings.len() as u64,
            },
            Vec::new(),
        )?;
        Ok(crate::app::CheckOutput {
            schema_version: crate::app::CHECK_OUTPUT_SCHEMA_VERSION.to_string(),
            harness_projections: Vec::new(),
            tool: "ripr".to_string(),
            mode: crate::app::Mode::Draft,
            root: PathBuf::from("."),
            base: None,
            analysis_outcome: Some(outcome),
            summary: crate::domain::Summary::default(),
            findings: findings.to_vec(),
            preview_language_advisories: Vec::new(),
            language_runs: Vec::new(),
            no_scope_provided: false,
            unanalyzed_working_tree: false,
            suppression: None,
            partial_scope: None,
        })
    }

    /// One snapshot whose single item and retained finding are the shared
    /// gaps test finding, with the card producers injected directly — no
    /// analysis, git, or inventory runs in these tests.
    fn session_with_card_producers(
        binding: Option<SeamCardBinding>,
    ) -> Result<WorkspaceSession, String> {
        let output = output(std::slice::from_ref(&super::super::gaps::test_finding()?))?;
        let mut snapshot = Snapshot::from_output(&output, Some("root:sha256:a"))
            .map_err(|failure| failure.detail)?;
        snapshot.card_producers = Some(SnapshotCardProducers {
            repository_head: "abc123".to_string(),
            bindings: binding.into_iter().collect(),
        });
        Ok(WorkspaceSession {
            in_flight: false,
            last_good: Some(Arc::new(snapshot)),
            last_failure: None,
            repairs: std::collections::BTreeMap::new(),
            superseded_attempts: std::collections::BTreeMap::new(),
        })
    }

    fn session_with_finding_and_binding(
        finding: crate::domain::Finding,
        binding: SeamCardBinding,
    ) -> Result<WorkspaceSession, String> {
        let output = output(std::slice::from_ref(&finding))?;
        let mut snapshot = Snapshot::from_output(&output, Some("root:sha256:a"))
            .map_err(|failure| failure.detail)?;
        snapshot.card_producers = Some(SnapshotCardProducers {
            repository_head: "abc123".to_string(),
            bindings: vec![binding],
        });
        Ok(WorkspaceSession {
            in_flight: false,
            last_good: Some(Arc::new(snapshot)),
            last_failure: None,
            repairs: std::collections::BTreeMap::new(),
            superseded_attempts: std::collections::BTreeMap::new(),
        })
    }

    fn binding_for(entry: &ClassifiedSeam, item_canonical_id: &str) -> SeamCardBinding {
        SeamCardBinding {
            item_canonical_id: item_canonical_id.to_string(),
            seam: entry.clone(),
            currentness: RepairCardSnapshotCurrentness::Current,
        }
    }

    fn temp_root() -> Result<PathBuf, String> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("clock: {error}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!("ripr-mcp-repair-card-{nanos}")))
    }

    #[test]
    fn repair_card_resource_uri_parsing_is_strict() {
        assert_eq!(
            repair_card_resource_id("ripr://repair-card/gap:test:1"),
            Some("gap:test:1")
        );
        for other in [
            "ripr://workspace/status",
            "ripr://repair-card/",
            "ripr://repair-card/a/b",
            "ripr://gap/gap:test:1",
            "https://example.com/repair-card/x",
        ] {
            assert_eq!(repair_card_resource_id(other), None, "{other}");
        }
    }

    #[test]
    fn card_read_fails_closed_on_every_missing_producer_fact() -> Result<(), String> {
        // No snapshot at all.
        let empty = WorkspaceSession::default();
        match empty.repair_card_document("gap:any", None, None) {
            Ok(value) => return Err(format!("no_snapshot expected: {value}")),
            Err(failure) if failure.code == CODE_NO_SNAPSHOT => {}
            Err(failure) => return Err(format!("unexpected code: {}", failure.code)),
        }
        // Snapshot without bound card producers (committed by a refresh that
        // predates the card projection).
        let output = output(std::slice::from_ref(&super::super::gaps::test_finding()?))?;
        let snapshot = Snapshot::from_output(&output, Some("root:sha256:a"))
            .map_err(|failure| failure.detail)?;
        let legacy = WorkspaceSession {
            last_good: Some(Arc::new(snapshot)),
            ..WorkspaceSession::default()
        };
        match legacy.repair_card_document("gap:test:1", None, None) {
            Ok(value) => return Err(format!("producer-less snapshot must fail closed: {value}")),
            Err(failure) if failure.code == CODE_NO_SNAPSHOT => {}
            Err(failure) => return Err(format!("unexpected code: {}", failure.code)),
        }
        // Producers present but no seam binds the item.
        let unbound = session_with_card_producers(None)?;
        match unbound.repair_card_document("gap:test:1", None, Some(Path::new("."))) {
            Ok(value) => return Err(format!("unbound item must fail closed: {value}")),
            Err(failure) if failure.code == CODE_SEAM_NOT_FOUND => {}
            Err(failure) => return Err(format!("unexpected code: {}", failure.code)),
        }
        // Unknown item identity.
        match unbound.repair_card_document("gap:missing", None, None) {
            Ok(value) => return Err(format!("unknown item must fail closed: {value}")),
            Err(failure) if failure.code == CODE_ITEM_NOT_FOUND => {}
            Err(failure) => return Err(format!("unexpected code: {}", failure.code)),
        }
        Ok(())
    }

    #[test]
    fn stale_snapshot_and_in_flight_read_fail_closed() -> Result<(), String> {
        let session = session_with_card_producers(None)?;
        match session.repair_card_document("gap:test:1", Some("snapshot:sha256:old"), None) {
            Ok(value) => return Err(format!("stale snapshot must fail closed: {value}")),
            Err(failure) if failure.code == super::super::workspace::CODE_STALE_SNAPSHOT => {}
            Err(failure) => return Err(format!("unexpected code: {}", failure.code)),
        }
        let in_flight = WorkspaceSession {
            in_flight: true,
            ..WorkspaceSession::default()
        };
        match in_flight.repair_card_document("gap:test:1", None, None) {
            Ok(value) => return Err(format!("in-flight read must fail closed: {value}")),
            Err(failure) if failure.code == super::super::workspace::CODE_ANALYSIS_IN_FLIGHT => {}
            Err(failure) => return Err(format!("unexpected code: {}", failure.code)),
        }
        Ok(())
    }

    #[test]
    fn witness_bound_card_projects_the_shared_handoff_card() -> Result<(), String> {
        let entry = weakly_gripped_entry();
        // The finding carries the seam's own producer gap identity, so the
        // shared owner-discriminated matcher binds the witness.
        let gap_id = repair_packet_eligibility(&entry)
            .readiness
            .canonical_gap_id
            .clone()
            .ok_or_else(|| "fixture seam must name a canonical gap".to_string())?;
        let mut finding = super::super::gaps::test_finding()?;
        let base = finding
            .canonical_gap
            .take()
            .ok_or_else(|| "fixture finding lost its gap".to_string())?;
        finding.canonical_gap = Some(FindingCanonicalGap {
            id: gap_id.clone(),
            owner: entry.seam.owner().to_string(),
            ..base
        });
        let session = session_with_finding_and_binding(finding, binding_for(&entry, &gap_id))?;
        // The durable attempt store read needs a real directory; an empty
        // temp root inventories as zero attempts.
        let root = temp_root()?;
        std::fs::create_dir_all(&root).map_err(|error| format!("create temp root: {error}"))?;
        let read = session.repair_card_document(&gap_id, None, Some(&root));
        std::fs::remove_dir_all(&root).map_err(|error| format!("clean temp root: {error}"))?;
        let document = read.map_err(|failure| failure.detail)?;
        if document.pointer("/schema_version").and_then(Value::as_str)
            != Some(REPAIR_CARD_SCHEMA_VERSION)
        {
            return Err(format!("card document lost its schema: {document}"));
        }
        if document
            .pointer("/card/schema_version")
            .and_then(Value::as_str)
            != Some(crate::domain::REPAIR_CARD_SCHEMA_VERSION)
        {
            return Err("the projected card must keep the repair_card.v1 schema".to_string());
        }
        if document
            .pointer("/card/subject/seam_id")
            .and_then(Value::as_str)
            != Some(entry.seam.id().as_str())
        {
            return Err("the card subject must name the bound seam".to_string());
        }
        if document
            .pointer("/card/subject/finding_id")
            .and_then(Value::as_str)
            != Some("finding:test:1")
        {
            return Err("the witness-bound card must name the finding identity".to_string());
        }
        // The weakly-gripped fixture never exposes a route: the gate stays
        // closed and the wire card carries the typed none, never a weakened
        // or invented next action.
        if document.pointer("/card/next_action") != Some(&Value::Null) {
            return Err("a gated route must project next_action null".to_string());
        }
        if document
            .pointer("/currentness/repository_head")
            .and_then(Value::as_str)
            != Some("abc123")
        {
            return Err("the card must bind the analyzed snapshot head".to_string());
        }
        let snapshot_link = document
            .pointer("/links/snapshot")
            .and_then(Value::as_str)
            .ok_or_else(|| "the card document lost its snapshot link".to_string())?;
        if !snapshot_link.starts_with("ripr://snapshot/snapshot:sha256:") {
            return Err(format!("snapshot link drifted: {snapshot_link}"));
        }
        let references = document
            .pointer("/card/detail_references")
            .and_then(Value::as_array)
            .ok_or_else(|| "card lost its detail references".to_string())?;
        if references.len() != 9 {
            return Err(format!(
                "the wire card must route all nine evidence families, got {}",
                references.len()
            ));
        }
        Ok(())
    }

    #[test]
    fn refusal_kinds_travel_under_their_pinned_wire_spellings() -> Result<(), String> {
        let cases = [
            (
                AgentCardError::refusal(
                    AgentCardRefusalKind::SeamNotFound,
                    "seam missing".to_string(),
                ),
                CODE_SEAM_NOT_FOUND,
            ),
            (
                AgentCardError::refusal(
                    AgentCardRefusalKind::IdentityUnnameable,
                    "no portable identity".to_string(),
                ),
                CODE_IDENTITY_UNNAMEABLE,
            ),
            (
                AgentCardError::refusal(
                    AgentCardRefusalKind::BudgetOverflow,
                    "card exceeded its budget".to_string(),
                ),
                CODE_BUDGET_OVERFLOW,
            ),
            (
                AgentCardError::operational("the producer could not complete".to_string()),
                CODE_ANALYSIS_FAILED,
            ),
        ];
        for (error, expected) in cases {
            let failure = agent_card_failure(error);
            if failure.code != expected {
                return Err(format!(
                    "expected {expected}, got {} ({})",
                    failure.code, failure.detail
                ));
            }
            if failure.detail.is_empty() || failure.recovery.is_empty() {
                return Err("typed failures must carry detail and recovery".to_string());
            }
        }
        Ok(())
    }

    #[test]
    fn seam_binding_requires_the_gap_owner_match() -> Result<(), String> {
        let entry = weakly_gripped_entry();
        repair_packet_eligibility(&entry)
            .readiness
            .canonical_gap_id
            .as_ref()
            .ok_or_else(|| "fixture seam must name a canonical gap".to_string())?;
        let output = output(std::slice::from_ref(&super::super::gaps::test_finding()?))?;
        let mut snapshot = Snapshot::from_output(&output, Some("root:sha256:a"))
            .map_err(|failure| failure.detail)?;
        // The binding names the snapshot item itself (`gap:test:1`), so the
        // read proceeds past binding selection and the fail-closed result
        // can only come from the owner discrimination: the fixture finding's
        // gap owner (`checkout`) is not the seam's owner
        // (`pricing::discounted_total`), so no committed finding names this
        // item with the bound seam's owner.
        snapshot.card_producers = Some(SnapshotCardProducers {
            repository_head: "abc123".to_string(),
            bindings: vec![binding_for(&entry, "gap:test:1")],
        });
        let session = WorkspaceSession {
            last_good: Some(Arc::new(snapshot)),
            ..WorkspaceSession::default()
        };
        match session.repair_card_document("gap:test:1", None, None) {
            Ok(value) => Err(format!(
                "owner-mismatched binding must fail closed: {value}"
            )),
            Err(failure) if failure.code == CODE_SEAM_NOT_FOUND => Ok(()),
            Err(failure) => Err(format!("unexpected code: {}", failure.code)),
        }
    }

    #[test]
    fn item_binding_owner_discriminates_at_bind_time() -> Result<(), String> {
        let entry = weakly_gripped_entry();
        let gap_id = repair_packet_eligibility(&entry)
            .readiness
            .canonical_gap_id
            .clone()
            .ok_or_else(|| "fixture seam must name a canonical gap".to_string())?;
        let owner = entry.seam.owner().to_string();
        // Positive: a finding that names the readiness gap with this seam's
        // owner credits the seam with the item at bind time.
        let mut finding = super::super::gaps::test_finding()?;
        let gap = finding
            .canonical_gap
            .as_mut()
            .ok_or_else(|| "fixture finding must name a canonical gap".to_string())?;
        gap.id = gap_id.clone();
        gap.owner = owner;
        let matched_output = output(std::slice::from_ref(&finding))?;
        let snapshot = Snapshot::from_output(&matched_output, Some("root:sha256:a"))
            .map_err(|failure| failure.detail)?;
        let bound = item_bound_by_seam(&snapshot, &entry)
            .ok_or_else(|| "owner-matching seam must bind its item at bind time".to_string())?;
        if bound.canonical_id != gap_id {
            return Err(format!("bound the wrong item: {}", bound.canonical_id));
        }
        // Negative: a finding that names the same gap with a different owner
        // must not credit this seam with the item — the committed finding is
        // the producer of the gap-to-owner record, so the bind fails closed.
        let mut other = super::super::gaps::test_finding()?;
        let other_gap = other
            .canonical_gap
            .as_mut()
            .ok_or_else(|| "fixture finding must name a canonical gap".to_string())?;
        other_gap.id = gap_id;
        other_gap.owner = "checkout".to_string();
        let other_output = output(std::slice::from_ref(&other))?;
        let other_snapshot = Snapshot::from_output(&other_output, Some("root:sha256:a"))
            .map_err(|failure| failure.detail)?;
        if item_bound_by_seam(&other_snapshot, &entry).is_some() {
            return Err("owner-mismatched seam must bind no item at bind time".to_string());
        }
        Ok(())
    }
}
