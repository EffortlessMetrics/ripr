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
//! Bounded RIPR-owned custom surfaces in this slice:
//!
//! - `ripr/listActionableItems`: the one live `riprAgent/*` request (#1603).
//!   Its handler reads no client-supplied fields, so the bound caps the
//!   serialized `params` value as one opaque blob; any larger payload is
//!   rejected before handler work begins. Additional riprAgent handlers must
//!   register their own typed bounds here before they land.
//! - `textDocument/didOpen` / `didSave` document text: bounded by the
//!   transport message cap, which was sized for exactly this class.
//!
//! Surfaces intentionally not bounded here: none — every live RIPR-owned
//! typed surface is listed above.

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

/// Maximum serialized size estimate for `ripr/listActionableItems` params.
/// The handler reads no client-supplied fields (#1603: the response is a pure
/// transform of the committed analysis snapshot), so the bound treats params
/// as one opaque blob; this still stops an attacker-controlled payload from
/// being held, walked, or echoed through the handler.
pub(super) const MAX_LIST_ACTIONABLE_ITEMS_PARAMS_BYTES: usize = 16 * 1024;

/// Depth guard for the size estimator. Parsed values are already capped at
/// serde_json's default recursion limit (128); anything deeper cannot have
/// come off the wire and is treated as over-budget.
const MAX_SIZE_ESTIMATE_DEPTH: usize = 256;

