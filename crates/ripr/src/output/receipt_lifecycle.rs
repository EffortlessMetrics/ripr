//! Shared receipt lifecycle labels for start-here surfaces.
//!
//! These labels describe whether static receipt evidence exists and how it
//! relates to the selected gap. They do not claim runtime adequacy, mutation
//! proof, gate eligibility, or merge readiness.

use serde_json::Value;

pub const RECEIPT_MISSING: &str = "receipt_missing";
pub const RECEIPT_FOUND: &str = "receipt_found";
pub const RECEIPT_STALE: &str = "receipt_stale";
pub const RECEIPT_GAP_MISMATCH: &str = "receipt_gap_mismatch";
pub const RECEIPT_MOVEMENT_IMPROVED: &str = "receipt_movement_improved";
pub const RECEIPT_MOVEMENT_UNCHANGED: &str = "receipt_movement_unchanged";
pub const RECEIPT_NOT_APPLICABLE: &str = "receipt_not_applicable";

pub fn normalize_receipt_lifecycle_state(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "missing" | "missing_receipt" | RECEIPT_MISSING => RECEIPT_MISSING.to_string(),
        "present" | "found" | RECEIPT_FOUND => RECEIPT_FOUND.to_string(),
        "stale" | "stale_receipt" | RECEIPT_STALE => RECEIPT_STALE.to_string(),
        "gap_mismatch" | "mismatch" | RECEIPT_GAP_MISMATCH => RECEIPT_GAP_MISMATCH.to_string(),
        "improved" | "receipt_improved" | "movement_improved" | RECEIPT_MOVEMENT_IMPROVED => {
            RECEIPT_MOVEMENT_IMPROVED.to_string()
        }
        "unchanged" | "receipt_unchanged" | "movement_unchanged" | RECEIPT_MOVEMENT_UNCHANGED => {
            RECEIPT_MOVEMENT_UNCHANGED.to_string()
        }
        "not_attempted"
        | "not_applicable"
        | "not_available"
        | "n/a"
        | "none"
        | RECEIPT_NOT_APPLICABLE => RECEIPT_NOT_APPLICABLE.to_string(),
        other => other.to_string(),
    }
}

pub fn receipt_lifecycle_state(raw: Option<&str>) -> String {
    raw.map(normalize_receipt_lifecycle_state)
        .unwrap_or_else(|| RECEIPT_MISSING.to_string())
}

pub fn receipt_lifecycle_state_from_movement(movement: Option<&str>) -> String {
    let Some(movement) = movement
        .map(str::trim)
        .filter(|movement| !movement.is_empty())
    else {
        return RECEIPT_FOUND.to_string();
    };
    match movement.to_ascii_lowercase().as_str() {
        "improved" | "receipt_improved" | "movement_improved" | RECEIPT_MOVEMENT_IMPROVED => {
            RECEIPT_MOVEMENT_IMPROVED.to_string()
        }
        "unchanged" | "receipt_unchanged" | "movement_unchanged" | RECEIPT_MOVEMENT_UNCHANGED => {
            RECEIPT_MOVEMENT_UNCHANGED.to_string()
        }
        "missing" | "missing_receipt" | RECEIPT_MISSING => RECEIPT_MISSING.to_string(),
        "stale" | "stale_receipt" | RECEIPT_STALE => RECEIPT_STALE.to_string(),
        "gap_mismatch" | "mismatch" | RECEIPT_GAP_MISMATCH => RECEIPT_GAP_MISMATCH.to_string(),
        "not_attempted"
        | "not_applicable"
        | "not_available"
        | "n/a"
        | "none"
        | RECEIPT_NOT_APPLICABLE => RECEIPT_NOT_APPLICABLE.to_string(),
        _ => RECEIPT_FOUND.to_string(),
    }
}

pub fn receipt_lifecycle_state_from_presence(
    receipt_present: bool,
    receipt_input_present: bool,
) -> String {
    if receipt_present {
        RECEIPT_FOUND.to_string()
    } else if receipt_input_present {
        RECEIPT_MISSING.to_string()
    } else {
        RECEIPT_NOT_APPLICABLE.to_string()
    }
}

pub fn receipt_lifecycle_state_from_receipt_value(receipt: &Value) -> String {
    if let Some(state) = string_path(receipt, &["summary", "receipt_state"])
        .or_else(|| string_path(receipt, &["receipt_state"]))
        .or_else(|| string_path(receipt, &["receipt", "state"]))
    {
        return normalize_receipt_lifecycle_state(&state);
    }

    let movement = string_path(receipt, &["provenance", "movement"])
        .or_else(|| string_path(receipt, &["static_movement", "state"]))
        .or_else(|| string_path(receipt, &["seam", "change"]))
        .or_else(|| string_path(receipt, &["summary", "next_action", "kind"]));
    receipt_lifecycle_state_from_movement(movement.as_deref())
}

pub fn receipt_lifecycle_state_is_present(state: &str) -> bool {
    matches!(
        normalize_receipt_lifecycle_state(state).as_str(),
        RECEIPT_FOUND | RECEIPT_MOVEMENT_IMPROVED | RECEIPT_MOVEMENT_UNCHANGED
    )
}

fn string_path(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalizes_legacy_presence_labels() {
        assert_eq!(normalize_receipt_lifecycle_state("present"), RECEIPT_FOUND);
        assert_eq!(
            normalize_receipt_lifecycle_state("missing"),
            RECEIPT_MISSING
        );
        assert_eq!(
            normalize_receipt_lifecycle_state("not_attempted"),
            RECEIPT_NOT_APPLICABLE
        );
    }

    #[test]
    fn maps_static_movement_to_lifecycle_state() {
        assert_eq!(
            receipt_lifecycle_state_from_movement(Some("improved")),
            RECEIPT_MOVEMENT_IMPROVED
        );
        assert_eq!(
            receipt_lifecycle_state_from_movement(Some("unchanged")),
            RECEIPT_MOVEMENT_UNCHANGED
        );
        assert_eq!(
            receipt_lifecycle_state_from_movement(Some("regressed")),
            RECEIPT_FOUND
        );
    }

    #[test]
    fn extracts_lifecycle_state_from_receipt_json() {
        let receipt = json!({
            "provenance": {"movement": "improved"},
            "summary": {"next_action": {"kind": "improved"}}
        });
        assert_eq!(
            receipt_lifecycle_state_from_receipt_value(&receipt),
            RECEIPT_MOVEMENT_IMPROVED
        );
    }
}
