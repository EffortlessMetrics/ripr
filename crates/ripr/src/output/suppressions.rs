//! Private suppressions loader for badge counting.
//!
//! `.ripr/suppressions.toml` declares accepted exceptions for static
//! findings the team has agreed to carry as known debt. Suppressed
//! findings remain visible in detailed reports — they only shift from
//! the unsuppressed badge bucket into the suppressed bucket. Every
//! entry requires `owner` and `reason`; `expires` is optional but
//! encouraged to prevent green-forever debt.
//!
//! This module is the policy substrate; it does not render anything
//! itself. The badge module consumes the parsed entries and threads
//! suppressed/expired counts into `BadgeSummary`.
//!
//! Expired entries do **not** apply (the underlying finding stays
//! unsuppressed) and surface as warnings on the badge so silently
//! "green forever" suppressions are impossible.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuppressionKind {
    /// Suppresses a static exposure-gap finding by `finding_id`.
    ExposureGap,
    /// Suppresses a test-efficiency entry by `(test, path)`.
    TestEfficiency,
}

impl SuppressionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SuppressionKind::ExposureGap => "exposure_gap",
            SuppressionKind::TestEfficiency => "test_efficiency",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "exposure_gap" => Some(Self::ExposureGap),
            "test_efficiency" => Some(Self::TestEfficiency),
            _ => None,
        }
    }

    pub fn supported() -> &'static [&'static str] {
        &["exposure_gap", "test_efficiency"]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuppressionEntry {
    pub kind: SuppressionKind,
    /// Required when `kind == ExposureGap`.
    pub finding_id: Option<String>,
    /// Required when `kind == TestEfficiency`.
    pub test: Option<String>,
    /// Optional path narrowing for `test_efficiency` selectors.
    pub path: Option<String>,
    pub reason: String,
    pub owner: String,
    /// ISO-8601 `YYYY-MM-DD` date string, validated at parse time.
    pub expires: Option<String>,
    pub block_line: usize,
}

pub const SUPPRESSIONS_PATH: &str = ".ripr/suppressions.toml";

/// Pure parser. Returns the parsed entries plus a list of structural
/// violations. Mirrors the validation style of
/// `parse_test_intent_manifest` and `parse_static_language_allowlist` in
/// `xtask`: every required field is checked, blank values are rejected,
/// unknown fields fail loudly, and absolute / backslash paths are
/// rejected so the file stays portable across machines.
pub fn parse_suppressions_manifest(text: &str) -> (Vec<SuppressionEntry>, Vec<String>) {
    let mut entries: Vec<SuppressionEntry> = Vec::new();
    let mut violations = Vec::new();
    let mut schema_seen = false;
    let mut current: Option<PendingSuppression> = None;

    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "[[suppressions]]" {
            if let Some(pending) = current.take() {
                finalize_suppression(pending, &mut entries, &mut violations);
            }
            current = Some(PendingSuppression::new(line_number));
            continue;
        }
        let Some((key, raw)) = trimmed.split_once('=') else {
            violations.push(format!(
                "{SUPPRESSIONS_PATH}:{line_number} expected `key = value`"
            ));
            continue;
        };
        let key = key.trim();
        let raw = raw.trim();
        if let Some(pending) = current.as_mut() {
            match key {
                "kind" => assign_field(raw, line_number, &mut violations, |parsed| {
                    pending.kind = Some((parsed, line_number));
                }),
                "finding_id" => assign_field(raw, line_number, &mut violations, |parsed| {
                    pending.finding_id = Some((parsed, line_number));
                }),
                "test" => assign_field(raw, line_number, &mut violations, |parsed| {
                    pending.test = Some((parsed, line_number));
                }),
                "path" => assign_field(raw, line_number, &mut violations, |parsed| {
                    pending.path = Some((parsed, line_number));
                }),
                "reason" => assign_field(raw, line_number, &mut violations, |parsed| {
                    pending.reason = Some((parsed, line_number));
                }),
                "owner" => assign_field(raw, line_number, &mut violations, |parsed| {
                    pending.owner = Some((parsed, line_number));
                }),
                "expires" => assign_field(raw, line_number, &mut violations, |parsed| {
                    pending.expires = Some((parsed, line_number));
                }),
                _ => violations.push(format!(
                    "{SUPPRESSIONS_PATH}:{line_number} unsupported `[[suppressions]]` field `{key}`"
                )),
            }
        } else if key == "schema_version" {
            schema_seen = true;
            match raw.parse::<u32>() {
                Ok(1) => {}
                Ok(other) => violations.push(format!(
                    "{SUPPRESSIONS_PATH}:{line_number} schema_version = {other} is not supported (expected 1)"
                )),
                Err(_) => violations.push(format!(
                    "{SUPPRESSIONS_PATH}:{line_number} schema_version must be an integer literal"
                )),
            }
        } else {
            violations.push(format!(
                "{SUPPRESSIONS_PATH}:{line_number} unsupported top-level field `{key}`"
            ));
        }
    }

    if let Some(pending) = current.take() {
        finalize_suppression(pending, &mut entries, &mut violations);
    }

    if !schema_seen {
        violations.push(format!(
            "{SUPPRESSIONS_PATH} is missing required `schema_version = 1` header"
        ));
    }

    let mut seen_exposure: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut seen_te: std::collections::BTreeSet<(String, Option<String>)> =
        std::collections::BTreeSet::new();
    for entry in &entries {
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

    (entries, violations)
}

struct PendingSuppression {
    block_line: usize,
    kind: Option<(String, usize)>,
    finding_id: Option<(String, usize)>,
    test: Option<(String, usize)>,
    path: Option<(String, usize)>,
    reason: Option<(String, usize)>,
    owner: Option<(String, usize)>,
    expires: Option<(String, usize)>,
}

impl PendingSuppression {
    fn new(block_line: usize) -> Self {
        Self {
            block_line,
            kind: None,
            finding_id: None,
            test: None,
            path: None,
            reason: None,
            owner: None,
            expires: None,
        }
    }
}

fn assign_field<F>(raw: &str, line_number: usize, violations: &mut Vec<String>, mut assign: F)
where
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

fn finalize_suppression(
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

    // expires validation (when present): YYYY-MM-DD literal.
    let expires = match pending.expires {
        Some((value, line)) => {
            if !is_iso_date(&value) {
                violations.push(format!(
                    "{SUPPRESSIONS_PATH}:{line} `expires` `{value}` is not in YYYY-MM-DD format"
                ));
                None
            } else {
                Some(value)
            }
        }
        None => None,
    };

    let finding_id = pending.finding_id.map(|(value, _)| value);
    let test = pending.test.map(|(value, _)| value);

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
                block_line,
            });
        }
    }
}

