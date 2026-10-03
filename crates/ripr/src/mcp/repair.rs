//! MCP repair projection: bounded repair transactions, durable attempt
//! reads, and receipt status — all without execution authority (ADR 0022).
//!
//! `ripr_prepare_repair` evaluates the producer repair-readiness facts the
//! snapshot committed on one canonical item and, only when every gate is
//! established, creates one session repair transaction with a deterministic,
//! root-bound attempt identity. The transaction is in-memory like the
//! snapshot: a restart drops it, and durable attempts remain owned by the
//! CLI before phase (`ripr agent repair --phase before`), which MCP reads
//! read-only through the shared [`crate::app::repair_attempt`] store.
//!
//! Boundary: this module never edits source or tests, never launches a
//! process, never executes verification or mutation commands, and never
//! loads project-local provider configuration. A missing readiness fact
//! stays a typed limitation; the tool must not guess it or create a
//! misleading attempt.

use super::gaps::GapItem;
use super::workspace::{AttemptFailure, CODE_ITEM_NOT_FOUND, WorkspaceSession, bounded_document};
use crate::app::repair_attempt::{
    AttemptTerminalReceipt, RepairAttemptInventoryEntry, RepairAttemptState,
    find_manifest_artifact_by_role, find_terminal_artifact_by_role, inventory_repair_attempts_from,
    load_attempt_terminal_receipt, repair_attempt_state_label,
};
use crate::output::receipt_lifecycle::receipt_lifecycle_state_from_receipt_value;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::Path;

pub(crate) const REPAIR_PACKET_SCHEMA_VERSION: &str = "ripr-mcp-repair-packet-v1";
pub(crate) const REPAIR_ATTEMPT_SCHEMA_VERSION: &str = "ripr-mcp-repair-attempt-v1";
pub(crate) const RECEIPT_STATUS_SCHEMA_VERSION: &str = "ripr-mcp-receipt-status-v1";

pub(crate) const REPAIR_ATTEMPT_TEMPLATE: &str = "ripr://repair-attempt/{attempt_id}";
pub(crate) const RECEIPT_TEMPLATE: &str = "ripr://receipt/{receipt_id}";

/// Typed failure codes this slice adds to the shared wire vocabulary. The
/// reserved codes from slice B stay owned by their states; unknown attempt
/// and receipt identities and canonically invalid durable manifests are
/// reachable only in this slice.
pub(crate) const CODE_ATTEMPT_NOT_FOUND: &str = "attempt_not_found";
pub(crate) const CODE_ATTEMPT_INVALID: &str = "attempt_invalid";

const ATTEMPT_ID_PREFIX: &str = "repair-attempt-";
const ATTEMPT_ID_HEX_LEN: usize = 24;

/// The product repair transaction's shared non-claims, verbatim from the
/// durable manifest authority; MCP repeats them on every prepared packet so
/// no surface can read preparation as completion.
const NON_CLAIMS: [&str; 3] = [
    "RIPR does not author or apply the focused test edit",
    "prepared evidence does not mean the gap is fixed or verified",
    "this transaction does not authorize mutation execution or merge",
];

/// One session repair transaction. Immutable once created: replay returns
/// the identical document, and no path mutates the state back to an earlier
/// value.
#[derive(Clone, Debug)]
pub(crate) struct RepairTransaction {
    pub(crate) attempt_id: String,
    pub(crate) snapshot_id: String,
    pub(crate) canonical_id: String,
    pub(crate) root_identity: Option<String>,
    pub(crate) created_unix_ms: u64,
    /// The exact bounded packet document `ripr_prepare_repair` returned at
    /// creation; replays return these identical bytes.
    pub(crate) packet: Value,
}

/// Extract the attempt id from a `ripr://repair-attempt/{attempt_id}` URI.
pub(crate) fn repair_attempt_resource_id(uri: &str) -> Option<&str> {
    uri.strip_prefix("ripr://repair-attempt/")
        .filter(|id| !id.is_empty() && !id.contains('/'))
}

/// Extract the receipt id from a `ripr://receipt/{receipt_id}` URI. Receipt
/// ids are attempt-bound: one retained receipt per durable attempt identity.
pub(crate) fn receipt_resource_id(uri: &str) -> Option<&str> {
    uri.strip_prefix("ripr://receipt/")
        .filter(|id| !id.is_empty() && !id.contains('/'))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn current_unix_ms() -> Result<u64, AttemptFailure> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .map_err(|_error| {
            AttemptFailure::new(
                super::workspace::CODE_ANALYSIS_FAILED,
                "the system clock predates the Unix epoch; no attempt identity can be bound",
                "retry ripr_prepare_repair once the clock is sane",
            )
        })
}

/// Deterministic, root-bound session attempt identity: the same current
/// snapshot, item, and root always prepare the same `repair-attempt-` id, so
/// a repeated prepare is a replay, never a second transaction. An attempt
/// prepared under root A cannot collide with root B's identity because the
/// host-local root identity is part of the digest input.
fn session_attempt_id(
    snapshot_id: &str,
    canonical_id: &str,
    root_identity: Option<&str>,
) -> Result<String, AttemptFailure> {
    let payload = json!({
        "schema": REPAIR_ATTEMPT_SCHEMA_VERSION,
        "snapshot_id": snapshot_id,
        "canonical_id": canonical_id,
        "root_identity": root_identity.unwrap_or("root:unavailable"),
    });
    let bytes = serde_json::to_vec(&payload).map_err(|error| {
        AttemptFailure::new(
            super::workspace::CODE_ANALYSIS_FAILED,
            format!("serialize attempt identity: {error}"),
            "retry ripr_prepare_repair",
        )
    })?;
    let digest = sha256_hex(&bytes);
    let suffix = digest.get(..ATTEMPT_ID_HEX_LEN).ok_or_else(|| {
        AttemptFailure::new(
            super::workspace::CODE_ANALYSIS_FAILED,
            "attempt identity digest shorter than its grammar allows",
            "retry ripr_prepare_repair",
        )
    })?;
    Ok(format!("{ATTEMPT_ID_PREFIX}{suffix}"))
}

/// Project one accepted product [`crate::domain::CommandSpec`] onto the MCP
/// wire exactly: every field the issue contract names travels under its
/// producer name, and the human display string is explicitly marked as
/// never-executable authority. MCP does not tokenize a display string and
/// does not reconstruct `program`/`args` from prose.
pub(crate) fn command_spec_document(spec: &crate::domain::CommandSpec) -> Value {
    let mut document = serde_json::to_value(spec).unwrap_or_else(|_error| {
        // CommandSpec is Serialize by contract; a serialization failure here
        // is an instrument problem. The bounded adapter must not panic, so a
        // minimal honest marker keeps the wire alive without fabricating
        // route fields.
        json!({ "command_id": spec.command_id.clone() })
    });
    let directness = match spec.execution_mode {
        crate::domain::CommandExecutionMode::Direct => "direct",
        crate::domain::CommandExecutionMode::ShellRequired
        | crate::domain::CommandExecutionMode::Manual => "visibly_non_direct",
    };
    if let Some(object) = document.as_object_mut() {
        object.insert("directness".to_string(), Value::from(directness));
        object.insert(
            "display_is_execution_authority".to_string(),
            Value::from(false),
        );
    }
    document
}

