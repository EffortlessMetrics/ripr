//! Test-only minting of evidence-grade repair-loop snapshots and verify documents.
//!
//! Terminal-receipt and retention tests must carry producer-valid evidence:
//! repo-exposure envelopes the artifact validator accepts, and canonical
//! agent-verify renders the receipt path recomputes. Hand-shaped blobs such as
//! `{"kind":"verify"}` cannot exercise binding checks, so these helpers mint
//! through the production authorities instead of duplicating their formats.
//! The input identity is a fixed well-formed value (never derived from the
//! fixture root's manifests) so before/after pairs stay comparable by
//! construction; repository head and content commitment come from the live
//! fixture repository exactly as production records them.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Fixed portable input identity for minted snapshots. It satisfies the
/// validator's `input:{version}:fnv1a64:<16 hex>` shape gate; before and
/// after share it so every minted pair is comparable.
const FIXTURE_INPUT_IDENTITY: &str = "input:v4:fnv1a64:0123456789abcdef";

/// One minimal seam row carrying every field the snapshot validator and the
/// outcome comparison require.
pub(crate) fn snapshot_seam(
    seam_id: &str,
    kind: &str,
    file: &str,
    line: u64,
    grip_class: &str,
) -> Value {
    json!({
        "seam_id": seam_id,
        "kind": kind,
        "file": file,
        "line": line,
        "grip_class": grip_class,
    })
}

/// Mint one valid repo-exposure snapshot binding `seams` to the repository's
/// current HEAD, with a committed content digest. Fails closed through the
/// production validator: the returned bytes always validate.
pub(crate) fn mint_repo_exposure_snapshot(root: &Path, seams: Value) -> Result<String, String> {
    let context = crate::agent::artifact::RepoExposureArtifactContext {
        root: root.to_path_buf(),
        mode: "draft".to_string(),
        base_revision: None,
        input_identity: FIXTURE_INPUT_IDENTITY.to_string(),
    };
    let identity = crate::agent::artifact::repo_exposure_artifact_metadata(
        &context,
        crate::agent::artifact::CONTENT_SHA256_PLACEHOLDER,
    )?;
    let document = json!({
        "schema_version": crate::output::repo_exposure::REPO_EXPOSURE_SCHEMA_VERSION,
        "scope": "repo",
        "run_status": "complete",
        "artifact": identity,
        "seams": seams,
    });
    let raw = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("render fixture snapshot: {error}"))?;
    commit_fixture_snapshot(&raw)
}

/// Splice the content commitment over a placeholder-rendered fixture snapshot.
/// The raw bytes already carry the fixed placeholder at the governed field,
/// so hashing them directly equals the validator's placeholder substitution.
fn commit_fixture_snapshot(raw_with_placeholder: &str) -> Result<String, String> {
    let digest = Sha256::digest(raw_with_placeholder.as_bytes());
    let mut rendered = String::from("sha256:");
    for byte in digest {
        rendered.push_str(&format!("{byte:02x}"));
    }
    let placeholder = crate::agent::artifact::CONTENT_SHA256_PLACEHOLDER;
    if raw_with_placeholder.matches(placeholder).count() != 1 {
        return Err(
            "fixture snapshot must carry exactly one content commitment placeholder".to_string(),
        );
    }
    Ok(raw_with_placeholder.replacen(placeholder, &rendered, 1))
}

/// The verdict fields a producer derives from one verify movement, as
/// `(receipt_state, next_action_kind)`. Hand-mapped from
/// `output::agent_receipt` so minted pairs are an independent oracle, not the
/// validator echoing itself: if the producer mapping changes, this table
/// fails loudly and must be updated consciously.
fn verdict_state_and_kind(movement: &str) -> Result<(&'static str, &'static str), String> {
    match movement {
        "improved" => Ok(("receipt_movement_improved", "improved")),
        "unchanged" => Ok(("receipt_movement_unchanged", "unchanged")),
        "changed" => Ok(("receipt_found", "changed")),
        "regressed" => Ok(("receipt_missing", "regressed")),
        "new" => Ok(("receipt_missing", "new_gap")),
        "resolved" => Ok(("receipt_missing", "resolved")),
        _ => Err(format!(
            "fixture movement `{movement}` has no mapped verdict state"
        )),
    }
}

