use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_EXPECTATIONS: &str =
    "fixtures/boundary_gap/expected/recommendation-calibration/expectations.json";
const DEFAULT_REPORT_JSON: &str = "target/ripr/reports/recommendation-calibration.json";
const LIMITS_NOTE: &str = "Advisory recommendation-quality evidence only; no telemetry, generated tests, source edits, mutation execution, or CI blocking.";

#[derive(Clone, Debug, Eq, PartialEq)]
struct RecommendationCalibrationArgs {
    root: PathBuf,
    pr_guidance: Option<PathBuf>,
    pilot_summary: Option<PathBuf>,
    agent_receipt: Option<PathBuf>,
    targeted_test_outcome: Option<PathBuf>,
    expectations: PathBuf,
    outcome_receipts: Vec<PathBuf>,
    out: PathBuf,
    out_md: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TargetedMovement {
    state: String,
    before_class: Option<String>,
    after_class: Option<String>,
}

pub(crate) fn recommendation_calibration(args: &[String]) -> Result<(), String> {
    let parsed = parse_recommendation_calibration_args(args)?;
    let report = recommendation_calibration_report(&parsed)?;
    let json = serde_json::to_string_pretty(&report)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render recommendation calibration JSON: {err}"))?;
    let markdown = recommendation_calibration_markdown(&report);

    write_output_path(&parsed.out, &json)?;
    write_output_path(&recommendation_calibration_md_path(&parsed), &markdown)
}

fn parse_recommendation_calibration_args(
    args: &[String],
) -> Result<RecommendationCalibrationArgs, String> {
    let mut root: Option<PathBuf> = None;
    let mut pr_guidance: Option<PathBuf> = None;
    let mut pilot_summary: Option<PathBuf> = None;
    let mut agent_receipt: Option<PathBuf> = None;
    let mut targeted_test_outcome: Option<PathBuf> = None;
    let mut expectations: Option<PathBuf> = None;
    let mut outcome_receipts = Vec::new();
    let mut out: Option<PathBuf> = None;
    let mut out_md: Option<PathBuf> = None;
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--root" => root = Some(next_path_arg(args, &mut index, arg)?),
            "--pr-guidance" | "--review-comments" => {
                pr_guidance = Some(next_path_arg(args, &mut index, arg)?);
            }
            "--pilot-summary" => pilot_summary = Some(next_path_arg(args, &mut index, arg)?),
            "--agent-receipt" => agent_receipt = Some(next_path_arg(args, &mut index, arg)?),
            "--targeted-test-outcome" => {
                targeted_test_outcome = Some(next_path_arg(args, &mut index, arg)?);
            }
            "--expectations" | "--calibration-expectations" => {
                expectations = Some(next_path_arg(args, &mut index, arg)?);
            }
            "--receipt" | "--outcome-receipt" => {
                outcome_receipts.push(next_path_arg(args, &mut index, arg)?);
            }
            "--out" => out = Some(next_path_arg(args, &mut index, arg)?),
            "--out-md" => out_md = Some(next_path_arg(args, &mut index, arg)?),
            "--help" | "-h" => return Err(recommendation_calibration_usage()),
            flag if flag.starts_with('-') => {
                return Err(format!(
                    "unknown recommendation-calibration option `{flag}`\n{}",
                    recommendation_calibration_usage()
                ));
            }
            positional => {
                if root.is_some() {
                    return Err(format!(
                        "unexpected extra positional argument `{positional}`\n{}",
                        recommendation_calibration_usage()
                    ));
                }
                root = Some(PathBuf::from(positional));
            }
        }
        index += 1;
    }

    Ok(RecommendationCalibrationArgs {
        root: root.unwrap_or_else(|| PathBuf::from(".")),
        pr_guidance,
        pilot_summary,
        agent_receipt,
        targeted_test_outcome,
        expectations: expectations.unwrap_or_else(|| PathBuf::from(DEFAULT_EXPECTATIONS)),
        outcome_receipts,
        out: out.unwrap_or_else(|| PathBuf::from(DEFAULT_REPORT_JSON)),
        out_md,
    })
}

