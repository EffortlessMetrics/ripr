//! `--exception-policy` TOML ledger for `ripr gate evaluate` (#1442).
//!
//! Downstream consumers ramping a gate keep a dated, auditable ledger of
//! named temporary burndown exceptions. ripr owns the enforcement
//! semantics — expiry, review-after deadlines, required-active entries,
//! and final-status ledgers — while the consumer owns only the policy
//! CONTENT (which exceptions are active, for how long).
//!
//! Evaluation rules, given `today` (UTC `YYYY-MM-DD`):
//!
//! - `expires < today` → `quality_exception_expired` (blocking).
//! - otherwise `review_after <= today` → `quality_exception_review_due`
//!   (blocking when the ledger header sets `due_review = "fail"`, a
//!   warning when `due_review = "warn"`; the default is `"fail"` so an
//!   unstated policy fails closed).
//! - a `[requirements] required_active` id with no ACTIVE exception →
//!   `quality_exception_required_missing` (blocking). An expired required
//!   exception raises both violations; both stay visible.
//! - ledger header `status = "final"` → every still-active exception is
//!   `quality_exception_final_active` (blocking): a final-enforcement
//!   ledger declares that no exception may remain.
//!
//! Loading is fail-closed: a missing or malformed ledger is an error the
//! gate reports as `config_error`, never a silently ignored input.

use crate::output::suppressions::is_iso_date;
use serde::Deserialize;
use std::path::Path;

/// Violation kinds, stable snake_case contract terms mirrored in
/// `docs/OUTPUT_SCHEMA.md`.
pub(crate) const VIOLATION_EXPIRED: &str = "quality_exception_expired";
pub(crate) const VIOLATION_REVIEW_DUE: &str = "quality_exception_review_due";
pub(crate) const VIOLATION_REQUIRED_MISSING: &str = "quality_exception_required_missing";
pub(crate) const VIOLATION_FINAL_ACTIVE: &str = "quality_exception_final_active";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExceptionLedger {
    schema_version: u32,
    policy: String,
    owner: Option<String>,
    status: Option<String>,
    updated: Option<String>,
    due_review: Option<String>,
    requirements: Option<RawRequirements>,
    #[serde(default)]
    exception: Vec<RawException>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRequirements {
    #[serde(default)]
    required_active: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawException {
    id: String,
    kind: String,
    scope: String,
    owner: String,
    reason: String,
    final_target: String,
    evidence: String,
    removal_criteria: String,
    created: String,
    review_after: String,
    expires: String,
    issue: Option<String>,
}

/// How a past-due `review_after` is enforced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DueReview {
    Warn,
    Fail,
}

impl DueReview {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Warn => "warn",
            Self::Fail => "fail",
        }
    }
}

/// A validated exception ledger, pre-evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExceptionLedger {
    pub(crate) ledger_status: String,
    /// Header `owner` metadata, surfaced in the gate-decision report so the
    /// accountable party stays visible next to the enforcement outcome.
    pub(crate) ledger_owner: Option<String>,
    pub(crate) due_review: DueReview,
    pub(crate) required_active: Vec<String>,
    pub(crate) exceptions: Vec<ExceptionEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExceptionEntry {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) scope: String,
    pub(crate) owner: String,
    pub(crate) reason: String,
    pub(crate) review_after: String,
    pub(crate) expires: String,
}

/// One enforced violation. `blocking: false` entries surface as gate
/// warnings; `blocking: true` entries drive the `blocked` status.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExceptionViolation {
    pub(crate) kind: String,
    pub(crate) exception_id: String,
    pub(crate) detail: String,
    pub(crate) blocking: bool,
}

/// Evaluation outcome carried on the gate decision report and projected
/// into gate-decision JSON/Markdown when `--exception-policy` was given.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExceptionPolicyReport {
    /// The ledger path exactly as the caller supplied it.
    pub(crate) path: String,
    pub(crate) ledger_status: String,
    pub(crate) ledger_owner: Option<String>,
    pub(crate) due_review: DueReview,
    /// Exceptions that remain active (not expired) today.
    pub(crate) active: Vec<ExceptionEntry>,
    pub(crate) violations: Vec<ExceptionViolation>,
}

impl ExceptionPolicyReport {
    pub(crate) fn blocking_count(&self) -> usize {
        self.violations.iter().filter(|v| v.blocking).count()
    }

    pub(crate) fn warning_details(&self) -> Vec<String> {
        self.violations
            .iter()
            .filter(|violation| !violation.blocking)
            .map(|violation| format!("{}: {}", violation.kind, violation.detail))
            .collect()
    }
}

