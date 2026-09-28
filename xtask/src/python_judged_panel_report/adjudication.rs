//! The typed current-judgment record: identity-checked parsing, locking, and
//! the per-case adjudication read-modify-write path (RIPR-SPEC-0092).

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::python_judged_panel::{
    PythonJudgedPanelItem, RowKind, load_validated_inventory, parse_json_without_duplicate_keys,
    row_kind,
};
use crate::python_judged_panel_replay::{sha256_hex, stable_case_slug};

use super::cli::AdjudicationRequest;
use super::judgment_semantics::{JudgmentSemantics, validate_judgment_semantics};
use super::replay_records::{cite_replay_record, list_json_files};
use super::report::sha256_file_or_blank;
use super::view::{AdjudicationState, derive_adjudication_view};
use super::{
    ADJUDICATE_RERUN, ADJUDICATION_KIND, ADJUDICATION_SCHEMA_VERSION, AUTHORITY_BOUNDARY, SPEC,
};

// ---------------------------------------------------------------------------
// Adjudication: the typed current-judgment record
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AdjudicationRecord {
    pub(super) schema_version: String,
    pub(super) kind: String,
    pub(super) spec: String,
    pub(super) case_id: String,
    pub(super) source_envelope: String,
    pub(super) expected_direction: String,
    pub(super) must_not_claim: Vec<String>,
    /// RIPR's own replay output (`{file, binary_version, binary_sha256,
    /// diff_sha256, candidate_classification}`), cited as advisory reference
    /// only and kept under a dedicated name so the adjudicator's `verdict`
    /// can never be confused with the candidate classification.
    #[serde(default)]
    pub(super) cited_replay_record: Option<Value>,
    pub(super) judgments: Vec<AdjudicationJudgment>,
    /// FIX f2XZN: sha256 over the complete validated row (every field,
    /// serialized) plus the referenced diff content digest at adjudication
    /// time. A stored digest that no longer matches the current row marks the
    /// record `stale_row` at report time.
    #[serde(default)]
    pub(super) row_revision_sha256: String,
    pub(super) authority_boundary: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AdjudicationJudgment {
    pub(super) reviewer_role: String,
    pub(super) reviewer_identity: String,
    pub(super) recorded_at: String,
    /// The adjudicator's own classification in the conservative static
    /// vocabulary, backed by the judgment's own evidence citations — never by
    /// copying the cited candidate classification.
    pub(super) verdict: String,
    #[serde(default)]
    pub(super) false_actionable: Option<bool>,
    #[serde(default)]
    pub(super) false_exposed: Option<bool>,
    #[serde(default)]
    pub(super) wrong_target: Option<bool>,
    #[serde(default)]
    pub(super) invalid_command: Option<bool>,
    #[serde(default)]
    pub(super) limitation_quality: Option<String>,
    pub(super) evidence_references: Vec<String>,
    #[serde(default)]
    pub(super) notes: Option<String>,
}

/// Records the judgment and returns a one-line disposition summary.
/// FIX fqlm (devin round 2): adjudicating one case is a read-modify-write
/// cycle on its record file, so two concurrent reviewers could each publish a
/// replacement and silently discard the other's judgment. FIX fqNv (devin
/// round 4): concurrent report commands can likewise interleave their json
/// and markdown renames into a mixed generation. Both are serialized by one
/// exclusive sibling lock file per contended path; a second concurrent run
/// gets a named error instead. The lock releases on drop, covering every
/// error path after acquisition, and a crash-left stale lock is named in the
/// error so it can be removed deliberately.
pub(super) struct PathLockGuard {
    lock_path: PathBuf,
}

impl Drop for PathLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

pub(super) fn acquire_path_lock(
    lock_path: PathBuf,
    what: &str,
    already_exists: impl FnOnce(&Path) -> String,
) -> Result<PathLockGuard, String> {
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
    {
        Ok(_file) => Ok(PathLockGuard { lock_path }),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(already_exists(&lock_path))
        }
        Err(error) => Err(format!(
            "write {what}: create lock `{}`: {error}",
            lock_path.display()
        )),
    }
}

