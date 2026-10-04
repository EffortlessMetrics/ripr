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

mod accept;
mod candidate;
mod currentness;
mod render;

// ---------------------------------------------------------------------------
// Tests (module named `python_eval_sweep_report` so
// `cargo test -p xtask python_eval_sweep_report` selects exactly this module;
// no unwrap/expect — assert macros and Result returns only)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "eval_sweep_report/tests.rs"]
mod python_eval_sweep_report;

use serde_json::Value;

use accept::run_candidate_report;
use currentness::run_currentness_check;

// Re-imported at the facade so `tests.rs` keeps reaching the moved items
// through `super::` exactly as it did when this module was one file.
#[cfg(test)]
use super::eval_sweep_check::{load_strict_json, sha256_hex, validate_accepted_manifest};
#[cfg(test)]
use accept::{
    REPORT_JSON, REPORT_MD, build_accepted_receipt, revalidate_candidate_bytes,
    write_pointer_atomically,
};
#[cfg(test)]
use candidate::{
    DISPOSITIONS_KIND, load_dispositions, read_candidate_rows, require_disposition_coverage,
};
#[cfg(test)]
use currentness::{
    CurrentIdentity, CurrentnessInputs, CurrentnessVerdict, compare_currentness,
    recompute_subject_inputs, split_portable,
};
#[cfg(test)]
use render::render_accepted_markdown;
#[cfg(test)]
use serde_json::json;
#[cfg(test)]
use std::collections::BTreeMap;
#[cfg(test)]
use std::path::{Path, PathBuf};

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

const POINTER_SCHEMA: &str = "0.1";
const POINTER_KIND: &str = "python_eval_sweep_current_pointer";
const RECEIPTS_DIR: &str = "receipts";
const POINTER_FILE: &str = "current.json";
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
/// Free-text budget for accepted-artifact free text: disposition notes,
/// evidence references, owners, recovery routes, and the pointer's as-of
/// disclosure. Bounded excerpts, never unbounded logs.
const NOTE_MAX_CHARS: usize = 512;
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
