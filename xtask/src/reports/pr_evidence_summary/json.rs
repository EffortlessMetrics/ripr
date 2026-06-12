use crate::reports::pr_evidence_summary::model::{
    GapCounts, LimitationEntry, NullableU64, PrEvidenceSummaryJson, ReceiptStatusCounts,
    TopLimitation, TopRepair, U64OrNotAvailable,
};
use crate::reports::pr_evidence_summary::util::value_path;
use serde_json::{Value, json};

/// Build the in-memory summary struct from parsed artifact values.
///
/// - `start_here_value`      — parsed start-here.json, or None.
/// - `gap_ledger_value`      — parsed gap-decision-ledger.json, or None.
/// - `repo_exposure_value`   — parsed repo-exposure.json, or None.
/// - `diff_report_value`     — parsed diff-report.json, or None.
/// - `baseline_value`        — parsed baseline snapshot, or None (no --baseline).
/// - `attempt_ledger_value`  — parsed swarm-attempt-ledger.json, or None.
///   When present, `verify_failed_receipts` is derived from its
///   `attempts[].verify_result` field. When absent, `verify_failed_receipts`
///   stays `not_available` (honest-absent rule: absence ≠ zero).
pub(super) fn build_pr_evidence_summary(
    start_here_value: Option<&Value>,
    gap_ledger_value: Option<&Value>,
    repo_exposure_value: Option<&Value>,
    diff_report_value: Option<&Value>,
    baseline_value: Option<&Value>,
    attempt_ledger_value: Option<&Value>,
) -> PrEvidenceSummaryJson {
    let run_status = derive_run_status(diff_report_value, repo_exposure_value);
    let changed_surfaces = derive_changed_surfaces(diff_report_value);
    let gaps = derive_gaps(gap_ledger_value, baseline_value);
    let limitations = derive_limitations(repo_exposure_value);
    let missing_receipts = derive_missing_receipts(gap_ledger_value);
    let receipt_status =
        derive_receipt_status(gap_ledger_value, &missing_receipts, attempt_ledger_value);
    let (top_repair, top_repair_state) = derive_top_repair(start_here_value);
    let top_limitation = limitations.first().map(|entry| TopLimitation {
        category: entry.category.clone(),
        repair_route: entry.repair_route.clone(),
        why_not_actionable: why_not_actionable_for_category(&entry.category),
    });
    let local_reproduction_commands =
        derive_local_reproduction_commands(start_here_value, diff_report_value);

    PrEvidenceSummaryJson {
        run_status,
        changed_surfaces,
        gaps,
        limitations,
        missing_receipts,
        receipt_status,
        top_repair,
        top_repair_state,
        top_limitation,
        local_reproduction_commands,
    }
}

fn derive_run_status(
    diff_report_value: Option<&Value>,
    repo_exposure_value: Option<&Value>,
) -> String {
    // Prefer diff-report run_status (top-level field per DiffReport struct).
    if let Some(status) = value_path(diff_report_value, &["run_status"])
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return status.to_string();
    }
    // Fall back to repo-exposure run_status.
    if let Some(status) = value_path(repo_exposure_value, &["run_status"])
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return status.to_string();
    }
    "unknown".to_string()
}

fn derive_changed_surfaces(diff_report_value: Option<&Value>) -> U64OrNotAvailable {
    // DiffReport.summary.changed_files
    if let Some(n) =
        value_path(diff_report_value, &["summary", "changed_files"]).and_then(Value::as_u64)
    {
        return U64OrNotAvailable::Value(n);
    }
    U64OrNotAvailable::NotAvailable
}

fn derive_gaps(gap_ledger_value: Option<&Value>, baseline_value: Option<&Value>) -> GapCounts {
    let total_actionable = value_path(gap_ledger_value, &["summary", "repairable_total"])
        .and_then(Value::as_u64)
        .map(U64OrNotAvailable::Value)
        .unwrap_or(U64OrNotAvailable::NotAvailable);

    let total_static_limitation =
        value_path(gap_ledger_value, &["summary", "static_limitation_total"])
            .and_then(Value::as_u64)
            .map(U64OrNotAvailable::Value)
            .unwrap_or(U64OrNotAvailable::NotAvailable);

    // Delta fields require a baseline. Honest-baseline rule: never fake zeros.
    let (new_actionable, resolved, regressed, gap_delta_note) = if baseline_value.is_some() {
        // When baseline is present compute simple deltas.
        let before_actionable = value_path(baseline_value, &["gaps", "total_actionable"])
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let after_actionable = match &total_actionable {
            U64OrNotAvailable::Value(n) => *n,
            U64OrNotAvailable::NotAvailable => 0,
        };
        let new_a = after_actionable.saturating_sub(before_actionable);
        let resolved_a = before_actionable.saturating_sub(after_actionable);
        (
            NullableU64::Value(new_a),
            NullableU64::Value(resolved_a),
            NullableU64::Value(0),
            None,
        )
    } else {
        (
            NullableU64::Null,
            NullableU64::Null,
            NullableU64::Null,
            Some(
                "no baseline snapshot provided; pass --baseline <before.json> for delta counts"
                    .to_string(),
            ),
        )
    };

    GapCounts {
        total_actionable,
        total_static_limitation,
        new_actionable,
        resolved,
        regressed,
        gap_delta_note,
    }
}

