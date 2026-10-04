//! `ripr agent card` (#4667): the compact repair-card handoff.
//!
//! This is the CLI adapter layer only. The producer lives in
//! `crate::app::repair_card_handoff`; the card schema and its budget live in
//! `crate::domain` / `crate::repair_card_budget`. This module owns entry
//! resolution, policy omission checks, and the two output shapes: the
//! versioned `RepairCardV1` JSON document (`--json`) and the compact human
//! summary (default), which presents typed fields verbatim and never
//! re-derives or enhances them.
//!
//! #5007 (RIPR-SPEC-0202): the adapter also owns the typed-refusal envelope.
//! Every deliberate named refusal of the handoff — seam-not-found,
//! policy-omitted, witness-unavailable, identity-unnameable, and
//! budget-overflow — renders one versioned `agent_card_refusal` document on
//! stderr under `--json` and maps to the decision exit code 3, so an
//! orchestrator branches on the exit status and the typed `error.kind`
//! alone. Human prose stays the non-authority rendering of the kind; the
//! card's own stdout stays empty on a refusal, exactly like `agent verify`
//! (its stdout is the handoff artifact).

use crate::analysis;
use crate::app::agent_brief::AgentBriefPolicy;
use crate::app::repair_card_handoff::AgentCardError;
use crate::cli::CommandError;
use crate::cli::agent::AgentCardOptions;
use crate::cli::commands_context::ensure_command_root;
use crate::config::load_for_root;
use crate::domain::{AgentCardRefusalKind, RepairCardTarget, RepairCardV1};
use crate::output;

use super::agent::unknown_seam_id_hint;

/// Schema version of the `ripr agent card` typed-refusal document
/// (`kind: "agent_card_refusal"`). Deliberately distinct from the success
/// document's `repair_card.v1`, so a consumer dispatching on
/// `schema_version` never confuses a refusal with a card.
const AGENT_CARD_REFUSAL_SCHEMA_VERSION: &str = "0.1";

pub(super) fn run_agent_card(options: AgentCardOptions) -> Result<(), CommandError> {
    ensure_command_root(&options.root, "agent card")?;
    let card = match render_agent_card(&options) {
        Ok(card) => card,
        Err(AgentCardError::Refusal { kind, message }) => {
            // A deliberate named refusal maps to the decision exit code 3,
            // as the verify/verify-execute/repair siblings do. Under
            // `--json` the typed envelope is the machine answer; stdout
            // stays empty because it is the card-artifact stream. The
            // prose message still reaches stderr through `CommandError`,
            // after the envelope, as the human rendering.
            if options.json {
                match render_agent_card_refusal(&options, kind, &message) {
                    Ok(rendered) => eprint!("{rendered}"),
                    Err(render_error) => eprintln!("ripr: {render_error}"),
                }
            }
            return Err(CommandError::Decision(message));
        }
        Err(AgentCardError::Operational(message)) => {
            return Err(CommandError::Failure(message));
        }
    };
    if options.json {
        let rendered = output::json::render_pretty_with_newline(&card, "agent card")?;
        print!("{rendered}");
        return Ok(());
    }
    let packet_command =
        crate::app::repair_card_handoff::bound_packet_command(&options.root, &options.seam_id);
    for line in agent_card_prose_lines(&card, &packet_command) {
        println!("{line}");
    }
    Ok(())
}

/// Render the versioned typed-refusal document of an `agent card` refusal:
/// the schema version, the document kind, and one typed `error` block
/// naming the refusal kind, the seam id the call asked for, the exact
/// human prose (the non-authority rendering), and the typed remedy route
/// an orchestrator can run instead.
fn render_agent_card_refusal(
    options: &AgentCardOptions,
    kind: AgentCardRefusalKind,
    message: &str,
) -> Result<String, String> {
    let document = serde_json::json!({
        "schema_version": AGENT_CARD_REFUSAL_SCHEMA_VERSION,
        "kind": "agent_card_refusal",
        "error": {
            "kind": kind.as_str(),
            "seam_id": options.seam_id,
            "message": message,
            "remedy_route": refusal_remedy_route(&options.root, &options.seam_id, kind),
        }
    });
    serde_json::to_string_pretty(&document)
        .map(|rendered| format!("{rendered}\n"))
        .map_err(|error| format!("serialize agent card refusal document failed: {error}"))
}

