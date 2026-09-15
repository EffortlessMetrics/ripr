//! Replay record reading (PR B output, read back typed) plus the case's
//! advisory replay citation for adjudications (RIPR-SPEC-0092).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::python_judged_panel::parse_json_without_duplicate_keys;
use crate::python_judged_panel_replay::stable_case_slug;

use super::{MISMATCH_VOCABULARY, OUTCOME_VOCABULARY, RECORD_KIND, RECORD_SCHEMA_VERSION, SPEC};

pub(super) fn cite_replay_record(records_dir: &str, case_id: &str) -> Option<Value> {
    let path = Path::new(records_dir).join(format!("{}.json", stable_case_slug(case_id)));
    let record = parse_replay_record_bytes(
        &fs::read_to_string(&path).ok()?,
        &path.display().to_string(),
    )
    .ok()?;
    // FIX fqNz (devin round 4): a file at the expected name is cited only
    // when it declares the requested case; foreign content under the right
    // name is never attributed as this case's advisory evidence.
    if record.case_id != case_id {
        return None;
    }
    Some(json!({
        "file": format!("{}.json", stable_case_slug(case_id)),
        "binary_version": text(record.binary.as_ref()?, "version")?,
        "binary_sha256": text(record.binary.as_ref()?, "sha256")?,
        "diff_sha256": record.diff.as_ref().and_then(|diff| text(diff, "sha256"))?,
        "candidate_classification": record
            .outcome
            .as_ref()
            .and_then(|outcome| text(outcome, "candidate_classification")),
    }))
}
// ---------------------------------------------------------------------------
// Replay record reader (PR B output, read back typed)
// ---------------------------------------------------------------------------

/// The subset of a replay record the report consumes. The `kind` strings are
/// checked against PR B's closed vocabularies; unknown fields (volatile
/// command line, temp workspace paths, stderr detail) are ignored on purpose
/// so they can never leak into report bytes.
#[derive(Debug, Deserialize)]
pub(super) struct ReplayRecordInput {
    pub(super) schema_version: String,
    pub(super) kind: String,
    pub(super) spec: String,
    pub(super) case_id: String,
    #[serde(default)]
    pub(super) row_kind: String,
    #[serde(default)]
    pub(super) binary: Option<Value>,
    #[serde(default)]
    pub(super) diff: Option<Value>,
    #[serde(default)]
    pub(super) outcome: Option<Value>,
    #[serde(default)]
    pub(super) comparison: Option<Value>,
    /// FIX #3677: the anchor identity the subject was reconstructed from
    /// (additive under schema 0.1 — older records simply lack it, and the
    /// report treats them as anchor-stale, never silently current).
    #[serde(default)]
    pub(super) anchor: Option<Value>,
}

/// One replay record projected onto the fields the report is allowed to echo.
pub(super) struct RecordView {
    pub(super) file_name: String,
    pub(super) outcome_kind: String,
    pub(super) candidate: Option<String>,
    pub(super) mismatch_kinds: Vec<String>,
    pub(super) comparison_unavailable: bool,
    pub(super) prior_actual_stale: bool,
    pub(super) binary_version: String,
    pub(super) binary_sha256: String,
    pub(super) diff_sha256: String,
    pub(super) row_kind: String,
    /// The echoed anchor (file, line, owner); `None` for a record written
    /// before the echo existed.
    pub(super) anchor: Option<RecordAnchor>,
}

/// The anchor echo, typed and validated at read time.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RecordAnchor {
    pub(super) file: String,
    pub(super) line: Option<u64>,
    pub(super) owner: String,
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

pub(super) fn parse_replay_record_bytes(
    body: &str,
    display: &str,
) -> Result<ReplayRecordInput, String> {
    let value = parse_json_without_duplicate_keys(body)
        .map_err(|error| format!("parse replay record `{display}`: {error}"))?;
    let record: ReplayRecordInput = serde_json::from_value(value)
        .map_err(|error| format!("parse replay record `{display}`: {error}"))?;
    if record.schema_version != RECORD_SCHEMA_VERSION
        || record.kind != RECORD_KIND
        || record.spec != SPEC
    {
        return Err(format!(
            "replay record `{display}` carries unknown identity (schema `{}`, kind `{}`, spec `{}`); re-run `cargo xtask python-judged-panel replay`",
            record.schema_version, record.kind, record.spec
        ));
    }
    Ok(record)
}

