//! SARIF 2.1.0 renderer for static `ripr` evidence.
//!
//! This module renders existing Finding and classified-seam facts. It does not
//! classify, suppress, or compare baselines; those decisions belong to
//! analysis, config/suppression loading, and future CI policy code.

use crate::analysis::ClassifiedSeam;
use crate::analysis::seams::SeamGripClass;
use crate::app::CheckOutput;
use crate::config::{ConfigSeverity, RiprConfig};
use crate::domain::{
    ExposureClass, Finding, MissingDiscriminatorFact, RelatedTest, StageEvidence, ValueFact,
};
use crate::output::suppressions::{
    SuppressionEntry, SuppressionKind, current_iso_date, is_expired,
};
use serde_json::{Map, Value, json};
use std::path::Path;

const SARIF_SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";
const SARIF_VERSION: &str = "2.1.0";
const RIPR_SARIF_SCHEMA_VERSION: &str = "0.1";
const SARIF_SPEC_URI: &str = "https://github.com/EffortlessMetrics/ripr/blob/main/docs/specs/RIPR-SPEC-0008-sarif-ci-policy.md";

/// Render diff-scoped Findings as SARIF.
pub(crate) fn render_findings_sarif(
    output: &CheckOutput,
    config: &RiprConfig,
    suppressions: &[SuppressionEntry],
) -> String {
    let today = current_iso_date();
    let rules = finding_rules();
    let results = output
        .findings
        .iter()
        .filter_map(|finding| finding_result(finding, config, suppressions, &today))
        .collect::<Vec<_>>();
    sarif_document("finding", rules, results)
}

/// Render repo-scoped classified seams as SARIF.
pub(crate) fn render_repo_seams_sarif(
    classified: &[ClassifiedSeam],
    config: &RiprConfig,
) -> String {
    let rules = seam_rules();
    let results = classified
        .iter()
        .filter_map(|entry| seam_result(entry, config))
        .collect::<Vec<_>>();
    sarif_document("repo_seam", rules, results)
}

fn sarif_document(scope: &str, rules: Vec<Value>, results: Vec<Value>) -> String {
    let document = json!({
        "$schema": SARIF_SCHEMA,
        "version": SARIF_VERSION,
        "runs": [
            {
                "tool": {
                    "driver": {
                        "name": "ripr",
                        "semanticVersion": env!("CARGO_PKG_VERSION"),
                        "informationUri": "https://github.com/EffortlessMetrics/ripr",
                        "rules": rules
                    }
                },
                "results": results,
                "properties": {
                    "tool": "ripr",
                    "schema_version": RIPR_SARIF_SCHEMA_VERSION,
                    "scope": scope
                }
            }
        ]
    });
    json_pretty(document)
}

fn json_pretty(value: Value) -> String {
    match serde_json::to_string_pretty(&value) {
        Ok(mut rendered) => {
            rendered.push('\n');
            rendered
        }
        Err(err) => format!(
            "{{\"version\":\"{SARIF_VERSION}\",\"runs\":[],\"properties\":{{\"render_error\":\"{}\"}}}}\n",
            escape_message(err.to_string().as_str())
        ),
    }
}

fn finding_result(
    finding: &Finding,
    config: &RiprConfig,
    suppressions: &[SuppressionEntry],
    today: &str,
) -> Option<Value> {
    let severity = config.severity().for_exposure(&finding.class);
    let level = sarif_level(severity)?;
    let rule_id = finding_rule_id(&finding.class);
    let file = normalize_path(&finding.probe.location.file);
    let line = finding.probe.location.line;
    let mut result = Map::new();
    result.insert("ruleId".to_string(), json!(rule_id));
    result.insert("level".to_string(), json!(level));
    result.insert(
        "message".to_string(),
        json!({ "text": finding_message(finding) }),
    );
    result.insert(
        "locations".to_string(),
        json!([physical_location(
            &file,
            line,
            Some(finding.probe.location.column)
        )]),
    );
    result.insert(
        "partialFingerprints".to_string(),
        json!({ "riprFingerprintV1": finding_fingerprint(&rule_id, finding, &file, line) }),
    );
    result.insert(
        "properties".to_string(),
        finding_properties(finding, severity),
    );
    if let Some(suppression) = active_exposure_suppression(finding, suppressions, today) {
        result.insert(
            "suppressions".to_string(),
            json!([suppression_metadata(suppression)]),
        );
    }
    Some(Value::Object(result))
}