fn derive_limitations(repo_exposure_value: Option<&Value>) -> Vec<LimitationEntry> {
    let Some(limitations) =
        value_path(repo_exposure_value, &["limitations"]).and_then(Value::as_array)
    else {
        return Vec::new();
    };
    limitations
        .iter()
        .filter_map(|entry| {
            let category = entry.get("category")?.as_str()?.to_string();
            let repair_route = entry
                .get("repair_route")
                .and_then(Value::as_str)
                .unwrap_or("not_available")
                .to_string();
            Some(LimitationEntry {
                category,
                repair_route,
            })
        })
        .collect()
}

fn derive_missing_receipts(gap_ledger_value: Option<&Value>) -> U64OrNotAvailable {
    let Some(summary) = value_path(gap_ledger_value, &["summary"]) else {
        return U64OrNotAvailable::NotAvailable;
    };
    // Prefer an explicit receipt_missing_total field if it ever appears.
    if let Some(n) = summary.get("receipt_missing_total").and_then(Value::as_u64) {
        return U64OrNotAvailable::Value(n);
    }
    // Derive: repairable_total minus receipt_improved_total.
    let repairable = summary
        .get("repairable_total")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let improved = summary
        .get("receipt_improved_total")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    U64OrNotAvailable::Value(repairable.saturating_sub(improved))
}

/// Return true when a `verify_result` string value indicates a real verify
/// failure that is counted by this summary.
///
/// Accepted failure tokens come from two producers:
/// - `swarm_ingest.rs` (`verify.status:"failed"`, `exit_code != 0`) → outcome
///   ledger `verify_result:"fail"` (or the verbatim string `"failed"` when the
///   originating value was the status field itself).
/// - The real-repair-attempts corpus, which uses `"error"` for unexpected
///   non-zero exit codes.
///
/// The full token set mirrors what `ripr-swarm attempt-ledger` tolerates in its
/// own `missing_verify_result` detection logic.
fn is_verify_failure_result(v: &str) -> bool {
    matches!(v, "fail" | "failed" | "error")
}

/// Derive `verify_failed_receipts` from the attempt-ledger artifact.
///
/// Returns `NotAvailable` when the artifact is absent — absence means the
/// ledger was never inspected, so a `0` would be a fake zero. Returns
/// `Value(n)` — including `Value(0)` — when the ledger IS present, because
/// then we actually read every entry and a genuinely-failed entry would have
/// been counted (the non-zero is validated and reachable by the test
/// `verify_failed_receipts_nonzero_when_attempt_ledger_has_real_failure`).
///
/// Only the top-level `attempts[]` array is scanned (current-run entries,
/// not the `latest_attempts[]` view). `verify_result` ∈
/// `{"fail", "failed", "error"}` counts as a failure.
fn count_verify_failed_from_attempt_ledger(
    attempt_ledger_value: Option<&Value>,
) -> U64OrNotAvailable {
    let Some(ledger) = attempt_ledger_value else {
        // Absent ledger → not_available (honest-absent rule).
        return U64OrNotAvailable::NotAvailable;
    };
    let attempts = match ledger.get("attempts").and_then(Value::as_array) {
        Some(arr) => arr,
        // Ledger present but `attempts` key absent or not an array → 0 failures
        // found (we did inspect the artifact and found nothing to count).
        None => return U64OrNotAvailable::Value(0),
    };
    let count = attempts
        .iter()
        .filter(|entry| {
            entry
                .get("verify_result")
                .and_then(Value::as_str)
                .is_some_and(is_verify_failure_result)
        })
        .count();
    U64OrNotAvailable::Value(count as u64)
}

/// Derive the six-count receipt-status object.
///
/// `receipts_present` and `missing_receipts` are derivable from existing
/// gap-ledger summary counts. Three counts stay `NotAvailable` because their
/// producers are not available here:
///
/// - `orphan_receipts`: requires a sweep of `target/ripr/receipts/` vs. ledger
///   records to find files that no record references. No filesystem scan is
///   performed during summary derivation.
///   Unlock: add a receipts/ dir sweep to the ledger build path.
/// - `stale_receipts`: the genuine staleness signal lives in `swarm_ingest`
///   (`staleness_status`), which the gap-ledger build does not consume.
///   Emitting `0` would be a fake zero — no production path produces a
///   non-zero today (see #1130 adversarial review).
///   Unlock: wire `swarm_ingest.staleness_status` into the gap-ledger build.
/// - `gap_mismatch_receipts`: requires reading each receipt file to compare
///   its recorded `canonical_gap_id` against the attached gap record.
///   The ledger ingest does not surface the receipt's own gap id field.
///   Unlock: read each receipt's own `canonical_gap_id` in the ledger build.
///
/// `verify_failed_receipts` is NOW derivable when `attempt_ledger_value` is
/// `Some(_)` — it is counted from `attempts[].verify_result` ∈
/// `{"fail", "failed", "error"}`, which flows from the real
/// `swarm_ingest.verify.status/exit_code` pipeline through
/// `actionable-gap-outcomes.json` into the attempt ledger. When the attempt
/// ledger is absent the field stays `not_available` (honest-absent rule).
fn derive_receipt_status(
    gap_ledger_value: Option<&Value>,
    missing_receipts: &U64OrNotAvailable,
    attempt_ledger_value: Option<&Value>,
) -> ReceiptStatusCounts {
    // receipts_present = receipt_improved_total + receipt_unchanged_after_attempt_total.
    // Both are advisory counts; if the summary block is absent we emit NotAvailable.
    let receipts_present = if let Some(summary) = value_path(gap_ledger_value, &["summary"]) {
        let improved = summary
            .get("receipt_improved_total")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let unchanged = summary
            .get("receipt_unchanged_after_attempt_total")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        // If neither field is present at all, we have no evidence — NotAvailable.
        // Both default to 0 when the key is absent but the summary block exists,
        // so we check whether the block itself was present (which it was if we
        // reached this branch).
        U64OrNotAvailable::Value(improved.saturating_add(unchanged))
    } else {
        U64OrNotAvailable::NotAvailable
    };

    // missing_receipts mirrors the already-computed top-level field.
    let missing = match missing_receipts {
        U64OrNotAvailable::Value(n) => U64OrNotAvailable::Value(*n),
        U64OrNotAvailable::NotAvailable => U64OrNotAvailable::NotAvailable,
    };

    ReceiptStatusCounts {
        receipts_present,
        missing_receipts: missing,
        // NOT DERIVABLE from this path: requires a receipts/ dir sweep vs.
        // ledger records. Unlock: add the sweep to the ledger build.
        orphan_receipts: U64OrNotAvailable::NotAvailable,
        // NOT DERIVABLE: the real staleness signal lives in swarm_ingest
        // (staleness_status), which the gap-ledger build does not consume.
        // Emitting 0 would be a fake zero — no production producer exists here.
        // Unlock: wire swarm_ingest.staleness_status into the gap-ledger build.
        stale_receipts: U64OrNotAvailable::NotAvailable,
        // NOT DERIVABLE from this path: requires reading each receipt file to
        // compare its own canonical_gap_id against the attached gap record.
        // Unlock: read each receipt's own canonical_gap_id in the ledger build.
        gap_mismatch_receipts: U64OrNotAvailable::NotAvailable,
        // DERIVABLE from the attempt ledger: count attempts[].verify_result ∈
        // {"fail", "failed", "error"}, which flows from the real
        // swarm_ingest.verify.status/exit_code pipeline. When the attempt
        // ledger is absent, stay not_available (honest-absent rule — absence
        // is NOT zero).
        verify_failed_receipts: count_verify_failed_from_attempt_ledger(attempt_ledger_value),
    }
}

