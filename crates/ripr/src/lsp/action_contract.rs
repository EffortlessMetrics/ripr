//! Versioned `CodeAction` data contract (#1892, #1661, RIPR-SPEC-0129).
//!
//! Every ripr code action carries a bounded `data` payload so a client (or a
//! future `codeAction/resolve` handler, #1751) can identify what the action
//! addresses without parsing title text. The payload mirrors the
//! `stable_id()` disclosure posture: hashed identities and producer-owned
//! identifiers only — no absolute paths, no fix-instruction summaries, no
//! retrieval references (those belong to resolve). Each action also carries
//! its stable snake_case machine name (`action_name`), which joins the
//! `action_id` fingerprint so constructors that share one command id on one
//! diagnostic still fingerprint distinctly.
//!
//! Disabled reasons are a closed vocabulary. Only reasons with a real
//! producer in the current code may be emitted (real producers only,
//! `docs/LEARNINGS.md`); reserved variants name their future producer and
//! the emit guard keeps them unemittable until that producer lands.
//!
//! The backend health/root gate (`lsp/backend.rs`) stays omit-only by
//! design: without a snapshot no diagnostic-addressing action can be
//! constructed, so there is no action to disable in place — nothing is
//! suppressed that a client could otherwise see.

use serde_json::Value;
use tower_lsp_server::ls_types::{Diagnostic, NumberOrString};

/// Payload schema version for `CodeAction.data` (#1892). Bump only with a
/// spec amendment to RIPR-SPEC-0129.
pub(super) const RIPR_CODE_ACTION_DATA_SCHEMA_VERSION: &str = "ripr-code-action-data-v1";

/// Closed vocabulary of machine-readable disabled reasons carried in
/// `CodeAction.data.disabled_reason`. Conservative language only: a reason
/// names why the action cannot execute, never an evidence verdict.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ActionDisabledReason {
    /// The current snapshot no longer carries the diagnostic the action
    /// addresses (stale gap-diagnostic suppression in `lsp/actions.rs`).
    StaleSnapshot,
    /// The negotiated client profile did not advertise the command the
    /// action would execute (disabled form of the #1776 filter).
    ClientCapabilityMissing,
    /// The gap record carries no safe verification command, so the verify
    /// handoff cannot be constructed.
    VerificationRouteUnavailable,
    /// The gap record carries no safe receipt command, so the receipt
    /// handoff cannot be constructed.
    ReceiptRouteUnavailable,
    /// The action addresses a preview/static limitation (a cross-language
    /// unresolved test target suppressed the repair-packet surface).
    PreviewOrStaticLimitation,
    // Reserved below: closed-vocabulary members with named future producers.
    // The emit guard (`disabled_reason_emittable`) keeps every reserved
    // reason unemitted until its producer lands — real producers only.
    /// Reserved: producer is a document-version staleness path that
    /// suppresses actions when the open document moved past the snapshot.
    StaleDocument,
    /// Reserved: producer is the backend health/root gate, which today
    /// stays omit-only (see module docs).
    WorkspaceRootBlocked,
    /// Reserved: producer is the quickfix repair-action fix-site resolution
    /// (#1904).
    FixSiteUnavailable,
    /// Reserved: producer is the quickfix exact-replacement availability
    /// check (#1904).
    ExactReplacementUnavailable,
    /// Reserved: producer is the quickfix ambiguous fix-site check (#1904).
    AmbiguousFixSite,
    /// Reserved: producer is the quickfix allowed-edit-surface check
    /// (#1904).
    OutsideAllowedEditSurface,
    /// Reserved: producer is the session-configuration validation path.
    ConfigurationInvalid,
}

