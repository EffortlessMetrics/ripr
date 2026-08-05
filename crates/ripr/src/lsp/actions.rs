use super::action_contract::{
    ActionDataInputs, ActionDisabledReason, ParsedActionData, action_data,
    disabled_reason_emittable, parse_validated_action_data,
};
use super::client_features::ClientFeatureProfile;
use super::gap_artifacts::{ValidatedGapArtifact, command_payload_is_safe, workspace_path_is_safe};
use super::state::AnalysisSnapshot;
use super::uri::file_uri_for_path;
use super::{
    COLLECT_CONTEXT_COMMAND, COLLECT_EVIDENCE_CONTEXT_COMMAND, COLLECT_RECEIPT_STATUS_COMMAND,
    COLLECT_REPAIR_PACKET_COMMAND, COLLECT_TOP_LIMITATION_COMMAND,
    COLLECT_WORKSPACE_STATUS_COMMAND, COPY_AFTER_SNAPSHOT_COMMAND, COPY_AGENT_BRIEF_COMMAND,
    COPY_AGENT_PACKET_COMMAND, COPY_AGENT_RECEIPT_COMMAND, COPY_AGENT_VERIFY_COMMAND,
    COPY_CONTEXT_COMMAND, COPY_SUGGESTED_ASSERTION_COMMAND, COPY_TARGETED_TEST_BRIEF_COMMAND,
    OPEN_RELATED_TEST_COMMAND, REFRESH_COMMAND,
};
use crate::agent::loop_commands;
use crate::analysis::ClassifiedSeam;
use crate::analysis::repair_route::{
    cross_language_test_target_unresolved, repair_packet_eligibility,
};
use crate::analysis::test_grip_evidence::{RelatedTestGrip, RelationConfidence};
use crate::domain::OracleStrength;
use crate::lsp::gap_artifacts::command_specs_for_projection;
use crate::output::agent_seam_packets::{
    suggested_assertion_for_classified_seam, targeted_test_brief_for_classified_seam,
};
use crate::output::evidence_record::CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY;
use serde_json::Value;
use std::path::{Path, PathBuf};
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionDisabled, CodeActionKind, CodeActionOrCommand, CodeActionParams,
    CodeActionResponse, Command, Diagnostic, LSPAny,
};

pub(super) fn code_action_response(
    params: &CodeActionParams,
    snapshot: Option<&AnalysisSnapshot>,
    client_features: &ClientFeatureProfile,
) -> CodeActionResponse {
    let mut actions = Vec::new();
    if let Some(context) = seam_action_context(params, snapshot) {
        push_seam_actions(&mut actions, params, context, client_features);
    }
    if let Some(context) = gap_action_context(params, snapshot) {
        push_gap_actions(&mut actions, params, context, client_features);
    } else if client_features.code_action_disabled
        && let Some((diagnostic, current)) = stale_gap_diagnostic(params, snapshot)
        && let Some(action) = disabled_action(
            INSPECT_GAP_PACKET_TITLE,
            "source.ripr.inspect",
            "copy_gap_repair_packet",
            COPY_CONTEXT_COMMAND,
            diagnostic,
            Some(current),
            ActionDisabledReason::StaleSnapshot,
        )
    {
        // #1892: the gap diagnostic is stale against the current snapshot —
        // a disabled-capable client sees the packet action inert instead of
        // absent; other clients keep the legacy omission.
        actions.push(action);
    }
    if let Some(diagnostic) = params
        .context
        .diagnostics
        .iter()
        .find(|d| is_ripr_diagnostic(d) && !is_seam_diagnostic(d) && !is_gap_diagnostic(d))
    {
        actions.push(copy_context_action(
            INSPECT_FINDING_CONTEXT_TITLE,
            INSPECT_FINDING_CONTEXT_COMMAND_TITLE,
            "copy_finding_context",
            copy_context_target(params, diagnostic),
            diagnostic,
            snapshot,
        ));
    }
    actions.push(CodeActionOrCommand::CodeAction(CodeAction {
        title: REFRESH_ANALYSIS_TITLE.to_string(),
        kind: Some(CodeActionKind::new("source.ripr.refresh")),
        command: Some(Command {
            title: REFRESH_ANALYSIS_TITLE.to_string(),
            command: REFRESH_COMMAND.to_string(),
            arguments: Some(Vec::new()),
        }),
        data: Some(action_data_payload(
            "source.ripr.refresh",
            "refresh_analysis",
            REFRESH_COMMAND,
            None,
            snapshot,
        )),
        ..CodeAction::default()
    }));
    // `CodeActionContext.only` filter (#1750, RIPR-SPEC-0129): LSP 3.17
    // hierarchical kind semantics — an action survives when a requested
    // kind equals or dot-segment-prefixes the action's kind. Absent `only`
    // leaves the response unfiltered; a kind-less action fails closed.
    if let Some(only) = &params.context.only {
        actions.retain(|action| kind_matches_only(action, only));
    }
    // Client-command policy (#1776 omit, #1892 disabled form,
    // RIPR-SPEC-0129): an action whose command executes client-side
    // (clipboard copies, related-test navigation) requires the negotiated
    // profile to have advertised that command. A client without
    // `CodeAction.disabled` support keeps the fail-closed omission; a
    // disabled-capable client instead receives the action inert — command
    // and edit stripped (a disabled action that still executes is the
    // cardinal-sin flip), kind retained for the `only` filter, and the
    // machine reason named in the data payload. Server-executed commands
    // run inside the server and stay unconditional.
    if client_features.code_action_disabled {
        for action in &mut actions {
            disable_missing_client_command_action(action, client_features);
        }
    } else {
        actions.retain(|action| command_allowed_for_client(action, client_features));
    }
    actions
}

/// LSP 3.17 `CodeActionContext.only` matching (#1750, RIPR-SPEC-0129): an
/// action survives when any requested kind equals the action's kind or is a
/// dot-segment prefix of it (`source` matches `source.ripr.inspect` and
/// `source.ripr.refresh`; `source.ripr.navigate` matches only that
/// subtree). An action with no kind fails closed when `only` is present.
fn kind_matches_only(action: &CodeActionOrCommand, only: &[CodeActionKind]) -> bool {
    let kind = match action {
        CodeActionOrCommand::CodeAction(action) => action.kind.as_ref(),
        CodeActionOrCommand::Command(_) => None,
    };
    let Some(kind) = kind else {
        return false;
    };
    only.iter().any(|requested| {
        kind.as_str() == requested.as_str()
            || kind
                .as_str()
                .strip_prefix(requested.as_str())
                .is_some_and(|rest| rest.starts_with('.'))
    })
}

/// Server-executed commands: the `executeCommandProvider` set from
/// `lsp/capabilities.rs`. They run inside the server through
/// `workspace/executeCommand`, so every client can run them regardless of
/// the negotiated `riprEditor` advertisement (#1776).
pub(super) const SERVER_EXECUTED_COMMANDS: [&str; 7] = [
    REFRESH_COMMAND,
    COLLECT_CONTEXT_COMMAND,
    COLLECT_EVIDENCE_CONTEXT_COMMAND,
    COLLECT_WORKSPACE_STATUS_COMMAND,
    COLLECT_REPAIR_PACKET_COMMAND,
    COLLECT_TOP_LIMITATION_COMMAND,
    COLLECT_RECEIPT_STATUS_COMMAND,
];

/// A code action survives the client-command filter when its command
/// executes inside the server, or when the negotiated profile advertised
/// the client-executed command (#1776, RIPR-SPEC-0129). An action with no
/// command executes nothing client-side and always survives; an unknown
/// client command fails closed to unsupported.
fn command_allowed_for_client(
    action: &CodeActionOrCommand,
    client_features: &ClientFeatureProfile,
) -> bool {
    let command = match action {
        CodeActionOrCommand::CodeAction(action) => action.command.as_ref(),
        CodeActionOrCommand::Command(command) => Some(command),
    };
    match command {
        None => true,
        Some(command) => command_permitted(&command.command, client_features),
    }
}

/// The shared client-command permission predicate (#1776, #1892): a command
/// is permitted when it executes inside the server or when the negotiated
/// profile advertised it. Both the omit filter and the disabled-form policy
/// delegate here so the two paths can never drift apart.
fn command_permitted(command: &str, client_features: &ClientFeatureProfile) -> bool {
    SERVER_EXECUTED_COMMANDS.contains(&command) || client_features.supports_client_command(command)
}

/// The capability the client must hold for a command (#1892): a
/// server-executed command runs inside the server (`"server"`); any other
/// command requires the client-command advertisement itself (#1776).
fn required_client_capability(command_id: &str) -> &str {
    if SERVER_EXECUTED_COMMANDS.contains(&command_id) {
        "server"
    } else {
        command_id
    }
}

/// The versioned `CodeAction.data` payload for an enabled action (#1892).
/// `action_name` is the action's stable snake_case machine identity — never
/// title text — and keeps the `action_id` distinct across constructors that
/// share one command id on one diagnostic.
fn action_data_payload(
    action_kind: &str,
    action_name: &'static str,
    command_id: &str,
    diagnostic: Option<&Diagnostic>,
    snapshot: Option<&AnalysisSnapshot>,
) -> LSPAny {
    action_data(&ActionDataInputs {
        action_kind,
        action_name,
        command_id,
        required_client_capability: required_client_capability(command_id),
        diagnostic,
        input_identity: snapshot.and_then(AnalysisSnapshot::input_identity_id),
        evidence_identity: snapshot.map(AnalysisSnapshot::evidence_identity),
        disabled_reason: None,
    })
}

/// Disabled form of the client-command filter (#1892, RIPR-SPEC-0129): an
/// action whose client-executed command was not advertised stays visible
/// but inert. The command/edit strip is mandatory — a disabled action that
/// still executes is the cardinal-sin flip — and `is_preferred` is never
/// true on a disabled action. Actions without a command (including actions
/// disabled at their suppression site) execute nothing and are left alone.
fn disable_missing_client_command_action(
    action: &mut CodeActionOrCommand,
    client_features: &ClientFeatureProfile,
) {
    let CodeActionOrCommand::CodeAction(action) = action else {
        return;
    };
    let Some(command) = action.command.as_ref() else {
        return;
    };
    if command_permitted(&command.command, client_features) {
        return;
    }
    disable_action_in_place(action, ActionDisabledReason::ClientCapabilityMissing);
}

/// Puts an already-emitted action into the inert disabled form with
/// `reason` (#1892, #1751): command and edit stripped (a disabled action
/// that still executes is the cardinal-sin flip), `is_preferred` cleared,
/// kind retained for the `CodeActionContext.only` filter, and the machine
/// reason named in the data payload. Both call-site reasons are statically
/// emittable; if the fail-closed emit guard ever rejects one, the action is
/// still stripped so it can never stay executable.
fn disable_action_in_place(action: &mut CodeAction, reason: ActionDisabledReason) {
    action.command = None;
    action.edit = None;
    action.is_preferred = None;
    if !disabled_reason_emittable(reason) {
        return;
    }
    action.disabled = Some(CodeActionDisabled {
        reason: reason.human_reason().to_string(),
    });
    if let Some(object) = action.data.as_mut().and_then(Value::as_object_mut) {
        object.insert(
            "disabled_reason".to_string(),
            Value::String(reason.as_str().to_string()),
        );
    }
}

