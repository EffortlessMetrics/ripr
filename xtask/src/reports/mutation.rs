//! Mutation calibration command (`mutation-calibration`): joins static
//! repo-exposure seam evidence to imported cargo-mutants runtime outcome data
//! and renders the advisory calibration report, plus its exclusive argument
//! parsing, runtime JSON import, seam join, and JSON/markdown rendering
//! helpers.
//!
//! Extracted verbatim from `main.rs` as a behavior-preserving decomposition
//! slice of #2119. Items are `pub(crate)` where `tests.rs` or `dispatch.rs`
//! need them so existing call sites compile unchanged.

use crate::run::run_output_owned;
use crate::{
    StaticSeamRecord, json_scalar_as_string, json_scalar_as_usize, markdown_cell, normalize_path,
    normalize_report_path, parse_repo_exposure_static_seams, read_json_value, read_text_lossy,
    repo_seam_inventory_command_args_for_root, write_report,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct MutationCalibrationArgs {
    pub(crate) root: String,
    pub(crate) mutants_json: PathBuf,
    pub(crate) repo_exposure_json: Option<PathBuf>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MutationOutcomeRecord {
    pub(crate) mutant_id: Option<String>,
    pub(crate) seam_id: Option<String>,
    pub(crate) file: Option<String>,
    pub(crate) line: Option<usize>,
    pub(crate) mutation_operator: String,
    pub(crate) runtime_outcome: String,
    pub(crate) duration: Option<String>,
    pub(crate) test_command: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MutationCalibrationReport {
    pub(crate) static_seams_total: usize,
    pub(crate) mutants_total: usize,
    pub(crate) agreement: MutationCalibrationAgreement,
    pub(crate) precision_notes: Vec<String>,
    pub(crate) missed_runtime_signals: Vec<MutationCalibrationRuntimeSignal>,
    pub(crate) static_only_findings: Vec<MutationCalibrationStaticOnlyFinding>,
    pub(crate) matched: Vec<MutationCalibrationMatch>,
    pub(crate) ambiguous_file_line: Vec<AmbiguousMutationCalibrationMatch>,
    pub(crate) unmatched_mutants: Vec<MutationOutcomeRecord>,
    pub(crate) static_without_runtime: Vec<StaticSeamRecord>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct MutationCalibrationAgreement {
    pub(crate) static_gap_and_runtime_signal: usize,
    pub(crate) static_gap_without_runtime_signal: usize,
    pub(crate) runtime_signal_without_static_gap: usize,
    pub(crate) static_clean_and_runtime_clean: usize,
    pub(crate) runtime_inconclusive: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MutationCalibrationRuntimeSignal {
    pub(crate) runtime: MutationOutcomeRecord,
    pub(crate) static_seam: Option<StaticSeamRecord>,
    pub(crate) confidence_label: &'static str,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MutationCalibrationStaticOnlyFinding {
    pub(crate) seam: StaticSeamRecord,
    pub(crate) confidence_label: &'static str,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MutationCalibrationMatch {
    pub(crate) join_method: &'static str,
    pub(crate) seam: StaticSeamRecord,
    pub(crate) mutation: MutationOutcomeRecord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AmbiguousMutationCalibrationMatch {
    pub(crate) mutation: MutationOutcomeRecord,
    pub(crate) candidates: Vec<StaticSeamRecord>,
}

pub(crate) const MUTATION_CALIBRATION_STATIC_WITHOUT_RUNTIME_SAMPLE_LIMIT: usize = 50;
const MUTATION_CALIBRATION_AGREEMENT_SAMPLE_LIMIT: usize = 50;
pub(crate) fn mutation_calibration_impl(args: &[String]) -> Result<(), String> {
    let parsed = parse_mutation_calibration_args(args)?;
    let repo_exposure_json = match parsed.repo_exposure_json.as_ref() {
        Some(path) => read_text_lossy(path)?,
        None => {
            let json_args =
                repo_seam_inventory_command_args_for_root("repo-exposure-json", &parsed.root);
            let json_output = run_output_owned("cargo", &json_args)?;
            write_report("repo-exposure.json", &json_output)?;
            json_output
        }
    };
    let mutants_json = read_mutation_input_json(&parsed.mutants_json)?;
    let static_seams = parse_repo_exposure_static_seams(&repo_exposure_json)?;
    let runtime_mutants = parse_mutation_outcomes_json(&mutants_json)?;
    let report = build_mutation_calibration_report(static_seams, runtime_mutants);
    write_report(
        "mutation-calibration.json",
        &mutation_calibration_report_json(&report)?,
    )?;
    write_report(
        "mutation-calibration.md",
        &mutation_calibration_report_markdown(&report),
    )
}

pub(crate) fn parse_mutation_calibration_args(
    args: &[String],
) -> Result<MutationCalibrationArgs, String> {
    let mut root: Option<String> = None;
    let mut mutants_json: Option<PathBuf> = None;
    let mut repo_exposure_json: Option<PathBuf> = None;
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--mutants-json" | "--cargo-mutants-json" | "--input" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        mutation_calibration_usage()
                    ));
                };
                mutants_json = Some(PathBuf::from(path));
            }
            "--repo-exposure-json" | "--static-json" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err(format!(
                        "missing value for `{arg}`\n{}",
                        mutation_calibration_usage()
                    ));
                };
                repo_exposure_json = Some(PathBuf::from(path));
            }
            "--help" | "-h" => return Err(mutation_calibration_usage()),
            flag if flag.starts_with('-') => {
                return Err(format!(
                    "unknown mutation-calibration option `{flag}`\n{}",
                    mutation_calibration_usage()
                ));
            }
            positional => {
                if root.is_some() {
                    return Err(format!(
                        "unexpected extra positional argument `{positional}`\n{}",
                        mutation_calibration_usage()
                    ));
                }
                root = Some(positional.to_string());
            }
        }
        index += 1;
    }

    let Some(mutants_json) = mutants_json else {
        return Err(format!(
            "mutation-calibration requires `--mutants-json <path>`\n{}",
            mutation_calibration_usage()
        ));
    };

    Ok(MutationCalibrationArgs {
        root: root.unwrap_or_else(|| ".".to_string()),
        mutants_json,
        repo_exposure_json,
    })
}

