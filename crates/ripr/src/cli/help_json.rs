//! Versioned machine-readable command and workflow discovery.
//!
//! RIPR-SPEC-0190 / issue #4825 (command-discovery C4): `ripr help --json`
//! projects the accepted typed authorities — the C1 command catalog, the C2
//! metadata table, and the C3 workflow catalog — into one strict, byte-stable
//! JSON document. It serializes existing catalog authority only: it never
//! parses human help, executes commands, inspects a repository, or strengthens
//! any command or workflow claim.
//!
//! Determinism and identity law (issue #4825): identical catalog inputs
//! produce byte-identical JSON across equivalent checkout roots, terminal
//! widths, color settings, locales, map order, and wall-clock time. Rows and
//! every string list are sorted by identity, the document carries no absolute
//! paths, PIDs, timestamps, or environment observations, and the catalog
//! digest is computed from the scoped catalog surface — commands, workflows,
//! and the catalog contract version — never from the product version,
//! document copy, rendered whitespace, human help constants, or the current
//! binary path.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::cli::command_catalog::{
    CATALOG_CONTRACT_VERSION, CommandCatalogEntry, catalog, catalog_violations,
};
use crate::cli::command_metadata::{CommandMetadata, metadata, metadata_violations};
use crate::cli::workflow_catalog::{
    WorkflowCatalogEntry, WorkflowNext, workflow_catalog, workflow_catalog_violations,
};

/// Schema version of the `help --json` document. Bump on any material shape
/// change to the DTO; the value is pinned by unit tests and documented in
/// `docs/OUTPUT_SCHEMA.md`.
pub(crate) const HELP_JSON_SCHEMA_VERSION: u64 = 1;

/// Top-level limitations of the machine document: what governs its identity
/// and what a consumer must not infer from it.
const DOCUMENT_LIMITATIONS: &[&str] = &[
    "ordering is explicit: commands and workflows sort by identity key, and every embedded string list is sorted",
    "the catalog digest covers the sorted command rows, workflow rows, and catalog contract version only; product version, human help wording, and renderer layout are excluded",
    "the document is advisory discovery data; it does not prove command outcomes or workflow success",
];

/// Explicit non-claims: the discovery route touches nothing outside the
/// static catalog tables.
const DOCUMENT_NON_CLAIMS: &[&str] = &[
    "does not execute commands or workflows",
    "does not run analysis, compilation, tests, or mutation",
    "does not spawn child processes, open the network, or start the LSP",
    "does not read git state, caches, or the filesystem beyond the binary itself",
    "does not write product artifacts or mutate the workspace",
];

#[derive(Serialize)]
struct HelpJsonDocument {
    schema_version: u64,
    product_version: &'static str,
    catalog_contract_version: &'static str,
    commands: Vec<CommandJsonRow>,
    workflows: Vec<WorkflowJsonRow>,
    catalog_digest: String,
    limitations: &'static [&'static str],
    non_claims: &'static [&'static str],
}

#[derive(Serialize)]
struct CommandJsonRow {
    id: &'static str,
    path: &'static str,
    class: &'static str,
    discovery: &'static str,
    relation: RelationJson,
    aliases: Vec<&'static str>,
    summary: &'static str,
    task: &'static str,
    workflows: Vec<&'static str>,
    operation: &'static str,
    cost: &'static str,
    effects: EffectsJson,
    primary_inputs: Vec<&'static str>,
    outputs: OutputsJson,
    state_target: Option<&'static str>,
    json_support: bool,
    example: &'static str,
    next_routes: Vec<&'static str>,
    stop_states: Vec<&'static str>,
    limitations: &'static str,
    not_applicable_reason: Option<&'static str>,
}