impl ActionDisabledReason {
    /// Machine-readable reason string for `data.disabled_reason`.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::StaleSnapshot => "stale_snapshot",
            Self::StaleDocument => "stale_document",
            Self::ClientCapabilityMissing => "client_capability_missing",
            Self::VerificationRouteUnavailable => "verification_route_unavailable",
            Self::ReceiptRouteUnavailable => "receipt_route_unavailable",
            Self::PreviewOrStaticLimitation => "preview_or_static_limitation",
            Self::WorkspaceRootBlocked => "workspace_root_blocked",
            Self::FixSiteUnavailable => "fix_site_unavailable",
            Self::ExactReplacementUnavailable => "exact_replacement_unavailable",
            Self::AmbiguousFixSite => "ambiguous_fix_site",
            Self::OutsideAllowedEditSurface => "outside_allowed_edit_surface",
            Self::ConfigurationInvalid => "configuration_invalid",
        }
    }

    /// Human-readable `CodeAction.disabled.reason` text (LSP surfaces it in
    /// the editor UI). Advisory, conservative wording.
    pub(super) fn human_reason(self) -> &'static str {
        match self {
            Self::StaleSnapshot => {
                "Analysis snapshot changed; refresh analysis before using this action"
            }
            Self::ClientCapabilityMissing => {
                "This client did not advertise the command this action needs"
            }
            Self::VerificationRouteUnavailable => {
                "No safe verification command is available in this gap record"
            }
            Self::ReceiptRouteUnavailable => {
                "No safe receipt command is available in this gap record"
            }
            Self::PreviewOrStaticLimitation => {
                "Static preview limitation: no cross-language repair route is available"
            }
            Self::StaleDocument => "The document changed after this analysis snapshot",
            Self::WorkspaceRootBlocked => "The workspace root is blocked for analysis",
            Self::FixSiteUnavailable => "No fix site is available for this finding",
            Self::ExactReplacementUnavailable => "No exact replacement is available",
            Self::AmbiguousFixSite => "The fix site is ambiguous",
            Self::OutsideAllowedEditSurface => "The fix is outside the allowed edit surface",
            Self::ConfigurationInvalid => "The session configuration is invalid",
        }
    }
}

/// Reasons a real producer emits today (#1892).
pub(super) const EMITTED_DISABLED_REASONS: &[ActionDisabledReason] = &[
    ActionDisabledReason::StaleSnapshot,
    ActionDisabledReason::ClientCapabilityMissing,
    ActionDisabledReason::VerificationRouteUnavailable,
    ActionDisabledReason::ReceiptRouteUnavailable,
    ActionDisabledReason::PreviewOrStaticLimitation,
];

/// Reserved reasons with named future producers. Never emitted until the
/// producer lands (real producers only).
pub(super) const RESERVED_DISABLED_REASONS: &[ActionDisabledReason] = &[
    ActionDisabledReason::StaleDocument,
    ActionDisabledReason::WorkspaceRootBlocked,
    ActionDisabledReason::FixSiteUnavailable,
    ActionDisabledReason::ExactReplacementUnavailable,
    ActionDisabledReason::AmbiguousFixSite,
    ActionDisabledReason::OutsideAllowedEditSurface,
    ActionDisabledReason::ConfigurationInvalid,
];

/// Fail-closed emit guard: a disabled action may only carry a reason with a
/// real producer. A reserved reason stays unemittable until its producer
/// lands; the redundant reserved check pins the disjointness invariant at
/// the emit site, not just in tests.
pub(super) fn disabled_reason_emittable(reason: ActionDisabledReason) -> bool {
    EMITTED_DISABLED_REASONS.contains(&reason) && !RESERVED_DISABLED_REASONS.contains(&reason)
}

/// Bounded action classes mirroring the advertised kind hierarchy
/// (`quickfix.ripr` / `source.ripr.*`). `verify` is reserved for the
/// advertised-but-unemitted `source.ripr.verify` kind.
pub(super) fn action_class_for_kind(kind: &str) -> &'static str {
    match kind {
        "source.ripr.navigate" => "navigate",
        "source.ripr.refresh" => "refresh",
        "source.ripr.verify" => "verify",
        _ => "inspect",
    }
}

