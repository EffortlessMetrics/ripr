//! Release-only sweep of committed producer documents against every published
//! schema (#3920).
//!
//! `check-verification-contracts` validates one hand-picked subject per
//! contract row. That denominator is kept by hand, and #3919 records why that
//! is not enough on its own: #3894's second commit exists because its first
//! commit's chosen golden missed two producer shapes. Replacing the required
//! gate's subject selection is deferred past the 0.11 cut, so this command is
//! the compensating release proof instead. It discovers every committed
//! producer document by an explicit binding rule, validates each one with the
//! same validator the gate uses, and reports discovered and validated counts
//! separately so a qualification run can replay it on the frozen candidate.
//!
//! Every byte it validates, schemas included, is read from one explicit commit
//! (`--rev`, default `HEAD`) through `git archive`, never from the working
//! tree, so a dirty or artifact-littered release checkout cannot change the
//! denominator or pass bytes the candidate does not carry.
//!
//! It is not wired into a required check. The binding table below is the
//! reviewed rule for which committed bytes are producer output, which are
//! consumer-input stimulus, and which schemas have no committed producer bytes
//! at all and must be fed live artifacts with `--artifact`.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::verification_contracts::validate_value_against_schema;

const REPORT_PATH: &str = "target/ripr/reports/schema-producer-sweep.json";
const USAGE: &str = "usage: cargo xtask schema-producer-sweep [--rev REV] [--artifact SCHEMA[#POINTER]=FILE[#POINTER]]...";
/// Every JSON file under this directory is a published schema, the same rule
/// `check-verification-contracts` applies to the working tree.
const SCHEMAS_PREFIX: &str = "schemas/";

