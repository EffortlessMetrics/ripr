//! Typed ingress payload bounds for RIPR-owned LSP handlers (issue #2034).
//!
//! The framing cap in `lsp/transport_bounds.rs` bounds every raw message;
//! these validators bound the *typed* fields a structurally valid message can
//! carry so no handler allocates, iterates, or echoes attacker-controlled
//! data without a reviewed limit. Each rejection is a bounded JSON-RPC
//! `-32602` (InvalidParams) error whose message names the bound and its
//! value — never attacker input.
//!
//! Composition: rejection happens at handler entry, before any analysis,
//! refresh scheduling, Git, filesystem, or subprocess work, and before the
//! early-return fast paths so a missing analysis snapshot cannot bypass the
//! bound.
//!
//! Surfaces intentionally not bounded here:
//!
//! - `riprAgent/*`: capability negotiation only in this slice
//!   (`lsp/agent_protocol.rs`, "no riprAgent requests are implemented"), so
//!   there is no live request family to bound. When handlers land they must
//!   register bounds here first.
//! - `textDocument/didOpen` / `didSave` document text: bounded by the
//!   transport message cap, which was sized for exactly this class.

use serde_json::Value;
use tower_lsp_server::jsonrpc::Error as LspError;
use tower_lsp_server::ls_types::{LSPAny, PreviousResultId};

/// Maximum serialized size estimate for `initialize.initialization_options`.
/// Only a handful of known keys are read (`lsp/config.rs`); a larger blob is
/// never legitimate configuration.
pub(super) const MAX_INITIALIZATION_OPTIONS_BYTES: usize = 64 * 1024;

/// Maximum `workspace/diagnostic` `previousResultIds` entries. Pull
/// diagnostics send one entry per tracked document, so the cap must tolerate
/// monorepo sessions; 4096 bounds the per-request BTreeSet clone and the
/// previous-id scan without rejecting real workspaces.
pub(super) const MAX_PREVIOUS_RESULT_IDS: usize = 4096;

/// Maximum bytes for one `previousResultIds` URI.
pub(super) const MAX_PREVIOUS_RESULT_ID_URI_BYTES: usize = 4096;

/// Maximum bytes for one `previousResultIds` result-id value. Server-issued
/// result ids are short digests.
pub(super) const MAX_PREVIOUS_RESULT_ID_VALUE_BYTES: usize = 1024;

/// Maximum `workspace/executeCommand` argument entries. Every RIPR command
/// takes zero or one argument object.
pub(super) const MAX_EXECUTE_COMMAND_ARGUMENTS: usize = 8;

/// Maximum serialized size estimate across all `executeCommand` arguments.
/// Bounds every downstream identifier (gap ids, seam ids, snapshot handles)
/// transitively.
pub(super) const MAX_EXECUTE_COMMAND_ARGUMENT_BYTES: usize = 64 * 1024;

/// Depth guard for the size estimator. Parsed values are already capped at
/// serde_json's default recursion limit (128); anything deeper cannot have
/// come off the wire and is treated as over-budget.
const MAX_SIZE_ESTIMATE_DEPTH: usize = 256;

pub(super) fn check_initialization_options(options: Option<&Value>) -> Result<(), LspError> {
    let Some(options) = options else {
        return Ok(());
    };
    if json_value_size(options) > MAX_INITIALIZATION_OPTIONS_BYTES {
        return Err(LspError::invalid_params(format!(
            "ripr lsp payload bound: initialization_options exceeds {MAX_INITIALIZATION_OPTIONS_BYTES} bytes"
        )));
    }
    Ok(())
}

pub(super) fn check_previous_result_ids(ids: &[PreviousResultId]) -> Result<(), LspError> {
    if ids.len() > MAX_PREVIOUS_RESULT_IDS {
        return Err(LspError::invalid_params(format!(
            "ripr lsp payload bound: previousResultIds exceeds {MAX_PREVIOUS_RESULT_IDS} entries"
        )));
    }
    for entry in ids {
        if entry.uri.as_str().len() > MAX_PREVIOUS_RESULT_ID_URI_BYTES {
            return Err(LspError::invalid_params(format!(
                "ripr lsp payload bound: a previousResultIds uri exceeds {MAX_PREVIOUS_RESULT_ID_URI_BYTES} bytes"
            )));
        }
        if entry.value.len() > MAX_PREVIOUS_RESULT_ID_VALUE_BYTES {
            return Err(LspError::invalid_params(format!(
                "ripr lsp payload bound: a previousResultIds value exceeds {MAX_PREVIOUS_RESULT_ID_VALUE_BYTES} bytes"
            )));
        }
    }
    Ok(())
}