/// Inputs for one action's `data` payload. Identity fields come from the
/// addressed diagnostic's producer-owned `data` block and diagnostic code;
/// nothing is re-derived from title text.
pub(super) struct ActionDataInputs<'a> {
    /// The action's `CodeActionKind` string (e.g. `source.ripr.inspect`).
    pub(super) action_kind: &'a str,
    /// The action's stable snake_case machine name (e.g.
    /// `copy_gap_repair_packet`) — never title text. Several constructors
    /// share a command id and kind, so the name is what keeps their
    /// `action_id` fingerprints distinct on one diagnostic.
    pub(super) action_name: &'a str,
    /// The command the action executes (or would execute, when disabled).
    pub(super) command_id: &'a str,
    /// The capability the client must have for the action to run: the
    /// client-command id, or `"server"` for server-executed commands.
    pub(super) required_client_capability: &'a str,
    /// The diagnostic the action addresses, when it addresses one.
    pub(super) diagnostic: Option<&'a Diagnostic>,
    /// The snapshot's `stable_id()` input identity, when a snapshot exists.
    pub(super) input_identity: Option<String>,
    /// The disabled reason, only when the action is disabled.
    pub(super) disabled_reason: Option<ActionDisabledReason>,
}

/// Builds the bounded, versioned `CodeAction.data` payload (#1892). The
/// `action_id` fingerprints the action class, the canonical addressed
/// identity, the command id, and the action's stable machine name — never
/// title text — so it is stable across calls and distinct across actions,
/// including the constructors that share one command id on one diagnostic.
pub(super) fn action_data(inputs: &ActionDataInputs<'_>) -> Value {
    let action_class = action_class_for_kind(inputs.action_kind);
    let canonical_identity = canonical_identity(inputs.diagnostic).unwrap_or_default();
    let action_id = crate::config::config_fingerprint(&format!(
        "{action_class}|{canonical_identity}|{command_id}|{action_name}",
        command_id = inputs.command_id,
        action_name = inputs.action_name
    ));
    let mut payload = serde_json::Map::new();
    payload.insert(
        "schema_version".to_string(),
        Value::String(RIPR_CODE_ACTION_DATA_SCHEMA_VERSION.to_string()),
    );
    payload.insert("action_id".to_string(), Value::String(action_id));
    payload.insert(
        "action_class".to_string(),
        Value::String(action_class.to_string()),
    );
    payload.insert(
        "action_kind".to_string(),
        Value::String(inputs.action_kind.to_string()),
    );
    payload.insert(
        "action_name".to_string(),
        Value::String(inputs.action_name.to_string()),
    );
    if let Some(diagnostic) = inputs.diagnostic {
        if let Some(code) = diagnostic_code_id(diagnostic) {
            payload.insert("diagnostic_id".to_string(), Value::String(code));
        }
        if let Some(data) = diagnostic.data.as_ref().and_then(Value::as_object) {
            for key in ["canonical_gap_id", "gap_id", "seam_id", "finding_id"] {
                if let Some(value) = data
                    .get(key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    payload.insert(key.to_string(), Value::String(value.to_string()));
                }
            }
        }
    }
    if let Some(identity) = &inputs.input_identity {
        payload.insert(
            "input_identity".to_string(),
            Value::String(identity.clone()),
        );
    }
    payload.insert(
        "required_client_capability".to_string(),
        Value::String(inputs.required_client_capability.to_string()),
    );
    if let Some(reason) = inputs.disabled_reason {
        payload.insert(
            "disabled_reason".to_string(),
            Value::String(reason.as_str().to_string()),
        );
    }
    Value::Object(payload)
}

/// The canonical addressed identity for the `action_id` fingerprint: the
/// first producer-owned identity present on the diagnostic.
fn canonical_identity(diagnostic: Option<&Diagnostic>) -> Option<String> {
    let data = diagnostic?.data.as_ref()?.as_object()?;
    for key in ["canonical_gap_id", "gap_id", "seam_id", "finding_id"] {
        if let Some(value) = data
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_string());
        }
    }
    None
}

