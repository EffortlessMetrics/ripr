use super::super::rust_index::{FunctionSummary, TestSummary};
use super::text::{delimited_contents_at, enum_variant_values, exact_error_variant};
use crate::domain::*;

pub(in crate::analysis) fn activation_evidence(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    related_tests: &[&TestSummary],
    flow_sinks: &[FlowSinkFact],
) -> ActivationEvidence {
    let mut observed_values = related_tests
        .iter()
        .flat_map(|test| value_facts_for_test(test, owner_fn))
        .collect::<Vec<_>>();
    observed_values.extend(observed_discriminator_values(
        probe,
        owner_fn,
        related_tests,
    ));
    sort_value_facts(&mut observed_values);

    let mut missing_discriminators =
        missing_discriminator_facts(probe, owner_fn, related_tests, flow_sinks, &observed_values);
    missing_discriminators.sort_by(|left, right| {
        left.value
            .cmp(&right.value)
            .then(left.reason.cmp(&right.reason))
            .then(
                left.flow_sink
                    .as_ref()
                    .map(|sink| sink.kind.as_str())
                    .cmp(&right.flow_sink.as_ref().map(|sink| sink.kind.as_str())),
            )
    });
    missing_discriminators
        .dedup_by(|left, right| left.value == right.value && left.reason == right.reason);

    ActivationEvidence {
        observed_values,
        missing_discriminators,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParameterValue {
    parameter: String,
    value: String,
    line: usize,
    text: String,
}

fn value_facts_for_test(test: &TestSummary, owner_fn: Option<&FunctionSummary>) -> Vec<ValueFact> {
    let owner_name = owner_fn.map(|owner| owner.name.as_str()).unwrap_or("");
    let parameters = owner_fn.map(function_parameters).unwrap_or_default();
    let mut facts = Vec::new();

    for call in &test.calls {
        if !owner_name.is_empty() && call.name != owner_name {
            continue;
        }
        let Some(arguments) = call_arguments(&call.text, &call.name) else {
            continue;
        };
        for (idx, argument) in arguments.iter().enumerate() {
            for value in scalar_values(argument) {
                let value = parameters
                    .get(idx)
                    .map(|parameter| format!("{parameter} = {value}"))
                    .unwrap_or(value);
                facts.push(ValueFact {
                    line: call.line,
                    text: call.text.clone(),
                    value,
                    context: ValueContext::FunctionArgument,
                });
            }
            for value in enum_variant_values(argument) {
                facts.push(ValueFact {
                    line: call.line,
                    text: call.text.clone(),
                    value,
                    context: ValueContext::EnumVariant,
                });
            }
        }
    }

    for assertion in &test.assertions {
        let assertion_arguments = macro_arguments(&assertion.text).unwrap_or_default();
        for argument in assertion_arguments {
            if argument.contains(owner_name) && !owner_name.is_empty() {
                continue;
            }
            for value in scalar_values(&argument) {
                facts.push(ValueFact {
                    line: assertion.line,
                    text: assertion.text.clone(),
                    value,
                    context: ValueContext::AssertionArgument,
                });
            }
        }
        for value in enum_variant_values(&assertion.text) {
            facts.push(ValueFact {
                line: assertion.line,
                text: assertion.text.clone(),
                value,
                context: ValueContext::EnumVariant,
            });
        }
    }

    for (offset, line) in test.body.lines().enumerate() {
        let line_number = test.start_line + offset;
        let trimmed = line.trim();
        if looks_like_table_row(trimmed) {
            for value in scalar_values(trimmed) {
                facts.push(ValueFact {
                    line: line_number,
                    text: trimmed.to_string(),
                    value,
                    context: ValueContext::TableRow,
                });
            }
        }
        if looks_like_builder_method(trimmed) {
            for value in scalar_values(trimmed) {
                facts.push(ValueFact {
                    line: line_number,
                    text: trimmed.to_string(),
                    value,
                    context: ValueContext::BuilderMethod,
                });
            }
        }
    }

    sort_value_facts(&mut facts);
    facts
}

fn observed_discriminator_values(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    related_tests: &[&TestSummary],
) -> Vec<ValueFact> {
    let Some((left, right)) = comparison_operands(&probe.expression) else {
        return Vec::new();
    };
    let Some(owner) = owner_fn else {
        return Vec::new();
    };
    let parameters = function_parameters(owner);
    let call_values = owner_call_parameter_values(related_tests, &owner.name, &parameters);
    let Some(left_parameter) = boundary_operand_parameter(owner, &parameters, &left) else {
        return Vec::new();
    };
    let right_parameter = boundary_operand_parameter(owner, &parameters, &right);
    let mut facts = Vec::new();

    for row in call_values {
        let Some(left_value) = parameter_value(&row, &left_parameter) else {
            continue;
        };
        let right_value = right_parameter
            .as_deref()
            .and_then(|parameter| parameter_value(&row, parameter))
            .map(|value| value.value)
            .or_else(|| literal_operand_value(&right));
        if right_value
            .as_deref()
            .is_some_and(|value| comparable_value(value) == comparable_value(&left_value.value))
        {
            facts.push(ValueFact {
                line: left_value.line,
                text: left_value.text.clone(),
                value: format!("{left} == {right}"),
                context: ValueContext::FunctionArgument,
            });
        }
    }

    facts
}

fn missing_discriminator_facts(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    related_tests: &[&TestSummary],
    flow_sinks: &[FlowSinkFact],
    observed_values: &[ValueFact],
) -> Vec<MissingDiscriminatorFact> {
    let mut missing = Vec::new();
    if matches!(probe.family, ProbeFamily::Predicate)
        && let Some(fact) =
            missing_boundary_discriminator(probe, owner_fn, related_tests, flow_sinks)
    {
        missing.push(fact);
    }
    if (matches!(probe.family, ProbeFamily::ErrorPath)
        || flow_sinks
            .iter()
            .any(|sink| sink.kind == FlowSinkKind::ErrorVariant))
        && let Some(fact) = missing_error_variant_discriminator(probe, related_tests, flow_sinks)
    {
        missing.push(fact);
    }
    if missing.is_empty()
        && observed_values
            .iter()
            .any(|fact| fact.value.contains(" == "))
    {
        return Vec::new();
    }
    missing
}

fn missing_boundary_discriminator(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    related_tests: &[&TestSummary],
    flow_sinks: &[FlowSinkFact],
) -> Option<MissingDiscriminatorFact> {
    let (left, right) = comparison_operands(&probe.expression)?;
    let owner = owner_fn?;
    let parameters = function_parameters(owner);
    let call_values = owner_call_parameter_values(related_tests, &owner.name, &parameters);
    if call_values.is_empty() {
        return None;
    }
    let left_parameter = boundary_operand_parameter(owner, &parameters, &left);
    let right_parameter = boundary_operand_parameter(owner, &parameters, &right);

    let equality_observed = left_parameter.as_deref().is_some_and(|left_parameter| {
        call_values.iter().any(|row| {
            let Some(left_value) = parameter_value(row, left_parameter) else {
                return false;
            };
            let right_value = right_parameter
                .as_deref()
                .and_then(|parameter| parameter_value(row, parameter))
                .map(|value| value.value)
                .or_else(|| literal_operand_value(&right));
            right_value
                .as_deref()
                .is_some_and(|value| comparable_value(value) == comparable_value(&left_value.value))
        })
    });
    if equality_observed {
        return None;
    }

    let left_values = left_parameter
        .as_deref()
        .map(|parameter| observed_parameter_values(&call_values, parameter))
        .unwrap_or_default();
    let right_parameter_values = right_parameter
        .as_deref()
        .and_then(|parameter| parameter_value_set(&call_values, parameter));
    let right_literal = literal_operand_value(&right);
    let reason = if let Some(right_values) = right_parameter_values {
        format!(
            "No related test call uses {left} equal to {right}; observed {left} values: {}; observed {right} values: {}",
            list_or_unknown(&left_values),
            list_or_unknown(&right_values)
        )
    } else if let Some(right_value) = right_literal {
        format!(
            "No related test call uses {left} equal to {right}; observed {left} values: {}; target {right} value: {right_value}",
            list_or_unknown(&left_values)
        )
    } else {
        format!(
            "No related test call uses {left} equal to {right}; observed {left} values: {}",
            list_or_unknown(&left_values)
        )
    };

    Some(MissingDiscriminatorFact {
        value: format!("{left} == {right}"),
        reason,
        flow_sink: first_visible_flow_sink(flow_sinks).cloned(),
    })
}

fn missing_error_variant_discriminator(
    probe: &Probe,
    related_tests: &[&TestSummary],
    flow_sinks: &[FlowSinkFact],
) -> Option<MissingDiscriminatorFact> {
    let variant = exact_error_variant(&probe.expression).or_else(|| {
        flow_sinks
            .iter()
            .find_map(|sink| exact_error_variant(&sink.text))
    })?;
    let exact_assertion_found = related_tests.iter().any(|test| {
        test.assertions.iter().any(|assertion| {
            assertion.kind == OracleKind::ExactErrorVariant && assertion.text.contains(&variant)
        })
    });
    if exact_assertion_found {
        return None;
    }

    Some(MissingDiscriminatorFact {
        value: variant.clone(),
        reason: format!("No exact error variant assertion for {variant}"),
        flow_sink: flow_sinks
            .iter()
            .find(|sink| sink.kind == FlowSinkKind::ErrorVariant)
            .or_else(|| first_visible_flow_sink(flow_sinks))
            .cloned(),
    })
}

fn owner_call_parameter_values(
    related_tests: &[&TestSummary],
    owner_name: &str,
    parameters: &[String],
) -> Vec<Vec<ParameterValue>> {
    let mut rows = Vec::new();
    if owner_name.is_empty() || parameters.is_empty() {
        return rows;
    }
    for test in related_tests {
        for call in &test.calls {
            if call.name != owner_name {
                continue;
            }
            let Some(arguments) = call_arguments(&call.text, &call.name) else {
                continue;
            };
            let row = arguments
                .iter()
                .enumerate()
                .filter_map(|(idx, argument)| {
                    let parameter = parameters.get(idx)?;
                    let value = scalar_values(argument).into_iter().next()?;
                    Some(ParameterValue {
                        parameter: parameter.clone(),
                        value,
                        line: call.line,
                        text: call.text.clone(),
                    })
                })
                .collect::<Vec<_>>();
            if !row.is_empty() {
                rows.push(row);
            }
        }
    }
    rows
}

fn parameter_value(row: &[ParameterValue], parameter: &str) -> Option<ParameterValue> {
    row.iter()
        .find(|value| value.parameter == parameter)
        .cloned()
}

fn parameter_value_set(rows: &[Vec<ParameterValue>], parameter: &str) -> Option<Vec<String>> {
    let mut values = observed_parameter_values(rows, parameter);
    if values.is_empty() {
        None
    } else {
        values.sort();
        values.dedup();
        Some(values)
    }
}

fn observed_parameter_values(rows: &[Vec<ParameterValue>], parameter: &str) -> Vec<String> {
    let mut values = rows
        .iter()
        .flat_map(|row| {
            row.iter()
                .filter(|value| value.parameter == parameter)
                .map(|value| value.value.clone())
        })
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn function_parameters(function: &FunctionSummary) -> Vec<String> {
    let signature = function
        .body
        .lines()
        .next()
        .unwrap_or(function.body.as_str());
    let Some(arguments) = delimited_contents_after(signature, '(') else {
        return Vec::new();
    };
    split_top_level_args(&arguments)
        .into_iter()
        .filter_map(|argument| {
            argument
                .split_once(':')
                .map(|(name, _)| name.trim().to_string())
        })
        .filter(|name| !name.is_empty() && name != "self" && name != "&self" && name != "mut self")
        .collect()
}

fn boundary_operand_parameter(
    function: &FunctionSummary,
    parameters: &[String],
    operand: &str,
) -> Option<String> {
    parameters
        .iter()
        .find(|parameter| parameter.as_str() == operand)
        .cloned()
        .or_else(|| boundary_local_operand_parameter(function, parameters, operand))
}

fn boundary_local_operand_parameter(
    function: &FunctionSummary,
    parameters: &[String],
    operand: &str,
) -> Option<String> {
    if operand.is_empty() {
        return None;
    }
    for parameter in parameters {
        if body_contains_wrapped_local_alias(&function.body, "Some", operand, parameter)
            || body_contains_wrapped_local_alias(&function.body, "Ok", operand, parameter)
            || body_contains_direct_local_alias(&function.body, operand, parameter)
        {
            return Some(parameter.clone());
        }
    }
    None
}

fn body_contains_wrapped_local_alias(
    body: &str,
    wrapper: &str,
    operand: &str,
    parameter: &str,
) -> bool {
    body.lines().any(|line| {
        let line = line.trim();
        let prefix = format!("if let {wrapper}({operand}) = ");
        line.strip_prefix(&prefix)
            .is_some_and(|rest| starts_with_identifier_token(rest, parameter))
    }) || (body_contains_match_parameter(body, parameter)
        && body_contains_wrapper_pattern(body, wrapper, operand))
}

fn body_contains_match_parameter(body: &str, parameter: &str) -> bool {
    body.lines().any(|line| {
        let line = line.trim();
        if is_comment_line(line) {
            return false;
        }
        line.find("match ")
            .map(|index| &line[index + "match ".len()..])
            .is_some_and(|rest| starts_with_identifier_token(rest, parameter))
    })
}

fn body_contains_wrapper_pattern(body: &str, wrapper: &str, operand: &str) -> bool {
    let pattern = format!("{wrapper}({operand})");
    body.lines().any(|line| {
        let line = line.trim();
        !is_comment_line(line) && line.contains(&pattern)
    })
}

fn is_comment_line(line: &str) -> bool {
    line.starts_with("//") || line.starts_with("/*") || line.starts_with('*')
}

fn starts_with_identifier_token(text: &str, token: &str) -> bool {
    let text = text.trim_start();
    let end = text
        .find(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .unwrap_or(text.len());
    end > 0 && &text[..end] == token
}

fn body_contains_direct_local_alias(body: &str, operand: &str, parameter: &str) -> bool {
    body.lines().any(|line| {
        let line = line.trim().trim_end_matches(';').trim();
        let Some(binding) = line.strip_prefix("let ") else {
            return false;
        };
        let Some((left, right)) = binding.split_once('=') else {
            return false;
        };
        let local_name = left.split_once(':').map(|(name, _)| name).unwrap_or(left);
        local_name.trim() == operand && right.trim() == parameter
    })
}

fn comparison_operands(expression: &str) -> Option<(String, String)> {
    for operator in [">=", "<=", "==", "!=", ">", "<"] {
        if let Some((left, right)) = expression.split_once(operator) {
            let left = clean_operand(left);
            let right = clean_operand(right);
            if !left.is_empty() && !right.is_empty() {
                return Some((left, right));
            }
        }
    }
    None
}

fn clean_operand(operand: &str) -> String {
    let cleaned = operand
        .trim()
        .trim_start_matches("if ")
        .trim_end_matches('{')
        .trim_end_matches(';')
        .trim();
    let cleaned = cleaned
        .split_once('{')
        .map(|(before, _)| before.trim())
        .unwrap_or(cleaned);
    cleaned.to_string()
}

fn literal_operand_value(operand: &str) -> Option<String> {
    scalar_values(operand).into_iter().next()
}

fn comparable_value(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .chars()
        .filter(|ch| *ch != '_')
        .collect()
}

fn first_visible_flow_sink(flow_sinks: &[FlowSinkFact]) -> Option<&FlowSinkFact> {
    flow_sinks
        .iter()
        .find(|sink| sink.kind != FlowSinkKind::Unknown)
}

fn list_or_unknown(values: &[String]) -> String {
    if values.is_empty() {
        "unknown".to_string()
    } else {
        values.join(", ")
    }
}

pub(in crate::analysis) fn has_observed_boundary_equality(activation: &ActivationEvidence) -> bool {
    activation
        .observed_values
        .iter()
        .any(|fact| fact.value.contains(" == "))
}

fn sort_value_facts(facts: &mut Vec<ValueFact>) {
    facts.sort_by(|left, right| {
        left.line
            .cmp(&right.line)
            .then(left.context.as_str().cmp(right.context.as_str()))
            .then(left.value.cmp(&right.value))
            .then(left.text.cmp(&right.text))
    });
    facts.dedup_by(|left, right| {
        left.line == right.line
            && left.text == right.text
            && left.value == right.value
            && left.context == right.context
    });
}

fn call_arguments(text: &str, name: &str) -> Option<Vec<String>> {
    let needle = format!("{name}(");
    let start = text.find(&needle)? + name.len();
    let contents = delimited_contents_at(text, start)?;
    Some(split_top_level_args(&contents))
}

fn macro_arguments(text: &str) -> Option<Vec<String>> {
    let start = text.find("!(")? + 1;
    let contents = delimited_contents_at(text, start)?;
    Some(split_top_level_args(&contents))
}

fn delimited_contents_after(text: &str, delimiter: char) -> Option<String> {
    let start = text.find(delimiter)?;
    delimited_contents_at(text, start)
}

fn split_top_level_args(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in text.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                if let Some(arg) = text.get(start..idx).map(str::trim)
                    && !arg.is_empty()
                {
                    args.push(arg.to_string());
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    if let Some(arg) = text.get(start..).map(str::trim)
        && !arg.is_empty()
    {
        args.push(arg.to_string());
    }
    args
}

fn scalar_values(text: &str) -> Vec<String> {
    let mut values = Vec::new();
    let chars = text.char_indices().collect::<Vec<_>>();
    let mut idx = 0usize;
    while idx < chars.len() {
        let (byte_idx, ch) = chars[idx];
        if ch == '"' {
            let mut end = byte_idx + ch.len_utf8();
            let mut cursor = idx + 1;
            let mut escaped = false;
            while cursor < chars.len() {
                let (next_byte, next_ch) = chars[cursor];
                end = next_byte + next_ch.len_utf8();
                if escaped {
                    escaped = false;
                } else if next_ch == '\\' {
                    escaped = true;
                } else if next_ch == '"' {
                    break;
                }
                cursor += 1;
            }
            if let Some(value) = text.get(byte_idx..end) {
                values.push(value.to_string());
            }
            idx = cursor.saturating_add(1);
            continue;
        }
        if ch.is_ascii_digit()
            || (ch == '-'
                && chars
                    .get(idx + 1)
                    .is_some_and(|(_, next_ch)| next_ch.is_ascii_digit()))
        {
            let mut end = byte_idx + ch.len_utf8();
            let mut cursor = idx + 1;
            while cursor < chars.len() {
                let (next_byte, next_ch) = chars[cursor];
                if next_ch.is_ascii_digit() || next_ch == '_' {
                    end = next_byte + next_ch.len_utf8();
                    cursor += 1;
                } else {
                    break;
                }
            }
            if let Some(value) = text.get(byte_idx..end) {
                values.push(value.to_string());
            }
            idx = cursor;
            continue;
        }
        idx += 1;
    }
    values.sort();
    values.dedup();
    values
}

fn looks_like_table_row(line: &str) -> bool {
    (line.starts_with('(') || line.starts_with('[') || line.contains("[(")) && line.contains(',')
}

fn looks_like_builder_method(line: &str) -> bool {
    line.contains('.')
        && line.contains('(')
        && (line.contains("builder")
            || line.contains("with_")
            || line.contains(".amount(")
            || line.contains(".token(")
            || line.contains(".threshold("))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::rust_index::{CallFact, OracleFact};
    use std::path::PathBuf;

    #[test]
    fn activation_evidence_records_observed_boundary_equality() {
        let owner = function(
            "pub fn score(amount: i32, threshold: i32) -> bool {\n    amount >= threshold\n}",
        );
        let test = test_with_call("score_uses_boundary", "score(100, 100);");
        let probe = probe(ProbeFamily::Predicate, "amount >= threshold");

        let activation = activation_evidence(&probe, Some(&owner), &[&test], &[]);

        assert!(has_observed_boundary_equality(&activation));
        assert!(activation.missing_discriminators.is_empty());
        assert!(activation.observed_values.iter().any(|fact| {
            fact.context == ValueContext::FunctionArgument && fact.value == "amount == threshold"
        }));
    }

    #[test]
    fn activation_evidence_resolves_direct_local_boundary_operand_alias() {
        let owner = function(
            "pub fn score(raw_amount: i32, threshold: i32) -> bool {\n    let amount = raw_amount;\n    amount >= threshold\n}",
        );
        let test = test_with_call("score_uses_boundary", "score(100, 100);");
        let probe = probe(ProbeFamily::Predicate, "amount >= threshold");

        let activation = activation_evidence(&probe, Some(&owner), &[&test], &[]);

        assert!(has_observed_boundary_equality(&activation));
        assert!(activation.missing_discriminators.is_empty());
        assert!(activation.observed_values.iter().any(|fact| {
            fact.context == ValueContext::FunctionArgument && fact.value == "amount == threshold"
        }));
    }

    #[test]
    fn activation_evidence_uses_exact_if_let_parameter_name_for_boundary_operand_alias() {
        let owner = function(
            "pub fn score(raw_amount: Option<i32>, raw_amount_extra: Option<i32>, threshold: i32) -> bool {\n    if let Some(amount) = raw_amount_extra { amount >= threshold } else { false }\n}",
        );
        let test = test_with_call("score_uses_boundary", "score(Some(100), Some(101), 100);");
        let probe = probe(ProbeFamily::Predicate, "amount >= threshold");

        let activation = activation_evidence(&probe, Some(&owner), &[&test], &[]);

        assert!(!has_observed_boundary_equality(&activation));
        assert_eq!(activation.missing_discriminators.len(), 1);
        assert!(
            activation.missing_discriminators[0]
                .reason
                .contains("observed amount values: 101"),
            "prefix parameter matches must not make raw_amount look like amount; got {:?}",
            activation.missing_discriminators
        );
    }

    #[test]
    fn activation_evidence_ignores_commented_match_boundary_operand_alias() {
        let owner = function(
            "pub fn score(raw_amount: Option<i32>, threshold: i32) -> bool {\n    // match raw_amount { Some(amount) => amount >= threshold, _ => false }\n    let amount = 1;\n    amount >= threshold\n}",
        );
        let test = test_with_call("score_uses_boundary", "score(Some(100), 100);");
        let probe = probe(ProbeFamily::Predicate, "amount >= threshold");

        let activation = activation_evidence(&probe, Some(&owner), &[&test], &[]);

        assert!(!has_observed_boundary_equality(&activation));
        assert_eq!(activation.missing_discriminators.len(), 1);
        assert!(
            activation.missing_discriminators[0]
                .reason
                .contains("observed amount values: unknown"),
            "commented match aliases must not resolve boundary operands; got {:?}",
            activation.missing_discriminators
        );
    }

    #[test]
    fn activation_evidence_ignores_commented_match_wrapper_pattern() {
        let owner = function(
            "pub fn score(raw_amount: Option<i32>, threshold: i32) -> bool {\n    let _seen = match raw_amount { _ => false };\n    // Some(amount)\n    let amount = 1;\n    amount >= threshold\n}",
        );
        let test = test_with_call("score_uses_boundary", "score(Some(100), 100);");
        let probe = probe(ProbeFamily::Predicate, "amount >= threshold");

        let activation = activation_evidence(&probe, Some(&owner), &[&test], &[]);

        assert!(!has_observed_boundary_equality(&activation));
        assert_eq!(activation.missing_discriminators.len(), 1);
        assert!(
            activation.missing_discriminators[0]
                .reason
                .contains("observed amount values: unknown"),
            "commented wrapper patterns must not resolve boundary operands; got {:?}",
            activation.missing_discriminators
        );
    }

    #[test]
    fn activation_evidence_keeps_computed_local_boundary_operand_unresolved() {
        let owner = function(
            "pub fn score(raw_amount: i32, threshold: i32) -> bool {\n    let amount = raw_amount + 1;\n    amount >= threshold\n}",
        );
        let test = test_with_call("score_uses_boundary", "score(100, 100);");
        let probe = probe(ProbeFamily::Predicate, "amount >= threshold");

        let activation = activation_evidence(&probe, Some(&owner), &[&test], &[]);

        assert!(!has_observed_boundary_equality(&activation));
        assert_eq!(activation.missing_discriminators.len(), 1);
        assert_eq!(
            activation.missing_discriminators[0].value,
            "amount == threshold"
        );
        assert!(
            activation.missing_discriminators[0]
                .reason
                .contains("observed amount values: unknown")
        );
    }

    #[test]
    fn activation_evidence_reports_missing_boundary_discriminator() {
        let owner = function("pub fn score(amount: i32) -> bool {\n    amount > 10\n}");
        let test = test_with_call("score_uses_adjacent_value", "score(9);");
        let probe = probe(ProbeFamily::Predicate, "amount > 10");
        let flow_sinks = vec![FlowSinkFact {
            kind: FlowSinkKind::ReturnValue,
            text: "amount > 10".to_string(),
            line: 2,
            owner: None,
        }];

        let activation = activation_evidence(&probe, Some(&owner), &[&test], &flow_sinks);

        assert_eq!(activation.missing_discriminators.len(), 1);
        assert_eq!(activation.missing_discriminators[0].value, "amount == 10");
        assert!(
            activation.missing_discriminators[0]
                .reason
                .contains("observed amount values: 9")
        );
        assert_eq!(
            activation.missing_discriminators[0]
                .flow_sink
                .as_ref()
                .map(|sink| &sink.kind),
            Some(&FlowSinkKind::ReturnValue)
        );
    }

    #[test]
    fn activation_evidence_omits_missing_error_variant_when_exact_assertion_exists() {
        let test = test_with_assertion(
            "rejects_revoked",
            "assert_eq!(err, AuthError::RevokedToken);",
            OracleKind::ExactErrorVariant,
        );
        let probe = probe(
            ProbeFamily::ErrorPath,
            "return Err(AuthError::RevokedToken);",
        );
        let flow_sinks = vec![FlowSinkFact {
            kind: FlowSinkKind::ErrorVariant,
            text: "Result::Err(AuthError::RevokedToken)".to_string(),
            line: 2,
            owner: None,
        }];

        let activation = activation_evidence(&probe, None, &[&test], &flow_sinks);

        assert!(activation.missing_discriminators.is_empty());
    }

    #[test]
    fn activation_evidence_sorts_multiple_missing_discriminators() {
        let owner = function(
            "pub fn score(amount: i32) -> Result<bool, AuthError> {\n    if amount > 10 { return Err(AuthError::Bad); }\n    Ok(true)\n}",
        );
        let test = test_with_call("score_uses_adjacent_value", "score(9);");
        let probe = probe(ProbeFamily::Predicate, "amount > 10");
        let flow_sinks = vec![FlowSinkFact {
            kind: FlowSinkKind::ErrorVariant,
            text: "Result::Err(AuthError::Bad)".to_string(),
            line: 2,
            owner: None,
        }];

        let activation = activation_evidence(&probe, Some(&owner), &[&test], &flow_sinks);
        let values = activation
            .missing_discriminators
            .iter()
            .map(|fact| fact.value.as_str())
            .collect::<Vec<_>>();

        assert_eq!(values, vec!["AuthError::Bad", "amount == 10"]);
    }

    #[test]
    fn value_facts_for_test_preserves_table_builder_and_assertion_contexts() {
        let test = TestSummary {
            name: "table_and_builder".to_string(),
            file: PathBuf::from("tests/value.rs"),
            start_line: 10,
            end_line: 16,
            body: r#"let rows = [(99, 100), (100, 100)];
let input = Request::builder().amount(100).token("abc").build();
assert_eq!(input.amount, 100);"#
                .to_string(),
            calls: Vec::new(),
            assertions: vec![oracle_fact(
                "assert_eq!(input.amount, 100);",
                OracleKind::ExactValue,
            )],
            literals: Vec::new(),
            attrs: Vec::new(),
        };

        let facts = value_facts_for_test(&test, None);

        assert!(
            facts
                .iter()
                .any(|fact| fact.context == ValueContext::TableRow && fact.value == "99")
        );
        assert!(
            facts
                .iter()
                .any(|fact| fact.context == ValueContext::BuilderMethod && fact.value == "100")
        );
        assert!(
            facts
                .iter()
                .any(|fact| fact.context == ValueContext::BuilderMethod && fact.value == "\"abc\"")
        );
        assert!(
            facts
                .iter()
                .any(|fact| fact.context == ValueContext::AssertionArgument && fact.value == "100")
        );
    }

    #[test]
    fn value_facts_for_test_filters_non_owner_calls_and_reads_enum_call_arguments() {
        let owner = function(
            "pub fn score(error: AuthError) -> Result<(), AuthError> {\n    Err(error)\n}",
        );
        let test = TestSummary {
            name: "enum_call".to_string(),
            file: PathBuf::from("tests/value.rs"),
            start_line: 10,
            end_line: 12,
            body: "other(AuthError::Ignored);\nscore(AuthError::RevokedToken);".to_string(),
            calls: vec![
                CallFact {
                    line: 11,
                    name: "other".to_string(),
                    text: "other(AuthError::Ignored);".to_string(),
                },
                CallFact {
                    line: 12,
                    name: "score".to_string(),
                    text: "score(AuthError::RevokedToken);".to_string(),
                },
                CallFact {
                    line: 13,
                    name: "score".to_string(),
                    text: "score;".to_string(),
                },
            ],
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
        };

        let facts = value_facts_for_test(&test, Some(&owner));

        assert!(
            facts
                .iter()
                .any(|fact| fact.value == "AuthError::RevokedToken")
        );
        assert!(!facts.iter().any(|fact| fact.value == "AuthError::Ignored"));
    }

    #[test]
    fn missing_boundary_handles_missing_left_and_nonliteral_target() {
        let owner = function("pub fn score(amount: i32) -> bool {\n    amount > 10\n}");
        let test = test_with_call("score_uses_other_value", "score(9);");
        let probe = probe(ProbeFamily::Predicate, "threshold > limit");

        let activation = activation_evidence(&probe, Some(&owner), &[&test], &[]);

        assert_eq!(activation.missing_discriminators.len(), 1);
        assert_eq!(
            activation.missing_discriminators[0].value,
            "threshold == limit"
        );
        assert!(
            activation.missing_discriminators[0]
                .reason
                .contains("observed threshold values: unknown")
        );
    }

    #[test]
    fn owner_call_parameter_values_handles_empty_inputs_and_skips_other_calls() {
        let test = TestSummary {
            name: "mixed_calls".to_string(),
            file: PathBuf::from("tests/value.rs"),
            start_line: 10,
            end_line: 12,
            body: "other(1);\nscore(2);".to_string(),
            calls: vec![
                CallFact {
                    line: 11,
                    name: "other".to_string(),
                    text: "other(1);".to_string(),
                },
                CallFact {
                    line: 12,
                    name: "score".to_string(),
                    text: "score(2);".to_string(),
                },
                CallFact {
                    line: 13,
                    name: "score".to_string(),
                    text: "score;".to_string(),
                },
            ],
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
        };

        assert!(owner_call_parameter_values(&[&test], "", &["amount".to_string()]).is_empty());
        assert!(owner_call_parameter_values(&[&test], "score", &[]).is_empty());

        let rows = owner_call_parameter_values(&[&test], "score", &["amount".to_string()]);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0].value, "2");
    }

    #[test]
    fn text_helpers_handle_braces_escapes_negative_numbers_and_dedup_contexts() {
        let owner = function("pub fn score(amount: i32) -> bool {\n    amount > 10\n}");
        let non_comparison_probe = probe(ProbeFamily::ReturnValue, "amount");
        assert!(observed_discriminator_values(&non_comparison_probe, Some(&owner), &[]).is_empty());
        assert!(
            observed_discriminator_values(&probe(ProbeFamily::Predicate, "amount > 10"), None, &[])
                .is_empty()
        );
        assert_eq!(
            comparison_operands("if amount > 10 {"),
            Some(("amount".to_string(), "10".to_string()))
        );
        assert_eq!(comparison_operands("> 10"), None);
        assert_eq!(
            call_arguments(r#"score("a\",b", -12)"#, "score"),
            Some(vec![r#""a\",b""#.to_string(), "-12".to_string()])
        );
        assert_eq!(
            scalar_values(r#""a\"b" -12"#),
            vec!["\"a\\\"b\"".to_string(), "-12".to_string()]
        );

        let mut facts = vec![
            value_fact(1, "score(1)", "1", ValueContext::FunctionArgument),
            value_fact(1, "score(1)", "1", ValueContext::AssertionArgument),
        ];

        sort_value_facts(&mut facts);

        assert_eq!(facts.len(), 2);
    }

    fn probe(family: ProbeFamily, expression: &str) -> Probe {
        Probe {
            id: ProbeId("probe:src/lib.rs:2:score".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 5),
            owner: None,
            family,
            delta: DeltaKind::Control,
            before: None,
            after: Some(expression.to_string()),
            expression: expression.to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        }
    }

    fn function(body: &str) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId("src/lib.rs::score".to_string()),
            name: "score".to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 3,
            body: body.to_string(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            is_test: false,
            attrs: Vec::new(),
        }
    }

    fn test_with_call(name: &str, call: &str) -> TestSummary {
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from("tests/score.rs"),
            start_line: 10,
            end_line: 12,
            body: call.to_string(),
            calls: vec![CallFact {
                name: "score".to_string(),
                line: 11,
                text: call.to_string(),
            }],
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
        }
    }

    fn test_with_assertion(name: &str, assertion: &str, kind: OracleKind) -> TestSummary {
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from("tests/score.rs"),
            start_line: 10,
            end_line: 12,
            body: assertion.to_string(),
            calls: Vec::new(),
            assertions: vec![oracle_fact(assertion, kind)],
            literals: Vec::new(),
            attrs: Vec::new(),
        }
    }

    fn oracle_fact(assertion: &str, kind: OracleKind) -> OracleFact {
        OracleFact {
            kind,
            strength: OracleStrength::Strong,
            line: 11,
            text: assertion.to_string(),
            observed_tokens: Vec::new(),
        }
    }

    fn value_fact(line: usize, text: &str, value: &str, context: ValueContext) -> ValueFact {
        ValueFact {
            line,
            text: text.to_string(),
            value: value.to_string(),
            context,
        }
    }
}