/// The replay record set plus the one shared as-of binary identity.
pub(super) type ReplayRecordSet = (BTreeMap<String, RecordView>, Option<(String, String)>);

/// Reads every `*.json` replay record under `records_dir` (a missing
/// directory means no replay data). All records must share one binary
/// identity: the report discloses a single as-of, and a mixed set cannot be
/// attributed to it.
pub(super) fn read_replay_records(records_dir: &Path) -> Result<ReplayRecordSet, String> {
    let mut views = BTreeMap::new();
    let mut binary_identity: Option<(String, String)> = None;
    if !records_dir.is_dir() {
        return Ok((views, binary_identity));
    }
    let mut entries = list_json_files(records_dir)?;
    entries.sort();
    for file_name in entries {
        let path = records_dir.join(&file_name);
        let body = fs::read_to_string(&path)
            .map_err(|error| format!("read replay record `{}`: {error}", path.display()))?;
        let record = parse_replay_record_bytes(&body, &path.display().to_string())?;
        if record.case_id.trim().is_empty() {
            return Err(format!(
                "replay record `{}` declares a blank case id",
                path.display()
            ));
        }
        // FIX fqoP (devin round 2): a record file is addressable only by its
        // case's stable slug — the same contract the adjudication reader
        // enforces — and a second record for one case id is a contradiction,
        // not a silent overwrite: filename order must never select which
        // evidence the report counts.
        let expected_file = format!("{}.json", stable_case_slug(&record.case_id));
        if file_name != expected_file {
            return Err(format!(
                "replay record `{}` declares case `{}` but is not named `{expected_file}`; rename it or re-run `cargo xtask python-judged-panel replay`",
                path.display(),
                record.case_id
            ));
        }
        if views.contains_key(&record.case_id) {
            return Err(format!(
                "replay record `{}` re-declares case `{}`, which already has a record in this set; keep one record per case and re-run `cargo xtask python-judged-panel replay`",
                path.display(),
                record.case_id
            ));
        }
        let version = record
            .binary
            .as_ref()
            .and_then(|binary| text(binary, "version"))
            .map(str::to_string)
            .ok_or_else(|| {
                format!(
                    "replay record `{}` carries no binary identity; re-run `cargo xtask python-judged-panel replay`",
                    path.display()
                )
            })?;
        let sha256 = require_sha256_digest(
            record
                .binary
                .as_ref()
                .and_then(|binary| text(binary, "sha256")),
            "binary sha256",
            &path.display().to_string(),
        )?;
        let identity = (version.clone(), sha256.clone());
        match &binary_identity {
            Some(existing) if *existing != identity => {
                return Err(format!(
                    "replay records under `{}` carry mixed binary identity (`{}` vs `{}`); re-run `cargo xtask python-judged-panel replay` so the record set has one as-of identity",
                    records_dir.display(),
                    existing.0,
                    identity.0
                ));
            }
            Some(_) => {}
            None => binary_identity = Some(identity.clone()),
        }
        let outcome = record.outcome.as_ref().ok_or_else(|| {
            format!(
                "replay record `{}` carries no typed outcome; re-run `cargo xtask python-judged-panel replay`",
                path.display()
            )
        })?;
        let outcome_kind = text(outcome, "kind")
            .filter(|kind| OUTCOME_VOCABULARY.contains(kind))
            .map(str::to_string)
            .ok_or_else(|| {
                format!(
                    "replay record `{}` carries unknown outcome kind; re-run `cargo xtask python-judged-panel replay`",
                    path.display()
                )
            })?;
        let candidate = text(outcome, "candidate_classification").map(str::to_string);
        let comparison = record.comparison.as_ref().ok_or_else(|| {
            format!(
                "replay record `{}` carries no typed comparison; re-run `cargo xtask python-judged-panel replay`",
                path.display()
            )
        })?;
        let comparison_kind = text(comparison, "kind").unwrap_or_default();
        if comparison_kind != "available" && comparison_kind != "comparison_unavailable" {
            return Err(format!(
                "replay record `{}` carries unknown comparison kind; re-run `cargo xtask python-judged-panel replay`",
                path.display()
            ));
        }
        let mut mismatch_kinds = Vec::new();
        for mismatch in comparison
            .get("mismatches")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let kind = text(mismatch, "kind").unwrap_or_default();
            if !MISMATCH_VOCABULARY.contains(&kind) {
                return Err(format!(
                    "replay record `{}` carries unknown mismatch kind; re-run `cargo xtask python-judged-panel replay`",
                    path.display()
                ));
            }
            mismatch_kinds.push(kind.to_string());
        }
        let view = RecordView {
            file_name,
            outcome_kind,
            candidate,
            mismatch_kinds,
            comparison_unavailable: comparison_kind == "comparison_unavailable",
            prior_actual_stale: comparison
                .get("stale")
                .is_some_and(|stale| !stale.is_null()),
            binary_version: identity.0.clone(),
            binary_sha256: identity.1.clone(),
            diff_sha256: require_sha256_digest(
                record.diff.as_ref().and_then(|diff| text(diff, "sha256")),
                "diff sha256",
                &path.display().to_string(),
            )?,
            anchor: require_anchor_echo(record.anchor.as_ref(), &path.display().to_string())?,
            row_kind: record.row_kind,
        };
        views.insert(record.case_id, view);
    }
    Ok((views, binary_identity))
}

