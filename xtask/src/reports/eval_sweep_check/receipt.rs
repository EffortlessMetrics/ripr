//! Retained run-receipt validation for schemas 0.2 and 0.3 (RIPR-SPEC-0086).
//! One validator owns receipt semantics — `validate_run_receipt` is shared
//! by check, refresh (which self-validates its produced candidate through
//! this exact validator), and report — and never rewrites or upgrades a
//! historical receipt. Fail closed on: mixed or unknown state vocabularies,
//! wrong-typed owned fields, malformed or stale digests, a changed
//! denominator, disagreeing duplicate identity copies, and contradictory
//! execution state; absent identities are disclosed `incomplete`, never
//! invented. Validated rows are reduced to `RowSummary` facts for the
//! aggregate layer.

use std::collections::BTreeMap;

use serde_json::Value;

use super::aggregate::{RowSummary, derive_denominator, validate_summary_agreement};
use super::manifest::{AcceptedManifest, AcceptedSubject, KNOWN_SHAPES};
use super::{
    ALIGNMENT_VOCABULARY, CLASSIFICATION_VOCABULARY, Diagnostic, KNOWN_SPEC, KNOWN_TIER, Verdict,
    as_object, check_git_sha, check_no_secrets, check_portable_path, check_sha256_digest,
    check_subject_url, fail, known_value_or_fail, opt_bool, opt_distribution, opt_string,
    opt_string_allow_empty, opt_string_array, opt_u64, reject_unknown_keys,
};

const REPORT_KIND: &str = "python_eval_sweep_report";
/// Receipt schemas this loader owns: `0.2` is the historical shape the sweep
/// command writes; `0.3` adds the currentness identities.
pub(super) const RECEIPT_SCHEMA_0_2: &str = "0.2";
pub(super) const RECEIPT_SCHEMA_0_3: &str = "0.3";
/// The complete 0.3 run-status vocabulary (issue #3565). A row outside this
/// set fails; every member stays selected in the denominator.
pub(super) const STATUS_VOCABULARY: [&str; 8] = [
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
// ---------------------------------------------------------------------------
// Retained run receipts
// ---------------------------------------------------------------------------

pub(super) const SUMMARY_KEYS: [&str; 20] = [
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
pub(super) fn validate_row(
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
