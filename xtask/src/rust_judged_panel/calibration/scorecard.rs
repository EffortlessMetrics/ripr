//! Deterministic JSON/Markdown scorecard from one joined DTO (#4795).
//!
//! Rates never invent a zero percent: no denominator is `not_measurable`.
//! Runtime results cannot rewrite structural judgment bytes.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::{
    JoinedRow, RERUN, RESULT_CAUGHT, RESULT_EQUIVALENT, RESULT_INCONCLUSIVE, RESULT_INSTRUMENT,
    RESULT_NOT_RUN, RESULT_STALE, RESULT_SURVIVED, RUNTIME_RESULTS, STATIC_DIRECTIONS, digest_pref,
    eligibility::ELIGIBLE, static_direction_from_terminal,
};

pub(super) struct RenderedScorecard {
    pub(super) json: String,
    pub(super) markdown: String,
    pub(super) value: Value,
}

pub(super) fn build_scorecard(
    selection_sha256: &str,
    judgments_sha256: &str,
    rolling_observation_sha256: &str,
    rows: &[JoinedRow],
) -> Result<RenderedScorecard, String> {
    if rows.is_empty() {
        return Err(format!(
            "calibration scorecard refused an empty judged set; a failed or empty run is not a successful scorecard\nrerun: {RERUN}"
        ));
    }
    let mut ordered = rows.to_vec();
    ordered.sort_by(|left, right| left.case.case_id.cmp(&right.case.case_id));
    let value = scorecard_value(
        selection_sha256,
        judgments_sha256,
        rolling_observation_sha256,
        &ordered,
    )?;
    let json = serde_json::to_string_pretty(&value)
        .map_err(|error| format!("serialize calibration scorecard: {error}\nrerun: {RERUN}"))?
        + "\n";
    let markdown = render_markdown(&value);
    Ok(RenderedScorecard {
        json,
        markdown,
        value,
    })
}

