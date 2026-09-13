//! Typed selection/attempt model and semantic validator for governed Python
//! repair attempts — `cargo xtask python-repair-trust check` (RIPR-SPEC-0176,
//! issue #3568).
//!
//! One crate-private loader owns selection, lifecycle, movement, execution,
//! currentness, and aggregate semantics for the external Python repair-trust
//! corpus:
//!
//! - the accepted **selection manifest** (`python_repair_trust_manifest`)
//!   fixes the denominator before outcomes are known. Every selection row
//!   carries the attempt/case/subject identity, the repository/base/head/tree
//!   pins with source currentness, the selection reason and diversity
//!   stratum, the native Python behavior identity (family/owner/
//!   discriminator/relation/oracle/limitation), the expected direction and
//!   claim boundary, the target state
//!   (existing/proposed/ambiguous/unavailable/unsafe) with its path, the
//!   selection timestamp/selector/authority-manifest digest, and an immutable
//!   `selection_digest` — recomputed over the row's canonical content, so a
//!   replaced or edited selected row fails closed. Serialization contract:
//!   the preimage is the selection row's compact UTF-8 JSON with sorted
//!   object keys and serde_json's default string escaping, with ONLY the
//!   `selection_digest` field removed (`authority_snapshot_digest` stays in
//!   the preimage); producers must hash exactly these bytes, not
//!   pretty-printed or differently escaped JSON.
//! - retained **attempt envelopes** (`python_repair_trust_attempts`) bind to
//!   the selection manifest by sha256 digest and carry the attempt lifecycle
//!   rows. Lifecycle states are
//!   `selected`/`eligible`/`started`/`edited`/`verified`/`reviewed`/
//!   `accepted`/`stale`/`rejected`/`abandoned`; terminal static movement is
//!   `closed`/`improved`/`unchanged`/`regressed`/`limited`/`stale`/
//!   `uncertain`; verification execution is a separate axis:
//!   `passed`/`failed`/`timed_out`/`cancelled`/`unavailable`/`not_run`/
//!   `invalid`.
//!
//! Two digest names carry the immutability bindings, each defined once:
//! `manifest_digest` is sha256 over the EXACT manifest file bytes — no
//! canonicalization, no reserialization. That one definition covers both
//! the envelope-level binding (each envelope's `manifest_digest` must equal
//! the checker's recomputation over the presented selection-manifest bytes,
//! so a changed manifest fails the stale-digest check) and each selection
//! row's recorded `authority_snapshot_digest` — the same digest over the exact
//! authority-snapshot bytes the row was selected from, recorded at
//! selection time and digested as recorded, never re-read offline. It is
//! distinct from the row's `selection_digest`: sha256 of the row's
//! canonical content, defined as the JSON serialization the checker
//! recomputes (the row without its digest field, re-serialized with sorted
//! object keys).
//!
//! Design laws (issue #3568 acceptance):
//!
//! - Selection precedes edit/outcome: every lifecycle starts at `selected`,
//!   and the state machine ordering is enforced (edited needs started,
//!   verified needs edited, reviewed needs verified, accepted needs
//!   reviewed; discard terminals come last).
//! - Selected rows cannot be deleted or replaced after outcome: an attempt
//!   naming an unknown attempt identity fails, a replaced selection row fails
//!   its recomputed digest, and a manifest changed under a bound envelope
//!   fails the stale-digest check.
//! - Native Python behavior identity remains authoritative: `SeamKind` (the
//!   Rust-side conversion shim vocabulary) appears nowhere in a corpus value.
//! - Source/analyzer/config/input/packet/target/command/patch/after_state
//!   identities are required as lifecycle advances: claiming `started`
//!   requires the analyzer/config/input identities, `edited` the patch
//!   digest, `verified` the command and after_state identities, and
//!   `reviewed`/`accepted` the packet reference. Missing identities at their
//!   transition fail closed — they are not invented and not excused. The
//!   source and target identities live on the selection row.
//! - Static movement and command execution cannot imply one another: the
//!   axes are separate fields with no cross-axis derivation. A recorded
//!   movement requires an edited lifecycle (movement is a static
//!   before/after comparison of an edit — never a consequence of a run), a
//!   verdict execution (`passed`/`failed`) requires the `verified` state,
//!   and a non-verdict execution contradicts a claimed `verified` state. An
//!   improved movement never requires a passed run, and a passed run never
//!   forces improved — both pairings stay representable.
//! - Partial/stale/abandoned attempts cannot appear completed: a
//!   `stale`/`rejected`/`abandoned` terminal forbids the completed lifecycle
//!   states (`verified`/`reviewed`/`accepted`) and the completed movements
//!   (`closed`/`improved`).
//! - Production/generated/vendor/environment edit surfaces are forbidden: a
//!   target path under a denied surface prefix must be declared
//!   `target_state: unsafe`, and an attempt that edits an unsafe target
//!   fails. Existing/proposed/ambiguous/unavailable/unsafe targets stay
//!   representable — ambiguity, unavailability, and unsafety are states, not
//!   errors, until someone attempts the edit.
//! - Aggregates are derived from rows: every recorded total and count map
//!   must equal the row-derived value exactly (hand-edited totals fail);
//!   with zero attempt rows a recorded nonzero aggregate is a fabricated
//!   claim. Achieved diversity is reported from rows, and a recorded
//!   `stratum_floor_met` must equal the floor comparison — a floor that is
//!   not met is never reported as met.
//! - Historical attempts are immutable: attempt identities are unique
//!   across the corpus, and a refresh appends a new record under a new
//!   attempt identity linked through `supersedes` to a record that ended
//!   `stale` — never by mutating the historical row.
//! - No corpus yet: `check` with no attempts input exits 0 with verdict
//!   `not_run` and empty-denominator semantics — never a vacuous pass. The
//!   verdict vocabulary is `valid`/`incomplete`/`not_run`, and none of the
//!   three is a support-tier, repair-correctness, gate, badge, or promotion
//!   claim.
//! - Ordinary CI validates accepted metadata offline: no repository
//!   materialization, no RIPR execution, no external commands, and no
//!   filesystem lookups beyond the supplied artifact files themselves (an
//!   external authority's bytes are digested as recorded, never re-read).
//!
//! Parser discipline follows the repo validator canon (`eval_sweep_check`):
//! strict JSON with duplicate-key rejection, deny-unknown-fields schemas,
//! present-null identity fields failing as garbage, checked arithmetic
//! everywhere, and fail-closed diagnostics naming subject/field/reason plus
//! the deterministic rerun command.
//!
//! Exit contract: `check` exits 0 when every present artifact is structurally
//! valid, disclosing absent optional selection identities (tree digest,
//! source currentness, limitation record) as `incomplete` and a missing
//! attempts dimension as `not_run`; it exits nonzero on any fail-closed
//! violation.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Value, json};

use crate::python_judged_panel::parse_json_without_duplicate_keys;
use crate::python_judged_panel_replay::sha256_hex;

pub(crate) const DEFAULT_MANIFEST: &str = "fixtures/python-repair-trust/manifest.json";
const RERUN_COMMAND: &str = "cargo xtask python-repair-trust check";
const CHECK_REPORT_JSON: &str = "python-repair-trust-check.json";
const CHECK_REPORT_MD: &str = "python-repair-trust-check.md";

pub(crate) const MANIFEST_KIND: &str = "python_repair_trust_manifest";
const ATTEMPTS_KIND: &str = "python_repair_trust_attempts";
pub(crate) const SCHEMA_VERSION: &str = "0.1";
pub(crate) const KNOWN_SPEC: &str = "RIPR-SPEC-0176";

/// The closed selection-manifest schema (deny-unknown). Every key the
/// manifest shape owns is listed; unknown keys are schema rot, not forward
/// compatibility.
const MANIFEST_KEYS: [&str; 5] = [
    "schema_version",
    "kind",
    "spec",
    "description",
    "selections",
];

/// The closed selection-row schema: everything issue #3568 requires a
/// selection row to retain. `tree`, `source_currentness`, and `limitation`
/// are optional identities whose absence is typed incomplete; every other
/// field is required.
const SELECTION_KEYS: [&str; 24] = [
    "attempt_id",
    "case_id",
    "subject_id",
    "repository",
    "base",
    "head",
    "tree",
    "source_currentness",
    "selection_reason",
    "diversity_stratum",
    "family",
    "owner",
    "discriminator",
    "relation",
    "oracle",
    "limitation",
    "expected_direction",
    "claim_boundary",
    "target_path",
    "target_state",
    "selected_at",
    "selector",
    "authority_snapshot_digest",
    "selection_digest",
];

/// The closed attempts-envelope schema.
const ATTEMPTS_KEYS: [&str; 6] = [
    "schema_version",
    "kind",
    "spec",
    "manifest_digest",
    "aggregates",
    "attempts",
];

/// The closed aggregates schema.
const AGGREGATES_KEYS: [&str; 8] = [
    "selected_denominator",
    "attempts_total",
    "lifecycle_counts",
    "movement_counts",
    "execution_counts",
    "achieved_strata",
    "stratum_floor",
    "stratum_floor_met",
];

/// The closed attempt-row schema.
const ATTEMPT_KEYS: [&str; 12] = [
    "attempt_id",
    "states",
    "supersedes",
    "movement",
    "execution",
    "analyzer",
    "config",
    "input",
    "patch",
    "command",
    "after_state",
    "packet",
];

/// Identity blocks inside an attempt row (deny-unknown each).
const ANALYZER_KEYS: [&str; 2] = ["source_sha", "binary_digest"];
const CONFIG_KEYS: [&str; 1] = ["profile"];
const INPUT_KEYS: [&str; 1] = ["digest"];
const PATCH_KEYS: [&str; 1] = ["digest"];
const COMMAND_KEYS: [&str; 1] = ["verification_command"];
const AFTER_STATE_KEYS: [&str; 1] = ["tree_digest"];
const PACKET_KEYS: [&str; 1] = ["reference"];

/// The identity blocks an attempt row can carry. A supplied block is
/// shape-validated at every lifecycle position; only its *requiredness* is
/// tied to the lifecycle transition.
const IDENTITY_BLOCKS: [&str; 7] = [
    "analyzer",
    "config",
    "input",
    "patch",
    "command",
    "after_state",
    "packet",
];

/// The per-field value check of one identity block (subject, field, value).
type BlockValueCheck = fn(&str, &str, &str) -> Result<(), String>;

/// The closed field schema and per-field value check of one identity block —
/// the same contract at every lifecycle position.
fn identity_block_schema(block: &str) -> Option<(&'static [&'static str], BlockValueCheck)> {
    match block {
        "analyzer" => Some((&ANALYZER_KEYS, analyzer_identity_value)),
        "config" => Some((&CONFIG_KEYS, require_non_empty_value)),
        "input" => Some((&INPUT_KEYS, check_sha256_digest)),
        "patch" => Some((&PATCH_KEYS, check_sha256_digest)),
        "command" => Some((&COMMAND_KEYS, require_non_empty_value)),
        "after_state" => Some((&AFTER_STATE_KEYS, check_sha256_digest)),
        "packet" => Some((&PACKET_KEYS, require_non_empty_value)),
        _ => None,
    }
}

/// A non-empty string identity value (`profile`, `verification_command`,
/// `reference`).
fn require_non_empty_value(subject: &str, field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(fail(subject, field, "required field must be non-empty"))
    } else {
        Ok(())
    }
}

/// Analyzer identity values: the source pin is a git SHA, every other field
/// a sha256 digest.
fn analyzer_identity_value(subject: &str, field: &str, value: &str) -> Result<(), String> {
    if field.ends_with("source_sha") {
        check_git_sha(subject, field, value)
    } else {
        check_sha256_digest(subject, field, value)
    }
}

/// The complete attempt-lifecycle vocabulary (issue #3568). A state outside
/// this set fails.
const LIFECYCLE_STATES: [&str; 10] = [
    "selected",
    "eligible",
    "started",
    "edited",
    "verified",
    "reviewed",
    "accepted",
    "stale",
    "rejected",
    "abandoned",
];

/// The ordered progression ranks. A lifecycle's progress states must appear
/// in this order; each progress state beyond `started` also requires its
/// predecessor.
const PROGRESS_RANKS: [(&str, u8); 7] = [
    ("selected", 0),
    ("eligible", 1),
    ("started", 2),
    ("edited", 3),
    ("verified", 4),
    ("reviewed", 5),
    ("accepted", 6),
];

/// Discard/terminal states: a lifecycle ends here, and an attempt that ends
/// here cannot appear completed.
const DISCARD_TERMINALS: [&str; 3] = ["stale", "rejected", "abandoned"];

/// Lifecycle states that evidence a completed attempt.
const COMPLETED_STATES: [&str; 3] = ["verified", "reviewed", "accepted"];

/// Terminal static movement vocabulary (issue #3568). Separate axis from the
/// lifecycle and from execution. Shared with the #3570 receipt validator.
pub(crate) const MOVEMENTS: [&str; 7] = [
    "closed",
    "improved",
    "unchanged",
    "regressed",
    "limited",
    "stale",
    "uncertain",
];

/// The completed movements an attempt can only reach through its own static
/// before/after comparison — never while stale, rejected, or abandoned.
const COMPLETED_MOVEMENTS: [&str; 2] = ["closed", "improved"];

/// Verification execution vocabulary (issue #3568). Separate axis from
/// static movement: neither implies the other. Shared with the #3570
/// receipt validator.
pub(crate) const EXECUTIONS: [&str; 7] = [
    "passed",
    "failed",
    "timed_out",
    "cancelled",
    "unavailable",
    "not_run",
    "invalid",
];

/// The executions that record a completed verification verdict — the only
/// ones that can evidence the `verified` lifecycle state. `timed_out`,
/// `cancelled`, `unavailable`, `not_run`, and `invalid` report how the
/// command went without producing a verdict, so they contradict a claimed
/// `verified` state.
const VERDICT_EXECUTIONS: [&str; 2] = ["passed", "failed"];

/// Target state vocabulary (issue #3568).
pub(crate) const TARGET_STATES: [&str; 5] =
    ["existing", "proposed", "ambiguous", "unavailable", "unsafe"];

/// Source-currentness vocabulary, mirrored from the repo's currentness
/// authority (RIPR-SPEC-0151): reuse, never a private vocabulary.
pub(crate) const SOURCE_CURRENTNESS: [&str; 4] = [
    "candidate_current",
    "base_deleted",
    "moved_or_renamed",
    "unresolved_subject",
];