/// Derive the wire status for one validated terminal receipt document. The
/// producer's actual movement is read before the lossy presence lifecycle:
/// a `regressed` movement must surface as `regressed`, never collapse into
/// the shared fail-closed `receipt_missing` bucket and read as
/// `after_pending` (review finding on #5155).
fn terminal_receipt_status(value: &Value) -> &'static str {
    let movement_regressed = value
        .pointer("/provenance/movement")
        .or(value.pointer("/static_movement/state"))
        .and_then(Value::as_str)
        .is_some_and(|movement| movement.trim().eq_ignore_ascii_case("regressed"));
    if movement_regressed {
        return "regressed";
    }
    receipt_status_from_lifecycle(&receipt_lifecycle_state_from_receipt_value(value))
}

/// Map the shared receipt-lifecycle vocabulary onto the receipt-status
/// vocabulary this slice exposes. Unrecognized producer states stay
/// non-claimed (`limited`), never an affirmative status.
fn receipt_status_from_lifecycle(normalized: &str) -> &'static str {
    use crate::output::receipt_lifecycle as lifecycle;
    match normalized {
        lifecycle::RECEIPT_MOVEMENT_IMPROVED => "improved",
        lifecycle::RECEIPT_MOVEMENT_UNCHANGED => "unchanged",
        lifecycle::RECEIPT_FOUND => "closed",
        lifecycle::RECEIPT_STALE => "stale",
        lifecycle::RECEIPT_GAP_MISMATCH => "invalid",
        lifecycle::RECEIPT_MISSING => "after_pending",
        lifecycle::RECEIPT_NOT_APPLICABLE => "limited",
        "regressed" => "regressed",
        _other => "limited",
    }
}

/// Typed CommandSpec routes carried by a durable attempt's retained packet,
/// projected through [`command_spec_document`]. Absence (legacy packets) is
/// normal and yields an empty route list plus one honest limitation; a
/// packet whose `command_specs` block does not validate as typed
/// `CommandSpec` values is omitted, never reconstructed from its display.
fn durable_command_routes(
    root: &Path,
    manifest: &crate::app::repair_attempt::RepairAttemptManifest,
) -> (Vec<Value>, Vec<String>) {
    let mut routes = Vec::new();
    let mut limitations = Vec::new();
    let Some(artifact) = find_manifest_artifact_by_role(manifest, "agent_packet") else {
        return (routes, limitations);
    };
    let path = root.join(&artifact.path);
    let Ok(bytes) = std::fs::read(&path) else {
        limitations.push(format!(
            "the retained packet {} could not be read; command routes stay unprojected",
            artifact.path
        ));
        return (routes, limitations);
    };
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        limitations
            .push("the retained packet is not JSON; command routes stay unprojected".to_string());
        return (routes, limitations);
    };
    let specs = value
        .pointer("/command_specs")
        .or_else(|| value.pointer("/packets/0/command_specs"));
    let Some(specs) = specs else {
        return (routes, limitations);
    };
    for (role, expected) in [
        ("verify", crate::domain::CommandRole::Verify),
        ("receipt", crate::domain::CommandRole::Receipt),
    ] {
        let Some(raw) = specs.get(role) else {
            continue;
        };
        let parsed = serde_json::from_value::<crate::domain::CommandSpec>(raw.clone());
        match parsed {
            Ok(spec) if spec.role == expected && spec.validate().is_ok() => {
                let mut document = command_spec_document(&spec);
                if let Some(object) = document.as_object_mut() {
                    object.insert("route_role".to_string(), Value::from(role));
                }
                routes.push(document);
            }
            _ok_or_err => {
                limitations.push(format!(
                    "the retained packet's {role} route is not a valid typed CommandSpec and stays unprojected"
                ));
            }
        }
    }
    (routes, limitations)
}

fn session_attempt_document(transaction: &RepairTransaction) -> Value {
    json!({
        "schema_version": REPAIR_ATTEMPT_SCHEMA_VERSION,
        "attempt_id": transaction.attempt_id,
        "origin": "mcp_session",
        "state": "awaiting_edit",
        "snapshot_id": transaction.snapshot_id,
        "canonical_id": transaction.canonical_id,
        "root_identity": transaction.root_identity,
        "created_unix_ms": transaction.created_unix_ms,
        "packet": transaction.packet.clone(),
        "links": {
            "repair_attempt": format!("ripr://repair-attempt/{}", transaction.attempt_id),
            "receipt": format!("ripr://receipt/{}", transaction.attempt_id),
        },
        "claim_boundary": "One session repair transaction bound to one committed snapshot and one canonical item. The server prepared evidence only: it never edits source, never launches a process, and never executes verification or mutation commands.",
        "limitations": [
            "the transaction is in-memory: restarting the server drops it; the durable CLI before phase (`ripr agent repair --phase before`) owns cross-session attempts",
            "command routes stay unprojected until a producer publishes typed CommandSpec routes on a durable packet",
        ],
    })
}

fn durable_attempt_document(
    manifest: &crate::app::repair_attempt::RepairAttemptManifest,
    root: &Path,
    root_identity: Option<&str>,
) -> Value {
    let (command_routes, route_limitations) = durable_command_routes(root, manifest);
    let terminal_receipt = match &manifest.state {
        RepairAttemptState::ReadyToFinish => match load_attempt_terminal_receipt(root, manifest) {
            AttemptTerminalReceipt::Issued { .. } => "issued",
            AttemptTerminalReceipt::Unavailable { .. } => "unavailable",
            AttemptTerminalReceipt::NotRetained => "not_retained",
        },
        _other => "not_applicable",
    };
    let after = manifest.after.as_ref().map(|after| {
        json!({
            "attempt_id": after.attempt_id.as_str(),
            "repository_head": after.repository_head,
            "delta_sha256": after.delta_sha256,
            "packet_sha256": after.packet_sha256,
            "current": after.current,
            "edit_cage_verdict": serde_json::to_value(&after.verdict).unwrap_or(Value::Null),
        })
    });
    let mut limitations = manifest.limitations.clone();
    limitations.extend(route_limitations);
    json!({
        "schema_version": REPAIR_ATTEMPT_SCHEMA_VERSION,
        "attempt_id": manifest.repair_attempt_id.as_str(),
        "origin": "durable_store",
        "state": repair_attempt_state_label(&manifest.state),
        "root_identity": root_identity,
        "repository_head": manifest.repository_head,
        "seam_id": manifest.seam_id,
        "producer_version": manifest.producer_version,
        "created_unix_ms": manifest.created_unix_ms,
        "artifacts": manifest.artifacts.iter().map(|artifact| json!({
            "role": artifact.role,
            "path": artifact.path,
            "sha256": artifact.sha256,
            "bytes": artifact.bytes,
        })).collect::<Vec<_>>(),
        "next_command": manifest.next_command,
        "after": after,
        "terminal_receipt": terminal_receipt,
        "command_routes": command_routes,
        "limitations": limitations,
        "non_claims": manifest.non_claims,
        "links": {
            "receipt": format!("ripr://receipt/{}", manifest.repair_attempt_id.as_str()),
        },
        "claim_boundary": "Read-only projection of one durable repair attempt from this session's root store, loaded and digest-validated by the shared repair-attempt authority on every read. MCP did not create it and executes nothing it names; the host-local root path is intentionally not projected.",
    })
}

