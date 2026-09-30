//! Acceptance-time validation and hygiene gates (RIPR-SPEC-0086): the
//! validated-candidate row projection (`read_candidate_rows`, built only
//! after `validate_run_receipt` passed), the typed dispositions sidecar
//! (closed deny-unknown schema, actionable-by-definition vocabulary,
//! required owner/recovery route, bounded free text), disposition coverage
//! over non-complete rows, and the accepted-artifact hygiene scan (shared
//! secret tripwires, absolute host paths) applied to every rendered
//! artifact before a byte is written.

use std::collections::BTreeMap;

use serde_json::Value;

use super::{NOTE_MAX_CHARS, SPEC, as_object, fail, reject_unknown_keys};
use crate::reports::eval_sweep_check::{
    RUN_STATUSES, load_strict_json, secret_tripwire_match, sha256_hex,
};

const DISPOSITIONS_SCHEMA: &str = "0.1";
pub(super) const DISPOSITIONS_KIND: &str = "python_eval_sweep_dispositions";
/// The closed dispositions-sidecar schema (deny-unknown).
const DISPOSITIONS_KEYS: [&str; 4] = ["schema_version", "kind", "spec", "dispositions"];
const DISPOSITION_ENTRY_KEYS: [&str; 6] = [
    "id",
    "disposition",
    "evidence_ref",
    "owner",
    "recovery_route",
    "notes",
];

/// Typed terminal dispositions for non-complete subjects (issue #3567). Every
/// member is terminal (no open/pending state) and actionable by definition,
/// so an owner and a recovery route are REQUIRED on every disposition; a
/// disposition without them fails closed.
const DISPOSITION_VOCABULARY: [&str; 6] = [
    // The failure was reproduced against the current accepted source/binary.
    "reproduced-current",
    // Reviewed against the current accepted source; explicitly dispositioned
    // without a live reproduction.
    "dispositioned-current",
    // A contained infrastructure failure; recovery reruns the managed refresh.
    "infrastructure-tempfail",
    // The upstream pin is unavailable or moved; recovery re-pins the manifest.
    "upstream-pin-unavailable",
    // The analyzer cannot support the input; recovery is scoped support work.
    "unsupported-input",
    // A retained historical failure explicitly dispositioned against current
    // source without a live reproduction.
    "historical-not-reproduced",
];
// ---------------------------------------------------------------------------
// Accepted-artifact hygiene
// ---------------------------------------------------------------------------

/// Scans rendered accepted-artifact text for absolute host paths. A
/// conservative tripwire, not a path parser: `file://` URLs, POSIX absolute
/// JSON string values, and drive-letter paths whose letter is not preceded by
/// another letter (so the `s:` inside an `https://` scheme never trips while
/// a Windows path prefix does).
fn absolute_path_tripwire(text: &str) -> Option<String> {
    if text.contains("file://") {
        return Some("artifact carries a `file://` URL".to_string());
    }
    let bytes = text.as_bytes();
    for (index, window) in bytes.windows(3).enumerate() {
        let drive_letter = window[1] == b':'
            && (window[2] == b'\\' || window[2] == b'/')
            && window[0].is_ascii_alphabetic()
            && (index == 0 || !bytes[index - 1].is_ascii_alphabetic());
        if drive_letter {
            return Some("artifact carries a drive-letter absolute path".to_string());
        }
    }
    for needle in ["\": \"/", "\":\"/", " /home/", " /Users/", " /tmp/"] {
        if text.contains(needle) {
            return Some("artifact carries a POSIX absolute host path".to_string());
        }
    }
    None
}

