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
    field(
        out,
        indent + 1,
        "recommended_next_step",
        finding.recommended_next_step.as_deref().unwrap_or(""),
        false,
    );
    out.push_str(&format!("{sp}}}"));
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
    use super::escape;

    #[test]
    fn escapes_json() {
        assert_eq!(escape("a\"b\n"), "a\\\"b\\n");
    }
}
