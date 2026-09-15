use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "typescript_false_actionable_audit";

pub(crate) const DEFAULT_TYPESCRIPT_FALSE_ACTIONABLE_OUT: &str =
    "target/ripr/reports/typescript-false-actionable-audit.json";
pub(crate) const DEFAULT_TYPESCRIPT_FALSE_ACTIONABLE_MD_OUT: &str =
    "target/ripr/reports/typescript-false-actionable-audit.md";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeScriptFalseActionableAuditInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) corpus_path: String,
    pub(crate) corpus_json: Result<String, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TypeScriptFalseActionableAuditReport {
    status: String,
    root: String,
    generated_at: String,
    inputs: TypeScriptFalseActionableAuditInputs,
    summary: TypeScriptFalseActionableSummary,
    disposition_counts: Vec<CountRow>,
    risk_class_counts: Vec<CountRow>,
    cases: Vec<TypeScriptFalseActionableCaseRow>,
    warnings: Vec<String>,
    limits: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct TypeScriptFalseActionableAuditInputs {
    corpus: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct TypeScriptFalseActionableSummary {
    cases_total: usize,
    must_remain_non_actionable_total: usize,
    repair_packet_ready_true_total: usize,
    actionable_gap_state_total: usize,
    complete_packet_category_total: usize,
    false_actionable_total: usize,
    false_actionable_denominator: usize,
    false_actionable_rate: f64,
    preview_boundary_violation_total: usize,
}

impl Default for TypeScriptFalseActionableSummary {
    fn default() -> Self {
        Self {
            cases_total: 0,
            must_remain_non_actionable_total: 0,
            repair_packet_ready_true_total: 0,
            actionable_gap_state_total: 0,
            complete_packet_category_total: 0,
            false_actionable_total: 0,
            false_actionable_denominator: 0,
            false_actionable_rate: 0.0,
            preview_boundary_violation_total: 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct CountRow {
    value: String,
    count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct TypeScriptFalseActionableCaseRow {
    id: String,
    language: String,
    risk_class: String,
    evidence_kind: String,
    disposition: String,
    gap_state: String,
    actionability_category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    static_limit_kind: Option<String>,
    repair_packet_ready: bool,
    must_remain_non_actionable: bool,
    authority_boundary: String,
    false_actionable: bool,
    source_fixture: String,
    source_finding_id: String,
}

pub(crate) fn build_typescript_false_actionable_audit_report(
    input: TypeScriptFalseActionableAuditInput,
) -> TypeScriptFalseActionableAuditReport {
    let mut warnings = Vec::new();
    let cases = match input.corpus_json {
        Ok(contents) => match parse_false_actionable_cases(&contents) {
            Ok(cases) => cases,
            Err(err) => {
                warnings.push(format!("parse {} failed: {err}", input.corpus_path));
                Vec::new()
            }
        },
        Err(err) => {
            warnings.push(err);
            Vec::new()
        }
    };

    let summary = summarize_false_actionable_cases(&cases);
    let disposition_counts = count_rows(cases.iter().map(|case| case.disposition.as_str()));
    let risk_class_counts = count_rows(cases.iter().map(|case| case.risk_class.as_str()));
    let status = if warnings.is_empty() {
        "advisory"
    } else {
        "blocked"
    }
    .to_string();

    TypeScriptFalseActionableAuditReport {
        status,
        root: input.root,
        generated_at: input.generated_at,
        inputs: TypeScriptFalseActionableAuditInputs {
            corpus: input.corpus_path,
        },
        summary,
        disposition_counts,
        risk_class_counts,
        cases,
        warnings,
        limits: vec![
            "Advisory TypeScript-family preview audit metric only.".to_string(),
            "The false-actionable rate is computed from explicit audit corpus rows; this report does not rerun analysis or execute TypeScript tests.".to_string(),
            "This report does not edit source, generate tests, call providers, run mutation testing, change gates, contribute badge authority, or promote support tiers.".to_string(),
        ],
    }
}

pub(crate) fn render_typescript_false_actionable_audit_json(
    report: &TypeScriptFalseActionableAuditReport,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct JsonReport<'a> {
        schema_version: &'static str,
        tool: &'static str,
        kind: &'static str,
        status: &'a str,
        root: &'a str,
        generated_at: &'a str,
        inputs: &'a TypeScriptFalseActionableAuditInputs,
        summary: &'a TypeScriptFalseActionableSummary,
        disposition_counts: &'a [CountRow],
        risk_class_counts: &'a [CountRow],
        cases: &'a [TypeScriptFalseActionableCaseRow],
        warnings: &'a [String],
        limits: &'a [String],
    }

    serde_json::to_string_pretty(&JsonReport {
        schema_version: SCHEMA_VERSION,
        tool: "ripr",
        kind: REPORT_KIND,
        status: &report.status,
        root: &report.root,
        generated_at: &report.generated_at,
        inputs: &report.inputs,
        summary: &report.summary,
        disposition_counts: &report.disposition_counts,
        risk_class_counts: &report.risk_class_counts,
        cases: &report.cases,
        warnings: &report.warnings,
        limits: &report.limits,
    })
    .map_err(|err| format!("serialize TypeScript false-actionable audit JSON failed: {err}"))
}

pub(crate) fn render_typescript_false_actionable_audit_markdown(
    report: &TypeScriptFalseActionableAuditReport,
) -> String {
    let mut out = String::new();
    out.push_str("# RIPR TypeScript False-Actionable Audit\n\n");
    out.push_str(&format!("Status: `{}`\n\n", md_inline(&report.status)));
    out.push_str(&format!("Root: `{}`\n\n", md_inline(&report.root)));
    out.push_str(
        "Authority: advisory TypeScript-family preview audit only. Gate-decision and badge artifacts keep their existing authority.\n\n",
    );

    out.push_str("## Summary\n\n");
    out.push_str(&format!("- Cases: `{}`\n", report.summary.cases_total));
    out.push_str(&format!(
        "- False actionable: `{}` / `{}` (`{:.3}`)\n",
        report.summary.false_actionable_total,
        report.summary.false_actionable_denominator,
        report.summary.false_actionable_rate
    ));
    out.push_str(&format!(
        "- Repair-packet-ready violations: `{}`; actionable gap-state violations: `{}`; complete-packet category violations: `{}`\n",
        report.summary.repair_packet_ready_true_total,
        report.summary.actionable_gap_state_total,
        report.summary.complete_packet_category_total
    ));
    out.push_str(&format!(
        "- Preview-boundary violations: `{}`\n\n",
        report.summary.preview_boundary_violation_total
    ));

    if !report.warnings.is_empty() {
        out.push_str("## Warnings\n\n");
        for warning in &report.warnings {
            out.push_str(&format!("- {}\n", md_inline(warning)));
        }
        out.push('\n');
    }

    render_count_table("Disposition Counts", &report.disposition_counts, &mut out);
    render_count_table("Risk Class Counts", &report.risk_class_counts, &mut out);

    out.push_str("## Cases\n\n");
    if report.cases.is_empty() {
        out.push_str("No audit cases were supplied.\n\n");
    } else {
        out.push_str("| case | disposition | risk | false actionable |\n");
        out.push_str("| --- | --- | --- | --- |\n");
        for case in &report.cases {
            out.push_str(&format!(
                "| `{}` | `{}` | {} | `{}` |\n",
                md_inline(&case.id),
                md_inline(&case.disposition),
                md_inline(&case.risk_class),
                case.false_actionable
            ));
        }
        out.push('\n');
    }

    out.push_str("## Limits\n\n");
    for limit in &report.limits {
        out.push_str(&format!("- {}\n", md_inline(limit)));
    }
    out
}

fn parse_false_actionable_cases(
    contents: &str,
) -> Result<Vec<TypeScriptFalseActionableCaseRow>, String> {
    let value: Value =
        serde_json::from_str(contents).map_err(|err| format!("invalid JSON: {err}"))?;
    if string_at(&value, &["schema_version"]) != Some("0.1") {
        return Err("expected schema_version 0.1".to_string());
    }
    if string_at(&value, &["kind"]) != Some("typescript_preview_false_actionable_audit_corpus") {
        return Err("expected kind typescript_preview_false_actionable_audit_corpus".to_string());
    }
    let cases = value
        .get("cases")
        .and_then(Value::as_array)
        .ok_or_else(|| "expected corpus object with cases array".to_string())?;

    cases
        .iter()
        .enumerate()
        .map(|(index, value)| case_row_from_value(value, index))
        .collect()
}

fn case_row_from_value(
    value: &Value,
    index: usize,
) -> Result<TypeScriptFalseActionableCaseRow, String> {
    let id = required_string_at(value, &["id"], index)?;
    let language = required_string_at(value, &["language"], index)?;
    let risk_class = required_string_at(value, &["risk_class"], index)?;
    let evidence_kind = required_string_at(value, &["evidence_kind"], index)?;
    let disposition = required_string_at(value, &["disposition"], index)?;
    let gap_state = required_string_at(value, &["gap_state"], index)?;
    let actionability_category = required_string_at(value, &["actionability_category"], index)?;
    let repair_packet_ready = required_bool_at(value, &["repair_packet_ready"], index)?;
    let must_remain_non_actionable =
        required_bool_at(value, &["must_remain_non_actionable"], index)?;
    let authority_boundary = required_string_at(value, &["authority_boundary"], index)?;
    let source_fixture = required_string_at(value, &["source_fixture"], index)?;
    let source_finding_id = required_string_at(value, &["source_finding_id"], index)?;
    let false_actionable = must_remain_non_actionable
        && (repair_packet_ready
            || gap_state == "actionable"
            || actionability_category == "complete_repair_packet");

    Ok(TypeScriptFalseActionableCaseRow {
        id: id.to_string(),
        language: language.to_string(),
        risk_class: risk_class.to_string(),
        evidence_kind: evidence_kind.to_string(),
        disposition: disposition.to_string(),
        gap_state: gap_state.to_string(),
        actionability_category: actionability_category.to_string(),
        static_limit_kind: string_at(value, &["static_limit_kind"]).map(ToString::to_string),
        repair_packet_ready,
        must_remain_non_actionable,
        authority_boundary: authority_boundary.to_string(),
        false_actionable,
        source_fixture: source_fixture.to_string(),
        source_finding_id: source_finding_id.to_string(),
    })
}

fn summarize_false_actionable_cases(
    cases: &[TypeScriptFalseActionableCaseRow],
) -> TypeScriptFalseActionableSummary {
    let mut summary = TypeScriptFalseActionableSummary {
        cases_total: cases.len(),
        ..TypeScriptFalseActionableSummary::default()
    };
    for case in cases {
        if case.must_remain_non_actionable {
            summary.must_remain_non_actionable_total += 1;
        }
        if case.repair_packet_ready {
            summary.repair_packet_ready_true_total += 1;
        }
        if case.gap_state == "actionable" {
            summary.actionable_gap_state_total += 1;
        }
        if case.actionability_category == "complete_repair_packet" {
            summary.complete_packet_category_total += 1;
        }
        if case.false_actionable {
            summary.false_actionable_total += 1;
        }
        if case.authority_boundary != "preview_advisory_only" {
            summary.preview_boundary_violation_total += 1;
        }
    }
    summary.false_actionable_denominator = summary.must_remain_non_actionable_total;
    if summary.false_actionable_denominator > 0 {
        summary.false_actionable_rate =
            summary.false_actionable_total as f64 / summary.false_actionable_denominator as f64;
    }
    summary
}

fn count_rows<'a>(values: impl Iterator<Item = &'a str>) -> Vec<CountRow> {
    let mut counts = BTreeMap::<String, usize>::new();
    for value in values {
        *counts.entry(value.to_string()).or_default() += 1;
    }
    let mut rows: Vec<CountRow> = counts
        .into_iter()
        .map(|(value, count)| CountRow { value, count })
        .collect();
    rows.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.value.cmp(&right.value))
    });
    rows
}

fn render_count_table(title: &str, rows: &[CountRow], out: &mut String) {
    out.push_str(&format!("## {title}\n\n"));
    if rows.is_empty() {
        out.push_str("No rows.\n\n");
        return;
    }
    out.push_str("| value | count |\n");
    out.push_str("| --- | ---: |\n");
    for row in rows {
        out.push_str(&format!(
            "| `{}` | `{}` |\n",
            md_inline(&row.value),
            row.count
        ));
    }
    out.push('\n');
}

fn string_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str()
}

