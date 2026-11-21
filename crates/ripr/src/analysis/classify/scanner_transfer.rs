//! Bounded literal-driven scanner transitions (#3296, the scanner arm
//! of the #3215 helper-evidence slice).
//!
//! A scanner helper has the exact shape the corpus pins:
//!
//! ```text
//! let mut state = <exact initial state>;
//! for symbol in <exact input>.chars() {
//!     state = match (state, symbol) { <literal arms> };
//! }
//! state
//! ```
//!
//! When the loop's input string and the initial state are both exact
//! (#3295 facts) and every arm's next state is a bare state token — an
//! identifier path (`Scan::Word`) or a plain string literal
//! (`"text"`) — the evaluator unrolls at most [`MAX_SCANNER_STEPS`]
//! transitions and returns the final state as an exact value. Anything
//! else — a data-dependent input, a non-literal arm, a body that
//! mutates other locals, a bound exceeded — stops and returns `None`;
//! the caller's existing limitation wording stays the disclosure.

use super::value_transfer::{ExactInputs, TypedValue};
use crate::analysis::language::{changed_let_binding, mask_rust_comments_and_strings};

/// The configured step bound. A scanner whose exact input exceeds it is
/// a named limitation, never a silently truncated evaluation.
pub(crate) const MAX_SCANNER_STEPS: usize = 32;

/// Evaluate a scanner helper's final state over its exact inputs, when
/// the helper body has the pinned scanner shape.
pub(crate) fn scanner_return_value(
    helper: &crate::analysis::facts::FunctionSummary,
    inputs: &ExactInputs,
) -> Option<TypedValue> {
    let shape = ScannerShape::parse(&helper.body)?;
    let initial = resolve_state(&shape.initial, inputs)?;
    let symbols = resolve_symbols(&shape.iterable, inputs)?;
    let mut state = initial;
    for (step, symbol) in symbols.iter().enumerate() {
        if step >= MAX_SCANNER_STEPS {
            return None;
        }
        state = transition(&shape.arms, &state, *symbol)?;
    }
    Some(TypedValue::Str(state))
}

/// The parsed scanner body: initial state, exact iterable expression,
/// and the literal transition arms.
struct ScannerShape {
    initial: String,
    iterable: String,
    arms: Vec<TransitionArm>,
}

struct TransitionArm {
    state: String,
    /// `None` is the `_` wildcard arm; `Some(ch)` is an exact char
    /// literal. A quoted `'_'` is a literal underscore, never the
    /// wildcard.
    symbol: Option<char>,
    next: String,
}

impl ScannerShape {
    /// Parse the scanner shape from raw function-body text. Returns
    /// `None` for any other body — including let-chains with extra
    /// mutations, nested loops, or `while` — so the non-scanner helper
    /// evaluator keeps its own authority.
    fn parse(body: &str) -> Option<ScannerShape> {
        let masked = mask_rust_comments_and_strings(body);
        let raw_lines: Vec<&str> = body.lines().collect();
        let masked_lines: Vec<&str> = masked.lines().collect();
        if raw_lines.len() != masked_lines.len() {
            return None;
        }
        let mut initial: Option<String> = None;
        let mut iterable: Option<String> = None;
        let mut arms: Vec<TransitionArm> = Vec::new();
        let mut saw_state_tail = false;
        let mut saw_loop_close = false;
        for (raw_line, masked_line) in raw_lines.iter().zip(masked_lines.iter()) {
            let raw = raw_line.trim();
            let trimmed = masked_line.trim();
            if trimmed.starts_with('#')
                || trimmed.starts_with("pub ")
                || trimmed.starts_with("pub(")
                || trimmed.starts_with("fn ")
                || trimmed.starts_with("async ")
                || trimmed.starts_with("const ")
                || trimmed.starts_with("unsafe ")
                || trimmed.is_empty()
            {
                continue;
            }
            if !saw_state_tail && initial.is_none() {
                // `let mut state = <init>;` — the shape check uses the
                // masked line, the initializer raw text (masking erases
                // string literals, and string states are supported).
                if !trimmed.starts_with("let mut state ") {
                    return None;
                }
                let (binding, init) = changed_let_binding(raw)?;
                if binding != "state" {
                    return None;
                }
                initial = Some(init.to_string());
                continue;
            }
            if iterable.is_none() {
                // `for symbol in <iterable> {` — same masked/raw split.
                let rest = raw.strip_prefix("for symbol in ")?;
                let rest = rest.strip_suffix('{')?;
                iterable = Some(rest.trim().to_string());
                continue;
            }
            if !saw_loop_close && trimmed == "}" {
                saw_loop_close = true;
                continue;
            }
            if saw_state_tail {
                // Only the function's closing brace may follow the tail.
                if trimmed != "}" {
                    return None;
                }
                continue;
            }
            if saw_loop_close {
                if trimmed != "state" {
                    return None;
                }
                saw_state_tail = true;
                continue;
            }
            // Inside the loop: `state = match (state, symbol) {` on one
            // line, then literal arms, then `};`.
            if arms.is_empty() && trimmed.starts_with("state = match (state, symbol) {") {
                continue;
            }
            if trimmed == "};" {
                continue;
            }
            if let Some(arm) = parse_arm(raw) {
                arms.push(arm);
                continue;
            }
            // Any other line inside the loop is an unsupported edge.
            return None;
        }
        if !saw_state_tail || !saw_loop_close {
            return None;
        }
        if arms.is_empty() {
            return None;
        }
        Some(ScannerShape {
            initial: initial?,
            iterable: iterable?,
            arms,
        })
    }
}

