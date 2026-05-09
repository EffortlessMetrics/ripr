use serde_json::{Value, json};
use std::path::Path;

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "coverage_grip_frontier";
const LIMITS_NOTE: &str = "Coverage is execution evidence; RIPR is static behavioral grip evidence. This report is advisory and does not claim test adequacy or runtime mutation outcomes.";
pub(crate) const DEFAULT_COVERAGE_GRIP_FRONTIER_OUT: &str =
    "target/ripr/reports/coverage-grip-frontier.json";
pub(crate) const DEFAULT_COVERAGE_GRIP_FRONTIER_MD_OUT: &str =
    "target/ripr/reports/coverage-grip-frontier.md";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoverageGripFrontierInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) coverage_path: Option<String>,
    pub(crate) ledger_path: Option<String>,
    pub(crate) baseline_delta_path: Option<String>,
    pub(crate) zero_status_path: Option<String>,
    pub(crate) coverage_json: Option<Result<String, String>>,
    pub(crate) ledger_json: Option<Result<String, String>>,
    pub(crate) baseline_delta_json: Option<Result<String, String>>,
    pub(crate) zero_status_json: Option<Result<String, String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoverageGripFrontierReport {
    root: String,
    generated_at: String,
    status: String,
    inputs: FrontierInputs,
    coverage: CoverageAxis,
    ripr: RiprAxis,
    quadrants: CoverageQuadrants,
    interpretation: String,
    warnings: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FrontierInputs {
    coverage: Option<String>,
    pr_evidence_ledger: Option<String>,
    baseline_debt_delta: Option<String>,
    ripr_zero_status: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoverageAxis {
    status: String,
    delta_percent: Option<String>,
    source: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiprAxis {
    source: Option<String>,
    new_policy_eligible: usize,
    baseline_resolved: usize,
    baseline_still_present: usize,
    acknowledged: usize,
    suppressed: usize,
    blocking_candidates: usize,
    visible_unresolved: usize,
    visible_unresolved_delta: Option<i64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CoverageQuadrants {
    covered_with_ripr_gap: usize,
    covered_without_ripr_gap: usize,
    uncovered_with_ripr_gap: usize,
    uncovered_without_ripr_gap: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ParsedSources {
    coverage: Option<Value>,
    ledger: Option<Value>,
    baseline_delta: Option<Value>,
    zero_status: Option<Value>,
    warnings: Vec<String>,
}

pub(crate) fn build_coverage_grip_frontier_report(
    input: CoverageGripFrontierInput,
) -> CoverageGripFrontierReport {
    let parsed = parse_sources(&input);
    let coverage = coverage_axis(input.coverage_path.as_deref(), parsed.coverage.as_ref());
    let ripr = ripr_axis(
        parsed.ledger.as_ref(),
        parsed.baseline_delta.as_ref(),
        parsed.zero_status.as_ref(),
        parsed.coverage.as_ref(),
    );
    let quadrants = coverage_quadrants(parsed.coverage.as_ref());
    let mut warnings = parsed.warnings;
    if coverage.status == "not_available" {
        warnings.push("coverage input not supplied; coverage axis is not available".to_string());
    } else if coverage.status == "unsupported" {
        warnings.push("coverage input has no supported coverage/grip frontier fields".to_string());
    }
    if ripr.source.is_none() {
        warnings.push(
            "RIPR movement input missing; provide PR evidence ledger, baseline delta, or RIPR Zero status"
                .to_string(),
        );
    }
    let interpretation = interpretation(&coverage, &ripr);
    let status = if ripr.source.is_some() {
        "advisory"
    } else {
        "incomplete"
    }
    .to_string();

    CoverageGripFrontierReport {
        root: input.root,
        generated_at: input.generated_at,
        status,
        inputs: FrontierInputs {
            coverage: input.coverage_path,
            pr_evidence_ledger: input.ledger_path,
            baseline_debt_delta: input.baseline_delta_path,
            ripr_zero_status: input.zero_status_path,
        },
        coverage,
        ripr,
        quadrants,
        interpretation,
        warnings,
    }
}

pub(crate) fn render_coverage_grip_frontier_json(
    report: &CoverageGripFrontierReport,
) -> Result<String, String> {
    serde_json::to_string_pretty(&json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "kind": REPORT_KIND,
        "status": report.status,
        "root": report.root,
        "generated_at": report.generated_at,
        "inputs": {
            "coverage": report.inputs.coverage,
            "pr_evidence_ledger": report.inputs.pr_evidence_ledger,
            "baseline_debt_delta": report.inputs.baseline_debt_delta,
            "ripr_zero_status": report.inputs.ripr_zero_status,
        },
        "coverage": {
            "status": report.coverage.status,
            "delta_percent": report.coverage.delta_percent,
            "source": report.coverage.source,
        },
        "ripr": {
            "source": report.ripr.source,
            "new_policy_eligible": report.ripr.new_policy_eligible,
            "baseline_resolved": report.ripr.baseline_resolved,
            "baseline_still_present": report.ripr.baseline_still_present,
            "acknowledged": report.ripr.acknowledged,
            "suppressed": report.ripr.suppressed,
            "blocking_candidates": report.ripr.blocking_candidates,
            "visible_unresolved": report.ripr.visible_unresolved,
            "visible_unresolved_delta": report.ripr.visible_unresolved_delta,
        },
        "quadrants": {
            "covered_with_ripr_gap": report.quadrants.covered_with_ripr_gap,
            "covered_without_ripr_gap": report.quadrants.covered_without_ripr_gap,
            "uncovered_with_ripr_gap": report.quadrants.uncovered_with_ripr_gap,
            "uncovered_without_ripr_gap": report.quadrants.uncovered_without_ripr_gap,
        },
        "interpretation": report.interpretation,
        "warnings": report.warnings,
        "limits_note": LIMITS_NOTE,
    }))
    .map_err(|err| format!("render coverage/grip frontier JSON failed: {err}"))
}

pub(crate) fn render_coverage_grip_frontier_markdown(
    report: &CoverageGripFrontierReport,
) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Coverage / Grip Frontier\n\n");
    out.push_str(&format!("Status: {}\n\n", report.status));
    out.push_str("Coverage axis:\n");
    out.push_str(&format!("- Status: {}\n", report.coverage.status));
    out.push_str(&format!(
        "- Delta percent: {}\n",
        report
            .coverage
            .delta_percent
            .as_deref()
            .unwrap_or("not_available")
    ));
    out.push_str(&format!(
        "- Source: {}\n\n",
        report.coverage.source.as_deref().unwrap_or("not_available")
    ));

    out.push_str("RIPR axis:\n");
    out.push_str(&format!(
        "- Source: {}\n",
        report.ripr.source.as_deref().unwrap_or("not_available")
    ));
    out.push_str(&format!(
        "- New policy-eligible gaps: {}\n",
        report.ripr.new_policy_eligible
    ));
    out.push_str(&format!(
        "- Baseline gaps resolved: {}\n",
        report.ripr.baseline_resolved
    ));
    out.push_str(&format!(
        "- Existing baseline gaps still present: {}\n",
        report.ripr.baseline_still_present
    ));
    out.push_str(&format!(
        "- Acknowledged gaps: {}\n",
        report.ripr.acknowledged
    ));
    out.push_str(&format!("- Suppressed gaps: {}\n", report.ripr.suppressed));
    out.push_str(&format!(
        "- Blocking candidates: {}\n",
        report.ripr.blocking_candidates
    ));
    out.push_str(&format!(
        "- Visible unresolved gaps: {}\n",
        report.ripr.visible_unresolved
    ));
    out.push_str(&format!(
        "- Visible unresolved delta: {}\n\n",
        report
            .ripr
            .visible_unresolved_delta
            .map(|value| value.to_string())
            .unwrap_or_else(|| "not_available".to_string())
    ));

    out.push_str("Quadrants:\n");
    out.push_str(&format!(
        "- Covered with RIPR gap: {}\n",
        report.quadrants.covered_with_ripr_gap
    ));
    out.push_str(&format!(
        "- Covered without RIPR gap: {}\n",
        report.quadrants.covered_without_ripr_gap
    ));
    out.push_str(&format!(
        "- Uncovered with RIPR gap: {}\n",
        report.quadrants.uncovered_with_ripr_gap
    ));
    out.push_str(&format!(
        "- Uncovered without RIPR gap: {}\n\n",
        report.quadrants.uncovered_without_ripr_gap
    ));

    out.push_str("Interpretation:\n");
    out.push_str(&format!("- {}\n\n", report.interpretation));
    if !report.warnings.is_empty() {
        out.push_str("Warnings:\n");
        for warning in &report.warnings {
            out.push_str(&format!("- {warning}\n"));
        }
        out.push('\n');
    }
    out.push_str("Limits:\n");
    out.push_str(LIMITS_NOTE);
    out.push('\n');
    out
}

pub(crate) fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn parse_sources(input: &CoverageGripFrontierInput) -> ParsedSources {
    let mut parsed = ParsedSources::default();
    parsed.coverage = parse_optional("coverage", &input.coverage_json, &mut parsed.warnings);
    parsed.ledger = parse_optional(
        "PR evidence ledger",
        &input.ledger_json,
        &mut parsed.warnings,
    );
    parsed.baseline_delta = parse_optional(
        "baseline debt delta",
        &input.baseline_delta_json,
        &mut parsed.warnings,
    );
    parsed.zero_status = parse_optional(
        "RIPR Zero status",
        &input.zero_status_json,
        &mut parsed.warnings,
    );
    parsed
}

fn parse_optional(
    label: &str,
    source: &Option<Result<String, String>>,
    warnings: &mut Vec<String>,
) -> Option<Value> {
    match source {
        Some(Ok(text)) => match serde_json::from_str::<Value>(text) {
            Ok(value) => Some(value),
            Err(err) => {
                warnings.push(format!("{label} input is invalid JSON: {err}"));
                None
            }
        },
        Some(Err(err)) => {
            warnings.push(err.clone());
            None
        }
        None => None,
    }
}

fn coverage_axis(path: Option<&str>, coverage: Option<&Value>) -> CoverageAxis {
    let Some(coverage) = coverage else {
        return CoverageAxis {
            status: "not_available".to_string(),
            delta_percent: None,
            source: None,
        };
    };
    let delta_percent = string_or_number_path(coverage, &["coverage_delta_percent"])
        .or_else(|| string_or_number_path(coverage, &["coverage", "delta_percent"]))
        .or_else(|| string_or_number_path(coverage, &["summary", "coverage_delta_percent"]));
    let has_quadrants = has_any_path(
        coverage,
        &[
            &["quadrants", "covered_with_ripr_gap"],
            &["covered_with_ripr_gap"],
            &["quadrants", "covered_without_ripr_gap"],
            &["covered_without_ripr_gap"],
            &["quadrants", "uncovered_with_ripr_gap"],
            &["uncovered_with_ripr_gap"],
            &["quadrants", "uncovered_without_ripr_gap"],
            &["uncovered_without_ripr_gap"],
        ],
    );
    let status = if delta_percent.is_some() || has_quadrants {
        "available"
    } else {
        "unsupported"
    };
    CoverageAxis {
        status: status.to_string(),
        delta_percent,
        source: path.map(ToOwned::to_owned),
    }
}

fn ripr_axis(
    ledger: Option<&Value>,
    baseline_delta: Option<&Value>,
    zero_status: Option<&Value>,
    coverage: Option<&Value>,
) -> RiprAxis {
    let source = if ledger.is_some_and(has_ledger_movement) {
        Some("pr_evidence_ledger".to_string())
    } else if zero_status.is_some_and(has_zero_status_movement) {
        Some("ripr_zero_status".to_string())
    } else if baseline_delta.is_some_and(has_baseline_delta_movement) {
        Some("baseline_debt_delta".to_string())
    } else {
        None
    };
    let new_policy_eligible = usize_path_from_sources(&[
        (ledger, &["movement", "new_policy_eligible"]),
        (zero_status, &["ripr_zero", "new_policy_eligible"]),
        (baseline_delta, &["delta", "new_policy_eligible"]),
    ])
    .unwrap_or(0);
    let baseline_resolved = usize_path_from_sources(&[
        (ledger, &["movement", "baseline_resolved"]),
        (zero_status, &["baseline", "resolved"]),
        (baseline_delta, &["delta", "resolved"]),
    ])
    .unwrap_or(0);
    let baseline_still_present = usize_path_from_sources(&[
        (ledger, &["movement", "baseline_still_present"]),
        (zero_status, &["baseline", "still_present"]),
        (baseline_delta, &["delta", "still_present"]),
    ])
    .unwrap_or(0);
    let acknowledged = usize_path_from_sources(&[
        (ledger, &["movement", "acknowledged"]),
        (baseline_delta, &["delta", "acknowledged"]),
    ])
    .unwrap_or(0);
    let suppressed = usize_path_from_sources(&[
        (ledger, &["movement", "suppressed"]),
        (baseline_delta, &["delta", "suppressed"]),
    ])
    .unwrap_or(0);
    let blocking_candidates = usize_path_from_sources(&[
        (ledger, &["movement", "blocking_candidates"]),
        (zero_status, &["ripr_zero", "blocking_candidates"]),
    ])
    .unwrap_or(0);
    let visible_unresolved = usize_path_from_sources(&[
        (ledger, &["movement", "visible_unresolved"]),
        (zero_status, &["ripr_zero", "visible_unresolved"]),
    ])
    .unwrap_or(baseline_still_present + new_policy_eligible + acknowledged);
    let visible_unresolved_delta = coverage
        .and_then(|value| i64_path(value, &["ripr_visible_unresolved_delta"]))
        .or_else(|| {
            coverage.and_then(|value| i64_path(value, &["ripr", "visible_unresolved_delta"]))
        })
        .or_else(|| {
            ledger.and_then(|value| {
                i64_path(
                    value,
                    &["coverage_grip_frontier", "ripr_visible_unresolved_delta"],
                )
            })
        });

    RiprAxis {
        source,
        new_policy_eligible,
        baseline_resolved,
        baseline_still_present,
        acknowledged,
        suppressed,
        blocking_candidates,
        visible_unresolved,
        visible_unresolved_delta,
    }
}

fn coverage_quadrants(coverage: Option<&Value>) -> CoverageQuadrants {
    let Some(coverage) = coverage else {
        return CoverageQuadrants::default();
    };
    CoverageQuadrants {
        covered_with_ripr_gap: usize_path_any(
            coverage,
            &[
                &["quadrants", "covered_with_ripr_gap"],
                &["covered_with_ripr_gap"],
            ],
        ),
        covered_without_ripr_gap: usize_path_any(
            coverage,
            &[
                &["quadrants", "covered_without_ripr_gap"],
                &["covered_without_ripr_gap"],
            ],
        ),
        uncovered_with_ripr_gap: usize_path_any(
            coverage,
            &[
                &["quadrants", "uncovered_with_ripr_gap"],
                &["uncovered_with_ripr_gap"],
            ],
        ),
        uncovered_without_ripr_gap: usize_path_any(
            coverage,
            &[
                &["quadrants", "uncovered_without_ripr_gap"],
                &["uncovered_without_ripr_gap"],
            ],
        ),
    }
}

fn interpretation(coverage: &CoverageAxis, ripr: &RiprAxis) -> String {
    if coverage.status == "not_available" {
        return "coverage input not supplied; behavioral grip movement remains visible on the RIPR axis"
            .to_string();
    }
    if coverage.status == "unsupported" {
        return "coverage input is present but unsupported; keep coverage and RIPR movement separate"
            .to_string();
    }
    if coverage_delta_is_zero(coverage.delta_percent.as_deref()) {
        if ripr.baseline_resolved > 0 || ripr.visible_unresolved_delta.unwrap_or(0) < 0 {
            return "behavioral grip improved without line-coverage movement".to_string();
        }
        return "coverage was flat; inspect RIPR movement separately".to_string();
    }
    if ripr.new_policy_eligible > 0 {
        return "coverage changed while new policy-eligible RIPR gaps remain visible".to_string();
    }
    "coverage and RIPR movement are reported separately; coverage is execution evidence, not adequacy"
        .to_string()
}

fn path_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn string_or_number_path(value: &Value, path: &[&str]) -> Option<String> {
    let value = path_value(value, path)?;
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    if let Some(number) = value.as_i64() {
        return Some(number.to_string());
    }
    value.as_f64().map(|number| number.to_string())
}

fn i64_path(value: &Value, path: &[&str]) -> Option<i64> {
    path_value(value, path).and_then(value_as_i64)
}

fn usize_path(value: &Value, path: &[&str]) -> Option<usize> {
    path_value(value, path).and_then(value_as_usize)
}

fn usize_path_from_sources(sources: &[(Option<&Value>, &[&str])]) -> Option<usize> {
    sources
        .iter()
        .find_map(|(value, path)| value.and_then(|value| usize_path(value, path)))
}

fn usize_path_any(value: &Value, paths: &[&[&str]]) -> usize {
    paths
        .iter()
        .find_map(|path| usize_path(value, path))
        .unwrap_or(0)
}

fn value_as_i64(value: &Value) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return Some(number);
    }
    if let Some(number) = value.as_u64() {
        return i64::try_from(number).ok();
    }
    if let Some(text) = value.as_str() {
        return text
            .trim()
            .parse::<i64>()
            .ok()
            .or_else(|| i64_from_f64_text(text));
    }
    value.as_f64().and_then(i64_from_f64)
}