fn derive_top_repair(start_here_value: Option<&Value>) -> (Option<TopRepair>, Option<String>) {
    let Some(v) = start_here_value else {
        return (None, Some("missing_artifact".to_string()));
    };
    let state = value_path(Some(v), &["selected", "state"])
        .and_then(Value::as_str)
        .unwrap_or("not_available");

    if state != "top_gap" {
        let top_repair_state = match state {
            "no_action" | "empty_diff" => "no_actionable_gap".to_string(),
            "missing_artifact" => "missing_artifact".to_string(),
            _ => state.to_string(),
        };
        return (None, Some(top_repair_state));
    }

    let sel = v.get("selected");

    let canonical_gap_id = value_path(sel, &["canonical_gap_id"])
        .or_else(|| value_path(sel, &["gap_id"]))
        .and_then(Value::as_str)
        .unwrap_or("not_available")
        .to_string();

    let language = value_path(sel, &["language"])
        .and_then(Value::as_str)
        .unwrap_or("not_available")
        .to_string();

    let repair_kind = value_path(sel, &["repair", "route"])
        .and_then(Value::as_str)
        .unwrap_or("not_available")
        .to_string();

    let target = value_path(sel, &["repair", "target_file"])
        .and_then(Value::as_str)
        .unwrap_or("not_available")
        .to_string();

    let verify_command = value_path(sel, &["verify_command"])
        .and_then(Value::as_str)
        .unwrap_or("not_available")
        .to_string();

    let receipt_command = value_path(sel, &["receipt_command"])
        .and_then(Value::as_str)
        .unwrap_or("not_available")
        .to_string();

    let receipt_state = value_path(sel, &["receipt_state"])
        .and_then(Value::as_str)
        .unwrap_or("receipt_missing")
        .to_string();

    (
        Some(TopRepair {
            canonical_gap_id,
            language,
            repair_kind,
            target,
            verify_command,
            receipt_command,
            receipt_state,
        }),
        None,
    )
}

fn derive_local_reproduction_commands(
    start_here_value: Option<&Value>,
    diff_report_value: Option<&Value>,
) -> Vec<String> {
    let mut commands = Vec::new();

    let base = value_path(diff_report_value, &["base"])
        .and_then(Value::as_str)
        .unwrap_or("origin/main");
    let head = value_path(diff_report_value, &["head"])
        .and_then(Value::as_str)
        .unwrap_or("HEAD");

    commands.push(format!("ripr check --base {base}"));
    commands.push(format!(
        "ripr first-pr --root . --base {base} --head {head}"
    ));

    // Add the verify_command from the top repair when it is a real command.
    let verify = value_path(start_here_value, &["selected", "verify_command"])
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && *s != "not_available");
    if let Some(verify) = verify {
        commands.push(verify.to_string());
    }

    commands
}

fn why_not_actionable_for_category(category: &str) -> String {
    match category {
        "repo_seam_limit_applied" => {
            "Seam inventory was capped; not all seams were analyzed in this run.".to_string()
        }
        "full_repo_context_not_run" => {
            "Full-repo context was not run; diff-scoped analysis only.".to_string()
        }
        "python_preview" => {
            "Python evidence is advisory preview only; no gate or badge authority.".to_string()
        }
        "perl_preview" => {
            "Perl evidence is advisory preview only; no gate or badge authority.".to_string()
        }
        "typescript_preview" => {
            "TypeScript evidence is advisory preview only; no gate or badge authority.".to_string()
        }
        "static_limitation_visibility_unknown" => {
            "Visibility of changed text could not be statically determined.".to_string()
        }
        other => format!("Static analyzer limitation: {other}."),
    }
}