fn seam_result(entry: &ClassifiedSeam, config: &RiprConfig) -> Option<Value> {
    let severity = config.severity().for_seam(entry.class);
    let level = sarif_level(severity)?;
    let rule_id = seam_rule_id(entry.class);
    let file = normalize_path(entry.seam.file());
    let line = entry.seam.display_line();
    let mut result = Map::new();
    result.insert("ruleId".to_string(), json!(rule_id));
    result.insert("level".to_string(), json!(level));
    result.insert(
        "message".to_string(),
        json!({ "text": seam_message(entry) }),
    );
    result.insert(
        "locations".to_string(),
        json!([physical_location(&file, line, None)]),
    );
    result.insert(
        "partialFingerprints".to_string(),
        json!({ "riprFingerprintV1": seam_fingerprint(&rule_id, entry, &file, line) }),
    );
    result.insert("properties".to_string(), seam_properties(entry, severity));
    if entry.class == SeamGripClass::Suppressed {
        result.insert(
            "suppressions".to_string(),
            json!([{
                "kind": "external",
                "justification": "seam is classified as suppressed by ripr configuration",
                "properties": {
                    "source": "ripr",
                    "grip_class": entry.class.as_str()
                }
            }]),
        );
    }
    Some(Value::Object(result))
}

fn sarif_level(severity: ConfigSeverity) -> Option<&'static str> {
    match severity {
        ConfigSeverity::Off => None,
        ConfigSeverity::Info | ConfigSeverity::Note => Some("note"),
        ConfigSeverity::Warning => Some("warning"),
    }
}

fn physical_location(file: &str, line: usize, column: Option<usize>) -> Value {
    let mut region = Map::new();
    region.insert("startLine".to_string(), json!(line.max(1)));
    if let Some(column) = column
        && column > 0
    {
        region.insert("startColumn".to_string(), json!(column));
    }
    json!({
        "physicalLocation": {
            "artifactLocation": { "uri": file },
            "region": region
        }
    })
}

fn finding_properties(finding: &Finding, severity: ConfigSeverity) -> Value {
    let mut properties = Map::new();
    properties.insert("tool".to_string(), json!("ripr"));
    properties.insert("kind".to_string(), json!("finding"));
    properties.insert("finding_id".to_string(), json!(finding.id.as_str()));
    properties.insert("probe_id".to_string(), json!(finding.probe.id.0.as_str()));
    properties.insert("classification".to_string(), json!(finding.class.as_str()));
    properties.insert("severity".to_string(), json!(severity.as_str()));
    properties.insert(
        "probe_family".to_string(),
        json!(finding.probe.family.as_str()),
    );
    properties.insert(
        "probe_delta".to_string(),
        json!(finding.probe.delta.as_str()),
    );
    properties.insert("confidence".to_string(), json!(finding.confidence));
    if let Some(owner) = &finding.probe.owner {
        properties.insert("owner".to_string(), json!(owner.0.as_str()));
    }
    properties.insert(
        "changed_expression".to_string(),
        json!(finding.probe.expression.as_str()),
    );
    properties.insert("ripr".to_string(), finding_ripr_properties(finding));
    properties.insert(
        "stop_reasons".to_string(),
        json!(
            finding
                .effective_stop_reasons()
                .iter()
                .map(|reason| reason.as_str())
                .collect::<Vec<_>>()
        ),
    );
    properties.insert(
        "related_tests_total".to_string(),
        json!(finding.related_tests.len()),
    );
    properties.insert(
        "related_tests".to_string(),
        json!(
            finding
                .related_tests
                .iter()
                .take(5)
                .map(related_test_properties)
                .collect::<Vec<_>>()
        ),
    );
    properties.insert(
        "flow_sinks".to_string(),
        json!(
            finding
                .flow_sinks
                .iter()
                .map(|sink| {
                    json!({
                        "kind": sink.kind.as_str(),
                        "text": sink.text.as_str(),
                        "line": sink.line,
                        "owner": sink.owner.as_ref().map(|owner| owner.0.as_str())
                    })
                })
                .collect::<Vec<_>>()
        ),
    );
    properties.insert(
        "observed_values".to_string(),
        value_facts(&finding.activation.observed_values),
    );
    properties.insert(
        "missing_discriminators".to_string(),
        missing_discriminators(&finding.activation.missing_discriminators),
    );
    properties.insert(
        "suggested_next_action".to_string(),
        json!(finding.recommended_next_step.as_deref().unwrap_or("")),
    );
    Value::Object(properties)
}