fn unknown_attempt(attempt_id: &str, root_present: bool) -> AttemptFailure {
    let (detail, recovery) = if root_present {
        (
            format!(
                "no repair attempt or receipt `{attempt_id}` exists in this session or in the durable store of this workspace root"
            ),
            "prepare the item with ripr_prepare_repair, or list durable attempts with `ripr agent status` in the repository",
        )
    } else {
        (
            format!(
                "no repair attempt or receipt `{attempt_id}` exists in this session, and the workspace root is unavailable so the durable store cannot be read"
            ),
            "restart the server with `ripr mcp --stdio --root <repository>` and retry",
        )
    };
    AttemptFailure::new(CODE_ATTEMPT_NOT_FOUND, detail, recovery)
}

/// Look up one durable attempt manifest by identity, validating it the same
/// way the after phase would. A manifest that fails canonical validation is
/// reported as `attempt_invalid`, never projected as if it were valid.
fn load_durable_attempt(
    root: &Path,
    attempt_id: &str,
) -> Result<crate::app::repair_attempt::RepairAttemptManifest, AttemptFailure> {
    let inventory = inventory_repair_attempts_from(root, None).map_err(|error| {
        AttemptFailure::new(
            CODE_ATTEMPT_INVALID,
            format!("the durable attempt store could not be inventoried: {error}"),
            "run `ripr agent status` in the repository for the full diagnostic",
        )
    })?;
    for entry in inventory {
        match entry {
            RepairAttemptInventoryEntry::Valid(manifest)
                if manifest.repair_attempt_id.as_str() == attempt_id =>
            {
                return Ok(*manifest);
            }
            RepairAttemptInventoryEntry::Invalid { directory, .. } if directory == attempt_id => {
                return Err(AttemptFailure::new(
                    CODE_ATTEMPT_INVALID,
                    format!("repair attempt `{attempt_id}` failed canonical validation"),
                    "run `ripr agent status` in the repository for the refusal detail",
                ));
            }
            _other => {}
        }
    }
    Err(AttemptFailure::new(
        CODE_ATTEMPT_NOT_FOUND,
        format!(
            "no repair attempt `{attempt_id}` exists in the durable store of this workspace root"
        ),
        "list durable attempts with `ripr agent status` in the repository",
    ))
}

fn session_receipt_document(transaction: &RepairTransaction) -> Value {
    json!({
        "schema_version": RECEIPT_STATUS_SCHEMA_VERSION,
        "receipt_id": transaction.attempt_id,
        "status": "awaiting_edit",
        "attempt": {
            "attempt_id": transaction.attempt_id,
            "state": "awaiting_edit",
            "snapshot_id": transaction.snapshot_id,
            "canonical_id": transaction.canonical_id,
            "root_identity": transaction.root_identity,
        },
        "receipt": Value::Null,
        "currentness": {
            "attempt_state": "awaiting_edit",
            "basis": "session transaction bound to its committed snapshot; a refresh supersedes the snapshot and with it every transaction prepared against it",
        },
        "limitations": [
            "RIPR performs no verification and issues no receipt: the external client owns the edit, the verification execution, and the receipt under its own authority",
            "static movement and focused runtime test execution remain separate evidence axes; this document reports static transaction state only",
        ],
        "non_claims": NON_CLAIMS,
        "claim_boundary": "Receipt state for one session repair transaction. Receipt issuance is external authority; this read-only projection can never upgrade a transaction it did not verify.",
        "links": {
            "repair_attempt": format!("ripr://repair-attempt/{}", transaction.attempt_id),
        },
    })
}

fn durable_receipt_document(
    manifest: &crate::app::repair_attempt::RepairAttemptManifest,
    root: &Path,
    root_identity: Option<&str>,
) -> Value {
    let state_label = repair_attempt_state_label(&manifest.state);
    let attempt_id = manifest.repair_attempt_id.as_str().to_string();
    let (status, receipt) = match &manifest.state {
        RepairAttemptState::Prepared | RepairAttemptState::AwaitingEdit => {
            let receipt = json!({
                "status": "not_issued",
                "note": "the attempt awaits the external focused-test edit; RIPR performs no verification",
            });
            ("awaiting_edit", receipt)
        }
        RepairAttemptState::Stale => {
            let receipt = json!({
                "status": "not_issued",
                "note": "the durable attempt is stale: its before evidence no longer compares against the current analysis input",
            });
            ("stale", receipt)
        }
        RepairAttemptState::Incomparable => {
            let receipt = json!({
                "status": "not_issued",
                "note": "the durable attempt is incomparable: its analysis inputs drifted, so no honest movement state exists",
            });
            ("limited", receipt)
        }
        RepairAttemptState::Failed => {
            let receipt = json!({
                "status": "not_issued",
                "note": "the durable attempt failed; no receipt state can be claimed",
            });
            ("invalid", receipt)
        }
        RepairAttemptState::ReadyToFinish => match load_attempt_terminal_receipt(root, manifest) {
            AttemptTerminalReceipt::NotRetained => {
                let receipt = json!({
                    "status": "not_retained",
                    "note": "the after phase finished but no digest-bound terminal receipt is retained; issue it with `ripr agent receipt --attempt <id>` in the repository",
                });
                ("verification_pending", receipt)
            }
            AttemptTerminalReceipt::Unavailable { path, reason } => {
                let receipt = json!({
                    "status": "unavailable",
                    "path": path,
                    "reason": reason,
                });
                ("invalid", receipt)
            }
            AttemptTerminalReceipt::Issued { path, value } => {
                // The producer's actual movement is read before the lossy
                // presence lifecycle: a `regressed` movement must surface as
                // `regressed`, never collapse into the shared fail-closed
                // `receipt_missing` bucket and read as `after_pending`.
                let lifecycle_state = receipt_lifecycle_state_from_receipt_value(&value);
                let status = terminal_receipt_status(&value);
                let binding = find_terminal_artifact_by_role(
                    manifest,
                    crate::app::repair_attempt::TERMINAL_RECEIPT_ROLE,
                )
                .map(|artifact| {
                    json!({
                        "path": artifact.path,
                        "sha256": artifact.sha256,
                        "bytes": artifact.bytes,
                    })
                })
                .unwrap_or(Value::Null);
                let receipt = json!({
                    "status": "issued",
                    "path": path,
                    "binding": binding,
                    "lifecycle_state": lifecycle_state,
                    "document": value,
                });
                (status, receipt)
            }
        },
    };
    json!({
        "schema_version": RECEIPT_STATUS_SCHEMA_VERSION,
        "receipt_id": attempt_id,
        "status": status,
        "attempt": {
            "attempt_id": attempt_id,
            "state": state_label,
            "root_identity": root_identity,
            "repository_head": manifest.repository_head,
            "seam_id": manifest.seam_id,
        },
        "receipt": receipt,
        "currentness": {
            "attempt_state": state_label,
            "after_current": manifest.after.as_ref().map(|after| after.current),
            "basis": "durable manifest and digest-bound terminal artifacts, re-validated by the shared repair-attempt authority on every read",
        },
        "limitations": [
            "static movement and focused runtime test execution remain separate evidence axes; this document reports static receipt state only",
            "RIPR executed nothing to produce this state and executes nothing in response to reading it",
        ],
        "non_claims": NON_CLAIMS,
        "claim_boundary": "Receipt status for one durable repair attempt. The receipt binds exact before/after/verify bytes under the attempt identity; MCP re-validates those bindings on every read and projects the state without joining by mtime or latest-file convention.",
        "links": {
            "repair_attempt": format!("ripr://repair-attempt/{attempt_id}"),
        },
    })
}