/// FIX #3677: a present echo must be well-shaped (non-blank file and owner,
/// integer line when present); absence is legal — it marks a record written
/// before the echo existed, and the report consumes that as anchor-staleness,
/// never as currency. File and owner are stored verbatim (round-2 review):
/// the echo is producer-copied row data, and trimming here would break the
/// comparison against a row whose declared value carries padding.
fn require_anchor_echo(
    value: Option<&Value>,
    display: &str,
) -> Result<Option<RecordAnchor>, String> {
    let Some(anchor) = value else {
        return Ok(None);
    };
    if !anchor.is_object() {
        return Err(format!(
            "replay record `{display}` carries a malformed anchor echo (expected an object with file, line, owner); re-run `cargo xtask python-judged-panel replay`"
        ));
    }
    let non_blank = |key: &str| -> Result<String, String> {
        let raw = text(anchor, key).ok_or_else(|| {
            format!(
                "replay record `{display}` carries an anchor echo without a {key}; re-run `cargo xtask python-judged-panel replay`"
            )
        })?;
        if raw.trim().is_empty() {
            return Err(format!(
                "replay record `{display}` carries an anchor echo with a blank {key}; re-run `cargo xtask python-judged-panel replay`"
            ));
        }
        Ok(raw.to_string())
    };
    let line = match anchor.get("line") {
        None | Some(Value::Null) => None,
        Some(value) => value.as_u64().map(Some).ok_or_else(|| {
            format!(
                "replay record `{display}` carries a malformed anchor echo line (`{value}` is not an integer); re-run `cargo xtask python-judged-panel replay`"
            )
        })?,
    };
    Ok(Some(RecordAnchor {
        file: non_blank("file")?,
        line,
        owner: non_blank("owner")?,
    }))
}

/// FIX (#3686, CodeRabbit #3685): digests bind evidence identity, so a blank
/// or malformed one must fail the read with a named error instead of
/// participating in a blank-to-blank currency match at report time (a
/// hand-edited blank digest plus a missing diff file would otherwise let a
/// stale replay report as current). Lowercase only: the producer's
/// `sha256_file_or_blank` emits lowercase, and an accepted uppercase variant
/// could never match, silently reporting every such record stale (round-2
/// review).
fn require_sha256_digest(value: Option<&str>, what: &str, display: &str) -> Result<String, String> {
    let digest = value.ok_or_else(|| {
        format!(
            "replay record `{display}` carries no {what}; re-run `cargo xtask python-judged-panel replay`"
        )
    })?;
    let lowercase_hex = digest
        .as_bytes()
        .iter()
        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());
    if digest.len() != 64 || !lowercase_hex {
        return Err(format!(
            "replay record `{display}` carries a malformed {what} (`{digest}` is not a 64-character lowercase hex sha256); re-run `cargo xtask python-judged-panel replay`"
        ));
    }
    Ok(digest.to_string())
}

pub(super) fn list_json_files(dir: &Path) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for entry in
        fs::read_dir(dir).map_err(|error| format!("read directory `{}`: {error}", dir.display()))?
    {
        let path = entry
            .map_err(|error| format!("read directory entry in `{}`: {error}", dir.display()))?
            .path();
        if path.is_file()
            && path.extension().and_then(|extension| extension.to_str()) == Some("json")
        {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| format!("record file name is not UTF-8: `{}`", path.display()))?;
            names.push(name.to_string());
        }
    }
    Ok(names)
}