/// `codeAction/resolve` revalidation (#1751, RIPR-SPEC-0129).
/// `textDocument/codeAction` already emits fully-resolved actions, so
/// resolve never strips or lazily attaches commands: it revalidates the
/// action against the current snapshot and the negotiated client profile
/// before the client executes it. A payload that is missing,
/// foreign-versioned, malformed, or fingerprint-inconsistent is a
/// fail-closed rejection (the `Err` message maps to `InvalidParams`,
/// mirroring the unsupported-command rejection at the backend). A
/// well-formed action whose snapshot, addressed artifact, or required
/// capability has lapsed returns in the inert disabled form naming the
/// emittable reason.
pub(super) fn resolve_action(
    mut action: CodeAction,
    snapshot: Option<&AnalysisSnapshot>,
    client_features: &ClientFeatureProfile,
) -> Result<CodeAction, String> {
    let data = action
        .data
        .clone()
        .ok_or_else(|| "code action is missing its ripr data payload".to_string())?;
    let command_id = resolve_command_id(&action, &data)?;
    let parsed = parse_validated_action_data(&data, &command_id)?;
    if parsed.addresses_artifact() {
        // Same health/root gate as `code_action`: without a current snapshot
        // the addressed evidence cannot be confirmed.
        let Some(snapshot) = snapshot else {
            disable_action_in_place(&mut action, ActionDisabledReason::StaleSnapshot);
            return Ok(action);
        };
        if let Some(payload_identity) = parsed.input_identity.as_deref()
            && Some(payload_identity) != snapshot.input_identity_id().as_deref()
        {
            disable_action_in_place(&mut action, ActionDisabledReason::StaleSnapshot);
            return Ok(action);
        }
        if parsed.seam_id.is_some()
            && snapshot.refresh.snapshot_id.is_some()
            && !AnalysisSnapshot::evidence_identities_match(
                parsed.evidence_identity.as_ref(),
                Some(&snapshot.evidence_identity()),
            )
        {
            // A cached action can outlive an otherwise identical full refresh.
            // The command target already carries this identity; keep resolve
            // fail-closed too so an old enabled action cannot execute.
            disable_action_in_place(&mut action, ActionDisabledReason::StaleSnapshot);
            return Ok(action);
        }
        if !addressed_artifact_still_present(&parsed, &data, snapshot) {
            disable_action_in_place(&mut action, ActionDisabledReason::StaleSnapshot);
            return Ok(action);
        }
    }
    if let Some(command) = action.command.as_ref() {
        // The payload's declared capability must agree with the attached
        // command — a mismatch is payload tampering, not a negotiation
        // difference.
        if parsed.required_client_capability != required_client_capability(&command.command) {
            return Err(
                "code action data required_client_capability disagrees with its command"
                    .to_string(),
            );
        }
        if !command_permitted(&command.command, client_features) {
            disable_action_in_place(&mut action, ActionDisabledReason::ClientCapabilityMissing);
            return Ok(action);
        }
        // Revalidation passed: the action resolves in its enabled form.
        action.disabled = None;
    }
    // An action that arrives without a command (already inert when emitted)
    // is returned as-is after revalidation: the constructors need the
    // original request context, so the command is not rebuilt here.
    Ok(action)
}

/// The command id the payload's `action_id` was fingerprinted with (#1751):
/// the attached command when present, otherwise — for an action the
/// omit-vs-disabled policy stripped — the payload's required capability,
/// which for a client-executed command is the command id itself. A stripped
/// server-executed command cannot be recovered and fails closed.
fn resolve_command_id(action: &CodeAction, data: &Value) -> Result<String, String> {
    if let Some(command) = action.command.as_ref() {
        return Ok(command.command.clone());
    }
    match data
        .get("required_client_capability")
        .and_then(Value::as_str)
    {
        Some(capability) if capability != "server" && !capability.trim().is_empty() => {
            Ok(capability.to_string())
        }
        _ => Err("code action carries no command and its payload cannot recover one".to_string()),
    }
}

/// The addressed-artifact freshness re-verification (#1751): the snapshot
/// must still carry the artifact the action addresses — the validated gap
/// artifact for a gap action (same predicate as the `code_action` emit
/// site), the classified seam for a seam action, the finding for a finding
/// action. The payload carries the addressed identities under the same keys
/// the diagnostic data uses, so the shared matcher consumes it directly.
fn addressed_artifact_still_present(
    parsed: &ParsedActionData,
    data: &Value,
    snapshot: &AnalysisSnapshot,
) -> bool {
    if parsed.addresses_gap() {
        return snapshot.gap_artifacts.iter().any(|artifact| {
            artifact.is_safe_projection_input()
                && artifact.is_actionable_gap()
                && artifact_matches_gap_diagnostic(artifact, data)
        });
    }
    if let Some(seam_id) = parsed.seam_id.as_deref() {
        return snapshot.classified_seam_by_id(seam_id).is_some();
    }
    if let Some(finding_id) = parsed.finding_id.as_deref() {
        return snapshot.finding_by_id(finding_id).is_some();
    }
    false
}

/// Builds an inert, diagnostic-addressing action for a suppression site
/// (#1892): the kind is retained (the `CodeActionContext.only` filter
/// fail-closes on kind-less actions), no command or edit is attached, and
/// the machine reason sits in the versioned data payload. Returns `None` —
/// the caller keeps the legacy omission — when the reason has no real
/// producer yet (fail-closed emit guard).
fn disabled_action(
    title: &str,
    action_kind: &'static str,
    action_name: &'static str,
    command_id: &str,
    diagnostic: &Diagnostic,
    snapshot: Option<&AnalysisSnapshot>,
    reason: ActionDisabledReason,
) -> Option<CodeActionOrCommand> {
    if !disabled_reason_emittable(reason) {
        return None;
    }
    Some(CodeActionOrCommand::CodeAction(CodeAction {
        title: title.to_string(),
        kind: Some(CodeActionKind::new(action_kind)),
        diagnostics: Some(vec![diagnostic.clone()]),
        disabled: Some(CodeActionDisabled {
            reason: reason.human_reason().to_string(),
        }),
        data: Some(action_data(&ActionDataInputs {
            action_kind,
            action_name,
            command_id,
            required_client_capability: required_client_capability(command_id),
            diagnostic: Some(diagnostic),
            input_identity: snapshot.and_then(AnalysisSnapshot::input_identity_id),
            evidence_identity: snapshot.map(AnalysisSnapshot::evidence_identity),
            disabled_reason: Some(reason),
        })),
        ..CodeAction::default()
    }))
}

/// A gap diagnostic the current snapshot no longer carries (#1892): any
/// action built from it would address superseded evidence, so a
/// disabled-capable client receives an inert action naming `stale_snapshot`
/// instead of silence. Returns the diagnostic and the snapshot only for the
/// staleness case — a missing artifact or missing snapshot stays omit-only.
fn stale_gap_diagnostic<'a>(
    params: &'a CodeActionParams,
    snapshot: Option<&'a AnalysisSnapshot>,
) -> Option<(&'a Diagnostic, &'a AnalysisSnapshot)> {
    let snapshot = snapshot?;
    let diagnostic = params
        .context
        .diagnostics
        .iter()
        .find(|d| is_ripr_diagnostic(d) && is_gap_diagnostic(d))?;
    let data = diagnostic.data.as_ref()?;
    if snapshot_has_current_gap_diagnostic(params, snapshot, data) {
        return None;
    }
    Some((diagnostic, snapshot))
}

struct SeamActionContext<'a> {
    diagnostic: &'a Diagnostic,
    seam: &'a ClassifiedSeam,
    snapshot: &'a AnalysisSnapshot,
}

struct GapActionContext<'a> {
    diagnostic: &'a Diagnostic,
    data: &'a Value,
    snapshot: &'a AnalysisSnapshot,
    artifact: &'a ValidatedGapArtifact,
}

fn seam_action_context<'a>(
    params: &'a CodeActionParams,
    snapshot: Option<&'a AnalysisSnapshot>,
) -> Option<SeamActionContext<'a>> {
    let snapshot = snapshot?;
    params
        .context
        .diagnostics
        .iter()
        .filter(|d| is_ripr_diagnostic(d) && is_seam_diagnostic(d))
        .find_map(|diagnostic| {
            snapshot
                .classified_seam_for_diagnostic(diagnostic)
                .filter(|_| snapshot_has_current_seam_diagnostic(params, snapshot, diagnostic))
                .map(|seam| SeamActionContext {
                    diagnostic,
                    seam,
                    snapshot,
                })
        })
}

fn snapshot_has_current_seam_diagnostic(
    params: &CodeActionParams,
    snapshot: &AnalysisSnapshot,
    cited: &Diagnostic,
) -> bool {
    let Some(cited_data) = cited.data.as_ref() else {
        return false;
    };
    snapshot
        .diagnostics_for_uri(&params.text_document.uri)
        .is_some_and(|diagnostics| {
            diagnostics.iter().any(|current| {
                is_ripr_diagnostic(current)
                    && is_seam_diagnostic(current)
                    && current.data.as_ref().is_some_and(|current_data| {
                        string_at(current_data, &["seam_id"]) == string_at(cited_data, &["seam_id"])
                            && evidence_identities_match(current_data, cited_data)
                    })
            })
        })
}

fn evidence_identities_match(left: &Value, right: &Value) -> bool {
    AnalysisSnapshot::evidence_identities_match(
        left.get("evidence_identity"),
        right.get("evidence_identity"),
    )
}

fn gap_action_context<'a>(
    params: &'a CodeActionParams,
    snapshot: Option<&'a AnalysisSnapshot>,
) -> Option<GapActionContext<'a>> {
    let snapshot = snapshot?;
    let diagnostic = params
        .context
        .diagnostics
        .iter()
        .find(|d| is_ripr_diagnostic(d) && is_gap_diagnostic(d))?;
    let data = diagnostic.data.as_ref()?;
    if !snapshot_has_current_gap_diagnostic(params, snapshot, data) {
        return None;
    }
    let artifact = snapshot.gap_artifacts.iter().find(|artifact| {
        artifact.is_safe_projection_input()
            && artifact.is_actionable_gap()
            && artifact_matches_gap_diagnostic(artifact, data)
    })?;
    Some(GapActionContext {
        diagnostic,
        data,
        snapshot,
        artifact,
    })
}

fn snapshot_has_current_gap_diagnostic(
    params: &CodeActionParams,
    snapshot: &AnalysisSnapshot,
    data: &Value,
) -> bool {
    snapshot
        .diagnostics_for_uri(&params.text_document.uri)
        .is_some_and(|diagnostics| {
            diagnostics.iter().any(|diagnostic| {
                diagnostic.data.as_ref().is_some_and(|current| {
                    is_gap_diagnostic(diagnostic) && gap_identities_overlap(current, data)
                })
            })
        })
}

fn artifact_matches_gap_diagnostic(artifact: &ValidatedGapArtifact, data: &Value) -> bool {
    let canonical_gap_id = string_at(data, &["canonical_gap_id"]);
    let seam_id = string_at(data, &["seam_id"]);
    let finding_id = string_at(data, &["finding_id"]);
    artifact.identities.iter().any(|identity| {
        canonical_gap_id.is_some_and(|value| identity.canonical_gap_id.as_deref() == Some(value))
            || seam_id.is_some_and(|value| identity.seam_id.as_deref() == Some(value))
            || finding_id.is_some_and(|value| identity.finding_id.as_deref() == Some(value))
    })
}

fn gap_identities_overlap(left: &Value, right: &Value) -> bool {
    for key in ["canonical_gap_id", "gap_id", "seam_id", "finding_id"] {
        if let (Some(left), Some(right)) = (string_at(left, &[key]), string_at(right, &[key]))
            && left == right
        {
            return true;
        }
    }
    false
}