fn mutation_calibration_usage() -> String {
    "usage: cargo xtask mutation-calibration [root] --mutants-json <path> [--repo-exposure-json <path>]"
        .to_string()
}
pub(crate) fn read_mutation_input_json(path: &Path) -> Result<String, String> {
    if path.is_dir() {
        let outcomes_path = path.join("outcomes.json");
        let mutants_path = path.join("mutants.json");
        let outcomes_exists = outcomes_path.exists();
        let mutants_exists = mutants_path.exists();

        if outcomes_exists && mutants_exists {
            let outcomes = read_json_value(&outcomes_path)?;
            let mutants = read_json_value(&mutants_path)?;
            return serde_json::to_string(&Value::Array(vec![outcomes, mutants]))
                .map_err(|err| format!("failed to combine cargo-mutants directory JSON: {err}"));
        }

        if outcomes_exists {
            return read_text_lossy(&outcomes_path);
        }
        if mutants_exists {
            return read_text_lossy(&mutants_path);
        }
        return Err(format!(
            "{} is a directory but contains neither outcomes.json nor mutants.json",
            normalize_path(path)
        ));
    }
    read_text_lossy(path)
}
pub(crate) fn parse_mutation_outcomes_json(
    json: &str,
) -> Result<Vec<MutationOutcomeRecord>, String> {
    let value: Value = serde_json::from_str(json)
        .map_err(|err| format!("failed to parse cargo-mutants JSON: {err}"))?;
    let mut records = Vec::new();
    collect_mutation_outcome_records(&value, &mut records);
    let mut records = merge_mutation_outcome_records(records);
    records.sort_by(|left, right| {
        left.seam_id
            .cmp(&right.seam_id)
            .then(left.file.cmp(&right.file))
            .then(left.line.cmp(&right.line))
            .then(left.mutation_operator.cmp(&right.mutation_operator))
            .then(left.runtime_outcome.cmp(&right.runtime_outcome))
    });
    Ok(records)
}

fn collect_mutation_outcome_records(value: &Value, records: &mut Vec<MutationOutcomeRecord>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_mutation_outcome_records(item, records);
            }
        }
        Value::Object(object) => {
            for key in [
                "outcomes",
                "mutants",
                "results",
                "mutations",
                "mutation_results",
            ] {
                if let Some(items) = object.get(key).and_then(Value::as_array) {
                    for item in items {
                        collect_mutation_outcome_records(item, records);
                    }
                }
            }
            if let Some(record) = mutation_outcome_record_from_object(object) {
                records.push(record);
            }
        }
        _ => {}
    }
}

