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
    ExposureClass, Finding, LanguageId, LanguageStatus, MissingDiscriminatorFact, RelatedTest,
    StageEvidence, ValueFact,
};
use crate::output::perl_preview_card::{perl_preview_card, perl_preview_card_json_value};
use crate::output::preview_actionability::{
    preview_actionability_for, preview_actionability_json_value,
};
use crate::output::python_repair_card::{python_repair_card, python_repair_card_json_value};
use crate::output::suppressions::{
    SuppressionEntry, SuppressionKind, current_iso_date, is_expired,
};
use crate::output::typescript_preview_card::{
    typescript_preview_card, typescript_preview_card_json_value,
};
use serde_json::{Map, Value, json};
use std::path::Path;

const SARIF_SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";
const SARIF_VERSION: &str = "2.1.0";
const RIPR_SARIF_SCHEMA_VERSION: &str = "0.1";
const SARIF_SPEC_URI: &str = "https://github.com/EffortlessMetrics/ripr/blob/main/docs/specs/RIPR-SPEC-0008-sarif-ci-policy.md";
const PYTHON_PREVIEW_AUTHORITY_BOUNDARY: &str = "preview_advisory_only";

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
    if let Some(gap) = &finding.canonical_gap {
        properties.insert("canonical_gap_id".to_string(), json!(gap.id.as_str()));
        properties.insert(
            "canonical_gap".to_string(),
            json!({
                "id": gap.id.as_str(),
                "language": gap.language.as_str(),
                "file": gap.file.as_str(),
                "owner": gap.owner.as_str(),
                "behavior_kind": gap.behavior_kind.as_str(),
                "probe_kind": gap.probe_kind.as_str(),
                "normalized_discriminator": gap.normalized_discriminator.as_str()
            }),
        );
    }
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
    if let Some(language) = finding.language {
        properties.insert("language".to_string(), json!(language.as_str()));
    }
    if let Some(status) = finding.language_status {
        properties.insert("language_status".to_string(), json!(status.as_str()));
    }
    if let Some(kind) = finding.owner_kind {
        properties.insert("owner_kind".to_string(), json!(kind.as_str()));
    }
    if let Some(kind) = finding.static_limit_kind {
        properties.insert("static_limit_kind".to_string(), json!(kind.as_str()));
    }
    if let Some(actionability) = preview_actionability_for(finding) {
        properties.insert(
            "preview_actionability".to_string(),
            preview_actionability_json_value(&actionability),
        );
    }
    let python_card = python_repair_card(finding);
    if let Some(card) = &python_card {
        properties.insert(
            "python_repair_card".to_string(),
            python_repair_card_json_value(card),
        );
    } else if let Some(no_action) = python_no_action_properties(finding) {
        properties.insert("python_no_action".to_string(), no_action);
    }
    if let Some(card) = typescript_preview_card(finding) {
        properties.insert(
            "typescript_preview_card".to_string(),
            typescript_preview_card_json_value(&card),
        );
    }
    if let Some(card) = perl_preview_card(finding) {
        properties.insert(
            "perl_preview_card".to_string(),
            perl_preview_card_json_value(&card),
        );
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

fn python_no_action_properties(finding: &Finding) -> Option<Value> {
    if finding.language != Some(LanguageId::Python)
        || finding.language_status != Some(LanguageStatus::Preview)
    {
        return None;
    }
    if let Some(no_action_kind) = python_ordinary_no_action_kind(finding) {
        return Some(python_ordinary_no_action_properties(
            finding,
            no_action_kind,
        ));
    }
    python_static_limit_no_action_properties(finding)
}

fn python_ordinary_no_action_kind(finding: &Finding) -> Option<&'static str> {
    match &finding.class {
        ExposureClass::Exposed => Some("already_observed"),
        ExposureClass::NoStaticPath => Some("no_related_test"),
        ExposureClass::WeaklyExposed if python_finding_is_heuristic_only(finding) => {
            Some("heuristic_only")
        }
        _ => None,
    }
}

fn python_finding_is_heuristic_only(finding: &Finding) -> bool {
    finding
        .evidence
        .iter()
        .any(|item| item.starts_with("related_test_uncertain:"))
        || finding
            .ripr
            .reach
            .summary
            .contains("heuristic Python test link")
}

