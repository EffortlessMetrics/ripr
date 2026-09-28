//! Accepted-receipt publication and the mechanical currentness gate —
//! `cargo xtask eval-sweep report` (RIPR-SPEC-0086, issue #3567).
//!
//! A candidate sweep (#3566) is not accepted evidence until its rows,
//! identities, aggregates, non-complete dispositions, and currentness are
//! independently checked and projected through one durable receipt. This route
//! is that promotion step, deterministic and offline:
//!
//! - `eval-sweep report --candidate <receipt.json> [--dispositions <path>]`
//!   validates a schema-0.3 candidate through the exact `eval-sweep check`
//!   semantics (`eval_sweep_check::validate_run_receipt` — one validator owns
//!   receipt semantics; report never re-implements them), then renders the
//!   accepted receipt JSON and a bounded Markdown report derived from the SAME
//!   validated rows as a dry run. No accepted state is written.
//! - `... --accept` additionally publishes the immutable accepted receipt under
//!   `<state-dir>/receipts/<receipt-sha256>.json` (content-addressed over the
//!   exact written bytes) together with the retained candidate
//!   (`receipts/<candidate-sha256>.candidate.json`, addressed by the
//!   candidate's own digest, so the currentness check can re-validate the
//!   rows through the shared validator) and atomically updates the current
//!   pointer `<state-dir>/current.json` to identify exactly that accepted
//!   receipt. Every accepted byte reaches its final path through the staged
//!   pattern (write a staging file in the same directory, flush, rename over
//!   the final path), so an interrupted run cannot leave truncated bytes
//!   under an accepted-artifact name. The self-addressed artifacts repair:
//!   identical bytes are an idempotent no-op, and a file whose bytes do not
//!   hash to its own digest address is not a valid prior artifact, so the
//!   atomic publication overwrites it with exactly the digest-named bytes
//!   (truncated bytes never block re-acceptance as "conflicting content").
//!   The Markdown copy is named by the receipt's digest, not by its own
//!   bytes, so it is not self-verifying against its address: an existing
//!   file with different bytes is a typed refusal, never silently repaired.
//! - `eval-sweep report --check-currentness` recomputes the identities the
//!   pointer binds against current state and reports the mechanical verdict:
//!   `current`, `stale`, `unverifiable`, or `not_run` (no pointer; never a
//!   pass). `current`/`unverifiable`/`not_run` exit 0 (with the non-current
//!   verdicts disclosed in full); `stale` exits nonzero — the gate signal
//!   that the accepted receipt must be re-accepted before promotion
//!   consumption.
//!
//! The current pointer contains NO independently editable totals — only
//! identity: the accepted receipt's digest and portable filename, an optional
//! as-of disclosure string, the manifest digest, the RIPR toolchain identity
//! block (source sha, binary digest, features, build profile), the
//! command-contract version, and per-subject bound identities (tree digest,
//! accepted-row digest, input digest, config identity). The currentness law
//! is mechanical: changed ripr source/binary bytes, a moved manifest, edited
//! accepted-row bytes, a moved subject tree pin, a substituted input path
//! (the bound config input must BE the manifest-declared input), moved
//! config/input bytes, or a different command contract flips the verdict to
//! `stale` — and editing
//! the as-of string can never repair it, because staleness derives only from
//! digest, binding, and vocabulary comparisons; as-of is never an input. The
//! manifest digest is compared over the raw file bytes BEFORE the manifest is
//! parsed or validated, so a moved manifest whose new bytes also fail
//! accepted-state validation reaches the promised `stale` verdict with both
//! reasons named (digest movement plus the validation failure) — a malformed
//! manifest never aborts the verdict path into a schema error.
//!
//! Dispositions are acceptance-time judgment and live OUTSIDE the closed 0.3
//! row schema (which is deny-unknown): a sidecar file maps each non-complete
//! subject to one typed terminal disposition with an evidence reference and
//! an owner/recovery route. Every disposition type in the owned vocabulary is
//! actionable by definition, so owner and recovery route are required on
//! every disposition — a disposition without them fails closed.
//!
//! Honesty boundaries, each load-bearing:
//!
//! - Language: the accepted receipt reports counts, distributions, and
//!   dispositions. It stays inside the conservative exposure vocabulary and
//!   never labels an outcome as caught, missed, exercised, or sufficient.
//!   Robustness and distribution metrics are informational and never become
//!   judged accuracy; the receipt's `non_claims` section states this inside
//!   the artifact itself.
//! - Every count is emitted as `{numerator, denominator}` with the
//!   denominator each contract defines — top-level counts over the subject
//!   denominator, outcomes and distribution buckets over the selected
//!   denominator, health tallies over the selected rows they tally, and the
//!   runtime count over the analyzed rows — no bare rate and no
//!   denominator-free number.
//! - Real producers only: what the validated rows record is projected; what
//!   they do not record is omitted (typed incomplete in the candidate's own
//!   disclosures, copied into the accepted receipt). A limitation
//!   distribution has no schema-0.3 row producer, so the accepted receipt
//!   carries a named disclosure instead of a fabricated taxonomy.
//! - Historical receipts stay immutable: a schema-0.2 candidate is refused
//!   with a typed refusal (it carries no currentness identities to bind), and
//!   acceptance never rewrites the retained historical receipt or any
//!   previously accepted artifact.
//! - Accepted-artifact hygiene: the rendered receipt, pointer, and Markdown
//!   are scanned before any byte is written — secret-shaped tokens (the
//!   shared validator tripwire list), absolute host paths, and oversized
//!   free-text notes fail closed.
//! - Claim boundary: acceptance establishes a current, reproducible
//!   operational-robustness denominator over the retained eight external
//!   Python subjects. It does not establish repair correctness and does not
//!   authorize any support-tier change.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::eval_sweep_check::{
    AcceptedManifest, RUN_STATUSES, load_strict_json, secret_tripwire_match, sha256_hex,
    validate_accepted_manifest, validate_run_receipt,
};
use super::eval_sweep_refresh::repo_root_anchor;

const DEFAULT_MANIFEST: &str = "fixtures/python-eval-sweep/manifest.json";
const DEFAULT_STATE_DIR: &str = "fixtures/python-eval-sweep/accepted";
const RERUN_COMMAND: &str = "cargo xtask eval-sweep report";
const USAGE: &str = "usage: cargo xtask eval-sweep report (--candidate <receipt.json> [--dispositions <path>] [--accept] | --check-currentness [--ripr-bin <path>] [--ripr-source-sha <sha>]) [--manifest <path>] [--state-dir <dir>] [--as-of <string>]";

const SPEC: &str = "RIPR-SPEC-0086";
const TIER: &str = "A";

/// Version of THIS command's artifact contract (accepted receipt + pointer
/// schemas and their derivation rules). The pointer binds it; a pointer
/// accepted under a different contract version is mechanically stale.
const CONTRACT_VERSION: &str = "1";

const ACCEPTED_SCHEMA: &str = "0.1";
const ACCEPTED_KIND: &str = "python_eval_sweep_accepted_receipt";
const POINTER_SCHEMA: &str = "0.1";
const POINTER_KIND: &str = "python_eval_sweep_current_pointer";
const DISPOSITIONS_SCHEMA: &str = "0.1";
const DISPOSITIONS_KIND: &str = "python_eval_sweep_dispositions";

const RECEIPTS_DIR: &str = "receipts";
const POINTER_FILE: &str = "current.json";

const REPORT_JSON: &str = "eval-sweep-report.json";
const REPORT_MD: &str = "eval-sweep-report.md";

/// The candidate schema acceptance binds currentness identities from. A 0.2
/// (historical) candidate carries none and is refused — historical receipts
/// remain valid retained artifacts, immutable and separately addressable.
const CANDIDATE_SCHEMA: &str = "0.3";

/// The closed current-pointer schema (deny-unknown). Identity only: any total
/// or rate field is schema rot and fails at the currentness read.
const POINTER_KEYS: [&str; 10] = [
    "schema_version",
    "kind",
    "spec",
    "receipt_file",
    "receipt_sha256",
    "command_contract_version",
    "as_of",
    "manifest_sha256",
    "ripr",
    "subjects",
];
/// The closed pointer `ripr` identity block: the toolchain identities the
/// currentness law binds. Copied only from what the validated candidate
/// recorded; absent identities are omitted, never invented.
const POINTER_RIPR_KEYS: [&str; 4] = ["source_sha", "binary_digest", "features", "build_profile"];
/// The closed per-subject pointer identity entry.
const POINTER_SUBJECT_KEYS: [&str; 5] = [
    "tree_digest",
    "row_sha256",
    "input_digest",
    "config_input",
    "config_profile",
];

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

/// The permissive license classes promotion counts as not license-blocked.
/// A row whose RECORDED license is outside this set is counted
/// license-blocked (a derived comparison over recorded fields, never a tier
/// ruling and never gating); a row that records no license is not counted
/// blocked — its absence is already a typed incomplete disclosure in the
/// candidate, not evidence of a blocked license.
const LICENSE_VOCABULARY: [&str; 6] = [
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "MIT OR Apache-2.0",
];

/// Free-text budget for accepted-artifact free text: disposition notes,
/// evidence references, owners, recovery routes, and the pointer's as-of
/// disclosure. Bounded excerpts, never unbounded logs.
const NOTE_MAX_CHARS: usize = 512;

/// The promotion-relevant non-claims embedded in every accepted receipt.
const NON_CLAIMS: [&str; 6] = [
    "no judged accuracy: robustness and distribution metrics are informational and never become judged accuracy",
    "no repair-correctness claim of any kind",
    "no support-tier change: acceptance does not authorize Python usable support",
    "no outcome-flip claim: acceptance never labels a changed behavior caught or missed; the analyzer reports static exposure evidence only",
    "no coverage claim beyond the retained eight-subject denominator",
    "no durable currentness claim: currentness is established only by eval-sweep report --check-currentness at consumption time",
];

// ---------------------------------------------------------------------------
// Args
// ---------------------------------------------------------------------------

struct ReportArgs {
    candidate: Option<String>,
    dispositions: Option<String>,
    accept: bool,
    check_currentness: bool,
    manifest: String,
    state_dir: String,
    as_of: Option<String>,
    ripr_bin: Option<String>,
    ripr_source_sha: Option<String>,
}

fn parse_report_args(args: &[String]) -> Result<ReportArgs, String> {
    let mut parsed = ReportArgs {
        candidate: None,
        dispositions: None,
        accept: false,
        check_currentness: false,
        manifest: DEFAULT_MANIFEST.to_string(),
        state_dir: DEFAULT_STATE_DIR.to_string(),
        as_of: None,
        ripr_bin: None,
        ripr_source_sha: None,
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--candidate" => {
                index += 1;
                parsed.candidate = Some(args.get(index).cloned().ok_or_else(|| {
                    format!("eval-sweep report --candidate requires a value\n{USAGE}")
                })?);
            }
            "--dispositions" => {
                index += 1;
                parsed.dispositions = Some(args.get(index).cloned().ok_or_else(|| {
                    format!("eval-sweep report --dispositions requires a value\n{USAGE}")
                })?);
            }
            "--accept" => parsed.accept = true,
            "--check-currentness" => parsed.check_currentness = true,
            "--manifest" => {
                index += 1;
                parsed.manifest = args.get(index).cloned().ok_or_else(|| {
                    format!("eval-sweep report --manifest requires a value\n{USAGE}")
                })?;
            }
            "--state-dir" => {
                index += 1;
                parsed.state_dir = args.get(index).cloned().ok_or_else(|| {
                    format!("eval-sweep report --state-dir requires a value\n{USAGE}")
                })?;
            }
            "--as-of" => {
                index += 1;
                parsed.as_of = Some(args.get(index).cloned().ok_or_else(|| {
                    format!("eval-sweep report --as-of requires a value\n{USAGE}")
                })?);
            }
            "--ripr-bin" => {
                index += 1;
                parsed.ripr_bin = Some(args.get(index).cloned().ok_or_else(|| {
                    format!("eval-sweep report --ripr-bin requires a value\n{USAGE}")
                })?);
            }
            "--ripr-source-sha" => {
                index += 1;
                parsed.ripr_source_sha = Some(args.get(index).cloned().ok_or_else(|| {
                    format!("eval-sweep report --ripr-source-sha requires a value\n{USAGE}")
                })?);
            }
            other => {
                return Err(format!(
                    "unknown eval-sweep report argument: {other}\n{USAGE}"
                ));
            }
        }
        index += 1;
    }
    if parsed.check_currentness {
        if parsed.candidate.is_some() || parsed.accept {
            return Err(format!(
                "eval-sweep report --check-currentness is mutually exclusive with --candidate/--accept\n{USAGE}"
            ));
        }
        return Ok(parsed);
    }
    if parsed.candidate.is_none() {
        return Err(format!(
            "eval-sweep report requires --candidate <receipt.json> or --check-currentness\n{USAGE}"
        ));
    }
    Ok(parsed)
}

// ---------------------------------------------------------------------------
// Shared strict-parsing helpers
// ---------------------------------------------------------------------------