fn mutation_outcome_record_from_object(
    object: &serde_json::Map<String, Value>,
) -> Option<MutationOutcomeRecord> {
    let mutant = nested_object(object, "mutant");
    let mutation = nested_object(object, "mutation");
    let location = nested_object(object, "location");
    let span = nested_object(object, "span")
        .or_else(|| mutant.and_then(|nested| nested_object(nested, "span")))
        .or_else(|| mutation.and_then(|nested| nested_object(nested, "span")))
        .or_else(|| location.and_then(|nested| nested_object(nested, "span")));

    let mutant_id = string_field_any(object, &["id", "mutant_id", "mutantId"]).or_else(|| {
        mutant.and_then(|nested| string_field_any(nested, &["id", "mutant_id", "mutantId"]))
    });
    let seam_id = string_field_any(object, &["seam_id", "seamId", "probe_id", "probeId"])
        .or_else(|| {
            mutant.and_then(|nested| {
                string_field_any(nested, &["seam_id", "seamId", "probe_id", "probeId"])
            })
        })
        .or_else(|| {
            mutation.and_then(|nested| {
                string_field_any(nested, &["seam_id", "seamId", "probe_id", "probeId"])
            })
        });
    let file = string_field_any(
        object,
        &["file", "path", "source_file", "src_file", "filename"],
    )
    .or_else(|| {
        mutant.and_then(|nested| {
            string_field_any(
                nested,
                &["file", "path", "source_file", "src_file", "filename"],
            )
        })
    })
    .or_else(|| {
        mutation.and_then(|nested| {
            string_field_any(
                nested,
                &["file", "path", "source_file", "src_file", "filename"],
            )
        })
    })
    .or_else(|| {
        location.and_then(|nested| {
            string_field_any(
                nested,
                &[
                    "file",
                    "path",
                    "source_file",
                    "src_file",
                    "filename",
                    "file_name",
                ],
            )
        })
    })
    .or_else(|| {
        span.and_then(|nested| {
            string_field_any(
                nested,
                &[
                    "file",
                    "path",
                    "source_file",
                    "src_file",
                    "filename",
                    "file_name",
                ],
            )
        })
    })
    .map(|path| normalize_report_path(&path));
    let line = usize_field_any(object, &["line", "line_start", "start_line", "startLine"])
        .or_else(|| {
            mutant.and_then(|nested| {
                usize_field_any(nested, &["line", "line_start", "start_line", "startLine"])
            })
        })
        .or_else(|| {
            mutation.and_then(|nested| {
                usize_field_any(nested, &["line", "line_start", "start_line", "startLine"])
            })
        })
        .or_else(|| {
            location.and_then(|nested| {
                usize_field_any(nested, &["line", "line_start", "start_line", "startLine"])
            })
        })
        .or_else(|| span.and_then(span_start_line));
    let mutation_operator = string_field_any(
        object,
        &[
            "operator",
            "mutation_operator",
            "mutator",
            "mutation",
            "description",
            "replacement",
            "name",
        ],
    )
    .or_else(|| {
        mutant.and_then(|nested| {
            string_field_any(
                nested,
                &[
                    "operator",
                    "mutation_operator",
                    "mutator",
                    "mutation",
                    "description",
                    "replacement",
                    "name",
                ],
            )
        })
    })
    .or_else(|| {
        mutation.and_then(|nested| {
            string_field_any(
                nested,
                &[
                    "operator",
                    "mutation_operator",
                    "mutator",
                    "mutation",
                    "description",
                    "replacement",
                    "name",
                ],
            )
        })
    })
    .unwrap_or_else(|| "unknown".to_string());
    let runtime_outcome =
        string_field_any(object, &["outcome", "status", "result", "summary", "state"])
            .unwrap_or_else(|| "unknown".to_string());
    let duration = string_field_any(
        object,
        &[
            "duration_ms",
            "durationMillis",
            "duration",
            "elapsed_ms",
            "elapsed",
        ],
    );
    let test_command = string_field_any(
        object,
        &["test_command", "testCommand", "command", "cmd", "test_cmd"],
    );

    let has_identity = mutant_id.is_some() || seam_id.is_some() || file.is_some() || line.is_some();
    let has_runtime_detail = runtime_outcome != "unknown"
        || mutation_operator != "unknown"
        || duration.is_some()
        || test_command.is_some();
    if !has_identity || !has_runtime_detail {
        return None;
    }

    Some(MutationOutcomeRecord {
        mutant_id,
        seam_id,
        file,
        line,
        mutation_operator,
        runtime_outcome,
        duration,
        test_command,
    })
}

fn nested_object<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Option<&'a serde_json::Map<String, Value>> {
    object.get(key).and_then(Value::as_object)
}

fn span_start_line(span: &serde_json::Map<String, Value>) -> Option<usize> {
    usize_field_any(span, &["line", "line_start", "start_line", "startLine"])
        .or_else(|| {
            nested_object(span, "start").and_then(|start| {
                usize_field_any(start, &["line", "line_start", "start_line", "startLine"])
            })
        })
        .or_else(|| {
            nested_object(span, "start_position").and_then(|start| {
                usize_field_any(start, &["line", "line_start", "start_line", "startLine"])
            })
        })
        .or_else(|| {
            nested_object(span, "lo").and_then(|start| {
                usize_field_any(start, &["line", "line_start", "start_line", "startLine"])
            })
        })
}

fn string_field_any(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(json_scalar_as_string))
        .filter(|value| !value.trim().is_empty())
}

