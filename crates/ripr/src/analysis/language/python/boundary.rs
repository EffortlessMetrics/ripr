//! Predicate boundary activation evidence for the Python preview adapter.
//!
//! A changed relational predicate (`qty > on_hand` -> `qty >= on_hand`) only
//! behaves differently when its operands are EQUAL. A strong exact-value
//! assertion that calls the owner away from that boundary (`reserve(10, 3)`)
//! still passes after the change, so reach plus a strong oracle does not
//! discriminate it.
//!
//! This mirrors the Rust activation rule
//! (`analysis/classify/activation.rs::missing_boundary_discriminator`):
//!
//! - the rule engages only when a strong related test calls the owner with at
//!   least one literal argument (Rust returns early when `call_values` is
//!   empty), because only then does static evidence see concrete owner inputs;
//! - the boundary is observed when some engaged call binds both comparison
//!   operands (literals, or owner parameters bound to literal arguments or
//!   literal defaults) to equal values;
//! - otherwise the boundary is missing. An operand that is neither a literal
//!   nor a parameter (`item.on_hand`, a computed `len(items)`, a comprehension
//!   local) is unresolved and never counts as observed, so this fails closed.
//!
//! When no strong call binds a literal argument (`Formatter()({...})`,
//! `reserve(item, qty)` with test locals), the rule cannot see the inputs in
//! either direction; the verdict stays with the existing oracle rules and the
//! finding carries a named `boundary_activation: unresolved` limitation.

use super::discriminators::{is_literal_python_model_field_value, python_string_literal_value};
use super::no_behavior::{
    call_arglists_with_offsets, call_segment_keyword_name, split_top_level_args,
};
use super::related_tests::{PythonRelatedCandidate, strongest_assertion};
use super::{PythonOwner, PythonTest};
use crate::domain::{OracleStrength, OwnerKind, ValueContext, ValueFact};
use std::collections::BTreeMap;

/// Whether static evidence places a strong related call on the boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BoundaryActivation {
    /// A strong related call binds both operands to equal literal values.
    Observed,
    /// Strong related calls bind literal owner inputs, but none places the
    /// operands on the boundary (or an operand cannot be resolved).
    Missing,
    /// No strong related call binds a literal owner input, so static evidence
    /// cannot see the activating inputs either way.
    Unresolved,
}

/// Boundary activation evidence for one changed relational predicate.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PythonBoundaryEvidence {
    pub(super) activation: BoundaryActivation,
    /// The equality discriminator that pins the changed boundary
    /// (`qty == item.on_hand`), named only when both operands are simple
    /// names or literals.
    pub(super) discriminator: Option<String>,
    /// Literal owner-call arguments (and the boundary equality, when observed)
    /// from strong related tests.
    pub(super) observed_values: Vec<ValueFact>,
    /// Why the boundary is observed, missing, or unresolved.
    pub(super) reason: String,
}

