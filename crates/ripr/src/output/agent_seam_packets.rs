//! Render classified seam gaps as agent-ready packets per
//! RIPR-SPEC-0005 (and the agent-packet shape in
//! `docs/OUTPUT_SCHEMA.md` § "Agent Seam Packets").
//!
//! Packets are emitted for actionable classes:
//!
//! - Headline-eligible classes (`Ungripped`, `WeaklyGripped`,
//!   `ReachableUnrevealed`, the four `*_unknown` classes) emit a
//!   `task: "write_targeted_test"` packet.
//! - `Opaque` emits a conservative `task: "inspect_static_limitation"`
//!   packet so the agent at least sees the static boundary.
//!
//! `StronglyGripped`, `Intentional`, and `Suppressed` produce no
//! packet — there is nothing for the agent to do.
//!
//! The packet schema is **0.3**, intentionally distinct from the
//! repo-exposure report's 0.1, because the packet is a separate
//! contract aimed at coding agents rather than reviewers.

use crate::analysis::ClassifiedSeam;
use crate::analysis::canonical_gap::{CanonicalGapIdentity, canonical_gap_identities};
use crate::analysis::seams::{ExpectedSink, RequiredDiscriminator, SeamGripClass, SeamKind};
use crate::analysis::test_grip_evidence::TestGripEvidence;
use crate::output::evidence_record::{evidence_record_for, evidence_record_json_value};
use crate::output::gap_decision_ledger::{GapRecord, GapRepairRoute, projection_eligible};
use crate::output::json::escape as json_escape;
use crate::output::path::{display_path, display_path_text};
use serde_json::json;

pub(crate) const AGENT_SEAM_PACKET_SCHEMA_VERSION: &str = "0.3";

/// Cap on related-tests rendered per packet. Mirrors the JSON-side
/// limit in `output::repo_exposure` so an agent inspecting the same
/// seam from either artifact sees the same evidence size.
const MAX_RELATED_TESTS_PER_PACKET: usize = 8;

/// Boilerplate string surfaced under `runtime_confirmation` to remind
/// agents that static evidence is preflight, not proof.
const RUNTIME_CONFIRMATION_NOTE: &str =
    "optional cargo-mutants confirmation; ripr reports static evidence only";

/// Boundary surfaced as a typed field so agent consumers can carry the
/// same non-claim language as `ripr first-pr` without scraping prose.
const STATIC_EVIDENCE_BOUNDARY: &str = "static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval.";

/// Render every actionable `ClassifiedSeam` in `classified` as an agent
/// packet, returning a JSON object with a `packets` array. Strongly-gripped,
/// intentional, and suppressed seams are skipped. `Opaque` seams emit a
/// conservative `inspect_static_limitation` packet so the agent at least
/// sees the static boundary that hides evidence.
pub(crate) fn render_agent_seam_packets_json(classified: &[ClassifiedSeam]) -> String {
    let canonical_gaps = canonical_gap_identities(classified);
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": \"{}\",\n",
        AGENT_SEAM_PACKET_SCHEMA_VERSION
    ));
    out.push_str("  \"scope\": \"repo\",\n");

    let actionable: Vec<&ClassifiedSeam> = classified
        .iter()
        .filter(|entry| is_actionable(entry.class))
        .collect();

    out.push_str(&format!("  \"packets_total\": {},\n", actionable.len()));
    out.push_str("  \"packets\": [");
    for (idx, entry) in actionable.iter().enumerate() {
        if idx == 0 {
            out.push('\n');
        }
        push_packet_json(&mut out, entry, canonical_gaps.get(entry.seam.id()));
        if idx + 1 != actionable.len() {
            out.push_str(",\n");
        } else {
            out.push('\n');
        }
    }
    if !actionable.is_empty() {
        out.push_str("  ");
    }
    out.push_str("]\n");
    out.push_str("}\n");
    out
}

/// Render the existing agent seam packet JSON envelope for one seam.
pub(crate) fn render_agent_seam_packet_json(entry: &ClassifiedSeam) -> String {
    render_agent_seam_packets_json(std::slice::from_ref(entry))
}

/// Render one explicit GapRecord as an agent packet. This is the same
/// agent-packet envelope used by seam packets, but the source is the gap
/// decision ledger, so callers do not rerun analysis or infer repairability
/// from raw static classifications.
pub(crate) fn render_agent_gap_record_packet_json(
    gap_ledger_path: &str,
    record: &GapRecord,
) -> Result<String, String> {
    validate_agent_gap_record_packet(record)?;
    let Some(route) = record.repair_route.as_ref() else {
        return Err("requires a repair_route".to_string());
    };
    let gap_id = gap_record_id(record);
    let Some(verify_command) = record.verification_commands.first().cloned() else {
        return Err("requires verification_commands".to_string());
    };
    let stop_conditions = stop_conditions_for(route);
    let anchor = record.anchor.as_ref();
    let file = anchor
        .and_then(|anchor| anchor.file.as_deref())
        .map(display_path_text);
    let line = anchor.and_then(|anchor| anchor.line);
    let owner = anchor.and_then(|anchor| anchor.owner.as_deref());
    let recommended_file = route
        .target_file
        .as_deref()
        .or(route.related_test.as_deref())
        .map(display_path_text);
    let authority_boundary = if record.authority_boundary.trim().is_empty() {
        "Agent packets are advisory; configured gate-decision artifacts remain pass/fail authority."
            .to_string()
    } else {
        record.authority_boundary.clone()
    };
    let pasteable_packet = pasteable_gap_repair_packet(
        gap_ledger_path,
        record,
        route,
        &verify_command,
        &stop_conditions,
        authority_boundary.as_str(),
    );
    let packet = json!({
        "task": task_for_gap_route(route),
        "source": "gap_decision_ledger",
        "gap_id": gap_id,
        "canonical_gap_id": non_empty(&record.canonical_gap_id),
        "gap_kind": record.kind.as_str(),
        "language": record.language.as_str(),
        "language_status": record.language_status.as_str(),
        "policy_state": record.policy_state.as_str(),
        "gap_state": record.gap_state.as_str(),
        "evidence_class": record.evidence_class.as_str(),
        "repairability": record.repairability.as_str(),
        "file": file,
        "line": line,
        "owner": owner,
        "anchor": {
            "file": anchor.and_then(|anchor| anchor.file.as_deref()).map(display_path_text),
            "line": line,
            "owner": owner,
            "dedupe_fingerprint": anchor.and_then(|anchor| anchor.dedupe_fingerprint.as_deref()),
        },
        "repair_route": route,
        "repair_kind": route.route_kind.as_str(),
        "changed_behavior": route.changed_behavior.as_deref(),
        "recommended_test": {
            "file": recommended_file,
            "name": route.related_test.as_deref(),
            "reason": recommended_test_reason(route),
        },
        "assertion_shape": route.assertion_shape.as_deref(),
        "evidence_ids": &record.evidence_ids,
        "verification_commands": &record.verification_commands,
        "verify_command": verify_command,
        "stop_conditions": stop_conditions,
        "repair_card": {
            "gap_kind": record.kind.as_str(),
            "changed_behavior": route.changed_behavior.as_deref(),
            "repair": repair_text_for_gap_route(route),
            "repair_route": route,
            "verification_commands": &record.verification_commands,
            "verify_command": verify_command,
            "source_artifact": gap_ledger_path,
            "authority_boundary": authority_boundary,
        },
        "llm_guidance": {
            "prompt": gap_record_prompt(route, &verify_command),
            "verify_command": verify_command,
            "stop_conditions": stop_conditions,
            "copyable_packet": pasteable_packet,
        },
        "runtime_confirmation": RUNTIME_CONFIRMATION_NOTE,
        "static_evidence_boundary": STATIC_EVIDENCE_BOUNDARY,
    });
    let envelope = json!({
        "schema_version": AGENT_SEAM_PACKET_SCHEMA_VERSION,
        "scope": "repo",
        "source": "gap_decision_ledger",
        "inputs": {
            "gap_ledger": gap_ledger_path,
        },
        "packets_total": 1,
        "packets": [packet],
    });
    let mut rendered = serde_json::to_string_pretty(&envelope)
        .map_err(|err| format!("render agent gap packet JSON failed: {err}"))?;
    rendered.push('\n');
    Ok(rendered)
}

/// Return the first concrete assertion example carried by the agent
/// seam packet v2 shape. This follows the packet content itself:
/// any seam with a concrete assertion template can expose the editor
/// action, while prose-only guidance remains hidden.
pub(crate) fn suggested_assertion_for_classified_seam(entry: &ClassifiedSeam) -> Option<String> {
    suggested_assertions_for(
        entry.seam.kind(),
        entry.seam.owner(),
        Some(entry.seam.required_discriminator()),
        &entry.evidence,
    )
    .into_iter()
    .find(|suggestion| {
        let trimmed = suggestion.trim_start();
        !trimmed.starts_with("//") && trimmed.contains("assert")
    })
}