fn usize_field_any(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<usize> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(json_scalar_as_usize))
}
pub(crate) fn build_mutation_calibration_report(
    static_seams: Vec<StaticSeamRecord>,
    runtime_mutants: Vec<MutationOutcomeRecord>,
) -> MutationCalibrationReport {
    let mut static_by_id: BTreeMap<String, usize> = BTreeMap::new();
    let mut static_by_line: BTreeMap<(String, usize), Vec<usize>> = BTreeMap::new();
    for (idx, seam) in static_seams.iter().enumerate() {
        static_by_id.insert(seam.seam_id.clone(), idx);
        static_by_line
            .entry((normalize_report_path(&seam.file), seam.line))
            .or_default()
            .push(idx);
    }

    let mut matched_static_ids = BTreeSet::new();
    let mut ambiguous_static_ids = BTreeSet::new();
    let mut matched = Vec::new();
    let mut ambiguous_file_line = Vec::new();
    let mut unmatched_mutants = Vec::new();

    for mutation in runtime_mutants {
        let seam_match = mutation
            .seam_id
            .as_ref()
            .and_then(|seam_id| static_by_id.get(seam_id).copied())
            .map(|idx| ("seam_id", idx))
            .or_else(|| {
                let file = mutation.file.as_ref()?;
                let line = mutation.line?;
                let key = (normalize_report_path(file), line);
                let candidates = static_by_line.get(&key)?;
                (candidates.len() == 1).then_some(("file_line", candidates[0]))
            });

        match seam_match {
            Some((join_method, idx)) => {
                let seam = static_seams[idx].clone();
                matched_static_ids.insert(seam.seam_id.clone());
                matched.push(MutationCalibrationMatch {
                    join_method,
                    seam,
                    mutation,
                });
            }
            None => {
                let candidates = mutation
                    .file
                    .as_ref()
                    .and_then(|file| {
                        let line = mutation.line?;
                        let key = (normalize_report_path(file), line);
                        static_by_line.get(&key)
                    })
                    .filter(|candidates| candidates.len() > 1);

                if let Some(candidates) = candidates {
                    let candidates = candidates
                        .iter()
                        .map(|idx| {
                            let seam = static_seams[*idx].clone();
                            ambiguous_static_ids.insert(seam.seam_id.clone());
                            seam
                        })
                        .collect::<Vec<_>>();
                    ambiguous_file_line.push(AmbiguousMutationCalibrationMatch {
                        mutation,
                        candidates,
                    });
                } else {
                    unmatched_mutants.push(mutation);
                }
            }
        }
    }

    let static_without_runtime = static_seams
        .iter()
        .filter(|seam| {
            !matched_static_ids.contains(&seam.seam_id)
                && !ambiguous_static_ids.contains(&seam.seam_id)
        })
        .cloned()
        .collect::<Vec<_>>();

    let (agreement, precision_notes, missed_runtime_signals, static_only_findings) =
        mutation_calibration_agreement(
            &static_seams,
            &matched,
            &ambiguous_file_line,
            &unmatched_mutants,
        );

    MutationCalibrationReport {
        static_seams_total: static_seams.len(),
        mutants_total: matched.len() + ambiguous_file_line.len() + unmatched_mutants.len(),
        agreement,
        precision_notes,
        missed_runtime_signals,
        static_only_findings,
        matched,
        ambiguous_file_line,
        unmatched_mutants,
        static_without_runtime,
    }
}

fn mutation_calibration_agreement(
    static_seams: &[StaticSeamRecord],
    matched: &[MutationCalibrationMatch],
    ambiguous_file_line: &[AmbiguousMutationCalibrationMatch],
    unmatched_mutants: &[MutationOutcomeRecord],
) -> (
    MutationCalibrationAgreement,
    Vec<String>,
    Vec<MutationCalibrationRuntimeSignal>,
    Vec<MutationCalibrationStaticOnlyFinding>,
) {
    let mut matches_by_seam: BTreeMap<&str, Vec<&MutationCalibrationMatch>> = BTreeMap::new();
    for record in matched {
        matches_by_seam
            .entry(record.seam.seam_id.as_str())
            .or_default()
            .push(record);
    }

    let mut agreement = MutationCalibrationAgreement::default();
    let mut missed_runtime_signals = Vec::new();
    let mut static_only_findings = Vec::new();

    for seam in static_seams {
        let records = matches_by_seam
            .get(seam.seam_id.as_str())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let has_runtime_gap = records
            .iter()
            .any(|record| runtime_gap_signal(&record.mutation.runtime_outcome));
        let has_runtime_clean = records
            .iter()
            .any(|record| runtime_clean_signal(&record.mutation.runtime_outcome));
        let has_runtime_inconclusive = records.iter().any(|record| {
            !runtime_gap_signal(&record.mutation.runtime_outcome)
                && !runtime_clean_signal(&record.mutation.runtime_outcome)
        });
        let has_static_gap = static_gap_signal(seam);

        match (has_static_gap, has_runtime_gap, has_runtime_clean) {
            (true, true, _) => agreement.static_gap_and_runtime_signal += 1,
            (true, false, _) => {
                agreement.static_gap_without_runtime_signal += 1;
                static_only_findings.push(MutationCalibrationStaticOnlyFinding {
                    seam: seam.clone(),
                    confidence_label: static_only_confidence_label(records),
                    reason: static_only_reason(records),
                });
            }
            (false, true, _) => {
                agreement.runtime_signal_without_static_gap += 1;
                for record in records
                    .iter()
                    .filter(|record| runtime_gap_signal(&record.mutation.runtime_outcome))
                {
                    missed_runtime_signals.push(MutationCalibrationRuntimeSignal {
                        runtime: record.mutation.clone(),
                        static_seam: Some(seam.clone()),
                        confidence_label: "contradicts_static_clean",
                        reason: "runtime gap signal joined to a static-clean seam".to_string(),
                    });
                }
            }
            (false, false, true) => agreement.static_clean_and_runtime_clean += 1,
            (false, false, false) => {}
        }

        if has_runtime_inconclusive {
            agreement.runtime_inconclusive += 1;
        }
    }

    for record in unmatched_mutants
        .iter()
        .filter(|record| runtime_gap_signal(&record.runtime_outcome))
    {
        agreement.runtime_signal_without_static_gap += 1;
        missed_runtime_signals.push(MutationCalibrationRuntimeSignal {
            runtime: record.clone(),
            static_seam: None,
            confidence_label: "runtime_only_signal",
            reason: "runtime gap signal did not join to a static seam".to_string(),
        });
    }

    for record in ambiguous_file_line {
        if runtime_gap_signal(&record.mutation.runtime_outcome) {
            agreement.runtime_inconclusive += 1;
        }
    }

    missed_runtime_signals.truncate(MUTATION_CALIBRATION_AGREEMENT_SAMPLE_LIMIT);
    static_only_findings.truncate(MUTATION_CALIBRATION_AGREEMENT_SAMPLE_LIMIT);

    (
        agreement,
        mutation_calibration_precision_notes(),
        missed_runtime_signals,
        static_only_findings,
    )
}