fn push_seam_actions(
    actions: &mut CodeActionResponse,
    params: &CodeActionParams,
    context: SeamActionContext<'_>,
    client_features: &ClientFeatureProfile,
) {
    let suggested_assertion = suggested_assertion_for_classified_seam(context.seam);
    let related_test = best_related_test_for_editor(context.seam);
    actions.push(copy_context_action(
        INSPECT_SEAM_PACKET_TITLE,
        INSPECT_SEAM_PACKET_TITLE,
        "copy_seam_packet",
        copy_seam_packet_target(params, context.diagnostic, context.seam),
        context.diagnostic,
        Some(context.snapshot),
    ));
    if cross_language_test_target_unresolved(context.seam) {
        // #1892: the cross-language limitation suppresses the repair-packet
        // surface; a disabled-capable client still sees the brief action
        // inert with the preview/static limitation named.
        if client_features.code_action_disabled
            && let Some(action) = disabled_action(
                TARGETED_TEST_BRIEF_TITLE,
                "source.ripr.inspect",
                "copy_targeted_test_brief",
                COPY_TARGETED_TEST_BRIEF_COMMAND,
                context.diagnostic,
                Some(context.snapshot),
                ActionDisabledReason::PreviewOrStaticLimitation,
            )
        {
            actions.push(action);
        }
        return;
    }
    // The targeted-test brief is the editor's repair-packet surface, so the
    // flip routes through the single producer-owned authority
    // (`repair_packet_eligibility`, RIPR-SPEC-0087 §8) instead of hand-conjoining
    // readiness and cross-language predicates. The remaining guard is content
    // availability (a concrete assertion template or related test), not a
    // readiness predicate.
    if repair_packet_eligibility(context.seam).eligible()
        && (suggested_assertion.is_some() || related_test.is_some())
    {
        actions.push(copy_targeted_test_brief_action(
            context.seam,
            targeted_test_brief_for_classified_seam(context.seam),
            context.diagnostic,
            Some(context.snapshot),
        ));
    }
    actions.push(copy_agent_loop_command_action(
        AGENT_PACKET_COMMAND_TITLE,
        COPY_AGENT_PACKET_COMMAND,
        "copy_agent_packet_command",
        agent_loop_command_target(
            context.snapshot,
            context.diagnostic,
            context.seam,
            "agent_packet",
            loop_commands::EDITOR_AGENT_PACKET_ARTIFACT,
            loop_commands::agent_packet_command(
                COMMAND_ROOT,
                context.seam.seam.id().as_str(),
                loop_commands::EDITOR_AGENT_PACKET_ARTIFACT,
            ),
        ),
        context.diagnostic,
        Some(context.snapshot),
    ));
    actions.push(copy_agent_loop_command_action(
        AGENT_BRIEF_COMMAND_TITLE,
        COPY_AGENT_BRIEF_COMMAND,
        "copy_agent_brief_command",
        agent_loop_command_target(
            context.snapshot,
            context.diagnostic,
            context.seam,
            "agent_brief",
            loop_commands::EDITOR_AGENT_BRIEF_ARTIFACT,
            loop_commands::agent_brief_command(
                COMMAND_ROOT,
                context.seam.seam.id().as_str(),
                loop_commands::EDITOR_AGENT_BRIEF_ARTIFACT,
            ),
        ),
        context.diagnostic,
        Some(context.snapshot),
    ));
    actions.push(copy_agent_loop_command_action(
        AFTER_SNAPSHOT_COMMAND_TITLE,
        COPY_AFTER_SNAPSHOT_COMMAND,
        "copy_after_snapshot_command",
        agent_loop_command_target(
            context.snapshot,
            context.diagnostic,
            context.seam,
            "after_snapshot",
            loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
            loop_commands::check_repo_exposure_command_with_base(
                COMMAND_ROOT,
                context.snapshot.base.as_deref(),
                context.snapshot.mode.as_str(),
                loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
            ),
        ),
        context.diagnostic,
        Some(context.snapshot),
    ));
    actions.push(copy_agent_loop_command_action(
        AGENT_VERIFY_COMMAND_TITLE,
        COPY_AGENT_VERIFY_COMMAND,
        "copy_agent_verify_command",
        agent_loop_command_target(
            context.snapshot,
            context.diagnostic,
            context.seam,
            "agent_verify",
            loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT,
            loop_commands::agent_verify_command(
                COMMAND_ROOT,
                loop_commands::PILOT_BEFORE_SNAPSHOT_ARTIFACT,
                loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
                Some(loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT),
            ),
        ),
        context.diagnostic,
        Some(context.snapshot),
    ));
    actions.push(copy_agent_loop_command_action(
        AGENT_RECEIPT_COMMAND_TITLE,
        COPY_AGENT_RECEIPT_COMMAND,
        "copy_agent_receipt_command",
        agent_loop_command_target(
            context.snapshot,
            context.diagnostic,
            context.seam,
            "agent_receipt",
            loop_commands::EDITOR_AGENT_RECEIPT_ARTIFACT,
            loop_commands::agent_receipt_command(
                COMMAND_ROOT,
                loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT,
                context.seam.seam.id().as_str(),
                Some(loop_commands::EDITOR_AGENT_RECEIPT_ARTIFACT),
            ),
        ),
        context.diagnostic,
        Some(context.snapshot),
    ));
    if let Some(assertion) = suggested_assertion {
        actions.push(copy_suggested_assertion_action(
            context.seam,
            assertion,
            context.diagnostic,
            Some(context.snapshot),
        ));
    }
    if let Some(related) = related_test
        && let Some(target) = related_test_target(context.snapshot, related)
    {
        actions.push(open_related_test_action(
            target,
            context.diagnostic,
            Some(context.snapshot),
        ));
    }
}

fn push_gap_actions(
    actions: &mut CodeActionResponse,
    params: &CodeActionParams,
    context: GapActionContext<'_>,
    client_features: &ClientFeatureProfile,
) {
    if !gap_cross_language_target_unresolved(context.data) {
        if let Some(target) =
            first_repair_packet_target(context.snapshot, context.diagnostic, context.artifact)
        {
            actions.push(copy_context_action(
                COPY_FIRST_REPAIR_PACKET_TITLE,
                COPY_FIRST_REPAIR_PACKET_TITLE,
                "copy_first_repair_packet",
                target,
                context.diagnostic,
                Some(context.snapshot),
            ));
        }
        if let Some(target) = python_agent_packet_target(
            params,
            context.snapshot,
            context.diagnostic,
            context.artifact,
        ) {
            actions.push(copy_context_action(
                COPY_PYTHON_AGENT_PACKET_TITLE,
                COPY_PYTHON_AGENT_PACKET_TITLE,
                "copy_python_agent_packet",
                target,
                context.diagnostic,
                Some(context.snapshot),
            ));
        }
        if let Some(target) = gap_repair_packet_target(
            params,
            context.snapshot,
            context.diagnostic,
            context.artifact,
        ) {
            actions.push(copy_context_action(
                INSPECT_GAP_PACKET_TITLE,
                INSPECT_GAP_PACKET_COMMAND_TITLE,
                "copy_gap_repair_packet",
                target,
                context.diagnostic,
                Some(context.snapshot),
            ));
        }
        if let Some(target) = python_repair_card_target(context.snapshot, context.data) {
            actions.push(copy_python_repair_card_action(
                target,
                context.diagnostic,
                Some(context.snapshot),
            ));
        }
        if let Some(target) = python_pytest_skeleton_target(context.snapshot, context.data) {
            actions.push(copy_python_pytest_skeleton_action(
                target,
                context.diagnostic,
                Some(context.snapshot),
            ));
        }
        // §PR8 (RIPR-SPEC-0088): Copy TypeScript repair packet action when
        // the TS finding is actionable (repair_packet_ready: true in the
        // diagnostic's preview_actionability data).
        if let Some(target) =
            typescript_repair_packet_target(params, context.diagnostic, context.data)
        {
            actions.push(copy_context_action(
                COPY_TYPESCRIPT_REPAIR_PACKET_TITLE,
                COPY_TYPESCRIPT_REPAIR_PACKET_TITLE,
                "copy_typescript_repair_packet",
                target,
                context.diagnostic,
                Some(context.snapshot),
            ));
        }
        if let Some(target) = gap_related_test_target(context.snapshot, context.data) {
            actions.push(open_related_test_action(
                target,
                context.diagnostic,
                Some(context.snapshot),
            ));
        }
        let verify_command = first_safe_command_at(
            context.snapshot.root.as_path(),
            context.data,
            &["verification_commands"],
        );
        match &verify_command {
            Some(command) => actions.push(copy_agent_loop_command_action(
                AGENT_VERIFY_COMMAND_TITLE,
                COPY_AGENT_VERIFY_COMMAND,
                "copy_agent_verify_command",
                gap_command_target(params, context.diagnostic, "gap_verify", command),
                context.diagnostic,
                Some(context.snapshot),
            )),
            None => {
                // #1892: the gap record carries no safe verification route —
                // a disabled-capable client sees the verify handoff inert
                // instead of absent.
                if client_features.code_action_disabled
                    && let Some(action) = disabled_action(
                        AGENT_VERIFY_COMMAND_TITLE,
                        "source.ripr.inspect",
                        "copy_agent_verify_command",
                        COPY_AGENT_VERIFY_COMMAND,
                        context.diagnostic,
                        Some(context.snapshot),
                        ActionDisabledReason::VerificationRouteUnavailable,
                    )
                {
                    actions.push(action);
                }
            }
        }
        if verify_command.is_some() {
            match first_safe_receipt_command(context.snapshot.root.as_path(), context.data) {
                Some(command) => actions.push(copy_agent_loop_command_action(
                    AGENT_RECEIPT_COMMAND_TITLE,
                    COPY_AGENT_RECEIPT_COMMAND,
                    "copy_agent_receipt_command",
                    gap_command_target(params, context.diagnostic, "gap_receipt", &command),
                    context.diagnostic,
                    Some(context.snapshot),
                )),
                None => {
                    // #1892: the gap record carries a verify route but no
                    // safe receipt route — a disabled-capable client sees
                    // the receipt handoff inert instead of absent.
                    if client_features.code_action_disabled
                        && let Some(action) = disabled_action(
                            AGENT_RECEIPT_COMMAND_TITLE,
                            "source.ripr.inspect",
                            "copy_agent_receipt_command",
                            COPY_AGENT_RECEIPT_COMMAND,
                            context.diagnostic,
                            Some(context.snapshot),
                            ActionDisabledReason::ReceiptRouteUnavailable,
                        )
                    {
                        actions.push(action);
                    }
                }
            }
        }
    } else if client_features.code_action_disabled
        && let Some(action) = disabled_action(
            INSPECT_GAP_PACKET_TITLE,
            "source.ripr.inspect",
            "copy_gap_repair_packet",
            COPY_CONTEXT_COMMAND,
            context.diagnostic,
            Some(context.snapshot),
            ActionDisabledReason::PreviewOrStaticLimitation,
        )
    {
        // #1892: the producer-owned cross-language limitation suppresses the
        // whole repair-packet block; a disabled-capable client still sees
        // the packet action inert with the limitation named.
        actions.push(action);
    }
    if let Some(target) = static_limit_note_target(params, context.diagnostic) {
        actions.push(copy_context_action(
            COPY_STATIC_LIMIT_NOTE_TITLE,
            COPY_STATIC_LIMIT_NOTE_TITLE,
            "copy_static_limit_note",
            target,
            context.diagnostic,
            Some(context.snapshot),
        ));
    }
}

fn copy_context_action(
    title: &str,
    command_title: &str,
    action_name: &'static str,
    target: LSPAny,
    diagnostic: &Diagnostic,
    snapshot: Option<&AnalysisSnapshot>,
) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title: title.to_string(),
        kind: Some(CodeActionKind::new("source.ripr.inspect")),
        diagnostics: Some(vec![diagnostic.clone()]),
        command: Some(Command {
            title: command_title.to_string(),
            command: COPY_CONTEXT_COMMAND.to_string(),
            arguments: Some(vec![target]),
        }),
        data: Some(action_data_payload(
            "source.ripr.inspect",
            action_name,
            COPY_CONTEXT_COMMAND,
            Some(diagnostic),
            snapshot,
        )),
        ..CodeAction::default()
    })
}

const COMMAND_ROOT: &str = ".";

