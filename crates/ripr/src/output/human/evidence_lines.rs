use crate::domain::Finding;

pub(super) fn evidence_path_lines(finding: &Finding) -> Vec<String> {
    let mut lines = vec![
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

    for sink in &finding.flow_sinks {
        lines.push(format!(
            "local flow reaches {}: {} (line {})",
            sink.kind.label(),
            sink.text,
            sink.line
        ));
    }

    for test in finding.related_tests.iter().take(5) {
        let oracle_kind = display_label(test.oracle_kind.as_str());
        let mut line = format!(
            "related test {}:{} {} uses {} {} oracle",
            test.file.display(),
            test.line,
            test.name,
            test.oracle_strength.as_str(),
            oracle_kind
        );
        if let Some(oracle) = &test.oracle {
            line.push_str(&format!(": {oracle}"));
        }
        lines.push(line);
    }

    for value in finding.activation.observed_values.iter().take(8) {
        let context = display_label(value.context.as_str());
        lines.push(format!(
            "observed {} value {} at line {}",
            context, value.value, value.line
        ));
    }

    if lines.len() == 5 && !finding.evidence.is_empty() {
        lines.extend(finding.evidence.iter().cloned());
    }

    lines
}

pub(super) fn weakness_lines(finding: &Finding) -> Vec<String> {
    let discriminator_values: Vec<&str> = finding
        .activation
        .missing_discriminators
        .iter()
        .map(|fact| fact.value.as_str())
        .collect();
    let mut lines = finding
        .missing
        .iter()
        .filter(|missing| !is_duplicate_discriminator_missing(missing, &discriminator_values))
        .cloned()
        .collect::<Vec<_>>();
    for discriminator in &finding.activation.missing_discriminators {
        lines.push(format!(
            "missing discriminator {}: {}",
            discriminator.value, discriminator.reason
        ));
    }
    lines
}

fn is_duplicate_discriminator_missing(missing: &str, discriminator_values: &[&str]) -> bool {
    let Some(value) = missing.strip_prefix("Missing discriminator value: ") else {
        return false;
    };
    discriminator_values.contains(&value)
}

fn display_label(value: &str) -> String {
    value.replace('_', " ")
}