fn mutation_calibration_precision_notes() -> Vec<String> {
    vec![
        "runtime gap signals are imported runtime labels such as missed, survived, not_caught, or uncaught".to_string(),
        "runtime clean signals are imported runtime labels such as caught or timeout".to_string(),
        "static_gap_without_runtime_signal includes static gap seams with no matched runtime gap signal in this import".to_string(),
        "ambiguous file/line runtime gap signals are counted as runtime_inconclusive until a seam_id or unambiguous location is available".to_string(),
    ]
}

fn static_only_reason(records: &[&MutationCalibrationMatch]) -> String {
    if records.is_empty() {
        "static gap seam has no matched runtime record in this import".to_string()
    } else if records
        .iter()
        .any(|record| runtime_clean_signal(&record.mutation.runtime_outcome))
    {
        "static gap seam matched runtime data without a runtime gap signal".to_string()
    } else {
        "static gap seam matched only runtime-inconclusive labels".to_string()
    }
}

fn static_only_confidence_label(records: &[&MutationCalibrationMatch]) -> &'static str {
    if records
        .iter()
        .any(|record| runtime_clean_signal(&record.mutation.runtime_outcome))
    {
        "contradicts_static_gap"
    } else {
        "no_runtime_data"
    }
}

fn static_gap_signal(seam: &StaticSeamRecord) -> bool {
    !matches!(
        seam.seam_grip_class.as_str(),
        "strongly_gripped" | "intentional" | "suppressed"
    )
}

fn runtime_gap_signal(outcome: &str) -> bool {
    matches!(
        normalize_runtime_label(outcome).as_str(),
        "missed" | "survived" | "survive" | "not_caught" | "uncaught"
    )
}

fn runtime_clean_signal(outcome: &str) -> bool {
    matches!(
        normalize_runtime_label(outcome).as_str(),
        "caught" | "timeout" | "timed_out" | "killed"
    )
}

fn confidence_label_for_match(record: &MutationCalibrationMatch) -> &'static str {
    let has_static_gap = static_gap_signal(&record.seam);
    if runtime_gap_signal(&record.mutation.runtime_outcome) {
        if has_static_gap {
            "supports_static_gap"
        } else {
            "contradicts_static_clean"
        }
    } else if runtime_clean_signal(&record.mutation.runtime_outcome) {
        if has_static_gap {
            "contradicts_static_gap"
        } else {
            "supports_static_clean"
        }
    } else {
        "no_runtime_data"
    }
}