/// The accepted-artifact hygiene scan: secret-shaped tokens (the shared
/// validator tripwire list) and absolute host paths. Applied to every
/// rendered accepted artifact before a byte is written; a hit is a typed
/// refusal naming the artifact.
pub(super) fn check_artifact_hygiene(artifact: &str, name: &str) -> Result<(), String> {
    if let Some(what) = secret_tripwire_match(artifact) {
        return Err(fail(
            name,
            "hygiene",
            format!("{what} — accepted artifacts must not carry secrets"),
        ));
    }
    if let Some(what) = absolute_path_tripwire(artifact) {
        return Err(fail(
            name,
            "hygiene",
            format!("{what} — accepted artifacts must not carry absolute host paths"),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Validated-candidate row projection
// ---------------------------------------------------------------------------

/// One validated candidate row, reduced to the facts the accepted receipt
/// projects. Built ONLY after `validate_run_receipt` passed, so recorded
/// values are well-typed; absent values stay `None` (typed incomplete
/// downstream, never invented).
pub(super) struct CandidateRowFacts {
    pub(super) id: String,
    pub(super) status: String,
    pub(super) counts_as_run: bool,
    pub(super) license: Option<String>,
    pub(super) runtime_ms: Option<u64>,
    pub(super) materialization: Option<String>,
    pub(super) detection: Option<String>,
    pub(super) corpus_state: Option<String>,
    pub(super) tree_digest: Option<String>,
    pub(super) snapshot: Option<String>,
    pub(super) selected_root: Option<String>,
    pub(super) input_digest: Option<String>,
    pub(super) config_profile: Option<String>,
    pub(super) config_input: Option<String>,
    pub(super) digest_raw: Option<String>,
    pub(super) digest_output: Option<String>,
    pub(super) digest_evidence: Option<String>,
    pub(super) repeat_comparable_with: Option<String>,
    pub(super) repeat_gap_ids_stable: Option<bool>,
    pub(super) repeat_unstable_gap_ids: Option<Vec<String>>,
    pub(super) classification: Option<BTreeMap<String, u64>>,
    pub(super) alignment: Option<BTreeMap<String, u64>>,
    /// sha256 over the row's canonical JSON — the accepted-row identity the
    /// pointer binds so any accepted-row byte change flips currentness.
    pub(super) row_sha256: String,
}

fn read_distribution(
    row: &serde_json::Map<String, Value>,
    key: &str,
) -> Option<BTreeMap<String, u64>> {
    let value = row.get(key)?;
    let map = value.as_object()?;
    let mut out = BTreeMap::new();
    for (name, count) in map {
        out.insert(name.clone(), count.as_u64().unwrap_or(0));
    }
    Some(out)
}

fn opt_nested_string(
    row: &serde_json::Map<String, Value>,
    block: &str,
    key: &str,
) -> Option<String> {
    row.get(block)?
        .as_object()?
        .get(key)?
        .as_str()
        .map(str::to_string)
}

/// Projects the validated candidate's rows. Validation already passed, so
/// this is a total projection over well-typed values.
pub(super) fn read_candidate_rows(candidate: &Value) -> Result<Vec<CandidateRowFacts>, String> {
    let rows = candidate
        .get("repos")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            fail(
                "candidate",
                "repos",
                "validated candidate must carry a repos array",
            )
        })?;
    let mut facts = Vec::new();
    for row in rows {
        let entry = as_object(row, "candidate", "repos", "validated receipt row")?;
        let id = entry
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let status = entry
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let repeat = match entry.get("repeat") {
            Some(Value::Object(repeat)) => Some(repeat),
            _ => None,
        };
        let (repeat_comparable_with, repeat_gap_ids_stable, repeat_unstable_gap_ids) = match repeat
        {
            Some(repeat) => (
                repeat
                    .get("comparable_with")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                repeat.get("gap_ids_stable").and_then(Value::as_bool),
                repeat
                    .get("unstable_gap_ids")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect::<Vec<_>>()
                    }),
            ),
            None => (None, None, None),
        };
        let digests = match entry.get("digests") {
            Some(Value::Object(digests)) => Some(digests),
            _ => None,
        };
        let (digest_raw, digest_output, digest_evidence) = match digests {
            Some(digests) => (
                digests
                    .get("raw")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                digests
                    .get("output")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                digests
                    .get("evidence")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            ),
            None => (None, None, None),
        };
        // Canonical row digest over the exact validated row bytes (serde_json
        // maps iterate in sorted key order, so this is deterministic).
        let canonical = serde_json::to_vec_pretty(row).map_err(|error| {
            fail(
                &id,
                "row",
                format!("cannot canonicalize the validated row: {error}"),
            )
        })?;
        facts.push(CandidateRowFacts {
            counts_as_run: RUN_STATUSES.contains(&status.as_str()),
            license: entry
                .get("license")
                .and_then(Value::as_str)
                .map(str::to_string),
            runtime_ms: entry.get("runtime_ms").and_then(Value::as_u64),
            materialization: entry
                .get("materialization")
                .and_then(Value::as_str)
                .map(str::to_string),
            detection: entry
                .get("detection")
                .and_then(Value::as_str)
                .map(str::to_string),
            corpus_state: opt_nested_string(entry, "corpus_selection", "state"),
            tree_digest: entry
                .get("tree_digest")
                .and_then(Value::as_str)
                .map(str::to_string),
            snapshot: entry
                .get("snapshot")
                .and_then(Value::as_str)
                .map(str::to_string),
            selected_root: entry
                .get("selected_root")
                .and_then(Value::as_str)
                .map(str::to_string),
            input_digest: entry
                .get("input_digest")
                .and_then(Value::as_str)
                .map(str::to_string),
            config_profile: opt_nested_string(entry, "config", "profile"),
            config_input: opt_nested_string(entry, "config", "input"),
            digest_raw,
            digest_output,
            digest_evidence,
            repeat_comparable_with,
            repeat_gap_ids_stable,
            repeat_unstable_gap_ids,
            classification: read_distribution(entry, "classification_counts"),
            alignment: read_distribution(entry, "alignment_counts"),
            row_sha256: sha256_hex(&canonical),
            id,
            status,
        });
    }
    Ok(facts)
}