fn next_path_arg(args: &[String], index: &mut usize, flag: &str) -> Result<PathBuf, String> {
    *index += 1;
    let Some(value) = args.get(*index) else {
        return Err(format!(
            "missing value for `{flag}`\n{}",
            recommendation_calibration_usage()
        ));
    };
    Ok(PathBuf::from(value))
}

fn recommendation_calibration_usage() -> String {
    "usage: cargo xtask recommendation-calibration [root] [--pr-guidance <path>] [--targeted-test-outcome <path>] [--expectations <path>] [--receipt <path>] [--out <json-path>] [--out-md <markdown-path>]".to_string()
}

fn recommendation_calibration_md_path(parsed: &RecommendationCalibrationArgs) -> PathBuf {
    parsed.out_md.clone().unwrap_or_else(|| {
        let mut path = parsed.out.clone();
        path.set_extension("md");
        path
    })
}

fn recommendation_calibration_report(
    parsed: &RecommendationCalibrationArgs,
) -> Result<Value, String> {
    let root = parsed.root.clone();
    let expectations_path = resolve_input_path(&root, &parsed.expectations);
    let expectations = read_json_file(&expectations_path)?;
    let cases = expectations
        .get("cases")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "{} is missing recommendation calibration `cases` array",
                display_path(&parsed.expectations)
            )
        })?;
    let receipts = read_receipts(&root, &parsed.outcome_receipts)?;
    let targeted_movement = match parsed.targeted_test_outcome.as_ref() {
        Some(path) => {
            let value = read_json_file(&resolve_input_path(&root, path))?;
            targeted_movement_by_seam(&value)
        }
        None => BTreeMap::new(),
    };

    let mut artifact_cache = BTreeMap::new();
    let mut recommendations = Vec::new();
    let mut suppressed = Vec::new();
    let mut warnings = Vec::new();
    let mut observed_sources = BTreeSet::new();

    for case in cases {
        let source_artifact = string_field(case, "source_artifact").ok_or_else(|| {
            "recommendation calibration case is missing `source_artifact`".to_string()
        })?;
        let source_collection = string_field(case, "source_collection").ok_or_else(|| {
            "recommendation calibration case is missing `source_collection`".to_string()
        })?;
        let source_item_id = string_field(case, "source_item_id");
        let expected = case
            .get("expected")
            .ok_or_else(|| "recommendation calibration case is missing `expected`".to_string())?;
        let seam_id = string_field(expected, "seam_id").unwrap_or("unknown");
        let case_id = string_field(case, "id").unwrap_or("unknown");
        let source_value =
            cached_source_artifact(&root, source_artifact, &mut artifact_cache, &mut warnings);
        let actual_item = source_value
            .as_ref()
            .and_then(|value| source_item(value, source_collection, source_item_id, seam_id));
        let receipt = receipt_for(&receipts, case_id, source_item_id, seam_id);
        let calibration = calibration_record(case, receipt, expected);
        let placement = placement_record(actual_item.as_ref(), receipt, expected);
        let suggested_test = suggested_test_record(actual_item.as_ref(), receipt, expected);
        let movement = static_movement_record(
            seam_id,
            receipt,
            expected,
            parsed.targeted_test_outcome.as_ref(),
            &targeted_movement,
        );
        let record_id = actual_item
            .as_ref()
            .and_then(|item| string_field(item, "id"))
            .or(source_item_id)
            .map(str::to_string)
            .unwrap_or_else(|| format!("ripr-review-{seam_id}"));

        if actual_item.is_none() && source_collection != "warnings" {
            warnings.push(format!(
                "case `{case_id}` did not find `{source_collection}` item for seam `{seam_id}` in `{source_artifact}`"
            ));
        }
        observed_sources.insert(source_artifact.to_string());

        if matches!(source_collection, "suppressed" | "warnings") {
            suppressed.push(json!({
                "id": record_id,
                "seam_id": seam_id,
                "source": source_collection,
                "reason": suppressed_reason(actual_item.as_ref(), expected),
                "quality": suppression_quality(receipt, expected),
                "calibration": calibration,
            }));
            continue;
        }

        recommendations.push(json!({
            "id": record_id,
            "seam_id": seam_id,
            "rank": recommendations.len() + 1,
            "source": source_collection,
            "placement": placement,
            "grip_class": actual_item
                .as_ref()
                .and_then(|item| string_field(item, "grip_class"))
                .unwrap_or("unknown"),
            "severity": actual_item
                .as_ref()
                .and_then(|item| string_field(item, "severity"))
                .unwrap_or("unknown"),
            "missing_discriminator": actual_item
                .as_ref()
                .and_then(|item| item.get("missing_discriminator"))
                .cloned()
                .unwrap_or(Value::Null),
            "suggested_test": suggested_test,
            "calibration": calibration,
            "static_movement": movement,
        }));
    }

    for path in optional_missing_inputs(parsed) {
        warnings.push(format!("optional input not provided: {path}"));
    }

    let summary = recommendation_calibration_summary(&recommendations, &suppressed);
    let latency = latency_record(&receipts);

    Ok(json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "status": if cases.is_empty() { "incomplete" } else { "advisory" },
        "root": display_path(&root),
        "inputs": {
            "pr_guidance": parsed.pr_guidance.as_ref().map(|path| display_path(path)),
            "pilot_summary": parsed.pilot_summary.as_ref().map(|path| display_path(path)),
            "agent_receipt": parsed.agent_receipt.as_ref().map(|path| display_path(path)),
            "targeted_test_outcome": parsed.targeted_test_outcome.as_ref().map(|path| display_path(path)),
            "calibration_expectations": display_path(&parsed.expectations),
            "outcome_receipts": parsed.outcome_receipts.iter().map(|path| display_path(path)).collect::<Vec<_>>(),
            "source_artifacts": observed_sources.into_iter().collect::<Vec<_>>(),
        },
        "summary": summary,
        "latency": latency,
        "recommendations": recommendations,
        "suppressed": suppressed,
        "warnings": warnings,
        "limits_note": LIMITS_NOTE,
    }))
}