/// `("text", 'symbol') => "next",` or `(Scan::Text, 's') => Scan::Word,`
/// — the state patterns and next states are qualified paths or plain
/// string literals; the symbol is a char literal or `_`.
fn parse_arm(line: &str) -> Option<TransitionArm> {
    let trimmed = line.trim().trim_end_matches(',').trim();
    let stripped = trimmed.strip_prefix('(')?;
    let (patterns, next) = stripped.split_once(") => ")?;
    let mut parts = patterns.splitn(2, ',');
    let state = normalize_state_token(parts.next()?)?;
    let raw_symbol = parts.next()?.trim();
    let symbol = if raw_symbol == "_" {
        None
    } else {
        // A plain one-character literal only: an escape sequence
        // (`'\n'`, `'\''`) fails closed instead of aliasing another
        // symbol, and a quoted `'_'` stays a literal underscore.
        let inner = raw_symbol.strip_prefix('\'')?.strip_suffix('\'')?;
        let mut chars = inner.chars();
        let ch = chars.next()?;
        if ch == '\\' || chars.next().is_some() {
            return None;
        }
        Some(ch)
    };
    let next = normalize_state_token(next)?;
    Some(TransitionArm {
        state,
        symbol,
        next,
    })
}

/// A state token: a plain string literal (`"text"`) or a
/// path-qualified identifier (`Scan::Word`). A bare single-segment
/// identifier is refused — in a pattern, value, or initializer
/// position it may be a variable or a binding, and treating its text
/// as a state would invent evidence (the token-coincidence family);
/// the initializer position resolves variables through the exact
/// inputs first instead.
fn normalize_state_token(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if let Some(inner) = trimmed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        if !inner.is_empty() && !inner.contains(['"', '\\', '\'', '\n']) {
            return Some(inner.to_string());
        }
        return None;
    }
    if trimmed.contains("::")
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == ':')
    {
        return Some(trimmed.to_string());
    }
    None
}

/// Resolve the initial state. A bare identifier is a variable first
/// (Rust semantics: a bound input resolves before any literal reading
/// of the same text — the token-coincidence guard), then a state
/// token: a plain string literal or an identifier path. An evaluated
/// input must itself be a valid plain string state.
fn resolve_state(initial: &str, inputs: &ExactInputs) -> Option<String> {
    match super::value_transfer::evaluate_initializer(initial, inputs) {
        super::value_transfer::EvalOutcome::Exact { value, .. } => match value {
            TypedValue::Str(state) => normalize_state_token(&format!("\"{state}\"")),
            _ => None,
        },
        _ => normalize_state_token(initial),
    }
}