pub(crate) fn mutation_calibration_report_json(
    report: &MutationCalibrationReport,
) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": "0.1",
        "scope": "repo",
        "status": "advisory",
        "metrics": {
            "static_seams_total": report.static_seams_total,
            "mutants_total": report.mutants_total,
            "matched_total": report.matched.len(),
            "ambiguous_file_line_total": report.ambiguous_file_line.len(),
            "unmatched_mutants_total": report.unmatched_mutants.len(),
            "static_without_runtime_total": report.static_without_runtime.len(),
            "runtime_outcome_counts": runtime_outcome_counts(report),
            "join_method_counts": join_method_counts(report),
        },
        "agreement": mutation_calibration_agreement_json(&report.agreement),
        "precision_notes": &report.precision_notes,
        "missed_runtime_signals": report
            .missed_runtime_signals
            .iter()
            .map(mutation_calibration_runtime_signal_json)
            .collect::<Vec<_>>(),
        "static_only_findings": report
            .static_only_findings
            .iter()
            .map(mutation_calibration_static_only_json)
            .collect::<Vec<_>>(),
        "matches": report
            .matched
            .iter()
            .map(mutation_calibration_match_json)
            .collect::<Vec<_>>(),
        "ambiguous_file_line_matches": report
            .ambiguous_file_line
            .iter()
            .map(ambiguous_mutation_calibration_match_json)
            .collect::<Vec<_>>(),
        "unmatched_mutants": report
            .unmatched_mutants
            .iter()
            .map(mutation_outcome_json)
            .collect::<Vec<_>>(),
        "static_without_runtime_sample": report
            .static_without_runtime
            .iter()
            .take(MUTATION_CALIBRATION_STATIC_WITHOUT_RUNTIME_SAMPLE_LIMIT)
            .map(static_seam_json)
            .collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&value)
        .map(|mut json| {
            json.push('\n');
            json
        })
        .map_err(|err| format!("failed to render mutation calibration JSON: {err}"))
}

fn mutation_calibration_agreement_json(agreement: &MutationCalibrationAgreement) -> Value {
    serde_json::json!({
        "static_gap_and_runtime_signal": agreement.static_gap_and_runtime_signal,
        "static_gap_without_runtime_signal": agreement.static_gap_without_runtime_signal,
        "runtime_signal_without_static_gap": agreement.runtime_signal_without_static_gap,
        "static_clean_and_runtime_clean": agreement.static_clean_and_runtime_clean,
        "runtime_inconclusive": agreement.runtime_inconclusive,
    })
}

fn mutation_calibration_runtime_signal_json(record: &MutationCalibrationRuntimeSignal) -> Value {
    serde_json::json!({
        "runtime": mutation_outcome_json(&record.runtime),
        "static": record.static_seam.as_ref().map(static_seam_json),
        "confidence_label": record.confidence_label,
        "reason": record.reason.as_str(),
    })
}

fn mutation_calibration_static_only_json(record: &MutationCalibrationStaticOnlyFinding) -> Value {
    serde_json::json!({
        "static": static_seam_json(&record.seam),
        "confidence_label": record.confidence_label,
        "reason": record.reason.as_str(),
    })
}

fn mutation_calibration_match_json(record: &MutationCalibrationMatch) -> Value {
    serde_json::json!({
        "join_method": record.join_method,
        "static": static_seam_json(&record.seam),
        "runtime": mutation_outcome_json(&record.mutation),
        "confidence_label": confidence_label_for_match(record),
    })
}

fn ambiguous_mutation_calibration_match_json(record: &AmbiguousMutationCalibrationMatch) -> Value {
    serde_json::json!({
        "runtime": mutation_outcome_json(&record.mutation),
        "confidence_label": "ambiguous_runtime_join",
        "candidates": record
            .candidates
            .iter()
            .map(static_seam_json)
            .collect::<Vec<_>>(),
    })
}

fn static_seam_json(record: &StaticSeamRecord) -> Value {
    serde_json::json!({
        "seam_id": record.seam_id.as_str(),
        "seam_kind": record.seam_kind.as_str(),
        "file": record.file.as_str(),
        "line": record.line,
        "seam_grip_class": record.seam_grip_class.as_str(),
        "oracle_kind": record.oracle_kind.as_str(),
        "oracle_strength": record.oracle_strength.as_str(),
        "observed_values": &record.observed_values,
        "missing_discriminators": &record.missing_discriminators,
    })
}

fn mutation_outcome_json(record: &MutationOutcomeRecord) -> Value {
    serde_json::json!({
        "mutant_id": record.mutant_id.as_deref(),
        "seam_id": record.seam_id.as_deref(),
        "file": record.file.as_deref(),
        "line": record.line,
        "mutation_operator": record.mutation_operator.as_str(),
        "runtime_outcome": record.runtime_outcome.as_str(),
        "duration": record.duration.as_deref(),
        "test_command": record.test_command.as_deref(),
    })
}

fn merge_mutation_outcome_records(
    records: Vec<MutationOutcomeRecord>,
) -> Vec<MutationOutcomeRecord> {
    let mut by_id: BTreeMap<String, MutationOutcomeRecord> = BTreeMap::new();
    let mut without_id = Vec::new();

    for record in records {
        match record.mutant_id.clone() {
            Some(id) => {
                if let Some(existing) = by_id.get_mut(&id) {
                    merge_mutation_outcome_record(existing, record);
                } else {
                    by_id.insert(id, record);
                }
            }
            None => without_id.push(record),
        }
    }

    by_id.into_values().chain(without_id).collect::<Vec<_>>()
}