/// Which committed documents a binding validates.
enum Corpus {
    /// Every document with this file name, anywhere in the repository.
    FileName(&'static str),
    /// Exactly these repository-relative paths.
    Paths(&'static [&'static str]),
    /// Every object under a `command_specs` map in any committed document,
    /// split by whether its role key is `verify`. Several producers embed
    /// these specs (agent packets, repo-exposure, first-useful-action), so a
    /// file-name rule would miss most of them.
    CommandSpecs { verify: bool },
    /// The producer writes only per-run bytes; nothing committed is producer
    /// output. Subjects come only from `--artifact`.
    RuntimeOnly(&'static str),
    /// No producer emits this shape, or the producer is pinned by a narrower
    /// in-crate authority this validator cannot replace. Never counted as
    /// validated.
    Exempt(&'static str),
}

struct Binding {
    schema_path: &'static str,
    /// JSON pointer to the subschema the subjects must match, or `None` for
    /// the whole schema.
    pointer: Option<&'static str>,
    corpus: Corpus,
    /// Hand-written edge fixtures, validated and counted apart from producer
    /// bytes so they can never be the only subjects.
    edge_fixtures: &'static [&'static str],
    /// Path prefixes whose matching documents are consumer-input stimulus,
    /// each with the reviewed reason.
    excluded: &'static [(&'static str, &'static str)],
}

const BINDINGS: &[Binding] = &[
    Binding {
        schema_path: "schemas/badges/shields-endpoint.schema.json",
        pointer: None,
        corpus: Corpus::Paths(&["badges/ripr.json", "badges/ripr-plus.json"]),
        edge_fixtures: &["tests/fixtures/verification/badge/ripr-plus.valid.json"],
        excluded: &[],
    },
    Binding {
        schema_path: "schemas/ripr/check.schema.json",
        pointer: None,
        corpus: Corpus::FileName("check.json"),
        edge_fixtures: &[
            "tests/fixtures/verification/ripr/check-complete.valid.json",
            "tests/fixtures/verification/ripr/check-incomplete.valid.json",
            "tests/fixtures/verification/ripr/check-limited.valid.json",
        ],
        excluded: &[(
            "fixtures/evidence-promotion-honesty-corpus/reports/",
            "hand-written `source_report` stimulus read by check-evidence-promotion-honesty, not `ripr check` output",
        )],
    },
    Binding {
        schema_path: "schemas/ripr/gate-decision.schema.json",
        pointer: None,
        corpus: Corpus::FileName("gate-decision.json"),
        edge_fixtures: &[
            "tests/fixtures/verification/ripr/gate-decision.valid.json",
            "tests/fixtures/verification/ripr/gate-decision.causal-delta.valid.json",
        ],
        excluded: &[
            (
                "fixtures/boundary_gap/expected/pr-evidence-ledger/",
                "`inputs.gate_decision` stimulus; the consumer reads an untyped subset",
            ),
            (
                "fixtures/boundary_gap/expected/pr-review-front-panel/",
                "`inputs.gate_decision` stimulus; the consumer reads an untyped subset",
            ),
            (
                "fixtures/boundary_gap/expected/report-packet-index/",
                "packet-tree placeholders the index renderer only inventories",
            ),
        ],
    },
    Binding {
        schema_path: "schemas/ripr/pr-evidence.schema.json",
        pointer: None,
        corpus: Corpus::RuntimeOnly(
            "`ripr pr-evidence` and `cargo xtask ripr-pr` write target/ripr/pr/repo-exposure.json per run",
        ),
        edge_fixtures: &["tests/fixtures/verification/ripr/pr-evidence.valid.json"],
        excluded: &[],
    },
    Binding {
        schema_path: "schemas/ripr/review-comments.schema.json",
        pointer: None,
        corpus: Corpus::RuntimeOnly(
            "`ripr review-comments` attaches its run receipt per run; committed pr-guidance goldens are renderer bytes without it",
        ),
        edge_fixtures: &[
            "tests/fixtures/verification/ripr/review-comments.valid.json",
            "tests/fixtures/verification/ripr/review-comments.gap-ledger.valid.json",
        ],
        excluded: &[],
    },
    Binding {
        schema_path: "schemas/ripr/repair-attempt.schema.json",
        pointer: None,
        corpus: Corpus::RuntimeOnly(
            "`ripr agent repair` writes target/ripr/repair-attempts/<id>/attempt.json per attempt",
        ),
        edge_fixtures: &[],
        excluded: &[],
    },
    Binding {
        schema_path: "schemas/ripr/repair-assurance.schema.json",
        pointer: Some("/$defs/command_spec"),
        corpus: Corpus::CommandSpecs { verify: false },
        edge_fixtures: &[],
        excluded: &[],
    },
    Binding {
        schema_path: "schemas/ripr/repair-assurance.schema.json",
        pointer: Some("/$defs/verification_command_spec"),
        corpus: Corpus::CommandSpecs { verify: true },
        edge_fixtures: &[],
        excluded: &[],
    },
    Binding {
        schema_path: "schemas/ripr/repair-assurance.schema.json",
        pointer: Some("/$defs/execution_result"),
        corpus: Corpus::RuntimeOnly(
            "`ripr agent verify-execute --result-json` commits one result per bounded execution",
        ),
        edge_fixtures: &[],
        excluded: &[],
    },
    Binding {
        schema_path: "schemas/ripr/repair-assurance.schema.json",
        pointer: None,
        corpus: Corpus::Exempt(
            "reserved: `implementation_state` is const `design_only`; no producer emits the envelope (RIPR-SPEC-0135)",
        ),
        edge_fixtures: &[],
        excluded: &[],
    },
    Binding {
        schema_path: "schemas/ripr/rust-repair-trust-corpus.schema.json",
        pointer: None,
        corpus: Corpus::Paths(&["metrics/rust-repair-trust/corpus.json"]),
        edge_fixtures: &[],
        excluded: &[],
    },
    Binding {
        schema_path: "schemas/ripr/ripr-agent-capability.schema.json",
        pointer: None,
        corpus: Corpus::Exempt(
            "narrower authority: capability_output_matches_the_published_schema in crates/ripr/src/lsp/agent_protocol.rs; the schema's cross-file `$ref` is outside this validator",
        ),
        edge_fixtures: &[],
        excluded: &[],
    },
    Binding {
        schema_path: "schemas/ripr/ripr-agent-error.schema.json",
        pointer: None,
        corpus: Corpus::Exempt("reserved envelope; no live producer (#3009)"),
        edge_fixtures: &[],
        excluded: &[],
    },
    Binding {
        schema_path: "schemas/ripr/ripr-agent-request.schema.json",
        pointer: None,
        corpus: Corpus::Exempt("reserved envelope; no live producer (#3009)"),
        edge_fixtures: &[],
        excluded: &[],
    },
    Binding {
        schema_path: "schemas/ripr/ripr-agent-success.schema.json",
        pointer: None,
        corpus: Corpus::Exempt("reserved envelope; no live producer (#3009)"),
        edge_fixtures: &[],
        excluded: &[],
    },
];

/// One `--artifact SCHEMA[#POINTER]=FILE[#POINTER]` input: live producer bytes
/// from a qualification run.
struct Artifact {
    schema_path: String,
    schema_pointer: Option<String>,
    file: PathBuf,
    subject_pointer: Option<String>,
}

pub(crate) fn schema_producer_sweep(args: &[String]) -> Result<(), String> {
    let options = parse_args(args)?;
    let root = crate::repo_root()?;
    let tree = load_tree(&root, &options.rev)?;
    let report = sweep(&tree, BINDINGS, &options.artifacts)?;
    let text = serde_json::to_string_pretty(&report.packet)
        .map_err(|error| format!("serialize schema producer sweep: {error}"))?;
    let path = root.join(REPORT_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::write(&path, format!("{text}\n"))
        .map_err(|error| format!("write {}: {error}", path.display()))?;
    for line in &report.summary {
        println!("{line}");
    }
    println!(
        "Wrote {REPORT_PATH} for commit {} (rows sha256:{})",
        tree.commit, report.digest
    );
    if report.violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "schema producer sweep failed:\n{}",
            report
                .violations
                .iter()
                .map(|violation| format!("- {violation}"))
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }
}

struct Options {
    rev: String,
    artifacts: Vec<Artifact>,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut rev = "HEAD".to_string();
    let mut artifacts = Vec::new();
    let mut remaining = args.iter();
    while let Some(arg) = remaining.next() {
        let value = remaining.next().ok_or_else(|| USAGE.to_string())?;
        if arg == "--rev" && !value.is_empty() {
            rev = value.clone();
            continue;
        }
        if arg != "--artifact" {
            return Err(USAGE.to_string());
        }
        let spec = value;
        let (schema, file) = spec.split_once('=').ok_or_else(|| USAGE.to_string())?;
        let (schema_path, schema_pointer) = split_pointer(schema);
        let (file, subject_pointer) = split_pointer(file);
        if schema_path.is_empty() || file.is_empty() {
            return Err(USAGE.to_string());
        }
        artifacts.push(Artifact {
            schema_path: schema_path.to_string(),
            schema_pointer,
            file: PathBuf::from(file),
            subject_pointer,
        });
    }
    Ok(Options { rev, artifacts })
}

fn split_pointer(text: &str) -> (&str, Option<String>) {
    match text.split_once('#') {
        Some((head, pointer)) => (head, Some(pointer.to_string())),
        None => (text, None),
    }
}

struct SweepReport {
    packet: Value,
    summary: Vec<String>,
    violations: Vec<String>,
    digest: String,
}

/// Counts for one binding row.
#[derive(Default)]
struct RowCounts {
    discovered: usize,
    validated: usize,
    edge: usize,
    excluded: usize,
    artifacts: usize,
    subjects: Vec<Value>,
}

fn sweep(
    tree: &CandidateTree,
    bindings: &[Binding],
    artifacts: &[Artifact],
) -> Result<SweepReport, String> {
    let mut violations = Vec::new();
    let inventory: Vec<String> = tree
        .files
        .keys()
        .filter(|path| path.starts_with(SCHEMAS_PREFIX))
        .cloned()
        .collect();
    if inventory.is_empty() {
        violations.push(
            "schemas/ holds no published schema, so the sweep has no denominator".to_string(),
        );
    }
    for schema_path in &inventory {
        if !bindings
            .iter()
            .any(|binding| binding.schema_path == schema_path)
        {
            violations.push(format!(
                "{schema_path} has no sweep binding; add a corpus rule, a runtime-only route, or a reviewed exemption"
            ));
        }
    }

    let CommittedDocuments {
        parsed: documents,
        malformed,
    } = committed_documents(tree, bindings);
    let mut schemas = BTreeMap::new();
    for schema_path in &inventory {
        let bytes = &tree.files[schema_path];
        let schema: Value = serde_json::from_slice(bytes)
            .map_err(|error| format!("parse {schema_path} at {}: {error}", tree.commit))?;
        schemas.insert(schema_path.clone(), (schema, sha256(bytes)));
    }

    let mut rows = Vec::new();
    let mut summary = Vec::new();
    for binding in bindings {
        let Some((schema, schema_digest)) = schemas.get(binding.schema_path) else {
            violations.push(format!(
                "binding names {} but no such published schema exists",
                binding.schema_path
            ));
            continue;
        };
        let subschema = match binding.pointer {
            None => schema,
            Some(pointer) => match schema.pointer(pointer) {
                Some(subschema) => subschema,
                None => {
                    violations.push(format!(
                        "{} has no subschema `{pointer}`",
                        binding.schema_path
                    ));
                    continue;
                }
            },
        };
        let label = row_label(binding.schema_path, binding.pointer);
        let mut counts = RowCounts::default();

        for (path, (digest, error)) in &malformed {
            if !corpus_binds_path(&binding.corpus, path)
                || binding
                    .excluded
                    .iter()
                    .any(|(prefix, _)| path.starts_with(prefix))
            {
                continue;
            }
            counts.discovered += 1;
            counts
                .subjects
                .push(json!({ "path": path, "sha256": digest, "role": "producer" }));
            violations.push(format!(
                "parse bound producer {path} at {}: {error}",
                tree.commit
            ));
        }

        for (path, (value, digest)) in &documents {
            let subjects = corpus_subjects(&binding.corpus, path, value);
            let unbound_lookalike = subjects.is_empty() && lookalike(binding, schema, path, value);
            if subjects.is_empty() && !unbound_lookalike {
                continue;
            }
            if let Some((_, reason)) = binding
                .excluded
                .iter()
                .find(|(prefix, _)| path.starts_with(prefix))
            {
                counts.excluded += 1;
                counts.subjects.push(
                    json!({ "path": path, "sha256": digest, "role": "excluded", "reason": reason }),
                );
                continue;
            }
            if unbound_lookalike {
                violations.push(format!(
                    "{path} looks like {label} output but no binding claims it; bind it or add a reviewed exclusion"
                ));
                continue;
            }
            for (location, subject) in subjects {
                counts.discovered += 1;
                let before = violations.len();
                validate_value_against_schema(
                    subject,
                    subschema,
                    schema,
                    location,
                    &mut violations,
                );
                if violations.len() == before {
                    counts.validated += 1;
                }
            }
            counts
                .subjects
                .push(json!({ "path": path, "sha256": digest, "role": "producer" }));
        }

        for edge in binding.edge_fixtures {
            let Some((value, digest)) = documents.get(*edge) else {
                violations.push(format!(
                    "{label} names edge fixture {edge}, which does not exist"
                ));
                continue;
            };
            let before = violations.len();
            validate_value_against_schema(
                value,
                subschema,
                schema,
                (*edge).to_string(),
                &mut violations,
            );
            if violations.len() == before {
                counts.edge += 1;
            }
            counts
                .subjects
                .push(json!({ "path": edge, "sha256": digest, "role": "edge_fixture" }));
        }

        for artifact in artifacts.iter().filter(|artifact| {
            artifact.schema_path == binding.schema_path
                && artifact.schema_pointer.as_deref() == binding.pointer
        }) {
            let bytes = fs::read(&artifact.file)
                .map_err(|error| format!("read {}: {error}", artifact.file.display()))?;
            let value: Value = serde_json::from_slice(&bytes)
                .map_err(|error| format!("parse {}: {error}", artifact.file.display()))?;
            let location = format!(
                "{}{}",
                artifact.file.display(),
                artifact.subject_pointer.as_deref().unwrap_or_default()
            );
            let subject = match artifact.subject_pointer.as_deref() {
                None => Some(&value),
                Some(pointer) => value.pointer(pointer),
            };
            let Some(subject) = subject else {
                violations.push(format!("{location} does not resolve to a subject"));
                continue;
            };
            let before = violations.len();
            validate_value_against_schema(
                subject,
                subschema,
                schema,
                location.clone(),
                &mut violations,
            );
            if violations.len() == before {
                counts.artifacts += 1;
            }
            counts
                .subjects
                .push(json!({ "path": location, "sha256": sha256(&bytes), "role": "artifact" }));
        }

        let disposition = match &binding.corpus {
            Corpus::Exempt(reason) => {
                summary.push(format!("{label}: exempt ({reason})"));
                json!({ "state": "exempt", "reason": reason })
            }
            Corpus::RuntimeOnly(route) => {
                let state = if counts.artifacts > 0 {
                    "artifact_validated"
                } else {
                    "not_run"
                };
                summary.push(format!(
                    "{label}: runtime-only, {} live artifact(s) validated{}",
                    counts.artifacts,
                    if counts.artifacts == 0 {
                        " (not_run: pass --artifact from a live run)"
                    } else {
                        ""
                    }
                ));
                json!({ "state": state, "route": route })
            }
            Corpus::FileName(_) | Corpus::Paths(_) | Corpus::CommandSpecs { .. } => {
                // A corpus rule that binds nothing is a sweep that did not run.
                if counts.discovered == 0 {
                    violations.push(format!("{label} bound no committed producer subject"));
                }
                summary.push(format!(
                    "{label}: {}/{} producer subjects valid, {} edge fixture(s), {} excluded stimulus, {} live artifact(s)",
                    counts.validated, counts.discovered, counts.edge, counts.excluded, counts.artifacts
                ));
                json!({ "state": if counts.discovered > 0 && counts.validated == counts.discovered { "corpus_validated" } else { "failed" } })
            }
        };
        rows.push(json!({
            "schema": binding.schema_path,
            "subschema": binding.pointer,
            "schema_sha256": schema_digest,
            "disposition": disposition,
            "producer_subjects_discovered": counts.discovered,
            "producer_subjects_valid": counts.validated,
            "edge_fixtures_valid": counts.edge,
            "excluded_stimulus": counts.excluded,
            "live_artifacts_valid": counts.artifacts,
            "subjects": counts.subjects,
        }));
    }

    // An artifact no row claims would otherwise be dropped without a trace,
    // and a mistyped schema path would read as a clean run.
    for artifact in artifacts {
        let claimed = bindings.iter().any(|binding| {
            artifact.schema_path == binding.schema_path
                && artifact.schema_pointer.as_deref() == binding.pointer
        });
        if !claimed {
            violations.push(format!(
                "--artifact {} names {}{}, which no sweep row validates",
                artifact.file.display(),
                artifact.schema_path,
                artifact
                    .schema_pointer
                    .as_deref()
                    .map(|pointer| format!("#{pointer}"))
                    .unwrap_or_default()
            ));
        }
    }

    let rows = Value::Array(rows);
    let digest = sha256(rows.to_string().as_bytes());
    let packet = json!({
        "schema_version": "0.1",
        "kind": "schema_producer_sweep",
        "commit": tree.commit,
        "validator": "xtask/src/verification_contracts.rs validate_value_against_schema",
        "published_schemas": inventory,
        "rows": rows,
        "rows_sha256": digest,
        "status": if violations.is_empty() { "pass" } else { "fail" },
        "violations": violations,
    });
    Ok(SweepReport {
        packet,
        summary,
        violations,
        digest,
    })
}

fn row_label(schema_path: &str, pointer: Option<&str>) -> String {
    match pointer {
        None => schema_path.to_string(),
        Some(pointer) => format!("{schema_path}#{pointer}"),
    }
}

/// The subjects `corpus` binds in one committed document, with locations.
fn corpus_subjects<'a>(corpus: &Corpus, path: &str, value: &'a Value) -> Vec<(String, &'a Value)> {
    match corpus {
        Corpus::FileName(name) if file_name(path) == *name => vec![(path.to_string(), value)],
        Corpus::Paths(paths) if paths.contains(&path) => vec![(path.to_string(), value)],
        Corpus::CommandSpecs { verify } => {
            let mut subjects = Vec::new();
            collect_command_specs(value, path.to_string(), *verify, &mut subjects);
            subjects
        }
        _ => Vec::new(),
    }
}

