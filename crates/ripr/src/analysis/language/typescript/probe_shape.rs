//! Probe-shape classification for the TypeScript preview adapter.

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptProbeShape {
    pub(crate) family: ProbeFamily,
    pub(crate) delta: DeltaKind,
    pub(crate) specific: bool,
}

impl TypeScriptProbeShape {
    pub(crate) fn new(family: ProbeFamily, delta: DeltaKind) -> Self {
        Self {
            family,
            delta,
            specific: true,
        }
    }

    pub(crate) fn ambiguous_fallback() -> Self {
        Self {
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            specific: false,
        }
    }
}

/// Syntax-first probe-family classifier for a changed line of TypeScript
/// or JavaScript source.
///
/// Inspects the leading non-whitespace tokens of `line_text` and falls
/// back to substring shape checks for ternary / arrow-bodied expressions.
/// Matches the families documented in RIPR-SPEC-0027 and pinned by the
/// TypeScript probe-fixture family.
///
/// The adapter operates without a type checker, so ambiguous shapes keep
/// the historical `Predicate` / `Control` fallback but are marked
/// non-specific so later repair guidance does not invent discriminators.
pub(crate) fn classify_probe_shape_detail(line_text: &str) -> TypeScriptProbeShape {
    let trimmed = line_text.trim_start();
    // Strip a leading `} ` (e.g., `} else if (...)`, `} else {`) so the
    // dedicated-keyword check still fires on close-brace-continuation
    // shapes that are common in JavaScript-style if/else ladders.
    let leading = trimmed.strip_prefix("} ").unwrap_or(trimmed).trim_start();

    if leading.starts_with("throw ")
        || leading.starts_with("throw(")
        || leading.starts_with("return Promise.reject(")
        || leading.starts_with("return Promise.reject ")
        || leading.starts_with("return await Promise.reject(")
        || leading.starts_with("return await Promise.reject ")
        || leading.starts_with("await Promise.reject(")
        || leading.starts_with("await Promise.reject ")
        || leading.starts_with("} catch ")
        || leading.starts_with("catch ")
    {
        return TypeScriptProbeShape::new(ProbeFamily::ErrorPath, DeltaKind::Control);
    }
    if is_object_literal_return_line(leading) {
        return TypeScriptProbeShape::new(ProbeFamily::FieldConstruction, DeltaKind::Value);
    }
    if leading.starts_with("return ") || leading == "return;" || leading.starts_with("return;") {
        return TypeScriptProbeShape::new(ProbeFamily::ReturnValue, DeltaKind::Value);
    }
    if leading.starts_with("if (")
        || leading.starts_with("if(")
        || leading.starts_with("else if (")
        || leading.starts_with("else if(")
        || leading.starts_with("while (")
        || leading.starts_with("while(")
        || leading.starts_with("for (")
        || leading.starts_with("for(")
        || leading.starts_with("switch (")
        || leading.starts_with("switch(")
        || leading.starts_with("case ")
        || leading.starts_with("default:")
    {
        return TypeScriptProbeShape::new(ProbeFamily::Predicate, DeltaKind::Control);
    }
    // Top-level ternary or short-circuit expression that is *not* embedded
    // in a `return` or assignment — treat as a predicate boundary.
    if (leading.contains("? ") && leading.contains(" : "))
        && !leading.starts_with("const ")
        && !leading.starts_with("let ")
        && !leading.starts_with("var ")
    {
        return TypeScriptProbeShape::new(ProbeFamily::Predicate, DeltaKind::Control);
    }
    if is_object_literal_field_line(leading) {
        return TypeScriptProbeShape::new(ProbeFamily::FieldConstruction, DeltaKind::Value);
    }
    // Field / property assignments: `this.x = ...`, `obj.x = ...`, or
    // top-level binding declarations inside a constructor / setter body.
    // Detected only when the line has the form `<ident chain> = <expr>`
    // without a leading function-call shape; this keeps statement-level
    // call expressions in the SideEffect bucket below.
    if let Some(eq_idx) = leading.find(" = ")
        && !leading.starts_with("if ")
        && !leading.starts_with("else ")
        && !leading.starts_with("return")
        && !leading.starts_with("throw")
    {
        let lhs = &leading[..eq_idx];
        let looks_like_assignment = lhs
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '[' || c == ']');
        let looks_like_declaration =
            lhs.starts_with("const ") || lhs.starts_with("let ") || lhs.starts_with("var ");
        if looks_like_assignment && !looks_like_declaration {
            return TypeScriptProbeShape::new(ProbeFamily::FieldConstruction, DeltaKind::Value);
        }
    }
    // Bare call-expression statement (e.g., `tracker.record(event);`,
    // `await logger.flush();`). Detected by trailing `);` after stripping
    // optional `await ` / `void ` / trailing comments.
    let call_candidate = leading
        .strip_prefix("await ")
        .unwrap_or(leading)
        .strip_prefix("void ")
        .unwrap_or_else(|| leading.strip_prefix("await ").unwrap_or(leading))
        .trim_end();
    let call_candidate = call_candidate
        .strip_suffix(';')
        .unwrap_or(call_candidate)
        .trim_end();
    if call_candidate.ends_with(')')
        && call_candidate.contains('(')
        && !call_candidate.starts_with("if")
        && !call_candidate.starts_with("while")
        && !call_candidate.starts_with("for")
        && !call_candidate.starts_with("switch")
        && !call_candidate.starts_with("return")
        && !call_candidate.starts_with("throw")
        && !call_candidate.starts_with("const ")
        && !call_candidate.starts_with("let ")
        && !call_candidate.starts_with("var ")
    {
        return TypeScriptProbeShape::new(ProbeFamily::SideEffect, DeltaKind::Effect);
    }
    // Fall through conservatively. The adapter does not recognise this shape,
    // so flagging it as a generic predicate-control change avoids committing
    // to a more specific family the preview surface cannot confirm.
    TypeScriptProbeShape::ambiguous_fallback()
}

