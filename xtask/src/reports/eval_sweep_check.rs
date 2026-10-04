//! Typed semantic validation for the accepted Python eval-sweep manifest and
//! retained run receipts — `cargo xtask eval-sweep check` (RIPR-SPEC-0086,
//! issue #3565).
//!
//! One loader owns manifest and run-row semantics here; the live-run path in
//! `eval_sweep.rs` keeps its lenient in-memory parse for sweeps, while this
//! module owns the accepted-artifact contract:
//!
//! - the accepted subject manifest (`python_eval_sweep_manifest`) must carry
//!   schema/kind/spec/tier and **exactly eight** uniquely identified subjects —
//!   the canonical denominator. Subject content is data-driven: any eight
//!   well-formed subjects validate, not just the retained fixture bytes.
//! - retained run receipts (`python_eval_sweep_report`) validate in two owned
//!   shapes: schema `0.2` (what `cargo xtask eval-sweep` writes today — the
//!   historical retained shape) and schema `0.3` (adds the currentness
//!   identities: binary/features/config/profile/input identity,
//!   materialization, detection, corpus-selection and execution states, the
//!   eight-word run status vocabulary, raw/output/evidence digests,
//!   repeat-run comparison identity, and a manifest-digest binding).
//!
//! Parser split (resolved with reason, #3733 review): the strict
//! duplicate-key-rejecting loader here is the check-path contract, while the
//! lenient in-memory parse in `eval_sweep.rs` stays the historical-tolerant
//! report path on purpose — the check path validates retained accepted
//! artifacts, the report path renders in-flight sweeps. The two converge
//! when #3566/#3567 own the refresh/report commands.
//!
//! Design laws (issue #3565 acceptance):
//!
//! - Fail closed on: duplicate or missing subjects, a changed denominator,
//!   unsafe (non-portable, absolute, or secret-bearing) paths, unknown state
//!   vocabulary, contradictory status (including a run-status row whose
//!   execution state says `not-executed`, and a false stability claim whose
//!   unstable list is omitted), disagreeing duplicate identity copies within
//!   one receipt, wrong-typed owned fields, malformed or stale digests, and
//!   hand-edited aggregates that disagree with the derived rows. Every
//!   failure names subject/field/reason and the deterministic rerun command.
//! - Aggregate agreement is checked in both directions. An analyzed
//!   (run-status) row must carry the aggregate source evidence the sweep
//!   records on every row it writes — `runtime_ms`, both distributions, and
//!   the 0.2 row-level `gap_ids_stable`; 0.3 stability lives in the optional
//!   `repeat` block, whose absence is typed incomplete — so a missing field
//!   can never silently disable a summary comparison. A recorded summary
//!   stability value over rows lacking 0.3 `repeat` stability evidence fails;
//!   unrecorded values are disclosed incomplete. Summary distributions must
//!   equal the row-derived key set exactly (zero-valued buckets included;
//!   with zero run rows a recorded distribution must still carry the full
//!   zero-filled key set the emitter writes — `absent` and `unknown`
//!   included — and every recorded bucket stays at zero), and
//!   the supplied `gate_status` must EQUAL the gate derived from the rows —
//!   `not_run` at zero runs, `pass` only with zero crashes and full stability
//!   evidence, `review` otherwise. Runtime totals and distribution merges use
//!   checked arithmetic: overflow is a structured failure naming the
//!   aggregate field, never a panic.
//! - The emitted summary is owned in full (#3733 review). With analyzed
//!   (run-status) rows, every summary aggregate the emitter writes must be
//!   present — a deleted field would silently disable its row-agreement
//!   check. Stability aggregates are required exactly when the rows fully
//!   evidence stability; the under-evidenced path discloses instead of
//!   failing on an omission the emitter could not have written. With zero
//!   run rows the `not_run`/not-a-vacuous-pass law extends to the summary:
//!   every analysis-bearing aggregate (classification/alignment counts,
//!   runtime min/median/max/total, stability counts and rate) must be zero
//!   or absent — any nonzero value is a fabricated claim about rows that
//!   never ran. One emitter-shape exception: the live emitter records
//!   `gap_id_stability_rate: 1.0` on zero-run reports (its empty-set
//!   guard), so exactly that value is accepted as a named `incomplete`
//!   disclosure (`vacuous zero-run stability rate`) — any other nonzero
//!   rate fails, and the disclosed receipt verdict is never a pass.
//! - Identity binding is symmetric (#3733 review). When the accepted
//!   manifest and the receipt both record a comparable identity (`license`,
//!   `tree_digest`, `snapshot`, `provenance`, `retention_class`) and both
//!   are well-formed, they must match — a mismatch fails naming both sides;
//!   a receipt value with no manifest side to bind discloses `incomplete` on
//!   the manifest side instead of fabricating a binding. Optional manifest
//!   identities are either absent (typed incomplete) or well-formed: an
//!   explicit null, an empty string, or a malformed value fails, because a
//!   present-but-garbage identity is not an absent one. The same present-null
//!   rule governs the receipt-side identity fields the binding and copy
//!   checks cover — the row-level binding identities, the receipt-level
//!   `ripr` block, and the fields of a present row `repository`/`binary`
//!   block: a key left out discloses `incomplete`, an explicit null fails
//!   naming the field (#3733 review). Absent
//!   `ripr.features` / `binary.features` disclose incomplete;
//!   present-but-malformed feature sets fail — as does a present `binary`
//!   block missing any of its owned fields (each is a named incomplete
//!   disclosure). Within one receipt, duplicate copies of the same identity
//!   must agree: the row-level `tree_digest`/`snapshot` vs the `repository`
//!   block, and a row's `binary` identity vs the receipt-level `ripr` block —
//!   a disagreement fails naming both locations.
//! - The accepted-manifest schema is closed. The owned top-level keys are
//!   `schema_version`/`kind`/`spec`/`tier`/`description`/`limits`/
//!   `synthetic_diff`/`repos`; the owned per-subject keys are
//!   `id`/`url`/`sha`/`license`/`shape`/`synthetic_diff`/`why` plus the
//!   optional identity fields (`tree_digest`/`snapshot`/`provenance`/
//!   `retention_class`). The canonical fixture carries exactly these keys, so
//!   deny-unknown costs nothing and catches schema rot and typos; every key
//!   the canonical manifest carries is owned here.
//! - Failed, unavailable, timeout, parse-failed, unsupported, partial, and
//!   stale rows **remain selected**: they are valid rows and stay in the
//!   denominator. The validator never treats a bad outcome as an invalid row.
//! - Missing identities are typed `incomplete`, never invented and never
//!   errors — the retained fixture carries no provenance/retention/snapshot
//!   identities by design, and historical 0.2 receipts carry no currentness
//!   fields. Validation never upgrades or rewrites a historical receipt.
//! - `repos_run == 0` is `not_run`, never a vacuous pass: a receipt claiming
//!   `pass` with zero run rows fails, and a receipt-less check reports
//!   `not_run` (which is not a pass).
//! - `absent` stays distinct from emitted `unknown` distributions: a recorded
//!   `alignment_counts` object — row-level or summary, at zero runs included —
//!   must carry both keys separately.
//! - Accepted validation is offline: no repository materialization, no RIPR
//!   execution, and no filesystem lookups beyond the two artifact files
//!   themselves (diff-path existence is a run-time concern, not a structural
//!   one).
//!
//! Exit contract: `check` exits 0 when every present artifact is structurally
//! valid, disclosing `incomplete` identities and a `not_run` receipt dimension
//! in the verdict; it exits nonzero on any fail-closed violation. Top-level
//! verdict precedence spans BOTH artifacts: `not_run` only when no receipt is
//! supplied; with a receipt, `incomplete` whenever the manifest or the receipt
//! discloses incomplete identities (a complete receipt never hides manifest
//! gaps), and `valid` only when both artifacts are structurally valid and
//! carry zero incompletes. The verdict vocabulary is `valid` / `incomplete` /
//! `not_run` — a structural currentness-readiness verdict, never a robustness
//! or adequacy claim.

