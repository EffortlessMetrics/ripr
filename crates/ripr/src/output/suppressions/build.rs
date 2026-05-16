//! Pending-block accumulation and finalization.
//!
//! Single-block scope: holds the per-line state for one
//! `[[suppressions]]` entry, validates required and optional fields,
//! and emits a [`SuppressionEntry`] into the parsed list.

use super::{SUPPRESSIONS_PATH, SuppressionEntry, SuppressionKind, is_iso_date};

pub(super) struct PendingSuppression {
    pub(super) block_line: usize,
    pub(super) kind: Option<(String, usize)>,
    pub(super) finding_id: Option<(String, usize)>,
    pub(super) test: Option<(String, usize)>,
    pub(super) path: Option<(String, usize)>,
    pub(super) reason: Option<(String, usize)>,
    pub(super) owner: Option<(String, usize)>,
    pub(super) expires: Option<(String, usize)>,
    pub(super) scope: Option<(String, usize)>,
    pub(super) created_at: Option<(String, usize)>,
    pub(super) last_seen: Option<(String, usize)>,
    pub(super) review_by: Option<(String, usize)>,
    pub(super) expected_visibility: Option<(String, usize)>,
    pub(super) static_class: Option<(String, usize)>,
    pub(super) language: Option<(String, usize)>,
    pub(super) language_status: Option<(String, usize)>,
}

impl PendingSuppression {
    pub(super) fn new(block_line: usize) -> Self {
        Self {
            block_line,
            kind: None,
            finding_id: None,
            test: None,
            path: None,
            reason: None,
            owner: None,
            expires: None,
            scope: None,
            created_at: None,
            last_seen: None,
            review_by: None,
            expected_visibility: None,
            static_class: None,
            language: None,
            language_status: None,
        }
    }
}

pub(super) fn assign_field<F>(
    raw: &str,
    line_number: usize,
    violations: &mut Vec<String>,
    mut assign: F,
) where
    F: FnMut(String),
{
    match parse_quoted_value(raw) {
        Ok(parsed) => assign(parsed),
        Err(message) => violations.push(format!("{SUPPRESSIONS_PATH}:{line_number} {message}")),
    }
}

fn parse_quoted_value(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.len() < 2 || !trimmed.starts_with('"') || !trimmed.ends_with('"') {
        return Err(format!("expected quoted string, got `{trimmed}`"));
    }
    Ok(trimmed[1..trimmed.len() - 1].to_string())
}

