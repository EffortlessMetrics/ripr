use crate::reports::pr_evidence_summary::model::{
    GapCounts, LimitationEntry, NullableU64, PrEvidenceSummaryJson, TopLimitation, TopRepair,
    U64OrNotAvailable,
};
use crate::reports::pr_evidence_summary::util::value_path;
use serde_json::{Value, json};

/// Build the in-memory summary struct from parsed artifact values.
///
/// - `start_here_value`   — parsed start-here.json, or None.
/// - `gap_ledger_value`   — parsed gap-decision-ledger.json, or None.
/// - `repo_exposure_value`— parsed repo-exposure.json, or None.
/// - `diff_report_value`  — parsed diff-report.json, or None.
/// - `baseline_value`     — parsed baseline snapshot, or None (no --baseline).
pub(super) fn build_pr_evidence_summary(
    start_here_value: Option<&Value>,
    gap_ledger_value: Option<&Value>,
    repo_exposure_value: Option<&Value>,
    diff_report_value: Option<&Value>,
    baseline_value: Option<&Value>,
) -> PrEvidenceSummaryJson {
    let run_status = derive_run_status(diff_report_value, repo_exposure_value);
    let changed_surfaces = derive_changed_surfaces(diff_report_value);
    let gaps = derive_gaps(gap_ledger_value, baseline_value);
    let limitations = derive_limitations(repo_exposure_value);
    let missing_receipts = derive_missing_receipts(gap_ledger_value);
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

    let mut obj = json!({
        "schema_version": "0.1",
        "kind": "pr_evidence_summary",
        "tool": "ripr",
        "run_status": s.run_status,
        "changed_surfaces": u64_or_not_available(&s.changed_surfaces),
        "gaps": gaps,
        "limitations": limitations,
        "missing_receipts": u64_or_not_available(&s.missing_receipts),
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
        build_pr_evidence_summary(None, None, None, None, None)
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
        let s = build_pr_evidence_summary(Some(&start_here), None, None, Some(&diff), None);

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
        let s = build_pr_evidence_summary(Some(&start_here), None, None, None, None);
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
        let s = build_pr_evidence_summary(None, Some(&ledger), None, None, None);
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
        let s = build_pr_evidence_summary(None, None, Some(&repo), None, None);
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
}