fn value_as_usize(value: &Value) -> Option<usize> {
    if let Some(number) = value.as_u64() {
        return usize::try_from(number).ok();
    }
    if let Some(number) = value.as_i64() {
        return u64::try_from(number)
            .ok()
            .and_then(|number| usize::try_from(number).ok());
    }
    if let Some(text) = value.as_str() {
        return text
            .trim()
            .parse::<u64>()
            .ok()
            .and_then(|number| usize::try_from(number).ok())
            .or_else(|| usize_from_f64_text(text));
    }
    value.as_f64().and_then(usize_from_f64)
}

fn i64_from_f64_text(text: &str) -> Option<i64> {
    text.trim().parse::<f64>().ok().and_then(i64_from_f64)
}

fn usize_from_f64_text(text: &str) -> Option<usize> {
    text.trim().parse::<f64>().ok().and_then(usize_from_f64)
}

fn i64_from_f64(number: f64) -> Option<i64> {
    if number.is_finite() && number.fract() == 0.0 {
        return format!("{number:.0}").parse::<i64>().ok();
    }
    None
}

fn usize_from_f64(number: f64) -> Option<usize> {
    if number.is_finite() && number.fract() == 0.0 && number >= 0.0 {
        return format!("{number:.0}").parse::<usize>().ok();
    }
    None
}

