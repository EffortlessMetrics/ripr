use super::types::*;
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn parse_repo_exposure_static_seams(
    json: &str,
) -> Result<Vec<StaticSeamRecord>, String> {
    let value: Value = serde_json::from_str(json)
        .map_err(|err| format!("failed to parse repo exposure JSON: {err}"))?;
    let seams = value
        .get("seams")
        .and_then(Value::as_array)
        .ok_or_else(|| "repo exposure JSON is missing `seams` array".to_string())?;

    let mut records = Vec::new();
    for seam in seams {
        let seam_id = required_json_string(seam, "seam_id")?;
        let seam_kind = required_json_string(seam, "kind")?;
        let file = normalize_report_path(&required_json_string(seam, "file")?);
        let line = required_json_usize(seam, "line")?;
        let seam_grip_class = required_json_string(seam, "grip_class")?;
        let (oracle_kind, oracle_strength) = strongest_related_oracle(seam);
        records.push(StaticSeamRecord {
            seam_id,
            seam_kind,
            file,
            line,
            seam_grip_class,
            oracle_kind,
            oracle_strength,
            observed_values: string_array_field(seam, "observed_values"),
            missing_discriminators: missing_discriminator_strings(seam),
        });
    }
    Ok(records)
}

pub(super) fn parse_mutation_outcomes_json(
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

fn required_json_string(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(json_scalar_as_string)
        .ok_or_else(|| format!("repo exposure seam is missing string field `{key}`"))
}

fn required_json_usize(value: &Value, key: &str) -> Result<usize, String> {
    value
        .get(key)
        .and_then(json_scalar_as_usize)
        .ok_or_else(|| format!("repo exposure seam is missing numeric field `{key}`"))
}

fn strongest_related_oracle(seam: &Value) -> (String, String) {
    let mut best_kind = "unknown".to_string();
    let mut best_strength = "unknown".to_string();
    let mut best_rank = 0;

    if let Some(related) = seam.get("related_tests").and_then(Value::as_array) {
        for test in related {
            let strength = test
                .get("oracle_strength")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let rank = oracle_strength_rank(strength);
            if rank > best_rank {
                best_rank = rank;
                best_strength = strength.to_string();
                best_kind = test
                    .get("oracle_kind")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
            }
        }
    }

    (best_kind, best_strength)
}

fn oracle_strength_rank(strength: &str) -> u8 {
    match strength {
        "strong" => 5,
        "medium" => 4,
        "weak" => 3,
        "smoke" => 2,
        "none" => 1,
        _ => 0,
    }
}

fn string_array_field(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(json_scalar_as_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn missing_discriminator_strings(seam: &Value) -> Vec<String> {
    seam.get("missing_discriminators")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    if let Some(value) = json_scalar_as_string(item) {
                        return Some(value);
                    }
                    let value = item.get("value").and_then(json_scalar_as_string)?;
                    match item.get("reason").and_then(json_scalar_as_string) {
                        Some(reason) if !reason.is_empty() => Some(format!("{value} ({reason})")),
                        _ => Some(value),
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
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

fn json_scalar_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn json_scalar_as_usize(value: &Value) -> Option<usize> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .and_then(|value| usize::try_from(value).ok()),
        Value::String(text) => text.trim().parse::<usize>().ok(),
        _ => None,
    }
}

pub(super) fn normalize_report_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    normalized
        .strip_prefix("./")
        .unwrap_or(normalized.as_str())
        .to_string()
}