fn seam_properties(entry: &ClassifiedSeam, severity: ConfigSeverity) -> Value {
    let seam = &entry.seam;
    let evidence = &entry.evidence;
    json!({
        "tool": "ripr",
        "kind": "seam",
        "seam_id": seam.id().as_str(),
        "grip_class": entry.class.as_str(),
        "severity": severity.as_str(),
        "seam_kind": seam.kind().as_str(),
        "owner": seam.owner(),
        "expression": seam.expression(),
        "expected_sink": seam.expected_sink().as_str(),
        "required_discriminator": seam.required_discriminator().as_str(),
        "headline_eligible": entry.class.is_headline_eligible(),
        "evidence": {
            "reach": stage_properties(&evidence.reach),
            "activate": stage_properties(&evidence.activate),
            "propagate": stage_properties(&evidence.propagate),
            "observe": stage_properties(&evidence.observe),
            "discriminate": stage_properties(&evidence.discriminate)
        },
        "related_tests_total": evidence.related_tests.len(),
        "related_tests": evidence
            .related_tests
            .iter()
            .take(8)
            .map(|test| json!({
                "name": test.test_name.as_str(),
                "file": normalize_path(&test.file),
                "line": test.line,
                "oracle_kind": test.oracle_kind.as_str(),
                "oracle_strength": test.oracle_strength.as_str(),
                "evidence_summary": test.evidence_summary.as_str(),
                "relation_reason": test.relation_reason.as_str(),
                "relation_confidence": test.relation_confidence.as_str()
            }))
            .collect::<Vec<_>>(),
        "observed_values": value_facts(&evidence.observed_values),
        "missing_discriminators": missing_discriminators(&evidence.missing_discriminators)
    })
}

fn finding_ripr_properties(finding: &Finding) -> Value {
    json!({
        "reach": stage_properties(&finding.ripr.reach),
        "infect": stage_properties(&finding.ripr.infect),
        "propagate": stage_properties(&finding.ripr.propagate),
        "observe": stage_properties(&finding.ripr.reveal.observe),
        "discriminate": stage_properties(&finding.ripr.reveal.discriminate)
    })
}

fn stage_properties(stage: &StageEvidence) -> Value {
    json!({
        "state": stage.state.as_str(),
        "confidence": stage.confidence.as_str(),
        "summary": stage.summary.as_str()
    })
}

fn related_test_properties(test: &RelatedTest) -> Value {
    json!({
        "name": test.name.as_str(),
        "file": normalize_path(&test.file),
        "line": test.line,
        "oracle_kind": test.oracle_kind.as_str(),
        "oracle_strength": test.oracle_strength.as_str(),
        "oracle": test.oracle.as_deref()
    })
}

fn value_facts(values: &[ValueFact]) -> Value {
    json!(
        values
            .iter()
            .map(|value| {
                json!({
                    "line": value.line,
                    "text": value.text.as_str(),
                    "value": value.value.as_str(),
                    "context": value.context.as_str()
                })
            })
            .collect::<Vec<_>>()
    )
}

