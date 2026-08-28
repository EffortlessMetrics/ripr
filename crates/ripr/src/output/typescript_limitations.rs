use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const SCHEMA_VERSION: &str = "0.1";
const REPORT_KIND: &str = "typescript_limitation_leaderboard";
const MAX_SAMPLES_PER_LIMITATION: usize = 3;

pub(crate) const DEFAULT_TYPESCRIPT_LIMITATIONS_OUT: &str =
    "target/ripr/reports/typescript-limitations.json";
pub(crate) const DEFAULT_TYPESCRIPT_LIMITATIONS_MD_OUT: &str =
    "target/ripr/reports/typescript-limitations.md";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeScriptLimitationLeaderboardInput {
    pub(crate) root: String,
    pub(crate) generated_at: String,
    pub(crate) check_output_path: String,
    pub(crate) check_output_json: Result<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeScriptLimitationLeaderboardReport {
    status: String,
    root: String,
    generated_at: String,
    inputs: TypeScriptLimitationLeaderboardInputs,
    summary: TypeScriptLimitationSummary,
    limitations: Vec<TypeScriptLimitationEntry>,
    warnings: Vec<String>,
    limits: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct TypeScriptLimitationLeaderboardInputs {
    check_output: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct TypeScriptLimitationSummary {
    findings_total: usize,
    typescript_family_findings_total: usize,
    limitations_total: usize,
    distinct_limitations_total: usize,
    top_limitation_kind: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct TypeScriptLimitationEntry {
    kind: String,
    count: usize,
    sources: Vec<String>,
    samples: Vec<TypeScriptLimitationSample>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct TypeScriptLimitationSample {
    finding_id: String,
    language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct LimitationAggregate {
    count: usize,
    sources: BTreeSet<String>,
    samples: Vec<TypeScriptLimitationSample>,
}

pub(crate) fn build_typescript_limitation_leaderboard_report(
    input: TypeScriptLimitationLeaderboardInput,
) -> TypeScriptLimitationLeaderboardReport {
    let mut warnings = Vec::new();
    let mut aggregates = BTreeMap::new();
    let mut summary = TypeScriptLimitationSummary::default();
    let parsed = match input.check_output_json {
        Ok(contents) => match collect_limitations_from_check_output(&contents, &mut aggregates) {
            Ok(parsed_summary) => {
                summary.findings_total = parsed_summary.findings_total;
                summary.typescript_family_findings_total =
                    parsed_summary.typescript_family_findings_total;
                true
            }
            Err(err) => {
                warnings.push(format!("parse {} failed: {err}", input.check_output_path));
                false
            }
        },
        Err(err) => {
            warnings.push(err);
            false
        }
    };

    let limitations = entries_from_aggregates(aggregates);
    summary.limitations_total = limitations.iter().map(|entry| entry.count).sum();
    summary.distinct_limitations_total = limitations.len();
    summary.top_limitation_kind = limitations.first().map(|entry| entry.kind.clone());

    let status = if parsed { "advisory" } else { "blocked" }.to_string();
    TypeScriptLimitationLeaderboardReport {
        status,
        root: input.root,
        generated_at: input.generated_at,
        inputs: TypeScriptLimitationLeaderboardInputs {
            check_output: input.check_output_path,
        },
        summary,
        limitations,
        warnings,
        limits: vec![
            "Advisory TypeScript-family preview limitation counts only.".to_string(),
            "Counts come from explicit check JSON evidence; this report does not rerun analysis."
                .to_string(),
            "This report does not execute TypeScript tests, edit source, generate tests, call providers, change gates, or contribute badge authority.".to_string(),
        ],
    }
}

pub(crate) fn render_typescript_limitation_leaderboard_json(
    report: &TypeScriptLimitationLeaderboardReport,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct JsonReport<'a> {
        schema_version: &'static str,
        tool: &'static str,
        kind: &'static str,
        status: &'a str,
        root: &'a str,
        generated_at: &'a str,
        inputs: &'a TypeScriptLimitationLeaderboardInputs,
        summary: &'a TypeScriptLimitationSummary,
        limitations: &'a [TypeScriptLimitationEntry],
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
        limitations: &report.limitations,
        warnings: &report.warnings,
        limits: &report.limits,
    })
    .map_err(|err| format!("serialize TypeScript limitation leaderboard JSON failed: {err}"))
}

pub(crate) fn render_typescript_limitation_leaderboard_markdown(
    report: &TypeScriptLimitationLeaderboardReport,
) -> String {
    let mut out = String::new();
    out.push_str("# RIPR TypeScript Limitation Leaderboard\n\n");
    out.push_str(&format!("Status: `{}`\n\n", md_inline(&report.status)));
    out.push_str(&format!("Root: `{}`\n\n", md_inline(&report.root)));
    out.push_str(
        "Authority: advisory TypeScript-family preview backlog signal only. Gate-decision and badge artifacts keep their existing authority.\n\n",
    );

    out.push_str("## Summary\n\n");
    out.push_str(&format!(
        "- Findings: `{}`; TypeScript-family findings: `{}`\n",
        report.summary.findings_total, report.summary.typescript_family_findings_total
    ));
    out.push_str(&format!(
        "- Limitation signals: `{}`; distinct kinds: `{}`\n",
        report.summary.limitations_total, report.summary.distinct_limitations_total
    ));
    let top = report
        .summary
        .top_limitation_kind
        .as_deref()
        .unwrap_or("none");
    out.push_str(&format!("- Top limitation: `{}`\n\n", md_inline(top)));

    if !report.warnings.is_empty() {
        out.push_str("## Warnings\n\n");
        for warning in &report.warnings {
            out.push_str(&format!("- {}\n", md_inline(warning)));
        }
        out.push('\n');
    }

    out.push_str("## Limitations\n\n");
    if report.limitations.is_empty() {
        out.push_str(
            "No TypeScript-family limitation signals were found in the supplied check output.\n\n",
        );
    } else {
        out.push_str("| limitation | count | sources | sample |\n");
        out.push_str("| --- | ---: | --- | --- |\n");
        for entry in &report.limitations {
            let sources = entry.sources.join(", ");
            let sample = entry
                .samples
                .first()
                .map(sample_label)
                .unwrap_or_else(|| "none".to_string());
            out.push_str(&format!(
                "| `{}` | `{}` | {} | {} |\n",
                md_inline(&entry.kind),
                entry.count,
                md_inline(&sources),
                md_inline(&sample)
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ParsedCheckOutputSummary {
    findings_total: usize,
    typescript_family_findings_total: usize,
}

fn collect_limitations_from_check_output(
    contents: &str,
    aggregates: &mut BTreeMap<String, LimitationAggregate>,
) -> Result<ParsedCheckOutputSummary, String> {
    let value: Value =
        serde_json::from_str(contents).map_err(|err| format!("invalid JSON: {err}"))?;
    let findings = value
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| "expected check output object with findings array".to_string())?;

    let mut summary = ParsedCheckOutputSummary {
        findings_total: findings.len(),
        typescript_family_findings_total: 0,
    };
    for finding in findings {
        let mut finding_limitations = limitation_kinds_from_evidence(finding);
        let language_family = typescript_family_language(finding);
        if language_family.is_some() || !finding_limitations.is_empty() {
            add_static_limit_kind(finding, &mut finding_limitations);
        }

        if language_family.is_some() || !finding_limitations.is_empty() {
            summary.typescript_family_findings_total += 1;
        }

        if finding_limitations.is_empty() {
            continue;
        }
        let sample = sample_from_finding(finding, language_family.unwrap_or("typescript_family"));
        for (kind, sources) in finding_limitations {
            let aggregate = aggregates.entry(kind).or_default();
            aggregate.count += 1;
            aggregate.sources.extend(sources);
            if aggregate.samples.len() < MAX_SAMPLES_PER_LIMITATION {
                aggregate.samples.push(sample.clone());
            }
        }
    }
    Ok(summary)
}

fn entries_from_aggregates(
    aggregates: BTreeMap<String, LimitationAggregate>,
) -> Vec<TypeScriptLimitationEntry> {
    let mut entries: Vec<TypeScriptLimitationEntry> = aggregates
        .into_iter()
        .map(|(kind, aggregate)| TypeScriptLimitationEntry {
            kind,
            count: aggregate.count,
            sources: aggregate.sources.into_iter().collect(),
            samples: aggregate.samples,
        })
        .collect();
    entries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.kind.cmp(&right.kind))
    });
    entries
}

fn limitation_kinds_from_evidence(finding: &Value) -> BTreeMap<String, BTreeSet<String>> {
    let mut limitations: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in evidence_lines(finding) {
        if let Some(kind) = line.strip_prefix("typescript_limitation: ") {
            add_limitation_source(&mut limitations, kind, "typescript_limitation");
        } else if let Some(kind) = line.strip_prefix("typescript_package_limitation: ") {
            add_limitation_source(&mut limitations, kind, "typescript_package_limitation");
        }
    }
    limitations
}

fn add_static_limit_kind(finding: &Value, limitations: &mut BTreeMap<String, BTreeSet<String>>) {
    let Some(kind) = string_at(finding, &["static_limit_kind"]) else {
        return;
    };
    let leaderboard_kind = static_limit_kind_to_typescript_limitation(kind).unwrap_or(kind);
    add_limitation_source(limitations, leaderboard_kind, "static_limit_kind");
}

fn add_limitation_source(
    limitations: &mut BTreeMap<String, BTreeSet<String>>,
    kind: &str,
    source: &str,
) {
    let normalized = kind.trim();
    if normalized.is_empty() {
        return;
    }
    limitations
        .entry(normalized.to_string())
        .or_default()
        .insert(source.to_string());
}

fn static_limit_kind_to_typescript_limitation(kind: &str) -> Option<&'static str> {
    match kind {
        "missing_import_graph" => Some("typescript_import_graph_unresolved"),
        "mocked_module" => Some("typescript_mock_only_observer"),
        _ => None,
    }
}

fn typescript_family_language(finding: &Value) -> Option<&'static str> {
    match string_at(finding, &["language"]) {
        Some("typescript") => Some("typescript"),
        Some("javascript") => Some("javascript"),
        _ => match string_at(finding, &["probe", "owner"]) {
            Some(owner) if owner.starts_with("typescript:") => Some("typescript"),
            Some(owner) if owner.starts_with("javascript:") => Some("javascript"),
            _ => match string_at(finding, &["typescript_preview_card", "language"]) {
                Some("typescript") => Some("typescript"),
                Some("javascript") => Some("javascript"),
                _ => None,
            },
        },
    }
}

fn sample_from_finding(finding: &Value, language: &str) -> TypeScriptLimitationSample {
    TypeScriptLimitationSample {
        finding_id: string_at(finding, &["id"])
            .unwrap_or("unknown-finding")
            .to_string(),
        language: language.to_string(),
        file: string_at(finding, &["probe", "file"]).map(ToString::to_string),
        line: u64_at(finding, &["probe", "line"]),
    }
}

fn evidence_lines(finding: &Value) -> Vec<&str> {
    finding
        .get("evidence")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

fn string_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str()
}

fn u64_at(value: &Value, path: &[&str]) -> Option<u64> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_u64()
}

fn sample_label(sample: &TypeScriptLimitationSample) -> String {
    match (&sample.file, sample.line) {
        (Some(file), Some(line)) => format!("{file}:{line} ({})", sample.finding_id),
        (Some(file), None) => format!("{file} ({})", sample.finding_id),
        _ => sample.finding_id.clone(),
    }
}

fn md_inline(value: &str) -> String {
    value.replace('|', "\\|").replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typescript_limitation_leaderboard_counts_unique_kinds_per_finding() -> Result<(), String> {
        let report =
            build_typescript_limitation_leaderboard_report(TypeScriptLimitationLeaderboardInput {
                root: ".".to_string(),
                generated_at: "123".to_string(),
                check_output_path: "check.json".to_string(),
                check_output_json: Ok(sample_check_output()),
            });

        assert_eq!(report.status, "advisory");
        assert_eq!(report.summary.findings_total, 3);
        assert_eq!(report.summary.typescript_family_findings_total, 2);
        assert_eq!(report.summary.limitations_total, 4);
        assert_eq!(report.summary.distinct_limitations_total, 3);
        assert_eq!(
            report.summary.top_limitation_kind.as_deref(),
            Some("typescript_package_root_unresolved")
        );
        assert_eq!(report.limitations[0].count, 2);
        assert!(
            report.limitations[0]
                .sources
                .contains(&"typescript_package_limitation".to_string())
        );
        let import_graph = report
            .limitations
            .iter()
            .find(|entry| entry.kind == "typescript_import_graph_unresolved")
            .ok_or_else(|| "missing import graph limitation entry".to_string())?;
        assert_eq!(import_graph.count, 1);
        assert!(
            import_graph
                .sources
                .contains(&"static_limit_kind".to_string())
        );
        assert_eq!(
            import_graph.samples[0].file.as_deref(),
            Some("src/view.jsx")
        );
        Ok(())
    }

    #[test]
    fn typescript_limitation_leaderboard_renders_json_and_markdown() -> Result<(), String> {
        let report =
            build_typescript_limitation_leaderboard_report(TypeScriptLimitationLeaderboardInput {
                root: ".".to_string(),
                generated_at: "123".to_string(),
                check_output_path: "check.json".to_string(),
                check_output_json: Ok(sample_check_output()),
            });

        let json_text = render_typescript_limitation_leaderboard_json(&report)?;
        assert!(json_text.contains("\"kind\": \"typescript_limitation_leaderboard\""));
        assert!(json_text.contains("\"typescript_import_graph_unresolved\""));
        assert!(json_text.contains("\"static_limit_kind\""));

        let markdown = render_typescript_limitation_leaderboard_markdown(&report);
        assert!(markdown.contains("# RIPR TypeScript Limitation Leaderboard"));
        assert!(markdown.contains("typescript_package_root_unresolved"));
        assert!(
            markdown.contains("Gate-decision and badge artifacts keep their existing authority")
        );
        Ok(())
    }

    #[test]
    fn typescript_limitation_leaderboard_blocks_on_malformed_check_output() {
        let report =
            build_typescript_limitation_leaderboard_report(TypeScriptLimitationLeaderboardInput {
                root: ".".to_string(),
                generated_at: "123".to_string(),
                check_output_path: "check.json".to_string(),
                check_output_json: Ok("{}".to_string()),
            });

        assert_eq!(report.status, "blocked");
        assert_eq!(report.summary.limitations_total, 0);
        assert!(report.warnings[0].contains("expected check output object with findings array"));
    }

    fn sample_check_output() -> String {
        r#"{
  "findings": [
    {
      "id": "probe:src_view.jsx:typescript_preview:11111111",
      "language": "javascript",
      "probe": {"file": "src/view.jsx", "line": 7, "owner": "javascript:src/view.jsx::View"},
      "static_limit_kind": "missing_import_graph",
      "evidence": [
        "typescript_limitation: typescript_import_graph_unresolved",
        "typescript_limitation: typescript_import_graph_unresolved",
        "typescript_package_limitation: typescript_package_root_unresolved"
      ]
    },
    {
      "id": "probe:src_dispatch.ts:typescript_preview:22222222",
      "language": "typescript",
      "probe": {"file": "src/dispatch.ts", "line": 2, "owner": "typescript:src/dispatch.ts::dispatch"},
      "static_limit_kind": "dynamic_dispatch",
      "evidence": [
        "typescript_package_limitation: typescript_package_root_unresolved"
      ]
    },
    {
      "id": "probe:src_lib.rs:predicate:33333333",
      "language": "rust",
      "probe": {"file": "src/lib.rs", "line": 1},
      "static_limit_kind": "dynamic_dispatch",
      "evidence": []
    }
  ]
}"#
        .to_string()
    }
}