fn coverage_delta_is_zero(delta_percent: Option<&str>) -> bool {
    delta_percent
        .and_then(|value| value.trim().parse::<f64>().ok())
        .is_some_and(|value| value.is_finite() && value == 0.0)
}

fn has_any_path(value: &Value, paths: &[&[&str]]) -> bool {
    paths.iter().any(|path| path_value(value, path).is_some())
}

fn has_ledger_movement(value: &Value) -> bool {
    has_any_path(
        value,
        &[
            &["movement", "new_policy_eligible"],
            &["movement", "baseline_resolved"],
            &["movement", "baseline_still_present"],
            &["movement", "acknowledged"],
            &["movement", "suppressed"],
            &["movement", "blocking_candidates"],
            &["movement", "visible_unresolved"],
        ],
    )
}

fn has_zero_status_movement(value: &Value) -> bool {
    has_any_path(
        value,
        &[
            &["ripr_zero", "new_policy_eligible"],
            &["ripr_zero", "blocking_candidates"],
            &["ripr_zero", "visible_unresolved"],
            &["baseline", "resolved"],
            &["baseline", "still_present"],
        ],
    )
}

fn has_baseline_delta_movement(value: &Value) -> bool {
    has_any_path(
        value,
        &[
            &["delta", "new_policy_eligible"],
            &["delta", "resolved"],
            &["delta", "still_present"],
            &["delta", "acknowledged"],
            &["delta", "suppressed"],
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report_with(
        coverage_json: Option<Result<String, String>>,
        ledger_json: Option<Result<String, String>>,
        baseline_delta_json: Option<Result<String, String>>,
        zero_status_json: Option<Result<String, String>>,
    ) -> CoverageGripFrontierReport {
        build_coverage_grip_frontier_report(CoverageGripFrontierInput {
            root: ".".to_string(),
            generated_at: "unix_ms:1000".to_string(),
            coverage_path: coverage_json
                .as_ref()
                .map(|_| "target/ripr/reports/coverage-summary.json".to_string()),
            ledger_path: ledger_json
                .as_ref()
                .map(|_| "target/ripr/reports/pr-evidence-ledger.json".to_string()),
            baseline_delta_path: baseline_delta_json
                .as_ref()
                .map(|_| "target/ripr/reports/baseline-debt-delta.json".to_string()),
            zero_status_path: zero_status_json
                .as_ref()
                .map(|_| "target/ripr/reports/ripr-zero-status.json".to_string()),
            coverage_json,
            ledger_json,
            baseline_delta_json,
            zero_status_json,
        })
    }

    #[test]
    fn coverage_grip_frontier_reports_flat_coverage_with_improved_grip() -> Result<(), String> {
        let coverage = r#"{
          "coverage_delta_percent": 0.0,
          "ripr_visible_unresolved_delta": -3,
          "quadrants": {
            "covered_with_ripr_gap": 4,
            "covered_without_ripr_gap": 20,
            "uncovered_with_ripr_gap": 2,
            "uncovered_without_ripr_gap": 8
          }
        }"#;
        let ledger = r#"{
          "movement": {
            "new_policy_eligible": 1,
            "baseline_resolved": 3,
            "baseline_still_present": 2,
            "acknowledged": 1,
            "suppressed": 0,
            "blocking_candidates": 0,
            "visible_unresolved": 4
          }
        }"#;
        let report = report_with(
            Some(Ok(coverage.to_string())),
            Some(Ok(ledger.to_string())),
            None,
            None,
        );

        let rendered = render_coverage_grip_frontier_json(&report)?;
        assert!(rendered.contains(r#""kind": "coverage_grip_frontier""#));
        assert!(rendered.contains(r#""delta_percent": "0""#));
        assert!(rendered.contains(r#""visible_unresolved_delta": -3"#));
        assert!(rendered.contains("behavioral grip improved without line-coverage movement"));
        let markdown = render_coverage_grip_frontier_markdown(&report);
        assert!(markdown.contains("Coverage axis:"));
        assert!(markdown.contains("Baseline gaps resolved: 3"));
        assert!(markdown.contains("Covered with RIPR gap: 4"));
        Ok(())
    }

    #[test]
    fn coverage_grip_frontier_keeps_missing_coverage_advisory() -> Result<(), String> {
        let delta = r#"{"delta":{"new_policy_eligible":2,"resolved":0,"still_present":5,"acknowledged":1,"suppressed":1}}"#;
        let report = report_with(None, None, Some(Ok(delta.to_string())), None);
        let rendered = render_coverage_grip_frontier_json(&report)?;

        assert!(rendered.contains(r#""status": "not_available""#));
        assert!(rendered.contains(r#""source": "baseline_debt_delta""#));
        assert!(rendered.contains("coverage input not supplied"));
        assert!(
            rendered.contains(
                "Coverage is execution evidence; RIPR is static behavioral grip evidence"
            )
        );
        Ok(())
    }

    #[test]
    fn coverage_grip_frontier_reports_incomplete_without_ripr_movement() -> Result<(), String> {
        let report = report_with(
            Some(Ok(r#"{"coverage_delta_percent": 2.0}"#.to_string())),
            None,
            None,
            None,
        );
        let rendered = render_coverage_grip_frontier_json(&report)?;

        assert!(rendered.contains(r#""status": "incomplete""#));
        assert!(rendered.contains("RIPR movement input missing"));
        assert!(rendered.contains("coverage is execution evidence, not adequacy"));
        Ok(())
    }

    #[test]
    fn coverage_grip_frontier_warns_on_invalid_inputs() -> Result<(), String> {
        let zero = r#"{"ripr_zero":{"visible_unresolved":1,"new_policy_eligible":0,"blocking_candidates":0},"baseline":{"resolved":1,"still_present":1}}"#;
        let report = report_with(
            Some(Ok("{}".to_string())),
            Some(Err("read ledger failed".to_string())),
            None,
            Some(Ok(zero.to_string())),
        );
        let rendered = render_coverage_grip_frontier_json(&report)?;

        assert!(rendered.contains("read ledger failed"));
        assert!(rendered.contains("coverage input has no supported coverage/grip frontier fields"));
        assert!(rendered.contains(r#""source": "ripr_zero_status""#));
        Ok(())
    }

    #[test]
    fn coverage_grip_frontier_parses_string_encoded_numbers() -> Result<(), String> {
        let coverage = r#"{
          "coverage_delta_percent": "0.00",
          "ripr_visible_unresolved_delta": "-2",
          "quadrants": {
            "covered_with_ripr_gap": "4",
            "covered_without_ripr_gap": "20",
            "uncovered_with_ripr_gap": "2.0",
            "uncovered_without_ripr_gap": "8"
          }
        }"#;
        let ledger = r#"{
          "movement": {
            "new_policy_eligible": "1",
            "baseline_resolved": "2",
            "baseline_still_present": "3",
            "acknowledged": "0",
            "suppressed": "0",
            "blocking_candidates": "0",
            "visible_unresolved": "4"
          }
        }"#;
        let report = report_with(
            Some(Ok(coverage.to_string())),
            Some(Ok(ledger.to_string())),
            None,
            None,
        );
        let rendered = render_coverage_grip_frontier_json(&report)?;

        assert!(rendered.contains(r#""baseline_resolved": 2"#));
        assert!(rendered.contains(r#""uncovered_with_ripr_gap": 2"#));
        assert!(rendered.contains(r#""visible_unresolved_delta": -2"#));
        assert!(rendered.contains("behavioral grip improved without line-coverage movement"));
        Ok(())
    }

    #[test]
    fn coverage_grip_frontier_falls_back_when_preferred_source_lacks_metric() -> Result<(), String>
    {
        let ledger = r#"{"movement":{"new_policy_eligible":0}}"#;
        let delta =
            r#"{"delta":{"resolved":7,"still_present":11,"acknowledged":2,"suppressed":1}}"#;
        let report = report_with(
            None,
            Some(Ok(ledger.to_string())),
            Some(Ok(delta.to_string())),
            None,
        );
        let rendered = render_coverage_grip_frontier_json(&report)?;

        assert!(rendered.contains(r#""source": "pr_evidence_ledger""#));
        assert!(rendered.contains(r#""new_policy_eligible": 0"#));
        assert!(rendered.contains(r#""baseline_resolved": 7"#));
        assert!(rendered.contains(r#""baseline_still_present": 11"#));
        assert!(rendered.contains(r#""acknowledged": 2"#));
        assert!(rendered.contains(r#""suppressed": 1"#));
        Ok(())
    }

    #[test]
    fn coverage_grip_frontier_skips_empty_preferred_sources() -> Result<(), String> {
        let zero = r#"{"ripr_zero":{"visible_unresolved":5,"new_policy_eligible":1,"blocking_candidates":0},"baseline":{"resolved":2,"still_present":4}}"#;
        let report = report_with(
            None,
            Some(Ok("{}".to_string())),
            None,
            Some(Ok(zero.to_string())),
        );
        let rendered = render_coverage_grip_frontier_json(&report)?;

        assert!(rendered.contains(r#""source": "ripr_zero_status""#));
        assert!(rendered.contains(r#""visible_unresolved": 5"#));
        assert!(rendered.contains(r#""baseline_resolved": 2"#));
        Ok(())
    }
}