pub(super) fn finalize_suppression(
    pending: PendingSuppression,
    entries: &mut Vec<SuppressionEntry>,
    violations: &mut Vec<String>,
) {
    let block_line = pending.block_line;

    let kind = match pending.kind {
        Some((value, line)) => match SuppressionKind::from_str(&value) {
            Some(kind) => Some(kind),
            None => {
                violations.push(format!(
                    "{SUPPRESSIONS_PATH}:{line} unsupported kind `{value}`; supported: {}",
                    SuppressionKind::supported().join(", ")
                ));
                None
            }
        },
        None => {
            violations.push(format!(
                "{SUPPRESSIONS_PATH}:{block_line} `[[suppressions]]` entry is missing required `kind`"
            ));
            None
        }
    };

    let owner = match pending.owner {
        Some((value, line)) => {
            if value.trim().is_empty() {
                violations.push(format!("{SUPPRESSIONS_PATH}:{line} `owner` is blank"));
                None
            } else {
                Some(value)
            }
        }
        None => {
            violations.push(format!(
                "{SUPPRESSIONS_PATH}:{block_line} `[[suppressions]]` entry is missing required `owner`"
            ));
            None
        }
    };

    let reason = match pending.reason {
        Some((value, line)) => {
            if value.trim().is_empty() {
                violations.push(format!("{SUPPRESSIONS_PATH}:{line} `reason` is blank"));
                None
            } else {
                Some(value)
            }
        }
        None => {
            violations.push(format!(
                "{SUPPRESSIONS_PATH}:{block_line} `[[suppressions]]` entry is missing required `reason`"
            ));
            None
        }
    };

    // path validation (when present): repo-relative, no backslash, no `*`.
    let path = match pending.path {
        Some((value, line)) => {
            if value.trim().is_empty() {
                violations.push(format!("{SUPPRESSIONS_PATH}:{line} `path` is empty"));
                None
            } else if value.contains('\\') {
                violations.push(format!(
                    "{SUPPRESSIONS_PATH}:{line} `path` `{value}` uses backslashes; use `/` separators"
                ));
                None
            } else if is_absolute_path(&value) {
                violations.push(format!(
                    "{SUPPRESSIONS_PATH}:{line} `path` `{value}` is absolute; entries must be repository-relative"
                ));
                None
            } else {
                Some(value)
            }
        }
        None => None,
    };

    // date validation (when present): YYYY-MM-DD literal.
    let expires = validate_optional_date("expires", pending.expires, violations);
    let created_at = validate_optional_date("created_at", pending.created_at, violations);
    let last_seen = validate_optional_date("last_seen", pending.last_seen, violations);
    let review_by = validate_optional_date("review_by", pending.review_by, violations);

    let finding_id = non_blank_selector("finding_id", pending.finding_id, violations);
    let test = non_blank_selector("test", pending.test, violations);

    if let Some(kind) = kind {
        match kind {
            SuppressionKind::ExposureGap => {
                if finding_id.is_none() {
                    violations.push(format!(
                        "{SUPPRESSIONS_PATH}:{block_line} `kind = \"exposure_gap\"` requires `finding_id`"
                    ));
                    return;
                }
                if test.is_some() {
                    violations.push(format!(
                        "{SUPPRESSIONS_PATH}:{block_line} `kind = \"exposure_gap\"` does not accept `test`"
                    ));
                    return;
                }
            }
            SuppressionKind::TestEfficiency => {
                if test.is_none() {
                    violations.push(format!(
                        "{SUPPRESSIONS_PATH}:{block_line} `kind = \"test_efficiency\"` requires `test`"
                    ));
                    return;
                }
                if finding_id.is_some() {
                    violations.push(format!(
                        "{SUPPRESSIONS_PATH}:{block_line} `kind = \"test_efficiency\"` does not accept `finding_id`"
                    ));
                    return;
                }
            }
        }
        if let (Some(owner), Some(reason)) = (owner, reason) {
            entries.push(SuppressionEntry {
                kind,
                finding_id,
                test,
                path,
                reason,
                owner,
                expires,
                scope: pending.scope.map(|(value, _)| value),
                created_at,
                last_seen,
                review_by,
                expected_visibility: pending.expected_visibility.map(|(value, _)| value),
                static_class: pending.static_class.map(|(value, _)| value),
                language: pending.language.map(|(value, _)| value),
                language_status: pending.language_status.map(|(value, _)| value),
                block_line,
            });
        }
    }
}

fn validate_optional_date(
    field: &str,
    raw: Option<(String, usize)>,
    violations: &mut Vec<String>,
) -> Option<String> {
    match raw {
        Some((value, line)) => {
            if !is_iso_date(&value) {
                violations.push(format!(
                    "{SUPPRESSIONS_PATH}:{line} `{field}` `{value}` is not in YYYY-MM-DD format"
                ));
                None
            } else {
                Some(value)
            }
        }
        None => None,
    }
}

fn non_blank_selector(
    field: &str,
    raw: Option<(String, usize)>,
    violations: &mut Vec<String>,
) -> Option<String> {
    match raw {
        Some((value, line)) => {
            if value.trim().is_empty() {
                violations.push(format!("{SUPPRESSIONS_PATH}:{line} `{field}` is blank"));
                None
            } else {
                Some(value)
            }
        }
        None => None,
    }
}

fn is_absolute_path(value: &str) -> bool {
    if value.starts_with('/') {
        return true;
    }
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}
