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
    // RIPR-PROP-0019 decision 5: a ledger disclosing a `limited_partial_scope`
    // producer run is not a valid gate input — fail closed rather than gating
    // on a partial denominator.
    if let Ok(value) = serde_json::from_str::<Value>(&text) {
        if super::discloses_limited_partial_scope(&value) {
            config_errors.push(format!(
                "required gap decision ledger input {} discloses a {} analysis run \
                 (gate_eligibility: {}); a partial denominator is never a gate input",
                display_path(path),
                crate::analysis::PartialDiffScope::RUN_STATUS,
                crate::analysis::PartialDiffScope::GATE_ELIGIBILITY,
            ));
            return Some(Vec::new());
        }
        if super::discloses_incomplete_analysis_outcome(&value) {
            let kind = super::incomplete_analysis_outcome_kind(&value);
            config_errors.push(format!(
                "required gap decision ledger input {} discloses an incomplete analysis \
                 outcome ({kind}); an incomplete denominator is never a gate input",
                display_path(path),
            ));
            return Some(Vec::new());
        }
    }
    match gap_decision_ledger::parse_gap_records_json(&text) {
        Ok(records) => {
            if let Err(errors) = gap_decision_ledger::validate_gap_record_seam_identities(&records)
            {
                for error in errors {
                    config_errors.push(format!(
                        "required gap decision ledger input {} is invalid: {error}",
                        display_path(path)
                    ));
                }
                return Some(Vec::new());
            }
            Some(records)
        }
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
        Ok(value) => {
            // RIPR-PROP-0019 decision 5: a baseline built from a
            // `limited_partial_scope` run is a partial denominator, never a
            // valid gate baseline — fail closed instead of diffing against it.
            if super::discloses_limited_partial_scope(&value) {
                config_errors.push(format!(
                    "baseline {} discloses a {} analysis run (gate_eligibility: {}); \
                     a partial denominator is never a baseline input",
                    display_path(path),
                    crate::analysis::PartialDiffScope::RUN_STATUS,
                    crate::analysis::PartialDiffScope::GATE_ELIGIBILITY,
                ));
                return BaselineIndex::default();
            }
            if super::discloses_incomplete_analysis_outcome(&value) {
                let kind = super::incomplete_analysis_outcome_kind(&value);
                config_errors.push(format!(
                    "baseline {} discloses an incomplete analysis outcome ({kind}); \
                     an incomplete denominator is never a baseline input",
                    display_path(path),
                ));
                return BaselineIndex::default();
            }
            if let Some(defect) = baseline_document_defect(&value) {
                config_errors.push(format!(
                    "baseline {} is not a recognized gate baseline: {defect}",
                    display_path(path),
                ));
                return BaselineIndex::default();
            }
            baseline_index_from_value(&value)
        }
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
    outcome == "missed" || outcome == "not_caught" || outcome == "uncaught" || outcome == "survived" // ripr-allow: static-language: runtime mutation-calibration import vocabulary, not static output
}
/// Kind marker `ripr baseline create` writes on a gate baseline ledger.
const GATE_BASELINE_KIND: &str = "gate_baseline";

/// Identity-bearing arrays the gate indexes from a baseline file. `entries`
/// is the `ripr baseline create` ledger shape; `decisions`, `comments`,
/// `summary_only`, and `suppressed` are the documented compatibility shapes
/// for reviewed hand-built baselines (docs/CI.md, "Gate baseline workflow").
const BASELINE_IDENTITY_ARRAYS: [&str; 5] = [
    "entries",
    "decisions",
    "comments",
    "summary_only",
    "suppressed",
];

/// Returns `Some(defect)` when `value` is not a baseline the gate can index:
/// not a JSON object, a `kind` other than
/// `gate_baseline` (or a `gate_baseline` without an `entries` array), or no
/// identity-bearing array at all. Such a file must not silently act as an
/// empty baseline (every finding new) or as an unrelated document whose ids
/// happen to be harvested. `schema_version` is not required because the
/// documented hand-built compatibility baselines may omit it.
fn baseline_document_defect(value: &Value) -> Option<String> {
    let Some(object) = value.as_object() else {
        return Some("expected a JSON object".to_string());
    };
    match object.get("kind") {
        Some(Value::String(kind)) if kind == GATE_BASELINE_KIND => {
            return if object.get("entries").is_some_and(Value::is_array) {
                None
            } else {
                Some(format!(
                    "`kind: {GATE_BASELINE_KIND}` requires an `entries` array"
                ))
            };
        }
        Some(other) => {
            return Some(format!(
                "field `kind` is {other}, expected \"{GATE_BASELINE_KIND}\""
            ));
        }
        None => {}
    }
    if BASELINE_IDENTITY_ARRAYS
        .iter()
        .any(|field| object.get(*field).is_some_and(Value::is_array))
    {
        None
    } else {
        Some(format!(
            "no identity array found (expected `kind: {GATE_BASELINE_KIND}` with `entries`, or one of {})",
            BASELINE_IDENTITY_ARRAYS
                .iter()
                .map(|field| format!("`{field}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
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