fn bool_at(value: &Value, path: &[&str]) -> Option<bool> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_bool()
}

fn required_string_at<'a>(
    value: &'a Value,
    path: &[&str],
    case_index: usize,
) -> Result<&'a str, String> {
    string_at(value, path).ok_or_else(|| {
        format!(
            "case {case_index} missing or non-string field {}",
            path.join(".")
        )
    })
}

fn required_bool_at(value: &Value, path: &[&str], case_index: usize) -> Result<bool, String> {
    bool_at(value, path).ok_or_else(|| {
        format!(
            "case {case_index} missing or non-boolean field {}",
            path.join(".")
        )
    })
}

fn md_inline(value: &str) -> String {
    value.replace('|', "\\|").replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn false_actionable_audit_reports_zero_rate_for_safe_corpus() -> Result<(), String> {
        let report =
            build_typescript_false_actionable_audit_report(TypeScriptFalseActionableAuditInput {
                root: ".".to_string(),
                generated_at: "123".to_string(),
                corpus_path: "corpus.json".to_string(),
                corpus_json: Ok(safe_corpus()),
            });

        assert_eq!(report.status, "advisory");
        assert_eq!(report.summary.cases_total, 2);
        assert_eq!(report.summary.must_remain_non_actionable_total, 2);
        assert_eq!(report.summary.false_actionable_total, 0);
        assert_eq!(report.summary.false_actionable_denominator, 2);
        assert!(report.summary.false_actionable_rate.abs() < f64::EPSILON);
        assert!(
            report
                .disposition_counts
                .iter()
                .any(|row| row.value == "safe_advisory" && row.count == 1)
        );
        Ok(())
    }

    #[test]
    fn false_actionable_audit_counts_packet_ready_and_complete_category() {
        let report =
            build_typescript_false_actionable_audit_report(TypeScriptFalseActionableAuditInput {
                root: ".".to_string(),
                generated_at: "123".to_string(),
                corpus_path: "corpus.json".to_string(),
                corpus_json: Ok(false_actionable_corpus()),
            });

        assert_eq!(report.summary.cases_total, 2);
        assert_eq!(report.summary.repair_packet_ready_true_total, 1);
        assert_eq!(report.summary.complete_packet_category_total, 1);
        assert_eq!(report.summary.false_actionable_total, 2);
        assert!((report.summary.false_actionable_rate - 1.0).abs() < f64::EPSILON);
        assert!(report.cases.iter().all(|case| case.false_actionable));
    }

    #[test]
    fn false_actionable_audit_renders_json_and_markdown() -> Result<(), String> {
        let report =
            build_typescript_false_actionable_audit_report(TypeScriptFalseActionableAuditInput {
                root: ".".to_string(),
                generated_at: "123".to_string(),
                corpus_path: "corpus.json".to_string(),
                corpus_json: Ok(safe_corpus()),
            });

        let json_text = render_typescript_false_actionable_audit_json(&report)?;
        assert!(json_text.contains("\"kind\": \"typescript_false_actionable_audit\""));
        assert!(json_text.contains("\"false_actionable_rate\": 0.0"));

        let markdown = render_typescript_false_actionable_audit_markdown(&report);
        assert!(markdown.contains("# RIPR TypeScript False-Actionable Audit"));
        assert!(markdown.contains("False actionable: `0` / `2` (`0.000`)"));
        assert!(
            markdown.contains("Gate-decision and badge artifacts keep their existing authority")
        );
        Ok(())
    }

    #[test]
    fn false_actionable_audit_blocks_on_malformed_corpus() {
        let report =
            build_typescript_false_actionable_audit_report(TypeScriptFalseActionableAuditInput {
                root: ".".to_string(),
                generated_at: "123".to_string(),
                corpus_path: "corpus.json".to_string(),
                corpus_json: Ok("{}".to_string()),
            });

        assert_eq!(report.status, "blocked");
        assert_eq!(report.summary.cases_total, 0);
        assert!(report.warnings[0].contains("expected schema_version 0.1"));
    }

    #[test]
    fn false_actionable_audit_blocks_on_incomplete_case_row() {
        let report =
            build_typescript_false_actionable_audit_report(TypeScriptFalseActionableAuditInput {
                root: ".".to_string(),
                generated_at: "123".to_string(),
                corpus_path: "corpus.json".to_string(),
                corpus_json: Ok(incomplete_case_corpus()),
            });

        assert_eq!(report.status, "blocked");
        assert_eq!(report.summary.cases_total, 0);
        assert!(
            report.warnings[0].contains("missing or non-boolean field must_remain_non_actionable")
        );
    }

    fn safe_corpus() -> String {
        r#"{
  "schema_version": "0.1",
  "kind": "typescript_preview_false_actionable_audit_corpus",
  "cases": [
    {
      "id": "snapshot_only_weak_oracle",
      "language": "typescript",
      "risk_class": "snapshot-only weak oracle",
      "evidence_kind": "snapshot",
      "disposition": "safe_advisory",
      "gap_state": "advisory",
      "actionability_category": "incomplete_repair_packet",
      "static_limit_kind": null,
      "repair_packet_ready": false,
      "must_remain_non_actionable": true,
      "authority_boundary": "preview_advisory_only",
      "source_fixture": "fixtures/typescript_jest_vitest_assertion_facts",
      "source_finding_id": "probe:one"
    },
    {
      "id": "dynamic_dispatch_limit",
      "language": "javascript",
      "risk_class": "dynamic dispatch limit",
      "evidence_kind": "dynamic_dispatch",
      "disposition": "named_static_limitation",
      "gap_state": "static_limitation",
      "actionability_category": "dynamic_dispatch",
      "static_limit_kind": "dynamic_dispatch",
      "repair_packet_ready": false,
      "must_remain_non_actionable": true,
      "authority_boundary": "preview_advisory_only",
      "source_fixture": "fixtures/ts_static_limit",
      "source_finding_id": "probe:two"
    }
  ]
}"#
        .to_string()
    }

    fn false_actionable_corpus() -> String {
        r#"{
  "schema_version": "0.1",
  "kind": "typescript_preview_false_actionable_audit_corpus",
  "cases": [
    {
      "id": "bad_packet_ready",
      "language": "typescript",
      "risk_class": "bad packet ready",
      "evidence_kind": "mock",
      "disposition": "safe_advisory",
      "gap_state": "advisory",
      "actionability_category": "incomplete_repair_packet",
      "repair_packet_ready": true,
      "must_remain_non_actionable": true,
      "authority_boundary": "preview_advisory_only",
      "source_fixture": "fixtures/x",
      "source_finding_id": "probe:bad"
    },
    {
      "id": "bad_complete_category",
      "language": "typescript",
      "risk_class": "bad complete category",
      "evidence_kind": "mock",
      "disposition": "safe_advisory",
      "gap_state": "advisory",
      "actionability_category": "complete_repair_packet",
      "repair_packet_ready": false,
      "must_remain_non_actionable": true,
      "authority_boundary": "preview_advisory_only",
      "source_fixture": "fixtures/x",
      "source_finding_id": "probe:bad2"
    }
  ]
}"#
        .to_string()
    }

    fn incomplete_case_corpus() -> String {
        r#"{
  "schema_version": "0.1",
  "kind": "typescript_preview_false_actionable_audit_corpus",
  "cases": [
    {
      "id": "bad_missing_denominator",
      "language": "typescript",
      "risk_class": "bad packet ready",
      "evidence_kind": "mock",
      "disposition": "safe_advisory",
      "gap_state": "actionable",
      "actionability_category": "complete_repair_packet",
      "repair_packet_ready": true,
      "authority_boundary": "preview_advisory_only",
      "source_fixture": "fixtures/x",
      "source_finding_id": "probe:bad"
    }
  ]
}"#
        .to_string()
    }
}