fn missing_discriminators(missing: &[MissingDiscriminatorFact]) -> Value {
    json!(
        missing
            .iter()
            .map(|missing| {
                json!({
                    "value": missing.value.as_str(),
                    "reason": missing.reason.as_str(),
                    "flow_sink": missing.flow_sink.as_ref().map(|sink| {
                        json!({
                            "kind": sink.kind.as_str(),
                            "text": sink.text.as_str(),
                            "line": sink.line,
                            "owner": sink.owner.as_ref().map(|owner| owner.0.as_str())
                        })
                    })
                })
            })
            .collect::<Vec<_>>()
    )
}

fn active_exposure_suppression<'a>(
    finding: &Finding,
    suppressions: &'a [SuppressionEntry],
    today: &str,
) -> Option<&'a SuppressionEntry> {
    suppressions.iter().find(|entry| {
        entry.kind == SuppressionKind::ExposureGap
            && entry.finding_id.as_deref() == Some(finding.id.as_str())
            && !is_expired(entry.expires.as_deref(), today)
    })
}

fn suppression_metadata(entry: &SuppressionEntry) -> Value {
    json!({
        "kind": "external",
        "justification": entry.reason.as_str(),
        "properties": {
            "source": "ripr",
            "suppression_kind": entry.kind.as_str(),
            "owner": entry.owner.as_str(),
            "expires": entry.expires.as_deref(),
            "block_line": entry.block_line
        }
    })
}

fn finding_message(finding: &Finding) -> String {
    let mut message = format!(
        "{} static exposure for {} probe",
        finding.class.as_str(),
        finding.probe.family.as_str()
    );
    if !finding.probe.expression.is_empty() {
        message.push_str(": ");
        message.push_str(&finding.probe.expression);
    }
    if let Some(next) = &finding.recommended_next_step {
        message.push_str(". Next step: ");
        message.push_str(next);
    }
    message
}

fn seam_message(entry: &ClassifiedSeam) -> String {
    format!(
        "{} seam grip for {}: {}",
        entry.class.as_str(),
        entry.seam.kind().as_str(),
        entry.seam.expression()
    )
}

fn finding_fingerprint(rule_id: &str, finding: &Finding, file: &str, line: usize) -> String {
    format!(
        "{rule_id}|{}|{}|{file}|{line}",
        finding.id, finding.probe.id.0
    )
}

fn seam_fingerprint(rule_id: &str, entry: &ClassifiedSeam, file: &str, line: usize) -> String {
    format!("{rule_id}|{}|{file}|{line}", entry.seam.id().as_str())
}

fn finding_rule_id(class: &ExposureClass) -> String {
    format!("ripr.finding.{}", class.as_str())
}

fn seam_rule_id(class: SeamGripClass) -> String {
    format!("ripr.seam.{}", class.as_str())
}

fn finding_rules() -> Vec<Value> {
    all_exposure_classes()
        .iter()
        .map(|class| {
            rule(
                finding_rule_id(class),
                format!("ripr {}", class.as_str()),
                format!("Static exposure finding classified as {}", class.as_str()),
            )
        })
        .collect()
}

fn seam_rules() -> Vec<Value> {
    SeamGripClass::ALL
        .iter()
        .map(|class| {
            rule(
                seam_rule_id(*class),
                format!("ripr seam {}", class.as_str()),
                format!("Repo seam grip evidence classified as {}", class.as_str()),
            )
        })
        .collect()
}

fn rule(id: String, name: String, short_description: String) -> Value {
    json!({
        "id": id,
        "name": name,
        "shortDescription": {
            "text": short_description
        },
        "helpUri": SARIF_SPEC_URI
    })
}

fn all_exposure_classes() -> [ExposureClass; 7] {
    [
        ExposureClass::Exposed,
        ExposureClass::WeaklyExposed,
        ExposureClass::ReachableUnrevealed,
        ExposureClass::NoStaticPath,
        ExposureClass::InfectionUnknown,
        ExposureClass::PropagationUnknown,
        ExposureClass::StaticUnknown,
    ]
}