const INSPECT_GAP_PACKET_TITLE: &str = "Inspect gap: copy repair packet";
const INSPECT_GAP_PACKET_COMMAND_TITLE: &str = "Inspect gap: copy context";
const INSPECT_FINDING_CONTEXT_TITLE: &str = "Inspect finding: copy context packet";
const INSPECT_FINDING_CONTEXT_COMMAND_TITLE: &str = "Inspect finding: copy context";
const INSPECT_SEAM_PACKET_TITLE: &str = "Inspect Test Gap - Copy Context";
const TARGETED_TEST_BRIEF_TITLE: &str = "Write targeted test: copy brief";
const SUGGESTED_ASSERTION_TITLE: &str = "Write targeted test: copy suggested assertion";
const OPEN_RELATED_TEST_TITLE: &str = "Write targeted test: open best related test";
const AGENT_PACKET_COMMAND_TITLE: &str = "Agent handoff: copy packet command";
const AGENT_BRIEF_COMMAND_TITLE: &str = "Agent handoff: copy brief command";
const AFTER_SNAPSHOT_COMMAND_TITLE: &str = "Verify after test: copy after-snapshot command";
const AGENT_VERIFY_COMMAND_TITLE: &str = "Verify after test: copy verify command";
const AGENT_RECEIPT_COMMAND_TITLE: &str = "Review result: copy receipt command";
const COPY_STATIC_LIMIT_NOTE_TITLE: &str = "Inspect gap: copy static-limit note";
const COPY_FIRST_REPAIR_PACKET_TITLE: &str = "Copy first repair packet";
const COPY_PYTHON_AGENT_PACKET_TITLE: &str = "Agent handoff: copy Python packet";
const COPY_PYTHON_REPAIR_CARD_TITLE: &str = "Copy Python repair card";
const COPY_PYTHON_PYTEST_SKELETON_TITLE: &str = "Write Python test: copy pytest skeleton";
const COPY_TYPESCRIPT_REPAIR_PACKET_TITLE: &str = "Copy TypeScript repair packet (advisory)";
const REFRESH_ANALYSIS_TITLE: &str = "Refresh Analysis - Saved Workspace Check";

fn copy_agent_loop_command_action(
    title: &str,
    command: &str,
    action_name: &'static str,
    target: LSPAny,
    diagnostic: &Diagnostic,
    snapshot: Option<&AnalysisSnapshot>,
) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title: title.to_string(),
        kind: Some(CodeActionKind::new("source.ripr.inspect")),
        diagnostics: Some(vec![diagnostic.clone()]),
        command: Some(Command {
            title: title.to_string(),
            command: command.to_string(),
            arguments: Some(vec![target]),
        }),
        data: Some(action_data_payload(
            "source.ripr.inspect",
            action_name,
            command,
            Some(diagnostic),
            snapshot,
        )),
        ..CodeAction::default()
    })
}

fn agent_loop_command_target(
    snapshot: &AnalysisSnapshot,
    diagnostic: &Diagnostic,
    seam: &ClassifiedSeam,
    label: &str,
    target_artifact: &str,
    command: String,
) -> LSPAny {
    serde_json::json!({
        "label": label,
        "command": command,
        "root": COMMAND_ROOT,
        "base": snapshot.base.as_deref(),
        "mode": snapshot.mode.as_str(),
        "seam_id": seam.seam.id().as_str(),
        "evidence_identity": snapshot.evidence_identity(),
        "seam_kind": seam.seam.kind().as_str(),
        "seam_file": seam.seam.file().to_string_lossy(),
        "owner": seam.seam.owner(),
        "line": seam.seam.display_line(),
        "severity": diagnostic.severity.and_then(diagnostic_severity_label),
        "diagnostic_range": {
            "start": {
                "line": diagnostic.range.start.line,
                "character": diagnostic.range.start.character,
            },
            "end": {
                "line": diagnostic.range.end.line,
                "character": diagnostic.range.end.character,
            },
        },
        "target_artifact": target_artifact,
        "before_snapshot": loop_commands::PILOT_BEFORE_SNAPSHOT_ARTIFACT,
        "after_snapshot": loop_commands::PILOT_AFTER_SNAPSHOT_ARTIFACT,
        "agent_packet_json": loop_commands::EDITOR_AGENT_PACKET_ARTIFACT,
        "agent_brief_json": loop_commands::EDITOR_AGENT_BRIEF_ARTIFACT,
        "agent_verify_json": loop_commands::EDITOR_AGENT_VERIFY_ARTIFACT,
        "agent_receipt_json": loop_commands::EDITOR_AGENT_RECEIPT_ARTIFACT,
    })
}

fn diagnostic_severity_label(
    severity: tower_lsp_server::ls_types::DiagnosticSeverity,
) -> Option<&'static str> {
    match severity {
        tower_lsp_server::ls_types::DiagnosticSeverity::ERROR => Some("error"),
        tower_lsp_server::ls_types::DiagnosticSeverity::WARNING => Some("warning"),
        tower_lsp_server::ls_types::DiagnosticSeverity::INFORMATION => Some("information"),
        tower_lsp_server::ls_types::DiagnosticSeverity::HINT => Some("hint"),
        _ => None,
    }
}

fn gap_repair_packet_target(
    params: &CodeActionParams,
    snapshot: &AnalysisSnapshot,
    diagnostic: &Diagnostic,
    artifact: &ValidatedGapArtifact,
) -> Option<LSPAny> {
    let data = diagnostic.data.as_ref()?;
    let repair_route = data.get("repair_route")?;
    repair_route.get("route_kind").and_then(non_empty_string)?;
    for key in ["target_file", "related_test"] {
        if let Some(path) = repair_route.get(key).and_then(non_empty_string)
            && !workspace_path_is_safe(snapshot.root.as_path(), path)
        {
            return None;
        }
    }
    let mut target = copy_context_target(params, diagnostic);
    let object = target.as_object_mut()?;
    object.insert(
        "label".to_string(),
        Value::String("gap_repair_packet".to_string()),
    );
    copy_optional_string(object, data, "language");
    copy_optional_string(object, data, "language_status");
    copy_optional_string(object, data, "gap_state");
    copy_optional_string(object, data, "policy_state");
    copy_optional_string(object, data, "repairability");
    copy_optional_string(object, data, "authority_boundary");
    copy_optional_value(object, data, "repair_route");
    let verification_commands =
        safe_commands_at(snapshot.root.as_path(), data, &["verification_commands"]);
    if !verification_commands.is_empty() {
        object.insert(
            "verification_commands".to_string(),
            serde_json::json!(verification_commands),
        );
    }
    let regeneration_commands =
        safe_commands_at(snapshot.root.as_path(), data, &["regeneration_commands"]);
    if !regeneration_commands.is_empty() {
        object.insert(
            "regeneration_commands".to_string(),
            serde_json::json!(regeneration_commands),
        );
    }
    if let Some(command) = first_safe_receipt_command(snapshot.root.as_path(), data) {
        object.insert("receipt_command".to_string(), Value::String(command));
    }
    if let Some(command_specs) = command_specs_for_projection(artifact) {
        object.insert("command_specs".to_string(), command_specs);
    }
    copy_optional_value(object, data, "receipt");
    if let Some(note) = static_limit_note(data) {
        object.insert("static_limit_note".to_string(), Value::String(note));
    }
    object.insert(
        "limits_note".to_string(),
        Value::String(
            "Static evidence only; no source edits, generated tests, provider calls, or runtime mutation execution."
                .to_string(),
        ),
    );
    Some(target)
}

fn python_agent_packet_target(
    params: &CodeActionParams,
    snapshot: &AnalysisSnapshot,
    diagnostic: &Diagnostic,
    artifact: &ValidatedGapArtifact,
) -> Option<LSPAny> {
    let data = diagnostic.data.as_ref()?;
    if string_at(data, &["source"]) != Some("gap_decision_ledger")
        || string_at(data, &["language"]) != Some("python")
        || string_at(data, &["gap_state"]) != Some("actionable")
        || string_at(data, &["repairability"]) != Some("repairable")
    {
        return None;
    }
    string_at(data, &["gap_id"])?;
    let gap_ledger = string_at(data, &["gap_ledger"])?;
    if !workspace_path_is_safe(snapshot.root.as_path(), gap_ledger) {
        return None;
    }
    first_safe_command_at(snapshot.root.as_path(), data, &["verification_commands"])?;
    first_safe_receipt_command(snapshot.root.as_path(), data)?;
    let mut target = gap_repair_packet_target(params, snapshot, diagnostic, artifact)?;
    let object = target.as_object_mut()?;
    object.insert(
        "label".to_string(),
        Value::String("python_agent_packet".to_string()),
    );
    object.insert(
        "freshness".to_string(),
        Value::String("validated_current_gap_record".to_string()),
    );
    object.insert(
        "packet_source".to_string(),
        Value::String("gap_decision_ledger".to_string()),
    );
    object.insert(
        "packet_kind".to_string(),
        Value::String("agent_gap_record_packet".to_string()),
    );
    Some(target)
}

fn first_repair_packet_target(
    snapshot: &AnalysisSnapshot,
    diagnostic: &Diagnostic,
    artifact: &ValidatedGapArtifact,
) -> Option<LSPAny> {
    let data = diagnostic.data.as_ref()?;
    let gap_identity = first_gap_identity(data)?;
    let repair_route = data.get("repair_route")?;
    repair_route.get("route_kind").and_then(non_empty_string)?;
    for key in ["target_file", "related_test"] {
        if let Some(path) = repair_route.get(key).and_then(non_empty_string)
            && !workspace_path_is_safe(snapshot.root.as_path(), path)
        {
            return None;
        }
    }
    let verify_command =
        first_safe_command_at(snapshot.root.as_path(), data, &["verification_commands"])?;
    let receipt_command = first_safe_receipt_command(snapshot.root.as_path(), data)?;
    let packet = first_repair_packet_text(data, repair_route, &verify_command, &receipt_command)?;
    let mut target = serde_json::Map::new();
    target.insert(
        "label".to_string(),
        Value::String("first_repair_packet".to_string()),
    );
    target.insert("packet".to_string(), Value::String(packet));
    target.insert(
        "gap_identity".to_string(),
        Value::String(gap_identity.to_string()),
    );
    copy_optional_string(&mut target, data, "gap_id");
    copy_optional_string(&mut target, data, "canonical_gap_id");
    copy_optional_string(&mut target, data, "seam_id");
    copy_optional_string(&mut target, data, "finding_id");
    copy_optional_string(&mut target, data, "language");
    copy_optional_string(&mut target, data, "language_status");
    copy_optional_string(&mut target, data, "gap_state");
    copy_optional_value(&mut target, data, "repair_route");
    target.insert("verify_command".to_string(), Value::String(verify_command));
    target.insert(
        "receipt_command".to_string(),
        Value::String(receipt_command),
    );
    if let Some(command_specs) = command_specs_for_projection(artifact) {
        target.insert("command_specs".to_string(), command_specs);
    }
    Some(Value::Object(target))
}

fn first_gap_identity(data: &Value) -> Option<&str> {
    ["canonical_gap_id", "gap_id", "seam_id", "finding_id"]
        .iter()
        .find_map(|key| string_at(data, &[*key]))
}

