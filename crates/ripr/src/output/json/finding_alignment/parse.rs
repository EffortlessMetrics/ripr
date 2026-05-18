use crate::domain::Finding;

use super::model::*;

pub(super) fn raw_finding_for(finding: &Finding) -> FindingAlignmentRawFinding {
    FindingAlignmentRawFinding {
        file: finding.probe.location.file.display().to_string(),
        line: finding.probe.location.line,
        kind: finding.class.as_str().to_string(),
        expression: finding.probe.expression.clone(),
        probe_kind: finding.probe.family.as_str().to_string(),
        source_id: finding.probe.id.0.clone(),
        evidence_record_ref: finding.id.clone(),
    }
}

pub(super) fn adjacent_literal_index(
    findings: &[Finding],
    used: &[bool],
    declaration_index: usize,
) -> Option<usize> {
    let declaration = &findings[declaration_index];
    findings
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != declaration_index && !used[*index])
        .filter(|(_, candidate)| candidate.probe.location.file == declaration.probe.location.file)
        .filter(|(_, candidate)| {
            candidate.probe.location.line == declaration.probe.location.line
                || candidate.probe.location.line == declaration.probe.location.line + 1
        })
        .find_map(|(index, candidate)| {
            parse_string_literal(&candidate.probe.expression).map(|_| index)
        })
}

pub(super) fn parse_config_policy_declaration(
    expression: &str,
) -> Option<PresentationTextDeclaration> {
    let trimmed = expression.trim();
    let const_pos = trimmed.find("const ")?;
    if const_pos > 0
        && trimmed[..const_pos]
            .chars()
            .last()
            .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
    {
        return None;
    }

    let after_const = &trimmed[const_pos + "const ".len()..];
    let name_end = after_const
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .unwrap_or(after_const.len());
    let constant_name = &after_const[..name_end];
    if constant_name.is_empty() || !is_config_policy_constant_name(constant_name) {
        return None;
    }

    let after_name = after_const[name_end..].trim_start();
    let after_colon = after_name.strip_prefix(':')?.trim_start();
    let equals_pos = after_colon.find('=')?;
    let after_equals = after_colon[equals_pos + 1..].trim_start();
    Some(PresentationTextDeclaration {
        constant_name: constant_name.to_string(),
        inline_literal: parse_string_literal(after_equals),
    })
}

pub(super) fn parse_presentation_text_declaration(
    expression: &str,
) -> Option<PresentationTextDeclaration> {
    let trimmed = expression.trim();
    let const_pos = trimmed.find("const ")?;
    if const_pos > 0
        && trimmed[..const_pos]
            .chars()
            .last()
            .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
    {
        return None;
    }

    let after_const = &trimmed[const_pos + "const ".len()..];
    let name_end = after_const
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .unwrap_or(after_const.len());
    let constant_name = &after_const[..name_end];
    if constant_name.is_empty() || !is_presentation_text_constant_name(constant_name) {
        return None;
    }

    let after_name = after_const[name_end..].trim_start();
    let after_colon = after_name.strip_prefix(':')?.trim_start();
    let equals_pos = after_colon.find('=')?;
    let ty = after_colon[..equals_pos].trim();
    if !matches!(ty, "&str" | "&'static str") {
        return None;
    }

    let after_equals = after_colon[equals_pos + 1..].trim_start();
    Some(PresentationTextDeclaration {
        constant_name: constant_name.to_string(),
        inline_literal: parse_string_literal(after_equals),
    })
}

fn is_config_policy_constant_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    [
        "CONFIG",
        "SETTING",
        "SETTINGS",
        "POLICY",
        "ALLOWLIST",
        "DENYLIST",
        "SCHEMA",
        "FIELD",
        "THRESHOLD",
        "SELECTOR",
        "VALIDATION",
        "ROUTING",
        "ROUTE",
        "OPAQUE",
    ]
    .iter()
    .any(|marker| upper.split('_').any(|part| part == *marker))
}

fn is_presentation_text_constant_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    [
        "TEXT",
        "LABEL",
        "LABELS",
        "HELP",
        "TITLE",
        "MESSAGE",
        "DESCRIPTION",
        "REPORT",
        "DISPLAY",
        "HEADER",
        "FOOTER",
    ]
    .iter()
    .any(|marker| upper.split('_').any(|part| part == *marker))
}

pub(super) fn parse_string_literal(expression: &str) -> Option<String> {
    let start = expression.find('"')?;
    let mut value = String::new();
    let mut escaped = false;

    for ch in expression[start + 1..].chars() {
        if escaped {
            match ch {
                'n' => value.push('\n'),
                'r' => value.push('\r'),
                't' => value.push('\t'),
                '"' => value.push('"'),
                '\\' => value.push('\\'),
                other => {
                    value.push('\\');
                    value.push(other);
                }
            }
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '"' => return Some(value),
            other => value.push(other),
        }
    }

    None
}