fn scorecard_value(
    selection_sha256: &str,
    judgments_sha256: &str,
    rolling_observation_sha256: &str,
    rows: &[JoinedRow],
) -> Result<Value, String> {
    let mut matrix: BTreeMap<String, BTreeMap<String, Vec<String>>> = BTreeMap::new();
    for direction in STATIC_DIRECTIONS {
        let mut inner = BTreeMap::new();
        for result in RUNTIME_RESULTS {
            inner.insert((*result).to_string(), Vec::new());
        }
        matrix.insert((*direction).to_string(), inner);
    }

    let mut by_expected: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut by_family: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut by_relation: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut by_oracle: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut by_witness: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut eligible = Vec::new();
    let mut attempted = Vec::new();
    let mut completed = Vec::new();
    let mut inconclusive = Vec::new();
    let mut not_run = Vec::new();
    let mut unavailable = Vec::new();
    let mut stale = Vec::new();
    let mut instrument_failed = Vec::new();
    let mut equivalent = Vec::new();
    let mut false_exposed_candidates = Vec::new();
    let mut under_credit_candidates = Vec::new();
    let mut limitation_correct = Vec::new();
    let mut wrong_target = Vec::new();
    let mut case_values = Vec::new();

    for row in rows {
        let static_dir = static_direction_from_terminal(&row.case.terminal)
            .unwrap_or(row.case.expected_direction.as_str());
        if let Some(bucket) = matrix
            .get_mut(static_dir)
            .and_then(|inner| inner.get_mut(row.runtime_result))
        {
            bucket.push(row.case.case_id.clone());
            bucket.sort();
        }
        push_cov(
            &mut by_expected,
            &row.case.expected_direction,
            &row.case.case_id,
        );
        push_cov(&mut by_family, &row.case.behavior_family, &row.case.case_id);
        push_cov(
            &mut by_relation,
            &row.case.relation_basis,
            &row.case.case_id,
        );
        push_cov(&mut by_oracle, &row.case.oracle_class, &row.case.case_id);
        push_cov(
            &mut by_witness,
            &row.case.witness_completeness,
            &row.case.case_id,
        );
        if row.eligibility == ELIGIBLE {
            eligible.push(row.case.case_id.clone());
        }
        match row.runtime_result {
            RESULT_CAUGHT | RESULT_SURVIVED => {
                attempted.push(row.case.case_id.clone());
                completed.push(row.case.case_id.clone());
            }
            RESULT_EQUIVALENT => {
                attempted.push(row.case.case_id.clone());
                equivalent.push(row.case.case_id.clone());
            }
            RESULT_INCONCLUSIVE => {
                attempted.push(row.case.case_id.clone());
                inconclusive.push(row.case.case_id.clone());
            }
            RESULT_INSTRUMENT => {
                attempted.push(row.case.case_id.clone());
                instrument_failed.push(row.case.case_id.clone());
            }
            RESULT_STALE => {
                attempted.push(row.case.case_id.clone());
                stale.push(row.case.case_id.clone());
                wrong_target.push(row.case.case_id.clone());
            }
            RESULT_NOT_RUN => {
                not_run.push(row.case.case_id.clone());
                if row.eligibility != ELIGIBLE {
                    unavailable.push(row.case.case_id.clone());
                }
            }
            _ => {}
        }
        if static_dir == "should_stay_quiet" && row.runtime_result == RESULT_SURVIVED {
            false_exposed_candidates.push(row.case.case_id.clone());
        }
        if static_dir == "should_gap" && row.runtime_result == RESULT_CAUGHT {
            under_credit_candidates.push(row.case.case_id.clone());
        }
        if static_dir == "should_limit"
            && matches!(
                row.runtime_result,
                RESULT_CAUGHT | RESULT_SURVIVED | RESULT_EQUIVALENT
            )
        {
            limitation_correct.push(row.case.case_id.clone());
        }
        case_values.push(case_value(row));
    }

    let completed_for_rates: BTreeSet<&str> = completed.iter().map(String::as_str).collect();
    let false_exposed_rate = rate_object(
        false_exposed_candidates.len(),
        completed_for_rates.len(),
        &false_exposed_candidates,
        &equivalent,
    );
    let under_credit_rate = rate_object(
        under_credit_candidates.len(),
        completed_for_rates.len(),
        &under_credit_candidates,
        &equivalent,
    );
    let false_actionable_rate = rate_object(0, completed_for_rates.len(), &[], &equivalent);
    let limitation_rate = rate_object(
        limitation_correct.len(),
        rows.iter()
            .filter(|row| {
                static_direction_from_terminal(&row.case.terminal) == Some("should_limit")
            })
            .count(),
        &limitation_correct,
        &[],
    );
    let wrong_target_rate = rate_object(wrong_target.len(), rows.len(), &wrong_target, &[]);

    let mut matrix_json = serde_json::Map::new();
    for (direction, results) in &matrix {
        let mut inner = serde_json::Map::new();
        for (result, ids) in results {
            inner.insert(
                result.clone(),
                json!({
                    "count": ids.len(),
                    "case_ids": ids,
                }),
            );
        }
        matrix_json.insert(direction.clone(), Value::Object(inner));
    }

    let mut body = json!({
        "schema_version": "0.1",
        "kind": "rust_judged_panel_calibration_scorecard",
        "authority": "EffortlessMetrics/ripr-swarm#4795",
        "inherited_authorities": [
            "EffortlessMetrics/ripr-swarm#3164",
            "EffortlessMetrics/ripr-swarm#3806",
            "EffortlessMetrics/ripr-swarm#4578"
        ],
        "selection_sha256": selection_sha256,
        "judgments_sha256": judgments_sha256,
        "rolling_observation_sha256": rolling_observation_sha256,
        "counts": {
            "selected": rows.len(),
            "judged": rows.len(),
            "calibration_eligible": eligible.len(),
            "attempted": attempted.len(),
            "completed": completed.len(),
            "inconclusive": inconclusive.len()
        },
        "coverage": {
            "by_expected_direction": coverage_object(&by_expected),
            "by_behavior_family": coverage_object(&by_family),
            "by_relation_basis": coverage_object(&by_relation),
            "by_oracle_class": coverage_object(&by_oracle),
            "by_witness_completeness": coverage_object(&by_witness)
        },
        "static_runtime_matrix": Value::Object(matrix_json),
        "candidates": {
            "false_actionable": false_actionable_rate,
            "false_exposed": false_exposed_rate,
            "static_under_credit": under_credit_rate,
            "limitation_correct": limitation_rate,
            "wrong_target": wrong_target_rate
        },
        "not_run_case_ids": not_run,
        "unavailable_case_ids": unavailable,
        "stale_case_ids": stale,
        "instrument_failed_case_ids": instrument_failed,
        "equivalent_or_unusable_case_ids": equivalent,
        "inconclusive_case_ids": inconclusive,
        "cases": case_values,
        "limits": [
            "Runtime results cannot rewrite independently accepted structural judgments.",
            "A survived mutant is a review candidate, not an automatic false-exposed conclusion.",
            "A caught mutant on a static gap is an under-credit candidate, not an automatic analyzer defect.",
            "should_limit remains a static-boundary judgment even when a runtime experiment completes.",
            "Equivalent, failed, timed-out, unavailable, stale and inconclusive rows stay in the denominator.",
            "No denominator is not_measurable, never a fake zero percent.",
            "#3076 route-yield denominators are referenced and never merged.",
            "#4578 rolling observation identity is bound and never merged into this classification denominator.",
            "No single quality score, support-tier, release, or publication claim."
        ],
        "non_claims": [
            "universal analyzer accuracy",
            "mutation adequacy",
            "representative sampling",
            "release readiness",
            "support promotion",
            "route-yield merger"
        ]
    });
    let digest_source = body.clone();
    let canonical = serde_json::to_vec(&digest_source).map_err(|error| {
        format!("canonicalize calibration scorecard digest: {error}\nrerun: {RERUN}")
    })?;
    body["digest"] = json!(digest_pref(&canonical));
    Ok(body)
}