/// The diagnostic's producer-owned code (e.g. `ripr-gap-...`), when set.
fn diagnostic_code_id(diagnostic: &Diagnostic) -> Option<String> {
    match diagnostic.code.as_ref()? {
        NumberOrString::String(code) => {
            let code = code.trim();
            if code.is_empty() {
                None
            } else {
                Some(code.to_string())
            }
        }
        NumberOrString::Number(code) => Some(code.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp_server::ls_types::{DiagnosticSeverity, Position, Range};

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
            code: Some(NumberOrString::String(
                "ripr-gap-MissingBoundaryAssertion".to_string(),
            )),
            code_description: None,
            source: Some("ripr".to_string()),
            message: "ripr gap: MissingBoundaryAssertion".to_string(),
            related_information: None,
            tags: None,
            data: Some(serde_json::json!({
                "gap_id": "gap:pr:pricing:threshold-boundary",
                "canonical_gap_id": "gap:rust:pricing:threshold-boundary",
                "seam_id": "seam-a",
                "finding_id": "finding-a"
            })),
        }
    }

    fn inputs_for<'a>(diagnostic: Option<&'a Diagnostic>) -> ActionDataInputs<'a> {
        ActionDataInputs {
            action_kind: "source.ripr.inspect",
            action_name: "copy_gap_repair_packet",
            command_id: "ripr.copyContext",
            required_client_capability: "ripr.copyContext",
            diagnostic,
            input_identity: Some("input:fnv1a64:0123456789abcdef".to_string()),
            disabled_reason: None,
        }
    }

    #[test]
    fn closed_disabled_reason_vocabulary_is_disjoint_unique_and_complete() -> Result<(), String> {
        let all = [
            ActionDisabledReason::StaleSnapshot,
            ActionDisabledReason::StaleDocument,
            ActionDisabledReason::ClientCapabilityMissing,
            ActionDisabledReason::VerificationRouteUnavailable,
            ActionDisabledReason::ReceiptRouteUnavailable,
            ActionDisabledReason::PreviewOrStaticLimitation,
            ActionDisabledReason::WorkspaceRootBlocked,
            ActionDisabledReason::FixSiteUnavailable,
            ActionDisabledReason::ExactReplacementUnavailable,
            ActionDisabledReason::AmbiguousFixSite,
            ActionDisabledReason::OutsideAllowedEditSurface,
            ActionDisabledReason::ConfigurationInvalid,
        ];
        let mut names = std::collections::BTreeSet::new();
        for reason in all {
            if !names.insert(reason.as_str()) {
                return Err(format!("duplicate reason name: {}", reason.as_str()));
            }
            let emitted = EMITTED_DISABLED_REASONS.contains(&reason);
            let reserved = RESERVED_DISABLED_REASONS.contains(&reason);
            if emitted == reserved {
                return Err(format!(
                    "reason {} must sit in exactly one of emitted/reserved",
                    reason.as_str()
                ));
            }
            if disabled_reason_emittable(reason) != emitted {
                return Err(format!(
                    "emit guard disagrees with the emitted set for {}",
                    reason.as_str()
                ));
            }
        }
        if names.len() != EMITTED_DISABLED_REASONS.len() + RESERVED_DISABLED_REASONS.len() {
            return Err("emitted + reserved must cover the whole vocabulary".to_string());
        }
        Ok(())
    }

    #[test]
    fn action_data_payload_shape_is_bounded_and_versioned() -> Result<(), String> {
        let diagnostic = gap_diagnostic();
        let payload = action_data(&inputs_for(Some(&diagnostic)));
        let object = payload
            .as_object()
            .ok_or_else(|| "payload must be an object".to_string())?;
        let expected_keys = [
            "action_class",
            "action_id",
            "action_kind",
            "action_name",
            "canonical_gap_id",
            "diagnostic_id",
            "finding_id",
            "gap_id",
            "input_identity",
            "required_client_capability",
            "schema_version",
            "seam_id",
        ];
        let actual_keys = object.keys().map(String::as_str).collect::<Vec<_>>();
        if actual_keys != expected_keys {
            return Err(format!(
                "payload keys drifted: expected {expected_keys:?}, got {actual_keys:?}"
            ));
        }
        if payload["schema_version"] != RIPR_CODE_ACTION_DATA_SCHEMA_VERSION {
            return Err(format!("schema version drifted: {payload}"));
        }
        if payload["action_class"] != "inspect" {
            return Err(format!("action class drifted: {payload}"));
        }
        if payload["action_kind"] != "source.ripr.inspect" {
            return Err(format!("action kind drifted: {payload}"));
        }
        if payload["action_name"] != "copy_gap_repair_packet" {
            return Err(format!("action name missing: {payload}"));
        }
        if payload["diagnostic_id"] != "ripr-gap-MissingBoundaryAssertion" {
            return Err(format!("diagnostic id missing: {payload}"));
        }
        if payload["canonical_gap_id"] != "gap:rust:pricing:threshold-boundary" {
            return Err(format!("canonical gap id missing: {payload}"));
        }
        if payload["input_identity"] != "input:fnv1a64:0123456789abcdef" {
            return Err(format!("input identity missing: {payload}"));
        }
        if payload.get("disabled_reason").is_some() {
            return Err("enabled actions must not carry disabled_reason".to_string());
        }
        let serialized = payload.to_string();
        // The needles are built programmatically so check-local-context does
        // not flag a drive-letter prefix literal in this source file (same
        // precedent as app/annotations.rs).
        const BACKSLASH: char = '\\';
        if serialized.contains("/workspace") {
            return Err(format!("payload leaks an absolute path: {serialized}"));
        }
        let unc_prefix = format!("{BACKSLASH}{BACKSLASH}");
        if serialized.contains(&unc_prefix) {
            return Err(format!("payload leaks a UNC path: {serialized}"));
        }
        for letter in b'A'..=b'Z' {
            let letter = char::from(letter);
            for separator in [BACKSLASH, '/'] {
                let drive_prefix = format!("{letter}:{separator}");
                if serialized.contains(&drive_prefix) {
                    return Err(format!(
                        "payload leaks a drive-letter path ({drive_prefix}): {serialized}"
                    ));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn action_id_is_deterministic_and_distinct_across_actions() -> Result<(), String> {
        let diagnostic = gap_diagnostic();
        let first = action_data(&inputs_for(Some(&diagnostic)));
        let second = action_data(&inputs_for(Some(&diagnostic)));
        if first["action_id"] != second["action_id"] {
            return Err("action_id must be stable across calls".to_string());
        }
        let mut other_command = inputs_for(Some(&diagnostic));
        other_command.command_id = "ripr.copyAgentVerifyCommand";
        let other_command = action_data(&other_command);
        if first["action_id"] == other_command["action_id"] {
            return Err("action_id must differ across commands".to_string());
        }
        let mut other_kind = inputs_for(Some(&diagnostic));
        other_kind.action_kind = "source.ripr.navigate";
        let other_kind = action_data(&other_kind);
        if first["action_id"] == other_kind["action_id"] {
            return Err("action_id must differ across action classes".to_string());
        }
        let mut other_name = inputs_for(Some(&diagnostic));
        other_name.action_name = "copy_first_repair_packet";
        let other_name = action_data(&other_name);
        if first["action_id"] == other_name["action_id"] {
            return Err(
                "action_id must differ across action names sharing one command".to_string(),
            );
        }
        let no_diagnostic = action_data(&inputs_for(None));
        if first["action_id"] == no_diagnostic["action_id"] {
            return Err("action_id must differ across addressed identities".to_string());
        }
        let action_id = first["action_id"]
            .as_str()
            .ok_or_else(|| "action_id must be a string".to_string())?;
        if !action_id.starts_with("fnv1a64:") {
            return Err(format!(
                "action_id must be a config fingerprint: {action_id}"
            ));
        }
        Ok(())
    }

    #[test]
    fn disabled_reason_is_included_only_when_disabled() -> Result<(), String> {
        let diagnostic = gap_diagnostic();
        let mut inputs = inputs_for(Some(&diagnostic));
        inputs.disabled_reason = Some(ActionDisabledReason::ClientCapabilityMissing);
        let payload = action_data(&inputs);
        if payload["disabled_reason"] != "client_capability_missing" {
            return Err(format!("disabled reason missing: {payload}"));
        }
        Ok(())
    }
}