/// Render a compact human/agent work order for the next targeted test.
/// This is intentionally derived from the same fields as the structured
/// agent seam packet so editor actions and JSON packets stay aligned.
pub(crate) fn targeted_test_brief_for_classified_seam(entry: &ClassifiedSeam) -> String {
    let seam = &entry.seam;
    let evidence = &entry.evidence;
    let missing = missing_discriminator_records_for(entry);
    let patterns_to_imitate = patterns_to_imitate_for(evidence);
    let patterns_to_avoid = patterns_to_avoid_for(entry);
    let outline = targeted_test_brief_outline_for_classified_seam(entry);

    let mut out = String::new();
    out.push_str("Target seam:\n");
    out.push_str(&format!(
        "- {}:{}\n",
        display_path(seam.file()),
        seam.display_line()
    ));
    out.push_str(&format!("- {}\n", seam.kind().as_str()));
    out.push_str(&format!("- {}\n", entry.class.as_str()));
    out.push_str(&format!("- owner: {}\n", seam.owner()));

    out.push_str("\nWhy it matters:\n");
    if let Some(test) = evidence.related_tests.first() {
        out.push_str(&format!(
            "- Related test evidence: {} uses {} {} oracle.\n",
            test.test_name,
            test.oracle_strength.as_str(),
            test.oracle_kind.as_str()
        ));
    } else {
        out.push_str("- No related test location is visible in saved-workspace analysis.\n");
    }
    out.push_str(&format!(
        "- Static discriminator summary: {}\n",
        evidence.discriminate.summary
    ));
    for record in missing.iter().take(3) {
        out.push_str(&format!(
            "- Missing discriminator: {} ({})\n",
            record.value, record.reason
        ));
    }

    out.push_str("\nAdd a targeted test:\n");
    out.push_str(&format!(
        "- Suggested file: {}\n",
        display_path_text(&outline.suggested_file)
    ));
    out.push_str(&format!("- Suggested name: {}\n", outline.suggested_name));
    if let Some(value) = outline.candidate_value.as_ref() {
        out.push_str(&format!("- Candidate value: {value}\n"));
    }
    out.push_str(&format!("- Assertion shape: {}\n", outline.assertion_shape));

    if !patterns_to_imitate.is_empty() {
        out.push_str("\nImitate:\n");
        for pattern in patterns_to_imitate.iter().take(3) {
            out.push_str(&format!(
                "- {} ({})\n",
                pattern.test.test_name, pattern.reason
            ));
        }
    }

    if !patterns_to_avoid.is_empty() {
        out.push_str("\nAvoid:\n");
        for pattern in patterns_to_avoid.iter().take(3) {
            out.push_str(&format!("- {} ({})\n", pattern.pattern, pattern.reason));
        }
    }

    out
}

pub(crate) struct TargetedTestBriefOutline {
    pub(crate) suggested_file: String,
    pub(crate) suggested_name: String,
    pub(crate) candidate_value: Option<String>,
    pub(crate) assertion_shape: String,
}

pub(crate) fn targeted_test_brief_outline_for_classified_seam(
    entry: &ClassifiedSeam,
) -> TargetedTestBriefOutline {
    let recommended = recommended_test_for(entry);
    let missing = missing_discriminator_records_for(entry);
    let candidate_value = candidate_values_for(entry, &missing)
        .into_iter()
        .next()
        .map(|value| value.value);
    let assertion_shape = assertion_shape_for_entry(entry);

    TargetedTestBriefOutline {
        suggested_file: recommended.file,
        suggested_name: recommended.name,
        candidate_value,
        assertion_shape: assertion_shape.example,
    }
}

fn validate_agent_gap_record_packet(record: &GapRecord) -> Result<(), String> {
    let projection = record
        .projection_eligibility
        .get("agent_packet")
        .ok_or_else(|| "is not agent-packet eligible: missing projection".to_string())?;
    if !projection_eligible(record, "agent_packet") {
        let reason = projection.reason.trim();
        if reason.is_empty() {
            return Err("is not agent-packet eligible".to_string());
        }
        return Err(format!("is not agent-packet eligible: {reason}"));
    }
    let Some(route) = record.repair_route.as_ref() else {
        return Err("requires a repair_route".to_string());
    };
    if record.verification_commands.is_empty() {
        return Err("requires verification_commands".to_string());
    }
    if record.repairability != "repairable" && route.route_kind != "InspectStaticLimit" {
        return Err("requires a repairable gap or bounded inspection route".to_string());
    }
    Ok(())
}

fn gap_record_id(record: &GapRecord) -> String {
    if let Some(gap_id) = non_empty(&record.gap_id) {
        return gap_id;
    }
    if let Some(canonical_gap_id) = non_empty(&record.canonical_gap_id) {
        return canonical_gap_id;
    }
    "unknown-gap".to_string()
}

fn non_empty(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_string())
}

fn task_for_gap_route(route: &GapRepairRoute) -> &'static str {
    match route.route_kind.as_str() {
        "InspectStaticLimit" => "inspect_static_limitation",
        "AddOutputGolden" => "add_output_golden",
        _ => "write_targeted_test",
    }
}

fn recommended_test_reason(route: &GapRepairRoute) -> &'static str {
    match route.route_kind.as_str() {
        "AddOutputGolden" => "add or update the output-contract proof named by the gap route",
        "InspectStaticLimit" => "inspect the static limitation before changing tests",
        _ => "place the focused repair where the gap route points",
    }
}

fn repair_text_for_gap_route(route: &GapRepairRoute) -> String {
    if let Some(assertion_shape) = route.assertion_shape.clone() {
        return assertion_shape;
    }
    if let Some(changed_behavior) = route.changed_behavior.clone() {
        return changed_behavior;
    }
    format!("Follow repair route `{}`.", route.route_kind)
}

fn stop_conditions_for(route: &GapRepairRoute) -> Vec<String> {
    let mut conditions = route.stop_conditions.clone();
    if conditions.is_empty() {
        conditions.push(
            "Stop if the gap record is no longer present or loses agent-packet eligibility."
                .to_string(),
        );
        conditions
            .push("Stop if the verification command cannot run from this workspace.".to_string());
    }
    conditions
}

fn gap_record_prompt(route: &GapRepairRoute, verify_command: &str) -> String {
    let repair = repair_text_for_gap_route(route);
    format!(
        "{repair} Use the supplied GapRecord fields as the repair boundary. Verify with `{verify_command}`."
    )
}

fn pasteable_gap_repair_packet(
    gap_ledger_path: &str,
    record: &GapRecord,
    route: &GapRepairRoute,
    verify_command: &str,
    stop_conditions: &[String],
    authority_boundary: &str,
) -> serde_json::Value {
    let gap_id = gap_record_id(record);
    let task = format!(
        "Repair the `{}` gap `{}` using the bounded `{}` route.",
        record.kind, gap_id, route.route_kind
    );
    let context = gap_record_packet_context(gap_ledger_path, record, route);
    let repair = gap_record_packet_repair(route);
    let verification = gap_record_packet_verification(record, verify_command);
    let receipt = gap_record_packet_receipt(record);
    let do_not_do = gap_record_packet_do_not_do(record);
    let markdown = pasteable_packet_markdown(PasteablePacketSections {
        task: &task,
        context: &context,
        repair: &repair,
        verification: &verification,
        receipt: &receipt,
        stop_conditions,
        do_not_do: &do_not_do,
        authority_boundary,
    });
    json!({
        "task": task,
        "context": context,
        "repair": repair,
        "verification": verification,
        "receipt": receipt,
        "stop_conditions": stop_conditions,
        "do_not_do": do_not_do,
        "authority_boundary": authority_boundary,
        "markdown": markdown,
    })
}

fn gap_record_packet_context(
    gap_ledger_path: &str,
    record: &GapRecord,
    route: &GapRepairRoute,
) -> Vec<String> {
    let mut context = Vec::new();
    context.push(format!(
        "Gap kind: {}; state: {}; policy: {}.",
        record.kind, record.gap_state, record.policy_state
    ));
    context.push(format!(
        "Language: {} ({}).",
        record.language, record.language_status
    ));
    if let Some(anchor) = record.anchor.as_ref() {
        if let Some(file) = anchor.file.as_deref() {
            let mut location = format!("Anchor: {}", display_path_text(file));
            if let Some(line) = anchor.line {
                location.push_str(&format!(":{line}"));
            }
            if let Some(owner) = anchor.owner.as_deref() {
                location.push_str(&format!(" in `{owner}`"));
            }
            location.push('.');
            context.push(location);
        } else if let Some(owner) = anchor.owner.as_deref() {
            context.push(format!("Anchor owner: `{owner}`."));
        }
    }
    if let Some(changed_behavior) = route.changed_behavior.as_deref() {
        context.push(format!("Changed behavior: `{changed_behavior}`."));
    }
    if let Some(discriminator) = route
        .assertion_shape
        .as_deref()
        .or(route.changed_behavior.as_deref())
    {
        context.push(format!("Missing discriminator: `{discriminator}`."));
    }
    if let Some(receipt_command) = record.receipt_command.as_deref() {
        context.push(format!("Receipt command: `{receipt_command}`."));
    }
    if let Some(receipt_path) = record
        .receipt
        .as_ref()
        .and_then(|receipt| receipt.path.as_deref())
    {
        context.push(format!(
            "Receipt path: {}.",
            display_path_text(receipt_path)
        ));
    }
    if let Some(target_file) = route.target_file.as_deref() {
        context.push(format!(
            "Repair target: {}.",
            display_path_text(target_file)
        ));
    }
    if let Some(related_test) = route.related_test.as_deref() {
        context.push(format!("Related test or proof target: `{related_test}`."));
    }
    if !record.evidence_ids.is_empty() {
        context.push(format!("Evidence IDs: {}.", record.evidence_ids.join(", ")));
    }
    context.push(format!("Source artifact: {}.", gap_ledger_path));
    context
}

fn gap_record_packet_repair(route: &GapRepairRoute) -> Vec<String> {
    let mut repair = Vec::new();
    repair.push(format!("Use repair route `{}`.", route.route_kind));
    repair.push(format!(
        "Focused proof intent: {}",
        gap_record_packet_focused_proof_intent(route)
    ));
    if let Some(assertion_shape) = route.assertion_shape.as_deref() {
        repair.push(format!(
            "Add or strengthen this check: `{assertion_shape}`."
        ));
        repair.push(format!(
            "Focused proof intent: add one focused assertion or output proof for `{assertion_shape}`."
        ));
    } else if let Some(changed_behavior) = route.changed_behavior.as_deref() {
        repair.push(format!("Add a focused check for `{changed_behavior}`."));
        repair.push(format!(
            "Focused proof intent: add one focused assertion or output proof for `{changed_behavior}`."
        ));
    } else {
        repair.push(repair_text_for_gap_route(route));
    }
    match route.route_kind.as_str() {
        "AddOutputGolden" => {
            repair.push(
                "Add or update the checked output/golden proof named by the route.".to_string(),
            );
        }
        "InspectStaticLimit" => {
            repair.push(
                "Inspect the named static limit before changing tests or output proofs."
                    .to_string(),
            );
        }
        _ => {
            repair.push(
                "Keep the repair focused on the selected gap and its related test target."
                    .to_string(),
            );
        }
    }
    repair
}