fn case_value(row: &JoinedRow) -> Value {
    json!({
        "case_id": row.case.case_id,
        "expected_direction": row.case.expected_direction,
        "terminal_structural_judgment": row.case.terminal,
        "static_direction": static_direction_from_terminal(&row.case.terminal)
            .unwrap_or(row.case.expected_direction.as_str()),
        "eligibility": row.eligibility,
        "runtime_result": row.runtime_result,
        "instrument_kind": row.instrument_kind,
        "non_calibration_reason": row.non_calibration_reason,
        "automatic_false_exposed": false,
        "receipt_digest": row.receipt.as_ref().map(|receipt| receipt.semantic_receipt_digest.clone()),
        "limitations": row.limitations
    })
}

fn push_cov(map: &mut BTreeMap<String, Vec<String>>, key: &str, case_id: &str) {
    let ids = map.entry(key.to_string()).or_default();
    ids.push(case_id.to_string());
    ids.sort();
}

fn coverage_object(map: &BTreeMap<String, Vec<String>>) -> Value {
    let mut object = serde_json::Map::new();
    for (key, ids) in map {
        object.insert(
            key.clone(),
            json!({
                "count": ids.len(),
                "case_ids": ids
            }),
        );
    }
    Value::Object(object)
}

fn rate_object(
    numerator: usize,
    denominator: usize,
    case_ids: &[String],
    excluded: &[String],
) -> Value {
    if denominator == 0 {
        json!({
            "numerator": numerator,
            "denominator": 0,
            "rate": "not_measurable",
            "case_ids": case_ids,
            "excluded_case_ids": excluded
        })
    } else {
        json!({
            "numerator": numerator,
            "denominator": denominator,
            "rate": format!("{numerator}/{denominator}"),
            "case_ids": case_ids,
            "excluded_case_ids": excluded
        })
    }
}