pub(super) fn acquire_record_lock(record_path: &Path) -> Result<PathLockGuard, String> {
    let file_name = record_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("record file name is not UTF-8: `{}`", record_path.display()))?;
    let parent = record_path
        .parent()
        .ok_or_else(|| format!("no parent directory for `{}`", record_path.display()))?;
    let lock_path = parent.join(format!(".{file_name}.lock"));
    acquire_path_lock(lock_path, "adjudication record", |lock_path| {
        format!(
            "adjudication record `{}` is locked (`{}` exists): another adjudication for this case may be in progress; if none is running, remove the stale lock\nrerun: {ADJUDICATE_RERUN}",
            record_path.display(),
            lock_path.display()
        )
    })
}

pub(crate) fn adjudicate_case_at(
    root: &Path,
    displays: &[&str],
    adjudications_dir: &str,
    records_dir: &str,
    request: &AdjudicationRequest,
) -> Result<String, String> {
    let loaded = load_validated_inventory(root, displays)?;
    // Case identity and slug uniqueness, checked against the full inventory
    // so an ambiguous record file name fails before any judgment is stored.
    let mut slugs = BTreeMap::<String, String>::new();
    let mut row: Option<(&str, &PythonJudgedPanelItem)> = None;
    for file in &loaded {
        for item in &file.envelope.items {
            let slug = stable_case_slug(&item.id);
            if let Some(previous) = slugs.insert(slug.clone(), item.id.clone()) {
                return Err(format!(
                    "case id slug collision: `{previous}` and `{}` both normalize to `{slug}`; rename one case id so every adjudication record file name is unique\nrerun: {ADJUDICATE_RERUN}",
                    item.id
                ));
            }
            if item.id == request.case_id {
                row = Some((file.display.as_str(), item));
            }
        }
    }
    let (source_envelope, item) = row.ok_or_else(|| {
        format!(
            "unknown case id `{}` in the validated inventory\nrerun: {ADJUDICATE_RERUN}",
            request.case_id
        )
    })?;
    validate_request(request, item)?;

    let judgment = AdjudicationJudgment {
        reviewer_role: request.role.clone(),
        reviewer_identity: request.identity.clone(),
        recorded_at: request.recorded_at.clone(),
        verdict: request.verdict.clone(),
        false_actionable: request.false_actionable,
        false_exposed: request.false_exposed,
        wrong_target: request.wrong_target,
        invalid_command: request.invalid_command,
        limitation_quality: request.limitation_quality.clone(),
        evidence_references: request.evidence.clone(),
        notes: request.notes.clone(),
    };

    let record_path = PathBuf::from(adjudications_dir)
        .join(format!("{}.json", stable_case_slug(&request.case_id)));
    // The adjudications directory must exist before the lock file can, so
    // this runs ahead of lock acquisition (FIX fqlm).
    if let Some(parent) = record_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "create adjudications directory `{}`: {error}\nrerun: {ADJUDICATE_RERUN}",
                parent.display()
            )
        })?;
    }
    let _record_lock = acquire_record_lock(&record_path)?;
    // FIX f2XZZ: only a missing record initializes a new one — every other
    // read failure returns a named error so an unreadable record is never
    // truncated or overwritten by a re-adjudication.
    let existing_body = match fs::read_to_string(&record_path) {
        Ok(body) => Some(body),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(format!(
                "read existing adjudication record `{}`: {error}; refusing to overwrite an unreadable record\nrerun: {ADJUDICATE_RERUN}",
                record_path.display()
            ));
        }
    };
    let row_revision = row_revision_sha256(root, item)?;
    let mut record = match existing_body {
        Some(body) => {
            let existing = parse_adjudication_record(&body, &record_path.display().to_string())?;
            if existing.case_id != request.case_id {
                return Err(format!(
                    "adjudication record `{}` declares case `{}` but was addressed for case `{}`",
                    record_path.display(),
                    existing.case_id,
                    request.case_id
                ));
            }
            // FIX f2XZN: the record is bound to the full row revision (every
            // row field plus the referenced diff content); a bound digest that
            // no longer matches means the row or its diff changed after
            // adjudication, and appending to it would mix revisions.
            if existing.row_revision_sha256 != row_revision {
                return Err(format!(
                    "adjudication record `{}` was recorded against a different revision of case `{}` (bound row revision `{}` does not match the current `{row_revision}`); the row or its diff changed after adjudication — re-record every role's judgment against the current row\nrerun: {ADJUDICATE_RERUN}",
                    record_path.display(),
                    request.case_id,
                    existing.row_revision_sha256
                ));
            }
            // FIX fqNa (devin round 4): the stored provenance echoes must
            // match the row this request was validated against, so a drifted
            // record never accumulates judgments under contradictory
            // provenance.
            if existing.source_envelope != source_envelope
                || existing.expected_direction != item.expected_direction
                || existing.must_not_claim != must_not_claim_echo(item)
            {
                return Err(format!(
                    "adjudication record `{}` carries provenance that contradicts the current validated row (envelope `{}` vs `{}`, direction `{}` vs `{}`); re-record every role's judgment against the current row\nrerun: {ADJUDICATE_RERUN}",
                    record_path.display(),
                    existing.source_envelope,
                    source_envelope,
                    existing.expected_direction,
                    item.expected_direction
                ));
            }
            existing
        }
        None => AdjudicationRecord {
            schema_version: ADJUDICATION_SCHEMA_VERSION.to_string(),
            kind: ADJUDICATION_KIND.to_string(),
            spec: SPEC.to_string(),
            case_id: request.case_id.clone(),
            source_envelope: source_envelope.to_string(),
            expected_direction: item.expected_direction.clone(),
            must_not_claim: must_not_claim_echo(item),
            cited_replay_record: None,
            judgments: Vec::new(),
            row_revision_sha256: row_revision.clone(),
            authority_boundary: AUTHORITY_BOUNDARY.to_string(),
        },
    };
    // FIX fqGSD (CodeRabbit #3681): one judgment per role — a role's current
    // judgment replaces whatever that role recorded before (including a
    // re-record under a changed reviewer identity), so role-to-identity stays
    // one-to-one and the record always carries each role's current view.
    record
        .judgments
        .retain(|existing| existing.reviewer_role != request.role);
    // One identity may never occupy two roles: the independence claim behind
    // `adjudicated` would be false. (After the retain above, no remaining
    // judgment carries this role — the role-condition simplification is
    // credited to the gemini review.)
    if record
        .judgments
        .iter()
        .any(|existing| existing.reviewer_identity == request.identity)
    {
        return Err(format!(
            "identity `{}` is already recorded under a different role on case `{}`; one identity must never occupy two roles — independence requires distinct people\nrerun: {ADJUDICATE_RERUN}",
            request.identity, request.case_id
        ));
    }
    record.judgments.push(judgment);
    record.cited_replay_record = cite_replay_record(records_dir, &request.case_id);

    let body = serde_json::to_string_pretty(&record)
        .map_err(|error| format!("serialize adjudication record: {error}"))?;
    write_adjudication_record_atomic(&record_path, &body)?;

    let view = derive_adjudication_view(&record, item, &row_revision);
    Ok(match view.state {
        AdjudicationState::Adjudicated => format!(
            "adjudicated by {} independent roles (verdict `{}`)",
            view.roles.len(),
            view.verdict_agreed.as_deref().unwrap_or("disputed")
        ),
        AdjudicationState::Inconclusive => format!(
            "recorded but inconclusive: {} independent role(s) agree no direction-admitted error axis is decided; inconclusive is a real state, not a pass",
            view.roles.len()
        ),
        AdjudicationState::Disputed => format!(
            "disputed: {} independent role(s) disagree on the terminal judgment; a recorded disposition is required",
            view.roles.len()
        ),
        _ => format!(
            "pending_second_role: {} role(s) recorded so far; a second independent role is required before this counts as adjudicated",
            view.roles.len()
        ),
    })
}

