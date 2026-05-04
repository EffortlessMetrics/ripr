use super::HOVER_TEXT;
use crate::domain::{Finding, OracleStrength};
use tower_lsp_server::ls_types::{
    Diagnostic, Hover, HoverContents, MarkupContent, MarkupKind, NumberOrString, Position, Range,
};

pub(super) fn hover_response() -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: HOVER_TEXT.to_string(),
        }),
        range: None,
    }
}

pub(super) fn diagnostic_hover_response(diagnostic: &Diagnostic) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: diagnostic_hover_markdown(diagnostic),
        }),
        range: Some(diagnostic.range),
    }
}

pub(super) fn finding_hover_response(finding: &Finding, diagnostic: &Diagnostic) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: finding_hover_markdown(finding, diagnostic),
        }),
        range: Some(diagnostic.range),
    }
}

pub(super) fn diagnostic_at_position<'a>(
    diagnostics: &'a [Diagnostic],
    position: &Position,
) -> Option<&'a Diagnostic> {
    diagnostics
        .iter()
        .find(|diagnostic| position_in_range(position, &diagnostic.range))
}

fn diagnostic_hover_markdown(diagnostic: &Diagnostic) -> String {
    let classification = diagnostic
        .code
        .as_ref()
        .map(number_or_string_label)
        .unwrap_or_else(|| "static exposure".to_string());
    let mut lines = vec![
        format!("**ripr** `{classification}`"),
        String::new(),
        diagnostic.message.clone(),
    ];
    if let Some(data) = &diagnostic.data {
        if let Some(finding_id) = data.get("finding_id").and_then(|value| value.as_str()) {
            lines.push(String::new());
            lines.push(format!("Finding: `{finding_id}`"));
        }
        if let Some(probe_id) = data.get("probe_id").and_then(|value| value.as_str()) {
            lines.push(format!("Probe: `{probe_id}`"));
        }
    }
    lines.join("\n")
}

fn finding_hover_markdown(finding: &Finding, _diagnostic: &Diagnostic) -> String {
    let mut lines = vec![
        format!("**ripr** `{}`", finding.class.as_str()),
        String::new(),
        format!(
            "`{}` probe · confidence `{:.2}`",
            finding.probe.family.as_str(),
            finding.confidence
        ),
    ];

    lines.push(String::new());
    lines.push("## Changed".to_string());
    lines.push(String::new());
    lines.push("```rust".to_string());
    if let Some(before) = &finding.probe.before {
        if let Some(after) = &finding.probe.after {
            lines.push(format!("- {before}"));
            lines.push(format!("+ {after}"));
        } else {
            lines.push(format!("- {before}"));
            lines.push(format!("+ {}", finding.probe.expression));
        }
    } else {
        lines.push(finding.probe.expression.clone());
    }
    lines.push("```".to_string());

    lines.push(String::new());
    lines.push("## Evidence".to_string());
    lines.push(String::new());
    lines.push(format!(
        "* reach {}: {}",
        stage_emoji(&finding.ripr.reach.state),
        stage_text(&finding.ripr.reach.state)
    ));
    lines.push(format!(
        "* infection {}: {}",
        stage_emoji(&finding.ripr.infect.state),
        stage_text(&finding.ripr.infect.state)
    ));
    lines.push(format!(
        "* propagation {}: {}",
        stage_emoji(&finding.ripr.propagate.state),
        stage_text(&finding.ripr.propagate.state)
    ));
    lines.push(format!(
        "* observation {}: {}",
        stage_emoji(&finding.ripr.reveal.observe.state),
        stage_text(&finding.ripr.reveal.observe.state)
    ));
    lines.push(format!(
        "* discriminator {}: {}",
        stage_emoji(&finding.ripr.reveal.discriminate.state),
        stage_text(&finding.ripr.reveal.discriminate.state)
    ));

    if !finding.flow_sinks.is_empty() {
        lines.push(String::new());
        lines.push("## Local flow".to_string());
        lines.push(String::new());
        for sink in finding.flow_sinks.iter().take(8) {
            lines.push(format!(
                "* {}: `{}` at line {}",
                sink.kind.label(),
                escape_backticks(&sink.text),
                sink.line
            ));
        }
    }

    if !finding.related_tests.is_empty() {
        lines.push(String::new());
        lines.push("## Related tests".to_string());
        lines.push(String::new());
        for test in finding.related_tests.iter().take(5) {
            let oracle_label = oracle_strength_label(&test.oracle_strength);
            lines.push(format!(
                "* `{}:{}` `{}` — {} {}",
                test.file.display(),
                test.line,
                escape_backticks(&test.name),
                oracle_label,
                test.oracle_kind.as_str()
            ));
        }
    }

    if !finding.activation.observed_values.is_empty() {
        lines.push(String::new());
        lines.push("## Observed values".to_string());
        lines.push(String::new());
        for value in finding.activation.observed_values.iter().take(8) {
            lines.push(format!(
                "* {}: `{}` at line {}",
                value.context.as_str(),
                escape_backticks(&value.value),
                value.line
            ));
        }
    }

    if !finding.activation.missing_discriminators.is_empty() {
        lines.push(String::new());
        lines.push("## Missing discriminator".to_string());
        lines.push(String::new());
        for discriminator in &finding.activation.missing_discriminators {
            lines.push(format!(
                "* `{}`: {}",
                escape_backticks(&discriminator.value),
                &discriminator.reason
            ));
        }
    }

    let effective_stops = finding.effective_stop_reasons();
    if !effective_stops.is_empty() {
        lines.push(String::new());
        lines.push("## Stop reasons".to_string());
        lines.push(String::new());
        for reason in effective_stops {
            lines.push(format!("* `{}`", reason.as_str()));
        }
    }

    if let Some(next_step) = &finding.recommended_next_step {
        lines.push(String::new());
        lines.push("## Next step".to_string());
        lines.push(String::new());
        lines.push(next_step.clone());
    }

    lines.push(String::new());
    lines.push(format!("Finding: `{}`", finding.id));
    lines.push(format!("Probe: `{}`", finding.probe.id));

    lines.join("\n")
}