fn python_ordinary_no_action_properties(finding: &Finding, no_action_kind: &str) -> Value {
    let changed_owner = finding.probe.owner.as_ref().map(|owner| owner.0.as_str());
    let stop_conditions = python_ordinary_no_action_stop_conditions(finding, no_action_kind);
    json!({
        "source": "check_python_preview",
        "language": "python",
        "language_status": "preview",
        "authority_boundary": PYTHON_PREVIEW_AUTHORITY_BOUNDARY,
        "repairability": "no_action",
        "repair_packet_ready": false,
        "repair_card_present": false,
        "gap_state": no_action_kind,
        "no_action_kind": no_action_kind,
        "changed_owner": changed_owner,
        "why_not_actionable": python_ordinary_no_action_reason(no_action_kind),
        "verify": {
            "command": Value::Null,
            "status": "not_applicable_no_action"
        },
        "receipt": {
            "command": Value::Null,
            "status": "not_applicable_no_action"
        },
        "stop_conditions": stop_conditions,
        "limits": [
            "Syntax-first Python preview evidence only.",
            "No repair card or agent packet emitted for no-action Python states.",
            "No source edits, generated tests, mutation execution, provider calls, or gate authority."
        ]
    })
}

fn python_ordinary_no_action_reason(no_action_kind: &str) -> &'static str {
    match no_action_kind {
        "already_observed" => {
            "Current Python test evidence already observes the changed behavior; no missing proof was routed."
        }
        "no_related_test" => {
            "No related Python test was statically linked, so RIPR cannot choose a safe edit target."
        }
        "heuristic_only" => {
            "Only heuristic Python related-test proximity was found, so bounded repair routing would overclaim."
        }
        _ => "Python preview did not find a bounded repair route.",
    }
}

fn python_ordinary_no_action_stop_conditions(
    finding: &Finding,
    no_action_kind: &str,
) -> Vec<&'static str> {
    let mut stop_conditions = finding
        .effective_stop_reasons()
        .iter()
        .map(|reason| reason.as_str())
        .collect::<Vec<_>>();
    if stop_conditions.is_empty() {
        stop_conditions.push(match no_action_kind {
            "already_observed" => "missing_proof_already_observed",
            "no_related_test" => "related_python_test_not_found",
            "heuristic_only" => "related_test_link_uncertain",
            _ => "no_repair_packet_emitted",
        });
    }
    stop_conditions
}

fn python_static_limit_no_action_properties(finding: &Finding) -> Option<Value> {
    let static_limit_kind = finding.static_limit_kind?;
    let static_limit_kind = static_limit_kind.as_str();
    let stop_reasons = finding
        .effective_stop_reasons()
        .iter()
        .map(|reason| reason.as_str())
        .collect::<Vec<_>>();
    let changed_owner = finding.probe.owner.as_ref().map(|owner| owner.0.as_str());
    let why_not_actionable = python_static_limit_detail(finding, static_limit_kind);

    Some(json!({
        "source": "check_python_preview",
        "language": "python",
        "language_status": "preview",
        "authority_boundary": PYTHON_PREVIEW_AUTHORITY_BOUNDARY,
        "repairability": "analyzer_limitation",
        "repair_packet_ready": false,
        "repair_card_present": false,
        "gap_state": "static_limitation",
        "no_action_kind": "static_limit",
        "static_limit_kind": static_limit_kind,
        "changed_owner": changed_owner,
        "why_not_actionable": why_not_actionable,
        "verify": {
            "command": Value::Null,
            "status": "not_applicable_static_limit"
        },
        "receipt": {
            "command": Value::Null,
            "status": "not_applicable_static_limit"
        },
        "stop_conditions": stop_reasons,
        "limits": [
            "Syntax-first Python preview evidence only.",
            "No repair card or agent packet emitted for static limits.",
            "No source edits, generated tests, mutation execution, provider calls, or gate authority."
        ]
    }))
}

