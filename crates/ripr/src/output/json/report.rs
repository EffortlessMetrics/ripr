use crate::app::CheckOutput;
use crate::config::RiprConfig;
use crate::domain::{
    Finding, FindingCanonicalGap, FlowSinkFact, MissingDiscriminatorFact, RelatedTest,
    StageEvidence, ValueFact,
};
use crate::output::perl_preview_card::{perl_preview_card, perl_preview_card_json_value};
use crate::output::preview_actionability::{
    preview_actionability_for, preview_actionability_json_value,
};
use crate::output::python_repair_card::{PythonRepairCard, python_repair_card};
use crate::output::typescript_preview_card::{
    typescript_preview_card, typescript_preview_card_json_value,
};
use serde_json::Value;
use std::collections::BTreeMap;

use super::finding_alignment;
use super::{array_field, escape, field, float_field, number_field};

pub fn render(output: &CheckOutput) -> String {
    render_with_config(output, &RiprConfig::default())
}

pub(crate) fn render_with_config(output: &CheckOutput, config: &RiprConfig) -> String {
    let finding_alignment = finding_alignment::report_for_findings(&output.findings);
    let mut out = String::new();
    out.push_str("{\n");
    field(&mut out, 1, "schema_version", &output.schema_version, true);
    field(&mut out, 1, "tool", &output.tool, true);
    field(&mut out, 1, "mode", output.mode.as_str(), true);
    field(
        &mut out,
        1,
        "root",
        &output.root.display().to_string(),
        true,
    );
    if let Some(base) = &output.base {
        field(&mut out, 1, "base", base, true);
    }
    out.push_str("  \"summary\": ");
    summary_json(&mut out, output);
    out.push_str(",\n");
    out.push_str("  \"findings\": [\n");
    let canonical_gap_counts = canonical_gap_counts(&output.findings);
    for (idx, finding) in output.findings.iter().enumerate() {
        finding_json_with_config_and_counts(&mut out, finding, 2, config, &canonical_gap_counts);
        if idx + 1 != output.findings.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]");
    if let Some(report) = finding_alignment.as_ref() {
        out.push_str(",\n");
        out.push_str("  \"finding_alignment\": ");
        finding_alignment::report_json(&mut out, report, 1);
        out.push('\n');
    } else {
        out.push('\n');
    }
    out.push_str("}\n");
    out
}

fn summary_json(out: &mut String, output: &CheckOutput) {
    let s = &output.summary;
    out.push_str(&format!(
        "{{\"changed_rust_files\":{},\"probes\":{},\"findings\":{},\"exposed\":{},\"weakly_exposed\":{},\"reachable_unrevealed\":{},\"no_static_path\":{},\"infection_unknown\":{},\"propagation_unknown\":{},\"static_unknown\":{}}}",
        s.changed_rust_files,
        s.probes,
        s.findings,
        s.exposed,
        s.weakly_exposed,
        s.reachable_unrevealed,
        s.no_static_path,
        s.infection_unknown,
        s.propagation_unknown,
        s.static_unknown
    ));
}

#[cfg(test)]
pub(super) fn finding_json(out: &mut String, finding: &Finding, indent: usize) {
    finding_json_with_config_and_counts(
        out,
        finding,
        indent,
        &RiprConfig::default(),
        &BTreeMap::new(),
    );
}

