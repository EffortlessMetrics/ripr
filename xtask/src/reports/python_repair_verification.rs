//! Verification-receipt validation for the governed Python repair-trust
//! corpus — `cargo xtask python-repair-trust check-verification`
//! (RIPR-SPEC-0176, #3570).
//!
//! The crate-side verification phase (#3570) executes one applied attempt's
//! producer-owned verify route through bounded controls, reruns the analysis
//! against the exact post-edit state, and publishes ONE immutable candidate
//! receipt whose execution and static-movement axes stay separate. This
//! validator is the offline promotion-side counterpart: given the accepted
//! selection manifest (#3568) and a set of candidate receipts, it re-checks
//! every digest anchor, the closed vocabularies, and the separation laws the
//! receipts assert, so a promotion pass can read the retained evidence
//! without re-running the driver.
//!
//! What the check enforces, fail closed:
//!
//! - the receipt binds to the ACCEPTED selection manifest by exact-bytes
//!   sha256 and to one accepted selection row by its recomputed canonical
//!   `selection_digest` — a changed manifest or a replaced row makes every
//!   affected receipt stale;
//! - the receipt's native identity (family, owner, discriminator, relation,
//!   oracle) and target agree exactly with the accepted row — a rewritten
//!   copy fails even while the digest anchors stay valid;
//! - the execution axis carries the closed state vocabulary
//!   (`passed`/`failed`/`timed_out`/`cancelled`/`unavailable`/`not_run`/
//!   `invalid`) and each state agrees with the retained process disposition:
//!   `passed` is completed with exit status 0 and full output commitments; a
//!   non-verdict state carries no exit status; timeout, cancellation, spawn
//!   failure, route unavailability, and pre-execution rejection stay
//!   distinct;
//! - the movement axis carries the closed movement vocabulary
//!   (`closed`/`improved`/`unchanged`/`regressed`/`limited`/`stale`/
//!   `uncertain`) with the native-identity join recorded and typed: a
//!   confident state requires the complete before/after joins, its recorded
//!   state must be the one its own headline/oracle evidence implies (the same
//!   transition table the producing comparison applies), and the documented
//!   partial shapes for `stale`, `uncertain`, and `limited` are enforced —
//!   each degraded state carries a typed reason, so a degraded join is never
//!   presented as a confident classification and a fabricated join never
//!   validates;
//! - NEITHER AXIS IMPLIES THE OTHER: every state pair across the two blocks
//!   validates on its own merits — `passed` with `unchanged`, `failed` with
//!   `improved`, `unavailable` with `uncertain`, `closed` next to an
//!   unrelated-finding regression all stay representable, and the schema has
//!   no lifecycle field (`states`/`lifecycle`/`accepted` are denied) so a
//!   passing execution alone can never mark an attempt accepted or closed;
//! - unrelated actionable movement stays visible in its own block with a
//!   row-derived consistency check (`regressed` implies after > before,
//!   `improved` implies after < before, never both);
//! - the rollback block is one of `proved`/`blocked`/`not_run` with its
//!   evidence: `proved` requires the restored head, `blocked`/`not_run` a
//!   typed reason;
//! - the standing non-claims and the standing claim boundary ride on every
//!   receipt verbatim, and the execution block's producer-owned scalars are
//!   type-checked (`exit_status`/`exit_signal` signed integers — a Windows
//!   termination code is negative, `duration_ms` non-negative, truncation
//!   and cancellation flags booleans).
//!
//! Verdict vocabulary: `valid`/`inconsistent`/`not_run` (no receipts
//! supplied). Violations take precedence: any violation makes the verdict
//! `inconsistent` and the command exits nonzero. `not_run` is reserved for
//! an empty input with no violations and is not a pass. None of the three is
//! a support-tier, repair-correctness, gate, badge, or promotion claim; the
//! governed cohort run stays with #3571.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Value, json};

use super::python_repair_trust::{
    EXECUTIONS, KNOWN_SPEC, MOVEMENTS, SelectionManifest, canonical_selection_digest,
    check_git_sha, check_portable_path, check_sha256_digest, known_value_or_fail, load_strict_json,
    reject_secret_tokens, reject_unknown_keys, require_string, validate_selection_manifest,
};

const RERUN_COMMAND: &str = "cargo xtask python-repair-trust check-verification";
const CHECK_REPORT_JSON: &str = "python-repair-verification-check.json";
const CHECK_REPORT_MD: &str = "python-repair-verification-check.md";

/// The receipt schema mirrors the crate's verification phase output exactly.
/// The crate and xtask share the schema by construction, not by a shared
/// dependency: the digest anchors are the contract.
const RECEIPT_KIND: &str = "python_repair_verification_receipt";
const RECEIPT_SCHEMA_VERSION: &str = "0.1";

const RECEIPT_KEYS: [&str; 16] = [
    "schema_version",
    "kind",
    "spec",
    "phase",
    "durable_attempt_id",
    "trust_attempt_id",
    "seam_id",
    "repository_head",
    "target_path",
    "identities",
    "command",
    "execution",
    "movement",
    "rollback",
    "non_claims",
    "claim_boundary",
];

/// The standing claim boundary every receipt carries verbatim (the same
/// constant the producing phase renders; the schema is shared by
/// construction, and a dropped or edited boundary fails validation).
const RECEIPT_CLAIM_BOUNDARY: &str = "Execution observation and static before/after movement are separate draft evidence axes; real mutation testing confirms behavior-change detection later.";

const IDENTITIES_KEYS: [&str; 8] = [
    "packet_sha256",
    "before_snapshot_sha256",
    "patch_sha256",
    "selection_manifest_sha256",
    "selection_digest",
    "binding_artifact_sha256",
    "config_profile",
    "analyzer_binary_sha256",
];

const COMMAND_KEYS: [&str; 3] = ["command_spec_sha256", "display", "authorization"];
const AUTHORIZATION_KEYS: [&str; 3] = ["status", "authority", "method"];
const AUTHORIZATION_STATUSES: [&str; 1] = ["granted"];
const AUTHORIZATION_METHODS: [&str; 1] = ["explicit-operator-flags"];

const EXECUTION_KEYS: [&str; 14] = [
    "state",
    "process_disposition",
    "exit_status",
    "exit_signal",
    "stdout_sha256",
    "stderr_sha256",
    "stdout_bytes",
    "stderr_bytes",
    "stdout_truncated",
    "stderr_truncated",
    "currentness",
    "duration_ms",
    "cancellation_requested",
    "reason",
];

const MOVEMENT_KEYS: [&str; 7] = [
    "state",
    "reason",
    "identity",
    "join",
    "unrelated",
    "after_snapshot_sha256",
    "after_run_status",
];

const NATIVE_IDENTITY_KEYS: [&str; 5] = ["family", "owner", "discriminator", "relation", "oracle"];

const JOIN_KEYS: [&str; 8] = [
    "before_seam_id",
    "after_seam_id",
    "before_grip_class",
    "after_grip_class",
    "before_oracle_strength",
    "after_oracle_strength",
    "before_headline_eligible",
    "after_headline_eligible",
];

const UNRELATED_KEYS: [&str; 4] = [
    "actionable_before",
    "actionable_after",
    "regressed",
    "improved",
];

const ROLLBACK_KEYS: [&str; 3] = ["state", "reason", "post_rollback_head"];

/// The retained process dispositions the bounded rails (and the phase's own
/// pre-execution refusals) can record.
const PROCESS_DISPOSITIONS: [&str; 7] = [
    "completed",
    "failed_to_start",
    "cancelled",
    "timed_out",
    "output_limit_exceeded",
    "rejected_before_execution",
    "route_unavailable",
];

/// The currentness labels the bounded rails retain.
const CURRENTNESS: [&str; 3] = ["current", "dirty_worktree", "historical_noncurrent"];

/// The after-analysis run statuses the producer can retain (the repo-exposure
/// document's own run_status vocabulary, restated).
const AFTER_RUN_STATUSES: [&str; 2] = ["complete", "seam_limit_applied"];

/// The standing non-claims every verification receipt must carry verbatim.
const VERIFICATION_NON_CLAIMS: [&str; 3] = [
    "no lifecycle, acceptance, or closure state is claimed by the verification record",
    "no repair correctness is claimed by the verification record",
    "no support, gate, badge, or promotion claim is made",
];

/// Fail-closed error text: names subject/field/reason plus the rerun command.
fn verification_fail(subject: &str, field: &str, reason: impl std::fmt::Display) -> String {
    format!(
        "python-repair-trust check-verification failed: subject=`{subject}` field=`{field}`: {reason}
rerun: {RERUN_COMMAND}"
    )
}

/// The durable attempt identity shape (#2927): `repair-attempt-` plus 24
/// lowercase hexadecimal characters.
fn check_durable_attempt_id(subject: &str, field: &str, value: &str) -> Result<(), String> {
    let shaped = match value.strip_prefix("repair-attempt-") {
        Some(suffix) => {
            suffix.len() == 24
                && suffix
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        }
        None => false,
    };
    if shaped {
        Ok(())
    } else {
        Err(verification_fail(
            subject,
            field,
            "durable attempt id must be `repair-attempt-` plus 24 lowercase hexadecimal characters",
        ))
    }
}

/// One validated receipt, reduced to what the report renders.
#[derive(Debug)]
struct VerificationReceipt {
    display: String,
    trust_attempt_id: String,
    execution_state: String,
    movement_state: String,
    rollback_state: String,
    unrelated_regressed: bool,
    unrelated_improved: bool,
}

struct VerificationCheckOutcome {
    manifest_path: String,
    manifest: Option<SelectionManifest>,
    receipts_input: Option<String>,
    receipts: Vec<VerificationReceipt>,
    violations: Vec<String>,
}

impl VerificationCheckOutcome {
    fn verdict(&self) -> &'static str {
        // Violations take precedence: a set of ALL-invalid receipts has zero
        // valid records but must fail closed as `inconsistent` (nonzero
        // exit), never pass as `not_run`.
        if !self.violations.is_empty() {
            return "inconsistent";
        }
        if self.receipts.is_empty() {
            "not_run"
        } else {
            "valid"
        }
    }
}