mod aggregate;
mod cli;
mod manifest;
mod receipt;
mod render;

// ---------------------------------------------------------------------------
// Tests (module named `python_eval_sweep` so
// `cargo test -p xtask python_eval_sweep` selects exactly this module; no
// unwrap/expect — assert macros and Result returns only)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "eval_sweep_check/tests.rs"]
mod python_eval_sweep;

use std::collections::BTreeMap;

use serde_json::Value;

use crate::python_judged_panel::parse_json_without_duplicate_keys;

pub(crate) use cli::run_check;
pub(crate) use manifest::{AcceptedManifest, AcceptedSubject, validate_accepted_manifest};
pub(crate) use receipt::{RUN_STATUSES, validate_run_receipt};

use receipt::ReceiptCheck;

// Re-imported at the facade so `tests.rs` keeps reaching the moved items
// through `super::` exactly as it did when this module was one file; the
// externally re-exported items above are covered there.
#[cfg(test)]
use receipt::{RECEIPT_SCHEMA_0_2, RECEIPT_SCHEMA_0_3, STATUS_VOCABULARY, validate_row};
#[cfg(test)]
use render::render_check_json;

const RERUN_COMMAND: &str = "cargo xtask eval-sweep check";
const MANIFEST_SCHEMA_VERSION: &str = "0.1";
const KNOWN_SPEC: &str = "RIPR-SPEC-0086";
const KNOWN_TIER: &str = "A";
/// The repo-wide conservative static vocabulary (AGENTS.md language rules).
const CLASSIFICATION_VOCABULARY: [&str; 7] = [
    "exposed",
    "weakly_exposed",
    "reachable_unrevealed",
    "no_static_path",
    "infection_unknown",
    "propagation_unknown",
    "static_unknown",
];
const ALIGNMENT_VOCABULARY: [&str; 9] = [
    "direct",
    "alias",
    "changed_sink_token",
    "orthogonal",
    "unknown",
    "absent",
    // The live 0.2 emitter's AlignmentCounts also writes these three
    // repair-packet presence counters; a receipt straight from today's
    // sweep must validate (#3733 review).
    "repair_placement_present",
    "verify_command_present",
    "python_repair_card_present",
];