fn collect_command_specs<'a>(
    value: &'a Value,
    location: String,
    verify: bool,
    subjects: &mut Vec<(String, &'a Value)>,
) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let child_location = format!("{location}/{key}");
                if key == "command_specs"
                    && let Some(specs) = child.as_object()
                {
                    for (role, spec) in specs {
                        if (role == "verify") == verify {
                            subjects.push((format!("{child_location}/{role}"), spec));
                        }
                    }
                    continue;
                }
                collect_command_specs(child, child_location, verify, subjects);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_command_specs(item, format!("{location}/{index}"), verify, subjects);
            }
        }
        _ => {}
    }
}

/// Whether an unbound document carries every top-level `const` discriminator
/// and required field of a whole-document schema. Such a document is either
/// producer output under an unexpected name or stimulus that needs a reviewed
/// exclusion; leaving it silent is how a producer golden escapes the sweep.
fn lookalike(binding: &Binding, schema: &Value, path: &str, value: &Value) -> bool {
    if binding.pointer.is_some()
        || matches!(binding.corpus, Corpus::Exempt(_))
        || binding.edge_fixtures.contains(&path)
    {
        return false;
    }
    let (Some(object), Some(properties)) = (
        value.as_object(),
        schema.get("properties").and_then(Value::as_object),
    ) else {
        return false;
    };
    let constants_match = properties
        .iter()
        .all(|(key, property)| match property.get("const") {
            Some(expected) => object.get(key) == Some(expected),
            None => true,
        });
    let required_present = schema
        .get("required")
        .and_then(Value::as_array)
        .is_some_and(|required| {
            required
                .iter()
                .filter_map(Value::as_str)
                .all(|key| object.contains_key(key))
        });
    constants_match && required_present
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The `.json` blobs of one commit: the sweep's only input besides
/// `--artifact` files.
struct CandidateTree {
    commit: String,
    files: BTreeMap<String, Vec<u8>>,
}

/// Read every `.json` blob of `rev` in `root` through one `git archive`, so
/// nothing untracked or uncommitted can reach the sweep.
fn load_tree(root: &Path, rev: &str) -> Result<CandidateTree, String> {
    let commit = git(
        root,
        &["rev-parse", "--verify", &format!("{rev}^{{commit}}")],
    )?;
    let commit = String::from_utf8_lossy(&commit).trim().to_string();
    let archive = git(
        root,
        &["archive", "--format=tar", &commit, "--", ":(glob)**/*.json"],
    )?;
    let mut files = BTreeMap::new();
    let mut entries = tar::Archive::new(archive.as_slice());
    for entry in entries
        .entries()
        .map_err(|error| format!("read git archive of {commit}: {error}"))?
    {
        let mut entry = entry.map_err(|error| format!("read git archive of {commit}: {error}"))?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry
            .path()
            .map_err(|error| format!("read git archive path: {error}"))?
            .to_string_lossy()
            .replace('\\', "/");
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|error| format!("read {path} from {commit}: {error}"))?;
        files.insert(path, bytes);
    }
    Ok(CandidateTree { commit, files })
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    crate::run::capture_process_output_in("git", &args, Some(root), &[], &[], &[])
        .map_err(|error| error.message)
}