fn finding_json_with_config_and_counts(
    out: &mut String,
    finding: &Finding,
    indent: usize,
    config: &RiprConfig,
    canonical_gap_counts: &BTreeMap<&str, usize>,
) {
    let sp = "  ".repeat(indent);
    out.push_str(&format!("{sp}{{\n"));
    field(out, indent + 1, "id", &finding.id, true);
    if let Some(gap) = &finding.canonical_gap {
        field(out, indent + 1, "canonical_gap_id", &gap.id, true);
        number_field(
            out,
            indent + 1,
            "canonical_gap_group_size",
            canonical_gap_counts
                .get(gap.id.as_str())
                .copied()
                .unwrap_or(1),
            true,
        );
        canonical_gap_json(out, gap, indent + 1, true);
    }
    field(
        out,
        indent + 1,
        "classification",
        finding.class.as_str(),
        true,
    );
    field(
        out,
        indent + 1,
        "severity",
        config.severity().for_exposure(&finding.class).as_str(),
        true,
    );
    float_field(out, indent + 1, "confidence", finding.confidence, true);
    out.push_str(&format!("{}\"probe\": {{\n", "  ".repeat(indent + 1)));
    field(out, indent + 2, "id", &finding.probe.id.0, true);
    field(
        out,
        indent + 2,
        "family",
        finding.probe.family.as_str(),
        true,
    );
    field(out, indent + 2, "delta", finding.probe.delta.as_str(), true);
    field(
        out,
        indent + 2,
        "file",
        &finding.probe.location.file.display().to_string(),
        true,
    );
    number_field(out, indent + 2, "line", finding.probe.location.line, true);
    let render_probe_owner = finding.probe.owner.is_some() && finding.language_status.is_some();
    field(
        out,
        indent + 2,
        "expression",
        &finding.probe.expression,
        render_probe_owner,
    );
    if render_probe_owner && let Some(owner) = &finding.probe.owner {
        field(out, indent + 2, "owner", &owner.0, false);
    }
    out.push_str(&format!("{} }},\n", "  ".repeat(indent + 1)));
    out.push_str(&format!("{}\"ripr\": {{\n", "  ".repeat(indent + 1)));
    stage_json(out, indent + 2, "reach", &finding.ripr.reach, true);
    stage_json(out, indent + 2, "infect", &finding.ripr.infect, true);
    stage_json(out, indent + 2, "propagate", &finding.ripr.propagate, true);
    stage_json(
        out,
        indent + 2,
        "observe",
        &finding.ripr.reveal.observe,
        true,
    );
    stage_json(
        out,
        indent + 2,
        "discriminate",
        &finding.ripr.reveal.discriminate,
        false,
    );
    out.push_str(&format!("{} }},\n", "  ".repeat(indent + 1)));
    let evidence_path = evidence_path_values(finding);
    array_field(out, indent + 1, "evidence_path", &evidence_path, true);
    flow_sinks_json(out, finding, indent + 1);
    out.push_str(",\n");
    array_field(out, indent + 1, "evidence", &finding.evidence, true);
    array_field(out, indent + 1, "missing", &finding.missing, true);
    activation_json(out, finding, indent + 1);
    out.push_str(",\n");
    value_facts_array_json(
        out,
        "observed_values",
        &finding.activation.observed_values,
        indent + 1,
    );
    out.push_str(",\n");
    missing_discriminators_array_json(
        out,
        "missing_discriminators",
        &finding.activation.missing_discriminators,
        indent + 1,
    );
    out.push_str(",\n");
    out.push_str(&format!(
        "{}\"related_tests\": [\n",
        "  ".repeat(indent + 1)
    ));
    for (idx, test) in finding.related_tests.iter().enumerate() {
        related_test_json(out, test, indent + 2);
        if idx + 1 != finding.related_tests.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{}],\n", "  ".repeat(indent + 1)));
    if let Some(placement) = repair_placement_from_evidence(finding) {
        repair_placement_json(out, &placement, indent + 1);
        out.push_str(",\n");
    }
    if let Some(card) = python_repair_card(finding) {
        python_repair_card_json(out, &card, indent + 1);
        out.push_str(",\n");
    }
    if let Some(card) = typescript_preview_card(finding) {
        json_value_field(
            out,
            indent + 1,
            "typescript_preview_card",
            &typescript_preview_card_json_value(&card),
        );
        out.push_str(",\n");
    }
    if let Some(card) = perl_preview_card(finding) {
        json_value_field(
            out,
            indent + 1,
            "perl_preview_card",
            &perl_preview_card_json_value(&card),
        );
        out.push_str(",\n");
    }
    if let Some(actionability) = preview_actionability_for(finding) {
        json_value_field(
            out,
            indent + 1,
            "preview_actionability",
            &preview_actionability_json_value(&actionability),
        );
        out.push_str(",\n");
    }
    let stop_reasons = stop_reason_values(finding);
    array_field(out, indent + 1, "stop_reasons", &stop_reasons, true);
    let strongest = strongest_related_test(finding);
    field(
        out,
        indent + 1,
        "oracle_kind",
        strongest
            .map(|test| test.oracle_kind.as_str())
            .unwrap_or("unknown"),
        true,
    );
    field(
        out,
        indent + 1,
        "oracle_strength",
        strongest
            .map(|test| test.oracle_strength.as_str())
            .unwrap_or("none"),
        true,
    );
    field(
        out,
        indent + 1,
        "recommended_next_step",
        finding.recommended_next_step.as_deref().unwrap_or(""),
        true,
    );
    let has_language = finding.language.is_some();
    let has_status = finding.language_status.is_some();
    let has_owner_kind = finding.owner_kind.is_some();
    let has_static_limit_kind = finding.static_limit_kind.is_some();
    field(
        out,
        indent + 1,
        "suggested_next_action",
        finding.recommended_next_step.as_deref().unwrap_or(""),
        has_language || has_status || has_owner_kind || has_static_limit_kind,
    );
    if let Some(language) = finding.language {
        field(
            out,
            indent + 1,
            "language",
            language.as_str(),
            has_status || has_owner_kind || has_static_limit_kind,
        );
    }
    if let Some(status) = finding.language_status {
        field(
            out,
            indent + 1,
            "language_status",
            status.as_str(),
            has_owner_kind || has_static_limit_kind,
        );
    }
    if let Some(kind) = finding.owner_kind {
        field(
            out,
            indent + 1,
            "owner_kind",
            kind.as_str(),
            has_static_limit_kind,
        );
    }
    if let Some(kind) = finding.static_limit_kind {
        field(out, indent + 1, "static_limit_kind", kind.as_str(), false);
    }
    out.push_str(&format!("{sp}}}"));
}

