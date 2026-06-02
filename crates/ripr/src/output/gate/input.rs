use super::model::*;
use super::{display_path, read_json_value_with_display, resolve_root_path, string_field};
use crate::output::gap_decision_ledger::{self, GapRecord};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn read_labels_impl(
    input: &GateEvaluateInput,
    warnings: &mut Vec<String>,
) -> Vec<String> {
    let mut labels = input
        .labels
        .iter()
        .filter(|label| !label.trim().is_empty())
        .cloned()
        .collect::<BTreeSet<_>>();
    if let Some(path) = &input.labels_json {
        let resolved = resolve_root_path(&input.root, path);
        match read_json_value_with_display(&resolved, path) {
            Ok(value) => {
                for label in labels_from_value(&value) {
                    labels.insert(label);
                }
            }
            Err(error) => warnings.push(format!(
                "optional labels_json {} is unavailable: {error}",
                display_path(path)
            )),
        }
    }
    labels.into_iter().collect()
}

pub(super) fn warn_for_optional_json_impl(
    root: &Path,
    path: Option<&PathBuf>,
    name: &str,
    warnings: &mut Vec<String>,
) {
    let Some(path) = path else {
        return;
    };
    if let Err(error) = read_json_value_with_display(&resolve_root_path(root, path), path) {
        warnings.push(format!(
            "optional {name} {} is unavailable: {error}",
            display_path(path)
        ));
    }
}

pub(super) fn read_gap_ledger_impl(
    input: &GateEvaluateInput,
    config_errors: &mut Vec<String>,
) -> Option<Vec<GapRecord>> {
    let path = input.gap_ledger.as_ref()?;
    let resolved = resolve_root_path(&input.root, path);
    let text = match fs::read_to_string(&resolved) {
        Ok(text) => text,
        Err(error) => {
            config_errors.push(format!(
                "required gap decision ledger input {} is invalid: read failed: {error}",
                display_path(path)
            ));
            return Some(Vec::new());
        }
    };
    match gap_decision_ledger::parse_gap_records_json(&text) {
        Ok(records) => Some(records),
        Err(error) => {
            config_errors.push(format!(
                "required gap decision ledger input {} is invalid: {error}",
                display_path(path)
            ));
            Some(Vec::new())
        }
    }
}

pub(super) fn read_recommendation_calibration_impl(
    input: &GateEvaluateInput,
    warnings: &mut Vec<String>,
) -> CalibrationIndex {
    let mut index = CalibrationIndex::default();
    let Some(path) = &input.recommendation_calibration else {
        return index;
    };
    let resolved = resolve_root_path(&input.root, path);
    let value = match read_json_value_with_display(&resolved, path) {
        Ok(v) => v,
        Err(error) => {
            warnings.push(format!(
                "optional recommendation_calibration {} is unavailable: {error}",
                display_path(path)
            ));
            return index;
        }
    };
    for item in value
        .get("recommendations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let evidence = CalibrationEvidence {
            available: true,
            outcome: string_field(item.pointer("/calibration/outcome")),
            confidence_effect: recommendation_confidence_effect(
                item.pointer("/calibration/outcome").and_then(Value::as_str),
            )
            .to_string(),
        };
        if let Some(id) = item.get("id").and_then(Value::as_str) {
            index.by_source_id.insert(id.to_string(), evidence.clone());
        }
        if let Some(seam_id) = item.get("seam_id").and_then(Value::as_str) {
            index.by_seam_id.insert(seam_id.to_string(), evidence);
        }
    }
    index
}

pub(super) fn read_mutation_calibration_impl(
    input: &GateEvaluateInput,
    warnings: &mut Vec<String>,
) -> CalibrationIndex {
    let mut index = CalibrationIndex::default();
    let Some(path) = &input.mutation_calibration else {
        return index;
    };
    let resolved = resolve_root_path(&input.root, path);
    let value = match read_json_value_with_display(&resolved, path) {
        Ok(v) => v,
        Err(error) => {
            warnings.push(format!(
                "optional mutation_calibration {} is unavailable: {error}",
                display_path(path)
            ));
            return index;
        }
    };
    for item in value
        .get("matches")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let seam_id = item
            .pointer("/static/seam_id")
            .and_then(Value::as_str)
            .or_else(|| item.pointer("/runtime/seam_id").and_then(Value::as_str));
        let Some(seam_id) = seam_id else {
            continue;
        };
        let outcome = item
            .pointer("/runtime/runtime_outcome")
            .and_then(Value::as_str)
            .or_else(|| item.pointer("/runtime/outcome").and_then(Value::as_str));
        index.by_seam_id.insert(
            seam_id.to_string(),
            CalibrationEvidence {
                available: true,
                outcome: outcome.map(ToOwned::to_owned),
                confidence_effect: mutation_confidence_effect(outcome).to_string(),
            },
        );
    }
    for item in value
        .get("static_only_findings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(seam_id) = item.pointer("/static/seam_id").and_then(Value::as_str) {
            index.by_seam_id.insert(
                seam_id.to_string(),
                CalibrationEvidence {
                    available: true,
                    outcome: Some("static_gap_without_runtime_signal".to_string()),
                    confidence_effect: "keeps_advisory".to_string(),
                },
            );
        }
    }
    if !value
        .get("ambiguous_file_line_matches")
        .and_then(Value::as_array)
        .map(|items| items.is_empty())
        .unwrap_or(true)
    {
        warnings.push(format!("mutation_calibration {} contains ambiguous file/line matches; those records do not raise gate confidence", display_path(path)));
    }
    index
}