fn merge_mutation_outcome_record(
    target: &mut MutationOutcomeRecord,
    source: MutationOutcomeRecord,
) {
    if target.seam_id.is_none() {
        target.seam_id = source.seam_id;
    }
    if target.file.is_none() {
        target.file = source.file;
    }
    if target.line.is_none() {
        target.line = source.line;
    }
    if target.mutation_operator == "unknown" && source.mutation_operator != "unknown" {
        target.mutation_operator = source.mutation_operator;
    }
    if target.runtime_outcome == "unknown" && source.runtime_outcome != "unknown" {
        target.runtime_outcome = source.runtime_outcome;
    }
    if target.duration.is_none() {
        target.duration = source.duration;
    }
    if target.test_command.is_none() {
        target.test_command = source.test_command;
    }
}

fn runtime_outcome_counts(report: &MutationCalibrationReport) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for record in report
        .matched
        .iter()
        .map(|matched| &matched.mutation)
        .chain(
            report
                .ambiguous_file_line
                .iter()
                .map(|ambiguous| &ambiguous.mutation),
        )
        .chain(report.unmatched_mutants.iter())
    {
        let key = normalize_runtime_label(&record.runtime_outcome);
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

fn join_method_counts(report: &MutationCalibrationReport) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for record in &report.matched {
        *counts.entry(record.join_method).or_insert(0) += 1;
    }
    counts
}

