use super::rust_index::{
    FunctionSummary, OracleFact, RustIndex, TestSummary, extract_identifier_tokens,
    extract_literals,
};
use crate::domain::*;
use std::path::Path;

pub fn classify_probe(probe: &Probe, index: &RustIndex) -> Finding {
    let owner_fn = probe.owner.as_ref().and_then(|owner| {
        index
            .functions
            .iter()
            .find(|function| &function.id == owner)
    });

    let related_tests = find_related_tests(probe, owner_fn, index);
    let reach = reach_evidence(&related_tests, owner_fn);
    let flow_sinks = local_flow_sinks(probe, owner_fn);
    let activation = activation_evidence(probe, owner_fn, &related_tests, &flow_sinks);
    let infect = infection_evidence(probe, &related_tests, &activation);
    let propagate = propagation_evidence(probe, &flow_sinks);
    let (observe, discriminate, related) = reveal_evidence(probe, &related_tests);

    let ripr = RiprEvidence {
        reach: reach.clone(),
        infect: infect.clone(),
        propagate: propagate.clone(),
        reveal: RevealEvidence {
            observe: observe.clone(),
            discriminate: discriminate.clone(),
        },
    };

    let class = classify(&reach, &infect, &propagate, &observe, &discriminate, probe);
    let confidence = confidence_score(&reach, &infect, &propagate, &observe, &discriminate, &class);
    let mut evidence = Vec::new();
    evidence.push(reach.summary.clone());
    if !infect.summary.is_empty() {
        evidence.push(infect.summary.clone());
    }
    if !propagate.summary.is_empty() {
        evidence.push(propagate.summary.clone());
    }
    if !observe.summary.is_empty() {
        evidence.push(observe.summary.clone());
    }
    if !discriminate.summary.is_empty() {
        evidence.push(discriminate.summary.clone());
    }
    evidence.sort();
    evidence.dedup();

    let missing = missing_evidence(probe, &class, &infect, &observe, &discriminate, &activation);
    let mut stop_reasons = stop_reasons(probe, owner_fn, &related_tests);
    ensure_unknown_stop_reason(&class, &mut stop_reasons);
    let recommended_next_step = recommended_next_step(probe, &class);

    Finding {
        id: probe.id.0.clone(),
        probe: probe.clone(),
        class,
        ripr,
        confidence,
        evidence,
        missing,
        flow_sinks,
        activation,
        stop_reasons,
        related_tests: related,
        recommended_next_step,
    }
}

fn ensure_unknown_stop_reason(class: &ExposureClass, stop_reasons: &mut Vec<StopReason>) {
    if class.requires_stop_reason()
        && stop_reasons.is_empty()
        && let Some(reason) = StopReason::for_unknown_class(class)
    {
        stop_reasons.push(reason);
    }
}

fn find_related_tests<'a>(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    index: &'a RustIndex,
) -> Vec<&'a TestSummary> {
    let mut related = Vec::new();
    let owner_name = owner_fn.map(|f| f.name.as_str()).unwrap_or("");
    let probe_tokens = extract_identifier_tokens(&probe.expression);
    let file_name = probe
        .location
        .file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let package_prefix = owner_fn.and_then(|owner| package_prefix(&owner.file));

    for test in &index.tests {
        if let Some(prefix) = &package_prefix
            && !normalize_path(&test.file).starts_with(prefix)
        {
            continue;
        }
        let calls_owner = !owner_name.is_empty()
            && (test.calls.iter().any(|call| call.name == owner_name)
                || test.body.contains(owner_name));
        let test_name = test.name.to_ascii_lowercase();
        let owner_name = owner_name.to_ascii_lowercase();
        let same_file_or_named = normalize_path(&test.file).contains(file_name)
            || (!owner_name.is_empty() && test_name.contains(&owner_name))
            || probe_tokens
                .iter()
                .any(|token| token.len() > 2 && test_name.contains(&token.to_ascii_lowercase()));

        if calls_owner || same_file_or_named {
            related.push(test);
        }
    }

    related.sort_by(|a, b| a.name.cmp(&b.name));
    related.dedup_by(|a, b| a.name == b.name && a.file == b.file);
    related
}

fn reach_evidence(
    related_tests: &[&TestSummary],
    owner_fn: Option<&FunctionSummary>,
) -> StageEvidence {
    if related_tests.is_empty() {
        StageEvidence::new(
            StageState::No,
            Confidence::Medium,
            "No static test path found for the changed owner",
        )
    } else {
        let target = owner_fn.map(|f| f.name.as_str()).unwrap_or("changed owner");
        let names = related_tests
            .iter()
            .take(3)
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        StageEvidence::new(
            StageState::Yes,
            Confidence::Medium,
            format!("Related tests appear to reach {target}: {names}"),
        )
    }
}

fn activation_evidence(
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
    let mut facts = Vec::new();

    for row in call_values {
        let Some(left_value) = parameter_value(&row, &left) else {
            continue;
        };
        let right_value = parameter_value(&row, &right)
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

    let equality_observed = call_values.iter().any(|row| {
        let Some(left_value) = parameter_value(row, &left) else {
            return false;
        };
        let right_value = parameter_value(row, &right)
            .map(|value| value.value)
            .or_else(|| literal_operand_value(&right));
        right_value
            .as_deref()
            .is_some_and(|value| comparable_value(value) == comparable_value(&left_value.value))
    });
    if equality_observed {
        return None;
    }

    let left_values = observed_parameter_values(&call_values, &left);
    let right_parameter_values = parameter_value_set(&call_values, &right);
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

fn exact_error_variant(text: &str) -> Option<String> {
    let start = text.find("Err(")?;
    let inner = delimited_contents_at(text, start + "Err".len())?;
    enum_variant_values(&inner).into_iter().next()
}

fn list_or_unknown(values: &[String]) -> String {
    if values.is_empty() {
        "unknown".to_string()
    } else {
        values.join(", ")
    }
}

fn has_observed_boundary_equality(activation: &ActivationEvidence) -> bool {
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

fn delimited_contents_at(text: &str, open_index: usize) -> Option<String> {
    let bytes = text.as_bytes();
    if bytes.get(open_index) != Some(&b'(') {
        return None;
    }
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in text.char_indices().skip_while(|(idx, _)| *idx < open_index) {
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
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let start = open_index + 1;
                    return text.get(start..idx).map(ToString::to_string);
                }
            }
            _ => {}
        }
    }
    None
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

fn enum_variant_values(text: &str) -> Vec<String> {
    let mut values = Vec::new();
    for token in text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':')) {
        if !token.contains("::") {
            continue;
        }
        let Some(last) = token.rsplit("::").next() else {
            continue;
        };
        if last
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_uppercase())
        {
            values.push(token.to_string());
        }
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

