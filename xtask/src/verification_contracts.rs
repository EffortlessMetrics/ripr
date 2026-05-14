use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

const VERIFICATION_README: &str = "docs/verification/README.md";

struct VerificationContract {
    schema_path: &'static str,
    fixture_path: &'static str,
    doc_path: &'static str,
    doc_markers: &'static [&'static str],
}

const CONTRACTS: &[VerificationContract] = &[
    VerificationContract {
        schema_path: "schemas/badges/shields-endpoint.schema.json",
        fixture_path: "tests/fixtures/verification/badge/ripr-plus.valid.json",
        doc_path: "docs/verification/badge-contract.md",
        doc_markers: &["schemaVersion", "label", "message", "color"],
    },
    VerificationContract {
        schema_path: "schemas/ripr/pr-evidence.schema.json",
        fixture_path: "tests/fixtures/verification/ripr/pr-evidence.valid.json",
        doc_path: "docs/verification/pr-evidence-contract.md",
        doc_markers: &[
            "schema_version",
            "tool",
            "kind",
            "scope",
            "status",
            "root",
            "base",
            "head",
            "summary",
            "artifacts[]",
            "warnings[]",
            "advisory_limits[]",
            "requires_targeted_mutation",
            "ripr_severe_gap",
            "routing_reason",
        ],
    },
    VerificationContract {
        schema_path: "schemas/ripr/review-comments.schema.json",
        fixture_path: "tests/fixtures/verification/ripr/review-comments.valid.json",
        doc_path: "docs/verification/pr-evidence-contract.md",
        doc_markers: &[
            "schema_version",
            "tool",
            "status",
            "root",
            "base",
            "head",
            "mode",
            "rendering_limits",
            "comments[]",
            "summary_only[]",
            "suppressed[]",
            "warnings[]",
            "limits_note",
        ],
    },
];

