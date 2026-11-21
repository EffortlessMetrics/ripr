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

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::python_judged_panel::parse_json_without_duplicate_keys;

const DEFAULT_MANIFEST: &str = "fixtures/python-eval-sweep/manifest.json";
const RERUN_COMMAND: &str = "cargo xtask eval-sweep check";
const CHECK_REPORT_JSON: &str = "eval-sweep-check.json";
const CHECK_REPORT_MD: &str = "eval-sweep-check.md";

const MANIFEST_KIND: &str = "python_eval_sweep_manifest";
const MANIFEST_SCHEMA_VERSION: &str = "0.1";

/// The closed accepted-manifest schema (deny-unknown). The canonical
/// `fixtures/python-eval-sweep/manifest.json` carries exactly the top-level
/// keys below, so every key it carries is owned and unknown keys are schema
/// rot, not forward compatibility.
const MANIFEST_KEYS: [&str; 8] = [
    "schema_version",
    "kind",
    "spec",
    "tier",
    "description",
    "limits",
    "synthetic_diff",
    "repos",
];
/// Owned per-subject keys: everything the canonical repos carry plus the
/// optional identity fields whose absence is typed incomplete.
const MANIFEST_REPO_KEYS: [&str; 11] = [
    "id",
    "url",
    "sha",
    "license",
    "shape",
    "synthetic_diff",
    "why",
    "tree_digest",
    "snapshot",
    "provenance",
    "retention_class",
];

const REPORT_KIND: &str = "python_eval_sweep_report";
const KNOWN_SPEC: &str = "RIPR-SPEC-0086";
const KNOWN_TIER: &str = "A";
const ACCEPTED_SUBJECT_COUNT: usize = 8;

/// Receipt schemas this loader owns: `0.2` is the historical shape the sweep
/// command writes; `0.3` adds the currentness identities.
const RECEIPT_SCHEMA_0_2: &str = "0.2";
const RECEIPT_SCHEMA_0_3: &str = "0.3";

/// Subject shape/layout tags (fixtures/python-eval-sweep/SPEC.md).
const KNOWN_SHAPES: [&str; 5] = [
    "pytest_library",
    "unittest_library",
    "click_typer",
    "fastapi_web",
    "flask_web",
];

/// The complete 0.3 run-status vocabulary (issue #3565). A row outside this
/// set fails; every member stays selected in the denominator.
const STATUS_VOCABULARY: [&str; 8] = [
    "complete",
    "partial",
    "parse-failed",
    "timed-out",
    "crashed",
    "unsupported",
    "tempfail",
    "stale",
];
/// Statuses that evidence an analysis attempt (count toward `repos_run`):
/// exactly these five. The complement — `unsupported`/`tempfail`/`stale` —
/// does not count, though every status stays selected in the denominator
/// (SPEC-0086). Shared with the report route (#3567), which projects the same
/// run/non-run split into the accepted receipt instead of re-deriving it.
pub(crate) const RUN_STATUSES: [&str; 5] = [
    "complete",
    "partial",
    "parse-failed",
    "timed-out",
    "crashed",
];

/// The complete 0.2 historical outcome vocabulary (the shape the sweep
/// command writes; `eval_sweep.rs::Outcome`).
const HISTORICAL_OUTCOMES: [&str; 6] = [
    "ok",
    "parse_failure",
    "timed_out",
    "crash",
    "clone_failed",
    "skipped_missing_checkout",
];

const MATERIALIZATION_STATES: [&str; 6] = [
    "materialized",
    "snapshot",
    "absent",
    "failed",
    "skipped",
    "unknown",
];
const DETECTION_STATES: [&str; 4] = ["detected", "failed", "unknown", "absent"];
const CORPUS_SELECTION_STATES: [&str; 5] = ["selected", "partial", "failed", "unknown", "absent"];
const EXECUTION_STATES: [&str; 5] = ["executed", "failed", "timed-out", "not-executed", "unknown"];
const BUILD_PROFILES: [&str; 2] = ["debug", "release"];
const GATE_STATUSES: [&str; 3] = ["not_run", "pass", "review"];

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
// Args
// ---------------------------------------------------------------------------

struct CheckArgs {
    manifest: String,
    runs: Option<String>,
}

fn parse_check_args(args: &[String]) -> Result<CheckArgs, String> {
    let mut parsed = CheckArgs {
        manifest: DEFAULT_MANIFEST.to_string(),
        runs: None,
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                index += 1;
                parsed.manifest = args.get(index).cloned().ok_or_else(|| {
                    format!("eval-sweep check --manifest requires a value\nrerun: {RERUN_COMMAND}")
                })?;
            }
            "--runs" => {
                index += 1;
                parsed.runs = Some(args.get(index).cloned().ok_or_else(|| {
                    format!("eval-sweep check --runs requires a value\nrerun: {RERUN_COMMAND}")
                })?);
            }
            other => {
                return Err(format!(
                    "unknown eval-sweep check argument: {other}\nusage: cargo xtask eval-sweep check [--manifest <path>] [--runs <path>]\nrerun: {RERUN_COMMAND}"
                ));
            }
        }
        index += 1;
    }
    Ok(parsed)
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

/// Subject ids become filesystem path components — the candidate subject dirs
/// and the raw/cache dirs the refresh route (#3566) derives from them — so
/// they must be safe identifiers: at most 64 characters, limited to
/// `[A-Za-z0-9._-]`, and never starting with `.` (which also bars `.`/`..`
/// and hidden components). The character set bars `/` and `\\`, so a hostile
/// id like `../../outside` fails here as a named diagnostic instead of
/// escaping candidate storage downstream.
const SUBJECT_ID_MAX_LENGTH: usize = 64;

fn check_subject_id(id: &str) -> Result<(), String> {
    if id.len() > SUBJECT_ID_MAX_LENGTH {
        return Err(fail(
            id,
            "id",
            format!(
                "subject id must be at most {SUBJECT_ID_MAX_LENGTH} characters, got {}",
                id.len()
            ),
        ));
    }
    if id.starts_with('.') {
        return Err(fail(
            id,
            "id",
            "subject id must not start with `.`: hidden or relative path components are not safe identifiers",
        ));
    }
    if !id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'_' || byte == b'-')
    {
        return Err(fail(
            id,
            "id",
            "subject id must use only `[A-Za-z0-9._-]`: path separators, whitespace, and other characters are not safe identifiers",
        ));
    }
    Ok(())
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
// Accepted subject manifest
// ---------------------------------------------------------------------------

/// One accepted subject: the immutable identity the denominator is built from.
/// The optional identity fields carry the manifest-side values receipt rows
/// bind against (`None` = not recorded, typed incomplete). Fields are shared
/// with the refresh route (#3566), which consumes the validated subjects
/// without re-parsing the manifest.
#[derive(Debug, Clone)]
pub(crate) struct AcceptedSubject {
    pub(crate) id: String,
    pub(crate) url: String,
    pub(crate) sha: String,
    pub(crate) license: String,
    pub(crate) shape: String,
    /// The resolved synthetic-diff path (per-repo, or the manifest-level
    /// fallback) as a portable repo-relative path. Retained so the refresh
    /// route (#3566) consumes the resolved input identity instead of
    /// re-parsing the manifest.
    pub(crate) synthetic_diff: String,
    pub(crate) tree_digest: Option<String>,
    pub(crate) snapshot: Option<String>,
    pub(crate) provenance: Option<String>,
    pub(crate) retention_class: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct AcceptedManifest {
    pub(crate) sha256: String,
    pub(crate) subjects: Vec<AcceptedSubject>,
    /// Identities the retained manifest does not carry by design; disclosed as
    /// `incomplete`, never invented.
    pub(crate) incomplete: Vec<Diagnostic>,
}

impl AcceptedManifest {
    pub(crate) fn subject(&self, id: &str) -> Option<&AcceptedSubject> {
        self.subjects.iter().find(|entry| entry.id == id)
    }

    pub(crate) fn ids(&self) -> Vec<String> {
        self.subjects.iter().map(|entry| entry.id.clone()).collect()
    }
}

/// Reads a `synthetic_diff` field at one manifest level. Absent or null is
/// `None`; a present value must be a string portable path. A malformed present
/// value fails at the level that records it (#3733 review) — a valid value at
/// the other level repairs ABSENCE, never malformedness.
fn synthetic_diff_at_level(
    owner: &str,
    object: &serde_json::Map<String, Value>,
) -> Result<Option<String>, String> {
    match object.get("synthetic_diff") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(path)) => {
            check_portable_path(owner, "synthetic_diff", path)?;
            Ok(Some(path.clone()))
        }
        Some(_) => Err(fail(
            owner,
            "synthetic_diff",
            "field must be a string diff path when present",
        )),
    }
}

/// Validates the accepted manifest: schema/kind/spec/tier, exactly eight
/// unique subjects, immutable repository identity, license and shape tags,
/// portable diff paths. Data-driven: any eight well-formed subjects pass.
/// Shared with the refresh route (#3566): refresh consumes the validated
/// subjects instead of re-parsing.
pub(crate) fn validate_accepted_manifest(
    value: &Value,
    sha256: String,
) -> Result<AcceptedManifest, String> {
    let top = as_object(value, "manifest", "manifest", "accepted manifest")?;
    reject_unknown_keys(top, &MANIFEST_KEYS, "manifest", "accepted manifest")?;
    for (field, expected) in [
        ("schema_version", MANIFEST_SCHEMA_VERSION),
        ("kind", MANIFEST_KIND),
        ("spec", KNOWN_SPEC),
        ("tier", KNOWN_TIER),
    ] {
        let actual = top.get(field).and_then(Value::as_str).ok_or_else(|| {
            fail(
                "manifest",
                field,
                "accepted manifest must declare this field",
            )
        })?;
        if actual != expected {
            return Err(fail(
                "manifest",
                field,
                format!("expected `{expected}`, got `{actual}`"),
            ));
        }
    }

    // Owned-but-unchecked top-level fields get their emitted-shape type
    // checks: `description` is a string, `limits` an array of strings, and a
    // present top-level `synthetic_diff` fallback is a portable path whether
    // or not any subject needs it (#3733 review).
    opt_string("manifest", top, "description")?;
    opt_string_array("manifest", top, "limits")?;
    let top_level_diff = synthetic_diff_at_level("manifest", top)?;

    let repos = value
        .get("repos")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            fail(
                "manifest",
                "repos",
                "accepted manifest must contain a repos array",
            )
        })?;
    if repos.len() != ACCEPTED_SUBJECT_COUNT {
        return Err(fail(
            "manifest",
            "repos",
            format!(
                "the accepted sweep denominator is exactly {ACCEPTED_SUBJECT_COUNT} selected subjects, got {}",
                repos.len()
            ),
        ));
    }

    let mut subjects = Vec::new();
    let mut seen = BTreeSet::new();
    let mut incomplete = Vec::new();
    for repo in repos {
        let entry = as_object(repo, "manifest", "repos", "manifest repo entry")?;
        reject_unknown_keys(
            entry,
            &MANIFEST_REPO_KEYS,
            "manifest",
            "manifest repo entry",
        )?;
        let id = entry
            .get("id")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| {
                fail(
                    "manifest",
                    "repos[].id",
                    "subject id must be a non-empty string",
                )
            })?
            .to_string();
        // Case-insensitive uniqueness (ASCII): subject ids become filesystem
        // path components, and on case-insensitive filesystems (Windows,
        // default macOS) `Alpha` and `alpha` would resolve to one directory —
        // two distinct ids colliding into one candidate tree. The charset
        // already bars `~` (short-name aliases) and path separators;
        // case-collision is the remaining alias, so it is rejected here.
        if !seen.insert(id.to_ascii_lowercase()) {
            return Err(fail(
                &id,
                "id",
                "duplicate subject id in the accepted manifest (ids must be unique case-insensitively: ids are path components, and a case-insensitive filesystem would collide `Alpha` with `alpha`)",
            ));
        }
        check_subject_id(&id)?;

        let url = entry
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| fail(&id, "url", "subject must declare an https repository url"))?;
        check_subject_url(&id, "url", url)?;

        let sha = entry
            .get("sha")
            .and_then(Value::as_str)
            .ok_or_else(|| fail(&id, "sha", "subject must pin an immutable source SHA"))?;
        check_git_sha(&id, "sha", sha)?;

        let license = entry
            .get("license")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| fail(&id, "license", "subject must declare its license class"))?
            .to_string();

        // `why` is owned; when recorded it must be a non-empty string (its
        // emitted shape), not a wrong-typed stand-in. Absent stays data-driven.
        opt_string(&id, entry, "why")?;

        let shape = entry
            .get("shape")
            .and_then(Value::as_str)
            .ok_or_else(|| fail(&id, "shape", "subject must declare a shape/layout tag"))?;
        known_value_or_fail(&id, "shape", shape, &KNOWN_SHAPES, "shape/layout tag")?;

        // Diff identity: per-repo path with the manifest-level fallback the
        // run path resolves. Each present value is checked at the level that
        // records it (#3733 review): a malformed present value fails even
        // when the other level supplies a valid fallback — only absence falls
        // through. Portable and secret-free; existence is a run-time concern,
        // not an offline structural one. The resolved path is retained on the
        // subject for the refresh route (#3566).
        let diff = synthetic_diff_at_level(&id, entry)?
            .or_else(|| top_level_diff.clone())
            .ok_or_else(|| {
                fail(
                    &id,
                    "synthetic_diff",
                    "subject has no synthetic_diff and the manifest has no top-level fallback",
                )
            })?;

        // Optional identities (#3733 review): absent is typed incomplete; a
        // present value must be well-formed, because an explicit null, an
        // empty string, or a malformed digest is a garbage identity, not an
        // absent one — that fails instead of silently completing the artifact.
        // Well-formed values are retained on the subject for receipt binding.
        let mut identities = [None, None, None, None];
        for (index, field) in ["tree_digest", "snapshot", "provenance", "retention_class"]
            .into_iter()
            .enumerate()
        {
            match entry.get(field) {
                None => incomplete.push(Diagnostic::new(
                    &id,
                    field,
                    "identity not recorded in the retained manifest; typed incomplete, not invented",
                )),
                Some(Value::Null) => {
                    return Err(fail(
                        &id,
                        field,
                        "identity is explicitly null; omit the field to record it absent — a present null is not an absent identity",
                    ));
                }
                Some(value) => {
                    let text = value
                        .as_str()
                        .ok_or_else(|| fail(&id, field, "identity must be a string when present"))?;
                    if text.trim().is_empty() {
                        return Err(fail(
                            &id,
                            field,
                            "identity must be non-empty when present",
                        ));
                    }
                    match field {
                        "tree_digest" => check_sha256_digest(&id, field, text)?,
                        "snapshot" => check_no_secrets(&id, field, text)?,
                        _ => {}
                    }
                    identities[index] = Some(text.to_string());
                }
            }
        }

        let [tree_digest, snapshot, provenance, retention_class] = identities;
        subjects.push(AcceptedSubject {
            id,
            url: url.to_string(),
            sha: sha.to_string(),
            license,
            shape: shape.to_string(),
            synthetic_diff: diff,
            tree_digest,
            snapshot,
            provenance,
            retention_class,
        });
    }

    Ok(AcceptedManifest {
        sha256,
        subjects,
        incomplete,
    })
}

// ---------------------------------------------------------------------------
// Retained run receipts
// ---------------------------------------------------------------------------

const SUMMARY_KEYS: [&str; 20] = [
    "repos_total",
    "repos_run",
    "repos_skipped",
    "repos_clone_failed",
    "crash_count",
    "crash_rate",
    "parse_failure_count",
    "parse_failure_rate",
    "timed_out_count",
    "runtime_ms_min",
    "runtime_ms_median",
    "runtime_ms_max",
    "runtime_ms_total",
    "gap_id_stable_count",
    "gap_id_unstable_count",
    "gap_id_stability_rate",
    "classification_counts",
    "alignment_counts",
    "gate_status",
    "gate_reason",
];

const ROW_KEYS_0_2: [&str; 11] = [
    "id",
    "sha",
    "shape",
    "outcome",
    "runtime_ms",
    "gap_ids",
    "gap_ids_stable",
    "unstable_gap_ids",
    "stderr_excerpt",
    "classification_counts",
    "alignment_counts",
];

const ROW_KEYS_0_3: [&str; 22] = [
    "id",
    "status",
    "repository",
    "tree_digest",
    "snapshot",
    "license",
    "retention_class",
    "provenance",
    "selected_root",
    "layout",
    "binary",
    "config",
    "input_digest",
    "materialization",
    "detection",
    "corpus_selection",
    "execution",
    "digests",
    "repeat",
    "runtime_ms",
    "classification_counts",
    "alignment_counts",
];

/// One validated row reduced to what denominator/aggregate derivation needs.
struct RowSummary {
    counts_as_run: bool,
    crashed: bool,
    parse_failed: bool,
    timed_out: bool,
    skipped: bool,
    clone_failed: bool,
    stability: Option<bool>,
    runtime_ms: Option<u64>,
    classification: Option<BTreeMap<String, u64>>,
    alignment: Option<BTreeMap<String, u64>>,
}

#[derive(Debug, Default)]
struct Derived {
    total: usize,
    run: usize,
    crashed: usize,
    parse_failed: usize,
    timed_out: usize,
    skipped: usize,
    clone_failed: usize,
    /// `None` when any run row lacks stability evidence.
    stable: Option<usize>,
    /// (min, median, max, total) over run rows; `None` when any run row lacks
    /// a runtime.
    runtime: Option<(u64, u64, u64, u64)>,
    classification: BTreeMap<String, u64>,
    alignment: BTreeMap<String, u64>,
}

impl Derived {
    fn crash_rate(&self) -> f64 {
        ratio(self.crashed, self.run)
    }

    fn parse_failure_rate(&self) -> f64 {
        ratio(self.parse_failed, self.run)
    }