fn is_absolute_path(value: &str) -> bool {
    if value.starts_with('/') {
        return true;
    }
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

/// True iff `value` is exactly `YYYY-MM-DD` with valid component ranges.
pub fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !bytes[..4].iter().all(|b| b.is_ascii_digit())
        || !bytes[5..7].iter().all(|b| b.is_ascii_digit())
        || !bytes[8..10].iter().all(|b| b.is_ascii_digit())
    {
        return false;
    }
    let m: u32 = (bytes[5] - b'0') as u32 * 10 + (bytes[6] - b'0') as u32;
    let d: u32 = (bytes[8] - b'0') as u32 * 10 + (bytes[9] - b'0') as u32;
    (1..=12).contains(&m) && (1..=31).contains(&d)
}

/// Loads suppressions relative to the analyzed workspace root. Returns
/// an empty list when the file does not exist (the normal case for
/// projects with no accepted debt). Parse violations are returned via
/// `Err` so the orchestrator can surface them through the existing
/// badge-rendering error path.
pub fn load_suppressions_for_root(root: &Path) -> Result<Vec<SuppressionEntry>, Vec<String>> {
    let path = root.join(SUPPRESSIONS_PATH);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|err| vec![format!("failed to read {}: {err}", path.display())])?;
    let (entries, violations) = parse_suppressions_manifest(&text);
    if violations.is_empty() {
        Ok(entries)
    } else {
        Err(violations)
    }
}

/// Today's UTC date in `YYYY-MM-DD` form. Pure-time helper used at the
/// boundary; tests pass synthetic values directly through
/// [`apply_exposure_suppressions`] / [`apply_test_efficiency_suppressions`].
pub fn current_iso_date() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days_since_epoch = (secs / 86_400) as i64;
    let (y, m, d) = days_to_civil_date(days_since_epoch);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Converts days since 1970-01-01 to a civil `(year, month, day)`.