fn normalize_runtime_label(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

pub(crate) fn mutation_calibration_report_markdown(report: &MutationCalibrationReport) -> String {
    let mut out = String::new();
    out.push_str("# ripr mutation calibration report\n\n");
    out.push_str("Status: advisory\n\n");
    out.push_str(
        "This report joins static seam evidence to supplied cargo-mutants runtime data. \
         Runtime outcome vocabulary in this report comes from that runtime data; static \
         ripr reports continue to use audit vocabulary only.\n\n",
    );
    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Count |\n| --- | ---: |\n");
    out.push_str(&format!(
        "| static_seams_total | {} |\n",
        report.static_seams_total
    ));
    out.push_str(&format!("| mutants_total | {} |\n", report.mutants_total));
    out.push_str(&format!("| matched_total | {} |\n", report.matched.len()));
    out.push_str(&format!(
        "| ambiguous_file_line_total | {} |\n",
        report.ambiguous_file_line.len()
    ));
    out.push_str(&format!(
        "| unmatched_mutants_total | {} |\n",
        report.unmatched_mutants.len()
    ));
    out.push_str(&format!(
        "| static_without_runtime_total | {} |\n",
        report.static_without_runtime.len()
    ));

    out.push_str("\n## Static/runtime agreement\n\n");
    out.push_str("| Agreement bucket | Count |\n| --- | ---: |\n");
    out.push_str(&format!(
        "| static_gap_and_runtime_signal | {} |\n",
        report.agreement.static_gap_and_runtime_signal
    ));
    out.push_str(&format!(
        "| static_gap_without_runtime_signal | {} |\n",
        report.agreement.static_gap_without_runtime_signal
    ));
    out.push_str(&format!(
        "| runtime_signal_without_static_gap | {} |\n",
        report.agreement.runtime_signal_without_static_gap
    ));
    out.push_str(&format!(
        "| static_clean_and_runtime_clean | {} |\n",
        report.agreement.static_clean_and_runtime_clean
    ));
    out.push_str(&format!(
        "| runtime_inconclusive | {} |\n",
        report.agreement.runtime_inconclusive
    ));

    out.push_str("\nPrecision notes:\n\n");
    for note in &report.precision_notes {
        out.push_str(&format!("- {}\n", markdown_cell(note)));
    }

    out.push_str("\n### Runtime signals without static gaps\n\n");
    if report.missed_runtime_signals.is_empty() {
        out.push_str("No imported runtime gap signals lacked a matching static gap.\n");
    } else {
        out.push_str("| Runtime mutant | Location | Runtime outcome | Static class | Confidence label | Reason |\n");
        out.push_str("| --- | --- | --- | --- | --- | --- |\n");
        for record in &report.missed_runtime_signals {
            let mutant = record.runtime.mutant_id.as_deref().unwrap_or("unknown");
            let location = mutation_location_label(&record.runtime);
            let static_class = record
                .static_seam
                .as_ref()
                .map(|seam| seam.seam_grip_class.as_str())
                .unwrap_or("unmatched");
            out.push_str(&format!(
                "| `{}` | {} | {} | `{}` | `{}` | {} |\n",
                markdown_cell(mutant),
                markdown_cell(&location),
                markdown_cell(&record.runtime.runtime_outcome),
                markdown_cell(static_class),
                record.confidence_label,
                markdown_cell(&record.reason)
            ));
        }
    }

    out.push_str("\n### Static gaps without runtime signals\n\n");
    if report.static_only_findings.is_empty() {
        out.push_str("No static gap seams lacked a runtime gap signal in this import.\n");
    } else {
        out.push_str("| Seam | Class | Location | Confidence label | Reason |\n");
        out.push_str("| --- | --- | --- | --- | --- |\n");
        for record in &report.static_only_findings {
            let location = format!("{}:{}", record.seam.file, record.seam.line);
            out.push_str(&format!(
                "| `{}` | `{}` | {} | `{}` | {} |\n",
                markdown_cell(&record.seam.seam_id),
                markdown_cell(&record.seam.seam_grip_class),
                markdown_cell(&location),
                record.confidence_label,
                markdown_cell(&record.reason)
            ));
        }
    }

    out.push_str("\n## Runtime Outcome Counts\n\n");
    out.push_str("| Runtime outcome | Count |\n| --- | ---: |\n");
    let counts = runtime_outcome_counts(report);
    if counts.is_empty() {
        out.push_str("| none | 0 |\n");
    } else {
        for (outcome, count) in counts {
            out.push_str(&format!("| {} | {} |\n", markdown_cell(&outcome), count));
        }
    }

    out.push_str("\n## Matched Mutants\n\n");
    if report.matched.is_empty() {
        out.push_str("No runtime mutants matched static seams.\n");
    } else {
        out.push_str("| Seam | Class | Oracle | Mutation operator | Runtime outcome | Join | Confidence label |\n");
        out.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
        for record in &report.matched {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}`/`{}` | {} | {} | `{}` | `{}` |\n",
                markdown_cell(&record.seam.seam_id),
                markdown_cell(&record.seam.seam_grip_class),
                markdown_cell(&record.seam.oracle_kind),
                markdown_cell(&record.seam.oracle_strength),
                markdown_cell(&record.mutation.mutation_operator),
                markdown_cell(&record.mutation.runtime_outcome),
                record.join_method,
                confidence_label_for_match(record)
            ));
        }
    }

    out.push_str("\n## Ambiguous File/Line Matches\n\n");
    if report.ambiguous_file_line.is_empty() {
        out.push_str(
            "No runtime mutants matched multiple static seams at the same file and line.\n",
        );
    } else {
        out.push_str("| Runtime mutant | Location | Runtime outcome | Confidence label | Candidate seams |\n");
        out.push_str("| --- | --- | --- | --- | --- |\n");
        for record in &report.ambiguous_file_line {
            let mutant = record.mutation.mutant_id.as_deref().unwrap_or("unknown");
            let location = mutation_location_label(&record.mutation);
            let candidates = record
                .candidates
                .iter()
                .map(|candidate| format!("`{}`", candidate.seam_id))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "| `{}` | {} | {} | `{}` | {} |\n",
                markdown_cell(mutant),
                markdown_cell(&location),
                markdown_cell(&record.mutation.runtime_outcome),
                "ambiguous_runtime_join",
                markdown_cell(&candidates)
            ));
        }
    }

    out.push_str("\n## Unmatched Runtime Mutants\n\n");
    if report.unmatched_mutants.is_empty() {
        out.push_str("All imported runtime mutants matched a static seam.\n");
    } else {
        out.push_str("| Location | Mutation operator | Runtime outcome | Test command |\n");
        out.push_str("| --- | --- | --- | --- |\n");
        for record in &report.unmatched_mutants {
            let location = mutation_location_label(record);
            let command = record.test_command.as_deref().unwrap_or("unknown");
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                markdown_cell(&location),
                markdown_cell(&record.mutation_operator),
                markdown_cell(&record.runtime_outcome),
                markdown_cell(command)
            ));
        }
    }

    out.push_str("\n## Static Seams Without Runtime Data\n\n");
    if report.static_without_runtime.is_empty() {
        out.push_str(
            "Every static seam matched at least one runtime mutant in the imported data.\n",
        );
    } else {
        out.push_str(
            "Sample only; see JSON `static_without_runtime_total` for the full count.\n\n",
        );
        out.push_str("| Seam | Kind | Class | Location | Confidence label |\n");
        out.push_str("| --- | --- | --- | --- | --- |\n");
        for seam in report
            .static_without_runtime
            .iter()
            .take(MUTATION_CALIBRATION_STATIC_WITHOUT_RUNTIME_SAMPLE_LIMIT)
        {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | {}:{} | `no_runtime_data` |\n",
                markdown_cell(&seam.seam_id),
                markdown_cell(&seam.seam_kind),
                markdown_cell(&seam.seam_grip_class),
                markdown_cell(&seam.file),
                seam.line
            ));
        }
    }

    out
}

fn mutation_location_label(record: &MutationOutcomeRecord) -> String {
    if let Some(seam_id) = record.seam_id.as_ref() {
        return format!("seam:{seam_id}");
    }
    match (&record.file, record.line) {
        (Some(file), Some(line)) => format!("{file}:{line}"),
        (Some(file), None) => file.clone(),
        (None, Some(line)) => format!("line {line}"),
        (None, None) => "unknown".to_string(),
    }
}

pub(crate) use self::mutation_calibration_impl as mutation_calibration;