fn canonical_gap_counts(findings: &[Finding]) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for finding in findings {
        if let Some(gap) = &finding.canonical_gap {
            *counts.entry(gap.id.as_str()).or_insert(0) += 1;
        }
    }
    counts
}

fn canonical_gap_json(out: &mut String, gap: &FindingCanonicalGap, indent: usize, trailing: bool) {
    let sp = "  ".repeat(indent);
    out.push_str(&format!("{sp}\"canonical_gap\": {{\n"));
    field(out, indent + 1, "id", &gap.id, true);
    field(out, indent + 1, "language", &gap.language, true);
    field(out, indent + 1, "file", &gap.file, true);
    field(out, indent + 1, "owner", &gap.owner, true);
    field(out, indent + 1, "behavior_kind", &gap.behavior_kind, true);
    field(out, indent + 1, "probe_kind", &gap.probe_kind, true);
    field(
        out,
        indent + 1,
        "normalized_discriminator",
        &gap.normalized_discriminator,
        false,
    );
    out.push('\n');
    out.push_str(&format!("{sp}}}"));
    if trailing {
        out.push(',');
    }
    out.push('\n');
}

fn evidence_path_values(finding: &Finding) -> Vec<String> {
    let mut values = vec![
        format!(
            "reach {}: {}",
            finding.ripr.reach.state.as_str(),
            finding.ripr.reach.summary
        ),
        format!(
            "infection {}: {}",
            finding.ripr.infect.state.as_str(),
            finding.ripr.infect.summary
        ),
        format!(
            "propagation {}: {}",
            finding.ripr.propagate.state.as_str(),
            finding.ripr.propagate.summary
        ),
        format!(
            "observation {}: {}",
            finding.ripr.reveal.observe.state.as_str(),
            finding.ripr.reveal.observe.summary
        ),
        format!(
            "discriminator {}: {}",
            finding.ripr.reveal.discriminate.state.as_str(),
            finding.ripr.reveal.discriminate.summary
        ),
    ];

    values.extend(finding.flow_sinks.iter().map(|sink| {
        format!(
            "local flow reaches {}: {} (line {})",
            sink.kind.label(),
            sink.text,
            sink.line
        )
    }));

    values.extend(finding.related_tests.iter().take(5).map(|test| {
        let oracle_kind = display_label(test.oracle_kind.as_str());
        let mut value = format!(
            "related test {}:{} {} uses {} {} oracle",
            test.file.display(),
            test.line,
            test.name,
            test.oracle_strength.as_str(),
            oracle_kind
        );
        if let Some(oracle) = &test.oracle {
            value.push_str(&format!(": {oracle}"));
        }
        value
    }));

    values.extend(
        finding
            .activation
            .observed_values
            .iter()
            .take(8)
            .map(|fact| {
                let context = display_label(fact.context.as_str());
                format!(
                    "observed {} value {} at line {}",
                    context, fact.value, fact.line
                )
            }),
    );

    values.extend(
        finding
            .activation
            .missing_discriminators
            .iter()
            .map(|fact| format!("missing discriminator {}: {}", fact.value, fact.reason)),
    );

    values
}

