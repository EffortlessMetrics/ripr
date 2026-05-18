use crate::output::suppressions::{
    SUPPRESSIONS_PATH, SuppressionEntry, SuppressionKind, is_expired, parse_suppressions_manifest,
};
use serde_json::{Value, json};

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "suppression_health";
const LIMITS_NOTE: &str = "Read-only advisory suppression-health report over the durable suppression manifest; suppressions remain visible and the report never creates, deletes, applies, or gates on suppressions.";

pub(crate) const DEFAULT_SUPPRESSION_HEALTH_OUT: &str =
    "target/ripr/reports/suppression-health.json";
pub(crate) const DEFAULT_SUPPRESSION_HEALTH_MD_OUT: &str =
    "target/ripr/reports/suppression-health.md";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SuppressionHealthInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) today: String,
    pub(crate) manifest_path: String,
    pub(crate) manifest_text: Option<Result<String, String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SuppressionHealthReport {
    root: String,
    generated_at: String,
    status: String,
    inputs: SuppressionHealthInputs,
    summary: SuppressionHealthSummary,
    records: Vec<SuppressionHealthRecord>,
    findings: Vec<SuppressionHealthFinding>,
    warnings: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SuppressionHealthInputs {
    manifest: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SuppressionHealthSummary {
    suppressions: usize,
    healthy: usize,
    missing_owner: usize,
    missing_reason: usize,
    missing_scope: usize,
    missing_created_at: usize,
    missing_last_seen: usize,
    missing_review_by_or_expires: usize,
    missing_expected_visibility: usize,
    missing_static_class: usize,
    stale: usize,
    overbroad_scope: usize,
    unknown_selector: usize,
    preview_without_preview_label: usize,
    warnings: usize,
    config_errors: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SuppressionHealthRecord {
    identity: String,
    kind: String,
    owner: String,
    reason: String,
    scope: Option<String>,
    created_at: Option<String>,
    last_seen: Option<String>,
    expires: Option<String>,
    review_by: Option<String>,
    expected_visibility: Option<String>,
    static_class: Option<String>,
    language: Option<String>,
    language_status: Option<String>,
    health: String,
    still_visible: bool,
    source: String,
    findings: Vec<SuppressionHealthFinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SuppressionHealthFinding {
    kind: String,
    severity: String,
    message: String,
    source: Option<String>,
}

pub(crate) fn build_suppression_health_report(
    input: SuppressionHealthInput,
) -> SuppressionHealthReport {
    let mut summary = SuppressionHealthSummary::default();
    let mut records = Vec::new();
    let mut findings = Vec::new();
    let mut warnings = Vec::new();

    let Some(manifest_text) = input.manifest_text else {
        let status = "no_suppressions".to_string();
        return SuppressionHealthReport {
            root: input.root,
            generated_at: input.generated_at,
            status,
            inputs: SuppressionHealthInputs {
                manifest: input.manifest_path,
            },
            summary,
            records,
            findings,
            warnings,
        };
    };

    let manifest_text = match manifest_text {
        Ok(text) => text,
        Err(error) => {
            let finding = SuppressionHealthFinding {
                kind: "manifest_unreadable".to_string(),
                severity: "config_error".to_string(),
                message: error,
                source: Some(input.manifest_path.clone()),
            };
            count_finding(&mut summary, &finding);
            findings.push(finding);
            summary.config_errors = 1;
            let status = "config_error".to_string();
            return SuppressionHealthReport {
                root: input.root,
                generated_at: input.generated_at,
                status,
                inputs: SuppressionHealthInputs {
                    manifest: input.manifest_path,
                },
                summary,
                records,
                findings,
                warnings,
            };
        }
    };

    let (entries, violations) = parse_suppressions_manifest(&manifest_text);
    for violation in violations {
        let finding = finding_from_violation(&input.manifest_path, &violation);
        count_finding(&mut summary, &finding);
        findings.push(finding);
    }

    for entry in entries {
        let record = record_for_entry(&input.manifest_path, &input.today, entry);
        for finding in &record.findings {
            count_finding(&mut summary, finding);
            findings.push(finding.clone());
        }
        records.push(record);
    }

    summary.suppressions = records.len();
    summary.healthy = records
        .iter()
        .filter(|record| record.health == "healthy")
        .count();
    summary.warnings = findings
        .iter()
        .filter(|finding| finding.severity == "warning")
        .count();
    summary.config_errors = findings
        .iter()
        .filter(|finding| finding.severity == "config_error")
        .count();

    if input.manifest_path == SUPPRESSIONS_PATH && records.is_empty() && findings.is_empty() {
        warnings.push("suppression manifest is present but contains no entries".to_string());
    }

    let status = if summary.config_errors > 0 {
        "config_error"
    } else if records.is_empty() {
        "no_suppressions"
    } else if summary.warnings > 0 {
        "warning"
    } else {
        "healthy"
    }
    .to_string();

    SuppressionHealthReport {
        root: input.root,
        generated_at: input.generated_at,
        status,
        inputs: SuppressionHealthInputs {
            manifest: input.manifest_path,
        },
        summary,
        records,
        findings,
        warnings,
    }
}

pub(crate) fn render_suppression_health_json(
    report: &SuppressionHealthReport,
) -> Result<String, String> {
    serde_json::to_string_pretty(&json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "kind": REPORT_KIND,
        "status": report.status,
        "root": report.root,
        "generated_at": report.generated_at,
        "inputs": {
            "manifest": report.inputs.manifest,
        },
        "summary": summary_json(&report.summary),
        "records": report.records.iter().map(record_json).collect::<Vec<_>>(),
        "findings": report.findings.iter().map(finding_json).collect::<Vec<_>>(),
        "warnings": report.warnings,
        "limits_note": LIMITS_NOTE,
    }))
    .map_err(|err| format!("failed to render suppression health JSON: {err}"))
}

pub(crate) fn render_suppression_health_markdown(report: &SuppressionHealthReport) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Suppression Health\n\n");
    out.push_str(&format!("Status: {}\n\n", report.status));
    out.push_str("| Identity | Kind | Health | Owner | Review | Findings |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- |\n");
    if report.records.is_empty() {
        out.push_str("| none | n/a | healthy | n/a | n/a | no durable suppressions |\n");
    } else {
        for record in &report.records {
            let finding_list = if record.findings.is_empty() {
                "none".to_string()
            } else {
                record
                    .findings
                    .iter()
                    .map(|finding| finding.kind.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let review = record
                .review_by
                .as_deref()
                .or(record.expires.as_deref())
                .unwrap_or("missing");
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                markdown_cell(&record.identity),
                record.kind,
                record.health,
                markdown_cell(&record.owner),
                markdown_cell(review),
                markdown_cell(&finding_list)
            ));
        }
    }

    if !report.findings.is_empty() {
        out.push_str("\nFindings:\n");
        for finding in &report.findings {
            out.push_str(&format!(
                "- {}: {} ({})\n",
                finding.kind, finding.message, finding.severity
            ));
        }
    }
    if !report.warnings.is_empty() {
        out.push_str("\nWarnings:\n");
        for warning in &report.warnings {
            out.push_str(&format!("- {warning}\n"));
        }
    }
    out.push_str("\nPolicy boundary:\n");
    out.push_str("- Suppressions are durable policy exceptions, not waivers or baselines.\n");
    out.push_str("- Suppressed findings remain visible as suppressed with owner and reason.\n");
    out.push_str(
        "- Preview-language suppressions require `language_status = \"preview\"` until promoted.\n",
    );
    out.push_str("\nLimits:\n");
    out.push_str(LIMITS_NOTE);
    out.push('\n');
    out
}

pub(crate) fn suppression_health_status(report: &SuppressionHealthReport) -> &str {
    &report.status
}

pub(crate) use crate::output::path::display_path;

fn record_for_entry(
    manifest_path: &str,
    today: &str,
    entry: SuppressionEntry,
) -> SuppressionHealthRecord {
    let identity = identity_for_entry(&entry);
    let source = format!("{manifest_path}:{}", entry.block_line);
    let mut findings = Vec::new();

    if entry.scope.as_deref().is_none_or(str::is_empty) {
        findings.push(record_finding(
            "missing_scope",
            "warning",
            "suppression is missing reviewed scope metadata",
            &source,
        ));
    }
    if entry.created_at.is_none() {
        findings.push(record_finding(
            "missing_created_at",
            "warning",
            "suppression is missing created_at date",
            &source,
        ));
    }
    if entry.last_seen.is_none() {
        findings.push(record_finding(
            "missing_last_seen",
            "warning",
            "suppression is missing last_seen date",
            &source,
        ));
    }
    if entry.review_by.is_none() && entry.expires.is_none() {
        findings.push(record_finding(
            "missing_review_by_or_expires",
            "warning",
            "suppression is missing review_by or expires date",
            &source,
        ));
    }
    if entry.expected_visibility.is_none() {
        findings.push(record_finding(
            "missing_expected_visibility",
            "warning",
            "suppression is missing expected_visibility metadata",
            &source,
        ));
    }
    if entry.static_class.is_none() {
        findings.push(record_finding(
            "missing_static_class",
            "warning",
            "suppression is missing static_class metadata",
            &source,
        ));
    }
    if is_expired(entry.expires.as_deref(), today) {
        findings.push(record_finding(
            "stale_suppression",
            "warning",
            "suppression expires date is in the past",
            &source,
        ));
    }
    if is_expired(entry.review_by.as_deref(), today) {
        findings.push(record_finding(
            "stale_suppression",
            "warning",
            "suppression review_by date is in the past",
            &source,
        ));
    }
    if entry.kind == SuppressionKind::TestEfficiency && entry.path.is_none() {
        findings.push(record_finding(
            "overbroad_scope",
            "warning",
            "test-efficiency suppression has no path selector",
            &source,
        ));
    }
    if entry.scope.as_deref().is_some_and(is_overbroad_scope_value) {
        findings.push(record_finding(
            "overbroad_scope",
            "warning",
            "suppression scope is broader than a reviewed seam, file, or test selector",
            &source,
        ));
    }
    if is_preview_language(entry.language.as_deref())
        && entry.language_status.as_deref() != Some("preview")
    {
        findings.push(record_finding(
            "preview_without_preview_label",
            "warning",
            "preview-language suppression is missing language_status = \"preview\"",
            &source,
        ));
    }

    let health = if findings.is_empty() {
        "healthy"
    } else {
        "warning"
    }
    .to_string();

    SuppressionHealthRecord {
        identity,
        kind: entry.kind.as_str().to_string(),
        owner: entry.owner,
        reason: entry.reason,
        scope: entry.scope,
        created_at: entry.created_at,
        last_seen: entry.last_seen,
        expires: entry.expires,
        review_by: entry.review_by,
        expected_visibility: entry.expected_visibility,
        static_class: entry.static_class,
        language: entry.language,
        language_status: entry.language_status,
        health,
        still_visible: true,
        source,
        findings,
    }
}

fn identity_for_entry(entry: &SuppressionEntry) -> String {
    match entry.kind {
        SuppressionKind::ExposureGap => entry
            .finding_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        SuppressionKind::TestEfficiency => match (&entry.test, &entry.path) {
            (Some(test), Some(path)) => format!("{test}@{path}"),
            (Some(test), None) => test.clone(),
            _ => "unknown".to_string(),
        },
    }
}

fn finding_from_violation(manifest_path: &str, violation: &str) -> SuppressionHealthFinding {
    let kind = if violation.contains("missing required `owner`")
        || violation.contains("`owner` is blank")
    {
        "missing_owner"
    } else if violation.contains("missing required `reason`")
        || violation.contains("`reason` is blank")
    {
        "missing_reason"
    } else if violation.contains("unsupported kind")
        || violation.contains("requires `finding_id`")
        || violation.contains("requires `test`")
        || violation.contains("`finding_id` is blank")
        || violation.contains("`test` is blank")
        || violation.contains("duplicate selector")
    {
        "unknown_selector"
    } else {
        "config_error"
    };

    SuppressionHealthFinding {
        kind: kind.to_string(),
        severity: "config_error".to_string(),
        message: violation.to_string(),
        source: Some(manifest_path.to_string()),
    }
}

fn count_finding(summary: &mut SuppressionHealthSummary, finding: &SuppressionHealthFinding) {
    match finding.kind.as_str() {
        "missing_owner" => summary.missing_owner += 1,
        "missing_reason" => summary.missing_reason += 1,
        "missing_scope" => summary.missing_scope += 1,
        "missing_created_at" => summary.missing_created_at += 1,
        "missing_last_seen" => summary.missing_last_seen += 1,
        "missing_review_by_or_expires" => summary.missing_review_by_or_expires += 1,
        "missing_expected_visibility" => summary.missing_expected_visibility += 1,
        "missing_static_class" => summary.missing_static_class += 1,
        "stale_suppression" => summary.stale += 1,
        "overbroad_scope" => summary.overbroad_scope += 1,
        "unknown_selector" => summary.unknown_selector += 1,
        "preview_without_preview_label" => summary.preview_without_preview_label += 1,
        _ => {}
    }
}

fn record_finding(
    kind: &str,
    severity: &str,
    message: &str,
    source: &str,
) -> SuppressionHealthFinding {
    SuppressionHealthFinding {
        kind: kind.to_string(),
        severity: severity.to_string(),
        message: message.to_string(),
        source: Some(source.to_string()),
    }
}

fn is_overbroad_scope_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "repo" | "repository" | "workspace" | "global" | "all" | "*" | "any"
    )
}