fn read_receipts(root: &Path, paths: &[PathBuf]) -> Result<Vec<Value>, String> {
    paths
        .iter()
        .map(|path| read_json_file(&resolve_input_path(root, path)))
        .collect()
}

fn cached_source_artifact(
    root: &Path,
    artifact: &str,
    cache: &mut BTreeMap<String, Value>,
    warnings: &mut Vec<String>,
) -> Option<Value> {
    if let Some(value) = cache.get(artifact) {
        return Some(value.clone());
    }
    let path = resolve_input_path(root, Path::new(artifact));
    match read_json_file(&path) {
        Ok(value) => {
            cache.insert(artifact.to_string(), value.clone());
            Some(value)
        }
        Err(err) => {
            warnings.push(format!(
                "missing or malformed source artifact `{artifact}`: {err}"
            ));
            None
        }
    }
}

fn source_item(
    source: &Value,
    collection: &str,
    source_item_id: Option<&str>,
    seam_id: &str,
) -> Option<Value> {
    match collection {
        "comments" | "summary_only" | "suppressed" => source
            .get(collection)
            .and_then(Value::as_array)?
            .iter()
            .find(|item| {
                source_item_id.is_some_and(|id| string_field(item, "id") == Some(id))
                    || string_field(item, "seam_id") == Some(seam_id)
            })
            .cloned(),
        "warnings" => {
            let found = source
                .get("warnings")
                .and_then(Value::as_array)?
                .iter()
                .filter_map(Value::as_str)
                .any(|warning| warning.contains(seam_id));
            found.then(|| json!({"seam_id": seam_id, "reason": "severity_off"}))
        }
        _ => None,
    }
}

fn receipt_for<'a>(
    receipts: &'a [Value],
    case_id: &str,
    source_item_id: Option<&str>,
    seam_id: &str,
) -> Option<&'a Value> {
    receipts
        .iter()
        .find(|receipt| string_field(receipt, "case_id") == Some(case_id))
        .or_else(|| {
            receipts
                .iter()
                .filter(|receipt| string_field(receipt, "case_id").is_none())
                .find(|receipt| {
                    receipt
                        .get("guidance")
                        .and_then(|guidance| string_field(guidance, "id"))
                        .is_some_and(|id| source_item_id == Some(id))
                        || receipt
                            .get("guidance")
                            .and_then(|guidance| string_field(guidance, "seam_id"))
                            == Some(seam_id)
                })
        })
}