fn display_label(value: &str) -> String {
    value.replace('_', " ")
}

fn strongest_related_test(finding: &Finding) -> Option<&RelatedTest> {
    finding
        .related_tests
        .iter()
        .max_by_key(|test| test.oracle_strength.rank())
}

fn activation_json(out: &mut String, finding: &Finding, indent: usize) {
    let sp = "  ".repeat(indent);
    out.push_str(&format!("{sp}\"activation\": {{\n"));
    value_facts_array_json(
        out,
        "observed_values",
        &finding.activation.observed_values,
        indent + 1,
    );
    out.push_str(",\n");
    missing_discriminators_array_json(
        out,
        "missing_discriminators",
        &finding.activation.missing_discriminators,
        indent + 1,
    );
    out.push('\n');
    out.push_str(&format!("{sp}}}"));
}

fn flow_sinks_json(out: &mut String, finding: &Finding, indent: usize) {
    out.push_str(&format!("{}\"flow_sinks\": [\n", "  ".repeat(indent)));
    for (idx, sink) in finding.flow_sinks.iter().enumerate() {
        out.push_str(&"  ".repeat(indent + 1));
        flow_sink_json(out, sink);
        if idx + 1 != finding.flow_sinks.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{}]", "  ".repeat(indent)));
}

fn value_facts_array_json(out: &mut String, name: &str, facts: &[ValueFact], indent: usize) {
    out.push_str(&format!("{}\"{name}\": [\n", "  ".repeat(indent)));
    for (idx, value) in facts.iter().enumerate() {
        value_fact_json(out, value, indent + 1);
        if idx + 1 != facts.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{}]", "  ".repeat(indent)));
}

fn missing_discriminators_array_json(
    out: &mut String,
    name: &str,
    facts: &[MissingDiscriminatorFact],
    indent: usize,
) {
    out.push_str(&format!("{}\"{name}\": [\n", "  ".repeat(indent)));
    for (idx, discriminator) in facts.iter().enumerate() {
        missing_discriminator_json(out, discriminator, indent + 1);
        if idx + 1 != facts.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&format!("{}]", "  ".repeat(indent)));
}

fn value_fact_json(out: &mut String, fact: &ValueFact, indent: usize) {
    let sp = "  ".repeat(indent);
    out.push_str(&format!("{sp}{{\n"));
    number_field(out, indent + 1, "line", fact.line, true);
    field(out, indent + 1, "text", &fact.text, true);
    field(out, indent + 1, "value", &fact.value, true);
    field(out, indent + 1, "context", fact.context.as_str(), false);
    out.push_str(&format!("{sp}}}"));
}

