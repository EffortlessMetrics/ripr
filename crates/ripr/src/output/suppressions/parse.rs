//! Manifest parsing entry point.
//!
//! Thin orchestrator that scans lines, rotates `[[suppressions]]`
//! blocks, finalizes them via [`build`], and runs the post-pass
//! duplicate-selector scan.

use super::build::{PendingSuppression, assign_field, finalize_suppression};
use super::{SUPPRESSIONS_PATH, SuppressionEntry, SuppressionKind};

/// Pure parser. Returns the parsed entries plus a list of structural
/// violations. Mirrors the validation style of
/// `parse_test_intent_manifest` and `parse_static_language_allowlist` in
/// `xtask`: every required field is checked, blank values are rejected,
/// unknown fields fail loudly, and absolute / backslash paths are
/// rejected so the file stays portable across machines.
pub fn parse_suppressions_manifest(text: &str) -> (Vec<SuppressionEntry>, Vec<String>) {
    let mut state = ParseState::default();

    for (index, line) in text.lines().enumerate() {
        dispatch_line(line, index + 1, &mut state);
    }

    if let Some(pending) = state.current.take() {
        finalize_suppression(pending, &mut state.entries, &mut state.violations);
    }

    if !state.schema_seen {
        state.violations.push(format!(
            "{SUPPRESSIONS_PATH} is missing required `schema_version = 1` header"
        ));
    }

    detect_duplicate_selectors(&state.entries, &mut state.violations);

    (state.entries, state.violations)
}

#[derive(Default)]
struct ParseState {
    entries: Vec<SuppressionEntry>,
    violations: Vec<String>,
    schema_seen: bool,
    current: Option<PendingSuppression>,
}

fn dispatch_line(line: &str, line_number: usize, state: &mut ParseState) {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return;
    }
    if trimmed == "[[suppressions]]" {
        if let Some(pending) = state.current.take() {
            finalize_suppression(pending, &mut state.entries, &mut state.violations);
        }
        state.current = Some(PendingSuppression::new(line_number));
        return;
    }
    let Some((key, raw)) = trimmed.split_once('=') else {
        state.violations.push(format!(
            "{SUPPRESSIONS_PATH}:{line_number} expected `key = value`"
        ));
        return;
    };
    let key = key.trim();
    let raw = raw.trim();
    if let Some(pending) = state.current.as_mut() {
        assign_pending_field(pending, key, raw, line_number, &mut state.violations);
    } else if key == "schema_version" {
        state.schema_seen = true;
        match raw.parse::<u32>() {
            Ok(1) => {}
            Ok(other) => state.violations.push(format!(
                "{SUPPRESSIONS_PATH}:{line_number} schema_version = {other} is not supported (expected 1)"
            )),
            Err(_) => state.violations.push(format!(
                "{SUPPRESSIONS_PATH}:{line_number} schema_version must be an integer literal"
            )),
        }
    } else {
        state.violations.push(format!(
            "{SUPPRESSIONS_PATH}:{line_number} unsupported top-level field `{key}`"
        ));
    }
}

fn assign_pending_field(
    pending: &mut PendingSuppression,
    key: &str,
    raw: &str,
    line_number: usize,
    violations: &mut Vec<String>,
) {
    match key {
        "kind" => assign_field(raw, line_number, violations, |parsed| {
            pending.kind = Some((parsed, line_number));
        }),
        "finding_id" => assign_field(raw, line_number, violations, |parsed| {
            pending.finding_id = Some((parsed, line_number));
        }),
        "test" => assign_field(raw, line_number, violations, |parsed| {
            pending.test = Some((parsed, line_number));
        }),
        "path" => assign_field(raw, line_number, violations, |parsed| {
            pending.path = Some((parsed, line_number));
        }),
        "reason" => assign_field(raw, line_number, violations, |parsed| {
            pending.reason = Some((parsed, line_number));
        }),
        "owner" => assign_field(raw, line_number, violations, |parsed| {
            pending.owner = Some((parsed, line_number));
        }),
        "expires" => assign_field(raw, line_number, violations, |parsed| {
            pending.expires = Some((parsed, line_number));
        }),
        "scope" => assign_field(raw, line_number, violations, |parsed| {
            pending.scope = Some((parsed, line_number));
        }),
        "created_at" => assign_field(raw, line_number, violations, |parsed| {
            pending.created_at = Some((parsed, line_number));
        }),
        "last_seen" => assign_field(raw, line_number, violations, |parsed| {
            pending.last_seen = Some((parsed, line_number));
        }),
        "review_by" => assign_field(raw, line_number, violations, |parsed| {
            pending.review_by = Some((parsed, line_number));
        }),
        "expected_visibility" => {
            assign_field(raw, line_number, violations, |parsed| {
                pending.expected_visibility = Some((parsed, line_number));
            });
        }
        "static_class" => assign_field(raw, line_number, violations, |parsed| {
            pending.static_class = Some((parsed, line_number));
        }),
        "language" => assign_field(raw, line_number, violations, |parsed| {
            pending.language = Some((parsed, line_number));
        }),
        "language_status" => assign_field(raw, line_number, violations, |parsed| {
            pending.language_status = Some((parsed, line_number));
        }),
        _ => violations.push(format!(
            "{SUPPRESSIONS_PATH}:{line_number} unsupported `[[suppressions]]` field `{key}`"
        )),
    }
}

fn detect_duplicate_selectors(entries: &[SuppressionEntry], violations: &mut Vec<String>) {
    let mut seen_exposure: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut seen_te: std::collections::BTreeSet<(String, Option<String>)> =
        std::collections::BTreeSet::new();
    for entry in entries {
        match entry.kind {
            SuppressionKind::ExposureGap => {
                if let Some(id) = &entry.finding_id
                    && !seen_exposure.insert(id.clone())
                {
                    violations.push(format!(
                        "{SUPPRESSIONS_PATH} duplicate selector finding_id `{id}` (declared near line {})",
                        entry.block_line
                    ));
                }
            }
            SuppressionKind::TestEfficiency => {
                if let Some(test) = &entry.test {
                    let key = (test.clone(), entry.path.clone());
                    if !seen_te.insert(key) {
                        let location = match &entry.path {
                            Some(p) => format!("`{}` at `{}`", test, p),
                            None => format!("`{}`", test),
                        };
                        violations.push(format!(
                            "{SUPPRESSIONS_PATH} duplicate selector {location} (declared near line {})",
                            entry.block_line
                        ));
                    }
                }
            }
        }
    }
}
