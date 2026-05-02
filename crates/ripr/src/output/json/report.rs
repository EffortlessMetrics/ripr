use crate::app::CheckOutput;
use crate::domain::{Finding, RelatedTest, StageEvidence};

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
        "oracle",
        test.oracle.as_deref().unwrap_or(""),
        false,
    );
    out.push_str(&format!("{sp}}}"));
}