fn parse_check_verification_args(args: &[String]) -> Result<(String, String), String> {
    let mut manifest = super::python_repair_trust::DEFAULT_MANIFEST.to_string();
    let mut receipts: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                index += 1;
                manifest = args.get(index).cloned().ok_or_else(|| {
                    format!(
                        "python-repair-trust check-verification --manifest requires a value\nrerun: {RERUN_COMMAND}"
                    )
                })?;
            }
            "--receipts" => {
                index += 1;
                receipts = Some(args.get(index).cloned().ok_or_else(|| {
                    format!(
                        "python-repair-trust check-verification --receipts requires a value\nrerun: {RERUN_COMMAND}"
                    )
                })?);
            }
            other => {
                return Err(format!(
                    "unknown python-repair-trust check-verification argument: {other}\nusage: cargo xtask python-repair-trust check-verification [--manifest <path>] --receipts <dir-or-file>\nrerun: {RERUN_COMMAND}"
                ));
            }
        }
        index += 1;
    }
    let receipts = receipts.ok_or_else(|| {
        format!(
            "python-repair-trust check-verification requires --receipts <dir-or-file>\nrerun: {RERUN_COMMAND}"
        )
    })?;
    Ok((manifest, receipts))
}

/// The command entry: `python-repair-trust check-verification`, dispatched
/// from the `python-repair-trust` subcommand router.
pub(crate) fn run_check_verification(args: &[String]) -> Result<(), String> {
    let (manifest_path, receipts_input) = parse_check_verification_args(args)?;
    let outcome = check_verification_artifacts(&manifest_path, &receipts_input)?;
    let verdict = outcome.verdict();

    match &outcome.manifest {
        None => println!(
            "python-repair-trust check-verification: manifest={manifest_path} selections=<none> (no accepted selection manifest; verification receipts cannot bind without one)"
        ),
        Some(manifest) => println!(
            "python-repair-trust check-verification: manifest={manifest_path} selections={} sha256={}",
            manifest.selections.len(),
            manifest.sha256
        ),
    }
    println!(
        "python-repair-trust check-verification: receipts={receipts_input} receipts={} violations={} verdict={verdict}",
        outcome.receipts.len(),
        outcome.violations.len()
    );
    for violation in &outcome.violations {
        println!("  violation: {violation}");
    }
    println!(
        "python-repair-trust check-verification verdict: {verdict} — receipt structural validation only; no completed or correct repair is established and execution never implies movement"
    );
    println!("rerun: {RERUN_COMMAND}");

    let report = render_check_verification_json(&outcome)?;
    crate::write_report(CHECK_REPORT_JSON, &format!("{report}\n"))?;
    let markdown = render_check_verification_markdown(&outcome, verdict);
    crate::write_report(CHECK_REPORT_MD, &markdown)?;
    if verdict == "inconsistent" {
        return Err(format!(
            "python-repair-trust check-verification found {} violation(s); see target/ripr/reports/{CHECK_REPORT_JSON}",
            outcome.violations.len()
        ));
    }
    Ok(())
}

fn check_verification_artifacts(
    manifest_path: &str,
    receipts_input: &str,
) -> Result<VerificationCheckOutcome, String> {
    if !Path::new(manifest_path).exists() {
        return Err(verification_fail(
            manifest_path,
            "manifest",
            "verification receipts bind to an accepted selection manifest; no manifest exists at this path",
        ));
    }
    let (value, manifest_sha256) = load_strict_json(manifest_path)?;
    let manifest = validate_selection_manifest(&value, manifest_sha256.clone())?;

    let path = Path::new(receipts_input);
    if !path.exists() {
        return Err(verification_fail(
            receipts_input,
            "receipts",
            "receipts input path does not exist",
        ));
    }
    let files = if path.is_dir() {
        let mut entries = Vec::new();
        let read = std::fs::read_dir(path).map_err(|error| {
            verification_fail(
                receipts_input,
                "receipts",
                format!("failed to read directory: {error}"),
            )
        })?;
        for entry in read {
            let entry = entry.map_err(|error| {
                verification_fail(
                    receipts_input,
                    "receipts",
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
        entries
    } else {
        vec![receipts_input.to_string()]
    };

    let mut receipts = Vec::new();
    let mut violations = Vec::new();
    for display in &files {
        match load_strict_json(display) {
            Ok((record, _sha256)) => {
                match validate_receipt(display, &record, &manifest, &manifest_sha256) {
                    Ok(receipt) => receipts.push(receipt),
                    Err(violation) => violations.push(violation),
                }
            }
            Err(error) => violations.push(error),
        }
    }
    Ok(VerificationCheckOutcome {
        manifest_path: manifest_path.to_string(),
        manifest: Some(manifest),
        receipts_input: Some(receipts_input.to_string()),
        receipts,
        violations,
    })
}

/// Reads one nested closed object block, denying unknown keys.
fn block<'a>(
    top: &'a serde_json::Map<String, Value>,
    parent: &str,
    field: &str,
    allowed: &[&str],
    display: &str,
) -> Result<&'a serde_json::Map<String, Value>, String> {
    let subject = format!("{parent}.{field}");
    let value = top
        .get(field)
        .ok_or_else(|| verification_fail(display, &subject, "required block is missing"))?;
    let object = value
        .as_object()
        .ok_or_else(|| verification_fail(display, &subject, "block must be a JSON object"))?;
    reject_unknown_keys(object, allowed, display, &subject)?;
    Ok(object)
}

/// Optional string field: `None` when absent or explicitly null (the same
/// null-as-absent rule the corpus validator applies to optional identities),
/// an error when blank or mistyped. Required fields go through
/// `require_string`, which never accepts a null.
fn opt_receipt_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
    display: &str,
) -> Result<Option<String>, String> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if !text.trim().is_empty() => Ok(Some(text.clone())),
        Some(Value::String(_)) => Err(verification_fail(
            display,
            field,
            "string field must be non-empty when present",
        )),
        Some(_) => Err(verification_fail(
            display,
            field,
            "field must be a string when present",
        )),
    }
}

fn opt_u64_field(
    object: &serde_json::Map<String, Value>,
    field: &str,
    display: &str,
) -> Result<Option<u64>, String> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or_else(|| {
            verification_fail(
                display,
                field,
                "field must be a non-negative integer when present",
            )
        }),
    }
}

/// Signed integer field: the producer records `exit_status`/`exit_signal` as
/// `Option<i32>`, and a Windows termination code is a negative `i32`, so a
/// receipt carrying one must validate instead of being misread as malformed.
fn opt_i64_field(
    object: &serde_json::Map<String, Value>,
    field: &str,
    display: &str,
) -> Result<Option<i64>, String> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_i64().map(Some).ok_or_else(|| {
            verification_fail(display, field, "field must be an integer when present")
        }),
    }
}

/// Boolean field: present means a JSON boolean; absent or null means `None`.
fn opt_bool_field(
    object: &serde_json::Map<String, Value>,
    field: &str,
    display: &str,
) -> Result<Option<bool>, String> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(flag)) => Ok(Some(*flag)),
        Some(_) => Err(verification_fail(
            display,
            field,
            "field must be a boolean when present",
        )),
    }
}

/// The oracle-strength rank the producing movement comparison uses
/// (strong > medium > weak > smoke > unknown > everything else). An
/// unrecognized spelling ranks lowest so an unrecognized oracle never counts
/// as evidence of strength — the same rule on both sides of the schema.
fn movement_oracle_rank(strength: &str) -> u8 {
    match strength {
        "strong" => 5,
        "medium" => 4,
        "weak" => 3,
        "smoke" => 2,
        "unknown" => 1,
        _ => 0,
    }
}

/// The headline/oracle transition table the producer applies to the joined
/// before/after seams. The validator re-derives the state from the recorded
/// joins so a receipt whose state contradicts its own evidence fails.
fn movement_state_from_joins(
    before_headline: bool,
    after_headline: bool,
    before_oracle_strength: &str,
    after_oracle_strength: &str,
) -> &'static str {
    use std::cmp::Ordering;
    match (
        before_headline,
        after_headline,
        movement_oracle_rank(after_oracle_strength)
            .cmp(&movement_oracle_rank(before_oracle_strength)),
    ) {
        (false, false, _) => "unchanged",
        (false, true, _) => "regressed",
        (true, false, _) => "closed",
        (true, true, Ordering::Greater) => "improved",
        (true, true, Ordering::Equal) => "unchanged",
        (true, true, Ordering::Less) => "regressed",
    }
}