/// Boundary evidence for a changed Predicate line that contains a relational
/// comparison (`<`, `<=`, `>`, `>=`). Returns None when the line has no
/// relational comparison (for example `if flag:` or `if x == 5:`), leaving the
/// caller's existing classification untouched.
pub(super) fn python_boundary_evidence(
    line_text: &str,
    owner: &PythonOwner,
    related_candidates: &[PythonRelatedCandidate<'_>],
) -> Option<PythonBoundaryEvidence> {
    let condition = predicate_condition(line_text);
    let operators = relational_operators(&condition);
    if operators.is_empty() {
        return None;
    }
    let operands = match operators.as_slice() {
        [(start, len)] => simple_comparison_operands(&condition, *start, *len),
        _ => None,
    };
    let discriminator = operands
        .as_ref()
        .map(|(left, right)| format!("{left} == {right}"));

    let rows = strong_owner_call_rows(owner, related_candidates);
    let mut observed_values: Vec<ValueFact> = rows
        .iter()
        .flat_map(|row| {
            row.bindings
                .iter()
                .filter(|(_, binding)| !binding.from_default)
                .map(|(parameter, binding)| ValueFact {
                    line: row.line,
                    text: row.text.clone(),
                    value: format!("{parameter} = {}", binding.value),
                    context: ValueContext::FunctionArgument,
                })
        })
        .collect();
    if observed_values.is_empty() {
        return Some(PythonBoundaryEvidence {
            activation: BoundaryActivation::Unresolved,
            discriminator,
            observed_values,
            reason: format!(
                "no strong related test call to `{}` binds a literal argument, so static evidence cannot place an input at the changed boundary of `{condition}`",
                owner.name
            ),
        });
    }

    let Some((left, right)) = operands else {
        let reason = if operators.len() > 1 {
            format!(
                "The changed predicate `{condition}` holds {} relational comparisons; static evidence cannot establish which boundary a related test call pins",
                operators.len()
            )
        } else {
            format!(
                "The changed predicate `{condition}` compares a computed operand; static evidence cannot establish that a related test call pins its boundary"
            )
        };
        return Some(PythonBoundaryEvidence {
            activation: BoundaryActivation::Missing,
            discriminator,
            observed_values,
            reason,
        });
    };

    let mut observed = false;
    let mut left_values = Vec::new();
    let mut right_values = Vec::new();
    for row in &rows {
        let left_value = resolve_operand(&left, row);
        let right_value = resolve_operand(&right, row);
        if let (Some(left_value), Some(right_value)) = (&left_value, &right_value)
            && literals_equal(left_value, right_value)
        {
            observed = true;
            observed_values.push(ValueFact {
                line: row.line,
                text: format!(
                    "{} | {}; {}",
                    row.text,
                    operand_provenance(&left, left_value),
                    operand_provenance(&right, right_value)
                ),
                value: format!("{left} == {right}"),
                context: ValueContext::FunctionArgument,
            });
        }
        left_values.extend(left_value);
        right_values.extend(right_value);
    }
    observed_values.sort_by(|a, b| {
        a.line
            .cmp(&b.line)
            .then(a.value.cmp(&b.value))
            .then(a.text.cmp(&b.text))
    });
    observed_values.dedup();

    // An operand that no strong call resolves (`item.on_hand`, a test local)
    // leaves the boundary unknown rather than missed: the test input may sit
    // on it at runtime (`reserve(Item("a", 3), 3)`). The finding still fails
    // closed to weak, but the equality is not handed out as a repair target,
    // so no repair card asks for a test that may already exist.
    let unresolved_operand = [(&left, &left_values), (&right, &right_values)]
        .into_iter()
        .any(|(operand, values)| literal_value(operand).is_none() && values.is_empty());
    let discriminator = if observed || !unresolved_operand {
        discriminator
    } else {
        None
    };
    let reason = if observed {
        format!("A strong related test call places {left} equal to {right}")
    } else {
        // A literal operand is its own value; only operands that vary with the
        // test input are listed.
        let observed_lists = [(&left, left_values), (&right, right_values)]
            .into_iter()
            .filter(|(operand, _)| literal_value(operand).is_none())
            .map(|(operand, values)| {
                format!("observed {operand} values: {}", list_or_unresolved(values))
            })
            .collect::<Vec<_>>();
        format!(
            "No strong related test call places {left} equal to {right}; {}",
            observed_lists.join("; ")
        )
    };
    Some(PythonBoundaryEvidence {
        activation: if observed {
            BoundaryActivation::Observed
        } else {
            BoundaryActivation::Missing
        },
        discriminator,
        observed_values,
        reason,
    })
}

fn operand_provenance(operand: &str, value: &str) -> String {
    if literal_value(operand).is_some() {
        format!("literal operand {operand}")
    } else {
        format!("{operand} = {value}")
    }
}

fn list_or_unresolved(mut values: Vec<String>) -> String {
    values.sort();
    values.dedup();
    if values.is_empty() {
        "unresolved".to_string()
    } else {
        values.join(", ")
    }
}

/// The boolean condition of a changed predicate line: the control prefix
/// (`if`/`elif`/`while`) and trailing `:` stripped, or the condition between
/// ` if ` and ` else ` of a conditional expression.
fn predicate_condition(line_text: &str) -> String {
    let text = line_text.trim().trim_end_matches(':').trim();
    for prefix in ["if ", "elif ", "while "] {
        if let Some(stripped) = text.strip_prefix(prefix) {
            return stripped.trim().to_string();
        }
    }
    if let Some((_, after_if)) = text.split_once(" if ")
        && let Some((condition, _)) = after_if.split_once(" else ")
    {
        return condition.trim().to_string();
    }
    text.to_string()
}

/// Byte offset and length of every relational comparison operator outside
/// string literals. Shift (`<<`, `>>`), arrow (`->`) and comparison-equality
/// (`==`, `!=`) operators are not relational boundaries.
fn relational_operators(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut operators = Vec::new();
    let mut quote: Option<u8> = None;
    let mut escaped = false;
    let mut idx = 0usize;
    while idx < bytes.len() {
        let byte = bytes[idx];
        if escaped {
            escaped = false;
            idx += 1;
            continue;
        }
        if byte == b'\\' {
            escaped = true;
            idx += 1;
            continue;
        }
        if let Some(active) = quote {
            if byte == active {
                quote = None;
            }
            idx += 1;
            continue;
        }
        match byte {
            b'\'' | b'"' => quote = Some(byte),
            b'#' => break,
            b'<' | b'>' => {
                let next = bytes.get(idx + 1).copied();
                let prev = idx.checked_sub(1).map(|p| bytes[p]);
                if next == Some(byte) || prev == Some(b'-') {
                    // `<<` / `>>` shift (and `<<=` / `>>=`), or the `->` arrow.
                    idx += 2;
                    continue;
                }
                let len = if next == Some(b'=') { 2 } else { 1 };
                operators.push((idx, len));
                idx += len;
                continue;
            }
            _ => {}
        }
        idx += 1;
    }
    operators
}

/// The two operands of the only relational comparison in `condition`, when
/// both are complete simple operands: a name, dotted attribute path, or
/// scalar literal. Returns None for a computed operand (`len(items)`,
/// `a + b`, `values[0]`), so the caller never binds a partial operand.
fn simple_comparison_operands(
    condition: &str,
    operator_start: usize,
    operator_len: usize,
) -> Option<(String, String)> {
    let left_side = condition.get(..operator_start)?.trim_end();
    let right_side = condition.get(operator_start + operator_len..)?.trim_start();

    let left_start = trailing_operand_start(left_side);
    let left = &left_side[left_start..];
    let before_left = left_side[..left_start].trim_end();
    let left_boundary_ok = before_left.is_empty()
        || before_left.ends_with('(')
        || ["not", "and", "or", "if"]
            .into_iter()
            .any(|keyword| ends_with_keyword(before_left, keyword));

    let right_end = leading_operand_end(right_side);
    let right = &right_side[..right_end];
    let after_right = right_side[right_end..].trim_start();
    let right_boundary_ok = after_right.is_empty()
        || after_right.starts_with(')')
        || ["and", "or", "for", "if", "else"]
            .into_iter()
            .any(|keyword| starts_with_keyword(after_right, keyword));

    (left_boundary_ok && right_boundary_ok && is_simple_operand(left) && is_simple_operand(right))
        .then(|| (left.to_string(), right.to_string()))
}

/// Byte offset where the operand that ends `text` starts: a quoted string
/// literal, or a run of name/number/attribute characters.
fn trailing_operand_start(text: &str) -> usize {
    if let Some(quote) = text
        .chars()
        .next_back()
        .filter(|ch| matches!(ch, '"' | '\''))
    {
        let body_end = text.len() - quote.len_utf8();
        return text[..body_end].rfind(quote).unwrap_or(body_end);
    }
    text.char_indices()
        .rev()
        .find(|(_, ch)| !is_operand_char(*ch))
        .map_or(0, |(idx, ch)| idx + ch.len_utf8())
}

/// Byte offset where the operand that starts `text` ends: a quoted string
/// literal, an optionally negative number, or a run of name/attribute
/// characters.
fn leading_operand_end(text: &str) -> usize {
    if let Some(quote) = text.chars().next().filter(|ch| matches!(ch, '"' | '\'')) {
        let body_start = quote.len_utf8();
        return text[body_start..]
            .find(quote)
            .map_or(text.len(), |idx| body_start + idx + quote.len_utf8());
    }
    let sign =
        usize::from(text.starts_with('-') && text[1..].starts_with(|ch: char| ch.is_ascii_digit()));
    text[sign..]
        .char_indices()
        .find(|(_, ch)| !is_operand_char(*ch))
        .map_or(text.len(), |(idx, _)| sign + idx)
}

fn is_operand_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '.'
}