    fn stability_rate(&self) -> Option<f64> {
        self.stable.map(|stable| {
            if self.run == 0 {
                1.0
            } else {
                stable as f64 / self.run as f64
            }
        })
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

/// One validated receipt: the structural facts the refresh route reports
/// after self-validating its own candidate (#3566).
pub(crate) struct ReceiptCheck {
    pub(crate) path: String,
    pub(crate) schema_version: String,
    pub(crate) denominator_selected: usize,
    pub(crate) denominator_run: usize,
    pub(crate) incomplete: Vec<Diagnostic>,
}

impl ReceiptCheck {
    pub(crate) fn verdict(&self) -> Verdict {
        if self.incomplete.is_empty() {
            Verdict::Valid
        } else {
            Verdict::Incomplete
        }
    }
}

/// Validates one retained run receipt against the accepted manifest.
/// Fails closed on the issue #3565 failure families; discloses missing
/// identities as `incomplete`. Never rewrites or upgrades the receipt.
/// Shared with the refresh route (#3566): refresh self-validates its own
/// candidate receipt through this exact validator before writing it, so a
/// produced candidate and `eval-sweep check --runs` agree by construction.
pub(crate) fn validate_run_receipt(
    value: &Value,
    manifest_sha256: &str,
    accepted: &AcceptedManifest,
    display: &str,
) -> Result<ReceiptCheck, String> {
    let top = as_object(value, display, "receipt", "run receipt")?;
    reject_unknown_envelope_keys(top)?;

    for (field, expected, what) in [
        ("kind", REPORT_KIND, "run receipt kind"),
        ("spec", KNOWN_SPEC, "spec"),
        ("tier", KNOWN_TIER, "tier"),
    ] {
        let actual = top.get(field).and_then(Value::as_str).ok_or_else(|| {
            fail(
                display,
                field,
                format!("run receipt must declare its {what}"),
            )
        })?;
        if actual != expected {
            return Err(fail(
                display,
                field,
                format!("expected `{expected}`, got `{actual}`"),
            ));
        }
    }

    let schema_version = top
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            fail(
                display,
                "schema_version",
                "run receipt must declare a schema_version",
            )
        })?
        .to_string();
    if schema_version != RECEIPT_SCHEMA_0_2 && schema_version != RECEIPT_SCHEMA_0_3 {
        return Err(fail(
            display,
            "schema_version",
            format!(
                "unsupported receipt schema `{schema_version}`; this loader owns {RECEIPT_SCHEMA_0_2} and {RECEIPT_SCHEMA_0_3}"
            ),
        ));
    }
    let is_current = schema_version == RECEIPT_SCHEMA_0_3;

    // Manifest binding: present-and-wrong is a stale digest (fail); absent is
    // typed incomplete for historical 0.2 receipts (they predate the binding).
    match opt_string(display, top, "manifest_digest")? {
        Some(digest) => {
            check_sha256_digest(display, "manifest_digest", &digest)?;
            if digest != manifest_sha256 {
                return Err(fail(
                    display,
                    "manifest_digest",
                    format!(
                        "stale digest: receipt is bound to manifest {digest} but the accepted manifest is {manifest_sha256}"
                    ),
                ));
            }
        }
        None => {
            if is_current {
                return Err(fail(
                    display,
                    "manifest_digest",
                    "schema-0.3 receipts are bound to the accepted manifest by digest",
                ));
            }
        }
    }
    let mut incomplete = Vec::new();
    if !is_current {
        incomplete.push(Diagnostic::new(
            display,
            "manifest_digest",
            "historical 0.2 receipt records no manifest digest binding; currentness against the accepted manifest is unverifiable",
        ));
    }

    // The receipt-level toolchain identity block: 0.3 receipts carry it, and
    // it is the reference side for the row `binary` copy checks (J2 below).
    let ripr_identity: Option<&serde_json::Map<String, Value>> = if is_current {
        validate_ripr_identity(top, display, &mut incomplete)?;
        top.get("ripr").and_then(Value::as_object)
    } else {
        None
    };

    let rows = value
        .get("repos")
        .and_then(Value::as_array)
        .ok_or_else(|| fail(display, "repos", "run receipt must contain a repos array"))?;
    let summary = value
        .get("summary")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            fail(
                display,
                "summary",
                "run receipt must contain a summary object",
            )
        })?;
    reject_unknown_keys(summary, &SUMMARY_KEYS, display, "summary")?;

    // Subject coverage: every accepted subject appears exactly once; no
    // unknown subjects; the denominator is unchanged.
    let mut rows_by_id: BTreeMap<String, &Value> = BTreeMap::new();
    for row in rows {
        let entry = as_object(row, display, "repos", "receipt row")?;
        let id = entry
            .get("id")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| {
                fail(
                    display,
                    "repos[].id",
                    "receipt row must name its subject id",
                )
            })?
            .to_string();
        if rows_by_id.insert(id.clone(), row).is_some() {
            return Err(fail(&id, "id", "duplicate subject row in the run receipt"));
        }
        if accepted.subject(&id).is_none() {
            return Err(fail(
                &id,
                "id",
                format!(
                    "receipt row names a subject outside the accepted denominator (accepted: {})",
                    accepted.ids().join(", ")
                ),
            ));
        }
    }
    if rows.len() != accepted.subjects.len() {
        return Err(fail(
            display,
            "repos",
            format!(
                "changed denominator: receipt carries {} row(s) but the accepted manifest selects {}",
                rows.len(),
                accepted.subjects.len()
            ),
        ));
    }
    for subject in &accepted.subjects {
        if !rows_by_id.contains_key(&subject.id) {
            return Err(fail(
                &subject.id,
                "id",
                "missing subject row: every accepted subject stays in the receipt denominator",
            ));
        }
    }

    // Per-row validation.
    let mut summaries = Vec::new();
    for subject in &accepted.subjects {
        let row = rows_by_id
            .get(&subject.id)
            .ok_or_else(|| fail(&subject.id, "id", "missing subject row"))?;
        let summary = validate_row(row, subject, is_current, ripr_identity, &mut incomplete)?;
        summaries.push(summary);
    }

    let derived = derive_denominator(&summaries, display)?;

    // Row/aggregate agreement + gate semantics (fail closed on hand-edited
    // aggregates and vacuous passes).
    validate_summary_agreement(display, summary, &derived, &mut incomplete)?;

    Ok(ReceiptCheck {
        path: display.to_string(),
        schema_version,
        denominator_selected: derived.total,
        denominator_run: derived.run,
        incomplete,
    })
}

/// Envelope key ownership: the shared keys plus the 0.3 currentness block.
/// A 0.2 envelope carrying 0.3 fields (or vice versa) is a mixed shape: fail.
fn reject_unknown_envelope_keys(top: &serde_json::Map<String, Value>) -> Result<(), String> {
    let version = top
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("?");
    for key in top.keys() {
        let allowed = matches!(
            key.as_str(),
            "schema_version" | "kind" | "spec" | "tier" | "summary" | "repos"
        ) || (version == RECEIPT_SCHEMA_0_3
            && matches!(key.as_str(), "manifest_digest" | "ripr"));
        if !allowed {
            return Err(fail(
                "receipt",
                key,
                format!("unknown envelope field `{key}` for receipt schema {version}"),
            ));
        }
    }
    if version == RECEIPT_SCHEMA_0_2 && top.contains_key("ripr") {
        return Err(fail(
            "receipt",
            "ripr",
            "schema-0.2 receipts must not carry the 0.3 currentness block",
        ));
    }
    if version == RECEIPT_SCHEMA_0_2 && top.contains_key("manifest_digest") {
        return Err(fail(
            "receipt",
            "manifest_digest",
            "schema-0.2 receipts must not carry the 0.3 manifest binding",
        ));
    }
    Ok(())
}

/// RIPR toolchain identity on a 0.3 receipt: the block is part of the 0.3
/// schema; present values are format-checked (malformed fails) and absent
/// fields are disclosed incomplete.
fn validate_ripr_identity(
    top: &serde_json::Map<String, Value>,
    display: &str,
    incomplete: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    let Some(Value::Object(ripr)) = top.get("ripr") else {
        return Err(fail(
            display,
            "ripr",
            "schema-0.3 receipts must carry the ripr toolchain identity block",
        ));
    };
    let allowed = [
        "source_sha",
        "tree_digest",
        "binary_digest",
        "version",
        "features",
        "build_profile",
    ];
    reject_unknown_keys(ripr, &allowed, display, "ripr identity")?;
    // A present-null identity is garbage, not absent (#3733 review): only a
    // key left out discloses incomplete below.
    reject_null_identity_fields(display, "ripr", ripr, &allowed)?;
    match opt_string(display, ripr, "source_sha")? {
        Some(sha) => check_git_sha(display, "ripr.source_sha", &sha)?,
        None => incomplete.push(Diagnostic::new(
            display,
            "ripr.source_sha",
            "analyzer source identity not recorded",
        )),
    }
    for field in ["tree_digest", "binary_digest"] {
        match opt_string(display, ripr, field)? {
            Some(digest) => check_sha256_digest(display, &format!("ripr.{field}"), &digest)?,
            None => incomplete.push(Diagnostic::new(
                display,
                &format!("ripr.{field}"),
                "digest not recorded",
            )),
        }
    }
    match opt_string(display, ripr, "build_profile")? {
        Some(profile) => known_value_or_fail(
            display,
            "ripr.build_profile",
            &profile,
            &BUILD_PROFILES,
            "build profile",
        )?,
        None => incomplete.push(Diagnostic::new(
            display,
            "ripr.build_profile",
            "build profile not recorded",
        )),
    }
    match opt_string(display, ripr, "version")? {
        Some(_) => {}
        None => incomplete.push(Diagnostic::new(
            display,
            "ripr.version",
            "analyzer version not recorded",
        )),
    }
    match opt_string_array(display, ripr, "features")? {
        Some(_) => {}
        None => incomplete.push(Diagnostic::new(
            display,
            "ripr.features",
            "feature set not recorded",
        )),
    }
    Ok(())
}

/// Validates one receipt row against its accepted subject. Absent identities
/// push `incomplete` diagnostics; present-but-wrong values fail.
fn validate_row(
    row: &Value,
    subject: &AcceptedSubject,
    is_current: bool,
    ripr_identity: Option<&serde_json::Map<String, Value>>,
    incomplete: &mut Vec<Diagnostic>,
) -> Result<RowSummary, String> {
    let id = &subject.id;
    let entry = as_object(row, id, "row", "receipt row")?;
    let allowed: &[&str] = if is_current {
        &ROW_KEYS_0_3
    } else {
        &ROW_KEYS_0_2
    };
    reject_unknown_keys(entry, allowed, id, "receipt row")?;

    // Run status: exactly one of the 0.2 outcome or the 0.3 status.
    let outcome = opt_string(id, entry, "outcome")?;
    let status = opt_string(id, entry, "status")?;
    if outcome.is_some() && status.is_some() {
        return Err(fail(
            id,
            "status",
            "row mixes the 0.2 `outcome` and the 0.3 `status` vocabularies",
        ));
    }
    let (status_label, counts_as_run) = if let Some(outcome) = outcome.as_deref() {
        known_value_or_fail(id, "outcome", outcome, &HISTORICAL_OUTCOMES, "run outcome")?;
        let counts = !matches!(outcome, "clone_failed" | "skipped_missing_checkout");
        (outcome.to_string(), counts)
    } else if let Some(status) = status.as_deref() {
        known_value_or_fail(id, "status", status, &STATUS_VOCABULARY, "run status")?;
        (status.to_string(), RUN_STATUSES.contains(&status))
    } else {
        return Err(fail(
            id,
            "status",
            "receipt row carries no run status (0.2 `outcome` or 0.3 `status` required)",
        ));
    };

    // Immutable repository/source identity: the row must not contradict the
    // accepted pin. Present-and-different fails; absent is incomplete.
    if is_current {
        validate_row_repository(entry, id, subject, incomplete)?;
        validate_row_currentness(entry, id, &status_label, subject, incomplete)?;
        // Duplicate copies of one identity inside the receipt must agree;
        // this runs after the well-formedness checks so a malformed value
        // fails with its own digest/shape diagnostic first.
        validate_row_identity_copies(entry, id, ripr_identity)?;
    } else {
        // Owned-but-unchecked 0.2 fields get their emitted-shape type checks
        // (`gap_ids` is an array of strings; `stderr_excerpt` a string that
        // may be empty). A wrong-typed value fails naming the field.
        opt_string_array(id, entry, "gap_ids")?;
        opt_string_allow_empty(id, entry, "stderr_excerpt")?;
        match opt_string(id, entry, "sha")? {
            Some(sha) => {
                check_git_sha(id, "sha", &sha)?;
                if sha != subject.sha {
                    return Err(fail(
                        id,
                        "sha",
                        format!(
                            "receipt source SHA `{sha}` does not match the accepted manifest pin `{}`",
                            subject.sha
                        ),
                    ));
                }
            }
            None => incomplete.push(Diagnostic::new(
                id,
                "sha",
                "source identity not recorded on the row",
            )),
        }
        match opt_string(id, entry, "shape")? {
            Some(shape) => {
                known_value_or_fail(id, "shape", &shape, &KNOWN_SHAPES, "shape/layout tag")?;
                if shape != subject.shape {
                    return Err(fail(
                        id,
                        "shape",
                        format!(
                            "receipt shape tag `{shape}` does not match the accepted manifest tag `{}`",
                            subject.shape
                        ),
                    ));
                }
            }
            None => incomplete.push(Diagnostic::new(
                id,
                "shape",
                "shape/layout tag not recorded on the row",
            )),
        }
        incomplete.push(Diagnostic::new(
            id,
            "currentness identities",
            "historical 0.2 row records no materialization/detection/corpus-selection/execution states, no binary/config/input identity, no evidence digests, and no repeat-run comparison identity; typed incomplete, not invented",
        ));
    }

    // Stability and contradiction: unstable gap-ID lists cannot coexist with a
    // stable claim, an unstable claim cannot carry an empty list, and a false
    // stability claim cannot omit its list (the sweep derives both fields from
    // the same comparison, so the emitted shape never produces any of those
    // pairings). 0.2 rows carry row-level evidence; 0.3
    // rows carry it inside the validated `repeat` block — the row-level
    // stability fields are denied there, so this is the only evidence source.
    let (stability, unstable_ids, stability_field, unstable_list_field) = if is_current {
        match entry.get("repeat") {
            Some(Value::Object(repeat)) => (
                opt_bool(id, repeat, "gap_ids_stable")?,
                opt_string_array(id, repeat, "unstable_gap_ids")?,
                "repeat.gap_ids_stable",
                "repeat.unstable_gap_ids",
            ),
            _ => (
                None,
                None,
                "repeat.gap_ids_stable",
                "repeat.unstable_gap_ids",
            ),
        }
    } else {
        (
            opt_bool(id, entry, "gap_ids_stable")?,
            opt_string_array(id, entry, "unstable_gap_ids")?,
            "gap_ids_stable",
            "unstable_gap_ids",
        )
    };
    match (stability, unstable_ids.as_ref()) {
        (Some(true), Some(unstable)) if !unstable.is_empty() => {
            return Err(fail(
                id,
                stability_field,
                "contradictory status: row claims stable gap IDs while listing unstable ones",
            ));
        }
        (Some(false), Some(unstable)) if unstable.is_empty() => {
            return Err(fail(
                id,
                stability_field,
                "contradictory status: row claims unstable gap IDs but lists none",
            ));
        }
        // A false stability claim is a comparison result: the same re-run that
        // produced it produces the unstable gap-ID list, so an omitted list is
        // a contradiction, not a quiet pass.
        (Some(false), None) => {
            return Err(fail(
                id,
                unstable_list_field,
                "contradictory status: a false stability claim requires the unstable gap-ID list; the comparison evidence is omitted",
            ));
        }
        _ => {}
    }

    // absent-vs-unknown: a recorded alignment distribution must keep the two
    // distinct (the emitted `unknown` enum value is not the unrecorded case).
    let classification = opt_distribution(id, entry, "classification_counts")?;
    if let Some(counts) = &classification {
        for name in counts.keys() {
            if !CLASSIFICATION_VOCABULARY.contains(&name.as_str()) {
                return Err(fail(
                    id,
                    "classification_counts",
                    format!(
                        "unknown classification `{name}`; known vocabulary: {}",
                        CLASSIFICATION_VOCABULARY.join(", ")
                    ),
                ));
            }
        }
    }
    let alignment = opt_distribution(id, entry, "alignment_counts")?;
    if let Some(counts) = &alignment {
        for name in counts.keys() {
            if !ALIGNMENT_VOCABULARY.contains(&name.as_str()) {
                return Err(fail(
                    id,
                    "alignment_counts",
                    format!("unknown oracle alignment `{name}`"),
                ));
            }
        }
        if !counts.contains_key("absent") || !counts.contains_key("unknown") {
            return Err(fail(
                id,
                "alignment_counts",
                "distribution must keep `absent` (field not emitted) distinct from `unknown` (emitted value); both keys are required",
            ));
        }
    }

    // A row that never ran must not carry analysis counts (terminal rows are
    // all-zero in the emitted shape).
    if !counts_as_run {
        for (name, counts) in [
            ("classification_counts", &classification),
            ("alignment_counts", &alignment),
        ] {
            if let Some(counts) = counts
                && counts.values().any(|value| *value != 0)
            {
                return Err(fail(
                    id,
                    name,
                    "contradictory status: a row that did not run carries non-zero analysis counts",
                ));
            }
        }
    }

    let runtime_ms = opt_u64(id, entry, "runtime_ms")?;

    // Aggregate source evidence is required on analyzed rows: the sweep
    // records these fields on every row it writes (terminal rows included),
    // and a missing field would silently disable the corresponding summary
    // comparison — letting fabricated aggregates validate. A 0.3 row's
    // stability evidence lives in the optional `repeat` block; its absence is
    // typed incomplete and enforced at the summary layer instead.
    if counts_as_run {
        if runtime_ms.is_none() {
            return Err(fail(
                id,
                "runtime_ms",
                "analyzed row omits its runtime; the owned row shape records runtime_ms on every row, and a missing value would silently disable the runtime aggregate check",
            ));
        }
        if classification.is_none() {
            return Err(fail(
                id,
                "classification_counts",
                "analyzed row omits its classification distribution; the owned row shape records classification_counts on every row, and a missing value would silently disable the distribution check",
            ));
        }
        if alignment.is_none() {
            return Err(fail(
                id,
                "alignment_counts",
                "analyzed row omits its alignment distribution; the owned row shape records alignment_counts on every row, and a missing value would silently disable the distribution check",
            ));
        }
        if !is_current && stability.is_none() {
            return Err(fail(
                id,
                "gap_ids_stable",
                "analyzed row omits its gap-ID stability evidence; the owned 0.2 row shape records gap_ids_stable on every row, and a missing value would silently disable the stability and gate checks",
            ));
        }
    }

    Ok(RowSummary {
        counts_as_run,
        crashed: status_label == "crashed" || status_label == "crash",
        parse_failed: status_label == "parse-failed" || status_label == "parse_failure",
        timed_out: status_label == "timed-out" || status_label == "timed_out",
        skipped: status_label == "skipped_missing_checkout",
        clone_failed: status_label == "tempfail" || status_label == "clone_failed",
        stability,
        runtime_ms,
        classification,
        alignment,
    })
}