// ── JSON serialization ────────────────────────────────────────────────────────

fn u64_or_not_available(v: &U64OrNotAvailable) -> Value {
    match v {
        U64OrNotAvailable::Value(n) => json!(*n),
        U64OrNotAvailable::NotAvailable => json!("not_available"),
    }
}

fn nullable_u64(v: &NullableU64) -> Value {
    match v {
        NullableU64::Value(n) => json!(*n),
        NullableU64::Null => Value::Null,
    }
}

/// Render the in-memory summary as a versioned JSON string.
pub(super) fn render_pr_evidence_summary_json(s: &PrEvidenceSummaryJson) -> String {
    let top_repair = match &s.top_repair {
        Some(r) => json!({
            "canonical_gap_id": r.canonical_gap_id,
            "language": r.language,
            "repair_kind": r.repair_kind,
            "target": r.target,
            "verify_command": r.verify_command,
            "receipt_command": r.receipt_command,
            "receipt_state": r.receipt_state
        }),
        None => Value::Null,
    };

    let top_limitation = match &s.top_limitation {
        Some(l) => json!({
            "category": l.category,
            "repair_route": l.repair_route,
            "why_not_actionable": l.why_not_actionable
        }),
        None => Value::Null,
    };

    let limitations: Vec<Value> = s
        .limitations
        .iter()
        .map(|l| {
            json!({
                "category": l.category,
                "repair_route": l.repair_route
            })
        })
        .collect();

    let mut gaps = json!({
        "total_actionable": u64_or_not_available(&s.gaps.total_actionable),
        "total_static_limitation": u64_or_not_available(&s.gaps.total_static_limitation),
        "new_actionable": nullable_u64(&s.gaps.new_actionable),
        "resolved": nullable_u64(&s.gaps.resolved),
        "regressed": nullable_u64(&s.gaps.regressed)
    });
    if let Some(note) = &s.gaps.gap_delta_note {
        gaps["gap_delta_note"] = json!(note);
    }

    let receipt_status = json!({
        "receipts_present": u64_or_not_available(&s.receipt_status.receipts_present),
        "missing_receipts": u64_or_not_available(&s.receipt_status.missing_receipts),
        "orphan_receipts": u64_or_not_available(&s.receipt_status.orphan_receipts),
        "stale_receipts": u64_or_not_available(&s.receipt_status.stale_receipts),
        "gap_mismatch_receipts": u64_or_not_available(&s.receipt_status.gap_mismatch_receipts),
        "verify_failed_receipts": u64_or_not_available(&s.receipt_status.verify_failed_receipts)
    });

    let mut obj = json!({
        "schema_version": "0.1",
        "kind": "pr_evidence_summary",
        "tool": "ripr",
        "run_status": s.run_status,
        "changed_surfaces": u64_or_not_available(&s.changed_surfaces),
        "gaps": gaps,
        "limitations": limitations,
        "missing_receipts": u64_or_not_available(&s.missing_receipts),
        "receipt_status": receipt_status,
        "local_reproduction_commands": s.local_reproduction_commands
    });

    // top_repair and top_repair_state: only include the non-null one.
    if s.top_repair.is_some() {
        obj["top_repair"] = top_repair;
        // omit top_repair_state
    } else {
        obj["top_repair"] = Value::Null;
        if let Some(state) = &s.top_repair_state {
            obj["top_repair_state"] = json!(state);
        }
    }

    if !matches!(top_limitation, Value::Null) {
        obj["top_limitation"] = top_limitation;
    }

    match serde_json::to_string_pretty(&obj) {
        Ok(mut json) => {
            json.push('\n');
            json
        }
        Err(_) => "{}\n".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn missing_all() -> PrEvidenceSummaryJson {
        build_pr_evidence_summary(None, None, None, None, None, None)
    }

    #[test]
    fn missing_all_artifacts_yields_unknown_run_status() {
        let s = missing_all();
        assert_eq!(s.run_status, "unknown");
    }

    #[test]
    fn missing_all_artifacts_changed_surfaces_not_available() {
        let s = missing_all();
        assert!(matches!(
            s.changed_surfaces,
            U64OrNotAvailable::NotAvailable
        ));
    }

    #[test]
    fn missing_all_artifacts_gaps_are_not_available_or_null() {
        let s = missing_all();
        assert!(matches!(
            s.gaps.total_actionable,
            U64OrNotAvailable::NotAvailable
        ));
        assert!(matches!(
            s.gaps.total_static_limitation,
            U64OrNotAvailable::NotAvailable
        ));
        assert!(matches!(s.gaps.new_actionable, NullableU64::Null));
        assert!(matches!(s.gaps.resolved, NullableU64::Null));
        assert!(matches!(s.gaps.regressed, NullableU64::Null));
        assert!(s.gaps.gap_delta_note.is_some());
    }

    #[test]
    fn missing_all_artifacts_top_repair_state_is_missing_artifact() {
        let s = missing_all();
        assert!(s.top_repair.is_none());
        assert_eq!(s.top_repair_state.as_deref(), Some("missing_artifact"));
    }

    #[test]
    fn missing_all_artifacts_json_shows_explicit_not_available_not_zeros() {
        let s = missing_all();
        let json = render_pr_evidence_summary_json(&s);
        assert!(json.contains("\"run_status\": \"unknown\""), "{json}");
        assert!(
            json.contains("\"changed_surfaces\": \"not_available\""),
            "{json}"
        );
        assert!(
            json.contains("\"total_actionable\": \"not_available\""),
            "{json}"
        );
        assert!(json.contains("\"new_actionable\": null"), "{json}");
        assert!(json.contains("no baseline snapshot provided"), "{json}");
        assert!(json.contains("\"missing_artifact\""), "{json}");
    }

    #[test]
    fn present_top_gap_populates_top_repair() -> Result<(), String> {
        let start_here = serde_json::json!({
            "status": "actionable",
            "selected": {
                "state": "top_gap",
                "canonical_gap_id": "gap:rust:pricing:discount:threshold-boundary",
                "language": "rust",
                "repair": {
                    "route": "AddBoundaryAssertion",
                    "target_file": "tests/pricing.rs"
                },
                "verify_command": "cargo xtask fixtures boundary_gap",
                "receipt_command": "ripr agent receipt --gap gap:pr:pricing",
                "receipt_state": "receipt_missing"
            }
        });
        let diff = serde_json::json!({
            "run_status": "diff_complete_full_repo_limited",
            "base": "origin/main",
            "head": "HEAD",
            "summary": {
                "changed_files": 3
            }
        });
        let s = build_pr_evidence_summary(Some(&start_here), None, None, Some(&diff), None, None);

        assert_eq!(s.run_status, "diff_complete_full_repo_limited");
        assert!(matches!(s.changed_surfaces, U64OrNotAvailable::Value(3)));
        let repair = match s.top_repair.as_ref() {
            Some(r) => r,
            None => return Err("top_repair must be present".to_string()),
        };
        assert_eq!(
            repair.canonical_gap_id,
            "gap:rust:pricing:discount:threshold-boundary"
        );
        assert_eq!(repair.language, "rust");
        assert_eq!(repair.repair_kind, "AddBoundaryAssertion");
        assert_eq!(repair.target, "tests/pricing.rs");
        assert_eq!(repair.verify_command, "cargo xtask fixtures boundary_gap");
        assert_eq!(
            repair.receipt_command,
            "ripr agent receipt --gap gap:pr:pricing"
        );
        assert_eq!(repair.receipt_state, "receipt_missing");
        assert!(s.top_repair_state.is_none());
        Ok(())
    }

    #[test]
    fn present_top_gap_json_is_copyable() {
        let start_here = serde_json::json!({
            "selected": {
                "state": "top_gap",
                "canonical_gap_id": "gap:rust:src:fn:boundary",
                "language": "rust",
                "repair": {
                    "route": "AddBoundaryAssertion",
                    "target_file": "tests/lib.rs"
                },
                "verify_command": "cargo test boundary",
                "receipt_command": "ripr agent receipt --gap gap:pr:src:fn",
                "receipt_state": "receipt_missing"
            }
        });
        let s = build_pr_evidence_summary(Some(&start_here), None, None, None, None, None);
        let json = render_pr_evidence_summary_json(&s);
        assert!(
            json.contains("\"canonical_gap_id\": \"gap:rust:src:fn:boundary\""),
            "{json}"
        );
        assert!(
            json.contains("\"verify_command\": \"cargo test boundary\""),
            "{json}"
        );
        assert!(
            json.contains("\"receipt_state\": \"receipt_missing\""),
            "{json}"
        );
        // top_repair_state must be absent when top_repair is present.
        assert!(!json.contains("\"top_repair_state\""), "{json}");
    }

    #[test]
    fn gap_ledger_counts_are_surfaced() {
        let ledger = serde_json::json!({
            "summary": {
                "repairable_total": 4,
                "static_limitation_total": 2,
                "receipt_improved_total": 1
            }
        });
        let s = build_pr_evidence_summary(None, Some(&ledger), None, None, None, None);
        assert!(matches!(
            s.gaps.total_actionable,
            U64OrNotAvailable::Value(4)
        ));
        assert!(matches!(
            s.gaps.total_static_limitation,
            U64OrNotAvailable::Value(2)
        ));
        // missing_receipts = repairable_total - receipt_improved_total = 3
        assert!(matches!(s.missing_receipts, U64OrNotAvailable::Value(3)));
    }

    #[test]
    fn repo_exposure_limitations_are_aggregated() -> Result<(), String> {
        let repo = serde_json::json!({
            "run_status": "seam_limit_applied",
            "limitations": [
                {
                    "category": "repo_seam_limit_applied",
                    "repair_route": "Set RIPR_REPO_EXPOSURE_SEAM_LIMIT=0"
                }
            ]
        });
        let s = build_pr_evidence_summary(None, None, Some(&repo), None, None, None);
        assert_eq!(s.run_status, "seam_limit_applied");
        assert_eq!(s.limitations.len(), 1);
        assert_eq!(s.limitations[0].category, "repo_seam_limit_applied");
        let top_lim = match s.top_limitation.as_ref() {
            Some(l) => l,
            None => return Err("top_limitation must be present".to_string()),
        };
        assert_eq!(top_lim.category, "repo_seam_limit_applied");
        assert!(
            top_lim.why_not_actionable.contains("capped"),
            "{}",
            top_lim.why_not_actionable
        );
        Ok(())
    }

    // ── receipt_status six-count tests ───────────────────────────────────────

    /// Without any artifacts, receipt_status fields should be not_available —
    /// never fake zeros.
    #[test]
    fn receipt_status_missing_all_artifacts_is_not_available() {
        let s = missing_all();
        assert!(
            matches!(
                s.receipt_status.receipts_present,
                U64OrNotAvailable::NotAvailable
            ),
            "receipts_present must be not_available when ledger is missing"
        );
        assert!(
            matches!(
                s.receipt_status.missing_receipts,
                U64OrNotAvailable::NotAvailable
            ),
            "receipt_status.missing_receipts must be not_available when ledger is missing"
        );
        // The four not-yet-derivable fields are always not_available.
        assert!(
            matches!(
                s.receipt_status.orphan_receipts,
                U64OrNotAvailable::NotAvailable
            ),
            "orphan_receipts must be not_available (not 0)"
        );
        assert!(
            matches!(
                s.receipt_status.stale_receipts,
                U64OrNotAvailable::NotAvailable
            ),
            "stale_receipts must be not_available (not 0)"
        );
        assert!(
            matches!(
                s.receipt_status.gap_mismatch_receipts,
                U64OrNotAvailable::NotAvailable
            ),
            "gap_mismatch_receipts must be not_available (not 0)"
        );
        assert!(
            matches!(
                s.receipt_status.verify_failed_receipts,
                U64OrNotAvailable::NotAvailable
            ),
            "verify_failed_receipts must be not_available (not 0)"
        );
    }

    /// JSON output must contain a receipt_status object with the four
    /// not-yet-derivable fields set to "not_available", never "0".
    /// All four stay not_available even when a ledger summary IS present,
    /// because the gap-ledger build has no real producer for them
    /// (see #1130 adversarial review — emitting 0 would be a fake zero).
    #[test]
    fn receipt_status_json_not_derivable_fields_are_not_available_not_zero() {
        let s = missing_all();
        let json = render_pr_evidence_summary_json(&s);
        assert!(
            json.contains("\"receipt_status\""),
            "receipt_status must be present in JSON: {json}"
        );
        assert!(
            json.contains("\"orphan_receipts\": \"not_available\""),
            "orphan_receipts must be not_available in JSON: {json}"
        );
        assert!(
            json.contains("\"stale_receipts\": \"not_available\""),
            "stale_receipts must be not_available in JSON: {json}"
        );
        assert!(
            json.contains("\"gap_mismatch_receipts\": \"not_available\""),
            "gap_mismatch_receipts must be not_available in JSON: {json}"
        );
        assert!(
            json.contains("\"verify_failed_receipts\": \"not_available\""),
            "verify_failed_receipts must be not_available in JSON: {json}"
        );
        // Confirm none of the four appear as a numeric 0.
        // (If they were 0, json would contain e.g. `"orphan_receipts": 0`.)
        assert!(
            !json.contains("\"orphan_receipts\": 0"),
            "orphan_receipts must NOT be 0: {json}"
        );
        assert!(
            !json.contains("\"stale_receipts\": 0"),
            "stale_receipts must NOT be 0: {json}"
        );
        assert!(
            !json.contains("\"gap_mismatch_receipts\": 0"),
            "gap_mismatch_receipts must NOT be 0: {json}"
        );
        assert!(
            !json.contains("\"verify_failed_receipts\": 0"),
            "verify_failed_receipts must NOT be 0: {json}"
        );
    }

    /// Regression guard for #1130: even when a gap-ledger summary IS present
    /// (and even if it carries receipt_stale_total / receipt_verify_failed_total
    /// keys), the three gap-ledger-path-only deferred fields stay not_available.
    /// The gap-ledger build has no production producer for stale/orphan/mismatch,
    /// so a numeric value here would be a fake zero / fabricated count.
    ///
    /// verify_failed_receipts also stays not_available here because no attempt
    /// ledger is supplied; it only becomes a real count when an attempt ledger
    /// IS supplied (see `verify_failed_receipts_nonzero_when_attempt_ledger_has_real_failure`).
    #[test]
    fn receipt_status_deferred_fields_stay_not_available_even_with_ledger_summary() {
        let ledger = serde_json::json!({
            "summary": {
                "repairable_total": 3,
                "receipt_improved_total": 1,
                "receipt_unchanged_after_attempt_total": 0,
                // These keys could appear if a future producer wrote them, but
                // the gap-ledger build has no real stale/orphan/mismatch producer.
                "receipt_stale_total": 0,
                "receipt_verify_failed_total": 0
            }
        });
        // No attempt ledger supplied → verify_failed_receipts stays not_available.
        let s = build_pr_evidence_summary(None, Some(&ledger), None, None, None, None);
        assert!(
            matches!(
                s.receipt_status.stale_receipts,
                U64OrNotAvailable::NotAvailable
            ),
            "stale_receipts must stay not_available (no real producer in gap-ledger build)"
        );
        assert!(
            matches!(
                s.receipt_status.verify_failed_receipts,
                U64OrNotAvailable::NotAvailable
            ),
            "verify_failed_receipts must stay not_available when no attempt ledger is supplied"
        );
        assert!(
            matches!(
                s.receipt_status.orphan_receipts,
                U64OrNotAvailable::NotAvailable
            ),
            "orphan_receipts must stay not_available"
        );
        assert!(
            matches!(
                s.receipt_status.gap_mismatch_receipts,
                U64OrNotAvailable::NotAvailable
            ),
            "gap_mismatch_receipts must stay not_available"
        );
    }

    /// receipts_present is derived as improved + unchanged_after_attempt.
    #[test]
    fn receipt_status_receipts_present_derived_from_ledger() {
        let ledger = serde_json::json!({
            "summary": {
                "repairable_total": 5,
                "receipt_improved_total": 2,
                "receipt_unchanged_after_attempt_total": 1
            }
        });
        let s = build_pr_evidence_summary(None, Some(&ledger), None, None, None, None);
        // receipts_present = improved(2) + unchanged(1) = 3
        assert!(
            matches!(
                s.receipt_status.receipts_present,
                U64OrNotAvailable::Value(3)
            ),
            "receipts_present must be 3 (2+1)"
        );
        // missing_receipts mirrors top-level: repairable(5) - improved(2) = 3
        assert!(
            matches!(
                s.receipt_status.missing_receipts,
                U64OrNotAvailable::Value(3)
            ),
            "receipt_status.missing_receipts must be 3"
        );
    }

    /// A repairable gap with no receipt evidence shows up as missing_receipts >= 1.
    #[test]
    fn claimed_repair_with_no_receipt_shows_in_missing_receipts() -> Result<(), String> {
        // One repairable gap, no receipt_improved_total => missing = 1.
        let ledger = serde_json::json!({
            "summary": {
                "repairable_total": 1,
                "receipt_improved_total": 0,
                "receipt_unchanged_after_attempt_total": 0
            }
        });
        let s = build_pr_evidence_summary(None, Some(&ledger), None, None, None, None);
        match s.receipt_status.missing_receipts {
            U64OrNotAvailable::Value(n) => assert!(
                n >= 1,
                "missing_receipts must be >= 1 when repair claimed with no receipt"
            ),
            U64OrNotAvailable::NotAvailable => {
                return Err(
                    "missing_receipts must be a concrete value when ledger is present".to_string(),
                );
            }
        }
        // Top-level missing_receipts must agree.
        match s.missing_receipts {
            U64OrNotAvailable::Value(n) => assert!(
                n >= 1,
                "top-level missing_receipts must be >= 1 when repair claimed with no receipt"
            ),
            U64OrNotAvailable::NotAvailable => {
                return Err(
                    "top-level missing_receipts must be a concrete value when ledger is present"
                        .to_string(),
                );
            }
        }
        Ok(())
    }

    /// JSON must contain receipt_status with receipts_present and
    /// missing_receipts as computed integers when the gap ledger is present.
    /// The three gap-ledger-path-only fields stay not_available because their
    /// producers are not on this path (see #1130).
    /// verify_failed_receipts stays not_available here because no attempt ledger
    /// is supplied; when an attempt ledger IS supplied it becomes a real count
    /// (see `verify_failed_receipts_nonzero_when_attempt_ledger_has_real_failure`).
    #[test]
    fn receipt_status_json_derived_fields_are_integers() {
        let ledger = serde_json::json!({
            "summary": {
                "repairable_total": 4,
                "receipt_improved_total": 1,
                "receipt_unchanged_after_attempt_total": 1
            }
        });
        // No attempt ledger → verify_failed_receipts stays not_available.
        let s = build_pr_evidence_summary(None, Some(&ledger), None, None, None, None);
        let json = render_pr_evidence_summary_json(&s);
        // receipts_present = 1+1 = 2
        assert!(
            json.contains("\"receipts_present\": 2"),
            "receipts_present must be 2 in JSON: {json}"
        );
        // missing_receipts in receipt_status = repairable(4) - improved(1) = 3
        // (Note: the JSON key appears twice — once at top level, once inside receipt_status.
        //  We cannot distinguish them by plain contains, so we just confirm the value 3 appears.)
        assert!(
            json.contains("\"missing_receipts\": 3"),
            "missing_receipts=3 must appear in JSON: {json}"
        );
        // The three gap-ledger-path-only deferred fields must still be not_available.
        assert!(
            json.contains("\"orphan_receipts\": \"not_available\""),
            "orphan_receipts must remain not_available even when ledger is present: {json}"
        );
        assert!(
            json.contains("\"stale_receipts\": \"not_available\""),
            "stale_receipts must remain not_available even when ledger is present: {json}"
        );
        assert!(
            json.contains("\"gap_mismatch_receipts\": \"not_available\""),
            "gap_mismatch_receipts must remain not_available even when ledger is present: {json}"
        );
        // verify_failed_receipts: not_available when no attempt ledger supplied.
        assert!(
            json.contains("\"verify_failed_receipts\": \"not_available\""),
            "verify_failed_receipts must be not_available without an attempt ledger: {json}"
        );
    }

    // ── verify_failed_receipts non-zero proof ────────────────────────────────

    /// Build an attempt-ledger JSON that mirrors the real `swarm_ingest.rs:861`
    /// fixture: `verify.status:"failed"`, `exit_code:1` → `attempt_outcome:
    /// "receipt_present"` with `classification.state:"verify_failed"`.
    ///
    /// When that entry flows through the attempt-ledger build (which reads
    /// `verify_result` from outcomes, populating it as `"fail"` or the verbatim
    /// status string), the resulting `attempts[].verify_result` is a failure
    /// token. This helper synthesises that terminal ledger JSON exactly as the
    /// real attempt-ledger JSON writer produces it — avoiding the full pipeline
    /// while being traceable to the real producer.
    fn attempt_ledger_with_one_failure() -> serde_json::Value {
        serde_json::json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "report": "swarm-attempt-ledger",
            "attempts": [
                {
                    "packet_id": "packet:python:verify-fail",
                    "canonical_gap_id": "gap:python:verify-fail",
                    "attempt_id": "attempt:gap-python-verify-fail:receipt-present",
                    "actor_kind": "codex",
                    "verify_command": "pytest tests/test_verify.py",
                    // "failed" is the verbatim value that swarm_ingest.rs records
                    // when verify.status:"failed" (exit_code:1) — the same case as
                    // swarm_ingest.rs:861 fail_closed_verify_failed_but_movement_claimed.
                    "verify_result": "failed",
                    "outcome": "receipt_present",
                    "receipt_state": "receipt_present",
                    "reason": "verify_failed guard fired before provenance check"
                }
            ]
        })
    }

    fn attempt_ledger_all_passed() -> serde_json::Value {
        serde_json::json!({
            "schema_version": "0.1",
            "tool": "ripr",
            "report": "swarm-attempt-ledger",
            "attempts": [
                {
                    "packet_id": "packet:rust:boundary",
                    "canonical_gap_id": "gap:rust:boundary",
                    "attempt_id": "attempt:gap-rust-boundary:evidence-improved",
                    "actor_kind": "codex",
                    "verify_command": "cargo test boundary",
                    "verify_result": "pass",
                    "outcome": "evidence_improved",
                    "receipt_state": "receipt_present",
                    "reason": "evidence improved after verify passed"
                }
            ]
        })
    }

    /// Non-zero proof (a): with a real-failure input, verify_failed_receipts == 1.
    /// The `verify_result:"failed"` value mirrors the output of the real
    /// swarm_ingest.rs pipeline for `verify.status:"failed"` (the
    /// `fail_closed_verify_failed_but_movement_claimed` fixture at line 861).
    #[test]
    fn verify_failed_receipts_nonzero_when_attempt_ledger_has_real_failure() {
        let ledger = attempt_ledger_with_one_failure();
        let s = build_pr_evidence_summary(None, None, None, None, None, Some(&ledger));
        assert!(
            matches!(
                s.receipt_status.verify_failed_receipts,
                U64OrNotAvailable::Value(1)
            ),
            "verify_failed_receipts must be 1 when attempt ledger has one failed-verify entry"
        );
        // stale/orphan/gap_mismatch must remain not_available in all cases.
        assert!(
            matches!(
                s.receipt_status.stale_receipts,
                U64OrNotAvailable::NotAvailable
            ),
            "stale_receipts must stay not_available"
        );
        assert!(
            matches!(
                s.receipt_status.orphan_receipts,
                U64OrNotAvailable::NotAvailable
            ),
            "orphan_receipts must stay not_available"
        );
        assert!(
            matches!(
                s.receipt_status.gap_mismatch_receipts,
                U64OrNotAvailable::NotAvailable
            ),
            "gap_mismatch_receipts must stay not_available"
        );
        let json = render_pr_evidence_summary_json(&s);
        assert!(
            json.contains("\"verify_failed_receipts\": 1"),
            "JSON must show verify_failed_receipts: 1 — got: {json}"
        );
    }

    /// Non-zero proof (b): with an all-passed attempt ledger, verify_failed_receipts == 0.
    /// This is an honest 0: we inspected the ledger and no failure was present.
    #[test]
    fn verify_failed_receipts_zero_when_attempt_ledger_all_passed() {
        let ledger = attempt_ledger_all_passed();
        let s = build_pr_evidence_summary(None, None, None, None, None, Some(&ledger));
        assert!(
            matches!(
                s.receipt_status.verify_failed_receipts,
                U64OrNotAvailable::Value(0)
            ),
            "verify_failed_receipts must be 0 (honest inspected zero) when all verifies passed"
        );
        let json = render_pr_evidence_summary_json(&s);
        assert!(
            json.contains("\"verify_failed_receipts\": 0"),
            "JSON must show verify_failed_receipts: 0 (honest zero) — got: {json}"
        );
        // stale/orphan/gap_mismatch must remain not_available.
        assert!(
            json.contains("\"stale_receipts\": \"not_available\""),
            "stale_receipts must stay not_available: {json}"
        );
        assert!(
            json.contains("\"orphan_receipts\": \"not_available\""),
            "orphan_receipts must stay not_available: {json}"
        );
        assert!(
            json.contains("\"gap_mismatch_receipts\": \"not_available\""),
            "gap_mismatch_receipts must stay not_available: {json}"
        );
    }

    /// Non-zero proof (c): with no attempt ledger, verify_failed_receipts == not_available.
    #[test]
    fn verify_failed_receipts_not_available_when_no_attempt_ledger() {
        let s = build_pr_evidence_summary(None, None, None, None, None, None);
        assert!(
            matches!(
                s.receipt_status.verify_failed_receipts,
                U64OrNotAvailable::NotAvailable
            ),
            "verify_failed_receipts must be not_available when no attempt ledger supplied"
        );
        let json = render_pr_evidence_summary_json(&s);
        assert!(
            json.contains("\"verify_failed_receipts\": \"not_available\""),
            "JSON must show verify_failed_receipts: not_available — got: {json}"
        );
    }

    /// is_verify_failure_result covers all expected failure tokens and rejects
    /// non-failure tokens.
    #[test]
    fn is_verify_failure_result_matches_expected_tokens() {
        assert!(is_verify_failure_result("fail"));
        assert!(is_verify_failure_result("failed"));
        assert!(is_verify_failure_result("error"));
        assert!(!is_verify_failure_result("pass"));
        assert!(!is_verify_failure_result("passed"));
        assert!(!is_verify_failure_result("not_run"));
        assert!(!is_verify_failure_result("not_applicable"));
        assert!(!is_verify_failure_result(""));
        assert!(!is_verify_failure_result("unknown"));
    }
}