/// Loads and validates an exception ledger. Fail-closed: any read, parse,
/// or validation failure is an `Err` naming the supplied path.
pub(crate) fn load_exception_ledger(
    resolved: &Path,
    display: &str,
) -> Result<ExceptionLedger, String> {
    let text = std::fs::read_to_string(resolved)
        .map_err(|err| format!("failed to read exception policy `{display}`: {err}"))?;
    let raw: RawExceptionLedger = toml::from_str(&text)
        .map_err(|err| format!("exception policy `{display}` is not valid TOML: {err}"))?;
    validate_ledger(raw, display)
}

fn validate_ledger(raw: RawExceptionLedger, display: &str) -> Result<ExceptionLedger, String> {
    let mut violations = Vec::new();
    if raw.schema_version != 1 {
        violations.push(format!(
            "schema_version = {} is not supported (expected 1)",
            raw.schema_version
        ));
    }
    if raw.policy != "quality-gate-exceptions" {
        violations.push(format!(
            "policy = `{}` is not supported (expected `quality-gate-exceptions`)",
            raw.policy
        ));
    }
    let due_review = match raw.due_review.as_deref() {
        None | Some("fail") => DueReview::Fail,
        Some("warn") => DueReview::Warn,
        Some(other) => {
            violations.push(format!(
                "due_review = `{other}` is not supported (expected `warn` or `fail`)"
            ));
            DueReview::Fail
        }
    };
    if let Some(updated) = raw.updated.as_deref()
        && !is_iso_date(updated)
    {
        violations.push(format!("updated `{updated}` is not in YYYY-MM-DD format"));
    }

    let mut seen_ids = std::collections::BTreeSet::new();
    let mut exceptions = Vec::new();
    for (index, entry) in raw.exception.iter().enumerate() {
        let label = if entry.id.trim().is_empty() {
            format!("[[exception]] #{}", index + 1)
        } else {
            format!("exception `{}`", entry.id)
        };
        for (field, value) in [
            ("id", &entry.id),
            ("kind", &entry.kind),
            ("scope", &entry.scope),
            ("owner", &entry.owner),
            ("reason", &entry.reason),
            ("final_target", &entry.final_target),
            ("evidence", &entry.evidence),
            ("removal_criteria", &entry.removal_criteria),
        ] {
            if value.trim().is_empty() {
                violations.push(format!("{label} has a blank required `{field}`"));
            }
        }
        for (field, value) in [
            ("created", &entry.created),
            ("review_after", &entry.review_after),
            ("expires", &entry.expires),
        ] {
            if !is_iso_date(value) {
                violations.push(format!(
                    "{label} `{field}` `{value}` is not in YYYY-MM-DD format"
                ));
            }
        }
        if !entry.id.trim().is_empty() && !seen_ids.insert(entry.id.clone()) {
            violations.push(format!("duplicate exception id `{}`", entry.id));
        }
        // `issue` is optional free-form metadata; no validation beyond TOML.
        let _ = &entry.issue;
        exceptions.push(ExceptionEntry {
            id: entry.id.clone(),
            kind: entry.kind.clone(),
            scope: entry.scope.clone(),
            owner: entry.owner.clone(),
            reason: entry.reason.clone(),
            review_after: entry.review_after.clone(),
            expires: entry.expires.clone(),
        });
    }
    for required in raw
        .requirements
        .as_ref()
        .map(|requirements| requirements.required_active.as_slice())
        .unwrap_or_default()
    {
        if required.trim().is_empty() {
            violations.push("requirements.required_active contains a blank id".to_string());
        }
    }

    if !violations.is_empty() {
        return Err(format!(
            "exception policy `{display}` is invalid:\n{}",
            violations.join("\n")
        ));
    }
    Ok(ExceptionLedger {
        ledger_status: raw.status.unwrap_or_else(|| "active".to_string()),
        ledger_owner: raw.owner,
        due_review,
        required_active: raw
            .requirements
            .map(|requirements| requirements.required_active)
            .unwrap_or_default(),
        exceptions,
    })
}