fn is_simple_operand(operand: &str) -> bool {
    let unsigned = operand.strip_prefix('-').unwrap_or(operand);
    !unsigned.is_empty()
        && !unsigned.starts_with('.')
        && !unsigned.ends_with('.')
        && (literal_value(operand).is_some()
            || unsigned
                .split('.')
                .all(super::static_limits::is_simple_python_identifier))
}

fn ends_with_keyword(text: &str, keyword: &str) -> bool {
    text.strip_suffix(keyword).is_some_and(|before| {
        before.is_empty() || before.ends_with(|ch: char| ch.is_whitespace() || ch == '(')
    })
}

fn starts_with_keyword(text: &str, keyword: &str) -> bool {
    text.strip_prefix(keyword)
        .is_some_and(|after| after.is_empty() || after.starts_with(char::is_whitespace))
}

/// A literal binding for one owner parameter in one test call.
#[derive(Clone, Debug)]
struct Binding {
    value: String,
    from_default: bool,
}

/// One analyzable call to the owner from a strong related test.
#[derive(Clone, Debug)]
struct CallRow {
    line: usize,
    text: String,
    bindings: BTreeMap<String, Binding>,
}

/// Owner calls from related tests whose strongest assertion is strong: the
/// same tests that could credit `exposed`. Each analyzable call binds owner
/// parameters to literal arguments (or literal defaults when omitted).
fn strong_owner_call_rows(
    owner: &PythonOwner,
    related_candidates: &[PythonRelatedCandidate<'_>],
) -> Vec<CallRow> {
    let (method_call, skip) = match owner.owner_kind {
        Some(OwnerKind::Method) => (true, 1),
        Some(OwnerKind::ClassMethod) => (
            true,
            usize::from(
                !owner
                    .decorators
                    .iter()
                    .any(|decorator| decorator.ends_with("staticmethod")),
            ),
        ),
        Some(OwnerKind::Function) => (false, 0),
        _ => return Vec::new(),
    };
    let mut rows = Vec::new();
    for candidate in related_candidates {
        if !candidate.relation.uses_oracle() {
            continue;
        }
        let strong = strongest_assertion(&candidate.test.assertions).is_some_and(|assertion| {
            assertion.oracle_strength.rank() >= OracleStrength::Strong.rank()
        });
        if !strong {
            continue;
        }
        for name in owner_call_names(owner, candidate.test, method_call) {
            for (offset, arglist) in
                call_arglists_with_offsets(&candidate.test.body_text, &name, method_call)
            {
                if let Some(bindings) = bind_call_arguments(owner, skip, arglist) {
                    rows.push(CallRow {
                        line: candidate.test.line
                            + candidate.test.body_text[..offset].matches('\n').count(),
                        text: call_line_text(&candidate.test.body_text, offset),
                        bindings,
                    });
                }
            }
        }
    }
    rows
}

/// The owner's own name plus any `from M import owner as alias` alias; a
/// method is only called through its own attribute name.
fn owner_call_names(owner: &PythonOwner, test: &PythonTest, method_call: bool) -> Vec<String> {
    let mut names = vec![owner.name.clone()];
    if !method_call {
        names.extend(
            test.imports
                .iter()
                .filter(|import| import.imported == owner.name && import.alias != owner.name)
                .map(|import| import.alias.clone()),
        );
    }
    names
}

fn call_line_text(body: &str, offset: usize) -> String {
    let start = body[..offset].rfind('\n').map_or(0, |idx| idx + 1);
    let end = body[offset..]
        .find('\n')
        .map_or(body.len(), |idx| offset + idx);
    body[start..end].trim().to_string()
}

/// Bind a call's literal arguments to owner parameters. Positional arguments
/// bind in declaration order after `skip` implicit receivers; keyword
/// arguments bind by name; an omitted parameter with a literal default binds
/// to that default. Returns None for a `*args` / `**kwargs` unpack, whose
/// binding is undecidable. Non-literal arguments stay unbound (unresolved).
fn bind_call_arguments(
    owner: &PythonOwner,
    skip: usize,
    arglist: &str,
) -> Option<BTreeMap<String, Binding>> {
    let positional: Vec<&str> = owner
        .parameters
        .iter()
        .filter(|parameter| !parameter.keyword_only)
        .skip(skip)
        .map(|parameter| parameter.name.as_str())
        .collect();
    let mut bound_names = Vec::new();
    let mut bindings = BTreeMap::new();
    let mut position = 0usize;
    for segment in split_top_level_args(arglist) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        if segment.starts_with('*') || segment.contains('#') {
            return None;
        }
        let (name, value) = match call_segment_keyword_name(segment) {
            Some(name) => {
                let value = segment
                    .split_once('=')
                    .map(|(_, value)| value.trim())
                    .unwrap_or_default();
                (Some(name.to_string()), value)
            }
            None => {
                let name = positional.get(position).map(|name| (*name).to_string());
                position += 1;
                (name, segment)
            }
        };
        let Some(name) = name else {
            continue;
        };
        bound_names.push(name.clone());
        if let Some(value) = literal_value(value) {
            bindings.insert(
                name,
                Binding {
                    value,
                    from_default: false,
                },
            );
        }
    }
    for parameter in owner.parameters.iter().skip(skip) {
        if bound_names.contains(&parameter.name) {
            continue;
        }
        if let Some(value) = parameter.default.as_deref().and_then(literal_value) {
            bindings.insert(
                parameter.name.clone(),
                Binding {
                    value,
                    from_default: true,
                },
            );
        }
    }
    Some(bindings)
}