/// Validates one candidate receipt against the accepted selection manifest.
/// Every digest anchor is recomputed, every closed field set is deny-unknown,
/// the execution and movement vocabularies stay closed and disposition-bound,
/// and the separation laws are pinned by what this function does NOT check:
/// no rule reads the execution state to derive the movement state or the
/// other way around.
fn validate_receipt(
    display: &str,
    record: &Value,
    manifest: &SelectionManifest,
    manifest_sha256: &str,
) -> Result<VerificationReceipt, String> {
    let top = record.as_object().ok_or_else(|| {
        verification_fail(
            display,
            "record",
            "verification receipt must be a JSON object",
        )
    })?;
    reject_unknown_keys(top, &RECEIPT_KEYS, display, "verification receipt")?;
    for (field, expected) in [
        ("schema_version", RECEIPT_SCHEMA_VERSION),
        ("kind", RECEIPT_KIND),
        ("spec", KNOWN_SPEC),
        ("phase", "verify"),
    ] {
        let actual = require_string(display, top, field)?;
        if actual != expected {
            return Err(verification_fail(
                display,
                field,
                format!("expected `{expected}`, got `{actual}`"),
            ));
        }
    }
    // The standing claim boundary rides on every receipt verbatim; a dropped,
    // edited, or weakened boundary is a different record, not this schema.
    let claim_boundary = require_string(display, top, "claim_boundary")?;
    if claim_boundary != RECEIPT_CLAIM_BOUNDARY {
        return Err(verification_fail(
            display,
            "claim_boundary",
            format!("expected the standing claim boundary verbatim, got `{claim_boundary}`"),
        ));
    }
    let durable_attempt_id = require_string(display, top, "durable_attempt_id")?;
    check_durable_attempt_id(display, "durable_attempt_id", &durable_attempt_id)?;
    let seam_id = require_string(display, top, "seam_id")?;
    let repository_head = require_string(display, top, "repository_head")?;
    check_git_sha(display, "repository_head", &repository_head)?;

    // Identities: every digest anchor recomputed or shape-checked.
    let identities = block(top, "receipt", "identities", &IDENTITIES_KEYS, display)?;
    for digest_field in [
        "packet_sha256",
        "before_snapshot_sha256",
        "patch_sha256",
        "selection_manifest_sha256",
        "selection_digest",
        "binding_artifact_sha256",
        "analyzer_binary_sha256",
    ] {
        let digest = require_string(display, identities, digest_field)?;
        check_sha256_digest(display, &format!("identities.{digest_field}"), &digest)?;
    }
    require_string(display, identities, "config_profile")?;
    let recorded_manifest_digest =
        require_string(display, identities, "selection_manifest_sha256")?;
    if recorded_manifest_digest != manifest_sha256 {
        return Err(verification_fail(
            display,
            "identities.selection_manifest_sha256",
            format!(
                "stale digest: the receipt binds to manifest {recorded_manifest_digest} but the accepted selection manifest is {manifest_sha256}"
            ),
        ));
    }
    let trust_attempt_id = require_string(display, top, "trust_attempt_id")?;
    let selection = manifest
        .selections
        .iter()
        .find(|selection| selection.attempt_id == trust_attempt_id)
        .ok_or_else(|| {
            verification_fail(
                &trust_attempt_id,
                "trust_attempt_id",
                "names an attempt identity outside the accepted selection denominator; selected rows cannot be substituted by name",
            )
        })?;
    let row_preimage = manifest
        .row_preimages
        .get(&trust_attempt_id)
        .ok_or_else(|| {
            verification_fail(
                &trust_attempt_id,
                "identities.selection_digest",
                "the accepted selection manifest retained no canonical preimage for this row",
            )
        })?;
    let recorded_selection_digest = require_string(display, identities, "selection_digest")?;
    let recomputed = canonical_selection_digest(row_preimage, &trust_attempt_id)?;
    if recorded_selection_digest != recomputed {
        return Err(verification_fail(
            &trust_attempt_id,
            "identities.selection_digest",
            format!(
                "selection row digest mismatch: the receipt pins `{recorded_selection_digest}` but the accepted row now digests to `{recomputed}`; a replaced or edited selected row makes the receipt stale"
            ),
        ));
    }

    // Command block: the typed route identity (both fields present together
    // or absent together) plus the explicit authorization pair.
    let command = block(top, "receipt", "command", &COMMAND_KEYS, display)?;
    let command_spec_sha256 = opt_receipt_string(command, "command_spec_sha256", display)?;
    let command_display = opt_receipt_string(command, "display", display)?;
    if command_spec_sha256.is_some() != command_display.is_some() {
        return Err(verification_fail(
            display,
            "command.command_spec_sha256",
            "the typed route digest and the display must be recorded together or omitted together",
        ));
    }
    if let Some(digest) = &command_spec_sha256 {
        check_sha256_digest(display, "command.command_spec_sha256", digest)?;
    }
    let authorization = block(
        command,
        "command",
        "authorization",
        &AUTHORIZATION_KEYS,
        display,
    )?;
    let status = require_string(display, authorization, "status")?;
    known_value_or_fail(
        display,
        "command.authorization.status",
        &status,
        &AUTHORIZATION_STATUSES,
        "authorization status",
    )?;
    require_string(display, authorization, "authority")?;
    let method = require_string(display, authorization, "method")?;
    known_value_or_fail(
        display,
        "command.authorization.method",
        &method,
        &AUTHORIZATION_METHODS,
        "authorization method",
    )?;

    // Execution axis: closed vocabulary, disposition agreement, and retained
    // output commitments. No rule below reads the movement block.
    let execution = block(top, "receipt", "execution", &EXECUTION_KEYS, display)?;
    let execution_state = require_string(display, execution, "state")?;
    known_value_or_fail(
        display,
        "execution.state",
        &execution_state,
        &EXECUTIONS,
        "verification execution",
    )?;
    let disposition = require_string(display, execution, "process_disposition")?;
    known_value_or_fail(
        display,
        "execution.process_disposition",
        &disposition,
        &PROCESS_DISPOSITIONS,
        "process disposition",
    )?;
    let exit_status = opt_i64_field(execution, "exit_status", display)?;
    let exit_signal = opt_i64_field(execution, "exit_signal", display)?;
    let stdout_digest = opt_receipt_string(execution, "stdout_sha256", display)?;
    let stderr_digest = opt_receipt_string(execution, "stderr_sha256", display)?;
    if let Some(digest) = &stdout_digest {
        check_sha256_digest(display, "execution.stdout_sha256", digest)?;
    }
    if let Some(digest) = &stderr_digest {
        check_sha256_digest(display, "execution.stderr_sha256", digest)?;
    }
    opt_u64_field(execution, "stdout_bytes", display)?;
    opt_u64_field(execution, "stderr_bytes", display)?;
    opt_u64_field(execution, "duration_ms", display)?;
    for flag_field in [
        "stdout_truncated",
        "stderr_truncated",
        "cancellation_requested",
    ] {
        opt_bool_field(execution, flag_field, display)?;
    }
    let currentness = opt_receipt_string(execution, "currentness", display)?;
    if let Some(currentness) = &currentness {
        known_value_or_fail(
            display,
            "execution.currentness",
            currentness,
            &CURRENTNESS,
            "execution currentness",
        )?;
    }
    let execution_reason = opt_receipt_string(execution, "reason", display)?;
    let route_recorded = command_spec_sha256.is_some();
    // Disposition/state agreement: `passed` is completed with the route's
    // accepted exit code (0) and full commitments; every non-verdict state
    // carries no exit status; each terminal state keeps its own name so
    // timeout, cancellation, spawn failure, non-zero exit, and unobservable
    // runs stay distinct.
    let ran_with_commitments = |display_field: &str| -> Result<(), String> {
        if stdout_digest.is_none() || stderr_digest.is_none() {
            return Err(verification_fail(
                display,
                display_field,
                "a stateless run still retains both output commitments; stdout_sha256 and stderr_sha256 are required once the route ran",
            ));
        }
        if currentness.is_none() {
            return Err(verification_fail(
                display,
                display_field,
                "a run retains its currentness disposition",
            ));
        }
        if !route_recorded {
            return Err(verification_fail(
                display,
                display_field,
                "a run retains the typed command identity it executed",
            ));
        }
        Ok(())
    };
    match execution_state.as_str() {
        "passed" => {
            if disposition != "completed" {
                return Err(verification_fail(
                    display,
                    "execution.state",
                    format!(
                        "`passed` requires process_disposition `completed`, got `{disposition}`"
                    ),
                ));
            }
            if exit_status != Some(0) {
                return Err(verification_fail(
                    display,
                    "execution.exit_status",
                    format!(
                        "`passed` requires exit status 0, got {exit_status:?}; a non-zero exit is `failed`"
                    ),
                ));
            }
            if exit_signal.is_some() {
                return Err(verification_fail(
                    display,
                    "execution.exit_signal",
                    "`passed` is a clean exit and carries no termination signal",
                ));
            }
            ran_with_commitments("execution.state")?;
            if currentness.as_deref() == Some("historical_noncurrent") {
                return Err(verification_fail(
                    display,
                    "execution.currentness",
                    "`passed` cannot ride a historical (non-current) run",
                ));
            }
        }
        "failed" => {
            if disposition != "completed" {
                return Err(verification_fail(
                    display,
                    "execution.state",
                    format!(
                        "`failed` requires process_disposition `completed`, got `{disposition}`"
                    ),
                ));
            }
            // A completed observation is failed on a non-zero exit or on a
            // termination signal (exit status null, signal retained); exactly
            // one of the two is present, so a fabricated pair fails closed.
            match (exit_status, exit_signal) {
                (Some(code), None) if code != 0 => {}
                (None, Some(_)) => {}
                _ => {
                    return Err(verification_fail(
                        display,
                        "execution.exit_status",
                        "`failed` requires a non-zero exit status or a retained termination signal",
                    ));
                }
            }
            ran_with_commitments("execution.state")?;
        }
        "timed_out" | "cancelled" => {
            if disposition != execution_state {
                return Err(verification_fail(
                    display,
                    "execution.state",
                    format!(
                        "`{}` requires process_disposition `{}`, got `{disposition}`",
                        execution_state, execution_state
                    ),
                ));
            }
            if exit_status.is_some() {
                return Err(verification_fail(
                    display,
                    "execution.exit_status",
                    format!("`{}` carries no exit status", execution_state),
                ));
            }
            if exit_signal.is_some() {
                return Err(verification_fail(
                    display,
                    "execution.exit_signal",
                    format!("`{}` carries no termination signal", execution_state),
                ));
            }
            ran_with_commitments("execution.state")?;
        }
        "unavailable" => {
            if disposition != "failed_to_start" && disposition != "route_unavailable" {
                return Err(verification_fail(
                    display,
                    "execution.state",
                    format!(
                        "`unavailable` requires process_disposition `failed_to_start` or `route_unavailable`, got `{disposition}`"
                    ),
                ));
            }
            if exit_status.is_some() {
                return Err(verification_fail(
                    display,
                    "execution.exit_status",
                    "`unavailable` carries no exit status",
                ));
            }
            if exit_signal.is_some() {
                return Err(verification_fail(
                    display,
                    "execution.exit_signal",
                    "`unavailable` carries no termination signal",
                ));
            }
            if execution_reason.is_none() {
                return Err(verification_fail(
                    display,
                    "execution.reason",
                    "`unavailable` retains the typed reason",
                ));
            }
        }
        "not_run" => {
            if disposition != "rejected_before_execution" {
                return Err(verification_fail(
                    display,
                    "execution.state",
                    format!(
                        "`not_run` requires process_disposition `rejected_before_execution`, got `{disposition}`"
                    ),
                ));
            }
            if exit_status.is_some()
                || stdout_digest.is_some()
                || stderr_digest.is_some()
                || currentness.is_some()
            {
                return Err(verification_fail(
                    display,
                    "execution.state",
                    "`not_run` retains no exit status, no output commitments, and no currentness",
                ));
            }
            if exit_signal.is_some() {
                return Err(verification_fail(
                    display,
                    "execution.exit_signal",
                    "`not_run` carries no termination signal",
                ));
            }
            if route_recorded {
                return Err(verification_fail(
                    display,
                    "execution.state",
                    "`not_run` retains no typed command identity",
                ));
            }
            if execution_reason.is_none() {
                return Err(verification_fail(
                    display,
                    "execution.reason",
                    "`not_run` retains the typed reason",
                ));
            }
        }
        "invalid" => {
            if disposition != "output_limit_exceeded" && disposition != "rejected_before_execution"
            {
                return Err(verification_fail(
                    display,
                    "execution.state",
                    format!(
                        "`invalid` requires process_disposition `output_limit_exceeded` or `rejected_before_execution`, got `{disposition}`"
                    ),
                ));
            }
            if exit_status.is_some() {
                return Err(verification_fail(
                    display,
                    "execution.exit_status",
                    "`invalid` carries no exit status",
                ));
            }
            if exit_signal.is_some() {
                return Err(verification_fail(
                    display,
                    "execution.exit_signal",
                    "`invalid` carries no termination signal",
                ));
            }
            if execution_reason.is_none() {
                return Err(verification_fail(
                    display,
                    "execution.reason",
                    "`invalid` retains the typed reason",
                ));
            }
        }
        _ => {
            // Unreachable: the vocabulary check above already failed.
        }
    }

    // Movement axis: closed vocabulary, recorded native-identity join, typed
    // reasons for degraded classifications, and the unrelated-finding block.
    // No rule below reads the execution block.
    let movement = block(top, "receipt", "movement", &MOVEMENT_KEYS, display)?;
    let movement_state = require_string(display, movement, "state")?;
    known_value_or_fail(
        display,
        "movement.state",
        &movement_state,
        &MOVEMENTS,
        "static movement",
    )?;
    let movement_reason = opt_receipt_string(movement, "reason", display)?;
    for degraded in ["stale", "uncertain", "limited"] {
        if movement_state == degraded && movement_reason.is_none() {
            return Err(verification_fail(
                display,
                "movement.reason",
                format!(
                    "a `{degraded}` movement retains the typed reason it is not a confident classification"
                ),
            ));
        }
    }
    for confident in ["closed", "improved", "unchanged", "regressed"] {
        if movement_state == confident && movement_reason.is_some() {
            return Err(verification_fail(
                display,
                "movement.reason",
                format!(
                    "a confident `{confident}` classification carries no degradation reason; the reason field is reserved for stale/uncertain/limited"
                ),
            ));
        }
    }
    let after_snapshot = require_string(display, movement, "after_snapshot_sha256")?;
    check_sha256_digest(display, "movement.after_snapshot_sha256", &after_snapshot)?;
    let after_run_status = require_string(display, movement, "after_run_status")?;
    known_value_or_fail(
        display,
        "movement.after_run_status",
        &after_run_status,
        &AFTER_RUN_STATUSES,
        "after-analysis run status",
    )?;
    // Native identity: the accepted row is authoritative, never the receipt.
    let identity = block(
        movement,
        "movement",
        "identity",
        &NATIVE_IDENTITY_KEYS,
        display,
    )?;
    for field in NATIVE_IDENTITY_KEYS {
        let receipt_value = require_string(display, identity, field)?;
        let row_value = row_preimage
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| {
                verification_fail(
                    &trust_attempt_id,
                    &format!("movement.identity.{field}"),
                    format!("the accepted selection row carries no comparable `{field}`"),
                )
            })?;
        if receipt_value != row_value {
            return Err(verification_fail(
                &trust_attempt_id,
                &format!("movement.identity.{field}"),
                format!(
                    "identity disagreement: the receipt names `{receipt_value}` but the accepted selection row names `{row_value}`; a rewritten identity copy is rejected"
                ),
            ));
        }
    }
    let row_target = normalize_repo_relative_path(&selection.target_path);
    let receipt_target_raw = require_string(display, top, "target_path")?;
    check_portable_path(display, "target_path", &receipt_target_raw)?;
    let receipt_target = normalize_repo_relative_path(&receipt_target_raw);
    if receipt_target != row_target {
        return Err(verification_fail(
            &trust_attempt_id,
            "target_path",
            format!(
                "target identity disagreement: the receipt names `{receipt_target}` but the accepted selection row names `{row_target}`"
            ),
        ));
    }
    let join = block(movement, "movement", "join", &JOIN_KEYS, display)?;
    // Typed join fields: seam ids and oracle strengths are non-empty strings,
    // headline eligibility is a boolean, and an absent join is an explicit
    // null — never a mistyped scalar.
    let join_string = |field: &str| -> Result<Option<String>, String> {
        match join.get(field) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(text)) if !text.trim().is_empty() => Ok(Some(text.clone())),
            Some(Value::String(_)) => Err(verification_fail(
                display,
                &format!("movement.join.{field}"),
                "join field must be a non-empty string when present",
            )),
            Some(_) => Err(verification_fail(
                display,
                &format!("movement.join.{field}"),
                "join field must be a string when present",
            )),
        }
    };
    let join_bool = |field: &str| -> Result<Option<bool>, String> {
        opt_bool_field(join, field, display).map_err(|error| {
            error.replace(
                &format!("field=`{field}`"),
                &format!("field=`movement.join.{field}`"),
            )
        })
    };
    let before_seam_id = join_string("before_seam_id")?;
    let after_seam_id = join_string("after_seam_id")?;
    let before_grip_class = join_string("before_grip_class")?;
    let after_grip_class = join_string("after_grip_class")?;
    let before_oracle_strength = join_string("before_oracle_strength")?;
    let after_oracle_strength = join_string("after_oracle_strength")?;
    let before_headline_eligible = join_bool("before_headline_eligible")?;
    let after_headline_eligible = join_bool("after_headline_eligible")?;
    let before_join = (
        &before_seam_id,
        &before_grip_class,
        &before_oracle_strength,
        &before_headline_eligible,
    );
    let after_join = (
        &after_seam_id,
        &after_grip_class,
        &after_oracle_strength,
        &after_headline_eligible,
    );
    let before_complete = before_join.0.is_some()
        && before_join.1.is_some()
        && before_join.2.is_some()
        && before_join.3.is_some();
    let before_absent = before_join.0.is_none()
        && before_join.1.is_none()
        && before_join.2.is_none()
        && before_join.3.is_none();
    let after_complete = after_join.0.is_some()
        && after_join.1.is_some()
        && after_join.2.is_some()
        && after_join.3.is_some();
    let after_absent = after_join.0.is_none()
        && after_join.1.is_none()
        && after_join.2.is_none()
        && after_join.3.is_none();
    // The join shapes mirror the producer exactly: a confident classification
    // resolves both joins and its recorded state must be the one its own
    // headline/oracle evidence implies (the same transition table the
    // producing comparison applies); the degraded shapes carry only what the
    // producer can record for them.
    match movement_state.as_str() {
        "closed" | "improved" | "unchanged" | "regressed" => {
            if !before_complete || !after_complete {
                return Err(verification_fail(
                    display,
                    "movement.join",
                    format!(
                        "a confident `{movement_state}` movement retains the complete before and after joins; a partial or null join cannot locate a confident classification"
                    ),
                ));
            }
            if before_seam_id.as_deref() != Some(seam_id.as_str()) {
                return Err(verification_fail(
                    display,
                    "movement.join.before_seam_id",
                    format!(
                        "a confident `{movement_state}` movement's before join names `{before_seam_id:?}` but the receipt binds seam `{seam_id}`; a different before seam is `stale`, not confident"
                    ),
                ));
            }
            let derived = movement_state_from_joins(
                before_headline_eligible.unwrap_or(false),
                after_headline_eligible.unwrap_or(false),
                before_oracle_strength.as_deref().unwrap_or_default(),
                after_oracle_strength.as_deref().unwrap_or_default(),
            );
            if derived != movement_state {
                return Err(verification_fail(
                    display,
                    "movement.state",
                    format!(
                        "the recorded `{movement_state}` movement contradicts its own joins (before headline {before_headline_eligible:?} / `{before_oracle_strength:?}`, after headline {after_headline_eligible:?} / `{after_oracle_strength:?}`), which resolve to `{derived}`"
                    ),
                ));
            }
        }
        "stale" => {
            if !before_complete || !after_absent {
                return Err(verification_fail(
                    display,
                    "movement.join",
                    "a `stale` movement retains the complete before join and no after join (the stale identity never resolved after the edit)",
                ));
            }
            if before_seam_id.as_deref() == Some(seam_id.as_str()) {
                return Err(verification_fail(
                    display,
                    "movement.join.before_seam_id",
                    format!(
                        "the `stale` movement's before join names the receipt's own seam `{seam_id}`; a stale join requires the identity to have moved to a different seam"
                    ),
                ));
            }
        }
        "limited" => {
            if after_run_status == "complete" {
                return Err(verification_fail(
                    display,
                    "movement.state",
                    "a `limited` movement requires a partial after analysis (`seam_limit_applied`); a complete analysis resolves the join or fails closed to `uncertain`",
                ));
            }
            if !before_absent || !after_absent {
                return Err(verification_fail(
                    display,
                    "movement.join",
                    "a `limited` movement carries no join (the comparison never resolves joins over a partial analysis)",
                ));
            }
        }
        _ => {
            // `uncertain`: the comparison can stop before the before join
            // (zero/multiple/empty identity — no join fields at all) or after
            // it (zero/multiple after join — the complete before join only).
            if !after_absent {
                return Err(verification_fail(
                    display,
                    "movement.join",
                    "an `uncertain` movement never retains an after join (a resolved after join is a confident classification)",
                ));
            }
            if !before_absent && !before_complete {
                return Err(verification_fail(
                    display,
                    "movement.join",
                    "an `uncertain` movement carries either no before join or the complete before join; a partial before join is not a producer shape",
                ));
            }
            if before_complete && before_seam_id.as_deref() != Some(seam_id.as_str()) {
                return Err(verification_fail(
                    display,
                    "movement.join.before_seam_id",
                    format!(
                        "the `uncertain` movement's before join names `{before_seam_id:?}` but the receipt binds seam `{seam_id}`; a resolved before join must be the receipt's own seam"
                    ),
                ));
            }
        }
    }
    let unrelated = block(movement, "movement", "unrelated", &UNRELATED_KEYS, display)?;
    let actionable_before = require_u64(unrelated, "actionable_before", display)?;
    let actionable_after = require_u64(unrelated, "actionable_after", display)?;
    let unrelated_regressed = require_bool(unrelated, "regressed", display)?;
    let unrelated_improved = require_bool(unrelated, "improved", display)?;
    // Row-derived consistency: the booleans restate the count comparison, so
    // a hand-edited flag fails even while the counts stay present.
    if unrelated_regressed && actionable_after <= actionable_before {
        return Err(verification_fail(
            display,
            "movement.unrelated.regressed",
            format!(
                "the receipt claims an unrelated regression but the counts show {actionable_before} -> {actionable_after}; a regression requires the unrelated actionable count to grow"
            ),
        ));
    }
    if unrelated_improved && actionable_after >= actionable_before {
        return Err(verification_fail(
            display,
            "movement.unrelated.improved",
            format!(
                "the receipt claims unrelated improvement but the counts show {actionable_before} -> {actionable_after}; an improvement requires the unrelated actionable count to shrink"
            ),
        ));
    }
    if !unrelated_regressed && !unrelated_improved && actionable_after != actionable_before {
        return Err(verification_fail(
            display,
            "movement.unrelated",
            format!(
                "the receipt claims no unrelated movement but the counts changed ({actionable_before} -> {actionable_after}); movement must stay visible"
            ),
        ));
    }

    // Rollback: a typed disposition with its evidence.
    let rollback = block(top, "receipt", "rollback", &ROLLBACK_KEYS, display)?;
    let rollback_state = require_string(display, rollback, "state")?;
    known_value_or_fail(
        display,
        "rollback.state",
        &rollback_state,
        &["proved", "blocked", "not_run"],
        "rollback disposition",
    )?;
    let rollback_reason = opt_receipt_string(rollback, "reason", display)?;
    let post_rollback_head = opt_receipt_string(rollback, "post_rollback_head", display)?;
    match rollback_state.as_str() {
        "proved" => {
            let head = post_rollback_head.ok_or_else(|| {
                verification_fail(
                    display,
                    "rollback.post_rollback_head",
                    "a proved rollback retains the restored head",
                )
            })?;
            check_git_sha(display, "rollback.post_rollback_head", &head)?;
        }
        "blocked" | "not_run" => {
            if rollback_reason.is_none() {
                return Err(verification_fail(
                    display,
                    "rollback.reason",
                    format!("a `{rollback_state}` rollback retains the typed reason"),
                ));
            }
            if post_rollback_head.is_some() {
                return Err(verification_fail(
                    display,
                    "rollback.post_rollback_head",
                    format!(
                        "a `{rollback_state}` rollback retains no restored head; only a proved rollback carries one"
                    ),
                ));
            }
        }
        _ => {
            // Unreachable: the vocabulary check above already failed.
        }
    }

    // Non-claims: the standing claim boundary must ride on the receipt
    // verbatim.
    let non_claims = match top.get("non_claims") {
        Some(Value::Array(values)) => values,
        _ => {
            return Err(verification_fail(
                display,
                "non_claims",
                "must be an array of non-claim strings",
            ));
        }
    };
    let mut recorded_non_claims = BTreeSet::new();
    for (index, value) in non_claims.iter().enumerate() {
        let text = value.as_str().ok_or_else(|| {
            verification_fail(
                display,
                &format!("non_claims[{index}]"),
                "non-claim must be a string",
            )
        })?;
        recorded_non_claims.insert(text);
    }
    let expected_non_claims: BTreeSet<&str> = VERIFICATION_NON_CLAIMS.into_iter().collect();
    if recorded_non_claims != expected_non_claims {
        return Err(verification_fail(
            display,
            "non_claims",
            format!(
                "must be exactly the standing non-claims {VERIFICATION_NON_CLAIMS:?}; a dropped or extra entry fails"
            ),
        ));
    }

    let record_value = Value::Object(top.clone());
    reject_secret_tokens(&record_value, display, "verification receipt")?;

    Ok(VerificationReceipt {
        display: display.to_string(),
        trust_attempt_id,
        execution_state,
        movement_state,
        rollback_state,
        unrelated_regressed,
        unrelated_improved,
    })
}