fn calibration_record(case: &Value, receipt: Option<&Value>, expected: &Value) -> Value {
    if let Some(receipt) = receipt {
        return json!({
            "outcome": receipt
                .get("outcome")
                .and_then(|outcome| string_field(outcome, "label"))
                .unwrap_or("unknown"),
            "source": "outcome_receipt",
            "reason": receipt
                .get("outcome")
                .and_then(|outcome| string_field(outcome, "reason"))
                .unwrap_or("outcome receipt supplied no reason"),
        });
    }
    json!({
        "outcome": string_field(expected, "outcome").unwrap_or("unknown"),
        "source": "fixture_expectation",
        "reason": string_field(case, "reason").unwrap_or("fixture expectation supplied no reason"),
    })
}

fn placement_record(actual: Option<&Value>, receipt: Option<&Value>, expected: &Value) -> Value {
    let quality = receipt
        .and_then(|receipt| receipt.get("placement"))
        .and_then(|placement| string_field(placement, "quality"))
        .or_else(|| string_field(expected, "placement_quality"))
        .unwrap_or("unknown");
    let placement = receipt
        .and_then(|receipt| receipt.get("placement"))
        .or_else(|| actual.and_then(|item| item.get("placement")));

    json!({
        "path": placement.and_then(|value| string_field(value, "path")),
        "line": placement.and_then(|value| value.get("line")).and_then(Value::as_u64),
        "mode": placement.and_then(|value| string_field(value, "mode")).unwrap_or("summary_only"),
        "quality": quality,
    })
}

fn suggested_test_record(
    actual: Option<&Value>,
    receipt: Option<&Value>,
    expected: &Value,
) -> Value {
    let receipt_test = receipt.and_then(|receipt| receipt.get("suggested_test"));
    let actual_test = actual.and_then(|item| item.get("suggested_test"));
    json!({
        "recommended_file": actual_test
            .and_then(|test| string_field(test, "recommended_file")),
        "near_test": actual_test.and_then(|test| string_field(test, "near_test")),
        "target_quality": receipt_test
            .and_then(|test| string_field(test, "target_quality"))
            .or_else(|| string_field(expected, "suggested_test_target_quality"))
            .unwrap_or("unknown"),
        "expected_file": receipt_test
            .and_then(|test| string_field(test, "expected_file"))
            .or_else(|| string_field(expected, "expected_test_file")),
    })
}

fn static_movement_record(
    seam_id: &str,
    receipt: Option<&Value>,
    expected: &Value,
    targeted_path: Option<&PathBuf>,
    targeted_movement: &BTreeMap<String, TargetedMovement>,
) -> Value {
    if let Some(movement) = targeted_movement.get(seam_id) {
        return json!({
            "state": movement.state,
            "source": "targeted_test_outcome",
            "artifact": targeted_path.map(|path| display_path(path)),
            "before_class": movement.before_class,
            "after_class": movement.after_class,
        });
    }
    if let Some(receipt_movement) = receipt.and_then(|receipt| receipt.get("static_movement")) {
        return json!({
            "state": string_field(receipt_movement, "state").unwrap_or("unknown"),
            "source": string_field(receipt_movement, "source").unwrap_or("outcome_receipt"),
            "artifact": string_field(receipt_movement, "artifact"),
            "before_class": Value::Null,
            "after_class": Value::Null,
        });
    }
    json!({
        "state": string_field(expected, "static_movement").unwrap_or("unknown"),
        "source": "fixture_expectation",
        "artifact": Value::Null,
        "before_class": Value::Null,
        "after_class": Value::Null,
    })
}

fn suppressed_reason(actual: Option<&Value>, expected: &Value) -> String {
    actual
        .and_then(|item| string_field(item, "reason"))
        .or_else(|| string_field(expected, "suppressed_reason"))
        .unwrap_or("unknown")
        .to_string()
}