fn infection_evidence(
    probe: &Probe,
    related_tests: &[&TestSummary],
    activation: &ActivationEvidence,
) -> StageEvidence {
    match probe.family {
        ProbeFamily::Predicate => {
            let probe_literals = extract_literals(&probe.expression);
            let test_literals = related_tests
                .iter()
                .flat_map(|test| test.literals.iter().map(|literal| literal.value.clone()))
                .collect::<Vec<_>>();
            if related_tests.is_empty() {
                StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "No tests were found, so activation/infection cannot be estimated",
                )
            } else if activation
                .missing_discriminators
                .iter()
                .any(|fact| fact.value.contains("=="))
            {
                StageEvidence::new(
                    StageState::Weak,
                    Confidence::Medium,
                    "Related tests contain input values, but the equality-boundary discriminator is missing",
                )
            } else if has_observed_boundary_equality(activation) {
                StageEvidence::new(
                    StageState::Yes,
                    Confidence::Medium,
                    "Detected related test input at the changed boundary",
                )
            } else if probe_literals.is_empty() {
                StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "Predicate changed, but no literal boundary was visible in the changed expression",
                )
            } else if probe_literals
                .iter()
                .any(|literal| test_literals.iter().any(|t| t == literal))
            {
                StageEvidence::new(
                    StageState::Yes,
                    Confidence::Medium,
                    format!(
                        "Detected test input literal matching changed boundary: {}",
                        probe_literals.join(", ")
                    ),
                )
            } else if !test_literals.is_empty() {
                StageEvidence::new(
                    StageState::Weak,
                    Confidence::Medium,
                    format!(
                        "Tests have literals [{}], but no detected value matches changed boundary [{}]",
                        test_literals.join(", "),
                        probe_literals.join(", ")
                    ),
                )
            } else {
                StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "Related tests use opaque fixtures; activation/infection is unknown",
                )
            }
        }
        ProbeFamily::StaticUnknown => StageEvidence::new(
            StageState::Unknown,
            Confidence::Unknown,
            "Changed syntax is not mapped to a high-confidence probe family",
        ),
        _ => {
            if related_tests.is_empty() {
                StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "No reachable tests were found, so infection cannot be established",
                )
            } else {
                StageEvidence::new(
                    StageState::Yes,
                    Confidence::Medium,
                    "Reachable tests can plausibly activate this changed behavior",
                )
            }
        }
    }
}