fn require_u64(
    object: &serde_json::Map<String, Value>,
    field: &str,
    display: &str,
) -> Result<u64, String> {
    match object.get(field) {
        Some(value) => value.as_u64().ok_or_else(|| {
            verification_fail(
                display,
                &format!("movement.unrelated.{field}"),
                "must be a non-negative integer",
            )
        }),
        None => Err(verification_fail(
            display,
            &format!("movement.unrelated.{field}"),
            "required count is missing",
        )),
    }
}

fn require_bool(
    object: &serde_json::Map<String, Value>,
    field: &str,
    display: &str,
) -> Result<bool, String> {
    match object.get(field) {
        Some(Value::Bool(flag)) => Ok(*flag),
        Some(_) => Err(verification_fail(
            display,
            &format!("movement.unrelated.{field}"),
            "must be a boolean",
        )),
        None => Err(verification_fail(
            display,
            &format!("movement.unrelated.{field}"),
            "required flag is missing",
        )),
    }
}

/// Normalizes a portable repo-relative path: `./` segments dropped, empty
/// segments collapsed (the same spelling rule the driver validator applies).
fn normalize_repo_relative_path(path: &str) -> String {
    path.split('/')
        .filter(|component| !component.is_empty() && *component != ".")
        .collect::<Vec<_>>()
        .join("/")
}