/// The typed remedy route of one refusal kind: the opposite-remedy pairs the
/// issue calls out (re-list seams vs check policy config vs rerun analysis
/// vs retrieve the full packet) stay distinguishable without prose parsing.
/// The root binds the same way `unknown_seam_id_hint` binds it.
fn refusal_remedy_route(
    root: &std::path::Path,
    seam_id: &str,
    kind: AgentCardRefusalKind,
) -> String {
    let root_path = root;
    let root = crate::agent::loop_commands::shell_arg(&crate::agent::loop_commands::bound_root(
        &root.to_string_lossy(),
    ));
    match kind {
        AgentCardRefusalKind::SeamNotFound => {
            format!("ripr pilot --root {root}")
        }
        AgentCardRefusalKind::PolicyOmitted => {
            format!(
                "ripr agent brief --root {root} --seam-id {} --json",
                crate::agent::loop_commands::shell_arg(seam_id)
            )
        }
        AgentCardRefusalKind::WitnessUnavailable => {
            format!("ripr check --root {root} --json")
        }
        AgentCardRefusalKind::IdentityUnnameable | AgentCardRefusalKind::BudgetOverflow => {
            crate::app::repair_card_handoff::bound_packet_command(root_path, seam_id)
        }
    }
}

fn render_agent_card(options: &AgentCardOptions) -> Result<RepairCardV1, AgentCardError> {
    let config = load_for_root(&options.root).map_err(AgentCardError::operational)?;
    let (classified, _) =
        analysis::inventory_classified_seams_at_with_config(&options.root, &config)
            .map_err(AgentCardError::operational)?;
    let entry = classified
        .iter()
        .find(|entry| entry.seam.id().as_str() == options.seam_id)
        .ok_or_else(|| {
            // #5007: a typed refusal; the prose stays byte-identical to the
            // pre-typing rendering, and the kind is the contract.
            AgentCardError::refusal(
                AgentCardRefusalKind::SeamNotFound,
                format!(
                    "agent card seam_id {} was not found. {}",
                    options.seam_id,
                    unknown_seam_id_hint(&options.root, &options.seam_id)
                ),
            )
        })?;

    let policy = AgentBriefPolicy::from_config(&config);
    if let Some(reason) = policy.omission_reason_for_class(entry.class) {
        // Mirror `agent packet` (#4332): a policy-omitted seam is a dead end
        // without the listing route; name it like the not-found refusals do.
        // #5007: the policy decision is a typed refusal, never a transient
        // error — re-listing seams cannot fix it.
        return Err(AgentCardError::refusal(
            AgentCardRefusalKind::PolicyOmitted,
            format!(
                "agent card seam_id {} {reason}. {}",
                options.seam_id,
                unknown_seam_id_hint(&options.root, &options.seam_id)
            ),
        ));
    }

    crate::app::repair_card_handoff::repair_card_for_entry(entry, &options.root, &config)
}

/// The default human output: the card's typed fields, in card order,
/// presented verbatim. The renderer never re-derives values, never reorders
/// evidence, and routes every omitted family to its typed reference. The one
/// enhancement over a raw field dump is the explicit non-actionable reason
/// when the card carries no next action, because a silent absence there reads
/// as a bug, not a gate decision.
///
/// Crate-visible for the #4669 usability measurement, which counts the human
/// presentation separately from the JSON wire shape (RIPR-SPEC-0196).
/// The wire spelling of a typed value, presented verbatim in the human
/// summary (the enum remains the authority; the renderer never re-interprets
/// a value and never prints a `{:?}` debug spelling).
fn wire_name<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