fn missing_discriminator_json(out: &mut String, fact: &MissingDiscriminatorFact, indent: usize) {
    let sp = "  ".repeat(indent);
    out.push_str(&format!("{sp}{{\n"));
    field(out, indent + 1, "value", &fact.value, true);
    field(out, indent + 1, "reason", &fact.reason, true);
    out.push_str(&format!("{}\"flow_sink\": ", "  ".repeat(indent + 1)));
    if let Some(sink) = &fact.flow_sink {
        flow_sink_json(out, sink);
    } else {
        out.push_str("null");
    }
    out.push('\n');
    out.push_str(&format!("{sp}}}"));
}

fn flow_sink_json(out: &mut String, sink: &FlowSinkFact) {
    out.push_str(&format!(
        "{{\"kind\":\"{}\",\"text\":\"{}\",\"line\":{}}}",
        sink.kind.as_str(),
        escape(&sink.text),
        sink.line
    ));
}

struct RepairPlacement {
    suggested_test_file: String,
    suggested_test_name: String,
    suggested_test_node_id: Option<String>,
    verify_command: String,
    verify_command_confidence: String,
}

fn repair_placement_from_evidence(finding: &Finding) -> Option<RepairPlacement> {
    Some(RepairPlacement {
        suggested_test_file: evidence_value(finding, "suggested_test_file: ")?.to_string(),
        suggested_test_name: evidence_value(finding, "suggested_test_name: ")?.to_string(),
        suggested_test_node_id: evidence_value(finding, "suggested_test_node_id: ")
            .map(ToString::to_string),
        verify_command: evidence_value(finding, "suggested_verify_command: ")?.to_string(),
        verify_command_confidence: evidence_value(
            finding,
            "suggested_verify_command_confidence: ",
        )?
        .to_string(),
    })
}

fn evidence_value<'a>(finding: &'a Finding, prefix: &str) -> Option<&'a str> {
    finding
        .evidence
        .iter()
        .find_map(|entry| entry.strip_prefix(prefix))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn repair_placement_json(out: &mut String, placement: &RepairPlacement, indent: usize) {
    out.push_str(&format!(
        "{}\"repair_placement\": {{\n",
        "  ".repeat(indent)
    ));
    field(
        out,
        indent + 1,
        "suggested_test_file",
        &placement.suggested_test_file,
        true,
    );
    field(
        out,
        indent + 1,
        "suggested_test_name",
        &placement.suggested_test_name,
        true,
    );
    if let Some(node_id) = &placement.suggested_test_node_id {
        field(out, indent + 1, "suggested_test_node_id", node_id, true);
    }
    field(
        out,
        indent + 1,
        "verify_command",
        &placement.verify_command,
        true,
    );
    field(
        out,
        indent + 1,
        "verify_command_confidence",
        &placement.verify_command_confidence,
        false,
    );
    out.push_str(&format!("{} }}", "  ".repeat(indent)));
}

fn json_value_field(out: &mut String, indent: usize, name: &str, value: &Value) {
    let sp = "  ".repeat(indent);
    out.push_str(&format!("{sp}\"{name}\": "));
    match serde_json::to_string_pretty(value) {
        Ok(rendered) => {
            for (idx, line) in rendered.lines().enumerate() {
                if idx > 0 {
                    out.push('\n');
                    out.push_str(&sp);
                }
                out.push_str(line);
            }
        }
        Err(_) => out.push_str("null"),
    }
}

