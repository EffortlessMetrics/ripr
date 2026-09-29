//! Byte-stable JSON and Markdown projections of the identity registry.
//!
//! Domain emitters write canonical text; they do not import a JSON crate.

use super::invariants::sort_records;
use super::record::{AdjacentField, IdentityRecord};

pub(crate) const REGISTRY_SCHEMA_VERSION: &str = "1.0";
pub(crate) const REGISTRY_MARKDOWN_PATH: &str = "docs/identity/REGISTRY.md";
pub(crate) const REGISTRY_JSON_PATH: &str = "docs/identity/registry.v1.json";

pub(crate) fn render_markdown(records: &[IdentityRecord], adjacent: &[AdjacentField]) -> String {
    let mut ordered = records.to_vec();
    sort_records(&mut ordered);
    let mut body = String::from(
        "# RIPR governed identity registry\n\n\
Generated from `crates/ripr/src/domain/identity`. Do not edit by hand.\n\n\
This table is the human projection of the machine-readable registry. It names\n\
authorities and compatibility posture. It does not prove that consumers have\n\
migrated, and it does not change actionability, currentness, transport\n\
behavior, or support claims.\n\n\
| Identity | Owner | Class | Invalidation | Canonical fields | Aliases |\n\
| --- | --- | --- | --- | --- | --- |\n",
    );
    for record in &ordered {
        let canonical = field_names(record, super::record::FieldRole::Canonical);
        let aliases = field_names(record, super::record::FieldRole::CompatibilityAlias);
        body.push_str(&format!(
            "| `{kind}` | `{owner}` ({issue}) | `{class}` | {invalidation} | {canonical} | {aliases} |\n",
            kind = record.kind.as_str(),
            owner = record.canonical_type.replace('|', "\\|"),
            issue = record.owner_issue.replace('|', "\\|"),
            class = record.portability.as_str(),
            invalidation = record.invalidation.replace('|', "\\|"),
            canonical = markdown_list(&canonical),
            aliases = markdown_list(&aliases),
        ));
    }
    body.push_str("\n## Relationships\n\n");
    for record in &ordered {
        if record.parents.is_empty() && record.children.is_empty() {
            continue;
        }
        let parents = kinds(record.parents);
        let children = kinds(record.children);
        body.push_str(&format!(
            "- `{}` parents: {}; children: {}\n",
            record.kind.as_str(),
            markdown_list(&parents),
            markdown_list(&children)
        ));
    }
    if !adjacent.is_empty() {
        body.push_str("\n## Adjacent identity-shaped fields\n\n");
        let mut adjacent_rows = adjacent.to_vec();
        adjacent_rows.sort_by_key(|entry| entry.name);
        for entry in adjacent_rows {
            body.push_str(&format!(
                "- `{}` on {} — {}\n",
                entry.name,
                markdown_list(entry.surfaces),
                entry.reason
            ));
        }
    }
    body
}