pub(super) fn check_execute_command_arguments(arguments: &[LSPAny]) -> Result<(), LspError> {
    if arguments.len() > MAX_EXECUTE_COMMAND_ARGUMENTS {
        return Err(LspError::invalid_params(format!(
            "ripr lsp payload bound: executeCommand arguments exceed {MAX_EXECUTE_COMMAND_ARGUMENTS} entries"
        )));
    }
    let size = arguments.iter().fold(0_usize, |total, value| {
        total.saturating_add(json_value_size(value))
    });
    if size > MAX_EXECUTE_COMMAND_ARGUMENT_BYTES {
        return Err(LspError::invalid_params(format!(
            "ripr lsp payload bound: executeCommand arguments exceed {MAX_EXECUTE_COMMAND_ARGUMENT_BYTES} bytes"
        )));
    }
    Ok(())
}

/// Allocation-free serialized-size estimate for a decoded JSON value.
/// Saturating on overflow and depth: the estimate only feeds `>` comparisons
/// against the bounds above, so saturation is fail-closed.
fn json_value_size(value: &Value) -> usize {
    json_value_size_at(value, 0)
}

fn json_value_size_at(value: &Value, depth: usize) -> usize {
    if depth > MAX_SIZE_ESTIMATE_DEPTH {
        return usize::MAX;
    }
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => 16,
        Value::String(text) => text.len(),
        Value::Array(items) => items.iter().fold(items.len(), |total, item| {
            total.saturating_add(json_value_size_at(item, depth + 1))
        }),
        Value::Object(entries) => entries.iter().fold(entries.len(), |total, (key, item)| {
            total
                .saturating_add(key.len())
                .saturating_add(json_value_size_at(item, depth + 1))
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp_server::jsonrpc::ErrorCode;

    fn assert_invalid_params(result: Result<(), LspError>) -> Result<(), String> {
        match result {
            Err(err) if err.code == ErrorCode::InvalidParams => {
                if err.message.len() > 256 {
                    return Err(format!(
                        "rejection message must be bounded: {}",
                        err.message
                    ));
                }
                Ok(())
            }
            other => Err(format!("expected InvalidParams, got: {other:?}")),
        }
    }

    #[test]
    fn initialization_options_within_bound_pass() -> Result<(), String> {
        let options = serde_json::json!({"mode": "fast", "seamDiagnostics": true});
        check_initialization_options(Some(&options))
            .map_err(|err| format!("legit options must pass: {err}"))?;
        check_initialization_options(None)
            .map_err(|err| format!("absent options must pass: {err}"))?;
        Ok(())
    }

    #[test]
    fn oversized_initialization_options_are_rejected() -> Result<(), String> {
        let options = serde_json::json!({"pad": "x".repeat(MAX_INITIALIZATION_OPTIONS_BYTES)});
        assert_invalid_params(check_initialization_options(Some(&options)))
    }

    #[test]
    fn previous_result_ids_bounds() -> Result<(), String> {
        let legit = serde_json::json!([
            {"uri": "file:///a.rs", "value": "digest-1"},
            {"uri": "file:///a.rs", "value": "digest-1"}
        ]);
        let ids: Vec<PreviousResultId> = serde_json::from_value(legit)
            .map_err(|err| format!("fixture ids must decode: {err}"))?;
        check_previous_result_ids(&ids).map_err(|err| format!("legit ids must pass: {err}"))?;

        let too_many: Vec<PreviousResultId> = (0..MAX_PREVIOUS_RESULT_IDS + 1)
            .filter_map(|index| {
                serde_json::from_value(serde_json::json!({
                    "uri": format!("file:///f{index}.rs"),
                    "value": "v"
                }))
                .ok()
            })
            .collect();
        assert_invalid_params(check_previous_result_ids(&too_many))?;

        let long_value: Vec<PreviousResultId> = serde_json::from_value(serde_json::json!([
            {"uri": "file:///a.rs", "value": "v".repeat(MAX_PREVIOUS_RESULT_ID_VALUE_BYTES + 1)}
        ]))
        .map_err(|err| format!("fixture ids must decode: {err}"))?;
        assert_invalid_params(check_previous_result_ids(&long_value))
    }

    #[test]
    fn execute_command_arguments_bounds() -> Result<(), String> {
        let legit = vec![serde_json::json!({"gap_id": "gap:rust:pricing:error_path"})];
        check_execute_command_arguments(&legit)
            .map_err(|err| format!("legit arguments must pass: {err}"))?;

        let too_many = vec![serde_json::json!({}); MAX_EXECUTE_COMMAND_ARGUMENTS + 1];
        assert_invalid_params(check_execute_command_arguments(&too_many))?;

        let too_big =
            vec![serde_json::json!({"pad": "x".repeat(MAX_EXECUTE_COMMAND_ARGUMENT_BYTES)})];
        assert_invalid_params(check_execute_command_arguments(&too_big))
    }

    #[test]
    fn size_estimate_is_saturating_and_bounded() -> Result<(), String> {
        assert_eq!(json_value_size(&Value::Null), 16);
        assert_eq!(json_value_size(&serde_json::json!("abcd")), 4);
        let nested = serde_json::json!({"a": ["x", {"b": 1}]});
        let estimate = json_value_size(&nested);
        assert!(
            estimate >= 1 + 1 + 1 + 16,
            "estimate must count entries: {estimate}"
        );
        Ok(())
    }
}