fn suppression_quality(receipt: Option<&Value>, expected: &Value) -> String {
    receipt
        .and_then(|receipt| receipt.get("suppression"))
        .and_then(|suppression| string_field(suppression, "quality"))
        .or_else(|| string_field(expected, "suppression_quality"))
        .unwrap_or("unknown")
        .to_string()
}

fn recommendation_calibration_summary(recommendations: &[Value], suppressed: &[Value]) -> Value {
    let mut useful = 0;
    let mut noisy = 0;
    let mut false_annotations = 0;
    let mut summary_only_correct = 0;
    let mut suppressed_correctly = 0;
    let mut target_file_correct = 0;
    let mut static_improved = 0;
    let mut static_unchanged = 0;
    let mut static_regressed = 0;
    let mut unknown = 0;

    for recommendation in recommendations {
        let outcome = recommendation_outcome(recommendation);
        match outcome {
            "useful" => useful += 1,
            "noisy" => noisy += 1,
            "summary_only_correct" => summary_only_correct += 1,
            "unknown" => unknown += 1,
            _ => {}
        }
        if recommendation["source"] == "comments"
            && matches!(
                outcome,
                "wrong_line" | "already_covered" | "wrong_target" | "noisy"
            )
        {
            false_annotations += 1;
        }
        if recommendation
            .get("suggested_test")
            .and_then(|test| string_field(test, "target_quality"))
            == Some("correct")
        {
            target_file_correct += 1;
        }
        match recommendation
            .get("static_movement")
            .and_then(|movement| string_field(movement, "state"))
            .unwrap_or("unknown")
        {
            "improved" | "resolved" => static_improved += 1,
            "unchanged" => static_unchanged += 1,
            "regressed" | "new_gap" => static_regressed += 1,
            "unknown" | "missing_after_snapshot" => unknown += 1,
            _ => {}
        }
    }

    for record in suppressed {
        match record
            .get("calibration")
            .and_then(|calibration| string_field(calibration, "outcome"))
            .unwrap_or("unknown")
        {
            "suppressed_correctly" => suppressed_correctly += 1,
            "unknown" => unknown += 1,
            _ => {}
        }
    }

    json!({
        "recommendations_evaluated": recommendations.len() + suppressed.len(),
        "top_recommendation_outcome": recommendations
            .first()
            .map(recommendation_outcome)
            .unwrap_or("unknown"),
        "useful": useful,
        "noisy": noisy,
        "false_annotations": false_annotations,
        "summary_only_correct": summary_only_correct,
        "suppressed_correctly": suppressed_correctly,
        "target_file_correct": target_file_correct,
        "static_improved": static_improved,
        "static_unchanged": static_unchanged,
        "static_regressed": static_regressed,
        "unknown": unknown,
    })
}

fn recommendation_outcome(recommendation: &Value) -> &str {
    recommendation
        .get("calibration")
        .and_then(|calibration| string_field(calibration, "outcome"))
        .unwrap_or("unknown")
}

fn latency_record(receipts: &[Value]) -> Value {
    let guidance = first_latency_value(receipts, "guidance_generated_unix_ms");
    let annotation = first_latency_value(receipts, "annotation_emitted_unix_ms");
    let outcome = first_latency_value(receipts, "outcome_recorded_unix_ms");
    json!({
        "guidance_generated_unix_ms": guidance,
        "annotation_emitted_unix_ms": annotation,
        "outcome_recorded_unix_ms": outcome,
        "annotation_latency_ms": elapsed_ms(guidance, annotation),
        "outcome_latency_ms": first_latency_value(receipts, "outcome_latency_ms")
            .or_else(|| elapsed_ms(guidance, outcome)),
    })
}

fn first_latency_value(receipts: &[Value], key: &str) -> Option<u64> {
    receipts.iter().find_map(|receipt| {
        receipt
            .get("latency")
            .and_then(|latency| latency.get(key))
            .and_then(Value::as_u64)
    })
}

fn elapsed_ms(start: Option<u64>, end: Option<u64>) -> Option<u64> {
    match (start, end) {
        (Some(start), Some(end)) => end.checked_sub(start),
        _ => None,
    }
}