/// Secret tripwire substrings for path/URL fields (case-insensitive). This is
/// a conservative tripwire, not a secret parser: a hit fails the artifact.
/// Shared with the report route (#3567), whose accepted-artifact hygiene scan
/// reuses the same tripwire list instead of forking a parallel one.
pub(crate) const SECRET_TRIPWIRES: [&str; 18] = [
    "api_key",
    "apikey",
    "api_token",
    "access_token",
    "auth_token",
    "password",
    "passwd",
    "secret",
    "credential",
    "bearer ",
    "private_key",
    "begin rsa private",
    "begin private key",
    "ghp_",
    "gho_",
    "github_pat_",
    "xoxb-",
    "xoxp-",
];
// ---------------------------------------------------------------------------
// Diagnostics and verdicts
// ---------------------------------------------------------------------------

/// One non-failing disclosure: an identity that is absent (typed `incomplete`,
/// never invented) or otherwise not currently established.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Diagnostic {
    subject: String,
    field: String,
    reason: String,
}

impl Diagnostic {
    fn new(subject: &str, field: &str, reason: impl Into<String>) -> Self {
        Self {
            subject: subject.to_string(),
            field: field.to_string(),
            reason: reason.into(),
        }
    }

    /// Renders one disclosure line; shared with the report route (#3567),
    /// which copies the validated candidate's typed disclosures into the
    /// accepted receipt.
    pub(crate) fn render(&self) -> String {
        format!(
            "subject=`{}` field=`{}`: {}",
            self.subject, self.field, self.reason
        )
    }
}

/// Fail-closed error text: names subject/field/reason plus the rerun command.
fn fail(subject: &str, field: &str, reason: impl std::fmt::Display) -> String {
    format!(
        "eval-sweep check failed: subject=`{subject}` field=`{field}`: {reason}\nrerun: {RERUN_COMMAND}"
    )
}