fn python_static_limit_detail(finding: &Finding, static_limit_kind: &str) -> String {
    finding
        .missing
        .iter()
        .find_map(|detail| non_empty(detail).map(ToString::to_string))
        .or_else(|| {
            finding
                .evidence
                .iter()
                .find(|detail| {
                    detail.contains("static_limit") || detail.contains(static_limit_kind)
                })
                .cloned()
        })
        .unwrap_or_else(|| {
            format!(
                "Python preview reported static limit `{static_limit_kind}` without a bounded repair route."
            )
        })
}

fn non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
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
        ActivationEvidence, Confidence, DeltaKind, FindingCanonicalGap, FlowSinkFact, FlowSinkKind,
        LanguageId, LanguageStatus, OracleKind, OracleStrength, OwnerKind, Probe, ProbeFamily,
        ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence,
        StageState, StaticLimitKind, StopReason, Summary, SymbolId, ValueContext,
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
    fn sarif_preserves_finding_canonical_gap_properties() -> Result<(), String> {
        let mut output = sample_output();
        output.findings[0].canonical_gap = Some(FindingCanonicalGap {
            id: "gap:python:src/pricing.py:apply_discount:predicate_boundary:predicate:amount>=threshold"
                .to_string(),
            language: "python".to_string(),
            file: "src/pricing.py".to_string(),
            owner: "apply_discount".to_string(),
            behavior_kind: "predicate_boundary".to_string(),
            probe_kind: "predicate".to_string(),
            normalized_discriminator: "amount>=threshold".to_string(),
        });

        let rendered = render_findings_sarif(&output, &RiprConfig::default(), &[]);
        let sarif = parse_json(&rendered)?;
        let result = first_result(&sarif)?;

        assert_eq!(
            result["properties"]["canonical_gap_id"],
            "gap:python:src/pricing.py:apply_discount:predicate_boundary:predicate:amount>=threshold"
        );
        assert_eq!(result["properties"]["canonical_gap"]["language"], "python");
        assert_eq!(
            result["properties"]["canonical_gap"]["normalized_discriminator"],
            "amount>=threshold"
        );
        Ok(())
    }

    #[test]
    fn sarif_projects_python_repair_card_properties() -> Result<(), String> {
        let mut output = sample_output();
        let finding = &mut output.findings[0];
        finding.language = Some(LanguageId::Python);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.canonical_gap = Some(FindingCanonicalGap {
            id: "gap:python:src/pricing.py:apply_discount:predicate_boundary:predicate:amount>=threshold"
                .to_string(),
            language: "python".to_string(),
            file: "src/pricing.py".to_string(),
            owner: "apply_discount".to_string(),
            behavior_kind: "predicate_boundary".to_string(),
            probe_kind: "predicate".to_string(),
            normalized_discriminator: "amount>=threshold".to_string(),
        });
        finding.evidence.extend([
            "suggested_test_file: tests/test_pricing.py".to_string(),
            "suggested_test_name: test_apply_discount_threshold_boundary".to_string(),
            "suggested_test_node_id: tests/test_pricing.py::test_apply_discount_threshold_boundary"
                .to_string(),
            "suggested_verify_command: pytest tests/test_pricing.py::test_apply_discount_threshold_boundary"
                .to_string(),
            "suggested_verify_command_confidence: high".to_string(),
        ]);

        let rendered = render_findings_sarif(&output, &RiprConfig::default(), &[]);
        let sarif = parse_json(&rendered)?;
        let result = first_result(&sarif)?;
        let card = &result["properties"]["python_repair_card"];

        assert_eq!(card["language"], "python");
        assert_eq!(card["language_status"], "preview");
        assert_eq!(card["authority_boundary"], "preview_advisory_only");
        assert_eq!(
            card["canonical_gap_id"],
            "gap:python:src/pricing.py:apply_discount:predicate_boundary:predicate:amount>=threshold"
        );
        assert_eq!(
            card["missing_discriminator"],
            "amount == discount_threshold"
        );
        assert_eq!(
            card["suggested_location"]["test_file"],
            "tests/test_pricing.py"
        );
        assert_eq!(
            card["verify"]["command"],
            "pytest tests/test_pricing.py::test_apply_discount_threshold_boundary"
        );
        assert_eq!(
            card["receipt"]["status"],
            "unavailable_until_python_gap_ledger"
        );
        assert!(result["properties"].get("python_no_action").is_none());
        Ok(())
    }

    #[test]
    fn sarif_projects_python_static_limit_no_action_properties() -> Result<(), String> {
        let mut output = sample_output();
        let finding = &mut output.findings[0];
        finding.id = "probe:src_runtime.py:2:python_preview".to_string();
        finding.class = ExposureClass::StaticUnknown;
        finding.language = Some(LanguageId::Python);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.static_limit_kind = Some(StaticLimitKind::DynamicDispatch);
        finding.probe.id = ProbeId("probe:src_runtime.py:2:python_preview".to_string());
        finding.probe.location = SourceLocation::new("src/runtime.py", 2, 1);
        finding.probe.owner = Some(SymbolId("python:src/runtime.py::dispatch".to_string()));
        finding.probe.expression = "return getattr(handler, name)(payload)".to_string();
        finding.missing = vec![
            "Static limit `dynamic_dispatch` prevents bounded repair routing because syntax alone cannot resolve runtime getattr dispatch.".to_string(),
        ];
        finding.stop_reasons = vec![StopReason::DynamicDispatchUnresolved];
        finding.related_tests = vec![RelatedTest {
            name: "test_dispatch_total".to_string(),
            file: PathBuf::from("tests/test_runtime.py"),
            line: 4,
            oracle: Some("assert dispatch(\"total\", 10) == 10".to_string()),
            oracle_kind: OracleKind::ExactValue,
            oracle_strength: OracleStrength::Strong,
        }];

        let rendered = render_findings_sarif(&output, &RiprConfig::default(), &[]);
        let sarif = parse_json(&rendered)?;
        let result = first_result(&sarif)?;
        let no_action = &result["properties"]["python_no_action"];

        assert_eq!(result["ruleId"], "ripr.finding.static_unknown");
        assert_eq!(result["properties"]["language"], "python");
        assert_eq!(result["properties"]["language_status"], "preview");
        assert_eq!(
            result["properties"]["static_limit_kind"],
            "dynamic_dispatch"
        );
        assert!(result["properties"].get("python_repair_card").is_none());
        assert_eq!(no_action["authority_boundary"], "preview_advisory_only");
        assert_eq!(no_action["repairability"], "analyzer_limitation");
        assert_eq!(no_action["repair_packet_ready"], false);
        assert_eq!(no_action["repair_card_present"], false);
        assert_eq!(no_action["static_limit_kind"], "dynamic_dispatch");
        assert_eq!(
            no_action["changed_owner"],
            "python:src/runtime.py::dispatch"
        );
        assert!(
            no_action["why_not_actionable"]
                .as_str()
                .is_some_and(|detail| detail.contains("dynamic_dispatch"))
        );
        assert_eq!(no_action["verify"]["command"], Value::Null);
        assert_eq!(no_action["verify"]["status"], "not_applicable_static_limit");
        assert_eq!(no_action["receipt"]["command"], Value::Null);
        assert_eq!(
            no_action["receipt"]["status"],
            "not_applicable_static_limit"
        );
        assert_eq!(
            no_action["stop_conditions"][0],
            "dynamic_dispatch_unresolved"
        );
        Ok(())
    }

    #[test]
    fn sarif_projects_perl_preview_card_properties() -> Result<(), String> {
        let mut output = sample_output();
        add_perl_preview_card_inputs(&mut output.findings[0]);

        let rendered = render_findings_sarif(&output, &RiprConfig::default(), &[]);
        let sarif = parse_json(&rendered)?;
        let result = first_result(&sarif)?;
        let card = &result["properties"]["perl_preview_card"];

        assert_eq!(card["card_version"], "perl_preview_card.v1");
        assert_eq!(card["language"], "perl");
        assert_eq!(card["language_status"], "preview");
        assert_eq!(card["authority_boundary"], "preview_advisory_only");
        assert_eq!(card["surface_scope"], "check_json_human_sarif_github");
        assert_eq!(card["public_repair_packet"], false);
        assert_eq!(card["repair_packet_ready"], false);
        assert_eq!(card["agent_packet_ready"], false);
        assert_eq!(card["gate_candidate"], false);
        assert_eq!(card["badge_candidate"], false);
        assert_eq!(card["ripr_zero_candidate"], false);
        assert_eq!(card["packet_id"], "perl-preview:gap-return");
        assert_eq!(
            card["canonical_gap_id"],
            "gap:perl:lib/My/App.pm:My::App::discount:return_value:exact_return_assertion:return_value"
        );
        assert_eq!(
            card["changed_owner"],
            "perl:lib/My/App.pm::My::App::discount"
        );
        assert_eq!(card["repair_route"], "add_exact_return_assertion");
        assert_eq!(card["missing_discriminator"], "return_value");
        assert_eq!(
            card["target_test_shape"],
            "Test::More exact_return_assertion"
        );
        assert_eq!(card["suggested_test_location"], "t/app.t::discount_smoke");
        assert_eq!(card["verify"]["command"], "prove t/app.t");
        assert_eq!(card["verify"]["status"], "fact_only_not_delegated");
        assert!(card["receipt"]["command"].is_null());
        assert_eq!(card["receipt"]["status"], "available_not_delegated");
        assert_eq!(card["raw_evidence_refs"][0]["file"], "lib/My/App.pm");
        assert_eq!(card["raw_evidence_refs"][0]["line"], 8);
        assert!(card.get("allowed_edit_surface").is_none());
        assert!(card.get("allowed_edit_boundaries").is_none());
        assert!(card.get("forbidden_files").is_none());
        assert!(card.get("receipt_command").is_none());
        assert!(result["properties"].get("perl_repair_card").is_none());
        assert!(
            result["properties"]
                .get("perl_internal_agent_packet")
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn sarif_projects_python_ordinary_no_action_properties() -> Result<(), String> {
        let mut already_observed = sample_finding();
        already_observed.id = "probe:src_pricing.py:2:observed".to_string();
        already_observed.class = ExposureClass::Exposed;
        already_observed.language = Some(LanguageId::Python);
        already_observed.language_status = Some(LanguageStatus::Preview);
        already_observed.probe.owner =
            Some(SymbolId("python:src/pricing.py::discount".to_string()));
        already_observed.recommended_next_step = None;

        let mut no_related = sample_finding();
        no_related.id = "probe:src_pricing.py:4:no_path".to_string();
        no_related.class = ExposureClass::NoStaticPath;
        no_related.language = Some(LanguageId::Python);
        no_related.language_status = Some(LanguageStatus::Preview);
        no_related.probe.location = SourceLocation::new("src/pricing.py", 4, 1);
        no_related.probe.owner = Some(SymbolId("python:src/pricing.py::discount".to_string()));
        no_related.related_tests.clear();
        no_related.recommended_next_step = None;

        let mut heuristic_only = sample_finding();
        heuristic_only.id = "probe:src_pricing.py:6:heuristic".to_string();
        heuristic_only.class = ExposureClass::WeaklyExposed;
        heuristic_only.language = Some(LanguageId::Python);
        heuristic_only.language_status = Some(LanguageStatus::Preview);
        heuristic_only.probe.location = SourceLocation::new("src/pricing.py", 6, 1);
        heuristic_only.probe.owner = Some(SymbolId("python:src/pricing.py::discount".to_string()));
        heuristic_only.evidence =
            vec!["related_test_uncertain: test_name_similarity (test_discount)".to_string()];
        heuristic_only.recommended_next_step = None;

        let output = CheckOutput {
            findings: vec![already_observed, no_related, heuristic_only],
            ..sample_output()
        };

        let rendered = render_findings_sarif(&output, &RiprConfig::default(), &[]);
        let sarif = parse_json(&rendered)?;
        let results = results(&sarif)?;

        let expected = [
            (
                "already_observed",
                "missing_proof_already_observed",
                "already observes",
            ),
            (
                "no_related_test",
                "related_python_test_not_found",
                "No related Python test",
            ),
            (
                "heuristic_only",
                "related_test_link_uncertain",
                "Only heuristic Python related-test proximity",
            ),
        ];
        for (result, (kind, stop_condition, reason_text)) in results.iter().zip(expected) {
            let no_action = &result["properties"]["python_no_action"];
            assert!(result["properties"].get("python_repair_card").is_none());
            assert_eq!(no_action["authority_boundary"], "preview_advisory_only");
            assert_eq!(no_action["repairability"], "no_action");
            assert_eq!(no_action["repair_packet_ready"], false);
            assert_eq!(no_action["repair_card_present"], false);
            assert_eq!(no_action["gap_state"], kind);
            assert_eq!(no_action["no_action_kind"], kind);
            assert_eq!(
                no_action["changed_owner"],
                "python:src/pricing.py::discount"
            );
            assert!(
                no_action["why_not_actionable"]
                    .as_str()
                    .is_some_and(|detail| detail.contains(reason_text)),
                "expected reason containing {reason_text:?}, got {no_action:?}"
            );
            assert_eq!(no_action["verify"]["command"], Value::Null);
            assert_eq!(no_action["verify"]["status"], "not_applicable_no_action");
            assert_eq!(no_action["receipt"]["command"], Value::Null);
            assert_eq!(no_action["receipt"]["status"], "not_applicable_no_action");
            assert_eq!(no_action["stop_conditions"][0], stop_condition);
        }
        Ok(())
    }

    #[test]
    fn sarif_preserves_preview_actionability_properties() -> Result<(), String> {
        let mut output = sample_output();
        let finding = &mut output.findings[0];
        finding.language = Some(LanguageId::TypeScript);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.owner_kind = Some(OwnerKind::Function);
        finding.static_limit_kind = Some(StaticLimitKind::MockedModule);
        finding.evidence = vec![
            "gap_state: advisory".to_string(),
            "actionability_category: incomplete_repair_packet".to_string(),
            "why_not_actionable: TypeScript preview lacks a complete repair packet contract"
                .to_string(),
            "repair_route: project canonical TypeScript repair packet fields later".to_string(),
            "missing_actionability_fields: canonical_gap_id, verify_command".to_string(),
            "evidence_needed_to_promote: canonical gap identity and verify command".to_string(),
            "raw_evidence_ref: file=src/lib.ts;line=2;kind=typescript_preview_probe;source_id=probe:src_lib.ts:2:typescript_preview;owner=discountedTotal".to_string(),
        ];

        let rendered = render_findings_sarif(&output, &RiprConfig::default(), &[]);
        let sarif = parse_json(&rendered)?;
        let result = first_result(&sarif)?;

        assert_eq!(result["properties"]["language"], "typescript");
        assert_eq!(result["properties"]["language_status"], "preview");
        assert_eq!(result["properties"]["owner_kind"], "function");
        assert_eq!(result["properties"]["static_limit_kind"], "mocked_module");
        assert_eq!(
            result["properties"]["preview_actionability"]["authority_boundary"],
            "preview_advisory_only"
        );
        assert_eq!(
            result["properties"]["preview_actionability"]["repair_packet_ready"],
            false
        );
        assert_eq!(
            result["properties"]["preview_actionability"]["raw_evidence_refs"][0]["file"],
            "src/lib.ts"
        );
        Ok(())
    }

    #[test]
    fn sarif_preserves_bun_cross_language_grip_card_properties() -> Result<(), String> {
        let mut output = sample_output();
        let finding = &mut output.findings[0];
        finding.language = Some(LanguageId::TypeScript);
        finding.language_status = Some(LanguageStatus::Preview);
        finding.owner_kind = Some(OwnerKind::Function);
        finding.evidence = vec![
            "owner: Blob::from_js_without_defer_gc".to_string(),
            "gap_state: static_limitation".to_string(),
            "actionability_category: cross_language_oracle_visibility_unresolved".to_string(),
            "why_not_actionable: configured Bun Blob TypeScript preview evidence is missing external discriminator(s): resizable_array_buffer; placement can name the existing TypeScript Blob test file, but RIPR cannot emit a public repair packet without verification, receipt, and edit-surface evidence".to_string(),
            "repair_route: analysis/cross-language-oracle-visibility".to_string(),
            "missing_actionability_fields: verify_command, receipt_command, must_not_change, allowed_edit_surface".to_string(),
            "missing_graph_legs: boundary_discriminator:resizable_array_buffer".to_string(),
            "unlock_condition: add or inspect the missing external TypeScript discriminator(s) in test/js/web/fetch/blob.test.ts and keep repair-packet projection blocked until verify, receipt, and edit-surface evidence exists".to_string(),
            "evidence_needed_to_promote: the missing TypeScript discriminator in the configured Blob test file plus verify command, receipt command, and edit constraints before repair-packet projection".to_string(),
            "raw_evidence_ref: leg=rust_seam;file=src/jsc/Blob.rs;line=42;kind=rust_boundary;source_id=probe:src_jsc_Blob_rs:42:typescript_bun_ub_cross_language_preview;owner=Blob::from_js_without_defer_gc;sample=array_buffer.shared || array_buffer.resizable".to_string(),
            "typescript_bun_ub_bridge_hint: confidence=configured_hint rust_file=src/jsc/Blob.rs rust_owner=Blob::from_js_without_defer_gc rust_boundary=\"array_buffer.shared || array_buffer.resizable\" ts_test_file=test/js/web/fetch/blob.test.ts".to_string(),
            "typescript_bun_ub_bridge_verdict: ts_missing_resizable missing_discriminators=resizable_array_buffer action=route_cross_language_oracle_visibility_limitation suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
            "typescript_bun_ub_cross_language_grip: state=rust_ungripped_ts_missing_discriminator rust_grip=ungripped ts_verdict=ts_missing_resizable action=route_cross_language_oracle_visibility_limitation authority=preview_advisory_only suggested_test_file=test/js/web/fetch/blob.test.ts repair_packet_ready=false".to_string(),
            "typescript_bun_ub_test_placement: rank=1 suggested_test_file=test/js/web/fetch/blob.test.ts reason=\"existing Blob + ArrayBuffer integration tests live there; missing discriminator is resizable ArrayBuffer\" basis=configured_bridge_suggested_test_file,same_js_surface,same_boundary_vocabulary authority=preview_advisory_only repair_packet_ready=false".to_string(),
        ];
        finding.activation.missing_discriminators = vec![MissingDiscriminatorFact {
            value: "resizable_array_buffer".to_string(),
            reason: "missing resizable ArrayBuffer discriminator".to_string(),
            flow_sink: None,
        }];

        let rendered = render_findings_sarif(&output, &RiprConfig::default(), &[]);
        let sarif = parse_json(&rendered)?;
        let result = first_result(&sarif)?;
        let grip = &result["properties"]["typescript_preview_card"]["bun_cross_language_grip"];

        assert_eq!(grip["state"], "rust_ungripped_ts_missing_discriminator");
        assert_eq!(grip["rust_seam"]["file"], "src/jsc/Blob.rs");
        assert_eq!(
            grip["typescript_evidence"]["missing_discriminators"][0],
            "resizable_array_buffer"
        );
        assert_eq!(
            grip["limitation_category"],
            "cross_language_oracle_visibility_unresolved"
        );
        assert_eq!(
            grip["repair_route"],
            "analysis/cross-language-oracle-visibility"
        );
        assert_eq!(
            grip["missing_graph_legs"][0],
            "boundary_discriminator:resizable_array_buffer"
        );
        assert_eq!(grip["raw_evidence_refs"][0]["leg"], "rust_seam");
        assert_eq!(
            grip["suggested_test_file"],
            "test/js/web/fetch/blob.test.ts"
        );
        assert_eq!(
            grip["placement"]["suggested_test_file"],
            "test/js/web/fetch/blob.test.ts"
        );
        assert_eq!(
            grip["placement"]["reason"],
            "existing Blob + ArrayBuffer integration tests live there; missing discriminator is resizable ArrayBuffer"
        );
        assert_eq!(grip["placement"]["repair_packet_ready"], false);
        assert_eq!(grip["repair_packet_ready"], false);
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

    fn add_perl_preview_card_inputs(finding: &mut Finding) {
        finding.id = "probe:lib_My_App_pm:8:perl_return".to_string();
        finding.canonical_gap = Some(FindingCanonicalGap {
            id: "gap:perl:lib/My/App.pm:My::App::discount:return_value:exact_return_assertion:return_value"
                .to_string(),
            language: "perl".to_string(),
            file: "lib/My/App.pm".to_string(),
            owner: "perl:lib/My/App.pm::My::App::discount".to_string(),
            behavior_kind: "return_value".to_string(),
            probe_kind: "exact_return_assertion".to_string(),
            normalized_discriminator: "return_value".to_string(),
        });
        finding.probe = Probe {
            id: ProbeId("probe:lib_My_App_pm:8:perl_return".to_string()),
            location: SourceLocation::new("lib/My/App.pm", 8, 5),
            owner: Some(SymbolId(
                "perl:lib/My/App.pm::My::App::discount".to_string(),
            )),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: Some("return $price".to_string()),
            after: Some("return $discounted".to_string()),
            expression: "return $discounted".to_string(),
            expected_sinks: vec!["return_value".to_string()],
            required_oracles: vec!["exact_return_assertion".to_string()],
        };
        finding.class = ExposureClass::WeaklyExposed;
        finding.ripr = RiprEvidence {
            reach: stage(
                StageState::Yes,
                "Perl fact packet links the related test to the changed owner",
            ),
            infect: stage(
                StageState::Yes,
                "Changed return value reaches the owner result",
            ),
            propagate: stage(
                StageState::Yes,
                "Return value can propagate to Test::More assertion",
            ),
            reveal: RevealEvidence {
                observe: stage(StageState::Yes, "Related test reaches the changed owner"),
                discriminate: stage(StageState::Weak, "Exact return discriminator is missing"),
            },
        };
        finding.confidence = 0.8;
        finding.evidence = vec![
            "perl_packet_id: perl-preview:gap-return".to_string(),
            "perl_repair_kind: add_exact_return_assertion".to_string(),
            "perl_target_test_shape: Test::More exact_return_assertion".to_string(),
            "perl_suggested_test_location: t/app.t::discount_smoke".to_string(),
            "perl_suggested_assertion: assert the exact returned `return_value` value".to_string(),
            "perl_verify_command: prove t/app.t".to_string(),
            "perl_receipt_command: ripr agent receipt --root . --verify-json target/ripr/workflow/agent-verify.json --seam-id perl-gap --json".to_string(),
            "perl_confidence: medium".to_string(),
            "perl_allowed_edit_boundary: t/app.t".to_string(),
            "perl_forbidden_edit_boundary: lib/My/App.pm, badges/ripr-plus.json".to_string(),
            "perl_stop_if: perl-lsp packet status changes".to_string(),
            "perl_must_not_change: do not edit Perl production code".to_string(),
            "raw_evidence_ref: leg=perl_change;file=lib/My/App.pm;line=8;kind=perl_change;source_id=change:lib/My/App.pm:8:return;owner=perl:lib/My/App.pm::My::App::discount;sample=return $discounted".to_string(),
            "raw_evidence_ref: leg=perl_oracle;file=t/app.t;line=7;kind=perl_oracle;source_id=oracle:t/app.t:7:is;owner=perl:lib/My/App.pm::My::App::discount;sample=is(discount(...), 90)".to_string(),
        ];
        finding.missing = vec!["return_value".to_string()];
        finding.activation.missing_discriminators = vec![MissingDiscriminatorFact {
            value: "return_value".to_string(),
            reason: "Related Perl test reaches the owner but lacks an exact return discriminator"
                .to_string(),
            flow_sink: None,
        }];
        finding.related_tests = vec![RelatedTest {
            name: "discount_smoke".to_string(),
            file: PathBuf::from("t/app.t"),
            line: 7,
            oracle: Some("ok(discount(...))".to_string()),
            oracle_kind: OracleKind::SmokeOnly,
            oracle_strength: OracleStrength::Weak,
        }];
        finding.recommended_next_step = Some("Add a focused Perl assertion.".to_string());
        finding.language = Some(LanguageId::Perl);
        finding.language_status = Some(LanguageStatus::Preview);
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
            canonical_gap: None,
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
            owner_kind: None,
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
