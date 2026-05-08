use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: &str = "0.1";
const DEFAULT_EXPECTATIONS: &str =
    "fixtures/boundary_gap/expected/recommendation-calibration/expectations.json";
const DEFAULT_OUT: &str = "target/ripr/reports/recommendation-calibration.json";
const LIMITS_NOTE: &str = "Advisory recommendation-quality evidence only; no telemetry, generated tests, source edits, runtime execution, or CI blocking.";

#[derive(Clone, Debug, Eq, PartialEq)]
struct RecommendationCalibrationArgs {
    root: PathBuf,
    pr_guidance: Vec<PathBuf>,
    expectations: PathBuf,
    outcome_receipts: Vec<PathBuf>,
    targeted_test_outcome: Option<PathBuf>,
    agent_receipt: Option<PathBuf>,
    out: PathBuf,
    out_md: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExpectedCase {
    id: String,
    source_artifact: String,
    source_collection: String,
    source_item_id: Option<String>,
    outcome: String,
    placement_quality: String,
    suggested_test_target_quality: String,
    suppressed_reason: Option<String>,
    suppression_quality: Option<String>,
    static_movement: String,
    seam_id: String,
    expected_test_file: Option<String>,
    missing_discriminator: Option<String>,
    reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GuidanceInput {
    path: String,
    value: Option<Value>,
    error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OutcomeReceipt {
    path: String,
    case_id: Option<String>,
    guidance_artifact: Option<String>,
    guidance_id: Option<String>,
    seam_id: Option<String>,
    outcome: String,
    source: String,
    reason: String,
    placement_quality: String,
    suggested_test_target_quality: String,
    expected_file: Option<String>,
    actual_file: Option<String>,
    suppression_reason: Option<String>,
    suppression_quality: Option<String>,
    static_movement: String,
    latency: Latency,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Latency {
    guidance_generated_unix_ms: Option<i64>,
    annotation_emitted_unix_ms: Option<i64>,
    outcome_recorded_unix_ms: Option<i64>,
    annotation_latency_ms: Option<i64>,
    outcome_latency_ms: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StaticMovement {
    state: String,
    source: String,
    before_class: Option<String>,
    after_class: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CalibratedRecommendation {
    id: String,
    seam_id: String,
    rank: usize,
    source: String,
    source_artifact: String,
    source_case: String,
    placement: RecommendationPlacement,
    grip_class: String,
    severity: String,
    missing_discriminator: Option<String>,
    suggested_test: SuggestedTest,
    outcome: String,
    outcome_source: String,
    outcome_reason: String,
    static_movement: StaticMovement,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RecommendationPlacement {
    path: Option<String>,
    line: Option<usize>,
    mode: Option<String>,
    quality: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SuggestedTest {
    recommended_file: Option<String>,
    near_test: Option<String>,
    target_quality: String,
    expected_file: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CalibratedSuppression {
    id: String,
    seam_id: String,
    source_artifact: String,
    source_case: String,
    reason: String,
    quality: String,
    outcome: String,
    outcome_source: String,
    outcome_reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RecommendationCalibrationReport {
    status: String,
    root: String,
    inputs: RecommendationCalibrationInputs,
    summary: RecommendationCalibrationSummary,
    latency: Latency,
    recommendations: Vec<CalibratedRecommendation>,
    suppressed: Vec<CalibratedSuppression>,
    warnings: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RecommendationCalibrationInputs {
    pr_guidance: Vec<String>,
    targeted_test_outcome: Option<String>,
    agent_receipt: Option<String>,
    calibration_expectations: String,
    outcome_receipts: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct RecommendationCalibrationSummary {
    recommendations_evaluated: usize,
    top_recommendation_outcome: String,
    useful: usize,
    noisy: usize,
    false_annotations: usize,
    summary_only_correct: usize,
    suppressed_correctly: usize,
    target_file_correct: usize,
    static_improved: usize,
    static_unchanged: usize,
    static_regressed: usize,
    unknown: usize,
}

pub(crate) fn recommendation_calibration(args: &[String]) -> Result<(), String> {
    let parsed = parse_recommendation_calibration_args(args)?;
    let report = build_recommendation_calibration_report(&parsed)?;
    write_text_file(&parsed.out, &recommendation_calibration_json(&report)?)?;
    write_text_file(
        &parsed.out_md,
        &recommendation_calibration_markdown(&report),
    )?;
    println!("Wrote {}", parsed.out.display());
    println!("Wrote {}", parsed.out_md.display());
    Ok(())
}

fn parse_recommendation_calibration_args(
    args: &[String],
) -> Result<RecommendationCalibrationArgs, String> {
    let mut root = PathBuf::from(".");
    let mut pr_guidance = Vec::new();
    let mut expectations = PathBuf::from(DEFAULT_EXPECTATIONS);
    let mut outcome_receipts = Vec::new();
    let mut targeted_test_outcome = None;
    let mut agent_receipt = None;
    let mut out = PathBuf::from(DEFAULT_OUT);
    let mut out_md = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = PathBuf::from(expect_arg(args, i, "--root")?);
            }
            "--pr-guidance" => {
                i += 1;
                pr_guidance.push(PathBuf::from(expect_arg(args, i, "--pr-guidance")?));
            }
            "--calibration-expectations" | "--expectations" => {
                i += 1;
                expectations = PathBuf::from(expect_arg(args, i, "--calibration-expectations")?);
            }
            "--outcome-receipts" => {
                i += 1;
                outcome_receipts.push(PathBuf::from(expect_arg(args, i, "--outcome-receipts")?));
            }
            "--targeted-test-outcome" => {
                i += 1;
                targeted_test_outcome = Some(PathBuf::from(expect_arg(
                    args,
                    i,
                    "--targeted-test-outcome",
                )?));
            }
            "--agent-receipt" => {
                i += 1;
                agent_receipt = Some(PathBuf::from(expect_arg(args, i, "--agent-receipt")?));
            }
            "--out" => {
                i += 1;
                out = PathBuf::from(expect_arg(args, i, "--out")?);
            }
            "--out-md" => {
                i += 1;
                out_md = Some(PathBuf::from(expect_arg(args, i, "--out-md")?));
            }
            "--help" | "-h" => return Err(recommendation_calibration_usage()),
            other => {
                return Err(format!(
                    "unknown recommendation-calibration option `{other}`\n{}",
                    recommendation_calibration_usage()
                ));
            }
        }
        i += 1;
    }

    let out_md = out_md.unwrap_or_else(|| markdown_path_for(&out));
    Ok(RecommendationCalibrationArgs {
        root,
        pr_guidance,
        expectations,
        outcome_receipts,
        targeted_test_outcome,
        agent_receipt,
        out,
        out_md,
    })
}

fn expect_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    args.get(index)
        .map(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("recommendation-calibration {flag} requires a value"))
}

fn recommendation_calibration_usage() -> String {
    "usage: cargo xtask recommendation-calibration [--root <path>] [--pr-guidance <path>] [--calibration-expectations <path>] [--outcome-receipts <path>] [--targeted-test-outcome <path>] [--agent-receipt <path>] [--out <json-path>] [--out-md <markdown-path>]".to_string()
}

fn markdown_path_for(out: &Path) -> PathBuf {
    let mut path = out.to_path_buf();
    path.set_extension("md");
    path
}

fn build_recommendation_calibration_report(
    args: &RecommendationCalibrationArgs,
) -> Result<RecommendationCalibrationReport, String> {
    let expectations = read_expectations(&resolve_root_path(&args.root, &args.expectations))?;
    let mut guidance_paths = args
        .pr_guidance
        .iter()
        .map(|path| display_path(path))
        .collect::<Vec<_>>();
    if guidance_paths.is_empty() {
        guidance_paths = expectations
            .iter()
            .map(|case| case.source_artifact.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }

    let guidance_inputs = guidance_paths
        .iter()
        .map(|path| read_guidance_input(&args.root, path))
        .collect::<Vec<_>>();
    let receipts = read_outcome_receipts(&args.root, &args.outcome_receipts)?;
    let movement = read_static_movement(args)?;
    let mut warnings = Vec::new();
    for input in &guidance_inputs {
        if let Some(error) = &input.error {
            warnings.push(error.clone());
        }
    }

    let mut recommendations = Vec::new();
    let mut suppressed = Vec::new();
    for case in &expectations {
        let receipt = receipt_for_case(case, &receipts);
        let Some(input) = guidance_inputs
            .iter()
            .find(|input| same_path_text(&input.path, &case.source_artifact))
        else {
            warnings.push(format!(
                "{} source artifact {} was not included",
                case.id, case.source_artifact
            ));
            continue;
        };
        let Some(value) = input.value.as_ref() else {
            continue;
        };
        match case.source_collection.as_str() {
            "comments" | "summary_only" => {
                if let Some(item) = find_guidance_item(value, &case.source_collection, case) {
                    recommendations.push(calibrated_recommendation(
                        case,
                        item,
                        receipt,
                        movement.get(&case.seam_id),
                        recommendations.len() + 1,
                    ));
                } else {
                    warnings.push(format!(
                        "{} did not find {} in {}",
                        case.id,
                        case.source_item_id.as_deref().unwrap_or("<missing id>"),
                        case.source_artifact
                    ));
                }
            }
            "suppressed" => {
                if find_guidance_item(value, "suppressed", case).is_some() {
                    suppressed.push(calibrated_suppression(case, receipt));
                } else {
                    warnings.push(format!(
                        "{} did not find suppressed item in {}",
                        case.id, case.source_artifact
                    ));
                }
            }
            "warnings" => {
                if warning_matches(value, case) {
                    suppressed.push(calibrated_suppression(case, receipt));
                } else {
                    warnings.push(format!(
                        "{} did not find matching warning in {}",
                        case.id, case.source_artifact
                    ));
                }
            }
            other => warnings.push(format!(
                "{} uses unsupported source collection `{other}`",
                case.id
            )),
        }
    }

    let summary = summarize_recommendation_calibration(&recommendations, &suppressed);
    let status = if recommendations.is_empty() && suppressed.is_empty() {
        "incomplete"
    } else {
        "advisory"
    }
    .to_string();
    Ok(RecommendationCalibrationReport {
        status,
        root: display_path(&args.root),
        inputs: RecommendationCalibrationInputs {
            pr_guidance: guidance_inputs
                .iter()
                .map(|input| input.path.clone())
                .collect(),
            targeted_test_outcome: args
                .targeted_test_outcome
                .as_ref()
                .map(|path| display_path(path)),
            agent_receipt: args.agent_receipt.as_ref().map(|path| display_path(path)),
            calibration_expectations: display_path(&args.expectations),
            outcome_receipts: receipts
                .iter()
                .map(|receipt| receipt.path.clone())
                .collect(),
        },
        latency: combine_latency(&receipts),
        summary,
        recommendations,
        suppressed,
        warnings,
    })
}

fn read_expectations(path: &Path) -> Result<Vec<ExpectedCase>, String> {
    let value = read_json_value(path)?;
    let cases = value
        .get("cases")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{} is missing `cases` array", display_path(path)))?;
    let mut parsed = Vec::new();
    for case in cases {
        let expected = case
            .get("expected")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                "recommendation expectation case is missing expected object".to_string()
            })?;
        parsed.push(ExpectedCase {
            id: required_string(case, "id")?,
            source_artifact: required_string(case, "source_artifact")?,
            source_collection: required_string(case, "source_collection")?,
            source_item_id: optional_string(case, "source_item_id"),
            outcome: required_object_string(expected, "outcome")?,
            placement_quality: required_object_string(expected, "placement_quality")?,
            suggested_test_target_quality: required_object_string(
                expected,
                "suggested_test_target_quality",
            )?,
            suppressed_reason: optional_object_string(expected, "suppressed_reason"),
            suppression_quality: optional_object_string(expected, "suppression_quality"),
            static_movement: required_object_string(expected, "static_movement")?,
            seam_id: required_object_string(expected, "seam_id")?,
            expected_test_file: optional_object_string(expected, "expected_test_file"),
            missing_discriminator: optional_object_string(expected, "missing_discriminator"),
            reason: required_string(case, "reason")?,
        });
    }
    Ok(parsed)
}

fn read_guidance_input(root: &Path, path: &str) -> GuidanceInput {
    let path_text = normalize_path_text(path);
    match read_json_value(&resolve_root_path(root, Path::new(path))) {
        Ok(value) => GuidanceInput {
            path: path_text,
            value: Some(value),
            error: None,
        },
        Err(error) => GuidanceInput {
            path: path_text.clone(),
            value: None,
            error: Some(format!(
                "missing or invalid PR guidance input {path_text}: {error}"
            )),
        },
    }
}

fn read_outcome_receipts(root: &Path, paths: &[PathBuf]) -> Result<Vec<OutcomeReceipt>, String> {
    let mut receipt_files = Vec::new();
    for path in paths {
        let resolved = resolve_root_path(root, path);
        if resolved.is_dir() {
            let display_base = display_path(path);
            for entry in fs::read_dir(&resolved).map_err(|err| {
                format!(
                    "read outcome receipt dir {} failed: {err}",
                    resolved.display()
                )
            })? {
                let entry =
                    entry.map_err(|err| format!("read outcome receipt entry failed: {err}"))?;
                let entry_path = entry.path();
                if entry_path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                    let display_name = entry_path
                        .file_name()
                        .and_then(|file_name| file_name.to_str())
                        .map(|file_name| format!("{display_base}/{file_name}"))
                        .unwrap_or_else(|| display_path(&entry_path));
                    receipt_files.push((entry_path, display_name));
                }
            }
        } else {
            receipt_files.push((resolved, display_path(path)));
        }
    }
    receipt_files.sort();

    receipt_files
        .iter()
        .map(|(path, display)| read_outcome_receipt(path, display))
        .collect()
}

fn read_outcome_receipt(path: &Path, display: &str) -> Result<OutcomeReceipt, String> {
    let value = read_json_value(path)?;
    let guidance = value.get("guidance").and_then(Value::as_object);
    let outcome = value
        .get("outcome")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{} is missing outcome object", display_path(path)))?;
    let placement = value.get("placement").and_then(Value::as_object);
    let suggested = value.get("suggested_test").and_then(Value::as_object);
    let suppression = value.get("suppression").and_then(Value::as_object);
    let static_movement = value.get("static_movement").and_then(Value::as_object);
    Ok(OutcomeReceipt {
        path: display.to_string(),
        case_id: optional_string(&value, "case_id"),
        guidance_artifact: guidance.and_then(|object| optional_object_string(object, "artifact")),
        guidance_id: guidance.and_then(|object| optional_object_string(object, "id")),
        seam_id: guidance.and_then(|object| optional_object_string(object, "seam_id")),
        outcome: required_object_string(outcome, "label")?,
        source: optional_object_string(outcome, "source").unwrap_or_else(|| "fixture".to_string()),
        reason: optional_object_string(outcome, "reason").unwrap_or_default(),
        placement_quality: placement
            .and_then(|object| optional_object_string(object, "quality"))
            .unwrap_or_else(|| "unknown".to_string()),
        suggested_test_target_quality: suggested
            .and_then(|object| optional_object_string(object, "target_quality"))
            .unwrap_or_else(|| "unknown".to_string()),
        expected_file: suggested.and_then(|object| optional_object_string(object, "expected_file")),
        actual_file: suggested.and_then(|object| optional_object_string(object, "actual_file")),
        suppression_reason: suppression.and_then(|object| optional_object_string(object, "reason")),
        suppression_quality: suppression
            .and_then(|object| optional_object_string(object, "quality")),
        static_movement: static_movement
            .and_then(|object| optional_object_string(object, "state"))
            .unwrap_or_else(|| "unknown".to_string()),
        latency: latency_from_value(value.get("latency")),
    })
}

fn read_static_movement(
    args: &RecommendationCalibrationArgs,
) -> Result<BTreeMap<String, StaticMovement>, String> {
    let mut by_seam = BTreeMap::new();
    if let Some(path) = &args.targeted_test_outcome {
        let value = read_json_value(&resolve_root_path(&args.root, path))?;
        collect_targeted_outcome_movement(&mut by_seam, &value, "moved", "direction");
        collect_targeted_outcome_movement(&mut by_seam, &value, "unchanged", "unchanged");
        collect_targeted_outcome_movement(&mut by_seam, &value, "regressed", "regressed");
        collect_targeted_outcome_seams(&mut by_seam, &value, "new", "new_gap");
        collect_targeted_outcome_seams(&mut by_seam, &value, "removed", "resolved");
    }
    if let Some(path) = &args.agent_receipt {
        let value = read_json_value(&resolve_root_path(&args.root, path))?;
        if let Some(seam) = value.get("seam")
            && let Some(seam_id) = string_field(seam, "seam_id")
        {
            by_seam.insert(
                seam_id,
                StaticMovement {
                    state: string_field(seam, "change").unwrap_or_else(|| "unknown".to_string()),
                    source: "agent_receipt".to_string(),
                    before_class: string_field(seam, "before"),
                    after_class: string_field(seam, "after"),
                },
            );
        }
    }
    Ok(by_seam)
}

fn collect_targeted_outcome_movement(
    by_seam: &mut BTreeMap<String, StaticMovement>,
    value: &Value,
    key: &str,
    state_field_or_literal: &str,
) {
    let Some(items) = value.get(key).and_then(Value::as_array) else {
        return;
    };
    for item in items {
        if let Some(seam_id) = string_field(item, "seam_id") {
            let state = if state_field_or_literal == "direction" {
                string_field(item, "direction").unwrap_or_else(|| "unknown".to_string())
            } else {
                state_field_or_literal.to_string()
            };
            by_seam.insert(
                seam_id,
                StaticMovement {
                    state,
                    source: "targeted_test_outcome".to_string(),
                    before_class: string_field(item, "before"),
                    after_class: string_field(item, "after"),
                },
            );
        }
    }
}

fn collect_targeted_outcome_seams(
    by_seam: &mut BTreeMap<String, StaticMovement>,
    value: &Value,
    key: &str,
    state: &str,
) {
    let Some(items) = value.get(key).and_then(Value::as_array) else {
        return;
    };
    for item in items {
        if let Some(seam_id) = string_field(item, "seam_id") {
            by_seam.insert(
                seam_id,
                StaticMovement {
                    state: state.to_string(),
                    source: "targeted_test_outcome".to_string(),
                    before_class: None,
                    after_class: string_field(item, "grip_class"),
                },
            );
        }
    }
}

fn receipt_for_case<'a>(
    case: &ExpectedCase,
    receipts: &'a [OutcomeReceipt],
) -> Option<&'a OutcomeReceipt> {
    receipts
        .iter()
        .find(|receipt| receipt.case_id.as_deref() == Some(case.id.as_str()))
        .or_else(|| {
            receipts.iter().find(|receipt| {
                receipt.guidance_artifact.as_deref() == Some(case.source_artifact.as_str())
                    && receipt.guidance_id.as_deref() == case.source_item_id.as_deref()
            })
        })
        .or_else(|| {
            if case.source_item_id.is_none() {
                receipts
                    .iter()
                    .find(|receipt| receipt.seam_id.as_deref() == Some(case.seam_id.as_str()))
            } else {
                None
            }
        })
}

fn find_guidance_item<'a>(
    value: &'a Value,
    collection: &str,
    case: &ExpectedCase,
) -> Option<&'a Value> {
    value
        .get(collection)
        .and_then(Value::as_array)?
        .iter()
        .find(|item| {
            case.source_item_id
                .as_ref()
                .is_some_and(|id| string_field(item, "id").as_deref() == Some(id.as_str()))
                || string_field(item, "seam_id").as_deref() == Some(case.seam_id.as_str())
        })
}

fn warning_matches(value: &Value, case: &ExpectedCase) -> bool {
    value
        .get("warnings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .any(|warning| warning.contains(&case.seam_id))
}

fn calibrated_recommendation(
    case: &ExpectedCase,
    item: &Value,
    receipt: Option<&OutcomeReceipt>,
    static_movement: Option<&StaticMovement>,
    rank: usize,
) -> CalibratedRecommendation {
    let suggested = item.get("suggested_test").and_then(Value::as_object);
    let receipt_outcome = receipt.map(|receipt| receipt.outcome.clone());
    let outcome = receipt_outcome.unwrap_or_else(|| case.outcome.clone());
    let outcome_source = receipt
        .map(|receipt| format!("outcome_receipt:{}", receipt.source))
        .unwrap_or_else(|| "fixture_expectation".to_string());
    let outcome_reason = receipt
        .map(|receipt| receipt.reason.clone())
        .filter(|reason| !reason.is_empty())
        .unwrap_or_else(|| case.reason.clone());
    let movement = static_movement_from_sources(case, receipt, static_movement);
    CalibratedRecommendation {
        id: string_field(item, "id").unwrap_or_else(|| {
            case.source_item_id
                .clone()
                .unwrap_or_else(|| format!("ripr-review-{}", case.seam_id))
        }),
        seam_id: string_field(item, "seam_id").unwrap_or_else(|| case.seam_id.clone()),
        rank,
        source: case.source_collection.clone(),
        source_artifact: case.source_artifact.clone(),
        source_case: case.id.clone(),
        placement: recommendation_placement(item, case, receipt),
        grip_class: string_field(item, "grip_class").unwrap_or_else(|| "unknown".to_string()),
        severity: string_field(item, "severity").unwrap_or_else(|| "unknown".to_string()),
        missing_discriminator: string_field(item, "missing_discriminator")
            .or_else(|| case.missing_discriminator.clone()),
        suggested_test: SuggestedTest {
            recommended_file: suggested
                .and_then(|object| optional_object_string(object, "recommended_file"))
                .or_else(|| receipt.and_then(|receipt| receipt.actual_file.clone())),
            near_test: suggested.and_then(|object| optional_object_string(object, "near_test")),
            target_quality: receipt
                .map(|receipt| receipt.suggested_test_target_quality.clone())
                .unwrap_or_else(|| case.suggested_test_target_quality.clone()),
            expected_file: receipt
                .and_then(|receipt| receipt.expected_file.clone())
                .or_else(|| case.expected_test_file.clone()),
        },
        outcome,
        outcome_source,
        outcome_reason,
        static_movement: movement,
    }
}

fn recommendation_placement(
    item: &Value,
    case: &ExpectedCase,
    receipt: Option<&OutcomeReceipt>,
) -> RecommendationPlacement {
    let placement = item.get("placement");
    let object = placement.and_then(Value::as_object);
    RecommendationPlacement {
        path: object
            .and_then(|object| optional_object_string(object, "path"))
            .or_else(|| {
                item.get("seam")
                    .and_then(Value::as_object)
                    .and_then(|object| optional_object_string(object, "file"))
            }),
        line: object
            .and_then(|object| usize_object_field(object, "line"))
            .or_else(|| {
                item.get("seam")
                    .and_then(Value::as_object)
                    .and_then(|object| usize_object_field(object, "line"))
            }),
        mode: object.and_then(|object| optional_object_string(object, "mode")),
        quality: receipt
            .map(|receipt| receipt.placement_quality.clone())
            .unwrap_or_else(|| case.placement_quality.clone()),
    }
}

fn calibrated_suppression(
    case: &ExpectedCase,
    receipt: Option<&OutcomeReceipt>,
) -> CalibratedSuppression {
    CalibratedSuppression {
        id: case
            .source_item_id
            .clone()
            .unwrap_or_else(|| format!("ripr-review-{}", case.id)),
        seam_id: case.seam_id.clone(),
        source_artifact: case.source_artifact.clone(),
        source_case: case.id.clone(),
        reason: receipt
            .and_then(|receipt| receipt.suppression_reason.clone())
            .filter(|reason| reason != "none")
            .or_else(|| case.suppressed_reason.clone())
            .unwrap_or_else(|| "unknown".to_string()),
        quality: receipt
            .and_then(|receipt| receipt.suppression_quality.clone())
            .filter(|quality| quality != "not_applicable")
            .or_else(|| case.suppression_quality.clone())
            .unwrap_or_else(|| "unknown".to_string()),
        outcome: receipt
            .map(|receipt| receipt.outcome.clone())
            .unwrap_or_else(|| case.outcome.clone()),
        outcome_source: receipt
            .map(|receipt| format!("outcome_receipt:{}", receipt.source))
            .unwrap_or_else(|| "fixture_expectation".to_string()),
        outcome_reason: receipt
            .map(|receipt| receipt.reason.clone())
            .filter(|reason| !reason.is_empty())
            .unwrap_or_else(|| case.reason.clone()),
    }
}

fn static_movement_from_sources(
    case: &ExpectedCase,
    receipt: Option<&OutcomeReceipt>,
    static_movement: Option<&StaticMovement>,
) -> StaticMovement {
    if let Some(receipt) = receipt {
        return StaticMovement {
            state: receipt.static_movement.clone(),
            source: "outcome_receipt".to_string(),
            before_class: None,
            after_class: None,
        };
    }
    if let Some(static_movement) = static_movement {
        return static_movement.clone();
    }
    StaticMovement {
        state: case.static_movement.clone(),
        source: "fixture_expectation".to_string(),
        before_class: None,
        after_class: None,
    }
}

fn summarize_recommendation_calibration(
    recommendations: &[CalibratedRecommendation],
    suppressed: &[CalibratedSuppression],
) -> RecommendationCalibrationSummary {
    let mut summary = RecommendationCalibrationSummary {
        top_recommendation_outcome: recommendations
            .first()
            .map(|recommendation| recommendation.outcome.clone())
            .or_else(|| {
                suppressed
                    .first()
                    .map(|suppression| suppression.outcome.clone())
            })
            .unwrap_or_else(|| "unknown".to_string()),
        ..RecommendationCalibrationSummary::default()
    };
    summary.recommendations_evaluated = recommendations.len() + suppressed.len();
    for outcome in recommendations
        .iter()
        .map(|recommendation| recommendation.outcome.as_str())
        .chain(
            suppressed
                .iter()
                .map(|suppression| suppression.outcome.as_str()),
        )
    {
        match outcome {
            "useful" => summary.useful += 1,
            "noisy" => summary.noisy += 1,
            "summary_only_correct" => summary.summary_only_correct += 1,
            "suppressed_correctly" => summary.suppressed_correctly += 1,
            "unknown" => summary.unknown += 1,
            _ => {}
        }
        if matches!(
            outcome,
            "noisy" | "wrong_line" | "already_covered" | "wrong_target"
        ) {
            summary.false_annotations += 1;
        }
    }
    summary.target_file_correct = recommendations
        .iter()
        .filter(|recommendation| recommendation.suggested_test.target_quality == "correct")
        .count();
    for movement in recommendations
        .iter()
        .map(|recommendation| recommendation.static_movement.state.as_str())
    {
        match movement {
            "improved" => summary.static_improved += 1,
            "unchanged" => summary.static_unchanged += 1,
            "regressed" => summary.static_regressed += 1,
            _ => {}
        }
    }
    summary
}

fn recommendation_calibration_json(
    report: &RecommendationCalibrationReport,
) -> Result<String, String> {
    let value = json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "status": report.status,
        "root": report.root,
        "inputs": {
            "pr_guidance": report.inputs.pr_guidance,
            "targeted_test_outcome": report.inputs.targeted_test_outcome,
            "agent_receipt": report.inputs.agent_receipt,
            "calibration_expectations": report.inputs.calibration_expectations,
            "outcome_receipts": report.inputs.outcome_receipts,
        },
        "summary": {
            "recommendations_evaluated": report.summary.recommendations_evaluated,
            "top_recommendation_outcome": report.summary.top_recommendation_outcome,
            "useful": report.summary.useful,
            "noisy": report.summary.noisy,
            "false_annotations": report.summary.false_annotations,
            "summary_only_correct": report.summary.summary_only_correct,
            "suppressed_correctly": report.summary.suppressed_correctly,
            "target_file_correct": report.summary.target_file_correct,
            "static_improved": report.summary.static_improved,
            "static_unchanged": report.summary.static_unchanged,
            "static_regressed": report.summary.static_regressed,
            "unknown": report.summary.unknown,
        },
        "latency": latency_json(&report.latency),
        "recommendations": report.recommendations.iter().map(recommendation_json).collect::<Vec<_>>(),
        "suppressed": report.suppressed.iter().map(suppression_json).collect::<Vec<_>>(),
        "warnings": report.warnings,
        "limits_note": LIMITS_NOTE,
    });
    serde_json::to_string_pretty(&value)
        .map(|mut text| {
            text.push('\n');
            text
        })
        .map_err(|err| format!("failed to render recommendation calibration JSON: {err}"))
}

fn recommendation_json(recommendation: &CalibratedRecommendation) -> Value {
    json!({
        "id": recommendation.id,
        "seam_id": recommendation.seam_id,
        "rank": recommendation.rank,
        "source": recommendation.source,
        "source_artifact": recommendation.source_artifact,
        "source_case": recommendation.source_case,
        "placement": {
            "path": recommendation.placement.path,
            "line": recommendation.placement.line,
            "mode": recommendation.placement.mode,
            "quality": recommendation.placement.quality,
        },
        "grip_class": recommendation.grip_class,
        "severity": recommendation.severity,
        "missing_discriminator": recommendation.missing_discriminator,
        "suggested_test": {
            "recommended_file": recommendation.suggested_test.recommended_file,
            "near_test": recommendation.suggested_test.near_test,
            "target_quality": recommendation.suggested_test.target_quality,
            "expected_file": recommendation.suggested_test.expected_file,
        },
        "calibration": {
            "outcome": recommendation.outcome,
            "source": recommendation.outcome_source,
            "reason": recommendation.outcome_reason,
        },
        "static_movement": {
            "state": recommendation.static_movement.state,
            "source": recommendation.static_movement.source,
            "before_class": recommendation.static_movement.before_class,
            "after_class": recommendation.static_movement.after_class,
        }
    })
}

fn suppression_json(suppression: &CalibratedSuppression) -> Value {
    json!({
        "id": suppression.id,
        "seam_id": suppression.seam_id,
        "source_artifact": suppression.source_artifact,
        "source_case": suppression.source_case,
        "reason": suppression.reason,
        "quality": suppression.quality,
        "calibration": {
            "outcome": suppression.outcome,
            "source": suppression.outcome_source,
            "reason": suppression.outcome_reason,
        }
    })
}

fn latency_json(latency: &Latency) -> Value {
    json!({
        "guidance_generated_unix_ms": latency.guidance_generated_unix_ms,
        "annotation_emitted_unix_ms": latency.annotation_emitted_unix_ms,
        "outcome_recorded_unix_ms": latency.outcome_recorded_unix_ms,
        "annotation_latency_ms": latency.annotation_latency_ms,
        "outcome_latency_ms": latency.outcome_latency_ms,
    })
}

fn recommendation_calibration_markdown(report: &RecommendationCalibrationReport) -> String {
    let mut out = String::new();
    out.push_str("# Recommendation Calibration\n\n");
    out.push_str(&format!("Status: {}\n\n", report.status));
    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Count |\n| --- | ---: |\n");
    out.push_str(&format!(
        "| recommendations_evaluated | {} |\n",
        report.summary.recommendations_evaluated
    ));
    out.push_str(&format!("| useful | {} |\n", report.summary.useful));
    out.push_str(&format!("| noisy | {} |\n", report.summary.noisy));
    out.push_str(&format!(
        "| false_annotations | {} |\n",
        report.summary.false_annotations
    ));
    out.push_str(&format!(
        "| summary_only_correct | {} |\n",
        report.summary.summary_only_correct
    ));
    out.push_str(&format!(
        "| suppressed_correctly | {} |\n",
        report.summary.suppressed_correctly
    ));
    out.push_str(&format!(
        "| target_file_correct | {} |\n",
        report.summary.target_file_correct
    ));
    out.push_str(&format!(
        "| static_improved | {} |\n",
        report.summary.static_improved
    ));
    out.push_str(&format!(
        "| static_unchanged | {} |\n",
        report.summary.static_unchanged
    ));
    out.push_str(&format!(
        "| static_regressed | {} |\n",
        report.summary.static_regressed
    ));
    out.push_str(&format!("| unknown | {} |\n\n", report.summary.unknown));

    if let Some(top) = report.recommendations.first() {
        out.push_str("## Top Recommendation\n\n");
        out.push_str(&format!(
            "- `{}` `{}` `{}`\n",
            md_escape(&top.seam_id),
            md_escape(&top.placement.quality),
            md_escape(&top.outcome)
        ));
        if let Some(path) = &top.placement.path {
            out.push_str(&format!(
                "- placement: `{}`:{} `{}`\n",
                md_escape(path),
                top.placement.line.unwrap_or(0),
                md_escape(top.placement.mode.as_deref().unwrap_or("unknown"))
            ));
        }
        out.push_str(&format!("- why: {}\n", md_escape(&top.outcome_reason)));
        if let Some(file) = &top.suggested_test.recommended_file {
            out.push_str(&format!(
                "- suggested test: `{}` `{}`\n",
                md_escape(file),
                md_escape(&top.suggested_test.target_quality)
            ));
        }
        out.push_str(&format!(
            "- static movement: `{}` from `{}`\n\n",
            md_escape(&top.static_movement.state),
            md_escape(&top.static_movement.source)
        ));
    }

    if !report.recommendations.is_empty() {
        out.push_str("## Recommendations\n\n");
        out.push_str("| Rank | Seam | Source | Placement | Outcome | Target | Movement |\n");
        out.push_str("| ---: | --- | --- | --- | --- | --- | --- |\n");
        for item in &report.recommendations {
            out.push_str(&format!(
                "| {} | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
                item.rank,
                md_escape(&item.seam_id),
                md_escape(&item.source_case),
                md_escape(&item.placement.quality),
                md_escape(&item.outcome),
                md_escape(&item.suggested_test.target_quality),
                md_escape(&item.static_movement.state)
            ));
        }
        out.push('\n');
    }

    if !report.suppressed.is_empty() {
        out.push_str("## Suppressed\n\n");
        out.push_str("| Seam | Reason | Quality | Outcome |\n| --- | --- | --- | --- |\n");
        for item in &report.suppressed {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | `{}` |\n",
                md_escape(&item.seam_id),
                md_escape(&item.reason),
                md_escape(&item.quality),
                md_escape(&item.outcome)
            ));
        }
        out.push('\n');
    }

    if !report.warnings.is_empty() {
        out.push_str("## Warnings\n\n");
        for warning in &report.warnings {
            out.push_str(&format!("- {}\n", md_escape(warning)));
        }
        out.push('\n');
    }
    out.push_str("## Limits\n\n");
    out.push_str(LIMITS_NOTE);
    out.push('\n');
    out
}