pub(crate) fn agent_card_prose_lines(card: &RepairCardV1, packet_command: &str) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("Repair card {}", card.repair_card_id));
    lines.push(format!("  schema: {}", card.schema_version));
    let mut subject = format!("  seam: {}", card.subject.seam_id);
    if let Some(finding_id) = &card.subject.finding_id {
        subject.push_str(&format!(" (finding {finding_id})"));
    }
    lines.push(subject);
    lines.push(format!(
        "  snapshot: {} @ {} ({})",
        card.snapshot.workspace_identity,
        card.snapshot.repository_head,
        wire_name(&card.snapshot.currentness)
    ));
    lines.push(format!(
        "  instruction: {:?} (fix site: {}, suggested assertion: {})",
        card.instruction.state,
        yes_no(card.instruction.has_fix_site),
        yes_no(card.instruction.has_suggested_assertion)
    ));
    let missing = if card.readiness.missing_evidence.is_empty() {
        "none".to_string()
    } else {
        card.readiness.missing_evidence.join("; ")
    };
    lines.push(format!(
        "  readiness: repair_ready={} (missing evidence: {missing})",
        card.readiness.repair_ready
    ));
    lines.push(format!("  changed behavior: {}", card.changed_behavior));
    lines.push(format!(
        "  blocker: {}",
        card.exact_blocker.as_deref().unwrap_or("-")
    ));
    if let Some(goal) = &card.assertion_goal {
        lines.push(format!(
            "  assertion goal: {goal:?}: {}",
            card.assertion_goal_detail.as_deref().unwrap_or("-")
        ));
    }
    if let Some(target) = &card.selected_target {
        let rendered = match target {
            RepairCardTarget::Existing {
                symbol_id,
                file,
                line,
                test_kind,
                relation,
                workspace_identity: _,
            } => format!(
                "  selected target: existing {symbol_id} {file}:{line} ({test_kind:?}, relation {relation})"
            ),
            RepairCardTarget::Proposed {
                file,
                owner,
                proposal_kind,
            } => format!("  selected target: proposed {owner} in {file} ({proposal_kind:?})"),
        };
        lines.push(rendered);
    }
    match &card.next_action {
        Some(action) => {
            lines.push(format!("  next action: {} ({})", action.display, action.command_id));
        }
        None => lines.push(format!(
            "  next action: none (instruction {:?} with repair_ready={} exposes no bounded route; inspect the detail references below)",
            card.instruction.state, card.readiness.repair_ready
        )),
    }
    if !card.allowed_files.is_empty() || !card.forbidden_files.is_empty() {
        lines.push(format!(
            "  edit cage: allowed=[{}] forbidden=[{}]",
            card.allowed_files.join(", "),
            card.forbidden_files.join(", ")
        ));
    }
    lines.push(format!(
        "  done when: static movement {:?}, focused test {:?}, edit cage {:?}, mutation {:?}, currentness {}",
        card.done_when.static_movement,
        card.done_when.focused_test_execution,
        card.done_when.edit_cage,
        card.done_when.mutation_confirmation,
        wire_name(&card.done_when.currentness)
    ));
    for stop in &card.stop_conditions {
        lines.push(format!("  stop: {stop}"));
    }
    for reference in &card.detail_references {
        match &reference.route {
            Some(route) => lines.push(format!("  detail [{:?}]: {route}", reference.family)),
            None => lines.push(format!(
                "  detail [{:?}]: unavailable ({})",
                reference.family,
                reference
                    .unavailable_reason
                    .as_deref()
                    .unwrap_or("no reason recorded")
            )),
        }
    }
    if !card.limitations.is_empty() {
        lines.push(format!("  limitations: {}", card.limitations.join("; ")));
    }
    if let Some(attempt) = &card.attempt {
        lines.push(format!(
            "  attempt: {} ({})",
            attempt.attempt_id, attempt.state
        ));
    }
    // The closing packet command is the caller's: the CLI passes one bound
    // to the selected root so it runs when pasted from any directory (#3999).
    // The card's own typed next action and detail routes stay portable (#4666
    // portability contract) and are printed verbatim above.
    lines.push(format!("  full packet: {packet_command}"));
    // A path with an apostrophe needs doubled quotes in PowerShell, so the
    // bash line alone would split there. Same translator as every other
    // plain-text command surface.
    if let Some(form) = crate::output::markdown::powershell_text_variant(packet_command) {
        lines.push(format!("  (PowerShell) {form}"));
    }
    lines
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #5007: every deliberate named refusal kind renders the versioned
    /// envelope with the typed `error.kind` wire spelling, the asked-for seam
    /// id, the verbatim prose, and a typed remedy route; the document kind
    /// and schema version stay distinct from the success card.
    #[test]
    fn refusal_envelope_names_kind_seam_message_and_remedy() -> Result<(), String> {
        for (kind, expected_kind, expected_remedy) in [
            (
                AgentCardRefusalKind::SeamNotFound,
                "seam_not_found",
                "ripr pilot --root ",
            ),
            (
                AgentCardRefusalKind::PolicyOmitted,
                "policy_omitted",
                "ripr agent brief --root ",
            ),
            (
                AgentCardRefusalKind::WitnessUnavailable,
                "witness_unavailable",
                "ripr check --root ",
            ),
            (
                AgentCardRefusalKind::IdentityUnnameable,
                "identity_unnameable",
                "ripr agent packet --root ",
            ),
            (
                AgentCardRefusalKind::BudgetOverflow,
                "budget_overflow",
                "ripr agent packet --root ",
            ),
        ] {
            let options = AgentCardOptions {
                root: std::path::PathBuf::from("."),
                seam_id: "seam-a".to_string(),
                json: true,
            };
            let rendered = render_agent_card_refusal(&options, kind, "the exact human prose.")?;
            let document: serde_json::Value = serde_json::from_str(&rendered)
                .map_err(|error| format!("refusal document did not parse: {error}"))?;
            if document["schema_version"] != AGENT_CARD_REFUSAL_SCHEMA_VERSION {
                return Err("refusal names the wrong schema version".to_string());
            }
            if document["kind"] != "agent_card_refusal" {
                return Err("refusal names the wrong document kind".to_string());
            }
            if document["schema_version"] == crate::domain::REPAIR_CARD_SCHEMA_VERSION {
                return Err("the refusal must not share the success card version".to_string());
            }
            if document["error"]["kind"] != expected_kind {
                return Err(format!(
                    "{expected_kind}: error.kind mismatch: {}",
                    document["error"]["kind"]
                ));
            }
            if document["error"]["seam_id"] != "seam-a" {
                return Err("error must name the asked-for seam id".to_string());
            }
            if document["error"]["message"] != "the exact human prose." {
                return Err("error must carry the verbatim human prose".to_string());
            }
            let remedy = document["error"]["remedy_route"]
                .as_str()
                .ok_or_else(|| "error must name a remedy route".to_string())?;
            if !remedy.starts_with(expected_remedy) {
                return Err(format!("{expected_kind}: unexpected remedy route {remedy}"));
            }
            // #5007 review: the packet and brief remedies must stay directly
            // executable — they bind the invocation's root and seam id, never
            // the process working directory or a bare seam.
            if matches!(
                expected_kind,
                "policy_omitted" | "identity_unnameable" | "budget_overflow"
            ) && !remedy.contains("--seam-id seam-a")
            {
                return Err(format!(
                    "{expected_kind}: remedy must carry the asked-for seam id: {remedy}"
                ));
            }
        }
        Ok(())
    }

    /// The two opposite-remedy pairs the issue calls out must stay
    /// distinguishable by kind alone: not-found routes to re-listing seams,
    /// policy-omitted routes to the policy config, never the same remedy.
    #[test]
    fn opposite_remedy_pairs_stay_distinguishable() -> Result<(), String> {
        let root = std::path::Path::new(".");
        let not_found = refusal_remedy_route(root, "seam-a", AgentCardRefusalKind::SeamNotFound);
        let policy_omitted =
            refusal_remedy_route(root, "seam-a", AgentCardRefusalKind::PolicyOmitted);
        if not_found == policy_omitted {
            return Err("not-found and policy-omitted must carry different remedies".to_string());
        }
        if !not_found.starts_with("ripr pilot --root ") {
            return Err("not-found remedy must re-list seams".to_string());
        }
        if !policy_omitted.starts_with("ripr agent brief --root ") {
            return Err("policy-omitted remedy must name the policy config surface".to_string());
        }
        if !policy_omitted.contains("--seam-id seam-a") {
            return Err("policy-omitted remedy must carry the asked-for seam id".to_string());
        }
        let packet = refusal_remedy_route(root, "seam-a", AgentCardRefusalKind::IdentityUnnameable);
        if !packet.starts_with("ripr agent packet --root ") || !packet.contains("--seam-id seam-a")
        {
            return Err("packet remedy must bind the invocation root and seam id".to_string());
        }
        Ok(())
    }
}