/// Parse committed documents outside `schemas/`. A malformed file selected by
/// a path or filename binding is a failed producer, not a missing subject.
/// Other non-JSON files (including JSONC stimulus) are outside this sweep.
struct CommittedDocuments {
    parsed: BTreeMap<String, (Value, String)>,
    malformed: BTreeMap<String, (String, String)>,
}

fn committed_documents(tree: &CandidateTree, bindings: &[Binding]) -> CommittedDocuments {
    let mut documents = BTreeMap::new();
    let mut malformed = BTreeMap::new();
    for (path, bytes) in tree
        .files
        .iter()
        .filter(|(path, _)| !path.starts_with(SCHEMAS_PREFIX))
    {
        match serde_json::from_slice::<Value>(bytes) {
            Ok(value) => {
                documents.insert(path.clone(), (value, sha256(bytes)));
            }
            Err(error)
                if bindings.iter().any(|binding| {
                    corpus_binds_path(&binding.corpus, path)
                        && !binding
                            .excluded
                            .iter()
                            .any(|(prefix, _)| path.starts_with(prefix))
                }) =>
            {
                malformed.insert(path.clone(), (sha256(bytes), error.to_string()));
            }
            Err(_) => {}
        }
    }
    CommittedDocuments {
        parsed: documents,
        malformed,
    }
}