fn combine_latency(receipts: &[OutcomeReceipt]) -> Latency {
    let mut combined = Latency::default();
    for receipt in receipts {
        combined.guidance_generated_unix_ms = combined
            .guidance_generated_unix_ms
            .or(receipt.latency.guidance_generated_unix_ms);
        combined.annotation_emitted_unix_ms = combined
            .annotation_emitted_unix_ms
            .or(receipt.latency.annotation_emitted_unix_ms);
        combined.outcome_recorded_unix_ms = combined
            .outcome_recorded_unix_ms
            .or(receipt.latency.outcome_recorded_unix_ms);
        combined.annotation_latency_ms = combined
            .annotation_latency_ms
            .or(receipt.latency.annotation_latency_ms);
        combined.outcome_latency_ms = combined
            .outcome_latency_ms
            .or(receipt.latency.outcome_latency_ms);
    }
    combined
}

fn latency_from_value(value: Option<&Value>) -> Latency {
    let Some(value) = value else {
        return Latency::default();
    };
    Latency {
        guidance_generated_unix_ms: i64_field(value, "guidance_generated_unix_ms"),
        annotation_emitted_unix_ms: i64_field(value, "annotation_emitted_unix_ms"),
        outcome_recorded_unix_ms: i64_field(value, "outcome_recorded_unix_ms"),
        annotation_latency_ms: i64_field(value, "annotation_latency_ms"),
        outcome_latency_ms: i64_field(value, "outcome_latency_ms"),
    }
}

