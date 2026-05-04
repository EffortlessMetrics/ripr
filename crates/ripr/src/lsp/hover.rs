use super::HOVER_TEXT;
use crate::domain::{Finding, StageEvidence};
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
            value: finding_hover_markdown(diagnostic, finding),
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

fn finding_hover_markdown(diagnostic: &Diagnostic, finding: &Finding) -> String {
    let classification = diagnostic
        .code
        .as_ref()
        .map(number_or_string_label)
        .unwrap_or_else(|| "static exposure".to_string());
    let mut lines = vec![
        format!("**ripr** `{classification}`"),
        String::new(),
        diagnostic.message.clone(),
        String::new(),
        "## RIPR Evidence".to_string(),
        stage_line("reach", &finding.ripr.reach),
        stage_line("infection", &finding.ripr.infect),
        stage_line("propagation", &finding.ripr.propagate),
        stage_line("observation", &finding.ripr.reveal.observe),
        stage_line("discriminator", &finding.ripr.reveal.discriminate),
    ];

    if !finding.related_tests.is_empty() {
        lines.push(String::new());
        lines.push("## Related Tests".to_string());
        for test in &finding.related_tests {
            let oracle_text = match &test.oracle {
                Some(oracle) => format!(
                    " \u{2014} {} {} oracle: {}",
                    test.oracle_strength.as_str(),
                    test.oracle_kind.as_str(),
                    oracle
                ),
                None => String::new(),
            };
            lines.push(format!(
                "- `{}:{}` `{}`{}",
                test.file.display(),
                test.line,
                test.name,
                oracle_text
            ));
        }
    }

    if !finding.missing.is_empty() {
        lines.push(String::new());
        lines.push("## Weakness".to_string());
        for item in &finding.missing {
            lines.push(format!("- {item}"));
        }
    }

    lines.join("\n")
}

fn stage_line(name: &str, stage: &StageEvidence) -> String {
    format!("* {name} {}: {}", stage.state.as_str(), stage.summary)
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