fn fail(subject: &str, field: &str, reason: impl std::fmt::Display) -> String {
    format!(
        "eval-sweep report failed: subject=`{subject}` field=`{field}`: {reason}\nrerun: {RERUN_COMMAND}"
    )
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

/// Digest fields are bare lowercase sha256 hex (64 chars); git identity
/// fields are bare lowercase 40-char hex. Malformed bound identities fail.
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

/// Component law for portable relative paths: every `/`-separated component
/// must be non-empty and never `.` or `..`. Empty components (consecutive
/// separators) are skipped by host path resolution, `..` escapes the base
/// directory, and `.` hides a component from exact matching — none of them may
/// reach a path join, or a crafted pointer could read outside the accepted
/// state directory. A path with no non-empty component at all is rejected too.
fn check_portable_components(subject: &str, field: &str, portable: &str) -> Result<(), String> {
    let components: Vec<&str> = portable.split('/').collect();
    if components.iter().all(|component| component.is_empty()) {
        return Err(fail(
            subject,
            field,
            "path must name at least one non-empty component",
        ));
    }
    for component in components {
        if component.is_empty() {
            return Err(fail(
                subject,
                field,
                format!(
                    "path `{portable}` must not contain empty components (consecutive separators)"
                ),
            ));
        }
        if component == "." || component == ".." {
            return Err(fail(
                subject,
                field,
                format!("path `{portable}` must not contain `{component}` components"),
            ));
        }
    }
    Ok(())
}

/// A portable relative path: forward slashes only, never absolute, and
/// component-checked (no empty, `.`, or `..` components).
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
    if path.starts_with('/') {
        return Err(fail(
            subject,
            field,
            format!("path `{path}` must be relative, not absolute"),
        ));
    }
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return Err(fail(
            subject,
            field,
            format!("path `{path}` must be relative, not a drive-letter absolute path"),
        ));
    }
    check_portable_components(subject, field, path)
}

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
fn check_artifact_hygiene(artifact: &str, name: &str) -> Result<(), String> {
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
struct CandidateRowFacts {
    id: String,
    status: String,
    counts_as_run: bool,
    license: Option<String>,
    runtime_ms: Option<u64>,
    materialization: Option<String>,
    detection: Option<String>,
    corpus_state: Option<String>,
    tree_digest: Option<String>,
    snapshot: Option<String>,
    selected_root: Option<String>,
    input_digest: Option<String>,
    config_profile: Option<String>,
    config_input: Option<String>,
    digest_raw: Option<String>,
    digest_output: Option<String>,
    digest_evidence: Option<String>,
    repeat_comparable_with: Option<String>,
    repeat_gap_ids_stable: Option<bool>,
    repeat_unstable_gap_ids: Option<Vec<String>>,
    classification: Option<BTreeMap<String, u64>>,
    alignment: Option<BTreeMap<String, u64>>,
    /// sha256 over the row's canonical JSON — the accepted-row identity the
    /// pointer binds so any accepted-row byte change flips currentness.
    row_sha256: String,
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
fn read_candidate_rows(candidate: &Value) -> Result<Vec<CandidateRowFacts>, String> {
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

struct Disposition {
    disposition: String,
    evidence_ref: String,
    owner: String,
    recovery_route: String,
    notes: Option<String>,
}

/// Reads + validates the dispositions sidecar. Fails closed on unknown keys,
/// unknown ids, duplicates, dispositions for complete rows, unknown
/// vocabulary, missing owner/recovery route (every owned disposition type is
/// actionable by definition), hygiene violations, and oversized free text
/// (every bounded-artifact field is capped, not just notes).
fn load_dispositions(
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
fn require_disposition_coverage(
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

// ---------------------------------------------------------------------------
// Accepted receipt derivation (pure)
// ---------------------------------------------------------------------------

/// One count, always with its denominator.
struct Count {
    numerator: usize,
    denominator: usize,
}

impl Count {
    fn to_json(&self) -> Value {
        json!({
            "numerator": self.numerator,
            "denominator": self.denominator,
        })
    }
}

fn merge_distribution(target: &mut BTreeMap<String, u64>, source: &BTreeMap<String, u64>) {
    for (name, count) in source {
        *target.entry(name.clone()).or_insert(0) += count;
    }
}

/// The per-subject bound identities (pointer-shaped): accepted row digest,
/// tree digest, input digest, config identity. Absent row identities are
/// omitted — never invented.
fn pointer_subject_identities(rows: &[CandidateRowFacts]) -> Value {
    let mut subjects = serde_json::Map::new();
    for row in rows {
        let mut entry = serde_json::Map::new();
        entry.insert("row_sha256".to_string(), json!(row.row_sha256));
        if let Some(tree) = &row.tree_digest {
            entry.insert("tree_digest".to_string(), json!(tree));
        }
        if let Some(input) = &row.input_digest {
            entry.insert("input_digest".to_string(), json!(input));
        }
        if let Some(input) = &row.config_input {
            entry.insert("config_input".to_string(), json!(input));
        }
        if let Some(profile) = &row.config_profile {
            entry.insert("config_profile".to_string(), json!(profile));
        }
        subjects.insert(row.id.clone(), Value::Object(entry));
    }
    Value::Object(subjects)
}

/// Builds the accepted receipt value from the validated rows and validated
/// dispositions. Every aggregate is derived here — the receipt never carries a
/// hand-entered total, and every count carries its denominator.
fn build_accepted_receipt(
    manifest: &AcceptedManifest,
    manifest_sha256: &str,
    candidate: &Value,
    candidate_sha256: &str,
    rows: &[CandidateRowFacts],
    dispositions: &BTreeMap<String, Disposition>,
    incomplete_disclosures: &[String],
) -> Value {
    let total = rows.len();
    let run = rows.iter().filter(|row| row.counts_as_run).count();
    let detection_count = |state: &str| {
        rows.iter()
            .filter(|row| row.detection.as_deref() == Some(state))
            .count()
    };
    let corpus_count = |state: &str| {
        rows.iter()
            .filter(|row| row.corpus_state.as_deref() == Some(state))
            .count()
    };
    let materialized = rows
        .iter()
        .filter(|row| {
            matches!(
                row.materialization.as_deref(),
                Some("materialized") | Some("snapshot")
            )
        })
        .count();
    let available = detection_count("detected");
    let stale = rows.iter().filter(|row| row.status == "stale").count();
    let license_blocked = rows
        .iter()
        .filter(|row| {
            row.license
                .as_deref()
                .is_some_and(|license| !LICENSE_VOCABULARY.contains(&license))
        })
        .count();
    let tempfail = rows.iter().filter(|row| row.status == "tempfail").count();

    // Every accepted-receipt count is emitted as `{numerator, denominator}`
    // with the denominator each contract defines: outcomes and distribution
    // buckets over the selected denominator, the runtime count over the
    // analyzed (run) rows, and each health tally over the selected rows it
    // tallies. A bucket sum below its denominator (non-run rows contribute
    // nothing) is the honest shape, not an error.
    let over_selected = |numerator: usize| {
        Count {
            numerator,
            denominator: total,
        }
        .to_json()
    };
    let outcome_count = |status: &str| rows.iter().filter(|row| row.status == status).count();
    let outcomes = json!({
        "complete": over_selected(outcome_count("complete")),
        "partial": over_selected(outcome_count("partial")),
        "parse_failed": over_selected(outcome_count("parse-failed")),
        "timed_out": over_selected(outcome_count("timed-out")),
        "crashed": over_selected(outcome_count("crashed")),
        "unsupported": over_selected(outcome_count("unsupported")),
        "tempfail": over_selected(outcome_count("tempfail")),
        "stale": over_selected(outcome_count("stale")),
    });

    // Runtime envelope: emitted only where reliable — every run row must
    // carry a runtime (the validator enforces recorded presence on run rows)
    // and at least one row must have run.
    let runtimes: Vec<u64> = rows
        .iter()
        .filter(|row| row.counts_as_run)
        .filter_map(|row| row.runtime_ms)
        .collect();
    let runtime_envelope = if run > 0 && runtimes.len() == run {
        let mut sorted = runtimes.clone();
        sorted.sort_unstable();
        let mut total_ms: u64 = 0;
        for runtime in &sorted {
            total_ms = total_ms.saturating_add(*runtime);
        }
        json!({
            "status": "reliable",
            "min_ms": sorted[0],
            "median_ms": sorted[sorted.len() / 2],
            "max_ms": sorted[sorted.len() - 1],
            "total_ms": total_ms,
            // The run-row count over the analyzed rows; the min/median/max/
            // total fields are durations, not counts, so they stay bare.
            "count": Count { numerator: runtimes.len(), denominator: run }.to_json(),
        })
    } else {
        json!({
            "status": "unavailable",
            "reason": if run == 0 { "no subject reached an analysis attempt" } else { "not every run row records a runtime" },
            "count": Count { numerator: runtimes.len(), denominator: run }.to_json(),
        })
    };

    // Repeat-run identity stability: compared = run rows carrying a recorded
    // comparison; the mismatch list carries the per-subject reasons.
    let compared_rows: Vec<&CandidateRowFacts> = rows
        .iter()
        .filter(|row| row.counts_as_run && row.repeat_gap_ids_stable.is_some())
        .collect();
    let stable_count = compared_rows
        .iter()
        .filter(|row| row.repeat_gap_ids_stable == Some(true))
        .count();
    let mismatches: Vec<Value> = compared_rows
        .iter()
        .filter(|row| row.repeat_gap_ids_stable == Some(false))
        .map(|row| {
            json!({
                "id": row.id,
                "unstable_gap_ids": row.repeat_unstable_gap_ids.clone().unwrap_or_default(),
            })
        })
        .collect();
    let stability_evidence = if run == 0 {
        "no-run"
    } else if compared_rows.len() == run {
        "complete"
    } else {
        "partial"
    };
    let stability = json!({
        "compared": Count { numerator: compared_rows.len(), denominator: run }.to_json(),
        "gap_ids_stable": Count { numerator: stable_count, denominator: compared_rows.len() }.to_json(),
        "evidence": stability_evidence,
        "mismatch_subjects": mismatches,
    });

    let mut classification: BTreeMap<String, u64> = BTreeMap::new();
    let mut alignment: BTreeMap<String, u64> = BTreeMap::new();
    for row in rows.iter().filter(|row| row.counts_as_run) {
        if let Some(counts) = &row.classification {
            merge_distribution(&mut classification, counts);
        }
        if let Some(counts) = &row.alignment {
            merge_distribution(&mut alignment, counts);
        }
    }
    let bucketed = |map: BTreeMap<String, u64>| -> BTreeMap<String, Value> {
        map.into_iter()
            .map(|(name, count)| (name, over_selected(count as usize)))
            .collect()
    };

    // Detection / corpus-selection health tallies, with the unrecorded share
    // disclosed (an absent state field is not an absent state value). Each
    // tally carries its checked dimension: the selected rows.
    let health = json!({
        "project_detection": {
            "detected": over_selected(detection_count("detected")),
            "failed": over_selected(detection_count("failed")),
            "unknown": over_selected(detection_count("unknown")),
            "absent": over_selected(detection_count("absent")),
            "unrecorded": over_selected(
                rows.iter().filter(|row| row.detection.is_none()).count(),
            ),
        },
        "corpus_selection": {
            "selected": over_selected(corpus_count("selected")),
            "partial": over_selected(corpus_count("partial")),
            "failed": over_selected(corpus_count("failed")),
            "unknown": over_selected(corpus_count("unknown")),
            "absent": over_selected(corpus_count("absent")),
            "unrecorded": over_selected(
                rows.iter().filter(|row| row.corpus_state.is_none()).count(),
            ),
        },
    });

    // Per-subject accepted rows (bounded by the fixed denominator).
    let mut subjects = Vec::new();
    for row in rows {
        let subject_pin = manifest.subject(&row.id);
        let identity = json!({
            "url": subject_pin.map(|subject| subject.url.clone()),
            "sha": subject_pin.map(|subject| subject.sha.clone()),
            "license": row.license,
            "shape": subject_pin.map(|subject| subject.shape.clone()),
            "tree_digest": row.tree_digest,
            "snapshot": row.snapshot,
            "selected_root": row.selected_root,
            "config_profile": row.config_profile,
            "config_input": row.config_input,
            "input_digest": row.input_digest,
            "row_sha256": row.row_sha256,
        });
        let evidence = json!({
            "raw": row.digest_raw,
            "output": row.digest_output,
            "evidence": row.digest_evidence,
        });
        let stability_block = json!({
            "compared": row.repeat_gap_ids_stable.is_some(),
            "comparable_with": row.repeat_comparable_with,
            "gap_ids_stable": row.repeat_gap_ids_stable,
            "unstable_gap_ids": row.repeat_unstable_gap_ids.clone().unwrap_or_default(),
        });
        let mut subject = json!({
            "id": row.id,
            "status": row.status,
            "counts_as_run": row.counts_as_run,
            "license_blocked": row.license.as_deref().is_some_and(|license| !LICENSE_VOCABULARY.contains(&license)),
            "identity": identity,
            "evidence": evidence,
            "stability": stability_block,
            "classification_counts": row.classification.clone().map(|counts| json!(counts)),
            "alignment_counts": row.alignment.clone().map(|counts| json!(counts)),
        });
        if let Some(runtime) = row.runtime_ms {
            subject["runtime_ms"] = json!(runtime);
        }
        if let Some(disposition) = dispositions.get(&row.id) {
            subject["disposition"] = json!({
                "disposition": disposition.disposition,
                "evidence_ref": disposition.evidence_ref,
                "owner": disposition.owner,
                "recovery_route": disposition.recovery_route,
                "notes": disposition.notes,
            });
        }
        subjects.push(subject);
    }

    // The candidate's own typed incomplete disclosures travel with the
    // accepted receipt (identity gaps are disclosed, never silently dropped
    // and never invented).
    let disclosures: Vec<Value> = incomplete_disclosures
        .iter()
        .map(|text| json!(text))
        .collect();

    json!({
        "schema_version": ACCEPTED_SCHEMA,
        "kind": ACCEPTED_KIND,
        "spec": SPEC,
        "tier": TIER,
        "command_contract_version": CONTRACT_VERSION,
        "candidate": {
            "kind": candidate.get("kind").and_then(Value::as_str).unwrap_or("python_eval_sweep_report"),
            "schema_version": candidate.get("schema_version").and_then(Value::as_str).unwrap_or(CANDIDATE_SCHEMA),
            "sha256": candidate_sha256,
            "manifest_sha256": manifest_sha256,
        },
        "denominator": Count { numerator: total, denominator: manifest.subjects.len() }.to_json(),
        "counts": {
            "selected": Count { numerator: total, denominator: manifest.subjects.len() }.to_json(),
            "run": Count { numerator: run, denominator: total }.to_json(),
            "materialized": Count { numerator: materialized, denominator: total }.to_json(),
            "available": Count { numerator: available, denominator: total }.to_json(),
            "stale": Count { numerator: stale, denominator: total }.to_json(),
            "license_blocked": Count { numerator: license_blocked, denominator: total }.to_json(),
            "tempfail": Count { numerator: tempfail, denominator: total }.to_json(),
        },
        "outcomes": outcomes,
        "runtime_envelope": runtime_envelope,
        "stability": stability,
        "distributions": {
            "classification": bucketed(classification),
            "alignment": bucketed(alignment),
            "limitation": {
                "disclosure": "no limitation distribution is emitted: the schema-0.3 receipt row records no limitation field (per-phase limitations live in the managed execution receipt), so a limitation taxonomy would be invented, not derived",
            },
        },
        "health": health,
        "subjects": subjects,
        "identities": {
            "manifest_sha256": manifest_sha256,
            "candidate_receipt_sha256": candidate_sha256,
            "command_contract_version": CONTRACT_VERSION,
            "ripr": candidate.get("ripr").cloned().unwrap_or(Value::Null),
            "subjects": pointer_subject_identities(rows),
        },
        "incomplete_disclosures": disclosures,
        "non_claims": NON_CLAIMS,
        "claim_boundary": "accepted operational-robustness evidence over the retained eight-subject denominator; informational metrics, never judged accuracy; no repair-correctness or support-tier claim",
    })
}

/// Builds the current pointer for one accepted receipt. Identity only — no
/// totals, no rates, no per-subject outcomes; the `as_of` string is a
/// disclosure, never a load-bearing identity.
fn build_pointer(
    receipt_sha256: &str,
    manifest_sha256: &str,
    candidate: &Value,
    rows: &[CandidateRowFacts],
    as_of: Option<&str>,
) -> Result<Value, String> {
    let mut pointer = serde_json::Map::new();
    pointer.insert("schema_version".to_string(), json!(POINTER_SCHEMA));
    pointer.insert("kind".to_string(), json!(POINTER_KIND));
    pointer.insert("spec".to_string(), json!(SPEC));
    pointer.insert(
        "receipt_file".to_string(),
        json!(format!("{RECEIPTS_DIR}/{receipt_sha256}.json")),
    );
    pointer.insert("receipt_sha256".to_string(), json!(receipt_sha256));
    pointer.insert(
        "command_contract_version".to_string(),
        json!(CONTRACT_VERSION),
    );
    if let Some(as_of) = as_of {
        let trimmed = as_of.trim();
        if trimmed.is_empty() {
            return Err(fail(
                "pointer",
                "as_of",
                "as-of must be non-empty when supplied",
            ));
        }
        if trimmed.chars().count() > NOTE_MAX_CHARS {
            return Err(fail(
                "pointer",
                "as_of",
                format!("as-of exceeds the {NOTE_MAX_CHARS}-character bound"),
            ));
        }
        check_artifact_hygiene(trimmed, "pointer.as_of")?;
        pointer.insert("as_of".to_string(), json!(trimmed));
    }
    pointer.insert("manifest_sha256".to_string(), json!(manifest_sha256));
    // The toolchain identity block copies ONLY the owned currentness fields
    // from the validated candidate envelope (the receipt keeps the rest).
    let candidate_ripr = candidate
        .get("ripr")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            fail(
                "candidate",
                "ripr",
                "schema-0.3 candidates carry the ripr identity block",
            )
        })?;
    let mut ripr = serde_json::Map::new();
    for field in POINTER_RIPR_KEYS {
        if let Some(value) = candidate_ripr.get(field) {
            ripr.insert(field.to_string(), value.clone());
        }
    }
    pointer.insert("ripr".to_string(), Value::Object(ripr));
    pointer.insert("subjects".to_string(), pointer_subject_identities(rows));
    Ok(Value::Object(pointer))
}

// ---------------------------------------------------------------------------
// Bounded Markdown (derived from the same validated rows)
// ---------------------------------------------------------------------------

/// Renders one bounded-Markdown table cell: pipe characters are escaped and
/// newline runs are flattened to spaces, so a free-text value (a disposition
/// owner, recovery route, or evidence reference) can never split or extend a
/// table row. The accepted receipt JSON keeps the raw value.
fn markdown_cell(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '|' => out.push_str("\\|"),
            '\n' | '\r' => out.push(' '),
            other => out.push(other),
        }
    }
    out
}

fn render_accepted_markdown(receipt: &Value) -> Result<String, String> {
    let counts = receipt
        .get("counts")
        .and_then(Value::as_object)
        .ok_or_else(|| fail("markdown", "counts", "accepted receipt must carry counts"))?;
    let mut out = String::new();
    out.push_str("# Python Eval Sweep — Accepted Receipt\n\n");
    out.push_str(
        "Accepted operational-robustness evidence over the retained eight-subject denominator.\n",
    );
    out.push_str("Informational metrics — never judged accuracy, never a support-tier change.\n\n");

    out.push_str("## Identity\n\n");
    let candidate = receipt.get("candidate").cloned().unwrap_or(json!({}));
    out.push_str(&format!(
        "- candidate receipt: sha256 `{}` (schema {})\n",
        candidate
            .get("sha256")
            .and_then(Value::as_str)
            .unwrap_or("?"),
        candidate
            .get("schema_version")
            .and_then(Value::as_str)
            .unwrap_or("?")
    ));
    out.push_str(&format!(
        "- manifest: sha256 `{}`\n",
        receipt
            .get("identities")
            .and_then(|identities| identities.get("manifest_sha256"))
            .and_then(Value::as_str)
            .unwrap_or("?")
    ));
    out.push_str(&format!(
        "- command contract: {}\n\n",
        receipt
            .get("command_contract_version")
            .and_then(Value::as_str)
            .unwrap_or("?")
    ));

    out.push_str("## Counts\n\n");
    out.push_str("| count | numerator | denominator |\n| --- | ---: | ---: |\n");
    for (name, count) in counts {
        let numerator = count.get("numerator").and_then(Value::as_u64);
        let denominator = count.get("denominator").and_then(Value::as_u64);
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            name,
            numerator
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string()),
            denominator
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string()),
        ));
    }
    out.push('\n');

    out.push_str("## Subjects\n\n");
    out.push_str("| id | status | disposition | owner |\n| --- | --- | --- | --- |\n");
    if let Some(subjects) = receipt.get("subjects").and_then(Value::as_array) {
        for subject in subjects {
            let id = subject.get("id").and_then(Value::as_str).unwrap_or("?");
            let status = subject.get("status").and_then(Value::as_str).unwrap_or("?");
            let disposition = subject
                .get("disposition")
                .and_then(|disposition| disposition.get("disposition"))
                .and_then(Value::as_str)
                .unwrap_or("-");
            let owner = subject
                .get("disposition")
                .and_then(|disposition| disposition.get("owner"))
                .and_then(Value::as_str)
                .unwrap_or("-");
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                markdown_cell(id),
                markdown_cell(status),
                markdown_cell(disposition),
                markdown_cell(owner)
            ));
        }
    }
    out.push('\n');

    out.push_str("## Non-claims\n\n");
    if let Some(non_claims) = receipt.get("non_claims").and_then(Value::as_array) {
        for claim in non_claims {
            if let Some(claim) = claim.as_str() {
                out.push_str(&format!("- {claim}\n"));
            }
        }
    }
    out.push('\n');
    out.push_str(&format!("rerun: `{RERUN_COMMAND} --check-currentness`\n"));
    Ok(out)
}

// ---------------------------------------------------------------------------
// Candidate report / accept
// ---------------------------------------------------------------------------