/// The iterable must reduce to an exact sequence of symbols: a #3295
/// char sequence (`<input>.chars()`) or string.
fn resolve_symbols(iterable: &str, inputs: &ExactInputs) -> Option<Vec<char>> {
    match super::value_transfer::evaluate_initializer(iterable, inputs) {
        super::value_transfer::EvalOutcome::Exact { value, .. } => match value {
            TypedValue::CharSeq(symbols) => Some(symbols),
            TypedValue::Str(symbols) => Some(symbols.chars().collect()),
            _ => None,
        },
        _ => None,
    }
}

/// One exact transition, first-match-wins exactly like the Rust match:
/// the first arm whose state pattern equals the current state and whose
/// symbol is the wildcard or the exact literal produces the next state.
/// `next` may be the state binding itself (already normalized away by
/// substituting the current state).
fn transition(arms: &[TransitionArm], state: &str, symbol: char) -> Option<String> {
    for arm in arms {
        if arm.state != state {
            continue;
        }
        if arm.symbol.is_none_or(|expected| expected == symbol) {
            return Some(arm.next.clone());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::facts::FunctionSourceRole;
    use crate::analysis::facts::FunctionSummary;
    use crate::domain::SymbolId;
    use std::path::PathBuf;

    fn scanner_fn(name: &str, body: &str) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId(format!("src/lib.rs::{name}")),
            name: name.to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 10,
            body: body.to_string(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            source_role: FunctionSourceRole::Production,
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    fn inputs(rows: &[(&str, &str)]) -> ExactInputs {
        rows.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn final_state(helper: &FunctionSummary, input: &str) -> Result<String, String> {
        match scanner_return_value(helper, &inputs(&[("input", input)])) {
            Some(TypedValue::Str(state)) => Ok(state),
            Some(other) => Err(format!("final state must be a string, got {other:?}")),
            None => Err("scanner must resolve over an exact input".into()),
        }
    }

    const BODY: &str = "pub fn scan(input: &str) -> State {
    let mut state = State::Text;
    for symbol in input.chars() {
        state = match (state, symbol) {
            (State::Text, ' ') => State::Text,
            (State::Text, _) => State::Word,
            (State::Word, ' ') => State::Text,
            (State::Word, _) => State::Word,
        };
    }
    state
}";

    #[test]
    fn scanner_resolves_exact_final_state() -> Result<(), String> {
        let helper = scanner_fn("scan", BODY);
        assert_eq!(final_state(&helper, "\"ab\"")?, "State::Word");
        assert_eq!(final_state(&helper, "\"ab \"")?, "State::Text");
        Ok(())
    }

    #[test]
    fn scanner_step_bound_is_a_named_limit() -> Result<(), String> {
        let helper = scanner_fn("scan", BODY);
        let long = format!("\"{}\"", "a".repeat(MAX_SCANNER_STEPS + 1));
        let resolved = scanner_return_value(&helper, &inputs(&[("input", long.as_str())]));
        assert!(
            resolved.is_none(),
            "an input beyond the step bound must stop, got {resolved:?}"
        );
        // At exactly the bound it still resolves.
        let at_bound = format!("\"{}\"", "a".repeat(MAX_SCANNER_STEPS));
        assert_eq!(final_state(&helper, &at_bound)?, "State::Word");
        Ok(())
    }

    #[test]
    fn data_dependent_and_non_literal_shapes_fail_closed() -> Result<(), String> {
        // Computed iterable (not an exact input): no resolution.
        let helper = scanner_fn("scan", BODY);
        assert!(
            scanner_return_value(&helper, &inputs(&[])).is_none(),
            "a missing input must not resolve"
        );
        // A non-literal arm (method call as next state): no resolution.
        let body = BODY.replace(
            "(State::Text, ' ') => State::Text,",
            "(State::Text, ' ') => State::parse(' '),",
        );
        let helper2 = scanner_fn("scan", &body);
        assert!(
            scanner_return_value(&helper2, &inputs(&[("input", "\"a\"")])).is_none(),
            "a non-literal next state must not resolve"
        );
        // A body with an extra mutation inside the loop: no resolution.
        let body3 = BODY.replace(
            "        state = match (state, symbol) {",
            "        seen = true;
        state = match (state, symbol) {",
        );
        let helper3 = scanner_fn("scan", &body3);
        assert!(
            scanner_return_value(&helper3, &inputs(&[("input", "\"a\"")])).is_none(),
            "an extra loop mutation must not resolve"
        );
        // The non-scanner let-chain body stays with the other authority.
        let plain = "pub fn one() -> u8 {
    let v = 1;
    v
}";
        let helper4 = scanner_fn("one", plain);
        assert!(scanner_return_value(&helper4, &inputs(&[])).is_none());
        Ok(())
    }

    #[test]
    fn wildcard_and_explicit_arm_precedence_is_exact() -> Result<(), String> {
        // Space matches the explicit arm (Text stays Text); every other
        // symbol falls to the wildcard (Text -> Word). Source order is
        // authoritative: a wildcard placed before an explicit arm wins
        // first, exactly like the Rust match.
        let reordered = BODY.replace(
            "            (State::Text, ' ') => State::Text,
            (State::Text, _) => State::Word,",
            "            (State::Text, _) => State::Word,
            (State::Text, ' ') => State::Text,",
        );
        assert_ne!(reordered, BODY, "the reordered-arm substitution must apply");
        let reordered_helper = scanner_fn("scan", &reordered);
        assert_eq!(final_state(&reordered_helper, "\" \"")?, "State::Word");
        let helper = scanner_fn("scan", BODY);
        assert_eq!(final_state(&helper, "\" \"")?, "State::Text");
        assert_eq!(final_state(&helper, "\"x\"")?, "State::Word");
        Ok(())
    }

    #[test]
    fn quoted_underscore_symbol_is_a_literal_not_a_wildcard() -> Result<(), String> {
        // A quoted `'_'` is a literal underscore match; only a bare `_`
        // is the wildcard. Under the old quote-trimming the quoted arm
        // degraded to the wildcard and swallowed every symbol.
        let underscore = BODY.replace(
            "            (State::Text, ' ') => State::Text,
            (State::Text, _) => State::Word,",
            "            (State::Text, '_') => State::Word,
            (State::Text, _) => State::Text,",
        );
        assert_ne!(
            underscore, BODY,
            "the underscore-arm substitution must apply"
        );
        let helper = scanner_fn("scan", &underscore);
        // A non-underscore symbol reaches the wildcard and stays Text;
        // the quoted '_' arm must not take it.
        assert_eq!(final_state(&helper, "\"x\"")?, "State::Text");
        // The literal underscore itself takes the quoted arm.
        assert_eq!(final_state(&helper, "\"_\"")?, "State::Word");
        Ok(())
    }

    #[test]
    fn escaped_char_literals_fail_closed() -> Result<(), String> {
        // `'\''` under naive quote-trimming degraded to a backslash
        // match; the evaluator must refuse the whole scanner instead.
        let quote = BODY.replace(
            "(State::Text, ' ') => State::Text,",
            "(State::Text, '\\'') => State::Word,",
        );
        assert_ne!(quote, BODY, "the escaped-quote substitution must apply");
        let helper = scanner_fn("scan", &quote);
        assert!(
            scanner_return_value(&helper, &inputs(&[("input", "\"x\"")])).is_none(),
            "an escaped char literal must refuse the scanner"
        );
        let newline = BODY.replace(
            "(State::Text, ' ') => State::Text,",
            "(State::Text, '\\n') => State::Word,",
        );
        assert_ne!(newline, BODY, "the escaped-newline substitution must apply");
        let helper2 = scanner_fn("scan", &newline);
        assert!(
            scanner_return_value(&helper2, &inputs(&[("input", "\"x\"")])).is_none(),
            "an escaped char literal must refuse the scanner"
        );
        Ok(())
    }
}

#[cfg(test)]
mod string_state_tests {
    use super::*;
    use crate::analysis::facts::FunctionSourceRole;
    use crate::analysis::facts::FunctionSummary;
    use crate::domain::SymbolId;
    use std::path::PathBuf;

    fn scanner_fn(name: &str, body: &str) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId(format!("src/lib.rs::{name}")),
            name: name.to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 10,
            body: body.to_string(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            source_role: FunctionSourceRole::Production,
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    fn final_state(helper: &FunctionSummary, input: &str) -> Result<String, String> {
        let mut inputs = ExactInputs::new();
        inputs.insert("input".to_string(), input.to_string());
        match scanner_return_value(helper, &inputs) {
            Some(TypedValue::Str(state)) => Ok(state),
            Some(other) => Err(format!("final state must be a string, got {other:?}")),
            None => Err("scanner must resolve over an exact input".into()),
        }
    }

    const BODY: &str = "pub fn scan(input: &str) -> &'static str {\n    let mut state = \"text\";\n    for symbol in input.chars() {\n        state = match (state, symbol) {\n            (\"text\", ' ') => \"text\",\n            (\"text\", _) => \"word\",\n            (\"word\", ' ') => \"text\",\n            (\"word\", _) => \"word\",\n        };\n    }\n    state\n}";

    #[test]
    fn bare_identifier_arm_states_are_refused() -> Result<(), String> {
        // A bare single-segment identifier in an arm's state pattern or
        // next state may be a variable or binding; refusing it is the
        // token-coincidence guard.
        let pattern = BODY.replace("(\"text\", ' ') => \"text\",", "(text, ' ') => \"text\",");
        let helper = scanner_fn("scan", &pattern);
        assert!(
            scanner_return_value(&helper, &ExactInputs::new()).is_none(),
            "a bare identifier state pattern must not resolve"
        );
        let next = BODY.replace("(\"text\", ' ') => \"text\",", "(\"text\", ' ') => input,");
        let helper2 = scanner_fn("scan", &next);
        assert!(
            scanner_return_value(&helper2, &ExactInputs::new()).is_none(),
            "a bare identifier next state must not resolve"
        );
        // A bare identifier initial falls to the evaluator (variable
        // first); unbound, the scanner refuses it rather than reading
        // the text as a state.
        let initial = BODY.replace("let mut state = \"text\";", "let mut state = word;");
        let helper3 = scanner_fn("scan", &initial);
        assert!(
            scanner_return_value(&helper3, &ExactInputs::new()).is_none(),
            "an unbound bare identifier initial must not resolve"
        );
        Ok(())
    }

    #[test]
    fn string_states_resolve_and_render_quoted() -> Result<(), String> {
        let helper = scanner_fn("scan", BODY);
        let mut inputs = ExactInputs::new();
        inputs.insert("input".to_string(), "\"ab\"".to_string());
        let value =
            scanner_return_value(&helper, &inputs).ok_or("string-state scanner must resolve")?;
        assert_eq!(value.render(), "\"word\"");
        assert_eq!(final_state(&helper, "\"ab \"")?, "text");
        Ok(())
    }

    #[test]
    fn initial_state_from_exact_input_identifier() -> Result<(), String> {
        // The initial state itself comes from a bound parameter.
        let body = BODY.replace("let mut state = \"text\";", "let mut state = start;");
        let helper = scanner_fn("scan", &body);
        let mut inputs = ExactInputs::new();
        inputs.insert("input".to_string(), "\" \"".to_string());
        inputs.insert("start".to_string(), "\"word\"".to_string());
        match scanner_return_value(&helper, &inputs) {
            Some(TypedValue::Str(state)) => assert_eq!(state, "text"),
            other => return Err(format!("must resolve from bound start, got {other:?}")),
        }
        Ok(())
    }

    #[test]
    fn escaped_or_empty_string_states_fail_closed() -> Result<(), String> {
        // Escapes are not plain string states: no resolution.
        let body = BODY.replace(
            "(\"text\", ' ') => \"text\",",
            "(\"text\", ' ') => \"te\\\"xt\",",
        );
        let helper = scanner_fn("scan", &body);
        assert!(
            scanner_return_value(&helper, &ExactInputs::new()).is_none(),
            "an escaped state literal must not resolve"
        );
        // Empty string state: not a state token.
        let body2 = BODY.replace("(\"text\", ' ') => \"text\",", "(\"text\", ' ') => \"\",");
        let helper2 = scanner_fn("scan", &body2);
        assert!(
            scanner_return_value(&helper2, &ExactInputs::new()).is_none(),
            "an empty state literal must not resolve"
        );
        Ok(())
    }
}