fn render_check_verification_json(outcome: &VerificationCheckOutcome) -> Result<String, String> {
    let receipts: Vec<Value> = outcome
        .receipts
        .iter()
        .map(|receipt| {
            json!({
                "record": receipt.display,
                "trust_attempt_id": receipt.trust_attempt_id,
                "execution": receipt.execution_state,
                "movement": receipt.movement_state,
                "rollback": receipt.rollback_state,
                "unrelated_regressed": receipt.unrelated_regressed,
                "unrelated_improved": receipt.unrelated_improved,
            })
        })
        .collect();
    let report = json!({
        "schema_version": "0.1",
        "kind": "python_repair_verification_check_report",
        "spec": KNOWN_SPEC,
        "manifest": outcome.manifest_path,
        "receipts_input": outcome.receipts_input,
        "receipt_count": outcome.receipts.len(),
        "receipts": receipts,
        "violations": outcome.violations,
        "verdict": outcome.verdict(),
        "claim_boundary": "Execution observation and static movement are separate draft evidence axes; no completed or correct repair is established.",
    });
    serde_json::to_string_pretty(&report).map_err(|error| {
        format!("serialize python-repair-verification check report failed: {error}")
    })
}

fn render_check_verification_markdown(outcome: &VerificationCheckOutcome, verdict: &str) -> String {
    let mut out = String::new();
    out.push_str("# Python Repair Verification Check\n\n");
    out.push_str(&format!("Verdict: **{verdict}**\n\n"));
    out.push_str(
        "Receipt structural validation only — execution and static movement are separate draft evidence axes; no completed or correct repair is established.\n\n",
    );
    out.push_str(&format!(
        "- manifest: {} ({} selections, sha256 `{}`)\n",
        outcome.manifest_path,
        outcome
            .manifest
            .as_ref()
            .map(|manifest| manifest.selections.len())
            .unwrap_or(0),
        outcome
            .manifest
            .as_ref()
            .map(|manifest| manifest.sha256.as_str())
            .unwrap_or("<none>")
    ));
    match &outcome.receipts_input {
        None => out.push_str("- receipts: none supplied (`not_run`; not a pass)\n"),
        Some(input) => {
            out.push_str(&format!(
                "- receipts: {} record(s) from `{input}`\n",
                outcome.receipts.len()
            ));
            for receipt in &outcome.receipts {
                out.push_str(&format!(
                    "  - {} ({}): execution `{}`, movement `{}`, rollback `{}`\n",
                    receipt.trust_attempt_id,
                    receipt.display,
                    receipt.execution_state,
                    receipt.movement_state,
                    receipt.rollback_state
                ));
            }
        }
    }
    if !outcome.violations.is_empty() {
        out.push_str(&format!(
            "\n## Violations ({})\n\n",
            outcome.violations.len()
        ));
        for violation in &outcome.violations {
            out.push_str(&format!("- {violation}\n"));
        }
    }
    out.push_str(&format!("\nrerun: `{RERUN_COMMAND}`\n"));
    out
}