fn python_repair_card_json(out: &mut String, card: &PythonRepairCard, indent: usize) {
    let sp = "  ".repeat(indent);
    out.push_str(&format!("{sp}\"python_repair_card\": {{\n"));
    field(out, indent + 1, "card_version", &card.card_version, true);
    field(out, indent + 1, "source", &card.source, true);
    field(
        out,
        indent + 1,
        "canonical_gap_id",
        &card.canonical_gap_id,
        true,
    );
    field(out, indent + 1, "language", &card.language, true);
    field(
        out,
        indent + 1,
        "language_status",
        &card.language_status,
        true,
    );
    field(
        out,
        indent + 1,
        "authority_boundary",
        &card.authority_boundary,
        true,
    );
    field(out, indent + 1, "repair_action", &card.repair_action, true);
    field(out, indent + 1, "changed_owner", &card.changed_owner, true);
    field(
        out,
        indent + 1,
        "changed_behavior",
        &card.changed_behavior,
        true,
    );
    field(
        out,
        indent + 1,
        "current_test_evidence",
        &card.current_test_evidence,
        true,
    );
    field(
        out,
        indent + 1,
        "missing_discriminator",
        &card.missing_discriminator,
        true,
    );
    field(
        out,
        indent + 1,
        "recommended_test_shape",
        &card.recommended_test_shape,
        true,
    );
    field(
        out,
        indent + 1,
        "suggested_assertion",
        &card.suggested_assertion,
        true,
    );
    out.push_str(&format!(
        "{}\"suggested_location\": {{\n",
        "  ".repeat(indent + 1)
    ));
    field(
        out,
        indent + 2,
        "test_file",
        &card.suggested_test_file,
        true,
    );
    field(
        out,
        indent + 2,
        "test_name",
        &card.suggested_test_name,
        card.suggested_test_node_id.is_some(),
    );
    if let Some(node_id) = &card.suggested_test_node_id {
        field(out, indent + 2, "pytest_node_id", node_id, false);
    }
    out.push_str(&format!("{} }},\n", "  ".repeat(indent + 1)));
    out.push_str(&format!("{}\"verify\": {{\n", "  ".repeat(indent + 1)));
    field(out, indent + 2, "command", &card.verify_command, true);
    field(
        out,
        indent + 2,
        "confidence",
        &card.verify_command_confidence,
        false,
    );
    out.push_str(&format!("{} }},\n", "  ".repeat(indent + 1)));
    out.push_str(&format!("{}\"receipt\": {{\n", "  ".repeat(indent + 1)));
    if let Some(command) = &card.receipt_command {
        field(out, indent + 2, "command", command, true);
    } else {
        out.push_str(&format!("{}\"command\": null,\n", "  ".repeat(indent + 2)));
    }
    field(out, indent + 2, "status", &card.receipt_status, true);
    field(out, indent + 2, "guidance", &card.receipt_guidance, false);
    out.push_str(&format!("{} }},\n", "  ".repeat(indent + 1)));
    array_field(
        out,
        indent + 1,
        "stop_conditions",
        &card.stop_conditions,
        true,
    );
    array_field(out, indent + 1, "limits", &card.limits, false);
    out.push_str(&format!("{sp} }}"));
}

pub(super) fn stop_reason_values(finding: &Finding) -> Vec<String> {
    finding
        .effective_stop_reasons()
        .iter()
        .map(|reason| reason.as_str().to_string())
        .collect()
}

fn stage_json(out: &mut String, indent: usize, name: &str, stage: &StageEvidence, trailing: bool) {
    let sp = "  ".repeat(indent);
    out.push_str(&format!(
        "{sp}\"{name}\": {{\"state\":\"{}\",\"confidence\":\"{}\",\"summary\":\"{}\"}}{}\n",
        stage.state.as_str(),
        stage.confidence.as_str(),
        escape(&stage.summary),
        if trailing { "," } else { "" }
    ));
}

pub(super) fn related_test_json(out: &mut String, test: &RelatedTest, indent: usize) {
    let sp = "  ".repeat(indent);
    out.push_str(&format!("{sp}{{\n"));
    field(out, indent + 1, "name", &test.name, true);
    field(
        out,
        indent + 1,
        "file",
        &test.file.display().to_string(),
        true,
    );
    number_field(out, indent + 1, "line", test.line, true);
    field(
        out,
        indent + 1,
        "oracle_strength",
        test.oracle_strength.as_str(),
        true,
    );
    field(
        out,
        indent + 1,
        "oracle_kind",
        test.oracle_kind.as_str(),
        true,
    );
    field(
        out,
        indent + 1,
        "oracle",
        test.oracle.as_deref().unwrap_or(""),
        false,
    );
    out.push_str(&format!("{sp}}}"));
}