fn is_preview_language(language: Option<&str>) -> bool {
    matches!(
        language,
        Some("typescript" | "javascript" | "tsx" | "jsx" | "python")
    )
}

fn summary_json(summary: &SuppressionHealthSummary) -> Value {
    json!({
        "suppressions": summary.suppressions,
        "healthy": summary.healthy,
        "missing_owner": summary.missing_owner,
        "missing_reason": summary.missing_reason,
        "missing_scope": summary.missing_scope,
        "missing_created_at": summary.missing_created_at,
        "missing_last_seen": summary.missing_last_seen,
        "missing_review_by_or_expires": summary.missing_review_by_or_expires,
        "missing_expected_visibility": summary.missing_expected_visibility,
        "missing_static_class": summary.missing_static_class,
        "stale": summary.stale,
        "overbroad_scope": summary.overbroad_scope,
        "unknown_selector": summary.unknown_selector,
        "preview_without_preview_label": summary.preview_without_preview_label,
        "warnings": summary.warnings,
        "config_errors": summary.config_errors,
    })
}

fn record_json(record: &SuppressionHealthRecord) -> Value {
    json!({
        "identity": record.identity,
        "kind": record.kind,
        "owner": record.owner,
        "reason": record.reason,
        "scope": record.scope,
        "created_at": record.created_at,
        "last_seen": record.last_seen,
        "expires": record.expires,
        "review_by": record.review_by,
        "expected_visibility": record.expected_visibility,
        "static_class": record.static_class,
        "language": record.language,
        "language_status": record.language_status,
        "health": record.health,
        "still_visible": record.still_visible,
        "source": record.source,
        "findings": record.findings.iter().map(finding_json).collect::<Vec<_>>(),
    })
}

