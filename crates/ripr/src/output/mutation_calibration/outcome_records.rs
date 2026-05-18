use super::{
    MutationOutcomeRecord, json_scalar_as_string, json_scalar_as_usize, normalize_report_path,
};
use serde_json::Value;

const MUTANT_ID_KEYS: &[&str] = &["id", "mutant_id", "mutantId"];
const SEAM_ID_KEYS: &[&str] = &["seam_id", "seamId", "probe_id", "probeId"];
const FILE_KEYS: &[&str] = &["file", "path", "source_file", "src_file", "filename"];
const NESTED_FILE_KEYS: &[&str] = &[
    "file",
    "path",
    "source_file",
    "src_file",
    "filename",
    "file_name",
];
const LINE_KEYS: &[&str] = &["line", "line_start", "start_line", "startLine"];
const OPERATOR_KEYS: &[&str] = &[
    "operator",
    "mutation_operator",
    "mutator",
    "mutation",
    "description",
    "replacement",
    "name",
];
const RUNTIME_OUTCOME_KEYS: &[&str] = &["outcome", "status", "result", "summary", "state"];
const DURATION_KEYS: &[&str] = &[
    "duration_ms",
    "durationMillis",
    "duration",
    "elapsed_ms",
    "elapsed",
];
const TEST_COMMAND_KEYS: &[&str] = &["test_command", "testCommand", "command", "cmd", "test_cmd"];

struct OutcomeObjectContext<'a> {
    object: &'a serde_json::Map<String, Value>,
    mutant: Option<&'a serde_json::Map<String, Value>>,
    mutation: Option<&'a serde_json::Map<String, Value>>,
    location: Option<&'a serde_json::Map<String, Value>>,
    span: Option<&'a serde_json::Map<String, Value>>,
}

struct OutcomeIdentity {
    mutant_id: Option<String>,
    seam_id: Option<String>,
    file: Option<String>,
    line: Option<usize>,
}

struct RuntimeDetails {
    mutation_operator: String,
    runtime_outcome: String,
    duration: Option<String>,
    test_command: Option<String>,
}

pub(super) fn parse_mutation_outcomes_json(
    json: &str,
) -> Result<Vec<MutationOutcomeRecord>, String> {
    let value: Value = serde_json::from_str(json)
        .map_err(|err| format!("failed to parse cargo-mutants JSON: {err}"))?;
    let mut records = Vec::new();
    collect_mutation_outcome_records(&value, &mut records);
    let mut records = super::merge_mutation_outcome_records(records);
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
            collect_nested_outcome_lists(object, records);
            if let Some(record) = mutation_outcome_record_from_object(object) {
                records.push(record);
            }
        }
        _ => {}
    }
}

fn collect_nested_outcome_lists(
    object: &serde_json::Map<String, Value>,
    records: &mut Vec<MutationOutcomeRecord>,
) {
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
}

fn mutation_outcome_record_from_object(
    object: &serde_json::Map<String, Value>,
) -> Option<MutationOutcomeRecord> {
    let context = OutcomeObjectContext::new(object);
    let identity = context.identity();
    let details = context.runtime_details();
    has_record_signal(&identity, &details).then_some(MutationOutcomeRecord {
        mutant_id: identity.mutant_id,
        seam_id: identity.seam_id,
        file: identity.file,
        line: identity.line,
        mutation_operator: details.mutation_operator,
        runtime_outcome: details.runtime_outcome,
        duration: details.duration,
        test_command: details.test_command,
    })
}

fn has_record_signal(identity: &OutcomeIdentity, details: &RuntimeDetails) -> bool {
    let has_identity = identity.mutant_id.is_some()
        || identity.seam_id.is_some()
        || identity.file.is_some()
        || identity.line.is_some();
    let has_runtime_detail = details.runtime_outcome != "unknown"
        || details.mutation_operator != "unknown"
        || details.duration.is_some()
        || details.test_command.is_some();
    has_identity && has_runtime_detail
}

