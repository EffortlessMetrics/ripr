//! Content-addressed publication of the accepted receipt (RIPR-SPEC-0086).
//! Every accepted aggregate is derived from the validated rows — never a
//! hand-entered total, and every count carries its denominator — the pointer
//! binds identity only, and every accepted byte reaches its final path
//! through the staged atomic pattern, so an interrupted run can never leave
//! truncated bytes under an accepted-artifact name. The self-addressed
//! artifacts (receipt, retained candidate) repair bytes that do not hash to
//! their own digest address; no other accepted artifact is ever rewritten.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::candidate::{
    CandidateRowFacts, Disposition, check_artifact_hygiene, load_dispositions, read_candidate_rows,
    require_disposition_coverage,
};
use super::render::render_accepted_markdown;
use super::{
    CONTRACT_VERSION, NOTE_MAX_CHARS, POINTER_FILE, POINTER_KIND, POINTER_RIPR_KEYS,
    POINTER_SCHEMA, RECEIPTS_DIR, RERUN_COMMAND, ReportArgs, SPEC, TIER, USAGE, fail,
};
use crate::reports::eval_sweep_check::{
    AcceptedManifest, load_strict_json, sha256_hex, validate_accepted_manifest,
    validate_run_receipt,
};

const ACCEPTED_SCHEMA: &str = "0.1";
const ACCEPTED_KIND: &str = "python_eval_sweep_accepted_receipt";
pub(super) const REPORT_JSON: &str = "eval-sweep-report.json";
pub(super) const REPORT_MD: &str = "eval-sweep-report.md";
/// The candidate schema acceptance binds currentness identities from. A 0.2
/// (historical) candidate carries none and is refused — historical receipts
/// remain valid retained artifacts, immutable and separately addressable.
const CANDIDATE_SCHEMA: &str = "0.3";
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
pub(super) fn build_accepted_receipt(
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
// Candidate report / accept
// ---------------------------------------------------------------------------

pub(super) fn run_candidate_report(parsed: &ReportArgs) -> Result<(), String> {
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
pub(super) fn revalidate_candidate_bytes(
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
pub(super) fn write_pointer_atomically(state_dir: &Path, pointer_text: &str) -> Result<(), String> {
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