impl WorkspaceSession {
    /// The live session transaction for one canonical item of one committed
    /// snapshot, if prepare created one.
    pub(crate) fn live_repair_attempt(
        &self,
        snapshot_id: &str,
        canonical_id: &str,
    ) -> Option<&str> {
        self.repairs.values().find_map(|transaction| {
            (transaction.snapshot_id == snapshot_id && transaction.canonical_id == canonical_id)
                .then_some(transaction.attempt_id.as_str())
        })
    }

    /// `ripr_prepare_repair`: evaluate the committed item's producer
    /// repair-readiness facts and, only when every gate is established,
    /// create (or replay) one bounded session repair transaction. An
    /// ineligible item returns an honest negative document with
    /// `repair_packet_ready: false` and no attempt — never a fabricated
    /// field and never a misleading transaction.
    pub(crate) fn prepare_repair(
        &mut self,
        gap_id: &str,
        requested: Option<&str>,
        root_identity: Option<&str>,
    ) -> Result<Value, AttemptFailure> {
        let snapshot = self.active_snapshot(requested)?;
        let snapshot_id = snapshot.snapshot_id.clone();
        let Some(item) = snapshot.item(gap_id) else {
            return Err(AttemptFailure::new(
                CODE_ITEM_NOT_FOUND,
                format!("no canonical item {gap_id} exists in the current snapshot"),
                "list the current canonical ids with ripr_list_gaps, then retry",
            ));
        };
        let readiness = &item.repair_readiness;
        if !readiness.ready {
            let reason = readiness.ineligibility.unwrap_or("repair_not_established");
            let document = json!({
                "schema_version": REPAIR_PACKET_SCHEMA_VERSION,
                "snapshot_id": snapshot_id,
                "requested_snapshot_id": requested,
                "item": {
                    "canonical_id": item.canonical_id,
                    "finding_id": item.finding_id,
                },
                "repair_packet_ready": false,
                "ineligibility": {
                    "reason": reason,
                    "detail": readiness.reason(),
                },
                "attempt": Value::Null,
                "recovery": "re-run ripr_refresh after the producer establishes the missing fact; ripr_prepare_repair never guesses a missing field",
                "limitations": [
                    "a route that lacks a safe target, discriminator, fix site, or command stays a typed limitation",
                    "no attempt was created: the tool must not create a misleading transaction",
                ],
                "claim_boundary": "Repair-packet evaluation for one canonical item. This negative document authorizes nothing and creates nothing.",
            });
            return bounded_document(document);
        }

        // Ready: replay the identical transaction when one exists for this
        // snapshot, item, and root; otherwise create it once.
        let attempt_id = session_attempt_id(&snapshot_id, &item.canonical_id, root_identity)?;
        if let Some(transaction) = self.repairs.get(&attempt_id) {
            return bounded_document(transaction.packet.clone());
        }
        let Some(fix_site) = readiness.fix_site.clone() else {
            // `ready` implies a fix site; keep the fail-closed shape instead
            // of unwrapping so a projection bug becomes a typed failure.
            return Err(AttemptFailure::new(
                CODE_ATTEMPT_INVALID,
                "the readiness projection reported ready without an established fix site",
                "re-run ripr_refresh and retry ripr_prepare_repair",
            ));
        };
        let created_unix_ms = current_unix_ms()?;
        let transaction = RepairTransaction {
            attempt_id: attempt_id.clone(),
            snapshot_id: snapshot_id.clone(),
            canonical_id: item.canonical_id.clone(),
            root_identity: root_identity.map(str::to_string),
            created_unix_ms,
            packet: Value::Null,
        };
        let changed_behavior = item
            .evidence_core
            .pointer("/changed_behavior")
            .cloned()
            .unwrap_or(Value::Null);
        let packet = json!({
            "schema_version": REPAIR_PACKET_SCHEMA_VERSION,
            "snapshot_id": snapshot_id,
            "requested_snapshot_id": requested,
            "repair_packet_ready": true,
            "attempt": {
                "attempt_id": attempt_id,
                "state": "awaiting_edit",
                "origin": "mcp_session",
                "root_identity": root_identity,
                "snapshot_id": snapshot_id,
                "canonical_id": item.canonical_id,
                "finding_id": item.finding_id,
                "created_unix_ms": created_unix_ms,
                "replay_resistant": "repeating prepare for the same current snapshot, item, and root returns these identical bytes and never creates a second transaction",
            },
            "changed_behavior": changed_behavior,
            "discriminator": item.evidence_core.pointer("/causal_attribution/normalized_discriminator").cloned().unwrap_or(Value::Null),
            "fix_site": {
                "test_name": fix_site.test_name,
                "file": fix_site.file,
                "line": fix_site.line,
                "oracle": fix_site.oracle,
                "oracle_kind": fix_site.oracle_kind,
                "established_by": "the strongest directly-related producer test grip (strong oracle, high-confidence direct relation) on a shared test-surface path",
            },
            "allowed_edit_surface": [fix_site_file(item)],
            "must_not_change": [
                crate::output::agent_seam_packets::EDIT_CAGE_PRODUCTION_STATEMENT,
                crate::output::agent_seam_packets::EDIT_CAGE_TERMINALITY_WARNING,
            ],
            "stop_conditions": [
                "stop if the focused test edit requires changing any file outside allowed_edit_surface",
                "stop if the repository moves away from the prepared snapshot; re-run ripr_refresh and prepare a fresh transaction",
            ],
            "before_evidence": {
                "snapshot_id": snapshot_id,
                "evidence_sha256": item.evidence_sha256,
            },
            "command_routes": Value::Array(Vec::new()),
            "command_route_limitation": "concrete typed CommandSpec routes are published only by the durable CLI before phase (`ripr agent repair --phase before`); this session never executes commands and never reconstructs argv from display prose",
            "limitations": [
                "the seam-pipeline target-admission authority has not run in this session; the fix site is the strongest producer test grip, not an admitted seam target",
                "the transaction is in-memory: restarting the server drops it; durable attempts survive under the CLI repair workflow and are readable here with ripr_get_repair_attempt",
            ],
            "non_claims": NON_CLAIMS,
            "links": {
                "gap": format!("ripr://gap/{}", item.canonical_id),
                "snapshot": format!("ripr://snapshot/{snapshot_id}"),
                "repair_attempt": format!("ripr://repair-attempt/{attempt_id}"),
                "receipt": format!("ripr://receipt/{attempt_id}"),
            },
            "claim_boundary": "One bounded repair transaction for one canonical item of one committed snapshot. RIPR prepared evidence only: it does not edit source, does not launch a process, and does not execute verification or mutation commands. The external client's approval and sandbox policy remains authoritative for every command.",
        });
        let packet = bounded_document(packet)?;
        let transaction = RepairTransaction {
            packet: packet.clone(),
            ..transaction
        };
        // Bound the in-memory map: transactions bound to an older snapshot can
        // never be served as live again, so evict them (one bounded packet
        // each, up to 128 KiB) and keep only an attempt_id → snapshot_id
        // tombstone that preserves the typed `superseded` failure.
        let mut evicted = Vec::new();
        self.repairs.retain(|id, existing| {
            if existing.snapshot_id != snapshot_id {
                evicted.push((id.clone(), existing.snapshot_id.clone()));
                false
            } else {
                true
            }
        });
        self.repairs.insert(attempt_id, transaction);
        for (id, old_snapshot) in evicted {
            self.superseded_attempts.insert(id, old_snapshot);
        }
        Ok(packet)
    }

