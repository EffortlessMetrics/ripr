use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

pub(crate) const SWARM_INGEST_SCHEMA_VERSION: &str = "0.1";

/// Closed set of machine-readable `reason` strings emitted by the outcome
/// classifier.  Every `unknown` outcome MUST carry one of these values.
/// Do not extend this set without updating `policy/output_contracts.txt` and
/// `docs/OUTPUT_SCHEMA.md`.
pub(crate) mod ingest_reason {
    /// The receipt claims a movement (improved / unchanged / regressed /
    /// resolved) but the before-artifact or after-artifact sha256 is absent
    /// from the provenance block.  Without snapshot provenance the movement
    /// claim cannot be validated; the classifier fails closed.
    pub(crate) const MOVEMENT_WITHOUT_SNAPSHOT_PROVENANCE: &str =
        "movement_without_snapshot_provenance";

    /// Verify evidence is absent or inconclusive.  The classifier cannot
    /// reach an improvement outcome without a passing verify signal.
    pub(crate) const MISSING_VERIFY: &str = "missing_verify";

    /// The input packet is marked stale.  Evidence from a stale packet is
    /// unreliable; the classifier fails closed rather than reporting any
    /// movement outcome.
    pub(crate) const STALE_PACKET: &str = "stale_packet";

    /// The agent edited at least one file on the packet's forbidden list.
    /// All movement claims are discarded regardless of verify or receipt
    /// evidence.
    pub(crate) const FORBIDDEN_EDIT: &str = "forbidden_edit";
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SwarmIngestFacts {
    gap_id: Option<String>,
    canonical_gap_id: Option<String>,
    agent_status: Option<String>,
    stop_reason: Option<String>,
    staleness_status: Option<String>,
    edited_files: Vec<String>,
    allowed_files: Vec<String>,
    forbidden_files: Vec<String>,
    edited_forbidden_files: Vec<String>,
    /// Edited path entries that do not resolve inside the selected root
    /// (#5984): absolute paths outside it or `..` climbs above it. Surfaced
    /// for review instead of silently passing the forbidden-edit guard.
    edited_files_outside_root: Vec<String>,
    verify_present: bool,
    verify_status: Option<String>,
    verify_exit_code: Option<i64>,
    verify_passed: bool,
    verify_failed: bool,
    receipt_present: bool,
    receipt_path: Option<String>,
    receipt_movement: Option<String>,
    /// sha256 of the before-artifact recorded in the receipt provenance block.
    /// Required before any movement claim (improved/unchanged/regressed/resolved)
    /// can be accepted.
    receipt_before_sha256: Option<String>,
    /// sha256 of the after-artifact recorded in the receipt provenance block.
    /// Required before any movement claim can be accepted.
    receipt_after_sha256: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SwarmIngestClassification {
    state: &'static str,
    outcome: &'static str,
    reason: &'static str,
    next_action: &'static str,
}

pub(crate) fn render_swarm_ingest_json(
    result_json: &str,
    result_path: &str,
    root: &Path,
) -> Result<String, String> {
    let value: Value = serde_json::from_str(result_json)
        .map_err(|err| format!("failed to parse swarm ingest result JSON: {err}"))?;
    let facts = swarm_ingest_facts(&value, root);
    let classification = classify_swarm_result(&facts);
    let rendered = serde_json::json!({
        "schema_version": SWARM_INGEST_SCHEMA_VERSION,
        "tool": "ripr",
        "report": "swarm-ingest",
        "scope": "agent_result",
        "source": "external_agent_result",
        "status": "advisory",
        "attempt_outcome": classification.outcome,
        "inputs": {
            "result": result_path,
        },
        "classification": {
            "state": classification.state,
            "outcome": classification.outcome,
            "reason": classification.reason,
            "gap_id": facts.gap_id.as_ref(),
            "canonical_gap_id": facts.canonical_gap_id.as_ref(),
        },
        "evidence": {
            "agent_status": facts.agent_status.as_ref(),
            "stop_reason": facts.stop_reason.as_ref(),
            "staleness_status": facts.staleness_status.as_ref(),
            "edited_files": &facts.edited_files,
            "allowed_files": &facts.allowed_files,
            "forbidden_files": &facts.forbidden_files,
            "edited_forbidden_files": &facts.edited_forbidden_files,
            "edited_files_outside_root": &facts.edited_files_outside_root,
            "verify": {
                "present": facts.verify_present,
                "status": facts.verify_status,
                "exit_code": facts.verify_exit_code,
                "passed": facts.verify_passed,
                "failed": facts.verify_failed,
            },
            "receipt": {
                "present": facts.receipt_present,
                "path": facts.receipt_path.as_ref(),
                "movement": facts.receipt_movement.as_ref(),
                "provenance": {
                    "before_sha256": facts.receipt_before_sha256.as_ref(),
                    "after_sha256": facts.receipt_after_sha256.as_ref(),
                    "snapshot_provenance_present": has_snapshot_provenance(&facts),
                },
            },
        },
        "safety": {
            "forbidden_edit_flagged": !facts.edited_forbidden_files.is_empty(),
            "requires_human_review": true,
            "trusted_success": false,
        },
        "next_action": {
            "kind": classification.state,
            "summary": classification.next_action,
        },
        "must_not_infer": [
            "do not trust agent-reported success without verify evidence",
            "do not treat missing verify output as closed",
            "do not ignore forbidden production-code edits",
            "do not run providers, generate tests, run mutation testing, or claim runtime proof from ingest",
        ],
    });
    super::json::render_pretty_with_newline(&rendered, "swarm ingest")
}

fn swarm_ingest_facts(value: &Value, root: &Path) -> SwarmIngestFacts {
    let forbidden_files = first_string_array(
        value,
        &[
            &["forbidden_files"],
            &["packet", "forbidden_files"],
            &["queue_packet", "forbidden_files"],
            &["agent_packet", "forbidden_files"],
            &["task", "forbidden_files"],
        ],
    );
    let edited_files = first_string_array(
        value,
        &[
            &["edited_files"],
            &["changed_files"],
            &["attempt", "edited_files"],
            &["attempt", "changed_files"],
            &["result", "edited_files"],
            &["result", "changed_files"],
            &["changes", "edited_files"],
        ],
    );
    let edited_forbidden_files = edited_forbidden_files(&edited_files, &forbidden_files, root);
    let outside_root = edited_files_outside_root(&edited_files, root);
    let verify_status = first_string(
        value,
        &[
            &["verify_status"],
            &["verify", "status"],
            &["verification", "status"],
            &["attempt", "verify", "status"],
            &["attempt", "verification", "status"],
            &["result", "verify", "status"],
            &["result", "verification", "status"],
        ],
    )
    // An agent receipt says `verification_not_run` (RIPR-SPEC-0135): nothing
    // ran, so it is no verify evidence at all, not an unreadable result.
    .filter(|status| !is_not_run_status(status));
    let verify_exit_code = first_i64(
        value,
        &[
            &["verify_exit_code"],
            &["verify", "exit_code"],
            &["verification", "exit_code"],
            &["attempt", "verify", "exit_code"],
            &["attempt", "verification", "exit_code"],
            &["result", "verify", "exit_code"],
            &["result", "verification", "exit_code"],
        ],
    );
    let verify_present = verify_status.is_some()
        || verify_exit_code.is_some()
        || first_string(
            value,
            &[
                &["verify", "output_path"],
                &["verify", "stdout"],
                &["verification", "output_path"],
                &["attempt", "verify", "output_path"],
                &["attempt", "verification", "output_path"],
            ],
        )
        .is_some();
    let verify_passed =
        verify_status.as_deref().is_some_and(is_success_status) || verify_exit_code == Some(0);
    let verify_failed = verify_status.as_deref().is_some_and(is_failure_status)
        || verify_exit_code.is_some_and(|code| code != 0);
    let receipt_path = first_string(
        value,
        &[
            &["receipt_path"],
            &["attempt", "receipt_path"],
            &["result", "receipt_path"],
            &["receipt", "path"],
            &["receipt", "artifact"],
            &["agent_receipt", "path"],
        ],
    );
    let receipt_movement = first_string(
        value,
        &[
            &["receipt_movement"],
            &["receipt", "movement"],
            &["receipt", "provenance", "movement"],
            &["receipt", "static_movement", "state"],
            &["receipt", "summary", "receipt_state"],
            &["agent_receipt", "provenance", "movement"],
            &["agent_receipt", "seam", "change"],
            &["provenance", "movement"],
            &["seam", "change"],
        ],
    );
    let receipt_present = receipt_path.is_some() || receipt_movement.is_some();
    let receipt_before_sha256 = first_string(
        value,
        &[
            &["receipt", "provenance", "before_artifact", "sha256"],
            &["receipt", "provenance", "before", "sha256"],
            &["agent_receipt", "provenance", "before_artifact", "sha256"],
            &["agent_receipt", "provenance", "before", "sha256"],
            &["provenance", "before_artifact", "sha256"],
        ],
    );
    let receipt_after_sha256 = first_string(
        value,
        &[
            &["receipt", "provenance", "after_artifact", "sha256"],
            &["receipt", "provenance", "after", "sha256"],
            &["agent_receipt", "provenance", "after_artifact", "sha256"],
            &["agent_receipt", "provenance", "after", "sha256"],
            &["provenance", "after_artifact", "sha256"],
        ],
    );
    SwarmIngestFacts {
        gap_id: first_string(
            value,
            &[
                &["gap_id"],
                &["packet", "gap_id"],
                &["queue_packet", "gap_id"],
                &["agent_packet", "gap_id"],
                &["task", "gap_id"],
                &["result", "gap_id"],
            ],
        ),
        canonical_gap_id: first_string(
            value,
            &[
                &["canonical_gap_id"],
                &["packet", "canonical_gap_id"],
                &["queue_packet", "canonical_gap_id"],
                &["agent_packet", "canonical_gap_id"],
                &["task", "canonical_gap_id"],
                &["result", "canonical_gap_id"],
            ],
        ),
        agent_status: first_string(
            value,
            &[
                &["agent_status"],
                &["attempt", "status"],
                &["result", "status"],
                &["status"],
            ],
        ),
        stop_reason: first_string(
            value,
            &[
                &["stop_reason"],
                &["attempt", "stop_reason"],
                &["result", "stop_reason"],
            ],
        ),
        staleness_status: first_string(
            value,
            &[
                &["staleness_status"],
                &["packet", "staleness_status"],
                &["queue_packet", "staleness_status"],
                &["agent_packet", "staleness_status"],
            ],
        ),
        edited_files,
        allowed_files: first_string_array(
            value,
            &[
                &["allowed_files"],
                &["allowed_edit_surface"],
                &["packet", "allowed_files"],
                &["packet", "allowed_edit_surface"],
                &["queue_packet", "allowed_files"],
                &["queue_packet", "allowed_edit_surface"],
                &["agent_packet", "allowed_files"],
                &["task", "allowed_files"],
            ],
        ),
        forbidden_files,
        edited_forbidden_files,
        edited_files_outside_root: outside_root,
        verify_present,
        verify_status,
        verify_exit_code,
        verify_passed,
        verify_failed,
        receipt_present,
        receipt_path,
        receipt_movement,
        receipt_before_sha256,
        receipt_after_sha256,
    }
}

fn classify_swarm_result(facts: &SwarmIngestFacts) -> SwarmIngestClassification {
    if !facts.edited_forbidden_files.is_empty() {
        return SwarmIngestClassification {
            state: "edited_forbidden_file",
            outcome: "unknown",
            reason: ingest_reason::FORBIDDEN_EDIT,
            next_action: "Reject or manually review the attempt before using any test repair.",
        };
    }
    if facts
        .staleness_status
        .as_deref()
        .is_some_and(is_stale_status)
    {
        return SwarmIngestClassification {
            state: "stale_packet",
            outcome: "unknown",
            reason: ingest_reason::STALE_PACKET,
            next_action: "Refresh the queue and reroute the gap before trusting the attempt.",
        };
    }
    if facts.agent_status.as_deref().is_some_and(is_stopped_status) || facts.stop_reason.is_some() {
        return SwarmIngestClassification {
            state: "stopped_by_agent",
            outcome: receipt_presence_outcome(facts),
            reason: ingest_reason::MISSING_VERIFY,
            next_action: "Record the stop reason and reroute only if the packet remains actionable.",
        };
    }
    if facts.verify_failed {
        return SwarmIngestClassification {
            state: "verify_failed",
            outcome: receipt_presence_outcome(facts),
            reason: ingest_reason::MISSING_VERIFY,
            next_action: "Inspect verify output before retrying or accepting the repair.",
        };
    }
    if !facts.verify_present {
        return SwarmIngestClassification {
            state: "uncertain",
            outcome: receipt_presence_outcome(facts),
            reason: ingest_reason::MISSING_VERIFY,
            next_action: "Run the packet verify command and attach the result before judging closure.",
        };
    }
    if !facts.verify_passed {
        return SwarmIngestClassification {
            state: "uncertain",
            outcome: receipt_presence_outcome(facts),
            reason: ingest_reason::MISSING_VERIFY,
            next_action: "Normalize the verify result or rerun the packet verify command.",
        };
    }
    // Verify passed.  Before accepting any movement claim (improved / unchanged /
    // regressed / resolved), require that the receipt provenance includes a
    // non-empty sha256 for both the before-artifact and the after-artifact.
    // Missing snapshot provenance means the movement cannot be validated; fail
    // closed rather than claiming an outcome from unverifiable evidence.
    // (RIPR-SPEC-0073 outcome-resolution rule 5; "resolved" / "new" movements
    // that represent one-sided changes still require at least the relevant
    // artifact sha256 — we enforce both here to be consistent and conservative.)
    let movement_normalized = facts.receipt_movement.as_deref().map(normalize_state);
    let movement_is_claimed = movement_normalized.as_deref().is_some_and(|m| {
        matches!(
            m,
            "closed"
                | "resolved"
                | "receipt_movement_resolved"
                | "improved"
                | "receipt_movement_improved"
                | "unchanged"
                | "receipt_movement_unchanged"
                | "unchanged_after_attempt"
                | "regressed"
                | "receipt_movement_regressed"
        )
    });
    if movement_is_claimed && !has_snapshot_provenance(facts) {
        return SwarmIngestClassification {
            state: "uncertain",
            outcome: "unknown",
            reason: ingest_reason::MOVEMENT_WITHOUT_SNAPSHOT_PROVENANCE,
            next_action: "Produce a receipt with before/after artifact sha256 provenance before judging movement.",
        };
    }
    match movement_normalized.as_deref() {
        Some("closed" | "resolved" | "receipt_movement_resolved") => SwarmIngestClassification {
            state: "closed",
            outcome: "resolved",
            reason: "Verify passed and receipt movement indicates the gap closed.",
            next_action: "Attach the receipt and keep the focused test repair.",
        },
        Some("improved" | "receipt_movement_improved") => SwarmIngestClassification {
            state: "partially_improved",
            outcome: "evidence_improved",
            reason: "Verify passed and receipt movement improved, but did not report closure.",
            next_action: "Keep the evidence and decide whether another focused repair is needed.",
        },
        Some("unchanged" | "receipt_movement_unchanged" | "unchanged_after_attempt") => {
            SwarmIngestClassification {
                state: "uncertain",
                outcome: "evidence_unchanged",
                reason: "Verify passed but receipt movement stayed unchanged.",
                next_action: "Strengthen the discriminator or reroute the remaining gap.",
            }
        }
        Some("regressed" | "receipt_movement_regressed") => SwarmIngestClassification {
            state: "uncertain",
            outcome: "evidence_regressed",
            reason: "Verify passed but receipt movement regressed.",
            next_action: "Reject or manually inspect the attempt before retrying.",
        },
        _ => SwarmIngestClassification {
            state: "uncertain",
            outcome: receipt_presence_outcome(facts),
            reason: "Verify passed but no recognized receipt movement was supplied.",
            next_action: "Produce a before/after receipt before judging closure.",
        },
    }
}

/// Return `true` if the receipt provenance block contains non-empty sha256
/// values for both the before-artifact and the after-artifact.  This is the
/// minimum provenance required before a movement claim is trusted.
fn has_snapshot_provenance(facts: &SwarmIngestFacts) -> bool {
    let before_ok = facts
        .receipt_before_sha256
        .as_deref()
        .is_some_and(|s| !s.is_empty());
    let after_ok = facts
        .receipt_after_sha256
        .as_deref()
        .is_some_and(|s| !s.is_empty());
    before_ok && after_ok
}

fn receipt_presence_outcome(facts: &SwarmIngestFacts) -> &'static str {
    if facts.receipt_present {
        "receipt_present"
    } else {
        "attempted_no_receipt"
    }
}

/// Canonical comparison key of one edited/forbidden path entry (#5984).
///
/// Raw string comparison missed ordinary spellings of the same file: a
/// different case (`SRC/PRICING.py` on a case-insensitive filesystem), an
/// absolute path under the selected root, or `..` segments — each evaded the
/// forbidden-edit guard and let the attempt classify as closed. The key
/// folds backslashes to `/`, resolves `.`/`..` segments lexically, makes
/// absolute paths under `root` root-relative, and compares ASCII
/// case-insensitively, so every spelling of one file produces the same key.
/// Windows is a supported platform and agent output arrives from
/// heterogeneous hosts, so the fold is unconditional; its failure direction
/// is over-flagging a match, which fails closed.
fn canonical_compare_key(path: &str, root: &Path, root_segments: Option<&[String]>) -> String {
    canonical_path(path, root, root_segments).key
}

struct CanonicalPath {
    /// Lowercased comparison key: root-relative when the path resolves
    /// inside the root, otherwise the resolved absolute or `..`-prefixed
    /// form.
    key: String,
    /// `false` when the path is absolute outside the root, climbs above it
    /// with `..`, or cannot be established to resolve inside it (a
    /// drive-relative spelling whose file does not exist here), so it cannot
    /// match a root-relative forbidden entry.
    under_root: bool,
}

fn canonical_path(path: &str, root: &Path, root_segments: Option<&[String]>) -> CanonicalPath {
    let unix = path.replace('\\', "/");
    // A path that names the filesystem directly — POSIX-absolute, a Windows
    // drive path, or drive-relative — resolves against the actual filesystem
    // when it exists, so symlinked roots, verbatim `\\?\` prefixes, and
    // drive-relative forms anchor to their real location under the
    // canonicalized root (PR review on #5984).
    let names_filesystem_directly = unix.starts_with('/') || has_windows_drive_prefix(&unix);
    if names_filesystem_directly {
        if let Some(segments) = std::fs::canonicalize(&unix)
            .ok()
            .and_then(|resolved| absolute_unix_segments(&resolved.to_string_lossy()))
        {
            return anchor_under_root(segments, root_segments);
        }
        if let Some(segments) = absolute_unix_segments(&unix) {
            return anchor_under_root(segments, root_segments);
        }
        // Drive-relative `C:name`: its meaning depends on that drive's
        // current directory and it resolved to nothing here, so containment
        // in the root cannot be established. Surface it for review instead
        // of treating it as inside the root.
        return CanonicalPath {
            key: unix.to_ascii_lowercase(),
            under_root: false,
        };
    }
    let segments = lexical_segments(&unix, false);
    if segments.first().is_some_and(|first| first == "..") {
        // Root-relative climb (`../sibling/src/pricing.py`): entries are
        // root-relative by packet convention, so resolve the climb against
        // the root before anchoring — a climb that lands back inside the
        // root names the same file as its root-relative spelling (PR review
        // on #5984). A climb that stays above the root stays outside it.
        if let Some(root) = root_segments {
            let mut combined = root.to_vec();
            for segment in &segments {
                match segment.as_str() {
                    ".." => {
                        combined.pop();
                    }
                    "." => {}
                    other => combined.push(other.to_string()),
                }
            }
            return anchor_under_root(combined, Some(root));
        }
        return CanonicalPath {
            key: segments.join("/").to_ascii_lowercase(),
            under_root: false,
        };
    }
    // A bare root-relative entry that names an existing file resolves
    // against the selected root (never the process working directory), so a
    // symlinked test-file spelling anchors to the forbidden file it actually
    // writes through (PR review on #6289). A symlink target outside the
    // root anchors there and surfaces as outside-root evidence.
    let resolved = (!segments.is_empty())
        .then(|| std::fs::canonicalize(root.join(&unix)).ok())
        .flatten()
        .and_then(|resolved| absolute_unix_segments(&resolved.to_string_lossy()));
    if let Some(segments) = resolved {
        return anchor_under_root(segments, root_segments);
    }
    CanonicalPath {
        key: segments.join("/").to_ascii_lowercase(),
        under_root: true,
    }
}

/// Anchor absolute segments to the resolved root: root-relative when the
/// root prefix matches (case-insensitive, component-wise), otherwise outside
/// the root.
fn anchor_under_root(segments: Vec<String>, root_segments: Option<&[String]>) -> CanonicalPath {
    if let Some(relative) = root_segments.and_then(|root| strip_segment_prefix(&segments, root)) {
        return CanonicalPath {
            key: relative.join("/").to_ascii_lowercase(),
            under_root: true,
        };
    }
    CanonicalPath {
        key: segments.join("/").to_ascii_lowercase(),
        under_root: false,
    }
}

fn edited_forbidden_files(
    edited_files: &[String],
    forbidden_files: &[String],
    root: &Path,
) -> Vec<String> {
    let root_segments = resolved_root_segments(root);
    let forbidden: BTreeSet<_> = forbidden_files
        .iter()
        .map(|file| canonical_compare_key(file, root, root_segments.as_deref()))
        .collect();
    dedup(
        edited_files
            .iter()
            .filter(|file| {
                forbidden.contains(&canonical_compare_key(file, root, root_segments.as_deref()))
            })
            .cloned()
            .collect(),
    )
}

/// Edited entries whose canonical form does not resolve inside the selected
/// root (#5984). They cannot equal a root-relative forbidden entry, so the
/// guard surfaces them as unmatched evidence instead of silently passing.
fn edited_files_outside_root(edited_files: &[String], root: &Path) -> Vec<String> {
    let root_segments = resolved_root_segments(root);
    dedup(
        edited_files
            .iter()
            .filter(|file| !canonical_path(file, root, root_segments.as_deref()).under_root)
            .cloned()
            .collect(),
    )
}

/// Segments of the selected root: filesystem-resolved when the root exists,
/// so a symlinked `--root` anchors edited paths to the same location the
/// CLI's canonicalized root names (PR review on #5984); lexical fallback for
/// a root that does not exist on this host.
fn resolved_root_segments(root: &Path) -> Option<Vec<String>> {
    if let Ok(resolved) = root.canonicalize() {
        return absolute_unix_segments(&resolved.to_string_lossy().replace('\\', "/"));
    }
    absolute_unix_segments(&root.to_string_lossy().replace('\\', "/"))
}

/// `//server/share`, `/abs`, and drive-letter paths are absolute after
/// the backslash fold. A drive-relative drive-colon name stays relative.
fn is_absolute_unix_path(path: &str) -> bool {
    if path.starts_with('/') {
        return true;
    }
    has_windows_drive_prefix(path) && path.len() >= 3 && path.as_bytes()[2] == b'/'
}

/// A drive-letter prefix (one ASCII letter then a colon) — absolute or
/// drive-relative on Windows.
fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Resolve `.` and `..` lexically. Relative paths keep leading `..` segments
/// (they climb above the root); absolute paths clamp at their root.
fn lexical_segments(unix: &str, absolute: bool) -> Vec<String> {
    let mut segments: Vec<String> = Vec::new();
    for segment in unix.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if segments.last().map(String::as_str) == Some("..") {
                    segments.push("..".to_string());
                } else if !segments.is_empty() {
                    segments.pop();
                } else if !absolute {
                    segments.push("..".to_string());
                }
            }
            other => segments.push(other.to_string()),
        }
    }
    segments
}