fn targeted_movement_by_seam(value: &Value) -> BTreeMap<String, TargetedMovement> {
    let mut out = BTreeMap::new();
    for (collection, fallback_state) in [
        ("moved", "improved"),
        ("unchanged", "unchanged"),
        ("regressed", "regressed"),
    ] {
        for item in value
            .get(collection)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(seam_id) = string_field(item, "seam_id") {
                out.insert(
                    seam_id.to_string(),
                    TargetedMovement {
                        state: string_field(item, "direction")
                            .unwrap_or(fallback_state)
                            .to_string(),
                        before_class: string_field(item, "before").map(str::to_string),
                        after_class: string_field(item, "after").map(str::to_string),
                    },
                );
            }
        }
    }
    for (collection, state) in [("new", "new_gap"), ("removed", "resolved")] {
        for item in value
            .get(collection)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(seam_id) = string_field(item, "seam_id") {
                out.insert(
                    seam_id.to_string(),
                    TargetedMovement {
                        state: state.to_string(),
                        before_class: None,
                        after_class: string_field(item, "grip_class").map(str::to_string),
                    },
                );
            }
        }
    }
    out
}

fn optional_missing_inputs(parsed: &RecommendationCalibrationArgs) -> Vec<&'static str> {
    let mut missing = Vec::new();
    if parsed.pr_guidance.is_none() {
        missing.push("pr_guidance");
    }
    if parsed.pilot_summary.is_none() {
        missing.push("pilot_summary");
    }
    if parsed.agent_receipt.is_none() {
        missing.push("agent_receipt");
    }
    if parsed.targeted_test_outcome.is_none() {
        missing.push("targeted_test_outcome");
    }
    if parsed.outcome_receipts.is_empty() {
        missing.push("outcome_receipts");
    }
    missing
}

fn recommendation_calibration_markdown(report: &Value) -> String {
    let mut out = String::new();
    out.push_str("# Recommendation Calibration\n\n");
    out.push_str(&format!(
        "Status: {}\n\n",
        string_field(report, "status").unwrap_or("unknown")
    ));
    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Count |\n| --- | ---: |\n");
    for key in [
        "recommendations_evaluated",
        "useful",
        "noisy",
        "false_annotations",
        "summary_only_correct",
        "suppressed_correctly",
        "target_file_correct",
        "static_improved",
        "static_unchanged",
        "static_regressed",
        "unknown",
    ] {
        let count = report
            .get("summary")
            .and_then(|summary| summary.get(key))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        out.push_str(&format!("| {key} | {count} |\n"));
    }

    out.push_str("\n## Top Recommendation\n\n");
    if let Some(top) = report
        .get("recommendations")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
    {
        let seam_id = string_field(top, "seam_id").unwrap_or("unknown");
        let outcome = top
            .get("calibration")
            .and_then(|calibration| string_field(calibration, "outcome"))
            .unwrap_or("unknown");
        let reason = top
            .get("calibration")
            .and_then(|calibration| string_field(calibration, "reason"))
            .unwrap_or("No reason available.");
        let path = top
            .get("placement")
            .and_then(|placement| string_field(placement, "path"))
            .unwrap_or("summary-only");
        let line = top
            .get("placement")
            .and_then(|placement| placement.get("line"))
            .and_then(Value::as_u64)
            .map(|line| line.to_string())
            .unwrap_or_else(|| "n/a".to_string());
        out.push_str(&format!("- seam: `{}`\n", md_escape(seam_id)));
        out.push_str(&format!("- location: `{}:{}`\n", md_escape(path), line));
        out.push_str(&format!("- outcome: `{}`\n", md_escape(outcome)));
        out.push_str(&format!("- why: {}\n", md_escape(reason)));
    } else {
        out.push_str("No visible recommendation was evaluated.\n");
    }

    out.push_str("\n## Suppressed\n\n");
    let suppressed = report
        .get("suppressed")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if suppressed.is_empty() {
        out.push_str("None.\n");
    } else {
        for item in suppressed {
            out.push_str(&format!(
                "- `{}`: {} ({})\n",
                md_escape(string_field(&item, "seam_id").unwrap_or("unknown")),
                md_escape(string_field(&item, "reason").unwrap_or("unknown")),
                md_escape(string_field(&item, "quality").unwrap_or("unknown"))
            ));
        }
    }

    out.push_str("\n## Warnings\n\n");
    let warnings = report
        .get("warnings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if warnings.is_empty() {
        out.push_str("None.\n");
    } else {
        for warning in warnings.iter().filter_map(Value::as_str) {
            out.push_str(&format!("- {}\n", md_escape(warning)));
        }
    }
    out.push_str("\nLimits: Advisory recommendation-quality evidence only.\n");
    out
}

