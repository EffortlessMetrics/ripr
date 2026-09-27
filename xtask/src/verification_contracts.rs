use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::schema_pattern::SchemaPattern;

const VERIFICATION_README: &str = "docs/verification/README.md";
const SCHEMA_PRODUCER_AUDIT: &str = "docs/verification/schema-producer-audit.md";
const SCHEMAS_DIRECTORY: &str = "schemas";
const RIPR_SCHEMA_PREFIX: &str = "schemas/ripr/";
/// Every JSON file under `schemas/` is a published schema. Matching the
/// broader extension keeps the `schema_version` rule below over exactly the
/// set it covered before the inventory was derived, and makes a non-schema
/// parked in `schemas/` declare itself in the audit rather than slip past.
const SCHEMA_FILE_SUFFIX: &str = ".json";

const PR_EVIDENCE_INSTALLED_PRODUCER: &str = "crates/ripr/src/app/pr_evidence.rs";
const PR_EVIDENCE_VERSION_PREFIX: &str = "json!({\n        \"schema_version\": \"";

/// One producer that writes a published schema version into its output.
struct VersionProducer {
    source_path: &'static str,
    /// Text immediately preceding each emitted version literal.
    prefix: &'static str,
    occurrences: usize,
}

const fn producer(
    source_path: &'static str,
    prefix: &'static str,
    occurrences: usize,
) -> VersionProducer {
    VersionProducer {
        source_path,
        prefix,
        occurrences,
    }
}