/// Evaluates a validated ledger against `today` (UTC `YYYY-MM-DD`).
pub(crate) fn evaluate_exception_ledger(
    ledger: &ExceptionLedger,
    today: &str,
    path: &str,
) -> ExceptionPolicyReport {
    let mut violations = Vec::new();
    let mut active = Vec::new();
    for entry in &ledger.exceptions {
        let expired = today > entry.expires.as_str();
        if expired {
            violations.push(ExceptionViolation {
                kind: VIOLATION_EXPIRED.to_string(),
                exception_id: entry.id.clone(),
                detail: format!(
                    "exception `{}` expired on {} (today {today}); remove it or renew it with a reviewed new expiry",
                    entry.id, entry.expires
                ),
                blocking: true,
            });
            continue;
        }
        if today >= entry.review_after.as_str() {
            violations.push(ExceptionViolation {
                kind: VIOLATION_REVIEW_DUE.to_string(),
                exception_id: entry.id.clone(),
                detail: format!(
                    "exception `{}` passed its review_after date {} (today {today}); re-review it and move review_after forward",
                    entry.id, entry.review_after
                ),
                blocking: ledger.due_review == DueReview::Fail,
            });
        }
        active.push(entry.clone());
    }
    for required in &ledger.required_active {
        if !active.iter().any(|entry| &entry.id == required) {
            violations.push(ExceptionViolation {
                kind: VIOLATION_REQUIRED_MISSING.to_string(),
                exception_id: required.clone(),
                detail: format!(
                    "required exception `{required}` is missing from the active set; restore it or update requirements.required_active"
                ),
                blocking: true,
            });
        }
    }
    if ledger.ledger_status == "final" {
        for entry in &active {
            violations.push(ExceptionViolation {
                kind: VIOLATION_FINAL_ACTIVE.to_string(),
                exception_id: entry.id.clone(),
                detail: format!(
                    "ledger status is `final` but exception `{}` is still active; final enforcement requires zero active exceptions",
                    entry.id
                ),
                blocking: true,
            });
        }
    }
    ExceptionPolicyReport {
        path: path.to_string(),
        ledger_status: ledger.ledger_status.clone(),
        ledger_owner: ledger.ledger_owner.clone(),
        due_review: ledger.due_review,
        active,
        violations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_LEDGER: &str = r##"
schema_version = 1
policy = "quality-gate-exceptions"
owner = "EffortlessMetrics"
status = "active"
updated = "2026-05-28"
due_review = "fail"

[requirements]
required_active = ["ripr-total-burndown"]

[[exception]]
id = "ripr-total-burndown"
kind = "temporary_burndown"
scope = "ripr_plus_total"
owner = "proof-lane"
issue = "#8197"
reason = "Existing repo-wide gaps predate the transition gate."
final_target = "repo-wide unresolved total = 0"
evidence = "target/receipts/quality/ripr-plus.json"
removal_criteria = "final mode requires unresolved total = 0"
created = "2026-05-28"
review_after = "2026-06-28"
expires = "2026-09-30"
"##;

    fn parse_valid() -> Result<ExceptionLedger, String> {
        let raw: RawExceptionLedger =
            toml::from_str(VALID_LEDGER).map_err(|err| format!("parse: {err}"))?;
        validate_ledger(raw, "policy/quality-gate-exceptions.toml")
    }

    #[test]
    fn valid_ledger_parses_with_consumer_schema() -> Result<(), String> {
        let ledger = parse_valid()?;
        assert_eq!(ledger.due_review, DueReview::Fail);
        assert_eq!(ledger.ledger_status, "active");
        assert_eq!(ledger.required_active, vec!["ripr-total-burndown"]);
        assert_eq!(ledger.exceptions.len(), 1);
        assert_eq!(ledger.exceptions[0].id, "ripr-total-burndown");
        Ok(())
    }

    #[test]
    fn active_exception_before_review_date_produces_no_violations() -> Result<(), String> {
        let ledger = parse_valid()?;
        let report = evaluate_exception_ledger(&ledger, "2026-06-01", "ledger.toml");
        assert!(report.violations.is_empty(), "{:?}", report.violations);
        assert_eq!(report.active.len(), 1);
        assert_eq!(report.blocking_count(), 0);
        Ok(())
    }

    #[test]
    fn expired_exception_is_blocking_and_leaves_active_set() -> Result<(), String> {
        let ledger = parse_valid()?;
        let report = evaluate_exception_ledger(&ledger, "2026-10-01", "ledger.toml");
        // Expired → quality_exception_expired AND the required entry is no
        // longer active → quality_exception_required_missing. Both visible.
        assert_eq!(report.active.len(), 0);
        assert_eq!(report.blocking_count(), 2);
        assert!(
            report
                .violations
                .iter()
                .any(|violation| violation.kind == VIOLATION_EXPIRED)
        );
        assert!(
            report
                .violations
                .iter()
                .any(|violation| violation.kind == VIOLATION_REQUIRED_MISSING)
        );
        Ok(())
    }

    #[test]
    fn review_due_blocks_under_fail_and_warns_under_warn() -> Result<(), String> {
        let mut ledger = parse_valid()?;
        let report = evaluate_exception_ledger(&ledger, "2026-06-28", "ledger.toml");
        assert_eq!(report.blocking_count(), 1);
        assert_eq!(report.violations[0].kind, VIOLATION_REVIEW_DUE);
        // Still active: review-due does not deactivate the exception.
        assert_eq!(report.active.len(), 1);

        ledger.due_review = DueReview::Warn;
        let report = evaluate_exception_ledger(&ledger, "2026-06-28", "ledger.toml");
        assert_eq!(report.blocking_count(), 0);
        assert_eq!(report.warning_details().len(), 1);
        assert!(report.warning_details()[0].contains(VIOLATION_REVIEW_DUE));
        Ok(())
    }

    #[test]
    fn final_status_ledger_blocks_on_any_active_exception() -> Result<(), String> {
        let mut ledger = parse_valid()?;
        ledger.ledger_status = "final".to_string();
        let report = evaluate_exception_ledger(&ledger, "2026-06-01", "ledger.toml");
        assert_eq!(report.blocking_count(), 1);
        assert_eq!(report.violations[0].kind, VIOLATION_FINAL_ACTIVE);
        Ok(())
    }

    #[test]
    fn required_active_missing_entry_is_blocking() -> Result<(), String> {
        let mut ledger = parse_valid()?;
        ledger
            .required_active
            .push("project-coverage-burndown".to_string());
        let report = evaluate_exception_ledger(&ledger, "2026-06-01", "ledger.toml");
        assert_eq!(report.blocking_count(), 1);
        assert_eq!(report.violations[0].kind, VIOLATION_REQUIRED_MISSING);
        assert_eq!(
            report.violations[0].exception_id,
            "project-coverage-burndown"
        );
        Ok(())
    }

    #[test]
    fn validation_rejects_blank_fields_bad_dates_duplicates_and_unknown_keys() {
        let missing_field = r#"
schema_version = 1
policy = "quality-gate-exceptions"

[[exception]]
id = "x"
kind = "temporary_burndown"
scope = "s"
owner = "o"
reason = ""
final_target = "t"
evidence = "e"
removal_criteria = "r"
created = "2026-05-28"
review_after = "2026-06-28"
expires = "2026-09-30"
"#;
        let raw: Result<RawExceptionLedger, _> = toml::from_str(missing_field);
        let err = raw
            .map_err(|e| e.to_string())
            .and_then(|raw| validate_ledger(raw, "x.toml"))
            .err();
        assert!(
            err.as_deref()
                .is_some_and(|e| e.contains("blank required `reason`")),
            "err: {err:?}"
        );

        let bad_date = missing_field
            .replace("reason = \"\"", "reason = \"r\"")
            .replace("expires = \"2026-09-30\"", "expires = \"soon\"");
        let err = toml::from_str::<RawExceptionLedger>(&bad_date)
            .map_err(|e| e.to_string())
            .and_then(|raw| validate_ledger(raw, "x.toml"))
            .err();
        assert!(
            err.as_deref()
                .is_some_and(|e| e.contains("`expires` `soon` is not in YYYY-MM-DD format")),
            "err: {err:?}"
        );

        // Unknown keys fail at the TOML layer (deny_unknown_fields) so a
        // typo'd field name can never be silently ignored.
        let unknown_key =
            "schema_version = 1\npolicy = \"quality-gate-exceptions\"\nunexpected = true\n";
        let err = toml::from_str::<RawExceptionLedger>(unknown_key)
            .map(|_| ())
            .err()
            .map(|e| e.to_string());
        assert!(
            err.as_deref().is_some_and(|e| e.contains("unexpected")),
            "unknown ledger keys must fail parsing loudly: {err:?}"
        );

        let unsupported_due =
            "schema_version = 1\npolicy = \"quality-gate-exceptions\"\ndue_review = \"ignore\"\n";
        let err = toml::from_str::<RawExceptionLedger>(unsupported_due)
            .map_err(|e| e.to_string())
            .and_then(|raw| validate_ledger(raw, "x.toml"))
            .err();
        assert!(
            err.as_deref()
                .is_some_and(|e| e.contains("due_review = `ignore` is not supported")),
            "err: {err:?}"
        );
    }

    #[test]
    fn loader_fails_closed_on_missing_file() {
        let err =
            load_exception_ledger(Path::new("does/not/exist.toml"), "does/not/exist.toml").err();
        assert!(
            err.as_deref()
                .is_some_and(|e| e.contains("failed to read exception policy")),
            "err: {err:?}"
        );
    }
}
