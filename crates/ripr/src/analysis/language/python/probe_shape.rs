use super::source_utils::normalized_path;
use super::static_limits::PythonStaticLimit;
use super::{
    PythonOwner, is_python_control_predicate_line, python_assignment_constructor_field_parts,
    python_exit_code_discriminator, python_return_constructor_field_discriminator,
    python_return_dict_field_discriminator,
};
use crate::domain::{
    Confidence, DeltaKind, FindingCanonicalGap, FlowSinkFact, FlowSinkKind, ProbeFamily,
    StageEvidence, StageState,
};
use std::path::Path;

pub(super) fn classify_probe_shape(line_text: &str) -> (ProbeFamily, DeltaKind) {
    let trimmed = line_text.trim_start();
    if is_python_cli_exit_line(trimmed) {
        return (ProbeFamily::SideEffect, DeltaKind::Effect);
    }
    if (trimmed.contains(" if ") && trimmed.contains(" else "))
        || trimmed.starts_with("if ")
        || trimmed.starts_with("elif ")
        || trimmed.starts_with("while ")
        || trimmed.starts_with("for ")
        || trimmed.starts_with("match ")
        || trimmed.starts_with("case ")
    {
        return (ProbeFamily::Predicate, DeltaKind::Control);
    }
    if trimmed.starts_with("raise ")
        || trimmed == "raise"
        || trimmed.starts_with("try:")
        || trimmed.starts_with("except ")
        || trimmed.starts_with("except* ")
        || trimmed.starts_with("finally:")
        || (trimmed.starts_with("with ") && trimmed.contains("raises("))
    {
        return (ProbeFamily::ErrorPath, DeltaKind::Control);
    }
    if python_return_dict_field_discriminator(trimmed).is_some()
        || python_return_constructor_field_discriminator(trimmed).is_some()
    {
        return (ProbeFamily::FieldConstruction, DeltaKind::Value);
    }
    if trimmed.starts_with("return ") || trimmed == "return" {
        return (ProbeFamily::ReturnValue, DeltaKind::Value);
    }
    if contains_mock_initializer(trimmed) {
        return (ProbeFamily::SideEffect, DeltaKind::Effect);
    }
    if let Some(eq_idx) = trimmed.find('=')
        && !trimmed.contains("==")
        && !trimmed.contains("!=")
        && !trimmed.contains(">=")
        && !trimmed.contains("<=")
    {
        let lhs = trimmed[..eq_idx].trim();
        if lhs.contains('.')
            && lhs.chars().all(|ch| {
                ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '[' || ch == ']'
            })
        {
            return (ProbeFamily::FieldConstruction, DeltaKind::Value);
        }
        if python_assignment_constructor_field_parts(trimmed).is_some() {
            return (ProbeFamily::FieldConstruction, DeltaKind::Value);
        }
        let rhs = trimmed[eq_idx + 1..].trim();
        if looks_like_call_expression(rhs) {
            return (ProbeFamily::SideEffect, DeltaKind::Effect);
        }
    }
    let call_candidate = trimmed.strip_prefix("await ").unwrap_or(trimmed).trim_end();
    if looks_like_call_expression(call_candidate)
        && !call_candidate.starts_with("assert ")
        && !call_candidate.starts_with("def ")
        && !call_candidate.starts_with("class ")
        && !call_candidate.starts_with("with ")
    {
        return (ProbeFamily::SideEffect, DeltaKind::Effect);
    }
    (ProbeFamily::Predicate, DeltaKind::Control)
}

pub(super) fn contains_mock_initializer(text: &str) -> bool {
    text.contains("Mock(") || text.contains("MagicMock(")
}

fn is_python_cli_exit_line(text: &str) -> bool {
    python_exit_code_discriminator(text).is_some()
}

pub(super) fn looks_like_call_expression(text: &str) -> bool {
    let text = text.trim_end_matches(';').trim_end();
    text.contains('(') && text.ends_with(')')
}

pub(super) fn canonical_python_gap_for(
    file: &Path,
    owner: &PythonOwner,
    probe_family: &ProbeFamily,
    line_text: &str,
) -> FindingCanonicalGap {
    let file = normalized_path(file);
    let behavior_kind = python_behavior_kind(probe_family).to_string();
    let probe_kind = probe_family.as_str().to_string();
    let normalized_discriminator = normalize_python_gap_discriminator(probe_family, line_text);
    let id = format!(
        "gap:python:{file}:{}:{behavior_kind}:{probe_kind}:{normalized_discriminator}",
        owner.qualified_name
    );

    FindingCanonicalGap {
        id,
        language: "python".to_string(),
        file,
        owner: owner.qualified_name.clone(),
        behavior_kind,
        probe_kind,
        normalized_discriminator,
    }
}

fn python_behavior_kind(probe_family: &ProbeFamily) -> &'static str {
    match probe_family {
        ProbeFamily::Predicate => "predicate_boundary",
        ProbeFamily::ReturnValue => "return_value",
        ProbeFamily::ErrorPath => "exception_path",
        ProbeFamily::FieldConstruction => "field_value",
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => "call_or_output_effect",
        ProbeFamily::MatchArm => "match_arm",
        ProbeFamily::StaticUnknown => "static_unknown",
    }
}