fn first_repair_packet_text(
    data: &Value,
    repair_route: &Value,
    verify_command: &str,
    receipt_command: &str,
) -> Option<String> {
    let gap_identity = first_gap_identity(data)?;
    let route_kind = repair_route.get("route_kind").and_then(non_empty_string)?;
    let mut lines = vec![
        "RIPR first repair packet".to_string(),
        String::new(),
        format!("Gap identity: {gap_identity}"),
    ];
    if let Some(language) = string_at(data, &["language"]) {
        lines.push(format!("Language: {language}"));
    }
    if let Some(status) = string_at(data, &["language_status"]) {
        lines.push(format!("Language status: {status}"));
    }
    if let Some(state) = string_at(data, &["gap_state"]) {
        lines.push(format!("Gap state: {state}"));
    }
    if let Some(note) = static_limit_note(data) {
        lines.push(String::new());
        lines.push(note);
    }
    lines.push(String::new());
    lines.push("Suggested action:".to_string());
    lines.push(format!("- Route: {route_kind}"));
    if let Some(changed_behavior) = repair_route
        .get("changed_behavior")
        .and_then(non_empty_string)
    {
        lines.push(format!("- Changed behavior: {changed_behavior}"));
    }
    if let Some(missing_discriminator) = missing_discriminator_for_packet(data, repair_route) {
        lines.push(format!("- Missing discriminator: {missing_discriminator}"));
    }
    if let Some(focused_proof_intent) = focused_proof_intent_for_packet(repair_route) {
        lines.push(format!("- Focused proof intent: {focused_proof_intent}"));
    }
    if let Some(assertion_shape) = repair_route
        .get("assertion_shape")
        .and_then(non_empty_string)
    {
        lines.push(format!(
            "- Add or strengthen one focused assertion: {assertion_shape}"
        ));
    }
    if let Some(related_test) = repair_route.get("related_test").and_then(non_empty_string) {
        lines.push(format!("- Related test: {related_test}"));
    } else if let Some(target_file) = repair_route.get("target_file").and_then(non_empty_string) {
        lines.push(format!("- Repair target: {target_file}"));
    }
    if let Some(items) = repair_route
        .get("stop_conditions")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
    {
        lines.push("- Stop conditions:".to_string());
        for item in items {
            if let Some(text) = item.as_str().map(str::trim).filter(|text| !text.is_empty()) {
                lines.push(format!("  - {text}"));
            }
        }
    }
    lines.push(String::new());
    let artifacts = repair_packet_artifacts(data);
    if !artifacts.is_empty() {
        lines.push("Artifacts:".to_string());
        for artifact in artifacts {
            lines.push(format!("- {artifact}"));
        }
        lines.push(String::new());
    }
    lines.push("Verify command:".to_string());
    lines.push(verify_command.to_string());
    lines.push(String::new());
    lines.push("Receipt command:".to_string());
    lines.push(receipt_command.to_string());
    lines.push(String::new());
    lines.push("Limits and non-claims:".to_string());
    lines.push("- Static editor evidence only.".to_string());
    lines.push("- Advisory by default; no gate eligibility or runtime adequacy claim.".to_string());
    lines.push("- Do not edit production code unless the packet explicitly scopes it.".to_string());
    lines.push(
        "- Do not generate tests, call providers, or run mutation execution from the editor."
            .to_string(),
    );
    Some(lines.join("\n"))
}

fn python_repair_card_target(snapshot: &AnalysisSnapshot, data: &Value) -> Option<LSPAny> {
    if string_at(data, &["language"]) != Some("python") {
        return None;
    }
    let route = data.get("repair_route")?;
    route.get("route_kind").and_then(non_empty_string)?;
    let target_file = route.get("target_file").and_then(non_empty_string)?;
    if !workspace_path_is_safe(snapshot.root.as_path(), target_file)
        || !path_matches_diagnostic_language(data, target_file)
    {
        return None;
    }
    if let Some(path) = route.get("related_test").and_then(non_empty_string)
        && !workspace_path_is_safe(snapshot.root.as_path(), path)
    {
        return None;
    }
    let verify_command =
        first_safe_command_at(snapshot.root.as_path(), data, &["verification_commands"])?;
    let receipt_command = first_safe_receipt_command(snapshot.root.as_path(), data);
    let card = python_repair_card_text(data, route, &verify_command, receipt_command.as_deref())?;
    let mut target = serde_json::Map::new();
    target.insert(
        "label".to_string(),
        Value::String("python_repair_card".to_string()),
    );
    target.insert("brief".to_string(), Value::String(card));
    target.insert(
        "freshness".to_string(),
        Value::String("validated_current_gap_record".to_string()),
    );
    copy_optional_string(&mut target, data, "gap_id");
    copy_optional_string(&mut target, data, "canonical_gap_id");
    copy_optional_string(&mut target, data, "language");
    copy_optional_string(&mut target, data, "language_status");
    copy_optional_string(&mut target, data, "gap_state");
    copy_optional_value(&mut target, data, "repair_route");
    target.insert("verify_command".to_string(), Value::String(verify_command));
    if let Some(command) = receipt_command {
        target.insert("receipt_command".to_string(), Value::String(command));
    }
    Some(Value::Object(target))
}

fn python_repair_card_text(
    data: &Value,
    route: &Value,
    verify_command: &str,
    receipt_command: Option<&str>,
) -> Option<String> {
    let gap_identity = first_gap_identity(data)?;
    let changed_owner = string_at(data, &["anchor", "owner"]).unwrap_or(gap_identity);
    let missing_discriminator = missing_discriminator_for_packet(data, route)?;
    let route_kind = route.get("route_kind").and_then(non_empty_string)?;
    let assertion = route
        .get("assertion_shape")
        .and_then(non_empty_string)
        .unwrap_or(missing_discriminator.as_str());
    let target_file = route.get("target_file").and_then(non_empty_string);
    let related_test = route.get("related_test").and_then(non_empty_string);
    let test_name = related_test
        .map(related_test_name_or_raw)
        .filter(|name| !name.is_empty());
    let mut lines = vec![
        "Python repair card (preview/advisory)".to_string(),
        String::new(),
        "Freshness: current validated GapRecord diagnostic.".to_string(),
        "If the editor status is stale, refresh analysis before assigning repair work.".to_string(),
        String::new(),
        "Changed owner:".to_string(),
        format!("  {changed_owner}"),
    ];
    if let Some(changed_behavior) = route.get("changed_behavior").and_then(non_empty_string) {
        lines.push("Changed behavior:".to_string());
        lines.push(format!("  {changed_behavior}"));
    }
    lines.push("Current test evidence:".to_string());
    if let Some(related_test) = related_test {
        lines.push(format!("  Related test target: {related_test}"));
        lines.push("  Current evidence is weak preview evidence; strengthen it with the missing discriminator.".to_string());
    } else {
        lines.push(
            "  No related test selector is available in this GapRecord; use the suggested file."
                .to_string(),
        );
    }
    lines.push("Missing discriminator:".to_string());
    lines.push(format!("  {missing_discriminator}"));
    lines.push("Recommended repair:".to_string());
    lines.push(format!("  {route_kind}"));
    lines.push("Suggested assertion:".to_string());
    lines.push(format!("  {assertion}"));
    lines.push("Suggested location:".to_string());
    match (target_file, test_name) {
        (Some(file), Some(name)) => {
            lines.push(format!("  File: {file}"));
            lines.push(format!("  Test: {name}"));
        }
        (Some(file), None) => lines.push(format!("  File: {file}")),
        (None, Some(name)) => lines.push(format!("  Test: {name}")),
        (None, None) => {
            lines.push("  No safe suggested location is available in this GapRecord.".to_string());
        }
    }
    lines.push("Verify:".to_string());
    lines.push(format!("  {verify_command}"));
    lines.push("Receipt:".to_string());
    lines.push(format!(
        "  {}",
        receipt_command.unwrap_or(
            "unavailable in this GapRecord; regenerate the gap ledger from check output"
        )
    ));
    if let Some(items) = route
        .get("stop_conditions")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
    {
        lines.push("Stop conditions:".to_string());
        for item in items {
            if let Some(text) = item.as_str().map(str::trim).filter(|text| !text.is_empty()) {
                lines.push(format!("  - {text}"));
            }
        }
    }
    lines.push("Limits:".to_string());
    lines.push(
        "  - Static preview evidence only; no correctness or mutation adequacy claim.".to_string(),
    );
    lines.push(
        "  - Edit only the suggested test surface unless a separate packet says otherwise."
            .to_string(),
    );
    lines.push("  - Do not generate tests, call providers, run imports, or edit production code from this card.".to_string());
    Some(lines.join("\n"))
}

fn python_pytest_skeleton_target(snapshot: &AnalysisSnapshot, data: &Value) -> Option<LSPAny> {
    if string_at(data, &["language"]) != Some("python") {
        return None;
    }
    let route = data.get("repair_route")?;
    let target_file = route.get("target_file").and_then(non_empty_string)?;
    if !workspace_path_is_safe(snapshot.root.as_path(), target_file)
        || !path_matches_diagnostic_language(data, target_file)
    {
        return None;
    }
    let verify_command =
        first_safe_command_at(snapshot.root.as_path(), data, &["verification_commands"])?;
    if !verify_command.starts_with("pytest ") {
        return None;
    }
    let test_name = python_test_name_for_skeleton(data, route, &verify_command);
    let assertion = route
        .get("assertion_shape")
        .and_then(non_empty_string)
        .unwrap_or("assert <observed> == <expected>");
    let missing_discriminator =
        missing_discriminator_for_packet(data, route).unwrap_or_else(|| assertion.to_string());
    let gap_identity = first_gap_identity(data).unwrap_or("unknown");
    let mut lines = vec![
        "# RIPR Python repair skeleton".to_string(),
        format!("# Gap: {gap_identity}"),
        format!("# Suggested file: {target_file}"),
        format!("# Missing discriminator: {missing_discriminator}"),
    ];
    if let Some(changed_behavior) = route.get("changed_behavior").and_then(non_empty_string) {
        lines.push(format!("# Changed behavior: {changed_behavior}"));
    }
    lines.push(format!("# Verify: {verify_command}"));
    lines.push(
        "# Boundary: preview static evidence; do not edit production code from this skeleton."
            .to_string(),
    );
    let stop_conditions = route
        .get("stop_conditions")
        .and_then(Value::as_array)
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .take(3)
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if !stop_conditions.is_empty() {
        lines.push("# Stop if:".to_string());
        for condition in stop_conditions {
            lines.push(format!("# - {condition}"));
        }
    }
    lines.push(String::new());
    lines.push(format!("def {test_name}():"));
    lines.push(format!(
        "    # Arrange inputs/fixtures that exercise {missing_discriminator}."
    ));
    lines.push("    # Act through the changed owner or route.".to_string());
    lines.push("    # Suggested assertion:".to_string());
    lines.push(format!("    # {assertion}"));
    lines.push(
        "    raise NotImplementedError(\"fill RIPR skeleton with imports, fixtures, and expected value\")"
            .to_string(),
    );

    Some(serde_json::json!({
        "label": "python_pytest_skeleton",
        "gap_id": string_at(data, &["gap_id"]),
        "canonical_gap_id": string_at(data, &["canonical_gap_id"]),
        "target_file": target_file,
        "test_name": test_name,
        "verify_command": verify_command,
        "brief": lines.join("\n"),
    }))
}