#[cfg(test)]
pub(crate) fn classify_probe_shape(line_text: &str) -> (ProbeFamily, DeltaKind) {
    let detail = classify_probe_shape_detail(line_text);
    (detail.family, detail.delta)
}

pub(crate) fn is_object_literal_return_line(line_text: &str) -> bool {
    let trimmed = line_text.trim_start();
    trimmed.starts_with("return {") || trimmed.starts_with("return ({")
}

fn is_object_literal_field_line(line_text: &str) -> bool {
    let trimmed = line_text.trim();
    if trimmed.starts_with("case ")
        || trimmed.starts_with("default:")
        || trimmed.starts_with("import ")
        || trimmed.starts_with("export ")
        || trimmed.ends_with(';')
        || trimmed.contains("=>")
    {
        return false;
    }
    let Some((key, rest)) = trimmed.split_once(':') else {
        return false;
    };
    let key = key.trim().trim_matches('"').trim_matches('\'');
    !key.is_empty()
        && !rest
            .trim_end_matches(',')
            .trim_end_matches('}')
            .trim()
            .is_empty()
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

pub(crate) fn typescript_flow_sink_for(
    probe_shape: &TypeScriptProbeShape,
    owner: &TypeScriptOwner,
    line: usize,
    line_text: &str,
) -> Option<FlowSinkFact> {
    if !probe_shape.specific {
        return None;
    }
    let kind = match probe_shape.family {
        ProbeFamily::ReturnValue => FlowSinkKind::ReturnValue,
        ProbeFamily::ErrorPath => FlowSinkKind::ErrorVariant,
        ProbeFamily::FieldConstruction => {
            if is_computed_field_construction(line_text) {
                return None;
            }
            FlowSinkKind::StructField
        }
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => {
            if is_computed_member_call(line_text) {
                return None;
            }
            FlowSinkKind::CallEffect
        }
        ProbeFamily::Predicate | ProbeFamily::MatchArm | ProbeFamily::StaticUnknown => {
            return None;
        }
    };

    Some(FlowSinkFact {
        kind,
        text: line_text.trim().to_string(),
        line,
        owner: Some(owner.symbol_id()),
    })
}

pub(crate) fn typescript_missing_discriminators(
    probe_shape: &TypeScriptProbeShape,
    line: usize,
    line_text: &str,
    flow_sink: Option<&FlowSinkFact>,
) -> Vec<MissingDiscriminatorFact> {
    if !probe_shape.specific {
        return Vec::new();
    }
    let Some(value) = typescript_missing_discriminator_value(&probe_shape.family, line_text) else {
        return Vec::new();
    };

    vec![MissingDiscriminatorFact {
        value,
        reason: typescript_missing_discriminator_reason(&probe_shape.family, line),
        flow_sink: flow_sink.cloned(),
    }]
}

pub(crate) fn typescript_missing_discriminator_reason(
    probe_family: &ProbeFamily,
    line: usize,
) -> String {
    let shape = match probe_family {
        ProbeFamily::Predicate => "equality-boundary",
        ProbeFamily::ReturnValue => "returned-value",
        ProbeFamily::ErrorPath => "thrown or rejected error",
        ProbeFamily::FieldConstruction => "field/object value",
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => "call side effect",
        ProbeFamily::MatchArm => "match-arm",
        ProbeFamily::StaticUnknown => "static",
    };
    format!("changed TypeScript {shape} at line {line} lacks a concrete preview discriminator")
}

pub(crate) fn typescript_missing_discriminator_value(
    probe_family: &ProbeFamily,
    line_text: &str,
) -> Option<String> {
    match probe_family {
        ProbeFamily::Predicate => typescript_boundary_discriminator(line_text),
        ProbeFamily::ReturnValue => typescript_return_value_discriminator(line_text),
        ProbeFamily::ErrorPath => typescript_error_path_discriminator(line_text),
        ProbeFamily::FieldConstruction => typescript_field_value_discriminator(line_text),
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => {
            typescript_call_effect_discriminator(line_text)
        }
        ProbeFamily::MatchArm | ProbeFamily::StaticUnknown => None,
    }
}

pub(crate) fn typescript_boundary_discriminator(line_text: &str) -> Option<String> {
    let expression = strip_typescript_control_prefix(line_text);
    for operator in ["===", "!==", ">=", "<=", "==", "!=", ">", "<"] {
        if let Some(idx) = expression.find(operator) {
            let left_raw = expression.get(..idx)?.trim();
            let right_raw = expression.get(idx + operator.len()..)?.trim();
            if operand_looks_like_call(left_raw) || operand_looks_like_call(right_raw) {
                return None;
            }
            let left = comparison_operand_before(&expression, idx)?;
            let right = comparison_operand_after(&expression, idx + operator.len())?;
            if is_simple_typescript_discriminator_operand(&left)
                && is_simple_typescript_discriminator_operand(&right)
            {
                return Some(format!("{left} == {right}"));
            }
        }
    }
    None
}

pub(crate) fn typescript_return_value_discriminator(line_text: &str) -> Option<String> {
    let expression = line_text
        .trim()
        .strip_prefix("return")?
        .trim()
        .trim_end_matches(';')
        .trim();
    if expression.is_empty() || expression == "{" || expression == "({" {
        None
    } else {
        Some(format!("return value == {expression}"))
    }
}

fn typescript_error_path_discriminator(line_text: &str) -> Option<String> {
    let text = line_text.trim().trim_end_matches(';').trim();
    if text.starts_with("throw ") || text.starts_with("throw(") {
        let raised = text
            .strip_prefix("throw ")
            .or_else(|| text.strip_prefix("throw("))
            .unwrap_or(text)
            .trim()
            .trim_end_matches(')');
        return typescript_error_value("throws", raised);
    }
    if let Some(argument) = promise_reject_argument(text) {
        return typescript_error_value("rejects", argument);
    }
    if text.starts_with("catch ") || text.starts_with("} catch ") {
        return Some("catch branch executes".to_string());
    }
    None
}

fn promise_reject_argument(text: &str) -> Option<&str> {
    let marker = "Promise.reject(";
    let start = text.find(marker)? + marker.len();
    let tail = text.get(start..)?;
    let end = tail.rfind(')')?;
    Some(tail.get(..end)?.trim())
}

fn typescript_error_value(prefix: &str, expression: &str) -> Option<String> {
    let expression = expression.trim();
    let error_type = if let Some(constructed) = expression.strip_prefix("new ") {
        constructed
            .split_once('(')
            .map(|(ty, _)| ty.trim())
            .unwrap_or(constructed.trim())
    } else if let Some((callee, _)) = expression.split_once('(') {
        let callee = callee.trim();
        if !starts_with_uppercase(callee) && !callee.ends_with("Error") {
            return None;
        }
        callee
    } else if let Some(message) = first_typescript_string_literal(expression) {
        return Some(format!("{prefix} error matching {message}"));
    } else {
        return None;
    };
    if error_type.is_empty() {
        return None;
    }
    if let Some(message) = first_typescript_string_literal(expression) {
        Some(format!("{prefix} {error_type} matching {message}"))
    } else {
        Some(format!("{prefix} {error_type}"))
    }
}

fn typescript_field_value_discriminator(line_text: &str) -> Option<String> {
    let text = line_text.trim().trim_end_matches(';').trim();
    if let Some(discriminator) = typescript_return_object_field_discriminator(text) {
        return Some(discriminator);
    }
    if let Some(discriminator) = typescript_object_field_discriminator(text) {
        return Some(discriminator);
    }
    let (lhs, rhs) = split_typescript_assignment(text)?;
    if lhs.is_empty() || rhs.is_empty() || lhs.contains('[') || lhs.contains(']') {
        None
    } else {
        Some(format!("{lhs} == {rhs}"))
    }
}

fn typescript_return_object_field_discriminator(line_text: &str) -> Option<String> {
    let expression = line_text
        .strip_prefix("return ")?
        .trim()
        .strip_prefix('(')
        .unwrap_or_else(|| {
            line_text
                .strip_prefix("return ")
                .unwrap_or(line_text)
                .trim()
        })
        .trim();
    let body = expression.strip_prefix('{')?;
    typescript_object_field_discriminator(body)
}

fn typescript_object_field_discriminator(line_text: &str) -> Option<String> {
    let body = line_text.trim().trim_end_matches(')').trim_end_matches('}');
    let (raw_key, rest) = body.split_once(':')?;
    let key = raw_key.trim().trim_matches('"').trim_matches('\'');
    let value = rest
        .split(',')
        .next()
        .unwrap_or(rest)
        .trim()
        .trim_end_matches('}')
        .trim();
    if !is_simple_typescript_object_key(key) || value.is_empty() {
        None
    } else {
        Some(format!("{key} == {value}"))
    }
}

fn typescript_call_effect_discriminator(line_text: &str) -> Option<String> {
    if is_computed_member_call(line_text) {
        return None;
    }
    let (callee, args) = typescript_call_parts(line_text)?;
    let first_arg = args
        .split(',')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches(')')
        .trim();
    if callee.to_ascii_lowercase().contains("mock") || callee.to_ascii_lowercase().contains("spy") {
        if first_arg.is_empty() {
            Some(format!("mock interaction {callee} is called"))
        } else {
            Some(format!("mock interaction {callee} called with {first_arg}"))
        }
    } else if let Some(literal) = first_typescript_string_literal(line_text) {
        if callee.contains("log") || callee.starts_with("console.") {
            Some(format!("log contains {literal}"))
        } else {
            Some(format!("call {callee} includes {literal}"))
        }
    } else if first_arg.is_empty() {
        Some(format!("call {callee} occurs"))
    } else {
        Some(format!("call {callee} includes {first_arg}"))
    }
}

pub(crate) fn split_typescript_assignment(text: &str) -> Option<(&str, &str)> {
    if text.contains("==") || text.contains("!=") || text.contains(">=") || text.contains("<=") {
        return None;
    }
    let (lhs, rhs) = text.split_once(" = ")?;
    Some((lhs.trim(), rhs.trim().trim_end_matches(';').trim()))
}

pub(crate) fn is_computed_field_construction(line_text: &str) -> bool {
    let text = line_text.trim();
    if let Some((lhs, _)) = split_typescript_assignment(text) {
        return lhs.contains('[') || lhs.contains(']');
    }
    contains_unquoted_shape(text, "{[") || contains_unquoted_shape(text, "{ [")
}

fn strip_typescript_control_prefix(line_text: &str) -> String {
    let mut text = line_text
        .trim()
        .trim_start_matches('}')
        .trim()
        .trim_end_matches('{')
        .trim()
        .to_string();
    for prefix in ["if", "else if", "while", "for", "case"] {
        if let Some(stripped) = text.strip_prefix(prefix) {
            text = stripped.trim().to_string();
            break;
        }
    }
    text.trim_start_matches('(')
        .trim_end_matches(')')
        .trim()
        .to_string()
}

fn comparison_operand_before(expression: &str, operator_start: usize) -> Option<String> {
    let left = expression.get(..operator_start)?.trim_end();
    let operand = left
        .rsplit(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '(' | ')' | '[' | ']' | '{' | '}' | ',' | ':' | '+' | '-' | '*' | '/' | '%'
                )
        })
        .find(|part| !part.is_empty())?;
    Some(operand.trim().to_string())
}