// Each published RIPR schema lists every producer that emits its version.
// Keep this inventory exhaustive: a newly published schema needs an explicit
// owner, and a schema written by more than one producer (for example both the
// installed `ripr` binary and unpublished xtask automation) lists each one so
// no producer can drift unreconciled.
const VERSION_AUTHORITIES: &[(&str, &[VersionProducer])] = &[
    (
        "check",
        &[producer(
            "crates/ripr/src/app.rs",
            "const CHECK_OUTPUT_SCHEMA_VERSION: &str = \"",
            1,
        )],
    ),
    (
        "gate-decision",
        &[producer(
            "crates/ripr/src/output/gate.rs",
            "const SCHEMA_VERSION: &str = \"",
            1,
        )],
    ),
    (
        "pr-evidence",
        &[
            // Installed `ripr pr-evidence`: success and error packets each
            // pin the public envelope.
            producer(
                PR_EVIDENCE_INSTALLED_PRODUCER,
                PR_EVIDENCE_VERSION_PREFIX,
                2,
            ),
            // Unpublished xtask compatibility producer, same two packets.
            producer(
                "xtask/src/reports/pr_evidence.rs",
                PR_EVIDENCE_VERSION_PREFIX,
                2,
            ),
        ],
    ),
    (
        "repair-assurance",
        &[producer(
            "crates/ripr/src/domain/verification_result.rs",
            "pub const VERIFICATION_EXECUTION_RESULT_SCHEMA_VERSION: &str = \"",
            1,
        )],
    ),
    (
        "repair-attempt",
        &[producer(
            "crates/ripr/src/app/repair_attempt.rs",
            "const REPAIR_ATTEMPT_SCHEMA_VERSION: &str = \"",
            1,
        )],
    ),
    (
        "review-comments",
        &[producer(
            "crates/ripr/src/output/review_comments.rs",
            "const REVIEW_COMMENTS_SCHEMA_VERSION: &str = \"",
            1,
        )],
    ),
    (
        "ripr-agent-capability",
        &[producer(
            "crates/ripr/src/lsp/agent_protocol.rs",
            "const RIPR_AGENT_SCHEMA_VERSION: &str = \"",
            1,
        )],
    ),
    (
        "ripr-agent-error",
        &[producer(
            "crates/ripr/src/lsp/agent_protocol.rs",
            "const RIPR_AGENT_SCHEMA_VERSION: &str = \"",
            1,
        )],
    ),
    (
        "ripr-agent-request",
        &[producer(
            "crates/ripr/src/lsp/agent_protocol.rs",
            "const RIPR_AGENT_SCHEMA_VERSION: &str = \"",
            1,
        )],
    ),
    (
        "ripr-agent-success",
        &[producer(
            "crates/ripr/src/lsp/agent_protocol.rs",
            "const RIPR_AGENT_SCHEMA_VERSION: &str = \"",
            1,
        )],
    ),
    (
        "rust-repair-trust-corpus",
        &[producer(
            "xtask/src/reports/rust_repair_trust.rs",
            "get(\"schema_version\").and_then(Value::as_str) == Some(\"",
            1,
        )],
    ),
];

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
    // The `--gap-ledger` route writes the same review-comments artifact but
    // renders an `inputs.gap_ledger` disclosure, eligible gap-record cards,
    // and gap-variant `suppressed[]` items (`gap_id`, nullable `file`/`line`)
    // that the diff-scoped fixture above never carries; the generation-time `--check` in
    // `xtask/src/reports/review_comments.rs` shells `ripr review-comments`
    // without `--gap-ledger`, so nothing consumed this shape before this row
    // while the closed published schema rejected it.
    VerificationContract {
        schema_path: "schemas/ripr/review-comments.schema.json",
        schema_pointer: None,
        fixture_path: "tests/fixtures/verification/ripr/review-comments.gap-ledger.valid.json",
        subject: ContractSubject::Document,
        doc_path: "docs/OUTPUT_SCHEMA.md",
        doc_markers: &[
            "inputs",
            "gap_ledger",
            "gap_record_anchor",
            "comments[].gap_id",
            "gap_repair_card",
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
    // `baseline_match_kind` is emitted only when a baseline match succeeded
    // through the legacy path/line/static_class fallback selector (issue
    // #1934, RIPR-SPEC-0014 § Baseline Comparison), so the hand-written
    // fixture above never carries it. This golden does, and it carries a
    // canonical-match decision beside it, so one subject binds both that the
    // schema admits the field and that it does not require it.
    VerificationContract {
        schema_path: "schemas/ripr/gate-decision.schema.json",
        schema_pointer: None,
        fixture_path: concat!(
            "fixtures/gate_baseline_fallback_disclosure/expected/gate-baseline/",
            "mixed-canonical-and-legacy-entries/gate-decision.json"
        ),
        subject: ContractSubject::Document,
        doc_path: "docs/OUTPUT_SCHEMA.md",
        doc_markers: &[
            "decisions[].baseline_match_kind",
            "\"legacy_path_line_class\"",
        ],
    },
    // `causal_comparison` and the per-decision canonical delta fields
    // (`delta_attribution`, `base_state`, `head_state`, `attribution_basis`,
    // `comparison_confidence`) are projected whenever the canonical PR delta
    // artifact loads in the gate path
    // (`crates/ripr/src/app/causal_projection.rs`), and `cargo xtask ripr-pr`
    // writes that artifact deterministically, so the standard workflow emits
    // gate-decision JSON carrying these fields. The hand-written fixture
    // above never carries them, so before this row the closed published
    // schema rejected live gate output whenever the delta artifact existed.
    VerificationContract {
        schema_path: "schemas/ripr/gate-decision.schema.json",
        schema_pointer: None,
        fixture_path: "tests/fixtures/verification/ripr/gate-decision.causal-delta.valid.json",
        subject: ContractSubject::Document,
        doc_path: "docs/OUTPUT_SCHEMA.md",
        doc_markers: &[
            "causal_comparison",
            "delta_attribution",
            "base_state",
            "head_state",
            "attribution_basis",
            "comparison_confidence",
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
    // The `test_harnesses` projection is checked against a producer golden
    // rather than a hand-written copy, for the reason stated below about the
    // trust corpus: a hand-written instance lets the schema confirm itself
    // while the bytes the product emits drift away. Both hand-written check
    // fixtures above are harness-free, so before this row the published
    // schema rejected real `ripr check --format json` output from any
    // repository with `[analysis.test_harnesses]` registrations and the gate
    // stayed green (#3883).
    VerificationContract {
        schema_path: "schemas/ripr/check.schema.json",
        schema_pointer: None,
        fixture_path: "fixtures/harness_dead_construction_no_exposed_credit/expected/check.json",
        subject: ContractSubject::Document,
        doc_path: "docs/OUTPUT_SCHEMA.md",
        doc_markers: &[
            "test_harnesses",
            "registration_id",
            "harness_kind",
            "adapter",
            "provenance",
            "limitations[]",
        ],
    },
    // Two more producer goldens, for the two shapes the published schema
    // rejected until #3912. The hand-written fixtures above carry neither, so
    // nothing in the contract table consumed them and the gate stayed green
    // while `ripr check --format json` emitted output the schema refused.
    //
    // `flow_sink: null` is the producer's answer when a missing discriminator
    // has no local flow sink to name. 13 goldens carry it and
    // `docs/OUTPUT_SCHEMA.md` documents it, but the schema demanded an object.
    VerificationContract {
        schema_path: "schemas/ripr/check.schema.json",
        schema_pointer: None,
        fixture_path: "fixtures/helper_chain_one_hop/expected/check.json",
        subject: ContractSubject::Document,
        doc_path: "docs/OUTPUT_SCHEMA.md",
        doc_markers: &["flow_sink", "missing_discriminators"],
    },
    // An empty `recommended_next_step` is the producer's "no action to
    // recommend". 40 goldens carry it, but the schema demanded `minLength: 1`.
    VerificationContract {
        schema_path: "schemas/ripr/check.schema.json",
        schema_pointer: None,
        fixture_path: "fixtures/match_arm_positive/expected/check.json",
        subject: ContractSubject::Document,
        doc_path: "docs/OUTPUT_SCHEMA.md",
        doc_markers: &["recommended_next_step", "suggested_next_action"],
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
    let audit = read_text(root.join(SCHEMA_PRODUCER_AUDIT))?;
    let mut violations = Vec::new();

    for required in [
        "badge-contract.md",
        "pr-evidence-contract.md",
        "artifact-layout.md",
        "annotation-policy.md",
        "schema-producer-audit.md",
    ] {
        if !readme.contains(required) {
            violations.push(format!("{VERIFICATION_README} does not link `{required}`"));
        }
    }

    // The published-schema inventory is read from disk rather than listed
    // here. A literal list is a second place to remember, and the schema it
    // forgets is the one no reader ever learns is unaudited: both documents
    // state that they cover every published schema, so a schema missing from
    // either made that claim false while this gate reported success.
    let published_schemas = published_schema_paths(&root)?;
    if published_schemas.is_empty() {
        violations.push(format!(
            "{} contains no published schema, so the audited inventory is empty",
            SCHEMAS_DIRECTORY
        ));
    }
    violations.extend(schema_documentation_violations(
        &published_schemas,
        &readme,
        &audit,
    ));

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

    let registered = version_authority_violations(
        &root,
        &published_schemas,
        &|path| read_text(root.join(path)),
        &mut violations,
    )?;
    // Reverse-direction check: every published RIPR schema declares a pinned
    // version and has exactly one reconciled authority.
    for rel_str in published_schemas
        .iter()
        .filter(|path| path.starts_with(RIPR_SCHEMA_PREFIX))
    {
        let name = rel_str
            .strip_prefix(RIPR_SCHEMA_PREFIX)
            .and_then(|name| name.strip_suffix(".schema.json"));
        if !name.is_some_and(|name| registered.contains(name)) {
            violations.push(format!("{rel_str} has no registered version authority"));
        }
        let schema = read_json(root.join(rel_str))?;
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

    if violations.is_empty() {
        // The audited count is reported beside the contract count so a run
        // cannot imply coverage of an inventory it never enumerated.
        println!(
            "verification contracts: checked {} contracts over {subjects_checked} producer subjects; audited {} published schemas",
            CONTRACTS.len(),
            published_schemas.len()
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

/// Reconcile every registered producer with its published schema and the
/// consumer-facing version table, returning the registered schema names.
///
/// Producer sources are read through `read_source` so a test can mutate one
/// producer in isolation and prove the gate notices.
fn version_authority_violations(
    root: &Path,
    published_schemas: &[String],
    read_source: &dyn Fn(&str) -> Result<String, String>,
    violations: &mut Vec<String>,
) -> Result<BTreeSet<&'static str>, String> {
    let version_doc = read_text(root.join("docs/OUTPUT_SCHEMA.md"))?;
    // Audit the disk inventory in both directions, then compare each producer,
    // the published schema, and the consumer-facing version table per row.
    let mut registered = BTreeSet::new();
    for (name, producers) in VERSION_AUTHORITIES {
        if !registered.insert(*name) {
            violations.push(format!("duplicate schema version authority: {name}"));
        }
        let schema_path = format!("schemas/ripr/{name}.schema.json");
        if !published_schemas.contains(&schema_path) {
            violations.push(format!(
                "{schema_path} has an authority but is not published"
            ));
            continue;
        }
        if producers.is_empty() {
            violations.push(format!("{schema_path} has no registered producer"));
            continue;
        }
        let schema = read_json(root.join(&schema_path))?;
        let pinned = schema
            .pointer("/properties/schema_version/const")
            .and_then(Value::as_str);
        for producer in *producers {
            let source = read_source(producer.source_path)?;
            if let Some(violation) = version_mismatch(
                &schema_path,
                pinned,
                producer.source_path,
                &source,
                producer.prefix,
                producer.occurrences,
                &version_doc,
            ) {
                violations.push(violation);
            }
        }
    }
    Ok(registered)
}

fn version_mismatch(
    schema_path: &str,
    pinned: Option<&str>,
    source_path: &str,
    source: &str,
    prefix: &str,
    expected_occurrences: usize,
    version_doc: &str,
) -> Option<String> {
    let producers = source
        .split(prefix)
        .skip(1)
        .filter_map(|rest| rest.split_once('"').map(|(version, _)| version))
        .collect::<Vec<_>>();
    let doc_row_prefix = format!("| `{schema_path}` | `");
    let documented = version_doc
        .lines()
        .filter_map(|line| {
            line.strip_prefix(&doc_row_prefix)?
                .split_once('`')
                .map(|(version, _)| version)
        })
        .collect::<Vec<_>>();
    if pinned.is_some()
        && producers.len() == expected_occurrences
        && producers.iter().all(|version| Some(*version) == pinned)
        && documented.len() == 1
        && documented[0] == pinned.unwrap_or("")
    {
        None
    } else {
        Some(format!(
            "{schema_path} version mismatch: schema={pinned:?}, {source_path}={producers:?} (expected {expected_occurrences} producer occurrences), docs/OUTPUT_SCHEMA.md={documented:?}"
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

/// Every published schema under `schemas/`, as repository-relative paths with
/// forward slashes, sorted.
///
/// This is the inventory both verification documents claim to cover. Deriving
/// it from disk is what makes that claim checkable: a schema added without a
/// README link or an audit row is reported here instead of sitting unaudited
/// behind a passing gate.
fn published_schema_paths(root: &Path) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    collect_published_schemas(root, &root.join(SCHEMAS_DIRECTORY), &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn collect_published_schemas(
    root: &Path,
    directory: &Path,
    paths: &mut Vec<String>,
) -> Result<(), String> {
    if !directory.is_dir() {
        return Ok(());
    }
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| format!("failed to read a {SCHEMAS_DIRECTORY} entry: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_published_schemas(root, &path, paths)?;
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.ends_with(SCHEMA_FILE_SUFFIX) {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|error| format!("{} is outside the repository: {error}", path.display()))?;
        paths.push(relative.to_string_lossy().replace('\\', "/"));
    }
    Ok(())
}

/// Reports each published schema that one of the two verification documents
/// does not mention.
///
/// The two rules stay separate on purpose. The README says where a schema
/// lives; the audit says which producer emits the bytes it describes and which
/// negative mutation proves that. A schema listed in one and absent from the
/// other is the case that actually occurred, so collapsing them into a single
/// "documented somewhere" test would have reported nothing.
fn schema_documentation_violations(
    published_schemas: &[String],
    readme: &str,
    audit: &str,
) -> Vec<String> {
    let mut violations = Vec::new();
    for schema_path in published_schemas {
        if !lists_schema_in_a_table_row(readme, schema_path) {
            violations.push(format!(
                "{VERIFICATION_README} has no schema-table row linking `{schema_path}`"
            ));
        }
        if !lists_schema_in_a_table_row(audit, schema_path) {
            violations.push(format!(
                "{SCHEMA_PRODUCER_AUDIT} has no audit-table row for `{schema_path}` — every published schema must name its producer and canonical subject, or carry an explicit exemption"
            ));
        }
    }
    violations
}

/// Whether `document` carries `schema_path` as a cell of a Markdown table
/// row, which is how both verification documents list a schema.
///
/// Searching the whole document instead would accept a passing mention
/// anywhere in it — a sentence, a note, a code block — while the violation
/// that silences claims a row exists. That is the same overstated contract
/// this gate exists to close, one level down.
///
/// The cell is matched with its backticks, so a longer path that merely
/// begins with this one, such as a `.bak` copy, cannot stand in for it.
fn lists_schema_in_a_table_row(document: &str, schema_path: &str) -> bool {
    let cell = format!("`{schema_path}`");
    document
        .lines()
        .any(|line| line.trim_start().starts_with('|') && line.contains(&cell))
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
    fn schema_version_reconciliation_rejects_each_drift_and_missing_rows() {
        let path = "schemas/ripr/example.schema.json";
        let source_path = "producer.rs";
        let prefix = "const VERSION: &str = \"";
        let source = "const VERSION: &str = \"0.1\";";
        let doc = "| `schemas/ripr/example.schema.json` | `0.1` | producer |";
        let check = |pinned: Option<&str>, source: &str, doc: &str| {
            version_mismatch(path, pinned, source_path, source, prefix, 1, doc)
        };

        assert!(check(Some("0.1"), source, doc).is_none());
        assert_eq!(
            check(Some("0.2"), source, doc).as_deref(),
            Some(
                "schemas/ripr/example.schema.json version mismatch: schema=Some(\"0.2\"), producer.rs=[\"0.1\"] (expected 1 producer occurrences), docs/OUTPUT_SCHEMA.md=[\"0.1\"]"
            )
        );
        assert!(check(Some("0.1"), "const VERSION: &str = \"0.2\";", doc).is_some());
        assert!(
            check(
                Some("0.1"),
                source,
                "| `schemas/ripr/example.schema.json` | `0.2` | producer |"
            )
            .is_some()
        );
        assert!(check(Some("0.1"), source, "").is_some());
        assert!(check(Some("0.1"), source, &format!("{doc}\n{doc}")).is_some());
        assert!(check(None, source, doc).is_some());
        assert!(check(Some("0.1"), &format!("{source}\n{source}"), doc).is_some());
        assert!(
            version_mismatch(
                path,
                Some("0.1"),
                source_path,
                &format!("{source}\nconst VERSION: &str = \"0.2\";"),
                prefix,
                2,
                doc,
            )
            .is_some()
        );
    }

    /// Mutate only the installed `ripr pr-evidence` producer and require the
    /// real inventory to reject it, so the unpublished xtask copy cannot stand
    /// in for the binary users actually run.
    #[test]
    fn installed_pr_evidence_producer_drift_fails_the_gate() -> Result<(), String> {
        let root = repo_root()?;
        let published = published_schema_paths(&root)?;
        let reconcile = |mutate: bool| -> Result<Vec<String>, String> {
            let read_source = |path: &str| -> Result<String, String> {
                let source = read_text(root.join(path))?;
                if !mutate || path != PR_EVIDENCE_INSTALLED_PRODUCER {
                    return Ok(source);
                }
                let current = format!("{PR_EVIDENCE_VERSION_PREFIX}0.1\"");
                if source.matches(&current).count() != 2 {
                    return Err(format!("{path} no longer emits two `0.1` packets"));
                }
                Ok(source.replacen(&current, &format!("{PR_EVIDENCE_VERSION_PREFIX}9.9\""), 1))
            };
            let mut violations = Vec::new();
            version_authority_violations(&root, &published, &read_source, &mut violations)?;
            Ok(violations)
        };

        let clean = reconcile(false)?;
        if !clean.is_empty() {
            return Err(format!("the unmutated tree must reconcile: {clean:?}"));
        }
        let drifted = reconcile(true)?;
        let expected = format!(
            "schemas/ripr/pr-evidence.schema.json version mismatch: schema=Some(\"0.1\"), {PR_EVIDENCE_INSTALLED_PRODUCER}=[\"9.9\", \"0.1\"] (expected 2 producer occurrences), docs/OUTPUT_SCHEMA.md=[\"0.1\"]"
        );
        if drifted != [expected.clone()] {
            return Err(format!("expected only `{expected}`, got {drifted:?}"));
        }
        Ok(())
    }

    #[test]
    fn published_schema_inventory_is_read_from_disk() -> Result<(), String> {
        let inventory = published_schema_paths(&repo_root()?)?;
        // Both published directories must be reached, or the inventory would
        // silently exclude a whole tree instead of one file.
        for expected in [
            "schemas/badges/shields-endpoint.schema.json",
            "schemas/ripr/check.schema.json",
            "schemas/ripr/repair-attempt.schema.json",
        ] {
            if !inventory.iter().any(|path| path == expected) {
                return Err(format!("the inventory omits `{expected}`: {inventory:?}"));
            }
        }
        for path in &inventory {
            if !path.ends_with(SCHEMA_FILE_SUFFIX) {
                return Err(format!("`{path}` is not a published schema"));
            }
        }
        Ok(())
    }

    const EXAMPLE_SCHEMA: &str = "schemas/ripr/example.schema.json";

    /// The shape both verification documents actually use: a Markdown table
    /// row whose first cell is the backticked repository-relative path.
    fn readme_row(path: &str) -> String {
        format!("| [`{path}`](../../{path}) | An example. |")
    }

    fn audit_row(path: &str) -> String {
        format!("| `{path}` | `0.1` | a producer | live |")
    }

    #[test]
    fn documentation_coverage_is_silent_when_both_documents_carry_the_inventory() {
        let inventory = vec![EXAMPLE_SCHEMA.to_string()];
        let violations = schema_documentation_violations(
            &inventory,
            &readme_row(EXAMPLE_SCHEMA),
            &audit_row(EXAMPLE_SCHEMA),
        );
        assert!(
            violations.is_empty(),
            "a documented schema was reported: {violations:?}"
        );
    }

    #[test]
    fn documentation_coverage_reports_a_schema_missing_only_from_the_audit() {
        // The case that actually occurred: the schema was reachable from the
        // repository but no row named its producer. The README rule must stay
        // silent here, or one omission would report as two and neither
        // message would say which document to repair.
        let inventory = vec![EXAMPLE_SCHEMA.to_string()];
        let violations = schema_documentation_violations(
            &inventory,
            &readme_row(EXAMPLE_SCHEMA),
            &audit_row("schemas/ripr/other.schema.json"),
        );
        assert_eq!(
            violations.len(),
            1,
            "expected exactly the audit violation: {violations:?}"
        );
        assert!(
            violations[0].starts_with(SCHEMA_PRODUCER_AUDIT),
            "the violation does not name the audit document: {violations:?}"
        );
        assert!(
            violations[0].contains(EXAMPLE_SCHEMA),
            "the violation does not name the schema: {violations:?}"
        );
    }

    #[test]
    fn documentation_coverage_reports_a_schema_missing_only_from_the_readme() {
        let inventory = vec![EXAMPLE_SCHEMA.to_string()];
        let violations = schema_documentation_violations(
            &inventory,
            &readme_row("schemas/ripr/other.schema.json"),
            &audit_row(EXAMPLE_SCHEMA),
        );
        assert_eq!(
            violations.len(),
            1,
            "expected exactly the README violation: {violations:?}"
        );
        assert!(
            violations[0].starts_with(VERIFICATION_README),
            "the violation does not name the README: {violations:?}"
        );
    }

    #[test]
    fn documentation_coverage_reports_a_schema_missing_from_both_documents() {
        let inventory = vec![EXAMPLE_SCHEMA.to_string()];
        let violations =
            schema_documentation_violations(&inventory, "no rows here", "no rows here");
        assert_eq!(
            violations.len(),
            2,
            "an undocumented schema must be reported by both rules: {violations:?}"
        );
    }

    #[test]
    fn a_mention_outside_a_table_row_is_not_a_row() {
        // The violation says a row is missing, so a passing mention in prose
        // or a code block must not silence it. Searching the whole document
        // would have accepted every line below.
        let inventory = vec![EXAMPLE_SCHEMA.to_string()];
        let prose = format!(
            "The audit covers `{EXAMPLE_SCHEMA}` and more.\n\n```text\n{EXAMPLE_SCHEMA}\n```\n"
        );
        let violations = schema_documentation_violations(&inventory, &prose, &prose);
        assert_eq!(
            violations.len(),
            2,
            "a schema mentioned only outside a table row must still be reported: {violations:?}"
        );
    }

    #[test]
    fn a_longer_path_beginning_with_the_schema_path_is_not_that_schema() {
        // `contains` on the bare path would accept a row for a neighbouring
        // file whose name merely starts with this one.
        let inventory = vec![EXAMPLE_SCHEMA.to_string()];
        let decoy = format!("{EXAMPLE_SCHEMA}.bak");
        let violations =
            schema_documentation_violations(&inventory, &readme_row(&decoy), &audit_row(&decoy));
        assert_eq!(
            violations.len(),
            2,
            "a row for `{decoy}` must not document `{EXAMPLE_SCHEMA}`: {violations:?}"
        );
    }

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
    fn review_comments_gap_ledger_card_has_its_own_schema_branch() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join("schemas/ripr/review-comments.schema.json"))?;
        // The fixture carries an eligible card shaped by
        // output::review_comments::gap_record_recommendation_json, as well as
        // suppressed records whose anchor can be absent.
        let packet = read_json(
            root.join("tests/fixtures/verification/ripr/review-comments.gap-ledger.valid.json"),
        )?;
        let check = |value: &Value| {
            let mut violations = Vec::new();
            validate_value_against_schema(
                value,
                &schema,
                &schema,
                "gap-ledger review comments".to_string(),
                &mut violations,
            );
            violations
        };
        assert_eq!(packet["summary"]["comments"], 1);
        assert!(check(&packet).is_empty(), "{:#?}", check(&packet));

        let mut missing_identity = packet.clone();
        missing_identity["comments"][0]
            .as_object_mut()
            .ok_or("missing fixture comment")?
            .remove("gap_id");
        assert!(!check(&missing_identity).is_empty());

        // The eligible card exists to carry the repair card; a gap-ledger
        // card without one is not a valid review recommendation.
        let mut missing_repair_card = packet.clone();
        missing_repair_card["comments"][0]
            .as_object_mut()
            .ok_or("missing fixture comment")?
            .remove("repair_card");
        assert!(!check(&missing_repair_card).is_empty());

        // Key presence alone is not a repair card: `null`, an empty object,
        // and a card without verification commands carry no repair guidance.
        for (label, card) in [
            ("null", Value::Null),
            ("empty object", serde_json::json!({})),
        ] {
            let mut hollow = packet.clone();
            hollow["comments"][0]["repair_card"] = card;
            assert!(!check(&hollow).is_empty(), "{label} repair_card accepted");
        }
        let mut no_commands = packet.clone();
        no_commands["comments"][0]["repair_card"]["verification_commands"] = serde_json::json!([]);
        assert!(!check(&no_commands).is_empty());

        // An eligible GapRecord needs no related test or target file.
        // gap_record_comment_json then projects null test-navigation fields,
        // which the gap-ledger card must admit.
        let mut no_related_test = packet.clone();
        {
            let suggested = &mut no_related_test["comments"][0]["suggested_test"];
            suggested["recommended_name"] = Value::Null;
            suggested["near_test"] = Value::Null;
            suggested["related_test"] = Value::Null;
        }
        no_related_test["comments"][0]["repair_card"]["repair_route"]
            .as_object_mut()
            .ok_or("missing fixture repair route")?
            .remove("related_test");
        assert!(
            check(&no_related_test).is_empty(),
            "{:#?}",
            check(&no_related_test)
        );
        let mut no_test_location = no_related_test.clone();
        no_test_location["comments"][0]["suggested_test"]["recommended_file"] = Value::Null;
        no_test_location["comments"][0]["repair_card"]["repair_route"]
            .as_object_mut()
            .ok_or("missing fixture repair route")?
            .remove("target_file");
        assert!(
            check(&no_test_location).is_empty(),
            "{:#?}",
            check(&no_test_location)
        );

        let mut wrong_placement = packet.clone();
        wrong_placement["comments"][0]["placement"]["mode"] =
            Value::String("exact_seam_line".to_string());
        assert!(!check(&wrong_placement).is_empty());

        let mut default_packet =
            read_json(root.join("tests/fixtures/verification/ripr/review-comments.valid.json"))?;
        assert!(
            !default_packet["comments"]
                .as_array()
                .ok_or("missing default comments")?
                .is_empty()
        );
        assert!(
            check(&default_packet).is_empty(),
            "{:#?}",
            check(&default_packet)
        );
        // Working-set cards keep their string test-navigation contract.
        for field in ["recommended_file", "recommended_name"] {
            let mut null_navigation = default_packet.clone();
            null_navigation["comments"][0]["suggested_test"][field] = Value::Null;
            assert!(
                !check(&null_navigation).is_empty(),
                "working-set null {field} accepted"
            );
        }
        default_packet["comments"][0]["placement"]["mode"] =
            Value::String("gap_record_anchor".to_string());
        assert!(!check(&default_packet).is_empty());
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

        // The incomplete case is asserted only at the level this change owns:
        // the `status` branch must stop rejecting it. It is deliberately not
        // asserted fully valid, because a separate divergence still rejects it
        // — `check.schema.json` allows any non-empty `claim_boundary` while this
        // schema pins it to one `const`, so a real incomplete outcome never
        // projects cleanly. That is tracked on its own and is not silently
        // absorbed here.
        let incomplete_error = with(false, "error");
        assert!(
            !incomplete_error
                .iter()
                .any(|violation| violation.contains(".status:")),
            "no status violation may remain for an error packet retaining an \
             incomplete analysis_outcome: {incomplete_error:#?}"
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

    /// Negative control for the `source_currentness` property added with
    /// #3883. The producer emits this field on every finding from a closed
    /// enum, so the schema states the enum rather than `type: string`; a
    /// disposition outside it must be rejected rather than silently admitted.
    /// Negative control for the `flow_sink` relaxation added with #3912. The
    /// producer emits `null` when a missing discriminator has no local flow
    /// sink to name, so the schema admits `null` — but not any other
    /// non-object, and not an absent key.
    #[test]
    fn check_schema_admits_a_null_flow_sink_without_admitting_anything_else() -> Result<(), String>
    {
        let root = repo_root()?;
        let schema = read_json(root.join("schemas/ripr/check.schema.json"))?;
        let golden = read_json(root.join("fixtures/helper_chain_one_hop/expected/check.json"))?;

        let mut wrong_type = golden.clone();
        wrong_type["findings"][0]["missing_discriminators"][0]["flow_sink"] =
            Value::String("error_variant".to_string());
        let violations = document_violations(&wrong_type, &schema, "wrong flow sink type");
        if !violations
            .iter()
            .any(|violation| violation.contains("expected type object|null, got string"))
        {
            return Err(format!(
                "a string flow sink was not rejected as the wrong type: {violations:#?}"
            ));
        }

        let mut absent = golden;
        absent["findings"][0]["missing_discriminators"][0]
            .as_object_mut()
            .ok_or("the golden's first missing discriminator should be an object")?
            .remove("flow_sink");
        let violations = document_violations(&absent, &schema, "absent flow sink");
        if !violations
            .iter()
            .any(|violation| violation.contains("missing required field `flow_sink`"))
        {
            return Err(format!(
                "admitting `null` must not make the key optional: {violations:#?}"
            ));
        }
        Ok(())
    }

    /// Negative control for the next-step relaxation added with #3912. The
    /// producer emits `""` for "no action to recommend", so the schema drops
    /// `minLength` — but the fields stay required strings.
    #[test]
    fn check_schema_admits_an_empty_next_step_without_admitting_a_non_string() -> Result<(), String>
    {
        let root = repo_root()?;
        let schema = read_json(root.join("schemas/ripr/check.schema.json"))?;
        let golden = read_json(root.join("fixtures/match_arm_positive/expected/check.json"))?;

        let mut wrong_type = golden.clone();
        wrong_type["findings"][0]["recommended_next_step"] = Value::from(0);
        let violations = document_violations(&wrong_type, &schema, "non-string next step");
        if !violations
            .iter()
            .any(|violation| violation.contains("expected type string, got number"))
        {
            return Err(format!(
                "a numeric next step was not rejected: {violations:#?}"
            ));
        }

        let mut absent = golden;
        absent["findings"][0]
            .as_object_mut()
            .ok_or("the golden's first finding should be an object")?
            .remove("suggested_next_action");
        let violations = document_violations(&absent, &schema, "absent next action");
        if !violations
            .iter()
            .any(|violation| violation.contains("missing required field `suggested_next_action`"))
        {
            return Err(format!(
                "dropping minLength must not make the field optional: {violations:#?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn check_schema_rejects_an_unknown_source_currentness() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join("schemas/ripr/check.schema.json"))?;
        let mut fixture =
            read_json(root.join("tests/fixtures/verification/ripr/check-complete.valid.json"))?;
        fixture["findings"][0]["source_currentness"] = Value::String("probably_fine".to_string());
        let mut violations = Vec::new();

        validate_value_against_schema(
            &fixture,
            &schema,
            &schema,
            "unknown source currentness fixture".to_string(),
            &mut violations,
        );

        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("is not in enum")),
            "expected an enum rejection for the disposition itself, got {violations:#?}"
        );
        assert!(
            !violations
                .iter()
                .any(|violation| violation.contains("unexpected field `source_currentness`")),
            "the schema must know the field, so the rejection is about its value and not \
             about the key: {violations:#?}"
        );
        Ok(())
    }

    /// Negative control for the `test_harness` definition added with #3883.
    /// The producer always emits `limitations`, as `[]` when there are none,
    /// so an entry without the key is producer drift and not an absent
    /// optional.
    #[test]
    fn check_schema_rejects_a_harness_projection_without_limitations() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join("schemas/ripr/check.schema.json"))?;
        let mut fixture = read_json(
            root.join("fixtures/harness_dead_construction_no_exposed_credit/expected/check.json"),
        )?;
        let entry = fixture["test_harnesses"][0]
            .as_object_mut()
            .ok_or("the golden's first harness projection should be an object")?;
        entry
            .remove("limitations")
            .ok_or("the golden's first harness projection should carry limitations")?;
        let mut violations = Vec::new();

        validate_value_against_schema(
            &fixture,
            &schema,
            &schema,
            // Deliberately neutral: an earlier draft named this location
            // "harness projection without limitations", and the assertion
            // below then matched the label rather than the validator, so the
            // control passed against a schema that knew nothing about the
            // field at all.
            "harness golden".to_string(),
            &mut violations,
        );

        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("missing required field `limitations`")),
            "expected a missing-limitations rejection, got {violations:#?}"
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
    const GATE_BASELINE_FALLBACK_GOLDEN: &str = concat!(
        "fixtures/gate_baseline_fallback_disclosure/expected/gate-baseline/",
        "mixed-canonical-and-legacy-entries/gate-decision.json"
    );
    /// Mirrors `BASELINE_MATCH_KIND_LEGACY_PATH_LINE_CLASS`, which is private
    /// to the gate renderer. Stated here as the value the published schema is
    /// expected to admit; the contract row is what binds it to real bytes.
    const LEGACY_MATCH_KIND: &str = "legacy_path_line_class";

    /// Negative control for the `baseline_match_kind` property added with
    /// #3912. The producer emits exactly one value, from a constant
    /// (`BASELINE_MATCH_KIND_LEGACY_PATH_LINE_CLASS`), and emits the key only
    /// on fallback-only baseline matches. So the schema states the closed set
    /// rather than `type: string`, and leaves the property optional. Both
    /// halves are pinned here: an unrecognised match kind is rejected, and the
    /// canonical-match decision beside it, which carries no such key, is not.
    #[test]
    fn gate_decision_schema_admits_only_the_disclosed_baseline_match_kind() -> Result<(), String> {
        let root = repo_root()?;
        let schema = read_json(root.join("schemas/ripr/gate-decision.schema.json"))?;
        let golden = read_json(root.join(GATE_BASELINE_FALLBACK_GOLDEN))?;

        // Assert the subject before claiming anything from it: this golden is
        // useful only because it holds both shapes at once.
        if golden["decisions"][0]["baseline_match_kind"] != Value::String(LEGACY_MATCH_KIND.into())
        {
            return Err(format!(
                "the golden's first decision should disclose `{LEGACY_MATCH_KIND}`, got {}",
                compact_json(&golden["decisions"][0]["baseline_match_kind"])
            ));
        }
        if golden["decisions"][1].get("baseline_match_kind").is_some() {
            return Err(
                "the golden's second decision should be a canonical match carrying no \
                 `baseline_match_kind`, so the optionality half of this control is real"
                    .to_string(),
            );
        }

        let mut drifted = golden;
        drifted["decisions"][0]["baseline_match_kind"] =
            Value::String("path_line_class".to_string());
        let mut violations = Vec::new();
        validate_value_against_schema(
            &drifted,
            &schema,
            &schema,
            "drifted gate decision".to_string(),
            &mut violations,
        );

        if !violations
            .iter()
            .any(|violation| violation.contains("value \"path_line_class\" is not in enum"))
        {
            return Err(format!(
                "an unrecognised baseline match kind was not rejected: {violations:#?}"
            ));
        }
        Ok(())
    }

    /// Every violation one whole document raises against one whole schema.
    fn document_violations(value: &Value, schema: &Value, location: &str) -> Vec<String> {
        let mut violations = Vec::new();
        validate_value_against_schema(value, schema, schema, location.to_string(), &mut violations);
        violations
    }

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
