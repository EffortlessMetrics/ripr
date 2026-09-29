//! `cargo xtask check-identity-registry`
//!
//! Network-free, deterministic check that the governed identity catalog
//! covers serialized identity-shaped fields and that the generated human
//! table agrees with the machine-readable registry.

use std::collections::BTreeSet;
use std::path::Path;

use ripr::domain::{
    GOVERNED_IDENTITY_SURFACES, IDENTITY_REGISTRY_JSON_PATH, IDENTITY_REGISTRY_MARKDOWN_PATH,
    identity_field_disposition, identity_registry_canonical_json, identity_registry_markdown,
    identity_registry_violations,
};
use serde_json::Value;

use crate::{FixKind, PolicyReportSpec, finish_policy_report, read_text_lossy};

pub(crate) fn check_identity_registry() -> Result<(), String> {
    if std::env::var("RIPR_WRITE_IDENTITY_REGISTRY")
        .ok()
        .as_deref()
        == Some("1")
    {
        write_generated_artifacts()?;
    }
    let mut violations = identity_registry_violations();
    violations.extend(generated_artifact_violations()?);
    violations.extend(unknown_field_violations()?);
    finish_policy_report(
        PolicyReportSpec {
            report_file: "identity-registry.md",
            check: "check-identity-registry",
            why_it_matters: "Governed identifier fields need one registry disposition so later migrations cannot invent a second owner or treat compatibility aliases as authority.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Update crates/ripr/src/domain/identity to record the field as canonical, alias, component, or adjacent.",
                "Regenerate docs/identity/REGISTRY.md and docs/identity/registry.v1.json with RIPR_WRITE_IDENTITY_REGISTRY=1 cargo xtask check-identity-registry.",
                "Do not collapse RepairAttemptId into an analysis-attempt or snapshot identity.",
            ],
            rerun_command: "cargo xtask check-identity-registry",
            exception_template: None,
        },
        &violations,
    )
}

fn generated_artifact_violations() -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    let expected_markdown = identity_registry_markdown();
    let expected_json = identity_registry_canonical_json();
    match read_text_lossy(Path::new(IDENTITY_REGISTRY_MARKDOWN_PATH)) {
        Ok(actual) if actual == expected_markdown => {}
        Ok(_) => violations.push(format!(
            "{IDENTITY_REGISTRY_MARKDOWN_PATH} does not match identity_registry_markdown(); regenerate from the domain catalog"
        )),
        Err(error) => violations.push(error),
    }
    match read_text_lossy(Path::new(IDENTITY_REGISTRY_JSON_PATH)) {
        Ok(actual) if actual == expected_json => {}
        Ok(_) => violations.push(format!(
            "{IDENTITY_REGISTRY_JSON_PATH} does not match identity_registry_canonical_json(); regenerate from the domain catalog"
        )),
        Err(error) => violations.push(error),
    }
    if expected_markdown != identity_registry_markdown()
        || expected_json != identity_registry_canonical_json()
    {
        violations.push(
            "identity registry renderers are not byte-stable across repeated calls".to_string(),
        );
    }
    Ok(violations)
}

fn write_generated_artifacts() -> Result<(), String> {
    let markdown = Path::new(IDENTITY_REGISTRY_MARKDOWN_PATH);
    if let Some(parent) = markdown.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    std::fs::write(markdown, identity_registry_markdown())
        .map_err(|error| format!("{IDENTITY_REGISTRY_MARKDOWN_PATH}: {error}"))?;
    std::fs::write(
        Path::new(IDENTITY_REGISTRY_JSON_PATH),
        identity_registry_canonical_json(),
    )
    .map_err(|error| format!("{IDENTITY_REGISTRY_JSON_PATH}: {error}"))?;
    Ok(())
}

fn unknown_field_violations() -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    for surface in GOVERNED_IDENTITY_SURFACES {
        let fields = identity_shaped_fields(Path::new(surface))?;
        violations.extend(surface_field_violations(surface, &fields));
    }
    Ok(violations)
}

fn surface_field_violations(surface: &str, fields: &BTreeSet<String>) -> Vec<String> {
    if fields.is_empty() {
        return vec![format!(
            "governed identity surface `{surface}` produced zero identity-shaped fields"
        )];
    }
    let mut violations = Vec::new();
    for field in fields {
        if identity_field_disposition(surface, field).is_none() {
            violations.push(format!(
                "unknown governed identity field `{field}` on `{surface}` has no registry disposition"
            ));
        }
    }
    violations
}

fn identity_shaped_fields(path: &Path) -> Result<BTreeSet<String>, String> {
    let text = read_text_lossy(path)?;
    if path.extension().and_then(|value| value.to_str()) == Some("json") {
        let value: Value = serde_json::from_str(&text)
            .map_err(|error| format!("{}: parse JSON schema: {error}", path.display()))?;
        let mut fields = BTreeSet::new();
        collect_schema_fields(&value, &mut fields);
        return Ok(fields);
    }
    Ok(collect_rust_string_fields(&text))
}

fn collect_schema_fields(value: &Value, fields: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            if let Some(Value::Object(properties)) = object.get("properties") {
                for key in properties.keys() {
                    if is_identity_shaped_field(key) {
                        fields.insert(key.clone());
                    }
                }
            }
            for nested in object.values() {
                collect_schema_fields(nested, fields);
            }
        }
        Value::Array(values) => {
            for nested in values {
                collect_schema_fields(nested, fields);
            }
        }
        _ => {}
    }
}