fn finding_json(finding: &SuppressionHealthFinding) -> Value {
    json!({
        "kind": finding.kind,
        "severity": finding.severity,
        "message": finding.message,
        "source": finding.source,
    })
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(manifest_text: Option<Result<&str, &str>>) -> SuppressionHealthInput {
        SuppressionHealthInput {
            root: ".".to_string(),
            generated_at: "unix_ms:1".to_string(),
            today: "2026-05-12".to_string(),
            manifest_path: ".ripr/suppressions.toml".to_string(),
            manifest_text: manifest_text
                .map(|result| result.map(str::to_string).map_err(str::to_string)),
        }
    }

    #[test]
    fn missing_manifest_is_healthy_no_suppressions() {
        let report = build_suppression_health_report(input(None));
        assert_eq!(report.status, "no_suppressions");
        assert_eq!(report.summary.suppressions, 0);
        assert!(report.findings.is_empty());
        let md = render_suppression_health_markdown(&report);
        assert!(md.contains("no durable suppressions"));
        assert_eq!(suppression_health_status(&report), "no_suppressions");
    }

    #[test]
    fn unreadable_manifest_is_config_error() {
        let report = build_suppression_health_report(input(Some(Err("read failed"))));
        assert_eq!(report.status, "config_error");
        assert_eq!(report.summary.config_errors, 1);
        assert_eq!(report.findings[0].kind, "manifest_unreadable");
    }

    #[test]
    fn empty_present_manifest_is_no_suppressions_with_warning() {
        let report = build_suppression_health_report(input(Some(Ok("schema_version = 1\n"))));
        assert_eq!(report.status, "no_suppressions");
        assert_eq!(report.warnings.len(), 1);
        let md = render_suppression_health_markdown(&report);
        assert!(md.contains("suppression manifest is present but contains no entries"));
    }

    #[test]
    fn complete_manifest_is_healthy_and_keeps_visibility() -> Result<(), String> {
        let report = build_suppression_health_report(input(Some(Ok(r#"schema_version = 1

[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:src/pricing.rs:88:predicate"
owner = "billing"
reason = "accepted durable policy exception"
scope = "seam:pricing::threshold"
created_at = "2026-01-01"
last_seen = "2026-05-01"
review_by = "2026-12-01"
expected_visibility = "suppressed_visible"
static_class = "weakly_exposed"
language = "rust"
"#))));
        assert_eq!(report.status, "healthy");
        assert_eq!(report.summary.suppressions, 1);
        assert_eq!(report.summary.healthy, 1);
        assert!(report.records[0].still_visible);
        let rendered = render_suppression_health_json(&report)?;
        assert!(rendered.contains("\"kind\": \"suppression_health\""));
        assert!(rendered.contains("\"still_visible\": true"));
        Ok(())
    }

    #[test]
    fn metadata_gaps_are_advisory_warnings() -> Result<(), String> {
        let report = build_suppression_health_report(input(Some(Ok(r#"schema_version = 1

[[suppressions]]
kind = "test_efficiency"
test = "cli_prints_help"
owner = "devtools"
reason = "temporary smoke exception"
"#))));
        assert_eq!(report.status, "warning");
        assert_eq!(report.summary.missing_scope, 1);
        assert_eq!(report.summary.missing_created_at, 1);
        assert_eq!(report.summary.missing_last_seen, 1);
        assert_eq!(report.summary.missing_review_by_or_expires, 1);
        assert_eq!(report.summary.missing_expected_visibility, 1);
        assert_eq!(report.summary.missing_static_class, 1);
        assert_eq!(report.summary.overbroad_scope, 1);
        let rendered = render_suppression_health_json(&report)?;
        assert!(rendered.contains("\"overbroad_scope\": 1"));
        assert!(rendered.contains("suppression is missing reviewed scope metadata"));
        let md = render_suppression_health_markdown(&report);
        assert!(md.contains("missing_scope"));
        assert!(md.contains("overbroad_scope"));
        Ok(())
    }

    #[test]
    fn stale_and_preview_label_gaps_are_flagged() {
        let report = build_suppression_health_report(input(Some(Ok(r#"schema_version = 1

[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:app.ts:10:branch"
owner = "frontend"
reason = "preview adapter false positive under review"
scope = "seam:frontend"
created_at = "2026-01-01"
last_seen = "2026-05-01"
expires = "2026-02-01"
review_by = "2026-01-31"
expected_visibility = "suppressed_visible"
static_class = "weakly_exposed"
language = "typescript"
"#))));
        assert_eq!(report.status, "warning");
        assert_eq!(report.summary.stale, 2);
        assert_eq!(report.summary.preview_without_preview_label, 1);
    }

    #[test]
    fn path_scoped_test_efficiency_suppression_can_be_healthy() {
        let report = build_suppression_health_report(input(Some(Ok(r#"schema_version = 1

[[suppressions]]
kind = "test_efficiency"
test = "cli_prints_help"
path = "tests/cli.rs"
owner = "devtools"
reason = "accepted durable policy exception"
scope = "test:tests/cli.rs::cli_prints_help"
created_at = "2026-01-01"
last_seen = "2026-05-01"
review_by = "2026-12-01"
expected_visibility = "suppressed_visible"
static_class = "weakly_exposed"
language = "rust"
"#))));
        assert_eq!(report.status, "healthy");
        assert_eq!(report.records[0].identity, "cli_prints_help@tests/cli.rs");
        let md = render_suppression_health_markdown(&report);
        assert!(md.contains("cli_prints_help@tests/cli.rs"));
    }

    #[test]
    fn explicit_broad_scope_is_flagged() {
        let report = build_suppression_health_report(input(Some(Ok(r#"schema_version = 1

[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:src/lib.rs:1:predicate"
owner = "team"
reason = "repo-wide exception under review"
scope = "repo"
created_at = "2026-01-01"
last_seen = "2026-05-01"
review_by = "2026-12-01"
expected_visibility = "suppressed_visible"
static_class = "weakly_exposed"
language = "rust"
"#))));
        assert_eq!(report.status, "warning");
        assert_eq!(report.summary.overbroad_scope, 1);
        assert_eq!(report.records[0].findings[0].kind, "overbroad_scope");
    }

    #[test]
    fn parser_violations_are_config_errors_with_policy_kinds() {
        let report = build_suppression_health_report(input(Some(Ok(r#"schema_version = 1

[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:src/lib.rs:1:predicate"
reason = "missing owner"

[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:src/lib.rs:2:predicate"
owner = "team"

[[suppressions]]
kind = "wishful"
owner = "team"
reason = "bad selector"

[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:src/lib.rs:3:predicate"
owner = "team"
reason = "bad date"
created_at = "2026/01/01"
"#))));
        assert_eq!(report.status, "config_error");
        assert_eq!(report.summary.missing_owner, 1);
        assert_eq!(report.summary.missing_reason, 1);
        assert_eq!(report.summary.unknown_selector, 1);
        assert!(report.summary.config_errors >= 4);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.kind == "config_error")
        );
    }
}