fn gap_record_packet_focused_proof_intent(route: &GapRepairRoute) -> String {
    let target = route
        .target_file
        .as_deref()
        .or(route.related_test.as_deref())
        .map(|target| format!(" in `{target}`"))
        .unwrap_or_default();
    match route.route_kind.as_str() {
        "AddOutputGolden" => route
            .assertion_shape
            .as_deref()
            .map(|assertion| format!("Add or update the output proof{target} so `{assertion}`."))
            .unwrap_or_else(|| format!("Add or update the output proof{target}.")),
        "AddBoundaryAssertion" => route
            .assertion_shape
            .as_deref()
            .map(|assertion| format!("Add a focused boundary assertion{target}: `{assertion}`."))
            .unwrap_or_else(|| format!("Add a focused boundary assertion{target}.")),
        "AddValueAssertion" => route
            .assertion_shape
            .as_deref()
            .map(|assertion| format!("Add a focused value assertion{target}: `{assertion}`."))
            .unwrap_or_else(|| format!("Add a focused value assertion{target}.")),
        "AddErrorDiscriminator" => route
            .assertion_shape
            .as_deref()
            .map(|assertion| format!("Add a focused error-path assertion{target}: `{assertion}`."))
            .unwrap_or_else(|| format!("Add a focused error-path assertion{target}.")),
        _ => route
            .assertion_shape
            .as_deref()
            .map(|assertion| format!("Add the focused proof{target}: `{assertion}`."))
            .or_else(|| {
                route
                    .changed_behavior
                    .as_deref()
                    .map(|changed| format!("Add a focused check{target} for `{changed}`."))
            })
            .unwrap_or_else(|| format!("Add the focused proof{target}.")),
    }
}

fn gap_record_packet_verification(record: &GapRecord, verify_command: &str) -> Vec<String> {
    let mut verification = Vec::new();
    verification.push(verify_command.to_string());
    if let Some(receipt_command) = record.receipt_command.as_deref() {
        verification.push(receipt_command.to_string());
    }
    verification
}

fn gap_record_packet_receipt(record: &GapRecord) -> Vec<String> {
    let mut receipt = Vec::new();
    if let Some(receipt_command) = record.receipt_command.as_deref() {
        receipt.push(format!("Run `{receipt_command}` after verification."));
    }
    if let Some(receipt_path) = record
        .receipt
        .as_ref()
        .and_then(|receipt| receipt.path.as_deref())
    {
        receipt.push(format!(
            "Keep receipt artifact `{receipt_path}` with the review."
        ));
    }
    if receipt.is_empty() {
        receipt.push("No receipt command or path was supplied by the gap record.".to_string());
    }
    receipt
}

fn gap_record_packet_do_not_do(record: &GapRecord) -> Vec<String> {
    let mut guidance = vec![
        "Do not edit production code unless the focused proof exposes a real product defect."
            .to_string(),
        "Do not broaden the change beyond this GapRecord and its repair route.".to_string(),
        "Do not treat static evidence as runtime mutation, coverage, or correctness proof."
            .to_string(),
        "Do not generate tests or call external providers from this packet.".to_string(),
    ];
    if record.language_status != "stable" {
        guidance.push("Do not treat preview-language evidence as gate authority.".to_string());
    }
    guidance
}

struct PasteablePacketSections<'a> {
    task: &'a str,
    context: &'a [String],
    repair: &'a [String],
    verification: &'a [String],
    receipt: &'a [String],
    stop_conditions: &'a [String],
    do_not_do: &'a [String],
    authority_boundary: &'a str,
}

fn pasteable_packet_markdown(sections: PasteablePacketSections<'_>) -> String {
    let mut out = String::new();
    out.push_str("## Task\n");
    out.push_str(sections.task);
    out.push_str("\n\n## Context\n");
    push_markdown_bullets(&mut out, sections.context);
    out.push_str("\n## Repair\n");
    push_markdown_bullets(&mut out, sections.repair);
    out.push_str("\n## Verification\n");
    push_markdown_bullets(&mut out, sections.verification);
    out.push_str("\n## Receipt\n");
    push_markdown_bullets(&mut out, sections.receipt);
    out.push_str("\n## Stop Conditions\n");
    push_markdown_bullets(&mut out, sections.stop_conditions);
    out.push_str("\n## Do Not Do\n");
    push_markdown_bullets(&mut out, sections.do_not_do);
    out.push_str("\n## Authority\n");
    out.push_str(sections.authority_boundary);
    out
}

fn push_markdown_bullets(out: &mut String, items: &[String]) {
    if items.is_empty() {
        out.push_str("- None supplied.\n");
        return;
    }
    for item in items {
        out.push_str("- ");
        out.push_str(item);
        out.push('\n');
    }
}

fn is_actionable(class: SeamGripClass) -> bool {
    // Headline-eligible classes are the natural agent targets.
    // `Opaque` is also actionable as `inspect_static_limitation`.
    // `Intentional` and `Suppressed` are governance classes; the
    // agent should not be told to "fix" them.
    class.is_headline_eligible() || matches!(class, SeamGripClass::Opaque)
}

fn task_for(class: SeamGripClass) -> &'static str {
    match class {
        SeamGripClass::Opaque => "inspect_static_limitation",
        _ => "write_targeted_test",
    }
}