/// C1 replacement/retirement relation, projected so machine consumers can
/// resolve a compatibility spelling to its canonical command and spot retired
/// rows without scraping human help.
#[derive(Serialize, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RelationJson {
    Canonical,
    AliasOf { target: &'static str },
    Retired { replacement: Option<&'static str> },
}

#[derive(Serialize)]
struct EffectsJson {
    may_run_analysis: bool,
    may_compile: bool,
    may_run_tests: bool,
    may_run_mutation: bool,
    may_use_network: bool,
    may_start_child_process: bool,
}

#[derive(Serialize)]
struct OutputsJson {
    default: Option<&'static str>,
    optional: Vec<&'static str>,
}

#[derive(Serialize)]
struct WorkflowJsonRow {
    id: &'static str,
    aliases: Vec<&'static str>,
    command_tag: &'static str,
    purpose: &'static str,
    applicability: &'static str,
    prerequisites: Vec<&'static str>,
    first_command: &'static str,
    steps: Vec<WorkflowStepJson>,
    optional_steps: Vec<WorkflowStepJson>,
    result_families: Vec<WorkflowFamilyJson>,
    artifacts_read: Vec<&'static str>,
    artifacts_written: Vec<&'static str>,
    recovery: Vec<WorkflowRecoveryJson>,
    stop_conditions: Vec<&'static str>,
    advanced_alternatives: Vec<&'static str>,
    limitations: &'static str,
}

#[derive(Serialize)]
struct WorkflowStepJson {
    command: &'static str,
    role: &'static str,
    cost: &'static str,
    operation: &'static str,
    effects: EffectsJson,
    writes: Vec<&'static str>,
}

#[derive(Serialize)]
struct WorkflowFamilyJson {
    from: &'static str,
    family: &'static str,
    next: WorkflowNextJson,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WorkflowNextJson {
    Command { command: &'static str },
    Stop { statement: &'static str },
    Limitation { statement: &'static str },
}

#[derive(Serialize)]
struct WorkflowRecoveryJson {
    when: &'static str,
    route: &'static str,
}

/// Fail-closed integrity check for the machine document (issue #4825 required
/// control 4 and 12): any catalog violation, a missing metadata join, or an
/// empty command/workflow surface fails before any document bytes exist.
fn document_violations(
    entries: &[CommandCatalogEntry],
    command_rows: &[CommandMetadata],
    workflows: &[WorkflowCatalogEntry],
) -> Vec<String> {
    let mut violations = catalog_violations(entries);
    violations.extend(metadata_violations(entries, command_rows));
    violations.extend(workflow_catalog_violations(
        entries,
        command_rows,
        workflows,
    ));
    if entries.is_empty() {
        violations.push("help --json cannot emit a document with zero command rows".to_string());
    }
    if workflows.is_empty() {
        violations.push("help --json cannot emit a document with zero workflow rows".to_string());
    }
    for entry in entries {
        if !command_rows.iter().any(|row| row.id == entry.id) {
            violations.push(format!(
                "help --json command row {:?} has no metadata projection",
                entry.id
            ));
        }
    }
    violations
}

fn sorted_strings(values: &[&'static str]) -> Vec<&'static str> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted
}

fn command_row(entry: &CommandCatalogEntry, metadata_row: &CommandMetadata) -> CommandJsonRow {
    CommandJsonRow {
        id: entry.id,
        path: entry.path,
        class: entry.class.as_str(),
        discovery: entry.discovery.as_str(),
        relation: match entry.relation {
            crate::cli::command_catalog::CommandRelation::Canonical => RelationJson::Canonical,
            crate::cli::command_catalog::CommandRelation::AliasOf(target) => {
                RelationJson::AliasOf { target }
            }
            crate::cli::command_catalog::CommandRelation::Retired { replacement } => {
                RelationJson::Retired { replacement }
            }
        },
        aliases: sorted_strings(entry.aliases),
        summary: metadata_row.summary,
        task: metadata_row.task,
        workflows: sorted_strings(metadata_row.workflows),
        operation: metadata_row.operation.as_str(),
        cost: metadata_row.cost.as_str(),
        effects: EffectsJson {
            may_run_analysis: metadata_row.effects.may_run_analysis,
            may_compile: metadata_row.effects.may_compile,
            may_run_tests: metadata_row.effects.may_run_tests,
            may_run_mutation: metadata_row.effects.may_run_mutation,
            may_use_network: metadata_row.effects.may_use_network,
            may_start_child_process: metadata_row.effects.may_start_child_process,
        },
        primary_inputs: sorted_strings(metadata_row.primary_inputs),
        outputs: OutputsJson {
            default: metadata_row.outputs.default,
            optional: sorted_strings(metadata_row.outputs.optional),
        },
        state_target: metadata_row.state_target,
        json_support: metadata_row.json_support,
        example: metadata_row.example,
        next_routes: sorted_strings(metadata_row.next_routes),
        stop_states: sorted_strings(metadata_row.stop_states),
        limitations: metadata_row.limitations,
        not_applicable_reason: metadata_row.not_applicable_reason,
    }
}

fn step_json(
    workflow_step: &crate::cli::workflow_catalog::WorkflowStep,
    command_rows: &[CommandMetadata],
) -> Result<WorkflowStepJson, String> {
    // The step's narrowable flags come from the workflow row itself; the
    // remaining flags come from the supplied C2 metadata row of the step's
    // command (the C3 mirror validator already proved the narrowable flags
    // agree). Reading the supplied table — never the production global —
    // keeps synthetic documents self-consistent.
    let Some(command_row) = command_rows
        .iter()
        .find(|row| row.id == workflow_step.command)
    else {
        return Err(format!(
            "help --json lost step command row {:?}",
            workflow_step.command
        ));
    };
    Ok(WorkflowStepJson {
        command: workflow_step.command,
        role: workflow_step.role,
        cost: workflow_step.cost.as_str(),
        operation: workflow_step.operation.as_str(),
        effects: EffectsJson {
            may_run_analysis: command_row.effects.may_run_analysis,
            may_compile: workflow_step.may_compile,
            may_run_tests: workflow_step.may_run_tests,
            may_run_mutation: command_row.effects.may_run_mutation,
            may_use_network: command_row.effects.may_use_network,
            may_start_child_process: workflow_step.may_start_child_process,
        },
        writes: sorted_strings(workflow_step.writes),
    })
}

fn workflow_row(
    row: &WorkflowCatalogEntry,
    command_rows: &[CommandMetadata],
) -> Result<WorkflowJsonRow, String> {
    Ok(WorkflowJsonRow {
        id: row.id,
        aliases: sorted_strings(row.aliases),
        command_tag: row.command_tag,
        purpose: row.purpose,
        applicability: row.applicability,
        prerequisites: sorted_strings(row.prerequisites),
        first_command: row.first_command,
        steps: row
            .steps
            .iter()
            .map(|step| step_json(step, command_rows))
            .collect::<Result<Vec<_>, String>>()?,
        optional_steps: row
            .optional_steps
            .iter()
            .map(|step| step_json(step, command_rows))
            .collect::<Result<Vec<_>, String>>()?,
        result_families: row
            .result_families
            .iter()
            .map(|family| WorkflowFamilyJson {
                from: family.from,
                family: family.family,
                next: match family.next {
                    WorkflowNext::Command(command) => WorkflowNextJson::Command { command },
                    WorkflowNext::Terminal(statement) => WorkflowNextJson::Stop { statement },
                    WorkflowNext::Limitation(statement) => {
                        WorkflowNextJson::Limitation { statement }
                    }
                },
            })
            .collect(),
        artifacts_read: sorted_strings(row.artifacts_read),
        artifacts_written: sorted_strings(row.artifacts_written),
        recovery: row
            .recovery
            .iter()
            .map(|route| WorkflowRecoveryJson {
                when: route.when,
                route: route.route,
            })
            .collect(),
        stop_conditions: sorted_strings(row.stop_conditions),
        advanced_alternatives: sorted_strings(row.advanced_alternatives),
        limitations: row.limitations,
    })
}

fn build_document(
    entries: &[CommandCatalogEntry],
    command_rows: &[CommandMetadata],
    workflows: &[WorkflowCatalogEntry],
) -> Result<HelpJsonDocument, String> {
    let violations = document_violations(entries, command_rows, workflows);
    if !violations.is_empty() {
        return Err(format!(
            "help --json catalog integrity failed: {violations:?}"
        ));
    }

    let mut commands: Vec<CommandJsonRow> = entries
        .iter()
        .map(|entry| {
            // Integrity above proves the join exists; keep the lookup local to
            // the projection so a row can never be silently dropped.
            let metadata_row = command_rows
                .iter()
                .find(|row| row.id == entry.id)
                .ok_or_else(|| format!("help --json lost command row {:?}", entry.id))?;
            Ok(command_row(entry, metadata_row))
        })
        .collect::<Result<Vec<_>, String>>()?;
    commands.sort_by(|left, right| left.id.cmp(right.id));

    let mut workflow_rows: Vec<WorkflowJsonRow> = workflows
        .iter()
        .map(|row| workflow_row(row, command_rows))
        .collect::<Result<Vec<_>, String>>()?;
    workflow_rows.sort_by(|left, right| left.id.cmp(right.id));

    let mut document = HelpJsonDocument {
        schema_version: HELP_JSON_SCHEMA_VERSION,
        product_version: env!("CARGO_PKG_VERSION"),
        catalog_contract_version: CATALOG_CONTRACT_VERSION,
        commands,
        workflows: workflow_rows,
        catalog_digest: String::new(),
        limitations: DOCUMENT_LIMITATIONS,
        non_claims: DOCUMENT_NON_CLAIMS,
    };

    // The digest is catalog identity: it covers the sorted command and
    // workflow rows plus the catalog contract version, and never reads the
    // product version, schema version, document copy, rendered whitespace,
    // human help constants, or the current binary path (issue #4825
    // determinism and identity law). A package release therefore remints
    // `product_version` without moving the catalog digest.
    document.catalog_digest = catalog_digest(&document)?;
    Ok(document)
}

/// The scoped digest input: exactly the governed catalog surface, serialized
/// with the same deterministic field order as the document itself.
#[derive(Serialize)]
struct CatalogDigestInput<'a> {
    catalog_contract_version: &'static str,
    commands: &'a [CommandJsonRow],
    workflows: &'a [WorkflowJsonRow],
}

fn catalog_digest(document: &HelpJsonDocument) -> Result<String, String> {
    let input = CatalogDigestInput {
        catalog_contract_version: document.catalog_contract_version,
        commands: &document.commands,
        workflows: &document.workflows,
    };
    let serialized = serde_json::to_string(&input)
        .map_err(|error| format!("help --json digest serialization failed: {error}"))?;
    Ok(sha256_hex(serialized.as_bytes()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Render the complete `help --json` document as one JSON line. Fails closed:
/// any catalog integrity failure returns an error before any bytes are
/// produced, so stdout can never carry a plausible partial document.
pub(crate) fn render_help_json() -> Result<String, String> {
    let document = build_document(catalog(), metadata(), workflow_catalog())?;
    serde_json::to_string(&document)
        .map_err(|error| format!("help --json serialization failed: {error}"))
}

/// `ripr help --json`: emit the document on stdout. The document is built in
/// full before printing; a broken catalog fails here with nothing on stdout.
pub(crate) fn print_help_json() -> Result<(), String> {
    let document = render_help_json()?;
    println!("{document}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{HELP_JSON_SCHEMA_VERSION, build_document, document_violations, render_help_json};
    use crate::cli::command_catalog::{CommandClass, catalog};
    use crate::cli::command_metadata::metadata;
    use crate::cli::workflow_catalog::{WorkflowNext, workflow_catalog};

    fn production_document() -> Result<super::HelpJsonDocument, String> {
        build_document(catalog(), metadata(), workflow_catalog()).map_err(|error| error.to_string())
    }

    /// The machine catalog must advertise its own route. `cmd:help` is the
    /// command that emits this document; reporting `json_support: false` for
    /// it made `ripr help --json` undiscoverable from the catalog (#5266).
    #[test]
    fn help_command_reports_json_support_in_the_machine_catalog() -> Result<(), String> {
        let document = production_document()?;
        let Some(help) = document.commands.iter().find(|row| row.id == "cmd:help") else {
            return Err("cmd:help row missing from the machine catalog".to_string());
        };
        if !help.json_support {
            return Err(
                "cmd:help must report json_support: true because ripr help --json emits this document"
                    .to_string(),
            );
        }
        if help.path != "help" {
            return Err(format!("cmd:help path misprojected: {}", help.path));
        }
        Ok(())
    }

    #[test]
    fn production_document_is_valid_and_nonempty() -> Result<(), String> {
        let rendered = render_help_json()?;
        let value: serde_json::Value = serde_json::from_str(&rendered)
            .map_err(|error| format!("document is not valid JSON: {error}"))?;
        if value["schema_version"] != HELP_JSON_SCHEMA_VERSION {
            return Err(format!("wrong schema_version: {}", value["schema_version"]));
        }
        let commands = value["commands"].as_array().ok_or("commands missing")?;
        let workflows = value["workflows"].as_array().ok_or("workflows missing")?;
        if commands.len() != catalog().len() {
            return Err("command row count does not cover the catalog".to_string());
        }
        if workflows.len() != workflow_catalog().len() || workflows.is_empty() {
            return Err("workflow rows do not cover the catalog".to_string());
        }
        let digest = value["catalog_digest"].as_str().ok_or("digest missing")?;
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("digest is not a sha256 hex string: {digest:?}"));
        }
        Ok(())
    }

    #[test]
    fn output_is_byte_stable_and_explicitly_ordered() -> Result<(), String> {
        let first = render_help_json()?;
        let second = render_help_json()?;
        if first != second {
            return Err("render is not byte-stable".to_string());
        }
        if first.contains('\n') || !first.ends_with('}') {
            return Err("document must be one complete JSON line".to_string());
        }
        let document = production_document()?;
        let mut last: Option<&str> = None;
        for row in &document.commands {
            if let Some(previous) = last
                && row.id < previous
            {
                return Err("command rows are not sorted by id".to_string());
            }
            last = Some(row.id);
        }
        last = None;
        for row in &document.workflows {
            if let Some(previous) = last
                && row.id < previous
            {
                return Err("workflow rows are not sorted by id".to_string());
            }
            last = Some(row.id);
        }
        Ok(())
    }

    #[test]
    fn reordered_source_rows_normalize_to_the_same_document() -> Result<(), String> {
        let mut entries = catalog().to_vec();
        entries.reverse();
        let mut workflows = workflow_catalog().to_vec();
        workflows.reverse();
        let document =
            build_document(&entries, metadata(), &workflows).map_err(|error| error.to_string())?;
        let production = production_document()?;
        if document.catalog_digest != production.catalog_digest {
            return Err("reordered source rows changed the semantic digest".to_string());
        }
        Ok(())
    }

    #[test]
    fn duplicate_command_identity_fails_closed() -> Result<(), String> {
        let mut entries = catalog().to_vec();
        let Some(first) = entries.first().copied() else {
            return Err("catalog empty".to_string());
        };
        entries.push(first);
        let violations = document_violations(&entries, metadata(), workflow_catalog());
        if violations
            .iter()
            .any(|violation| violation.contains("duplicate"))
        {
            return Ok(());
        }
        Err(format!(
            "expected duplicate-identity failure, got {violations:?}"
        ))
    }

    #[test]
    fn zero_command_or_workflow_rows_fail_closed() -> Result<(), String> {
        let empty_workflows: Vec<crate::cli::workflow_catalog::WorkflowCatalogEntry> = Vec::new();
        if build_document(catalog(), metadata(), &empty_workflows).is_ok() {
            return Err("zero workflow rows must fail closed".to_string());
        }
        let empty_entries: Vec<crate::cli::command_catalog::CommandCatalogEntry> = Vec::new();
        if build_document(&empty_entries, metadata(), workflow_catalog()).is_ok() {
            return Err("zero command rows must fail closed".to_string());
        }
        Ok(())
    }

    #[test]
    fn workflow_references_resolve_to_exact_command_rows() -> Result<(), String> {
        let document = production_document()?;
        for workflow in &document.workflows {
            for step in workflow.steps.iter().chain(workflow.optional_steps.iter()) {
                let Some(command) = document.commands.iter().find(|row| row.id == step.command)
                else {
                    return Err(format!(
                        "workflow {:?} step references missing command {:?}",
                        workflow.id, step.command
                    ));
                };
                if !matches!(
                    command.class,
                    "public" | "compatibility" | "advanced" | "internal"
                ) {
                    return Err(format!("command {:?} has no classification", command.id));
                }
            }
            for family in &workflow.result_families {
                if !document.commands.iter().any(|row| row.id == family.from) {
                    return Err(format!(
                        "workflow {:?} family {:?} names missing producer {:?}",
                        workflow.id, family.family, family.from
                    ));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn single_metadata_change_updates_only_the_intended_row_and_digest() -> Result<(), String> {
        let production = production_document()?;
        let mut rows = metadata().to_vec();
        let Some(check) = rows.iter_mut().find(|row| row.id == "cmd:check") else {
            return Err("cmd:check metadata missing".to_string());
        };
        // A summary edit is a single metadata-field change. Cost/operation
        // are governed by the coherence invariants, so they cannot be
        // mutated one-sided the way control 9 models a real edit.
        check.summary = "mutation-pinned summary proving row-isolation control";
        let mutated = build_document(catalog(), &rows, workflow_catalog())
            .map_err(|error| error.to_string())?;
        if mutated.catalog_digest == production.catalog_digest {
            return Err("a metadata change must remint the digest".to_string());
        }
        let mut changed = 0;
        for (before, after) in production.commands.iter().zip(mutated.commands.iter()) {
            let before_json =
                serde_json::to_string(before).map_err(|error| format!("serialize row: {error}"))?;
            let after_json =
                serde_json::to_string(after).map_err(|error| format!("serialize row: {error}"))?;
            if before_json != after_json {
                changed += 1;
                if before.id != "cmd:check" {
                    return Err(format!("unintended row change: {}", before.id));
                }
            }
        }
        if changed != 1 {
            return Err(format!("expected exactly one changed row, got {changed}"));
        }
        Ok(())
    }

    #[test]
    fn single_workflow_transition_change_updates_row_and_digest() -> Result<(), String> {
        let production = production_document()?;
        let mut workflows = workflow_catalog().to_vec();
        let Some(row) = workflows.iter_mut().find(|row| row.id == "inspect-change") else {
            return Err("inspect-change missing".to_string());
        };
        let mut families = row.result_families.to_vec();
        // Mutate an already-terminal transition's statement. Re-pointing a
        // command edge would sever declared reachability, which is a graph
        // invariant, not the row-isolation surface this control targets.
        let Some(family) = families
            .iter_mut()
            .find(|family| matches!(family.next, WorkflowNext::Terminal(_)))
        else {
            return Err("inspect-change has no terminal family".to_string());
        };
        family.next = WorkflowNext::Terminal(
            "stop: pinned transition mutation for the row-isolation control",
        );
        row.result_families = Box::leak(families.into_boxed_slice());
        let mutated =
            build_document(catalog(), metadata(), &workflows).map_err(|error| error.to_string())?;
        if mutated.catalog_digest == production.catalog_digest {
            return Err("a workflow transition change must remint the digest".to_string());
        }
        let mut changed = 0;
        for (before, after) in production.workflows.iter().zip(mutated.workflows.iter()) {
            let before_json =
                serde_json::to_string(before).map_err(|error| format!("serialize row: {error}"))?;
            let after_json =
                serde_json::to_string(after).map_err(|error| format!("serialize row: {error}"))?;
            if before_json != after_json {
                changed += 1;
                if before.id != "inspect-change" {
                    return Err(format!("unintended workflow change: {}", before.id));
                }
            }
        }
        if changed != 1 {
            return Err(format!(
                "expected exactly one changed workflow, got {changed}"
            ));
        }
        Ok(())
    }

    #[test]
    fn human_help_wording_is_outside_the_identity_surface() -> Result<(), String> {
        let rendered = render_help_json()?;
        // The renderer headers of the human workflow route are not governed
        // catalog fields; they must never leak into the machine document.
        if rendered.contains("RIPR workflows — bounded task guidance") {
            return Err("human help header leaked into the machine document".to_string());
        }
        if rendered.contains("Nothing on this screen runs a command") {
            return Err("human help copy leaked into the machine document".to_string());
        }
        Ok(())
    }

    #[test]
    fn classifications_and_aliases_are_projected() -> Result<(), String> {
        let document = production_document()?;
        let mut compatibility = 0;
        let mut advanced = 0;
        for row in &document.commands {
            match row.class {
                "compatibility" => compatibility += 1,
                "advanced" => advanced += 1,
                "public" | "internal" => {}
                other => return Err(format!("unknown classification {other:?}")),
            }
            let entry = catalog()
                .iter()
                .find(|entry| entry.id == row.id)
                .ok_or_else(|| format!("catalog lost {:?}", row.id))?;
            // Aliases are compatibility identity (C1): every source alias must
            // project exactly, in the governed sorted order, for every class.
            let mut expected_aliases = entry.aliases.to_vec();
            expected_aliases.sort_unstable();
            if row.aliases != expected_aliases {
                return Err(format!("row {:?} lost or reordered its aliases", row.id));
            }
            if (entry.class == CommandClass::Public) != (row.class == "public") {
                return Err(format!("row {:?} misprojects its classification", row.id));
            }
        }
        if compatibility == 0 || advanced == 0 {
            return Err("compatibility and advanced classifications must both project".to_string());
        }
        Ok(())
    }

    #[test]
    fn compatibility_relation_projects_the_canonical_target() -> Result<(), String> {
        let document = production_document()?;
        let Some(row) = document
            .commands
            .iter()
            .find(|row| row.id == "cmd:compat.start-here")
        else {
            return Err("cmd:compat.start-here row missing".to_string());
        };
        // A machine consumer must resolve the compatibility spelling to its
        // canonical command without scraping human help (C1 relation).
        match &row.relation {
            super::RelationJson::AliasOf { target } if *target == "first-pr" => {}
            other => {
                return Err(format!("start-here relation misprojected: {other:?}").to_string());
            }
        }
        if row.discovery != "compatibility_alias" {
            return Err(format!(
                "start-here discovery posture misprojected: {}",
                row.discovery
            ));
        }
        Ok(())
    }

    const MUTATED_LIMITATIONS: &[&str] = &["mutated document copy pin"];

    #[test]
    fn catalog_digest_ignores_product_version_and_document_copy() -> Result<(), String> {
        let mut document = production_document()?;
        let original_digest = document.catalog_digest.clone();
        // A package release or a wording edit of the boundary statements
        // remints neither the catalog nor its digest.
        document.product_version = "0.0.0-mutation-pin";
        document.limitations = MUTATED_LIMITATIONS;
        document.non_claims = MUTATED_LIMITATIONS;
        let reminted = super::catalog_digest(&document)?;
        if reminted != original_digest {
            return Err(
                "catalog digest moved under a product-version or document-copy-only change"
                    .to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn step_effects_follow_the_supplied_metadata_table() -> Result<(), String> {
        let mut rows = metadata().to_vec();
        let Some(check) = rows.iter_mut().find(|row| row.id == "cmd:check") else {
            return Err("cmd:check metadata missing".to_string());
        };
        // An unmirrored flag: valid to differ in a supplied table, and the
        // workflow step projection must follow the same table the command
        // rows project from — never the production global.
        check.effects.may_use_network = true;
        let document = build_document(catalog(), &rows, workflow_catalog())
            .map_err(|error| error.to_string())?;
        let Some(workflow) = document
            .workflows
            .iter()
            .find(|row| row.id == "inspect-change")
        else {
            return Err("inspect-change missing".to_string());
        };
        let Some(step) = workflow
            .steps
            .iter()
            .find(|step| step.command == "cmd:check")
        else {
            return Err("cmd:check step missing".to_string());
        };
        if !step.effects.may_use_network {
            return Err("step effects ignored the supplied metadata table".to_string());
        }
        let Some(command) = document.commands.iter().find(|row| row.id == "cmd:check") else {
            return Err("cmd:check command row missing".to_string());
        };
        if command.effects.may_use_network != step.effects.may_use_network {
            return Err("command and step projections disagree".to_string());
        }
        Ok(())
    }
}
