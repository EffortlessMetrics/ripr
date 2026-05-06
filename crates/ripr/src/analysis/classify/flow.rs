use super::super::rust_index::FunctionSummary;
use super::text::exact_error_variant;
use crate::domain::*;

pub(in crate::analysis) fn propagation_evidence(
    probe: &Probe,
    flow_sinks: &[FlowSinkFact],
) -> StageEvidence {
    if matches!(probe.family, ProbeFamily::StaticUnknown) {
        return StageEvidence::new(
            StageState::Unknown,
            Confidence::Low,
            "No propagation model is available for this changed syntax",
        );
    }

    if let Some(sink) = flow_sinks
        .iter()
        .find(|sink| sink.kind != FlowSinkKind::Unknown)
    {
        StageEvidence::new(
            StageState::Yes,
            Confidence::Medium,
            format!(
                "Changed behavior appears to influence {}: {}",
                sink.kind.label(),
                sink.text
            ),
        )
    } else {
        StageEvidence::new(
            StageState::Unknown,
            Confidence::Low,
            "Propagation is not statically obvious from syntax-first analysis",
        )
    }
}

pub(in crate::analysis) fn local_flow_sinks(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
) -> Vec<FlowSinkFact> {
    let owner = owner_fn.map(|function| function.id.clone());
    let mut sinks = match probe.family {
        ProbeFamily::StaticUnknown => vec![flow_sink(
            FlowSinkKind::Unknown,
            "unknown sink",
            probe.location.line,
            owner.clone(),
        )],
        ProbeFamily::ErrorPath => vec![flow_sink(
            FlowSinkKind::ErrorVariant,
            result_error_text(&probe.expression),
            probe.location.line,
            owner.clone(),
        )],
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => {
            if probe.expression.contains("Err(") {
                vec![flow_sink(
                    FlowSinkKind::ErrorVariant,
                    result_error_text(&probe.expression),
                    probe.location.line,
                    owner.clone(),
                )]
            } else if probe.expression.starts_with("return ")
                || probe.expression.contains("Ok(")
                || probe.expression.contains("Some(")
            {
                vec![flow_sink(
                    FlowSinkKind::ReturnValue,
                    return_sink_text(&probe.expression),
                    probe.location.line,
                    owner.clone(),
                )]
            } else {
                vec![flow_sink(
                    FlowSinkKind::CallEffect,
                    call_effect_text(&probe.expression),
                    probe.location.line,
                    owner.clone(),
                )]
            }
        }
        ProbeFamily::FieldConstruction => vec![flow_sink(
            FlowSinkKind::StructField,
            field_sink_text(&probe.expression),
            probe.location.line,
            owner.clone(),
        )],
        ProbeFamily::MatchArm => vec![match_arm_sink(probe, owner.clone())],
        ProbeFamily::ReturnValue => vec![return_value_sink(probe, owner_fn, owner.clone())],
        ProbeFamily::Predicate => predicate_flow_sinks(probe, owner_fn, owner.clone()),
    };

    sinks.sort_by(|a, b| {
        a.kind
            .as_str()
            .cmp(b.kind.as_str())
            .then(a.line.cmp(&b.line))
            .then(a.text.cmp(&b.text))
    });
    sinks.dedup_by(|a, b| a.kind == b.kind && a.line == b.line && a.text == b.text);
    sinks
}

fn predicate_flow_sinks(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    owner: Option<SymbolId>,
) -> Vec<FlowSinkFact> {
    if let Some(error) = first_error_return(owner_fn, probe.location.line) {
        return vec![flow_sink(
            FlowSinkKind::ErrorVariant,
            result_error_text(&error.text),
            error.line,
            owner,
        )];
    }
    if let Some(return_fact) = nearest_return(owner_fn, probe.location.line) {
        return vec![flow_sink(
            FlowSinkKind::ReturnValue,
            return_sink_text(&return_fact.text),
            return_fact.line,
            owner,
        )];
    }
    if let Some(field) = first_field_construction(owner_fn, probe.location.line) {
        return vec![flow_sink(
            FlowSinkKind::StructField,
            field_sink_text(&field.text),
            field.line,
            owner,
        )];
    }
    if let Some(branch) = next_branch_value(owner_fn, probe.location.line) {
        return vec![flow_sink(
            FlowSinkKind::ReturnValue,
            branch.text,
            branch.line,
            owner,
        )];
    }
    Vec::new()
}