fn push_packet_json(
    out: &mut String,
    entry: &ClassifiedSeam,
    canonical_gap: Option<&CanonicalGapIdentity>,
) {
    let seam = &entry.seam;
    let evidence = &entry.evidence;
    out.push_str("    {\n");
    out.push_str(&format!("      \"task\": \"{}\",\n", task_for(entry.class)));
    out.push_str(&format!(
        "      \"seam_id\": \"{}\",\n",
        json_escape(seam.id().as_str())
    ));
    out.push_str(&format!(
        "      \"owner\": \"{}\",\n",
        json_escape(seam.owner())
    ));
    out.push_str(&format!(
        "      \"seam_kind\": \"{}\",\n",
        seam.kind().as_str()
    ));
    out.push_str(&format!(
        "      \"file\": \"{}\",\n",
        json_escape(&display_path(seam.file()))
    ));
    out.push_str(&format!("      \"line\": {},\n", seam.display_line()));
    out.push_str(&format!(
        "      \"changed_expression\": \"{}\",\n",
        json_escape(seam.expression())
    ));
    out.push_str(&format!(
        "      \"current_grip\": \"{}\",\n",
        entry.class.as_str()
    ));
    out.push_str(&format!(
        "      \"headline_eligible\": {},\n",
        entry.class.is_headline_eligible()
    ));

    let recommended = recommended_test_for(entry);
    out.push_str("      \"recommended_test\": {");
    out.push_str(&format!(
        "\"name\": \"{}\", ",
        json_escape(recommended.name.as_str())
    ));
    out.push_str(&format!(
        "\"file\": \"{}\", ",
        json_escape(recommended.file.as_str())
    ));
    out.push_str(&format!(
        "\"reason\": \"{}\"",
        json_escape(recommended.reason.as_str())
    ));
    out.push_str("},\n");

    let nearest_strong = nearest_strong_test_to_imitate(evidence);
    out.push_str("      \"nearest_strong_test_to_imitate\": ");
    if let Some(test) = nearest_strong {
        push_related_test_reference(out, test, "nearest strong related test by ranked evidence");
    } else {
        out.push_str("null");
    }
    out.push_str(",\n");

    out.push_str("      \"evidence\": {");
    out.push_str(&format!(
        "\"reach\": \"{}\", ",
        evidence.reach.state.as_str()
    ));
    out.push_str(&format!(
        "\"activate\": \"{}\", ",
        evidence.activate.state.as_str()
    ));
    out.push_str(&format!(
        "\"propagate\": \"{}\", ",
        evidence.propagate.state.as_str()
    ));
    out.push_str(&format!(
        "\"observe\": \"{}\", ",
        evidence.observe.state.as_str()
    ));
    out.push_str(&format!(
        "\"discriminate\": \"{}\"",
        evidence.discriminate.state.as_str()
    ));
    out.push_str("},\n");

    out.push_str("      \"observed_values\": [");
    for (idx, value) in evidence.observed_values.iter().enumerate() {
        out.push_str(&format!("\"{}\"", json_escape(value.value.as_str())));
        if idx + 1 != evidence.observed_values.len() {
            out.push_str(", ");
        }
    }
    out.push_str("],\n");

    let missing = missing_discriminator_records_for(entry);
    let candidate_values = candidate_values_for(entry, &missing);
    out.push_str("      \"missing_discriminators\": [");
    if !missing.is_empty() {
        out.push('\n');
        for (idx, record) in missing.iter().enumerate() {
            out.push_str(&format!(
                "        {{\"value\": \"{}\", \"reason\": \"{}\"}}",
                json_escape(record.value.as_str()),
                json_escape(record.reason.as_str())
            ));
            if idx + 1 != missing.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("      ");
    }
    out.push_str("],\n");

    out.push_str("      \"candidate_values\": [");
    if !candidate_values.is_empty() {
        out.push('\n');
        for (idx, value) in candidate_values.iter().enumerate() {
            out.push_str(&format!(
                "        {{\"value\": \"{}\", \"reason\": \"{}\"}}",
                json_escape(value.value.as_str()),
                json_escape(value.reason.as_str())
            ));
            if idx + 1 != candidate_values.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("      ");
    }
    out.push_str("],\n");

    out.push_str(&format!(
        "      \"missing_oracle_shape\": \"{}\",\n",
        json_escape(&missing_oracle_shape_for(seam.kind(), seam.expected_sink()))
    ));

    let assertion_shape = assertion_shape_for_entry(entry);
    out.push_str("      \"assertion_shape\": {");
    out.push_str(&format!("\"kind\": \"{}\", ", assertion_shape.kind));
    out.push_str(&format!(
        "\"example\": \"{}\"",
        json_escape(assertion_shape.example.as_str())
    ));
    out.push_str("},\n");

    out.push_str("      \"related_existing_tests\": [");
    if !evidence.related_tests.is_empty() {
        out.push('\n');
        let cap = evidence
            .related_tests
            .len()
            .min(MAX_RELATED_TESTS_PER_PACKET);
        for (idx, grip) in evidence.related_tests.iter().take(cap).enumerate() {
            out.push_str("        {");
            out.push_str(&format!(
                "\"name\": \"{}\", ",
                json_escape(grip.test_name.as_str())
            ));
            out.push_str(&format!(
                "\"file\": \"{}\", ",
                json_escape(&display_path(&grip.file))
            ));
            out.push_str(&format!("\"line\": {}, ", grip.line));
            out.push_str(&format!(
                "\"oracle_kind\": \"{}\", ",
                grip.oracle_kind.as_str()
            ));
            out.push_str(&format!(
                "\"oracle_strength\": \"{}\", ",
                grip.oracle_strength.as_str()
            ));
            out.push_str(&format!(
                "\"evidence_summary\": \"{}\", ",
                json_escape(grip.evidence_summary.as_str())
            ));
            out.push_str(&format!(
                "\"relation_reason\": \"{}\", ",
                grip.relation_reason.as_str()
            ));
            out.push_str(&format!(
                "\"relation_confidence\": \"{}\"",
                grip.relation_confidence.as_str()
            ));
            out.push('}');
            if idx + 1 != cap {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("      ");
    }
    out.push_str("],\n");

    let patterns_to_imitate = patterns_to_imitate_for(evidence);
    out.push_str("      \"patterns_to_imitate\": [");
    if !patterns_to_imitate.is_empty() {
        out.push('\n');
        for (idx, pattern) in patterns_to_imitate.iter().enumerate() {
            out.push_str("        ");
            push_related_test_reference(out, pattern.test, pattern.reason.as_str());
            if idx + 1 != patterns_to_imitate.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("      ");
    }
    out.push_str("],\n");

    let patterns_to_avoid = patterns_to_avoid_for(entry);
    out.push_str("      \"patterns_to_avoid\": [");
    if !patterns_to_avoid.is_empty() {
        out.push('\n');
        for (idx, pattern) in patterns_to_avoid.iter().enumerate() {
            out.push_str(&format!(
                "        {{\"pattern\": \"{}\", \"reason\": \"{}\"}}",
                json_escape(pattern.pattern.as_str()),
                json_escape(pattern.reason.as_str())
            ));
            if idx + 1 != patterns_to_avoid.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("      ");
    }
    out.push_str("],\n");

    let suggested = suggested_assertions_for(
        seam.kind(),
        seam.owner(),
        Some(seam.required_discriminator()),
        evidence,
    );
    out.push_str("      \"suggested_assertions\": [");
    for (idx, suggestion) in suggested.iter().enumerate() {
        out.push_str(&format!("\"{}\"", json_escape(suggestion)));
        if idx + 1 != suggested.len() {
            out.push_str(", ");
        }
    }
    out.push_str("],\n");
    out.push_str(&format!(
        "      \"confidence\": \"{}\",\n",
        packet_confidence_for(entry)
    ));
    let evidence_record = evidence_record_json_value(&evidence_record_for(entry, canonical_gap));
    out.push_str("      \"evidence_record\": ");
    out.push_str(&evidence_record.to_string());
    out.push_str(",\n");
    out.push_str(&format!(
        "      \"runtime_confirmation\": \"{}\",\n",
        json_escape(RUNTIME_CONFIRMATION_NOTE)
    ));
    out.push_str(&format!(
        "      \"static_evidence_boundary\": \"{}\"\n",
        json_escape(STATIC_EVIDENCE_BOUNDARY)
    ));
    out.push_str("    }");
}

/// A flat (value, reason) record carried in the packet's
/// `missing_discriminators` array. Mirrors the field shape of
/// `MissingDiscriminatorFact` but excludes `flow_sink` because the
/// packet already carries the sink class via `missing_oracle_shape`.
pub(crate) struct MissingRecord {
    pub(crate) value: String,
    pub(crate) reason: String,
}

pub(crate) struct CandidateValue {
    pub(crate) value: String,
    pub(crate) reason: String,
}

pub(crate) struct RecommendedTest {
    pub(crate) name: String,
    pub(crate) file: String,
    pub(crate) reason: String,
}

pub(crate) struct AssertionShape {
    pub(crate) kind: &'static str,
    pub(crate) example: String,
}

struct ImitationPattern<'a> {
    test: &'a crate::analysis::test_grip_evidence::RelatedTestGrip,
    reason: String,
}

struct AvoidPattern {
    pattern: String,
    reason: String,
}

pub(crate) fn recommended_test_for(entry: &ClassifiedSeam) -> RecommendedTest {
    let owner_short = owner_short(entry.seam.owner());
    let name = format!(
        "{}_{}",
        snake_case_token(owner_short),
        test_name_suffix_for(entry.seam.kind())
    );
    if let Some(test) = nearest_strong_test_to_imitate(&entry.evidence) {
        return RecommendedTest {
            name,
            file: display_path(&test.file),
            reason: "place the new targeted test next to the nearest strong related test"
                .to_string(),
        };
    }
    if let Some(test) = entry.evidence.related_tests.first() {
        return RecommendedTest {
            name,
            file: display_path(&test.file),
            reason: "place the new targeted test next to the highest-confidence related test"
                .to_string(),
        };
    }
    RecommendedTest {
        name,
        file: inferred_test_file(entry.seam.file(), owner_short),
        reason: "no related test file was visible; inferred from the production seam file"
            .to_string(),
    }
}

fn owner_short(owner: &str) -> &str {
    owner.rsplit("::").next().unwrap_or(owner)
}

fn test_name_suffix_for(kind: SeamKind) -> &'static str {
    match kind {
        SeamKind::PredicateBoundary => "boundary_discriminator",
        SeamKind::ErrorVariant => "exact_error_variant",
        SeamKind::ReturnValue => "return_value_discriminator",
        SeamKind::FieldConstruction => "field_discriminator",
        SeamKind::SideEffect => "side_effect_observer",
        SeamKind::MatchArm => "match_arm_discriminator",
        SeamKind::CallPresence => "call_presence_observer",
    }
}

fn inferred_test_file(file: &std::path::Path, owner_short: &str) -> String {
    let stem = file
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(owner_short);
    format!("tests/{}_tests.rs", snake_case_token(stem))
}

fn snake_case_token(raw: &str) -> String {
    let mut out = String::new();
    let mut previous_was_sep = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            previous_was_sep = false;
        } else if !previous_was_sep && !out.is_empty() {
            out.push('_');
            previous_was_sep = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        "targeted".to_string()
    } else {
        out
    }
}

pub(crate) fn nearest_strong_test_to_imitate(
    evidence: &TestGripEvidence,
) -> Option<&crate::analysis::test_grip_evidence::RelatedTestGrip> {
    evidence
        .related_tests
        .iter()
        .find(|test| test.oracle_strength == crate::domain::OracleStrength::Strong)
}

fn push_related_test_reference(
    out: &mut String,
    test: &crate::analysis::test_grip_evidence::RelatedTestGrip,
    reason: &str,
) {
    out.push('{');
    out.push_str(&format!(
        "\"name\": \"{}\", ",
        json_escape(test.test_name.as_str())
    ));
    out.push_str(&format!(
        "\"file\": \"{}\", ",
        json_escape(&display_path(&test.file))
    ));
    out.push_str(&format!("\"line\": {}, ", test.line));
    out.push_str(&format!(
        "\"oracle_kind\": \"{}\", ",
        test.oracle_kind.as_str()
    ));
    out.push_str(&format!(
        "\"oracle_strength\": \"{}\", ",
        test.oracle_strength.as_str()
    ));
    out.push_str(&format!(
        "\"relation_reason\": \"{}\", ",
        test.relation_reason.as_str()
    ));
    out.push_str(&format!(
        "\"relation_confidence\": \"{}\", ",
        test.relation_confidence.as_str()
    ));
    out.push_str(&format!("\"reason\": \"{}\"", json_escape(reason)));
    out.push('}');
}

pub(crate) fn candidate_values_for(
    entry: &ClassifiedSeam,
    missing: &[MissingRecord],
) -> Vec<CandidateValue> {
    let mut out: Vec<CandidateValue> = missing
        .iter()
        .map(|record| CandidateValue {
            value: record.value.clone(),
            reason: record.reason.clone(),
        })
        .collect();
    if out.is_empty() {
        out.push(candidate_value_from_required(
            entry.seam.required_discriminator(),
        ));
    }
    out
}

fn candidate_value_from_required(required: &RequiredDiscriminator) -> CandidateValue {
    match required {
        RequiredDiscriminator::BoundaryValue { description } => CandidateValue {
            value: format!("input that exercises {description}"),
            reason: "exercise the predicate boundary named by the seam".to_string(),
        },
        RequiredDiscriminator::ErrorVariant { variant } => CandidateValue {
            value: format!("input that triggers {variant}"),
            reason: "force the exact error variant rather than any error".to_string(),
        },
        RequiredDiscriminator::ReturnValue { description } => CandidateValue {
            value: format!("input that changes {description}"),
            reason: "observe the returned value sink named by the seam".to_string(),
        },
        RequiredDiscriminator::FieldValue { field } => CandidateValue {
            value: format!("input that sets {field}"),
            reason: "observe the constructed field value".to_string(),
        },
        RequiredDiscriminator::Effect { sink } => CandidateValue {
            value: format!("input that produces {sink}"),
            reason: "observe the side effect sink".to_string(),
        },
        RequiredDiscriminator::MatchArmTaken { arm } => CandidateValue {
            value: format!("input that selects {arm}"),
            reason: "exercise the changed match arm".to_string(),
        },
        RequiredDiscriminator::CallSite { target } => CandidateValue {
            value: format!("input that reaches call {target}"),
            reason: "observe the call site with a mock or spy".to_string(),
        },
    }
}

pub(crate) fn assertion_shape_for(
    kind: SeamKind,
    owner: &str,
    evidence: &TestGripEvidence,
) -> AssertionShape {
    let example = suggested_assertions_for(kind, owner, None, evidence)
        .into_iter()
        .next()
        .unwrap_or_else(|| "assert_eq!(actual, expected)".to_string());
    AssertionShape {
        kind: assertion_shape_kind_for(kind),
        example,
    }
}

pub(crate) fn assertion_shape_for_entry(entry: &ClassifiedSeam) -> AssertionShape {
    let example = suggested_assertions_for(
        entry.seam.kind(),
        entry.seam.owner(),
        Some(entry.seam.required_discriminator()),
        &entry.evidence,
    )
    .into_iter()
    .next()
    .unwrap_or_else(|| "assert_eq!(actual, expected)".to_string());
    AssertionShape {
        kind: assertion_shape_kind_for(entry.seam.kind()),
        example,
    }
}

fn assertion_shape_kind_for(kind: SeamKind) -> &'static str {
    match kind {
        SeamKind::PredicateBoundary => "exact_return_value",
        SeamKind::ErrorVariant => "exact_error_variant",
        SeamKind::ReturnValue => "exact_return_value",
        SeamKind::FieldConstruction => "field_equality",
        SeamKind::SideEffect => "side_effect_observer",
        SeamKind::MatchArm => "match_result",
        SeamKind::CallPresence => "call_expectation",
    }
}

fn patterns_to_imitate_for(evidence: &TestGripEvidence) -> Vec<ImitationPattern<'_>> {
    evidence
        .related_tests
        .iter()
        .filter(|test| {
            matches!(
                test.oracle_strength,
                crate::domain::OracleStrength::Strong | crate::domain::OracleStrength::Medium
            )
        })
        .take(3)
        .map(|test| ImitationPattern {
            test,
            reason: format!(
                "{} {} oracle with {} relation",
                test.oracle_strength.as_str(),
                test.oracle_kind.as_str(),
                test.relation_confidence.as_str()
            ),
        })
        .collect()
}

fn patterns_to_avoid_for(entry: &ClassifiedSeam) -> Vec<AvoidPattern> {
    let mut out: Vec<AvoidPattern> = entry
        .evidence
        .related_tests
        .iter()
        .filter(|test| {
            matches!(
                test.oracle_strength,
                crate::domain::OracleStrength::Weak
                    | crate::domain::OracleStrength::Smoke
                    | crate::domain::OracleStrength::None
                    | crate::domain::OracleStrength::Unknown
            )
        })
        .take(3)
        .map(|test| AvoidPattern {
            pattern: format!(
                "{} in {}",
                test.oracle_kind.as_str(),
                test.test_name.as_str()
            ),
            reason: "this related test reaches nearby behavior but lacks an exact discriminator"
                .to_string(),
        })
        .collect();
    if !entry.evidence.missing_discriminators.is_empty() {
        out.push(AvoidPattern {
            pattern: "adding another test with only already-observed values".to_string(),
            reason: "candidate values should include the missing discriminator".to_string(),
        });
    }
    if out.is_empty() && matches!(entry.class, SeamGripClass::Ungripped) {
        out.push(AvoidPattern {
            pattern: "copying a smoke-only test shape".to_string(),
            reason: "ungripped seams need a meaningful observer, not just execution".to_string(),
        });
    }
    out
}

fn packet_confidence_for(entry: &ClassifiedSeam) -> &'static str {
    if matches!(entry.class, SeamGripClass::Opaque) {
        return "unknown";
    }
    if entry.evidence.related_tests.iter().any(|test| {
        test.relation_confidence == crate::analysis::test_grip_evidence::RelationConfidence::High
    }) {
        return "high";
    }
    if entry.evidence.related_tests.iter().any(|test| {
        test.relation_confidence == crate::analysis::test_grip_evidence::RelationConfidence::Medium
    }) || !entry.evidence.missing_discriminators.is_empty()
    {
        return "medium";
    }
    "low"
}