/// Structural verdict. `valid`/`incomplete`/`not_run` exit 0 (with
/// `incomplete` identities disclosed in full); fail-closed violations exit
/// nonzero. None of these is a robustness or adequacy claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    Valid,
    Incomplete,
    NotRun,
}

impl Verdict {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Verdict::Valid => "valid",
            Verdict::Incomplete => "incomplete",
            Verdict::NotRun => "not_run",
        }
    }
}
// ---------------------------------------------------------------------------
// Shared strict-parsing helpers
// ---------------------------------------------------------------------------

/// Reads a file and parses it as JSON with duplicate-key rejection (structural
/// rot fails at load). Returns the parsed value and the sha256 hex of the raw
/// bytes (used for the manifest-digest binding). Shared with the refresh route
/// (#3566), which consumes the same strict loader instead of re-parsing.
pub(crate) fn load_strict_json(display: &str) -> Result<(Value, String), String> {
    let bytes = std::fs::read(display)
        .map_err(|error| fail(display, "file", format!("failed to read: {error}")))?;
    let text = String::from_utf8(bytes)
        .map_err(|error| fail(display, "file", format!("not valid UTF-8: {error}")))?;
    let value = parse_json_without_duplicate_keys(&text)
        .map_err(|error| fail(display, "json", format!("not well-formed JSON: {error}")))?;
    Ok((value, sha256_hex(text.as_bytes())))
}

/// sha256 hex of the exact bytes; shared with the refresh route (#3566) so
/// both surfaces define the digest once.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn as_object<'a>(
    value: &'a Value,
    subject: &str,
    field: &str,
    what: &str,
) -> Result<&'a serde_json::Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| fail(subject, field, format!("{what} must be a JSON object")))
}

/// Rejects keys outside the owned schema (structural rot).
fn reject_unknown_keys(
    object: &serde_json::Map<String, Value>,
    allowed: &[&str],
    subject: &str,
    what: &str,
) -> Result<(), String> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(fail(
                subject,
                key,
                format!("{what} has unknown field `{key}` (denied to catch schema rot and typos)"),
            ));
        }
    }
    Ok(())
}

/// Present-and-non-null string; `None` = absent or explicit null (typed
/// incomplete by the caller). A present non-string or blank value fails.
fn opt_string(
    subject: &str,
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<String>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => {
            if text.trim().is_empty() {
                Err(fail(
                    subject,
                    key,
                    "string field must be non-empty when present",
                ))
            } else {
                Ok(Some(text.clone()))
            }
        }
        Some(_) => Err(fail(subject, key, "field must be a string when present")),
    }
}

/// Present-and-non-null string that may be empty: `stderr_excerpt`'s emitted
/// shape is a plain excerpt string that is legitimately empty when a run
/// wrote no stderr. A present non-string fails; absence is fine.
fn opt_string_allow_empty(
    subject: &str,
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<String>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(fail(subject, key, "field must be a string when present")),
    }
}

fn opt_u64(
    subject: &str,
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<u64>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => match value.as_u64() {
            Some(number) => Ok(Some(number)),
            None => Err(fail(
                subject,
                key,
                "field must be a non-negative integer when present",
            )),
        },
    }
}

fn opt_bool(
    subject: &str,
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<bool>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(flag)) => Ok(Some(*flag)),
        Some(_) => Err(fail(subject, key, "field must be a boolean when present")),
    }
}

fn opt_string_array(
    subject: &str,
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<Vec<String>>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(items)) => {
            let mut out = Vec::new();
            for item in items {
                match item.as_str() {
                    Some(text) if !text.trim().is_empty() => out.push(text.to_string()),
                    _ => {
                        return Err(fail(
                            subject,
                            key,
                            "array field must contain only non-empty strings",
                        ));
                    }
                }
            }
            Ok(Some(out))
        }
        Some(_) => Err(fail(
            subject,
            key,
            "field must be an array of strings when present",
        )),
    }
}