    /// `ripr_get_repair_attempt` / `ripr://repair-attempt/{attempt_id}`:
    /// session transactions first, then the durable store of this session's
    /// root, each fail-closed and replay-resistant.
    pub(crate) fn repair_attempt_document(
        &self,
        attempt_id: &str,
        root: Option<&Path>,
        root_identity: Option<&str>,
    ) -> Result<Value, AttemptFailure> {
        if self.in_flight {
            return Err(AttemptFailure::new(
                super::workspace::CODE_ANALYSIS_IN_FLIGHT,
                "an analysis attempt is running",
                "poll ripr_workspace_status until attempt_state leaves in_flight, then retry",
            ));
        }
        if let Some(transaction) = self.repairs.get(attempt_id) {
            let current = self
                .last_good
                .as_ref()
                .map(|snapshot| snapshot.snapshot_id.as_str());
            if current != Some(transaction.snapshot_id.as_str()) {
                return Err(AttemptFailure::new(
                    "superseded",
                    format!(
                        "repair attempt `{attempt_id}` was prepared against snapshot `{}`, which is no longer the current completed snapshot",
                        transaction.snapshot_id
                    ),
                    "re-read ripr_workspace_status for the current snapshot identity and prepare a fresh transaction"
                )
                .with_data(json!({ "current_snapshot_id": current })));
            }
            return bounded_document(session_attempt_document(transaction));
        }
        if let Some(old_snapshot) = self.superseded_attempts.get(attempt_id) {
            let current = self
                .last_good
                .as_ref()
                .map(|snapshot| snapshot.snapshot_id.as_str());
            return Err(AttemptFailure::new(
                "superseded",
                format!(
                    "repair attempt `{attempt_id}` was prepared against snapshot `{old_snapshot}`, which is no longer the current completed snapshot"
                ),
                "re-read ripr_workspace_status for the current snapshot identity and prepare a fresh transaction"
            )
            .with_data(json!({ "current_snapshot_id": current })));
        }
        let Some(root) = root else {
            return Err(unknown_attempt(attempt_id, false));
        };
        let manifest = load_durable_attempt(root, attempt_id)?;
        bounded_document(durable_attempt_document(&manifest, root, root_identity))
    }

    /// `ripr_get_receipt_status` / `ripr://receipt/{receipt_id}`: current
    /// receipt state for one attempt identity, without joining by mtime or
    /// latest-file convention and without executing anything.
    pub(crate) fn receipt_status_document(
        &self,
        receipt_id: &str,
        root: Option<&Path>,
        root_identity: Option<&str>,
    ) -> Result<Value, AttemptFailure> {
        if self.in_flight {
            return Err(AttemptFailure::new(
                super::workspace::CODE_ANALYSIS_IN_FLIGHT,
                "an analysis attempt is running",
                "poll ripr_workspace_status until attempt_state leaves in_flight, then retry",
            ));
        }
        if let Some(transaction) = self.repairs.get(receipt_id) {
            let current = self
                .last_good
                .as_ref()
                .map(|snapshot| snapshot.snapshot_id.as_str());
            if current != Some(transaction.snapshot_id.as_str()) {
                return Err(AttemptFailure::new(
                    "superseded",
                    format!(
                        "receipt `{receipt_id}` was bound to snapshot `{}`, which is no longer the current completed snapshot",
                        transaction.snapshot_id
                    ),
                    "re-read ripr_workspace_status for the current snapshot identity"
                )
                .with_data(json!({ "current_snapshot_id": current })));
            }
            return bounded_document(session_receipt_document(transaction));
        }
        if let Some(old_snapshot) = self.superseded_attempts.get(receipt_id) {
            let current = self
                .last_good
                .as_ref()
                .map(|snapshot| snapshot.snapshot_id.as_str());
            return Err(AttemptFailure::new(
                "superseded",
                format!(
                    "receipt `{receipt_id}` was bound to snapshot `{old_snapshot}`, which is no longer the current completed snapshot"
                ),
                "re-read ripr_workspace_status for the current snapshot identity"
            )
            .with_data(json!({ "current_snapshot_id": current })));
        }
        let Some(root) = root else {
            return Err(unknown_attempt(receipt_id, false));
        };
        let manifest = load_durable_attempt(root, receipt_id)?;
        bounded_document(durable_receipt_document(&manifest, root, root_identity))
    }
}