fn comparison_operand_after(expression: &str, operator_end: usize) -> Option<String> {
    let right = expression.get(operator_end..)?.trim_start();
    let operand = right
        .split(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '(' | ')' | '[' | ']' | '{' | '}' | ',' | ':' | '+' | '-' | '*' | '/' | '%'
                )
        })
        .find(|part| !part.is_empty())?;
    Some(operand.trim().to_string())
}

fn is_simple_typescript_discriminator_operand(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|ch| {
            ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '"' || ch == '\''
        })
}

fn is_simple_typescript_object_key(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

fn operand_looks_like_call(value: &str) -> bool {
    value.contains('(') || value.contains(')')
}

fn typescript_call_parts(line_text: &str) -> Option<(String, String)> {
    let mut text = line_text
        .trim()
        .strip_prefix("await ")
        .unwrap_or(line_text.trim())
        .trim();
    text = text.strip_prefix("void ").unwrap_or(text).trim();
    let text = text.trim_end_matches(';').trim();
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    if close <= open {
        return None;
    }
    let callee = text.get(..open)?.trim();
    if callee.is_empty()
        || !callee
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' || ch == '.')
    {
        return None;
    }
    let args = text.get(open + 1..close)?.trim();
    Some((callee.to_string(), args.to_string()))
}

pub(crate) fn is_computed_member_call(line_text: &str) -> bool {
    let text = line_text.trim();
    ["](", "]?.", "?.["]
        .iter()
        .any(|shape| contains_unquoted_shape(text, shape))
}

pub(crate) fn contains_unquoted_shape(text: &str, shape: &str) -> bool {
    text.match_indices(shape).any(|(idx, _)| {
        !line_prefix_looks_like_comment_or_string(text, idx) && !inside_block_comment(text, idx)
    })
}

fn first_typescript_string_literal(text: &str) -> Option<String> {
    let mut start = None;
    let mut escaped = false;
    for (idx, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' || ch == '\'' || ch == '`' {
            start = Some((idx, ch));
            break;
        }
    }
    let (start_idx, quote) = start?;
    escaped = false;
    for (relative_idx, ch) in text[start_idx + quote.len_utf8()..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            let end_idx = start_idx + quote.len_utf8() + relative_idx + quote.len_utf8();
            return text.get(start_idx..end_idx).map(str::to_string);
        }
    }
    None
}
