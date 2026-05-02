use crate::app::CheckOutput;
use crate::domain::{Finding, RelatedTest, StageEvidence};

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

pub fn render_context_packet(finding: &Finding, max_related_tests: usize) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    field(&mut out, 1, "version", "1.0", true);
    field(&mut out, 1, "tool", "ripr", true);
    out.push_str("  \"probe\": {\n");
    field(&mut out, 2, "id", &finding.probe.id.0, true);
    field(&mut out, 2, "family", finding.probe.family.as_str(), true);
    field(&mut out, 2, "delta", finding.probe.delta.as_str(), true);
    field(
        &mut out,
        2,
        "file",
        &finding.probe.location.file.display().to_string(),
        true,
    );
    number_field(&mut out, 2, "line", finding.probe.location.line, true);
    field(
        &mut out,
        2,
        "changed_expression",
        &finding.probe.expression,
        false,
    );
    out.push_str("  },\n");
    out.push_str("  \"ripr\": {\n");
    field(
        &mut out,
        2,
        "reach",
        finding.ripr.reach.state.as_str(),
        true,
    );
    field(
        &mut out,
        2,
        "infect",
        finding.ripr.infect.state.as_str(),
        true,
    );
    field(
        &mut out,
        2,
        "propagate",
        finding.ripr.propagate.state.as_str(),
        true,
    );
    field(
        &mut out,
        2,
        "observe",
        finding.ripr.reveal.observe.state.as_str(),
        true,
    );
    field(
        &mut out,
        2,
        "discriminate",
        finding.ripr.reveal.discriminate.state.as_str(),
        false,
    );
    out.push_str("  },\n");
    out.push_str("  \"related_tests\": [\n");
    for (idx, test) in finding
        .related_tests
        .iter()
        .take(max_related_tests)
        .enumerate()
    {
        related_test_json(&mut out, test, 2);
        if idx + 1 != finding.related_tests.iter().take(max_related_tests).count() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ],\n");
    out.push_str("  \"missing\": [");
    for (idx, missing) in finding.missing.iter().enumerate() {
        out.push_str(&format!("\"{}\"", escape(missing)));
        if idx + 1 != finding.missing.len() {
            out.push_str(", ");
        }
    }
    out.push_str("],\n");
    let stop_reasons = stop_reason_values(finding);
    out.push_str("  \"stop_reasons\": [");
    for (idx, reason) in stop_reasons.iter().enumerate() {
        out.push_str(&format!("\"{}\"", escape(reason)));
        if idx + 1 != stop_reasons.len() {
            out.push_str(", ");
        }
    }
    out.push_str("],\n");
    field(
        &mut out,
        1,
        "recommended_next_step",
        finding.recommended_next_step.as_deref().unwrap_or(""),
        false,
    );
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

fn finding_json(out: &mut String, finding: &Finding, indent: usize) {
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
    array_field(out, indent + 1, "evidence", &finding.evidence, true);
    array_field(out, indent + 1, "missing", &finding.missing, true);
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
    field(
        out,
        indent + 1,
        "recommended_next_step",
        finding.recommended_next_step.as_deref().unwrap_or(""),
        false,
    );
    out.push_str(&format!("{sp}}}"));
}

fn stop_reason_values(finding: &Finding) -> Vec<String> {
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

fn related_test_json(out: &mut String, test: &RelatedTest, indent: usize) {
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
        "oracle",
        test.oracle.as_deref().unwrap_or(""),
        false,
    );
    out.push_str(&format!("{sp}}}"));
}

fn field(out: &mut String, indent: usize, name: &str, value: &str, trailing: bool) {
    out.push_str(&format!(
        "{}\"{}\": \"{}\"{}\n",
        "  ".repeat(indent),
        name,
        escape(value),
        if trailing { "," } else { "" }
    ));
}

fn number_field(out: &mut String, indent: usize, name: &str, value: usize, trailing: bool) {
    out.push_str(&format!(
        "{}\"{}\": {}{}\n",
        "  ".repeat(indent),
        name,
        value,
        if trailing { "," } else { "" }
    ));
}

fn float_field(out: &mut String, indent: usize, name: &str, value: f32, trailing: bool) {
    out.push_str(&format!(
        "{}\"{}\": {:.2}{}\n",
        "  ".repeat(indent),
        name,
        value,
        if trailing { "," } else { "" }
    ));
}

fn array_field(out: &mut String, indent: usize, name: &str, values: &[String], trailing: bool) {
    out.push_str(&format!("{}\"{}\": [", "  ".repeat(indent), name));
    for (idx, value) in values.iter().enumerate() {
        out.push_str(&format!("\"{}\"", escape(value)));
        if idx + 1 != values.len() {
            out.push_str(", ");
        }
    }
    out.push_str(&format!("]{}\n", if trailing { "," } else { "" }));
}

fn escape(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{escape, finding_json, render_context_packet};
    use crate::domain::{
        Confidence, DeltaKind, ExposureClass, Finding, Probe, ProbeFamily, ProbeId, RevealEvidence,
        RiprEvidence, SourceLocation, StageEvidence, StageState,
    };

    #[test]
    fn escapes_json() {
        assert_eq!(escape("a\"b\n"), "a\\\"b\\n");
    }

    #[test]
    fn finding_json_includes_effective_stop_reasons_for_unknowns() {
        let finding = unknown_finding();
        let mut out = String::new();

        finding_json(&mut out, &finding, 0);

        assert!(out.contains("\"stop_reasons\": [\"static_probe_unknown\"]"));
    }

    #[test]
    fn context_packet_includes_effective_stop_reasons_for_unknowns() {
        let finding = unknown_finding();
        let packet = render_context_packet(&finding, 5);

        assert!(packet.contains("\"stop_reasons\": [\"static_probe_unknown\"]"));
    }

    fn unknown_finding() -> Finding {
        Finding {
            id: "probe:src_lib_rs:1:static_unknown".to_string(),
            probe: Probe {
                id: ProbeId("probe:src_lib_rs:1:static_unknown".to_string()),
                location: SourceLocation::new("src/lib.rs", 1, 1),
                owner: None,
                family: ProbeFamily::StaticUnknown,
                delta: DeltaKind::Unknown,
                before: None,
                after: None,
                expression: "unknown syntax".to_string(),
                expected_sinks: vec![],
                required_oracles: vec![],
            },
            class: ExposureClass::StaticUnknown,
            ripr: RiprEvidence {
                reach: stage("No stable syntax owner"),
                infect: stage("Changed syntax is not mapped to a probe"),
                propagate: stage("No propagation model is available"),
                reveal: RevealEvidence {
                    observe: stage("No observation model is available"),
                    discriminate: stage("No discriminator model is available"),
                },
            },
            confidence: 0.2,
            evidence: vec![],
            missing: vec![],
            stop_reasons: vec![],
            related_tests: vec![],
            recommended_next_step: Some("Escalate to real mutation testing.".to_string()),
        }
    }

    fn stage(summary: &str) -> StageEvidence {
        StageEvidence::new(StageState::Unknown, Confidence::Low, summary)
    }
}