fn normalize_python_gap_discriminator(probe_family: &ProbeFamily, line_text: &str) -> String {
    let mut text = line_text.trim().trim_end_matches(';').trim().to_string();
    match probe_family {
        ProbeFamily::Predicate => {
            for prefix in ["if ", "elif ", "while ", "for ", "match ", "case "] {
                if let Some(stripped) = text.strip_prefix(prefix) {
                    text = stripped.to_string();
                    break;
                }
            }
            text = text.trim_end_matches(':').trim().to_string();
        }
        ProbeFamily::ReturnValue => {
            if let Some(stripped) = text.strip_prefix("return ") {
                text = stripped.to_string();
            }
        }
        ProbeFamily::ErrorPath => {
            if let Some(stripped) = text.strip_prefix("raise ") {
                text = stripped.to_string();
            }
            text = text.trim_end_matches(':').trim().to_string();
        }
        _ => {}
    }
    normalize_gap_key_text(&text)
}

fn normalize_gap_key_text(text: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_word = false;
    let mut pending_separator = false;

    for character in text.chars() {
        if character.is_ascii_alphanumeric() || character == '_' || character == '.' {
            if pending_separator && previous_was_word {
                normalized.push('_');
            }
            normalized.push(character.to_ascii_lowercase());
            previous_was_word = true;
            pending_separator = false;
        } else if matches!(
            character,
            '=' | '!' | '<' | '>' | '+' | '-' | '*' | '/' | '%' | '[' | ']'
        ) {
            normalized.push(character);
            previous_was_word = false;
            pending_separator = false;
        } else {
            pending_separator = true;
        }
    }

    let trimmed = normalized.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed
    }
}

pub(super) fn python_infection_evidence(
    probe_family: &ProbeFamily,
    line_text: &str,
) -> StageEvidence {
    let summary = match probe_family {
        ProbeFamily::Predicate => {
            if is_python_control_predicate_line(line_text) {
                format!(
                    "Changed Python predicate can alter branch selection: `{}`",
                    line_text.trim()
                )
            } else {
                format!(
                    "Changed Python expression can alter preview-classified predicate behavior: `{}`",
                    line_text.trim()
                )
            }
        }
        ProbeFamily::ReturnValue => {
            format!(
                "Changed Python return expression can alter the owner return value: `{}`",
                line_text.trim()
            )
        }
        ProbeFamily::ErrorPath => {
            format!(
                "Changed Python error path can alter raised exception/control behavior: `{}`",
                line_text.trim()
            )
        }
        ProbeFamily::FieldConstruction => {
            format!(
                "Changed Python field or attribute construction can alter object state: `{}`",
                line_text.trim()
            )
        }
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => {
            format!(
                "Changed Python call or output effect can alter observable side effects: `{}`",
                line_text.trim()
            )
        }
        ProbeFamily::MatchArm => {
            format!(
                "Changed Python match arm can alter selected branch behavior: `{}`",
                line_text.trim()
            )
        }
        ProbeFamily::StaticUnknown => {
            "Python preview could not classify the changed behavior shape.".to_string()
        }
    };
    StageEvidence::new(StageState::Yes, Confidence::Low, summary)
}

pub(super) fn python_propagation_evidence(
    probe_family: &ProbeFamily,
    line_text: &str,
    static_limit: Option<&PythonStaticLimit>,
) -> StageEvidence {
    if let Some(limit) = static_limit {
        return StageEvidence::new(
            StageState::Unknown,
            Confidence::Low,
            format!(
                "Static limit `{}` prevents a safe Python propagation claim.",
                limit.kind.as_str()
            ),
        );
    }

    match probe_family {
        ProbeFamily::ReturnValue => StageEvidence::new(
            StageState::Yes,
            Confidence::Low,
            "Changed Python return value is already at the owner output boundary.",
        ),
        ProbeFamily::ErrorPath => StageEvidence::new(
            StageState::Yes,
            Confidence::Low,
            "Changed Python error path propagates through the exception/control boundary.",
        ),
        ProbeFamily::FieldConstruction => StageEvidence::new(
            StageState::Weak,
            Confidence::Low,
            "Changed Python field construction can propagate through returned or retained object state; exact runtime object flow is not resolved.",
        ),
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => StageEvidence::new(
            StageState::Weak,
            Confidence::Low,
            "Changed Python call/output behavior can propagate through side effects; runtime target resolution is not inferred.",
        ),
        ProbeFamily::Predicate | ProbeFamily::MatchArm => {
            let summary = if matches!(probe_family, ProbeFamily::Predicate)
                && !is_python_control_predicate_line(line_text)
            {
                "Changed Python fallback expression can propagate through selected behavior; preview evidence does not prove the concrete downstream sink."
            } else if matches!(probe_family, ProbeFamily::Predicate) {
                "Changed Python control flow can propagate by selecting a different branch; preview evidence does not prove the concrete downstream sink."
            } else {
                "Changed Python match arm can propagate by selecting a different branch; preview evidence does not prove the concrete downstream sink."
            };
            StageEvidence::new(StageState::Weak, Confidence::Low, summary)
        }
        ProbeFamily::StaticUnknown => StageEvidence::new(
            StageState::Unknown,
            Confidence::Low,
            "Python preview could not classify a propagation path for this changed behavior.",
        ),
    }
}

pub(super) fn python_flow_sink_for(
    probe_family: &ProbeFamily,
    owner: &PythonOwner,
    line: usize,
    line_text: &str,
) -> Option<FlowSinkFact> {
    let kind = match probe_family {
        ProbeFamily::ReturnValue => FlowSinkKind::ReturnValue,
        ProbeFamily::ErrorPath => FlowSinkKind::ErrorVariant,
        ProbeFamily::FieldConstruction => FlowSinkKind::StructField,
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => FlowSinkKind::CallEffect,
        ProbeFamily::Predicate | ProbeFamily::MatchArm => FlowSinkKind::Unknown,
        ProbeFamily::StaticUnknown => return None,
    };

    Some(FlowSinkFact {
        kind,
        text: line_text.trim().to_string(),
        line,
        owner: Some(owner.symbol_id()),
    })
}