fn corpus_binds_path(corpus: &Corpus, path: &str) -> bool {
    match corpus {
        Corpus::FileName(name) => file_name(path) == *name,
        Corpus::Paths(paths) => paths.contains(&path),
        _ => false,
    }
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{temp_dir, write};

    const EXAMPLE_SCHEMA: &str = "schemas/ripr/example.schema.json";

    /// A closed schema with the two things the sweep keys on: a `const`
    /// discriminator and required fields.
    fn example_root() -> PathBuf {
        let root = temp_dir("schema-producer-sweep");
        write(
            &root.join(EXAMPLE_SCHEMA),
            r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://example.invalid/example.schema.json",
  "type": "object",
  "additionalProperties": false,
  "required": ["schema_version", "status"],
  "properties": {
    "schema_version": { "const": "0.1" },
    "status": { "enum": ["pass", "fail"] }
  }
}"#,
        );
        root
    }

    fn binding(corpus: Corpus, excluded: &'static [(&'static str, &'static str)]) -> Binding {
        Binding {
            schema_path: EXAMPLE_SCHEMA,
            pointer: None,
            corpus,
            edge_fixtures: &[],
            excluded,
        }
    }

    /// Commit everything under `root` except `live.json`, which stands for a
    /// per-run artifact outside the candidate.
    fn commit(root: &Path) -> Result<(), String> {
        for args in [
            vec!["init", "-q"],
            vec!["add", "-A", "--", ".", ":(exclude)live.json"],
            vec![
                "-c",
                "user.name=sweep",
                "-c",
                "user.email=sweep@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "candidate",
            ],
        ] {
            git(root, &args)?;
        }
        Ok(())
    }

    fn sweep_committed(
        root: &Path,
        bindings: &[Binding],
        artifacts: &[Artifact],
    ) -> Result<SweepReport, String> {
        commit(root)?;
        sweep(&load_tree(root, "HEAD")?, bindings, artifacts)
    }

    fn run(
        root: &Path,
        bindings: &[Binding],
        artifacts: &[Artifact],
    ) -> Result<SweepReport, String> {
        let report = sweep_committed(root, bindings, artifacts);
        let _ = fs::remove_dir_all(root);
        report
    }

    #[test]
    fn a_valid_bound_producer_document_is_counted_as_validated() -> Result<(), String> {
        let root = example_root();
        write(
            &root.join("fixtures/a/expected/example.json"),
            r#"{"schema_version":"0.1","status":"pass"}"#,
        );
        let report = run(
            &root,
            &[binding(Corpus::FileName("example.json"), &[])],
            &[],
        )?;
        if !report.violations.is_empty() {
            return Err(format!("unexpected violations: {:?}", report.violations));
        }
        let row = &report.packet["rows"][0];
        if row["producer_subjects_discovered"] != 1 || row["producer_subjects_valid"] != 1 {
            return Err(format!("expected one validated subject: {row}"));
        }
        if row["disposition"]["state"] != "corpus_validated" {
            return Err(format!("expected corpus_validated: {row}"));
        }
        Ok(())
    }

    #[test]
    fn a_bound_document_missing_a_required_field_fails_even_though_it_no_longer_looks_alike()
    -> Result<(), String> {
        // Binding is by file name, not by shape, so a producer that drops a
        // required field cannot make its own golden fall out of the sweep.
        let root = example_root();
        write(
            &root.join("fixtures/a/expected/example.json"),
            r#"{"schema_version":"0.1"}"#,
        );
        let report = run(
            &root,
            &[binding(Corpus::FileName("example.json"), &[])],
            &[],
        )?;
        if !report
            .violations
            .iter()
            .any(|violation| violation.contains("status"))
        {
            return Err(format!(
                "the missing required field was not reported: {:?}",
                report.violations
            ));
        }
        if report.packet["rows"][0]["disposition"]["state"] != "failed" {
            return Err("a failed subject must not leave the row corpus_validated".to_string());
        }
        Ok(())
    }

    #[test]
    fn malformed_bound_producer_json_fails_with_its_committed_path() -> Result<(), String> {
        for corpus in [
            Corpus::FileName("example.json"),
            Corpus::Paths(&["fixtures/a/expected/example.json"]),
        ] {
            let root = example_root();
            write(
                &root.join("fixtures/a/expected/example.json"),
                r#"{"schema_version":"0.1","status":"pass""#,
            );
            let report = run(&root, &[binding(corpus, &[])], &[])?;
            let row = &report.packet["rows"][0];
            if row["disposition"]["state"] != "failed"
                || row["producer_subjects_discovered"] != 1
                || row["producer_subjects_valid"] != 0
                || !report.violations.iter().any(|violation| {
                    violation.contains("fixtures/a/expected/example.json")
                        && violation.contains("parse")
                })
            {
                return Err(format!(
                    "malformed bound producer escaped the sweep: {row}; {:?}",
                    report.violations
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn non_object_command_spec_roles_fail_in_both_binding_rows() -> Result<(), String> {
        for (role, verify) in [("verify", true), ("build", false)] {
            let root = example_root();
            write(
                &root.join("fixtures/a/expected/packet.json"),
                &format!(r#"{{"command_specs":{{"{role}":null}}}}"#),
            );
            let report = run(&root, &[binding(Corpus::CommandSpecs { verify }, &[])], &[])?;
            let row = &report.packet["rows"][0];
            if row["disposition"]["state"] != "failed"
                || row["producer_subjects_discovered"] != 1
                || row["producer_subjects_valid"] != 0
                || !report.violations.iter().any(|violation| {
                    violation.contains(&format!("packet.json/command_specs/{role}"))
                })
            {
                return Err(format!(
                    "non-object {role} spec escaped the sweep: {row}; {:?}",
                    report.violations
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn an_unbound_lookalike_fails_until_bound_or_excluded() -> Result<(), String> {
        let root = example_root();
        write(
            &root.join("fixtures/a/expected/example.json"),
            r#"{"schema_version":"0.1","status":"pass"}"#,
        );
        write(
            &root.join("fixtures/a/inputs/renamed.json"),
            r#"{"schema_version":"0.1","status":"pass"}"#,
        );
        let unbound = sweep_committed(
            &root,
            &[binding(Corpus::FileName("example.json"), &[])],
            &[],
        )?;
        if !unbound
            .violations
            .iter()
            .any(|violation| violation.contains("fixtures/a/inputs/renamed.json"))
        {
            let _ = fs::remove_dir_all(&root);
            return Err(format!(
                "the renamed lookalike was not reported: {:?}",
                unbound.violations
            ));
        }
        let excluded = run(
            &root,
            &[binding(
                Corpus::FileName("example.json"),
                &[("fixtures/a/inputs/", "stimulus")],
            )],
            &[],
        )?;
        if !excluded.violations.is_empty() || excluded.packet["rows"][0]["excluded_stimulus"] != 1 {
            return Err(format!(
                "a reviewed exclusion must count, not fail: {}",
                excluded.packet
            ));
        }
        Ok(())
    }

    #[test]
    fn a_corpus_rule_that_binds_nothing_is_a_sweep_that_did_not_run() -> Result<(), String> {
        let root = example_root();
        let report = run(
            &root,
            &[binding(Corpus::FileName("example.json"), &[])],
            &[],
        )?;
        if !report
            .violations
            .iter()
            .any(|violation| violation.contains("bound no committed producer subject"))
        {
            return Err(format!("zero subjects passed: {:?}", report.violations));
        }
        Ok(())
    }

    #[test]
    fn a_published_schema_without_a_binding_fails() -> Result<(), String> {
        let root = example_root();
        let report = run(&root, &[], &[])?;
        if !report
            .violations
            .iter()
            .any(|violation| violation.contains("has no sweep binding"))
        {
            return Err(format!("an unbound schema passed: {:?}", report.violations));
        }
        Ok(())
    }

    #[test]
    fn runtime_only_rows_validate_supplied_artifacts_and_report_not_run_without_them()
    -> Result<(), String> {
        let root = example_root();
        let artifact = root.join("live.json");
        write(
            &artifact,
            r#"{"result":{"schema_version":"0.1","status":"maybe"}}"#,
        );
        let runtime = || binding(Corpus::RuntimeOnly("per-run bytes"), &[]);

        let not_run = sweep_committed(&root, &[runtime()], &[])?;
        if !not_run.violations.is_empty()
            || not_run.packet["rows"][0]["disposition"]["state"] != "not_run"
        {
            let _ = fs::remove_dir_all(&root);
            return Err(format!(
                "a runtime-only row without artifacts must be not_run: {}",
                not_run.packet
            ));
        }

        let supplied = run(
            &root,
            &[runtime()],
            &[Artifact {
                schema_path: EXAMPLE_SCHEMA.to_string(),
                schema_pointer: None,
                file: artifact.clone(),
                subject_pointer: Some("/result".to_string()),
            }],
        )?;
        if !supplied
            .violations
            .iter()
            .any(|violation| violation.contains("live.json/result"))
        {
            return Err(format!(
                "an invalid live artifact passed: {:?}",
                supplied.violations
            ));
        }
        Ok(())
    }

    #[test]
    fn the_sweep_reads_the_commit_not_the_working_tree() -> Result<(), String> {
        let root = example_root();
        let golden = root.join("fixtures/a/expected/example.json");
        write(&golden, r#"{"schema_version":"0.1","status":"pass"}"#);
        commit(&root)?;
        // After the commit: the tracked golden is broken in the working tree
        // and an untracked lookalike appears. Neither is candidate bytes.
        write(&golden, r#"{"schema_version":"0.1"}"#);
        write(
            &root.join("rc.json"),
            r#"{"schema_version":"0.1","status":"pass"}"#,
        );
        let tree = load_tree(&root, "HEAD");
        let _ = fs::remove_dir_all(&root);
        let tree = tree?;
        let report = sweep(
            &tree,
            &[binding(Corpus::FileName("example.json"), &[])],
            &[],
        )?;
        if !report.violations.is_empty() || report.packet["commit"] != tree.commit.as_str() {
            return Err(format!(
                "working-tree bytes reached the sweep: {:?}",
                report.violations
            ));
        }
        if tree.files.contains_key("rc.json") {
            return Err("an untracked file was read as candidate bytes".to_string());
        }
        Ok(())
    }

    #[test]
    fn an_artifact_no_row_claims_fails_instead_of_being_dropped() -> Result<(), String> {
        let root = example_root();
        let artifact = root.join("live.json");
        write(&artifact, r#"{"schema_version":"0.1","status":"pass"}"#);
        let report = run(
            &root,
            &[binding(Corpus::RuntimeOnly("per-run bytes"), &[])],
            &[Artifact {
                schema_path: "schemas/ripr/exmaple.schema.json".to_string(),
                schema_pointer: None,
                file: artifact,
                subject_pointer: None,
            }],
        )?;
        if !report
            .violations
            .iter()
            .any(|violation| violation.contains("no sweep row validates"))
        {
            return Err(format!(
                "a mistyped artifact schema passed: {:?}",
                report.violations
            ));
        }
        Ok(())
    }

    #[test]
    fn artifact_arguments_parse_schema_and_subject_pointers() -> Result<(), String> {
        let parsed = parse_args(&[
            "--artifact".to_string(),
            "schemas/ripr/repair-assurance.schema.json#/$defs/execution_result=out.json#/result"
                .to_string(),
        ])?;
        if parsed.rev != "HEAD" {
            return Err(format!("--rev must default to HEAD, got {}", parsed.rev));
        }
        let [artifact] = parsed.artifacts.as_slice() else {
            return Err("expected one artifact".to_string());
        };
        if artifact.schema_pointer.as_deref() != Some("/$defs/execution_result")
            || artifact.subject_pointer.as_deref() != Some("/result")
            || artifact.file != Path::new("out.json")
        {
            return Err("pointers were not split from the paths".to_string());
        }
        for malformed in [
            vec!["--artifact"],
            vec!["--check"],
            vec!["--artifact", "no-separator"],
        ] {
            let args: Vec<String> = malformed.iter().map(|arg| (*arg).to_string()).collect();
            match parse_args(&args) {
                Err(error) if error == USAGE => {}
                Err(error) => {
                    return Err(format!(
                        "{malformed:?} failed with `{error}`, not the usage text"
                    ));
                }
                Ok(_) => return Err(format!("{malformed:?} was accepted")),
            }
        }
        Ok(())
    }

    /// The binding table and the published inventory must name the same set,
    /// so a new schema cannot ship without a sweep disposition and a renamed
    /// one cannot leave a binding that silently resolves nothing. This test
    /// deliberately stops at the table: validating the whole committed corpus
    /// is the release command's job, not a new required gate during the cut
    /// (#3919).
    #[test]
    fn the_binding_table_covers_exactly_the_published_inventory() -> Result<(), String> {
        let tree = load_tree(&crate::repo_root()?, "HEAD")?;
        let inventory: Vec<&String> = tree
            .files
            .keys()
            .filter(|path| path.starts_with(SCHEMAS_PREFIX))
            .collect();
        for schema_path in &inventory {
            if !BINDINGS
                .iter()
                .any(|binding| binding.schema_path == schema_path.as_str())
            {
                return Err(format!("{schema_path} has no sweep binding"));
            }
        }
        for binding in BINDINGS {
            if !inventory
                .iter()
                .any(|schema_path| schema_path.as_str() == binding.schema_path)
            {
                return Err(format!("binding names unpublished {}", binding.schema_path));
            }
        }
        Ok(())
    }
}