/// Build the LSP action target for "Copy TypeScript repair packet (advisory)"
/// (RIPR-SPEC-0088 §PR8). Returns `None` unless:
/// - `data.preview_actionability.repair_packet_ready == true`
/// - `data.language == "typescript"` (or javascript)
/// - `data.language_status == "preview"`
///
/// The target carries verify_command, receipt_command, canonical_gap_id, and
/// edit_surface from the `preview_actionability` data so the VS Code extension
/// can copy them to the clipboard without a separate server round-trip.
fn typescript_repair_packet_target(
    params: &CodeActionParams,
    diagnostic: &Diagnostic,
    data: &Value,
) -> Option<LSPAny> {
    // Only for TypeScript/JavaScript preview findings.
    let language = string_at(data, &["language"])?;
    if language != "typescript" && language != "javascript" {
        return None;
    }
    let language_status = string_at(data, &["language_status"]).unwrap_or("stable");
    if language_status != "preview" {
        return None;
    }

    // repair_packet_ready must be true (from preview_actionability in diagnostic data).
    let actionability = data.get("preview_actionability")?;
    let repair_ready = actionability
        .get("repair_packet_ready")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !repair_ready {
        return None;
    }

    // Extract verify/receipt from typescript_repair_packet if present in data,
    // else fall back to verification_commands from the gap data.
    let ts_packet = data.get("typescript_repair_packet");
    let verify_command = ts_packet
        .and_then(|p| p.get("verify_command"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            data.get("verification_commands")
                .and_then(|cmds| cmds.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .filter(|s| !s.trim().is_empty())
        })?;

    let receipt_command = ts_packet
        .and_then(|p| p.get("receipt_command"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty());

    let canonical_gap_id = ts_packet
        .and_then(|p| p.get("canonical_gap_id"))
        .and_then(|v| v.as_str())
        .or_else(|| string_at(data, &["canonical_gap_id"]));

    let edit_surface: Vec<&str> = ts_packet
        .and_then(|p| p.get("allowed_edit_surface"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();

    let mut target = serde_json::Map::new();
    target.insert(
        "label".to_string(),
        Value::String("typescript_repair_packet".to_string()),
    );
    target.insert(
        "line".to_string(),
        serde_json::Value::Number(serde_json::Number::from(
            params.range.start.line.saturating_add(1),
        )),
    );
    if let Some(id) = canonical_gap_id {
        target.insert(
            "canonical_gap_id".to_string(),
            Value::String(id.to_string()),
        );
    }
    target.insert("language".to_string(), Value::String(language.to_string()));
    target.insert(
        "language_status".to_string(),
        Value::String(language_status.to_string()),
    );
    target.insert(
        "authority_boundary".to_string(),
        Value::String("preview_advisory_only".to_string()),
    );
    target.insert(
        "verify_command".to_string(),
        Value::String(verify_command.to_string()),
    );
    if let Some(receipt) = receipt_command {
        target.insert(
            "receipt_command".to_string(),
            Value::String(receipt.to_string()),
        );
    }
    if !edit_surface.is_empty() {
        target.insert(
            "allowed_edit_surface".to_string(),
            Value::Array(
                edit_surface
                    .iter()
                    .map(|s| Value::String(s.to_string()))
                    .collect(),
            ),
        );
    }
    if let Some(finding_id) = diagnostic
        .data
        .as_ref()
        .and_then(|d| d.as_object())
        .and_then(|obj| obj.get("finding_id"))
    {
        target.insert("finding_id".to_string(), finding_id.clone());
    }
    Some(Value::Object(target))
}

fn python_test_name_for_skeleton(data: &Value, route: &Value, verify_command: &str) -> String {
    route
        .get("related_test")
        .and_then(non_empty_string)
        .map(related_test_name_or_raw)
        .or_else(|| pytest_node_id_test_name(verify_command))
        .map(sanitize_python_test_name)
        .filter(|name| name.starts_with("test_"))
        .unwrap_or_else(|| {
            let identity = first_gap_identity(data).unwrap_or("python_gap");
            format!("test_{}", slug_identifier(identity))
        })
}

fn related_test_name_or_raw(raw: &str) -> &str {
    let name = related_test_name(raw);
    if name.is_empty() { raw.trim() } else { name }
}

fn pytest_node_id_test_name(command: &str) -> Option<&str> {
    command
        .split_whitespace()
        .find_map(|token| token.split_once("::").map(|(_, name)| name))
        .map(|name| name.rsplit("::").next().unwrap_or(name).trim())
        .filter(|name| !name.is_empty())
}

fn sanitize_python_test_name(name: &str) -> String {
    let mut out = String::new();
    let mut last_underscore = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_underscore = false;
        } else if !last_underscore {
            out.push('_');
            last_underscore = true;
        }
    }
    let sanitized = out.trim_matches('_');
    if sanitized.is_empty() {
        "test_ripr_gap".to_string()
    } else if sanitized.starts_with("test_") {
        sanitized.to_string()
    } else {
        format!("test_{sanitized}")
    }
}

fn slug_identifier(value: &str) -> String {
    sanitize_python_test_name(value)
        .trim_start_matches("test_")
        .chars()
        .take(80)
        .collect::<String>()
}

fn missing_discriminator_for_packet(data: &Value, repair_route: &Value) -> Option<String> {
    string_at(data, &["missing_discriminator"])
        .or_else(|| {
            repair_route
                .get("missing_discriminator")
                .and_then(non_empty_string)
        })
        .or_else(|| {
            repair_route
                .get("assertion_shape")
                .and_then(non_empty_string)
        })
        .or_else(|| {
            repair_route
                .get("changed_behavior")
                .and_then(non_empty_string)
        })
        .map(ToOwned::to_owned)
}

fn focused_proof_intent_for_packet(repair_route: &Value) -> Option<String> {
    let assertion_shape = repair_route
        .get("assertion_shape")
        .and_then(non_empty_string)?;
    let target = repair_route
        .get("related_test")
        .and_then(non_empty_string)
        .or_else(|| repair_route.get("target_file").and_then(non_empty_string))
        .unwrap_or("the related test or proof target");
    Some(format!(
        "Add one focused assertion or output proof in {target} for {assertion_shape}."
    ))
}

fn repair_packet_artifacts(data: &Value) -> Vec<String> {
    let mut artifacts = Vec::new();
    for key in ["source_artifact", "gap_ledger", "first_pr_packet"] {
        if let Some(artifact) = string_at(data, &[key])
            && !artifacts.iter().any(|item| item == artifact)
        {
            artifacts.push(artifact.to_string());
        }
    }
    artifacts
}

fn copy_optional_string(object: &mut serde_json::Map<String, Value>, data: &Value, key: &str) {
    if let Some(value) = data.get(key).and_then(non_empty_string) {
        object.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn copy_optional_value(object: &mut serde_json::Map<String, Value>, data: &Value, key: &str) {
    if let Some(value) = data.get(key) {
        object.insert(key.to_string(), value.clone());
    }
}

fn gap_command_target(
    params: &CodeActionParams,
    diagnostic: &Diagnostic,
    label: &str,
    command: &str,
) -> LSPAny {
    let mut target = copy_context_target(params, diagnostic);
    if let Some(object) = target.as_object_mut() {
        object.insert("label".to_string(), Value::String(label.to_string()));
        object.insert("command".to_string(), Value::String(command.to_string()));
        object.insert("root".to_string(), Value::String(COMMAND_ROOT.to_string()));
        let Some(data) = diagnostic.data.as_ref() else {
            return target;
        };
        for key in [
            "gap_id",
            "canonical_gap_id",
            "gap_kind",
            "language",
            "language_status",
            "gap_state",
            "policy_state",
            "repairability",
        ] {
            copy_optional_string(object, data, key);
        }
    }
    target
}

fn first_safe_command_at(root: &Path, data: &Value, path: &[&str]) -> Option<String> {
    safe_commands_at(root, data, path).into_iter().next()
}

fn safe_commands_at(root: &Path, data: &Value, path: &[&str]) -> Vec<String> {
    value_at(data, path)
        .and_then(Value::as_array)
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|command| command_payload_is_safe(root, command))
        .map(ToOwned::to_owned)
        .collect()
}

fn first_safe_receipt_command(root: &Path, data: &Value) -> Option<String> {
    [
        &["receipt_command"][..],
        &["commands", "receipt"][..],
        &["receipt", "command"][..],
    ]
    .iter()
    .filter_map(|path| string_at(data, path))
    .find(|command| command_payload_is_safe(root, command))
    .map(ToOwned::to_owned)
}

fn gap_related_test_target(snapshot: &AnalysisSnapshot, data: &Value) -> Option<LSPAny> {
    let route = data.get("repair_route")?;
    let related_test = route.get("related_test").and_then(non_empty_string);
    let target_file = route.get("target_file").and_then(non_empty_string);
    let repair_target = related_test
        .filter(|raw| path_matches_diagnostic_language(data, related_test_path_part(raw)))
        .or(target_file)?;
    if !workspace_path_is_safe(snapshot.root.as_path(), repair_target) {
        return None;
    }
    let file = related_test_path_part(repair_target);
    if !path_matches_diagnostic_language(data, file) {
        return None;
    }
    let absolute = if Path::new(file).is_absolute() {
        PathBuf::from(file)
    } else {
        snapshot.root.join(file)
    };
    if !absolute.is_file() {
        return None;
    }
    let uri = file_uri_for_path(&absolute).ok()?;
    let line = route
        .get("target_line")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    Some(serde_json::json!({
        "uri": uri.as_str(),
        "line": line,
        "test_name": related_test.map(related_test_name_or_raw).unwrap_or(""),
    }))
}

fn related_test_path_part(raw: &str) -> &str {
    raw.split_once("::").map_or(raw, |(path, _)| path).trim()
}

fn related_test_name(raw: &str) -> &str {
    raw.split_once("::").map_or("", |(_, name)| name).trim()
}

fn path_matches_diagnostic_language(data: &Value, path: &str) -> bool {
    match string_at(data, &["language"]) {
        Some("rust") => path.ends_with(".rs"),
        Some("python") => path.ends_with(".py"),
        Some("typescript") => path.ends_with(".ts") || path.ends_with(".tsx"),
        Some("javascript") => path.ends_with(".js") || path.ends_with(".jsx"),
        _ => false,
    }
}

fn static_limit_note_target(params: &CodeActionParams, diagnostic: &Diagnostic) -> Option<LSPAny> {
    let data = diagnostic.data.as_ref()?;
    let note = static_limit_note(data)?;
    let mut target = copy_context_target(params, diagnostic);
    let object = target.as_object_mut()?;
    object.insert(
        "label".to_string(),
        Value::String("static_limit_note".to_string()),
    );
    object.insert("note".to_string(), Value::String(note));
    object.insert(
        "limits_note".to_string(),
        Value::String("Static evidence only; no runtime adequacy claim.".to_string()),
    );
    if let Some(navigation_target) = value_at(data, &["navigation_only_target"]).cloned() {
        object.insert("navigation_only_target".to_string(), navigation_target);
    }
    Some(target)
}

/// Whether a gap diagnostic carries the producer-owned cross-language
/// test-target limitation. The category string is emitted by the producer
/// authority (`analysis::repair_route::cross_language_test_target_unresolved`
/// via `output::evidence_record` / ledger adapters); the LSP consumes the
/// emitted category across the artifact shapes it ingests and does not
/// re-derive the predicate — no `ClassifiedSeam` exists for ledger gaps.
fn gap_cross_language_target_unresolved(data: &Value) -> bool {
    const CATEGORY: &str = CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY;
    string_at(data, &["static_limit_kind"]) == Some(CATEGORY)
        || string_at(data, &["static_limit_category"]) == Some(CATEGORY)
        || value_at(data, &["projection_exclusion_reasons"])
            .and_then(Value::as_array)
            .is_some_and(|reasons| {
                reasons
                    .iter()
                    .any(|reason| reason.as_str() == Some(CATEGORY))
            })
        || limitation_array_contains_category(data, &["static_limits"], CATEGORY)
        || limitation_array_contains_category(data, &["static_limitations"], CATEGORY)
}

fn limitation_array_contains_category(data: &Value, path: &[&str], category: &str) -> bool {
    value_at(data, path)
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                string_at(item, &["static_limit_kind"]) == Some(category)
                    || string_at(item, &["static_limit_category"]) == Some(category)
                    || string_at(item, &["category"]) == Some(category)
            })
        })
}

fn static_limit_note(data: &Value) -> Option<String> {
    let mut lines = Vec::new();
    if let Some(kind) = string_at(data, &["static_limit_kind"]) {
        lines.push(format!("Static limit: {kind}"));
    }
    if let Some(detail) = string_at(data, &["static_limit_detail"]) {
        lines.push(format!("Detail: {detail}"));
    }
    if let Some(items) = value_at(data, &["static_limits"]).and_then(Value::as_array) {
        for item in items {
            if let Some(kind) = string_at(item, &["static_limit_kind"]) {
                lines.push(format!("Static limit: {kind}"));
            }
            if let Some(detail) = string_at(item, &["static_limit_detail"]) {
                lines.push(format!("Detail: {detail}"));
            }
        }
    }
    if let Some(target_lines) = navigation_only_target_lines(data) {
        lines.extend(target_lines);
    }
    if lines.is_empty() {
        None
    } else {
        lines.push("Boundary: static evidence only; advisory action.".to_string());
        Some(lines.join("\n"))
    }
}