/// 0.3 repository identity block: a present block must be an object — a
/// wrong-typed block is malformed, not absent, so it fails naming the field
/// (#3733 review); only ABSENCE discloses incomplete. A present block must
/// restate the accepted pin when it carries url/sha; a partial block is typed
/// incomplete.
fn validate_row_repository(
    entry: &serde_json::Map<String, Value>,
    id: &str,
    subject: &AcceptedSubject,
    incomplete: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    let repository = match entry.get("repository") {
        None | Some(Value::Null) => {
            incomplete.push(Diagnostic::new(
                id,
                "repository",
                "repository identity block not recorded",
            ));
            return Ok(());
        }
        Some(Value::Object(repository)) => repository,
        Some(_) => {
            return Err(fail(
                id,
                "repository",
                "repository identity must be an object when present",
            ));
        }
    };
    let allowed: [&str; 4] = ["url", "sha", "tree_digest", "snapshot"];
    reject_unknown_keys(repository, &allowed, id, "repository identity")?;
    // A present-null identity inside the block is garbage, not absent
    // (#3733 review).
    reject_null_identity_fields(id, "repository", repository, &allowed)?;
    let url = opt_string(id, repository, "url")?;
    let sha = opt_string(id, repository, "sha")?;
    if url.is_none() || sha.is_none() {
        incomplete.push(Diagnostic::new(
            id,
            "repository",
            "repository identity block is partial (url/sha not both recorded)",
        ));
    }
    if let Some(url) = &url {
        check_subject_url(id, "repository.url", url)?;
        if *url != subject.url {
            return Err(fail(
                id,
                "repository.url",
                format!(
                    "receipt repository url `{url}` does not match the accepted manifest pin `{}`",
                    subject.url
                ),
            ));
        }
    }
    if let Some(sha) = &sha {
        check_git_sha(id, "repository.sha", sha)?;
        if *sha != subject.sha {
            return Err(fail(
                id,
                "repository.sha",
                format!(
                    "receipt repository sha `{sha}` does not match the accepted manifest pin `{}`",
                    subject.sha
                ),
            ));
        }
    }
    if let Some(tree) = opt_string(id, repository, "tree_digest")? {
        check_sha256_digest(id, "repository.tree_digest", &tree)?;
    }
    if let Some(snapshot) = opt_string(id, repository, "snapshot")? {
        check_no_secrets(id, "repository.snapshot", &snapshot)?;
    }
    Ok(())
}

/// A recorded value with the loader's null-is-absent rule: an explicit null
/// is not a comparable copy of an identity.
fn identity_value<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Option<&'a Value> {
    match object.get(field) {
        Some(Value::Null) | None => None,
        Some(value) => Some(value),
    }
}

/// An explicitly null identity field is present-but-garbage, not absent (the
/// receipt-side twin of the manifest's null-identity rule): it fails naming
/// the field, while a key left out stays a typed-incomplete disclosure
/// (#3733 review). `prefix` is the owning block (`""` for top-level row
/// fields, `ripr`/`repository`/`binary` inside their blocks).
fn reject_null_identity_fields(
    subject: &str,
    prefix: &str,
    object: &serde_json::Map<String, Value>,
    fields: &[&str],
) -> Result<(), String> {
    for field in fields {
        if matches!(object.get(*field), Some(Value::Null)) {
            let named = if prefix.is_empty() {
                (*field).to_string()
            } else {
                format!("{prefix}.{field}")
            };
            return Err(fail(
                subject,
                &named,
                "identity is explicitly null; omit the field to record it absent — a present null is not an absent identity",
            ));
        }
    }
    Ok(())
}

/// Two recorded copies of the same identity must agree: when the same
/// identity appears at two locations in one receipt, the copies describe one
/// entity, so a disagreement is a hand-edit. A mismatch fails naming both
/// locations; a one-sided record is not comparable and keeps its own
/// absent-is-incomplete rule.
fn require_matching_identity_copies(
    id: &str,
    a_location: &str,
    a: Option<&Value>,
    b_location: &str,
    b: Option<&Value>,
) -> Result<(), String> {
    if let (Some(a_value), Some(b_value)) = (a, b)
        && a_value != b_value
    {
        return Err(fail(
            id,
            a_location,
            format!(
                "contradictory identity: `{a_location}` does not match `{b_location}`; both locations record the same identity, and disagreeing copies are a hand-edit"
            ),
        ));
    }
    Ok(())
}

/// Duplicate identity copies inside one 0.3 receipt must agree (#3733
/// review): the row-level `tree_digest`/`snapshot` vs the `repository` block,
/// and a row's `binary` identity vs the receipt-level `ripr` block.
fn validate_row_identity_copies(
    entry: &serde_json::Map<String, Value>,
    id: &str,
    ripr: Option<&serde_json::Map<String, Value>>,
) -> Result<(), String> {
    if let Some(Value::Object(repository)) = entry.get("repository") {
        for field in ["tree_digest", "snapshot"] {
            require_matching_identity_copies(
                id,
                field,
                identity_value(entry, field),
                &format!("repository.{field}"),
                identity_value(repository, field),
            )?;
        }
    }
    if let (Some(Value::Object(binary)), Some(ripr)) = (entry.get("binary"), ripr) {
        for (row_field, row_location, receipt_field, receipt_location) in [
            (
                "digest",
                "binary.digest",
                "binary_digest",
                "ripr.binary_digest",
            ),
            ("version", "binary.version", "version", "ripr.version"),
            ("features", "binary.features", "features", "ripr.features"),
            (
                "build_profile",
                "binary.build_profile",
                "build_profile",
                "ripr.build_profile",
            ),
        ] {
            require_matching_identity_copies(
                id,
                row_location,
                identity_value(binary, row_field),
                receipt_location,
                identity_value(ripr, receipt_field),
            )?;
        }
    }
    Ok(())
}

/// 0.3 row currentness: state vocabularies, binary/config/input identity,
/// evidence digests, repeat-run identity, and the receipt-vs-manifest identity
/// binding. Unknown states fail; absent states and identities are typed
/// incomplete; contradictions fail; a well-formed receipt identity that
/// contradicts the manifest pin fails naming both sides, while a receipt value
/// with no manifest side to bind discloses the manifest gap instead.
fn validate_row_currentness(
    entry: &serde_json::Map<String, Value>,
    id: &str,
    status: &str,
    subject: &AcceptedSubject,
    incomplete: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    // State fields: present values must use the known vocabulary.
    for (field, vocabulary, what) in [
        (
            "materialization",
            &MATERIALIZATION_STATES[..],
            "materialization state",
        ),
        ("detection", &DETECTION_STATES[..], "detection state"),
        ("execution", &EXECUTION_STATES[..], "execution state"),
    ] {
        match opt_string(id, entry, field)? {
            Some(state) => known_value_or_fail(id, field, &state, vocabulary, what)?,
            None => incomplete.push(Diagnostic::new(id, field, "state not recorded")),
        }
    }
    if let Some(state) = opt_string(id, entry, "execution")? {
        // Contradiction matrix: the status and the execution state must agree
        // about what happened.
        let contradiction = match status {
            "complete" | "parse-failed" => state != "executed",
            "timed-out" => state != "timed-out",
            "crashed" => state != "failed",
            // A `partial` row is a run-status row: it counts toward
            // `repos_run`, so `not-executed` contradicts it. The other ran /
            // failed states stay honest about how the partial attempt went.
            "partial" => state == "not-executed",
            _ => false,
        };
        if contradiction {
            return Err(fail(
                id,
                "execution",
                format!(
                    "contradictory status: status `{status}` cannot coexist with execution state `{state}`"
                ),
            ));
        }
    }
    if let Some(materialization) = opt_string(id, entry, "materialization")?
        && matches!(status, "complete" | "partial")
        && matches!(materialization.as_str(), "absent" | "failed")
    {
        return Err(fail(
            id,
            "materialization",
            format!(
                "contradictory status: status `{status}` cannot coexist with materialization `{materialization}`"
            ),
        ));
    }
    if let Some(detection) = opt_string(id, entry, "detection")?
        && matches!(status, "complete" | "partial")
        && matches!(detection.as_str(), "absent" | "failed")
    {
        return Err(fail(
            id,
            "detection",
            format!(
                "contradictory status: status `{status}` cannot coexist with detection state `{detection}`"
            ),
        ));
    }

    // Corpus selection: state vocabulary plus optional selected counts.
    match entry.get("corpus_selection") {
        None | Some(Value::Null) => incomplete.push(Diagnostic::new(
            id,
            "corpus_selection",
            "corpus-selection state not recorded",
        )),
        Some(Value::Object(selection)) => {
            let allowed: [&str; 5] = [
                "state",
                "source_files",
                "test_files",
                "generated_files",
                "vendor_files",
            ];
            reject_unknown_keys(selection, &allowed, id, "corpus_selection")?;
            match opt_string(id, selection, "state")? {
                Some(state) => known_value_or_fail(
                    id,
                    "corpus_selection.state",
                    &state,
                    &CORPUS_SELECTION_STATES,
                    "corpus-selection state",
                )?,
                None => {
                    return Err(fail(
                        id,
                        "corpus_selection.state",
                        "corpus-selection block must declare its state",
                    ));
                }
            }
            for field in [
                "source_files",
                "test_files",
                "generated_files",
                "vendor_files",
            ] {
                opt_u64(id, selection, field)?;
            }
        }
        Some(_) => {
            return Err(fail(
                id,
                "corpus_selection",
                "corpus-selection must be an object when present",
            ));
        }
    }

    // Binary identity (digest/version/features/profile).
    match entry.get("binary") {
        None | Some(Value::Null) => {
            incomplete.push(Diagnostic::new(
                id,
                "binary",
                "binary identity not recorded",
            ));
        }
        Some(Value::Object(binary)) => {
            let allowed: [&str; 4] = ["digest", "version", "features", "build_profile"];
            reject_unknown_keys(binary, &allowed, id, "binary identity")?;
            // A present-null identity inside the block is garbage, not
            // absent (#3733 review).
            reject_null_identity_fields(id, "binary", binary, &allowed)?;
            match opt_string(id, binary, "digest")? {
                Some(digest) => check_sha256_digest(id, "binary.digest", &digest)?,
                None => incomplete.push(Diagnostic::new(
                    id,
                    "binary.digest",
                    "binary digest not recorded",
                )),
            }
            if opt_string(id, binary, "version")?.is_none() {
                incomplete.push(Diagnostic::new(
                    id,
                    "binary.version",
                    "binary version not recorded",
                ));
            }
            match opt_string(id, binary, "build_profile")? {
                Some(profile) => known_value_or_fail(
                    id,
                    "binary.build_profile",
                    &profile,
                    &BUILD_PROFILES,
                    "build profile",
                )?,
                None => incomplete.push(Diagnostic::new(
                    id,
                    "binary.build_profile",
                    "build profile not recorded",
                )),
            }
            match opt_string_array(id, binary, "features")? {
                Some(_) => {}
                None => incomplete.push(Diagnostic::new(
                    id,
                    "binary.features",
                    "feature set not recorded",
                )),
            }
        }
        Some(_) => {
            return Err(fail(
                id,
                "binary",
                "binary identity must be an object when present",
            ));
        }
    }

    // Config/profile/input identity.
    match entry.get("config") {
        None | Some(Value::Null) => {
            incomplete.push(Diagnostic::new(
                id,
                "config",
                "config identity not recorded",
            ));
        }
        Some(Value::Object(config)) => {
            let allowed: [&str; 2] = ["profile", "input"];
            reject_unknown_keys(config, &allowed, id, "config identity")?;
            if opt_string(id, config, "profile")?.is_none() {
                incomplete.push(Diagnostic::new(
                    id,
                    "config.profile",
                    "config profile not recorded",
                ));
            }
            match opt_string(id, config, "input")? {
                Some(input) => check_portable_path(id, "config.input", &input)?,
                None => incomplete.push(Diagnostic::new(
                    id,
                    "config.input",
                    "input identity not recorded",
                )),
            }
        }
        Some(_) => {
            return Err(fail(
                id,
                "config",
                "config identity must be an object when present",
            ));
        }
    }
    match opt_string(id, entry, "input_digest")? {
        Some(digest) => check_sha256_digest(id, "input_digest", &digest)?,
        None => incomplete.push(Diagnostic::new(
            id,
            "input_digest",
            "input digest not recorded",
        )),
    }

    // Selected root and layout tags.
    match opt_string(id, entry, "selected_root")? {
        Some(root) => check_portable_path(id, "selected_root", &root)?,
        None => incomplete.push(Diagnostic::new(
            id,
            "selected_root",
            "selected root not recorded",
        )),
    }
    opt_string_array(id, entry, "layout")?;

    // Tree/snapshot identity, license/provenance/retention restatement, and
    // the receipt-vs-manifest identity binding (#3733 review): when both
    // sides record a comparable identity and both are well-formed, they must
    // MATCH — a mismatch fails naming both sides. When only the receipt
    // records it, the value cannot be bound, so the manifest side discloses
    // incomplete instead of fabricating a binding. A present-null row
    // identity is garbage, not absent (#3733 review): it fails naming the
    // field, exactly like the manifest-side rule; only a key left out
    // discloses incomplete below.
    reject_null_identity_fields(
        id,
        "",
        entry,
        &[
            "tree_digest",
            "snapshot",
            "license",
            "retention_class",
            "provenance",
        ],
    )?;
    for field in [
        "tree_digest",
        "snapshot",
        "license",
        "retention_class",
        "provenance",
    ] {
        let recorded = opt_string(id, entry, field)?;
        match recorded.as_deref() {
            Some(tree) if field == "tree_digest" => check_sha256_digest(id, field, tree)?,
            Some(snapshot) if field == "snapshot" => check_no_secrets(id, field, snapshot)?,
            Some(_) => {}
            None => incomplete.push(Diagnostic::new(
                id,
                field,
                "identity not recorded on the row; typed incomplete, not invented",
            )),
        }
        let manifest_side = match field {
            "tree_digest" => subject.tree_digest.as_deref(),
            "snapshot" => subject.snapshot.as_deref(),
            "license" => Some(subject.license.as_str()),
            "provenance" => subject.provenance.as_deref(),
            "retention_class" => subject.retention_class.as_deref(),
            _ => None,
        };
        match (recorded.as_deref(), manifest_side) {
            (Some(receipt_value), Some(manifest_value)) if receipt_value != manifest_value => {
                return Err(fail(
                    id,
                    field,
                    format!(
                        "receipt {field} `{receipt_value}` does not match the accepted manifest {field} `{manifest_value}`"
                    ),
                ));
            }
            (Some(receipt_value), None) => incomplete.push(Diagnostic::new(
                id,
                &format!("manifest.{field}"),
                format!(
                    "receipt records {field} `{receipt_value}` but the accepted manifest records none; the binding is unverifiable (typed incomplete, not invented)"
                ),
            )),
            _ => {}
        }
    }

    // Evidence digests: raw/output/evidence.
    match entry.get("digests") {
        None | Some(Value::Null) => {
            incomplete.push(Diagnostic::new(
                id,
                "digests",
                "evidence digests not recorded",
            ));
        }
        Some(Value::Object(digests)) => {
            let allowed: [&str; 3] = ["raw", "output", "evidence"];
            reject_unknown_keys(digests, &allowed, id, "evidence digests")?;
            for field in allowed {
                match opt_string(id, digests, field)? {
                    Some(digest) => check_sha256_digest(id, &format!("digests.{field}"), &digest)?,
                    None => incomplete.push(Diagnostic::new(
                        id,
                        &format!("digests.{field}"),
                        "digest not recorded",
                    )),
                }
            }
        }
        Some(_) => {
            return Err(fail(
                id,
                "digests",
                "evidence digests must be an object when present",
            ));
        }
    }

    // Repeat-run comparison identity.
    match entry.get("repeat") {
        None | Some(Value::Null) => incomplete.push(Diagnostic::new(
            id,
            "repeat",
            "repeat-run comparison identity not recorded",
        )),
        Some(Value::Object(repeat)) => {
            let allowed: [&str; 4] = [
                "comparable_with",
                "gap_ids_stable",
                "unstable_gap_ids",
                "repeat_stderr",
            ];
            reject_unknown_keys(repeat, &allowed, id, "repeat-run identity")?;
            if opt_string(id, repeat, "comparable_with")?.is_none() {
                return Err(fail(
                    id,
                    "repeat.comparable_with",
                    "repeat-run identity must name the run it was compared against",
                ));
            }
            opt_bool(id, repeat, "gap_ids_stable")?;
            opt_string_array(id, repeat, "unstable_gap_ids")?;
            // The repeat pass's raw-stderr digest (SPEC-0086 retention): when
            // recorded it must be a real sha256 hex digest over the retained
            // second-pass stderr bytes under `<out>/raw/`. Optional so
            // historical 0.3 rows stay historical.
            if let Some(digest) = opt_string(id, repeat, "repeat_stderr")? {
                check_sha256_digest(id, "repeat.repeat_stderr", &digest)?;
            }
        }
        Some(_) => {
            return Err(fail(
                id,
                "repeat",
                "repeat-run identity must be an object when present",
            ));
        }
    }

    Ok(())
}

/// Derives the denominator and aggregates from validated rows only — the
/// arithmetic every hand-entered summary number must agree with. Checked
/// throughout: an overflowing aggregate is a structured failure naming the
/// summary field it would feed, never a panic.
fn derive_denominator(rows: &[RowSummary], display: &str) -> Result<Derived, String> {
    let mut derived = Derived {
        total: rows.len(),
        ..Derived::default()
    };
    let mut runtimes: Vec<u64> = Vec::new();
    let mut stability_complete = true;
    for row in rows {
        if row.crashed {
            derived.crashed += 1;
        }
        if row.parse_failed {
            derived.parse_failed += 1;
        }
        if row.timed_out {
            derived.timed_out += 1;
        }
        if row.skipped {
            derived.skipped += 1;
        }
        if row.clone_failed {
            derived.clone_failed += 1;
        }
        if !row.counts_as_run {
            continue;
        }
        derived.run += 1;
        match row.stability {
            Some(true) => {
                let stable = derived.stable.get_or_insert(0);
                *stable += 1;
            }
            Some(false) => {
                derived.stable.get_or_insert(0);
            }
            None => stability_complete = false,
        }
        if let Some(runtime) = row.runtime_ms {
            runtimes.push(runtime);
        }
        // Analyzed rows always carry both distributions (enforced at row
        // level); terminal rows never enter the aggregates.
        if let Some(class) = &row.classification {
            merge_distribution(
                &mut derived.classification,
                class,
                "classification_counts",
                display,
            )?;
        }
        if let Some(align) = &row.alignment {
            merge_distribution(&mut derived.alignment, align, "alignment_counts", display)?;
        }
    }
    if !stability_complete {
        derived.stable = None;
    }
    if !runtimes.is_empty() && runtimes.len() == derived.run {
        runtimes.sort_unstable();
        let mut total: u64 = 0;
        for runtime in &runtimes {
            total = total.checked_add(*runtime).ok_or_else(|| {
                fail(
                    display,
                    "summary.runtime_ms_total",
                    format!(
                        "aggregate overflow: runtime total exceeds u64 when summed across {} run row(s)",
                        runtimes.len()
                    ),
                )
            })?;
        }
        derived.runtime = Some((
            runtimes[0],
            runtimes[runtimes.len() / 2],
            runtimes[runtimes.len() - 1],
            total,
        ));
    }
    Ok(derived)
}