fn normalize_path(path: &Path) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    raw.strip_prefix("./").unwrap_or(&raw).to_string()
}

fn escape_message(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamKind};
    use crate::analysis::test_grip_evidence::{
        RelatedTestGrip, RelationConfidence, RelationReason, TestGripEvidence,
    };
    use crate::app::{CheckOutput, Mode};
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, FlowSinkFact, FlowSinkKind, OracleKind,
        OracleStrength, Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence,
        SourceLocation, StageEvidence, StageState, Summary, SymbolId, ValueContext,
    };
    use serde_json::Value;
    use std::path::PathBuf;

    #[test]
    fn sarif_renders_findings_with_stable_rule_ids() -> Result<(), String> {
        let rendered = render_findings_sarif(&sample_output(), &RiprConfig::default(), &[]);
        let sarif = parse_json(&rendered)?;
        let rule_ids = rule_ids(&sarif)?;
        let result = first_result(&sarif)?;

        assert_eq!(sarif["version"], "2.1.0");
        assert!(rule_ids.contains(&"ripr.finding.weakly_exposed".to_string()));
        assert_eq!(result["ruleId"], "ripr.finding.weakly_exposed");
        assert_eq!(result["level"], "warning");
        assert_eq!(result["properties"]["kind"], "finding");
        assert_eq!(result["properties"]["finding_id"], "finding:discount");
        assert_eq!(
            result["partialFingerprints"]["riprFingerprintV1"],
            "ripr.finding.weakly_exposed|finding:discount|probe:src/pricing.rs:88:predicate|src/pricing.rs|88"
        );
        Ok(())
    }

    #[test]
    fn sarif_renders_seams_with_stable_rule_ids() -> Result<(), String> {
        let rendered =
            render_repo_seams_sarif(&[weakly_gripped_classified()], &RiprConfig::default());
        let sarif = parse_json(&rendered)?;
        let rule_ids = rule_ids(&sarif)?;
        let result = first_result(&sarif)?;

        assert!(rule_ids.contains(&"ripr.seam.weakly_gripped".to_string()));
        assert!(rule_ids.contains(&"ripr.seam.suppressed".to_string()));
        assert_eq!(result["ruleId"], "ripr.seam.weakly_gripped");
        assert_eq!(result["properties"]["kind"], "seam");
        assert_eq!(result["properties"]["grip_class"], "weakly_gripped");
        assert_eq!(result["properties"]["seam_kind"], "predicate_boundary");
        Ok(())
    }

    #[test]
    fn sarif_uses_configured_finding_severity() -> Result<(), String> {
        let config = parse_config(
            r#"
[severity.findings]
weakly_exposed = "note"
"#,
        )?;
        let rendered = render_findings_sarif(&sample_output(), &config, &[]);
        let sarif = parse_json(&rendered)?;
        let result = first_result(&sarif)?;

        assert_eq!(result["level"], "note");
        assert_eq!(result["properties"]["severity"], "note");
        Ok(())
    }

    #[test]
    fn sarif_uses_configured_seam_severity() -> Result<(), String> {
        let config = parse_config(
            r#"
[severity.seams]
weakly_gripped = "note"
"#,
        )?;
        let rendered = render_repo_seams_sarif(&[weakly_gripped_classified()], &config);
        let sarif = parse_json(&rendered)?;
        let result = first_result(&sarif)?;

        assert_eq!(result["level"], "note");
        assert_eq!(result["properties"]["severity"], "note");
        Ok(())
    }

    #[test]
    fn sarif_omits_off_seam_class() -> Result<(), String> {
        let rendered =
            render_repo_seams_sarif(&[strongly_gripped_classified()], &RiprConfig::default());
        let sarif = parse_json(&rendered)?;
        let results = results(&sarif)?;

        assert!(results.is_empty(), "strongly_gripped seams default to off");
        Ok(())
    }

    #[test]
    fn sarif_attaches_suppression_metadata() -> Result<(), String> {
        let suppression = SuppressionEntry {
            kind: SuppressionKind::ExposureGap,
            finding_id: Some("finding:discount".to_string()),
            test: None,
            path: None,
            reason: "tracked in integration suite".to_string(),
            owner: "team-ripr".to_string(),
            expires: Some("2099-01-01".to_string()),
            scope: None,
            created_at: None,
            last_seen: None,
            review_by: None,
            expected_visibility: None,
            static_class: None,
            language: None,
            language_status: None,
            block_line: 4,
        };
        let rendered =
            render_findings_sarif(&sample_output(), &RiprConfig::default(), &[suppression]);
        let sarif = parse_json(&rendered)?;
        let result = first_result(&sarif)?;

        assert_eq!(result["suppressions"][0]["kind"], "external");
        assert_eq!(
            result["suppressions"][0]["justification"],
            "tracked in integration suite"
        );
        assert_eq!(
            result["suppressions"][0]["properties"]["owner"],
            "team-ripr"
        );
        Ok(())
    }

    #[test]
    fn sarif_output_is_valid_json() -> Result<(), String> {
        let findings = render_findings_sarif(&sample_output(), &RiprConfig::default(), &[]);
        let seams = render_repo_seams_sarif(&[weakly_gripped_classified()], &RiprConfig::default());

        let _ = parse_json(&findings)?;
        let _ = parse_json(&seams)?;
        Ok(())
    }

    #[test]
    fn sarif_preserves_static_language() {
        let rendered = render_findings_sarif(&sample_output(), &RiprConfig::default(), &[]);
        assert!(rendered.contains("weakly_exposed"));
        assert!(rendered.contains("static exposure"));
        assert!(rendered.contains("equality boundary is absent"));
    }

    fn parse_config(text: &str) -> Result<RiprConfig, String> {
        crate::config::tests_only_parse(text)
    }

    fn parse_json(text: &str) -> Result<Value, String> {
        serde_json::from_str(text).map_err(|err| err.to_string())
    }

    fn rule_ids(sarif: &Value) -> Result<Vec<String>, String> {
        let Some(rules) = sarif["runs"][0]["tool"]["driver"]["rules"].as_array() else {
            return Err("missing SARIF rules array".to_string());
        };
        rules
            .iter()
            .map(|rule| {
                rule["id"]
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| "SARIF rule missing id".to_string())
            })
            .collect()
    }

    fn first_result(sarif: &Value) -> Result<&Value, String> {
        let results = results(sarif)?;
        results
            .first()
            .copied()
            .ok_or_else(|| "expected at least one SARIF result".to_string())
    }

    fn results(sarif: &Value) -> Result<Vec<&Value>, String> {
        let Some(results) = sarif["runs"][0]["results"].as_array() else {
            return Err("missing SARIF results array".to_string());
        };
        Ok(results.iter().collect())
    }

    fn sample_output() -> CheckOutput {
        CheckOutput {
            schema_version: "0.1".to_string(),
            tool: "ripr".to_string(),
            mode: Mode::Draft,
            root: PathBuf::from("repo"),
            base: Some("origin/main".to_string()),
            summary: Summary::default(),
            findings: vec![sample_finding()],
        }
    }

    fn sample_finding() -> Finding {
        Finding {
            id: "finding:discount".to_string(),
            probe: Probe {
                id: ProbeId("probe:src/pricing.rs:88:predicate".to_string()),
                location: SourceLocation::new("src/pricing.rs", 88, 9),
                owner: Some(SymbolId("pricing::discounted_total".to_string())),
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: Some("amount > discount_threshold".to_string()),
                after: Some("amount >= discount_threshold".to_string()),
                expression: "amount >= discount_threshold".to_string(),
                expected_sinks: vec!["return_value".to_string()],
                required_oracles: vec!["exact returned value".to_string()],
            },
            class: ExposureClass::WeaklyExposed,
            ripr: RiprEvidence {
                reach: stage(StageState::Yes, "related tests call discounted_total"),
                infect: stage(StageState::Weak, "boundary value is missing"),
                propagate: stage(StageState::Yes, "local flow reaches return value"),
                reveal: RevealEvidence {
                    observe: stage(StageState::Yes, "tests assert returned values"),
                    discriminate: stage(StageState::Weak, "equality boundary is absent"),
                },
            },
            confidence: 0.75,
            evidence: vec!["local flow reaches return value".to_string()],
            missing: vec!["amount == discount_threshold".to_string()],
            flow_sinks: vec![FlowSinkFact {
                kind: FlowSinkKind::ReturnValue,
                text: "returned discounted total".to_string(),
                line: 90,
                owner: Some(SymbolId("pricing::discounted_total".to_string())),
            }],
            activation: ActivationEvidence {
                observed_values: vec![ValueFact {
                    line: 12,
                    text: "discounted_total(50, 100)".to_string(),
                    value: "amount = 50".to_string(),
                    context: ValueContext::FunctionArgument,
                }],
                missing_discriminators: vec![MissingDiscriminatorFact {
                    value: "amount == discount_threshold".to_string(),
                    reason: "no related test calls the equality boundary".to_string(),
                    flow_sink: None,
                }],
            },
            stop_reasons: Vec::new(),
            related_tests: vec![RelatedTest {
                name: "below_threshold_has_no_discount".to_string(),
                file: PathBuf::from("tests/pricing.rs"),
                line: 12,
                oracle: Some("assert_eq!(discounted_total(50, 100), 50)".to_string()),
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
            }],
            recommended_next_step: Some("Add an equality-boundary assertion".to_string()),
            language: None,
            language_status: None,
            static_limit_kind: None,
        }
    }

    fn weakly_gripped_classified() -> ClassifiedSeam {
        classified_seam(SeamGripClass::WeaklyGripped)
    }

    fn strongly_gripped_classified() -> ClassifiedSeam {
        classified_seam(SeamGripClass::StronglyGripped)
    }

    fn classified_seam(class: SeamGripClass) -> ClassifiedSeam {
        let seam = RepoSeam::new(
            "src/pricing.rs",
            "pricing::discounted_total",
            SeamKind::PredicateBoundary,
            42,
            88,
            "amount >= discount_threshold",
            RequiredDiscriminator::BoundaryValue {
                description: "amount == discount_threshold".to_string(),
            },
            ExpectedSink::ReturnValue,
        );
        let evidence = TestGripEvidence {
            seam_id: seam.id().clone(),
            related_tests: vec![RelatedTestGrip {
                test_name: "below_threshold_has_no_discount".to_string(),
                file: PathBuf::from("tests/pricing.rs"),
                line: 12,
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
                evidence_summary: "exact value assertion".to_string(),
                relation_reason: RelationReason::DirectOwnerCall,
                relation_confidence: RelationConfidence::High,
            }],
            reach: stage(StageState::Yes, "related test calls owner"),
            activate: stage(StageState::Yes, "amount value observed"),
            propagate: stage(StageState::Yes, "return value sink reached"),
            observe: stage(StageState::Yes, "returned value asserted"),
            discriminate: stage(StageState::Weak, "equality boundary absent"),
            observed_values: vec![ValueFact {
                line: 12,
                text: "discounted_total(50, 100)".to_string(),
                value: "amount = 50".to_string(),
                context: ValueContext::FunctionArgument,
            }],
            missing_discriminators: vec![MissingDiscriminatorFact {
                value: "amount == discount_threshold".to_string(),
                reason: "observed values do not include equality boundary".to_string(),
                flow_sink: None,
            }],
        };
        ClassifiedSeam {
            seam,
            evidence,
            class,
        }
    }

    fn stage(state: StageState, summary: &str) -> StageEvidence {
        StageEvidence::new(state, Confidence::Medium, summary)
    }
}