fn collect_rust_string_fields(text: &str) -> BTreeSet<String> {
    let mut fields = BTreeSet::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            index += 1;
            let start = index;
            while index < bytes.len() && bytes[index] != b'"' {
                if bytes[index] == b'\\' {
                    index += 1;
                }
                index += 1;
            }
            if index > start
                && let Ok(literal) = std::str::from_utf8(&bytes[start..index])
                && is_identity_shaped_field(literal)
            {
                fields.insert(literal.to_string());
            }
        }
        index += 1;
    }
    fields
}

fn is_identity_shaped_field(name: &str) -> bool {
    let Some(rest) = name
        .strip_suffix("_id")
        .or_else(|| name.strip_suffix("_identity"))
    else {
        return false;
    };
    !rest.is_empty()
        && rest
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::{
        collect_rust_string_fields, collect_schema_fields, is_identity_shaped_field,
        surface_field_violations,
    };
    use ripr::domain::{
        GOVERNED_IDENTITY_SURFACES, identity_field_disposition, identity_registry_canonical_json,
        identity_registry_markdown, identity_registry_violations,
    };
    use serde_json::json;

    #[test]
    fn identity_registry_check_accepts_the_production_catalog() {
        assert_eq!(identity_registry_violations(), Vec::<String>::new());
        assert!(!identity_registry_markdown().is_empty());
        assert!(identity_registry_canonical_json().starts_with('{'));
        assert!(!GOVERNED_IDENTITY_SURFACES.is_empty());
    }

    #[test]
    fn scanner_extracts_identity_shaped_schema_properties_and_ignores_unrelated_keys() {
        let schema = json!({
            "properties": {
                "snapshot_id": { "type": "string" },
                "schema_version": { "type": "string" },
                "$id": { "type": "string" }
            },
            "$defs": {
                "after": {
                    "properties": {
                        "attempt_id": { "type": "string" },
                        "current": { "type": "boolean" }
                    }
                }
            }
        });
        let mut fields = std::collections::BTreeSet::new();
        collect_schema_fields(&schema, &mut fields);
        assert!(fields.contains("snapshot_id"));
        assert!(fields.contains("attempt_id"));
        assert!(!fields.contains("schema_version"));
        assert!(!fields.contains("$id"));
        assert!(!fields.contains("current"));
    }

    #[test]
    fn scanner_extracts_quoted_rust_identity_fields() {
        let source = r#"
            payload.insert("action_id".to_string(), value);
            payload.insert("schema_version".to_string(), value);
            let _ = "not_an_identifier";
        "#;
        let fields = collect_rust_string_fields(source);
        assert!(fields.contains("action_id"));
        assert!(!fields.contains("schema_version"));
        assert!(!fields.contains("not_an_identifier"));
        assert!(!is_identity_shaped_field("not_an_identifier"));
    }

    #[test]
    fn unknown_field_has_no_disposition() {
        assert!(is_identity_shaped_field("brand_new_widget_id"));
        assert_eq!(
            identity_field_disposition(
                "schemas/ripr/ripr-agent-success.schema.json",
                "brand_new_widget_id"
            ),
            None
        );
    }

    #[test]
    fn repair_attempt_id_is_not_an_analysis_attempt_disposition() {
        assert_eq!(
            identity_field_disposition(
                "schemas/ripr/repair-attempt.schema.json",
                "repair_attempt_id"
            ),
            Some("RepairAttemptId")
        );
        assert_eq!(
            identity_field_disposition(
                "schemas/ripr/ripr-agent-success.schema.json",
                "snapshot_id"
            ),
            Some("AnalysisAttemptId")
        );
    }

    #[test]
    fn identity_shaped_names_reject_empty_suffix_uppercase_and_bare_id() {
        assert!(!is_identity_shaped_field("_id"));
        assert!(!is_identity_shaped_field("_identity"));
        assert!(!is_identity_shaped_field("id"));
        assert!(!is_identity_shaped_field("identity"));
        assert!(!is_identity_shaped_field("Foo_id"));
        assert!(!is_identity_shaped_field("snapshotId"));
        assert!(is_identity_shaped_field("a_id"));
        assert!(is_identity_shaped_field("root_identity"));
    }

    #[test]
    fn same_attempt_id_name_keeps_repair_and_feedback_dispositions_apart() {
        assert_eq!(
            identity_field_disposition("schemas/ripr/repair-attempt.schema.json", "attempt_id"),
            Some("RepairAttemptId")
        );
        assert_eq!(
            identity_field_disposition("crates/ripr/src/output/feedback.rs", "attempt_id"),
            Some("adjacent")
        );
    }

    #[test]
    fn scanner_reports_unknown_and_zero_field_surfaces() {
        let mut unknown = std::collections::BTreeSet::new();
        unknown.insert("brand_new_widget_id".to_string());
        let unknown_violations =
            surface_field_violations("schemas/ripr/ripr-agent-success.schema.json", &unknown);
        assert!(
            unknown_violations.iter().any(|violation| {
                violation.contains("brand_new_widget_id") && violation.contains("unknown")
            }),
            "{unknown_violations:?}"
        );

        let empty = std::collections::BTreeSet::new();
        let empty_violations =
            surface_field_violations("schemas/ripr/ripr-agent-success.schema.json", &empty);
        assert!(
            empty_violations
                .iter()
                .any(|violation| violation.contains("zero identity-shaped fields")),
            "{empty_violations:?}"
        );
    }

    #[test]
    fn rust_scanner_keeps_identity_literals_after_escaped_quotes() {
        let source = r#"
            let _ = "not_an_identifier";
            let escaped = "pre\"fix";
            payload.insert("receipt_id", value);
        "#;
        let fields = collect_rust_string_fields(source);
        assert!(fields.contains("receipt_id"));
        assert!(!fields.contains("not_an_identifier"));
        assert!(!fields.contains(r#"pre\"fix"#));
        assert!(!fields.contains("pre\\"));
    }
}