// ---------------------------------------------------------------------------
// Tests (module named `python_repair_verification_semantics` under the file
// module `python_repair_verification`, so `cargo test -p xtask
// python_repair_verification` selects exactly these tests; no unwrap/expect —
// assert macros and Result returns only)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod python_repair_verification_semantics {
    use super::*;

    const GIT_SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const GIT_SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const DIGEST_ONE: &str = "1010101010101010101010101010101010101010101010101010101010101010";
    const DIGEST_TWO: &str = "2020202020202020202020202020202020202020202020202020202020202020";
    const DIGEST_THREE: &str = "3030303030303030303030303030303030303030303030303030303030303030";
    const DIGEST_FOUR: &str = "4040404040404040404040404040404040404040404040404040404040404040";
    const DIGEST_FIVE: &str = "5050505050505050505050505050505050505050505050505050505050505050";
    const DIGEST_SIX: &str = "6060606060606060606060606060606060606060606060606060606060606060";
    const DIGEST_SEVEN: &str = "7070707070707070707070707070707070707070707070707070707070707070";
    const DIGEST_EIGHT: &str = "8080808080808080808080808080808080808080808080808080808080808080";
    const DIGEST_NINE: &str = "9090909090909090909090909090909090909090909090909090909090909090";
    const ATTEMPT: &str = "repair-attempt-0123456789abcdef01234567";
    const TRUST_ATTEMPT: &str = "att-1";

    fn object_mut<'a>(
        value: &'a mut Value,
        subject: &str,
    ) -> Result<&'a mut serde_json::Map<String, Value>, String> {
        value
            .as_object_mut()
            .ok_or_else(|| format!("fixture {subject} is not an object"))
    }

    /// The fixture's accepted selection manifest and its exact-bytes digest.
    fn fixture_manifest() -> Result<(Value, String), String> {
        let mut row = json!({
            "attempt_id": TRUST_ATTEMPT,
            "case_id": format!("case-{TRUST_ATTEMPT}"),
            "subject_id": "subj-1",
            "repository": "https://example.com/subj-1",
            "base": GIT_SHA_A,
            "head": GIT_SHA_B,
            "tree": DIGEST_ONE,
            "source_currentness": "candidate_current",
            "selection_reason": "behavior changed in the diff and the case discriminates it",
            "diversity_stratum": "pytest_library",
            "family": "predicate_boundary",
            "owner": "pricing.calculate_discount",
            "discriminator": "amount >= threshold",
            "relation": "test calls owner directly",
            "oracle": "assert exact boundary value",
            "expected_direction": "should_gap",
            "claim_boundary": "static exposure evidence only",
            "target_path": "tests/test_pricing.py",
            "target_state": "existing",
            "selected_at": "2026-09-10T00:00:00Z",
            "selector": "campaign-selector",
            "authority_snapshot_digest": DIGEST_TWO,
        });
        let row_digest = canonical_selection_digest(object_mut(&mut row, "row")?, TRUST_ATTEMPT)?;
        object_mut(&mut row, "row")?
            .insert("selection_digest".to_string(), Value::String(row_digest));
        let manifest = json!({
            "schema_version": "0.1",
            "kind": "python_repair_trust_manifest",
            "spec": "RIPR-SPEC-0176",
            "description": "verification receipt fixture",
            "selections": [row],
        });
        let serialized = serde_json::to_string_pretty(&manifest)
            .map_err(|error| format!("manifest serialization: {error}"))?;
        let sha = crate::python_judged_panel_replay::sha256_hex(serialized.as_bytes());
        Ok((manifest, sha))
    }

    /// A complete, well-formed receipt bound to the fixture manifest: a
    /// passed execution over a typed route next to an unchanged movement.
    fn fixture_receipt(manifest_sha: &str) -> Result<Value, String> {
        let (mut manifest, _) = fixture_manifest()?;
        let row = manifest
            .get_mut("selections")
            .and_then(Value::as_array_mut)
            .and_then(|rows| rows.first_mut())
            .ok_or_else(|| "fixture manifest carries no row".to_string())?;
        let row_digest = canonical_selection_digest(object_mut(row, "row")?, TRUST_ATTEMPT)?;
        Ok(json!({
            "schema_version": "0.1",
            "kind": "python_repair_verification_receipt",
            "spec": "RIPR-SPEC-0176",
            "phase": "verify",
            "durable_attempt_id": ATTEMPT,
            "trust_attempt_id": TRUST_ATTEMPT,
            "seam_id": "seam-1",
            "repository_head": GIT_SHA_B,
            "target_path": "tests/test_pricing.py",
            "identities": {
                "packet_sha256": DIGEST_THREE,
                "before_snapshot_sha256": DIGEST_FOUR,
                "patch_sha256": DIGEST_FIVE,
                "selection_manifest_sha256": manifest_sha,
                "selection_digest": row_digest,
                "binding_artifact_sha256": DIGEST_SIX,
                "config_profile": "default",
                "analyzer_binary_sha256": DIGEST_SEVEN,
            },
            "command": {
                "command_spec_sha256": DIGEST_EIGHT,
                "display": "ripr agent verify --root . --before b --after a --json",
                "authorization": {
                    "status": "granted",
                    "authority": "operator-a",
                    "method": "explicit-operator-flags",
                },
            },
            "execution": {
                "state": "passed",
                "process_disposition": "completed",
                "exit_status": 0,
                "exit_signal": null,
                "stdout_sha256": DIGEST_NINE,
                "stderr_sha256": DIGEST_TWO,
                "stdout_bytes": 10,
                "stderr_bytes": 0,
                "stdout_truncated": false,
                "stderr_truncated": false,
                "currentness": "current",
                "duration_ms": 25,
                "cancellation_requested": false,
                "reason": null,
            },
            "movement": {
                "state": "unchanged",
                "reason": null,
                "identity": {
                    "family": "predicate_boundary",
                    "owner": "pricing.calculate_discount",
                    "discriminator": "amount >= threshold",
                    "relation": "test calls owner directly",
                    "oracle": "assert exact boundary value",
                },
                "join": {
                    "before_seam_id": "seam-1",
                    "after_seam_id": "seam-1",
                    "before_grip_class": "weakly_gripped",
                    "after_grip_class": "weakly_gripped",
                    "before_oracle_strength": "weak",
                    "after_oracle_strength": "weak",
                    "before_headline_eligible": true,
                    "after_headline_eligible": true,
                },
                "unrelated": {
                    "actionable_before": 2,
                    "actionable_after": 2,
                    "regressed": false,
                    "improved": false,
                },
                "after_snapshot_sha256": DIGEST_FIVE,
                "after_run_status": "complete",
            },
            "rollback": {
                "state": "not_run",
                "reason": "rollback was not requested on this verification run",
                "post_rollback_head": null,
            },
            "non_claims": VERIFICATION_NON_CLAIMS,
            "claim_boundary": RECEIPT_CLAIM_BOUNDARY,
        }))
    }

    /// Writes manifest + receipt to a temp directory, runs the offline check,
    /// and returns the outcome (cleaning up regardless of the result).
    fn check(mutate: impl FnOnce(&mut Value)) -> Result<VerificationCheckOutcome, String> {
        let (manifest_value, manifest_sha) = fixture_manifest()?;
        let mut record = fixture_receipt(&manifest_sha)?;
        mutate(&mut record);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("clock: {error}"))?
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("ripr-verify-check-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&directory).map_err(|error| format!("create temp dir: {error}"))?;
        let written = (|| -> Result<(), String> {
            let manifest_text = serde_json::to_string_pretty(&manifest_value)
                .map_err(|error| format!("manifest text: {error}"))?;
            std::fs::write(directory.join("manifest.json"), manifest_text)
                .map_err(|error| format!("manifest write: {error}"))?;
            let receipt_text = serde_json::to_string_pretty(&record)
                .map_err(|error| format!("receipt text: {error}"))?;
            std::fs::write(directory.join("receipt.json"), receipt_text)
                .map_err(|error| format!("receipt write: {error}"))?;
            Ok(())
        })();
        let manifest_display = directory.join("manifest.json").display().to_string();
        let receipt_display = directory.join("receipt.json").display().to_string();
        let outcome = written
            .and_then(|()| check_verification_artifacts(&manifest_display, &receipt_display));
        let _ = std::fs::remove_dir_all(&directory);
        outcome
    }

    /// Expects a valid outcome and returns the validated receipt.
    fn valid(mutate: impl FnOnce(&mut Value)) -> Result<VerificationReceipt, String> {
        let outcome = check(mutate)?;
        if outcome.verdict() != "valid" || !outcome.violations.is_empty() {
            return Err(format!(
                "expected a valid outcome, got {:?}: {:?}",
                outcome.verdict(),
                outcome.violations
            ));
        }
        outcome
            .receipts
            .into_iter()
            .next()
            .ok_or_else(|| "no receipt validated".to_string())
    }

    /// Expects an inconsistent outcome and returns the first violation.
    fn violation(mutate: impl FnOnce(&mut Value), needle: &str) -> Result<String, String> {
        let outcome = check(mutate)?;
        if outcome.verdict() != "inconsistent" {
            return Err(format!(
                "expected an inconsistent outcome, got {:?}: {:?}",
                outcome.verdict(),
                outcome.violations
            ));
        }
        let first = outcome
            .violations
            .first()
            .ok_or_else(|| "no violation recorded".to_string())?;
        if !first.contains(needle) {
            return Err(format!("violation does not name `{needle}`: {first}"));
        }
        Ok(first.clone())
    }

    /// Replaces one field in the receipt. A missing parent silently leaves
    /// the record untouched; the assertions below then fail on the outcome,
    /// keeping a broken mutation visible instead of a panic.
    fn set(path: &[&str], value: Value) -> impl FnOnce(&mut Value) {
        let path: Vec<String> = path.iter().map(|part| (*part).to_string()).collect();
        move |record: &mut Value| {
            let mut current = record;
            for part in &path[..path.len() - 1] {
                match current.get_mut(part) {
                    Some(next) => current = next,
                    None => return,
                }
            }
            if let Some(object) = current.as_object_mut() {
                object.insert(path[path.len() - 1].clone(), value);
            }
        }
    }

    fn execution_body(state: &str, disposition: &str, exit_status: Value) -> Value {
        json!({
            "state": state,
            "process_disposition": disposition,
            "exit_status": exit_status,
            "exit_signal": null,
            "stdout_sha256": DIGEST_NINE,
            "stderr_sha256": DIGEST_TWO,
            "stdout_bytes": 1,
            "stderr_bytes": 0,
            "stdout_truncated": false,
            "stderr_truncated": false,
            "currentness": "current",
            "duration_ms": 1,
            "cancellation_requested": false,
            "reason": null,
        })
    }

    // ------------------------------------------------------------------
    // Acceptance: the clean receipt and the issue's representable pairs.
    // ------------------------------------------------------------------

    #[test]
    fn accepts_the_clean_passed_receipt() -> Result<(), String> {
        let receipt = valid(|_record| {})?;
        if receipt.execution_state != "passed"
            || receipt.movement_state != "unchanged"
            || receipt.rollback_state != "not_run"
        {
            return Err(format!("fixture receipt reduced wrong: {receipt:?}"));
        }
        Ok(())
    }

    #[test]
    fn every_execution_state_stays_representable() -> Result<(), String> {
        // A failed command next to improved movement stays representable.
        valid(|record| {
            set(
                &["execution"],
                execution_body("failed", "completed", json!(1)),
            )(record);
            set(&["movement", "state"], json!("improved"))(record);
            set(
                &["movement", "join", "after_oracle_strength"],
                json!("strong"),
            )(record);
        })?;

        valid(|record| {
            set(
                &["execution"],
                execution_body("timed_out", "timed_out", json!(null)),
            )(record);
        })?;

        valid(|record| {
            set(
                &["execution"],
                execution_body("cancelled", "cancelled", json!(null)),
            )(record);
        })?;

        // An unavailable runner next to an uncertain movement (a zero
        // native-identity join) stays representable.
        valid(|record| {
            let mut body = execution_body("unavailable", "failed_to_start", json!(null));
            let Ok(object) = object_mut(&mut body, "execution body") else {
                return;
            };
            object.insert("stdout_sha256".to_string(), Value::Null);
            object.insert("stderr_sha256".to_string(), Value::Null);
            object.insert("stdout_bytes".to_string(), Value::Null);
            object.insert("stderr_bytes".to_string(), Value::Null);
            object.insert("currentness".to_string(), Value::Null);
            object.insert("duration_ms".to_string(), Value::Null);
            object.insert(
                "reason".to_string(),
                json!("spawn failed: program not found"),
            );
            set(&["execution"], body)(record);
            set(&["movement", "state"], json!("uncertain"))(record);
            set(
                &["movement", "reason"],
                json!("the native identity joins zero seams in the before analysis"),
            )(record);
            set(
                &["movement", "join"],
                json!({
                    "before_seam_id": null,
                    "after_seam_id": null,
                    "before_grip_class": null,
                    "after_grip_class": null,
                    "before_oracle_strength": null,
                    "after_oracle_strength": null,
                    "before_headline_eligible": null,
                    "after_headline_eligible": null,
                }),
            )(record);
        })?;

        // A rejected pre-execution is not_run with no route identity and no
        // commitments at all.
        valid(|record| {
            let mut body = execution_body("not_run", "rejected_before_execution", json!(null));
            let Ok(object) = object_mut(&mut body, "execution body") else {
                return;
            };
            object.insert("stdout_sha256".to_string(), Value::Null);
            object.insert("stderr_sha256".to_string(), Value::Null);
            object.insert("stdout_bytes".to_string(), Value::Null);
            object.insert("stderr_bytes".to_string(), Value::Null);
            object.insert("currentness".to_string(), Value::Null);
            object.insert("duration_ms".to_string(), Value::Null);
            object.insert(
                "reason".to_string(),
                json!("the retained packet declares no canonical typed verify route"),
            );
            set(&["execution"], body)(record);
            set(&["command", "command_spec_sha256"], json!(null))(record);
            set(&["command", "display"], json!(null))(record);
        })?;

        // An output-limit run is invalid with a reason but still carries the
        // route identity: the run happened, only its verdict is unobservable.
        valid(|record| {
            let mut body = execution_body("invalid", "output_limit_exceeded", json!(null));
            let Ok(object) = object_mut(&mut body, "execution body") else {
                return;
            };
            object.insert("stdout_truncated".to_string(), json!(true));
            object.insert("stdout_bytes".to_string(), json!(1_048_576));
            object.insert(
                "reason".to_string(),
                json!("output limit exceeded before the route could report"),
            );
            set(&["execution"], body)(record);
        })?;

        Ok(())
    }

    #[test]
    fn the_issue_pair_matrix_stays_representable() -> Result<(), String> {
        // command passed + gap unchanged (the fixture default) is pinned by
        // `accepts_the_clean_passed_receipt`.

        // command failed + static evidence improved.
        valid(|record| {
            set(&["execution", "state"], json!("failed"))(record);
            set(&["execution", "exit_status"], json!(3))(record);
            set(&["movement", "state"], json!("improved"))(record);
            set(
                &["movement", "join", "after_oracle_strength"],
                json!("strong"),
            )(record);
        })?;

        // command unavailable + movement uncertain: the comparison resolved
        // the before join and found no after join, so the receipt carries the
        // complete before join and no after join (the producer shape).
        valid(|record| {
            let mut body = execution_body("unavailable", "route_unavailable", json!(null));
            let Ok(object) = object_mut(&mut body, "execution body") else {
                return;
            };
            object.insert("stdout_sha256".to_string(), Value::Null);
            object.insert("stderr_sha256".to_string(), Value::Null);
            object.insert("stdout_bytes".to_string(), Value::Null);
            object.insert("stderr_bytes".to_string(), Value::Null);
            object.insert("currentness".to_string(), Value::Null);
            object.insert("duration_ms".to_string(), Value::Null);
            object.insert("reason".to_string(), json!("no producer route"));
            set(&["execution"], body)(record);
            set(&["command", "command_spec_sha256"], json!(null))(record);
            set(&["command", "display"], json!(null))(record);
            set(&["movement", "state"], json!("uncertain"))(record);
            set(&["movement", "reason"], json!("stale join"))(record);
            set(&["movement", "join", "after_seam_id"], json!(null))(record);
            set(&["movement", "join", "after_grip_class"], json!(null))(record);
            set(&["movement", "join", "after_oracle_strength"], json!(null))(record);
            set(
                &["movement", "join", "after_headline_eligible"],
                json!(null),
            )(record);
        })?;

        // command passed + wrong target discovered in review: the passed run
        // rides a stale movement without the receipt becoming invalid — the
        // review dimension lives in the corpus, not here. The stale shape
        // carries the moved before seam (a different seam than the receipt
        // binds) and no after join.
        valid(|record| {
            set(&["movement", "state"], json!("stale"))(record);
            set(
                &["movement", "reason"],
                json!("stale join: the native identity now resolves to another seam"),
            )(record);
            set(&["movement", "join", "before_seam_id"], json!("seam-moved"))(record);
            set(&["movement", "join", "after_seam_id"], json!(null))(record);
            set(&["movement", "join", "after_grip_class"], json!(null))(record);
            set(&["movement", "join", "after_oracle_strength"], json!(null))(record);
            set(
                &["movement", "join", "after_headline_eligible"],
                json!(null),
            )(record);
        })?;

        // gap closed + unrelated findings regressed: both stay visible.
        valid(|record| {
            set(&["movement", "state"], json!("closed"))(record);
            set(
                &["movement", "join", "after_headline_eligible"],
                json!(false),
            )(record);
            set(
                &["movement", "unrelated"],
                json!({
                    "actionable_before": 2,
                    "actionable_after": 3,
                    "regressed": true,
                    "improved": false,
                }),
            )(record);
        })?;

        Ok(())
    }

    // ------------------------------------------------------------------
    // Fail-closed rules.
    // ------------------------------------------------------------------

    #[test]
    fn rejects_a_stale_manifest_digest() -> Result<(), String> {
        let (_, real_sha) = fixture_manifest()?;
        let forged = if real_sha.ends_with('0') {
            format!("{}1", &real_sha[..63])
        } else {
            format!("{}0", &real_sha[..63])
        };
        violation(
            set(&["identities", "selection_manifest_sha256"], json!(forged)),
            "stale digest",
        )?;
        Ok(())
    }

    #[test]
    fn rejects_a_replaced_selection_row_digest() -> Result<(), String> {
        violation(
            set(&["identities", "selection_digest"], json!(DIGEST_ONE)),
            "selection row digest mismatch",
        )?;
        Ok(())
    }

    #[test]
    fn rejects_an_unknown_trust_attempt() -> Result<(), String> {
        violation(
            set(&["trust_attempt_id"], json!("att-outside")),
            "outside the accepted selection denominator",
        )?;
        Ok(())
    }

    #[test]
    fn rejects_rewritten_identity_copies() -> Result<(), String> {
        for field in ["family", "owner", "discriminator", "relation", "oracle"] {
            violation(
                set(&["movement", "identity", field], json!("rewritten")),
                "identity disagreement",
            )?;
        }
        violation(
            set(&["target_path"], json!("tests/other_test.py")),
            "target identity disagreement",
        )?;
        Ok(())
    }

    #[test]
    fn rejects_disagreement_between_execution_state_and_disposition() -> Result<(), String> {
        violation(
            set(&["execution", "exit_status"], json!(1)),
            "`passed` requires exit status 0",
        )?;
        violation(
            |record| {
                set(&["execution", "state"], json!("failed"))(record);
                set(&["execution", "exit_status"], json!(0))(record);
            },
            "`failed` requires a non-zero exit status",
        )?;
        violation(
            |record| {
                set(&["execution", "state"], json!("failed"))(record);
                set(&["execution", "exit_status"], json!(null))(record);
            },
            "`failed` requires a non-zero exit status",
        )?;
        violation(
            set(&["execution", "process_disposition"], json!("cancelled")),
            "`passed` requires process_disposition `completed`",
        )?;
        violation(
            |record| {
                set(&["execution", "state"], json!("cancelled"))(record);
                set(&["execution", "process_disposition"], json!("completed"))(record);
            },
            "`cancelled` requires process_disposition `cancelled`",
        )?;
        violation(
            set(&["execution", "state"], json!("bogus")),
            "unknown verification execution",
        )?;
        Ok(())
    }

    #[test]
    fn rejects_a_passed_run_without_output_commitments() -> Result<(), String> {
        violation(
            set(&["execution", "stdout_sha256"], json!(null)),
            "retains both output commitments",
        )?;
        violation(
            set(&["execution", "currentness"], json!(null)),
            "retains its currentness disposition",
        )?;
        violation(
            |record| {
                set(&["command", "command_spec_sha256"], json!(null))(record);
                set(&["command", "display"], json!(null))(record);
            },
            "retains the typed command identity",
        )?;
        Ok(())
    }

    #[test]
    fn rejects_movement_outside_the_closed_vocabulary_and_unreasoned_degradation()
    -> Result<(), String> {
        violation(
            set(&["movement", "state"], json!("unbounded")),
            "unknown static movement",
        )?;
        violation(
            set(&["movement", "state"], json!("uncertain")),
            "retains the typed reason",
        )?;
        violation(
            set(&["movement", "state"], json!("stale")),
            "retains the typed reason",
        )?;
        Ok(())
    }

    #[test]
    fn rejects_a_confident_movement_carrying_a_degradation_reason() -> Result<(), String> {
        violation(
            set(&["movement", "reason"], json!("downgraded")),
            "carries no degradation reason",
        )?;
        Ok(())
    }

    #[test]
    fn rejects_hand_edited_unrelated_flags() -> Result<(), String> {
        violation(
            set(&["movement", "unrelated", "regressed"], json!(true)),
            "claims an unrelated regression",
        )?;
        violation(
            set(&["movement", "unrelated", "improved"], json!(true)),
            "claims unrelated improvement",
        )?;
        violation(
            set(&["movement", "unrelated", "actionable_after"], json!(5)),
            "movement must stay visible",
        )?;
        Ok(())
    }

    #[test]
    fn rejects_incomplete_rollback_dispositions() -> Result<(), String> {
        violation(
            set(
                &["rollback"],
                json!({
                    "state": "proved",
                    "reason": null,
                    "post_rollback_head": null,
                }),
            ),
            "retains the restored head",
        )?;
        violation(
            set(
                &["rollback"],
                json!({
                    "state": "blocked",
                    "reason": null,
                    "post_rollback_head": null,
                }),
            ),
            "retains the typed reason",
        )?;
        violation(
            set(&["rollback", "post_rollback_head"], json!("not-a-sha")),
            "retains no restored head",
        )?;
        violation(
            set(
                &["rollback"],
                json!({
                    "state": "proved",
                    "reason": null,
                    "post_rollback_head": "not-a-sha",
                }),
            ),
            "git identity must be a bare lowercase 40-character commit SHA",
        )?;
        Ok(())
    }

    #[test]
    fn rejects_dropped_extra_or_non_string_non_claims() -> Result<(), String> {
        violation(
            set(&["non_claims"], json!(["only one claim"])),
            "must be exactly the standing non-claims",
        )?;
        let mut with_extra = VERIFICATION_NON_CLAIMS.to_vec();
        with_extra.push("an extra softening claim");
        violation(
            set(&["non_claims"], json!(with_extra)),
            "must be exactly the standing non-claims",
        )?;
        Ok(())
    }

    #[test]
    fn lifecycle_fields_are_denied_so_a_pass_cannot_mark_acceptance() -> Result<(), String> {
        for field in ["states", "lifecycle", "accepted", "reviewed"] {
            violation(set(&[field], json!(["smuggled"])), "unknown field")?;
        }
        Ok(())
    }

    #[test]
    fn malformed_shapes_fail_with_the_field_named() -> Result<(), String> {
        violation(
            set(&["durable_attempt_id"], json!("attempt-1")),
            "durable attempt id must be",
        )?;
        violation(
            set(&["identities", "patch_sha256"], json!("short")),
            "bare lowercase sha256 hex",
        )?;
        violation(
            set(&["schema_version"], json!("9.9")),
            "expected `0.1`, got `9.9`",
        )?;
        violation(
            set(&["phase"], json!("accept")),
            "expected `verify`, got `accept`",
        )?;
        violation(
            set(&["movement", "after_run_status"], json!("partial")),
            "unknown after-analysis run status",
        )?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Fabricated joins, scalar types, signal termination, claim boundary.
    // ------------------------------------------------------------------

    #[test]
    fn requires_the_standing_claim_boundary_verbatim() -> Result<(), String> {
        violation(
            set(&["claim_boundary"], json!("a softened boundary")),
            "expected the standing claim boundary verbatim",
        )?;
        violation(
            set(&["claim_boundary"], json!(null)),
            "a present null is not a value",
        )?;
        Ok(())
    }

    #[test]
    fn rejects_fabricated_or_contradictory_movement_joins() -> Result<(), String> {
        // The review's exact example: a confident state over null joins.
        violation(
            |record| {
                set(&["movement", "state"], json!("closed"))(record);
                set(
                    &["movement", "join"],
                    json!({
                        "before_seam_id": null,
                        "after_seam_id": null,
                        "before_grip_class": null,
                        "after_grip_class": null,
                        "before_oracle_strength": null,
                        "after_oracle_strength": null,
                        "before_headline_eligible": null,
                        "after_headline_eligible": null,
                    }),
                )(record);
            },
            "retains the complete before and after joins",
        )?;
        // A confident state its own headline/oracle evidence contradicts.
        violation(
            set(&["movement", "state"], json!("closed")),
            "contradicts its own joins",
        )?;
        // A before seam the receipt does not bind is `stale`, never confident.
        violation(
            set(
                &["movement", "join", "before_seam_id"],
                json!("seam-elsewhere"),
            ),
            "a different before seam is `stale`",
        )?;
        // A stale join that still names the receipt's own seam is impossible.
        violation(
            |record| {
                set(&["movement", "state"], json!("stale"))(record);
                set(
                    &["movement", "reason"],
                    json!("stale join: the identity moved"),
                )(record);
                set(&["movement", "join", "after_seam_id"], json!(null))(record);
                set(&["movement", "join", "after_grip_class"], json!(null))(record);
                set(&["movement", "join", "after_oracle_strength"], json!(null))(record);
                set(
                    &["movement", "join", "after_headline_eligible"],
                    json!(null),
                )(record);
            },
            "requires the identity to have moved",
        )?;
        // `limited` over a complete analysis is impossible: the producer
        // either resolves the join or degrades to `uncertain`.
        violation(
            |record| {
                set(&["movement", "state"], json!("limited"))(record);
                set(
                    &["movement", "reason"],
                    json!("the after analysis was partial"),
                )(record);
                set(
                    &["movement", "join"],
                    json!({
                        "before_seam_id": null,
                        "after_seam_id": null,
                        "before_grip_class": null,
                        "after_grip_class": null,
                        "before_oracle_strength": null,
                        "after_oracle_strength": null,
                        "before_headline_eligible": null,
                        "after_headline_eligible": null,
                    }),
                )(record);
            },
            "requires a partial after analysis",
        )?;
        // An uncertain movement never retains an after join (the fixture's
        // after join stays present, which the producer cannot record for a
        // degraded state).
        violation(
            |record| {
                set(&["movement", "state"], json!("uncertain"))(record);
                set(
                    &["movement", "reason"],
                    json!("the native identity joins zero seams in the after analysis"),
                )(record);
            },
            "never retains an after join",
        )?;
        // A partial before join is not a producer shape.
        violation(
            |record| {
                set(&["movement", "state"], json!("uncertain"))(record);
                set(
                    &["movement", "reason"],
                    json!("the native identity joins zero seams in the before analysis"),
                )(record);
                set(&["movement", "join", "after_seam_id"], json!(null))(record);
                set(&["movement", "join", "after_grip_class"], json!(null))(record);
                set(&["movement", "join", "after_oracle_strength"], json!(null))(record);
                set(
                    &["movement", "join", "after_headline_eligible"],
                    json!(null),
                )(record);
                set(&["movement", "join", "before_seam_id"], json!("seam-1"))(record);
                set(&["movement", "join", "before_grip_class"], json!(null))(record);
            },
            "a partial before join is not a producer shape",
        )?;
        // The documented `limited` shape itself stays representable.
        valid(|record| {
            set(&["movement", "state"], json!("limited"))(record);
            set(
                &["movement", "reason"],
                json!("the after analysis was partial (run_status `seam_limit_applied`)"),
            )(record);
            set(
                &["movement", "after_run_status"],
                json!("seam_limit_applied"),
            )(record);
            set(
                &["movement", "join"],
                json!({
                    "before_seam_id": null,
                    "after_seam_id": null,
                    "before_grip_class": null,
                    "after_grip_class": null,
                    "before_oracle_strength": null,
                    "after_oracle_strength": null,
                    "before_headline_eligible": null,
                    "after_headline_eligible": null,
                }),
            )(record);
        })?;
        Ok(())
    }

    #[test]
    fn rejects_mistyped_producer_execution_scalars() -> Result<(), String> {
        violation(
            set(&["execution", "exit_signal"], json!({})),
            "field must be an integer when present",
        )?;
        violation(
            set(&["execution", "exit_signal"], json!("6")),
            "field must be an integer when present",
        )?;
        violation(
            set(&["execution", "duration_ms"], json!("fast")),
            "field must be a non-negative integer when present",
        )?;
        violation(
            set(&["execution", "stdout_truncated"], json!("yes")),
            "field must be a boolean when present",
        )?;
        violation(
            set(&["execution", "cancellation_requested"], json!(1)),
            "field must be a boolean when present",
        )?;
        Ok(())
    }

    #[test]
    fn keeps_signal_termination_and_negative_exit_codes_representable() -> Result<(), String> {
        // A completed observation terminated by signal: exit status null, the
        // signal retained, execution state `failed`.
        valid(|record| {
            set(&["execution", "state"], json!("failed"))(record);
            set(&["execution", "exit_status"], json!(null))(record);
            set(&["execution", "exit_signal"], json!(6))(record);
        })?;
        // A Windows access violation is a negative exit code, not a malformed
        // receipt: the signed producer type must validate.
        valid(|record| {
            set(&["execution", "state"], json!("failed"))(record);
            set(&["execution", "exit_status"], json!(-1073741819))(record);
        })?;
        // An exit status paired with a signal is not a producer shape.
        violation(
            |record| {
                set(&["execution", "state"], json!("failed"))(record);
                set(&["execution", "exit_status"], json!(1))(record);
                set(&["execution", "exit_signal"], json!(6))(record);
            },
            "`failed` requires a non-zero exit status or a retained termination signal",
        )?;
        // A clean exit paired with a signal is not a producer shape.
        violation(
            set(&["execution", "exit_signal"], json!(6)),
            "`passed` is a clean exit and carries no termination signal",
        )?;
        // A timed-out run carries no signal (the rails drop it off the
        // completed disposition only).
        violation(
            |record| {
                set(
                    &["execution"],
                    execution_body("timed_out", "timed_out", json!(null)),
                )(record);
                set(&["execution", "exit_signal"], json!(9))(record);
            },
            "`timed_out` carries no termination signal",
        )?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Verdicts.
    // ------------------------------------------------------------------

    #[test]
    fn not_run_is_never_a_pass() -> Result<(), String> {
        let outcome = VerificationCheckOutcome {
            manifest_path: "manifest.json".to_string(),
            manifest: None,
            receipts_input: None,
            receipts: Vec::new(),
            violations: Vec::new(),
        };
        if outcome.verdict() != "not_run" {
            return Err("an empty input must be not_run".to_string());
        }
        Ok(())
    }

    #[test]
    fn invalid_only_receipts_fail_closed_as_inconsistent() -> Result<(), String> {
        let outcome = check(|record| {
            set(&["execution", "exit_status"], json!(7))(record);
        })?;
        if outcome.verdict() != "inconsistent" || outcome.violations.is_empty() {
            return Err(format!(
                "all-invalid receipts must fail closed, got {:?}",
                outcome.verdict()
            ));
        }
        Ok(())
    }
}