fn validate_request(
    request: &AdjudicationRequest,
    item: &PythonJudgedPanelItem,
) -> Result<(), String> {
    validate_judgment_semantics(JudgmentSemantics {
        role: &request.role,
        identity: &request.identity,
        verdict: &request.verdict,
        false_actionable: request.false_actionable,
        false_exposed: request.false_exposed,
        evidence_references: &request.evidence,
        limitation_quality: request.limitation_quality.as_deref(),
        direction: &item.expected_direction,
        carryover: row_kind(item) == RowKind::Carryover,
    })
}

pub(super) fn must_not_claim_echo(item: &PythonJudgedPanelItem) -> Vec<String> {
    item.must_not_claim.value().cloned().unwrap_or_default()
}

/// Cites the case's replay record when one exists, so the adjudication binds
/// the advisory candidate data it had available. Absence is honest: the
fn parse_adjudication_record(body: &str, display: &str) -> Result<AdjudicationRecord, String> {
    let value = parse_json_without_duplicate_keys(body)
        .map_err(|error| format!("parse adjudication record `{display}`: {error}"))?;
    let record: AdjudicationRecord = serde_json::from_value(value)
        .map_err(|error| format!("parse adjudication record `{display}`: {error}"))?;
    if record.schema_version != ADJUDICATION_SCHEMA_VERSION
        || record.kind != ADJUDICATION_KIND
        || record.spec != SPEC
    {
        return Err(format!(
            "adjudication record `{display}` carries unknown identity (schema `{}`, kind `{}`, spec `{}`); expected {ADJUDICATION_SCHEMA_VERSION}/{ADJUDICATION_KIND}/{SPEC}",
            record.schema_version, record.kind, record.spec
        ));
    }
    if record.authority_boundary != AUTHORITY_BOUNDARY {
        return Err(format!(
            "adjudication record `{display}` carries authority boundary `{}`; expected `{AUTHORITY_BOUNDARY}`",
            record.authority_boundary
        ));
    }
    Ok(record)
}
pub(super) fn read_adjudication_records(
    adjudications_dir: &Path,
) -> Result<BTreeMap<String, AdjudicationRecord>, String> {
    let mut records = BTreeMap::new();
    if !adjudications_dir.is_dir() {
        return Ok(records);
    }
    let mut entries = list_json_files(adjudications_dir)?;
    entries.sort();
    for file_name in entries {
        let path = adjudications_dir.join(&file_name);
        let body = fs::read_to_string(&path)
            .map_err(|error| format!("read adjudication record `{}`: {error}", path.display()))?;
        let record = parse_adjudication_record(&body, &path.display().to_string())?;
        if record.case_id.trim().is_empty() {
            return Err(format!(
                "adjudication record `{}` declares a blank case id",
                path.display()
            ));
        }
        let expected_file = format!("{}.json", stable_case_slug(&record.case_id));
        if expected_file != file_name {
            return Err(format!(
                "adjudication record `{file_name}` declares case `{}` but its record file name must be `{expected_file}`",
                record.case_id
            ));
        }
        records.insert(record.case_id.clone(), record);
    }
    Ok(records)
}
fn write_adjudication_record_atomic(path: &Path, body: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("no parent directory for `{}`", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("record file name is not UTF-8: `{}`", path.display()))?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let temp = parent.join(format!(".{file_name}.tmp-{}-{nanos}", std::process::id()));
    let staged = (|| -> Result<(), String> {
        let mut file = fs::File::create(&temp)
            .map_err(|error| format!("stage `{}`: {error}", temp.display()))?;
        file.write_all(body.as_bytes())
            .map_err(|error| format!("stage `{}`: {error}", temp.display()))?;
        file.sync_all()
            .map_err(|error| format!("flush `{}`: {error}", temp.display()))?;
        Ok(())
    })();
    if let Err(error) = staged {
        let _ = fs::remove_file(&temp);
        return Err(format!(
            "write adjudication record `{}`: {error}
rerun: {ADJUDICATE_RERUN}",
            path.display()
        ));
    }
    if let Err(rename_error) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(format!(
            "publish adjudication record `{}`: rename failed ({rename_error}); the prior record is unchanged\nrerun: {ADJUDICATE_RERUN}",
            path.display()
        ));
    }
    Ok(())
}

/// FIX f2XZN: the row revision an adjudication binds — sha256 over the
/// complete validated row (every field, serialized deterministically) plus
/// the referenced diff content digest. `Nullable::Missing` and explicit null
/// both serialize as null: they carry the same row content here.
pub(super) fn row_revision_sha256(
    root: &Path,
    item: &PythonJudgedPanelItem,
) -> Result<String, String> {
    let mut serialized = serde_json::to_vec(item)
        .map_err(|error| format!("serialize case `{}` for revision binding: {error}", item.id))?;
    serialized.push(0);
    serialized.extend_from_slice(sha256_file_or_blank(&root.join(&item.diff_path)).as_bytes());
    Ok(sha256_hex(&serialized))
}