/// Build the `missing_discriminators` array carried in the packet,
/// pairing analyzer-emitted hypotheses with a predicate-boundary
/// fallback when the seam expression names a clear boundary.
pub(crate) fn missing_discriminator_records_for(entry: &ClassifiedSeam) -> Vec<MissingRecord> {
    let mut out: Vec<MissingRecord> = entry
        .evidence
        .missing_discriminators
        .iter()
        .map(|m| MissingRecord {
            value: m.value.clone(),
            reason: m.reason.clone(),
        })
        .collect();
    // For predicate-boundary seams, surface the boundary expression
    // explicitly even when the analyzer hypothesis only names the RHS
    // token (or hasn't fired). This pins the most common ask.
    if matches!(entry.seam.kind(), SeamKind::PredicateBoundary)
        && let RequiredDiscriminator::BoundaryValue { description } =
            entry.seam.required_discriminator()
        && !out.iter().any(|r| r.value.contains(description.as_str()))
    {
        out.insert(0, MissingRecord {
            value: format!("input that hits the boundary: {description}"),
            reason: "predicate uses an equality-bearing operator; tests should exercise the boundary case"
                .to_string(),
        });
    }
    out
}

/// Suggest the oracle *shape* a test should use, derived from the
/// seam's kind and expected sink. The returned string is human-facing
/// guidance — the suggested-assertion list carries the literal
/// templates.
fn missing_oracle_shape_for(kind: SeamKind, sink: ExpectedSink) -> String {
    match kind {
        SeamKind::PredicateBoundary => {
            "exact returned value assertion at the equality boundary".to_string()
        }
        SeamKind::ErrorVariant => {
            "exact error-variant assertion (matches! / assert_matches!)".to_string()
        }
        SeamKind::ReturnValue => "exact value assertion on the returned value".to_string(),
        SeamKind::FieldConstruction => "field equality or whole-object assertion".to_string(),
        SeamKind::SideEffect => format!(
            "mock expectation, event/state observer, or persistence assertion ({})",
            sink.as_str()
        ),
        SeamKind::MatchArm => "exact value assertion on the match result".to_string(),
        SeamKind::CallPresence => "mock or spy assertion on the call site".to_string(),
    }
}

/// Best-effort assertion templates the agent can fill in. These are
/// guidance, not generated tests — placeholders are intentional.
fn suggested_assertions_for(
    kind: SeamKind,
    owner: &str,
    required: Option<&RequiredDiscriminator>,
    evidence: &TestGripEvidence,
) -> Vec<String> {
    let owner_short = owner.rsplit("::").next().unwrap_or(owner);
    match kind {
        SeamKind::PredicateBoundary => {
            let hint = predicate_boundary_assertion_hint(required, evidence);
            vec![format!(
                "assert_eq!({owner_short}(/* {hint} */), /* expected */)"
            )]
        }
        SeamKind::ErrorVariant => vec![format!(
            "assert!(matches!({owner_short}(/* trigger */), Err(/* exact variant */)))"
        )],
        SeamKind::ReturnValue => vec![format!(
            "assert_eq!({owner_short}(/* input */), /* expected */)"
        )],
        SeamKind::FieldConstruction => vec![format!(
            "let result = {owner_short}(/* input */); assert_eq!(result.field, /* expected */);"
        )],
        SeamKind::SideEffect => vec![format!(
            "// arrange a mock/observer; assert {owner_short}(...) produced the expected effect"
        )],
        SeamKind::MatchArm => vec![format!(
            "assert_eq!({owner_short}(/* input selecting this arm */), /* expected */)"
        )],
        SeamKind::CallPresence => vec![format!(
            "// assert that {owner_short} called the expected target"
        )],
    }
}