/// Expected-direction vocabulary, mirrored from the judged-panel case
/// authority (RIPR-SPEC-0092): the directions the retained cases admit.
const EXPECTED_DIRECTIONS: [&str; 3] = ["should_gap", "should_stay_quiet", "should_limit"];

/// Denied edit-surface path prefixes (portable, lowercase, forward-slash).
/// A target path under any of these is a production/generated/vendor/
/// environment edit surface: it must be declared `target_state: unsafe`, and
/// attempting an edit on it fails validation.
const DENIED_SURFACE_PREFIXES: [&str; 14] = [
    "target/",
    "dist/",
    "build/",
    "vendor/",
    "vendored/",
    "node_modules/",
    "generated/",
    "__pycache__/",
    "site-packages/",
    ".venv/",
    "venv/",
    "env/",
    ".tox/",
    ".eggs/",
];

/// Secret tripwire substrings for path/URL fields (case-insensitive), shared
/// with the eval-sweep check discipline: a conservative tripwire, not a
/// secret parser — a hit fails the artifact.
const SECRET_TRIPWIRES: [&str; 18] = [
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

/// One non-failing disclosure: an optional identity that is absent (typed
/// `incomplete`, never invented).
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

    fn render(&self) -> String {
        format!(
            "subject=`{}` field=`{}`: {}",
            self.subject, self.field, self.reason
        )
    }
}

/// Fail-closed error text: names subject/field/reason plus the rerun command.
pub(crate) fn fail(subject: &str, field: &str, reason: impl std::fmt::Display) -> String {
    format!(
        "python-repair-trust check failed: subject=`{subject}` field=`{field}`: {reason}\nrerun: {RERUN_COMMAND}"
    )
}

/// Structural verdict. `valid`/`incomplete`/`not_run` exit 0 (with
/// `incomplete` identities disclosed in full); fail-closed violations exit
/// nonzero. None of these is a support-tier, repair-correctness, gate,
/// badge, or promotion claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Valid,
    Incomplete,
    NotRun,
}

impl Verdict {
    fn as_str(self) -> &'static str {
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
    manifest_explicit: bool,
    attempts: Option<String>,
}