fn run_candidate_report(parsed: &ReportArgs) -> Result<(), String> {
    let candidate_path = parsed
        .candidate
        .as_deref()
        .ok_or_else(|| format!("eval-sweep report requires --candidate <receipt.json>\n{USAGE}"))?;

    // One validator owns receipt semantics: the candidate must pass the exact
    // `eval-sweep check --runs` contract before anything is derived.
    let (manifest_value, manifest_sha256) = load_strict_json(&parsed.manifest)?;
    let accepted = validate_accepted_manifest(&manifest_value, manifest_sha256.clone())?;
    let (candidate_value, candidate_sha256) = load_strict_json(candidate_path)?;
    let candidate_schema = candidate_value
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    if candidate_schema != CANDIDATE_SCHEMA {
        return Err(fail(
            candidate_path,
            "schema_version",
            format!(
                "acceptance binds currentness identities, so it accepts only schema-{CANDIDATE_SCHEMA} candidates; got `{candidate_schema}` — historical receipts remain valid retained artifacts, immutable and separately addressable, and are never rewritten"
            ),
        ));
    }
    let check = validate_run_receipt(
        &candidate_value,
        &manifest_sha256,
        &accepted,
        candidate_path,
    )?;
    let rows = read_candidate_rows(&candidate_value)?;
    // The candidate's typed incomplete disclosures travel with the accepted
    // receipt — but their receipt-level subject is the candidate's host
    // display path, and accepted artifacts carry no absolute host paths, so
    // the path is replaced with a portable subject label (field and reason
    // content is preserved verbatim).
    let disclosures: Vec<String> = check
        .incomplete
        .iter()
        .map(|diagnostic| {
            diagnostic.render().replace(
                &format!("subject=`{candidate_path}`"),
                "subject=`candidate receipt`",
            )
        })
        .collect();

    // Dispositions: required exactly when non-complete rows exist.
    let non_complete = rows.iter().filter(|row| row.status != "complete").count();
    let dispositions = match &parsed.dispositions {
        Some(path) => load_dispositions(path, &rows)?,
        None => {
            if non_complete > 0 {
                return Err(fail(
                    candidate_path,
                    "dispositions",
                    format!(
                        "{non_complete} non-complete subject(s) carry no terminal disposition; supply --dispositions <path> with one typed disposition per non-complete subject"
                    ),
                ));
            }
            BTreeMap::new()
        }
    };
    require_disposition_coverage(&rows, &dispositions)?;

    let receipt = build_accepted_receipt(
        &accepted,
        &manifest_sha256,
        &candidate_value,
        &candidate_sha256,
        &rows,
        &dispositions,
        &disclosures,
    );
    // The accepted receipt's identity is the sha256 over its EXACT written
    // bytes (pretty JSON plus the trailing newline the file carries), so the
    // currentness recomputation hashes the same bytes the pointer bound.
    let receipt_text = serde_json::to_string_pretty(&receipt).map_err(|error| {
        fail(
            "receipt",
            "render",
            format!("cannot render the accepted receipt: {error}"),
        )
    })?;
    let receipt_bytes = format!("{receipt_text}\n");
    let receipt_sha256 = sha256_hex(receipt_bytes.as_bytes());
    let markdown = render_accepted_markdown(&receipt)?;
    let pointer = build_pointer(
        &receipt_sha256,
        &manifest_sha256,
        &candidate_value,
        &rows,
        parsed.as_of.as_deref(),
    )?;
    let pointer_text = serde_json::to_string_pretty(&pointer).map_err(|error| {
        fail(
            "pointer",
            "render",
            format!("cannot render the current pointer: {error}"),
        )
    })?;

    // Hygiene before any byte is written: secrets, absolute host paths,
    // bounded free text (notes are capped at read time).
    check_artifact_hygiene(&receipt_bytes, "accepted receipt")?;
    check_artifact_hygiene(&markdown, "accepted markdown")?;
    check_artifact_hygiene(&pointer_text, "current pointer")?;

    if !parsed.accept {
        crate::write_report(REPORT_JSON, &receipt_bytes)?;
        crate::write_report(REPORT_MD, &markdown)?;
        println!(
            "eval-sweep report: dry run — candidate validated (denominator selected={} run={} incomplete_disclosures={}); accepted receipt rendered to {REPORT_JSON}/{REPORT_MD}; no accepted state written",
            check.denominator_selected,
            check.denominator_run,
            disclosures.len(),
        );
        println!("rerun: {RERUN_COMMAND}");
        return Ok(());
    }

    // Accept: verify the candidate file still holds the validated bytes
    // BEFORE any accepted byte is written (a candidate edited between the
    // initial parse and this point is a typed refusal, so the accepted
    // artifacts can never retain new bytes under the OLD candidate digest
    // while `current.json` moves), then publish the content-addressed
    // receipt, its Markdown, and the retained candidate through the staged
    // atomic pattern, and move the pointer. Every accepted write is atomic,
    // so an interrupted run cannot leave truncated bytes under an accepted
    // artifact name; the self-addressed artifacts (receipt, retained
    // candidate) repair a file whose bytes do not hash to their own digest
    // address, and no other accepted artifact is ever rewritten.
    let candidate_bytes = revalidate_candidate_bytes(Path::new(candidate_path), &candidate_sha256)?;
    let state_dir = PathBuf::from(&parsed.state_dir);
    let receipts_dir = state_dir.join(RECEIPTS_DIR);
    std::fs::create_dir_all(&receipts_dir).map_err(|error| {
        fail(
            &parsed.state_dir,
            "state-dir",
            format!("cannot create the accepted receipts directory: {error}"),
        )
    })?;
    let receipt_target = receipts_dir.join(format!("{receipt_sha256}.json"));
    // The receipt path is content-addressed over exactly the bytes staged
    // here, so publishing them by atomic rename keeps the address
    // self-verifying: the file at `<sha256>.json` hashes to `<sha256>` by
    // construction. A file already present with other bytes is not a valid
    // prior artifact — truncated by an interrupted write, or edited after
    // acceptance — and is repaired rather than preserved: keeping it would
    // block re-acceptance forever as "conflicting content" that its own
    // digest address refutes. Identical bytes remain an idempotent no-op.
    match std::fs::read(&receipt_target) {
        Ok(existing) if existing == receipt_bytes.as_bytes() => {
            println!(
                "eval-sweep report: accepted receipt `{}` already accepted (identical bytes); immutable artifact untouched",
                receipt_target.to_string_lossy()
            );
        }
        _ => {
            publish_bytes_atomically(
                &receipt_target,
                receipt_bytes.as_bytes(),
                "accepted receipt",
            )?;
        }
    }
    let markdown_target = receipts_dir.join(format!("{receipt_sha256}.md"));
    if markdown_target.exists() {
        // An existing Markdown is verified, never silently kept or rewritten:
        // its name is the RECEIPT's digest, not a digest of its own bytes, so
        // an edited artifact under the accepted receipt's digest is a typed
        // refusal (the self-addressed artifacts above can repair because
        // their address refutes corrupt bytes; this one cannot, so it
        // refuses).
        let existing = std::fs::read(&markdown_target).map_err(|error| {
            fail(
                &markdown_target.to_string_lossy(),
                "file",
                format!("existing accepted markdown cannot be read: {error}"),
            )
        })?;
        if existing != markdown.as_bytes() {
            return Err(fail(
                &markdown_target.to_string_lossy(),
                "file",
                "an accepted markdown with this digest exists with different bytes; accepted evidence is immutable and is never overwritten — investigate the edited file before re-accepting",
            ));
        }
    } else {
        publish_bytes_atomically(&markdown_target, markdown.as_bytes(), "accepted markdown")?;
    }
    // The retained candidate: addressed by the candidate's own digest, so the
    // currentness check can re-validate the accepted rows through the shared
    // validator without trusting any pointer content. The staged bytes are
    // exactly the ones verified by the pre-write digest re-check above, so
    // the same self-addressed repair law applies: a file under this digest
    // with other bytes is corrupt (an interrupted write, or an edit), not a
    // valid prior artifact, and is repaired rather than preserved.
    let candidate_target = receipts_dir.join(format!("{candidate_sha256}.candidate.json"));
    match std::fs::read(&candidate_target) {
        Ok(existing) if existing == candidate_bytes => {}
        _ => publish_bytes_atomically(&candidate_target, &candidate_bytes, "retained candidate")?,
    }

    write_pointer_atomically(&state_dir, &pointer_text)?;
    println!(
        "eval-sweep report: accepted receipt {} (denominator selected={} run={}); pointer moved to the newest accepted receipt",
        receipt_target.to_string_lossy(),
        check.denominator_selected,
        check.denominator_run,
    );
    println!(
        "rerun: {RERUN_COMMAND} --check-currentness --state-dir {}",
        parsed.state_dir
    );
    Ok(())
}

/// Re-reads the candidate file immediately before the acceptance writes and
/// verifies its bytes still hash to the digest recorded when the candidate was
/// parsed and validated. A candidate edited between validation and acceptance
/// is a typed refusal: the accepted artifacts must retain exactly the
/// validated bytes under the recorded digest, never new bytes under the old
/// candidate identity.
fn revalidate_candidate_bytes(
    candidate_path: &Path,
    bound_sha256: &str,
) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(candidate_path).map_err(|error| {
        fail(
            &candidate_path.to_string_lossy(),
            "file",
            format!("validated candidate cannot be re-read for retention: {error}"),
        )
    })?;
    let actual = sha256_hex(&bytes);
    if actual != bound_sha256 {
        return Err(fail(
            &candidate_path.to_string_lossy(),
            "candidate_sha256",
            format!(
                "the candidate file changed after validation: the report parsed sha256 `{bound_sha256}` but the file now hashes to `{actual}`; re-run the report so the accepted artifacts retain exactly the validated candidate"
            ),
        ));
    }
    Ok(bytes)
}

/// Publishes bytes at their final path through the staged pattern: the bytes
/// are written to a staging file in the target's directory and flushed FIRST,
/// then renamed over the final path. Renaming over an existing destination
/// replaces it in one step on the platforms this route supports (Unix, and
/// Windows `fs::rename` moves with replace-existing). The rename is atomic, so
/// there is no window in which the target holds partial bytes — an interrupted
/// run leaves either the previous bytes or the complete new bytes at the final
/// path, never a truncated artifact. If a host refuses rename-over-existing,
/// the fallback removes the target only after the staged bytes are fully
/// written and flushed, then renames; the residual window between that remove
/// and the rename can lose the file after a crash but can never leave partial
/// bytes.
fn publish_bytes_atomically(target: &Path, bytes: &[u8], what: &str) -> Result<(), String> {
    let file_name = target
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or_else(|| {
            fail(
                &target.to_string_lossy(),
                "file",
                format!("cannot stage the {what}: the target has no file name"),
            )
        })?;
    let temp = target.with_file_name(format!("{file_name}.tmp-{}", std::process::id()));
    {
        let mut staged = std::fs::File::create(&temp).map_err(|error| {
            fail(
                &temp.to_string_lossy(),
                "file",
                format!("cannot write the {what} staging file: {error}"),
            )
        })?;
        staged.write_all(bytes).map_err(|error| {
            fail(
                &temp.to_string_lossy(),
                "file",
                format!("cannot write the {what} staging file: {error}"),
            )
        })?;
        staged.sync_all().map_err(|error| {
            fail(
                &temp.to_string_lossy(),
                "file",
                format!("cannot flush the {what} staging file: {error}"),
            )
        })?;
    }
    // Replace-in-place rename: an existing target is NOT removed first, so it
    // stays readable until the rename swaps the bytes in one step.
    if std::fs::rename(&temp, target).is_ok() {
        return Ok(());
    }
    // Fallback for hosts that refuse rename-over-existing: remove then
    // rename, only after the staged bytes are fully written and flushed
    // (above).
    if target.exists() {
        std::fs::remove_file(target).map_err(|error| {
            fail(
                &target.to_string_lossy(),
                "file",
                format!("cannot replace the {what}: {error}"),
            )
        })?;
    }
    std::fs::rename(&temp, target).map_err(|error| {
        fail(
            &target.to_string_lossy(),
            "file",
            format!("cannot move the staged {what} into place: {error}"),
        )
    })?;
    Ok(())
}

/// Atomically updates the current pointer: the staged file is written and
/// flushed FIRST, then renamed over the existing pointer (see
/// `publish_bytes_atomically` for the atomicity and fallback law). Readers
/// after the write see either the old pointer or the new one — never a
/// missing pointer and never a partial pointer.
fn write_pointer_atomically(state_dir: &Path, pointer_text: &str) -> Result<(), String> {
    std::fs::create_dir_all(state_dir).map_err(|error| {
        fail(
            &state_dir.to_string_lossy(),
            "state-dir",
            format!("cannot create the accepted state directory: {error}"),
        )
    })?;
    let target = state_dir.join(POINTER_FILE);
    publish_bytes_atomically(
        &target,
        format!("{pointer_text}\n").as_bytes(),
        "current pointer",
    )
}

// ---------------------------------------------------------------------------
// Currentness gate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CurrentnessVerdict {
    Current,
    Stale,
    Unverifiable,
}

impl CurrentnessVerdict {
    fn as_str(self) -> &'static str {
        match self {
            CurrentnessVerdict::Current => "current",
            CurrentnessVerdict::Stale => "stale",
            CurrentnessVerdict::Unverifiable => "unverifiable",
        }
    }
}

/// The recomputed current identities the pointer is compared against. Fields
/// the loader cannot recompute offline stay `None` — those identities are
/// disclosed `unverifiable`, never assumed current.
struct CurrentIdentity {
    /// Flag override or `git rev-parse HEAD` in the repository root.
    ripr_source_sha: Option<String>,
    /// sha256 of the supplied `--ripr-bin` bytes.
    ripr_binary_digest: Option<String>,
    /// Per-subject recomputed input digest (sha256 over the current synthetic
    /// diff bytes), only where the input file resolves and reads.
    subject_inputs: BTreeMap<String, String>,
}

struct CurrentnessComparison {
    verdict: CurrentnessVerdict,
    stale: Vec<String>,
    unverifiable: Vec<String>,
}

/// Everything one currentness comparison reads: the parsed pointer, the
/// accepted receipt with its recomputed digest, the retained candidate with
/// its recomputed digest, the CURRENT accepted manifest (when it loads and
/// validates — see `manifest_validation_error`), and the recomputed live
/// identities.
struct CurrentnessInputs<'a> {
    pointer: &'a serde_json::Map<String, Value>,
    accepted_receipt: &'a Value,
    receipt_sha256: &'a str,
    candidate: &'a Value,
    candidate_sha256: &'a str,
    /// The CURRENT accepted manifest after a successful load + validation.
    /// `None` when the manifest fails either: the comparison then records the
    /// failure as a stale reason and skips the manifest-dependent re-checks
    /// (they need the validated manifest and cannot flip an already-stale
    /// verdict).
    accepted: Option<&'a AcceptedManifest>,
    current_manifest_sha256: &'a str,
    /// The manifest load/validation failure, when the current manifest bytes
    /// do not parse or validate against the accepted-state contract. Never
    /// aborts the verdict path: the digest comparison runs on the raw bytes,
    /// so a moved manifest reaches the promised `stale` verdict even when its
    /// new bytes are malformed, and the failure is named as an additional
    /// stale reason.
    manifest_validation_error: Option<&'a str>,
    current: &'a CurrentIdentity,
}

/// The pure currentness comparison: pointer-bound identities vs the accepted
/// receipt bytes, the retained candidate bytes, and the recomputed current
/// state. Staleness derives ONLY from digest, binding, and vocabulary
/// comparisons — the `as_of` disclosure is never an input, so editing it can
/// never repair a stale pointer.
fn compare_currentness(inputs: &CurrentnessInputs) -> Result<CurrentnessComparison, String> {
    let pointer = inputs.pointer;
    let accepted_receipt = inputs.accepted_receipt;
    let receipt_sha256 = inputs.receipt_sha256;
    let candidate = inputs.candidate;
    let candidate_sha256 = inputs.candidate_sha256;
    let current_manifest_sha256 = inputs.current_manifest_sha256;
    let current = inputs.current;
    let mut stale: Vec<String> = Vec::new();
    let mut unverifiable: Vec<String> = Vec::new();

    // 1. The accepted receipt's bytes must still hash to the bound digest.
    let bound_receipt = pointer
        .get("receipt_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            fail(
                "pointer",
                "receipt_sha256",
                "pointer must bind the receipt digest",
            )
        })?
        .to_string();
    check_sha256_digest("pointer", "receipt_sha256", &bound_receipt)?;
    if bound_receipt != receipt_sha256 {
        stale.push(format!(
            "accepted receipt bytes changed: pointer binds sha256 `{bound_receipt}` but the artifact hashes to `{receipt_sha256}`"
        ));
    }

    // 2. The manifest file's current digest against the bound copy.
    let bound_manifest = pointer
        .get("manifest_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            fail(
                "pointer",
                "manifest_sha256",
                "pointer must bind the manifest digest",
            )
        })?
        .to_string();
    check_sha256_digest("pointer", "manifest_sha256", &bound_manifest)?;
    if bound_manifest != current_manifest_sha256 {
        stale.push(format!(
            "accepted manifest changed: pointer binds sha256 `{bound_manifest}` but the manifest hashes to `{current_manifest_sha256}`"
        ));
    }

    // 2b. The manifest must also still load and validate against the
    // accepted-state contract. The failure never aborts the verdict path:
    // the digest comparison above ran on the raw bytes, so a moved manifest
    // is recorded stale even when its new bytes are malformed, and the
    // failure itself is named as an additional stale reason.
    if let Some(error) = inputs.manifest_validation_error {
        let first = error.lines().next().unwrap_or_default();
        stale.push(format!("accepted manifest failed validation: {first}"));
    }

    // 3. The command contract version.
    let bound_contract = pointer
        .get("command_contract_version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            fail(
                "pointer",
                "command_contract_version",
                "pointer must bind the command contract version",
            )
        })?
        .to_string();
    if bound_contract != CONTRACT_VERSION {
        stale.push(format!(
            "command contract moved: pointer was accepted under contract `{bound_contract}`, this checker enforces `{CONTRACT_VERSION}`"
        ));
    }

    // 4. The accepted receipt's candidate binding: the retained candidate
    // must still be the exact bytes the accepted receipt was derived from.
    let bound_candidate = accepted_receipt
        .get("candidate")
        .and_then(|candidate| candidate.get("sha256"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            fail(
                "accepted receipt",
                "candidate.sha256",
                "accepted receipt must bind its candidate digest",
            )
        })?
        .to_string();
    if bound_candidate != candidate_sha256 {
        stale.push(format!(
            "retained candidate changed: the accepted receipt binds sha256 `{bound_candidate}` but the retained candidate hashes to `{candidate_sha256}`"
        ));
    }

    // 5. The pointer's per-subject identity map must agree with the accepted
    // receipt's own identity projection (both derive from the same validated
    // rows; a disagreement is an edit in one of them).
    let bound_subjects = pointer.get("subjects").cloned().ok_or_else(|| {
        fail(
            "pointer",
            "subjects",
            "pointer must bind per-subject identities",
        )
    })?;
    let receipt_subjects = accepted_receipt
        .get("identities")
        .and_then(|identities| identities.get("subjects"))
        .cloned()
        .ok_or_else(|| {
            fail(
                "accepted receipt",
                "identities.subjects",
                "accepted receipt must carry its per-subject identity projection",
            )
        })?;
    if bound_subjects != receipt_subjects {
        stale.push(
            "pointer subject identities disagree with the accepted receipt's identity projection"
                .to_string(),
        );
    }

    // 6. Accepted-row digests and pointer-vs-candidate per-subject identity
    // bindings. Every row of the retained candidate must still hash to its
    // bound row digest, and every bound per-subject identity copy must agree
    // with the candidate rows (a tree/input/config identity edit in either
    // place is stale).
    let pointer_subjects = bound_subjects.as_object().ok_or_else(|| {
        fail(
            "pointer",
            "subjects",
            "pointer subject identities must be an object",
        )
    })?;
    let candidate_rows = candidate
        .get("repos")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            fail(
                "candidate",
                "repos",
                "retained candidate must carry a repos array",
            )
        })?;
    let mut row_digests: BTreeMap<String, String> = BTreeMap::new();
    for row in candidate_rows {
        let entry = as_object(row, "candidate", "repos", "retained candidate row")?;
        let id = entry
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let canonical = serde_json::to_vec_pretty(row).map_err(|error| {
            fail(
                &id,
                "row",
                format!("cannot canonicalize the accepted row: {error}"),
            )
        })?;
        let row_digest = sha256_hex(&canonical);
        row_digests.insert(id.clone(), row_digest);

        let bound = match pointer_subjects.get(&id) {
            Some(bound) => bound,
            None => {
                stale.push(format!(
                    "accepted subject `{id}` carries no pointer identity binding"
                ));
                continue;
            }
        };
        let bound = as_object(bound, &id, "subjects", "pointer subject identity")?;
        reject_unknown_keys(
            bound,
            &POINTER_SUBJECT_KEYS,
            &id,
            "pointer subject identity",
        )?;
        for field in POINTER_SUBJECT_KEYS {
            let Some(bound_value) = bound.get(field) else {
                // A missing subject identity is never silently skipped: an
                // unbound identity cannot be re-verified, so the verdict can
                // never read `current` on its strength. (`row_sha256`
                // absence is additionally refused outright by the row-digest
                // binding below; the toolchain-level analogues are disclosed
                // by the live source/binary comparisons in step 9.)
                unverifiable.push(format!(
                    "pointer binds no `{field}` for subject `{id}`; that identity cannot be re-verified"
                ));
                continue;
            };
            let current_value = match field {
                "row_sha256" => Some(Value::String(sha256_hex(&canonical))),
                "tree_digest" => entry.get("tree_digest").cloned(),
                "input_digest" => entry.get("input_digest").cloned(),
                "config_input" => entry
                    .get("config")
                    .and_then(|config| config.get("input"))
                    .cloned(),
                "config_profile" => entry
                    .get("config")
                    .and_then(|config| config.get("profile"))
                    .cloned(),
                _ => None,
            };
            match current_value {
                Some(value) if value != *bound_value => stale.push(format!(
                    "bound identity moved for subject `{id}`: pointer `{field}` no longer matches the accepted receipt"
                )),
                Some(_) => {}
                None => stale.push(format!(
                    "bound identity moved for subject `{id}`: pointer binds `{field}` but the accepted receipt records none"
                )),
            }
        }
    }
    if row_digests.len() != pointer_subjects.len() {
        stale.push(format!(
            "pointer binds {} subject identity entries but the accepted receipt carries {} rows",
            pointer_subjects.len(),
            row_digests.len()
        ));
    }
    for (id, digest) in &row_digests {
        match pointer_subjects.get(id).map(|bound| {
            bound
                .get("row_sha256")
                .and_then(Value::as_str)
                .map(str::to_string)
        }) {
            Some(Some(bound_row)) => {
                if bound_row != *digest {
                    stale.push(format!(
                        "accepted row bytes changed for subject `{id}`: pointer binds row sha256 `{bound_row}` but the row hashes to `{digest}`"
                    ));
                }
            }
            Some(None) => {
                return Err(fail(
                    id,
                    "subjects.row_sha256",
                    "pointer subject must bind its row digest",
                ));
            }
            None => stale.push(format!(
                "accepted subject `{id}` carries no pointer identity binding"
            )),
        }
    }

    // 7. The toolchain identity block: the pointer's bound copies must agree
    // with the retained candidate's envelope (an edited feature/build/source
    // copy in either place is stale).
    let pointer_ripr = pointer
        .get("ripr")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            fail(
                "pointer",
                "ripr",
                "pointer must bind the toolchain identity block",
            )
        })?;
    reject_unknown_keys(
        pointer_ripr,
        &POINTER_RIPR_KEYS,
        "pointer",
        "pointer ripr identity",
    )?;
    if let Some(bound_source) = pointer_ripr.get("source_sha").and_then(Value::as_str) {
        check_git_sha("pointer", "ripr.source_sha", bound_source)?;
    }
    if let Some(bound_binary) = pointer_ripr.get("binary_digest").and_then(Value::as_str) {
        check_sha256_digest("pointer", "ripr.binary_digest", bound_binary)?;
    }
    let candidate_ripr = candidate.get("ripr").and_then(Value::as_object);
    for field in POINTER_RIPR_KEYS {
        let Some(bound_value) = pointer_ripr.get(field) else {
            // A missing toolchain identity is never silently skipped: an
            // unbound identity cannot be re-verified, so the verdict can
            // never read `current` on its strength (the step-6 subject rule
            // applied to the toolchain block; the source/binary analogues
            // are additionally disclosed by the live comparisons in step 9).
            unverifiable.push(format!(
                "pointer binds no toolchain identity `{field}`; that identity cannot be re-verified"
            ));
            continue;
        };
        match candidate_ripr.and_then(|block| block.get(field)) {
            Some(candidate_value) if candidate_value != bound_value => stale.push(format!(
                "bound toolchain identity `{field}` no longer matches the retained candidate"
            )),
            Some(_) => {}
            None => stale.push(format!(
                "pointer binds toolchain identity `{field}` but the retained candidate records none"
            )),
        }
    }

    // 8. The receipt-vs-CURRENT-manifest validation: subject tree pins,
    // licenses, and the manifest digest binding inside the retained candidate
    // are re-checked against the manifest as it exists now. A changed tree
    // pin or any other manifest movement makes the accepted receipt
    // structurally stale. Skipped when the manifest itself failed to load or
    // validate: the 2b stale reason already owns the verdict and this
    // re-validation needs the validated manifest.
    if let Some(accepted) = inputs.accepted
        && let Err(error) = validate_run_receipt(
            candidate,
            current_manifest_sha256,
            accepted,
            "retained candidate",
        )
    {
        let first = error.lines().next().unwrap_or_default().to_string();
        stale.push(format!(
            "accepted receipt no longer validates against the current accepted state: {first}"
        ));
    }

    // 9. Live toolchain movement: analyzer source sha and binary bytes
    // against the recomputed current values; absent recompute inputs are
    // unverifiable (never assumed current).
    match (
        pointer_ripr.get("source_sha").and_then(Value::as_str),
        current.ripr_source_sha.as_deref(),
    ) {
        (Some(bound), Some(current_sha)) if bound != current_sha => stale.push(format!(
            "analyzer source moved: pointer binds source sha `{bound}` but the current source is `{current_sha}`"
        )),
        (Some(_), Some(_)) => {}
        (Some(_), None) => unverifiable.push(
            "analyzer source identity could not be recomputed (no git HEAD and no --ripr-source-sha); source currentness is unverifiable"
                .to_string(),
        ),
        (None, _) => unverifiable.push(
            "pointer binds no analyzer source sha; source currentness is unverifiable".to_string(),
        ),
    }
    match (
        pointer_ripr.get("binary_digest").and_then(Value::as_str),
        current.ripr_binary_digest.as_deref(),
    ) {
        (Some(bound), Some(current_digest)) if bound != current_digest => stale.push(format!(
            "analyzer binary moved: pointer binds binary sha256 `{bound}` but the supplied binary hashes to `{current_digest}`"
        )),
        (Some(_), Some(_)) => {}
        (Some(_), None) => unverifiable.push(
            "binary identity could not be recomputed (no --ripr-bin supplied); binary currentness is unverifiable"
                .to_string(),
        ),
        (None, _) => unverifiable.push(
            "pointer binds no binary digest; binary currentness is unverifiable".to_string(),
        ),
    }

    // 10. Config/input identity, PATH-BOUND: the pointer's per-subject
    // `config_input` must BE the manifest-declared synthetic diff for that
    // subject — a candidate that hashed a different portable file is stale,
    // not current, even when the digest matches the substituted bytes — and
    // the bound `input_digest` must match the recomputed digest of the
    // CURRENT bytes at the manifest-declared path (the recomputation hashes
    // the declared path only). A missing or unrecomputable subject identity
    // is disclosed unverifiable and can never leave the verdict `current`.
    // Skipped when the manifest failed to load or validate: the declared
    // input paths are unavailable then, and the 2b stale reason already owns
    // the verdict.
    if let Some(accepted) = inputs.accepted {
        for (id, bound) in pointer_subjects.iter() {
            let declared = accepted
                .subject(id)
                .map(|subject| subject.synthetic_diff.as_str());
            let bound_config = bound.get("config_input").and_then(Value::as_str);
            match (bound_config, declared) {
                (Some(bound), Some(declared_path)) if bound != declared_path => {
                    stale.push(format!(
                        "input path substituted for subject `{id}`: pointer binds config_input `{bound}` but the accepted manifest declares `{declared_path}`; the hashed input must be the manifest-declared input"
                    ));
                }
                (Some(_), Some(_)) => {}
                (Some(_), None) => {
                    // Outside the manifest denominator; step 8's re-validation
                    // against the current manifest already reports it stale.
                }
                (None, Some(declared_path)) => unverifiable.push(format!(
                    "input path for subject `{id}` could not be verified (the pointer binds no config_input while the accepted manifest declares `{declared_path}`)"
                )),
                (None, None) => {}
            }
            let bound_input = bound
                .get("input_digest")
                .and_then(Value::as_str)
                .map(str::to_string);
            match (bound_input, current.subject_inputs.get(id)) {
                (Some(bound), Some(recomputed)) if bound != *recomputed => stale.push(format!(
                    "input moved for subject `{id}`: pointer binds input sha256 `{bound}` but the current input hashes to `{recomputed}`"
                )),
                (Some(_), Some(_)) => {}
                (Some(_), None) => unverifiable.push(format!(
                    "input identity for subject `{id}` could not be recomputed (the manifest-declared input file did not resolve or read)"
                )),
                (None, recomputed) => unverifiable.push(format!(
                    "input identity for subject `{id}` could not be verified (the pointer binds no input digest; the manifest-declared input {})",
                    match recomputed {
                        Some(digest) => format!("currently hashes to `{digest}`"),
                        None => "did not resolve or read".to_string(),
                    }
                )),
            }
        }
    }

    let verdict = if !stale.is_empty() {
        CurrentnessVerdict::Stale
    } else if !unverifiable.is_empty() {
        CurrentnessVerdict::Unverifiable
    } else {
        CurrentnessVerdict::Current
    };
    Ok(CurrentnessComparison {
        verdict,
        stale,
        unverifiable,
    })
}