pub(super) fn read_baseline_impl(
    input: &GateEvaluateInput,
    warnings: &mut Vec<String>,
    config_errors: &mut Vec<String>,
) -> BaselineIndex {
    if input.mode.requires_baseline() && input.baseline.is_none() {
        config_errors.push(format!(
            "{} mode requires an explicit --baseline artifact",
            input.mode.as_str()
        ));
        return BaselineIndex::default();
    }
    let Some(path) = &input.baseline else {
        return BaselineIndex::default();
    };
    let resolved = resolve_root_path(&input.root, path);
    match read_json_value_with_display(&resolved, path) {
        Ok(value) => baseline_index_from_value(&value),
        Err(error) if input.mode.requires_baseline() => {
            config_errors.push(format!(
                "required baseline {} is invalid: {error}",
                display_path(path)
            ));
            BaselineIndex::default()
        }
        Err(error) => {
            warnings.push(format!(
                "optional baseline {} is unavailable: {error}",
                display_path(path)
            ));
            BaselineIndex::default()
        }
    }
}

fn labels_from_value(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_else(|| {
            value
                .get("labels")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToOwned::to_owned)
                        .collect()
                })
                .unwrap_or_default()
        })
}
fn recommendation_confidence_effect(outcome: Option<&str>) -> &'static str {
    match outcome {
        Some("useful" | "summary_only_correct" | "suppressed_correctly") => "supports_static_gap",
        Some("noisy" | "wrong_line" | "wrong_target" | "already_covered") => "keeps_advisory",
        Some(_) => "unknown",
        None => "not_used",
    }
}
fn mutation_confidence_effect(outcome: Option<&str>) -> &'static str {
    let Some(outcome) = outcome else {
        return "not_used";
    };
    if is_runtime_gap_outcome(outcome) {
        "supports_static_gap"
    } else if matches!(
        outcome,
        "caught" | "timeout" | "static_gap_without_runtime_signal"
    ) {
        "keeps_advisory"
    } else {
        "unknown"
    }
}
fn is_runtime_gap_outcome(outcome: &str) -> bool {
    outcome == "missed"
        || outcome == "not_caught"
        || outcome == "uncaught"
        || outcome == format!("{}{}", "sur", "vived")
}
pub(super) fn baseline_index_from_value(value: &Value) -> BaselineIndex {
    let mut index = BaselineIndex::default();
    for item in value
        .get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        collect_identity(&mut index.identities, item.get("canonical_gap_id"));
        collect_identity(
            &mut index.identities,
            item.pointer("/identity/canonical_gap_id"),
        );
        collect_identity(&mut index.identities, item.pointer("/identity/seam_id"));
        collect_identity(&mut index.identities, item.pointer("/identity/source_id"));
        collect_identity(&mut index.identities, item.pointer("/identity/id"));
        collect_identity(&mut index.identities, item.pointer("/identity/dedupe_key"));
        collect_identity(&mut index.identities, item.pointer("/identity/fallback"));
        collect_identity(
            &mut index.identities,
            item.pointer("/evidence_record/canonical_gap_id"),
        );
    }
    for item in value
        .get("decisions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        collect_identity(&mut index.identities, item.get("canonical_gap_id"));
        collect_identity(
            &mut index.identities,
            item.pointer("/identity/canonical_gap_id"),
        );
        collect_identity(
            &mut index.identities,
            item.pointer("/evidence_record/canonical_gap_id"),
        );
        collect_identity(&mut index.identities, item.get("seam_id"));
        collect_identity(&mut index.identities, item.get("source_id"));
    }
    for collection in ["comments", "summary_only", "suppressed"] {
        for item in value
            .get(collection)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            collect_identity(&mut index.identities, item.get("canonical_gap_id"));
            collect_identity(
                &mut index.identities,
                item.pointer("/identity/canonical_gap_id"),
            );
            collect_identity(
                &mut index.identities,
                item.pointer("/evidence_record/canonical_gap_id"),
            );
            collect_identity(&mut index.identities, item.get("seam_id"));
            collect_identity(&mut index.identities, item.get("id"));
            collect_identity(&mut index.identities, item.get("dedupe_key"));
        }
    }
    index
}
fn collect_identity(identities: &mut BTreeSet<String>, value: Option<&Value>) {
    if let Some(text) = value.and_then(Value::as_str).filter(|t| !t.is_empty()) {
        identities.insert(text.to_string());
    }
}