fn opt_distribution(
    subject: &str,
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<BTreeMap<String, u64>>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            let map = value.as_object().ok_or_else(|| {
                fail(
                    subject,
                    key,
                    "distribution must be an object of integer counts",
                )
            })?;
            let mut out = BTreeMap::new();
            for (name, count) in map {
                let number = count.as_u64().ok_or_else(|| {
                    fail(
                        subject,
                        key,
                        format!("distribution count `{name}` must be a non-negative integer"),
                    )
                })?;
                out.insert(name.clone(), number);
            }
            Ok(Some(out))
        }
    }
}

fn opt_number(
    subject: &str,
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<f64>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_f64()
            .map(Some)
            .ok_or_else(|| fail(subject, key, "field must be a number when present")),
    }
}

// ---------------------------------------------------------------------------
// Portable paths, secrets, digests
// ---------------------------------------------------------------------------

/// A portable repo-relative path: forward slashes only, never absolute, no
/// `..` components, and free of secret tripwires.
fn check_portable_path(subject: &str, field: &str, path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err(fail(subject, field, "path must be non-empty"));
    }
    if path.contains('\\') {
        return Err(fail(
            subject,
            field,
            format!("path `{path}` is not portable: backslash separators are not allowed"),
        ));
    }
    if path.starts_with('/') || path.starts_with('\\') || path.starts_with("//") {
        return Err(fail(
            subject,
            field,
            format!("path `{path}` must be repo-relative, not absolute"),
        ));
    }
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return Err(fail(
            subject,
            field,
            format!("path `{path}` must be repo-relative, not a drive-letter absolute path"),
        ));
    }
    if path.split('/').any(|component| component == "..") {
        return Err(fail(
            subject,
            field,
            format!("path `{path}` must not contain `..` components"),
        ));
    }
    check_no_secrets(subject, field, path)
}

/// https URL with a real dotted host, no whitespace anywhere, no embedded
/// credentials, and no secret tripwires. After the scheme check, the host
/// (characters after `https://` up to the first `/`) must be non-empty and
/// carry at least one `.`: a hostless (`https:///path`) or dotless
/// (`https://host`) value has no repository host, so it is malformed — the
/// no-dot rule is deliberate (a bare single-label host is not an accepted
/// repository URL here), not an oversight.
fn check_subject_url(subject: &str, field: &str, url: &str) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err(fail(
            subject,
            field,
            format!("repository url must be https, got `{url}`"),
        ));
    }
    if url.chars().any(char::is_whitespace) {
        return Err(fail(
            subject,
            field,
            format!("repository url must not contain whitespace, got `{url}`"),
        ));
    }
    let authority = match url.strip_prefix("https://") {
        Some(rest) => match rest.split_once('/') {
            Some((host, _path)) => host,
            None => rest,
        },
        None => "",
    };
    if authority.is_empty() {
        return Err(fail(
            subject,
            field,
            format!("repository url `{url}` has no host after the scheme"),
        ));
    }
    if !authority.contains('.') {
        return Err(fail(
            subject,
            field,
            format!(
                "repository url `{url}` has no dotted host (a repository url needs a host like `example.com`)"
            ),
        ));
    }
    if url.contains('@') {
        return Err(fail(
            subject,
            field,
            "repository url must not embed credentials (`user:pass@host`)",
        ));
    }
    check_no_secrets(subject, field, url)
}

fn check_no_secrets(subject: &str, field: &str, text: &str) -> Result<(), String> {
    match secret_tripwire_match(text) {
        Some(what) => Err(fail(subject, field, what)),
        None => Ok(()),
    }
}