fn propagation_evidence(probe: &Probe, flow_sinks: &[FlowSinkFact]) -> StageEvidence {
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

fn local_flow_sinks(probe: &Probe, owner_fn: Option<&FunctionSummary>) -> Vec<FlowSinkFact> {
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

fn reveal_evidence(
    probe: &Probe,
    related_tests: &[&TestSummary],
) -> (StageEvidence, StageEvidence, Vec<RelatedTest>) {
    if related_tests.is_empty() {
        return (
            StageEvidence::new(
                StageState::No,
                Confidence::Medium,
                "No reachable test oracle found",
            ),
            StageEvidence::new(
                StageState::No,
                Confidence::Medium,
                "No assertion can discriminate the changed behavior without a reachable test",
            ),
            Vec::new(),
        );
    }

    let analysis = analyze_related_assertions(probe, related_tests);
    let related = finalize_related_tests(analysis.related);
    let observe = build_observe_evidence(analysis.matched_any);
    let discriminate =
        build_discriminate_evidence(&analysis.strongest, &analysis.strongest_kind, &probe.family);

    (observe, discriminate, related)
}

struct RevealAssertionAnalysis {
    related: Vec<RelatedTest>,
    strongest: OracleStrength,
    strongest_kind: OracleKind,
    matched_any: bool,
}

fn analyze_related_assertions(
    probe: &Probe,
    related_tests: &[&TestSummary],
) -> RevealAssertionAnalysis {
    let probe_tokens = extract_identifier_tokens(&probe.expression);
    let mut related = Vec::new();
    let mut strongest = OracleStrength::None;
    let mut strongest_kind = OracleKind::Unknown;
    let mut matched_any = false;

    for test in related_tests {
        if test.assertions.is_empty() {
            related.push(RelatedTest {
                name: test.name.clone(),
                file: test.file.clone(),
                line: test.start_line,
                oracle: None,
                oracle_kind: OracleKind::Unknown,
                oracle_strength: OracleStrength::None,
            });
            continue;
        }
        for assertion in &test.assertions {
            if assertion_matches_probe(
                &probe_tokens,
                &probe.family,
                assertion,
                test.assertions.len(),
            ) {
                matched_any = true;
                let relative_strength = probe_relative_oracle_strength(&probe.family, assertion);
                if relative_strength.rank() > strongest.rank() {
                    strongest = relative_strength.clone();
                    strongest_kind = assertion.kind.clone();
                }
                related.push(RelatedTest {
                    name: test.name.clone(),
                    file: test.file.clone(),
                    line: test.start_line,
                    oracle: Some(assertion.text.clone()),
                    oracle_kind: assertion.kind.clone(),
                    oracle_strength: relative_strength,
                });
            }
        }
    }

    RevealAssertionAnalysis {
        related,
        strongest,
        strongest_kind,
        matched_any,
    }
}

fn assertion_matches_probe(
    probe_tokens: &[String],
    family: &ProbeFamily,
    assertion: &OracleFact,
    assertion_count: usize,
) -> bool {
    let token_match = probe_tokens
        .iter()
        .any(|token| token.len() > 3 && assertion.text.contains(token));
    let family_match = oracle_matches_family(family, assertion);
    token_match || family_match || assertion_count == 1
}

fn finalize_related_tests(mut related: Vec<RelatedTest>) -> Vec<RelatedTest> {
    related.sort_by(|a, b| a.name.cmp(&b.name).then(a.line.cmp(&b.line)));
    related.dedup_by(|a, b| a.name == b.name && a.oracle == b.oracle);
    related
}

fn build_observe_evidence(matched_any: bool) -> StageEvidence {
    if matched_any {
        StageEvidence::new(
            StageState::Yes,
            Confidence::Medium,
            "A related test observes a value or effect near the changed behavior",
        )
    } else {
        StageEvidence::new(
            StageState::No,
            Confidence::Medium,
            "Related tests were found, but no assertion appears to observe the changed value, error, field, or effect",
        )
    }
}

fn build_discriminate_evidence(
    strongest: &OracleStrength,
    strongest_kind: &OracleKind,
    family: &ProbeFamily,
) -> StageEvidence {
    match strongest {
        OracleStrength::Strong => StageEvidence::new(
            StageState::Yes,
            Confidence::Medium,
            match strongest_kind {
                OracleKind::ExactErrorVariant => {
                    "Strong oracle found: exact error variant assertion"
                }
                OracleKind::WholeObjectEquality => {
                    "Strong oracle found: whole-object equality assertion"
                }
                _ => "Strong oracle found: exact value or pattern assertion",
            },
        ),
        OracleStrength::Medium => StageEvidence::new(
            StageState::Weak,
            Confidence::Medium,
            match strongest_kind {
                OracleKind::Snapshot => {
                    "Medium oracle found: snapshot assertion observes the changed behavior"
                }
                OracleKind::MockExpectation => {
                    "Medium oracle found: mock or expectation observes the changed behavior"
                }
                _ => "Medium oracle found: property or partial structural assertion",
            },
        ),
        OracleStrength::Weak => StageEvidence::new(
            StageState::Weak,
            Confidence::High,
            match (strongest_kind, family) {
                (OracleKind::BroadError, ProbeFamily::ErrorPath) => {
                    "Only broad error oracle found; is_err() does not discriminate exact error variants"
                }
                (OracleKind::BroadError, _) => {
                    "Only broad error oracle found; it may not discriminate the changed behavior exactly"
                }
                (OracleKind::RelationalCheck, _) => {
                    "Only relational oracle found; it may not discriminate the changed value exactly"
                }
                _ => {
                    "Only weak oracle found, such as a broad relational assertion or non-empty check"
                }
            },
        ),
        OracleStrength::Smoke => StageEvidence::new(
            StageState::Weak,
            Confidence::High,
            "Only smoke oracle found, such as unwrap/expect or execution without a discriminator",
        ),
        OracleStrength::None => StageEvidence::new(
            StageState::No,
            Confidence::Medium,
            "No assertion found on related tests",
        ),
        OracleStrength::Unknown => StageEvidence::new(
            StageState::Unknown,
            Confidence::Low,
            "Assertions exist, but oracle strength is unknown",
        ),
    }
}

fn oracle_matches_family(family: &ProbeFamily, assertion: &OracleFact) -> bool {
    let text = assertion.text.as_str();
    match family {
        ProbeFamily::ErrorPath => {
            matches!(
                assertion.kind,
                OracleKind::ExactErrorVariant | OracleKind::BroadError
            ) || text.contains("Err")
                || text.contains("Error::")
        }
        ProbeFamily::SideEffect => {
            matches!(assertion.kind, OracleKind::MockExpectation)
                || text.contains("expect")
                || text.contains("mock")
                || text.contains("saved")
                || text.contains("published")
        }
        ProbeFamily::FieldConstruction => {
            matches!(
                assertion.kind,
                OracleKind::ExactValue
                    | OracleKind::WholeObjectEquality
                    | OracleKind::RelationalCheck
                    | OracleKind::Snapshot
            ) || text.contains('.')
        }
        ProbeFamily::Predicate => {
            matches!(
                assertion.kind,
                OracleKind::ExactValue
                    | OracleKind::RelationalCheck
                    | OracleKind::ExactErrorVariant
                    | OracleKind::Snapshot
            )
        }
        ProbeFamily::ReturnValue => {
            matches!(
                assertion.kind,
                OracleKind::ExactValue
                    | OracleKind::WholeObjectEquality
                    | OracleKind::RelationalCheck
                    | OracleKind::Snapshot
                    | OracleKind::SmokeOnly
            )
        }
        ProbeFamily::CallDeletion => {
            matches!(
                assertion.kind,
                OracleKind::MockExpectation
                    | OracleKind::ExactValue
                    | OracleKind::RelationalCheck
                    | OracleKind::SmokeOnly
            ) || text.contains("assert")
                || text.contains("expect")
        }
        ProbeFamily::MatchArm => {
            matches!(
                assertion.kind,
                OracleKind::ExactErrorVariant
                    | OracleKind::ExactValue
                    | OracleKind::RelationalCheck
                    | OracleKind::Snapshot
            )
        }
        ProbeFamily::StaticUnknown => false,
    }
}

fn probe_relative_oracle_strength(family: &ProbeFamily, assertion: &OracleFact) -> OracleStrength {
    match family {
        ProbeFamily::ErrorPath => match assertion.kind {
            OracleKind::ExactErrorVariant => OracleStrength::Strong,
            OracleKind::BroadError => OracleStrength::Weak,
            OracleKind::SmokeOnly => OracleStrength::Smoke,
            _ => assertion.strength.clone(),
        },
        ProbeFamily::ReturnValue
        | ProbeFamily::Predicate
        | ProbeFamily::FieldConstruction
        | ProbeFamily::MatchArm => match assertion.kind {
            OracleKind::ExactValue
            | OracleKind::ExactErrorVariant
            | OracleKind::WholeObjectEquality => OracleStrength::Strong,
            OracleKind::Snapshot | OracleKind::MockExpectation => OracleStrength::Medium,
            OracleKind::RelationalCheck | OracleKind::BroadError => OracleStrength::Weak,
            OracleKind::SmokeOnly => OracleStrength::Smoke,
            OracleKind::Unknown => OracleStrength::Unknown,
        },
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => match assertion.kind {
            OracleKind::MockExpectation => OracleStrength::Medium,
            OracleKind::ExactValue | OracleKind::WholeObjectEquality => OracleStrength::Strong,
            OracleKind::RelationalCheck | OracleKind::BroadError => OracleStrength::Weak,
            OracleKind::SmokeOnly => OracleStrength::Smoke,
            OracleKind::ExactErrorVariant => OracleStrength::Medium,
            OracleKind::Snapshot => OracleStrength::Medium,
            OracleKind::Unknown => OracleStrength::Unknown,
        },
        ProbeFamily::StaticUnknown => OracleStrength::Unknown,
    }
}

fn classify(
    reach: &StageEvidence,
    infect: &StageEvidence,
    propagate: &StageEvidence,
    observe: &StageEvidence,
    discriminate: &StageEvidence,
    probe: &Probe,
) -> ExposureClass {
    if matches!(probe.family, ProbeFamily::StaticUnknown) {
        return ExposureClass::StaticUnknown;
    }
    if reach.state == StageState::No {
        return ExposureClass::NoStaticPath;
    }
    if infect.state == StageState::Unknown || infect.state == StageState::Opaque {
        return ExposureClass::InfectionUnknown;
    }
    if propagate.state == StageState::Unknown || propagate.state == StageState::Opaque {
        return ExposureClass::PropagationUnknown;
    }
    if observe.state == StageState::No {
        return ExposureClass::ReachableUnrevealed;
    }
    if discriminate.state == StageState::Yes
        && infect.state == StageState::Yes
        && propagate.state == StageState::Yes
    {
        ExposureClass::Exposed
    } else {
        ExposureClass::WeaklyExposed
    }
}

fn confidence_score(
    reach: &StageEvidence,
    infect: &StageEvidence,
    propagate: &StageEvidence,
    observe: &StageEvidence,
    discriminate: &StageEvidence,
    class: &ExposureClass,
) -> f32 {
    let states = [
        &reach.state,
        &infect.state,
        &propagate.state,
        &observe.state,
        &discriminate.state,
    ];
    let mut score = 0.0;
    for state in states {
        score += match state {
            StageState::Yes => 0.2,
            StageState::Weak => 0.12,
            StageState::Unknown => 0.07,
            StageState::Opaque => 0.05,
            StageState::No => 0.02,
            StageState::NotApplicable => 0.1,
        };
    }
    if matches!(
        class,
        ExposureClass::NoStaticPath | ExposureClass::ReachableUnrevealed
    ) {
        score = (score + 0.15_f32).min(0.95_f32);
    }
    (score * 100.0).round() / 100.0
}

fn missing_evidence(
    probe: &Probe,
    class: &ExposureClass,
    infect: &StageEvidence,
    observe: &StageEvidence,
    discriminate: &StageEvidence,
    activation: &ActivationEvidence,
) -> Vec<String> {
    let mut missing = Vec::new();
    match class {
        ExposureClass::Exposed => {}
        ExposureClass::NoStaticPath => {
            missing.push("No static test path reaches the changed owner".to_string())
        }
        ExposureClass::ReachableUnrevealed => missing.push(
            "No detected assertion observes the changed value, error, field, or effect".to_string(),
        ),
        ExposureClass::InfectionUnknown => missing.push(infect.summary.clone()),
        ExposureClass::PropagationUnknown => missing.push(
            "No clear propagation path from changed behavior to an observable sink".to_string(),
        ),
        ExposureClass::StaticUnknown => missing.push(
            "Syntax-first analysis cannot classify this change; use deep mode or real mutation"
                .to_string(),
        ),
        ExposureClass::WeaklyExposed => {}
    }
    if matches!(probe.family, ProbeFamily::Predicate)
        && infect.state != StageState::Yes
        && !activation
            .missing_discriminators
            .iter()
            .any(|fact| fact.value.contains("=="))
    {
        missing.push("No detected boundary input for the changed predicate".to_string());
    }
    if observe.state != StageState::Yes {
        missing.push("No relevant oracle was detected".to_string());
    }
    if discriminate.state != StageState::Yes {
        if matches!(probe.family, ProbeFamily::ErrorPath) {
            missing.push("No exact error variant discriminator was detected".to_string());
        } else {
            missing.push("No strong discriminator was detected".to_string());
        }
    }
    missing.extend(
        activation
            .missing_discriminators
            .iter()
            .map(|fact| format!("Missing discriminator value: {}", fact.value)),
    );
    missing.sort();
    missing.dedup();
    missing
}

fn stop_reasons(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    related_tests: &[&TestSummary],
) -> Vec<StopReason> {
    let mut reasons = Vec::new();
    if owner_fn.is_none() {
        reasons.push(StopReason::NoChangedRustLine);
    }
    if related_tests.iter().any(|test| {
        test.body.contains("fixture") || test.body.contains("builder") || test.body.contains("arb_")
    }) {
        reasons.push(StopReason::FixtureOpaque);
    }
    if probe.expression.contains("async")
        || probe.expression.contains("spawn")
        || probe.expression.contains("await")
    {
        reasons.push(StopReason::AsyncBoundaryOpaque);
    }
    if contains_macro_invocation(&probe.expression) {
        reasons.push(StopReason::ProcMacroOpaque);
    }
    reasons.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    reasons.dedup_by(|a, b| a.as_str() == b.as_str());
    reasons
}

fn contains_macro_invocation(expression: &str) -> bool {
    for (idx, ch) in expression.char_indices() {
        if ch != '!' || expression[idx + 1..].starts_with('=') {
            continue;
        }
        let before_bang = expression[..idx].trim_end();
        if before_bang
            .chars()
            .last()
            .is_some_and(|ch| ch == '_' || ch == ')' || ch.is_ascii_alphanumeric())
        {
            return true;
        }
    }
    false
}

fn recommended_next_step(probe: &Probe, class: &ExposureClass) -> Option<String> {
    match class {
        ExposureClass::Exposed => None,
        ExposureClass::WeaklyExposed => Some(match probe.family {
            ProbeFamily::Predicate => "Add boundary tests for below, equal, and above the changed threshold with exact assertions.".to_string(),
            ProbeFamily::ErrorPath => "Assert the exact error variant or payload instead of only is_err().".to_string(),
            ProbeFamily::SideEffect => "Add a mock expectation, event receiver assertion, persisted-state check, or metric assertion for the changed effect.".to_string(),
            ProbeFamily::ReturnValue => "Replace broad assertions with exact equality or a property that constrains the changed returned value.".to_string(),
            _ => "Strengthen the related assertion so it discriminates the changed behavior.".to_string(),
        }),
        ExposureClass::ReachableUnrevealed => Some("Add a meaningful assertion that observes the changed value, branch, error, field, event, or side effect.".to_string()),
        ExposureClass::NoStaticPath => Some("Add or identify a test path that reaches the changed owner, or run ready-mode mutation to confirm coverage.".to_string()),
        ExposureClass::InfectionUnknown => Some("Add a targeted boundary or negative-path test, or teach ripr about the fixture/builder in ripr.toml.".to_string()),
        ExposureClass::PropagationUnknown | ExposureClass::StaticUnknown => Some("Escalate to real mutation testing or deep static analysis for this probe.".to_string()),
    }
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

fn package_prefix(path: &Path) -> Option<String> {
    let normalized = normalize_path(path);
    if let Some(rest) = normalized.strip_prefix("crates/")
        && let Some((crate_name, crate_relative)) = rest.split_once('/')
        && (crate_relative.starts_with("src/") || crate_relative.starts_with("tests/"))
    {
        return Some(format!("crates/{crate_name}/"));
    }
    for marker in ["/src/", "/tests/"] {
        if let Some(idx) = normalized.rfind(marker) {
            let prefix = &normalized[..idx];
            if prefix.is_empty() {
                return None;
            }
            return Some(format!("{prefix}/"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::rust_index::{CallFact, LiteralFact, OracleFact, ReturnFact};
    use std::path::PathBuf;

    #[test]
    fn given_owner_symbol_when_resolving_owner_then_matches_full_identity() {
        let crate_b_fn = function("crates/crate_b/src/lib.rs", "score");
        let crate_a_fn = function("crates/crate_a/src/lib.rs", "score");
        let index = RustIndex {
            functions: vec![crate_b_fn, crate_a_fn],
            tests: vec![
                test(
                    "crates/crate_b/tests/score.rs",
                    "crate_b_score_test",
                    "score(2)",
                    "assert_eq!(score(2), 3);",
                ),
                test(
                    "crates/crate_a/tests/score.rs",
                    "crate_a_score_test",
                    "score(1)",
                    "assert_eq!(score(1), 2);",
                ),
            ],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:crate_a:score".to_string()),
            location: SourceLocation::new("crates/crate_a/src/lib.rs", 2, 1),
            owner: Some(SymbolId("crates/crate_a/src/lib.rs::score".to_string())),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: None,
            after: Some("score + 1".to_string()),
            expression: "score + 1".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.related_tests.len(), 1);
        assert_eq!(finding.related_tests[0].name, "crate_a_score_test");
    }

    #[test]
    fn given_unrelated_test_mentions_probe_token_when_owner_is_not_called_then_no_static_path() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "discounted_total")],
            tests: vec![TestSummary {
                name: "token_label_includes_token_text".to_string(),
                file: PathBuf::from("tests/tokens.rs"),
                start_line: 1,
                end_line: 4,
                body: "token_label(\"discount_threshold\");\nassert_eq!(token_label(\"discount_threshold\"), \"token:discount_threshold\");".to_string(),
                calls: vec![CallFact {
                    line: 1,
                    name: "token_label".to_string(),
                    text: "token_label(\"discount_threshold\")".to_string(),
                }],
                assertions: vec![oracle_fact(
                    "assert_eq!(token_label(\"discount_threshold\"), \"token:discount_threshold\");",
                    OracleKind::ExactValue,
                    OracleStrength::Strong,
                )],
                literals: vec![LiteralFact {
                    line: 1,
                    value: "\"discount_threshold\"".to_string(),
                }],
            }],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::discounted_total".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("amount >= discount_threshold".to_string()),
            expression: "amount >= discount_threshold".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.class, ExposureClass::NoStaticPath);
        assert_eq!(finding.ripr.reach.state, StageState::No);
        assert!(finding.related_tests.is_empty());
    }

    #[test]
    fn given_three_character_probe_token_in_test_name_when_owner_is_not_called_then_test_is_related()
     {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "tax_total")],
            tests: vec![TestSummary {
                name: "vat_boundary_is_checked_by_macro".to_string(),
                file: PathBuf::from("tests/tax.rs"),
                start_line: 1,
                end_line: 4,
                body: "assert_eq!(macro_tax_case!(100), 120);".to_string(),
                calls: vec![CallFact {
                    line: 1,
                    name: "macro_tax_case".to_string(),
                    text: "macro_tax_case!(100)".to_string(),
                }],
                assertions: vec![oracle_fact(
                    "assert_eq!(macro_tax_case!(100), 120);",
                    OracleKind::ExactValue,
                    OracleStrength::Strong,
                )],
                literals: vec![LiteralFact {
                    line: 1,
                    value: "100".to_string(),
                }],
            }],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::tax_total".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("vat >= threshold".to_string()),
            expression: "vat >= threshold".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.ripr.reach.state, StageState::Yes);
        assert_eq!(finding.related_tests.len(), 1);
        assert_eq!(
            finding.related_tests[0].name,
            "vat_boundary_is_checked_by_macro"
        );
    }

    #[test]
    fn given_workspace_paths_when_extracting_package_prefix_then_handles_nested_markers() {
        assert_eq!(
            package_prefix(Path::new("crates/foo/src/support/src/lib.rs")).as_deref(),
            Some("crates/foo/")
        );
        assert_eq!(
            package_prefix(Path::new("crates/foo/tests/support/tests/cases.rs")).as_deref(),
            Some("crates/foo/")
        );
        assert_eq!(
            package_prefix(Path::new("vendor/foo/src/support/src/lib.rs")).as_deref(),
            Some("vendor/foo/src/support/")
        );
        assert_eq!(
            package_prefix(Path::new("crates/ripr/examples/sample/src/lib.rs")).as_deref(),
            Some("crates/ripr/examples/sample/")
        );
    }

    #[test]
    fn given_non_workspace_paths_when_extracting_package_prefix_then_returns_none() {
        assert_eq!(package_prefix(Path::new("src/lib.rs")), None);
        assert_eq!(package_prefix(Path::new("tests/basic.rs")), None);
        assert_eq!(package_prefix(Path::new("README.md")), None);
    }

    #[test]
    fn given_mixed_separator_path_when_normalizing_then_uses_workspace_relative_form() {
        let normalized = normalize_path(Path::new("./crates\\ripr\\src\\lib.rs"));
        assert_eq!(normalized, "crates/ripr/src/lib.rs");
    }

    #[test]
    fn given_infection_unknown_probe_when_classified_then_stop_reason_is_present() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "price")],
            tests: vec![test(
                "tests/pricing.rs",
                "price_test",
                "price(1)",
                "assert_eq!(price(1), 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::price".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("amount >= threshold".to_string()),
            expression: "amount >= threshold".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.class, ExposureClass::InfectionUnknown);
        assert!(finding.unknown_has_stop_reason());
        assert!(
            finding
                .stop_reasons
                .iter()
                .any(|reason| { reason.as_str() == StopReason::InfectionEvidenceUnknown.as_str() })
        );
    }

    #[test]
    fn given_propagation_unknown_probe_when_classified_then_stop_reason_is_present() {
        let function = FunctionSummary {
            body: "value".to_string(),
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/score.rs",
                "score_test",
                "score(1)",
                "assert_eq!(score(1), 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:return_value".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: None,
            after: Some("value".to_string()),
            expression: "value".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.class, ExposureClass::PropagationUnknown);
        assert!(finding.unknown_has_stop_reason());
        assert!(
            finding.stop_reasons.iter().any(|reason| {
                reason.as_str() == StopReason::PropagationEvidenceUnknown.as_str()
            })
        );
    }

    #[test]
    fn given_static_unknown_probe_when_classified_then_stop_reason_is_present() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test(
                "tests/score.rs",
                "score_test",
                "score(1)",
                "assert_eq!(score(1), 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:static_unknown".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::StaticUnknown,
            delta: DeltaKind::Unknown,
            before: None,
            after: Some("score!(1)".to_string()),
            expression: "score".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.class, ExposureClass::StaticUnknown);
        assert!(finding.unknown_has_stop_reason());
        assert!(
            finding
                .stop_reasons
                .iter()
                .any(|reason| { reason.as_str() == StopReason::StaticProbeUnknown.as_str() })
        );
    }

    #[test]
    fn given_exact_error_variant_assertion_when_error_path_probe_changes_then_oracle_is_strong() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test_with_oracle(
                "tests/errors.rs",
                "revoked_token_is_exact",
                "score(\"\")",
                oracle_fact(
                    "assert_matches!(score(\"\"), Err(AuthError::RevokedToken));",
                    OracleKind::ExactErrorVariant,
                    OracleStrength::Strong,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:error_path".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ErrorPath,
            delta: DeltaKind::Control,
            before: None,
            after: Some("Err(AuthError::RevokedToken)".to_string()),
            expression: "Err(AuthError::RevokedToken)".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Yes);
        assert_eq!(
            finding.ripr.reveal.discriminate.summary,
            "Strong oracle found: exact error variant assertion"
        );
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Strong
        );
    }

    #[test]
    fn given_broad_is_err_assertion_when_error_variant_changes_then_oracle_is_weak() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test_with_oracle(
                "tests/errors.rs",
                "revoked_token_is_broad",
                "score(\"\")",
                oracle_fact(
                    "assert!(score(\"\").is_err());",
                    OracleKind::BroadError,
                    OracleStrength::Weak,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:error_path".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ErrorPath,
            delta: DeltaKind::Control,
            before: None,
            after: Some("Err(AuthError::RevokedToken)".to_string()),
            expression: "Err(AuthError::RevokedToken)".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Weak);
        assert_eq!(
            finding.ripr.reveal.discriminate.summary,
            "Only broad error oracle found; is_err() does not discriminate exact error variants"
        );
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Weak
        );
        assert!(
            finding
                .missing
                .iter()
                .any(|missing| { missing == "No exact error variant discriminator was detected" })
        );
    }

    #[test]
    fn given_unwrap_only_test_when_return_value_probe_changes_then_oracle_is_smoke() {
        let unwrap_only = format!("score(1).{}();", "unwrap");
        let index = RustIndex {
            functions: vec![FunctionSummary {
                body: "pub fn score(input: i32) -> Result<i32, Error> { Ok(input) }".to_string(),
                ..function("src/lib.rs", "score")
            }],
            tests: vec![test_with_oracle(
                "tests/score.rs",
                "score_smoke",
                "score(1)",
                oracle_fact(&unwrap_only, OracleKind::SmokeOnly, OracleStrength::Smoke),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:return_value".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: None,
            after: Some("return Ok(input + 1)".to_string()),
            expression: "return Ok(input + 1)".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Weak);
        assert_eq!(
            finding.ripr.reveal.discriminate.summary,
            "Only smoke oracle found, such as unwrap/expect or execution without a discriminator"
        );
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Smoke
        );
    }

    #[test]
    fn given_broad_error_assertion_when_non_error_probe_changes_then_gap_stays_generic() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test_with_oracle(
                "tests/score.rs",
                "score_call_is_broad",
                "score(1)",
                oracle_fact(
                    "assert!(score(1).is_err());",
                    OracleKind::BroadError,
                    OracleStrength::Weak,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:call_deletion".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::CallDeletion,
            delta: DeltaKind::Effect,
            before: None,
            after: Some("client.send(input)".to_string()),
            expression: "client.send(input)".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Weak);
        assert_eq!(
            finding.ripr.reveal.discriminate.summary,
            "Only broad error oracle found; it may not discriminate the changed behavior exactly"
        );
        assert!(
            finding
                .missing
                .iter()
                .any(|missing| { missing == "No strong discriminator was detected" })
        );
        assert!(
            !finding
                .missing
                .iter()
                .any(|missing| { missing == "No exact error variant discriminator was detected" })
        );
    }

    #[test]
    fn given_changed_predicate_when_branch_returns_value_then_flow_sink_is_return_value() {
        let function = FunctionSummary {
            body: r#"pub fn score(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}"#
            .to_string(),
            start_line: 1,
            end_line: 7,
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/score.rs",
                "score_threshold",
                "score(100, 50)",
                "assert_eq!(score(100, 50), 90);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("amount >= threshold".to_string()),
            expression: "amount >= threshold".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ReturnValue);
        assert_eq!(finding.flow_sinks[0].text, "amount - 10");
        assert_eq!(finding.flow_sinks[0].line, 3);
        assert_eq!(
            finding.ripr.propagate.summary,
            "Changed behavior appears to influence returned value: amount - 10"
        );
    }

    #[test]
    fn given_changed_error_variant_when_result_err_is_returned_then_flow_sink_is_error_variant() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test_with_oracle(
                "tests/errors.rs",
                "revoked_token_is_broad",
                "score(\"\")",
                oracle_fact(
                    "assert!(score(\"\").is_err());",
                    OracleKind::BroadError,
                    OracleStrength::Weak,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:error_path".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ErrorPath,
            delta: DeltaKind::Value,
            before: None,
            after: Some("return Err(AuthError::RevokedToken);".to_string()),
            expression: "return Err(AuthError::RevokedToken);".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ErrorVariant);
        assert_eq!(
            finding.flow_sinks[0].text,
            "Result::Err(AuthError::RevokedToken)"
        );
    }

    #[test]
    fn given_changed_side_effect_call_when_effect_method_is_called_then_flow_sink_is_call_effect() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test(
                "tests/score.rs",
                "score_publishes",
                "score(1)",
                "assert_eq!(score(1), 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:side_effect".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::SideEffect,
            delta: DeltaKind::Effect,
            before: None,
            after: Some("events.publish(score)".to_string()),
            expression: "events.publish(score)".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::CallEffect);
        assert_eq!(finding.flow_sinks[0].text, "events.publish(score)");
    }

    #[test]
    fn given_changed_field_construction_when_field_is_assigned_then_flow_sink_is_struct_field() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test(
                "tests/score.rs",
                "score_builds_field",
                "score(1)",
                "assert_eq!(score(1).total, 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:field_construction".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::FieldConstruction,
            delta: DeltaKind::Value,
            before: None,
            after: Some("total: computed_total".to_string()),
            expression: "total: computed_total".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::StructField);
        assert_eq!(finding.flow_sinks[0].text, "total: computed_total");
    }

    #[test]
    fn given_changed_match_arm_when_arm_returns_value_then_flow_sink_is_match_arm_return() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test(
                "tests/score.rs",
                "score_matches",
                "score(1)",
                "assert_eq!(score(1), 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:match_arm".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::MatchArm,
            delta: DeltaKind::Control,
            before: None,
            after: Some("Some(value) => value + 1,".to_string()),
            expression: "Some(value) => value + 1,".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::MatchArm);
        assert_eq!(finding.flow_sinks[0].text, "value + 1");
    }

    #[test]
    fn given_changed_return_binding_when_function_returns_ok_then_flow_sink_is_return_value() {
        let function = FunctionSummary {
            body: "pub fn score(input: i32) -> Result<i32, Error> { let value = input + 1; Ok(value) }"
                .to_string(),
            returns: vec![ReturnFact {
                line: 1,
                text: "Ok(value)".to_string(),
            }],
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/score.rs",
                "score_returns_value",
                "score(1)",
                "assert_eq!(score(1), Ok(2));",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:1:return_value".to_string()),
            location: SourceLocation::new("src/lib.rs", 1, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: None,
            after: Some("let value = input + 1".to_string()),
            expression: "let value = input + 1".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ReturnValue);
        assert_eq!(finding.flow_sinks[0].text, "Ok(value)");
    }

    #[test]
    fn given_changed_predicate_when_branch_returns_error_then_flow_sink_is_error_variant() {
        let function = FunctionSummary {
            body: r#"pub fn authenticate(token: &str) -> Result<User, AuthError> {
    if token.is_empty() {
        return Err(AuthError::RevokedToken);
    }
    Ok(User)
}"#
            .to_string(),
            start_line: 1,
            end_line: 6,
            returns: vec![
                ReturnFact {
                    line: 3,
                    text: "return Err(AuthError::RevokedToken);".to_string(),
                },
                ReturnFact {
                    line: 5,
                    text: "Ok(User)".to_string(),
                },
            ],
            ..function("src/lib.rs", "authenticate")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test_with_oracle(
                "tests/auth.rs",
                "empty_token_is_rejected",
                "authenticate(\"\")",
                oracle_fact(
                    "assert!(authenticate(\"\").is_err());",
                    OracleKind::BroadError,
                    OracleStrength::Weak,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::authenticate".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("token.is_empty()".to_string()),
            expression: "token.is_empty()".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ErrorVariant);
        assert_eq!(
            finding.flow_sinks[0].text,
            "Result::Err(AuthError::RevokedToken)"
        );
    }

    #[test]
    fn given_changed_predicate_when_branch_constructs_field_then_flow_sink_is_struct_field() {
        let function = FunctionSummary {
            body: r#"pub fn quote(amount: i32) -> Quote {
    if amount > 0 {
        Quote {
            total: amount - 10,
        }
    } else {
        Quote {
            total: amount,
        }
    }
}"#
            .to_string(),
            start_line: 1,
            end_line: 11,
            ..function("src/lib.rs", "quote")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/quote.rs",
                "positive_quote_has_total",
                "quote(100)",
                "assert_eq!(quote(100).total, 90);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::quote".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("amount > 0".to_string()),
            expression: "amount > 0".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::StructField);
        assert_eq!(finding.flow_sinks[0].text, "total: amount - 10");
    }

    #[test]
    fn given_changed_predicate_when_return_contains_colon_in_string_then_flow_sink_is_return_value()
    {
        let function = FunctionSummary {
            body: r#"pub fn message(code: i32) -> String {
    if code > 0 {
        format!("error:{code}")
    } else {
        "ok".to_string()
    }
}"#
            .to_string(),
            start_line: 1,
            end_line: 7,
            ..function("src/lib.rs", "message")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/message.rs",
                "message_returns_error_code",
                "message(1)",
                "assert_eq!(message(1), \"error:1\");",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::message".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("code > 0".to_string()),
            expression: "code > 0".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ReturnValue);
        assert_eq!(finding.flow_sinks[0].text, "format!(\"error:{code}\")");
    }

    #[test]
    fn given_changed_predicate_when_next_line_is_assignment_then_flow_sink_stays_unknown() {
        let function = FunctionSummary {
            body: r#"pub fn score(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        let discounted = amount - 10;
        discounted
    } else {
        amount
    }
}"#
            .to_string(),
            start_line: 1,
            end_line: 8,
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/score.rs",
                "score_threshold",
                "score(100, 50)",
                "assert_eq!(score(100, 50), 90);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("amount >= threshold".to_string()),
            expression: "amount >= threshold".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert!(finding.flow_sinks.is_empty());
        assert_eq!(finding.ripr.propagate.state, StageState::Unknown);
    }

    #[test]
    fn given_changed_return_after_early_return_when_no_downstream_return_exists_then_sink_is_unknown()
     {
        let function = FunctionSummary {
            body: r#"pub fn score(amount: i32) -> i32 {
    if amount < 0 {
        return 0;
    }
    let adjusted = amount;
    adjusted
}"#
            .to_string(),
            start_line: 1,
            end_line: 7,
            returns: vec![ReturnFact {
                line: 3,
                text: "return 0;".to_string(),
            }],
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/score.rs",
                "score_positive",
                "score(1)",
                "assert_eq!(score(1), 1);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:5:return_value".to_string()),
            location: SourceLocation::new("src/lib.rs", 5, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: None,
            after: Some("adjusted".to_string()),
            expression: "adjusted".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::Unknown);
        assert_eq!(finding.ripr.propagate.state, StageState::Unknown);
    }

    #[test]
    fn given_changed_call_deletion_when_result_ok_is_returned_then_flow_sink_is_return_value() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test(
                "tests/score.rs",
                "score_returns_value",
                "score(1)",
                "assert_eq!(score(1), Ok(2));",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:call".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::CallDeletion,
            delta: DeltaKind::Effect,
            before: None,
            after: Some("Ok(total)".to_string()),
            expression: "Ok(total)".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ReturnValue);
        assert_eq!(finding.flow_sinks[0].text, "Ok(total)");
    }

    #[test]
    fn given_changed_match_arm_when_arm_returns_error_then_flow_sink_is_error_variant() {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "authenticate")],
            tests: vec![test_with_oracle(
                "tests/auth.rs",
                "revoked_token_is_exact",
                "authenticate(\"\")",
                oracle_fact(
                    "assert_matches!(authenticate(\"\"), Err(AuthError::RevokedToken));",
                    OracleKind::ExactErrorVariant,
                    OracleStrength::Strong,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:match_arm".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::authenticate".to_string())),
            family: ProbeFamily::MatchArm,
            delta: DeltaKind::Control,
            before: None,
            after: Some("None => Err(AuthError::RevokedToken),".to_string()),
            expression: "None => Err(AuthError::RevokedToken),".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::ErrorVariant);
        assert_eq!(
            finding.flow_sinks[0].text,
            "Result::Err(AuthError::RevokedToken)"
        );
    }

    #[test]
    fn given_changed_opaque_return_expression_when_no_sink_is_obvious_then_propagation_is_unknown()
    {
        let index = RustIndex {
            functions: vec![function("src/lib.rs", "score")],
            tests: vec![test(
                "tests/score.rs",
                "score_returns_value",
                "score(1)",
                "assert_eq!(score(1), 2);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:return_value".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ReturnValue,
            delta: DeltaKind::Value,
            before: None,
            after: Some("value".to_string()),
            expression: "value".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::Unknown);
        assert_eq!(finding.ripr.propagate.state, StageState::Unknown);
        assert_eq!(
            finding.ripr.propagate.summary,
            "Propagation is not statically obvious from syntax-first analysis"
        );
    }

    #[test]
    fn given_boundary_predicate_when_tests_skip_equal_value_then_activation_names_missing_boundary()
    {
        let function = FunctionSummary {
            body: r#"pub fn score(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}"#
            .to_string(),
            start_line: 1,
            end_line: 7,
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![
                test(
                    "tests/score.rs",
                    "below_threshold_has_no_discount",
                    "score(50, 100)",
                    "assert_eq!(score(50, 100), 50);",
                ),
                test(
                    "tests/score.rs",
                    "far_above_threshold_discounts",
                    "score(10_000, 100)",
                    "assert_eq!(score(10_000, 100), 9_990);",
                ),
            ],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("amount >= threshold".to_string()),
            expression: "amount >= threshold".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.class, ExposureClass::WeaklyExposed);
        assert_eq!(finding.ripr.infect.state, StageState::Weak);
        assert!(finding.activation.observed_values.iter().any(|fact| {
            fact.context == ValueContext::FunctionArgument && fact.value == "amount = 50"
        }));
        assert!(finding.activation.observed_values.iter().any(|fact| {
            fact.context == ValueContext::FunctionArgument && fact.value == "amount = 10_000"
        }));
        assert!(finding.activation.observed_values.iter().any(|fact| {
            fact.context == ValueContext::FunctionArgument && fact.value == "threshold = 100"
        }));
        assert_eq!(finding.activation.missing_discriminators.len(), 1);
        assert_eq!(
            finding.activation.missing_discriminators[0].value,
            "amount == threshold"
        );
        assert_eq!(
            finding.activation.missing_discriminators[0]
                .flow_sink
                .as_ref()
                .map(|sink| &sink.kind),
            Some(&FlowSinkKind::ReturnValue)
        );
    }

    #[test]
    fn given_boundary_predicate_when_equal_value_exists_then_activation_has_no_missing_boundary() {
        let function = FunctionSummary {
            body: r#"pub fn score(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold {
        amount - 10
    } else {
        amount
    }
}"#
            .to_string(),
            start_line: 1,
            end_line: 7,
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test(
                "tests/score.rs",
                "equal_threshold_discounts",
                "score(100, 100)",
                "assert_eq!(score(100, 100), 90);",
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:2:predicate".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("amount >= threshold".to_string()),
            expression: "amount >= threshold".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert_eq!(finding.ripr.infect.state, StageState::Yes);
        assert!(finding.activation.missing_discriminators.is_empty());
        assert!(finding.activation.observed_values.iter().any(|fact| {
            fact.context == ValueContext::FunctionArgument && fact.value == "amount == threshold"
        }));
    }

    #[test]
    fn given_error_path_probe_when_test_uses_is_err_then_exact_error_variant_is_missing() {
        let function = FunctionSummary {
            body: r#"pub fn score(token: &str) -> Result<&'static str, AuthError> {
    if token.is_empty() {
        return Err(AuthError::RevokedToken);
    }
    Ok("accepted")
}"#
            .to_string(),
            start_line: 1,
            end_line: 6,
            returns: vec![
                ReturnFact {
                    line: 3,
                    text: "return Err(AuthError::RevokedToken);".to_string(),
                },
                ReturnFact {
                    line: 5,
                    text: "Ok(\"accepted\")".to_string(),
                },
            ],
            ..function("src/lib.rs", "score")
        };
        let index = RustIndex {
            functions: vec![function],
            tests: vec![test_with_oracle(
                "tests/errors.rs",
                "empty_token_is_rejected",
                "score(\"\")",
                oracle_fact(
                    "assert!(score(\"\").is_err());",
                    OracleKind::BroadError,
                    OracleStrength::Weak,
                ),
            )],
            ..RustIndex::default()
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib_rs:3:error_path".to_string()),
            location: SourceLocation::new("src/lib.rs", 3, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family: ProbeFamily::ErrorPath,
            delta: DeltaKind::Value,
            before: None,
            after: Some("return Err(AuthError::RevokedToken);".to_string()),
            expression: "return Err(AuthError::RevokedToken);".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };

        let finding = classify_probe(&probe, &index);

        assert!(finding.activation.observed_values.iter().any(|fact| {
            fact.context == ValueContext::FunctionArgument && fact.value == "token = \"\""
        }));
        assert_eq!(finding.activation.missing_discriminators.len(), 1);
        assert_eq!(
            finding.activation.missing_discriminators[0].value,
            "AuthError::RevokedToken"
        );
        assert_eq!(
            finding.activation.missing_discriminators[0]
                .flow_sink
                .as_ref()
                .map(|sink| &sink.kind),
            Some(&FlowSinkKind::ErrorVariant)
        );
    }

    #[test]
    fn given_table_rows_and_builder_calls_when_extracting_values_then_contexts_are_preserved() {
        let test = TestSummary {
            name: "table_and_builder".to_string(),
            file: PathBuf::from("tests/value.rs"),
            start_line: 10,
            end_line: 16,
            body: r#"let rows = [(99, 100), (100, 100)];
let input = Request::builder().amount(100).token("abc").build();
assert_eq!(input.amount, 100);"#
                .to_string(),
            calls: vec![],
            assertions: vec![oracle_fact(
                "assert_eq!(input.amount, 100);",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
            literals: vec![],
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
    fn given_probe_family_and_exposure_class_when_recommending_next_step_then_guidance_matches() {
        let predicate_probe = probe(ProbeFamily::Predicate, DeltaKind::Control, "value > 10");
        let return_value_probe = probe(ProbeFamily::ReturnValue, DeltaKind::Value, "value + 1");

        assert_eq!(
            recommended_next_step(&predicate_probe, &ExposureClass::Exposed),
            None
        );
        assert_eq!(
            recommended_next_step(&predicate_probe, &ExposureClass::WeaklyExposed).as_deref(),
            Some(
                "Add boundary tests for below, equal, and above the changed threshold with exact assertions."
            )
        );
        assert_eq!(
            recommended_next_step(&return_value_probe, &ExposureClass::WeaklyExposed).as_deref(),
            Some(
                "Replace broad assertions with exact equality or a property that constrains the changed returned value."
            )
        );
        assert_eq!(
            recommended_next_step(&predicate_probe, &ExposureClass::ReachableUnrevealed).as_deref(),
            Some(
                "Add a meaningful assertion that observes the changed value, branch, error, field, event, or side effect."
            )
        );
        assert_eq!(
            recommended_next_step(&predicate_probe, &ExposureClass::NoStaticPath).as_deref(),
            Some(
                "Add or identify a test path that reaches the changed owner, or run ready-mode mutation to confirm coverage."
            )
        );
        assert_eq!(
            recommended_next_step(&predicate_probe, &ExposureClass::InfectionUnknown).as_deref(),
            Some(
                "Add a targeted boundary or negative-path test, or teach ripr about the fixture/builder in ripr.toml."
            )
        );
        assert_eq!(
            recommended_next_step(&predicate_probe, &ExposureClass::StaticUnknown).as_deref(),
            Some("Escalate to real mutation testing or deep static analysis for this probe.")
        );
    }

    #[test]
    fn given_macro_like_expression_when_collecting_stop_reasons_then_ignores_inequality_tokens() {
        let inequality = probe(
            ProbeFamily::StaticUnknown,
            DeltaKind::Unknown,
            "value != threshold",
        );
        let unary_not = probe(ProbeFamily::StaticUnknown, DeltaKind::Unknown, "!enabled");
        let macro_with_inequality = probe(
            ProbeFamily::StaticUnknown,
            DeltaKind::Unknown,
            "value != threshold && trace!(value)",
        );

        assert_eq!(stop_reason_labels(&inequality), Vec::<&str>::new());
        assert_eq!(stop_reason_labels(&unary_not), Vec::<&str>::new());
        assert_eq!(
            stop_reason_labels(&macro_with_inequality),
            vec!["proc_macro_opaque"]
        );
    }

    #[test]
    fn given_duplicate_stop_reasons_when_collecting_then_results_are_deduplicated_and_sorted() {
        let probe = probe(
            ProbeFamily::StaticUnknown,
            DeltaKind::Unknown,
            "async move { spawn(task).await; trace!(task); }",
        );

        let labels = stop_reason_labels(&probe);
        assert_eq!(labels, vec!["async_boundary_opaque", "proc_macro_opaque"]);
    }

    #[test]
    fn stop_reasons_include_fixture_and_missing_owner_signals() {
        let probe = probe(
            ProbeFamily::CallDeletion,
            DeltaKind::Effect,
            "client.send(input)",
        );
        let fixture_test = test(
            "tests/service.rs",
            "service_uses_fixture",
            "score(1)",
            "let fixture = build_fixture(); assert_eq!(score(1), 2);",
        );

        let reasons = stop_reasons(&probe, None, &[&fixture_test]);
        let labels: Vec<&str> = reasons.iter().map(StopReason::as_str).collect();

        assert_eq!(labels, vec!["fixture_opaque", "no_changed_rust_line"]);
    }

    fn stop_reason_labels(probe: &Probe) -> Vec<&str> {
        let owner = function("crates/ripr/src/lib.rs", "dummy");
        let reasons = stop_reasons(probe, Some(&owner), &[]);
        let labels: Vec<&str> = reasons.iter().map(StopReason::as_str).collect();
        labels
    }

    fn probe(family: ProbeFamily, delta: DeltaKind, expression: &str) -> Probe {
        Probe {
            id: ProbeId("probe:test".to_string()),
            location: SourceLocation::new("crates/ripr/src/lib.rs", 1, 1),
            owner: None,
            family,
            delta,
            before: None,
            after: None,
            expression: expression.to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        }
    }

    fn function(file: &str, name: &str) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId(format!("{file}::{name}")),
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 3,
            body: format!("pub fn {name}(input: i32) -> i32 {{ input }}"),
            calls: vec![],
            returns: vec![],
            literals: vec![],
            is_test: false,
        }
    }

    fn test(file: &str, name: &str, call: &str, assertion: &str) -> TestSummary {
        test_with_oracle(
            file,
            name,
            call,
            oracle_fact(assertion, OracleKind::ExactValue, OracleStrength::Strong),
        )
    }

    fn test_with_oracle(file: &str, name: &str, call: &str, oracle: OracleFact) -> TestSummary {
        let body = format!("{call};\n{}", oracle.text.as_str());
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 4,
            body,
            calls: vec![CallFact {
                line: 1,
                name: "score".to_string(),
                text: call.to_string(),
            }],
            assertions: vec![oracle],
            literals: vec![LiteralFact {
                line: 1,
                value: "1".to_string(),
            }],
        }
    }

    fn oracle_fact(assertion: &str, kind: OracleKind, strength: OracleStrength) -> OracleFact {
        OracleFact {
            line: 2,
            text: assertion.to_string(),
            kind,
            strength,
            observed_tokens: extract_identifier_tokens(assertion),
        }
    }
}