fn render_markdown(value: &Value) -> String {
    let mut out = String::from("# Rust judged-panel calibration scorecard\n\n");
    out.push_str(
        "Authority: `#4795`. Structural judgments stay independent of runtime results.\n\n",
    );
    let counts = &value["counts"];
    out.push_str("## Counts\n\n");
    for key in [
        "selected",
        "judged",
        "calibration_eligible",
        "attempted",
        "completed",
        "inconclusive",
    ] {
        out.push_str(&format!("- {key}: {}\n", counts[key].as_u64().unwrap_or(0)));
    }
    out.push_str("\n## Identities\n\n");
    out.push_str(&format!(
        "- selection: `{}`\n- judgments: `{}`\n- rolling observation: `{}`\n- digest: `{}`\n",
        value["selection_sha256"].as_str().unwrap_or(""),
        value["judgments_sha256"].as_str().unwrap_or(""),
        value["rolling_observation_sha256"].as_str().unwrap_or(""),
        value["digest"].as_str().unwrap_or("")
    ));
    out.push_str("\n## Static × runtime matrix\n\n");
    out.push_str("| static | runtime | count | cases |\n| --- | --- | --- | --- |\n");
    if let Some(matrix) = value["static_runtime_matrix"].as_object() {
        for (direction, results) in matrix {
            if let Some(results) = results.as_object() {
                for (result, cell) in results {
                    let count = cell["count"].as_u64().unwrap_or(0);
                    if count == 0 {
                        continue;
                    }
                    let ids = cell["case_ids"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ");
                    out.push_str(&format!(
                        "| `{direction}` | `{result}` | {count} | {ids} |\n"
                    ));
                }
            }
        }
    }
    out.push_str("\n## Candidates\n\n");
    for key in [
        "false_actionable",
        "false_exposed",
        "static_under_credit",
        "limitation_correct",
        "wrong_target",
    ] {
        let candidate = &value["candidates"][key];
        out.push_str(&format!(
            "- {key}: {} (numerator {}, denominator {})\n",
            candidate["rate"].as_str().unwrap_or("not_measurable"),
            candidate["numerator"].as_u64().unwrap_or(0),
            candidate["denominator"].as_u64().unwrap_or(0)
        ));
    }
    out.push_str(
        "\nSurvived mutants are retained without an automatic false-exposed conclusion.\n",
    );
    out.push_str("\n## Cases\n\n");
    if let Some(cases) = value["cases"].as_array() {
        for case in cases {
            out.push_str(&format!(
                "- `{}`: eligibility `{}`, runtime `{}`, terminal `{}`\n",
                case["case_id"].as_str().unwrap_or(""),
                case["eligibility"].as_str().unwrap_or(""),
                case["runtime_result"].as_str().unwrap_or(""),
                case["terminal_structural_judgment"].as_str().unwrap_or("")
            ));
        }
    }
    out.push_str("\n## Limits\n\n");
    if let Some(limits) = value["limits"].as_array() {
        for limit in limits {
            if let Some(limit) = limit.as_str() {
                out.push_str(&format!("- {limit}\n"));
            }
        }
    }
    out
}

pub(super) fn write_scorecard(
    json_path: &Path,
    markdown_path: &Path,
    report: &RenderedScorecard,
) -> Result<(), String> {
    stage_write(json_path, &report.json)?;
    stage_write(markdown_path, &report.markdown)?;
    Ok(())
}

fn stage_write(path: &Path, body: &str) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| {
        format!(
            "no parent directory for `{}`\nrerun: {RERUN}",
            path.display()
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "create calibration output `{}`: {error}\nrerun: {RERUN}",
            parent.display()
        )
    })?;
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let staged = parent.join(format!(
        ".{}.{}-{unique}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("scorecard"),
        std::process::id()
    ));
    {
        let mut file = fs::File::create(&staged).map_err(|error| {
            format!(
                "stage calibration output `{}`: {error}\nrerun: {RERUN}",
                staged.display()
            )
        })?;
        file.write_all(body.as_bytes()).map_err(|error| {
            format!(
                "write calibration output `{}`: {error}\nrerun: {RERUN}",
                staged.display()
            )
        })?;
        file.sync_all().map_err(|error| {
            format!(
                "flush calibration output `{}`: {error}\nrerun: {RERUN}",
                staged.display()
            )
        })?;
    }
    fs::rename(&staged, path).map_err(|error| {
        let _ = fs::remove_file(&staged);
        format!(
            "publish calibration output `{}`: {error}\nrerun: {RERUN}",
            path.display()
        )
    })?;
    Ok(())
}

pub(super) fn verify_stored(path: &Path, expected: &str) -> Result<(), String> {
    let stored = fs::read_to_string(path).map_err(|error| {
        format!(
            "calibration scorecard `{}` is missing; run `{RERUN}` first: {error}",
            path.display()
        )
    })?;
    if stored != expected {
        return Err(format!(
            "calibration scorecard `{}` drifted from a fresh derivation; run `{RERUN}` to refresh",
            path.display()
        ));
    }
    Ok(())
}

pub(super) fn output_paths(out_dir: &str) -> (PathBuf, PathBuf) {
    let dir = PathBuf::from(out_dir);
    (dir.join("scorecard.json"), dir.join("scorecard.md"))
}

/// Used by tests to observe that a failed command left no success artifact.
#[cfg(test)]
pub(super) fn scorecard_exists(out_dir: &Path) -> bool {
    out_dir.join("scorecard.json").is_file()
}