fn read_json_value(path: &Path) -> Result<Value, String> {
    let text =
        fs::read_to_string(path).map_err(|err| format!("read {} failed: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("parse {} failed: {err}", path.display()))
}

fn write_text_file(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    fs::write(path, text).map_err(|err| format!("write {} failed: {err}", path.display()))
}

fn required_string(value: &Value, key: &str) -> Result<String, String> {
    string_field(value, key).ok_or_else(|| format!("expected string field `{key}`"))
}

fn required_object_string(object: &Map<String, Value>, key: &str) -> Result<String, String> {
    optional_object_string(object, key).ok_or_else(|| format!("expected string field `{key}`"))
}

fn optional_string(value: &Value, key: &str) -> Option<String> {
    match value.get(key) {
        Some(Value::String(value)) => Some(value.clone()),
        _ => None,
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    optional_string(value, key)
}

fn optional_object_string(object: &Map<String, Value>, key: &str) -> Option<String> {
    match object.get(key) {
        Some(Value::String(value)) => Some(value.clone()),
        _ => None,
    }
}

fn usize_object_field(object: &Map<String, Value>, key: &str) -> Option<usize> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn i64_field(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64)
}

fn same_path_text(left: &str, right: &str) -> bool {
    normalize_path_text(left) == normalize_path_text(right)
}

fn display_path(path: &Path) -> String {
    normalize_path_text(&path.display().to_string())
}

fn resolve_root_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        let rooted = root.join(path);
        if rooted.exists() {
            rooted
        } else {
            workspace_root().join(path)
        }
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn normalize_path_text(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    if normalized.is_empty() {
        ".".to_string()
    } else {
        normalized
            .strip_prefix("./")
            .unwrap_or(&normalized)
            .to_string()
    }
}

fn md_escape(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn recommendation_calibration_args_parse_defaults_and_inputs() -> Result<(), String> {
        let parsed = parse_recommendation_calibration_args(&[
            "--root".to_string(),
            ".".to_string(),
            "--pr-guidance".to_string(),
            "target/ripr/review/comments.json".to_string(),
            "--outcome-receipts".to_string(),
            "fixtures/outcomes".to_string(),
            "--targeted-test-outcome".to_string(),
            "target/ripr/outcome.json".to_string(),
            "--out".to_string(),
            "target/ripr/reports/recommendation-calibration.json".to_string(),
        ])?;
        assert_eq!(parsed.root, PathBuf::from("."));
        assert_eq!(
            parsed.pr_guidance,
            vec![PathBuf::from("target/ripr/review/comments.json")]
        );
        assert_eq!(parsed.expectations, PathBuf::from(DEFAULT_EXPECTATIONS));
        assert_eq!(
            parsed.out_md,
            PathBuf::from("target/ripr/reports/recommendation-calibration.md")
        );
        Ok(())
    }

    #[test]
    fn recommendation_calibration_report_counts_fixture_expectations() -> Result<(), String> {
        let args = RecommendationCalibrationArgs {
            root: PathBuf::from("."),
            pr_guidance: Vec::new(),
            expectations: PathBuf::from(DEFAULT_EXPECTATIONS),
            outcome_receipts: vec![PathBuf::from(
                "fixtures/boundary_gap/expected/recommendation-calibration/outcome-receipts",
            )],
            targeted_test_outcome: None,
            agent_receipt: None,
            out: PathBuf::from(DEFAULT_OUT),
            out_md: PathBuf::from("target/ripr/reports/recommendation-calibration.md"),
        };
        let report = build_recommendation_calibration_report(&args)?;
        assert_eq!(report.status, "advisory");
        assert_eq!(report.summary.recommendations_evaluated, 10);
        assert_eq!(report.summary.top_recommendation_outcome, "useful");
        assert_eq!(report.summary.useful, 2);
        assert_eq!(report.summary.noisy, 1);
        assert_eq!(report.summary.false_annotations, 4);
        assert_eq!(report.summary.summary_only_correct, 2);
        assert_eq!(report.summary.suppressed_correctly, 2);
        assert_eq!(report.summary.target_file_correct, 6);
        assert_eq!(report.summary.static_improved, 2);
        assert_eq!(report.summary.static_unchanged, 3);
        assert_eq!(report.summary.static_regressed, 0);
        assert_eq!(report.summary.unknown, 0);
        assert!(
            report
                .recommendations
                .iter()
                .any(|item| item.outcome == "wrong_target"
                    && item.suggested_test.target_quality == "wrong_target")
        );
        assert!(
            report
                .suppressed
                .iter()
                .any(|item| item.reason == "severity_off"
                    && item.quality == "suppressed_correctly")
        );
        Ok(())
    }

    #[test]
    fn recommendation_calibration_missing_guidance_is_incomplete() -> Result<(), String> {
        let dir = temp_dir("recommendation-calibration-missing")?;
        let expectations = dir.join("expectations.json");
        fs::write(
            &expectations,
            r#"{
              "cases": [
                {
                  "id": "missing",
                  "source_artifact": "missing-comments.json",
                  "source_collection": "comments",
                  "source_item_id": "ripr-review-missing",
                  "expected": {
                    "outcome": "unknown",
                    "placement_quality": "unknown",
                    "suggested_test_target_quality": "unknown",
                    "suppressed_reason": null,
                    "suppression_quality": null,
                    "static_movement": "unknown",
                    "seam_id": "missing-seam",
                    "expected_test_file": null,
                    "missing_discriminator": null
                  },
                  "reason": "missing guidance stays incomplete"
                }
              ]
            }"#,
        )
        .map_err(|err| format!("write expectations failed: {err}"))?;
        let args = RecommendationCalibrationArgs {
            root: dir.clone(),
            pr_guidance: vec![dir.join("missing-comments.json")],
            expectations,
            outcome_receipts: Vec::new(),
            targeted_test_outcome: None,
            agent_receipt: None,
            out: dir.join("recommendation-calibration.json"),
            out_md: dir.join("recommendation-calibration.md"),
        };
        let report = build_recommendation_calibration_report(&args)?;
        assert_eq!(report.status, "incomplete");
        assert_eq!(report.summary.recommendations_evaluated, 0);
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("missing or invalid PR guidance input"))
        );
        Ok(())
    }

    #[test]
    fn recommendation_calibration_json_and_markdown_are_structured() -> Result<(), String> {
        let args = RecommendationCalibrationArgs {
            root: PathBuf::from("."),
            pr_guidance: vec![PathBuf::from(
                "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
            )],
            expectations: PathBuf::from(DEFAULT_EXPECTATIONS),
            outcome_receipts: vec![PathBuf::from(
                "fixtures/boundary_gap/expected/recommendation-calibration/outcome-receipts/useful.json",
            )],
            targeted_test_outcome: None,
            agent_receipt: None,
            out: PathBuf::from(DEFAULT_OUT),
            out_md: PathBuf::from("target/ripr/reports/recommendation-calibration.md"),
        };
        let report = build_recommendation_calibration_report(&args)?;
        let json_text = recommendation_calibration_json(&report)?;
        let value: Value = serde_json::from_str(&json_text)
            .map_err(|err| format!("recommendation calibration JSON should parse: {err}"))?;
        assert_eq!(value["schema_version"], SCHEMA_VERSION);
        assert_eq!(value["status"], "advisory");
        assert_eq!(value["summary"]["top_recommendation_outcome"], "useful");
        assert_eq!(
            value["recommendations"][0]["calibration"]["source"],
            "outcome_receipt:fixture"
        );
        assert!(json_text.contains(LIMITS_NOTE));

        let markdown = recommendation_calibration_markdown(&report);
        assert!(markdown.contains("# Recommendation Calibration"));
        assert!(markdown.contains("Status: advisory"));
        assert!(markdown.contains("Top Recommendation"));
        assert!(markdown.contains("Advisory recommendation-quality evidence only"));
        Ok(())
    }

    #[test]
    fn recommendation_calibration_command_writes_reports() -> Result<(), String> {
        let dir = temp_dir("recommendation-calibration-command")?;
        let out = dir.join("recommendation-calibration.json");
        let out_md = dir.join("recommendation-calibration.md");
        recommendation_calibration(&[
            "--root".to_string(),
            repo_root()?.display().to_string(),
            "--pr-guidance".to_string(),
            "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json".to_string(),
            "--outcome-receipts".to_string(),
            "fixtures/boundary_gap/expected/recommendation-calibration/outcome-receipts/useful.json".to_string(),
            "--out".to_string(),
            out.display().to_string(),
            "--out-md".to_string(),
            out_md.display().to_string(),
        ])?;
        let json_text =
            fs::read_to_string(&out).map_err(|err| format!("read written json failed: {err}"))?;
        let markdown =
            fs::read_to_string(&out_md).map_err(|err| format!("read written md failed: {err}"))?;
        assert!(json_text.contains("\"top_recommendation_outcome\": \"useful\""));
        assert!(markdown.contains("# Recommendation Calibration"));
        Ok(())
    }

    #[test]
    fn recommendation_calibration_fixture_matches_checked_reports() -> Result<(), String> {
        let args = RecommendationCalibrationArgs {
            root: PathBuf::from("."),
            pr_guidance: Vec::new(),
            expectations: PathBuf::from(DEFAULT_EXPECTATIONS),
            outcome_receipts: vec![PathBuf::from(
                "fixtures/boundary_gap/expected/recommendation-calibration/outcome-receipts",
            )],
            targeted_test_outcome: None,
            agent_receipt: None,
            out: PathBuf::from(DEFAULT_OUT),
            out_md: PathBuf::from("target/ripr/reports/recommendation-calibration.md"),
        };
        let report = build_recommendation_calibration_report(&args)?;
        let expected_json = fs::read_to_string(repo_root()?.join(
            "fixtures/boundary_gap/expected/recommendation-calibration/recommendation-calibration.json",
        ))
        .map_err(|err| format!("read checked recommendation calibration JSON: {err}"))?;
        let expected_md = fs::read_to_string(repo_root()?.join(
            "fixtures/boundary_gap/expected/recommendation-calibration/recommendation-calibration.md",
        ))
        .map_err(|err| format!("read checked recommendation calibration Markdown: {err}"))?;
        assert_eq!(recommendation_calibration_json(&report)?, expected_json);
        assert_eq!(recommendation_calibration_markdown(&report), expected_md);
        Ok(())
    }

    fn temp_dir(name: &str) -> Result<PathBuf, String> {
        let mut path = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system time before unix epoch: {err}"))?
            .as_nanos();
        path.push(format!("ripr-{name}-{stamp}"));
        fs::create_dir_all(&path).map_err(|err| format!("create temp dir failed: {err}"))?;
        Ok(path)
    }

    fn repo_root() -> Result<PathBuf, String> {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "xtask should live under repo root".to_string())
    }
}