pub(crate) fn check_verification_contracts(args: &[String]) -> Result<(), String> {
    if !args.iter().all(|arg| arg == "--check") {
        return Err("usage: cargo xtask check-verification-contracts [--check]".to_string());
    }

    let root = repo_root()?;
    let readme = read_text(root.join(VERIFICATION_README))?;
    let mut violations = Vec::new();

    for required in [
        "badge-contract.md",
        "pr-evidence-contract.md",
        "artifact-layout.md",
        "annotation-policy.md",
        "schemas/badges/shields-endpoint.schema.json",
        "schemas/ripr/pr-evidence.schema.json",
        "schemas/ripr/review-comments.schema.json",
    ] {
        if !readme.contains(required) {
            violations.push(format!("{VERIFICATION_README} does not link `{required}`"));
        }
    }

    for contract in CONTRACTS {
        let schema = read_json(root.join(contract.schema_path))?;
        validate_schema_document(contract.schema_path, &schema, &mut violations);

        let fixture = read_json(root.join(contract.fixture_path))?;
        validate_value_against_schema(
            &fixture,
            &schema,
            &schema,
            contract.fixture_path.to_string(),
            &mut violations,
        );

        let doc = read_text(root.join(contract.doc_path))?;
        for marker in contract.doc_markers {
            if !doc.contains(&format!("`{marker}`")) {
                violations.push(format!(
                    "{} does not document schema field `{marker}`",
                    contract.doc_path
                ));
            }
        }
    }

    if violations.is_empty() {
        println!(
            "verification contracts: checked {} schemas and fixtures",
            CONTRACTS.len()
        );
        Ok(())
    } else {
        Err(format!(
            "verification contract check failed:\n{}",
            violations
                .iter()
                .map(|violation| format!("- {violation}"))
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }
}

fn validate_schema_document(path: &str, schema: &Value, violations: &mut Vec<String>) {
    let Some(object) = schema.as_object() else {
        violations.push(format!("{path} must be a JSON object"));
        return;
    };

    if object.get("$schema").and_then(Value::as_str)
        != Some("https://json-schema.org/draft/2020-12/schema")
    {
        violations.push(format!("{path} must use JSON Schema draft 2020-12"));
    }
    if object.get("$id").and_then(Value::as_str).is_none() {
        violations.push(format!("{path} is missing `$id`"));
    }
    if object.get("type").and_then(Value::as_str) != Some("object") {
        violations.push(format!("{path} top-level type must be object"));
    }

    let required = string_array(object.get("required"));
    if required.is_empty() {
        violations.push(format!("{path} must define at least one required field"));
    }

    let properties = object
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    if properties.is_empty() {
        violations.push(format!("{path} must define top-level properties"));
    }

    for field in required {
        if !properties.contains(field.as_str()) {
            violations.push(format!(
                "{path} requires `{field}` but does not define it in properties"
            ));
        }
    }
}

fn validate_value_against_schema(
    value: &Value,
    schema: &Value,
    root_schema: &Value,
    location: String,
    violations: &mut Vec<String>,
) {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        match resolve_ref(root_schema, reference) {
            Some(resolved) => {
                validate_value_against_schema(value, resolved, root_schema, location, violations)
            }
            None => violations.push(format!(
                "{location}: unresolved schema reference {reference}"
            )),
        }
        return;
    }

    if let Some(all_of) = schema.get("allOf").and_then(Value::as_array) {
        for (index, subschema) in all_of.iter().enumerate() {
            validate_value_against_schema(
                value,
                subschema,
                root_schema,
                format!("{location}/allOf[{index}]"),
                violations,
            );
        }
    }

    if let Some(any_of) = schema.get("anyOf").and_then(Value::as_array) {
        let mut matched = false;
        let mut messages = Vec::new();
        for subschema in any_of {
            let mut nested = Vec::new();
            validate_value_against_schema(
                value,
                subschema,
                root_schema,
                location.clone(),
                &mut nested,
            );
            if nested.is_empty() {
                matched = true;
                break;
            }
            messages.extend(nested);
        }
        if !matched {
            violations.push(format!(
                "{location}: did not match any allowed schema ({})",
                messages.join("; ")
            ));
        }
    }

    if let Some(expected) = schema.get("const")
        && value != expected
    {
        violations.push(format!(
            "{location}: expected const {}, got {}",
            compact_json(expected),
            compact_json(value)
        ));
    }

    if let Some(allowed) = schema.get("enum").and_then(Value::as_array)
        && !allowed.iter().any(|candidate| candidate == value)
    {
        violations.push(format!(
            "{location}: value {} is not in enum",
            compact_json(value)
        ));
    }

    if let Some(schema_type) = schema.get("type") {
        validate_type(value, schema_type, &location, violations);
    }

    if value.is_object() {
        validate_object(value, schema, root_schema, &location, violations);
    }
    if value.is_array() {
        validate_array(value, schema, root_schema, &location, violations);
    }
    if let Some(text) = value.as_str() {
        validate_string(text, schema, &location, violations);
    }
    if value.is_number() {
        validate_number(value, schema, &location, violations);
    }
}

fn validate_object(
    value: &Value,
    schema: &Value,
    root_schema: &Value,
    location: &str,
    violations: &mut Vec<String>,
) {
    let Some(object) = value.as_object() else {
        return;
    };
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return;
    };

    for field in string_array(schema.get("required")) {
        if !object.contains_key(&field) {
            violations.push(format!("{location}: missing required field `{field}`"));
        }
    }

    if schema.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
        for field in object.keys() {
            if !properties.contains_key(field) {
                violations.push(format!("{location}: unexpected field `{field}`"));
            }
        }
    }

    for (field, field_schema) in properties {
        if let Some(field_value) = object.get(field) {
            validate_value_against_schema(
                field_value,
                field_schema,
                root_schema,
                format!("{location}.{field}"),
                violations,
            );
        }
    }
}

fn validate_array(
    value: &Value,
    schema: &Value,
    root_schema: &Value,
    location: &str,
    violations: &mut Vec<String>,
) {
    let Some(items_schema) = schema.get("items") else {
        return;
    };
    let Some(items) = value.as_array() else {
        return;
    };
    for (index, item) in items.iter().enumerate() {
        validate_value_against_schema(
            item,
            items_schema,
            root_schema,
            format!("{location}[{index}]"),
            violations,
        );
    }
}

fn validate_string(text: &str, schema: &Value, location: &str, violations: &mut Vec<String>) {
    if let Some(min_length) = schema.get("minLength").and_then(Value::as_u64)
        && text.chars().count() < min_length as usize
    {
        violations.push(format!(
            "{location}: string shorter than minLength {min_length}"
        ));
    }
    if let Some(max_length) = schema.get("maxLength").and_then(Value::as_u64)
        && text.chars().count() > max_length as usize
    {
        violations.push(format!(
            "{location}: string longer than maxLength {max_length}"
        ));
    }
}