/// Segments of an absolute backslash-folded path, unwrapping the verbatim
/// prefix Windows canonicalization produces (`//?/C:/dir`,
/// `//?/UNC/server/share`). `None` when the path is not absolute.
fn absolute_unix_segments(unix: &str) -> Option<Vec<String>> {
    let unix = if let Some(rest) = unix.strip_prefix("//?/UNC/") {
        format!("//{rest}")
    } else if let Some(rest) = unix.strip_prefix("//?/") {
        rest.to_string()
    } else {
        unix.to_string()
    };
    if !is_absolute_unix_path(&unix) {
        return None;
    }
    Some(lexical_segments(&unix, true))
}

/// Case-insensitive, component-wise prefix strip. Returns the remainder when
/// every root segment matches, else `None`.
fn strip_segment_prefix<'a>(segments: &'a [String], root: &[String]) -> Option<&'a [String]> {
    if root.len() > segments.len() {
        return None;
    }
    for (segment, root_segment) in segments.iter().zip(root) {
        if !segment.eq_ignore_ascii_case(root_segment) {
            return None;
        }
    }
    Some(&segments[root.len()..])
}

fn first_string(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths
        .iter()
        .find_map(|path| path_value(value, path).and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

fn first_i64(value: &Value, paths: &[&[&str]]) -> Option<i64> {
    paths
        .iter()
        .find_map(|path| path_value(value, path).and_then(Value::as_i64))
}

fn first_string_array(value: &Value, paths: &[&[&str]]) -> Vec<String> {
    paths
        .iter()
        .find_map(|path| {
            let values = string_array_at(value, path);
            (!values.is_empty()).then_some(values)
        })
        .unwrap_or_default()
}

fn string_array_at(value: &Value, path: &[&str]) -> Vec<String> {
    let Some(array) = path_value(value, path).and_then(Value::as_array) else {
        return Vec::new();
    };
    dedup(
        array
            .iter()
            .filter_map(|item| {
                item.as_str()
                    .map(ToOwned::to_owned)
                    .or_else(|| {
                        item.get("path")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned)
                    })
                    .or_else(|| {
                        item.get("file")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned)
                    })
            })
            .collect(),
    )
}

