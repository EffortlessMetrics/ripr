use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::schema_pattern::SchemaPattern;

const VERIFICATION_README: &str = "docs/verification/README.md";

/// Which value inside `fixture_path` a contract validates.
///
/// A published schema is not always the shape of a whole file. Some producer
/// bytes only exist embedded in a larger generated artifact, and validating a
/// hand-written copy of them instead would make the contract self-confirming.
enum ContractSubject {
    /// The whole fixture document.
    Document,
    /// One value selected by JSON pointer.
    Pointer(&'static str),
    /// Every element of the array at `array`, optionally narrowed by `item`.
    EachItem {
        array: &'static str,
        item: Option<&'static str>,
    },
}

struct VerificationContract {
    schema_path: &'static str,
    /// JSON pointer to the subschema that owns the subject, or `None` for the
    /// whole schema document.
    schema_pointer: Option<&'static str>,
    fixture_path: &'static str,
    subject: ContractSubject,
    doc_path: &'static str,
    doc_markers: &'static [&'static str],
}

const CONTRACTS: &[VerificationContract] = &[
    VerificationContract {
        schema_path: "schemas/badges/shields-endpoint.schema.json",
        schema_pointer: None,
        fixture_path: "tests/fixtures/verification/badge/ripr-plus.valid.json",
        subject: ContractSubject::Document,
        doc_path: "docs/verification/badge-contract.md",
        doc_markers: &["schemaVersion", "label", "message", "color"],
    },
    VerificationContract {
        schema_path: "schemas/ripr/pr-evidence.schema.json",
        schema_pointer: None,
        fixture_path: "tests/fixtures/verification/ripr/pr-evidence.valid.json",
        subject: ContractSubject::Document,
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
        schema_pointer: None,
        fixture_path: "tests/fixtures/verification/ripr/review-comments.valid.json",
        subject: ContractSubject::Document,
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
    VerificationContract {
        schema_path: "schemas/ripr/gate-decision.schema.json",
        schema_pointer: None,
        fixture_path: "tests/fixtures/verification/ripr/gate-decision.valid.json",
        subject: ContractSubject::Document,
        doc_path: "docs/OUTPUT_SCHEMA.md",
        doc_markers: &[
            "schema_version",
            "tool",
            "status",
            "mode",
            "root",
            "decisions[]",
            "repair_route",
            "canonical_gap_id",
            "seam_id",
            "gap_state",
            "changed_owner",
            "repair_target",
            "verify_command",
            "receipt_command",
            "inspection_command",
            "authority_boundary",
            "incomplete_repair_route",
        ],
    },
    VerificationContract {
        schema_path: "schemas/ripr/check.schema.json",
        schema_pointer: None,
        fixture_path: "tests/fixtures/verification/ripr/check-complete.valid.json",
        subject: ContractSubject::Document,
        doc_path: "docs/OUTPUT_SCHEMA.md",
        doc_markers: &[
            "schema_version",
            "tool",
            "mode",
            "root",
            "summary",
            "analysis_outcome",
            "findings",
            "finding_alignment",
        ],
    },
    VerificationContract {
        schema_path: "schemas/ripr/check.schema.json",
        schema_pointer: None,
        fixture_path: "tests/fixtures/verification/ripr/check-limited.valid.json",
        subject: ContractSubject::Document,
        doc_path: "docs/OUTPUT_SCHEMA.md",
        doc_markers: &[],
    },
    // The trust corpus of record is its own canonical instance. Validating a
    // hand-written copy instead would let the schema confirm itself while the
    // artifact `cargo xtask rust-repair-trust` actually reads drifts away.
    VerificationContract {
        schema_path: "schemas/ripr/rust-repair-trust-corpus.schema.json",
        schema_pointer: None,
        fixture_path: "metrics/rust-repair-trust/corpus.json",
        subject: ContractSubject::Document,
        doc_path: "docs/verification/schema-producer-audit.md",
        doc_markers: &[
            "schema_version",
            "kind",
            "authorization",
            "cases",
            "exclusions",
            "observations",
        ],
    },
    // `command_specs.verify` in a generated agent packet is producer output
    // from `crate::agent::command_specs`, so the published command-spec
    // contract is checked against bytes the product actually emitted.
    VerificationContract {
        schema_path: "schemas/ripr/repair-assurance.schema.json",
        schema_pointer: Some("/$defs/verification_command_spec"),
        fixture_path: "fixtures/boundary_gap/expected/editor-agent-loop/agent-packet.json",
        subject: ContractSubject::Pointer(
            "/packets/0/evidence_record/canonical_item/command_specs/verify",
        ),
        doc_path: "docs/verification/schema-producer-audit.md",
        doc_markers: &["command_spec", "verification_command_spec"],
    },
    VerificationContract {
        schema_path: "schemas/ripr/repair-assurance.schema.json",
        schema_pointer: Some("/$defs/command_spec"),
        fixture_path: "fixtures/boundary_gap/expected/editor-agent-loop/agent-packet.json",
        subject: ContractSubject::Pointer(
            "/packets/0/evidence_record/canonical_item/command_specs/receipt",
        ),
        doc_path: "docs/verification/schema-producer-audit.md",
        doc_markers: &["authority_boundary", "working_directory"],
    },
    // The `RepairAssuranceV1` envelope has no producer: `implementation_state`
    // is pinned to `design_only`. The design corpus is the narrower authority
    // that the vocabulary spec already claims, so it is registered here rather
    // than left as an unenforced sentence in `fixtures/assurance_vocabulary/`.
    VerificationContract {
        schema_path: "schemas/ripr/repair-assurance.schema.json",
        schema_pointer: None,
        fixture_path: "fixtures/assurance_vocabulary/assurance/corpus.json",
        subject: ContractSubject::EachItem {
            array: "/cases",
            item: Some("/record"),
        },
        doc_path: "docs/verification/schema-producer-audit.md",
        doc_markers: &[
            "implementation_state",
            "static_movement",
            "verification",
            "receipt_state",
            "runtime_mutation",
            "non_claims",
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
        "schemas/ripr/gate-decision.schema.json",
        "schemas/ripr/check.schema.json",
        "schemas/ripr/repair-assurance.schema.json",
        "schemas/ripr/rust-repair-trust-corpus.schema.json",
        "schema-producer-audit.md",
    ] {
        if !readme.contains(required) {
            violations.push(format!("{VERIFICATION_README} does not link `{required}`"));
        }
    }

    let mut subjects_checked = 0usize;
    for contract in CONTRACTS {
        let schema = read_json(root.join(contract.schema_path))?;
        validate_schema_document(contract.schema_path, &schema, &mut violations);

        let subschema = match contract.schema_pointer {
            None => Some(&schema),
            Some(pointer) => match schema.pointer(pointer) {
                Some(value) => Some(value),
                None => {
                    violations.push(format!(
                        "{} does not define the registered subschema `{pointer}`",
                        contract.schema_path
                    ));
                    None
                }
            },
        };

        let fixture = read_json(root.join(contract.fixture_path))?;
        if let Some(subschema) = subschema {
            let subjects = contract.subjects(&fixture, &mut violations);
            // A contract that resolves no subject is a gate that did not run.
            if subjects.is_empty() {
                violations.push(format!(
                    "{} registers {} but resolved no subject to validate",
                    contract.schema_path, contract.fixture_path
                ));
            }
            for (location, value) in subjects {
                subjects_checked += 1;
                validate_value_against_schema(value, subschema, &schema, location, &mut violations);
            }
        }

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

    // Reverse-direction check: every schemas/ripr/*.json must define a
    // schema_version property with a const value. This is the first
    // enforcement step toward #1720 (per-output version reconciliation).
    let ripr_schema_dir = root.join("schemas/ripr");
    if ripr_schema_dir.is_dir() {
        let mut schema_files = fs::read_dir(&ripr_schema_dir)
            .map_err(|error| format!("failed to read schemas/ripr: {error}"))?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    Some(path)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        schema_files.sort();
        for schema_path in &schema_files {
            let rel = schema_path.strip_prefix(&root).unwrap_or(schema_path);
            let rel_str = rel.to_string_lossy();
            let schema = read_json(schema_path.clone())?;
            let props = schema.get("properties").and_then(Value::as_object);
            let Some(props) = props else {
                violations.push(format!("{rel_str} has no properties"));
                continue;
            };
            let Some(sv_prop) = props.get("schema_version") else {
                violations.push(format!(
                    "{rel_str} is missing `schema_version` property — every ripr output schema must declare a version (#1720)"
                ));
                continue;
            };
            if sv_prop.get("const").is_none() {
                violations.push(format!(
                    "{rel_str} schema_version must use `const` for a pinned version (#1720)"
                ));
            }
        }
    }

    if violations.is_empty() {
        println!(
            "verification contracts: checked {} contracts over {subjects_checked} producer subjects",
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

impl VerificationContract {
    /// Resolve the fixture values this contract validates, reporting a
    /// violation for any registered pointer that does not resolve.
    fn subjects<'a>(
        &self,
        fixture: &'a Value,
        violations: &mut Vec<String>,
    ) -> Vec<(String, &'a Value)> {
        match self.subject {
            ContractSubject::Document => vec![(self.fixture_path.to_string(), fixture)],
            ContractSubject::Pointer(pointer) => match fixture.pointer(pointer) {
                Some(value) => vec![(format!("{}{pointer}", self.fixture_path), value)],
                None => {
                    violations.push(format!(
                        "{} does not contain the registered subject `{pointer}`",
                        self.fixture_path
                    ));
                    Vec::new()
                }
            },
            ContractSubject::EachItem { array, item } => {
                let Some(entries) = fixture.pointer(array).and_then(Value::as_array) else {
                    violations.push(format!(
                        "{} does not contain the registered subject array `{array}`",
                        self.fixture_path
                    ));
                    return Vec::new();
                };
                entries
                    .iter()
                    .enumerate()
                    .filter_map(|(index, entry)| {
                        // A case the corpus advertises as invalid is a negative,
                        // not a positive subject. Feeding it here reported it as
                        // a passing subject and inflated the denominator: the
                        // record is schema-valid, because its invalidity is
                        // semantic (root identity, producer-bound command
                        // digest) and enforced by a narrower authority this path
                        // never calls. The advertised negative was therefore
                        // never exercised while the count implied it was.
                        if is_advertised_negative(entry) {
                            return None;
                        }
                        let value = match item {
                            None => Some(entry),
                            Some(item) => entry.pointer(item),
                        };
                        // An entry that does not expose the registered member
                        // is not a subject of this contract; the corpus keeps
                        // patch-shaped negative cases alongside whole records.
                        value.map(|value| {
                            (
                                format!(
                                    "{}{array}/{index}{}",
                                    self.fixture_path,
                                    item.unwrap_or_default()
                                ),
                                value,
                            )
                        })
                    })
                    .collect()
            }
        }
    }
}

pub(crate) fn validate_json_file_against_schema(
    root: &Path,
    value_path: &str,
    schema_path: &str,
) -> Result<(), String> {
    let schema = read_json(root.join(schema_path))?;
    let value = read_json(root.join(value_path))?;
    let mut violations = Vec::new();
    validate_value_against_schema(
        &value,
        &schema,
        &schema,
        value_path.to_string(),
        &mut violations,
    );
    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{value_path} does not match {schema_path}:\n{}",
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
    // `$ref` does not suppress its siblings. A schema may write
    // `{"$ref": "...", "minItems": 1}`, and returning here would silently drop
    // every sibling keyword — an empty array passed `minItems: 1` because this
    // path never reached the array checks. Validate the referenced schema, then
    // fall through so the sibling keywords on this schema object are evaluated
    // too.
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        match resolve_ref(root_schema, reference) {
            Some(resolved) => validate_value_against_schema(
                value,
                resolved,
                root_schema,
                location.clone(),
                violations,
            ),
            None => violations.push(format!(
                "{location}: unresolved schema reference {reference}"
            )),
        }
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

    if let Some(one_of) = schema.get("oneOf").and_then(Value::as_array) {
        let mut messages = Vec::new();
        let matches = one_of
            .iter()
            .filter(|subschema| {
                let mut nested = Vec::new();
                validate_value_against_schema(
                    value,
                    subschema,
                    root_schema,
                    location.clone(),
                    &mut nested,
                );
                let matched = nested.is_empty();
                messages.extend(nested);
                matched
            })
            .count();
        if matches != 1 {
            violations.push(format!(
                "{location}: matched {matches} of {} mutually exclusive schemas ({})",
                one_of.len(),
                messages.join("; ")
            ));
        }
    }

    // `if`/`then`/`else` carry the conditional field requirements that make a
    // state vocabulary closed — a validator that skipped them would accept a
    // state paired with evidence the schema forbids.
    if let Some(condition) = schema.get("if") {
        let mut condition_violations = Vec::new();
        validate_value_against_schema(
            value,
            condition,
            root_schema,
            location.clone(),
            &mut condition_violations,
        );
        let branch = if condition_violations.is_empty() {
            schema.get("then")
        } else {
            schema.get("else")
        };
        if let Some(branch) = branch {
            validate_value_against_schema(value, branch, root_schema, location.clone(), violations);
        }
    }

    if let Some(negated) = schema.get("not") {
        let mut nested = Vec::new();
        validate_value_against_schema(value, negated, root_schema, location.clone(), &mut nested);
        if nested.is_empty() {
            violations.push(format!("{location}: matched a forbidden schema"));
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

/// A corpus case the fixture advertises as invalid is a negative, never a
/// positive contract subject. Shared by the subject selector and its test so
/// the test asserts the retained selection rule rather than restating it.
/// Fail closed on type drift. Matching `as_bool == Some(true)` would let
/// `"invalid": "true"` or `"invalid": 1` fall through as a positive subject —
/// the advertised negative would silently re-enter the passing count, which is
/// the defect this predicate exists to prevent. Any present `invalid` that is
/// not literally `false` is treated as a negative; `assurance_corpus_marks_invalid_with_a_boolean`
/// rejects the non-boolean shape outright so the corpus cannot drift there
/// unnoticed.
fn is_advertised_negative(entry: &Value) -> bool {
    match entry.get("invalid") {
        None => false,
        Some(Value::Bool(flag)) => *flag,
        Some(_) => true,
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

    // `required` is independent of `properties`. A conditional `then` branch is
    // routinely written as `{"required": ["duplicate_of"]}` with no
    // `properties` at all, and returning early on a missing `properties` made
    // every such branch unenforceable — the malformed value was accepted with
    // zero violations.
    for field in string_array(schema.get("required")) {
        if !object.contains_key(&field) {
            violations.push(format!("{location}: missing required field `{field}`"));
        }
    }

    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return;
    };

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
    let Some(items) = value.as_array() else {
        return;
    };

    if let Some(min_items) = schema.get("minItems").and_then(Value::as_u64)
        && (items.len() as u64) < min_items
    {
        violations.push(format!(
            "{location}: array shorter than minItems {min_items}"
        ));
    }
    if let Some(max_items) = schema.get("maxItems").and_then(Value::as_u64)
        && (items.len() as u64) > max_items
    {
        violations.push(format!(
            "{location}: array longer than maxItems {max_items}"
        ));
    }
    if schema.get("uniqueItems").and_then(Value::as_bool) == Some(true) {
        let mut seen = BTreeSet::new();
        for item in items {
            if !seen.insert(compact_json(item)) {
                violations.push(format!(
                    "{location}: uniqueItems violated by duplicate {}",
                    compact_json(item)
                ));
            }
        }
    }

    let Some(items_schema) = schema.get("items") else {
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
    // `pattern` carries the identity commitments — commit SHAs, `sha256:`
    // digests, relative working directories. Compilation is fail-closed, so an
    // uninterpretable pattern is reported rather than assumed satisfied.
    if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
        match SchemaPattern::compile(pattern) {
            Ok(compiled) => {
                if !compiled.is_match(text) {
                    violations.push(format!(
                        "{location}: string does not match pattern {pattern}"
                    ));
                }
            }
            Err(error) => violations.push(format!("{location}: {error}")),
        }
    }
}

fn validate_number(value: &Value, schema: &Value, location: &str, violations: &mut Vec<String>) {
    if let Some(minimum) = schema.get("minimum").and_then(Value::as_f64)
        && let Some(actual) = value.as_f64()
        && actual < minimum
    {
        violations.push(format!("{location}: number below minimum {minimum}"));
    }
    if let Some(maximum) = schema.get("maximum").and_then(Value::as_f64)
        && let Some(actual) = value.as_f64()
        && actual > maximum
    {
        violations.push(format!("{location}: number above maximum {maximum}"));
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
        "number" => value.is_number(),
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

    /// Turning on `if`/`then` evaluation in this validator made the
    /// review-comments conditionals enforceable for the first time, and they
    /// immediately rejected an honest producer packet.
    ///
    /// `error_review_comments_packet` (`xtask/src/reports/review_comments.rs`)
    /// emits `status: "error"` and deliberately retains the producer's
    /// `analysis_outcome` — `error_packet_retains_check_output_analysis_outcome`
    /// pins that on purpose, so a failed run can still say what the analysis
    /// found. Both branches keyed on `analysis_complete` pinned `status` to a
    /// single `const`, so an error packet was rejected whichever outcome it
    /// carried. Observed on this PR's own CI as
    /// `allOf[1].status: expected const "incomplete", got "error"`.
    ///
    /// The relaxation must not degrade into "any status is fine anywhere": the
    /// complete/incomplete split is why these branches exist, so each is
    /// asserted in both directions.
    #[test]
    fn review_comments_schema_admits_error_without_losing_the_completeness_split()
    -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join("schemas/ripr/review-comments.schema.json"))?;
        let base =
            read_json(root.join("tests/fixtures/verification/ripr/review-comments.valid.json"))?;

        // Both outcomes come from real `ripr check` producer fixtures rather
        // than hand-shaped stubs. `analysis_outcome_projection` requires
        // `outcome` beside `analysis_complete` and forbids extra properties, so
        // a synthesized stub would fail for reasons unrelated to `status` and
        // would prove nothing about the branches under test.
        let outcome_for = |complete: bool| -> Result<Value, String> {
            let fixture = if complete {
                "tests/fixtures/verification/ripr/check-complete.valid.json"
            } else {
                "tests/fixtures/verification/ripr/check-incomplete.valid.json"
            };
            let producer = read_json(root.join(fixture))?;
            producer
                .get("analysis_outcome")
                .filter(|outcome| !outcome.is_null())
                .cloned()
                .ok_or_else(|| format!("{fixture} has no analysis_outcome to project"))
        };
        let complete_outcome = outcome_for(true)?;
        let incomplete_outcome = outcome_for(false)?;
        assert_eq!(
            complete_outcome["analysis_complete"],
            serde_json::json!(true),
            "check-complete fixture must actually carry a completed analysis"
        );
        assert_eq!(
            incomplete_outcome["analysis_complete"],
            serde_json::json!(false),
            "check-incomplete fixture must actually carry an incomplete analysis"
        );

        let with = |complete: bool, status: &str| {
            let mut value = base.clone();
            value["status"] = serde_json::json!(status);
            value["analysis_outcome"] = if complete {
                complete_outcome.clone()
            } else {
                incomplete_outcome.clone()
            };
            let mut violations = Vec::new();
            validate_value_against_schema(
                &value,
                &schema,
                &schema,
                format!("review comments {status}/{complete}"),
                &mut violations,
            );
            violations
        };

        assert!(
            with(true, "error").is_empty(),
            "an error packet retaining a completed analysis_outcome must validate: {:#?}",
            with(true, "error")
        );

        // The incomplete case is asserted fully valid: the producer emits one
        // `claim_boundary` sentence unconditionally
        // (`ANALYSIS_OUTCOME_CLAIM_BOUNDARY`, enforced on typed deserialize at
        // crates/ripr/src/analysis_outcome.rs), and both schemas now pin that
        // producer authority, so a real incomplete outcome projects cleanly.
        let incomplete_error = with(false, "error");
        assert!(
            incomplete_error.is_empty(),
            "an error packet retaining an incomplete analysis_outcome must validate: {incomplete_error:#?}"
        );

        // The discrimination that must survive the relaxation.
        assert!(
            !with(false, "advisory").is_empty(),
            "advisory guidance must still be rejected when analysis did not complete"
        );
        assert!(
            !with(true, "incomplete").is_empty(),
            "an incomplete status must still be rejected when analysis completed"
        );
        Ok(())
    }

    /// A conditional `then` branch is routinely written as bare `required`
    /// with no `properties`. `validate_object` used to return early when
    /// `properties` was absent, which made every such branch unenforceable:
    /// the malformed value was accepted with zero violations, so the
    /// conditional looked like a passing check while checking nothing.
    #[test]
    fn required_is_enforced_without_a_properties_sibling() {
        let schema = serde_json::json!({
            "type": "object",
            "allOf": [{
                "if": {
                    "properties": { "classification": { "const": "duplicate_observation" } },
                    "required": ["classification"]
                },
                "then": { "required": ["duplicate_of"] }
            }]
        });

        let missing = serde_json::json!({ "classification": "duplicate_observation" });
        let mut violations = Vec::new();
        validate_value_against_schema(
            &missing,
            &schema,
            &schema,
            "observation".to_string(),
            &mut violations,
        );
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("missing required field `duplicate_of`")),
            "a required-only conditional branch must be enforced: {violations:#?}"
        );

        let present = serde_json::json!({
            "classification": "duplicate_observation",
            "duplicate_of": "obs-1"
        });
        let mut violations = Vec::new();
        validate_value_against_schema(
            &present,
            &schema,
            &schema,
            "observation".to_string(),
            &mut violations,
        );
        assert!(
            violations.is_empty(),
            "a satisfied conditional must not report violations: {violations:#?}"
        );
    }

    /// `$ref` does not suppress its siblings. The validator used to return
    /// immediately after resolving a reference, so a schema written as
    /// `{"$ref": ..., "minItems": 1}` silently accepted an empty array — the
    /// sibling constraint was never reached.
    #[test]
    fn keywords_beside_a_ref_are_still_evaluated() {
        let schema = serde_json::json!({
            "$defs": { "paths": { "type": "array", "items": { "type": "string" } } },
            "type": "object",
            "properties": {
                "source_refs": { "$ref": "#/$defs/paths", "minItems": 1 }
            }
        });

        let empty = serde_json::json!({ "source_refs": [] });
        let mut violations = Vec::new();
        validate_value_against_schema(
            &empty,
            &schema,
            &schema,
            "attempt".to_string(),
            &mut violations,
        );
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("minItems")),
            "a minItems sibling of $ref must be evaluated: {violations:#?}"
        );

        // The referenced schema must still be applied, so a type error inside
        // it is reported rather than lost to the sibling handling.
        let wrong_type = serde_json::json!({ "source_refs": [7] });
        let mut violations = Vec::new();
        validate_value_against_schema(
            &wrong_type,
            &schema,
            &schema,
            "attempt".to_string(),
            &mut violations,
        );
        assert!(
            !violations.is_empty(),
            "the referenced schema must still be applied: {violations:#?}"
        );

        let populated = serde_json::json!({ "source_refs": ["src/lib.rs"] });
        let mut violations = Vec::new();
        validate_value_against_schema(
            &populated,
            &schema,
            &schema,
            "attempt".to_string(),
            &mut violations,
        );
        assert!(
            violations.is_empty(),
            "a satisfied ref-plus-sibling schema must not report violations: {violations:#?}"
        );
    }

    #[test]
    fn complete_check_fixture_matches_schema() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join("schemas/ripr/check.schema.json"))?;
        let fixture =
            read_json(root.join("tests/fixtures/verification/ripr/check-complete.valid.json"))?;
        let mut violations = Vec::new();

        validate_value_against_schema(
            &fixture,
            &schema,
            &schema,
            "complete check fixture".to_string(),
            &mut violations,
        );

        assert!(violations.is_empty(), "{violations:#?}");
        Ok(())
    }

    #[test]
    fn incomplete_check_fixture_matches_schema_without_becoming_complete() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join("schemas/ripr/check.schema.json"))?;
        let fixture =
            read_json(root.join("tests/fixtures/verification/ripr/check-incomplete.valid.json"))?;
        let mut violations = Vec::new();

        validate_value_against_schema(
            &fixture,
            &schema,
            &schema,
            "incomplete check fixture".to_string(),
            &mut violations,
        );

        assert!(violations.is_empty(), "{violations:#?}");
        assert_eq!(fixture["analysis_outcome"]["analysis_complete"], false);
        assert_eq!(fixture["findings"].as_array().map(Vec::len), Some(0));
        Ok(())
    }

    /// #3072: `claim_boundary` is producer-owned — `AnalysisOutcome` assigns
    /// one sentence unconditionally and rejects any other on typed deserialize
    /// (crates/ripr/src/analysis_outcome.rs), so both schemas pin that const.
    /// An invented boundary sentence must still be rejected: relaxing the pin
    /// to free text would remove the check rather than correct it.
    #[test]
    fn check_schema_rejects_an_invented_claim_boundary() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join("schemas/ripr/check.schema.json"))?;
        let mut fixture =
            read_json(root.join("tests/fixtures/verification/ripr/check-incomplete.valid.json"))?;
        fixture["analysis_outcome"]["outcome"]["claim_boundary"] = serde_json::json!(
            "Static analysis is incomplete; an empty findings array is not a clean result."
        );
        let mut violations = Vec::new();

        validate_value_against_schema(
            &fixture,
            &schema,
            &schema,
            "invented claim_boundary".to_string(),
            &mut violations,
        );

        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("claim_boundary")),
            "an invented claim_boundary sentence must be rejected: {violations:#?}"
        );
        Ok(())
    }

    #[test]
    fn limited_check_fixture_matches_schema_without_becoming_consumable() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join("schemas/ripr/check.schema.json"))?;
        let fixture =
            read_json(root.join("tests/fixtures/verification/ripr/check-limited.valid.json"))?;
        let mut violations = Vec::new();

        validate_value_against_schema(
            &fixture,
            &schema,
            &schema,
            "limited check fixture".to_string(),
            &mut violations,
        );

        assert!(violations.is_empty(), "{violations:#?}");
        assert_eq!(fixture["analysis_scope"]["downstream_consumable"], false);
        assert_eq!(
            fixture["run_limitations"][0]["downstream_consumable"],
            false
        );
        Ok(())
    }

    #[test]
    fn check_schema_rejects_unknown_top_level_field() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join("schemas/ripr/check.schema.json"))?;
        let mut fixture =
            read_json(root.join("tests/fixtures/verification/ripr/check-complete.valid.json"))?;
        fixture["unexpected_release_field"] = Value::Bool(true);
        let mut violations = Vec::new();

        validate_value_against_schema(
            &fixture,
            &schema,
            &schema,
            "drifted check fixture".to_string(),
            &mut violations,
        );

        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("unexpected field `unexpected_release_field`")),
            "expected top-level schema drift rejection, got {violations:#?}"
        );
        Ok(())
    }

    #[test]
    fn check_schema_rejects_negative_fractional_confidence() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join("schemas/ripr/check.schema.json"))?;
        let mut fixture =
            read_json(root.join("tests/fixtures/verification/ripr/check-complete.valid.json"))?;
        fixture["findings"][0]["confidence"] = serde_json::json!(-0.5);
        let mut violations = Vec::new();

        validate_value_against_schema(
            &fixture,
            &schema,
            &schema,
            "fractional confidence fixture".to_string(),
            &mut violations,
        );

        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("number below minimum")),
            "expected fractional minimum rejection, got {violations:#?}"
        );
        Ok(())
    }

    #[test]
    fn assertion_guidance_schema_rejects_invalid_state_shapes() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join("schemas/ripr/review-comments.schema.json"))?;
        let guidance_schema = schema
            .pointer("/$defs/suggested_test/properties/assertion_guidance")
            .ok_or_else(|| "assertion guidance schema is missing".to_string())?;

        let validate = |value: Value| {
            let mut violations = Vec::new();
            validate_value_against_schema(
                &value,
                guidance_schema,
                &schema,
                "assertion guidance".to_string(),
                &mut violations,
            );
            violations
        };

        let valid_concrete = serde_json::json!({
            "state": "concrete",
            "example": "assert_eq!(value, expected)",
            "kind": "exact_return_value",
            "basis": "seam_required_discriminator",
            "observer_kind": null,
            "reason": null,
            "recovery": null
        });
        assert!(validate(valid_concrete).is_empty());

        let invalid_concrete = serde_json::json!({
            "state": "concrete",
            "example": null,
            "kind": "exact_return_value",
            "basis": "seam_required_discriminator",
            "observer_kind": null,
            "reason": null,
            "recovery": null
        });
        assert!(!validate(invalid_concrete).is_empty());

        let invalid_non_concrete = serde_json::json!({
            "state": "stale",
            "example": "assert_eq!(value, expected)",
            "kind": null,
            "basis": null,
            "observer_kind": null,
            "reason": "snapshot_stale",
            "recovery": "refresh_analysis"
        });
        assert!(!validate(invalid_non_concrete).is_empty());

        let invalid_observer_setup = serde_json::json!({
            "state": "requires_observer_setup",
            "example": null,
            "kind": null,
            "basis": null,
            "observer_kind": null,
            "reason": "observer_not_statically_visible",
            "recovery": null
        });
        assert!(!validate(invalid_observer_setup).is_empty());
        Ok(())
    }

    const TRUST_CORPUS: &str = "metrics/rust-repair-trust/corpus.json";
    const TRUST_CORPUS_SCHEMA: &str = "schemas/ripr/rust-repair-trust-corpus.schema.json";
    const ASSURANCE_SCHEMA: &str = "schemas/ripr/repair-assurance.schema.json";
    const ASSURANCE_CORPUS: &str = "fixtures/assurance_vocabulary/assurance/corpus.json";
    const AGENT_PACKET_GOLDEN: &str =
        "fixtures/boundary_gap/expected/editor-agent-loop/agent-packet.json";

    fn violations_for(value: &Value, subschema: &Value, root_schema: &Value) -> Vec<String> {
        let mut violations = Vec::new();
        validate_value_against_schema(
            value,
            subschema,
            root_schema,
            "subject".to_string(),
            &mut violations,
        );
        violations
    }

    fn subschema<'a>(schema: &'a Value, pointer: &str) -> Result<&'a Value, String> {
        schema
            .pointer(pointer)
            .ok_or_else(|| format!("schema must define {pointer}"))
    }

    /// The corpus of record — the exact bytes `cargo xtask rust-repair-trust`
    /// reads — must satisfy its published schema. Validating a hand-written
    /// copy instead would let the schema confirm itself.
    #[test]
    fn live_rust_repair_trust_corpus_matches_its_schema() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join(TRUST_CORPUS_SCHEMA))?;
        let corpus = read_json(root.join(TRUST_CORPUS))?;

        assert!(
            violations_for(&corpus, &schema, &schema).is_empty(),
            "{:#?}",
            violations_for(&corpus, &schema, &schema)
        );
        // Which member arrays carry live data decides what this contract
        // actually exercises. `cases` is empty today, so the attempt
        // subschema is registered but unexercised; that must stay visible
        // rather than be read as coverage.
        assert_eq!(corpus["cases"].as_array().map(Vec::len), Some(0));
        assert!(
            corpus["exclusions"]
                .as_array()
                .is_some_and(|entries| !entries.is_empty())
        );
        assert!(
            corpus["observations"]
                .as_array()
                .is_some_and(|entries| !entries.is_empty())
        );
        Ok(())
    }

    /// An entry that cannot be attributed to an exact revision is the corpus
    /// failure that matters: `analyzed_head_sha` is what makes a receipt-backed
    /// observation checkable at all.
    #[test]
    fn rust_repair_trust_corpus_rejects_a_non_sha_analyzed_head() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join(TRUST_CORPUS_SCHEMA))?;
        let mut corpus = read_json(root.join(TRUST_CORPUS))?;
        corpus["exclusions"][0]["analyzed_head_sha"] = Value::String("main".to_string());

        let violations = violations_for(&corpus, &schema, &schema);
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("does not match pattern")),
            "expected a head-SHA pattern rejection, got {violations:#?}"
        );
        Ok(())
    }

    /// The generated agent packet carries real `CommandSpec` bytes from
    /// `crate::agent::command_specs`, so the published command-spec contract is
    /// checked against output the product actually emitted.
    #[test]
    fn producer_command_specs_match_the_published_contract() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join(ASSURANCE_SCHEMA))?;
        let packet = read_json(root.join(AGENT_PACKET_GOLDEN))?;
        let specs = packet
            .pointer("/packets/0/evidence_record/canonical_item/command_specs")
            .ok_or("the golden packet must carry producer command specs")?;

        let verify = specs.get("verify").ok_or("missing verify spec")?;
        let receipt = specs.get("receipt").ok_or("missing receipt spec")?;
        assert!(
            violations_for(
                verify,
                subschema(&schema, "/$defs/verification_command_spec")?,
                &schema
            )
            .is_empty()
        );
        assert!(
            violations_for(receipt, subschema(&schema, "/$defs/command_spec")?, &schema).is_empty()
        );
        Ok(())
    }

    /// A command spec that names an absolute working directory escapes the
    /// declared root. The schema's `pattern` is the only place that constraint
    /// is expressed, so the contract must reject it.
    #[test]
    fn command_spec_contract_rejects_an_absolute_working_directory() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join(ASSURANCE_SCHEMA))?;
        let packet = read_json(root.join(AGENT_PACKET_GOLDEN))?;
        let mut receipt = packet
            .pointer("/packets/0/evidence_record/canonical_item/command_specs/receipt")
            .ok_or("the golden packet must carry a receipt command spec")?
            .clone();
        receipt["working_directory"] = Value::String(r"drive-letter:\workspace".to_string());

        let violations = violations_for(
            &receipt,
            subschema(&schema, "/$defs/command_spec")?,
            &schema,
        );
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("does not match pattern")),
            "expected a working-directory rejection, got {violations:#?}"
        );
        Ok(())
    }

    /// The verify subschema exists to stop a receipt route from being executed
    /// through the verification authority boundary.
    #[test]
    fn verification_command_spec_contract_rejects_a_receipt_route() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join(ASSURANCE_SCHEMA))?;
        let packet = read_json(root.join(AGENT_PACKET_GOLDEN))?;
        let mut verify = packet
            .pointer("/packets/0/evidence_record/canonical_item/command_specs/verify")
            .ok_or("the golden packet must carry a verify command spec")?
            .clone();
        verify["role"] = Value::String("receipt".to_string());

        let violations = violations_for(
            &verify,
            subschema(&schema, "/$defs/verification_command_spec")?,
            &schema,
        );
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("expected const \"verify\"")),
            "expected a verify-role rejection, got {violations:#?}"
        );
        Ok(())
    }

    /// Every design-corpus case must expose a subject the contract can reach,
    /// so a case cannot be silently skipped by the registered pointer.
    #[test]
    fn assurance_corpus_cases_all_expose_a_contract_subject() -> Result<(), String> {
        let root = repo_root()?;
        let corpus = read_json(root.join(ASSURANCE_CORPUS))?;
        let cases = corpus["cases"]
            .as_array()
            .ok_or("the assurance corpus must declare cases")?;
        assert!(!cases.is_empty());
        for case in cases {
            assert!(
                case.get("record").is_some() || case.get("record_patch").is_some(),
                "case {:?} exposes neither a record nor a record patch",
                case.get("id")
            );
        }
        Ok(())
    }

    /// Every case the corpus advertises as invalid must say why, and must not
    /// be counted as a passing positive subject.
    ///
    /// `wrong_root` and `fabricated_result` are schema-valid: their invalidity
    /// is semantic — root identity and producer-bound command-spec digest — and
    /// is enforced by `VerificationExecutionResultV1::validate_against`, which
    /// the contract walk never calls. Feeding them as positive subjects reported
    /// them as passing, so the corpus claimed negative coverage it did not
    /// deliver.
    ///
    /// This pins the honest boundary: an invalid case is excluded from positive
    /// subjects, and it must declare either a schema-expressible negative
    /// (`record_patch`, exercised by the test below) or an `expected_failure`
    /// token naming the narrower authority that rejects it. Silence is not an
    /// option, because silence is what let this pass.
    #[test]
    fn assurance_corpus_invalid_cases_declare_their_authority() -> Result<(), String> {
        let root = repo_root()?;
        let corpus = read_json(root.join(ASSURANCE_CORPUS))?;
        let cases = corpus["cases"]
            .as_array()
            .ok_or("the assurance corpus must declare cases")?;

        let invalid = cases
            .iter()
            .filter(|case| case.get("invalid").and_then(Value::as_bool) == Some(true))
            .collect::<Vec<_>>();
        assert!(
            !invalid.is_empty(),
            "the corpus must retain advertised negative cases"
        );

        for case in &invalid {
            let id = case
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("<unnamed>");
            let schema_expressible = case.get("record_patch").is_some();
            let declares_authority = case
                .get("expected_failure")
                .and_then(Value::as_str)
                .is_some_and(|token| !token.is_empty());
            assert!(
                schema_expressible || declares_authority,
                "invalid case `{id}` declares neither a record_patch the schema can reject \
                 nor an expected_failure naming the authority that rejects it"
            );
        }

        // Exercise the real selector, not the predicate. Asserting
        // `is_advertised_negative` here would restate the rule and pass even if
        // the selector stopped applying it.
        let contract = CONTRACTS
            .iter()
            .find(|contract| contract.fixture_path == ASSURANCE_CORPUS)
            .ok_or("the assurance corpus must be a registered contract")?;
        let mut violations = Vec::new();
        let subjects = contract.subjects(&corpus, &mut violations);
        assert!(
            violations.is_empty(),
            "subject selection reported violations: {violations:#?}"
        );
        assert!(
            !subjects.is_empty(),
            "the corpus must still yield positive subjects"
        );

        let invalid_records = invalid
            .iter()
            .filter_map(|case| case.get("record"))
            .collect::<Vec<_>>();
        for record in &invalid_records {
            assert!(
                !subjects.iter().any(|(_, value)| *value == *record),
                "an advertised negative reached positive subject selection"
            );
        }
        // A case without `/record` (patch-shaped) was already excluded by the
        // missing-pointer filter, so the expected count is the cases that both
        // carry a record and are not advertised negatives.
        let expected = cases
            .iter()
            .filter(|case| !is_advertised_negative(case) && case.get("record").is_some())
            .count();
        assert_eq!(
            subjects.len(),
            expected,
            "positive subject count must be exactly the valid record-carrying cases"
        );
        Ok(())
    }

    /// The `invalid` marker decides whether a case is excluded from positive
    /// subjects, so its type is load-bearing. A drift to `"invalid": "true"`
    /// must be rejected outright rather than silently reinterpreted, and
    /// `is_advertised_negative` must fail closed on any non-boolean it does see.
    #[test]
    fn assurance_corpus_marks_invalid_with_a_boolean() -> Result<(), String> {
        let root = repo_root()?;
        let corpus = read_json(root.join(ASSURANCE_CORPUS))?;
        let cases = corpus["cases"]
            .as_array()
            .ok_or("the assurance corpus must declare cases")?;

        for case in cases {
            if let Some(marker) = case.get("invalid") {
                let id = case
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("<unnamed>");
                assert!(
                    marker.is_boolean(),
                    "case `{id}` declares a non-boolean `invalid` marker {marker};                      the exclusion rule is only meaningful for a boolean"
                );
            }
        }

        // The predicate itself must not reopen the hole the corpus check closes.
        for drifted in [
            serde_json::json!({ "invalid": "true" }),
            serde_json::json!({ "invalid": 1 }),
            serde_json::json!({ "invalid": serde_json::Value::Null }),
        ] {
            assert!(
                is_advertised_negative(&drifted),
                "a non-boolean `invalid` marker must fail closed: {drifted}"
            );
        }
        assert!(!is_advertised_negative(&serde_json::json!({
            "invalid": false
        })));
        assert!(!is_advertised_negative(&serde_json::json!({})));
        Ok(())
    }

    /// The corpus declares its own negative: an absolute working directory in a
    /// command spec. Applying the declared patch must fail the envelope schema,
    /// or the design-only contract enforces nothing.
    #[test]
    fn assurance_corpus_rejects_an_absolute_command_working_directory() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join(ASSURANCE_SCHEMA))?;
        let corpus = read_json(root.join(ASSURANCE_CORPUS))?;
        let cases = corpus["cases"]
            .as_array()
            .ok_or("the assurance corpus must declare cases")?;
        let patch = cases
            .iter()
            .find(|case| case.get("id").and_then(Value::as_str) == Some("malformed_command_spec"))
            .and_then(|case| case.get("record_patch"))
            .ok_or("the corpus must declare the malformed command spec case")?;
        let mut record = cases
            .iter()
            .find(|case| case.get("id").and_then(Value::as_str) == Some("unchanged_pass"))
            .and_then(|case| case.get("record"))
            .ok_or("the corpus must declare a valid baseline record")?
            .clone();
        assert!(violations_for(&record, &schema, &schema).is_empty());

        let patch = patch
            .as_object()
            .ok_or("the record patch must be an object")?;
        for (field, value) in patch {
            record[field] = value.clone();
        }

        let violations = violations_for(&record, &schema, &schema);
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("does not match pattern")),
            "expected the declared malformed spec to be rejected, got {violations:#?}"
        );
        Ok(())
    }

    /// `if`/`then` carries the closure of the assurance state vocabulary. A
    /// validator that skipped it would accept `verification_not_run` paired
    /// with the command spec and result that state forbids.
    #[test]
    fn assurance_schema_rejects_evidence_its_state_forbids() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join(ASSURANCE_SCHEMA))?;
        let corpus = read_json(root.join(ASSURANCE_CORPUS))?;
        let cases = corpus["cases"]
            .as_array()
            .ok_or("the assurance corpus must declare cases")?;
        let mut record = cases
            .iter()
            .find(|case| case.get("id").and_then(Value::as_str) == Some("unchanged_pass"))
            .and_then(|case| case.get("record"))
            .ok_or("the corpus must declare a valid executed-pass record")?
            .clone();
        record["verification"]["state"] = Value::String("verification_not_run".to_string());

        let violations = violations_for(&record, &schema, &schema);
        assert!(
            !violations.is_empty(),
            "a not-run state carrying an execution result must be rejected"
        );
        Ok(())
    }

    /// A registered contract that resolves no subject is a gate that did not
    /// run, not a gate that passed.
    #[test]
    fn a_contract_pointer_that_resolves_nothing_is_a_violation() {
        let contract = VerificationContract {
            schema_path: ASSURANCE_SCHEMA,
            schema_pointer: None,
            fixture_path: ASSURANCE_CORPUS,
            subject: ContractSubject::Pointer("/cases/9999/record"),
            doc_path: "docs/verification/schema-producer-audit.md",
            doc_markers: &[],
        };
        let mut violations = Vec::new();
        let fixture = serde_json::json!({"cases": []});
        let subjects = contract.subjects(&fixture, &mut violations);

        assert!(subjects.is_empty());
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("does not contain the registered subject")),
            "{violations:#?}"
        );
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