fn merge_distribution(
    target: &mut BTreeMap<String, u64>,
    source: &BTreeMap<String, u64>,
    field: &str,
    display: &str,
) -> Result<(), String> {
    for (name, count) in source {
        let bucket = target.entry(name.clone()).or_insert(0);
        *bucket = bucket.checked_add(*count).ok_or_else(|| {
            fail(
                display,
                &format!("summary.{field}.{name}"),
                format!(
                    "aggregate overflow: bucket `{name}` exceeds u64 when summed across run rows"
                ),
            )
        })?;
    }
    Ok(())
}

/// Fails closed when hand-entered summary numbers disagree with the derived
/// rows; discloses absent core fields as incomplete. The summary itself is
/// owned in full (#3733 review): analyzed receipts must carry every aggregate
/// the emitter writes, and zero-run receipts must not carry a nonzero
/// analysis-bearing aggregate.
fn validate_summary_agreement(
    display: &str,
    summary: &serde_json::Map<String, Value>,
    derived: &Derived,
    incomplete: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    // Summary ownership (#3733 review). With analyzed rows the receipt must
    // carry the emitted summary in full: the sweep records every aggregate on
    // every receipt, and a deleted field would silently disable its
    // row-agreement check. Stability aggregates are required exactly when the
    // rows fully evidence stability; the under-evidenced path discloses
    // instead of failing on an omission the emitter could not have written.
    const STABILITY_AGGREGATES: [&str; 3] = [
        "gap_id_stable_count",
        "gap_id_unstable_count",
        "gap_id_stability_rate",
    ];
    if derived.run > 0 {
        let stability_required = derived.stable.is_some();
        for field in SUMMARY_KEYS {
            if STABILITY_AGGREGATES.contains(&field) && !stability_required {
                continue;
            }
            if !summary_records_value(summary, field) {
                return Err(fail(
                    display,
                    &format!("summary.{field}"),
                    "required summary aggregate is missing: the sweep records the full summary on every receipt, and an omitted field would silently disable its row-agreement check",
                ));
            }
        }
    } else {
        // Zero-run law: `repos_run == 0` is `not_run`, never a vacuous pass —
        // extended to the summary. Every analysis-bearing aggregate must be
        // zero or absent; a recorded nonzero value claims analysis that never
        // happened.
        for field in [
            "runtime_ms_min",
            "runtime_ms_median",
            "runtime_ms_max",
            "runtime_ms_total",
            "gap_id_stable_count",
            "gap_id_unstable_count",
        ] {
            if let Some(value) = opt_u64(display, summary, field)?
                && value != 0
            {
                return Err(fail(
                    display,
                    &format!("summary.{field}"),
                    format!(
                        "repos_run == 0: aggregate must be zero or absent, got {value} — nothing ran, so a nonzero analysis-bearing aggregate is fabricated"
                    ),
                ));
            }
        }
        if let Some(value) = opt_number(display, summary, "gap_id_stability_rate")? {
            // The live emitter records `gap_id_stability_rate: 1.0` on every
            // zero-run report (eval_sweep.rs `compute_metrics` empty-set
            // guard), so a real untouched receipt must validate. Exactly that
            // value is accepted as a named incomplete disclosure — a vacuous
            // zero-run stability rate, not a measured claim — and the
            // disclosure keeps the receipt verdict `incomplete`, never a
            // pass. Any other nonzero rate stays a fabricated claim.
            if (value - 1.0).abs() <= 1e-9 {
                incomplete.push(Diagnostic::new(
                    display,
                    "summary.gap_id_stability_rate",
                    "vacuous zero-run stability rate: the emitter records 1.0 when repos_run == 0; no stability was measured, typed incomplete, not a pass",
                ));
            } else if value != 0.0 {
                return Err(fail(
                    display,
                    "summary.gap_id_stability_rate",
                    format!(
                        "repos_run == 0: stability rate must be zero, the emitter's vacuous 1.0, or absent, got {value} — nothing ran, so any other nonzero stability claim is fabricated"
                    ),
                ));
            }
        }
        for (field, vocabulary) in [
            ("classification_counts", &CLASSIFICATION_VOCABULARY[..]),
            ("alignment_counts", &ALIGNMENT_VOCABULARY[..]),
        ] {
            if let Some(counts) = opt_distribution(display, summary, field)? {
                for (name, count) in &counts {
                    if *count != 0 {
                        return Err(fail(
                            display,
                            &format!("summary.{field}.{name}"),
                            format!(
                                "repos_run == 0: bucket `{name}` claims {count} but nothing ran — analysis-bearing aggregates must be zero or absent at zero runs"
                            ),
                        ));
                    }
                }
                // A recorded distribution must be the emitter's zero-filled
                // shape (eval_sweep.rs `to_json` writes every bucket, at zero
                // runs included): a missing key — `absent`/`unknown` included
                // — is a dropped field, not a zero, and would erase the
                // field-not-emitted distinction (#3733 review).
                for name in vocabulary {
                    if !counts.contains_key(*name) {
                        return Err(fail(
                            display,
                            &format!("summary.{field}.{name}"),
                            format!(
                                "repos_run == 0: recorded distribution omits required bucket `{name}`; the emitter zero-fills every bucket, so a recorded distribution must carry the full emitted key set"
                            ),
                        ));
                    }
                }
            }
        }
    }

    // Denominator agreement.
    match opt_u64(display, summary, "repos_total")? {
        Some(value) if value as usize != derived.total => {
            return Err(fail(
                display,
                "summary.repos_total",
                format!(
                    "hand-edited aggregate: summary claims {value} selected row(s) but the receipt carries {}",
                    derived.total
                ),
            ));
        }
        _ => {}
    }
    match opt_u64(display, summary, "repos_run")? {
        Some(value) if value as usize != derived.run => {
            return Err(fail(
                display,
                "summary.repos_run",
                format!(
                    "hand-edited aggregate: summary claims {value} run row(s) but the rows derive {}",
                    derived.run
                ),
            ));
        }
        _ => {}
    }

    // Outcome counts.
    let count_fields: [(&str, usize); 5] = [
        ("crash_count", derived.crashed),
        ("parse_failure_count", derived.parse_failed),
        ("timed_out_count", derived.timed_out),
        ("repos_skipped", derived.skipped),
        ("repos_clone_failed", derived.clone_failed),
    ];
    for (field, expected) in count_fields {
        match opt_u64(display, summary, field)? {
            Some(value) if value as usize != expected => {
                return Err(fail(
                    display,
                    &format!("summary.{field}"),
                    format!(
                        "hand-edited aggregate: summary claims {value} but the rows derive {expected}"
                    ),
                ));
            }
            _ => {}
        }
    }

    // Stability counts (derivable only when every run row carries evidence).
    if let Some(stable) = derived.stable {
        let unstable = derived.run.saturating_sub(stable);
        for (field, expected) in [
            ("gap_id_stable_count", stable),
            ("gap_id_unstable_count", unstable),
        ] {
            match opt_u64(display, summary, field)? {
                Some(value) if value as usize != expected => {
                    return Err(fail(
                        display,
                        &format!("summary.{field}"),
                        format!(
                            "hand-edited aggregate: summary claims {value} but the rows derive {expected}"
                        ),
                    ));
                }
                _ => {}
            }
        }
    } else if derived.run > 0 {
        // Missing stability evidence must not silently disable the aggregate
        // check: a recorded value cannot be verified against the rows, and an
        // unrecorded one is disclosed incomplete (never invented).
        let recorded = [
            "gap_id_stable_count",
            "gap_id_unstable_count",
            "gap_id_stability_rate",
        ]
        .iter()
        .any(|field| summary_records_value(summary, field));
        if recorded {
            return Err(fail(
                display,
                "summary.gap_id_stable_count",
                "stability aggregate recorded but analyzed rows lack complete stability evidence (gap_ids_stable / repeat.gap_ids_stable on every run row); the value cannot be derived",
            ));
        }
        incomplete.push(Diagnostic::new(
            display,
            "summary.gap_id_stable_count",
            "stability aggregates not derivable: analyzed rows lack complete stability evidence; absent values are disclosed, not invented",
        ));
    }

    // Runtime aggregates (only derivable when every run row carries one).
    if let Some((min, median, max, total)) = derived.runtime {
        for (field, expected) in [
            ("runtime_ms_min", min),
            ("runtime_ms_median", median),
            ("runtime_ms_max", max),
            ("runtime_ms_total", total),
        ] {
            match opt_u64(display, summary, field)? {
                Some(value) if value != expected => {
                    return Err(fail(
                        display,
                        &format!("summary.{field}"),
                        format!(
                            "hand-edited aggregate: summary claims {value} but the rows derive {expected}"
                        ),
                    ));
                }
                _ => {}
            }
        }
    }

    // Rates.
    for (field, expected) in [
        ("crash_rate", derived.crash_rate()),
        ("parse_failure_rate", derived.parse_failure_rate()),
    ] {
        match opt_number(display, summary, field)? {
            Some(value) if (value - expected).abs() > 1e-9 => {
                return Err(fail(
                    display,
                    &format!("summary.{field}"),
                    format!(
                        "hand-edited aggregate: summary claims {value} but the rows derive {expected}"
                    ),
                ));
            }
            _ => {}
        }
    }
    if let Some(expected) = derived.stability_rate() {
        match opt_number(display, summary, "gap_id_stability_rate")? {
            Some(value) if (value - expected).abs() > 1e-9 => {
                return Err(fail(
                    display,
                    "summary.gap_id_stability_rate",
                    format!(
                        "hand-edited aggregate: summary claims {value} but the rows derive {expected}"
                    ),
                ));
            }
            _ => {}
        }
    }

    // Distribution agreement: exact map equality against the row-derived key
    // set — no unknown buckets, no missing buckets the rows establish, and
    // zero-valued buckets participate like any other (the sweep writes every
    // bucket). With zero run rows the zero-run law above already bounds every
    // recorded bucket to zero and requires the full emitted key set; the
    // recorded key set is still vocabulary-checked.
    for (field, vocabulary, derived_counts) in [
        (
            "classification_counts",
            &CLASSIFICATION_VOCABULARY[..],
            &derived.classification,
        ),
        (
            "alignment_counts",
            &ALIGNMENT_VOCABULARY[..],
            &derived.alignment,
        ),
    ] {
        if let Some(summary_counts) = opt_distribution(display, summary, field)? {
            if derived_counts.is_empty() {
                for name in summary_counts.keys() {
                    if !vocabulary.contains(&name.as_str()) {
                        return Err(fail(
                            display,
                            &format!("summary.{field}.{name}"),
                            format!(
                                "unknown aggregate bucket `{name}`; known vocabulary: {}",
                                vocabulary.join(", ")
                            ),
                        ));
                    }
                }
            } else {
                check_distribution_equality(display, field, &summary_counts, derived_counts)?;
            }
        }
    }

    // `gate_reason` is owned by the emitted summary; when recorded it must be
    // a non-empty string (its emitted shape), not a wrong-typed stand-in.
    opt_string(display, summary, "gate_reason")?;

    // Gate semantics: the supplied gate_status must EQUAL the gate derived
    // from the rows — `not_run` at zero runs, `pass` only with zero crashes
    // and full stability evidence, `review` otherwise. A wrong `not_run` or
    // `review` is as hand-edited as a wrong count.
    let expected_gate = if derived.run == 0 {
        "not_run"
    } else if derived.crashed == 0 && derived.stable == Some(derived.run) {
        "pass"
    } else {
        "review"
    };
    match opt_string(display, summary, "gate_status")?.as_deref() {
        Some(gate) => {
            known_value_or_fail(
                display,
                "summary.gate_status",
                gate,
                &GATE_STATUSES,
                "gate status",
            )?;
            if gate != expected_gate {
                if derived.run == 0 {
                    return Err(fail(
                        display,
                        "summary.gate_status",
                        format!(
                            "repos_run == 0 is `not_run`, never `{gate}`: a zero-run receipt must not claim an analyzed verdict"
                        ),
                    ));
                }
                if gate == "pass" {
                    if derived.crashed > 0 {
                        return Err(fail(
                            display,
                            "summary.gate_status",
                            format!(
                                "hand-edited aggregate: `pass` claimed but {} row(s) crashed",
                                derived.crashed
                            ),
                        ));
                    }
                    return Err(fail(
                        display,
                        "summary.gate_status",
                        "`pass` claimed without full per-row stability evidence (gap_ids_stable / repeat.gap_ids_stable on every run row)",
                    ));
                }
                return Err(fail(
                    display,
                    "summary.gate_status",
                    format!(
                        "hand-edited aggregate: gate status `{gate}` does not equal the gate derived from the rows (`{expected_gate}`; repos_run={}, crashes={}, stability evidence complete={})",
                        derived.run,
                        derived.crashed,
                        derived.stable == Some(derived.run)
                    ),
                ));
            }
        }
        None => incomplete.push(Diagnostic::new(
            display,
            "summary.gate_status",
            "gate status not recorded",
        )),
    }
    Ok(())
}

/// True when the summary records a present, non-null value for `field`.
fn summary_records_value(summary: &serde_json::Map<String, Value>, field: &str) -> bool {
    matches!(summary.get(field), Some(value) if !value.is_null())
}

