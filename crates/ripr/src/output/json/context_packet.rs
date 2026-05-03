use crate::domain::{Finding, MissingDiscriminatorFact, ValueFact};

use super::{array_field, escape, field, number_field};
use crate::output::json::report::{related_test_json, stop_reason_values};

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
    let related_test_count = finding.related_tests.len().min(max_related_tests);
    out.push_str("  \"related_tests\": [\n");
    for (idx, test) in finding
        .related_tests
        .iter()
        .take(max_related_tests)
        .enumerate()
    {
        related_test_json(&mut out, test, 2);
        if idx + 1 != related_test_count {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ],\n");
    value_array(
        &mut out,
        1,
        "observed_values",
        &finding.activation.observed_values,
    );
    out.push_str(",\n");
    discriminator_array(
        &mut out,
        1,
        "missing_discriminators",
        &finding.activation.missing_discriminators,
    );
    out.push_str(",\n");
    array_field(&mut out, 1, "missing", &finding.missing, true);
    let stop_reasons = stop_reason_values(finding);
    array_field(&mut out, 1, "stop_reasons", &stop_reasons, true);
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

fn value_array(out: &mut String, indent: usize, name: &str, values: &[ValueFact]) {
    out.push_str(&format!("{}\"{name}\": [", "  ".repeat(indent)));
    for (idx, value) in values.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&format!(
            "{{\"value\":\"{}\",\"context\":\"{}\",\"line\":{}}}",
            escape(&value.value),
            value.context.as_str(),
            value.line
        ));
    }
    out.push(']');
}

fn discriminator_array(
    out: &mut String,
    indent: usize,
    name: &str,
    values: &[MissingDiscriminatorFact],
) {
    out.push_str(&format!("{}\"{name}\": [", "  ".repeat(indent)));
    for (idx, value) in values.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&format!(
            "{{\"value\":\"{}\",\"reason\":\"{}\"}}",
            escape(&value.value),
            escape(&value.reason)
        ));
    }
    out.push(']');
}