/// Recomputes the per-subject input digest from the CURRENT input files: the
/// manifest-DECLARED synthetic diff for each accepted subject — never a
/// candidate-named substitute path — resolved manifest-directory-relative
/// first, then repository-root-relative (the same resolution order the
/// refresh route documents). The pointer-vs-manifest path binding itself is
/// enforced in the comparison; this recomputation makes the hashed bytes the
/// bytes of the declared input by construction. Subjects whose input does
/// not resolve or read are omitted — disclosed unverifiable, never assumed
/// current.
fn recompute_subject_inputs(
    accepted: &AcceptedManifest,
    manifest_path: &str,
) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let manifest_dir = Path::new(manifest_path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    for subject in &accepted.subjects {
        let from_manifest_dir = manifest_dir.join(&subject.synthetic_diff);
        let from_repo_root = repo_root_anchor().join(&subject.synthetic_diff);
        let resolved = if from_manifest_dir.is_file() {
            Some(from_manifest_dir)
        } else if from_repo_root.is_file() {
            Some(from_repo_root)
        } else {
            None
        };
        if let Some(path) = resolved
            && let Ok(bytes) = std::fs::read(&path)
        {
            out.insert(subject.id.clone(), sha256_hex(&bytes));
        }
    }
    out
}

/// Resolves the current analyzer source sha: the explicit flag wins;
/// otherwise a bounded `git rev-parse HEAD` in the repository root. Any
/// failure is `None` — disclosed unverifiable, never guessed.
fn resolve_current_source_sha(flag: Option<&str>) -> Option<String> {
    if let Some(sha) = flag {
        return Some(sha.to_string());
    }
    let root = repo_root_anchor();
    let output = crate::run::capture_output_with_timeout(
        "git",
        &[
            "-C".to_string(),
            root.to_string_lossy().to_string(),
            "rev-parse".to_string(),
            "HEAD".to_string(),
        ],
        &[],
        std::time::Duration::from_secs(30),
        "eval-sweep report currentness source-sha probe",
    )
    .ok()?;
    if output.timed_out || !output.status.is_some_and(|status| status.success()) {
        return None;
    }
    let sha = output.stdout.trim().to_string();
    if sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(sha)
    } else {
        None
    }
}

fn run_currentness_check(parsed: &ReportArgs) -> Result<(), String> {
    let pointer_path = Path::new(&parsed.state_dir).join(POINTER_FILE);
    if !pointer_path.exists() {
        println!(
            "eval-sweep report currentness: pointer=<none> verdict=not_run (no accepted receipt published; not_run is not a pass)"
        );
        println!("rerun: {RERUN_COMMAND}");
        return Ok(());
    }
    let (pointer_value, _pointer_sha) = load_strict_json(&pointer_path.to_string_lossy())?;
    let pointer = as_object(&pointer_value, "pointer", "pointer", "current pointer")?;
    reject_unknown_keys(pointer, &POINTER_KEYS, "pointer", "current pointer")?;
    for (field, expected) in [
        ("schema_version", POINTER_SCHEMA),
        ("kind", POINTER_KIND),
        ("spec", SPEC),
    ] {
        let actual = pointer
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| fail("pointer", field, "current pointer must declare this field"))?;
        if actual != expected {
            return Err(fail(
                "pointer",
                field,
                format!("expected `{expected}`, got `{actual}`"),
            ));
        }
    }
    // The as-of disclosure is hygiene-checked but never enters the identity
    // comparison below (editing it can never repair staleness): when present
    // it must be a non-empty bounded string, so a malformed value cannot ride
    // along inside an otherwise current pointer.
    if let Some(as_of) = pointer.get("as_of") {
        let text = as_of.as_str().ok_or_else(|| {
            fail(
                "pointer",
                "as_of",
                "as-of must be a JSON string when present",
            )
        })?;
        if text.trim().is_empty() {
            return Err(fail(
                "pointer",
                "as_of",
                "as-of must be non-empty when present",
            ));
        }
        let length = text.chars().count();
        if length > NOTE_MAX_CHARS {
            return Err(fail(
                "pointer",
                "as_of",
                format!(
                    "as-of exceeds the {NOTE_MAX_CHARS}-character bound ({length} characters); accepted artifacts carry bounded excerpts, never unbounded logs"
                ),
            ));
        }
    }
    let receipt_file = pointer
        .get("receipt_file")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            fail(
                "pointer",
                "receipt_file",
                "pointer must name its accepted receipt",
            )
        })?;
    check_portable_path("pointer", "receipt_file", receipt_file)?;
    let receipt_path = split_portable("pointer", "receipt_file", &parsed.state_dir, receipt_file)?;
    if !receipt_path.exists() {
        return Err(fail(
            &receipt_path.to_string_lossy(),
            "file",
            "the pointer names an accepted receipt that does not exist; the accepted artifact is missing",
        ));
    }
    let receipt_bytes = std::fs::read(&receipt_path).map_err(|error| {
        fail(
            &receipt_path.to_string_lossy(),
            "file",
            format!("accepted receipt cannot be read: {error}"),
        )
    })?;
    let receipt_sha256 = sha256_hex(&receipt_bytes);
    let (accepted_receipt, _receipt_file_sha) = load_strict_json(&receipt_path.to_string_lossy())?;

    // The retained candidate: addressed by the digest the accepted receipt
    // binds. The rows, identity blocks, and validator bindings all compare
    // against these exact bytes.
    let candidate_binding = accepted_receipt
        .get("candidate")
        .and_then(|candidate| candidate.get("sha256"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            fail(
                "accepted receipt",
                "candidate.sha256",
                "accepted receipt must bind its candidate digest",
            )
        })?
        .to_string();
    let candidate_path = split_portable(
        "accepted receipt",
        "candidate.sha256",
        &parsed.state_dir,
        &format!("{RECEIPTS_DIR}/{candidate_binding}.candidate.json"),
    )?;
    if !candidate_path.exists() {
        return Err(fail(
            &candidate_path.to_string_lossy(),
            "file",
            "the accepted receipt's retained candidate does not exist; the accepted artifact is missing",
        ));
    }
    let candidate_bytes = std::fs::read(&candidate_path).map_err(|error| {
        fail(
            &candidate_path.to_string_lossy(),
            "file",
            format!("retained candidate cannot be read: {error}"),
        )
    })?;
    let candidate_sha256 = sha256_hex(&candidate_bytes);
    let (candidate_value, _candidate_file_sha) =
        load_strict_json(&candidate_path.to_string_lossy())?;

    // The current accepted manifest: the comparison baseline for both the
    // bound manifest digest and the candidate re-validation. The raw bytes
    // are hashed FIRST: a moved manifest is itself the promised `stale`
    // verdict, so the parse/validation is attempted separately and its
    // failure is carried into the comparison as an additional named stale
    // reason — a malformed changed manifest must reach the `stale` verdict,
    // never abort the check with a schema error.
    let manifest_bytes = std::fs::read(&parsed.manifest).map_err(|error| {
        fail(
            &parsed.manifest,
            "file",
            format!("accepted manifest cannot be read: {error}"),
        )
    })?;
    let current_manifest_sha256 = sha256_hex(&manifest_bytes);
    let manifest_state = load_strict_json(&parsed.manifest).and_then(|(manifest_value, _)| {
        validate_accepted_manifest(&manifest_value, current_manifest_sha256.clone())
    });
    let (accepted, manifest_validation_error) = match manifest_state {
        Ok(accepted) => (Some(accepted), None),
        Err(error) => (None, Some(error)),
    };

    let current = CurrentIdentity {
        ripr_source_sha: resolve_current_source_sha(parsed.ripr_source_sha.as_deref()),
        ripr_binary_digest: match &parsed.ripr_bin {
            Some(path) => {
                let bytes = std::fs::read(path).map_err(|error| {
                    fail(
                        path,
                        "ripr-bin",
                        format!("supplied binary cannot be read: {error}"),
                    )
                })?;
                Some(sha256_hex(&bytes))
            }
            None => None,
        },
        // Without a validated manifest the declared input paths are
        // unavailable; the comparison already carries the manifest failure as
        // a stale reason, so no per-subject recomputation is attempted.
        subject_inputs: accepted
            .as_ref()
            .map(|accepted| recompute_subject_inputs(accepted, &parsed.manifest))
            .unwrap_or_default(),
    };

    let comparison = compare_currentness(&CurrentnessInputs {
        pointer,
        accepted_receipt: &accepted_receipt,
        receipt_sha256: &receipt_sha256,
        candidate: &candidate_value,
        candidate_sha256: &candidate_sha256,
        accepted: accepted.as_ref(),
        current_manifest_sha256: &current_manifest_sha256,
        manifest_validation_error: manifest_validation_error.as_deref(),
        current: &current,
    })?;

    println!(
        "eval-sweep report currentness: pointer={} receipt={} verdict={}",
        pointer_path.to_string_lossy(),
        receipt_path.to_string_lossy(),
        comparison.verdict.as_str(),
    );
    for reason in &comparison.stale {
        println!("  stale: {reason}");
    }
    for reason in &comparison.unverifiable {
        println!("  unverifiable: {reason}");
    }
    println!(
        "eval-sweep report currentness verdict: {} — mechanical identity comparison; not a robustness or adequacy claim",
        comparison.verdict.as_str()
    );
    println!("rerun: {RERUN_COMMAND} --check-currentness");
    match comparison.verdict {
        CurrentnessVerdict::Current | CurrentnessVerdict::Unverifiable => Ok(()),
        // Stale is the gate signal: the accepted receipt is no longer current
        // for promotion consumption, so the command exits nonzero — with the
        // exact movements named, so the consumer knows what to re-accept.
        CurrentnessVerdict::Stale => {
            let mut error = format!(
                "eval-sweep report currentness: pointer is STALE ({} stale reason(s)); the accepted receipt must be re-accepted from a fresh candidate before promotion consumption",
                comparison.stale.len()
            );
            for reason in &comparison.stale {
                error.push_str(&format!("\n  stale: {reason}"));
            }
            error.push_str(&format!("\nrerun: {RERUN_COMMAND}"));
            Err(error)
        }
    }
}