/// The shared conservative secret tripwire: returns a description of the
/// matched tripwire shape, or `None` when the text carries none. One list, two
/// consumers (this validator and the #3567 accepted-artifact hygiene scan), so
/// the detection data is never forked.
pub(crate) fn secret_tripwire_match(text: &str) -> Option<String> {
    let lowered = text.to_ascii_lowercase();
    for tripwire in SECRET_TRIPWIRES {
        if lowered.contains(tripwire) {
            return Some(format!(
                "value must not carry a secret-shaped token (matched tripwire `{tripwire}`)"
            ));
        }
    }
    // Cheap O(1) length gate BEFORE the substring find: a text shorter than
    // the key shape can never match, so short values skip the scan entirely.
    // The key-shaped suffix is still measured from the match offset, so a
    // trailing `AKIA` with too few bytes after it is not a hit.
    if text.len() >= 20
        && text
            .find("AKIA")
            .is_some_and(|offset| text[offset..].len() >= 20)
    {
        return Some("value must not carry an AWS-access-key-shaped token".to_string());
    }
    None
}

/// Digest fields are bare lowercase sha256 hex (64 chars); git identity fields
/// are bare lowercase 40-char hex. Absent stays incomplete; malformed fails.
fn check_sha256_digest(subject: &str, field: &str, digest: &str) -> Result<(), String> {
    let ok = digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if ok {
        Ok(())
    } else {
        Err(fail(
            subject,
            field,
            "digest must be bare lowercase sha256 hex (64 characters) when present",
        ))
    }
}

fn check_git_sha(subject: &str, field: &str, sha: &str) -> Result<(), String> {
    let ok = sha.len() == 40
        && sha
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if ok {
        Ok(())
    } else {
        Err(fail(
            subject,
            field,
            "git identity must be a bare lowercase 40-character commit SHA when present",
        ))
    }
}
fn known_value_or_fail(
    subject: &str,
    field: &str,
    value: &str,
    vocabulary: &[&str],
    what: &str,
) -> Result<(), String> {
    if vocabulary.contains(&value) {
        Ok(())
    } else {
        Err(fail(
            subject,
            field,
            format!(
                "unknown {what} `{value}`; known vocabulary: {}",
                vocabulary.join(", ")
            ),
        ))
    }
}
// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

pub(crate) struct CheckOutcome {
    manifest_path: String,
    pub(crate) accepted: AcceptedManifest,
    receipt: Option<ReceiptCheck>,
}

impl CheckOutcome {
    /// Top-level verdict precedence across BOTH artifacts: `not_run` only when
    /// no receipt was supplied; with a receipt, `incomplete` whenever the
    /// manifest or the receipt discloses incomplete identities — a complete
    /// receipt never hides manifest gaps — and `valid` only when both are
    /// structurally valid with zero incompletes.
    pub(crate) fn verdict(&self) -> Verdict {
        match &self.receipt {
            None => Verdict::NotRun,
            Some(receipt) => {
                if self.accepted.incomplete.is_empty() && receipt.incomplete.is_empty() {
                    Verdict::Valid
                } else {
                    Verdict::Incomplete
                }
            }
        }
    }

    fn incomplete(&self) -> Vec<&Diagnostic> {
        let mut all: Vec<&Diagnostic> = self.accepted.incomplete.iter().collect();
        if let Some(receipt) = &self.receipt {
            all.extend(receipt.incomplete.iter());
        }
        all
    }
}

/// The full offline check: load + validate the accepted manifest, then (when
/// supplied) the retained receipt. Writes nothing; `run_check` renders.
/// Shared with the refresh route's end-to-end symmetry tests (#3566).
pub(crate) fn check_artifacts(
    manifest_path: &str,
    runs_path: Option<&str>,
) -> Result<CheckOutcome, String> {
    let (manifest_value, manifest_sha256) = load_strict_json(manifest_path)?;
    let accepted = validate_accepted_manifest(&manifest_value, manifest_sha256)?;

    let receipt = match runs_path {
        None => None,
        Some(path) => {
            let (receipt_value, _receipt_sha) = load_strict_json(path)?;
            Some(validate_run_receipt(
                &receipt_value,
                &accepted.sha256,
                &accepted,
                path,
            )?)
        }
    };

    Ok(CheckOutcome {
        manifest_path: manifest_path.to_string(),
        accepted,
        receipt,
    })
}