fn return_value_sink(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    owner: Option<SymbolId>,
) -> FlowSinkFact {
    if probe.expression.contains("Err(") {
        return flow_sink(
            FlowSinkKind::ErrorVariant,
            result_error_text(&probe.expression),
            probe.location.line,
            owner,
        );
    }
    if let Some(return_fact) = nearest_return(owner_fn, probe.location.line) {
        return flow_sink(
            FlowSinkKind::ReturnValue,
            return_sink_text(&return_fact.text),
            return_fact.line,
            owner,
        );
    }
    if !is_obvious_return_expression(&probe.expression) {
        return flow_sink(
            FlowSinkKind::Unknown,
            "unknown sink",
            probe.location.line,
            owner,
        );
    }
    flow_sink(
        FlowSinkKind::ReturnValue,
        return_sink_text(&probe.expression),
        probe.location.line,
        owner,
    )
}

fn match_arm_sink(probe: &Probe, owner: Option<SymbolId>) -> FlowSinkFact {
    let arm_result = probe
        .expression
        .split_once("=>")
        .map(|(_, result)| result.trim().trim_end_matches(',').to_string())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| probe.expression.clone());

    if arm_result.contains("Err(") {
        flow_sink(
            FlowSinkKind::ErrorVariant,
            result_error_text(&arm_result),
            probe.location.line,
            owner,
        )
    } else {
        flow_sink(
            FlowSinkKind::MatchArm,
            arm_result,
            probe.location.line,
            owner,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LocalTextFact {
    line: usize,
    text: String,
}

fn first_error_return(
    owner_fn: Option<&FunctionSummary>,
    probe_line: usize,
) -> Option<LocalTextFact> {
    owner_fn.and_then(|function| {
        function
            .returns
            .iter()
            .find(|return_fact| return_fact.line >= probe_line && return_fact.text.contains("Err("))
            .map(|return_fact| LocalTextFact {
                line: return_fact.line,
                text: return_fact.text.clone(),
            })
    })
}

fn nearest_return(owner_fn: Option<&FunctionSummary>, probe_line: usize) -> Option<LocalTextFact> {
    owner_fn.and_then(|function| {
        function
            .returns
            .iter()
            .filter(|return_fact| return_fact.line >= probe_line)
            .min_by_key(|return_fact| return_fact.line - probe_line)
            .map(|return_fact| LocalTextFact {
                line: return_fact.line,
                text: return_fact.text.clone(),
            })
    })
}

fn next_branch_value(
    owner_fn: Option<&FunctionSummary>,
    probe_line: usize,
) -> Option<LocalTextFact> {
    let function = owner_fn?;
    let start_index = probe_line.saturating_sub(function.start_line);
    function
        .body
        .lines()
        .enumerate()
        .skip(start_index + 1)
        .find_map(|(offset, line)| {
            let text = line.trim().trim_end_matches(',').to_string();
            if !looks_like_branch_tail_expression(&text) {
                return None;
            }
            Some(LocalTextFact {
                line: function.start_line + offset,
                text,
            })
        })
}

fn first_field_construction(
    owner_fn: Option<&FunctionSummary>,
    probe_line: usize,
) -> Option<LocalTextFact> {
    owner_fn.and_then(|function| {
        function
            .body
            .lines()
            .enumerate()
            .skip(probe_line.saturating_sub(function.start_line))
            .find_map(|(offset, line)| {
                let text = line.trim().trim_end_matches(',').to_string();
                if looks_like_field_assignment(&text) {
                    Some(LocalTextFact {
                        line: function.start_line + offset,
                        text,
                    })
                } else {
                    None
                }
            })
    })
}

fn flow_sink(
    kind: FlowSinkKind,
    text: impl Into<String>,
    line: usize,
    owner: Option<SymbolId>,
) -> FlowSinkFact {
    FlowSinkFact {
        kind,
        text: text.into(),
        line,
        owner,
    }
}

fn result_error_text(text: &str) -> String {
    if let Some(variant) = exact_error_variant(text) {
        return format!("Result::Err({variant})");
    }
    if let Some(start) = text.find("Err(") {
        let error = text[start..]
            .trim()
            .trim_start_matches("return ")
            .trim_end_matches(';')
            .trim_end_matches(',')
            .to_string();
        return format!("Result::{error}");
    }
    return_sink_text(text)
}

fn return_sink_text(text: &str) -> String {
    text.trim()
        .trim_start_matches("return ")
        .trim_end_matches(';')
        .trim_end_matches(',')
        .trim()
        .to_string()
}

fn call_effect_text(text: &str) -> String {
    return_sink_text(text)
}

fn field_sink_text(text: &str) -> String {
    return_sink_text(text)
}

fn looks_like_field_assignment(text: &str) -> bool {
    let Some((field, _)) = text.split_once(':') else {
        return false;
    };
    if text.contains("::") {
        return false;
    }
    let field = field.trim();
    !field.is_empty()
        && field
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && field
            .chars()
            .next()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
}

fn looks_like_branch_tail_expression(text: &str) -> bool {
    if text.is_empty()
        || text == "{"
        || text == "}"
        || text.starts_with("else")
        || text.starts_with("//")
        || text.starts_with("let ")
        || text.ends_with(';')
    {
        return false;
    }
    if text.contains(" = ")
        || text.contains(" += ")
        || text.contains(" -= ")
        || text.contains(" *= ")
        || text.contains(" /= ")
    {
        return false;
    }
    is_obvious_return_expression(text)
}

fn is_obvious_return_expression(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with("return ")
        || trimmed.starts_with("Ok(")
        || trimmed.starts_with("Some(")
        || trimmed.contains("Err(")
        || trimmed.contains('(')
        || trimmed.contains('"')
        || trimmed.chars().any(|ch| ch.is_ascii_digit())
        || [" + ", " - ", " * ", " / ", " % "]
            .iter()
            .any(|operator| trimmed.contains(operator))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::rust_index::ReturnFact;
    use std::path::PathBuf;

    #[test]
    fn predicate_flow_uses_nearest_return_after_changed_line() {
        let owner = function(
            "pub fn score(amount: i32) -> i32 {\n    if amount > 10 {\n        amount - 1\n    }\n}",
        );
        let probe = probe(ProbeFamily::Predicate, "amount > 10", 2);

        let sinks = local_flow_sinks(&probe, Some(&owner));

        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].kind, FlowSinkKind::ReturnValue);
        assert_eq!(sinks[0].text, "amount - 1");
        assert_eq!(sinks[0].line, 3);
    }

    #[test]
    fn error_path_flow_uses_exact_error_variant_text() {
        let probe = probe(
            ProbeFamily::ErrorPath,
            "return Err(AuthError::RevokedToken);",
            2,
        );

        let sinks = local_flow_sinks(&probe, None);

        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].kind, FlowSinkKind::ErrorVariant);
        assert_eq!(sinks[0].text, "Result::Err(AuthError::RevokedToken)");
    }

    #[test]
    fn propagation_names_first_visible_sink() {
        let probe = probe(ProbeFamily::Predicate, "amount > 10", 2);
        let sinks = vec![FlowSinkFact {
            kind: FlowSinkKind::ReturnValue,
            text: "amount - 1".to_string(),
            line: 3,
            owner: None,
        }];

        let evidence = propagation_evidence(&probe, &sinks);

        assert_eq!(evidence.state, StageState::Yes);
        assert_eq!(
            evidence.summary,
            "Changed behavior appears to influence returned value: amount - 1"
        );
    }

    #[test]
    fn propagation_is_unknown_for_static_unknown_probe() {
        let probe = probe(ProbeFamily::StaticUnknown, "let value = total;", 2);

        let evidence = propagation_evidence(&probe, &[]);

        assert_eq!(evidence.state, StageState::Unknown);
        assert_eq!(
            evidence.summary,
            "No propagation model is available for this changed syntax"
        );
    }

    fn function(body: &str) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId("src/lib.rs::score".to_string()),
            name: "score".to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: body.lines().count(),
            body: body.to_string(),
            calls: Vec::new(),
            returns: vec![ReturnFact {
                line: 3,
                text: "amount - 1".to_string(),
            }],
            literals: Vec::new(),
            is_test: false,
            attrs: Vec::new(),
        }
    }

    fn probe(family: ProbeFamily, expression: &str, line: usize) -> Probe {
        Probe {
            id: ProbeId("probe:test".to_string()),
            location: SourceLocation::new("src/lib.rs", line, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family,
            delta: DeltaKind::Control,
            before: None,
            after: Some(expression.to_string()),
            expression: expression.to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        }
    }
}