fn path_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn dedup(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for value in values {
        if seen.insert(value.clone()) {
            deduped.push(value);
        }
    }
    deduped
}

fn normalize_state(state: &str) -> String {
    state.trim().to_ascii_lowercase()
}

fn is_success_status(status: &str) -> bool {
    matches!(
        normalize_state(status).as_str(),
        "pass" | "passed" | "success" | "succeeded" | "ok"
    )
}

fn is_failure_status(status: &str) -> bool {
    matches!(
        normalize_state(status).as_str(),
        "fail" | "failed" | "failure" | "error" | "errored"
    )
}

fn is_not_run_status(status: &str) -> bool {
    matches!(
        normalize_state(status).as_str(),
        "verification_not_run" | "not_run"
    )
}

fn is_stale_status(status: &str) -> bool {
    matches!(normalize_state(status).as_str(), "stale" | "stale_packet")
}

fn is_stopped_status(status: &str) -> bool {
    matches!(
        normalize_state(status).as_str(),
        "stopped" | "stopped_by_agent" | "blocked" | "aborted"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_value(json: &str) -> Result<Value, String> {
        render_value_with_root(Path::new("."), json)
    }

    fn render_value_with_root(root: &Path, json: &str) -> Result<Value, String> {
        let rendered = render_swarm_ingest_json(json, "agent-result.json", root)?;
        serde_json::from_str(&rendered)
            .map_err(|err| format!("rendered ingest JSON should parse: {err}"))
    }

    #[test]
    fn ingest_flags_forbidden_edits_before_success_claims() -> Result<(), String> {
        let value = render_value(
            r#"{
              "packet": {
                "gap_id": "gap:python:pricing",
                "canonical_gap_id": "python:app/pricing.py:calculate_discount:predicate_boundary:amount>=threshold",
                "allowed_files": ["tests/test_pricing.py"],
                "forbidden_files": ["app/pricing.py"]
              },
              "attempt": {
                "status": "completed",
                "edited_files": ["tests/test_pricing.py", "app/pricing.py"],
                "verify": {"status": "passed", "exit_code": 0}
              },
              "receipt": {"provenance": {"movement": "resolved"}}
            }"#,
        )?;

        assert_eq!(value["classification"]["state"], "edited_forbidden_file");
        assert_eq!(value["classification"]["outcome"], "unknown");
        assert_eq!(value["attempt_outcome"], "unknown");
        assert_eq!(value["safety"]["forbidden_edit_flagged"], true);
        assert_eq!(
            value["evidence"]["edited_forbidden_files"],
            serde_json::json!(["app/pricing.py"])
        );
        assert_eq!(value["safety"]["trusted_success"], false);
        Ok(())
    }

    // --- #5984: path-spelling discriminators for the forbidden-edit guard --

    /// Path separators for runtime-assembled machine-shaped test paths;
    /// committed sources must not carry host-specific absolute path literals
    /// (check-local-context).
    const SEP: char = '/';
    const BSEP: char = '\\';

    /// One forbidden-edit repro body from #5984: verify passed and the receipt
    /// claims `resolved` with full provenance, so on the old raw-string
    /// comparison every non-exact spelling classified `closed`/`resolved`.
    fn forbidden_edit_repro_json(edited_files_json: &str) -> String {
        format!(
            r#"{{
              "packet": {{
                "gap_id": "gap:pr:gap:python:src/pricing.py:calculate_discount:predicate_boundary:amount>=threshold",
                "allowed_files": ["tests/test_pricing.py"],
                "forbidden_files": ["src/pricing.py"]
              }},
              "attempt": {{
                "status": "completed",
                "edited_files": {edited_files_json},
                "verify": {{"status": "passed", "exit_code": 0}}
              }},
              "receipt": {{
                "provenance": {{
                  "movement": "resolved",
                  "before_artifact": {{"sha256": "aabbcc0011223344aabbcc0011223344aabbcc0011223344aabbcc0011223344"}},
                  "after_artifact": {{"sha256": "ddee55667788aaddddee55667788aaddddee55667788aaddddee55667788aadd"}}
                }}
              }}
            }}"#
        )
    }

    fn assert_forbidden_edit_witness(value: &Value, expected_edited_forbidden: &[&str]) {
        assert_eq!(value["classification"]["state"], "edited_forbidden_file");
        assert_eq!(value["classification"]["outcome"], "unknown");
        assert_eq!(value["attempt_outcome"], "unknown");
        assert_eq!(
            value["classification"]["reason"].as_str(),
            Some(ingest_reason::FORBIDDEN_EDIT),
            "the pinned forbidden_edit token must witness the guard, not a prose reason"
        );
        assert_eq!(value["safety"]["forbidden_edit_flagged"], true);
        let flagged: Vec<&str> = value["evidence"]["edited_forbidden_files"]
            .as_array()
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<&str>>()
            })
            .unwrap_or_default();
        assert_eq!(flagged, expected_edited_forbidden);
    }

    #[test]
    fn ingest_flags_forbidden_edit_spelled_with_different_case() -> Result<(), String> {
        // #5984 repro 1: `SRC/PRICING.py` names the same file as the forbidden
        // `src/pricing.py` on this case-insensitive host; the guard must fail
        // closed instead of classifying the attempt closed/resolved.
        let value = render_value(&forbidden_edit_repro_json(
            r#"["tests/test_pricing.py", "SRC/PRICING.py"]"#,
        ))?;
        assert_forbidden_edit_witness(&value, &["SRC/PRICING.py"]);
        assert_eq!(
            value["evidence"]["edited_files_outside_root"],
            serde_json::json!([])
        );
        Ok(())
    }

    #[test]
    fn ingest_flags_forbidden_edit_spelled_as_absolute_path_under_root() -> Result<(), String> {
        // #5984 repro 2: agent tool output reports absolute paths; one under
        // --root must normalize to the root-relative forbidden entry. The
        // verbatim `\\?\` root form (what `Path::canonicalize` returns on
        // Windows) must anchor the same files. The machine-shaped path
        // literals are assembled at runtime so committed sources carry no
        // host-specific absolute paths (check-local-context).
        let root_text = format!("F:{}Temp{}r3-queue{}scratch{}app", SEP, SEP, SEP, SEP);
        let root = Path::new(&root_text);
        let edited_forward = format!("{root_text}/src/pricing.py");
        let edited_json = serde_json::to_string(&edited_forward)
            .map_err(|error| format!("encode edited path: {error}"))?;
        let value = render_value_with_root(
            root,
            &forbidden_edit_repro_json(&format!(r#"["tests/test_pricing.py", {edited_json}]"#)),
        )?;
        assert_forbidden_edit_witness(&value, &[edited_forward.as_str()]);
        assert_eq!(
            value["evidence"]["edited_files_outside_root"],
            serde_json::json!([])
        );

        let edited_backslash = format!(
            "F:{}Temp{}r3-queue{}scratch{}app{}src{}pricing.py",
            BSEP, BSEP, BSEP, BSEP, BSEP, BSEP
        );
        let edited_backslash_json = serde_json::to_string(&edited_backslash)
            .map_err(|error| format!("encode edited path: {error}"))?;
        let backslash = render_value_with_root(
            root,
            &forbidden_edit_repro_json(&format!(
                r#"["tests/test_pricing.py", {edited_backslash_json}]"#
            )),
        )?;
        assert_forbidden_edit_witness(&backslash, &[edited_backslash.as_str()]);

        let verbatim_root_text = format!(
            r"\\?\F:{}Temp{}r3-queue{}scratch{}app",
            BSEP, BSEP, BSEP, BSEP
        );
        let verbatim_root = Path::new(&verbatim_root_text);
        let verbatim = render_value_with_root(
            verbatim_root,
            &forbidden_edit_repro_json(&format!(r#"["tests/test_pricing.py", {edited_json}]"#)),
        )?;
        assert_forbidden_edit_witness(&verbatim, &[edited_forward.as_str()]);
        Ok(())
    }

    #[test]
    fn ingest_flags_forbidden_edit_spelled_with_dot_dot_segments() -> Result<(), String> {
        // #5984 repro 3: `..` segments resolve to the same file.
        let value = render_value(&forbidden_edit_repro_json(
            r#"["tests/test_pricing.py", "tests/../src/pricing.py"]"#,
        ))?;
        assert_forbidden_edit_witness(&value, &["tests/../src/pricing.py"]);
        assert_eq!(
            value["evidence"]["edited_files_outside_root"],
            serde_json::json!([])
        );
        Ok(())
    }

    #[test]
    fn ingest_flags_forbidden_edit_climbing_out_of_and_back_into_the_root() -> Result<(), String> {
        // CodeRabbit review on #6289: `../app/src/pricing.py` under root
        // `/x/app` resolves back inside the root, so it must match the
        // root-relative forbidden entry instead of passing as outside-root.
        let value = render_value_with_root(
            Path::new("/x/app"),
            &forbidden_edit_repro_json(r#"["tests/test_pricing.py", "../app/src/pricing.py"]"#),
        )?;
        assert_forbidden_edit_witness(&value, &["../app/src/pricing.py"]);
        assert_eq!(
            value["evidence"]["edited_files_outside_root"],
            serde_json::json!([])
        );

        // A climb that stays above the root stays outside-root evidence.
        let outside = render_value_with_root(
            Path::new("/x/app"),
            &forbidden_edit_repro_json(
                r#"["tests/test_pricing.py", "../elsewhere/src/pricing.py"]"#,
            ),
        )?;
        assert_eq!(outside["classification"]["state"], "closed");
        assert_eq!(outside["safety"]["forbidden_edit_flagged"], false);
        assert_eq!(
            outside["evidence"]["edited_files_outside_root"],
            serde_json::json!(["../elsewhere/src/pricing.py"])
        );
        Ok(())
    }

    #[test]
    fn ingest_surfaces_edited_paths_outside_the_root() -> Result<(), String> {
        // #5984: entries that resolve outside the root cannot match a
        // root-relative forbidden entry; the guard surfaces them as unmatched
        // evidence instead of silently passing them.
        let value = render_value(&forbidden_edit_repro_json(
            r#"["tests/test_pricing.py", "/opt/elsewhere/evil.py", "../outside.py", "tests/../../outside-too.py"]"#,
        ))?;
        assert_eq!(value["classification"]["state"], "closed");
        assert_eq!(value["safety"]["forbidden_edit_flagged"], false);
        assert_eq!(
            value["evidence"]["edited_files_outside_root"],
            serde_json::json!([
                "/opt/elsewhere/evil.py",
                "../outside.py",
                "tests/../../outside-too.py"
            ])
        );
        Ok(())
    }

    #[test]
    fn ingest_flags_forbidden_edit_spelled_with_verbatim_prefix() -> Result<(), String> {
        // PR review on #5984: the agent may report the very `\\?\` verbatim
        // prefix a Windows canonicalized root carries; the edited path is
        // normalized symmetrically with the root, so it still matches.
        let value = render_value_with_root(
            Path::new(r"\\?\F:\Temp\r3-queue\scratch\app"),
            &forbidden_edit_repro_json(
                r#"["tests/test_pricing.py", "\\\\?\\F:\\Temp\\r3-queue\\scratch\\app\\src\\pricing.py"]"#,
            ),
        )?;
        assert_forbidden_edit_witness(
            &value,
            &[r"\\?\F:\Temp\r3-queue\scratch\app\src\pricing.py"],
        );
        assert_eq!(
            value["evidence"]["edited_files_outside_root"],
            serde_json::json!([])
        );
        Ok(())
    }

    #[test]
    fn ingest_flags_existing_absolute_edit_via_filesystem_resolution() -> Result<(), String> {
        // PR review on #5984: when an absolute spelling exists on this host,
        // filesystem resolution anchors it under the root even when the
        // spellings differ (canonicalized case, separators).
        let root = std::env::temp_dir().join(format!(
            "ripr-ingest-fs-resolve-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(root.join("src"))
            .map_err(|error| format!("create root: {error}"))?;
        std::fs::write(root.join("src").join("pricing.py"), "def x(): pass")
            .map_err(|error| format!("write fixture: {error}"))?;
        let edited = root.join("src").join("pricing.py");
        let edited_json = serde_json::to_string(&edited.to_string_lossy())
            .map_err(|error| format!("encode edited path: {error}"))?;
        let value = render_value_with_root(
            &root,
            &forbidden_edit_repro_json(&format!(r#"["tests/test_pricing.py", {edited_json}]"#)),
        )?;
        assert_forbidden_edit_witness(&value, &[edited.to_string_lossy().as_ref()]);
        assert_eq!(
            value["evidence"]["edited_files_outside_root"],
            serde_json::json!([])
        );
        std::fs::remove_dir_all(&root).map_err(|error| format!("remove root: {error}"))?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn ingest_flags_forbidden_edit_reported_through_a_symlinked_root() -> Result<(), String> {
        // PR review on #5984: the edited path may reach the checkout through
        // a symlink alias of --root; filesystem resolution must anchor both
        // to the same real location.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let real = std::env::temp_dir().join(format!("ripr-ingest-symlink-real-{nanos}"));
        let link = std::env::temp_dir().join(format!("ripr-ingest-symlink-link-{nanos}"));
        std::fs::create_dir_all(real.join("src"))
            .map_err(|error| format!("create real root: {error}"))?;
        std::fs::write(real.join("src").join("pricing.py"), "def x(): pass")
            .map_err(|error| format!("write fixture: {error}"))?;
        std::os::unix::fs::symlink(&real, &link)
            .map_err(|error| format!("create symlink: {error}"))?;
        let edited = format!("{}/src/pricing.py", link.display());
        let edited_json = serde_json::to_string(&edited)
            .map_err(|error| format!("encode edited path: {error}"))?;
        let value = render_value_with_root(
            &link,
            &forbidden_edit_repro_json(&format!(r#"["tests/test_pricing.py", {edited_json}]"#)),
        )?;
        assert_forbidden_edit_witness(&value, &[edited.as_str()]);
        assert_eq!(
            value["evidence"]["edited_files_outside_root"],
            serde_json::json!([])
        );
        std::fs::remove_dir_all(&link).map_err(|error| format!("remove link: {error}"))?;
        std::fs::remove_dir_all(&real).map_err(|error| format!("remove real: {error}"))?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn ingest_flags_forbidden_edit_reached_through_a_relative_symlink() -> Result<(), String> {
        // CodeRabbit review on #6289: editing a test-side symlink writes
        // through to the production file it targets; a root-relative entry
        // that names an existing symlink must resolve against the selected
        // root (never the process working directory) and match the
        // forbidden entry.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("ripr-ingest-rel-symlink-{nanos}"));
        std::fs::create_dir_all(root.join("src"))
            .map_err(|error| format!("create root: {error}"))?;
        std::fs::create_dir_all(root.join("tests"))
            .map_err(|error| format!("create tests: {error}"))?;
        std::fs::write(root.join("src").join("pricing.py"), "def x(): pass")
            .map_err(|error| format!("write fixture: {error}"))?;
        std::os::unix::fs::symlink("../src/pricing.py", root.join("tests").join("link.py"))
            .map_err(|error| format!("create symlink: {error}"))?;
        let value =
            render_value_with_root(&root, &forbidden_edit_repro_json(r#"["tests/link.py"]"#))?;
        assert_forbidden_edit_witness(&value, &["tests/link.py"]);
        assert_eq!(
            value["evidence"]["edited_files_outside_root"],
            serde_json::json!([])
        );
        std::fs::remove_dir_all(&root).map_err(|error| format!("remove root: {error}"))?;
        Ok(())
    }

    #[test]
    fn ingest_surfaces_unresolvable_drive_relative_edits_outside_the_root() -> Result<(), String> {
        // PR review on #5984: a drive-relative `X:name` spelling resolves
        // against that drive's current directory, not the selected root.
        // When it resolves to nothing here, containment cannot be
        // established, so it is surfaced for review instead of silently
        // counting as inside the root.
        let value = render_value(&forbidden_edit_repro_json(
            r#"["tests/test_pricing.py", "Z:definitely-absent-r3/src/pricing.py"]"#,
        ))?;
        assert_eq!(value["classification"]["state"], "closed");
        assert_eq!(value["safety"]["forbidden_edit_flagged"], false);
        assert_eq!(
            value["evidence"]["edited_files_outside_root"],
            serde_json::json!(["Z:definitely-absent-r3/src/pricing.py"])
        );
        Ok(())
    }

    #[test]
    fn ingest_requires_verify_before_closure() -> Result<(), String> {
        let value = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:pricing"},
              "attempt": {"status": "completed", "edited_files": ["tests/test_pricing.py"]},
              "receipt": {"provenance": {"movement": "resolved"}}
            }"#,
        )?;

        assert_eq!(value["classification"]["state"], "uncertain");
        assert_eq!(value["classification"]["outcome"], "receipt_present");
        assert_eq!(value["evidence"]["verify"]["present"], false);
        assert_eq!(value["evidence"]["receipt"]["present"], true);
        // Verify-absent guard fires before the provenance check; reason is missing_verify.
        assert_eq!(
            value["classification"]["reason"].as_str(),
            Some(ingest_reason::MISSING_VERIFY)
        );
        Ok(())
    }

    #[test]
    fn ingest_treats_a_receipt_verification_not_run_as_missing_verify() -> Result<(), String> {
        // #4234: an agent receipt carries `verification.status:
        // "verification_not_run"`. Ingested as-is, that must read as absent
        // verify evidence, never as a present verify result.
        let value = render_value(
            r#"{
              "verification": {"status": "verification_not_run", "commands_run": []},
              "provenance": {"movement": "improved"}
            }"#,
        )?;

        assert_eq!(value["evidence"]["verify"]["present"], false);
        assert_eq!(value["classification"]["state"], "uncertain");
        assert_eq!(
            value["next_action"]["summary"],
            "Run the packet verify command and attach the result before judging closure."
        );
        Ok(())
    }

    #[test]
    fn ingest_classifies_stopped_failed_improved_and_closed_attempts() -> Result<(), String> {
        let stopped = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:stopped"},
              "attempt": {"status": "stopped", "stop_reason": "expected value is ambiguous"}
            }"#,
        )?;
        assert_eq!(stopped["classification"]["state"], "stopped_by_agent");
        assert_eq!(stopped["classification"]["outcome"], "attempted_no_receipt");

        let failed = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:failed"},
              "attempt": {"verify": {"status": "failed", "exit_code": 1}}
            }"#,
        )?;
        assert_eq!(failed["classification"]["state"], "verify_failed");
        assert_eq!(failed["classification"]["outcome"], "attempted_no_receipt");

        // Complete-evidence cases: verify passed + movement + before/after sha256 provenance.
        // These must still surface the correct positive outcome (no false suppression).
        let improved = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:improved"},
              "attempt": {"verify": {"status": "passed", "exit_code": 0}},
              "receipt": {
                "provenance": {
                  "movement": "improved",
                  "before_artifact": {"sha256": "aabbcc0011223344aabbcc0011223344aabbcc0011223344aabbcc0011223344"},
                  "after_artifact": {"sha256": "ddee55667788aaddddee55667788aaddddee55667788aaddddee55667788aadd"}
                }
              }
            }"#,
        )?;
        assert_eq!(improved["classification"]["state"], "partially_improved");
        assert_eq!(improved["classification"]["outcome"], "evidence_improved");

        let closed = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:closed", "staleness_status": "not_evaluated"},
              "attempt": {"verify": {"status": "passed", "exit_code": 0}},
              "receipt": {
                "provenance": {
                  "movement": "resolved",
                  "before_artifact": {"sha256": "aabbcc0011223344aabbcc0011223344aabbcc0011223344aabbcc0011223344"},
                  "after_artifact": {"sha256": "ddee55667788aaddddee55667788aaddddee55667788aaddddee55667788aadd"}
                }
              }
            }"#,
        )?;
        assert_eq!(closed["classification"]["state"], "closed");
        assert_eq!(closed["classification"]["outcome"], "resolved");

        let unchanged = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:unchanged"},
              "attempt": {"verify": {"status": "passed", "exit_code": 0}},
              "receipt": {
                "provenance": {
                  "movement": "unchanged",
                  "before_artifact": {"sha256": "aabbcc0011223344aabbcc0011223344aabbcc0011223344aabbcc0011223344"},
                  "after_artifact": {"sha256": "aabbcc0011223344aabbcc0011223344aabbcc0011223344aabbcc0011223344"}
                }
              }
            }"#,
        )?;
        assert_eq!(unchanged["classification"]["state"], "uncertain");
        assert_eq!(unchanged["classification"]["outcome"], "evidence_unchanged");

        let regressed = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:regressed"},
              "attempt": {"verify": {"status": "passed", "exit_code": 0}},
              "receipt": {
                "provenance": {
                  "movement": "regressed",
                  "before_artifact": {"sha256": "aabbcc0011223344aabbcc0011223344aabbcc0011223344aabbcc0011223344"},
                  "after_artifact": {"sha256": "ddee55667788aaddddee55667788aaddddee55667788aaddddee55667788aadd"}
                }
              }
            }"#,
        )?;
        assert_eq!(regressed["classification"]["state"], "uncertain");
        assert_eq!(regressed["classification"]["outcome"], "evidence_regressed");

        // Stale packet guard fires before the provenance check.
        let stale = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:stale", "staleness_status": "stale"},
              "attempt": {"verify": {"status": "passed", "exit_code": 0}},
              "receipt": {"provenance": {"movement": "resolved"}}
            }"#,
        )?;
        assert_eq!(stale["classification"]["state"], "stale_packet");
        assert_eq!(stale["classification"]["outcome"], "unknown");
        assert_eq!(
            stale["classification"]["reason"].as_str(),
            Some(ingest_reason::STALE_PACKET)
        );
        Ok(())
    }

    // --- Fail-closed tests (RIPR-SPEC-0073 rule 5) -------------------------

    #[test]
    fn fail_closed_movement_claimed_without_before_sha256() -> Result<(), String> {
        // After sha256 present, before sha256 absent → fail closed.
        let value = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:no-before-sha"},
              "attempt": {"verify": {"status": "passed", "exit_code": 0}},
              "receipt": {
                "provenance": {
                  "movement": "resolved",
                  "after_artifact": {"sha256": "ddee55667788aaddddee55667788aaddddee55667788aaddddee55667788aadd"}
                }
              }
            }"#,
        )?;
        assert_eq!(value["attempt_outcome"], "unknown");
        assert_eq!(value["classification"]["outcome"], "unknown");
        assert_eq!(
            value["classification"]["reason"].as_str(),
            Some(ingest_reason::MOVEMENT_WITHOUT_SNAPSHOT_PROVENANCE)
        );
        assert_eq!(
            value["evidence"]["receipt"]["provenance"]["snapshot_provenance_present"],
            false
        );
        Ok(())
    }

    #[test]
    fn fail_closed_movement_claimed_without_after_sha256() -> Result<(), String> {
        // Before sha256 present, after sha256 absent → fail closed.
        let value = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:no-after-sha"},
              "attempt": {"verify": {"status": "passed", "exit_code": 0}},
              "receipt": {
                "provenance": {
                  "movement": "improved",
                  "before_artifact": {"sha256": "aabbcc0011223344aabbcc0011223344aabbcc0011223344aabbcc0011223344"}
                }
              }
            }"#,
        )?;
        assert_eq!(value["attempt_outcome"], "unknown");
        assert_eq!(value["classification"]["outcome"], "unknown");
        assert_eq!(
            value["classification"]["reason"].as_str(),
            Some(ingest_reason::MOVEMENT_WITHOUT_SNAPSHOT_PROVENANCE)
        );
        Ok(())
    }

    #[test]
    fn fail_closed_movement_claimed_without_any_sha256() -> Result<(), String> {
        // Both sha256 absent (existing fixture shape) → fail closed for any movement claim.
        for movement in &["resolved", "improved", "unchanged", "regressed"] {
            let json = format!(
                r#"{{
                  "packet": {{"gap_id": "gap:python:no-sha-{movement}"}},
                  "attempt": {{"verify": {{"status": "passed", "exit_code": 0}}}},
                  "receipt": {{"provenance": {{"movement": "{movement}"}}}}
                }}"#
            );
            let value = render_value(&json)?;
            assert_eq!(
                value["attempt_outcome"], "unknown",
                "movement={movement}: expected unknown outcome when sha256 absent"
            );
            assert_eq!(
                value["classification"]["reason"].as_str(),
                Some(ingest_reason::MOVEMENT_WITHOUT_SNAPSHOT_PROVENANCE),
                "movement={movement}: expected movement_without_snapshot_provenance reason"
            );
        }
        Ok(())
    }

    #[test]
    fn fail_closed_verify_missing_but_movement_claimed() -> Result<(), String> {
        // Verify absent; movement claimed with full sha256 provenance → capped at
        // presence-based outcome (missing_verify fires before provenance check).
        let value = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:no-verify"},
              "receipt": {
                "provenance": {
                  "movement": "resolved",
                  "before_artifact": {"sha256": "aabbcc0011223344aabbcc0011223344aabbcc0011223344aabbcc0011223344"},
                  "after_artifact": {"sha256": "ddee55667788aaddddee55667788aaddddee55667788aaddddee55667788aadd"}
                }
              }
            }"#,
        )?;
        assert_ne!(value["attempt_outcome"], "resolved");
        assert_ne!(value["attempt_outcome"], "evidence_improved");
        assert_eq!(
            value["classification"]["reason"].as_str(),
            Some(ingest_reason::MISSING_VERIFY)
        );
        // Receipt is present, so presence-based outcome is receipt_present, not unknown.
        assert_eq!(value["attempt_outcome"], "receipt_present");
        Ok(())
    }

    #[test]
    fn fail_closed_verify_failed_but_movement_claimed() -> Result<(), String> {
        // Verify failed; movement claimed with full sha256 provenance → capped at
        // presence-based outcome (verify_failed guard fires before provenance check).
        let value = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:verify-fail"},
              "attempt": {"verify": {"status": "failed", "exit_code": 1}},
              "receipt": {
                "provenance": {
                  "movement": "improved",
                  "before_artifact": {"sha256": "aabbcc0011223344aabbcc0011223344aabbcc0011223344aabbcc0011223344"},
                  "after_artifact": {"sha256": "ddee55667788aaddddee55667788aaddddee55667788aaddddee55667788aadd"}
                }
              }
            }"#,
        )?;
        assert_ne!(value["attempt_outcome"], "evidence_improved");
        assert_ne!(value["attempt_outcome"], "resolved");
        assert_eq!(value["classification"]["state"], "verify_failed");
        assert_eq!(
            value["classification"]["reason"].as_str(),
            Some(ingest_reason::MISSING_VERIFY)
        );
        assert_eq!(value["attempt_outcome"], "receipt_present");
        Ok(())
    }

    #[test]
    fn fail_closed_forbidden_edit_with_resolved_claim_and_full_provenance() -> Result<(), String> {
        // Forbidden-edit guard fires first; resolved + full sha256 still → unknown.
        let value = render_value(
            r#"{
              "packet": {
                "gap_id": "gap:python:forbidden-resolved",
                "allowed_files": ["tests/test_pricing.py"],
                "forbidden_files": ["app/pricing.py"]
              },
              "attempt": {
                "status": "completed",
                "edited_files": ["tests/test_pricing.py", "app/pricing.py"],
                "verify": {"status": "passed", "exit_code": 0}
              },
              "receipt": {
                "provenance": {
                  "movement": "resolved",
                  "before_artifact": {"sha256": "aabbcc0011223344aabbcc0011223344aabbcc0011223344aabbcc0011223344"},
                  "after_artifact": {"sha256": "ddee55667788aaddddee55667788aaddddee55667788aaddddee55667788aadd"}
                }
              }
            }"#,
        )?;
        assert_eq!(value["attempt_outcome"], "unknown");
        assert_eq!(value["classification"]["state"], "edited_forbidden_file");
        assert_eq!(
            value["classification"]["reason"].as_str(),
            Some(ingest_reason::FORBIDDEN_EDIT)
        );
        assert_eq!(value["safety"]["forbidden_edit_flagged"], true);
        Ok(())
    }

    #[test]
    fn complete_evidence_unchanged_and_regressed_still_surface() -> Result<(), String> {
        // Verify that unchanged/regressed with full provenance still surface
        // their correct outcomes (no false suppression).
        let unchanged = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:unchanged-complete"},
              "attempt": {"verify": {"status": "passed", "exit_code": 0}},
              "receipt": {
                "provenance": {
                  "movement": "unchanged",
                  "before_artifact": {"sha256": "aabb0011aabb0011aabb0011aabb0011aabb0011aabb0011aabb0011aabb0011"},
                  "after_artifact": {"sha256": "aabb0011aabb0011aabb0011aabb0011aabb0011aabb0011aabb0011aabb0011"}
                }
              }
            }"#,
        )?;
        assert_eq!(unchanged["attempt_outcome"], "evidence_unchanged");
        assert_eq!(unchanged["classification"]["outcome"], "evidence_unchanged");
        assert_eq!(
            unchanged["evidence"]["receipt"]["provenance"]["snapshot_provenance_present"],
            true
        );

        let regressed = render_value(
            r#"{
              "packet": {"gap_id": "gap:python:regressed-complete"},
              "attempt": {"verify": {"status": "passed", "exit_code": 0}},
              "receipt": {
                "provenance": {
                  "movement": "regressed",
                  "before_artifact": {"sha256": "aabb0011aabb0011aabb0011aabb0011aabb0011aabb0011aabb0011aabb0011"},
                  "after_artifact": {"sha256": "ccdd2233ccdd2233ccdd2233ccdd2233ccdd2233ccdd2233ccdd2233ccdd2233"}
                }
              }
            }"#,
        )?;
        assert_eq!(regressed["attempt_outcome"], "evidence_regressed");
        assert_eq!(regressed["classification"]["outcome"], "evidence_regressed");
        Ok(())
    }

    // --- End fail-closed tests -----------------------------------------------

    #[test]
    fn python_preview_closed_agent_result_fixture_matches_expected_json() -> Result<(), String> {
        let input = include_str!(
            "../../../../fixtures/first_successful_pr/python-preview-gap/inputs/agent-results/closed.json"
        );
        let expected = include_str!(
            "../../../../fixtures/first_successful_pr/python-preview-gap/expected/swarm-ingest/closed.json"
        );
        let rendered =
            render_swarm_ingest_json(input, "inputs/agent-results/closed.json", Path::new("."))?;
        let rendered: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("rendered ingest JSON should parse: {err}"))?;
        let expected: Value = serde_json::from_str(expected)
            .map_err(|err| format!("expected ingest JSON should parse: {err}"))?;

        assert_eq!(rendered, expected);
        assert_eq!(rendered["classification"]["state"], "closed");
        assert_eq!(rendered["attempt_outcome"], "resolved");
        assert_eq!(rendered["safety"]["forbidden_edit_flagged"], false);
        assert_eq!(
            rendered["evidence"]["receipt"]["provenance"]["snapshot_provenance_present"],
            true
        );
        Ok(())
    }

    #[test]
    fn movement_without_provenance_fixture_matches_expected_json() -> Result<(), String> {
        let input = include_str!(
            "../../../../fixtures/first_successful_pr/python-preview-gap/inputs/agent-results/movement_without_provenance.json"
        );
        let expected = include_str!(
            "../../../../fixtures/first_successful_pr/python-preview-gap/expected/swarm-ingest/movement_without_provenance.json"
        );
        let rendered = render_swarm_ingest_json(
            input,
            "inputs/agent-results/movement_without_provenance.json",
            Path::new("."),
        )?;
        let rendered: Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("rendered ingest JSON should parse: {err}"))?;
        let expected: Value = serde_json::from_str(expected)
            .map_err(|err| format!("expected ingest JSON should parse: {err}"))?;

        assert_eq!(rendered, expected);
        assert_eq!(rendered["attempt_outcome"], "unknown");
        assert_eq!(
            rendered["classification"]["reason"].as_str(),
            Some(ingest_reason::MOVEMENT_WITHOUT_SNAPSHOT_PROVENANCE)
        );
        assert_eq!(
            rendered["evidence"]["receipt"]["provenance"]["snapshot_provenance_present"],
            false
        );
        Ok(())
    }
}