/// Joins a state dir with a portable (forward-slash) pointer-relative path on
/// any host. The split is the containment boundary: the portable path is
/// rejected unless every `/`-separated component is non-empty and non-dot
/// (`split` preserves empty components, and host path resolution skips them,
/// so an unguarded join could walk outside the accepted state directory).
fn split_portable(
    subject: &str,
    field: &str,
    state_dir: &str,
    portable: &str,
) -> Result<PathBuf, String> {
    check_portable_components(subject, field, portable)?;
    let mut path = PathBuf::from(state_dir);
    for component in portable.split('/') {
        path.push(component);
    }
    Ok(path)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub(crate) fn run_report(args: &[String]) -> Result<(), String> {
    let parsed = parse_report_args(args)?;
    if parsed.check_currentness {
        run_currentness_check(&parsed)
    } else {
        run_candidate_report(&parsed)
    }
}

// ---------------------------------------------------------------------------
// Tests (module named `python_eval_sweep_report` so
// `cargo test -p xtask python_eval_sweep_report` selects exactly this module;
// no unwrap/expect — assert macros and Result returns only)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod python_eval_sweep_report {
    use super::*;
    use crate::python_judged_panel::parse_json_without_duplicate_keys;

    const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const SHA_C: &str = "cccccccccccccccccccccccccccccccccccccccc";
    const SHA_D: &str = "dddddddddddddddddddddddddddddddddddddddd";
    const SHA_E: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const SHA_F: &str = "ffffffffffffffffffffffffffffffffffffffff";
    const SHA_0: &str = "1010101010101010101010101010101010101010";
    const SHA_1: &str = "1111111111111111111111111111111111111111";
    const DIGEST_ONE: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const DIGEST_TWO: &str = "2222222222222222222222222222222222222222222222222222222222222222";
    const SOURCE_SHA: &str = "9999aaaabbbbccccddddeeeeffff000011112222";
    const BINARY_BYTES_V1: &str = "test-binary-bytes-v1";
    const BINARY_BYTES_V2: &str = "test-binary-bytes-v2";

    /// The per-subject tree identity, recorded identically by the sandbox
    /// manifest and the candidate rows (the validator binds the two copies).
    /// With every per-subject identity bound, the currentness happy path is
    /// genuinely `current`; a subject with an unbound identity is disclosed
    /// unverifiable and can never read `current`.
    fn tree_digest_for(id: &str) -> String {
        sha256_hex(format!("tree-for-{id}\n").as_bytes())
    }

    /// A data-driven eight-subject manifest sandbox whose synthetic diffs
    /// exist as real files under `<dir>/diffs/<id>.diff`, so the currentness
    /// input recomputation reads real bytes.
    struct TestSandbox {
        dir: PathBuf,
    }

    impl TestSandbox {
        fn new(label: &str) -> Result<Self, String> {
            let dir = std::env::temp_dir().join(format!(
                "ripr-evalsweep-report-{label}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("diffs"))
                .map_err(|error| format!("create sandbox: {error}"))?;
            Ok(Self { dir })
        }

        fn path(&self, relative: &str) -> PathBuf {
            let mut path = self.dir.clone();
            for component in relative.split('/') {
                path.push(component);
            }
            path
        }

        fn manifest_value(&self) -> Value {
            let ids = [
                ("alpha", SHA_A, "pytest_library", "MIT"),
                ("bravo", SHA_B, "unittest_library", "Apache-2.0"),
                ("charlie", SHA_C, "click_typer", "BSD-3-Clause"),
                ("delta", SHA_D, "pytest_library", "MIT"),
                ("echo", SHA_E, "flask_web", "MIT"),
                ("foxtrot", SHA_F, "fastapi_web", "MIT OR Apache-2.0"),
                ("golf", SHA_0, "pytest_library", "ISC"),
                ("hotel", SHA_1, "pytest_library", "BSD-2-Clause"),
            ];
            let mut repos = Vec::new();
            for (id, sha, shape, license) in ids {
                repos.push(json!({
                    "id": id,
                    "url": format!("https://example.com/{id}"),
                    "sha": sha,
                    "license": license,
                    "shape": shape,
                    "tree_digest": tree_digest_for(id),
                    "synthetic_diff": format!("diffs/{id}.diff"),
                }));
            }
            json!({
                "schema_version": "0.1",
                "kind": "python_eval_sweep_manifest",
                "spec": SPEC,
                "tier": TIER,
                "description": "report-route sandbox manifest",
                "repos": repos,
            })
        }

        /// Writes the manifest and per-subject diff files; returns the
        /// manifest digest over the exact file bytes.
        fn write_manifest(&self) -> Result<String, String> {
            let value = self.manifest_value();
            let text = serde_json::to_string_pretty(&value)
                .map_err(|error| format!("serialize manifest: {error}"))?;
            let bytes = text.as_bytes();
            std::fs::write(self.path("manifest.json"), bytes)
                .map_err(|error| format!("write manifest: {error}"))?;
            for id in [
                "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel",
            ] {
                std::fs::write(
                    self.path(&format!("diffs/{id}.diff")),
                    format!("diff-for-{id}\n"),
                )
                .map_err(|error| format!("write diff: {error}"))?;
            }
            Ok(sha256_hex(bytes))
        }

        fn manifest_path_string(&self) -> String {
            self.path("manifest.json").to_string_lossy().to_string()
        }

        fn state_dir(&self) -> String {
            self.path("accepted").to_string_lossy().to_string()
        }

        fn dispositions_path(&self) -> Result<String, String> {
            let path = self.path("dispositions.json");
            write_json(&path, &dispositions_value())?;
            Ok(path.to_string_lossy().to_string())
        }
    }

    /// The test binary whose bytes hash to `binary_digest_v1()`; a second,
    /// distinct binary is `binary_bytes_v2`.
    fn binary_digest_v1() -> String {
        sha256_hex(BINARY_BYTES_V1.as_bytes())
    }

    /// A valid schema-0.3 candidate for the sandbox manifest. Statuses cover
    /// the full vocabulary (row 0 complete, then partial/parse-failed/
    /// timed-out/crashed/unsupported/tempfail/stale); the summary is derived
    /// honestly exactly as the validator requires. `runtime_base` lets a test
    /// build a second, distinct-but-valid candidate.
    fn candidate_value(
        sandbox: &TestSandbox,
        manifest_sha: &str,
        binary_digest: &str,
        runtime_base: u64,
    ) -> Value {
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
        let manifest = sandbox.manifest_value();
        let repos = manifest.get("repos").and_then(Value::as_array);
        let mut rows = Vec::new();
        for (index, repo) in repos.into_iter().flatten().enumerate() {
            let id = repo.get("id").and_then(Value::as_str).unwrap_or_default();
            let sha = repo.get("sha").and_then(Value::as_str).unwrap_or_default();
            let license = repo
                .get("license")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let status = statuses[index];
            let execution = executions[index];
            let ran = matches!(
                status,
                "complete" | "partial" | "parse-failed" | "timed-out" | "crashed"
            );
            let mut row = json!({
                "id": id,
                "status": status,
                "repository": {
                    "url": format!("https://example.com/{id}"),
                    "sha": sha,
                },
                "license": license,
                "selected_root": format!("subjects/{id}"),
                "layout": ["pytest_library"],
                "tree_digest": tree_digest_for(id),
                "binary": {
                    "digest": binary_digest,
                    "version": "ripr 0.11.0",
                    "features": ["python"],
                    "build_profile": "debug",
                },
                "config": {
                    "profile": "default",
                    "input": format!("diffs/{id}.diff"),
                },
                "input_digest": sha256_hex(format!("diff-for-{id}\n").as_bytes()),
                "materialization": if ran { "materialized" } else { "absent" },
                "detection": if ran { "detected" } else { "absent" },
                "corpus_selection": {
                    "state": if ran { "selected" } else { "absent" },
                    "source_files": 10,
                    "test_files": 5,
                    "generated_files": 0,
                    "vendor_files": 0,
                },
                "execution": execution,
                "runtime_ms": runtime_base + index as u64 * 100,
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
            if ran && let Some(entry) = row.as_object_mut() {
                entry.insert(
                    "digests".to_string(),
                    json!({
                        "raw": DIGEST_ONE,
                        "output": DIGEST_TWO,
                        "evidence": DIGEST_ONE,
                    }),
                );
            }
            if status == "complete"
                && let Some(entry) = row.as_object_mut()
            {
                entry.insert(
                    "repeat".to_string(),
                    json!({
                        "comparable_with": "pass-1",
                        "gap_ids_stable": true,
                        "unstable_gap_ids": [],
                    }),
                );
            }
            if !ran && let Some(entry) = row.as_object_mut() {
                entry.insert(
                    "classification_counts".to_string(),
                    json!({"exposed": 0, "weakly_exposed": 0, "reachable_unrevealed": 0, "no_static_path": 0, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0}),
                );
                entry.insert(
                    "alignment_counts".to_string(),
                    json!({"direct": 0, "alias": 0, "changed_sink_token": 0, "orthogonal": 0, "unknown": 0, "absent": 0}),
                );
                entry.remove("runtime_ms");
            }
            rows.push(row);
        }
        // Derived aggregates over the five run rows (complete/partial/
        // parse-failed/timed-out/crashed): one crash, one parse failure, one
        // timeout; runtimes base..base+400 (min base, median base+200, max
        // base+400, total base*5+1000); classification weakly_exposed=1 +
        // static_unknown=1; alignment orthogonal=1, unknown=1, absent=3.
        // Only the complete row carries repeat evidence, so the stability
        // aggregates are omitted (under-evidenced — the producer shape).
        json!({
            "schema_version": "0.3",
            "kind": "python_eval_sweep_report",
            "spec": SPEC,
            "tier": TIER,
            "manifest_digest": manifest_sha,
            "ripr": {
                "source_sha": SOURCE_SHA,
                "tree_digest": DIGEST_ONE,
                "binary_digest": binary_digest,
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
                "runtime_ms_min": runtime_base,
                "runtime_ms_median": runtime_base + 200,
                "runtime_ms_max": runtime_base + 400,
                "runtime_ms_total": runtime_base * 5 + 1000,
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

    /// Terminal dispositions for the seven non-complete rows of the standard
    /// sandbox candidate.
    fn dispositions_value() -> Value {
        let entries = [
            ("bravo", "dispositioned-current"),
            ("charlie", "reproduced-current"),
            ("delta", "historical-not-reproduced"),
            ("echo", "reproduced-current"),
            ("foxtrot", "unsupported-input"),
            ("golf", "infrastructure-tempfail"),
            ("hotel", "upstream-pin-unavailable"),
        ];
        let mut dispositions = Vec::new();
        for (id, disposition) in entries {
            dispositions.push(json!({
                "id": id,
                "disposition": disposition,
                "evidence_ref": format!("digests.evidence for {id}"),
                "owner": "language-adapter",
                "recovery_route": "rerun the managed refresh after the fix lands",
            }));
        }
        json!({
            "schema_version": "0.1",
            "kind": DISPOSITIONS_KIND,
            "spec": SPEC,
            "dispositions": dispositions,
        })
    }

    fn write_json(path: &Path, value: &Value) -> Result<(), String> {
        let text =
            serde_json::to_string_pretty(value).map_err(|error| format!("serialize: {error}"))?;
        std::fs::write(path, format!("{text}\n")).map_err(|error| format!("write: {error}"))
    }

    fn args_for(sandbox: &TestSandbox, extra: &[&str]) -> Vec<String> {
        let mut args = vec![
            "--candidate".to_string(),
            sandbox.path("candidate.json").to_string_lossy().to_string(),
            "--manifest".to_string(),
            sandbox.manifest_path_string(),
            "--state-dir".to_string(),
            sandbox.state_dir(),
        ];
        args.extend(extra.iter().map(|text| text.to_string()));
        args
    }

    /// Builds the sandbox, writes the manifest + a valid candidate, and
    /// returns the sandbox with the parsed report args (dry run by default).
    fn prepared_args(label: &str, extra: &[&str]) -> Result<(TestSandbox, Vec<String>), String> {
        let sandbox = TestSandbox::new(label)?;
        let manifest_sha = sandbox.write_manifest()?;
        let candidate = candidate_value(&sandbox, &manifest_sha, &binary_digest_v1(), 100);
        write_json(&sandbox.path("candidate.json"), &candidate)?;
        let args = args_for(&sandbox, extra);
        Ok((sandbox, args))
    }

    /// Parses a written file as strict JSON.
    fn read_strict(path: &Path) -> Result<Value, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("read {}: {error}", path.to_string_lossy()))?;
        parse_json_without_duplicate_keys(&text)
            .map_err(|error| format!("parse {}: {error}", path.to_string_lossy()))
    }

    fn expect_fail(result: Result<(), String>, needle: &str) -> Result<(), String> {
        expect_fail_all(result, &[needle])
    }

    /// Asserts a fail-closed result: Err, mentioning EVERY needle and the
    /// rerun command.
    fn expect_fail_all(result: Result<(), String>, needles: &[&str]) -> Result<(), String> {
        let error = match result {
            Ok(()) => {
                return Err(format!(
                    "expected failure containing `{needles:?}`, got success"
                ));
            }
            Err(error) => error,
        };
        for needle in needles {
            if !error.contains(needle) {
                return Err(format!("failure `{error}` must mention `{needle}`"));
            }
        }
        // Refusals the shared validator raises carry its own rerun command
        // (`eval-sweep check`); the report route's own refusals carry this
        // command. Either is acceptable: both name a deterministic rerun.
        if !error.contains("rerun: cargo xtask eval-sweep") {
            return Err(format!("failure `{error}` must carry a rerun command"));
        }
        Ok(())
    }

    fn with_dispositions(args: &[String], dispositions: &str) -> Vec<String> {
        args.iter()
            .map(|arg| arg.replace("DISPOSITIONS", dispositions))
            .collect()
    }

    fn dry_run_dispositions(sandbox: &TestSandbox, args: &[String]) -> Result<Vec<String>, String> {
        let dispositions = sandbox.dispositions_path()?;
        Ok(with_dispositions(args, &dispositions))
    }

    /// Reads the accepted receipt the sandbox pointer names.
    fn accepted_receipt(sandbox: &TestSandbox) -> Result<(Value, PathBuf), String> {
        let pointer = read_strict(&sandbox.path("accepted/current.json"))?;
        let receipt_file = pointer
            .get("receipt_file")
            .and_then(Value::as_str)
            .ok_or_else(|| "pointer must name its receipt".to_string())?
            .to_string();
        let receipt_path = sandbox.path(&format!("accepted/{receipt_file}"));
        let receipt = read_strict(&receipt_path)?;
        Ok((receipt, receipt_path))
    }

    /// Currentness args with matching identity inputs (the source sha and
    /// binary the candidate bound).
    fn currentness_args(sandbox: &TestSandbox, binary_path: &Path, extra: &[&str]) -> Vec<String> {
        let mut args = vec![
            "--check-currentness".to_string(),
            "--manifest".to_string(),
            sandbox.manifest_path_string(),
            "--state-dir".to_string(),
            sandbox.state_dir(),
            "--ripr-source-sha".to_string(),
            SOURCE_SHA.to_string(),
            "--ripr-bin".to_string(),
            binary_path.to_string_lossy().to_string(),
        ];
        args.extend(extra.iter().map(|text| text.to_string()));
        args
    }

    fn write_binary_v1(sandbox: &TestSandbox) -> Result<PathBuf, String> {
        let path = sandbox.path("ripr-v1.bin");
        std::fs::write(&path, BINARY_BYTES_V1).map_err(|error| format!("write binary: {error}"))?;
        Ok(path)
    }

    fn write_binary_v2(sandbox: &TestSandbox) -> Result<PathBuf, String> {
        let path = sandbox.path("ripr-v2.bin");
        std::fs::write(&path, BINARY_BYTES_V2).map_err(|error| format!("write binary: {error}"))?;
        Ok(path)
    }

    /// Prepares an accepted sandbox: manifest + candidate + dispositions,
    /// accepted with pointer. Returns the sandbox.
    fn accepted_sandbox(label: &str) -> Result<TestSandbox, String> {
        let (sandbox, args) =
            prepared_args(label, &["--dispositions", "DISPOSITIONS", "--accept"])?;
        let dispositions = sandbox.dispositions_path()?;
        run_report(&with_dispositions(&args, &dispositions))?;
        Ok(sandbox)
    }

    // -- happy paths ---------------------------------------------------------

    #[test]
    fn bare_command_requires_a_mode() -> Result<(), String> {
        let error = match run_report(&[]) {
            Ok(()) => return Err("bare invocation must fail with usage".to_string()),
            Err(error) => error,
        };
        assert!(
            error.contains("requires --candidate") && error.contains("--check-currentness"),
            "bare invocation must explain its modes: {error}"
        );
        Ok(())
    }

    #[test]
    fn valid_candidate_dry_run_renders_agreeing_json_and_markdown() -> Result<(), String> {
        let (sandbox, args) = prepared_args("dry-run", &["--dispositions", "DISPOSITIONS"])?;
        let args = dry_run_dispositions(&sandbox, &args)?;
        run_report(&args)?;

        let report_text = std::fs::read_to_string(crate::reports_dir().join(REPORT_JSON))
            .map_err(|error| format!("read report: {error}"))?;
        let receipt = parse_json_without_duplicate_keys(&report_text)
            .map_err(|error| format!("parse report: {error}"))?;
        let markdown = std::fs::read_to_string(crate::reports_dir().join(REPORT_MD))
            .map_err(|error| format!("read markdown: {error}"))?;

        // JSON and Markdown derive from the same validated rows: same
        // subjects, same counts.
        let subjects = receipt
            .get("subjects")
            .and_then(Value::as_array)
            .ok_or_else(|| "receipt must carry subjects".to_string())?;
        assert_eq!(subjects.len(), 8, "all eight subjects appear exactly once");
        let mut ids_in_json = Vec::new();
        for subject in subjects {
            ids_in_json.push(
                subject
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "subject id".to_string())?
                    .to_string(),
            );
        }
        for id in [
            "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel",
        ] {
            assert!(ids_in_json.contains(&id.to_string()), "{id} must appear");
            assert!(markdown.contains(id), "markdown must name {id}");
        }
        let counts = receipt
            .get("counts")
            .and_then(Value::as_object)
            .ok_or_else(|| "receipt counts".to_string())?;
        for (name, count) in counts {
            let numerator = count.get("numerator").and_then(Value::as_u64);
            let denominator = count.get("denominator").and_then(Value::as_u64);
            assert!(
                numerator.is_some() && denominator.is_some(),
                "count `{name}` must carry numerator and denominator"
            );
            let numerator = numerator.unwrap_or(0);
            assert!(
                markdown.contains(&numerator.to_string()),
                "markdown must agree with count {name}={numerator}"
            );
        }
        let numerator = |name: &str| -> Result<u64, String> {
            counts
                .get(name)
                .and_then(|count| count.get("numerator"))
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("count {name}"))
        };
        assert_eq!(numerator("selected")?, 8);
        assert_eq!(numerator("run")?, 5);
        // Dispositions projected on non-complete subjects; none on complete.
        let disposed = subjects
            .iter()
            .filter(|subject| subject.get("disposition").is_some())
            .count();
        assert_eq!(
            disposed, 7,
            "every non-complete subject carries a disposition"
        );
        let alpha = subjects
            .iter()
            .find(|subject| subject.get("id").and_then(Value::as_str) == Some("alpha"))
            .ok_or_else(|| "alpha row".to_string())?;
        assert!(
            alpha.get("disposition").is_none(),
            "complete rows carry no disposition"
        );
        // Non-claims are embedded in the artifact itself.
        let non_claims = receipt
            .get("non_claims")
            .and_then(Value::as_array)
            .ok_or_else(|| "non_claims".to_string())?;
        assert!(!non_claims.is_empty());
        Ok(())
    }

    #[test]
    fn accept_appends_immutably_and_moves_pointer() -> Result<(), String> {
        let sandbox = TestSandbox::new("accept-immut")?;
        let manifest_sha = sandbox.write_manifest()?;
        let dispositions = sandbox.dispositions_path()?;

        let candidate_a = candidate_value(&sandbox, &manifest_sha, &binary_digest_v1(), 100);
        write_json(&sandbox.path("candidate.json"), &candidate_a)?;
        run_report(&with_dispositions(
            &args_for(&sandbox, &["--dispositions", "DISPOSITIONS", "--accept"]),
            &dispositions,
        ))?;

        let pointer_path = sandbox.path("accepted/current.json");
        let pointer_one = read_strict(&pointer_path)?;
        let receipt_file_one = pointer_one
            .get("receipt_file")
            .and_then(Value::as_str)
            .ok_or_else(|| "receipt_file".to_string())?
            .to_string();
        let receipt_bytes_one =
            std::fs::read(sandbox.path(&format!("accepted/{receipt_file_one}")))
                .map_err(|error| format!("read receipt one: {error}"))?;

        // A second, distinct-but-valid candidate (different runtimes),
        // accepted afterwards: adds a receipt without mutating the first; the
        // pointer moves to the newest.
        let candidate_b = candidate_value(&sandbox, &manifest_sha, &binary_digest_v1(), 500);
        write_json(&sandbox.path("candidate.json"), &candidate_b)?;
        run_report(&with_dispositions(
            &args_for(&sandbox, &["--dispositions", "DISPOSITIONS", "--accept"]),
            &dispositions,
        ))?;

        let pointer_two = read_strict(&pointer_path)?;
        let receipt_file_two = pointer_two
            .get("receipt_file")
            .and_then(Value::as_str)
            .ok_or_else(|| "receipt_file".to_string())?
            .to_string();
        assert_ne!(
            receipt_file_one, receipt_file_two,
            "the second accept must add a new content-addressed receipt"
        );
        let receipt_bytes_two =
            std::fs::read(sandbox.path(&format!("accepted/{receipt_file_two}")))
                .map_err(|error| format!("read receipt two: {error}"))?;
        assert_eq!(
            receipt_bytes_one,
            std::fs::read(sandbox.path(&format!("accepted/{receipt_file_one}")))
                .map_err(|error| format!("re-read receipt one: {error}"))?,
            "the first accepted receipt must be byte-identical after the second accept"
        );

        // The pointer identifies exactly the newest accepted receipt.
        let receipt_sha = sha256_hex(&receipt_bytes_two);
        assert_eq!(
            pointer_two.get("receipt_sha256").and_then(Value::as_str),
            Some(receipt_sha.as_str()),
        );
        assert_eq!(
            pointer_two.get("receipt_file").and_then(Value::as_str),
            Some(receipt_file_two.as_str()),
        );

        // Re-accepting the SAME candidate is an idempotent no-op: no third
        // receipt, artifacts untouched, pointer unchanged.
        run_report(&with_dispositions(
            &args_for(&sandbox, &["--dispositions", "DISPOSITIONS", "--accept"]),
            &dispositions,
        ))?;
        let receipts = std::fs::read_dir(sandbox.path("accepted/receipts"))
            .map_err(|error| format!("read receipts dir: {error}"))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                name.ends_with(".json") && !name.ends_with(".candidate.json")
            })
            .count();
        assert_eq!(receipts, 2, "idempotent re-accept must not add a receipt");
        Ok(())
    }

    #[test]
    fn pointer_contains_no_totals_only_identity() -> Result<(), String> {
        let (sandbox, args) = prepared_args(
            "pointer-shape",
            &[
                "--dispositions",
                "DISPOSITIONS",
                "--accept",
                "--as-of",
                "2026-09-10T00:00:00Z",
            ],
        )?;
        let dispositions = sandbox.dispositions_path()?;
        run_report(&with_dispositions(&args, &dispositions))?;

        let pointer = read_strict(&sandbox.path("accepted/current.json"))?;
        let mut keys: Vec<&str> = pointer
            .as_object()
            .ok_or_else(|| "pointer object".to_string())?
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "as_of",
                "command_contract_version",
                "kind",
                "manifest_sha256",
                "receipt_file",
                "receipt_sha256",
                "ripr",
                "schema_version",
                "spec",
                "subjects"
            ],
            "the pointer's field set is exactly identity fields"
        );
        // No total/rate-shaped field anywhere in the pointer tree.
        let lowered = serde_json::to_string(&pointer)
            .map_err(|error| format!("serialize pointer: {error}"))?
            .to_ascii_lowercase();
        for banned in [
            "\"total",
            "count\"",
            "\"rate",
            "numerator",
            "denominator",
            "crash",
            "runtime_ms",
        ] {
            assert!(
                !lowered.contains(banned),
                "pointer must not carry totals/rates; found `{banned}`"
            );
        }
        Ok(())
    }

    // -- candidate validation failures ---------------------------------------

    #[test]
    fn hand_edited_candidate_totals_fail() -> Result<(), String> {
        let (sandbox, args) = prepared_args("hand-edited", &["--dispositions", "DISPOSITIONS"])?;
        let mut candidate = read_strict(&sandbox.path("candidate.json"))?;
        if let Some(summary) = candidate.get_mut("summary").and_then(Value::as_object_mut) {
            summary.insert("repos_total".to_string(), json!(9));
        }
        write_json(&sandbox.path("candidate.json"), &candidate)?;
        let dispositions = sandbox.dispositions_path()?;
        expect_fail(
            run_report(&with_dispositions(&args, &dispositions)),
            "hand-edited aggregate",
        )
    }

    #[test]
    fn missing_and_duplicate_subjects_fail() -> Result<(), String> {
        let (sandbox, args) =
            prepared_args("missing-subject", &["--dispositions", "DISPOSITIONS"])?;
        let mut candidate = read_strict(&sandbox.path("candidate.json"))?;
        if let Some(rows) = candidate.get_mut("repos").and_then(Value::as_array_mut) {
            rows.pop();
        }
        write_json(&sandbox.path("candidate.json"), &candidate)?;
        let dispositions = sandbox.dispositions_path()?;
        expect_fail(
            run_report(&with_dispositions(&args, &dispositions)),
            "changed denominator",
        )?;

        let (sandbox, args) = prepared_args("dup-subject", &["--dispositions", "DISPOSITIONS"])?;
        let mut candidate = read_strict(&sandbox.path("candidate.json"))?;
        if let Some(rows) = candidate.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(first) = rows.first().cloned()
        {
            rows.push(first);
        }
        write_json(&sandbox.path("candidate.json"), &candidate)?;
        let dispositions = sandbox.dispositions_path()?;
        expect_fail(
            run_report(&with_dispositions(&args, &dispositions)),
            "duplicate subject row",
        )
    }

    #[test]
    fn non_complete_row_without_disposition_fails() -> Result<(), String> {
        // No --dispositions at all: the seven non-complete rows are
        // unexplained, so the report refuses.
        let (_sandbox, args) = prepared_args("no-dispositions", &[])?;
        expect_fail(run_report(&args), "carry no terminal disposition")
    }

    #[test]
    fn disposition_gaps_fail_closed() -> Result<(), String> {
        // Missing one disposition (hotel's): its non-complete row is
        // unexplained.
        let (sandbox, args) = prepared_args("disp-gap", &["--dispositions", "DISPOSITIONS"])?;
        let mut value = dispositions_value();
        if let Some(entries) = value.get_mut("dispositions").and_then(Value::as_array_mut) {
            entries.retain(|entry| entry.get("id").and_then(Value::as_str) != Some("hotel"));
        }
        let dispositions = {
            let path = sandbox.path("dispositions.json");
            write_json(&path, &value)?;
            path.to_string_lossy().to_string()
        };
        expect_fail(
            run_report(&with_dispositions(&args, &dispositions)),
            "hotel",
        )?;

        // A disposition without its owner fails (every owned disposition type
        // is actionable by definition).
        let (sandbox, args) = prepared_args("disp-owner", &["--dispositions", "DISPOSITIONS"])?;
        let mut value = dispositions_value();
        if let Some(first) = value
            .get_mut("dispositions")
            .and_then(Value::as_array_mut)
            .and_then(|entries| entries.first_mut())
            .and_then(|entry| entry.as_object_mut())
        {
            first.remove("owner");
        }
        let dispositions = {
            let path = sandbox.path("dispositions.json");
            write_json(&path, &value)?;
            path.to_string_lossy().to_string()
        };
        expect_fail(
            run_report(&with_dispositions(&args, &dispositions)),
            "requires a non-empty owner",
        )?;

        // The same for the recovery route.
        let (sandbox, args) = prepared_args("disp-route", &["--dispositions", "DISPOSITIONS"])?;
        let mut value = dispositions_value();
        if let Some(first) = value
            .get_mut("dispositions")
            .and_then(Value::as_array_mut)
            .and_then(|entries| entries.first_mut())
            .and_then(|entry| entry.as_object_mut())
        {
            first.remove("recovery_route");
        }
        let dispositions = {
            let path = sandbox.path("dispositions.json");
            write_json(&path, &value)?;
            path.to_string_lossy().to_string()
        };
        expect_fail(
            run_report(&with_dispositions(&args, &dispositions)),
            "requires a non-empty recovery_route",
        )?;

        // An oversized owner defeats the bounded-artifact contract exactly
        // like an oversized note: every bounded-artifact field is capped.
        let (sandbox, args) =
            prepared_args("disp-huge-owner", &["--dispositions", "DISPOSITIONS"])?;
        let mut value = dispositions_value();
        if let Some(first) = value
            .get_mut("dispositions")
            .and_then(Value::as_array_mut)
            .and_then(|entries| entries.first_mut())
            .and_then(|entry| entry.as_object_mut())
        {
            first.insert("owner".to_string(), json!("x".repeat(NOTE_MAX_CHARS + 1)));
        }
        let dispositions = {
            let path = sandbox.path("dispositions.json");
            write_json(&path, &value)?;
            path.to_string_lossy().to_string()
        };
        expect_fail(
            run_report(&with_dispositions(&args, &dispositions)),
            "exceeds the 512-character bound",
        )?;

        // A disposition for a COMPLETE row contradicts the run.
        let (sandbox, args) = prepared_args("disp-complete", &["--dispositions", "DISPOSITIONS"])?;
        let mut value = dispositions_value();
        if let Some(entries) = value.get_mut("dispositions").and_then(Value::as_array_mut) {
            entries.push(json!({
                "id": "alpha",
                "disposition": "dispositioned-current",
                "evidence_ref": "none",
                "owner": "language-adapter",
                "recovery_route": "none needed",
            }));
        }
        let dispositions = {
            let path = sandbox.path("dispositions.json");
            write_json(&path, &value)?;
            path.to_string_lossy().to_string()
        };
        expect_fail(
            run_report(&with_dispositions(&args, &dispositions)),
            "a terminal disposition contradicts a complete run",
        )?;

        // Duplicate ids fail.
        let (sandbox, args) = prepared_args("disp-dup", &["--dispositions", "DISPOSITIONS"])?;
        let mut value = dispositions_value();
        if let Some(entries) = value.get_mut("dispositions").and_then(Value::as_array_mut)
            && let Some(first) = entries.first().cloned()
        {
            entries.push(first);
        }
        let dispositions = {
            let path = sandbox.path("dispositions.json");
            write_json(&path, &value)?;
            path.to_string_lossy().to_string()
        };
        expect_fail(
            run_report(&with_dispositions(&args, &dispositions)),
            "duplicate disposition",
        )?;

        // Unknown ids fail.
        let (sandbox, args) = prepared_args("disp-unknown", &["--dispositions", "DISPOSITIONS"])?;
        let mut value = dispositions_value();
        if let Some(entries) = value.get_mut("dispositions").and_then(Value::as_array_mut) {
            entries.push(json!({
                "id": "outsider",
                "disposition": "dispositioned-current",
                "evidence_ref": "none",
                "owner": "language-adapter",
                "recovery_route": "none",
            }));
        }
        let dispositions = {
            let path = sandbox.path("dispositions.json");
            write_json(&path, &value)?;
            path.to_string_lossy().to_string()
        };
        expect_fail(
            run_report(&with_dispositions(&args, &dispositions)),
            "outside the candidate denominator",
        )
    }

    #[test]
    fn disposition_vocabulary_and_hygiene_fail_closed() -> Result<(), String> {
        // Unknown disposition type.
        let (sandbox, args) = prepared_args("disp-vocab", &["--dispositions", "DISPOSITIONS"])?;
        let mut value = dispositions_value();
        if let Some(first) = value
            .get_mut("dispositions")
            .and_then(Value::as_array_mut)
            .and_then(|entries| entries.first_mut())
            .and_then(|entry| entry.as_object_mut())
        {
            first.insert("disposition".to_string(), json!("mostly_fine"));
        }
        let dispositions = {
            let path = sandbox.path("dispositions.json");
            write_json(&path, &value)?;
            path.to_string_lossy().to_string()
        };
        expect_fail(
            run_report(&with_dispositions(&args, &dispositions)),
            "unknown disposition",
        )?;

        // An absolute host path in a disposition note fails the artifact
        // hygiene scan (the drive letter is assembled at runtime so this
        // source file never contains a local absolute path).
        let (sandbox, args) = prepared_args("disp-abs", &["--dispositions", "DISPOSITIONS"])?;
        let mut value = dispositions_value();
        let absolute = format!("{}:\\Users\\agent\\notes.txt", 'C');
        if let Some(first) = value
            .get_mut("dispositions")
            .and_then(Value::as_array_mut)
            .and_then(|entries| entries.first_mut())
            .and_then(|entry| entry.as_object_mut())
        {
            first.insert("notes".to_string(), json!(absolute));
        }
        let dispositions = {
            let path = sandbox.path("dispositions.json");
            write_json(&path, &value)?;
            path.to_string_lossy().to_string()
        };
        expect_fail(
            run_report(&with_dispositions(&args, &dispositions)),
            "drive-letter absolute path",
        )?;

        // A secret-shaped token fails the hygiene scan.
        let (sandbox, args) = prepared_args("disp-secret", &["--dispositions", "DISPOSITIONS"])?;
        let mut value = dispositions_value();
        if let Some(first) = value
            .get_mut("dispositions")
            .and_then(Value::as_array_mut)
            .and_then(|entries| entries.first_mut())
            .and_then(|entry| entry.as_object_mut())
        {
            first.insert(
                "notes".to_string(),
                json!("retry with api_key=hunter2 or it fails"),
            );
        }
        let dispositions = {
            let path = sandbox.path("dispositions.json");
            write_json(&path, &value)?;
            path.to_string_lossy().to_string()
        };
        expect_fail(
            run_report(&with_dispositions(&args, &dispositions)),
            "secret-shaped token",
        )?;

        // An oversized note is an unbounded log and fails.
        let (sandbox, args) = prepared_args("disp-huge", &["--dispositions", "DISPOSITIONS"])?;
        let mut value = dispositions_value();
        if let Some(first) = value
            .get_mut("dispositions")
            .and_then(Value::as_array_mut)
            .and_then(|entries| entries.first_mut())
            .and_then(|entry| entry.as_object_mut())
        {
            first.insert("notes".to_string(), json!("x".repeat(NOTE_MAX_CHARS + 1)));
        }
        let dispositions = {
            let path = sandbox.path("dispositions.json");
            write_json(&path, &value)?;
            path.to_string_lossy().to_string()
        };
        expect_fail(
            run_report(&with_dispositions(&args, &dispositions)),
            "character bound",
        )
    }

    #[test]
    fn historical_0_2_candidate_refuses_acceptance_with_typed_note() -> Result<(), String> {
        let sandbox = TestSandbox::new("historical")?;
        sandbox.write_manifest()?;
        // A 0.2-shaped receipt (historical retained shape); the route refuses
        // on the schema before any validation is spent on it.
        let rows: Vec<Value> = sandbox
            .manifest_value()
            .get("repos")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|repo| {
                json!({
                    "id": repo.get("id").cloned().unwrap_or_default(),
                    "sha": repo.get("sha").cloned().unwrap_or_default(),
                    "shape": "pytest_library",
                    "outcome": "ok",
                    "runtime_ms": 100,
                    "gap_ids": [],
                    "gap_ids_stable": true,
                    "unstable_gap_ids": [],
                    "stderr_excerpt": "",
                    "classification_counts": {"exposed": 0, "weakly_exposed": 0, "reachable_unrevealed": 0, "no_static_path": 0, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0},
                    "alignment_counts": {"direct": 0, "alias": 0, "changed_sink_token": 0, "orthogonal": 0, "unknown": 0, "absent": 0},
                })
            })
            .collect();
        let historical = json!({
            "schema_version": "0.2",
            "kind": "python_eval_sweep_report",
            "spec": SPEC,
            "tier": TIER,
            "summary": {
                "repos_total": 8,
                "repos_run": 8,
                "repos_skipped": 0,
                "repos_clone_failed": 0,
                "crash_count": 0,
                "crash_rate": 0.0,
                "parse_failure_count": 0,
                "parse_failure_rate": 0.0,
                "timed_out_count": 0,
                "runtime_ms_min": 100,
                "runtime_ms_median": 100,
                "runtime_ms_max": 100,
                "runtime_ms_total": 800,
                "gap_id_stable_count": 8,
                "gap_id_unstable_count": 0,
                "gap_id_stability_rate": 1.0,
                "classification_counts": {"exposed": 0, "weakly_exposed": 0, "reachable_unrevealed": 0, "no_static_path": 0, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0},
                "alignment_counts": {"direct": 0, "alias": 0, "changed_sink_token": 0, "orthogonal": 0, "unknown": 0, "absent": 0},
                "gate_status": "pass",
                "gate_reason": "8 repo(s) analyzed",
            },
            "repos": rows,
        });
        write_json(&sandbox.path("candidate.json"), &historical)?;
        expect_fail(
            run_report(&args_for(&sandbox, &["--accept"])),
            "historical receipts remain valid retained artifacts",
        )?;
        // Nothing was accepted.
        assert!(
            !sandbox.path("accepted/current.json").exists(),
            "a refused candidate must not move the pointer"
        );
        Ok(())
    }

    // -- currentness ----------------------------------------------------------

    #[test]
    fn no_pointer_reports_not_run_and_exits_zero() -> Result<(), String> {
        let sandbox = TestSandbox::new("not-run")?;
        sandbox.write_manifest()?;
        let binary = write_binary_v1(&sandbox)?;
        run_report(&currentness_args(&sandbox, &binary, &[]))?;
        Ok(())
    }

    #[test]
    fn currentness_is_current_with_matching_identities() -> Result<(), String> {
        let sandbox = accepted_sandbox("current-ok")?;
        let binary = write_binary_v1(&sandbox)?;
        run_report(&currentness_args(&sandbox, &binary, &[]))?;
        Ok(())
    }

    #[test]
    fn binary_digest_change_flips_stale() -> Result<(), String> {
        let sandbox = accepted_sandbox("stale-binary")?;
        // Different binary bytes: the mechanical law flips the pointer stale.
        let binary = write_binary_v2(&sandbox)?;
        expect_fail_all(
            run_report(&currentness_args(&sandbox, &binary, &[])),
            &["STALE", "analyzer binary moved"],
        )
    }

    #[test]
    fn manifest_digest_change_flips_stale() -> Result<(), String> {
        let sandbox = accepted_sandbox("stale-manifest")?;
        let binary = write_binary_v1(&sandbox)?;
        // Rewrite the manifest (same semantics, different bytes): the bound
        // manifest digest no longer matches the file.
        let manifest_path = sandbox.path("manifest.json");
        let mut value = read_strict(&manifest_path)?;
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "description".to_string(),
                json!("report-route sandbox manifest (edited)"),
            );
        }
        write_json(&manifest_path, &value)?;
        expect_fail(
            run_report(&currentness_args(&sandbox, &binary, &[])),
            "accepted manifest changed",
        )
    }

    #[test]
    fn moved_malformed_manifest_still_reaches_stale_with_both_reasons() -> Result<(), String> {
        // A changed manifest whose new bytes ALSO fail accepted-state
        // validation (an unknown top-level key) must reach the promised
        // `stale` verdict, never abort with a schema error: the digest
        // comparison runs on the raw bytes first, so the verdict carries BOTH
        // the digest-movement reason and the validation failure (named with
        // the offending key), and the gate still exits nonzero.
        let sandbox = accepted_sandbox("stale-malformed-manifest")?;
        let binary = write_binary_v1(&sandbox)?;
        let manifest_path = sandbox.path("manifest.json");
        let mut value = read_strict(&manifest_path)?;
        if let Some(object) = value.as_object_mut() {
            object.insert("surprise_total".to_string(), json!(9));
        }
        write_json(&manifest_path, &value)?;
        expect_fail_all(
            run_report(&currentness_args(&sandbox, &binary, &[])),
            &[
                "STALE",
                "accepted manifest changed",
                "accepted manifest failed validation",
                "surprise_total",
            ],
        )
    }

    #[test]
    fn accepted_row_or_tree_change_flips_stale() -> Result<(), String> {
        let sandbox = accepted_sandbox("stale-row")?;
        let binary = write_binary_v1(&sandbox)?;
        // Edit one retained candidate row's tree identity post-accept: the
        // retained candidate bytes AND the bound row digest both move.
        let (accepted_receipt, _receipt_path) = accepted_receipt(&sandbox)?;
        let candidate_sha = accepted_receipt
            .get("candidate")
            .and_then(|candidate| candidate.get("sha256"))
            .and_then(Value::as_str)
            .ok_or_else(|| "candidate binding".to_string())?
            .to_string();
        let candidate_path =
            sandbox.path(&format!("accepted/receipts/{candidate_sha}.candidate.json"));
        let mut candidate = read_strict(&candidate_path)?;
        if let Some(rows) = candidate.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(first) = rows.first_mut().and_then(|row| row.as_object_mut())
        {
            first.insert("tree_digest".to_string(), json!(DIGEST_TWO));
        }
        write_json(&candidate_path, &candidate)?;
        expect_fail_all(
            run_report(&currentness_args(&sandbox, &binary, &[])),
            &[
                "STALE",
                "retained candidate changed",
                "accepted row bytes changed for subject `alpha`",
            ],
        )
    }

    #[test]
    fn input_change_flips_stale() -> Result<(), String> {
        let sandbox = accepted_sandbox("stale-input")?;
        let binary = write_binary_v1(&sandbox)?;
        // The subject's synthetic diff bytes move: the bound input digest no
        // longer matches the recomputed current input.
        std::fs::write(sandbox.path("diffs/alpha.diff"), "diff-for-alpha (moved)\n")
            .map_err(|error| format!("write diff: {error}"))?;
        expect_fail_all(
            run_report(&currentness_args(&sandbox, &binary, &[])),
            &["STALE", "input moved for subject `alpha`"],
        )
    }

    #[test]
    fn pointer_config_edit_flips_stale() -> Result<(), String> {
        let sandbox = accepted_sandbox("stale-config")?;
        let binary = write_binary_v1(&sandbox)?;
        // Editing a bound identity copy INSIDE the pointer (here: config
        // profile) is a pointer-vs-receipt disagreement: stale.
        let pointer_path = sandbox.path("accepted/current.json");
        let mut pointer = read_strict(&pointer_path)?;
        if let Some(alpha) = pointer
            .get_mut("subjects")
            .and_then(Value::as_object_mut)
            .and_then(|subjects| subjects.get_mut("alpha"))
            .and_then(|entry| entry.as_object_mut())
        {
            alpha.insert("config_profile".to_string(), json!("edited"));
        }
        write_json(&pointer_path, &pointer)?;
        expect_fail_all(
            run_report(&currentness_args(&sandbox, &binary, &[])),
            &["STALE", "`config_profile` no longer matches"],
        )
    }

    #[test]
    fn pointer_features_edit_flips_stale() -> Result<(), String> {
        let sandbox = accepted_sandbox("stale-features")?;
        let binary = write_binary_v1(&sandbox)?;
        // The bound feature copy is part of the identity block; editing it is
        // a pointer-vs-receipt disagreement: stale.
        let pointer_path = sandbox.path("accepted/current.json");
        let mut pointer = read_strict(&pointer_path)?;
        if let Some(ripr) = pointer
            .get_mut("ripr")
            .and_then(|block| block.as_object_mut())
        {
            ripr.insert("features".to_string(), json!(["python", "extra"]));
        }
        write_json(&pointer_path, &pointer)?;
        expect_fail_all(
            run_report(&currentness_args(&sandbox, &binary, &[])),
            &["STALE", "`features` no longer matches"],
        )
    }

    #[test]
    fn source_sha_change_flips_stale() -> Result<(), String> {
        let sandbox = accepted_sandbox("stale-source")?;
        let binary = write_binary_v1(&sandbox)?;
        expect_fail(
            run_report(&currentness_args(
                &sandbox,
                &binary,
                &[
                    "--ripr-source-sha",
                    "1111222233334444555566667777888899990000",
                ],
            )),
            "analyzer source moved",
        )
    }

    #[test]
    fn editing_as_of_never_repairs_staleness() -> Result<(), String> {
        // Stale via the binary movement...
        let sandbox = accepted_sandbox("as-of-edit")?;
        let binary = write_binary_v2(&sandbox)?;
        expect_fail(
            run_report(&currentness_args(&sandbox, &binary, &[])),
            "STALE",
        )?;
        // ...and still stale after the as-of disclosure string is edited: the
        // comparison never reads as-of.
        let pointer_path = sandbox.path("accepted/current.json");
        let mut pointer = read_strict(&pointer_path)?;
        if let Some(object) = pointer.as_object_mut() {
            object.insert(
                "as_of".to_string(),
                json!("2099-01-01T00:00:00Z (freshened)"),
            );
        }
        write_json(&pointer_path, &pointer)?;
        expect_fail(
            run_report(&currentness_args(&sandbox, &binary, &[])),
            "STALE",
        )?;

        // The mirror case: a CURRENT pointer stays current when only the
        // as-of string is edited (as-of is a disclosure, not an identity).
        let sandbox = accepted_sandbox("as-of-edit-current")?;
        let binary = write_binary_v1(&sandbox)?;
        run_report(&currentness_args(&sandbox, &binary, &[]))?;
        let pointer_path = sandbox.path("accepted/current.json");
        let mut pointer = read_strict(&pointer_path)?;
        if let Some(object) = pointer.as_object_mut() {
            object.insert("as_of".to_string(), json!("2099-01-01T00:00:00Z (edited)"));
        }
        write_json(&pointer_path, &pointer)?;
        run_report(&currentness_args(&sandbox, &binary, &[]))?;
        Ok(())
    }

    #[test]
    fn malformed_as_of_pointer_fails_currentness() -> Result<(), String> {
        // The as-of value never enters the identity comparison, but a
        // malformed disclosure (empty, non-string, oversized) must fail the
        // check naming the field instead of riding along inside an otherwise
        // current pointer.
        for (label, value, needle) in [
            ("as-of-empty", json!(""), "as-of must be non-empty"),
            ("as-of-number", json!(5), "as-of must be a JSON string"),
            (
                "as-of-oversized",
                json!("x".repeat(NOTE_MAX_CHARS + 1)),
                "512-character bound",
            ),
        ] {
            let sandbox = accepted_sandbox(label)?;
            let binary = write_binary_v1(&sandbox)?;
            let pointer_path = sandbox.path("accepted/current.json");
            let mut pointer = read_strict(&pointer_path)?;
            if let Some(object) = pointer.as_object_mut() {
                object.insert("as_of".to_string(), value);
            }
            write_json(&pointer_path, &pointer)?;
            expect_fail_all(
                run_report(&currentness_args(&sandbox, &binary, &[])),
                &[needle, "as_of"],
            )?;
        }

        // A normal as_of value passes the hygiene check and currentness.
        let sandbox = accepted_sandbox("as-of-shape-good")?;
        let binary = write_binary_v1(&sandbox)?;
        let pointer_path = sandbox.path("accepted/current.json");
        let mut pointer = read_strict(&pointer_path)?;
        if let Some(object) = pointer.as_object_mut() {
            object.insert("as_of".to_string(), json!("2026-09-10T00:00:00Z"));
        }
        write_json(&pointer_path, &pointer)?;
        run_report(&currentness_args(&sandbox, &binary, &[]))?;
        Ok(())
    }

    #[test]
    fn unverifiable_identities_disclose_without_claiming_current() -> Result<(), String> {
        let sandbox = accepted_sandbox("unverifiable")?;
        // No --ripr-bin: the binary identity cannot be recomputed, so the
        // verdict must not claim `current`. The command exits 0 with the
        // disclosure; the pure comparison is asserted directly.
        let args = vec![
            "--check-currentness".to_string(),
            "--manifest".to_string(),
            sandbox.manifest_path_string(),
            "--state-dir".to_string(),
            sandbox.state_dir(),
            "--ripr-source-sha".to_string(),
            SOURCE_SHA.to_string(),
        ];
        run_report(&args)?;

        let pointer = read_strict(&sandbox.path("accepted/current.json"))?;
        let pointer_object = pointer
            .as_object()
            .ok_or_else(|| "pointer object".to_string())?;
        let (accepted_receipt, receipt_path) = accepted_receipt(&sandbox)?;
        let receipt_sha = sha256_hex(
            &std::fs::read(&receipt_path).map_err(|error| format!("read receipt: {error}"))?,
        );
        let candidate_binding = accepted_receipt
            .get("candidate")
            .and_then(|candidate| candidate.get("sha256"))
            .and_then(Value::as_str)
            .ok_or_else(|| "candidate binding".to_string())?
            .to_string();
        let candidate_path = sandbox.path(&format!(
            "accepted/receipts/{candidate_binding}.candidate.json"
        ));
        let candidate = read_strict(&candidate_path)?;
        let candidate_sha = sha256_hex(
            &std::fs::read(&candidate_path).map_err(|error| format!("read candidate: {error}"))?,
        );
        let current = CurrentIdentity {
            ripr_source_sha: Some(SOURCE_SHA.to_string()),
            ripr_binary_digest: None,
            subject_inputs: BTreeMap::new(),
        };
        let (manifest_value, manifest_sha) = load_strict_json(&sandbox.manifest_path_string())?;
        let accepted = validate_accepted_manifest(&manifest_value, manifest_sha.clone())?;
        let comparison = compare_currentness(&CurrentnessInputs {
            pointer: pointer_object,
            accepted_receipt: &accepted_receipt,
            receipt_sha256: &receipt_sha,
            candidate: &candidate,
            candidate_sha256: &candidate_sha,
            accepted: Some(&accepted),
            current_manifest_sha256: &manifest_sha,
            manifest_validation_error: None,
            current: &current,
        })?;
        assert_eq!(
            comparison.verdict,
            CurrentnessVerdict::Unverifiable,
            "a missing binary recompute input must leave currentness unverifiable: {:?}",
            comparison.unverifiable
        );
        assert!(
            comparison
                .unverifiable
                .iter()
                .any(|reason| reason.contains("binary")),
            "the binary gap must be disclosed: {:?}",
            comparison.unverifiable
        );
        Ok(())
    }

    #[test]
    fn config_input_substitution_flips_stale() -> Result<(), String> {
        // A candidate that names a DIFFERENT portable input file for a
        // subject — hashing the substitute's bytes — must not pass
        // currentness: the pointer's config input must BE the
        // manifest-declared input path for that subject, and the refusal
        // names both paths.
        let (sandbox, args) =
            prepared_args("input-sub", &["--dispositions", "DISPOSITIONS", "--accept"])?;
        let mut candidate = read_strict(&sandbox.path("candidate.json"))?;
        if let Some(alpha) = candidate
            .get_mut("repos")
            .and_then(Value::as_array_mut)
            .and_then(|rows| rows.first_mut())
            .and_then(|row| row.as_object_mut())
        {
            if let Some(config) = alpha.get_mut("config").and_then(Value::as_object_mut) {
                config.insert("input".to_string(), json!("diffs/evil.diff"));
            }
            alpha.insert(
                "input_digest".to_string(),
                json!(sha256_hex(b"diff-for-alpha (substituted)\n")),
            );
        }
        write_json(&sandbox.path("candidate.json"), &candidate)?;
        std::fs::write(
            sandbox.path("diffs/evil.diff"),
            "diff-for-alpha (substituted)\n",
        )
        .map_err(|error| format!("write substituted diff: {error}"))?;
        let dispositions = sandbox.dispositions_path()?;
        run_report(&with_dispositions(&args, &dispositions))?;

        let binary = write_binary_v1(&sandbox)?;
        expect_fail_all(
            run_report(&currentness_args(&sandbox, &binary, &[])),
            &[
                "STALE",
                "input path substituted for subject `alpha`",
                "diffs/evil.diff",
                "diffs/alpha.diff",
            ],
        )
    }

    #[test]
    fn missing_input_digest_is_never_current() -> Result<(), String> {
        // A candidate row that records no input_digest leaves the pointer
        // with no input identity for that subject; with every other identity
        // matching, the gate must still refuse `current` — a missing subject
        // identity is disclosed unverifiable and names the subject.
        let (sandbox, args) = prepared_args(
            "missing-input",
            &["--dispositions", "DISPOSITIONS", "--accept"],
        )?;
        let mut candidate = read_strict(&sandbox.path("candidate.json"))?;
        if let Some(alpha) = candidate
            .get_mut("repos")
            .and_then(Value::as_array_mut)
            .and_then(|rows| rows.first_mut())
            .and_then(|row| row.as_object_mut())
        {
            alpha.remove("input_digest");
        }
        write_json(&sandbox.path("candidate.json"), &candidate)?;
        let dispositions = sandbox.dispositions_path()?;
        run_report(&with_dispositions(&args, &dispositions))?;

        // Direct comparison: source/binary identities match and every
        // declared input recomputes, yet alpha's missing input digest must
        // keep the verdict unverifiable.
        let pointer = read_strict(&sandbox.path("accepted/current.json"))?;
        let pointer_object = pointer
            .as_object()
            .ok_or_else(|| "pointer object".to_string())?;
        let (accepted_receipt, receipt_path) = accepted_receipt(&sandbox)?;
        let receipt_sha = sha256_hex(
            &std::fs::read(&receipt_path).map_err(|error| format!("read receipt: {error}"))?,
        );
        let candidate_binding = accepted_receipt
            .get("candidate")
            .and_then(|candidate| candidate.get("sha256"))
            .and_then(Value::as_str)
            .ok_or_else(|| "candidate binding".to_string())?
            .to_string();
        let candidate_path = sandbox.path(&format!(
            "accepted/receipts/{candidate_binding}.candidate.json"
        ));
        let candidate = read_strict(&candidate_path)?;
        let candidate_sha = sha256_hex(
            &std::fs::read(&candidate_path).map_err(|error| format!("read candidate: {error}"))?,
        );
        let (manifest_value, manifest_sha) = load_strict_json(&sandbox.manifest_path_string())?;
        let accepted = validate_accepted_manifest(&manifest_value, manifest_sha.clone())?;
        let current = CurrentIdentity {
            ripr_source_sha: Some(SOURCE_SHA.to_string()),
            ripr_binary_digest: Some(binary_digest_v1()),
            subject_inputs: recompute_subject_inputs(&accepted, &sandbox.manifest_path_string()),
        };
        let comparison = compare_currentness(&CurrentnessInputs {
            pointer: pointer_object,
            accepted_receipt: &accepted_receipt,
            receipt_sha256: &receipt_sha,
            candidate: &candidate,
            candidate_sha256: &candidate_sha,
            accepted: Some(&accepted),
            current_manifest_sha256: &manifest_sha,
            manifest_validation_error: None,
            current: &current,
        })?;
        assert_eq!(
            comparison.verdict,
            CurrentnessVerdict::Unverifiable,
            "a subject with no bound input identity must never read current: stale={:?} unverifiable={:?}",
            comparison.stale,
            comparison.unverifiable
        );
        assert!(
            comparison
                .unverifiable
                .iter()
                .any(|reason| reason.contains("alpha") && reason.contains("input")),
            "the disclosure must name the subject and the input identity: {:?}",
            comparison.unverifiable
        );

        // End to end: the command exits 0 with the disclosure (unverifiable
        // is disclosed in full, never a gate pass dressed as current).
        let binary = write_binary_v1(&sandbox)?;
        run_report(&currentness_args(&sandbox, &binary, &[]))?;
        Ok(())
    }

    #[test]
    fn split_portable_rejects_traversal_component_shapes() -> Result<(), String> {
        // The split is the containment boundary: empty components (consecutive
        // separators), `.`, `..`, and separator-only paths are rejected, so no
        // pointer-shaped portable path can walk outside the state directory.
        let good = split_portable("pointer", "receipt_file", "state", "receipts/abc123.json")?;
        assert_eq!(
            good,
            PathBuf::from("state").join("receipts").join("abc123.json"),
            "the normal receipt_file shape still joins"
        );
        for bad in [
            "receipts//../../secret",
            "a//b",
            "receipts/./abc.json",
            "./receipts/abc.json",
            ".",
            "..",
            "receipts/",
            "receipts//",
            "//",
            "",
        ] {
            let error = match split_portable("pointer", "receipt_file", "state", bad) {
                Ok(path) => {
                    return Err(format!(
                        "split_portable must reject `{bad}`, joined `{}`",
                        path.display()
                    ));
                }
                Err(error) => error,
            };
            assert!(
                error.contains("receipt_file"),
                "refusal `{error}` must name the field"
            );
        }
        Ok(())
    }

    #[test]
    fn pointer_receipt_file_with_empty_or_dot_components_is_refused() -> Result<(), String> {
        // A crafted pointer receipt_file cannot use consecutive separators or
        // `.` components to slip a traversal path past the portable-path
        // checks: every such shape is refused naming `receipt_file`, while the
        // normal `receipts/<sha>.json` shape still reads.
        for (index, bad) in [
            "receipts//leak.json",
            "receipts/./leak.json",
            "a//..//b/../../outside",
        ]
        .into_iter()
        .enumerate()
        {
            let sandbox = accepted_sandbox(&format!("portable-{index}"))?;
            let binary = write_binary_v1(&sandbox)?;
            let pointer_path = sandbox.path("accepted/current.json");
            let mut pointer = read_strict(&pointer_path)?;
            if let Some(object) = pointer.as_object_mut() {
                object.insert("receipt_file".to_string(), json!(bad));
            }
            write_json(&pointer_path, &pointer)?;
            expect_fail_all(
                run_report(&currentness_args(&sandbox, &binary, &[])),
                &["receipt_file"],
            )?;
        }
        // The normal shape still passes end to end.
        let sandbox = accepted_sandbox("portable-good")?;
        let binary = write_binary_v1(&sandbox)?;
        run_report(&currentness_args(&sandbox, &binary, &[]))?;
        Ok(())
    }

    #[test]
    fn missing_toolchain_identity_is_never_current() -> Result<(), String> {
        // A pointer that binds no `features` or `build_profile` copy (the
        // candidate recorded none) cannot verify that identity: the verdict
        // must read `unverifiable` — never `current` — with the field named
        // (the step-6 subject rule applied to the toolchain block).
        for (label, field) in [
            ("tc-no-features", "features"),
            ("tc-no-profile", "build_profile"),
        ] {
            let sandbox = accepted_sandbox(label)?;
            let pointer_path = sandbox.path("accepted/current.json");
            let mut pointer = read_strict(&pointer_path)?;
            if let Some(ripr) = pointer
                .get_mut("ripr")
                .and_then(|block| block.as_object_mut())
            {
                ripr.remove(field);
            }
            write_json(&pointer_path, &pointer)?;
            let pointer_object = pointer
                .as_object()
                .ok_or_else(|| "pointer object".to_string())?;
            let (accepted_receipt, receipt_path) = accepted_receipt(&sandbox)?;
            let receipt_sha = sha256_hex(
                &std::fs::read(&receipt_path).map_err(|error| format!("read receipt: {error}"))?,
            );
            let candidate_binding = accepted_receipt
                .get("candidate")
                .and_then(|candidate| candidate.get("sha256"))
                .and_then(Value::as_str)
                .ok_or_else(|| "candidate binding".to_string())?
                .to_string();
            let candidate_path = sandbox.path(&format!(
                "accepted/receipts/{candidate_binding}.candidate.json"
            ));
            let candidate = read_strict(&candidate_path)?;
            let candidate_sha = sha256_hex(
                &std::fs::read(&candidate_path)
                    .map_err(|error| format!("read candidate: {error}"))?,
            );
            let (manifest_value, manifest_sha) = load_strict_json(&sandbox.manifest_path_string())?;
            let accepted = validate_accepted_manifest(&manifest_value, manifest_sha.clone())?;
            let current = CurrentIdentity {
                ripr_source_sha: Some(SOURCE_SHA.to_string()),
                ripr_binary_digest: Some(binary_digest_v1()),
                subject_inputs: recompute_subject_inputs(
                    &accepted,
                    &sandbox.manifest_path_string(),
                ),
            };
            let comparison = compare_currentness(&CurrentnessInputs {
                pointer: pointer_object,
                accepted_receipt: &accepted_receipt,
                receipt_sha256: &receipt_sha,
                candidate: &candidate,
                candidate_sha256: &candidate_sha,
                accepted: Some(&accepted),
                current_manifest_sha256: &manifest_sha,
                manifest_validation_error: None,
                current: &current,
            })?;
            assert_eq!(
                comparison.verdict,
                CurrentnessVerdict::Unverifiable,
                "a pointer binding no `{field}` must never read current: stale={:?} unverifiable={:?}",
                comparison.stale,
                comparison.unverifiable
            );
            assert!(
                comparison
                    .unverifiable
                    .iter()
                    .any(|reason| reason.contains("toolchain") && reason.contains(field)),
                "the disclosure must name the missing toolchain identity `{field}`: {:?}",
                comparison.unverifiable
            );

            // End to end: the command exits 0 with the disclosure.
            let binary = write_binary_v1(&sandbox)?;
            run_report(&currentness_args(&sandbox, &binary, &[]))?;
        }
        Ok(())
    }

    #[test]
    fn disposition_owner_pipes_and_newlines_cannot_split_the_markdown_table() -> Result<(), String>
    {
        // A free-text disposition owner carrying a pipe and a newline must
        // not split or extend the accepted Markdown table: the cell renders
        // escaped and flattened while the receipt JSON keeps the raw value.
        // The pure derivation path (sidecar -> accepted receipt -> Markdown)
        // avoids the shared dry-run report files, which sibling tests
        // rewrite concurrently.
        let sandbox = TestSandbox::new("md-escape")?;
        let manifest_sha = sandbox.write_manifest()?;
        let candidate = candidate_value(&sandbox, &manifest_sha, &binary_digest_v1(), 100);
        let candidate_path = sandbox.path("candidate.json");
        write_json(&candidate_path, &candidate)?;
        let candidate_sha = sha256_hex(
            &std::fs::read(&candidate_path).map_err(|error| format!("read candidate: {error}"))?,
        );
        let rows = read_candidate_rows(&candidate)?;
        let mut value = dispositions_value();
        if let Some(first) = value
            .get_mut("dispositions")
            .and_then(Value::as_array_mut)
            .and_then(|entries| entries.first_mut())
            .and_then(|entry| entry.as_object_mut())
        {
            first.insert("owner".to_string(), json!("team\nops|lead"));
        }
        let dispositions_path = sandbox.path("dispositions.json");
        write_json(&dispositions_path, &value)?;
        let dispositions = load_dispositions(&dispositions_path.to_string_lossy(), &rows)?;
        require_disposition_coverage(&rows, &dispositions)?;
        let (manifest_value, current_manifest_sha) =
            load_strict_json(&sandbox.manifest_path_string())?;
        let accepted = validate_accepted_manifest(&manifest_value, current_manifest_sha.clone())?;
        let receipt = build_accepted_receipt(
            &accepted,
            &current_manifest_sha,
            &candidate,
            &candidate_sha,
            &rows,
            &dispositions,
            &[],
        );
        let markdown = render_accepted_markdown(&receipt)?;
        assert!(
            markdown.contains("team ops\\|lead"),
            "the owner cell must render escaped and flattened: {markdown}"
        );
        assert!(
            !markdown.contains("team\nops") && !markdown.contains("team|lead"),
            "the raw owner value must not leak into the table: {markdown}"
        );
        // Table integrity: every subjects-section line keeps exactly the five
        // structural cell separators (an unescaped pipe or a raw newline would
        // break that count).
        let section = markdown
            .split("## Subjects")
            .nth(1)
            .and_then(|rest| rest.split("## Non-claims").next())
            .ok_or_else(|| "markdown subjects section".to_string())?;
        for line in section.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let structural = line.replace("\\|", "");
            assert_eq!(
                structural.matches('|').count(),
                5,
                "every subjects-table line must keep exactly five cell separators: {line}"
            );
        }

        // The JSON keeps the raw value.
        let raw_owner = receipt
            .get("subjects")
            .and_then(Value::as_array)
            .and_then(|subjects| {
                subjects
                    .iter()
                    .find(|subject| subject.get("id").and_then(Value::as_str) == Some("bravo"))
            })
            .and_then(|subject| subject.get("disposition"))
            .and_then(|disposition| disposition.get("owner"))
            .and_then(Value::as_str)
            .ok_or_else(|| "bravo disposition owner".to_string())?;
        assert_eq!(
            raw_owner, "team\nops|lead",
            "the receipt JSON keeps the raw disposition value"
        );
        Ok(())
    }

    // -- derived receipt content ----------------------------------------------

    #[test]
    fn stability_mismatch_reasons_are_recorded_in_the_receipt() -> Result<(), String> {
        let (sandbox, args) =
            prepared_args("mismatch", &["--dispositions", "DISPOSITIONS", "--accept"])?;
        // Flip the one compared row's repeat comparison to unstable: the
        // mismatch list must name the subject and its unstable gap ids.
        let mut candidate = read_strict(&sandbox.path("candidate.json"))?;
        if let Some(rows) = candidate.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(first) = rows.first_mut().and_then(|row| row.as_object_mut())
            && let Some(Value::Object(repeat)) = first.get_mut("repeat")
        {
            repeat.insert("gap_ids_stable".to_string(), json!(false));
            repeat.insert(
                "unstable_gap_ids".to_string(),
                json!(["gap:python:alpha-1"]),
            );
        }
        write_json(&sandbox.path("candidate.json"), &candidate)?;
        let dispositions = sandbox.dispositions_path()?;
        run_report(&with_dispositions(&args, &dispositions))?;

        let (receipt, _path) = accepted_receipt(&sandbox)?;
        let mismatches = receipt
            .get("stability")
            .and_then(|stability| stability.get("mismatch_subjects"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            mismatches.len(),
            1,
            "the one unstable comparison is recorded"
        );
        assert_eq!(
            mismatches
                .first()
                .and_then(|entry| entry.get("id"))
                .and_then(Value::as_str),
            Some("alpha"),
        );
        assert!(
            mismatches
                .first()
                .and_then(|entry| entry.get("unstable_gap_ids"))
                .and_then(Value::as_array)
                .map(|ids| ids.contains(&json!("gap:python:alpha-1")))
                .unwrap_or(false),
            "the mismatch reason carries the unstable gap ids"
        );
        Ok(())
    }

    #[test]
    fn runtime_envelope_is_derived_and_bounded() -> Result<(), String> {
        let (sandbox, args) =
            prepared_args("runtime", &["--dispositions", "DISPOSITIONS", "--accept"])?;
        let dispositions = sandbox.dispositions_path()?;
        run_report(&with_dispositions(&args, &dispositions))?;

        let (receipt, _path) = accepted_receipt(&sandbox)?;
        let envelope = receipt
            .get("runtime_envelope")
            .ok_or_else(|| "runtime_envelope".to_string())?;
        assert_eq!(
            envelope.get("status").and_then(Value::as_str),
            Some("reliable")
        );
        assert_eq!(envelope.get("min_ms").and_then(Value::as_u64), Some(100));
        assert_eq!(envelope.get("median_ms").and_then(Value::as_u64), Some(300));
        assert_eq!(envelope.get("max_ms").and_then(Value::as_u64), Some(500));
        assert_eq!(envelope.get("total_ms").and_then(Value::as_u64), Some(1500));
        // The count carries the analyzed-row denominator (5 run rows).
        let count = envelope
            .get("count")
            .and_then(Value::as_object)
            .ok_or_else(|| "runtime_envelope.count".to_string())?;
        assert_eq!(count.get("numerator").and_then(Value::as_u64), Some(5));
        assert_eq!(count.get("denominator").and_then(Value::as_u64), Some(5));
        Ok(())
    }

    #[test]
    fn counts_and_health_are_derived_from_rows() -> Result<(), String> {
        let (sandbox, args) =
            prepared_args("counts", &["--dispositions", "DISPOSITIONS", "--accept"])?;
        let dispositions = sandbox.dispositions_path()?;
        run_report(&with_dispositions(&args, &dispositions))?;

        let (receipt, _path) = accepted_receipt(&sandbox)?;
        let counts = receipt
            .get("counts")
            .and_then(Value::as_object)
            .ok_or_else(|| "counts".to_string())?;
        // Row facts: 5 materialized (the ran rows), 5 detected/available,
        // 1 stale, 1 tempfail, 0 license-blocked.
        let numerator = |name: &str| -> Result<u64, String> {
            counts
                .get(name)
                .and_then(|count| count.get("numerator"))
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("count {name}"))
        };
        assert_eq!(numerator("selected")?, 8);
        assert_eq!(numerator("run")?, 5);
        assert_eq!(numerator("materialized")?, 5);
        assert_eq!(numerator("available")?, 5);
        assert_eq!(numerator("stale")?, 1);
        assert_eq!(numerator("tempfail")?, 1);
        assert_eq!(numerator("license_blocked")?, 0);
        // Every count in the outcomes/health/distribution blocks carries
        // numerator + denominator (the selected denominator) — no bare
        // denominator-free number anywhere.
        let count_shape = |name: &str, count: &Value| -> Result<(), String> {
            let shape_ok = count.get("numerator").and_then(Value::as_u64).is_some()
                && count.get("denominator").and_then(Value::as_u64).is_some();
            if shape_ok {
                Ok(())
            } else {
                Err(format!(
                    "count `{name}` must carry numerator and denominator: {count}"
                ))
            }
        };
        let numerator_in =
            |block: &serde_json::Map<String, Value>, name: &str| -> Result<u64, String> {
                block
                    .get(name)
                    .and_then(|count| count.get("numerator"))
                    .and_then(Value::as_u64)
                    .ok_or_else(|| format!("count {name}"))
            };
        let outcomes = receipt
            .get("outcomes")
            .and_then(Value::as_object)
            .ok_or_else(|| "outcomes".to_string())?;
        for (name, count) in outcomes {
            count_shape(name, count)?;
        }
        assert_eq!(numerator_in(outcomes, "complete")?, 1);
        assert_eq!(numerator_in(outcomes, "crashed")?, 1);
        assert_eq!(numerator_in(outcomes, "parse_failed")?, 1);
        assert_eq!(numerator_in(outcomes, "timed_out")?, 1);
        assert_eq!(numerator_in(outcomes, "unsupported")?, 1);
        let health = receipt
            .get("health")
            .and_then(Value::as_object)
            .ok_or_else(|| "health".to_string())?;
        let detection = health
            .get("project_detection")
            .and_then(Value::as_object)
            .ok_or_else(|| "project_detection".to_string())?;
        let corpus = health
            .get("corpus_selection")
            .and_then(Value::as_object)
            .ok_or_else(|| "corpus_selection".to_string())?;
        for (name, count) in detection.iter().chain(corpus) {
            count_shape(name, count)?;
        }
        assert_eq!(numerator_in(detection, "detected")?, 5);
        assert_eq!(numerator_in(detection, "unrecorded")?, 0);
        assert_eq!(numerator_in(corpus, "selected")?, 5);
        assert_eq!(numerator_in(corpus, "absent")?, 3);
        // Distributions agree with the rows: weakly_exposed=1, static_unknown=1.
        let distributions = receipt
            .get("distributions")
            .and_then(Value::as_object)
            .ok_or_else(|| "distributions".to_string())?;
        let classification = distributions
            .get("classification")
            .and_then(Value::as_object)
            .ok_or_else(|| "classification".to_string())?;
        let alignment = distributions
            .get("alignment")
            .and_then(Value::as_object)
            .ok_or_else(|| "alignment".to_string())?;
        for (name, count) in classification.iter().chain(alignment) {
            count_shape(name, count)?;
        }
        assert_eq!(numerator_in(classification, "weakly_exposed")?, 1,);
        assert_eq!(numerator_in(classification, "static_unknown")?, 1);
        // Each distribution bucket carries the selected denominator (non-run
        // rows contribute no buckets, so sums below 8 are the honest shape).
        assert_eq!(
            classification
                .get("weakly_exposed")
                .and_then(|count| count.get("denominator"))
                .and_then(Value::as_u64),
            Some(8),
        );
        assert_eq!(numerator_in(alignment, "orthogonal")?, 1);
        assert_eq!(numerator_in(alignment, "absent")?, 3);
        // The limitation distribution carries a named disclosure, never an
        // invented taxonomy.
        assert!(
            distributions
                .get("limitation")
                .and_then(|limitation| limitation.get("disclosure"))
                .and_then(Value::as_str)
                .map(|text| text.contains("no limitation distribution"))
                .unwrap_or(false),
        );
        Ok(())
    }

    #[test]
    fn dry_run_writes_no_accepted_state() -> Result<(), String> {
        let (sandbox, args) = prepared_args("dry-no-state", &["--dispositions", "DISPOSITIONS"])?;
        let dispositions = sandbox.dispositions_path()?;
        run_report(&with_dispositions(&args, &dispositions))?;
        assert!(
            !sandbox.path("accepted").exists(),
            "a dry run must not create accepted state"
        );
        Ok(())
    }

    #[test]
    fn pointer_field_hygiene_fails_on_secrets() -> Result<(), String> {
        // A pointer as-of carrying a secret-shaped token fails hygiene before
        // any write.
        let (sandbox, args) = prepared_args(
            "pointer-hygiene",
            &[
                "--dispositions",
                "DISPOSITIONS",
                "--accept",
                "--as-of",
                "api_key=hunter2",
            ],
        )?;
        let dispositions = sandbox.dispositions_path()?;
        expect_fail(
            run_report(&with_dispositions(&args, &dispositions)),
            "secret-shaped token",
        )?;
        assert!(
            !sandbox.path("accepted/current.json").exists(),
            "a hygiene failure must not move the pointer"
        );
        Ok(())
    }

    // -- acceptance write-path integrity --------------------------------------

    #[test]
    fn candidate_modified_after_parse_refuses_acceptance() -> Result<(), String> {
        // The accept-time window: a candidate file edited between the initial
        // parse and the acceptance writes must be refused, so the accepted
        // artifacts can never retain new bytes under the OLD candidate
        // digest while the pointer moves.
        let (sandbox, _args) = prepared_args("candidate-edit", &[])?;
        let candidate_path = sandbox.path("candidate.json");
        let (_value, parsed_sha) = load_strict_json(&candidate_path.to_string_lossy())?;
        // The post-parse edit:
        let mut edited = read_strict(&candidate_path)?;
        if let Some(rows) = edited.get_mut("repos").and_then(Value::as_array_mut) {
            rows.pop();
        }
        write_json(&candidate_path, &edited)?;
        let error = match revalidate_candidate_bytes(&candidate_path, &parsed_sha) {
            Ok(_) => return Err("an edited candidate must be refused at acceptance".to_string()),
            Err(error) => error,
        };
        for needle in ["changed after validation", parsed_sha.as_str()] {
            assert!(
                error.contains(needle),
                "refusal `{error}` must mention `{needle}`"
            );
        }
        // The bytes actually on disk still revalidate: the honest rerun path
        // re-parses the current bytes and records their digest.
        let current_bytes =
            std::fs::read(&candidate_path).map_err(|error| format!("read candidate: {error}"))?;
        let current_sha = sha256_hex(&current_bytes);
        let verified = revalidate_candidate_bytes(&candidate_path, &current_sha)?;
        assert_eq!(
            verified, current_bytes,
            "verified bytes are exactly the file bytes"
        );
        Ok(())
    }

    #[test]
    fn pointer_replacement_preserves_a_readable_current_pointer() -> Result<(), String> {
        // Replacing an established pointer must leave a readable current.json
        // in place after the write: the staged file is written and flushed,
        // then renamed over the existing pointer without removing it first
        // (remove+rename is only the documented fallback for hosts that
        // refuse rename-over-existing, after the staged bytes are durable).
        let sandbox = TestSandbox::new("pointer-replace")?;
        let state_dir = sandbox.path("accepted");
        std::fs::create_dir_all(&state_dir)
            .map_err(|error| format!("create state dir: {error}"))?;
        let pointer_path = state_dir.join(POINTER_FILE);
        write_json(
            &pointer_path,
            &json!({"schema_version": POINTER_SCHEMA, "kind": POINTER_KIND, "spec": SPEC}),
        )?;
        write_pointer_atomically(&state_dir, "{\"replaced\": true}")?;
        let replaced = read_strict(&pointer_path)?;
        assert_eq!(
            replaced.get("replaced").and_then(Value::as_bool),
            Some(true),
            "the new pointer is in place after the replacement"
        );
        // A second replacement exercises rename-over-existing again: the
        // pointer stays readable and parseable throughout.
        write_pointer_atomically(&state_dir, "{\"replaced\": false}")?;
        let replaced = read_strict(&pointer_path)?;
        assert_eq!(
            replaced.get("replaced").and_then(Value::as_bool),
            Some(false)
        );
        Ok(())
    }

    #[test]
    fn truncated_accepted_artifacts_are_repaired_on_reacceptance() -> Result<(), String> {
        // A digest-addressed artifact whose on-disk bytes do not hash to its
        // own address is not a valid prior artifact (truncated by an
        // interrupted write, or edited): re-acceptance repairs it with
        // exactly the digest-named bytes instead of refusing forever as
        // "conflicting content".
        let (sandbox, args) = prepared_args(
            "accept-repair",
            &["--dispositions", "DISPOSITIONS", "--accept"],
        )?;
        let dispositions = sandbox.dispositions_path()?;
        let args = with_dispositions(&args, &dispositions);
        run_report(&args)?;

        let pointer = read_strict(&sandbox.path("accepted/current.json"))?;
        let receipt_file = pointer
            .get("receipt_file")
            .and_then(Value::as_str)
            .ok_or_else(|| "receipt_file".to_string())?
            .to_string();
        let receipt_path = sandbox.path(&format!("accepted/{receipt_file}"));
        let (accepted_receipt, _path) = accepted_receipt(&sandbox)?;
        let candidate_sha = accepted_receipt
            .get("candidate")
            .and_then(|candidate| candidate.get("sha256"))
            .and_then(Value::as_str)
            .ok_or_else(|| "candidate.sha256".to_string())?
            .to_string();
        let candidate_path =
            sandbox.path(&format!("accepted/receipts/{candidate_sha}.candidate.json"));
        let receipt_bytes =
            std::fs::read(&receipt_path).map_err(|error| format!("read receipt: {error}"))?;
        let candidate_bytes =
            std::fs::read(&candidate_path).map_err(|error| format!("read candidate: {error}"))?;
        // The published bytes hash to their digest addresses by construction.
        assert_eq!(
            sha256_hex(&receipt_bytes),
            pointer
                .get("receipt_sha256")
                .and_then(Value::as_str)
                .ok_or_else(|| "receipt_sha256".to_string())?,
            "the accepted receipt's bytes hash to its digest address"
        );
        assert_eq!(
            sha256_hex(&candidate_bytes),
            candidate_sha,
            "the retained candidate's bytes hash to its digest address"
        );

        // Simulate interrupted writes: truncated bytes at both digest
        // addresses.
        std::fs::write(&receipt_path, &receipt_bytes[..receipt_bytes.len() / 2])
            .map_err(|error| format!("truncate receipt: {error}"))?;
        std::fs::write(
            &candidate_path,
            &candidate_bytes[..candidate_bytes.len() / 2],
        )
        .map_err(|error| format!("truncate candidate: {error}"))?;

        // Re-acceptance repairs both instead of refusing.
        run_report(&args)?;
        let repaired_receipt =
            std::fs::read(&receipt_path).map_err(|error| format!("reread receipt: {error}"))?;
        let repaired_candidate =
            std::fs::read(&candidate_path).map_err(|error| format!("reread candidate: {error}"))?;
        assert_eq!(
            repaired_receipt, receipt_bytes,
            "the receipt must hold exactly the staged bytes after repair"
        );
        assert_eq!(
            repaired_candidate, candidate_bytes,
            "the retained candidate must hold exactly the verified bytes after repair"
        );
        Ok(())
    }

    #[test]
    fn edited_markdown_refuses_reacceptance_and_matching_is_idempotent() -> Result<(), String> {
        // An existing accepted Markdown is verified on re-acceptance: edited
        // bytes under the accepted receipt's digest are a typed refusal (the
        // mirror of the existing-different-JSON rule); matching bytes are an
        // idempotent no-op.
        let (sandbox, args) =
            prepared_args("md-edit", &["--dispositions", "DISPOSITIONS", "--accept"])?;
        let dispositions = sandbox.dispositions_path()?;
        let args = with_dispositions(&args, &dispositions);
        run_report(&args)?;
        let pointer = read_strict(&sandbox.path("accepted/current.json"))?;
        let receipt_sha = pointer
            .get("receipt_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| "receipt_sha256".to_string())?
            .to_string();
        let markdown_path = sandbox.path(&format!("accepted/receipts/{receipt_sha}.md"));
        let original =
            std::fs::read_to_string(&markdown_path).map_err(|error| format!("read md: {error}"))?;

        // Corrupt the retained Markdown, then re-accept the same candidate:
        // the edited artifact is refused, never silently kept or rewritten.
        std::fs::write(&markdown_path, "# edited after acceptance\n")
            .map_err(|error| format!("write markdown: {error}"))?;
        expect_fail(run_report(&args), "different bytes")?;

        // Restored bytes: re-acceptance is an idempotent no-op.
        std::fs::write(&markdown_path, &original)
            .map_err(|error| format!("restore markdown: {error}"))?;
        run_report(&args)?;
        Ok(())
    }
}