fn navigation_only_target_lines(data: &Value) -> Option<Vec<String>> {
    let target = value_at(data, &["navigation_only_target"])?;
    let file = string_at(target, &["file"])?;
    let line = target
        .get("line")
        .and_then(Value::as_u64)
        .map(|line| line.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let mut lines = vec![format!("Navigation-only target: {file}:{line}")];
    if let Some(test_name) = string_at(target, &["test_name"]) {
        lines.push(format!("External observer: {test_name}"));
    }
    if let Some(language) = string_at(target, &["language"]) {
        lines.push(format!("External language: {language}"));
    }
    if let Some(route) = string_at(target, &["limitation_route"]) {
        lines.push(format!("Limitation route: {route}"));
    }
    let ready = target
        .get("repair_packet_ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    lines.push(format!("Repair packet ready: {ready}"));
    Some(lines)
}

fn string_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    value_at(value, path).and_then(non_empty_string)
}

fn value_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn non_empty_string(value: &Value) -> Option<&str> {
    let text = value.as_str()?.trim();
    if text.is_empty() { None } else { Some(text) }
}

fn copy_targeted_test_brief_action(
    seam: &ClassifiedSeam,
    brief: String,
    diagnostic: &Diagnostic,
    snapshot: Option<&AnalysisSnapshot>,
) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title: TARGETED_TEST_BRIEF_TITLE.to_string(),
        kind: Some(CodeActionKind::new("source.ripr.inspect")),
        diagnostics: Some(vec![diagnostic.clone()]),
        command: Some(Command {
            title: TARGETED_TEST_BRIEF_TITLE.to_string(),
            command: COPY_TARGETED_TEST_BRIEF_COMMAND.to_string(),
            arguments: Some(vec![serde_json::json!({
                "seam_id": seam.seam.id().as_str(),
                "brief": brief,
            })]),
        }),
        data: Some(action_data_payload(
            "source.ripr.inspect",
            "copy_targeted_test_brief",
            COPY_TARGETED_TEST_BRIEF_COMMAND,
            Some(diagnostic),
            snapshot,
        )),
        ..CodeAction::default()
    })
}

fn copy_python_pytest_skeleton_action(
    target: LSPAny,
    diagnostic: &Diagnostic,
    snapshot: Option<&AnalysisSnapshot>,
) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title: COPY_PYTHON_PYTEST_SKELETON_TITLE.to_string(),
        kind: Some(CodeActionKind::new("source.ripr.inspect")),
        diagnostics: Some(vec![diagnostic.clone()]),
        command: Some(Command {
            title: COPY_PYTHON_PYTEST_SKELETON_TITLE.to_string(),
            command: COPY_TARGETED_TEST_BRIEF_COMMAND.to_string(),
            arguments: Some(vec![target]),
        }),
        data: Some(action_data_payload(
            "source.ripr.inspect",
            "copy_python_pytest_skeleton",
            COPY_TARGETED_TEST_BRIEF_COMMAND,
            Some(diagnostic),
            snapshot,
        )),
        ..CodeAction::default()
    })
}

fn copy_python_repair_card_action(
    target: LSPAny,
    diagnostic: &Diagnostic,
    snapshot: Option<&AnalysisSnapshot>,
) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title: COPY_PYTHON_REPAIR_CARD_TITLE.to_string(),
        kind: Some(CodeActionKind::new("source.ripr.inspect")),
        diagnostics: Some(vec![diagnostic.clone()]),
        command: Some(Command {
            title: COPY_PYTHON_REPAIR_CARD_TITLE.to_string(),
            command: COPY_TARGETED_TEST_BRIEF_COMMAND.to_string(),
            arguments: Some(vec![target]),
        }),
        data: Some(action_data_payload(
            "source.ripr.inspect",
            "copy_python_repair_card",
            COPY_TARGETED_TEST_BRIEF_COMMAND,
            Some(diagnostic),
            snapshot,
        )),
        ..CodeAction::default()
    })
}

fn copy_suggested_assertion_action(
    seam: &ClassifiedSeam,
    assertion: String,
    diagnostic: &Diagnostic,
    snapshot: Option<&AnalysisSnapshot>,
) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title: SUGGESTED_ASSERTION_TITLE.to_string(),
        kind: Some(CodeActionKind::new("source.ripr.inspect")),
        diagnostics: Some(vec![diagnostic.clone()]),
        command: Some(Command {
            title: SUGGESTED_ASSERTION_TITLE.to_string(),
            command: COPY_SUGGESTED_ASSERTION_COMMAND.to_string(),
            arguments: Some(vec![serde_json::json!({
                "seam_id": seam.seam.id().as_str(),
                "assertion": assertion,
            })]),
        }),
        data: Some(action_data_payload(
            "source.ripr.inspect",
            "copy_suggested_assertion",
            COPY_SUGGESTED_ASSERTION_COMMAND,
            Some(diagnostic),
            snapshot,
        )),
        ..CodeAction::default()
    })
}

fn open_related_test_action(
    target: LSPAny,
    diagnostic: &Diagnostic,
    snapshot: Option<&AnalysisSnapshot>,
) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title: OPEN_RELATED_TEST_TITLE.to_string(),
        kind: Some(CodeActionKind::new("source.ripr.navigate")),
        diagnostics: Some(vec![diagnostic.clone()]),
        command: Some(Command {
            title: OPEN_RELATED_TEST_TITLE.to_string(),
            command: OPEN_RELATED_TEST_COMMAND.to_string(),
            arguments: Some(vec![target]),
        }),
        data: Some(action_data_payload(
            "source.ripr.navigate",
            "open_related_test",
            OPEN_RELATED_TEST_COMMAND,
            Some(diagnostic),
            snapshot,
        )),
        ..CodeAction::default()
    })
}

fn is_ripr_diagnostic(diagnostic: &Diagnostic) -> bool {
    diagnostic.source.as_deref() == Some("ripr")
}

fn is_seam_diagnostic(diagnostic: &Diagnostic) -> bool {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("seam_id"))
        .and_then(|value| value.as_str())
        .is_some()
}

fn is_gap_diagnostic(diagnostic: &Diagnostic) -> bool {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("gap_id"))
        .and_then(|value| value.as_str())
        .is_some()
}

fn copy_context_target(params: &CodeActionParams, diagnostic: &Diagnostic) -> LSPAny {
    let mut target = serde_json::Map::new();
    target.insert(
        "uri".to_string(),
        serde_json::Value::String(params.text_document.uri.as_str().to_string()),
    );
    target.insert(
        "line".to_string(),
        serde_json::Value::Number(serde_json::Number::from(
            params.range.start.line.saturating_add(1),
        )),
    );
    if let Some(data) = &diagnostic.data
        && let Some(obj) = data.as_object()
    {
        if let Some(finding_id) = obj.get("finding_id").and_then(|v| v.as_str()) {
            target.insert(
                "finding_id".to_string(),
                serde_json::Value::String(finding_id.to_string()),
            );
        }
        if let Some(probe_id) = obj.get("probe_id").and_then(|v| v.as_str()) {
            target.insert(
                "probe_id".to_string(),
                serde_json::Value::String(probe_id.to_string()),
            );
        }
        if let Some(seam_id) = obj.get("seam_id").and_then(|v| v.as_str()) {
            target.insert(
                "seam_id".to_string(),
                serde_json::Value::String(seam_id.to_string()),
            );
        }
        if let Some(seam_kind) = obj.get("seam_kind").and_then(|v| v.as_str()) {
            target.insert(
                "seam_kind".to_string(),
                serde_json::Value::String(seam_kind.to_string()),
            );
        }
        for key in [
            "gap_id",
            "canonical_gap_id",
            "gap_kind",
            "gap_ledger",
            "language",
            "language_status",
            "owner_kind",
            "static_limit_kind",
            "explain_command",
        ] {
            if let Some(value) = obj.get(key).and_then(|v| v.as_str()) {
                target.insert(
                    key.to_string(),
                    serde_json::Value::String(value.to_string()),
                );
            }
        }
        copy_optional_value(&mut target, data, "preview_actionability");
        copy_optional_value(&mut target, data, "witness");
        copy_optional_value(&mut target, data, "evidence_identity");
    }
    serde_json::Value::Object(target)
}

fn copy_seam_packet_target(
    params: &CodeActionParams,
    diagnostic: &Diagnostic,
    seam: &ClassifiedSeam,
) -> LSPAny {
    let mut target = copy_context_target(params, diagnostic);
    if let Some(obj) = target.as_object_mut() {
        obj.insert(
            "line".to_string(),
            serde_json::Value::Number(serde_json::Number::from(seam.seam.display_line())),
        );
        obj.insert(
            "seam_id".to_string(),
            serde_json::Value::String(seam.seam.id().as_str().to_string()),
        );
        obj.insert(
            "seam_kind".to_string(),
            serde_json::Value::String(seam.seam.kind().as_str().to_string()),
        );
    }
    target
}

fn best_related_test_for_editor(seam: &ClassifiedSeam) -> Option<&RelatedTestGrip> {
    seam.evidence
        .related_tests
        .iter()
        .find(|test| test.oracle_strength == OracleStrength::Strong)
        .or_else(|| {
            seam.evidence
                .related_tests
                .iter()
                .min_by_key(|test| relation_confidence_rank(test.relation_confidence))
        })
}

fn relation_confidence_rank(confidence: RelationConfidence) -> u8 {
    match confidence {
        RelationConfidence::High => 0,
        RelationConfidence::Medium => 1,
        RelationConfidence::Low => 2,
        RelationConfidence::Opaque => 3,
    }
}

fn related_test_target(snapshot: &AnalysisSnapshot, related: &RelatedTestGrip) -> Option<LSPAny> {
    let path = absolute_related_test_path(snapshot, related);
    if !super::uri::path_is_within_root(snapshot.root.as_path(), &path) {
        return None;
    }
    let uri = file_uri_for_path(&path).ok()?;
    Some(serde_json::json!({
        "uri": uri.as_str(),
        "line": related.line,
        "test_name": related.test_name.as_str(),
    }))
}