fn read_json_file(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

fn resolve_input_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn write_output_path(path: &Path, body: &str) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))?;
    }
    fs::write(path, body).map_err(|err| format!("write {}: {err}", path.display()))
}

fn string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn md_escape(value: &str) -> String {
    value.replace('|', "\\|").replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(path: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(path)
    }

    #[test]
    fn recommendation_calibration_args_parse_inputs_and_outputs() -> Result<(), String> {
        let args = vec![
            "--root".to_string(),
            "repo".to_string(),
            "--pr-guidance".to_string(),
            "comments.json".to_string(),
            "--targeted-test-outcome".to_string(),
            "outcome.json".to_string(),
            "--expectations".to_string(),
            "expectations.json".to_string(),
            "--receipt".to_string(),
            "receipt.json".to_string(),
            "--out".to_string(),
            "report.json".to_string(),
        ];
        let parsed = parse_recommendation_calibration_args(&args)?;
        assert_eq!(parsed.root, PathBuf::from("repo"));
        assert_eq!(parsed.pr_guidance, Some(PathBuf::from("comments.json")));
        assert_eq!(
            parsed.targeted_test_outcome,
            Some(PathBuf::from("outcome.json"))
        );
        assert_eq!(parsed.expectations, PathBuf::from("expectations.json"));
        assert_eq!(parsed.outcome_receipts, vec![PathBuf::from("receipt.json")]);
        assert_eq!(parsed.out, PathBuf::from("report.json"));
        assert_eq!(
            recommendation_calibration_md_path(&parsed),
            PathBuf::from("report.md")
        );
        Ok(())
    }

    #[test]
    fn recommendation_calibration_counts_fixture_expectations() -> Result<(), String> {
        let args = RecommendationCalibrationArgs {
            root: fixture("."),
            pr_guidance: Some(fixture(
                "fixtures/boundary_gap/expected/recommendation-calibration/synthetic-pr-guidance.json",
            )),
            pilot_summary: None,
            agent_receipt: None,
            targeted_test_outcome: None,
            expectations: fixture(
                "fixtures/boundary_gap/expected/recommendation-calibration/expectations.json",
            ),
            outcome_receipts: Vec::new(),
            out: PathBuf::from("target/ripr/reports/recommendation-calibration.json"),
            out_md: None,
        };
        let report = recommendation_calibration_report(&args)?;
        assert_eq!(report["status"], "advisory");
        assert_eq!(report["summary"]["recommendations_evaluated"], 10);
        assert_eq!(report["summary"]["top_recommendation_outcome"], "useful");
        assert_eq!(report["summary"]["useful"], 2);
        assert_eq!(report["summary"]["noisy"], 1);
        assert_eq!(report["summary"]["false_annotations"], 4);
        assert_eq!(report["summary"]["summary_only_correct"], 2);
        assert_eq!(report["summary"]["suppressed_correctly"], 2);
        assert_eq!(report["summary"]["target_file_correct"], 6);
        assert_eq!(report["summary"]["static_improved"], 2);
        assert_eq!(report["summary"]["static_unchanged"], 3);
        assert_eq!(report["summary"]["static_regressed"], 0);
        assert_eq!(report["recommendations"].as_array().map(Vec::len), Some(8));
        assert_eq!(report["suppressed"].as_array().map(Vec::len), Some(2));
        Ok(())
    }

    #[test]
    fn recommendation_calibration_outcome_receipts_override_fixture_labels() -> Result<(), String> {
        let args = RecommendationCalibrationArgs {
            root: fixture("."),
            pr_guidance: None,
            pilot_summary: None,
            agent_receipt: None,
            targeted_test_outcome: None,
            expectations: fixture(
                "fixtures/boundary_gap/expected/recommendation-calibration/expectations.json",
            ),
            outcome_receipts: vec![fixture(
                "fixtures/boundary_gap/expected/recommendation-calibration/outcome-receipts/wrong-target.json",
            )],
            out: PathBuf::from("target/ripr/reports/recommendation-calibration.json"),
            out_md: None,
        };
        let report = recommendation_calibration_report(&args)?;
        let trait_case = report["recommendations"]
            .as_array()
            .and_then(|items| {
                items
                    .iter()
                    .find(|item| item["seam_id"] == "calibration-trait-generic")
            })
            .ok_or("missing trait generic recommendation")?;
        assert_eq!(trait_case["calibration"]["source"], "outcome_receipt");
        assert_eq!(trait_case["calibration"]["outcome"], "wrong_target");
        assert_eq!(
            trait_case["suggested_test"]["target_quality"],
            "wrong_target"
        );
        Ok(())
    }

    #[test]
    fn recommendation_calibration_imports_targeted_static_movement() -> Result<(), String> {
        let targeted = json!({
            "moved": [
                {
                    "seam_id": "8f7fa8644fd12280",
                    "direction": "improved",
                    "before": "weakly_gripped",
                    "after": "strongly_gripped"
                }
            ],
            "unchanged": [],
            "regressed": [],
            "new": [],
            "removed": []
        });
        let target_dir = std::env::temp_dir().join(format!(
            "ripr-recommendation-calibration-{}",
            std::process::id()
        ));
        fs::create_dir_all(&target_dir).map_err(|err| format!("create temp dir: {err}"))?;
        let outcome = target_dir.join("targeted-test-outcome.json");
        let targeted_json = serde_json::to_string(&targeted)
            .map_err(|err| format!("render targeted outcome: {err}"))?;
        fs::write(&outcome, targeted_json)
            .map_err(|err| format!("write targeted outcome: {err}"))?;

        let args = RecommendationCalibrationArgs {
            root: fixture("."),
            pr_guidance: None,
            pilot_summary: None,
            agent_receipt: None,
            targeted_test_outcome: Some(outcome.clone()),
            expectations: fixture(
                "fixtures/boundary_gap/expected/recommendation-calibration/expectations.json",
            ),
            outcome_receipts: Vec::new(),
            out: PathBuf::from("target/ripr/reports/recommendation-calibration.json"),
            out_md: None,
        };
        let report = recommendation_calibration_report(&args)?;
        let first = &report["recommendations"][0];
        assert_eq!(first["static_movement"]["source"], "targeted_test_outcome");
        assert_eq!(first["static_movement"]["state"], "improved");
        assert_eq!(first["static_movement"]["before_class"], "weakly_gripped");
        assert_eq!(first["static_movement"]["after_class"], "strongly_gripped");
        let _ = fs::remove_dir_all(target_dir);
        Ok(())
    }

    #[test]
    fn recommendation_calibration_markdown_names_top_recommendation() -> Result<(), String> {
        let args = RecommendationCalibrationArgs {
            root: fixture("."),
            pr_guidance: None,
            pilot_summary: None,
            agent_receipt: None,
            targeted_test_outcome: None,
            expectations: fixture(
                "fixtures/boundary_gap/expected/recommendation-calibration/expectations.json",
            ),
            outcome_receipts: Vec::new(),
            out: PathBuf::from("target/ripr/reports/recommendation-calibration.json"),
            out_md: None,
        };
        let report = recommendation_calibration_report(&args)?;
        let markdown = recommendation_calibration_markdown(&report);
        assert!(markdown.contains("# Recommendation Calibration"));
        assert!(markdown.contains("recommendations_evaluated"));
        assert!(markdown.contains("8f7fa8644fd12280"));
        assert!(markdown.contains("Advisory recommendation-quality evidence only"));
        Ok(())
    }
}