/// Standard algorithm (Howard Hinnant); good for any `i64` day input.
fn days_to_civil_date(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i32 + (era * 400) as i32;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m_raw = if mp < 10 { mp + 3 } else { mp - 9 };
    let m = m_raw as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// True if `expires` is set and lexicographically before `today` (strict).
/// Returns `false` for `None` (no expiry → always valid).
pub fn is_expired(expires: Option<&str>, today: &str) -> bool {
    match expires {
        Some(e) => today > e,
        None => false,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SuppressionApplication {
    /// finding_id strings whose entries are actively suppressed.
    pub suppressed_findings: std::collections::BTreeSet<String>,
    /// `(test_name, path)` pairs whose entries are actively suppressed
    /// for the test-efficiency badge surface.
    pub suppressed_tests: std::collections::BTreeSet<(String, Option<String>)>,
    /// Warnings — currently expired suppressions and unmatched selectors.
    pub warnings: Vec<String>,
}

/// Applies exposure-gap suppressions against a slice of finding ids that
/// the caller already considered "candidate exposure gaps" (i.e. the
/// classes counted by `ripr_badge_summary`). Unmatched selectors and
/// expired entries surface as warnings; expired entries are not applied
/// so they cannot silently keep the badge green.
pub fn apply_exposure_suppressions(
    candidate_finding_ids: &[String],
    suppressions: &[SuppressionEntry],
    today: &str,
) -> SuppressionApplication {
    let mut app = SuppressionApplication::default();
    let candidate_set: std::collections::BTreeSet<&str> =
        candidate_finding_ids.iter().map(String::as_str).collect();

    for entry in suppressions
        .iter()
        .filter(|e| e.kind == SuppressionKind::ExposureGap)
    {
        let Some(id) = &entry.finding_id else {
            continue;
        };
        if is_expired(entry.expires.as_deref(), today) {
            app.warnings.push(format!(
                "expired {} suppression for `{id}` (expired on {})",
                entry.kind.as_str(),
                entry.expires.as_deref().unwrap_or("unknown")
            ));
            continue;
        }
        if !candidate_set.contains(id.as_str()) {
            app.warnings.push(format!(
                "{} suppression for `{id}` did not match any current exposure-gap finding",
                entry.kind.as_str()
            ));
            continue;
        }
        app.suppressed_findings.insert(id.clone());
    }
    app
}

/// Applies test-efficiency suppressions against a slice of `(name, path)`
/// pairs from the test-efficiency report. Unmatched selectors and expired
/// entries surface as warnings; expired entries are not applied.
pub fn apply_test_efficiency_suppressions(
    candidate_entries: &[(String, String)],
    suppressions: &[SuppressionEntry],
    today: &str,
) -> SuppressionApplication {
    let mut app = SuppressionApplication::default();
    for entry in suppressions
        .iter()
        .filter(|e| e.kind == SuppressionKind::TestEfficiency)
    {
        let Some(test_name) = &entry.test else {
            continue;
        };
        let key_label = match &entry.path {
            Some(p) => format!("`{}` at `{}`", test_name, p),
            None => format!("`{}`", test_name),
        };
        if is_expired(entry.expires.as_deref(), today) {
            app.warnings.push(format!(
                "expired {} suppression for {key_label} (expired on {})",
                entry.kind.as_str(),
                entry.expires.as_deref().unwrap_or("unknown")
            ));
            continue;
        }
        let matches: Vec<&(String, String)> = candidate_entries
            .iter()
            .filter(|(name, path)| {
                name == test_name && entry.path.as_ref().map(|p| p == path).unwrap_or(true)
            })
            .collect();
        if matches.is_empty() {
            app.warnings.push(format!(
                "{} suppression for {key_label} did not match any test-efficiency entry",
                entry.kind.as_str()
            ));
            continue;
        }
        for (name, path) in matches {
            // Always store the actual entry path so name-only suppressions
            // matching multiple files dedupe per-(name, path) — i.e., the
            // count reflects distinct tests suppressed, not distinct
            // selectors.
            app.suppressed_tests
                .insert((name.clone(), Some(path.clone())));
        }
    }
    app
}

#[cfg(test)]
mod tests {
    use super::{
        SUPPRESSIONS_PATH, SuppressionApplication, SuppressionEntry, SuppressionKind,
        apply_exposure_suppressions, apply_test_efficiency_suppressions, current_iso_date,
        days_to_civil_date, is_expired, is_iso_date, parse_suppressions_manifest,
    };

    #[test]
    fn parse_accepts_well_formed_manifest_with_both_kinds() {
        let text = r#"
schema_version = 1

[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:src/pricing.rs:88:predicate"
reason = "Covered by integration test."
owner = "billing"
expires = "2026-09-01"

[[suppressions]]
kind = "test_efficiency"
test = "cli_prints_help"
path = "tests/cli.rs"
reason = "Intentionally broad CLI smoke."
owner = "devtools"
"#;
        let (entries, violations) = parse_suppressions_manifest(text);
        assert!(
            violations.is_empty(),
            "unexpected violations: {violations:?}"
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].kind, SuppressionKind::ExposureGap);
        assert_eq!(
            entries[0].finding_id.as_deref(),
            Some("probe:src/pricing.rs:88:predicate")
        );
        assert_eq!(entries[0].expires.as_deref(), Some("2026-09-01"));
        assert_eq!(entries[1].kind, SuppressionKind::TestEfficiency);
        assert_eq!(entries[1].test.as_deref(), Some("cli_prints_help"));
        assert_eq!(entries[1].path.as_deref(), Some("tests/cli.rs"));
    }

    #[test]
    fn parse_requires_schema_version() {
        let text = r#"
[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:x"
reason = "y"
owner = "z"
"#;
        let (_, violations) = parse_suppressions_manifest(text);
        assert!(violations.iter().any(|v| v.contains("schema_version = 1")));
    }

    #[test]
    fn parse_requires_kind_owner_reason() {
        let cases = [
            (
                r#"schema_version = 1

[[suppressions]]
finding_id = "probe:x"
owner = "z"
reason = "y"
"#,
                "missing required `kind`",
            ),
            (
                r#"schema_version = 1

[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:x"
reason = "y"
"#,
                "missing required `owner`",
            ),
            (
                r#"schema_version = 1

[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:x"
owner = "z"
"#,
                "missing required `reason`",
            ),
        ];
        for (text, fragment) in cases {
            let (_, violations) = parse_suppressions_manifest(text);
            assert!(
                violations.iter().any(|v| v.contains(fragment)),
                "expected `{fragment}` violation for: {text}\nviolations: {violations:?}"
            );
        }
    }

    #[test]
    fn parse_rejects_unknown_kind() {
        let text = r#"schema_version = 1

[[suppressions]]
kind = "wishful"
finding_id = "probe:x"
owner = "z"
reason = "y"
"#;
        let (_, violations) = parse_suppressions_manifest(text);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("unsupported kind `wishful`"))
        );
    }

    #[test]
    fn parse_rejects_unknown_top_level_or_block_field() {
        let text = r#"schema_version = 1
priority = "high"

[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:x"
owner = "z"
reason = "y"
priority = "high"
"#;
        let (_, violations) = parse_suppressions_manifest(text);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("unsupported top-level field `priority`"))
        );
        assert!(
            violations
                .iter()
                .any(|v| v.contains("unsupported `[[suppressions]]` field `priority`"))
        );
    }

    #[test]
    fn parse_rejects_blank_owner_or_reason() {
        let text = r#"schema_version = 1

[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:x"
owner = "   "
reason = "  "
"#;
        let (_, violations) = parse_suppressions_manifest(text);
        assert!(violations.iter().any(|v| v.contains("`owner` is blank")));
        assert!(violations.iter().any(|v| v.contains("`reason` is blank")));
    }

    #[test]
    fn parse_rejects_kind_field_mismatches() {
        let exposure_with_test = r#"schema_version = 1

[[suppressions]]
kind = "exposure_gap"
test = "x"
owner = "z"
reason = "y"
"#;
        let (_, violations) = parse_suppressions_manifest(exposure_with_test);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("`kind = \"exposure_gap\"` requires `finding_id`"))
                || violations
                    .iter()
                    .any(|v| v.contains("does not accept `test`"))
        );

        let test_with_finding_id = r#"schema_version = 1

[[suppressions]]
kind = "test_efficiency"
finding_id = "probe:x"
test = "x"
owner = "z"
reason = "y"
"#;
        let (_, violations) = parse_suppressions_manifest(test_with_finding_id);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("does not accept `finding_id`"))
        );
    }

    #[test]
    fn parse_rejects_absolute_or_backslash_path() {
        let abs = r#"schema_version = 1

[[suppressions]]
kind = "test_efficiency"
test = "x"
path = "/abs/path.rs"
owner = "z"
reason = "y"
"#;
        let (_, violations) = parse_suppressions_manifest(abs);
        assert!(violations.iter().any(|v| v.contains("is absolute")));

        let drive = "Z";
        let sep = ":/";
        let win = format!(
            r#"schema_version = 1

[[suppressions]]
kind = "test_efficiency"
test = "x"
path = "{drive}{sep}abs/path.rs"
owner = "z"
reason = "y"
"#
        );
        let (_, violations) = parse_suppressions_manifest(&win);
        assert!(violations.iter().any(|v| v.contains("is absolute")));

        let backslash = r#"schema_version = 1

[[suppressions]]
kind = "test_efficiency"
test = "x"
path = "tests\\cli.rs"
owner = "z"
reason = "y"
"#;
        let (_, violations) = parse_suppressions_manifest(backslash);
        assert!(violations.iter().any(|v| v.contains("backslashes")));
    }

    #[test]
    fn parse_rejects_invalid_expires_format() {
        let text = r#"schema_version = 1

[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:x"
owner = "z"
reason = "y"
expires = "Sept 1, 2026"
"#;
        let (_, violations) = parse_suppressions_manifest(text);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("not in YYYY-MM-DD format"))
        );
    }

    #[test]
    fn parse_rejects_duplicate_selectors() {
        let text = r#"schema_version = 1

[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:x"
owner = "z"
reason = "first"

[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:x"
owner = "z"
reason = "second"

[[suppressions]]
kind = "test_efficiency"
test = "alpha"
path = "tests/a.rs"
owner = "z"
reason = "first"

[[suppressions]]
kind = "test_efficiency"
test = "alpha"
path = "tests/a.rs"
owner = "z"
reason = "second"
"#;
        let (_, violations) = parse_suppressions_manifest(text);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("duplicate selector finding_id `probe:x`"))
        );
        assert!(
            violations
                .iter()
                .any(|v| v.contains("duplicate selector `alpha` at `tests/a.rs`"))
        );
    }

    #[test]
    fn is_iso_date_validates_format_and_ranges() {
        assert!(is_iso_date("2026-09-01"));
        assert!(is_iso_date("2025-12-31"));
        assert!(!is_iso_date("2026/09/01"));
        assert!(!is_iso_date("26-09-01"));
        assert!(!is_iso_date("2026-13-01"));
        assert!(!is_iso_date("2026-09-32"));
        assert!(!is_iso_date(""));
    }

    #[test]
    fn is_expired_compares_lexicographically_and_treats_none_as_active() {
        assert!(!is_expired(None, "2026-05-03"));
        assert!(!is_expired(Some("2026-09-01"), "2026-05-03"));
        assert!(!is_expired(Some("2026-05-03"), "2026-05-03"));
        assert!(is_expired(Some("2025-12-31"), "2026-05-03"));
        assert!(is_expired(Some("2026-04-30"), "2026-05-03"));
    }

    #[test]
    fn days_to_civil_date_round_trips_known_anchors() {
        // 1970-01-01 is day 0.
        assert_eq!(days_to_civil_date(0), (1970, 1, 1));
        // 2000-01-01 is day 10957 (well-known anchor).
        assert_eq!(days_to_civil_date(10957), (2000, 1, 1));
        // 2026-05-03 is day 20576 (today's anchor for this PR).
        assert_eq!(days_to_civil_date(20576), (2026, 5, 3));
        assert_eq!(days_to_civil_date(20577), (2026, 5, 4));
    }

    #[test]
    fn current_iso_date_returns_yyyy_mm_dd_format() {
        let today = current_iso_date();
        assert!(is_iso_date(&today), "current_iso_date returned `{today}`");
    }

    fn exposure_entry(
        finding_id: &str,
        expires: Option<&str>,
        block_line: usize,
    ) -> SuppressionEntry {
        SuppressionEntry {
            kind: SuppressionKind::ExposureGap,
            finding_id: Some(finding_id.to_string()),
            test: None,
            path: None,
            reason: "stated".to_string(),
            owner: "team".to_string(),
            expires: expires.map(str::to_string),
            block_line,
        }
    }

    fn test_efficiency_entry(
        test: &str,
        path: Option<&str>,
        expires: Option<&str>,
        block_line: usize,
    ) -> SuppressionEntry {
        SuppressionEntry {
            kind: SuppressionKind::TestEfficiency,
            finding_id: None,
            test: Some(test.to_string()),
            path: path.map(str::to_string),
            reason: "stated".to_string(),
            owner: "team".to_string(),
            expires: expires.map(str::to_string),
            block_line,
        }
    }

    #[test]
    fn apply_exposure_suppressions_moves_matched_findings_into_suppressed_set() {
        let candidates = vec![
            "probe:a".to_string(),
            "probe:b".to_string(),
            "probe:c".to_string(),
        ];
        let suppressions = vec![
            exposure_entry("probe:a", None, 10),
            exposure_entry("probe:c", Some("2099-01-01"), 20),
        ];
        let app = apply_exposure_suppressions(&candidates, &suppressions, "2026-05-03");

        assert!(app.suppressed_findings.contains("probe:a"));
        assert!(app.suppressed_findings.contains("probe:c"));
        assert!(!app.suppressed_findings.contains("probe:b"));
        assert!(app.warnings.is_empty());
    }

    #[test]
    fn apply_exposure_suppressions_warns_on_expired_and_unmatched_selectors() {
        let candidates = vec!["probe:a".to_string()];
        let suppressions = vec![
            // Expired — must NOT apply, must surface as warning.
            exposure_entry("probe:a", Some("2025-01-01"), 10),
            // Unmatched — must surface as warning.
            exposure_entry("probe:does_not_exist", Some("2099-01-01"), 20),
        ];
        let app = apply_exposure_suppressions(&candidates, &suppressions, "2026-05-03");

        assert!(
            app.suppressed_findings.is_empty(),
            "expired and unmatched suppressions must not apply"
        );
        assert_eq!(app.warnings.len(), 2);
        assert!(
            app.warnings
                .iter()
                .any(|w| w.contains("expired") && w.contains("probe:a"))
        );
        assert!(
            app.warnings
                .iter()
                .any(|w| w.contains("did not match") && w.contains("probe:does_not_exist"))
        );
    }

    #[test]
    fn apply_test_efficiency_suppressions_matches_by_test_or_test_and_path() {
        let candidates = vec![
            ("test_alpha".to_string(), "tests/a.rs".to_string()),
            ("test_alpha".to_string(), "tests/b.rs".to_string()),
            ("test_beta".to_string(), "tests/c.rs".to_string()),
        ];
        let suppressions = vec![
            // Matches both alpha entries (no path narrowing).
            test_efficiency_entry("test_alpha", None, None, 10),
        ];
        let app = apply_test_efficiency_suppressions(&candidates, &suppressions, "2026-05-03");
        assert_eq!(app.suppressed_tests.len(), 2);
        assert!(app.warnings.is_empty());

        let suppressions_with_path = vec![test_efficiency_entry(
            "test_alpha",
            Some("tests/a.rs"),
            None,
            10,
        )];
        let app =
            apply_test_efficiency_suppressions(&candidates, &suppressions_with_path, "2026-05-03");
        assert_eq!(app.suppressed_tests.len(), 1);
    }

    #[test]
    fn apply_test_efficiency_suppressions_surfaces_unmatched_and_expired_warnings() {
        let candidates = vec![("test_alpha".to_string(), "tests/a.rs".to_string())];
        let suppressions = vec![
            test_efficiency_entry("test_does_not_exist", None, Some("2099-01-01"), 10),
            test_efficiency_entry("test_alpha", None, Some("2025-01-01"), 20),
        ];
        let app = apply_test_efficiency_suppressions(&candidates, &suppressions, "2026-05-03");

        assert!(app.suppressed_tests.is_empty());
        assert_eq!(app.warnings.len(), 2);
        assert!(app.warnings.iter().any(|w| w.contains("did not match")));
        assert!(app.warnings.iter().any(|w| w.contains("expired")));
    }

    #[test]
    fn suppressions_path_constant_matches_expected_layout() {
        assert_eq!(SUPPRESSIONS_PATH, ".ripr/suppressions.toml");
    }

    #[test]
    fn application_has_default_constructor() {
        let app = SuppressionApplication::default();
        assert!(app.suppressed_findings.is_empty());
        assert!(app.suppressed_tests.is_empty());
        assert!(app.warnings.is_empty());
    }
}