fn predicate_boundary_assertion_hint(
    required: Option<&RequiredDiscriminator>,
    evidence: &TestGripEvidence,
) -> String {
    if let Some(RequiredDiscriminator::BoundaryValue { description }) = required
        && !description.trim().is_empty()
    {
        return format!("boundary input where {}", description.trim());
    }
    if let Some(missing) = evidence.missing_discriminators.first()
        && !missing.value.trim().is_empty()
    {
        return format!("boundary input for {}", missing.value.trim());
    }
    "boundary input".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamKind};
    use crate::analysis::test_grip_evidence::{RelatedTestGrip, TestGripEvidence};
    use crate::domain::{
        Confidence, MissingDiscriminatorFact, OracleKind, OracleStrength, StageEvidence,
        StageState, ValueContext, ValueFact,
    };
    use std::path::PathBuf;

    fn stage(state: StageState) -> StageEvidence {
        StageEvidence::new(state, Confidence::Medium, "test stage")
    }

    fn boundary_seam() -> RepoSeam {
        RepoSeam::new(
            "src/pricing.rs",
            "pricing::discounted_total",
            SeamKind::PredicateBoundary,
            42,
            88,
            "amount >= discount_threshold",
            RequiredDiscriminator::BoundaryValue {
                description: "amount >= discount_threshold".to_string(),
            },
            ExpectedSink::ReturnValue,
        )
    }

    fn seam_with(
        owner: &str,
        kind: SeamKind,
        required: RequiredDiscriminator,
        sink: ExpectedSink,
    ) -> RepoSeam {
        RepoSeam::new(
            "src/service.rs",
            owner,
            kind,
            7,
            14,
            "changed expression",
            required,
            sink,
        )
    }

    fn related_test_with(
        name: &str,
        oracle_kind: OracleKind,
        oracle_strength: OracleStrength,
        relation_confidence: crate::analysis::test_grip_evidence::RelationConfidence,
    ) -> RelatedTestGrip {
        RelatedTestGrip {
            test_name: name.to_string(),
            file: PathBuf::from("tests/service.rs"),
            line: 21,
            oracle_kind,
            oracle_strength,
            evidence_summary: "related oracle evidence".to_string(),
            relation_reason: crate::analysis::test_grip_evidence::RelationReason::DirectOwnerCall,
            relation_confidence,
        }
    }

    fn classified_with(
        seam: RepoSeam,
        class: SeamGripClass,
        related_tests: Vec<RelatedTestGrip>,
    ) -> ClassifiedSeam {
        let seam_id = seam.id().clone();
        ClassifiedSeam {
            seam,
            evidence: TestGripEvidence {
                seam_id,
                related_tests,
                reach: stage(StageState::Yes),
                activate: stage(StageState::Yes),
                propagate: stage(StageState::Weak),
                observe: stage(StageState::Weak),
                discriminate: stage(StageState::No),
                observed_values: Vec::new(),
                missing_discriminators: Vec::new(),
            },
            class,
        }
    }

    fn weakly_gripped_classified() -> ClassifiedSeam {
        let seam = boundary_seam();
        let evidence = TestGripEvidence {
            seam_id: seam.id().clone(),
            related_tests: vec![RelatedTestGrip {
                test_name: "below_threshold_has_no_discount".to_string(),
                file: PathBuf::from("tests/pricing.rs"),
                line: 12,
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
                evidence_summary: "exact value assertion".to_string(),
                relation_reason:
                    crate::analysis::test_grip_evidence::RelationReason::DirectOwnerCall,
                relation_confidence: crate::analysis::test_grip_evidence::RelationConfidence::High,
            }],
            reach: stage(StageState::Yes),
            activate: stage(StageState::Yes),
            propagate: stage(StageState::Yes),
            observe: stage(StageState::Yes),
            discriminate: stage(StageState::Yes),
            observed_values: vec![ValueFact {
                line: 12,
                text: "discounted_total(50, 100)".to_string(),
                value: "50".to_string(),
                context: ValueContext::FunctionArgument,
            }],
            missing_discriminators: vec![MissingDiscriminatorFact {
                value: "discount_threshold (equality boundary)".to_string(),
                reason: "observed values do not include the equality-boundary case".to_string(),
                flow_sink: None,
            }],
        };
        ClassifiedSeam {
            seam,
            evidence,
            class: SeamGripClass::WeaklyGripped,
        }
    }

    fn ungripped_classified() -> ClassifiedSeam {
        let seam = boundary_seam();
        let evidence = TestGripEvidence {
            seam_id: seam.id().clone(),
            related_tests: Vec::new(),
            reach: stage(StageState::No),
            activate: stage(StageState::No),
            propagate: stage(StageState::No),
            observe: stage(StageState::No),
            discriminate: stage(StageState::No),
            observed_values: Vec::new(),
            missing_discriminators: Vec::new(),
        };
        ClassifiedSeam {
            seam,
            evidence,
            class: SeamGripClass::Ungripped,
        }
    }

    fn strongly_gripped_classified() -> ClassifiedSeam {
        let seam = boundary_seam();
        let evidence = TestGripEvidence {
            seam_id: seam.id().clone(),
            related_tests: Vec::new(),
            reach: stage(StageState::Yes),
            activate: stage(StageState::Yes),
            propagate: stage(StageState::Yes),
            observe: stage(StageState::Yes),
            discriminate: stage(StageState::Yes),
            observed_values: Vec::new(),
            missing_discriminators: Vec::new(),
        };
        ClassifiedSeam {
            seam,
            evidence,
            class: SeamGripClass::StronglyGripped,
        }
    }

    #[test]
    fn given_weakly_gripped_boundary_seam_when_packet_is_rendered_then_missing_boundary_value_is_present()
    -> Result<(), String> {
        let json = render_agent_seam_packets_json(&[weakly_gripped_classified()]);
        if !json.contains("\"current_grip\": \"weakly_gripped\"") {
            return Err(format!("missing current_grip in: {json}"));
        }
        if !json.contains("\"headline_eligible\": true") {
            return Err(format!("missing headline_eligible: {json}"));
        }
        if !json.contains("discount_threshold (equality boundary)") {
            return Err(format!(
                "expected boundary value in missing_discriminators: {json}"
            ));
        }
        if !json.contains("\"missing_oracle_shape\": \"exact returned value assertion") {
            return Err(format!("expected predicate-boundary oracle shape: {json}"));
        }
        if !json.contains("\"runtime_confirmation\":") {
            return Err(format!("missing runtime_confirmation: {json}"));
        }
        if !json.contains("\"static_evidence_boundary\": \"static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval.\"") {
            return Err(format!("missing static_evidence_boundary: {json}"));
        }
        Ok(())
    }

    #[test]
    fn missing_discriminators_carry_value_and_reason_objects() -> Result<(), String> {
        let json = render_agent_seam_packets_json(&[weakly_gripped_classified()]);
        if !json.contains(
            "{\"value\": \"discount_threshold (equality boundary)\", \"reason\": \"observed values do not include the equality-boundary case\"}",
        ) {
            return Err(format!(
                "expected structured missing_discriminator record in: {json}"
            ));
        }
        Ok(())
    }

    #[test]
    fn given_opaque_seam_when_packet_is_rendered_then_task_is_inspect_static_limitation()
    -> Result<(), String> {
        let mut entry = weakly_gripped_classified();
        entry.class = SeamGripClass::Opaque;
        let json = render_agent_seam_packets_json(&[entry]);
        if !json.contains("\"task\": \"inspect_static_limitation\"") {
            return Err(format!(
                "expected task=inspect_static_limitation for opaque seam: {json}"
            ));
        }
        if !json.contains("\"current_grip\": \"opaque\"") {
            return Err(format!("missing current_grip=opaque: {json}"));
        }
        if !json.contains("\"headline_eligible\": false") {
            return Err(format!(
                "expected headline_eligible=false for opaque: {json}"
            ));
        }
        Ok(())
    }

    #[test]
    fn predicate_boundary_fallback_emits_when_no_analyzer_hypothesis_fired() -> Result<(), String> {
        // Construct a weakly-gripped predicate seam with EMPTY
        // missing_discriminators (no analyzer hypothesis). The packet
        // should still surface the equality-boundary fallback so an
        // agent has something to act on.
        let mut entry = weakly_gripped_classified();
        entry.evidence.missing_discriminators = Vec::new();
        let json = render_agent_seam_packets_json(&[entry]);
        if !json
            .contains("\"value\": \"input that hits the boundary: amount >= discount_threshold\"")
        {
            return Err(format!(
                "expected predicate-boundary fallback record when analyzer hypothesis is empty: {json}"
            ));
        }
        if !json.contains("predicate uses an equality-bearing operator") {
            return Err(format!("expected fallback reason text: {json}"));
        }
        Ok(())
    }

    #[test]
    fn given_intentional_seam_when_packets_are_requested_then_no_packet_is_emitted()
    -> Result<(), String> {
        let mut entry = weakly_gripped_classified();
        entry.class = SeamGripClass::Intentional;
        let json = render_agent_seam_packets_json(&[entry]);
        if !json.contains("\"packets_total\": 0") {
            return Err(format!("intentional seam should produce no packet: {json}"));
        }
        Ok(())
    }

    #[test]
    fn given_ungripped_seam_when_packet_is_rendered_then_task_is_write_targeted_test()
    -> Result<(), String> {
        let json = render_agent_seam_packets_json(&[ungripped_classified()]);
        if !json.contains("\"task\": \"write_targeted_test\"") {
            return Err(format!("missing task field: {json}"));
        }
        if !json.contains("\"current_grip\": \"ungripped\"") {
            return Err(format!("missing current_grip ungripped: {json}"));
        }
        Ok(())
    }

    #[test]
    fn given_strongly_gripped_seam_when_packets_are_requested_then_no_actionable_packet_is_emitted()
    -> Result<(), String> {
        let json = render_agent_seam_packets_json(&[strongly_gripped_classified()]);
        if !json.contains("\"packets_total\": 0") {
            return Err(format!(
                "expected packets_total=0 for strongly-gripped input: {json}"
            ));
        }
        if !json.contains("\"packets\": []") {
            return Err(format!("expected empty packets array: {json}"));
        }
        Ok(())
    }

    #[test]
    fn given_related_tests_when_packet_is_rendered_then_oracle_kind_and_strength_are_present()
    -> Result<(), String> {
        let json = render_agent_seam_packets_json(&[weakly_gripped_classified()]);
        for needle in [
            "\"name\": \"below_threshold_has_no_discount\"",
            "\"oracle_kind\": \"exact_value\"",
            "\"oracle_strength\": \"strong\"",
        ] {
            if !json.contains(needle) {
                return Err(format!("missing {needle:?} in: {json}"));
            }
        }
        Ok(())
    }

    #[test]
    fn given_agent_packet_with_related_tests_when_rendered_then_relation_fields_are_emitted() {
        let json = render_agent_seam_packets_json(&[weakly_gripped_classified()]);
        assert!(
            json.contains("\"relation_reason\": \"direct_owner_call\""),
            "relation_reason missing: {json}"
        );
        assert!(
            json.contains("\"relation_confidence\": \"high\""),
            "relation_confidence missing: {json}"
        );
    }

    #[test]
    fn given_agent_packet_with_related_tests_when_rendered_then_highest_confidence_test_is_first()
    -> Result<(), String> {
        // Build an evidence record with two related tests where the
        // first by file/name order is low-confidence and the second is
        // high-confidence. The renderer iterates `related_tests` in
        // order, so the *order in the vec* is what determines which
        // appears first in the packet — confirm the Vec is already
        // ranked.
        use crate::analysis::test_grip_evidence::{RelationConfidence, RelationReason};
        let mut entry = weakly_gripped_classified();
        let high = RelatedTestGrip {
            test_name: "z_high_confidence".to_string(),
            file: PathBuf::from("tests/zeta.rs"),
            line: 1,
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
            evidence_summary: "exact value assertion".to_string(),
            relation_reason: RelationReason::DirectOwnerCall,
            relation_confidence: RelationConfidence::High,
        };
        let low = RelatedTestGrip {
            test_name: "a_low_confidence".to_string(),
            file: PathBuf::from("tests/alpha.rs"),
            line: 1,
            oracle_kind: OracleKind::Unknown,
            oracle_strength: OracleStrength::None,
            evidence_summary: "no oracle in test body".to_string(),
            relation_reason: RelationReason::FixtureOwnerAffinity,
            relation_confidence: RelationConfidence::Low,
        };
        // Caller provides a ranked vec — `evidence_for_seam` always
        // emits ranked, so this mirrors the production path.
        entry.evidence.related_tests = vec![high, low];

        let json = render_agent_seam_packets_json(&[entry]);
        let high_idx = json
            .find("\"name\": \"z_high_confidence\"")
            .ok_or_else(|| "high-confidence test missing".to_string())?;
        let low_idx = json
            .find("\"name\": \"a_low_confidence\"")
            .ok_or_else(|| "low-confidence test missing".to_string())?;
        if high_idx >= low_idx {
            return Err(format!(
                "high-confidence test must render before low-confidence; \
                 high@{high_idx} low@{low_idx}"
            ));
        }
        Ok(())
    }

    #[test]
    fn packet_v2_carries_recommended_test_candidate_values_assertion_shape_and_confidence()
    -> Result<(), String> {
        let json = render_agent_seam_packets_json(&[weakly_gripped_classified()]);
        for needle in [
            "\"recommended_test\": {\"name\": \"discounted_total_boundary_discriminator\"",
            "\"file\": \"tests/pricing.rs\"",
            "\"nearest_strong_test_to_imitate\": {\"name\": \"below_threshold_has_no_discount\"",
            "\"candidate_values\": [",
            "\"value\": \"input that hits the boundary: amount >= discount_threshold\"",
            "\"assertion_shape\": {\"kind\": \"exact_return_value\"",
            "\"example\": \"assert_eq!(discounted_total(/* boundary input where amount >= discount_threshold */), /* expected */)\"",
            "\"confidence\": \"high\"",
        ] {
            if !json.contains(needle) {
                return Err(format!("missing v2 field {needle:?} in: {json}"));
            }
        }
        Ok(())
    }

    #[test]
    fn packet_carries_shared_evidence_record_projection() -> Result<(), String> {
        let json = render_agent_seam_packets_json(&[weakly_gripped_classified()]);
        let value = serde_json::from_str::<serde_json::Value>(&json)
            .map_err(|err| format!("agent packet JSON should parse: {err}"))?;
        let packet = value
            .get("packets")
            .and_then(serde_json::Value::as_array)
            .and_then(|packets| packets.first())
            .ok_or_else(|| format!("missing packet in: {json}"))?;
        let packet_seam_id = packet
            .get("seam_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("missing packet seam_id in: {json}"))?;
        let record = packet
            .get("evidence_record")
            .ok_or_else(|| format!("missing packet evidence_record in: {json}"))?;
        assert_eq!(
            record
                .get("schema_version")
                .and_then(serde_json::Value::as_str),
            Some("0.1"),
            "expected evidence_record schema 0.1 in: {json}"
        );
        assert_eq!(
            record.get("seam_id").and_then(serde_json::Value::as_str),
            Some(packet_seam_id),
            "expected shared seam identity in: {json}"
        );
        assert_eq!(
            record
                .get("recommendation")
                .and_then(|recommendation| recommendation.get("assertion_shape"))
                .and_then(|shape| shape.get("kind"))
                .and_then(serde_json::Value::as_str),
            Some("exact_return_value"),
            "expected record assertion shape in: {json}"
        );
        assert!(
            json.contains(
                "\"recommended_test\": {\"name\": \"discounted_total_boundary_discriminator\"",
            ),
            "top-level packet fields should remain present: {json}"
        );
        Ok(())
    }

    #[test]
    fn gap_record_packet_carries_shared_repair_route_and_stop_conditions() -> Result<(), String> {
        let records = crate::output::gap_decision_ledger::parse_gap_records_json(
            r#"{"records":[{
              "gap_id":"gap:pr:pricing",
              "canonical_gap_id":"gap:rust:pricing",
              "kind":"MissingBoundaryAssertion",
              "language":"rust",
              "language_status":"stable",
              "scope":"pr_local",
              "evidence_class":"predicate_boundary",
              "gap_state":"actionable",
              "policy_state":"new",
              "repairability":"repairable",
              "anchor":{"file":"src/pricing.rs","line":42,"owner":"pricing::discount","dedupe_fingerprint":"gap:pricing"},
              "evidence_ids":["evidence:pricing-boundary"],
              "repair_route":{
                "route_kind":"AddBoundaryAssertion",
                "target_file":"tests/pricing.rs",
                "related_test":"discount_threshold_boundary",
                "assertion_shape":"assert_eq!(discount(100, 100), 90)",
                "changed_behavior":"amount == threshold",
                "stop_conditions":["Stop if this is baseline debt."]
              },
              "verification_commands":["cargo xtask fixtures boundary_gap"],
              "receipt_command":"ripr outcome --before target/ripr/workflow/before.json --after target/ripr/workflow/after.json --out target/ripr/receipts/gap-pr-pricing.targeted-test-outcome.json",
              "receipt":{"path":"target/ripr/receipts/gap-pr-pricing.targeted-test-outcome.json"},
              "projection_eligibility":{"agent_packet":{"eligible":true,"reason":"bounded repair route"}},
              "authority_boundary":"Gate decision remains pass/fail authority."
            }]}"#,
        )?;
        let record = records
            .first()
            .ok_or_else(|| "expected parsed gap record".to_string())?;
        let json = render_agent_gap_record_packet_json(
            "target/ripr/reports/gap-decision-ledger.json",
            record,
        )?;
        let value = serde_json::from_str::<serde_json::Value>(&json)
            .map_err(|err| format!("gap packet JSON should parse: {err}"))?;
        let packet = value
            .get("packets")
            .and_then(serde_json::Value::as_array)
            .and_then(|packets| packets.first())
            .ok_or_else(|| format!("missing gap packet in: {json}"))?;

        assert_eq!(
            value.get("source").and_then(serde_json::Value::as_str),
            Some("gap_decision_ledger")
        );
        assert_eq!(
            packet.get("gap_id").and_then(serde_json::Value::as_str),
            Some("gap:pr:pricing")
        );
        assert_eq!(
            packet.get("task").and_then(serde_json::Value::as_str),
            Some("write_targeted_test")
        );
        assert_eq!(
            packet
                .get("repair_route")
                .and_then(|route| route.get("route_kind"))
                .and_then(serde_json::Value::as_str),
            Some("AddBoundaryAssertion")
        );
        assert_eq!(
            packet
                .get("verify_command")
                .and_then(serde_json::Value::as_str),
            Some("cargo xtask fixtures boundary_gap")
        );
        assert_eq!(
            packet
                .get("repair_card")
                .and_then(|card| card.get("source_artifact"))
                .and_then(serde_json::Value::as_str),
            Some("target/ripr/reports/gap-decision-ledger.json")
        );
        assert!(
            packet.get("confidence").is_none(),
            "gap packets must not carry generic confidence: {json}"
        );
        assert_eq!(
            packet
                .get("static_evidence_boundary")
                .and_then(serde_json::Value::as_str),
            Some(STATIC_EVIDENCE_BOUNDARY),
            "gap packets should carry the typed static boundary: {json}"
        );
        assert!(
            json.contains("Stop if this is baseline debt."),
            "expected stop condition from GapRecord: {json}"
        );
        let copyable = packet
            .get("llm_guidance")
            .and_then(|guidance| guidance.get("copyable_packet"))
            .ok_or_else(|| format!("missing copyable repair packet in: {json}"))?;
        let copyable_markdown = copyable
            .get("markdown")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("missing copyable markdown in: {json}"))?;
        for heading in [
            "## Task",
            "## Context",
            "## Repair",
            "## Verification",
            "## Receipt",
            "## Stop Conditions",
            "## Do Not Do",
        ] {
            assert!(
                copyable_markdown.contains(heading),
                "copyable packet should carry {heading:?} in: {copyable_markdown}"
            );
        }
        assert!(
            copyable_markdown.contains(
                "Repair the `MissingBoundaryAssertion` gap `gap:pr:pricing` using the bounded `AddBoundaryAssertion` route."
            ),
            "copyable packet should name the task: {copyable_markdown}"
        );
        assert!(
            copyable_markdown
                .contains("- Related test or proof target: `discount_threshold_boundary`."),
            "copyable packet should name the related target: {copyable_markdown}"
        );
        assert!(
            copyable_markdown.contains("- Receipt command: `ripr outcome --before target/ripr/workflow/before.json --after target/ripr/workflow/after.json --out target/ripr/receipts/gap-pr-pricing.targeted-test-outcome.json`."),
            "copyable packet should name the receipt command: {copyable_markdown}"
        );
        assert!(
            copyable_markdown.contains(
                "- Receipt path: target/ripr/receipts/gap-pr-pricing.targeted-test-outcome.json."
            ),
            "copyable packet should name the receipt path: {copyable_markdown}"
        );
        assert!(
            copyable_markdown.contains("- Focused proof intent: Add a focused boundary assertion in `tests/pricing.rs`: `assert_eq!(discount(100, 100), 90)`."),
            "copyable packet should name the focused proof intent: {copyable_markdown}"
        );
        assert!(
            copyable_markdown
                .contains("- Missing discriminator: `assert_eq!(discount(100, 100), 90)`."),
            "copyable packet should name the missing discriminator: {copyable_markdown}"
        );
        assert!(
            copyable_markdown
                .contains("- Add or strengthen this check: `assert_eq!(discount(100, 100), 90)`."),
            "copyable packet should name the repair: {copyable_markdown}"
        );
        assert!(
            copyable_markdown.contains(
                "- Focused proof intent: add one focused assertion or output proof for `assert_eq!(discount(100, 100), 90)`."
            ),
            "copyable packet should name the focused proof intent: {copyable_markdown}"
        );
        assert!(
            copyable_markdown.contains("- cargo xtask fixtures boundary_gap"),
            "copyable packet should include verification: {copyable_markdown}"
        );
        assert!(
            copyable_markdown.contains("- Run `ripr outcome --before target/ripr/workflow/before.json --after target/ripr/workflow/after.json --out target/ripr/receipts/gap-pr-pricing.targeted-test-outcome.json` after verification."),
            "copyable packet should include receipt instructions: {copyable_markdown}"
        );
        assert!(
            copyable_markdown.contains("- Stop if this is baseline debt."),
            "copyable packet should include stop conditions: {copyable_markdown}"
        );
        assert!(
            copyable_markdown.contains(
                "- Do not edit production code unless the focused proof exposes a real product defect."
            ),
            "copyable packet should include do-not-do guidance: {copyable_markdown}"
        );
        Ok(())
    }

    #[test]
    fn gap_record_packet_rejects_ineligible_no_action_records() -> Result<(), String> {
        let records = crate::output::gap_decision_ledger::parse_gap_records_json(
            r#"{"records":[{
              "gap_id":"gap:already-observed",
              "kind":"NoActionAlreadyObserved",
              "language":"rust",
              "language_status":"stable",
              "scope":"pr_local",
              "policy_state":"resolved",
              "repairability":"no_action",
              "repair_route":{"route_kind":"NoAction"},
              "verification_commands":["cargo xtask fixtures"],
              "projection_eligibility":{"agent_packet":{"eligible":false,"reason":"already_observed"}}
            }]}"#,
        )?;
        let record = records
            .first()
            .ok_or_else(|| "expected parsed gap record".to_string())?;
        assert_eq!(
            render_agent_gap_record_packet_json("gap-ledger.json", record),
            Err("is not agent-packet eligible: already_observed".to_string())
        );
        Ok(())
    }

    #[test]
    fn packet_v2_normalizes_windows_related_test_paths() -> Result<(), String> {
        let mut entry = weakly_gripped_classified();
        entry.evidence.related_tests[0].file = PathBuf::from(r"tests\pricing.rs");

        let json = render_agent_seam_packets_json(&[entry]);
        assert!(
            json.contains("\"file\": \"tests/pricing.rs\""),
            "expected normalized related test file in packet JSON, got {json}"
        );
        assert!(
            !json.contains(r"tests\\pricing.rs"),
            "expected packet JSON to avoid host-specific separators, got {json}"
        );
        Ok(())
    }

    #[test]
    fn packet_v2_carries_patterns_to_imitate_and_avoid() -> Result<(), String> {
        let json = render_agent_seam_packets_json(&[weakly_gripped_classified()]);
        for needle in [
            "\"patterns_to_imitate\": [",
            "\"reason\": \"strong exact_value oracle with high relation\"",
            "\"patterns_to_avoid\": [",
            "\"pattern\": \"adding another test with only already-observed values\"",
            "\"reason\": \"candidate values should include the missing discriminator\"",
        ] {
            if !json.contains(needle) {
                return Err(format!("missing pattern field {needle:?} in: {json}"));
            }
        }
        Ok(())
    }

    #[test]
    fn targeted_test_brief_carries_plain_text_work_order() -> Result<(), String> {
        let brief = targeted_test_brief_for_classified_seam(&weakly_gripped_classified());
        for needle in [
            "Target seam:",
            "- src/pricing.rs:88",
            "- predicate_boundary",
            "- weakly_gripped",
            "- owner: pricing::discounted_total",
            "Why it matters:",
            "- Related test evidence: below_threshold_has_no_discount uses strong exact_value oracle.",
            "- Missing discriminator: input that hits the boundary: amount >= discount_threshold",
            "Add a targeted test:",
            "- Suggested file: tests/pricing.rs",
            "- Suggested name: discounted_total_boundary_discriminator",
            "- Candidate value: input that hits the boundary: amount >= discount_threshold",
            "- Assertion shape: assert_eq!(discounted_total(/* boundary input where amount >= discount_threshold */), /* expected */)",
            "Imitate:",
            "- below_threshold_has_no_discount (strong exact_value oracle with high relation)",
            "Avoid:",
            "- adding another test with only already-observed values",
        ] {
            if !brief.contains(needle) {
                return Err(format!("missing brief text {needle:?} in:\n{brief}"));
            }
        }
        Ok(())
    }

    #[test]
    fn targeted_test_brief_uses_inferred_file_when_no_related_test_exists() -> Result<(), String> {
        let brief = targeted_test_brief_for_classified_seam(&ungripped_classified());
        for needle in [
            "- No related test location is visible in saved-workspace analysis.",
            "- Suggested file: tests/pricing_tests.rs",
            "- Candidate value: input that hits the boundary: amount >= discount_threshold",
            "- copying a smoke-only test shape",
        ] {
            if !brief.contains(needle) {
                return Err(format!(
                    "missing inferred brief text {needle:?} in:\n{brief}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn packet_v2_recommends_inferred_test_file_when_no_related_test_exists() -> Result<(), String> {
        let json = render_agent_seam_packets_json(&[ungripped_classified()]);
        for needle in [
            "\"recommended_test\": {\"name\": \"discounted_total_boundary_discriminator\"",
            "\"file\": \"tests/pricing_tests.rs\"",
            "\"nearest_strong_test_to_imitate\": null",
            "\"confidence\": \"low\"",
        ] {
            if !json.contains(needle) {
                return Err(format!(
                    "missing inferred recommendation {needle:?} in: {json}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn packet_v2_carries_exact_error_variant_guidance_for_error_seams() -> Result<(), String> {
        let seam = seam_with(
            "auth::authenticate",
            SeamKind::ErrorVariant,
            RequiredDiscriminator::ErrorVariant {
                variant: "AuthError::RevokedToken".to_string(),
            },
            ExpectedSink::ErrorChannel,
        );
        let related = related_test_with(
            "empty_token_is_rejected",
            OracleKind::BroadError,
            OracleStrength::Weak,
            crate::analysis::test_grip_evidence::RelationConfidence::High,
        );
        let json = render_agent_seam_packets_json(&[classified_with(
            seam,
            SeamGripClass::WeaklyGripped,
            vec![related],
        )]);
        for needle in [
            "\"name\": \"authenticate_exact_error_variant\"",
            "\"candidate_values\": [",
            "\"value\": \"input that triggers AuthError::RevokedToken\"",
            "\"missing_oracle_shape\": \"exact error-variant assertion",
            "\"assertion_shape\": {\"kind\": \"exact_error_variant\"",
            "assert!(matches!(authenticate(/* trigger */), Err(/* exact variant */)))",
            "\"pattern\": \"broad_error in empty_token_is_rejected\"",
        ] {
            if !json.contains(needle) {
                return Err(format!(
                    "missing error-variant guidance {needle:?} in: {json}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn packet_v2_carries_side_effect_and_call_observer_guidance() -> Result<(), String> {
        let side_effect = seam_with(
            "billing::charge_customer",
            SeamKind::SideEffect,
            RequiredDiscriminator::Effect {
                sink: "payment event".to_string(),
            },
            ExpectedSink::SideEffect,
        );
        let call_presence = seam_with(
            "billing::sync_invoice",
            SeamKind::CallPresence,
            RequiredDiscriminator::CallSite {
                target: "repository.save".to_string(),
            },
            ExpectedSink::SideEffect,
        );
        let json = render_agent_seam_packets_json(&[
            classified_with(side_effect, SeamGripClass::Ungripped, Vec::new()),
            classified_with(call_presence, SeamGripClass::Ungripped, Vec::new()),
        ]);
        for needle in [
            "\"name\": \"charge_customer_side_effect_observer\"",
            "\"value\": \"input that produces payment event\"",
            "\"assertion_shape\": {\"kind\": \"side_effect_observer\"",
            "\"name\": \"sync_invoice_call_presence_observer\"",
            "\"value\": \"input that reaches call repository.save\"",
            "\"assertion_shape\": {\"kind\": \"call_expectation\"",
            "\"pattern\": \"copying a smoke-only test shape\"",
        ] {
            if !json.contains(needle) {
                return Err(format!(
                    "missing effect/call guidance {needle:?} in: {json}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn packet_v2_reports_medium_and_unknown_confidence_cases() -> Result<(), String> {
        let medium_related = related_test_with(
            "helper_test_observes_output",
            OracleKind::RelationalCheck,
            OracleStrength::Medium,
            crate::analysis::test_grip_evidence::RelationConfidence::Medium,
        );
        let mut opaque = weakly_gripped_classified();
        opaque.class = SeamGripClass::Opaque;
        let json = render_agent_seam_packets_json(&[
            classified_with(
                seam_with(
                    "math::score",
                    SeamKind::ReturnValue,
                    RequiredDiscriminator::ReturnValue {
                        description: "score".to_string(),
                    },
                    ExpectedSink::ReturnValue,
                ),
                SeamGripClass::WeaklyGripped,
                vec![medium_related],
            ),
            opaque,
        ]);
        for needle in [
            "\"confidence\": \"medium\"",
            "\"reason\": \"medium relational_check oracle with medium relation\"",
            "\"task\": \"inspect_static_limitation\"",
            "\"confidence\": \"unknown\"",
        ] {
            if !json.contains(needle) {
                return Err(format!("missing confidence case {needle:?} in: {json}"));
            }
        }
        Ok(())
    }

    #[test]
    fn suggested_assertion_helper_keeps_setup_assertions_but_omits_comment_guidance()
    -> Result<(), String> {
        let field = classified_with(
            seam_with(
                "pricing::build_quote",
                SeamKind::FieldConstruction,
                RequiredDiscriminator::FieldValue {
                    field: "quote.total".to_string(),
                },
                ExpectedSink::OutputField,
            ),
            SeamGripClass::WeaklyGripped,
            Vec::new(),
        );
        let opaque_field = classified_with(
            seam_with(
                "pricing::build_quote",
                SeamKind::FieldConstruction,
                RequiredDiscriminator::FieldValue {
                    field: "quote.total".to_string(),
                },
                ExpectedSink::OutputField,
            ),
            SeamGripClass::Opaque,
            Vec::new(),
        );
        let side_effect = classified_with(
            seam_with(
                "service::publish_event",
                SeamKind::SideEffect,
                RequiredDiscriminator::Effect {
                    sink: "event bus publish".to_string(),
                },
                ExpectedSink::SideEffect,
            ),
            SeamGripClass::WeaklyGripped,
            Vec::new(),
        );

        let Some(assertion) = suggested_assertion_for_classified_seam(&field) else {
            return Err("expected field construction assertion".to_string());
        };
        assert!(
            assertion.contains("assert_eq!(result.field"),
            "unexpected field assertion: {assertion}"
        );
        assert!(
            suggested_assertion_for_classified_seam(&opaque_field).is_some(),
            "opaque packet with concrete assertion guidance should expose the same assertion action"
        );
        assert!(suggested_assertion_for_classified_seam(&side_effect).is_none());
        Ok(())
    }

    #[test]
    fn schema_version_is_pinned_to_zero_three() {
        let json = render_agent_seam_packets_json(&[weakly_gripped_classified()]);
        assert!(
            json.contains("\"schema_version\": \"0.3\""),
            "expected schema_version 0.3: {json}"
        );
    }

    #[test]
    fn empty_input_emits_well_formed_json() {
        let json = render_agent_seam_packets_json(&[]);
        assert!(json.contains("\"packets_total\": 0"));
        assert!(json.contains("\"packets\": []"));
        assert!(json.contains("\"schema_version\": \"0.3\""));
    }

    #[test]
    fn suggested_assertion_for_predicate_boundary_uses_owner_and_missing_value() {
        let json = render_agent_seam_packets_json(&[weakly_gripped_classified()]);
        // owner short name is `discounted_total`.
        assert!(
            json.contains(
                "assert_eq!(discounted_total(/* boundary input where amount >= discount_threshold */"
            ),
            "expected templated assert_eq! suggestion: {json}"
        );
    }
}