/// The allowed edit surface file of one ready item; the readiness gate
/// already established that the fix site is a shared test-surface path.
fn fix_site_file(item: &GapItem) -> String {
    item.repair_readiness
        .fix_site
        .as_ref()
        .map(|site| site.file.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::super::workspace::CODE_STALE_SNAPSHOT;
    use super::*;
    use crate::analysis_outcome::{AnalysisOutcome, AnalysisOutcomeCounts, AnalysisOutcomeKind};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn output(
        kind: AnalysisOutcomeKind,
        findings: &[crate::domain::Finding],
    ) -> Result<crate::app::CheckOutput, String> {
        let outcome = AnalysisOutcome::new(
            kind,
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

    fn session_with(
        findings: &[crate::domain::Finding],
        root_identity: &str,
    ) -> Result<WorkspaceSession, String> {
        let output = output(AnalysisOutcomeKind::CompleteWithFindings, findings)?;
        let snapshot = super::super::workspace::Snapshot::from_output(&output, Some(root_identity))
            .map_err(|failure| failure.detail)?;
        Ok(WorkspaceSession {
            in_flight: false,
            last_good: Some(Arc::new(snapshot)),
            last_failure: None,
            repairs: std::collections::BTreeMap::new(),
            superseded_attempts: std::collections::BTreeMap::new(),
        })
    }

    fn unique_test_dir(name: &str) -> Result<PathBuf, String> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("clock: {error}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!("ripr-mcp-repair-{name}-{nanos}")))
    }

    #[test]
    fn complete_route_creates_one_replay_resistant_attempt() -> Result<(), String> {
        let mut session = session_with(&[super::super::gaps::test_finding()?], "root:sha256:a")?;
        let first = session
            .prepare_repair("gap:test:1", None, Some("root:sha256:a"))
            .map_err(|failure| failure.detail)?;
        if first
            .pointer("/repair_packet_ready")
            .and_then(Value::as_bool)
            != Some(true)
        {
            return Err(format!("the complete route must prepare a packet: {first}"));
        }
        let attempt_id = first
            .pointer("/attempt/attempt_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "the prepared packet lost its attempt identity".to_string())?;
        if !attempt_id.starts_with("repair-attempt-") {
            return Err(format!(
                "attempt id drifted from the shared grammar: {attempt_id}"
            ));
        }
        if first.pointer("/attempt/state").and_then(Value::as_str) != Some("awaiting_edit") {
            return Err("a fresh transaction must await the external edit".to_string());
        }
        let surface = first
            .pointer("/allowed_edit_surface/0")
            .and_then(Value::as_str)
            .ok_or_else(|| "the ready packet lost its allowed edit surface".to_string())?;
        if surface != "tests/checkout.rs" {
            return Err(format!("allowed edit surface drifted: {surface}"));
        }
        let second = session
            .prepare_repair("gap:test:1", None, Some("root:sha256:a"))
            .map_err(|failure| failure.detail)?;
        if second != first {
            return Err("repeated prepare must replay the identical document".to_string());
        }
        if session.repairs.len() != 1 {
            return Err(format!(
                "repeated prepare must not create a second transaction: {}",
                session.repairs.len()
            ));
        }
        Ok(())
    }

    #[test]
    fn missing_producer_facts_never_create_a_misleading_attempt() -> Result<(), String> {
        let mut no_discriminator = super::super::gaps::test_finding()?;
        no_discriminator.canonical_gap = None;
        let mut session = session_with(&[no_discriminator], "root:sha256:a")?;
        let document = session
            .prepare_repair("finding:test:1", None, Some("root:sha256:a"))
            .map_err(|failure| failure.detail)?;
        if document
            .pointer("/repair_packet_ready")
            .and_then(Value::as_bool)
            != Some(false)
        {
            return Err(format!(
                "ineligible item must not prepare a packet: {document}"
            ));
        }
        if document.pointer("/attempt") != Some(&Value::Null) {
            return Err("ineligible item must not create an attempt".to_string());
        }
        if document
            .pointer("/ineligibility/reason")
            .and_then(Value::as_str)
            != Some("missing_discriminator")
        {
            return Err(format!("ineligibility lost its typed reason: {document}"));
        }
        if !session.repairs.is_empty() {
            return Err("no transaction may exist after an ineligible prepare".to_string());
        }
        Ok(())
    }

    #[test]
    fn stale_snapshot_and_unknown_items_fail_closed() -> Result<(), String> {
        let mut session = session_with(&[super::super::gaps::test_finding()?], "root:sha256:a")?;
        let current = session
            .last_good
            .as_ref()
            .ok_or_else(|| "missing snapshot".to_string())?
            .snapshot_id
            .clone();
        match session.prepare_repair("gap:test:1", Some("snapshot:sha256:old"), None) {
            Ok(value) => Err(format!("stale snapshot must fail closed: {value}")),
            Err(failure) if failure.code != CODE_STALE_SNAPSHOT => {
                Err(format!("unexpected failure code: {}", failure.code))
            }
            Err(failure) => {
                if failure
                    .data
                    .pointer("/current_snapshot_id")
                    .and_then(Value::as_str)
                    != Some(current.as_str())
                {
                    return Err(format!(
                        "stale prepare lost the current identity: {}",
                        failure.data
                    ));
                }
                Ok(())
            }
        }?;
        match session.prepare_repair("gap:missing", None, None) {
            Ok(value) => Err(format!("unknown item must fail closed: {value}")),
            Err(failure) if failure.code == CODE_ITEM_NOT_FOUND => Ok(()),
            Err(failure) => Err(format!("unexpected failure code: {}", failure.code)),
        }
    }

    #[test]
    fn attempt_identity_is_root_bound() -> Result<(), String> {
        let mut first = session_with(&[super::super::gaps::test_finding()?], "root:sha256:a")?;
        let mut second = session_with(&[super::super::gaps::test_finding()?], "root:sha256:b")?;
        let one = first
            .prepare_repair("gap:test:1", None, Some("root:sha256:a"))
            .map_err(|failure| failure.detail)?;
        let two = second
            .prepare_repair("gap:test:1", None, Some("root:sha256:b"))
            .map_err(|failure| failure.detail)?;
        let one_id = one
            .pointer("/attempt/attempt_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing attempt id".to_string())?;
        let two_id = two
            .pointer("/attempt/attempt_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing attempt id".to_string())?;
        if one_id == two_id {
            return Err(
                "an attempt prepared under root A must not share root B's identity".to_string(),
            );
        }
        // Root B's session cannot see root A's attempt.
        match second.repair_attempt_document(one_id, None, Some("root:sha256:b")) {
            Ok(value) => Err(format!("cross-root attempt read must fail closed: {value}")),
            Err(failure) if failure.code == CODE_ATTEMPT_NOT_FOUND => Ok(()),
            Err(failure) => Err(format!("unexpected failure code: {}", failure.code)),
        }
    }

    #[test]
    fn superseded_attempt_fails_closed_after_refresh() -> Result<(), String> {
        let mut session = session_with(&[super::super::gaps::test_finding()?], "root:sha256:a")?;
        let packet = session
            .prepare_repair("gap:test:1", None, Some("root:sha256:a"))
            .map_err(|failure| failure.detail)?;
        let attempt_id = packet
            .pointer("/attempt/attempt_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing attempt id".to_string())?
            .to_string();
        // Changed evidence changes the evidence digest and with it the
        // snapshot identity, so a refresh supersedes the old transaction's
        // binding.
        let mut changed = super::super::gaps::test_finding()?;
        changed
            .evidence
            .push("a later analysis pass adds one more evidence line".to_string());
        let refreshed = session_with(&[changed], "root:sha256:a")?;
        session.last_good = refreshed.last_good;
        match session.repair_attempt_document(&attempt_id, None, Some("root:sha256:a")) {
            Ok(value) => Err(format!("superseded attempt must fail closed: {value}")),
            Err(failure) if failure.code == "superseded" => Ok(()),
            Err(failure) => Err(format!("unexpected failure code: {}", failure.code)),
        }
    }

    #[test]
    fn receipt_status_for_session_attempt_awaits_the_external_edit() -> Result<(), String> {
        let mut session = session_with(&[super::super::gaps::test_finding()?], "root:sha256:a")?;
        let packet = session
            .prepare_repair("gap:test:1", None, Some("root:sha256:a"))
            .map_err(|failure| failure.detail)?;
        let attempt_id = packet
            .pointer("/attempt/attempt_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing attempt id".to_string())?;
        let status = session
            .receipt_status_document(attempt_id, None, Some("root:sha256:a"))
            .map_err(|failure| failure.detail)?;
        if status.pointer("/status").and_then(Value::as_str) != Some("awaiting_edit") {
            return Err(format!("session receipt status drifted: {status}"));
        }
        if status.pointer("/receipt").is_none() {
            return Err("receipt status lost its receipt block".to_string());
        }
        if status.pointer("/attempt/state").and_then(Value::as_str) != Some("awaiting_edit") {
            return Err("receipt status lost the attempt state".to_string());
        }
        Ok(())
    }

    #[test]
    fn unknown_and_invalid_durable_attempts_fail_closed() -> Result<(), String> {
        let root = unique_test_dir("durable-missing")?;
        std::fs::create_dir_all(&root).map_err(|error| format!("create root: {error}"))?;
        let session = WorkspaceSession::default();
        match session.repair_attempt_document(
            "repair-attempt-abcdef0123456789abcdef01",
            Some(&root),
            None,
        ) {
            Ok(value) => Err(format!("missing durable attempt must fail closed: {value}")),
            Err(failure) if failure.code == CODE_ATTEMPT_NOT_FOUND => Ok(()),
            Err(failure) => Err(format!("unexpected failure code: {}", failure.code)),
        }?;
        // A manifest that fails canonical validation reports attempt_invalid.
        let attempt_dir =
            root.join("target/ripr/repair-attempts/repair-attempt-abcdef0123456789abcdef02");
        std::fs::create_dir_all(&attempt_dir)
            .map_err(|error| format!("create attempt dir: {error}"))?;
        std::fs::write(attempt_dir.join("attempt.json"), "{not json")
            .map_err(|error| format!("write manifest: {error}"))?;
        match session.repair_attempt_document(
            "repair-attempt-abcdef0123456789abcdef02",
            Some(&root),
            None,
        ) {
            Ok(value) => Err(format!("invalid durable attempt must fail closed: {value}")),
            Err(failure) if failure.code == CODE_ATTEMPT_INVALID => Ok(()),
            Err(failure) => Err(format!("unexpected failure code: {}", failure.code)),
        }?;
        std::fs::remove_dir_all(&root).map_err(|error| format!("remove root: {error}"))?;
        Ok(())
    }

    #[test]
    fn receipt_status_vocabulary_maps_the_shared_lifecycle() {
        let cases = [
            ("receipt_movement_improved", "improved"),
            ("receipt_movement_unchanged", "unchanged"),
            ("receipt_found", "closed"),
            ("receipt_stale", "stale"),
            ("receipt_gap_mismatch", "invalid"),
            ("receipt_missing", "after_pending"),
            ("receipt_not_applicable", "limited"),
            ("regressed", "regressed"),
            ("future_unknown_state", "limited"),
        ];
        for (normalized, expected) in cases {
            assert_eq!(
                receipt_status_from_lifecycle(normalized),
                expected,
                "{normalized}"
            );
        }
    }

    #[test]
    fn regressed_movement_survives_the_presence_lifecycle() {
        // A real regressed receipt would collapse to receipt_missing through
        // the shared fail-closed movement mapping; the wire must still say
        // regressed (#5155 review).
        let regressed = json!({
            "provenance": { "movement": "regressed" },
            "summary": { "receipt_state": "receipt_found" }
        });
        assert_eq!(terminal_receipt_status(&regressed), "regressed");
        let regressed_alt = json!({ "static_movement": { "state": "regressed" } });
        assert_eq!(terminal_receipt_status(&regressed_alt), "regressed");
        let improved = json!({ "provenance": { "movement": "improved" } });
        assert_eq!(terminal_receipt_status(&improved), "improved");
        let absent = json!({ "provenance": { "movement": "" } });
        assert_eq!(terminal_receipt_status(&absent), "after_pending");
    }

    #[test]
    fn repair_reads_fail_closed_while_refresh_is_in_flight() -> Result<(), String> {
        let mut session = session_with(&[super::super::gaps::test_finding()?], "root:sha256:a")?;
        let packet = session
            .prepare_repair("gap:test:1", None, Some("root:sha256:a"))
            .map_err(|failure| failure.detail)?;
        let attempt_id = packet
            .pointer("/attempt/attempt_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing attempt id".to_string())?
            .to_string();
        session.in_flight = true;
        match session.repair_attempt_document(&attempt_id, None, Some("root:sha256:a")) {
            Ok(value) => Err(format!("in-flight attempt read must fail closed: {value}")),
            Err(failure) if failure.code == crate::mcp::workspace::CODE_ANALYSIS_IN_FLIGHT => {
                Ok(())
            }
            Err(failure) => Err(format!("unexpected failure code: {}", failure.code)),
        }?;
        match session.receipt_status_document(&attempt_id, None, Some("root:sha256:a")) {
            Ok(value) => Err(format!("in-flight receipt read must fail closed: {value}")),
            Err(failure) if failure.code == crate::mcp::workspace::CODE_ANALYSIS_IN_FLIGHT => {
                Ok(())
            }
            Err(failure) => Err(format!("unexpected failure code: {}", failure.code)),
        }?;
        Ok(())
    }

    #[test]
    fn evicted_transactions_keep_the_superseded_typed_code() -> Result<(), String> {
        let mut session = session_with(&[super::super::gaps::test_finding()?], "root:sha256:a")?;
        let first = session
            .prepare_repair("gap:test:1", None, Some("root:sha256:a"))
            .map_err(|failure| failure.detail)?;
        let first_id = first
            .pointer("/attempt/attempt_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing attempt id".to_string())?
            .to_string();
        // A refresh that changes the evidence supersedes the first binding;
        // preparing against the new snapshot evicts the old packet.
        let mut changed = super::super::gaps::test_finding()?;
        changed
            .evidence
            .push("a later analysis pass adds one more evidence line".to_string());
        let refreshed = session_with(&[changed], "root:sha256:a")?;
        session.last_good = refreshed.last_good;
        let second = session
            .prepare_repair("gap:test:1", None, Some("root:sha256:a"))
            .map_err(|failure| failure.detail)?;
        let second_id = second
            .pointer("/attempt/attempt_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing attempt id".to_string())?
            .to_string();
        if second_id == first_id {
            return Err("a new snapshot must bind a new attempt identity".to_string());
        }
        if session.repairs.contains_key(&first_id) {
            return Err("the superseded packet must not stay retained in memory".to_string());
        }
        match session.repair_attempt_document(&first_id, None, Some("root:sha256:a")) {
            Ok(value) => Err(format!("evicted attempt must stay superseded: {value}")),
            Err(failure) if failure.code == "superseded" => Ok(()),
            Err(failure) => Err(format!("unexpected failure code: {}", failure.code)),
        }?;
        match session.receipt_status_document(&first_id, None, Some("root:sha256:a")) {
            Ok(value) => Err(format!("evicted receipt must stay superseded: {value}")),
            Err(failure) if failure.code == "superseded" => Ok(()),
            Err(failure) => Err(format!("unexpected failure code: {}", failure.code)),
        }?;
        // The live transaction still reads normally.
        session
            .repair_attempt_document(&second_id, None, Some("root:sha256:a"))
            .map_err(|failure| failure.detail)?;
        Ok(())
    }

    #[test]
    fn command_spec_modes_round_trip_without_display_authority() -> Result<(), String> {
        let direct = crate::agent::command_specs::agent_verify_command_spec(
            ".",
            "target/ripr/workflow/before.json",
            "target/ripr/workflow/after.json",
            None,
        );
        let direct_doc = command_spec_document(&direct);
        if direct_doc
            .pointer("/execution_mode")
            .and_then(Value::as_str)
            != Some("direct")
        {
            return Err(format!("direct mode drifted: {direct_doc}"));
        }
        if direct_doc.pointer("/directness").and_then(Value::as_str) != Some("direct") {
            return Err("the direct route must stay visibly direct".to_string());
        }
        let shell_required = crate::agent::command_specs::agent_regeneration_command_spec(
            crate::agent::command_specs::AgentArtifactRoute::Packet,
            ".",
            "seam-a",
            "target/out.json",
        );
        let shell_doc = command_spec_document(&shell_required);
        if shell_doc.pointer("/execution_mode").and_then(Value::as_str) != Some("shell_required")
            || shell_doc.pointer("/directness").and_then(Value::as_str)
                != Some("visibly_non_direct")
        {
            return Err(format!(
                "shell-required mode must stay visibly non-direct: {shell_doc}"
            ));
        }
        let manual = crate::domain::CommandSpec {
            execution_mode: crate::domain::CommandExecutionMode::Manual,
            ..shell_required.clone()
        };
        let manual_doc = command_spec_document(&manual);
        if manual_doc
            .pointer("/execution_mode")
            .and_then(Value::as_str)
            != Some("manual")
            || manual_doc.pointer("/directness").and_then(Value::as_str)
                != Some("visibly_non_direct")
        {
            return Err(format!(
                "manual mode must stay visibly non-direct: {manual_doc}"
            ));
        }
        for document in [&direct_doc, &shell_doc, &manual_doc] {
            for field in [
                "/command_id",
                "/role",
                "/program",
                "/args",
                "/timeout_ms",
                "/expected_exit_codes",
                "/human_display",
                "/authority_boundary",
            ] {
                if document.pointer(field).is_none() {
                    return Err(format!("command spec projection lost {field}: {document}"));
                }
            }
            if document
                .pointer("/display_is_execution_authority")
                .and_then(Value::as_bool)
                != Some(false)
            {
                return Err("the display string must never carry execution authority".to_string());
            }
        }
        Ok(())
    }

    #[test]
    fn argv_never_reconstructs_from_the_display_string() -> Result<(), String> {
        let mut spec = crate::agent::command_specs::agent_verify_command_spec(
            ".",
            "target/ripr/workflow/before.json",
            "target/ripr/workflow/after.json",
            None,
        );
        spec.args.push("pricing crate".to_string());
        let document = command_spec_document(&spec);
        let args = document
            .pointer("/args")
            .and_then(Value::as_array)
            .ok_or_else(|| "command spec lost its argv".to_string())?;
        if !args.iter().any(|arg| arg == "pricing crate") {
            return Err(format!(
                "structured argv lost the spaced argument: {document}"
            ));
        }
        let display = document
            .pointer("/human_display")
            .and_then(Value::as_str)
            .ok_or_else(|| "command spec lost its display".to_string())?;
        let naive_tokens = display.split(' ').map(str::to_string).collect::<Vec<_>>();
        let structured_args = args
            .iter()
            .map(|arg| arg.as_str().unwrap_or_default().to_string())
            .collect::<Vec<_>>();
        if naive_tokens == structured_args {
            return Err(
                "the removal experiment failed: display whitespace tokenization reproduced argv"
                    .to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn resource_uri_parsing_is_strict() {
        assert_eq!(
            repair_attempt_resource_id("ripr://repair-attempt/repair-attempt-abc"),
            Some("repair-attempt-abc")
        );
        assert_eq!(
            receipt_resource_id("ripr://receipt/repair-attempt-abc"),
            Some("repair-attempt-abc")
        );
        for other in [
            "ripr://workspace/status",
            "ripr://repair-attempt/",
            "ripr://repair-attempt/a/b",
            "ripr://receipt/",
            "https://example.com/receipt/x",
        ] {
            assert_eq!(repair_attempt_resource_id(other), None, "{other}");
            assert_eq!(receipt_resource_id(other), None, "{other}");
        }
    }
}