fn escape_backticks(text: &str) -> String {
    text.replace('`', "'")
}

fn stage_emoji(state: &crate::domain::StageState) -> &'static str {
    match state {
        crate::domain::StageState::Yes => "yes",
        crate::domain::StageState::No => "no",
        crate::domain::StageState::Weak => "weak",
        crate::domain::StageState::Unknown => "unknown",
        crate::domain::StageState::Opaque => "opaque",
        crate::domain::StageState::NotApplicable => "n/a",
    }
}

fn stage_text(state: &crate::domain::StageState) -> &'static str {
    match state {
        crate::domain::StageState::Yes => "evidence found",
        crate::domain::StageState::No => "no evidence",
        crate::domain::StageState::Weak => "weak evidence",
        crate::domain::StageState::Unknown => "evidence unknown",
        crate::domain::StageState::Opaque => "opaque",
        crate::domain::StageState::NotApplicable => "not applicable",
    }
}

fn oracle_strength_label(strength: &OracleStrength) -> &'static str {
    match strength {
        OracleStrength::Strong => "strong",
        OracleStrength::Medium => "medium",
        OracleStrength::Weak => "weak",
        OracleStrength::Smoke => "smoke",
        OracleStrength::None => "none",
        OracleStrength::Unknown => "unknown",
    }
}

fn number_or_string_label(value: &NumberOrString) -> String {
    match value {
        NumberOrString::Number(number) => number.to_string(),
        NumberOrString::String(text) => text.clone(),
    }
}

fn position_in_range(position: &Position, range: &Range) -> bool {
    position_is_after_or_equal(position, &range.start) && position_is_before(position, &range.end)
}

fn position_is_after_or_equal(position: &Position, start: &Position) -> bool {
    position.line > start.line
        || (position.line == start.line && position.character >= start.character)
}

fn position_is_before(position: &Position, end: &Position) -> bool {
    position.line < end.line || (position.line == end.line && position.character < end.character)
}