/// Mint a verdict-consistent terminal receipt/verify pair for a finished
/// attempt manifest, as `(receipt_bytes, verify_bytes)`. The verify document
/// carries the current agent-verify schema, the attempt's retained before
/// snapshot with its recomputed content commitment, and one seam row with
/// `movement`; the receipt carries the 4-field after binding plus the
/// verdict fields a producer derives from that movement. The pair is schema-
/// and binding-valid but not a canonical after-phase render: retention tests
/// that need canonical evidence mint it through [`mint_canonical_verify`]
/// and the receipt half through [`mint_bound_receipt`] instead.
pub(crate) fn mint_bound_receipt_pair(
    root: &Path,
    manifest: &crate::app::repair_attempt::RepairAttemptManifest,
    movement: &str,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let retained_before = root.join(
        &crate::app::repair_attempt::find_manifest_artifact_by_role(manifest, "before_snapshot")
            .ok_or_else(|| "fixture manifest has no before_snapshot".to_string())?
            .path,
    );
    let before_text = std::fs::read_to_string(&retained_before)
        .map_err(|error| format!("read retained fixture before failed: {error}"))?;
    let before_commitment = crate::agent::artifact::recompute_content_commitment(&before_text)?;
    let seam_row = if movement == "new" || movement == "resolved" {
        json!({
            "seam_id": manifest.seam_id,
            "seam_kind": "predicate_boundary",
            "file": "src/lib.rs",
            "line": 1,
            "grip_class": "weakly_gripped",
            "change": movement,
        })
    } else {
        json!({
            "seam_id": manifest.seam_id,
            "seam_kind": "predicate_boundary",
            "file": "src/lib.rs",
            "line": 1,
            "before": "weakly_gripped",
            "after": if movement == "improved" { "strongly_gripped" } else { "weakly_gripped" },
            "change": movement,
            "evidence_delta": [],
        })
    };
    let bucket = match movement {
        "unchanged" => "unchanged_seams",
        "new" => "new_gaps",
        "resolved" => "resolved_gaps",
        _ => "changed_seams",
    };
    let mut verify = json!({
        "schema_version": crate::output::outcome::AGENT_VERIFY_SCHEMA_VERSION,
        "tool": "ripr",
        "status": "advisory",
        "inputs": {
            "before": retained_before.display().to_string(),
            "after": root.join("target/ripr/workflow/after.json").display().to_string(),
            "before_content_sha256": before_commitment,
            "after_content_sha256": sha256_prefixed(b"fixture-after-bytes"),
        },
        "artifact_currentness": "historical_before_current_after",
        "summary": {"improved": 0, "changed": 0, "regressed": 0, "unchanged": 1, "new": 0, "resolved": 0},
        "changed_seams": [],
        "unchanged_seams": [],
        "new_gaps": [],
        "resolved_gaps": [],
    });
    verify[bucket] = json!([seam_row]);
    let mut verify_bytes = serde_json::to_vec_pretty(&verify)
        .map_err(|error| format!("serialize fixture verify failed: {error}"))?;
    verify_bytes.push(b'\n');
    let receipt_bytes = mint_bound_receipt(manifest, movement, &sha256_prefixed(&verify_bytes))?;
    Ok((receipt_bytes, verify_bytes))
}

/// Mint the receipt half of a terminal pair against prebuilt verify bytes:
/// the 4-field after binding plus the verdict fields a producer derives from
/// `movement`, recording `verify_sha256` as its verify artifact digest.
pub(crate) fn mint_bound_receipt(
    manifest: &crate::app::repair_attempt::RepairAttemptManifest,
    movement: &str,
    verify_sha256: &str,
) -> Result<Vec<u8>, String> {
    let after = manifest
        .after
        .as_ref()
        .ok_or_else(|| "fixture manifest carries no after verdict".to_string())?;
    let (state, kind) = verdict_state_and_kind(movement)?;
    let receipt = json!({
        "schema_version": crate::output::agent_receipt::AGENT_RECEIPT_SCHEMA_VERSION,
        "status": "advisory",
        "analysis_outcome_status": "complete",
        "analysis_outcome_error": Value::Null,
        "analysis_outcome": {"analysis_complete": true},
        "provenance": {
            "seam_id": manifest.seam_id,
            "movement": movement,
            "verify_artifact": {
                "path": crate::agent::loop_commands::WORKFLOW_AGENT_VERIFY_ARTIFACT,
                "sha256": verify_sha256,
            },
        },
        "seam": {"seam_id": manifest.seam_id, "change": movement},
        "summary": {
            "receipt_state": state,
            "next_action": {"kind": kind},
        },
        "repair_attempt": {
            "attempt_id": after.attempt_id.as_str(),
            "after_head": after.repository_head,
            "delta_sha256": after.delta_sha256,
            "packet_sha256": after.packet_sha256
        }
    });
    let mut receipt_bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("serialize fixture receipt failed: {error}"))?;
    receipt_bytes.push(b'\n');
    Ok(receipt_bytes)
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut rendered = String::from("sha256:");
    for byte in digest {
        rendered.push_str(&format!("{byte:02x}"));
    }
    rendered
}

/// Mint the canonical agent-verify document for two snapshot files, through
/// the same production authorities the after phase uses: artifact validation,
/// comparability, lineage, movement, outcome comparison, and canonical
/// rendering bound to the validated content digests. `before` and `after` are
/// read for comparison and named in the verify inputs by display spelling, so
/// pass the exact paths the consumer under test must resolve.
pub(crate) fn mint_canonical_verify(
    root: &Path,
    before: &Path,
    after: &Path,
) -> Result<String, String> {
    let before_json = std::fs::read_to_string(before)
        .map_err(|error| format!("read fixture before {}: {error}", before.display()))?;
    let after_json = std::fs::read_to_string(after)
        .map_err(|error| format!("read fixture after {}: {error}", after.display()))?;
    let before_identity = crate::agent::artifact::validate_repo_exposure_artifact(
        root,
        &before_json,
        "fixture before",
    )?;
    let after_identity = crate::agent::artifact::validate_repo_exposure_artifact(
        root,
        &after_json,
        "fixture after",
    )?;
    crate::agent::artifact::validate_comparable_pair(&before_identity, &after_identity)
        .map_err(|error| format!("fixture snapshots are incomparable: {error}"))?;
    crate::agent::artifact::validate_pair_lineage(root, &before_identity, &after_identity)
        .map_err(|error| format!("fixture snapshots lack lineage: {error}"))?;
    crate::agent::artifact::validate_verify_movement(&before_identity, &after_identity)
        .map_err(|error| format!("fixture snapshots have no movement: {error}"))?;
    let currentness = crate::agent::artifact::pair_currentness_label(
        &before_identity.currentness,
        &after_identity.currentness,
    );
    let report = crate::output::outcome::targeted_test_outcome_report_from_json(
        &before_json,
        &after_json,
        before.display().to_string(),
        after.display().to_string(),
    )?;
    let binding = crate::output::outcome::AgentVerifyArtifactBinding {
        before_content_sha256: before_identity.content_sha256,
        after_content_sha256: after_identity.content_sha256,
    };
    crate::output::outcome::render_agent_verify_json_with_currentness(
        &report,
        Some(currentness),
        &binding,
    )
}