pub(crate) fn render_canonical_json(
    records: &[IdentityRecord],
    adjacent: &[AdjacentField],
) -> String {
    let mut ordered = records.to_vec();
    sort_records(&mut ordered);
    let mut body = String::from("{\n");
    push_json_entry(
        &mut body,
        2,
        "schema_version",
        REGISTRY_SCHEMA_VERSION,
        true,
    );
    body.push_str("  \"identities\": [\n");
    for (index, record) in ordered.iter().enumerate() {
        body.push_str("    {\n");
        push_json_entry(&mut body, 6, "kind", record.kind.as_str(), true);
        push_json_entry(&mut body, 6, "canonical_type", record.canonical_type, true);
        push_json_entry(&mut body, 6, "owner_path", record.owner_path, true);
        push_json_entry(&mut body, 6, "owner_issue", record.owner_issue, true);
        push_json_entry(
            &mut body,
            6,
            "portability",
            record.portability.as_str(),
            true,
        );
        push_json_str_array(
            &mut body,
            6,
            "semantic_inputs",
            record.semantic_inputs,
            true,
        );
        push_json_str_array(
            &mut body,
            6,
            "volatile_excluded",
            record.volatile_excluded,
            true,
        );
        push_json_str_array(&mut body, 6, "parents", &kinds(record.parents), true);
        push_json_str_array(&mut body, 6, "children", &kinds(record.children), true);
        push_json_entry(&mut body, 6, "invalidation", record.invalidation, true);
        push_json_entry(&mut body, 6, "persistence", record.persistence, true);
        body.push_str("      \"serialization\": [\n");
        for (field_index, field) in record.serialization.iter().enumerate() {
            body.push_str("        {\n");
            push_json_entry(&mut body, 10, "name", field.name, true);
            push_json_entry(&mut body, 10, "role", field.role.as_str(), true);
            push_json_entry(&mut body, 10, "surface", field.surface, true);
            push_json_entry(&mut body, 10, "visibility", field.visibility.as_str(), true);
            match field.removal_generation {
                Some(generation) => {
                    push_json_entry(&mut body, 10, "removal_generation", generation, false)
                }
                None => body.push_str("          \"removal_generation\": null\n"),
            }
            body.push_str("        }");
            if field_index + 1 != record.serialization.len() {
                body.push(',');
            }
            body.push('\n');
        }
        body.push_str("      ],\n");
        push_json_str_array(
            &mut body,
            6,
            "competing_wrappers",
            record.competing_wrappers,
            false,
        );
        body.push_str("    }");
        if index + 1 != ordered.len() {
            body.push(',');
        }
        body.push('\n');
    }
    body.push_str("  ],\n");
    body.push_str("  \"adjacent_fields\": [\n");
    let mut adjacent_rows = adjacent.to_vec();
    adjacent_rows.sort_by_key(|entry| (entry.name, entry.reason));
    for (index, entry) in adjacent_rows.iter().enumerate() {
        body.push_str("    {\n");
        push_json_entry(&mut body, 6, "name", entry.name, true);
        push_json_str_array(&mut body, 6, "surfaces", entry.surfaces, true);
        push_json_entry(&mut body, 6, "reason", entry.reason, false);
        body.push_str("    }");
        if index + 1 != adjacent_rows.len() {
            body.push(',');
        }
        body.push('\n');
    }
    body.push_str("  ]\n}\n");
    body
}

fn field_names(record: &IdentityRecord, role: super::record::FieldRole) -> Vec<&'static str> {
    let mut names = record
        .serialization
        .iter()
        .filter(|field| {
            field.role == role && field.visibility == super::record::FieldVisibility::Public
        })
        .map(|field| field.name)
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup();
    names
}

fn kinds(values: &[super::kinds::IdentityKind]) -> Vec<&'static str> {
    values.iter().map(|kind| kind.as_str()).collect()
}

fn markdown_list(values: &[&str]) -> String {
    if values.is_empty() {
        return "—".to_string();
    }
    values
        .iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn push_json_entry(body: &mut String, indent: usize, key: &str, value: &str, trailing_comma: bool) {
    body.push_str(&" ".repeat(indent));
    body.push('"');
    body.push_str(key);
    body.push_str("\": \"");
    push_escaped(body, value);
    body.push('"');
    if trailing_comma {
        body.push(',');
    }
    body.push('\n');
}

fn push_json_str_array(
    body: &mut String,
    indent: usize,
    key: &str,
    values: &[&str],
    trailing_comma: bool,
) {
    body.push_str(&" ".repeat(indent));
    body.push('"');
    body.push_str(key);
    body.push_str("\": [");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            body.push_str(", ");
        }
        body.push('"');
        push_escaped(body, value);
        body.push('"');
    }
    body.push(']');
    if trailing_comma {
        body.push(',');
    }
    body.push('\n');
}

fn push_escaped(body: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '"' => body.push_str("\\\""),
            '\\' => body.push_str("\\\\"),
            '\n' => body.push_str("\\n"),
            '\r' => body.push_str("\\r"),
            '\t' => body.push_str("\\t"),
            other => body.push(other),
        }
    }
}