fn absolute_related_test_path(snapshot: &AnalysisSnapshot, related: &RelatedTestGrip) -> PathBuf {
    if related.file.is_absolute() {
        related.file.clone()
    } else {
        snapshot.root.join(&related.file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::command_specs::{agent_receipt_command_spec, agent_verify_command_spec};
    use crate::app::Mode;
    use crate::domain::{LanguageId, LanguageStatus};
    use crate::lsp::gap_artifacts::{GapArtifactIdentity, GapArtifactKind};
    use crate::lsp::state::RefreshMetadata;
    use std::collections::BTreeMap;
    use tower_lsp_server::ls_types::{
        CodeActionContext, DiagnosticSeverity, Position, Range, TextDocumentIdentifier, Uri,
    };

    #[test]
    fn gap_diagnostic_without_snapshot_gets_refresh_only() -> Result<(), String> {
        let diagnostic = gap_diagnostic();
        let params = code_action_params(vec![diagnostic])?;

        let actions = code_action_response(&params, None, &ClientFeatureProfile::unsupported());
        let titles = action_titles(&actions);

        assert_eq!(titles, vec![REFRESH_ANALYSIS_TITLE]);
        Ok(())
    }

    #[test]
    fn disabled_action_fails_closed_for_reserved_reasons() -> Result<(), String> {
        // #1892: the emit guard keeps the closed vocabulary honest — a
        // reserved reason (no real producer yet) yields no action at all,
        // while every emitted reason yields an inert action with no command
        // or edit attached.
        let diagnostic = gap_diagnostic();
        for reason in crate::lsp::action_contract::RESERVED_DISABLED_REASONS {
            if disabled_action(
                TARGETED_TEST_BRIEF_TITLE,
                "source.ripr.inspect",
                "copy_targeted_test_brief",
                COPY_TARGETED_TEST_BRIEF_COMMAND,
                &diagnostic,
                None,
                *reason,
            )
            .is_some()
            {
                return Err(format!(
                    "reserved reason {} must not be emittable",
                    reason.as_str()
                ));
            }
        }
        for reason in crate::lsp::action_contract::EMITTED_DISABLED_REASONS {
            let action = disabled_action(
                TARGETED_TEST_BRIEF_TITLE,
                "source.ripr.inspect",
                "copy_targeted_test_brief",
                COPY_TARGETED_TEST_BRIEF_COMMAND,
                &diagnostic,
                None,
                *reason,
            )
            .ok_or_else(|| format!("emitted reason {} was rejected", reason.as_str()))?;
            let CodeActionOrCommand::CodeAction(action) = action else {
                return Err("expected code action literal".to_string());
            };
            if action.command.is_some() || action.edit.is_some() {
                return Err(format!(
                    "disabled action for {} must not carry a command or edit",
                    reason.as_str()
                ));
            }
            if action.kind.is_none() || action.is_preferred == Some(true) {
                return Err(format!(
                    "disabled action for {} must keep its kind and stay un-preferred",
                    reason.as_str()
                ));
            }
            let machine_reason = action
                .data
                .as_ref()
                .and_then(|data| data.get("disabled_reason"))
                .and_then(Value::as_str);
            if machine_reason != Some(reason.as_str()) {
                return Err(format!(
                    "disabled action must name {} in its data payload",
                    reason.as_str()
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn copy_context_target_forwards_gap_identity_and_ledger() -> Result<(), String> {
        let diagnostic = gap_diagnostic();
        let params = code_action_params(vec![diagnostic.clone()])?;

        let target = copy_context_target(&params, &diagnostic);

        assert_eq!(
            target["uri"], "file:///workspace/src/pricing.rs",
            "target URI should match request URI"
        );
        assert_eq!(target["line"], 12);
        assert_eq!(target["gap_id"], "gap:pr:pricing:threshold-boundary");
        assert_eq!(
            target["canonical_gap_id"],
            "gap:rust:pricing:threshold-boundary"
        );
        assert_eq!(target["gap_kind"], "MissingBoundaryAssertion");
        assert_eq!(
            target["gap_ledger"],
            "target/ripr/reports/gap-decision-ledger.json"
        );
        Ok(())
    }

    #[test]
    fn gap_command_target_keeps_context_when_diagnostic_has_no_data() -> Result<(), String> {
        let params = code_action_params(Vec::new())?;
        let target = gap_command_target(
            &params,
            &Diagnostic::default(),
            "gap_verify",
            "cargo xtask verify",
        );

        if target["label"] != "gap_verify"
            || target["command"] != "cargo xtask verify"
            || target["root"] != COMMAND_ROOT
            || target["uri"] != "file:///workspace/src/pricing.rs"
            || target["line"] != 12
        {
            return Err(format!("gap command context was not preserved: {target}"));
        }
        Ok(())
    }

    #[test]
    fn python_pytest_skeleton_rejects_non_python_or_unsafe_targets() {
        let snapshot = python_snapshot();
        let mut data = serde_json::json!({
            "language": "rust",
            "canonical_gap_id": "gap:rust:pricing:threshold-boundary",
            "repair_route": {
                "target_file": "tests/test_pricing.py",
                "assertion_shape": "assert result == expected"
            },
            "verification_commands": ["pytest tests/test_pricing.py::test_boundary"]
        });

        assert!(python_pytest_skeleton_target(&snapshot, &data).is_none());

        data["language"] = serde_json::json!("python");
        data["repair_route"]["target_file"] = serde_json::json!("../outside/test_pricing.py");

        assert!(python_pytest_skeleton_target(&snapshot, &data).is_none());
    }

    #[test]
    fn gap_repair_packet_projects_only_validated_command_specs() -> Result<(), String> {
        let (params, diagnostic) = gap_action_request()?;
        let snapshot = python_snapshot();
        let artifact = action_artifact();
        let valid_target = gap_repair_packet_target(&params, &snapshot, &diagnostic, &artifact)
            .ok_or_else(|| "valid gap repair target was omitted".to_string())?;
        if !valid_target
            .get("command_specs")
            .is_some_and(|value| value["verify"]["command_id"] == "ripr:agent:verify")
        {
            return Err(format!(
                "valid command specs were not projected: {valid_target}"
            ));
        }

        let mut multiple = artifact.clone();
        let verify_spec = multiple
            .verify_command_specs
            .first()
            .cloned()
            .ok_or_else(|| "artifact omitted verify command spec".to_string())?;
        let receipt_spec = multiple
            .receipt_command_specs
            .first()
            .cloned()
            .ok_or_else(|| "artifact omitted receipt command spec".to_string())?;
        multiple.verify_command_specs.push(verify_spec);
        multiple.receipt_command_specs.push(receipt_spec);
        let multiple_target = gap_repair_packet_target(&params, &snapshot, &diagnostic, &multiple)
            .ok_or_else(|| "multi-route gap repair target was omitted".to_string())?;
        if !multiple_target
            .get("command_specs")
            .is_some_and(|value| value["verify"].is_array() && value["receipt"].is_array())
        {
            return Err(format!(
                "multi-route specs were not arrays: {multiple_target}"
            ));
        }

        let mut empty = artifact.clone();
        empty.verify_command_specs.clear();
        empty.receipt_command_specs.clear();
        assert_command_specs_omitted(&params, &snapshot, &diagnostic, &empty)?;

        let mut invalid = artifact.clone();
        invalid
            .verify_command_specs
            .first_mut()
            .ok_or_else(|| "artifact omitted verify command spec".to_string())?
            .program
            .clear();
        assert_command_specs_omitted(&params, &snapshot, &diagnostic, &invalid)?;

        let mut mismatched = artifact;
        mismatched
            .receipt_command_specs
            .first_mut()
            .ok_or_else(|| "artifact omitted receipt command spec".to_string())?
            .role = crate::domain::CommandRole::Verify;
        assert_command_specs_omitted(&params, &snapshot, &diagnostic, &mismatched)?;
        Ok(())
    }

    #[test]
    fn python_pytest_name_helpers_cover_node_ids_and_fallbacks() {
        assert_eq!(
            pytest_node_id_test_name("pytest tests/test_pricing.py::TestPricing::test_boundary"),
            Some("test_boundary")
        );
        assert_eq!(
            pytest_node_id_test_name("pytest tests/test_pricing.py::"),
            None
        );
        assert_eq!(sanitize_python_test_name(""), "test_ripr_gap");
        assert_eq!(
            sanitize_python_test_name("Discount Boundary!"),
            "test_discount_boundary"
        );

        let data = serde_json::json!({
            "canonical_gap_id": "python:app/pricing.py:calculate_discount:predicate_boundary:amount>=threshold"
        });
        let route = serde_json::json!({});

        assert_eq!(
            python_test_name_for_skeleton(&data, &route, "pytest tests/test_pricing.py"),
            "test_python_app_pricing_py_calculate_discount_predicate_boundary_amount_threshold"
        );
    }

    fn code_action_params(diagnostics: Vec<Diagnostic>) -> Result<CodeActionParams, String> {
        Ok(CodeActionParams {
            text_document: TextDocumentIdentifier::new(test_uri(
                "file:///workspace/src/pricing.rs",
            )?),
            range: Range {
                start: Position {
                    line: 11,
                    character: 0,
                },
                end: Position {
                    line: 11,
                    character: 120,
                },
            },
            context: CodeActionContext {
                diagnostics,
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
    }

    fn gap_action_request() -> Result<(CodeActionParams, Diagnostic), String> {
        let diagnostic = Diagnostic {
            range: Range {
                start: Position {
                    line: 11,
                    character: 0,
                },
                end: Position {
                    line: 11,
                    character: 120,
                },
            },
            severity: Some(DiagnosticSeverity::WARNING),
            code: None,
            code_description: None,
            source: Some("ripr".to_string()),
            message: "ripr gap: MissingBoundaryAssertion".to_string(),
            related_information: None,
            tags: None,
            data: Some(serde_json::json!({
                "source": "gap_decision_ledger",
                "gap_id": "gap:py:pricing",
                "canonical_gap_id": "gap:py:pricing",
                "language": "python",
                "gap_state": "actionable",
                "repairability": "repairable",
                "repair_route": {
                    "route_kind": "existing_test_strengthening",
                    "target_file": "tests/test_pricing.py",
                    "related_test": "tests/test_pricing.py::test_discount_boundary"
                },
                "verification_commands": ["ripr agent verify --root . --json"],
                "receipt_command": "ripr agent receipt --root . --verify-json verify.json --seam-id seam-a --json"
            })),
        };
        Ok((code_action_params(vec![diagnostic.clone()])?, diagnostic))
    }

    fn action_artifact() -> ValidatedGapArtifact {
        ValidatedGapArtifact {
            kind: GapArtifactKind::GapDecisionLedger,
            root: Some(".".to_string()),
            identities: vec![GapArtifactIdentity {
                canonical_gap_id: Some("gap:py:pricing".to_string()),
                seam_id: Some("seam-a".to_string()),
                finding_id: None,
            }],
            language: Some(LanguageId::Python),
            language_status: Some(LanguageStatus::Preview),
            gap_state: Some("actionable".to_string()),
            related_paths: vec!["tests/test_pricing.py".to_string()],
            verify_commands: vec!["ripr agent verify --root . --json".to_string()],
            receipt_commands: vec![
                "ripr agent receipt --root . --verify-json verify.json --seam-id seam-a --json"
                    .to_string(),
            ],
            verify_command_specs: vec![agent_verify_command_spec(
                ".",
                "before.json",
                "after.json",
                None,
            )],
            receipt_command_specs: vec![agent_receipt_command_spec(
                ".",
                "verify.json",
                "seam-a",
                Some("receipt.json"),
            )],
            static_limit_kinds: Vec::new(),
            has_text_static_limit: false,
        }
    }

    fn assert_command_specs_omitted(
        params: &CodeActionParams,
        snapshot: &AnalysisSnapshot,
        diagnostic: &Diagnostic,
        artifact: &ValidatedGapArtifact,
    ) -> Result<(), String> {
        let target = gap_repair_packet_target(params, snapshot, diagnostic, artifact)
            .ok_or_else(|| "gap repair target was omitted before projection".to_string())?;
        if target.get("command_specs").is_some() {
            return Err(format!("invalid command specs were projected: {target}"));
        }
        Ok(())
    }

    fn gap_diagnostic() -> Diagnostic {
        Diagnostic {
            range: Range {
                start: Position {
                    line: 11,
                    character: 0,
                },
                end: Position {
                    line: 11,
                    character: 120,
                },
            },
            severity: Some(DiagnosticSeverity::WARNING),
            code: None,
            code_description: None,
            source: Some("ripr".to_string()),
            message: "ripr gap: MissingBoundaryAssertion".to_string(),
            related_information: None,
            tags: None,
            data: Some(serde_json::json!({
                "source": "gap_decision_ledger",
                "gap_id": "gap:pr:pricing:threshold-boundary",
                "canonical_gap_id": "gap:rust:pricing:threshold-boundary",
                "gap_kind": "MissingBoundaryAssertion",
                "gap_ledger": "target/ripr/reports/gap-decision-ledger.json"
            })),
        }
    }

    fn python_snapshot() -> AnalysisSnapshot {
        AnalysisSnapshot {
            root: PathBuf::from("/workspace"),
            input_identity: None,
            base: None,
            mode: Mode::Draft,
            refresh: RefreshMetadata::default(),
            findings: Vec::new(),
            analysis_outcome: None,
            diagnostic_profile: crate::config::LspDiagnosticProfile::Full,
            classified_seams: Vec::new(),
            gap_artifacts: Vec::new(),
            gap_artifact_rejections: Vec::new(),
            diagnostics_by_uri: BTreeMap::new(),
            delivery_selection: None,
            seams_deferred: false,
            partial_scope: None,
            component_outcomes: Vec::new(),
            out_of_scope_test_file_findings: 0,
        }
    }

    fn action_titles(actions: &[CodeActionOrCommand]) -> Vec<&str> {
        actions
            .iter()
            .map(|action| match action {
                CodeActionOrCommand::CodeAction(action) => action.title.as_str(),
                CodeActionOrCommand::Command(command) => command.title.as_str(),
            })
            .collect()
    }

    fn test_uri(uri: &str) -> Result<Uri, String> {
        uri.parse::<Uri>()
            .map_err(|err| format!("failed to parse test URI: {err}"))
    }
}