fn validate_number(value: &Value, schema: &Value, location: &str, violations: &mut Vec<String>) {
    if let Some(minimum) = schema.get("minimum").and_then(Value::as_i64) {
        let Some(actual) = value
            .as_i64()
            .or_else(|| value.as_u64().map(|number| number as i64))
        else {
            return;
        };
        if actual < minimum {
            violations.push(format!("{location}: number below minimum {minimum}"));
        }
    }
}

fn validate_type(value: &Value, schema_type: &Value, location: &str, violations: &mut Vec<String>) {
    let allowed = match schema_type {
        Value::String(text) => vec![text.as_str()],
        Value::Array(values) => values.iter().filter_map(Value::as_str).collect(),
        _ => {
            violations.push(format!("{location}: schema type must be string or array"));
            return;
        }
    };

    if !allowed
        .iter()
        .any(|allowed_type| value_matches_type(value, allowed_type))
    {
        violations.push(format!(
            "{location}: expected type {}, got {}",
            allowed.join("|"),
            value_type(value)
        ));
    }
}

fn value_matches_type(value: &Value, expected_type: &str) -> bool {
    match expected_type {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => {
            value.as_i64().is_some()
                || value
                    .as_u64()
                    .is_some_and(|number| i64::try_from(number).is_ok())
        }
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    }
}

fn value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn resolve_ref<'a>(root_schema: &'a Value, reference: &str) -> Option<&'a Value> {
    let pointer = reference.strip_prefix('#')?;
    root_schema.pointer(pointer)
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect()
}

fn read_json(path: PathBuf) -> Result<Value, String> {
    let text = read_text(&path)?;
    serde_json::from_str(&text).map_err(|err| format!("parse {}: {err}", path.display()))
}

fn read_text(path: impl AsRef<Path>) -> Result<String, String> {
    let path = path.as_ref();
    fs::read_to_string(path).map_err(|err| format!("read {}: {err}", path.display()))
}

fn repo_root() -> Result<PathBuf, String> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().map(Path::to_path_buf).ok_or_else(|| {
        format!(
            "failed to resolve repo root from {}",
            manifest_dir.display()
        )
    })
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_accepts_default_and_check_modes() -> Result<(), String> {
        check_verification_contracts(&[])?;
        check_verification_contracts(&["--check".to_string()])
    }

    #[test]
    fn command_rejects_unknown_args() {
        let err = match check_verification_contracts(&["--write".to_string()]) {
            Ok(()) => "unexpected args should fail".to_string(),
            Err(err) => err,
        };
        assert!(err.contains("usage: cargo xtask check-verification-contracts"));
    }

    #[test]
    fn valid_badge_fixture_matches_shields_schema() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join("schemas/badges/shields-endpoint.schema.json"))?;
        let fixture =
            read_json(root.join("tests/fixtures/verification/badge/ripr-plus.valid.json"))?;
        let mut violations = Vec::new();

        validate_value_against_schema(
            &fixture,
            &schema,
            &schema,
            "badge fixture".to_string(),
            &mut violations,
        );

        assert!(violations.is_empty(), "{violations:#?}");
        Ok(())
    }

    #[test]
    fn valid_pr_evidence_fixture_matches_schema() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join("schemas/ripr/pr-evidence.schema.json"))?;
        let fixture =
            read_json(root.join("tests/fixtures/verification/ripr/pr-evidence.valid.json"))?;
        let mut violations = Vec::new();

        validate_value_against_schema(
            &fixture,
            &schema,
            &schema,
            "pr evidence fixture".to_string(),
            &mut violations,
        );

        assert!(violations.is_empty(), "{violations:#?}");
        Ok(())
    }

    #[test]
    fn valid_review_comments_fixture_matches_schema() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join("schemas/ripr/review-comments.schema.json"))?;
        let fixture =
            read_json(root.join("tests/fixtures/verification/ripr/review-comments.valid.json"))?;
        let mut violations = Vec::new();

        validate_value_against_schema(
            &fixture,
            &schema,
            &schema,
            "review comments fixture".to_string(),
            &mut violations,
        );

        assert!(violations.is_empty(), "{violations:#?}");
        Ok(())
    }

    #[test]
    fn invalid_type_reports_actionable_path() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join("schemas/badges/shields-endpoint.schema.json"))?;
        let fixture = serde_json::json!({
            "schemaVersion": 1,
            "label": "ripr+",
            "message": 0,
            "color": "brightgreen"
        });
        let mut violations = Vec::new();

        validate_value_against_schema(
            &fixture,
            &schema,
            &schema,
            "badge fixture".to_string(),
            &mut violations,
        );

        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("badge fixture.message")),
            "{violations:#?}"
        );
        Ok(())
    }
}