// ---------------------------------------------------------------------------
// Dispositions sidecar
// ---------------------------------------------------------------------------

pub(super) struct Disposition {
    pub(super) disposition: String,
    pub(super) evidence_ref: String,
    pub(super) owner: String,
    pub(super) recovery_route: String,
    pub(super) notes: Option<String>,
}

/// Reads + validates the dispositions sidecar. Fails closed on unknown keys,
/// unknown ids, duplicates, dispositions for complete rows, unknown
/// vocabulary, missing owner/recovery route (every owned disposition type is
/// actionable by definition), hygiene violations, and oversized free text
/// (every bounded-artifact field is capped, not just notes).
pub(super) fn load_dispositions(
    path: &str,
    rows: &[CandidateRowFacts],
) -> Result<BTreeMap<String, Disposition>, String> {
    let (value, _sha) = load_strict_json(path)?;
    let top = as_object(&value, path, "dispositions", "dispositions sidecar")?;
    reject_unknown_keys(top, &DISPOSITIONS_KEYS, path, "dispositions sidecar")?;
    for (field, expected) in [
        ("schema_version", DISPOSITIONS_SCHEMA),
        ("kind", DISPOSITIONS_KIND),
        ("spec", SPEC),
    ] {
        let actual = top
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| fail(path, field, "dispositions sidecar must declare this field"))?;
        if actual != expected {
            return Err(fail(
                path,
                field,
                format!("expected `{expected}`, got `{actual}`"),
            ));
        }
    }
    let entries = top
        .get("dispositions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            fail(
                path,
                "dispositions",
                "must be an array of disposition entries",
            )
        })?;

    let mut by_id: BTreeMap<String, Disposition> = BTreeMap::new();
    for entry in entries {
        let object = as_object(entry, path, "dispositions[]", "disposition entry")?;
        reject_unknown_keys(object, &DISPOSITION_ENTRY_KEYS, path, "disposition entry")?;
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| fail(path, "dispositions[].id", "must name a subject id"))?
            .to_string();
        let disposition = object
            .get("disposition")
            .and_then(Value::as_str)
            .ok_or_else(|| fail(&id, "disposition", "must declare a typed disposition"))?
            .to_string();
        if !DISPOSITION_VOCABULARY.contains(&disposition.as_str()) {
            return Err(fail(
                &id,
                "disposition",
                format!(
                    "unknown disposition `{disposition}`; known vocabulary: {}",
                    DISPOSITION_VOCABULARY.join(", ")
                ),
            ));
        }
        // Every owned disposition type is terminal and actionable by
        // definition, so the evidence reference, owner, and recovery route
        // are required — a disposition without them is an unresolved
        // follow-up wearing a terminal label, which fails closed. Each is
        // also a bounded-artifact field: an unlimited-length free-text value
        // would defeat the bounded-artifact contract the same way an
        // oversized note would, so every field is capped.
        for field in ["evidence_ref", "owner", "recovery_route"] {
            let text = object
                .get(field)
                .and_then(Value::as_str)
                .filter(|text| !text.trim().is_empty())
                .ok_or_else(|| {
                    fail(
                        &id,
                        field,
                        format!(
                            "disposition `{disposition}` is actionable and requires a non-empty {field}"
                        ),
                    )
                })?;
            if text.chars().count() > NOTE_MAX_CHARS {
                return Err(fail(
                    &id,
                    field,
                    format!(
                        "{field} exceeds the {NOTE_MAX_CHARS}-character bound ({} characters); accepted artifacts carry bounded excerpts, never unbounded logs",
                        text.chars().count()
                    ),
                ));
            }
            check_artifact_hygiene(text, &format!("dispositions[{id}].{field}"))?;
        }
        if let Some(notes) = object.get("notes") {
            let notes = notes
                .as_str()
                .ok_or_else(|| fail(&id, "notes", "notes must be a string when present"))?;
            if notes.chars().count() > NOTE_MAX_CHARS {
                return Err(fail(
                    &id,
                    "notes",
                    format!(
                        "notes exceed the {NOTE_MAX_CHARS}-character bound ({} characters); accepted artifacts carry bounded excerpts, never unbounded logs",
                        notes.chars().count()
                    ),
                ));
            }
            check_artifact_hygiene(notes, &format!("dispositions[{id}].notes"))?;
        }
        let row = rows.iter().find(|row| row.id == id).ok_or_else(|| {
            fail(
                &id,
                "id",
                "disposition names a subject outside the candidate denominator",
            )
        })?;
        if row.status == "complete" {
            return Err(fail(
                &id,
                "disposition",
                "subject is complete; a terminal disposition contradicts a complete run",
            ));
        }
        let inserted = Disposition {
            disposition,
            evidence_ref: object
                .get("evidence_ref")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            owner: object
                .get("owner")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            recovery_route: object
                .get("recovery_route")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            notes: object
                .get("notes")
                .and_then(Value::as_str)
                .map(str::to_string),
        };
        if by_id.insert(id.clone(), inserted).is_some() {
            return Err(fail(&id, "id", "duplicate disposition for one subject"));
        }
    }
    Ok(by_id)
}

/// Enforces disposition coverage: every non-complete row carries exactly one
/// disposition (a non-complete subject without one is an unexplained failure
/// in accepted evidence).
pub(super) fn require_disposition_coverage(
    rows: &[CandidateRowFacts],
    dispositions: &BTreeMap<String, Disposition>,
) -> Result<(), String> {
    for row in rows {
        if row.status == "complete" {
            continue;
        }
        if !dispositions.contains_key(&row.id) {
            return Err(fail(
                &row.id,
                "disposition",
                format!(
                    "status `{}` is non-complete and carries no terminal disposition; supply --dispositions with one typed disposition for every non-complete subject",
                    row.status
                ),
            ));
        }
    }
    Ok(())
}