/// Exact map equality between a recorded summary distribution and the
/// row-derived key set: every summary bucket must be row-established (extra
/// buckets are denied by name) and every derived bucket must be present at
/// the derived count.
fn check_distribution_equality(
    display: &str,
    field: &str,
    summary_counts: &BTreeMap<String, u64>,
    derived_counts: &BTreeMap<String, u64>,
) -> Result<(), String> {
    for name in summary_counts.keys() {
        if !derived_counts.contains_key(name) {
            return Err(fail(
                display,
                &format!("summary.{field}.{name}"),
                format!(
                    "summary distribution carries bucket `{name}` that the rows never establish (extra buckets are denied; the known keys come from the row distributions)"
                ),
            ));
        }
    }
    for (name, count) in derived_counts {
        match summary_counts.get(name) {
            Some(actual) if actual == count => {}
            Some(actual) => {
                return Err(fail(
                    display,
                    &format!("summary.{field}.{name}"),
                    format!(
                        "hand-edited aggregate: summary claims {actual} but the rows derive {count}"
                    ),
                ));
            }
            None => {
                return Err(fail(
                    display,
                    &format!("summary.{field}.{name}"),
                    format!(
                        "hand-edited aggregate: rows derive {count} for `{name}` but the summary omits it"
                    ),
                ));
            }
        }
    }
    Ok(())
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

pub(crate) fn run_check(args: &[String]) -> Result<(), String> {
    let parsed = parse_check_args(args)?;
    let outcome = check_artifacts(&parsed.manifest, parsed.runs.as_deref())?;
    let verdict = outcome.verdict();

    println!(
        "eval-sweep check: manifest={} subjects={} sha256={}",
        outcome.manifest_path,
        outcome.accepted.subjects.len(),
        outcome.accepted.sha256
    );
    match &outcome.receipt {
        None => println!(
            "eval-sweep check: receipt=<none> verdict={} (no retained receipt supplied; not_run is not a pass)",
            verdict.as_str()
        ),
        Some(receipt) => println!(
            "eval-sweep check: receipt={} schema={} denominator selected={} run={} verdict={}",
            receipt.path,
            receipt.schema_version,
            receipt.denominator_selected,
            receipt.denominator_run,
            verdict.as_str()
        ),
    }
    let disclosures = outcome.incomplete();
    println!(
        "eval-sweep check: incomplete identities disclosed: {}",
        disclosures.len()
    );
    for diagnostic in &disclosures {
        println!("  incomplete: {}", diagnostic.render());
    }
    println!(
        "eval-sweep check verdict: {} — structural validation only; not a currentness, robustness, or adequacy claim",
        verdict.as_str()
    );
    println!("rerun: {RERUN_COMMAND}");

    let json = render_check_json(&outcome)?;
    crate::write_report(CHECK_REPORT_JSON, &format!("{json}\n"))?;
    let markdown = render_check_markdown(&outcome, verdict);
    crate::write_report(CHECK_REPORT_MD, &markdown)?;
    Ok(())
}

fn render_check_json(outcome: &CheckOutcome) -> Result<String, String> {
    let verdict = outcome.verdict();
    let manifest_incomplete: Vec<Value> = outcome
        .accepted
        .incomplete
        .iter()
        .map(|diagnostic| json!(diagnostic.render()))
        .collect();
    let receipt_json = match &outcome.receipt {
        None => json!(null),
        Some(receipt) => {
            let incomplete: Vec<Value> = receipt
                .incomplete
                .iter()
                .map(|diagnostic| json!(diagnostic.render()))
                .collect();
            json!({
                "path": receipt.path,
                "schema_version": receipt.schema_version,
                "denominator": {
                    "selected": receipt.denominator_selected,
                    "run": receipt.denominator_run,
                },
                "verdict": receipt.verdict().as_str(),
                "incomplete": incomplete,
            })
        }
    };
    let document = json!({
        "schema_version": "0.1",
        "kind": "python_eval_sweep_check_report",
        "spec": KNOWN_SPEC,
        "verdict": verdict.as_str(),
        "manifest": {
            "path": outcome.manifest_path,
            "schema_version": MANIFEST_SCHEMA_VERSION,
            "subjects": outcome.accepted.subjects.len(),
            "subject_ids": outcome.accepted.ids(),
            "sha256": outcome.accepted.sha256,
            "incomplete": manifest_incomplete,
        },
        "receipt": receipt_json,
        "offline": true,
        "claim_boundary": "structural validation only; not a currentness, robustness, or adequacy claim",
        "rerun": RERUN_COMMAND,
    });
    serde_json::to_string_pretty(&document)
        .map_err(|error| format!("failed to render eval-sweep check JSON: {error}"))
}

fn render_check_markdown(outcome: &CheckOutcome, verdict: Verdict) -> String {
    let mut out = String::new();
    out.push_str("# Eval Sweep Check\n\n");
    out.push_str(&format!("Verdict: **{}**\n\n", verdict.as_str()));
    out.push_str(
        "Structural validation only — not a currentness, robustness, or adequacy claim.\n\n",
    );
    out.push_str(&format!(
        "- manifest: {} ({} subjects, sha256 `{}`)\n",
        outcome.manifest_path,
        outcome.accepted.subjects.len(),
        outcome.accepted.sha256
    ));
    for subject in &outcome.accepted.subjects {
        out.push_str(&format!(
            "  - {} @ {} ({} — {})\n",
            subject.id, subject.sha, subject.license, subject.shape
        ));
    }
    match &outcome.receipt {
        None => out.push_str(
            "- receipt: none supplied (`not_run`; not a pass — supply --runs <receipt> to validate retained rows)\n",
        ),
        Some(receipt) => {
            out.push_str(&format!(
                "- receipt: {} schema {}, denominator selected={} run={}, verdict={}\n",
                receipt.path,
                receipt.schema_version,
                receipt.denominator_selected,
                receipt.denominator_run,
                receipt.verdict().as_str()
            ));
        }
    }
    let disclosures = outcome.incomplete();
    out.push_str(&format!(
        "\n## Incomplete identities ({})\n\n",
        disclosures.len()
    ));
    out.push_str(
        "Missing identities are typed incomplete; they are not invented and not errors.\n\n",
    );
    if disclosures.is_empty() {
        out.push_str("None.\n");
    }
    for diagnostic in &disclosures {
        out.push_str(&format!("- {}\n", diagnostic.render()));
    }
    out.push_str(&format!("\nrerun: `{RERUN_COMMAND}`\n"));
    out
}

// ---------------------------------------------------------------------------
// Tests (module named `python_eval_sweep` so
// `cargo test -p xtask python_eval_sweep` selects exactly this module; no
// unwrap/expect — assert macros and Result returns only)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod python_eval_sweep {
    use super::*;

    const VALID_SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const VALID_SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const VALID_SHA_C: &str = "cccccccccccccccccccccccccccccccccccccccc";
    const VALID_SHA_D: &str = "dddddddddddddddddddddddddddddddddddddddd";
    const VALID_SHA_E: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const VALID_SHA_F: &str = "ffffffffffffffffffffffffffffffffffffffff";
    const VALID_SHA_0: &str = "1010101010101010101010101010101010101010";
    const VALID_SHA_1: &str = "1111111111111111111111111111111111111111";
    const DIGEST_ONE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const DIGEST_TWO: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn subject_json(id: &str, sha: &str, shape: &str, license: &str) -> Value {
        json!({
            "id": id,
            "url": format!("https://example.com/{id}"),
            "sha": sha,
            "license": license,
            "shape": shape,
            "synthetic_diff": format!("fixtures/python-eval-sweep/diffs/{id}.diff"),
        })
    }

    /// An alternate valid eight-subject manifest: different ids, shas, shapes,
    /// and licenses from the retained fixture — proves the validator is
    /// data-driven, not fixture-byte-hardcoded.
    fn alternate_manifest() -> Value {
        json!({
            "schema_version": "0.1",
            "kind": "python_eval_sweep_manifest",
            "spec": "RIPR-SPEC-0086",
            "tier": "A",
            "description": "alternate data-driven manifest",
            "repos": [
                subject_json("alpha", VALID_SHA_A, "pytest_library", "MIT"),
                subject_json("bravo", VALID_SHA_B, "unittest_library", "Apache-2.0"),
                subject_json("charlie", VALID_SHA_C, "click_typer", "BSD-3-Clause"),
                subject_json("delta", VALID_SHA_D, "pytest_library", "MIT"),
                subject_json("echo", VALID_SHA_E, "flask_web", "MIT"),
                subject_json("foxtrot", VALID_SHA_F, "fastapi_web", "MIT OR Apache-2.0"),
                subject_json("golf", VALID_SHA_0, "pytest_library", "MIT"),
                subject_json("hotel", VALID_SHA_1, "pytest_library", "BSD-2-Clause"),
            ]
        })
    }

    /// The alternate manifest with every optional identity field present, so
    /// accepted validation discloses zero incompletes (the other pole of the
    /// top-level verdict contract).
    fn identity_complete_manifest() -> Value {
        let mut value = alternate_manifest();
        if let Some(repos) = value.get_mut("repos").and_then(Value::as_array_mut) {
            for repo in repos.iter_mut() {
                if let Some(entry) = repo.as_object_mut() {
                    entry.insert("tree_digest".to_string(), json!(DIGEST_ONE));
                    entry.insert("snapshot".to_string(), json!("snapshot-identities"));
                    entry.insert("provenance".to_string(), json!("campaign-sweep"));
                    entry.insert("retention_class".to_string(), json!("retained-evidence"));
                }
            }
        }
        value
    }

    fn parsed(value: &Value) -> Result<Value, String> {
        let text = serde_json::to_string_pretty(value)
            .map_err(|error| format!("serialize test JSON: {error}"))?;
        parse_json_without_duplicate_keys(&text)
            .map_err(|error| format!("test JSON must parse: {error}"))
    }

    fn accepted_manifest(value: &Value) -> Result<(AcceptedManifest, String), String> {
        let text = serde_json::to_string_pretty(value)
            .map_err(|error| format!("serialize test JSON: {error}"))?;
        let sha = sha256_hex(text.as_bytes());
        let value = parse_json_without_duplicate_keys(&text)
            .map_err(|error| format!("test JSON must parse: {error}"))?;
        let manifest = validate_accepted_manifest(&value, sha.clone())?;
        Ok((manifest, sha))
    }

    /// Asserts a fail-closed result: Err, mentioning `needle` and the rerun
    /// command. Returns a test failure otherwise.
    fn expect_fail(result: Result<(), String>, needle: &str) -> Result<(), String> {
        let error = match result {
            Ok(()) => {
                return Err(format!(
                    "expected failure containing `{needle}`, got success"
                ));
            }
            Err(error) => error,
        };
        if !error.contains(needle) {
            return Err(format!("failure `{error}` must mention `{needle}`"));
        }
        if !error.contains(RERUN_COMMAND) {
            return Err(format!("failure `{error}` must carry the rerun command"));
        }
        Ok(())
    }

    fn validate_manifest_value(value: &Value) -> Result<(), String> {
        validate_accepted_manifest(value, String::new()).map(|_| ())
    }

    fn validate_receipt_value(
        receipt: &Value,
        manifest: &AcceptedManifest,
        manifest_sha: &str,
    ) -> Result<ReceiptCheck, String> {
        validate_run_receipt(receipt, manifest_sha, manifest, "receipt.json")
    }

    // -- data-driven acceptance -------------------------------------------

    #[test]
    fn accepts_alternate_valid_manifest_not_fixture_bytes() -> Result<(), String> {
        let (manifest, _) = accepted_manifest(&alternate_manifest())?;
        assert_eq!(manifest.subjects.len(), 8);
        assert!(manifest.subject("alpha").is_some());
        assert!(manifest.subject("hotel").is_some());
        assert!(manifest.subject("click").is_none());
        // All identities the retained fixture lacks are disclosed incomplete:
        // tree_digest, snapshot, provenance, retention_class per subject.
        assert_eq!(manifest.incomplete.len(), 8 * 4);
        Ok(())
    }

    #[test]
    fn check_artifacts_passes_on_alternate_manifest_in_temp_dir() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-evalsweep-check-valid-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|error| format!("create dir: {error}"))?;
        let path = dir.join("manifest.json");
        let text = serde_json::to_string_pretty(&alternate_manifest())
            .map_err(|error| error.to_string())?;
        std::fs::write(&path, &text).map_err(|error| format!("write manifest: {error}"))?;

        let outcome = check_artifacts(path.to_string_lossy().as_ref(), None);
        let _ = std::fs::remove_dir_all(&dir);

        let outcome = outcome?;
        assert_eq!(outcome.accepted.subjects.len(), 8);
        assert_eq!(outcome.verdict(), Verdict::NotRun);
        Ok(())
    }

    // -- manifest failure shapes -------------------------------------------

    #[test]
    fn rejects_manifest_with_wrong_kind() -> Result<(), String> {
        let mut value = alternate_manifest();
        value["kind"] = json!("some_other_manifest");
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "expected `python_eval_sweep_manifest`",
        )
    }

    #[test]
    fn rejects_manifest_with_seven_subjects_changed_denominator() -> Result<(), String> {
        let mut value = alternate_manifest();
        if let Some(repos) = value["repos"].as_array_mut() {
            repos.pop();
        }
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "exactly 8 selected subjects, got 7",
        )
    }

    /// Subject ids are safe identifiers: path-traversal and hidden/relative
    /// ids fail with a named diagnostic, while the full safe charset (at the
    /// length cap) passes. Existing valid ids (the fixture's and the
    /// alternate manifest's) are covered by the acceptance tests and the
    /// `eval-sweep check` gate over the canonical fixture.
    #[test]
    fn manifest_rejects_unsafe_subject_ids() -> Result<(), String> {
        let swapped = |id: &str| -> Result<Value, String> {
            let mut value = alternate_manifest();
            let entry = value
                .get_mut("repos")
                .and_then(Value::as_array_mut)
                .and_then(|repos| repos.first_mut())
                .and_then(Value::as_object_mut)
                .ok_or_else(|| "alternate manifest repos[0]".to_string())?;
            entry.insert("id".to_string(), json!(id));
            Ok(value)
        };
        for (bad, needle) in [
            ("../../outside", "must not start with"),
            ("..", "must not start with"),
            (".hidden", "must not start with"),
            ("a/b", "must use only"),
            ("a\\b", "must use only"),
            ("a b", "must use only"),
        ] {
            expect_fail(validate_manifest_value(&parsed(&swapped(bad)?)?), needle)?;
        }
        let long_id = "a".repeat(65);
        expect_fail(
            validate_manifest_value(&parsed(&swapped(&long_id)?)?),
            "at most 64 characters",
        )?;

        // The full safe charset at the exact length cap passes.
        let edge_ok = "b.x_y-9".repeat(9) + "c";
        assert_eq!(edge_ok.len(), 64, "edge id must sit at the cap");
        validate_manifest_value(&parsed(&swapped(&edge_ok)?)?)?;
        Ok(())
    }

    #[test]
    fn rejects_duplicate_subject_ids() -> Result<(), String> {
        let mut value = alternate_manifest();
        if let Some(repos) = value["repos"].as_array_mut()
            && let Some(first) = repos.first().cloned()
        {
            repos[1] = first;
        }
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "duplicate subject id",
        )
    }

    /// Subject ids are unique CASE-insensitively (ASCII): they become
    /// filesystem path components, and on a case-insensitive filesystem
    /// (Windows, default macOS) `Alpha` and `alpha` would resolve to one
    /// candidate directory — two distinct subjects colliding into one tree.
    /// The charset already bars `~` (short-name aliases) and separators; the
    /// case alias is rejected at the duplicate check.
    #[test]
    fn rejects_subject_ids_differing_only_by_case() -> Result<(), String> {
        let mut value = alternate_manifest();
        let first_id = value["repos"][0]["id"]
            .as_str()
            .ok_or_else(|| "first subject id".to_string())?
            .to_string();
        let uppercase = first_id.to_ascii_uppercase();
        assert_ne!(
            first_id, uppercase,
            "the test needs an id with a distinct uppercase form"
        );
        value["repos"][1]["id"] = json!(uppercase);
        value["repos"][1]["url"] = json!(format!("https://example.com/{uppercase}"));
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "duplicate subject id",
        )?;
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "case-insensitively",
        )
    }

    #[test]
    fn rejects_non_https_and_credential_urls() -> Result<(), String> {
        let mut value = alternate_manifest();
        value["repos"][0]["url"] = json!("http://example.com/alpha");
        expect_fail(validate_manifest_value(&parsed(&value)?), "must be https")?;

        let mut value = alternate_manifest();
        value["repos"][0]["url"] = json!("https://user:pass@example.com/alpha");
        expect_fail(validate_manifest_value(&parsed(&value)?), "credentials")
    }

    #[test]
    fn rejects_absolute_and_secret_bearing_paths() -> Result<(), String> {
        let mut value = alternate_manifest();
        // The drive letter is assembled at runtime so this source file never
        // contains a local absolute path for the local-context gate.
        let drive_letter_absolute = format!("{}:/repo/fixtures/alpha.diff", 'C');
        value["repos"][0]["synthetic_diff"] = json!(drive_letter_absolute);
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "drive-letter absolute path",
        )?;

        let mut value = alternate_manifest();
        value["repos"][0]["synthetic_diff"] = json!("configs/api_key/alpha.diff");
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "secret-shaped token",
        )
    }

    #[test]
    fn rejects_unknown_shape_tag_and_malformed_sha() -> Result<(), String> {
        let mut value = alternate_manifest();
        value["repos"][0]["shape"] = json!("pytest_monorepo");
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "unknown shape/layout tag",
        )?;

        let mut value = alternate_manifest();
        value["repos"][0]["sha"] = json!("deadbeef");
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "40-character commit SHA",
        )
    }

    #[test]
    fn rejects_duplicate_json_keys_at_load() {
        let body = format!(
            "{{\"kind\": \"a\", \"kind\": \"b\", \"schema_version\": \"{MANIFEST_SCHEMA_VERSION}\", \"spec\": \"{KNOWN_SPEC}\", \"tier\": \"{KNOWN_TIER}\", \"repos\": []}}"
        );
        let parsed = parse_json_without_duplicate_keys(&body);
        assert!(parsed.is_err(), "duplicate keys must fail at load");
    }

    #[test]
    fn manifest_rejects_unknown_top_level_and_repo_keys() -> Result<(), String> {
        let mut value = alternate_manifest();
        value["notes"] = json!("unexpected");
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "unknown field `notes`",
        )?;

        let mut value = alternate_manifest();
        value["repos"][0]["coverage"] = json!(0.9);
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "unknown field `coverage`",
        )
    }

    #[test]
    fn canonical_fixture_manifest_passes_with_owned_keys_only() -> Result<(), String> {
        // The deny-unknown decision is only free if the canonical fixture
        // carries exactly the owned keys; run the real bytes end to end.
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../fixtures/python-eval-sweep/manifest.json"
        );
        let outcome = check_artifacts(path, None)?;
        assert_eq!(outcome.accepted.subjects.len(), 8);
        assert_eq!(outcome.accepted.subjects[0].id, "click");
        assert_eq!(outcome.verdict(), Verdict::NotRun);
        Ok(())
    }

    // -- retained historical receipt (0.2) ---------------------------------

    /// A historical Tier A receipt shape (schema 0.2) for the alternate
    /// subjects, mirroring the retained #1160 receipt: 8 rows, 2 parse
    /// failures, 1 timeout, gate pass with full stability. Rows: outcomes
    /// index-aligned; runtimes 1000..8000 (min 1000, median 5000, max 8000,
    /// total 36000).
    fn historical_receipt_0_2(value_manifest: &Value) -> Value {
        let repos = value_manifest
            .get("repos")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let statuses = [
            "ok",
            "parse_failure",
            "ok",
            "ok",
            "ok",
            "parse_failure",
            "timed_out",
            "ok",
        ];
        let mut rows = Vec::new();
        for (index, repo) in repos.iter().enumerate() {
            let id = repo.get("id").and_then(Value::as_str).unwrap_or_default();
            let sha = repo.get("sha").and_then(Value::as_str).unwrap_or_default();
            let shape = repo
                .get("shape")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let outcome = statuses.get(index).copied().unwrap_or("ok");
            rows.push(json!({
                "id": id,
                "sha": sha,
                "shape": shape,
                "outcome": outcome,
                "runtime_ms": 1000 + index as u64 * 1000,
                "gap_ids": [],
                "gap_ids_stable": true,
                "unstable_gap_ids": [],
                "stderr_excerpt": "",
                "classification_counts": {
                    "exposed": 0,
                    "weakly_exposed": 0,
                    "reachable_unrevealed": 0,
                    "no_static_path": 0,
                    "infection_unknown": 0,
                    "propagation_unknown": 0,
                    "static_unknown": if outcome == "parse_failure" { 1 } else { 0 },
                },
                "alignment_counts": {
                    "direct": 0,
                    "alias": 0,
                    "changed_sink_token": 0,
                    "orthogonal": 0,
                    "unknown": 0,
                    "absent": 0,
                },
            }));
        }
        json!({
            "schema_version": "0.2",
            "kind": "python_eval_sweep_report",
            "spec": KNOWN_SPEC,
            "tier": "A",
            "summary": {
                "repos_total": 8,
                "repos_run": 8,
                "repos_skipped": 0,
                "repos_clone_failed": 0,
                "crash_count": 0,
                "crash_rate": 0.0,
                "parse_failure_count": 2,
                "parse_failure_rate": 0.25,
                "timed_out_count": 1,
                "runtime_ms_min": 1000,
                "runtime_ms_median": 5000,
                "runtime_ms_max": 8000,
                "runtime_ms_total": 36000,
                "gap_id_stable_count": 8,
                "gap_id_unstable_count": 0,
                "gap_id_stability_rate": 1.0,
                "classification_counts": {
                    "exposed": 0,
                    "weakly_exposed": 0,
                    "reachable_unrevealed": 0,
                    "no_static_path": 0,
                    "infection_unknown": 0,
                    "propagation_unknown": 0,
                    "static_unknown": 2,
                },
                "alignment_counts": {
                    "direct": 0,
                    "alias": 0,
                    "changed_sink_token": 0,
                    "orthogonal": 0,
                    "unknown": 0,
                    "absent": 0,
                },
                "gate_status": "pass",
                "gate_reason": "8 repo(s) analyzed; no crashes; canonical gap IDs stable across the re-run",
            },
            "repos": rows,
        })
    }

    #[test]
    fn historical_receipt_validates_incomplete_without_rewrite() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        let receipt = historical_receipt_0_2(&alternate_manifest());
        let check = validate_receipt_value(&receipt, &manifest, &sha)?;
        assert_eq!(check.schema_version, RECEIPT_SCHEMA_0_2);
        // Every row stays selected; all eight outcomes count as run.
        assert_eq!(check.denominator_selected, 8);
        assert_eq!(check.denominator_run, 8);
        // Historical rows disclose their missing currentness identities.
        assert!(
            !check.incomplete.is_empty(),
            "historical receipt must disclose incomplete identities"
        );
        assert!(check.incomplete.iter().any(|diagnostic| {
            diagnostic.subject == "alpha" && diagnostic.field == "currentness identities"
        }));
        assert!(
            check
                .incomplete
                .iter()
                .any(|diagnostic| diagnostic.field == "manifest_digest")
        );
        Ok(())
    }

    #[test]
    fn historical_receipt_keeps_failed_rows_selected() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        // Flip one row to each non-run outcome; both must remain selected.
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut) {
            if let Some(entry) = rows[0].as_object_mut() {
                entry.insert("outcome".to_string(), json!("clone_failed"));
            }
            if let Some(entry) = rows[7].as_object_mut() {
                entry.insert("outcome".to_string(), json!("skipped_missing_checkout"));
            }
        }
        // Re-derive the hand-entered aggregates honestly (run = 6).
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.insert("repos_run".to_string(), json!(6));
            summary.insert("repos_clone_failed".to_string(), json!(1));
            summary.insert("repos_skipped".to_string(), json!(1));
            summary.insert("gap_id_stable_count".to_string(), json!(6));
            summary.insert("runtime_ms_min".to_string(), json!(2000));
            summary.insert("runtime_ms_median".to_string(), json!(5000));
            summary.insert("runtime_ms_max".to_string(), json!(7000));
            summary.insert("runtime_ms_total".to_string(), json!(27000));
            summary.insert("parse_failure_rate".to_string(), json!(0.3333333333333333));
        }
        let check = validate_receipt_value(&receipt, &manifest, &sha)?;
        assert_eq!(check.denominator_selected, 8);
        assert_eq!(check.denominator_run, 6);
        Ok(())
    }

    #[test]
    fn receipt_rejects_unknown_subject_and_missing_subject() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert("id".to_string(), json!("outsider"));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "outside the accepted denominator",
        )?;

        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut) {
            rows.pop();
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "changed denominator",
        )
    }

    #[test]
    fn receipt_rejects_duplicate_rows() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(first) = rows.first().cloned()
        {
            rows[1] = first;
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "duplicate subject row",
        )
    }

    #[test]
    fn receipt_rejects_unknown_outcome_state() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert("outcome".to_string(), json!("mostly_fine"));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "unknown run outcome",
        )
    }

    #[test]
    fn receipt_rejects_identity_contradiction_with_manifest_pin() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert(
                "sha".to_string(),
                json!("9999999999999999999999999999999999999999"),
            );
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "does not match the accepted manifest pin",
        )
    }

    #[test]
    fn receipt_rejects_stale_manifest_digest() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(object) = receipt.as_object_mut() {
            object.insert("manifest_digest".to_string(), json!(DIGEST_ONE));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "stale digest",
        )
    }

    #[test]
    fn gate_status_must_equal_the_derived_gate() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        // Analyzed, crash-free, fully stable rows derive `pass`: a claimed
        // `not_run` fails even though it is in the vocabulary.
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.insert("gate_status".to_string(), json!("not_run"));
            summary.insert("gate_reason".to_string(), json!("unwarranted"));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "does not equal the gate derived from the rows",
        )?;

        // The same rows with an unwarranted `review` fail too.
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.insert("gate_status".to_string(), json!("review"));
            summary.insert("gate_reason".to_string(), json!("unwarranted"));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "does not equal the gate derived from the rows",
        )
    }

    #[test]
    fn analyzed_row_missing_distribution_fails_and_rejects_summary_totals() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.remove("classification_counts");
        }
        // Arbitrary summary totals cannot rescue a row that omits its
        // aggregate source evidence.
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.insert(
                "classification_counts".to_string(),
                json!({"exposed": 99, "weakly_exposed": 0, "reachable_unrevealed": 0, "no_static_path": 0, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0}),
            );
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "omits its classification distribution",
        )
    }

    #[test]
    fn analyzed_row_missing_runtime_fails() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.remove("runtime_ms");
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "omits its runtime",
        )
    }

    #[test]
    fn under_evidenced_stability_rejects_recorded_summary_and_discloses_absent()
    -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        // 0.3 stability evidence lives in the optional `repeat` block: a run
        // row without it disables the stability aggregate, so a recorded
        // summary stability value fails (it cannot be derived).
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
            && let Some(Value::Object(repeat)) = entry.get_mut("repeat")
        {
            repeat.remove("gap_ids_stable");
        }
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.insert("gap_id_stable_count".to_string(), json!(5));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "stability aggregate recorded but analyzed rows lack complete stability evidence",
        )?;

        // Without recorded values the gap is disclosed incomplete, not
        // invented and not failed: over rows lacking `repeat` evidence the
        // emitter could not have written the stability aggregates, so they
        // are omitted here and the absence is disclosed.
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
            && let Some(Value::Object(repeat)) = entry.get_mut("repeat")
        {
            repeat.remove("gap_ids_stable");
        }
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.remove("gap_id_stable_count");
            summary.remove("gap_id_unstable_count");
            summary.remove("gap_id_stability_rate");
        }
        let check = validate_receipt_value(&receipt, &manifest, &sha)?;
        assert!(
            check
                .incomplete
                .iter()
                .any(|diagnostic| diagnostic.field == "summary.gap_id_stable_count"),
            "under-evidenced stability must be disclosed incomplete: {:?}",
            check.incomplete
        );
        Ok(())
    }

    #[test]
    fn summary_distribution_extra_bucket_fails() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        // An extra classification bucket cannot hide behind derived-key
        // iteration.
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut)
            && let Some(Value::Object(counts)) = summary.get_mut("classification_counts")
        {
            counts.insert("flawlessly_verified".to_string(), json!(1));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "never establish",
        )?;

        // An extra alignment bucket fails the same way.
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut)
            && let Some(Value::Object(counts)) = summary.get_mut("alignment_counts")
        {
            counts.insert("repair_placement_present".to_string(), json!(1));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "never establish",
        )
    }

    #[test]
    fn zero_valued_buckets_participate_in_exact_agreement() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        // Zero-valued buckets are part of the emitted shape (the sweep writes
        // every bucket), so a missing zero bucket still fails.
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut)
            && let Some(Value::Object(counts)) = summary.get_mut("alignment_counts")
        {
            counts.remove("direct");
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "but the summary omits it",
        )?;

        // The full zero-filled bucket set agrees.
        let receipt = historical_receipt_0_2(&alternate_manifest());
        validate_receipt_value(&receipt, &manifest, &sha)?;
        Ok(())
    }

    #[test]
    fn runtime_total_overflow_fails_structurally_without_panic() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut) {
            for row in rows.iter_mut().take(2) {
                if let Some(entry) = row.as_object_mut() {
                    entry.insert("runtime_ms".to_string(), json!(u64::MAX));
                }
            }
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "summary.runtime_ms_total",
        )?;
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "aggregate overflow",
        )
    }

    #[test]
    fn distribution_merge_overflow_fails_structurally_without_panic() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut) {
            for row in rows.iter_mut().take(2) {
                if let Some(entry) = row.as_object_mut()
                    && let Some(Value::Object(counts)) = entry.get_mut("classification_counts")
                {
                    counts.insert("static_unknown".to_string(), json!(u64::MAX));
                }
            }
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "summary.classification_counts.static_unknown",
        )?;
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "aggregate overflow",
        )
    }

    #[test]
    fn receipt_rejects_hand_edited_aggregates() -> Result<(), String> {
        for (field, wrong) in [
            ("repos_total", json!(9)),
            ("repos_run", json!(7)),
            ("crash_count", json!(3)),
            ("parse_failure_count", json!(5)),
            ("gap_id_stable_count", json!(4)),
            ("runtime_ms_total", json!(99)),
            ("gap_id_stability_rate", json!(0.5)),
        ] {
            let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
            let mut receipt = historical_receipt_0_2(&alternate_manifest());
            if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
                summary.insert(field.to_string(), wrong);
            }
            expect_fail(
                validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
                "hand-edited aggregate",
            )?;
        }
        Ok(())
    }

    /// A zero-run receipt: every row is a skipped non-run row with zero
    /// analysis counts, and the hand-entered aggregates are honestly all-zero
    /// with the not_run gate. The stability rate is 0.0 here; the live
    /// emitter's own zero-run shape records the vacuous 1.0 instead, which
    /// validates as a named incomplete disclosure (see
    /// `emitter_shaped_zero_run_stability_rate_discloses_instead_of_failing`).
    fn zero_run_receipt_0_2(value_manifest: &Value) -> Value {
        let mut receipt = historical_receipt_0_2(value_manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut) {
            for row in rows.iter_mut() {
                if let Some(entry) = row.as_object_mut() {
                    entry.insert("outcome".to_string(), json!("skipped_missing_checkout"));
                    entry.insert(
                        "classification_counts".to_string(),
                        json!({"exposed": 0, "weakly_exposed": 0, "reachable_unrevealed": 0, "no_static_path": 0, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0}),
                    );
                    entry.insert(
                        "alignment_counts".to_string(),
                        json!({"direct": 0, "alias": 0, "changed_sink_token": 0, "orthogonal": 0, "unknown": 0, "absent": 0}),
                    );
                }
            }
        }
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.insert("repos_run".to_string(), json!(0));
            summary.insert("repos_skipped".to_string(), json!(8));
            summary.insert("gap_id_stable_count".to_string(), json!(0));
            summary.insert("gap_id_stability_rate".to_string(), json!(0.0));
            summary.insert("crash_rate".to_string(), json!(0.0));
            summary.insert("parse_failure_count".to_string(), json!(0));
            summary.insert("parse_failure_rate".to_string(), json!(0.0));
            summary.insert("timed_out_count".to_string(), json!(0));
            summary.insert("runtime_ms_min".to_string(), json!(0));
            summary.insert("runtime_ms_median".to_string(), json!(0));
            summary.insert("runtime_ms_max".to_string(), json!(0));
            summary.insert("runtime_ms_total".to_string(), json!(0));
            summary.insert(
                "classification_counts".to_string(),
                json!({"exposed": 0, "weakly_exposed": 0, "reachable_unrevealed": 0, "no_static_path": 0, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0}),
            );
            // The emitter zero-fills every alignment bucket, at zero runs
            // included (eval_sweep.rs `to_json`), so the recorded summary
            // carries the full nine-key set — `absent`/`unknown` and the
            // three repair-packet counters included.
            summary.insert(
                "alignment_counts".to_string(),
                json!({"direct": 0, "alias": 0, "changed_sink_token": 0, "orthogonal": 0, "unknown": 0, "absent": 0, "repair_placement_present": 0, "verify_command_present": 0, "python_repair_card_present": 0}),
            );
            summary.insert("gate_status".to_string(), json!("not_run"));
            summary.insert("gate_reason".to_string(), json!("no repos analyzed"));
        }
        receipt
    }

    #[test]
    fn receipt_rejects_vacuous_pass_and_accepts_not_run() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        // A zero-run receipt claiming pass fails: never a vacuous pass.
        let mut receipt = zero_run_receipt_0_2(&alternate_manifest());
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.insert("gate_status".to_string(), json!("pass"));
            summary.insert("gate_reason".to_string(), json!("nothing ran"));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "repos_run == 0 is `not_run`, never `pass`",
        )?;

        // The same zero-run receipt with the honest not_run gate validates.
        let receipt = zero_run_receipt_0_2(&alternate_manifest());
        let check = validate_receipt_value(&receipt, &manifest, &sha)?;
        assert_eq!(check.denominator_selected, 8);
        assert_eq!(check.denominator_run, 0);
        Ok(())
    }

    #[test]
    fn zero_run_receipt_rejects_fabricated_summary_aggregates() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        // A nonzero classification bucket claims analysis that never ran.
        let mut receipt = zero_run_receipt_0_2(&alternate_manifest());
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut)
            && let Some(Value::Object(counts)) = summary.get_mut("classification_counts")
        {
            counts.insert("static_unknown".to_string(), json!(5));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "summary.classification_counts.static_unknown",
        )?;

        // The same zero-run law bounds the runtime aggregates and the
        // stability counts — any nonzero value is fabricated. For the
        // stability rate only values OTHER than the emitter's vacuous
        // zero-run 1.0 fail; exactly 1.0 discloses incomplete instead (the
        // emitter-shaped receipt is pinned separately below).
        for (field, value) in [
            ("runtime_ms_total", json!(1234)),
            ("gap_id_stable_count", json!(3)),
            ("gap_id_stability_rate", json!(0.5)),
        ] {
            let mut receipt = zero_run_receipt_0_2(&alternate_manifest());
            if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
                summary.insert(field.to_string(), value);
            }
            expect_fail(
                validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
                &format!("summary.{field}"),
            )?;
        }

        // The all-zero summary validates (absent would too).
        let receipt = zero_run_receipt_0_2(&alternate_manifest());
        validate_receipt_value(&receipt, &manifest, &sha)?;
        Ok(())
    }

    #[test]
    fn emitter_shaped_zero_run_stability_rate_discloses_instead_of_failing() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        // A fresh zero-run receipt straight from the live emitter records
        // `gap_id_stability_rate: 1.0` (eval_sweep.rs `compute_metrics`
        // empty-set guard), so a real untouched `eval-sweep` report must not
        // fail check. The rate validates — as a named incomplete disclosure
        // (`vacuous zero-run stability rate`), which keeps the receipt
        // verdict `incomplete`, never a pass.
        let mut receipt = zero_run_receipt_0_2(&alternate_manifest());
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.insert("gap_id_stability_rate".to_string(), json!(1.0));
        }
        let check = validate_receipt_value(&receipt, &manifest, &sha)?;
        assert_eq!(check.denominator_run, 0);
        assert!(
            check.incomplete.iter().any(|diagnostic| {
                diagnostic.field == "summary.gap_id_stability_rate"
                    && diagnostic
                        .reason
                        .contains("vacuous zero-run stability rate")
            }),
            "the emitter's zero-run 1.0 rate must be disclosed vacuous, not failed: {:?}",
            check.incomplete
        );
        assert_eq!(check.verdict(), Verdict::Incomplete);

        // Any OTHER nonzero rate at zero runs is still a fabricated claim
        // about rows that never ran.
        let mut receipt = zero_run_receipt_0_2(&alternate_manifest());
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.insert("gap_id_stability_rate".to_string(), json!(0.5));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "summary.gap_id_stability_rate",
        )
    }

    #[test]
    fn receipt_rejects_pass_claim_without_stability_evidence() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        // A missing 0.2 stability field on an analyzed row fails at the row
        // level (H4: aggregate source evidence is required there).
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.remove("gap_ids_stable");
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "omits its gap-ID stability evidence",
        )?;

        // With evidence present but not all-stable (stable=false plus the
        // unstable IDs the comparison produced), a claimed `pass` still fails
        // at the gate. The stability aggregates are re-derived honestly so
        // the gate equality is what fails.
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert("gap_ids_stable".to_string(), json!(false));
            entry.insert("unstable_gap_ids".to_string(), json!(["gap:python:x"]));
        }
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.insert("gap_id_stable_count".to_string(), json!(7));
            summary.insert("gap_id_unstable_count".to_string(), json!(1));
            summary.insert("gap_id_stability_rate".to_string(), json!(0.875));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "`pass` claimed without full per-row stability evidence",
        )
    }

    #[test]
    fn receipt_rejects_contradictory_stability_claims() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert("unstable_gap_ids".to_string(), json!(["gap:python:x"]));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "contradictory status",
        )
    }

    #[test]
    fn receipt_keeps_absent_distinct_from_unknown_distributions() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        // A recorded distribution missing the `absent` key fails.
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
            && let Some(Value::Object(align)) = entry.get_mut("alignment_counts")
        {
            align.remove("absent");
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "keep `absent` (field not emitted) distinct from `unknown` (emitted value)",
        )?;

        // A distribution with distinct absent and unknown populations validates.
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert(
                    "alignment_counts".to_string(),
                    json!({"direct": 1, "alias": 0, "changed_sink_token": 0, "orthogonal": 2, "unknown": 3, "absent": 0}),
                );
            entry.insert(
                    "classification_counts".to_string(),
                    json!({"exposed": 0, "weakly_exposed": 1, "reachable_unrevealed": 0, "no_static_path": 2, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0}),
                );
        }
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.insert(
                "alignment_counts".to_string(),
                json!({"direct": 1, "alias": 0, "changed_sink_token": 0, "orthogonal": 2, "unknown": 3, "absent": 0}),
            );
            summary.insert(
                "classification_counts".to_string(),
                json!({"exposed": 0, "weakly_exposed": 1, "reachable_unrevealed": 0, "no_static_path": 2, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 2}),
            );
        }
        validate_receipt_value(&receipt, &manifest, &sha)?;
        Ok(())
    }

    #[test]
    fn receipt_rejects_unknown_classification_vocabulary() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            // Any value outside the conservative 7-class vocabulary fails —
            // here a made-up strong-claim word, which is exactly the family
            // the language rules ban from real outputs.
            entry.insert(
                "classification_counts".to_string(),
                json!({"flawlessly_verified": 1}),
            );
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "unknown classification",
        )
    }

    // -- currentness receipt (0.3) ------------------------------------------

    /// A fully-current 0.3 receipt for the given manifest: every status in
    /// the eight-word vocabulary appears once; failed/unavailable rows stay
    /// selected and only the five analysis-attempting statuses count as run.
    /// Row identities restate the manifest's own values when it pins them
    /// (the receipt binds against the manifest side), and the summary carries
    /// the full emitted aggregate set the sweep writes (#3733 review) minus
    /// the stability aggregates: the builder attaches the `repeat` block only
    /// to `complete` rows — the producer shape, since a stability pass only
    /// runs where the first result is complete — so stability is under-
    /// evidenced whenever any run row is non-complete and the aggregates are
    /// absent (typed incomplete, never derived over partial evidence).
    fn current_receipt_0_3(manifest: &AcceptedManifest) -> Value {
        let statuses = [
            "complete",
            "partial",
            "parse-failed",
            "timed-out",
            "crashed",
            "unsupported",
            "tempfail",
            "stale",
        ];
        let executions = [
            "executed",
            "executed",
            "executed",
            "timed-out",
            "failed",
            "not-executed",
            "not-executed",
            "unknown",
        ];
        let mut rows = Vec::new();
        for (index, subject) in manifest.subjects.iter().enumerate() {
            let status = statuses[index];
            let execution = executions[index];
            let ran = matches!(
                status,
                "complete" | "partial" | "parse-failed" | "timed-out" | "crashed"
            );
            let tree = subject
                .tree_digest
                .clone()
                .unwrap_or_else(|| DIGEST_ONE.to_string());
            let snapshot = subject
                .snapshot
                .clone()
                .unwrap_or_else(|| format!("snapshot-{}", subject.id));
            let provenance = subject
                .provenance
                .clone()
                .unwrap_or_else(|| "campaign-sweep".to_string());
            let retention = subject
                .retention_class
                .clone()
                .unwrap_or_else(|| "retained-evidence".to_string());
            let mut row = json!({
                "id": subject.id,
                "status": status,
                "repository": {
                    "url": subject.url,
                    "sha": subject.sha,
                    "tree_digest": tree,
                },
                "tree_digest": tree,
                "snapshot": snapshot,
                "license": subject.license,
                "retention_class": retention,
                "provenance": provenance,
                "selected_root": format!("target/ripr/eval-sweep/checkouts/{}", subject.id),
                "layout": ["pytest_library"],
                "binary": {
                    "digest": DIGEST_TWO,
                    "version": "ripr 0.11.0",
                    "features": ["python"],
                    "build_profile": "debug",
                },
                "config": {
                    "profile": "ripr-default",
                    "input": "ripr.toml.example",
                },
                "input_digest": DIGEST_ONE,
                "materialization": if ran { "materialized" } else { "absent" },
                "detection": if ran { "detected" } else { "absent" },
                "corpus_selection": {
                    "state": if ran { "selected" } else { "absent" },
                    "source_files": 10 + index as u64,
                    "test_files": 5 + index as u64,
                    "generated_files": 0,
                    "vendor_files": 0,
                },
                "execution": execution,
                "digests": {
                    "raw": DIGEST_ONE,
                    "output": DIGEST_TWO,
                    "evidence": DIGEST_ONE,
                },
                "runtime_ms": 100 * (index as u64 + 1),
                "classification_counts": {
                    "exposed": 0,
                    "weakly_exposed": if status == "complete" { 1 } else { 0 },
                    "reachable_unrevealed": 0,
                    "no_static_path": 0,
                    "infection_unknown": 0,
                    "propagation_unknown": 0,
                    "static_unknown": if status == "parse-failed" { 1 } else { 0 },
                },
                "alignment_counts": {
                    "direct": 0,
                    "alias": 0,
                    "changed_sink_token": 0,
                    "orthogonal": if status == "complete" { 1 } else { 0 },
                    "unknown": if status == "partial" { 1 } else { 0 },
                    "absent": if matches!(status, "parse-failed" | "crashed" | "timed-out") { 1 } else { 0 },
                },
            });
            // Non-run rows carry no analysis counts (nothing was analyzed).
            if !ran && let Some(entry) = row.as_object_mut() {
                entry.insert(
                        "classification_counts".to_string(),
                        json!({"exposed": 0, "weakly_exposed": 0, "reachable_unrevealed": 0, "no_static_path": 0, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0}),
                    );
                entry.insert(
                        "alignment_counts".to_string(),
                        json!({"direct": 0, "alias": 0, "changed_sink_token": 0, "orthogonal": 0, "unknown": 0, "absent": 0}),
                    );
            }
            // The `repeat` block exists only where the stability pass actually
            // compared — a `complete` first result. A compared non-complete
            // row would claim stability over no comparison (equal failure gap
            // sets must never read as `stable`), so the builder matches the
            // producer: non-complete rows omit the block and disclose the
            // absent comparison as typed incomplete.
            if status == "complete"
                && let Some(entry) = row.as_object_mut()
            {
                entry.insert(
                    "repeat".to_string(),
                    json!({
                        "comparable_with": format!("{}#run-1", subject.id),
                        "gap_ids_stable": true,
                        "unstable_gap_ids": [],
                    }),
                );
            }
            rows.push(row);
        }
        // Derived aggregates over the five run rows (complete/partial/
        // parse-failed/timed-out/crashed): one crash, one parse failure, one
        // timeout, one tempfail; runtimes 100..500 (min 100, median 300, max
        // 500, total 1500); classification weakly_exposed=1 + static_unknown=1;
        // alignment orthogonal=1, unknown=1, absent=3. Only the complete row
        // carries `repeat` evidence, so the stability aggregates are absent —
        // the under-evidenced path the validator discloses, never a value
        // derived over partial evidence. The rest of the emitted summary is
        // present, exactly as the sweep writes it (#3733 review).
        json!({
            "schema_version": "0.3",
            "kind": "python_eval_sweep_report",
            "spec": KNOWN_SPEC,
            "tier": "A",
            "manifest_digest": manifest.sha256,
            "ripr": {
                "source_sha": VALID_SHA_A,
                "tree_digest": DIGEST_ONE,
                "binary_digest": DIGEST_TWO,
                "version": "ripr 0.11.0",
                "features": ["python"],
                "build_profile": "debug",
            },
            "summary": {
                "repos_total": 8,
                "repos_run": 5,
                "repos_skipped": 0,
                "repos_clone_failed": 1,
                "crash_count": 1,
                "crash_rate": 0.2,
                "parse_failure_count": 1,
                "parse_failure_rate": 0.2,
                "timed_out_count": 1,
                "runtime_ms_min": 100,
                "runtime_ms_median": 300,
                "runtime_ms_max": 500,
                "runtime_ms_total": 1500,
                "classification_counts": {
                    "exposed": 0,
                    "weakly_exposed": 1,
                    "reachable_unrevealed": 0,
                    "no_static_path": 0,
                    "infection_unknown": 0,
                    "propagation_unknown": 0,
                    "static_unknown": 1,
                },
                "alignment_counts": {
                    "direct": 0,
                    "alias": 0,
                    "changed_sink_token": 0,
                    "orthogonal": 1,
                    "unknown": 1,
                    "absent": 3,
                },
                "gate_status": "review",
                "gate_reason": "1 crash over 5 run rows",
            },
            "repos": rows,
        })
    }

    #[test]
    fn current_receipt_all_eight_statuses_validate_and_stay_selected() -> Result<(), String> {
        // The identity-complete manifest pins every optional identity, so the
        // receipt's restated identities bind cleanly. The builder attaches
        // `repeat` only to the `complete` row (the producer shape), so the
        // seven other rows disclose the absent comparison as typed incomplete,
        // and the under-evidenced summary stability aggregate is disclosed at
        // the receipt level — nothing else is incomplete.
        let complete = identity_complete_manifest();
        let (manifest, sha) = accepted_manifest(&complete)?;
        let receipt = current_receipt_0_3(&manifest);
        let check = validate_receipt_value(&receipt, &manifest, &sha)?;
        assert_eq!(check.schema_version, RECEIPT_SCHEMA_0_3);
        // Every row stays selected; only five attempted analysis.
        assert_eq!(check.denominator_selected, 8);
        assert_eq!(check.denominator_run, 5);
        assert_eq!(
            check
                .incomplete
                .iter()
                .filter(|diagnostic| diagnostic.field != "repeat"
                    && diagnostic.field != "summary.gap_id_stable_count")
                .count(),
            0,
            "only the absent repeat comparisons and the under-evidenced stability aggregate are disclosed: {:?}",
            check.incomplete
        );
        assert_eq!(
            check.incomplete.len(),
            8,
            "one repeat disclosure per non-complete row plus the summary disclosure: {:?}",
            check.incomplete
        );
        Ok(())
    }

    #[test]
    fn current_receipt_rejects_unknown_status_and_states() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        for (field, value, needle) in [
            ("status", json!("mostly_worked"), "unknown run status"),
            (
                "materialization",
                json!("teleported"),
                "unknown materialization state",
            ),
            ("detection", json!("guessed"), "unknown detection state"),
            ("execution", json!("vibes"), "unknown execution state"),
        ] {
            let mut receipt = current_receipt_0_3(&manifest);
            if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
                && let Some(entry) = rows[0].as_object_mut()
            {
                entry.insert(field.to_string(), value);
            }
            expect_fail(
                validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
                needle,
            )?;
        }
        Ok(())
    }

    #[test]
    fn current_receipt_rejects_contradictory_status_pairs() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        for (field, value) in [
            ("execution", json!("timed-out")),
            ("materialization", json!("absent")),
            ("detection", json!("failed")),
        ] {
            let mut receipt = current_receipt_0_3(&manifest);
            if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
                && let Some(entry) = rows[0].as_object_mut()
            {
                entry.insert(field.to_string(), value);
            }
            expect_fail(
                validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
                "contradictory status",
            )?;
        }
        Ok(())
    }

    #[test]
    fn current_receipt_rejects_nested_stability_contradictions_in_both_directions()
    -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        // Direction 1: claims stable while listing unstable IDs in `repeat`.
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
            && let Some(Value::Object(repeat)) = entry.get_mut("repeat")
        {
            repeat.insert("unstable_gap_ids".to_string(), json!(["gap:python:x"]));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "claims stable gap IDs while listing unstable ones",
        )?;

        // Direction 2: claims unstable while listing no unstable IDs.
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
            && let Some(Value::Object(repeat)) = entry.get_mut("repeat")
        {
            repeat.insert("gap_ids_stable".to_string(), json!(false));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "claims unstable gap IDs but lists none",
        )
    }

    /// Flips a builder receipt into eight fully-observed complete runs with
    /// `repeat` evidence on every row and a re-derived honest summary (8
    /// complete runs, no crashes/parse failures/timeouts, runtimes 100..800 —
    /// min 100, median 500, max 800, total 3600 — 8/8 stable,
    /// weakly_exposed=8, direct=8, `pass` gate). This is the one shape under
    /// which the stability aggregates are derivable: every run row carries a
    /// compared pass (#3733 review).
    fn flip_to_fully_complete_0_3(receipt: &mut Value) {
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut) {
            for row in rows.iter_mut() {
                if let Some(entry) = row.as_object_mut() {
                    entry.insert("status".to_string(), json!("complete"));
                    entry.insert("execution".to_string(), json!("executed"));
                    entry.insert("materialization".to_string(), json!("materialized"));
                    entry.insert("detection".to_string(), json!("detected"));
                    if let Some(Value::Object(corpus)) = entry.get_mut("corpus_selection") {
                        corpus.insert("state".to_string(), json!("selected"));
                    }
                    entry.insert(
                        "classification_counts".to_string(),
                        json!({"exposed": 0, "weakly_exposed": 1, "reachable_unrevealed": 0, "no_static_path": 0, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0}),
                    );
                    entry.insert(
                        "alignment_counts".to_string(),
                        json!({"direct": 1, "alias": 0, "changed_sink_token": 0, "orthogonal": 0, "unknown": 0, "absent": 0}),
                    );
                    entry.insert(
                        "repeat".to_string(),
                        json!({
                            "comparable_with": "pass-1",
                            "gap_ids_stable": true,
                            "unstable_gap_ids": [],
                        }),
                    );
                }
            }
        }
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.insert("repos_run".to_string(), json!(8));
            summary.insert("repos_clone_failed".to_string(), json!(0));
            summary.insert("crash_count".to_string(), json!(0));
            summary.insert("crash_rate".to_string(), json!(0.0));
            summary.insert("parse_failure_count".to_string(), json!(0));
            summary.insert("parse_failure_rate".to_string(), json!(0.0));
            summary.insert("timed_out_count".to_string(), json!(0));
            summary.insert("runtime_ms_min".to_string(), json!(100));
            summary.insert("runtime_ms_median".to_string(), json!(500));
            summary.insert("runtime_ms_max".to_string(), json!(800));
            summary.insert("runtime_ms_total".to_string(), json!(3600));
            summary.insert("gap_id_stable_count".to_string(), json!(8));
            summary.insert("gap_id_unstable_count".to_string(), json!(0));
            summary.insert("gap_id_stability_rate".to_string(), json!(1.0));
            summary.insert(
                "classification_counts".to_string(),
                json!({"exposed": 0, "weakly_exposed": 8, "reachable_unrevealed": 0, "no_static_path": 0, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0}),
            );
            summary.insert(
                "alignment_counts".to_string(),
                json!({"direct": 8, "alias": 0, "changed_sink_token": 0, "orthogonal": 0, "unknown": 0, "absent": 0}),
            );
            summary.insert("gate_status".to_string(), json!("pass"));
            summary.insert(
                "gate_reason".to_string(),
                json!("8 complete runs; no crashes; repeat evidence stable on every row"),
            );
        }
    }

    #[test]
    fn current_receipt_with_full_repeat_evidence_reaches_pass() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        let mut receipt = current_receipt_0_3(&manifest);
        // Every row becomes a fully-observed complete run whose stability
        // evidence lives in `repeat`; the rows now derive the `pass` gate.
        flip_to_fully_complete_0_3(&mut receipt);
        let check = validate_receipt_value(&receipt, &manifest, &sha)?;
        assert_eq!(check.denominator_selected, 8);
        assert_eq!(check.denominator_run, 8);
        Ok(())
    }

    #[test]
    fn current_receipt_rejects_malformed_digests_and_paths() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert("input_digest".to_string(), json!("not-a-digest"));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "bare lowercase sha256 hex",
        )?;

        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert("selected_root".to_string(), json!("/var/lib/ripr"));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "repo-relative, not absolute",
        )
    }

    #[test]
    fn current_receipt_requires_manifest_binding() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(object) = receipt.as_object_mut() {
            object.remove("manifest_digest");
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "schema-0.3 receipts are bound to the accepted manifest by digest",
        )
    }

    #[test]
    fn current_receipt_discloses_missing_identity_fields_as_incomplete() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            for field in [
                "binary",
                "config",
                "digests",
                "repeat",
                "corpus_selection",
                "materialization",
                "detection",
                "execution",
                "input_digest",
                "selected_root",
            ] {
                entry.remove(field);
            }
        }
        // With the row's `repeat` evidence gone, the stability aggregates are
        // no longer derivable — the emitter could not have written them, so
        // they are omitted here and the absence is disclosed below.
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.remove("gap_id_stable_count");
            summary.remove("gap_id_unstable_count");
            summary.remove("gap_id_stability_rate");
        }
        let check = validate_receipt_value(&receipt, &manifest, &sha)?;
        let fields: BTreeSet<String> = check
            .incomplete
            .iter()
            .filter(|diagnostic| diagnostic.subject == "alpha")
            .map(|diagnostic| diagnostic.field.clone())
            .collect();
        for field in [
            "binary",
            "config",
            "digests",
            "repeat",
            "corpus_selection",
            "materialization",
            "detection",
            "execution",
            "input_digest",
            "selected_root",
        ] {
            assert!(
                fields.contains(field),
                "missing `{field}` must be typed incomplete, got: {fields:?}"
            );
        }
        Ok(())
    }

    // -- #3733 review fixes ---------------------------------------------------

    #[test]
    fn analyzed_receipt_missing_summary_aggregate_fails() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        // An analyzed receipt must carry the emitted summary in full: the
        // emitter records every aggregate, and a deleted field would silently
        // disable its row-agreement check (#3733 review).
        for field in ["runtime_ms_total", "classification_counts", "crash_rate"] {
            let mut receipt = current_receipt_0_3(&manifest);
            if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
                summary.remove(field);
            }
            expect_fail(
                validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
                &format!("summary.{field}"),
            )?;
        }
        Ok(())
    }

    #[test]
    fn fully_evidenced_receipt_requires_stability_aggregates() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        // Every run row carries `repeat` evidence (the fully-complete shape is
        // the one shape where stability is derivable), so the emitter writes
        // the stability aggregates; deleting one must not silently disable the
        // stability comparison.
        let mut receipt = current_receipt_0_3(&manifest);
        flip_to_fully_complete_0_3(&mut receipt);
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.remove("gap_id_stable_count");
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "summary.gap_id_stable_count",
        )
    }

    #[test]
    fn manifest_rejects_null_empty_and_malformed_identities() -> Result<(), String> {
        // Explicit null: a present-but-null identity is not an absent one.
        let mut value = identity_complete_manifest();
        value["repos"][0]["provenance"] = Value::Null;
        expect_fail(validate_manifest_value(&parsed(&value)?), "explicitly null")?;

        // Malformed digest: present-but-garbage fails instead of completing.
        let mut value = identity_complete_manifest();
        value["repos"][0]["tree_digest"] = json!("nothex");
        expect_fail(validate_manifest_value(&parsed(&value)?), "sha256 hex")?;

        // Empty string.
        let mut value = identity_complete_manifest();
        value["repos"][0]["snapshot"] = json!("");
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "must be non-empty",
        )?;

        // License stays required: null and empty fail outright, with the
        // subject and field named.
        let mut value = alternate_manifest();
        value["repos"][0]["license"] = Value::Null;
        expect_fail(validate_manifest_value(&parsed(&value)?), "field=`license`")?;

        let mut value = alternate_manifest();
        value["repos"][2]["license"] = json!("");
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "subject=`charlie`",
        )
    }

    #[test]
    fn receipt_identity_must_match_the_manifest_binding() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        // License mismatch: both sides record the identity, so a difference
        // fails naming both sides.
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert("license".to_string(), json!("GPL-3.0-only"));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "does not match the accepted manifest license",
        )?;

        // The same for an optional identity the manifest pins.
        let (complete_manifest, complete_sha) = accepted_manifest(&identity_complete_manifest())?;
        let mut receipt = current_receipt_0_3(&complete_manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert("retention_class".to_string(), json!("discarded"));
        }
        expect_fail(
            validate_receipt_value(&receipt, &complete_manifest, &complete_sha).map(|_| ()),
            "does not match the accepted manifest retention_class",
        )
    }

    #[test]
    fn receipt_identity_without_manifest_side_discloses_incomplete() -> Result<(), String> {
        // Manifest without tree_digest; the receipt carries one. The value
        // cannot be bound, so the manifest side discloses incomplete — the
        // outcome is neither valid nor a failure (#3733 review).
        let mut gappy = identity_complete_manifest();
        if let Some(repos) = gappy.get_mut("repos").and_then(Value::as_array_mut) {
            for repo in repos.iter_mut() {
                if let Some(entry) = repo.as_object_mut() {
                    entry.remove("tree_digest");
                }
            }
        }
        let (manifest, sha) = accepted_manifest(&gappy)?;
        let receipt_value = current_receipt_0_3(&manifest);
        let check = validate_receipt_value(&receipt_value, &manifest, &sha)?;
        assert!(
            check.incomplete.iter().any(|diagnostic| {
                diagnostic.subject == "alpha" && diagnostic.field == "manifest.tree_digest"
            }),
            "an unbindable receipt tree_digest must disclose the manifest side: {:?}",
            check.incomplete
        );
        assert_eq!(check.verdict(), Verdict::Incomplete);
        Ok(())
    }

    #[test]
    fn absent_features_disclose_incomplete_and_malformed_features_fail() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        // Receipt-level: absent ripr.features discloses incomplete.
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(ripr) = receipt.get_mut("ripr").and_then(Value::as_object_mut) {
            ripr.remove("features");
        }
        let check = validate_receipt_value(&receipt, &manifest, &sha)?;
        assert!(
            check
                .incomplete
                .iter()
                .any(|diagnostic| diagnostic.field == "ripr.features"),
            "absent ripr.features must disclose incomplete: {:?}",
            check.incomplete
        );

        // Receipt-level: present-but-malformed fails.
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(ripr) = receipt.get_mut("ripr").and_then(Value::as_object_mut) {
            ripr.insert("features".to_string(), json!("python"));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "array of strings",
        )?;

        // Row-level: absent binary.features discloses incomplete.
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
            && let Some(Value::Object(binary)) = entry.get_mut("binary")
        {
            binary.remove("features");
        }
        let check = validate_receipt_value(&receipt, &manifest, &sha)?;
        assert!(
            check
                .incomplete
                .iter()
                .any(|diagnostic| diagnostic.subject == "alpha"
                    && diagnostic.field == "binary.features"),
            "absent binary.features must disclose incomplete: {:?}",
            check.incomplete
        );

        // Row-level: present-but-malformed fails.
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
            && let Some(Value::Object(binary)) = entry.get_mut("binary")
        {
            binary.insert("features".to_string(), json!(42));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "array of strings",
        )
    }

    #[test]
    fn present_null_receipt_identities_fail_while_absent_discloses_incomplete() -> Result<(), String>
    {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        // Row-level binding identities: an explicit null is a present-but-
        // garbage identity, not an absent one — it fails naming the field
        // (the receipt-side twin of the manifest's null-identity rule).
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert("provenance".to_string(), Value::Null);
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "field=`provenance`",
        )?;

        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert("tree_digest".to_string(), Value::Null);
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "field=`tree_digest`",
        )?;

        // Receipt level: a null field in the `ripr` identity block fails.
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(ripr) = receipt.get_mut("ripr").and_then(Value::as_object_mut) {
            ripr.insert("source_sha".to_string(), Value::Null);
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "field=`ripr.source_sha`",
        )?;

        // The copy-check blocks fail the same way: a null inside a present
        // `repository` / `binary` block is garbage, not absence.
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
            && let Some(Value::Object(repository)) = entry.get_mut("repository")
        {
            repository.insert("url".to_string(), Value::Null);
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "field=`repository.url`",
        )?;

        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
            && let Some(Value::Object(binary)) = entry.get_mut("binary")
        {
            binary.insert("digest".to_string(), Value::Null);
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "field=`binary.digest`",
        )?;

        // Key ABSENT keeps the typed-incomplete disclosure, never a failure.
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.remove("provenance");
        }
        let check = validate_receipt_value(&receipt, &manifest, &sha)?;
        assert!(
            check.incomplete.iter().any(|diagnostic| {
                diagnostic.subject == "alpha" && diagnostic.field == "provenance"
            }),
            "an absent provenance key must disclose incomplete: {:?}",
            check.incomplete
        );
        Ok(())
    }

    #[test]
    fn run_status_split_pins_which_statuses_count_as_run() -> Result<(), String> {
        // The 0.3 denominator split is exact (SPEC-0086): the five
        // analysis-attempting statuses count toward `repos_run`, and
        // `unsupported`/`tempfail`/`stale` do not — while every status stays
        // selected either way.
        let subject = AcceptedSubject {
            id: "alpha".to_string(),
            url: "https://example.com/alpha".to_string(),
            sha: VALID_SHA_A.to_string(),
            license: "MIT".to_string(),
            shape: "pytest_library".to_string(),
            synthetic_diff: "fixtures/python-eval-sweep/diffs/alpha.diff".to_string(),
            tree_digest: None,
            snapshot: None,
            provenance: None,
            retention_class: None,
        };
        assert_eq!(STATUS_VOCABULARY.len(), 8);
        for status in STATUS_VOCABULARY {
            let counts_as_run = RUN_STATUSES.contains(&status);
            let mut row = json!({
                "id": subject.id,
                "status": status,
            });
            if counts_as_run {
                row["runtime_ms"] = json!(100);
                row["classification_counts"] = json!({
                    "exposed": 0, "weakly_exposed": 0, "reachable_unrevealed": 0,
                    "no_static_path": 0, "infection_unknown": 0,
                    "propagation_unknown": 0, "static_unknown": 0,
                });
                row["alignment_counts"] = json!({
                    "direct": 0, "alias": 0, "changed_sink_token": 0,
                    "orthogonal": 0, "unknown": 0, "absent": 0,
                });
            }
            let mut incomplete = Vec::new();
            let summary = validate_row(&row, &subject, true, None, &mut incomplete)?;
            assert_eq!(
                summary.counts_as_run,
                counts_as_run,
                "status `{status}` must {} toward repos_run",
                if counts_as_run { "count" } else { "not count" },
            );
        }
        Ok(())
    }

    // -- focused fix round: execution/stability contradictions, identity
    //    copies, binary disclosure, owned-field type checks -------------------

    #[test]
    fn partial_row_with_not_executed_state_fails() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        // `partial` is a run-status row (it counts toward `repos_run`), so an
        // execution state of `not-executed` contradicts it.
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[1].as_object_mut()
        {
            entry.insert("execution".to_string(), json!("not-executed"));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "status `partial` cannot coexist with execution state `not-executed`",
        )
    }

    #[test]
    fn disagreeing_tree_digest_copies_fail_and_matching_copies_pass() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        // The `repository` block copy disagrees with the row-level digest:
        // both locations record one identity, so the mismatch fails naming
        // both.
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
            && let Some(Value::Object(repository)) = entry.get_mut("repository")
        {
            repository.insert("tree_digest".to_string(), json!(DIGEST_TWO));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "`tree_digest` does not match `repository.tree_digest`",
        )?;

        // Matching copies validate (the fixture restates the same digest).
        let receipt = current_receipt_0_3(&manifest);
        validate_receipt_value(&receipt, &manifest, &sha)?;
        Ok(())
    }

    #[test]
    fn row_binary_identity_must_match_the_receipt_ripr_block() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        // The row `binary` block restates the receipt-level `ripr` identity;
        // a disagreeing copy of any owned field fails naming both locations.
        for (field, wrong) in [
            ("digest", json!(DIGEST_ONE)),
            ("version", json!("ripr 9.9.9")),
            ("features", json!(["rust"])),
            ("build_profile", json!("release")),
        ] {
            let mut receipt = current_receipt_0_3(&manifest);
            if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
                && let Some(entry) = rows[0].as_object_mut()
                && let Some(Value::Object(binary)) = entry.get_mut("binary")
            {
                binary.insert(field.to_string(), wrong);
            }
            expect_fail(
                validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
                &format!("`binary.{field}` does not match"),
            )?;
        }
        Ok(())
    }

    #[test]
    fn false_stability_claim_requires_the_unstable_list() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        // 0.3: a false claim inside `repeat` with the list omitted fails
        // naming the omitted field.
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
            && let Some(Value::Object(repeat)) = entry.get_mut("repeat")
        {
            repeat.insert("gap_ids_stable".to_string(), json!(false));
            repeat.remove("unstable_gap_ids");
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "field=`repeat.unstable_gap_ids`",
        )?;

        // 0.2: the same omission at the row level fails the same way.
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert("gap_ids_stable".to_string(), json!(false));
            entry.remove("unstable_gap_ids");
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "field=`unstable_gap_ids`",
        )
    }

    #[test]
    fn present_binary_block_without_build_profile_discloses_incomplete() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        // A present `binary` block missing `build_profile` is a named
        // incomplete disclosure, not a silent pass.
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
            && let Some(Value::Object(binary)) = entry.get_mut("binary")
        {
            binary.remove("build_profile");
        }
        let check = validate_receipt_value(&receipt, &manifest, &sha)?;
        assert!(
            check.incomplete.iter().any(|diagnostic| {
                diagnostic.subject == "alpha" && diagnostic.field == "binary.build_profile"
            }),
            "missing binary.build_profile must be disclosed incomplete: {:?}",
            check.incomplete
        );
        assert_eq!(check.verdict(), Verdict::Incomplete);

        // A malformed profile value still fails outright.
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
            && let Some(Value::Object(binary)) = entry.get_mut("binary")
        {
            binary.insert("build_profile".to_string(), json!("ultra"));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "unknown build profile",
        )
    }

    #[test]
    fn owned_fields_get_type_checks_per_emitted_shape() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        // Summary: wrong-typed `gate_reason` (number) fails naming the field.
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.insert("gate_reason".to_string(), json!(7));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "field=`gate_reason`",
        )?;

        // 0.2 row: wrong-typed `gap_ids` (object) fails.
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert("gap_ids".to_string(), json!({"gap:python:x": 1}));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "field=`gap_ids`",
        )?;

        // 0.2 row: wrong-typed `stderr_excerpt` (array) fails.
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert("stderr_excerpt".to_string(), json!(["boom"]));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "field=`stderr_excerpt`",
        )?;

        // Manifest: wrong-typed `why` (array) fails naming the subject.
        let mut value = alternate_manifest();
        value["repos"][0]["why"] = json!(["because"]);
        expect_fail(validate_manifest_value(&parsed(&value)?), "subject=`alpha`")?;

        // Manifest: wrong-typed `limits` (object) fails.
        let mut value = alternate_manifest();
        value["limits"] = json!({"static": true});
        expect_fail(validate_manifest_value(&parsed(&value)?), "field=`limits`")?;

        // Manifest: wrong-typed `description` (number) fails.
        let mut value = alternate_manifest();
        value["description"] = json!(42);
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "field=`description`",
        )
    }

    // -- hardening round: malformed blocks fail, host rules, zero-run keys ---

    #[test]
    fn malformed_repository_block_fails_while_absence_discloses_incomplete() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        // A wrong-typed `repository` block is malformed, not absent: it fails
        // naming the field instead of disclosing an incomplete identity
        // (#3733 review).
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert("repository".to_string(), json!("x"));
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "repository identity must be an object when present",
        )?;

        // Only ABSENCE discloses: removing the block stays a typed-incomplete
        // disclosure, not a failure.
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.remove("repository");
        }
        let check = validate_receipt_value(&receipt, &manifest, &sha)?;
        assert!(
            check.incomplete.iter().any(|diagnostic| {
                diagnostic.subject == "alpha" && diagnostic.field == "repository"
            }),
            "an absent repository block must disclose incomplete: {:?}",
            check.incomplete
        );
        assert_eq!(check.verdict(), Verdict::Incomplete);
        Ok(())
    }

    #[test]
    fn malformed_synthetic_diff_fails_even_with_a_valid_fallback() -> Result<(), String> {
        // A per-subject wrong type cannot hide behind a valid top-level
        // fallback: a fallback repairs absence, never malformedness
        // (#3733 review).
        let mut value = alternate_manifest();
        value["synthetic_diff"] = json!("fixtures/python-eval-sweep/synthetic-diff.diff");
        value["repos"][0]["synthetic_diff"] = json!(42);
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "field=`synthetic_diff`",
        )?;

        // The same for a present non-portable path at the subject level.
        let mut value = alternate_manifest();
        value["synthetic_diff"] = json!("fixtures/python-eval-sweep/synthetic-diff.diff");
        value["repos"][0]["synthetic_diff"] = json!("../escape.diff");
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "must not contain `..`",
        )?;

        // And a malformed top-level fallback fails at its own level even
        // when every subject carries a valid path.
        let mut value = alternate_manifest();
        value["synthetic_diff"] = json!(["fixtures/python-eval-sweep/synthetic-diff.diff"]);
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "field=`synthetic_diff`",
        )
    }

    #[test]
    fn rejects_hostless_whitespace_and_dotless_https_urls() -> Result<(), String> {
        for (url, needle) in [
            ("https:///path", "has no host"),
            ("https://ex ample.com/x", "must not contain whitespace"),
            // A dotless host is malformed under the same conservative host
            // rule: `https://host` carries no repository host shape, so it is
            // rejected by design, not overlooked.
            ("https://host", "no dotted host"),
        ] {
            let mut value = alternate_manifest();
            value["repos"][0]["url"] = json!(url);
            expect_fail(validate_manifest_value(&parsed(&value)?), needle)?;
        }

        // Existing valid URLs keep passing.
        let mut value = alternate_manifest();
        value["repos"][0]["url"] = json!("https://example.com/alpha/nested/path");
        validate_manifest_value(&parsed(&value)?)?;
        Ok(())
    }

    #[test]
    fn zero_run_summary_distribution_requires_the_emitted_key_set() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

        // A zero-run alignment_counts without `absent`: every recorded bucket
        // is zero, but the distribution drops a required emitted bucket, so
        // it fails (#3733 review).
        let mut receipt = zero_run_receipt_0_2(&alternate_manifest());
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut)
            && let Some(Value::Object(counts)) = summary.get_mut("alignment_counts")
        {
            counts.remove("absent");
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "omits required bucket `absent`",
        )?;

        // The same law for `unknown` and for a classification bucket: the
        // emitter zero-fills every bucket (eval_sweep.rs `to_json`), so a
        // recorded distribution must carry the full emitted key set.
        for (field, bucket) in [
            ("alignment_counts", "unknown"),
            ("classification_counts", "static_unknown"),
        ] {
            let mut receipt = zero_run_receipt_0_2(&alternate_manifest());
            if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut)
                && let Some(Value::Object(counts)) = summary.get_mut(field)
            {
                counts.remove(bucket);
            }
            expect_fail(
                validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
                &format!("omits required bucket `{bucket}`"),
            )?;
        }

        // The full zero-filled emitted key set validates.
        let receipt = zero_run_receipt_0_2(&alternate_manifest());
        validate_receipt_value(&receipt, &manifest, &sha)?;
        Ok(())
    }

    // -- verdict + rendering -------------------------------------------------

    #[test]
    fn not_run_is_never_a_pass_in_verdict_vocabulary() -> Result<(), String> {
        let (manifest, _) = accepted_manifest(&alternate_manifest())?;
        let outcome = CheckOutcome {
            manifest_path: "manifest.json".to_string(),
            accepted: manifest,
            receipt: None,
        };
        assert_eq!(outcome.verdict(), Verdict::NotRun);
        assert_eq!(outcome.verdict().as_str(), "not_run");
        Ok(())
    }

    #[test]
    fn top_level_verdict_is_incomplete_when_manifest_gaps_survive_a_complete_receipt()
    -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        // The retained manifest discloses 32 incomplete identities (four per
        // subject); the 0.3 receipt is otherwise complete on its own — but the
        // identities it restates have no manifest side to bind against, so
        // each one discloses the unbindable manifest gap (#3733 review)
        // instead of fabricating a binding.
        assert_eq!(manifest.incomplete.len(), 8 * 4);
        let receipt_value = current_receipt_0_3(&manifest);
        let receipt = validate_receipt_value(&receipt_value, &manifest, &sha)?;
        assert!(
            receipt
                .incomplete
                .iter()
                .any(|diagnostic| diagnostic.field == "manifest.tree_digest"),
            "a receipt identity with no manifest side must disclose the gap: {:?}",
            receipt.incomplete
        );
        let outcome = CheckOutcome {
            manifest_path: "manifest.json".to_string(),
            accepted: manifest,
            receipt: Some(receipt),
        };
        // A complete receipt must not hide manifest gaps: the top-level
        // verdict is exactly `incomplete`.
        assert_eq!(outcome.verdict(), Verdict::Incomplete);
        assert_eq!(outcome.verdict().as_str(), "incomplete");
        Ok(())
    }

    #[test]
    fn top_level_verdict_is_valid_only_when_both_artifacts_carry_zero_incompletes()
    -> Result<(), String> {
        let complete = identity_complete_manifest();
        let (manifest, sha) = accepted_manifest(&complete)?;
        assert!(manifest.incomplete.is_empty());
        // The fully-complete shape (every row complete with compared repeat
        // evidence) is the one receipt shape with zero incompletes: rows
        // without a compared pass disclose the absent comparison, so a
        // builder receipt over even a pinned manifest is `incomplete`.
        let mut receipt_value = current_receipt_0_3(&manifest);
        flip_to_fully_complete_0_3(&mut receipt_value);
        let receipt = validate_receipt_value(&receipt_value, &manifest, &sha)?;
        assert!(receipt.incomplete.is_empty());
        let outcome = CheckOutcome {
            manifest_path: "manifest.json".to_string(),
            accepted: manifest,
            receipt: Some(receipt),
        };
        assert_eq!(outcome.verdict(), Verdict::Valid);
        assert_eq!(outcome.verdict().as_str(), "valid");
        Ok(())
    }

    #[test]
    fn check_report_json_is_stable_and_versioned() -> Result<(), String> {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        let receipt_value = current_receipt_0_3(&manifest);
        let receipt = validate_receipt_value(&receipt_value, &manifest, &sha)?;
        let outcome = CheckOutcome {
            manifest_path: "manifest.json".to_string(),
            accepted: manifest,
            receipt: Some(receipt),
        };
        let rendered_a = render_check_json(&outcome);
        let rendered_b = render_check_json(&outcome);
        let text_a = rendered_a?;
        assert_eq!(
            Ok(text_a.clone()),
            rendered_b,
            "check JSON must be deterministic"
        );
        assert!(text_a.contains("\"kind\": \"python_eval_sweep_check_report\""));
        // The receipt validates to `incomplete` or `valid`, never a bare pass.
        assert!(
            text_a.contains("\"verdict\": \"valid\"")
                || text_a.contains("\"verdict\": \"incomplete\"")
        );
        Ok(())
    }

    #[test]
    fn diagnostics_name_subject_field_reason_and_rerun() -> Result<(), String> {
        let mut value = alternate_manifest();
        value["repos"][2]["license"] = json!("");
        let error = match validate_manifest_value(&parsed(&value)?) {
            Ok(()) => return Err("empty license must fail the manifest".to_string()),
            Err(error) => error,
        };
        for needle in ["subject=`charlie`", "field=`license`", RERUN_COMMAND] {
            assert!(
                error.contains(needle),
                "diagnostic `{error}` must mention `{needle}`"
            );
        }
        Ok(())
    }
}