pub(super) fn check_initialization_options(options: Option<&Value>) -> Result<(), LspError> {
    let Some(options) = options else {
        return Ok(());
    };
    if !JsonSizeBudget::new(MAX_INITIALIZATION_OPTIONS_BYTES).admits(options, 0) {
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
    let mut budget = JsonSizeBudget::new(MAX_EXECUTE_COMMAND_ARGUMENT_BYTES);
    if !budget.admits_values(arguments, 0) {
        return Err(LspError::invalid_params(format!(
            "ripr lsp payload bound: executeCommand arguments exceed {MAX_EXECUTE_COMMAND_ARGUMENT_BYTES} bytes"
        )));
    }
    Ok(())
}

/// Bound `ripr/listActionableItems` params at handler entry, before any
/// snapshot access or early-return fast path (#2034). The handler reads no
/// client-supplied fields, so the check caps the params blob as a whole
/// rather than per-field; the rejection names only the bound and its value.
pub(super) fn check_list_actionable_items_params(params: &LSPAny) -> Result<(), LspError> {
    let mut budget = JsonSizeBudget::new(MAX_LIST_ACTIONABLE_ITEMS_PARAMS_BYTES);
    if !budget.admits(params, 0) {
        return Err(LspError::invalid_params(format!(
            "ripr lsp payload bound: ripr/listActionableItems params exceed {MAX_LIST_ACTIONABLE_ITEMS_PARAMS_BYTES} bytes"
        )));
    }
    Ok(())
}

/// Allocation-free predicate for the existing decoded-JSON size estimate.
/// The accounting is unchanged: scalar allowance, UTF-8 string/key bytes,
/// and container entry counts. This is not an exact wire-size calculation.
/// Stop at the first over-budget charge rather than walking a rejected tail.
struct JsonSizeBudget {
    remaining: usize,
    #[cfg(test)]
    visited_values: usize,
}

impl JsonSizeBudget {
    fn new(limit: usize) -> Self {
        Self {
            remaining: limit,
            #[cfg(test)]
            visited_values: 0,
        }
    }

    fn charge(&mut self, size: usize) -> bool {
        let Some(remaining) = self.remaining.checked_sub(size) else {
            return false;
        };
        self.remaining = remaining;
        true
    }

    fn admits_values(&mut self, values: &[Value], depth: usize) -> bool {
        values.iter().all(|value| self.admits(value, depth))
    }

    fn admits(&mut self, value: &Value, depth: usize) -> bool {
        #[cfg(test)]
        {
            self.visited_values += 1;
        }
        if depth > MAX_SIZE_ESTIMATE_DEPTH {
            return false;
        }
        match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => self.charge(16),
            Value::String(text) => self.charge(text.len()),
            Value::Array(items) => self.charge(items.len()) && self.admits_values(items, depth + 1),
            Value::Object(entries) => {
                self.charge(entries.len())
                    && entries
                        .iter()
                        .all(|(key, item)| self.charge(key.len()) && self.admits(item, depth + 1))
            }
        }
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
    fn list_actionable_items_params_bounds() -> Result<(), String> {
        // The handler reads no client-supplied fields, so null and any
        // small payload pass; only the serialized size budget is enforced.
        check_list_actionable_items_params(&serde_json::Value::Null)
            .map_err(|err| format!("null params must pass: {err}"))?;
        check_list_actionable_items_params(&serde_json::json!({}))
            .map_err(|err| format!("empty object params must pass: {err}"))?;

        let exact = serde_json::Value::String("x".repeat(MAX_LIST_ACTIONABLE_ITEMS_PARAMS_BYTES));
        check_list_actionable_items_params(&exact)
            .map_err(|err| format!("exact params bound must pass: {err}"))?;

        let oversized =
            serde_json::Value::String("x".repeat(MAX_LIST_ACTIONABLE_ITEMS_PARAMS_BYTES + 1));
        let error = check_list_actionable_items_params(&oversized)
            .err()
            .ok_or("over-budget params must fail")?;
        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert_eq!(
            error.message,
            "ripr lsp payload bound: ripr/listActionableItems params exceed 16384 bytes"
        );
        Ok(())
    }

    #[test]
    fn size_estimate_is_saturating_and_bounded() {
        let mut scalar = JsonSizeBudget::new(16);
        assert!(scalar.admits(&Value::Null, 0));
        assert_eq!(scalar.remaining, 0);
        let mut text = JsonSizeBudget::new(4);
        assert!(text.admits(&serde_json::json!("abcd"), 0));
        assert_eq!(text.remaining, 0);
        let nested = serde_json::json!({"a": ["x", {"b": 1}]});
        let mut exact = JsonSizeBudget::new(23);
        assert!(exact.admits(&nested, 0));
        assert_eq!(exact.remaining, 0);
        assert!(!JsonSizeBudget::new(22).admits(&nested, 0));
        assert!(!JsonSizeBudget::new(64).charge(usize::MAX));
    }

    #[test]
    fn exact_payload_bounds_are_inclusive_and_shared() -> Result<(), String> {
        let exact = Value::String("x".repeat(MAX_INITIALIZATION_OPTIONS_BYTES));
        check_initialization_options(Some(&exact))
            .map_err(|err| format!("exact initialization bound must pass: {err}"))?;
        let oversized = Value::String("x".repeat(MAX_INITIALIZATION_OPTIONS_BYTES + 1));
        let error = check_initialization_options(Some(&oversized))
            .err()
            .ok_or("over-budget initialization must fail")?;
        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert_eq!(
            error.message,
            "ripr lsp payload bound: initialization_options exceeds 65536 bytes"
        );

        let half = MAX_EXECUTE_COMMAND_ARGUMENT_BYTES / 2;
        let mut arguments = vec![
            Value::String("x".repeat(half)),
            Value::String("y".repeat(MAX_EXECUTE_COMMAND_ARGUMENT_BYTES - half)),
        ];
        check_execute_command_arguments(&arguments)
            .map_err(|err| format!("exact shared argument bound must pass: {err}"))?;
        arguments.push(Value::String("z".to_string()));
        let error = check_execute_command_arguments(&arguments)
            .err()
            .ok_or("aggregate over-budget arguments must fail")?;
        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert_eq!(
            error.message,
            "ripr lsp payload bound: executeCommand arguments exceed 65536 bytes"
        );
        check_execute_command_arguments(&[])
            .map_err(|err| format!("empty arguments must pass: {err}"))?;
        Ok(())
    }

    // Independent, deliberately exhaustive reference for the pre-change
    // accounting. Keep this out of production: its full traversal is the
    // work regression that the visit-count controls below discriminate.
    fn legacy_size_at(value: &Value, depth: usize) -> usize {
        if depth > MAX_SIZE_ESTIMATE_DEPTH {
            return usize::MAX;
        }
        match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => 16,
            Value::String(text) => text.len(),
            Value::Array(items) => items.iter().fold(items.len(), |total, item| {
                total.saturating_add(legacy_size_at(item, depth + 1))
            }),
            Value::Object(entries) => entries.iter().fold(entries.len(), |total, (key, item)| {
                total
                    .saturating_add(key.len())
                    .saturating_add(legacy_size_at(item, depth + 1))
            }),
        }
    }

    fn nested_value(depth: usize) -> Value {
        (0..depth).fold(Value::String("x".to_string()), |value, _| {
            Value::Array(vec![value])
        })
    }

    #[test]
    fn budget_matches_legacy_acceptance_across_value_shapes() {
        let scalars = vec![
            Value::Null,
            Value::Bool(true),
            serde_json::json!(-1),
            serde_json::json!(u64::MAX),
            serde_json::json!(1.25),
            Value::String(String::new()),
            Value::String("é\\\n\"".to_string()),
        ];
        let mut cases = scalars.clone();
        cases.push(Value::Array(Vec::new()));
        cases.push(serde_json::json!({}));
        for value in scalars {
            cases.push(Value::Array(vec![value.clone(), Value::Null]));
            cases.push(serde_json::json!({"key": value}));
        }
        cases.push(Value::String("x".repeat(MAX_INITIALIZATION_OPTIONS_BYTES)));
        cases.push(Value::String(
            "x".repeat(MAX_INITIALIZATION_OPTIONS_BYTES + 1),
        ));
        cases.push(nested_value(MAX_SIZE_ESTIMATE_DEPTH));
        cases.push(nested_value(MAX_SIZE_ESTIMATE_DEPTH + 1));
        for (index, value) in cases.iter().enumerate() {
            let estimate = legacy_size_at(value, 0);
            for limit in [
                0,
                1,
                3,
                4,
                15,
                16,
                17,
                23,
                24,
                63,
                64,
                1024,
                MAX_INITIALIZATION_OPTIONS_BYTES - 1,
                MAX_INITIALIZATION_OPTIONS_BYTES,
            ] {
                assert_eq!(
                    JsonSizeBudget::new(limit).admits(value, 0),
                    estimate <= limit,
                    "acceptance drift in case {index} at budget {limit}"
                );
            }
        }
    }

    #[test]
    fn oversized_container_skips_all_children() {
        let value = Value::Array(vec![Value::Null; MAX_INITIALIZATION_OPTIONS_BYTES + 1]);
        let mut budget = JsonSizeBudget::new(MAX_INITIALIZATION_OPTIONS_BYTES);
        assert!(!budget.admits(&value, 0));
        assert_eq!(
            budget.visited_values, 1,
            "container length decides rejection"
        );
    }

    #[test]
    fn first_oversized_array_value_skips_the_tail() {
        for tail_len in [0, 1, 16, 4096] {
            let mut items = vec![Value::String(
                "x".repeat(MAX_INITIALIZATION_OPTIONS_BYTES + 1),
            )];
            items.extend(std::iter::repeat_n(Value::Null, tail_len));
            let mut budget = JsonSizeBudget::new(MAX_INITIALIZATION_OPTIONS_BYTES);
            assert!(!budget.admits(&Value::Array(items), 0));
            assert_eq!(
                budget.visited_values, 2,
                "must visit only the container and rejecting child, not {tail_len} later values"
            );
        }
    }

    #[test]
    fn oversized_object_key_skips_its_value() {
        let value = Value::Object(
            [(
                "k".repeat(MAX_INITIALIZATION_OPTIONS_BYTES),
                Value::Array(vec![Value::Null; 4096]),
            )]
            .into_iter()
            .collect(),
        );
        let mut budget = JsonSizeBudget::new(MAX_INITIALIZATION_OPTIONS_BYTES);
        assert!(!budget.admits(&value, 0));
        assert_eq!(
            budget.visited_values, 1,
            "over-budget key skips its subtree"
        );
    }

    #[test]
    fn first_oversized_argument_skips_later_arguments() {
        let mut arguments = vec![Value::String(
            "x".repeat(MAX_EXECUTE_COMMAND_ARGUMENT_BYTES + 1),
        )];
        arguments.extend(std::iter::repeat_n(
            Value::Array(vec![Value::Null; 4096]),
            MAX_EXECUTE_COMMAND_ARGUMENTS - 1,
        ));
        let mut budget = JsonSizeBudget::new(MAX_EXECUTE_COMMAND_ARGUMENT_BYTES);
        assert!(!budget.admits_values(&arguments, 0));
        assert_eq!(
            budget.visited_values, 1,
            "later argument trees are not visited"
        );
    }

    #[test]
    fn depth_guard_preserves_the_exact_boundary() {
        let at_limit = nested_value(MAX_SIZE_ESTIMATE_DEPTH);
        assert!(JsonSizeBudget::new(MAX_INITIALIZATION_OPTIONS_BYTES).admits(&at_limit, 0));
        let over_limit = nested_value(MAX_SIZE_ESTIMATE_DEPTH + 1);
        let mut budget = JsonSizeBudget::new(MAX_INITIALIZATION_OPTIONS_BYTES);
        assert!(!budget.admits(&over_limit, 0));
        assert_eq!(budget.visited_values, MAX_SIZE_ESTIMATE_DEPTH + 2);
    }
}