/// A comparison operand's value in one call row: its own literal value, or the
/// literal bound to the owner parameter it names. Anything else is unresolved.
fn resolve_operand(operand: &str, row: &CallRow) -> Option<String> {
    literal_value(operand).or_else(|| row.bindings.get(operand).map(|b| b.value.clone()))
}

/// The canonical text of a scalar Python literal (number with `_` separators
/// removed, string, `True`/`False`/`None`), or None for anything else.
fn literal_value(text: &str) -> Option<String> {
    let text = text.trim();
    let without_separators: String = text.chars().filter(|ch| *ch != '_').collect();
    let numeric = !without_separators.is_empty()
        && without_separators
            .chars()
            .all(|ch| ch.is_ascii_digit() || ch == '.' || ch == '-');
    if numeric && is_literal_python_model_field_value(&without_separators) {
        return Some(without_separators);
    }
    if is_literal_python_model_field_value(text) {
        return Some(text.to_string());
    }
    None
}

/// Literal equality as Python compares the scalar values: decimals by value
/// (`100 == 100.0`), strings by content regardless of quote style, and
/// `True`/`False`/`None` by identity. Mixed kinds never compare equal here
/// (for example `True == 1`), which fails closed toward "not observed".
fn literals_equal(left: &str, right: &str) -> bool {
    match (canonical_decimal(left), canonical_decimal(right)) {
        (Some(left), Some(right)) => return left == right,
        (Some(_), None) | (None, Some(_)) => return false,
        (None, None) => {}
    }
    match (
        python_string_literal_value(left),
        python_string_literal_value(right),
    ) {
        (Some(left), Some(right)) => left == right,
        (None, None) => left == right,
        _ => false,
    }
}

/// Canonical text of a plain decimal literal so values Python compares equal
/// render identically: `100`, `100.0` and `0100.00` all become `100`, and
/// `-0.0` becomes `0`. None for anything that is not a plain decimal.
fn canonical_decimal(text: &str) -> Option<String> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (integer, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    if integer.is_empty() && fraction.is_empty()
        || !integer
            .chars()
            .chain(fraction.chars())
            .all(|ch| ch.is_ascii_digit())
    {
        return None;
    }
    let integer = integer.trim_start_matches('0');
    let fraction = fraction.trim_end_matches('0');
    let magnitude = match (integer.is_empty(), fraction.is_empty()) {
        (true, true) => return Some("0".to_string()),
        (false, true) => integer.to_string(),
        (true, false) => format!("0.{fraction}"),
        (false, false) => format!("{integer}.{fraction}"),
    };
    Some(if negative {
        format!("-{magnitude}")
    } else {
        magnitude
    })
}