fn parse_check_args(args: &[String]) -> Result<CheckArgs, String> {
    let mut parsed = CheckArgs {
        manifest: DEFAULT_MANIFEST.to_string(),
        manifest_explicit: false,
        attempts: None,
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                index += 1;
                parsed.manifest = args.get(index).cloned().ok_or_else(|| {
                    format!(
                        "python-repair-trust check --manifest requires a value\nrerun: {RERUN_COMMAND}"
                    )
                })?;
                parsed.manifest_explicit = true;
            }
            "--attempts" => {
                index += 1;
                parsed.attempts = Some(args.get(index).cloned().ok_or_else(|| {
                    format!(
                        "python-repair-trust check --attempts requires a value\nrerun: {RERUN_COMMAND}"
                    )
                })?);
            }
            other => {
                return Err(format!(
                    "unknown python-repair-trust check argument: {other}\nusage: cargo xtask python-repair-trust check [--manifest <path>] [--attempts <dir-or-file>]\nrerun: {RERUN_COMMAND}"
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
/// rot fails at load). Returns the parsed value and the canonical
/// `manifest_digest`: sha256 over the exact file bytes — no
/// canonicalization, no reserialization — the one manifest-level digest
/// definition; the envelope binding and each selection row's recorded value
/// are instances of it.
pub(crate) fn load_strict_json(display: &str) -> Result<(Value, String), String> {
    let bytes = std::fs::read(display)
        .map_err(|error| fail(display, "file", format!("failed to read: {error}")))?;
    let text = String::from_utf8(bytes)
        .map_err(|error| fail(display, "file", format!("not valid UTF-8: {error}")))?;
    let value = parse_json_without_duplicate_keys(&text)
        .map_err(|error| fail(display, "json", format!("not well-formed JSON: {error}")))?;
    Ok((value, sha256_hex(text.as_bytes())))
}

pub(crate) fn as_object<'a>(
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
pub(crate) fn reject_unknown_keys(
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

/// Required non-empty string. Absent, null, wrong-typed, and blank all fail
/// with the field named — required identities are never invented.
pub(crate) fn require_string(
    subject: &str,
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<String, String> {
    match object.get(key) {
        None => Err(fail(subject, key, "required field is missing")),
        Some(Value::Null) => Err(fail(
            subject,
            key,
            "required field is explicitly null; a present null is not a value",
        )),
        Some(Value::String(text)) => {
            if text.trim().is_empty() {
                Err(fail(subject, key, "required field must be non-empty"))
            } else {
                Ok(text.clone())
            }
        }
        Some(_) => Err(fail(subject, key, "field must be a string")),
    }
}

/// Present-and-non-null optional string; `None` = absent. An explicit null on
/// an optional identity is garbage, not absence — the identity checks fail it
/// with the field named.
pub(crate) fn opt_string(
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

pub(crate) fn known_value_or_fail(
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
// Paths, URLs, digests, dates, secrets
// ---------------------------------------------------------------------------

/// A portable repo-relative path: forward slashes only, never absolute, no
/// `..` components, and free of secret tripwires.
pub(crate) fn check_portable_path(subject: &str, field: &str, path: &str) -> Result<(), String> {
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
    if path.starts_with('/') {
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

/// True when the portable repo-relative path falls under a denied
/// production/generated/vendor/environment edit surface.
fn is_denied_edit_surface(path: &str) -> bool {
    let lowered = path.to_ascii_lowercase();
    DENIED_SURFACE_PREFIXES
        .iter()
        .any(|prefix| lowered.starts_with(prefix))
        || lowered
            .split('/')
            .any(|component| component.contains(".generated."))
}

/// https URL with a usable host, no whitespace, no embedded credentials, and
/// no secret tripwires.
fn check_https_url(subject: &str, field: &str, url: &str) -> Result<(), String> {
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
    let authority = url
        .strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
        .unwrap_or_default();
    if authority.is_empty() || !authority.contains('.') {
        return Err(fail(
            subject,
            field,
            format!(
                "repository url `{url}` has no usable host (a repository url needs a host like `example.com`)"
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
    let lowered = text.to_ascii_lowercase();
    for tripwire in SECRET_TRIPWIRES {
        if lowered.contains(tripwire) {
            return Err(fail(
                subject,
                field,
                format!(
                    "value must not carry a secret-shaped token (matched tripwire `{tripwire}`)"
                ),
            ));
        }
    }
    Ok(())
}

/// Digest fields are bare lowercase sha256 hex (64 chars); git identity
/// fields are bare lowercase 40-char hex.
pub(crate) fn check_sha256_digest(subject: &str, field: &str, digest: &str) -> Result<(), String> {
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
            "digest must be bare lowercase sha256 hex (64 characters)",
        ))
    }
}

pub(crate) fn check_git_sha(subject: &str, field: &str, sha: &str) -> Result<(), String> {
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
            "git identity must be a bare lowercase 40-character commit SHA",
        ))
    }
}

/// Every string value in a corpus artifact stays free of secret tripwires —
/// a conservative recursive tripwire, not a secret parser; a hit fails with
/// the field path named.
pub(crate) fn reject_secret_tokens(
    value: &Value,
    subject: &str,
    field: &str,
) -> Result<(), String> {
    match value {
        Value::String(text) => check_no_secrets(subject, field, text),
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                reject_secret_tokens(item, subject, &format!("{field}[{index}]"))?;
            }
            Ok(())
        }
        Value::Object(map) => {
            for (key, item) in map {
                reject_secret_tokens(item, subject, &format!("{field}.{key}"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The native Python behavior identity is authoritative: the Rust-side
/// conversion vocabulary (`SeamKind`) appears nowhere in a corpus value. A
/// hit fails with the field path named.
fn reject_seam_kind_vocabulary(value: &Value, subject: &str, field: &str) -> Result<(), String> {
    match value {
        Value::String(text) => {
            if text.contains("SeamKind") {
                Err(fail(
                    subject,
                    field,
                    "native Python behavior identity remains authoritative: no SeamKind conversion vocabulary may appear in a corpus value",
                ))
            } else {
                Ok(())
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                reject_seam_kind_vocabulary(item, subject, &format!("{field}[{index}]"))?;
            }
            Ok(())
        }
        Value::Object(map) => {
            for (key, item) in map {
                reject_seam_kind_vocabulary(item, subject, &format!("{field}.{key}"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// `selected_at` must start with an ISO-like date (`YYYY-MM-DD`). Loose about
/// the time-of-day tail; strict about the date anchor.
fn check_date_prefix(subject: &str, field: &str, value: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    let date_shaped = bytes.len() >= 10
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit);
    if date_shaped {
        Ok(())
    } else {
        Err(fail(
            subject,
            field,
            "timestamp must start with an ISO-like date (`YYYY-MM-DD`)",
        ))
    }
}

// ---------------------------------------------------------------------------
// Selection manifest
// ---------------------------------------------------------------------------

/// One accepted selection row: the immutable denominator entry fixed before
/// outcomes are known.
#[derive(Debug, Clone)]
pub(crate) struct Selection {
    pub(crate) attempt_id: String,
    pub(crate) diversity_stratum: String,
    pub(crate) target_path: String,
    pub(crate) target_state: String,
}

#[derive(Debug, Clone)]
pub(crate) struct SelectionManifest {
    pub(crate) sha256: String,
    pub(crate) selections: Vec<Selection>,
    /// Optional identities the rows do not record; disclosed as
    /// `incomplete`, never invented.
    pub(crate) incomplete: Vec<Diagnostic>,
    /// Canonical row preimages by attempt identity, retained so digest-bound
    /// consumers (the #3569 driver binding validator) can recompute each
    /// row's `selection_digest` against the exact accepted content.
    pub(crate) row_preimages: BTreeMap<String, serde_json::Map<String, Value>>,
}

impl SelectionManifest {
    fn selection(&self, attempt_id: &str) -> Option<&Selection> {
        self.selections
            .iter()
            .find(|entry| entry.attempt_id == attempt_id)
    }
}

/// Computes the canonical digest of a selection row: sha256 over the row's
/// JSON with `selection_digest` removed, re-serialized compactly. The
/// preimage is collected into a `BTreeMap` first so the key order is sorted
/// regardless of any workspace-level serde_json `preserve_order` feature
/// (with it, `Map` iterates in insertion order and the bytes would vary by
/// construction order; rows are flat string-valued objects, so top-level
/// sorting is the whole canonicalization). This is the immutability anchor —
/// any content change moves the digest.
pub(crate) fn canonical_selection_digest(
    row: &serde_json::Map<String, Value>,
    attempt_id: &str,
) -> Result<String, String> {
    let mut canonical = row.clone();
    canonical.remove("selection_digest");
    let sorted: std::collections::BTreeMap<String, Value> = canonical.into_iter().collect();
    let text = serde_json::to_string(&sorted).map_err(|error| {
        fail(
            attempt_id,
            "selection_digest",
            format!("canonical serialization failed: {error}"),
        )
    })?;
    Ok(sha256_hex(text.as_bytes()))
}

/// Validates the accepted selection manifest. Data-driven: any set of
/// well-formed selection rows validates, not just a retained cohort.
pub(crate) fn validate_selection_manifest(
    value: &Value,
    sha256: String,
) -> Result<SelectionManifest, String> {
    let top = as_object(value, "manifest", "manifest", "selection manifest")?;
    reject_unknown_keys(top, &MANIFEST_KEYS, "manifest", "selection manifest")?;
    for (field, expected) in [
        ("schema_version", SCHEMA_VERSION),
        ("kind", MANIFEST_KIND),
        ("spec", KNOWN_SPEC),
    ] {
        let actual = top.get(field).and_then(Value::as_str).ok_or_else(|| {
            fail(
                "manifest",
                field,
                "selection manifest must declare this field",
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
    opt_string("manifest", top, "description")?;

    let selections = value
        .get("selections")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            fail(
                "manifest",
                "selections",
                "selection manifest must contain a selections array",
            )
        })?;

    let mut parsed = Vec::new();
    let mut seen = BTreeSet::new();
    let mut incomplete = Vec::new();
    let mut preimages = BTreeMap::new();
    for row in selections {
        let entry = as_object(row, "manifest", "selections", "selection row")?;
        reject_unknown_keys(entry, &SELECTION_KEYS, "manifest", "selection row")?;
        let attempt_id = require_string("manifest", entry, "attempt_id")?;
        if !seen.insert(attempt_id.clone()) {
            return Err(fail(
                &attempt_id,
                "attempt_id",
                "duplicate attempt identity in the selection manifest",
            ));
        }
        let diversity_stratum = require_string(&attempt_id, entry, "diversity_stratum")?;
        let target_state = require_string(&attempt_id, entry, "target_state")?;
        validate_selection_row(entry, &attempt_id, &target_state, &mut incomplete)?;
        preimages.insert(attempt_id.clone(), entry.clone());
        parsed.push(Selection {
            attempt_id,
            diversity_stratum,
            // `.get()` keeps a malformed row (a missing or non-string
            // `target_path`) a typed diagnostic instead of a panic; the row
            // validator reports the missing field.
            target_path: entry
                .get("target_path")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            target_state,
        });
    }

    Ok(SelectionManifest {
        sha256,
        selections: parsed,
        incomplete,
        row_preimages: preimages,
    })
}

/// Validates one selection row against the issue #3568 field contract.
/// Required fields fail closed; optional identities (`tree`,
/// `source_currentness`, `limitation`) disclose incompleteness when absent
/// and fail when present-but-malformed.
fn validate_selection_row(
    entry: &serde_json::Map<String, Value>,
    attempt_id: &str,
    target_state: &str,
    incomplete: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    for field in [
        "case_id",
        "subject_id",
        "repository",
        "base",
        "head",
        "selection_reason",
        "family",
        "owner",
        "discriminator",
        "relation",
        "oracle",
        "expected_direction",
        "claim_boundary",
        "target_path",
        "selected_at",
        "selector",
        "authority_snapshot_digest",
        "selection_digest",
    ] {
        require_string(attempt_id, entry, field)?;
    }

    check_https_url(
        attempt_id,
        "repository",
        entry["repository"].as_str().unwrap_or_default(),
    )?;
    check_git_sha(
        attempt_id,
        "base",
        entry["base"].as_str().unwrap_or_default(),
    )?;
    check_git_sha(
        attempt_id,
        "head",
        entry["head"].as_str().unwrap_or_default(),
    )?;
    check_date_prefix(
        attempt_id,
        "selected_at",
        entry["selected_at"].as_str().unwrap_or_default(),
    )?;
    check_portable_path(
        attempt_id,
        "target_path",
        entry["target_path"].as_str().unwrap_or_default(),
    )?;
    check_sha256_digest(
        attempt_id,
        "authority_snapshot_digest",
        entry["authority_snapshot_digest"]
            .as_str()
            .unwrap_or_default(),
    )?;
    known_value_or_fail(
        attempt_id,
        "expected_direction",
        entry["expected_direction"].as_str().unwrap_or_default(),
        &EXPECTED_DIRECTIONS,
        "expected direction",
    )?;
    known_value_or_fail(
        attempt_id,
        "target_state",
        target_state,
        &TARGET_STATES,
        "target state",
    )?;

    let target_path = entry["target_path"].as_str().unwrap_or_default();
    if is_denied_edit_surface(target_path) && target_state != "unsafe" {
        return Err(fail(
            attempt_id,
            "target_state",
            format!(
                "target path `{target_path}` falls under a production/generated/vendor/environment edit surface and must be declared `unsafe`"
            ),
        ));
    }

    // Optional identities: absent discloses incomplete; an explicit null or a
    // malformed value is garbage, not absence, and fails.
    match entry.get("tree") {
        None => incomplete.push(Diagnostic::new(
            attempt_id,
            "tree",
            "tree digest not recorded at selection time; typed incomplete, not invented",
        )),
        Some(Value::Null) => {
            return Err(fail(
                attempt_id,
                "tree",
                "identity is explicitly null; omit the field to record it absent — a present null is not an absent identity",
            ));
        }
        Some(Value::String(tree)) => check_sha256_digest(attempt_id, "tree", tree)?,
        Some(_) => {
            return Err(fail(
                attempt_id,
                "tree",
                "field must be a string when present",
            ));
        }
    }
    match entry.get("source_currentness") {
        None => incomplete.push(Diagnostic::new(
            attempt_id,
            "source_currentness",
            "source currentness not recorded; typed incomplete, not invented",
        )),
        Some(Value::Null) => {
            return Err(fail(
                attempt_id,
                "source_currentness",
                "identity is explicitly null; omit the field to record it absent — a present null is not an absent identity",
            ));
        }
        Some(Value::String(state)) => known_value_or_fail(
            attempt_id,
            "source_currentness",
            state,
            &SOURCE_CURRENTNESS,
            "source-currentness disposition",
        )?,
        Some(_) => {
            return Err(fail(
                attempt_id,
                "source_currentness",
                "field must be a string when present",
            ));
        }
    }
    match entry.get("limitation") {
        None => incomplete.push(Diagnostic::new(
            attempt_id,
            "limitation",
            "limitation record not made; typed incomplete, not invented",
        )),
        Some(Value::Null) => {
            return Err(fail(
                attempt_id,
                "limitation",
                "identity is explicitly null; omit the field to record it absent — a present null is not an absent identity",
            ));
        }
        Some(Value::String(text)) => {
            if text.trim().is_empty() {
                return Err(fail(
                    attempt_id,
                    "limitation",
                    "must be non-empty when present",
                ));
            }
        }
        Some(_) => {
            return Err(fail(
                attempt_id,
                "limitation",
                "field must be a string when present",
            ));
        }
    }

    // Content sanity scans run before the immutability anchor so a row
    // carrying a forbidden token fails with that reason, not a generic digest
    // mismatch: every string stays free of secret tripwires, and the native
    // Python behavior identity admits no `SeamKind` conversion vocabulary.
    let row_value = Value::Object(entry.clone());
    reject_secret_tokens(&row_value, attempt_id, "selection")?;
    reject_seam_kind_vocabulary(&row_value, attempt_id, "selection")?;

    // The immutable digest: the recorded value must equal the canonical
    // content digest, so any replacement or edit of a selected row fails.
    let recorded = entry["selection_digest"].as_str().unwrap_or_default();
    let recomputed = canonical_selection_digest(entry, attempt_id)?;
    if recorded != recomputed {
        return Err(fail(
            attempt_id,
            "selection_digest",
            format!(
                "selection row digest mismatch: recorded `{recorded}` but the canonical content digests to `{recomputed}`; selected rows are immutable once outcomes exist"
            ),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Attempt envelopes
// ---------------------------------------------------------------------------

/// One validated attempt row reduced to what aggregate derivation needs.
#[derive(Debug)]
struct AttemptRow {
    attempt_id: String,
    final_state: String,
    supersedes: Option<String>,
    movement: Option<String>,
    execution: Option<String>,
    stratum: String,
}

#[derive(Debug, Default)]
struct EnvelopeAggregates {
    attempts_total: u64,
    lifecycle_counts: BTreeMap<String, u64>,
    movement_counts: BTreeMap<String, u64>,
    execution_counts: BTreeMap<String, u64>,
    achieved_strata: BTreeMap<String, u64>,
}

#[derive(Debug)]
struct AttemptsEnvelope {
    display: String,
    rows: Vec<AttemptRow>,
    incomplete: Vec<Diagnostic>,
}

/// Validates the attempts input. `input` is a file (one envelope) or a
/// directory (every `*.json` file, sorted, one envelope each). Every
/// envelope binds to the selection manifest by sha256 digest.
fn validate_attempts_input(
    input: &str,
    manifest: &SelectionManifest,
    manifest_sha256: &str,
) -> Result<Vec<AttemptsEnvelope>, String> {
    let path = Path::new(input);
    if !path.exists() {
        return Err(fail(
            input,
            "attempts",
            "attempts input path does not exist",
        ));
    }
    let files: Vec<String> = if path.is_dir() {
        let mut entries = Vec::new();
        let read = std::fs::read_dir(path).map_err(|error| {
            fail(
                input,
                "attempts",
                format!("failed to read directory: {error}"),
            )
        })?;
        for entry in read {
            let entry = entry.map_err(|error| {
                fail(
                    input,
                    "attempts",
                    format!("failed to read directory entry: {error}"),
                )
            })?;
            let name = entry.file_name().to_string_lossy().to_string();
            let full = entry.path();
            if full.is_file() && name.ends_with(".json") {
                entries.push(full.to_string_lossy().to_string());
            }
        }
        entries.sort();
        if entries.is_empty() {
            return Err(fail(
                input,
                "attempts",
                "attempts directory carries no *.json envelopes",
            ));
        }
        entries
    } else {
        vec![input.to_string()]
    };

    let mut envelopes = Vec::new();
    let mut seen_ids: BTreeSet<String> = BTreeSet::new();
    for display in &files {
        let envelope =
            validate_attempts_envelope(display, manifest, manifest_sha256, &mut seen_ids)?;
        envelopes.push(envelope);
    }
    validate_supersedes_relations(&envelopes)?;
    Ok(envelopes)
}

/// Validates one attempts envelope: envelope identity, manifest binding,
/// per-row semantics, aggregate agreement, and uniqueness of attempt
/// identities across the whole loaded corpus.
fn validate_attempts_envelope(
    display: &str,
    manifest: &SelectionManifest,
    manifest_sha256: &str,
    seen_ids: &mut BTreeSet<String>,
) -> Result<AttemptsEnvelope, String> {
    let (value, _sha) = load_strict_json(display)?;
    let top = as_object(&value, display, "envelope", "attempts envelope")?;
    reject_unknown_keys(top, &ATTEMPTS_KEYS, display, "attempts envelope")?;
    for (field, expected) in [
        ("schema_version", SCHEMA_VERSION),
        ("kind", ATTEMPTS_KIND),
        ("spec", KNOWN_SPEC),
    ] {
        let actual = top
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| fail(display, field, "attempts envelope must declare this field"))?;
        if actual != expected {
            return Err(fail(
                display,
                field,
                format!("expected `{expected}`, got `{actual}`"),
            ));
        }
    }

    // The manifest binding: the envelope is only current for the exact
    // selection manifest bytes it was built against.
    let binding = require_string(display, top, "manifest_digest")?;
    check_sha256_digest(display, "manifest_digest", &binding)?;
    if binding != manifest_sha256 {
        return Err(fail(
            display,
            "manifest_digest",
            format!(
                "stale digest: envelope is bound to manifest {binding} but the accepted selection manifest is {manifest_sha256}"
            ),
        ));
    }

    // Aggregates are owned in full: the envelope always carries the aggregate
    // set, and it must agree with the derived rows below.
    let aggregates = match top.get("aggregates") {
        Some(Value::Object(map)) => map,
        Some(_) => {
            return Err(fail(display, "aggregates", "aggregates must be an object"));
        }
        None => {
            return Err(fail(
                display,
                "aggregates",
                "attempts envelope must carry its aggregates",
            ));
        }
    };

    let rows_value = value
        .get("attempts")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            fail(
                display,
                "attempts",
                "attempts envelope must contain an attempts array",
            )
        })?;

    let incomplete = Vec::new();
    let mut rows = Vec::new();
    for row in rows_value {
        let entry = as_object(row, display, "attempts", "attempt row")?;
        reject_unknown_keys(entry, &ATTEMPT_KEYS, display, "attempt row")?;
        let attempt_id = require_string(display, entry, "attempt_id")?;
        if !seen_ids.insert(attempt_id.clone()) {
            return Err(fail(
                &attempt_id,
                "attempt_id",
                "duplicate attempt identity: a historical attempt is immutable, and a refresh appends a new record under a new attempt identity",
            ));
        }
        let row = validate_attempt_row(entry, &attempt_id, manifest)?;
        rows.push(row);
    }

    let derived = derive_aggregates(&rows, display)?;
    validate_aggregates(display, aggregates, &derived, manifest)?;

    Ok(AttemptsEnvelope {
        display: display.to_string(),
        rows,
        incomplete,
    })
}

/// Validates one attempt row: lifecycle state machine, movement/execution
/// separation, transition-required identities, and the unsafe-target law.
fn validate_attempt_row(
    entry: &serde_json::Map<String, Value>,
    attempt_id: &str,
    manifest: &SelectionManifest,
) -> Result<AttemptRow, String> {
    // Every attempt must reference an existing selected row: a missing
    // reference is a deleted or replaced selection, never a new freedom.
    let selection = manifest.selection(attempt_id).ok_or_else(|| {
        fail(
            attempt_id,
            "attempt_id",
            "names an attempt identity outside the accepted selection denominator; selected rows cannot be deleted or replaced after outcome",
        )
    })?;

    // Lifecycle states: vocabulary, selected-first, no repeats, ordered
    // progression, discard terminals last.
    let states = entry
        .get("states")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            fail(
                attempt_id,
                "states",
                "attempt row must carry a states array",
            )
        })?;
    if states.is_empty() {
        return Err(fail(
            attempt_id,
            "states",
            "attempt lifecycle must carry at least the `selected` state",
        ));
    }
    let mut parsed_states = Vec::new();
    let mut state_set = BTreeSet::new();
    for state in states {
        let state = state
            .as_str()
            .ok_or_else(|| fail(attempt_id, "states", "lifecycle states must be strings"))?;
        known_value_or_fail(
            attempt_id,
            "states",
            state,
            &LIFECYCLE_STATES,
            "lifecycle state",
        )?;
        if !state_set.insert(state.to_string()) {
            return Err(fail(
                attempt_id,
                "states",
                format!(
                    "lifecycle state `{state}` repeats; a lifecycle passes through each state once"
                ),
            ));
        }
        parsed_states.push(state.to_string());
    }
    if parsed_states[0] != "selected" {
        return Err(fail(
            attempt_id,
            "states",
            format!(
                "selection precedes edit and outcome: the lifecycle must start at `selected`, got `{}`",
                parsed_states[0]
            ),
        ));
    }
    let rank = |state: &str| -> u8 {
        PROGRESS_RANKS
            .iter()
            .find(|(name, _)| *name == state)
            .map(|(_, rank)| *rank)
            .unwrap_or(0)
    };
    let mut last_rank = 0u8;
    for state in &parsed_states {
        // Discard terminals are ordered by the terminal-last rule, not by the
        // progress ranks.
        if DISCARD_TERMINALS.contains(&state.as_str()) {
            continue;
        }
        let current = rank(state);
        if current < last_rank {
            return Err(fail(
                attempt_id,
                "states",
                format!(
                    "state machine ordering violated: `{state}` appears after a later progress state; the lifecycle ordering is enforced"
                ),
            ));
        }
        last_rank = last_rank.max(current);
    }
    for (state, predecessor) in [
        ("edited", "started"),
        ("verified", "edited"),
        ("reviewed", "verified"),
        ("accepted", "reviewed"),
    ] {
        if state_set.contains(state) && !state_set.contains(predecessor) {
            return Err(fail(
                attempt_id,
                "states",
                format!(
                    "lifecycle ordering violated: `{state}` requires `{predecessor}` to have been reached"
                ),
            ));
        }
    }
    let final_state = parsed_states[parsed_states.len() - 1].clone();
    for terminal in DISCARD_TERMINALS {
        if state_set.contains(terminal) && final_state != terminal {
            return Err(fail(
                attempt_id,
                "states",
                format!(
                    "`{terminal}` is a terminal state: a lifecycle that went {terminal} ends there"
                ),
            ));
        }
    }

    // Partial/stale/abandoned attempts cannot appear completed.
    let discarded = DISCARD_TERMINALS
        .iter()
        .any(|terminal| state_set.contains(*terminal));
    if discarded {
        for completed in COMPLETED_STATES {
            if state_set.contains(completed) {
                return Err(fail(
                    attempt_id,
                    "states",
                    format!(
                        "contradictory states: a stale/rejected/abandoned attempt cannot appear completed (`{completed}`)"
                    ),
                ));
            }
        }
    }

    // Movement: separate axis, requires an edit, never implied by a run.
    let movement = opt_string(attempt_id, entry, "movement")?;
    if let Some(movement) = &movement {
        known_value_or_fail(
            attempt_id,
            "movement",
            movement,
            &MOVEMENTS,
            "static movement",
        )?;
        if !state_set.contains("edited") {
            return Err(fail(
                attempt_id,
                "movement",
                format!(
                    "static movement `{movement}` recorded without an edited lifecycle: movement is a before/after comparison of an edit and cannot be implied by selection or by a verification run"
                ),
            ));
        }
        if discarded && COMPLETED_MOVEMENTS.contains(&movement.as_str()) {
            return Err(fail(
                attempt_id,
                "movement",
                format!(
                    "contradictory states: a stale/rejected/abandoned attempt cannot appear completed (`{movement}` movement)"
                ),
            ));
        }
    }

    // Execution: separate axis, never implies movement. A verdict execution
    // evidences the `verified` state; a non-verdict execution contradicts it.
    let execution = opt_string(attempt_id, entry, "execution")?;
    if let Some(execution) = &execution {
        known_value_or_fail(
            attempt_id,
            "execution",
            execution,
            &EXECUTIONS,
            "verification execution",
        )?;
        if VERDICT_EXECUTIONS.contains(&execution.as_str()) {
            if !state_set.contains("verified") {
                return Err(fail(
                    attempt_id,
                    "execution",
                    format!(
                        "contradictory states: a `{execution}` verdict requires the lifecycle to have reached `verified`"
                    ),
                ));
            }
        } else if state_set.contains("verified") {
            return Err(fail(
                attempt_id,
                "execution",
                format!(
                    "contradictory states: the lifecycle claims `verified` but the execution state `{execution}` records no completed verification verdict"
                ),
            ));
        }
    }
    if state_set.contains("verified") {
        match &execution {
            Some(execution) if VERDICT_EXECUTIONS.contains(&execution.as_str()) => {}
            Some(execution) => {
                return Err(fail(
                    attempt_id,
                    "execution",
                    format!(
                        "contradictory states: the lifecycle claims `verified` but the execution state `{execution}` records no completed verification verdict"
                    ),
                ));
            }
            None => {
                return Err(fail(
                    attempt_id,
                    "execution",
                    "the lifecycle claims `verified` but records no verification execution state",
                ));
            }
        }
    }

    // Unsafe edit surfaces are forbidden to attempt: a declared-unsafe target
    // that was edited fails. Ambiguous/unavailable targets stay representable
    // until an edit is attempted on them — only `existing` targets admit a
    // consistent edit.
    if state_set.contains("edited") && selection.target_state == "unsafe" {
        return Err(fail(
            attempt_id,
            "states",
            "attempted edit on an unsafe target: production/generated/vendor/environment edit surfaces are forbidden",
        ));
    }

    let supersedes = opt_string(attempt_id, entry, "supersedes")?;
    if supersedes.as_deref() == Some(attempt_id) {
        return Err(fail(
            attempt_id,
            "supersedes",
            "an attempt cannot supersede itself",
        ));
    }

    reject_seam_kind_vocabulary(&Value::Object(entry.clone()), attempt_id, "attempt")?;

    // Supplied identity blocks are shape-validated at every lifecycle
    // position: an unknown field, a present null, a mistyped value, or a
    // malformed digest fails when supplied, not only once its transition is
    // reached — a malformed block cannot hide before its transition. Only
    // the required-at-transition rule below stays state-dependent.
    for block in IDENTITY_BLOCKS {
        let Some((allowed, check)) = identity_block_schema(block) else {
            continue;
        };
        match entry.get(block) {
            None => {}
            Some(Value::Object(block_object)) => {
                validate_supplied_block(attempt_id, block, block_object, allowed, check)?;
            }
            Some(Value::Null) => {
                return Err(fail(
                    attempt_id,
                    block,
                    "identity is explicitly null; omit the field to record it absent — a present null is not an absent identity",
                ));
            }
            Some(_) => {
                return Err(fail(
                    attempt_id,
                    block,
                    format!("`{block}` identity block must be an object"),
                ));
            }
        }
    }

    // Identities required as lifecycle advances. Each reached state demands
    // its identities present — absent identities at their transition fail
    // closed (they are never invented). The shape of a supplied block is
    // already validated above at any position.
    if state_set.contains("started") {
        validate_required_block(
            attempt_id,
            entry,
            "analyzer",
            "started",
            &ANALYZER_KEYS,
            analyzer_identity_value,
        )?;
        validate_required_block(
            attempt_id,
            entry,
            "config",
            "started",
            &CONFIG_KEYS,
            require_non_empty_value,
        )?;
        validate_required_block(
            attempt_id,
            entry,
            "input",
            "started",
            &INPUT_KEYS,
            check_sha256_digest,
        )?;
    }
    if state_set.contains("edited") {
        validate_required_block(
            attempt_id,
            entry,
            "patch",
            "edited",
            &PATCH_KEYS,
            check_sha256_digest,
        )?;
    }
    if state_set.contains("verified") {
        validate_required_block(
            attempt_id,
            entry,
            "command",
            "verified",
            &COMMAND_KEYS,
            require_non_empty_value,
        )?;
        validate_required_block(
            attempt_id,
            entry,
            "after_state",
            "verified",
            &AFTER_STATE_KEYS,
            check_sha256_digest,
        )?;
    }
    if state_set.contains("reviewed") || state_set.contains("accepted") {
        validate_required_block(
            attempt_id,
            entry,
            "packet",
            "reviewed",
            &PACKET_KEYS,
            require_non_empty_value,
        )?;
    }

    Ok(AttemptRow {
        attempt_id: attempt_id.to_string(),
        final_state,
        supersedes,
        movement,
        execution,
        stratum: selection.diversity_stratum.clone(),
    })
}

/// Shape-validates one supplied identity block, whatever the lifecycle
/// position: unknown keys are denied and every present field must be a
/// non-null, well-formed string passing its block's value check. A
/// malformed supplied block fails here rather than hiding before its
/// transition; only the required-at-transition rule
/// (`validate_required_block`) is state-dependent.
fn validate_supplied_block(
    attempt_id: &str,
    block: &str,
    block_object: &serde_json::Map<String, Value>,
    allowed: &[&str],
    check: BlockValueCheck,
) -> Result<(), String> {
    let what = format!("`{block}` identity block");
    reject_unknown_keys(block_object, allowed, attempt_id, &what)?;
    for field in allowed {
        let value = match block_object.get(*field) {
            None => continue,
            Some(Value::Null) => {
                return Err(fail(
                    attempt_id,
                    &format!("{block}.{field}"),
                    "identity is explicitly null; a present null is not a value",
                ));
            }
            Some(Value::String(text)) => text.clone(),
            Some(_) => {
                return Err(fail(
                    attempt_id,
                    &format!("{block}.{field}"),
                    "identity must be a string when present",
                ));
            }
        };
        check(attempt_id, &format!("{block}.{field}"), &value)?;
    }
    Ok(())
}

/// Validates one transition-required identity block: the block and every
/// required field inside it must be present when the lifecycle has reached
/// `state`. Absence at the transition fails closed; the shape of a supplied
/// block is validated at any lifecycle position (see
/// `validate_supplied_block`) and is re-checked here so the required path
/// stands alone.
fn validate_required_block(
    attempt_id: &str,
    entry: &serde_json::Map<String, Value>,
    block: &str,
    state: &str,
    allowed: &[&str],
    check: BlockValueCheck,
) -> Result<(), String> {
    let what = format!("`{state}` transition identity");
    let block_object = match entry.get(block) {
        Some(Value::Object(map)) => map,
        Some(Value::Null) => {
            return Err(fail(
                attempt_id,
                block,
                format!(
                    "required identity is explicitly null: reaching `{state}` requires the {block} identity"
                ),
            ));
        }
        Some(_) => {
            return Err(fail(attempt_id, block, format!("{what} must be an object")));
        }
        None => {
            return Err(fail(
                attempt_id,
                block,
                format!(
                    "required identity is missing: reaching `{state}` requires the {block} identity"
                ),
            ));
        }
    };
    validate_supplied_block(attempt_id, block, block_object, allowed, check)?;
    for field in allowed {
        if !block_object.contains_key(*field) {
            return Err(fail(
                attempt_id,
                &format!("{block}.{field}"),
                format!(
                    "required identity is missing: reaching `{state}` requires `{block}.{field}`"
                ),
            ));
        }
    }
    Ok(())
}

/// Cross-envelope `supersedes` relations: a refresh appends a new record
/// keyed by a new attempt identity, linked to a historical record that ended
/// `stale`. Superseding a non-stale (or unknown) record mutates history
/// instead of appending, so it fails; two records superseding the same stale
/// record leave two current successors, so that fails too.
fn validate_supersedes_relations(envelopes: &[AttemptsEnvelope]) -> Result<(), String> {
    let mut final_state_by_id: BTreeMap<String, String> = BTreeMap::new();
    let mut successor_by_target: BTreeMap<String, String> = BTreeMap::new();
    for envelope in envelopes {
        for row in &envelope.rows {
            final_state_by_id.insert(row.attempt_id.clone(), row.final_state.clone());
        }
    }
    for envelope in envelopes {
        for row in &envelope.rows {
            let Some(target) = &row.supersedes else {
                continue;
            };
            if successor_by_target
                .insert(target.clone(), row.attempt_id.clone())
                .is_some()
            {
                return Err(fail(
                    &row.attempt_id,
                    "supersedes",
                    format!(
                        "two records supersede `{target}`; one historical attempt has exactly one current successor"
                    ),
                ));
            }
            match final_state_by_id.get(target) {
                None => {
                    return Err(fail(
                        &row.attempt_id,
                        "supersedes",
                        format!(
                            "names attempt identity `{target}` that the loaded corpus does not retain; a refresh links to the retained stale record it replaces"
                        ),
                    ));
                }
                Some(final_state) if final_state != "stale" => {
                    return Err(fail(
                        &row.attempt_id,
                        "supersedes",
                        format!(
                            "superseded attempt `{target}` ended `{final_state}`; only a stale record is refreshed — completed and rejected records are immutable history"
                        ),
                    ));
                }
                Some(_) => {}
            }
        }
    }
    // A refresh chain must terminate: following `supersedes` links from any
    // record may never revisit an identity. A cycle means impossible history
    // (every link in the cycle supersedes a stale record, yet nothing in the
    // cycle could have been stale first).
    for envelope in envelopes {
        for row in &envelope.rows {
            if let Some(revisited) = refresh_chain_revisits(&successor_by_target, &row.attempt_id) {
                return Err(fail(
                    &row.attempt_id,
                    "supersedes",
                    format!(
                        "refresh chain revisits `{revisited}`; a `supersedes` cycle is impossible history (every link claims to refresh a stale record, so no link in the cycle could have been stale first)"
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Walks the `supersedes` successor map from `start`; returns the first
/// identity visited twice, if the chain ever loops back.
fn refresh_chain_revisits(
    successor_by_target: &BTreeMap<String, String>,
    start: &str,
) -> Option<String> {
    let mut seen = BTreeSet::new();
    let mut current = start.to_string();
    loop {
        if !seen.insert(current.clone()) {
            return Some(current);
        }
        match successor_by_target.get(&current) {
            Some(next) => current = next.clone(),
            None => return None,
        }
    }
}

// ---------------------------------------------------------------------------
// Aggregates
// ---------------------------------------------------------------------------

/// Derives the aggregates from validated rows only — the arithmetic every
/// hand-entered total must agree with. Checked throughout: an overflowing
/// count is a structured failure naming the aggregate field, never a panic.
fn derive_aggregates(rows: &[AttemptRow], display: &str) -> Result<EnvelopeAggregates, String> {
    let mut derived = EnvelopeAggregates {
        attempts_total: rows.len() as u64,
        ..EnvelopeAggregates::default()
    };
    for row in rows {
        bump(
            &mut derived.lifecycle_counts,
            &row.final_state,
            "lifecycle_counts",
            display,
        )?;
        if let Some(movement) = &row.movement {
            bump(
                &mut derived.movement_counts,
                movement,
                "movement_counts",
                display,
            )?;
        }
        if let Some(execution) = &row.execution {
            bump(
                &mut derived.execution_counts,
                execution,
                "execution_counts",
                display,
            )?;
        }
        bump(
            &mut derived.achieved_strata,
            &row.stratum,
            "achieved_strata",
            display,
        )?;
    }
    Ok(derived)
}

fn bump(
    target: &mut BTreeMap<String, u64>,
    key: &str,
    field: &str,
    display: &str,
) -> Result<(), String> {
    let bucket = target.entry(key.to_string()).or_insert(0);
    *bucket = bucket.checked_add(1).ok_or_else(|| {
        fail(
            display,
            &format!("aggregates.{field}.{key}"),
            "aggregate overflow: count exceeds u64 when summed across attempt rows",
        )
    })?;
    Ok(())
}

/// Validates the recorded aggregates against the derived rows: required
/// totals at nonzero denominators, exact map equality both ways, the
/// zero-row fabrication law, and honest diversity floors.
fn validate_aggregates(
    display: &str,
    aggregates: &serde_json::Map<String, Value>,
    derived: &EnvelopeAggregates,
    manifest: &SelectionManifest,
) -> Result<(), String> {
    reject_unknown_keys(aggregates, &AGGREGATES_KEYS, display, "aggregates")?;

    let has_rows = derived.attempts_total > 0;

    // Required-at-rows (mirror of the eval-sweep SUMMARY_KEYS discipline):
    // once rows exist the envelope records the full owned aggregate set —
    // every field in `AGGREGATES_KEYS`, including the selected denominator
    // and the diversity aggregates. An omitted field would silently disable
    // its row-agreement check and slip a partial envelope through as valid.
    if has_rows {
        for field in AGGREGATES_KEYS {
            if matches!(aggregates.get(field), None | Some(Value::Null)) {
                return Err(fail(
                    display,
                    &format!("aggregates.{field}"),
                    "required aggregate is missing: the envelope records the full aggregate set on every corpus with rows, and an omitted field would silently disable its row-agreement check",
                ));
            }
        }
    }

    // The selected denominator: optional but exact — the count is the
    // manifest's fixed selection set, the denominator fixed before outcomes.
    match opt_u64(display, aggregates, "selected_denominator")? {
        // Compare in u64: a `value as usize` truncation could fold an
        // oversized fabricated denominator onto the true count on 32-bit
        // targets (#3734 review).
        Some(value) if value != manifest.selections.len() as u64 => {
            return Err(fail(
                display,
                "aggregates.selected_denominator",
                format!(
                    "hand-edited aggregate: the envelope claims {value} selected row(s) but the accepted selection manifest carries {}",
                    manifest.selections.len()
                ),
            ));
        }
        _ => {}
    }

    match opt_u64(display, aggregates, "attempts_total")? {
        Some(value) if value != derived.attempts_total => {
            return Err(fail(
                display,
                "aggregates.attempts_total",
                format!(
                    "hand-edited aggregate: the envelope claims {value} attempt row(s) but the rows derive {}",
                    derived.attempts_total
                ),
            ));
        }
        _ => {}
    }

    // Exact map equality, both directions, for every count map.
    for (field, derived_map) in [
        ("lifecycle_counts", &derived.lifecycle_counts),
        ("movement_counts", &derived.movement_counts),
        ("execution_counts", &derived.execution_counts),
        ("achieved_strata", &derived.achieved_strata),
    ] {
        if let Some(recorded) = opt_distribution(display, aggregates, field)? {
            check_map_equality(display, field, &recorded, derived_map)?;
        }
    }

    // Achieved diversity without pretending the floor was met.
    let floor = opt_distribution(display, aggregates, "stratum_floor")?;
    let floor_met_derived = match &floor {
        Some(floor) => floor.iter().all(|(stratum, minimum)| {
            derived.achieved_strata.get(stratum).copied().unwrap_or(0) >= *minimum
        }),
        None => false,
    };
    match opt_bool(display, aggregates, "stratum_floor_met")? {
        Some(recorded) if recorded != floor_met_derived => {
            if recorded {
                return Err(fail(
                    display,
                    "aggregates.stratum_floor_met",
                    "stratum floor claimed met but the achieved diversity does not meet the recorded floor (or no floor is recorded); achieved diversity is reported without pretending the target floor was met",
                ));
            }
            return Err(fail(
                display,
                "aggregates.stratum_floor_met",
                "stratum floor claimed unmet but the achieved diversity meets the recorded floor",
            ));
        }
        _ => {}
    }
    Ok(())
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
                    "count map must be an object of integer counts",
                )
            })?;
            let mut out = BTreeMap::new();
            for (name, count) in map {
                let number = count.as_u64().ok_or_else(|| {
                    fail(
                        subject,
                        key,
                        format!("count `{name}` must be a non-negative integer"),
                    )
                })?;
                out.insert(name.clone(), number);
            }
            Ok(Some(out))
        }
    }
}

/// Exact map equality between a recorded count map and the row-derived key
/// set: extra keys are denied by name, missing keys fail, and every value
/// must match.
fn check_map_equality(
    display: &str,
    field: &str,
    recorded: &BTreeMap<String, u64>,
    derived: &BTreeMap<String, u64>,
) -> Result<(), String> {
    for (name, count) in recorded {
        match derived.get(name) {
            Some(expected) if expected == count => {}
            Some(expected) => {
                return Err(fail(
                    display,
                    &format!("aggregates.{field}.{name}"),
                    format!(
                        "hand-edited aggregate: the envelope claims {count} but the rows derive {expected}"
                    ),
                ));
            }
            None => {
                return Err(fail(
                    display,
                    &format!("aggregates.{field}.{name}"),
                    format!(
                        "hand-edited aggregate: the envelope claims {count} for `{name}` but the rows never establish that key"
                    ),
                ));
            }
        }
    }
    for (name, expected) in derived {
        match recorded.get(name) {
            Some(count) if count == expected => {}
            Some(count) => {
                return Err(fail(
                    display,
                    &format!("aggregates.{field}.{name}"),
                    format!(
                        "hand-edited aggregate: the envelope claims {count} but the rows derive {expected}"
                    ),
                ));
            }
            None => {
                return Err(fail(
                    display,
                    &format!("aggregates.{field}.{name}"),
                    format!(
                        "hand-edited aggregate: the rows derive {expected} for `{name}` but the envelope omits it"
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

struct CheckOutcome {
    manifest_path: String,
    manifest: Option<SelectionManifest>,
    attempts_input: Option<String>,
    envelopes: Vec<AttemptsEnvelope>,
}

impl CheckOutcome {
    /// Top-level verdict precedence: `not_run` when no attempts dimension is
    /// supplied or it carries zero rows (a zero-row corpus is not_run, never
    /// a vacuous pass); `incomplete` when any artifact discloses incomplete
    /// identities; `valid` only when everything is structurally valid with
    /// zero incompletes.
    fn verdict(&self) -> Verdict {
        let total_rows: usize = self
            .envelopes
            .iter()
            .map(|envelope| envelope.rows.len())
            .sum();
        if self.envelopes.is_empty() || total_rows == 0 {
            return Verdict::NotRun;
        }
        let manifest_clean = self
            .manifest
            .as_ref()
            .map(|manifest| manifest.incomplete.is_empty())
            .unwrap_or(true);
        let envelopes_clean = self
            .envelopes
            .iter()
            .all(|envelope| envelope.incomplete.is_empty());
        if manifest_clean && envelopes_clean {
            Verdict::Valid
        } else {
            Verdict::Incomplete
        }
    }

    fn incomplete(&self) -> Vec<&Diagnostic> {
        let mut all: Vec<&Diagnostic> = self
            .manifest
            .as_ref()
            .map(|manifest| manifest.incomplete.iter().collect())
            .unwrap_or_default();
        for envelope in &self.envelopes {
            all.extend(envelope.incomplete.iter());
        }
        all
    }
}

/// The full offline check: load + validate the selection manifest, then (when
/// supplied) the attempts input. When no manifest exists at the default path
/// and none was supplied, the denominator is simply not established yet —
/// the check reports `not_run` and exits 0.
fn check_artifacts(
    manifest_path: &str,
    manifest_explicit: bool,
    attempts: Option<&str>,
) -> Result<CheckOutcome, String> {
    let manifest = if Path::new(manifest_path).exists() {
        let (value, sha256) = load_strict_json(manifest_path)?;
        Some(validate_selection_manifest(&value, sha256)?)
    } else {
        if manifest_explicit {
            return Err(fail(
                manifest_path,
                "manifest",
                "explicitly supplied selection manifest does not exist",
            ));
        }
        if attempts.is_some() {
            return Err(fail(
                manifest_path,
                "manifest",
                "attempts were supplied but no accepted selection manifest exists to bind them",
            ));
        }
        None
    };

    let envelopes = match (attempts, &manifest) {
        (None, _) => Vec::new(),
        (Some(input), None) => {
            return Err(fail(
                input,
                "attempts",
                "no accepted selection manifest exists; attempts bind to a selection manifest by digest",
            ));
        }
        (Some(input), Some(manifest)) => {
            validate_attempts_input(input, manifest, &manifest.sha256)?
        }
    };

    Ok(CheckOutcome {
        manifest_path: manifest_path.to_string(),
        manifest,
        attempts_input: attempts.map(str::to_string),
        envelopes,
    })
}

pub(crate) fn run_check(args: &[String]) -> Result<(), String> {
    let parsed = parse_check_args(args)?;
    let outcome = check_artifacts(
        &parsed.manifest,
        parsed.manifest_explicit,
        parsed.attempts.as_deref(),
    )?;
    let verdict = outcome.verdict();

    match &outcome.manifest {
        None => println!(
            "python-repair-trust check: manifest={} selections=<none> (no accepted selection manifest at the default path; the denominator is not established)",
            outcome.manifest_path
        ),
        Some(manifest) => println!(
            "python-repair-trust check: manifest={} selections={} sha256={}",
            outcome.manifest_path,
            manifest.selections.len(),
            manifest.sha256
        ),
    }
    let total_rows: usize = outcome
        .envelopes
        .iter()
        .map(|envelope| envelope.rows.len())
        .sum();
    match &outcome.attempts_input {
        None => println!(
            "python-repair-trust check: attempts=<none> verdict={} (no attempts supplied; not_run is not a pass)",
            verdict.as_str()
        ),
        Some(input) => println!(
            "python-repair-trust check: attempts={input} envelopes={} rows={total_rows} verdict={}",
            outcome.envelopes.len(),
            verdict.as_str()
        ),
    }
    let disclosures = outcome.incomplete();
    println!(
        "python-repair-trust check: incomplete identities disclosed: {}",
        disclosures.len()
    );
    for diagnostic in &disclosures {
        println!("  incomplete: {}", diagnostic.render());
    }
    println!(
        "python-repair-trust check verdict: {} — selection and lifecycle structural validation only; no completed or correct repair is established",
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
    let manifest_json = match &outcome.manifest {
        None => json!(null),
        Some(manifest) => {
            let incomplete: Vec<Value> = manifest
                .incomplete
                .iter()
                .map(|diagnostic| json!(diagnostic.render()))
                .collect();
            let attempt_ids: Vec<String> = manifest
                .selections
                .iter()
                .map(|selection| selection.attempt_id.clone())
                .collect();
            json!({
                "path": outcome.manifest_path,
                "sha256": manifest.sha256,
                "selections": manifest.selections.len(),
                "selection_attempt_ids": attempt_ids,
                "incomplete": incomplete,
            })
        }
    };
    let attempts_json = if outcome.envelopes.is_empty() {
        json!(null)
    } else {
        let total_rows: usize = outcome
            .envelopes
            .iter()
            .map(|envelope| envelope.rows.len())
            .sum();
        let envelope_items: Vec<Value> = outcome
            .envelopes
            .iter()
            .map(|envelope| {
                let envelope_verdict = if envelope.rows.is_empty() {
                    Verdict::NotRun
                } else if envelope.incomplete.is_empty() {
                    Verdict::Valid
                } else {
                    Verdict::Incomplete
                };
                json!({
                    "path": envelope.display,
                    "rows": envelope.rows.len(),
                    "verdict": envelope_verdict.as_str(),
                })
            })
            .collect();
        json!({
            "input": outcome.attempts_input,
            "envelopes": envelope_items,
            "rows": total_rows,
            "verdict": verdict.as_str(),
        })
    };
    let document = json!({
        "schema_version": "0.1",
        "kind": "python_repair_trust_check_report",
        "spec": KNOWN_SPEC,
        "verdict": verdict.as_str(),
        "manifest": manifest_json,
        "attempts": attempts_json,
        "offline": true,
        "claim_boundary": "selection and lifecycle structural validation only; no completed or correct repair is established",
        "rerun": RERUN_COMMAND,
    });
    serde_json::to_string_pretty(&document)
        .map_err(|error| format!("failed to render python-repair-trust check JSON: {error}"))
}

fn render_check_markdown(outcome: &CheckOutcome, verdict: Verdict) -> String {
    let mut out = String::new();
    out.push_str("# Python Repair Trust Check\n\n");
    out.push_str(&format!("Verdict: **{}**\n\n", verdict.as_str()));
    out.push_str(
        "Selection and lifecycle structural validation only — no completed or correct repair is established.\n\n",
    );
    match &outcome.manifest {
        None => out.push_str(&format!(
            "- manifest: none at `{}` (the denominator is not established; not_run)\n",
            outcome.manifest_path
        )),
        Some(manifest) => {
            out.push_str(&format!(
                "- manifest: {} ({} selections, sha256 `{}`)\n",
                outcome.manifest_path,
                manifest.selections.len(),
                manifest.sha256
            ));
            for selection in &manifest.selections {
                out.push_str(&format!(
                    "  - {} (stratum `{}`)\n",
                    selection.attempt_id, selection.diversity_stratum
                ));
            }
        }
    }
    if outcome.envelopes.is_empty() {
        out.push_str(
            "- attempts: none supplied (`not_run`; not a pass — supply --attempts <dir-or-file> to validate attempt rows)\n",
        );
    } else {
        let total_rows: usize = outcome
            .envelopes
            .iter()
            .map(|envelope| envelope.rows.len())
            .sum();
        out.push_str(&format!(
            "- attempts: {} envelope(s), {total_rows} row(s)\n",
            outcome.envelopes.len()
        ));
        for envelope in &outcome.envelopes {
            let envelope_verdict = if envelope.rows.is_empty() {
                Verdict::NotRun
            } else if envelope.incomplete.is_empty() {
                Verdict::Valid
            } else {
                Verdict::Incomplete
            };
            out.push_str(&format!(
                "  - {} rows={} verdict={}\n",
                envelope.display,
                envelope.rows.len(),
                envelope_verdict.as_str()
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

/// The command entries: `check` (the corpus validator) and `check-driver`
/// (the #3569 driver binding validator).
pub(crate) fn python_repair_trust(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("check") => run_check(&args[1..]),
        Some("check-driver") => super::python_repair_driver::run_check_driver(&args[1..]),
        Some("check-verification") => {
            super::python_repair_verification::run_check_verification(&args[1..])
        }
        Some(other) => Err(format!(
            "unknown python-repair-trust subcommand: {other}\nusage: cargo xtask python-repair-trust check [--manifest <path>] [--attempts <dir-or-file>]\nrerun: {RERUN_COMMAND}"
        )),
        None => Err(format!(
            "python-repair-trust requires a subcommand\nusage: cargo xtask python-repair-trust check [--manifest <path>] [--attempts <dir-or-file>]\nrerun: {RERUN_COMMAND}"
        )),
    }
}

// ---------------------------------------------------------------------------
// Tests (module named `python_repair_trust_semantics` under the file module
// `python_repair_trust`, so `cargo test -p xtask python_repair_trust` selects
// exactly these tests; no unwrap/expect — assert macros and Result returns
// only)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod python_repair_trust_semantics {
    use super::*;

    const GIT_SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const GIT_SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const GIT_SHA_C: &str = "cccccccccccccccccccccccccccccccccccccccc";
    const DIGEST_ONE: &str = "1010101010101010101010101010101010101010101010101010101010101010";
    const DIGEST_TWO: &str = "2020202020202020202020202020202020202020202020202020202020202020";

    /// A complete, well-formed selection row: the recorded `selection_digest`
    /// is recomputed from the content, so the row is internally consistent.
    fn selection_json(
        attempt_id: &str,
        case_id: &str,
        subject_id: &str,
        stratum: &str,
        direction: &str,
        target_path: &str,
        target_state: &str,
    ) -> Value {
        let mut row = json!({
            "attempt_id": attempt_id,
            "case_id": case_id,
            "subject_id": subject_id,
            "repository": format!("https://example.com/{subject_id}"),
            "base": GIT_SHA_B,
            "head": GIT_SHA_C,
            "tree": DIGEST_ONE,
            "source_currentness": "candidate_current",
            "selection_reason": "behavior changed in the diff and the case discriminates it",
            "diversity_stratum": stratum,
            "family": "error_path_gating",
            "owner": "module.handler",
            "discriminator": "raises ValueError on empty payload",
            "relation": "case calls owner directly",
            "oracle": "pytest.raises exact message pin",
            "limitation": "none recorded",
            "expected_direction": direction,
            "claim_boundary": "static exposure evidence only",
            "target_path": target_path,
            "target_state": target_state,
            "selected_at": "2026-09-10T00:00:00Z",
            "selector": "campaign-selector",
            "authority_snapshot_digest": DIGEST_TWO,
        });
        if let Some(entry) = row.as_object_mut() {
            let digest = canonical_selection_digest(entry, attempt_id).unwrap_or_default();
            entry.insert("selection_digest".to_string(), json!(digest));
        }
        row
    }

    /// The alternate cohort: five selections across four strata and all three
    /// directions, including an ambiguous (wrong-target) and an unsafe
    /// (denied-surface) target that stay representable.
    fn alternate_selections() -> Vec<Value> {
        vec![
            selection_json(
                "att-alpha",
                "case-alpha",
                "subj-alpha",
                "pytest_library",
                "should_gap",
                "src/alpha/handler.py",
                "existing",
            ),
            selection_json(
                "att-bravo",
                "case-bravo",
                "subj-bravo",
                "click_typer",
                "should_stay_quiet",
                "src/bravo/cli.py",
                "proposed",
            ),
            selection_json(
                "att-charlie",
                "case-charlie",
                "subj-charlie",
                "fastapi_web",
                "should_limit",
                "src/charlie/api.py",
                "ambiguous",
            ),
            selection_json(
                "att-delta",
                "case-delta",
                "subj-delta",
                "flask_web",
                "should_gap",
                "vendor/lib/ext.py",
                "unsafe",
            ),
            selection_json(
                "att-echo",
                "case-echo",
                "subj-echo",
                "flask_web",
                "should_gap",
                "src/echo/service.py",
                "existing",
            ),
            // The refresh attempt is itself selected: the denominator is
            // fixed before outcomes, and a refresh wave re-accepts the
            // manifest with the new attempt identity included.
            selection_json(
                "att-echo-2",
                "case-echo",
                "subj-echo",
                "flask_web",
                "should_gap",
                "src/echo/service.py",
                "existing",
            ),
        ]
    }

    fn manifest_value(selections: Vec<Value>) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "kind": MANIFEST_KIND,
            "spec": KNOWN_SPEC,
            "description": "alternate data-driven selection cohort",
            "selections": selections,
        })
    }

    /// Recomputes every `selection_digest` after a test mutates selection-row
    /// content, so the mutation tests the rule under examination instead of
    /// tripping the immutability anchor.
    fn recompute_selection_digests(value: &mut Value) {
        if let Some(selections) = value.get_mut("selections").and_then(Value::as_array_mut) {
            for row in selections.iter_mut() {
                if let Some(entry) = row.as_object_mut() {
                    let attempt_id = entry
                        .get("attempt_id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let digest = canonical_selection_digest(entry, &attempt_id).unwrap_or_default();
                    entry.insert("selection_digest".to_string(), json!(digest));
                }
            }
        }
    }

    /// The identity blocks an attempt row must carry once its lifecycle has
    /// reached the given states.
    fn attempt_identities(states_reached: &[&str]) -> Vec<(String, Value)> {
        let mut row = serde_json::Map::new();
        let reached = |state: &str| states_reached.contains(&state);
        if reached("started") {
            row.insert(
                "analyzer".to_string(),
                json!({"source_sha": GIT_SHA_A, "binary_digest": DIGEST_TWO}),
            );
            row.insert("config".to_string(), json!({"profile": "ripr-default"}));
            row.insert("input".to_string(), json!({"digest": DIGEST_ONE}));
        }
        if reached("edited") {
            row.insert("patch".to_string(), json!({"digest": DIGEST_TWO}));
        }
        if reached("verified") {
            row.insert(
                "command".to_string(),
                json!({"verification_command": "pytest -q tests/test_handler.py"}),
            );
            row.insert(
                "after_state".to_string(),
                json!({"tree_digest": DIGEST_ONE}),
            );
        }
        if reached("reviewed") || reached("accepted") {
            row.insert(
                "packet".to_string(),
                json!({"reference": "repair-packet:att"}),
            );
        }
        row.into_iter().collect()
    }

    fn with_identities(row: &mut Value, states_reached: &[&str]) {
        if let (Some(entry), identities) = (row.as_object_mut(), attempt_identities(states_reached))
        {
            for (key, value) in identities {
                entry.insert(key, value);
            }
        }
    }

    fn attempts_value(rows: Vec<Value>, manifest_sha: &str, aggregates: Value) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "kind": ATTEMPTS_KIND,
            "spec": KNOWN_SPEC,
            "manifest_digest": manifest_sha,
            "aggregates": aggregates,
            "attempts": rows,
        })
    }

    /// The full alternate corpus rows: every lifecycle shape the issue wants
    /// representable — a completed accepted attempt, a failed-verification
    /// attempt, an abandoned edit with an uncertain movement, a stale
    /// selection with a non-run execution, a stale historical record, and the
    /// refresh appended under a new identity through `supersedes`.
    fn alternate_attempt_rows() -> Vec<Value> {
        let mut rows = Vec::new();

        let mut alpha = json!({
            "attempt_id": "att-alpha",
            "states": ["selected", "eligible", "started", "edited", "verified", "reviewed", "accepted"],
            "movement": "improved",
            "execution": "passed",
        });
        with_identities(&mut alpha, &["started", "edited", "verified", "reviewed"]);
        rows.push(alpha);

        let mut bravo = json!({
            "attempt_id": "att-bravo",
            "states": ["selected", "started", "edited", "verified"],
            "movement": "unchanged",
            "execution": "failed",
        });
        with_identities(&mut bravo, &["started", "edited", "verified"]);
        rows.push(bravo);

        let mut charlie = json!({
            "attempt_id": "att-charlie",
            "states": ["selected", "eligible", "started", "edited", "abandoned"],
            "movement": "uncertain",
            "execution": "cancelled",
        });
        with_identities(&mut charlie, &["started", "edited"]);
        rows.push(charlie);

        rows.push(json!({
            "attempt_id": "att-delta",
            "states": ["selected", "stale"],
            "execution": "not_run",
        }));

        rows.push(json!({
            "attempt_id": "att-echo",
            "states": ["selected", "eligible", "stale"],
            "execution": "unavailable",
        }));

        let mut echo_refresh = json!({
            "attempt_id": "att-echo-2",
            "states": ["selected", "started", "edited"],
            "supersedes": "att-echo",
            "movement": "limited",
        });
        with_identities(&mut echo_refresh, &["started", "edited"]);
        rows.push(echo_refresh);

        rows
    }

    /// Aggregates derived by hand over `alternate_attempt_rows` — the honest
    /// totals the validator must reproduce exactly.
    fn alternate_aggregates(selection_count: usize) -> Value {
        json!({
            "selected_denominator": selection_count,
            "attempts_total": 6,
            "lifecycle_counts": {
                "accepted": 1,
                "verified": 1,
                "abandoned": 1,
                "stale": 2,
                "edited": 1,
            },
            "movement_counts": {
                "improved": 1,
                "unchanged": 1,
                "uncertain": 1,
                "limited": 1,
            },
            "execution_counts": {
                "passed": 1,
                "failed": 1,
                "cancelled": 1,
                "not_run": 1,
                "unavailable": 1,
            },
            "achieved_strata": {
                "pytest_library": 1,
                "click_typer": 1,
                "fastapi_web": 1,
                "flask_web": 3,
            },
            "stratum_floor": {
                "flask_web": 3,
            },
            "stratum_floor_met": true,
        })
    }

    fn parsed(value: &Value) -> Result<Value, String> {
        let text = serde_json::to_string_pretty(value)
            .map_err(|error| format!("serialize test JSON: {error}"))?;
        parse_json_without_duplicate_keys(&text)
            .map_err(|error| format!("test JSON must parse: {error}"))
    }

    fn accepted_manifest(value: &Value) -> Result<(SelectionManifest, String), String> {
        let text = serde_json::to_string_pretty(value)
            .map_err(|error| format!("serialize test JSON: {error}"))?;
        let sha = sha256_hex(text.as_bytes());
        let value = parse_json_without_duplicate_keys(&text)
            .map_err(|error| format!("test JSON must parse: {error}"))?;
        let manifest = validate_selection_manifest(&value, sha.clone())?;
        Ok((manifest, sha))
    }

    fn validate_manifest_value(value: &Value) -> Result<(), String> {
        validate_selection_manifest(value, String::new()).map(|_| ())
    }

    /// Validates an envelope end to end through the file-backed input path so
    /// directory/file handling, digest binding, and strict parsing all run.
    fn validate_envelope_value(
        envelope: &Value,
        manifest: &SelectionManifest,
        manifest_sha: &str,
    ) -> Result<Vec<AttemptsEnvelope>, String> {
        let text = serde_json::to_string_pretty(envelope)
            .map_err(|error| format!("serialize test JSON: {error}"))?;
        let dir = std::env::temp_dir().join(format!(
            "ripr-pyrt-envelope-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|error| format!("create dir: {error}"))?;
        let path = dir.join("attempts.json");
        std::fs::write(&path, &text).map_err(|error| format!("write envelope: {error}"))?;
        let result =
            validate_attempts_input(path.to_string_lossy().as_ref(), manifest, manifest_sha);
        let _ = std::fs::remove_dir_all(&dir);
        result
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

    fn full_valid_envelope() -> Result<(SelectionManifest, String, Value), String> {
        let manifest_value = manifest_value(alternate_selections());
        let (manifest, sha) = accepted_manifest(&manifest_value)?;
        let envelope = attempts_value(
            alternate_attempt_rows(),
            &sha,
            alternate_aggregates(manifest.selections.len()),
        );
        Ok((manifest, sha, envelope))
    }

    // -- data-driven acceptance --------------------------------------------

    #[test]
    fn accepts_alternate_valid_corpus_not_fixture_bytes() -> Result<(), String> {
        let (manifest, sha, envelope) = full_valid_envelope()?;
        assert_eq!(manifest.selections.len(), 6);
        assert!(manifest.selection("att-alpha").is_some());
        assert!(manifest.selection("att-echo-2").is_some());
        assert!(manifest.selection("att-nowhere").is_none());
        assert!(
            manifest.incomplete.is_empty(),
            "the identity-complete cohort discloses nothing: {:?}",
            manifest.incomplete
        );
        let envelopes = validate_envelope_value(&envelope, &manifest, &sha)?;
        assert_eq!(envelopes.len(), 1);
        assert_eq!(envelopes[0].rows.len(), 6);
        Ok(())
    }

    #[test]
    fn accepts_a_second_different_cohort() -> Result<(), String> {
        // A different cohort (different ids, strata, directions, two rows)
        // with honestly derived aggregates validates the same way: the
        // validator is data-driven, not pinned to the first cohort. The full
        // owned aggregate set is required with rows, so this cohort carries
        // every field too — an empty floor met vacuously, reported met.
        let selections = vec![
            selection_json(
                "w1",
                "c1",
                "s1",
                "unittest_library",
                "should_gap",
                "pkg/mod.py",
                "existing",
            ),
            selection_json(
                "w2",
                "c2",
                "s2",
                "unittest_library",
                "should_limit",
                "pkg/other.py",
                "existing",
            ),
        ];
        let manifest_value = manifest_value(selections);
        let (manifest, sha) = accepted_manifest(&manifest_value)?;
        let rows = vec![
            json!({
                "attempt_id": "w1",
                "states": ["selected", "eligible"],
            }),
            json!({
                "attempt_id": "w2",
                "states": ["selected", "rejected"],
            }),
        ];
        let aggregates = json!({
            "selected_denominator": 2,
            "attempts_total": 2,
            "lifecycle_counts": {"eligible": 1, "rejected": 1},
            "movement_counts": {},
            "execution_counts": {},
            "achieved_strata": {"unittest_library": 2},
            "stratum_floor": {},
            "stratum_floor_met": true,
        });
        let envelope = attempts_value(rows, &sha, aggregates);
        let envelopes = validate_envelope_value(&envelope, &manifest, &sha)?;
        assert_eq!(envelopes[0].rows.len(), 2);
        Ok(())
    }

    #[test]
    fn check_artifacts_end_to_end_in_temp_dir() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-pyrt-check-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|error| format!("create dir: {error}"))?;
        let manifest_text = serde_json::to_string_pretty(&manifest_value(alternate_selections()))
            .map_err(|error| error.to_string())?;
        let manifest_path = dir.join("manifest.json");
        std::fs::write(&manifest_path, &manifest_text)
            .map_err(|error| format!("write manifest: {error}"))?;
        let manifest_sha = sha256_hex(manifest_text.as_bytes());
        let envelope = attempts_value(
            alternate_attempt_rows(),
            &manifest_sha,
            alternate_aggregates(6),
        );
        let envelope_text =
            serde_json::to_string_pretty(&envelope).map_err(|error| error.to_string())?;
        let attempts_path = dir.join("attempts.json");
        std::fs::write(&attempts_path, &envelope_text)
            .map_err(|error| format!("write attempts: {error}"))?;

        let outcome = check_artifacts(
            manifest_path.to_string_lossy().as_ref(),
            true,
            Some(attempts_path.to_string_lossy().as_ref()),
        );
        let _ = std::fs::remove_dir_all(&dir);

        let outcome = outcome?;
        assert_eq!(outcome.verdict(), Verdict::Valid);
        Ok(())
    }

    // -- no corpus yet -------------------------------------------------------

    #[test]
    fn no_corpus_at_default_path_is_not_run_exit_zero() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-pyrt-empty-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|error| format!("create dir: {error}"))?;
        let missing = dir.join("manifest.json");
        let outcome = check_artifacts(missing.to_string_lossy().as_ref(), false, None);
        let _ = std::fs::remove_dir_all(&dir);
        let outcome = outcome?;
        assert_eq!(outcome.verdict(), Verdict::NotRun);
        assert_eq!(outcome.verdict().as_str(), "not_run");
        Ok(())
    }

    #[test]
    fn zero_row_envelope_is_not_run_never_a_pass() -> Result<(), String> {
        let manifest_value = manifest_value(alternate_selections());
        let (manifest, sha) = accepted_manifest(&manifest_value)?;
        let envelope = attempts_value(
            Vec::new(),
            &sha,
            json!({
                "selected_denominator": 6,
                "attempts_total": 0,
                "lifecycle_counts": {},
                "movement_counts": {},
                "execution_counts": {},
            }),
        );
        let envelopes = validate_envelope_value(&envelope, &manifest, &sha)?;
        assert_eq!(envelopes[0].rows.len(), 0);
        let outcome = CheckOutcome {
            manifest_path: "manifest.json".to_string(),
            manifest: Some(manifest.clone()),
            attempts_input: Some("attempts.json".to_string()),
            envelopes,
        };
        assert_eq!(outcome.verdict(), Verdict::NotRun);

        // A zero-row envelope claiming a nonzero aggregate is fabricated.
        let envelope = attempts_value(
            Vec::new(),
            &sha,
            json!({
                "attempts_total": 3,
                "lifecycle_counts": {"accepted": 3},
                "movement_counts": {},
                "execution_counts": {},
            }),
        );
        expect_fail(
            validate_envelope_value(&envelope, &manifest, &sha).map(|_| ()),
            "hand-edited aggregate",
        )
    }

    // -- manifest failure shapes --------------------------------------------

    #[test]
    fn rejects_manifest_with_wrong_kind() -> Result<(), String> {
        let mut value = manifest_value(alternate_selections());
        value["kind"] = json!("some_other_manifest");
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "expected `python_repair_trust_manifest`",
        )
    }

    #[test]
    fn rejects_duplicate_attempt_identities_in_manifest() -> Result<(), String> {
        let mut selections = alternate_selections();
        if let Some(first) = selections.first().cloned() {
            selections[1] = first;
        }
        expect_fail(
            validate_manifest_value(&parsed(&manifest_value(selections))?),
            "duplicate attempt identity",
        )
    }

    #[test]
    fn rejects_selection_digest_replacement_and_edit() -> Result<(), String> {
        // Replacing the recorded digest (a tampered row) fails.
        let mut value = manifest_value(alternate_selections());
        value["selections"][0]["selection_digest"] = json!(DIGEST_ONE);
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "selection row digest mismatch",
        )?;

        // Editing any content field without recomputing the digest fails the
        // same way — selected rows are immutable once outcomes exist.
        let mut value = manifest_value(alternate_selections());
        value["selections"][0]["head"] = json!(GIT_SHA_A);
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "selection row digest mismatch",
        )
    }

    #[test]
    fn rejects_unknown_vocabularies() -> Result<(), String> {
        for (field, wrong, needle) in [
            ("target_state", json!("forbidden"), "unknown target state"),
            (
                "expected_direction",
                json!("should_score_high"),
                "unknown expected direction",
            ),
            (
                "source_currentness",
                json!("vibes"),
                "unknown source-currentness disposition",
            ),
        ] {
            let mut value = manifest_value(alternate_selections());
            value["selections"][0][field] = wrong;
            expect_fail(validate_manifest_value(&parsed(&value)?), needle)?;
        }
        Ok(())
    }

    #[test]
    fn rejects_missing_required_selection_fields() -> Result<(), String> {
        for field in [
            "case_id",
            "subject_id",
            "family",
            "owner",
            "discriminator",
            "relation",
            "oracle",
            "claim_boundary",
            "selector",
            "authority_snapshot_digest",
        ] {
            let mut value = manifest_value(alternate_selections());
            if let Some(entry) = value["selections"][0].as_object_mut() {
                entry.remove(field);
            }
            expect_fail(
                validate_manifest_value(&parsed(&value)?),
                &format!("field=`{field}`"),
            )?;
        }
        Ok(())
    }

    #[test]
    fn rejects_unsafe_paths_and_secret_bearing_values() -> Result<(), String> {
        let mut value = manifest_value(alternate_selections());
        value["selections"][0]["target_path"] = json!("/etc/passwd");
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "repo-relative, not absolute",
        )?;

        let mut value = manifest_value(alternate_selections());
        value["selections"][0]["target_path"] = json!("../escape.py");
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "must not contain `..`",
        )?;

        let mut value = manifest_value(alternate_selections());
        value["selections"][0]["repository"] = json!("https://user:pass@example.com/x");
        expect_fail(validate_manifest_value(&parsed(&value)?), "credentials")?;

        let mut value = manifest_value(alternate_selections());
        value["selections"][0]["selection_reason"] = json!("found the api_key hole");
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "secret-shaped token",
        )
    }

    #[test]
    fn denied_edit_surface_must_be_declared_unsafe() -> Result<(), String> {
        let mut value = manifest_value(alternate_selections());
        value["selections"][3]["target_path"] = json!("vendor/lib/ext.py");
        value["selections"][3]["target_state"] = json!("existing");
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "must be declared `unsafe`",
        )?;

        // The same denied path declared unsafe validates.
        let value = manifest_value(alternate_selections());
        validate_manifest_value(&parsed(&value)?)?;

        // A generated-file marker counts as a denied surface too.
        let mut value = manifest_value(alternate_selections());
        value["selections"][3]["target_path"] = json!("src/schema.generated.py");
        value["selections"][3]["target_state"] = json!("existing");
        expect_fail(
            validate_manifest_value(&parsed(&value)?),
            "must be declared `unsafe`",
        )
    }

    #[test]
    fn seam_kind_appears_nowhere_in_a_corpus() -> Result<(), String> {
        let mut value = manifest_value(alternate_selections());
        value["selections"][0]["relation"] = json!("SeamKind::ErrorPath probe relation");
        expect_fail(validate_manifest_value(&parsed(&value)?), "SeamKind")
    }

    #[test]
    fn absent_optional_selection_identities_disclose_incomplete() -> Result<(), String> {
        let mut value = manifest_value(alternate_selections());
        if let Some(entry) = value["selections"][0].as_object_mut() {
            for field in ["tree", "source_currentness", "limitation"] {
                entry.remove(field);
            }
        }
        recompute_selection_digests(&mut value);
        let manifest = validate_selection_manifest(&parsed(&value)?, String::new())?;
        let fields: Vec<String> = manifest
            .incomplete
            .iter()
            .map(|diagnostic| diagnostic.field.clone())
            .collect();
        for field in ["tree", "source_currentness", "limitation"] {
            assert!(
                fields.contains(&field.to_string()),
                "missing `{field}` must be typed incomplete, got: {fields:?}"
            );
        }
        // Present-but-null stays garbage, not absence.
        let mut value = manifest_value(alternate_selections());
        value["selections"][0]["tree"] = Value::Null;
        expect_fail(validate_manifest_value(&parsed(&value)?), "explicitly null")
    }

    // -- attempts failure shapes ---------------------------------------------

    #[test]
    fn rejects_attempt_outside_the_selection_denominator() -> Result<(), String> {
        let (manifest, sha, mut envelope) = full_valid_envelope()?;
        if let Some(rows) = envelope.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert("attempt_id".to_string(), json!("att-ghost"));
        }
        expect_fail(
            validate_envelope_value(&envelope, &manifest, &sha).map(|_| ()),
            "outside the accepted selection denominator",
        )
    }

    #[test]
    fn rejects_stale_manifest_digest() -> Result<(), String> {
        let (manifest, sha, envelope) = full_valid_envelope()?;
        let mut envelope = envelope;
        if let Some(object) = envelope.as_object_mut() {
            object.insert("manifest_digest".to_string(), json!(DIGEST_ONE));
        }
        expect_fail(
            validate_envelope_value(&envelope, &manifest, &sha).map(|_| ()),
            "stale digest",
        )
    }

    #[test]
    fn rejects_duplicate_attempts_across_envelopes() -> Result<(), String> {
        let (manifest, sha, envelope) = full_valid_envelope()?;
        // Two envelopes carrying the same attempt identity: the historical
        // record was mutated instead of appended under a new identity.
        let text = serde_json::to_string_pretty(&envelope).map_err(|error| error.to_string())?;
        let dir = std::env::temp_dir().join(format!(
            "ripr-pyrt-dup-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|error| format!("create dir: {error}"))?;
        std::fs::write(dir.join("a.json"), &text).map_err(|error| format!("write a: {error}"))?;
        std::fs::write(dir.join("b.json"), &text).map_err(|error| format!("write b: {error}"))?;
        let result = validate_attempts_input(dir.to_string_lossy().as_ref(), &manifest, &sha);
        let _ = std::fs::remove_dir_all(&dir);
        expect_fail(result.map(|_| ()), "duplicate attempt identity")
    }

    #[test]
    fn rejects_contradictory_lifecycle_states() -> Result<(), String> {
        let (manifest, sha, envelope) = full_valid_envelope()?;

        // Discard terminal + completed lifecycle state: the attempt reaches
        // verified through the full ordered progression, then goes stale.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[4].as_object_mut()
        {
            entry.insert(
                "states".to_string(),
                json!(["selected", "started", "edited", "verified", "stale"]),
            );
            entry.insert("execution".to_string(), json!("passed"));
            with_identities_from(entry, &["started", "edited", "verified"]);
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "cannot appear completed",
        )?;

        // Discard terminal + completed movement: charlie's abandoned attempt
        // has edited, so flipping its movement to a completed one is exactly
        // the completed-after-discard shape.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[2].as_object_mut()
        {
            entry.insert("movement".to_string(), json!("improved"));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "cannot appear completed",
        )?;

        // Progress ordering: edited before started.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert(
                "states".to_string(),
                json!(["selected", "edited", "started"]),
            );
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "state machine ordering",
        )?;

        // verified requires edited.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert(
                "states".to_string(),
                json!(["selected", "started", "verified"]),
            );
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "requires `edited`",
        )?;

        // A lifecycle that does not start at selected.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert("states".to_string(), json!(["started", "edited"]));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "must start at `selected`",
        )?;

        // Repeated state.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert(
                "states".to_string(),
                json!(["selected", "started", "started", "edited"]),
            );
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "repeats",
        )
    }

    /// Merges the transition identities for `states_reached` into an existing
    /// attempt-row object.
    fn with_identities_from(entry: &mut serde_json::Map<String, Value>, states_reached: &[&str]) {
        for (key, value) in attempt_identities(states_reached) {
            entry.insert(key, value);
        }
    }

    #[test]
    fn movement_and_execution_cannot_imply_one_another() -> Result<(), String> {
        let (manifest, sha, envelope) = full_valid_envelope()?;

        // A run claiming static movement without an edit: the movement is
        // being implied by the execution.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[4].as_object_mut()
        {
            entry.insert(
                "states".to_string(),
                json!(["selected", "started", "stale"]),
            );
            entry.insert("execution".to_string(), json!("unavailable"));
            entry.insert("movement".to_string(), json!("improved"));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "cannot be implied by selection or by a verification run",
        )?;

        // A verdict execution recorded without the verified lifecycle state.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[1].as_object_mut()
        {
            entry.insert(
                "states".to_string(),
                json!(["selected", "started", "edited"]),
            );
            entry.insert("execution".to_string(), json!("passed"));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "requires the lifecycle to have reached `verified`",
        )?;

        // A claimed verified state whose execution produced no verdict.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[1].as_object_mut()
        {
            entry.insert(
                "states".to_string(),
                json!(["selected", "started", "edited", "verified"]),
            );
            entry.insert("execution".to_string(), json!("timed_out"));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "records no completed verification verdict",
        )?;

        // The representable pairings stay valid: a failed execution with a
        // regressed movement, and (in the base corpus) a passed execution
        // whose movement is its own static observation.
        let mut representable = envelope;
        if let Some(rows) = representable
            .get_mut("attempts")
            .and_then(Value::as_array_mut)
            && let Some(entry) = rows[1].as_object_mut()
        {
            entry.insert("movement".to_string(), json!("regressed"));
        }
        if let Some(aggregates) = representable
            .get_mut("aggregates")
            .and_then(Value::as_object_mut)
            && let Some(entries) = aggregates
                .get_mut("movement_counts")
                .and_then(Value::as_object_mut)
        {
            entries.remove("unchanged");
            entries.insert("regressed".to_string(), json!(1));
        }
        let envelopes = validate_envelope_value(&representable, &manifest, &sha)?;
        assert_eq!(envelopes[0].rows[1].movement.as_deref(), Some("regressed"));
        assert_eq!(envelopes[0].rows[1].execution.as_deref(), Some("failed"));
        Ok(())
    }

    #[test]
    fn missing_required_identities_at_transitions_fail() -> Result<(), String> {
        let (manifest, sha, envelope) = full_valid_envelope()?;
        let cases: [(&str, usize, &str); 5] = [
            (
                "analyzer",
                0,
                "reaching `started` requires the analyzer identity",
            ),
            ("patch", 1, "reaching `edited` requires the patch identity"),
            (
                "command",
                1,
                "reaching `verified` requires the command identity",
            ),
            (
                "after_state",
                1,
                "reaching `verified` requires the after_state identity",
            ),
            (
                "packet",
                0,
                "reaching `reviewed` requires the packet identity",
            ),
        ];
        for (block, row_index, needle) in cases {
            let mut broken = envelope.clone();
            if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
                && let Some(entry) = rows[row_index].as_object_mut()
            {
                entry.remove(block);
            }
            expect_fail(
                validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
                needle,
            )?;
        }

        // A present block with the required field missing fails with the
        // field named.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[1].as_object_mut()
        {
            entry.insert("patch".to_string(), json!({}));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "field=`patch.digest`",
        )?;

        // A malformed identity value fails too.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[1].as_object_mut()
        {
            entry.insert("patch".to_string(), json!({"digest": "nothex"}));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "sha256 hex",
        )
    }

    #[test]
    fn supplied_identity_blocks_are_shape_validated_before_their_transition() -> Result<(), String>
    {
        let (manifest, sha, envelope) = full_valid_envelope()?;

        // An unknown field inside a supplied block fails even though the
        // row's lifecycle has not reached the block's transition yet: schema
        // rot is rejected at any lifecycle position, not only at `started`.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[3].as_object_mut()
        {
            entry.insert(
                "analyzer".to_string(),
                json!({"source_sha": GIT_SHA_A, "binary_digest": DIGEST_TWO, "hostname": "build-agent-1"}),
            );
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "unknown field `hostname`",
        )?;

        // A malformed value supplied before its transition fails the same
        // way: well-formed value types are part of the shape contract.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[3].as_object_mut()
        {
            entry.insert("patch".to_string(), json!({"digest": "nothex"}));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "sha256 hex",
        )?;

        // A well-formed early block stays representable: att-delta never
        // reaches `started`, and the well-formed identity records validate
        // without demanding their transition.
        let mut early = envelope;
        if let Some(rows) = early.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[3].as_object_mut()
        {
            entry.insert(
                "analyzer".to_string(),
                json!({"source_sha": GIT_SHA_A, "binary_digest": DIGEST_TWO}),
            );
            entry.insert("config".to_string(), json!({"profile": "ripr-default"}));
            entry.insert("input".to_string(), json!({"digest": DIGEST_ONE}));
        }
        let envelopes = validate_envelope_value(&early, &manifest, &sha)?;
        assert_eq!(envelopes[0].rows[3].final_state, "stale");
        Ok(())
    }

    #[test]
    fn unsafe_edit_surface_attempted_fails() -> Result<(), String> {
        let (manifest, sha, envelope) = full_valid_envelope()?;

        // The declared-unsafe selection (att-delta) gains an edit.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[3].as_object_mut()
        {
            entry.insert(
                "states".to_string(),
                json!(["selected", "started", "edited", "stale"]),
            );
            entry.insert("patch".to_string(), json!({"digest": DIGEST_TWO}));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "attempted edit on an unsafe target",
        )?;

        // Not attempting the unsafe target stays representable: the stale
        // delta row with a not_run execution validates (pinned by the base
        // corpus test above).
        Ok(())
    }

    #[test]
    fn supersedes_violations_fail_and_valid_refresh_appends() -> Result<(), String> {
        let (manifest, sha, envelope) = full_valid_envelope()?;

        // Superseding a record that did not end stale.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[5].as_object_mut()
        {
            entry.insert("supersedes".to_string(), json!("att-alpha"));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "only a stale record is refreshed",
        )?;

        // Superseding an unknown identity.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[5].as_object_mut()
        {
            entry.insert("supersedes".to_string(), json!("att-nowhere"));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "does not retain",
        )?;

        // Two records superseding the same stale record: the stale delta row
        // also claims the att-echo refresh.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[3].as_object_mut()
        {
            entry.insert("supersedes".to_string(), json!("att-echo"));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "exactly one current successor",
        )?;

        // The valid refresh shape (att-echo-2 supersedes the stale att-echo)
        // is pinned by the base corpus test.

        // A `supersedes` cycle is impossible history (every link in a cycle
        // claims to refresh a stale record, yet nothing in the cycle could
        // have been stale first). The chain walk is pinned directly: a
        // two-link cycle A→B→A revisits A; an acyclic chain terminates.
        let cyclic: BTreeMap<String, String> = BTreeMap::from([
            ("att-a".to_string(), "att-b".to_string()),
            ("att-b".to_string(), "att-a".to_string()),
        ]);
        assert_eq!(
            refresh_chain_revisits(&cyclic, "att-a").as_deref(),
            Some("att-a")
        );
        assert_eq!(
            refresh_chain_revisits(&cyclic, "att-b").as_deref(),
            Some("att-b")
        );
        let acyclic: BTreeMap<String, String> = BTreeMap::from([
            ("att-a".to_string(), "att-b".to_string()),
            ("att-b".to_string(), "att-c".to_string()),
        ]);
        assert_eq!(refresh_chain_revisits(&acyclic, "att-a"), None);
        Ok(())
    }

    // -- denominators ---------------------------------------------------------

    #[test]
    fn oversized_denominator_cannot_truncate_onto_the_true_count() -> Result<(), String> {
        let (manifest, sha, envelope) = full_valid_envelope()?;
        // u64 value that would truncate to the manifest's selection count (6)
        // under a 32-bit `value as usize` cast (6 + 2^32).
        let truncated = (manifest.selections.len() as u64) + (1u64 << 32);
        let mut broken = envelope.clone();
        if let Some(aggregates) = broken.get_mut("aggregates").and_then(Value::as_object_mut) {
            aggregates.insert("selected_denominator".to_string(), json!(truncated));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "hand-edited aggregate",
        )?;
        Ok(())
    }

    // -- aggregates -----------------------------------------------------------

    #[test]
    fn rejects_hand_edited_aggregates() -> Result<(), String> {
        let (manifest, sha, envelope) = full_valid_envelope()?;
        for (field, wrong) in [
            ("selected_denominator", json!(4)),
            ("attempts_total", json!(7)),
        ] {
            let mut broken = envelope.clone();
            if let Some(aggregates) = broken.get_mut("aggregates").and_then(Value::as_object_mut) {
                aggregates.insert(field.to_string(), wrong);
            }
            expect_fail(
                validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
                "hand-edited aggregate",
            )?;
        }

        // Every count map fails on a wrong value.
        let map_cases = [
            ("lifecycle_counts", "accepted", json!(2)),
            ("movement_counts", "improved", json!(0)),
            ("execution_counts", "not_run", json!(5)),
            ("achieved_strata", "flask_web", json!(2)),
        ];
        for (map, key, wrong) in map_cases {
            let mut broken = envelope.clone();
            if let Some(aggregates) = broken.get_mut("aggregates").and_then(Value::as_object_mut)
                && let Some(entries) = aggregates.get_mut(map).and_then(Value::as_object_mut)
            {
                entries.insert(key.to_string(), wrong);
            }
            expect_fail(
                validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
                "hand-edited aggregate",
            )?;
        }

        // A fabricated key the rows never establish fails too.
        let mut broken = envelope.clone();
        if let Some(aggregates) = broken.get_mut("aggregates").and_then(Value::as_object_mut)
            && let Some(entries) = aggregates
                .get_mut("movement_counts")
                .and_then(Value::as_object_mut)
        {
            entries.insert("closed".to_string(), json!(1));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "never establish",
        )
    }

    #[test]
    fn missing_required_aggregates_with_rows_fail() -> Result<(), String> {
        let (manifest, sha, envelope) = full_valid_envelope()?;

        // The full owned aggregate set validates: the pinned base corpus
        // shape carries every field the envelope owns.
        validate_envelope_value(&envelope, &manifest, &sha)?;

        // With rows present, every owned aggregate field is required — an
        // omitted field (the selected denominator or a diversity aggregate
        // included) would silently disable its row-agreement check.
        for field in AGGREGATES_KEYS {
            let mut broken = envelope.clone();
            if let Some(aggregates) = broken.get_mut("aggregates").and_then(Value::as_object_mut) {
                aggregates.remove(field);
            }
            expect_fail(
                validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
                &format!("aggregates.{field}"),
            )?;
        }
        Ok(())
    }

    #[test]
    fn stratum_floor_pretending_fails() -> Result<(), String> {
        let (manifest, sha, envelope) = full_valid_envelope()?;

        // A floor the cohort does not meet, reported as met: forbidden.
        let mut broken = envelope.clone();
        if let Some(aggregates) = broken.get_mut("aggregates").and_then(Value::as_object_mut) {
            aggregates.insert("stratum_floor".to_string(), json!({"click_typer": 3}));
            aggregates.insert("stratum_floor_met".to_string(), json!(true));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "without pretending the target floor was met",
        )?;

        // Claiming true with no floor recorded at all: forbidden. With rows
        // present the omission is itself the failure — the full owned
        // aggregate set is required, so the missing floor fails as a
        // required aggregate before any met-claim comparison.
        let mut broken = envelope.clone();
        if let Some(aggregates) = broken.get_mut("aggregates").and_then(Value::as_object_mut) {
            aggregates.remove("stratum_floor");
            aggregates.insert("stratum_floor_met".to_string(), json!(true));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "aggregates.stratum_floor",
        )?;

        // The honest disclosure validates: floor not met, reported unmet.
        let mut honest = envelope;
        if let Some(aggregates) = honest.get_mut("aggregates").and_then(Value::as_object_mut) {
            aggregates.insert("stratum_floor".to_string(), json!({"click_typer": 3}));
            aggregates.insert("stratum_floor_met".to_string(), json!(false));
        }
        validate_envelope_value(&honest, &manifest, &sha)?;

        // The met floor (flask_web >= 3) with stratum_floor_met true is the
        // pinned base corpus shape and validates.
        Ok(())
    }

    #[test]
    fn rejects_unknown_attempt_and_envelope_vocabulary() -> Result<(), String> {
        let (manifest, sha, envelope) = full_valid_envelope()?;
        for (field, wrong, needle) in [
            ("movement", json!("skyrocketed"), "unknown static movement"),
            (
                "execution",
                json!("vibes"),
                "unknown verification execution",
            ),
        ] {
            let mut broken = envelope.clone();
            if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
                && let Some(entry) = rows[0].as_object_mut()
            {
                entry.insert(field.to_string(), wrong);
            }
            expect_fail(
                validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
                needle,
            )?;
        }

        // Unknown lifecycle state.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert(
                "states".to_string(),
                json!(["selected", "mostly_worked", "accepted"]),
            );
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "unknown lifecycle state",
        )
    }

    #[test]
    fn rejects_duplicate_json_keys_at_load() {
        let body = format!(
            "{{\"kind\": \"a\", \"kind\": \"b\", \"schema_version\": \"{SCHEMA_VERSION}\", \"spec\": \"{KNOWN_SPEC}\", \"attempts\": [], \"manifest_digest\": \"{DIGEST_ONE}\", \"aggregates\": {{}}}}"
        );
        let parsed = parse_json_without_duplicate_keys(&body);
        assert!(parsed.is_err(), "duplicate keys must fail at load");
    }

    #[test]
    fn rejects_unknown_envelope_and_row_fields() -> Result<(), String> {
        let (manifest, sha, envelope) = full_valid_envelope()?;

        let mut broken = envelope.clone();
        if let Some(object) = broken.as_object_mut() {
            object.insert("notes".to_string(), json!("unexpected"));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "unknown field `notes`",
        )?;

        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert("verdict".to_string(), json!("flawless"));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "unknown field `verdict`",
        )?;

        // An unknown key inside an identity block fails as well.
        let mut broken = envelope.clone();
        if let Some(rows) = broken.get_mut("attempts").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
            && let Some(analyzer) = entry.get_mut("analyzer").and_then(Value::as_object_mut)
        {
            analyzer.insert("hostname".to_string(), json!("build-agent-1"));
        }
        expect_fail(
            validate_envelope_value(&broken, &manifest, &sha).map(|_| ()),
            "unknown field `hostname`",
        )
    }

    // -- verdict + rendering ---------------------------------------------------

    #[test]
    fn not_run_is_never_a_pass_in_verdict_vocabulary() -> Result<(), String> {
        let outcome = CheckOutcome {
            manifest_path: "manifest.json".to_string(),
            manifest: None,
            attempts_input: None,
            envelopes: Vec::new(),
        };
        assert_eq!(outcome.verdict(), Verdict::NotRun);
        assert_eq!(outcome.verdict().as_str(), "not_run");
        Ok(())
    }

    #[test]
    fn top_level_verdict_is_incomplete_when_selection_gaps_survive() -> Result<(), String> {
        // A cohort with absent optional identities: structurally valid rows,
        // disclosed incompletes, verdict incomplete — never a bare pass.
        let mut value = manifest_value(alternate_selections());
        if let Some(entry) = value["selections"][0].as_object_mut() {
            entry.remove("tree");
        }
        recompute_selection_digests(&mut value);
        let (manifest, sha) = accepted_manifest(&value)?;
        assert!(!manifest.incomplete.is_empty());
        let envelope = attempts_value(
            alternate_attempt_rows(),
            &sha,
            alternate_aggregates(manifest.selections.len()),
        );
        let envelopes = validate_envelope_value(&envelope, &manifest, &sha)?;
        let outcome = CheckOutcome {
            manifest_path: "manifest.json".to_string(),
            manifest: Some(manifest),
            attempts_input: Some("attempts.json".to_string()),
            envelopes,
        };
        assert_eq!(outcome.verdict(), Verdict::Incomplete);
        assert_eq!(outcome.verdict().as_str(), "incomplete");
        Ok(())
    }

    #[test]
    fn top_level_verdict_is_valid_only_with_zero_incompletes() -> Result<(), String> {
        let (manifest, sha, envelope) = full_valid_envelope()?;
        let envelopes = validate_envelope_value(&envelope, &manifest, &sha)?;
        let outcome = CheckOutcome {
            manifest_path: "manifest.json".to_string(),
            manifest: Some(manifest),
            attempts_input: Some("attempts.json".to_string()),
            envelopes,
        };
        assert_eq!(outcome.verdict(), Verdict::Valid);
        assert_eq!(outcome.verdict().as_str(), "valid");
        Ok(())
    }

    #[test]
    fn check_report_json_is_stable_and_versioned() -> Result<(), String> {
        let (manifest, sha, envelope) = full_valid_envelope()?;
        let envelopes = validate_envelope_value(&envelope, &manifest, &sha)?;
        let outcome = CheckOutcome {
            manifest_path: "manifest.json".to_string(),
            manifest: Some(manifest),
            attempts_input: Some("attempts.json".to_string()),
            envelopes,
        };
        let rendered_a = render_check_json(&outcome);
        let rendered_b = render_check_json(&outcome);
        let text_a = rendered_a?;
        assert_eq!(
            Ok(text_a.clone()),
            rendered_b,
            "check JSON must be deterministic"
        );
        assert!(text_a.contains("\"kind\": \"python_repair_trust_check_report\""));
        assert!(text_a.contains("\"offline\": true"));
        assert!(
            text_a.contains("\"verdict\": \"valid\"")
                || text_a.contains("\"verdict\": \"incomplete\"")
        );
        Ok(())
    }

    #[test]
    fn diagnostics_name_subject_field_reason_and_rerun() -> Result<(), String> {
        let mut value = manifest_value(alternate_selections());
        value["selections"][2]["oracle"] = json!("");
        let error = match validate_manifest_value(&parsed(&value)?) {
            Ok(()) => return Err("empty oracle must fail the manifest".to_string()),
            Err(error) => error,
        };
        for needle in ["subject=`att-charlie`", "field=`oracle`", RERUN_COMMAND] {
            assert!(
                error.contains(needle),
                "diagnostic `{error}` must mention `{needle}`"
            );
        }
        Ok(())
    }
}