impl<'a> OutcomeObjectContext<'a> {
    fn new(object: &'a serde_json::Map<String, Value>) -> Self {
        let mutant = nested_object(object, "mutant");
        let mutation = nested_object(object, "mutation");
        let location = nested_object(object, "location");
        let span = nested_object(object, "span")
            .or_else(|| mutant.and_then(|nested| nested_object(nested, "span")))
            .or_else(|| mutation.and_then(|nested| nested_object(nested, "span")))
            .or_else(|| location.and_then(|nested| nested_object(nested, "span")));
        Self {
            object,
            mutant,
            mutation,
            location,
            span,
        }
    }

    fn identity(&self) -> OutcomeIdentity {
        OutcomeIdentity {
            mutant_id: self.mutant_id(),
            seam_id: self.seam_id(),
            file: self.file(),
            line: self.line(),
        }
    }

    fn runtime_details(&self) -> RuntimeDetails {
        RuntimeDetails {
            mutation_operator: self.mutation_operator(),
            runtime_outcome: string_field_any(self.object, RUNTIME_OUTCOME_KEYS)
                .unwrap_or_else(|| "unknown".to_string()),
            duration: string_field_any(self.object, DURATION_KEYS),
            test_command: string_field_any(self.object, TEST_COMMAND_KEYS),
        }
    }

    fn mutant_id(&self) -> Option<String> {
        string_field_any(self.object, MUTANT_ID_KEYS).or_else(|| {
            self.mutant
                .and_then(|nested| string_field_any(nested, MUTANT_ID_KEYS))
        })
    }

    fn seam_id(&self) -> Option<String> {
        string_field_any(self.object, SEAM_ID_KEYS)
            .or_else(|| {
                self.mutant
                    .and_then(|nested| string_field_any(nested, SEAM_ID_KEYS))
            })
            .or_else(|| {
                self.mutation
                    .and_then(|nested| string_field_any(nested, SEAM_ID_KEYS))
            })
    }

    fn file(&self) -> Option<String> {
        string_field_any(self.object, FILE_KEYS)
            .or_else(|| {
                self.mutant
                    .and_then(|nested| string_field_any(nested, FILE_KEYS))
            })
            .or_else(|| {
                self.mutation
                    .and_then(|nested| string_field_any(nested, FILE_KEYS))
            })
            .or_else(|| {
                self.location
                    .and_then(|nested| string_field_any(nested, NESTED_FILE_KEYS))
            })
            .or_else(|| {
                self.span
                    .and_then(|nested| string_field_any(nested, NESTED_FILE_KEYS))
            })
            .map(|path| normalize_report_path(&path))
    }

    fn line(&self) -> Option<usize> {
        usize_field_any(self.object, LINE_KEYS)
            .or_else(|| {
                self.mutant
                    .and_then(|nested| usize_field_any(nested, LINE_KEYS))
            })
            .or_else(|| {
                self.mutation
                    .and_then(|nested| usize_field_any(nested, LINE_KEYS))
            })
            .or_else(|| {
                self.location
                    .and_then(|nested| usize_field_any(nested, LINE_KEYS))
            })
            .or_else(|| self.span.and_then(span_start_line))
    }

    fn mutation_operator(&self) -> String {
        string_field_any(self.object, OPERATOR_KEYS)
            .or_else(|| {
                self.mutant
                    .and_then(|nested| string_field_any(nested, OPERATOR_KEYS))
            })
            .or_else(|| {
                self.mutation
                    .and_then(|nested| string_field_any(nested, OPERATOR_KEYS))
            })
            .unwrap_or_else(|| "unknown".to_string())
    }
}

fn nested_object<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Option<&'a serde_json::Map<String, Value>> {
    object.get(key).and_then(Value::as_object)
}

fn span_start_line(span: &serde_json::Map<String, Value>) -> Option<usize> {
    usize_field_any(span, LINE_KEYS)
        .or_else(|| {
            nested_object(span, "start").and_then(|start| usize_field_any(start, LINE_KEYS))
        })
        .or_else(|| {
            nested_object(span, "start_position")
                .and_then(|start| usize_field_any(start, LINE_KEYS))
        })
        .or_else(|| nested_object(span, "lo").and_then(|start| usize_field_any(start, LINE_KEYS)))
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
