use crate::app::CheckOutput;
use crate::domain::{
    Finding, FlowSinkFact, MissingDiscriminatorFact, RelatedTest, StageEvidence, ValueFact,
};

use super::{array_field, escape, field, float_field, number_field};

pub fn render(output: &CheckOutput) -> String {
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
    for (idx, finding) in output.findings.iter().enumerate() {
        finding_json(&mut out, finding, 2);
        if idx + 1 != output.findings.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]\n");
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

pub(super) fn finding_json(out: &mut String, finding: &Finding, indent: usize) {
    let sp = "  ".repeat(indent);
    out.push_str(&format!("{sp}{{\n"));
    field(out, indent + 1, "id", &finding.id, true);
    field(
        out,
        indent + 1,
        "classification",
        finding.class.as_str(),
        true,
    );
    field(out, indent + 1, "severity", finding.class.severity(), true);
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
    field(
        out,
        indent + 2,
        "expression",
        &finding.probe.expression,
        false,
    );
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
    field(
        out,
        indent + 1,
        "suggested_next_action",
        finding.recommended_next_step.as_deref().unwrap_or(""),
        false,
    );
    out.push_str(&format!("{sp}}}"));
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
