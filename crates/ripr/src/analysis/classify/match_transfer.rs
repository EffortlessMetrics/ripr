//! Bounded literal-driven `match` transfer (#3296, the match-arm arm
//! of the #3215 helper-evidence slice).
//!
//! A match helper has the exact shape the corpus pins:
//!
//! ```text
//! match <exact scrutinee> {
//!     "literal" => "value",
//!     _ => "value",
//! }
//! ```
//!
//! as the whole helper body (a tail expression). When the scrutinee
//! resolves exactly (#3295 facts) and every arm's pattern is a plain
//! string literal or `_` and every arm's value is a plain string
//! literal or one nested direct call to a unique helper, the
//! evaluator returns the first arm whose literal equals the scrutinee
//! — source order is authoritative, exactly like the Rust match. A
//! nested call resolves through the shared bounded context (the
//! #3296 recursive row): distinct `(helper, bound inputs)` states
//! unroll within the explicit hop bound, while a repeated state (a
//! true cycle) or the bound itself refuses. Anything else — a guard,
//! an alternative pattern, a computed value, a bare-identifier
//! pattern, a non-string scrutinee, any statement after the closing
//! brace — refuses the whole shape and returns `None`; the caller's
//! existing limitation wording stays the disclosure.

use super::helper_transfer::HelperEval;
use super::value_transfer::{EvalOutcome, ExactInputs, TypedValue, evaluate_initializer};
use crate::analysis::language::mask_rust_comments_and_strings;

/// The parsed match body: the exact scrutinee expression and the
/// literal arms in source order.
struct MatchShape {
    scrutinee: String,
    arms: Vec<MatchArm>,
}

/// An arm's value: a plain string literal, or one nested direct call
/// to a unique helper (`label_of("a")`) resolved through the shared
/// bounded context (#3296 recursive row).
enum MatchValue {
    Literal(String),
    Call(String),
}

struct MatchArm {
    /// `None` is the `_` wildcard arm; `Some(literal)` is a plain
    /// string-literal pattern. A wildcard still respects source order.
    pattern: Option<String>,
    value: MatchValue,
}

/// Evaluate a match helper's return over its exact inputs, when the
/// helper body is one literal `match` tail expression.
pub(crate) fn match_return_value(
    helper: &crate::analysis::facts::FunctionSummary,
    inputs: &ExactInputs,
    eval: &HelperEval<'_>,
) -> Option<TypedValue> {
    let shape = MatchShape::parse(&helper.body)?;
    let subject = match evaluate_initializer(&shape.scrutinee, inputs) {
        EvalOutcome::Exact {
            value: TypedValue::Str(text),
            ..
        } => text,
        _ => return None,
    };
    for arm in &shape.arms {
        let selected = !matches!(&arm.pattern, Some(literal) if literal != &subject);
        if selected {
            return match &arm.value {
                MatchValue::Literal(value) => Some(TypedValue::Str(value.clone())),
                MatchValue::Call(call) => {
                    super::helper_transfer::nested_call_value(call, inputs, eval)
                }
            };
        }
    }
    // No arm matched and no wildcard was reached.
    None
}

impl MatchShape {
    /// Parse the match shape from raw function-body text. Returns
    /// `None` for any other body — including let-chains, statements
    /// after the match, or a match that is not the tail expression —
    /// so the other helper evaluators keep their own authority.
    fn parse(body: &str) -> Option<MatchShape> {
        let masked = mask_rust_comments_and_strings(body);
        let raw_lines: Vec<&str> = body.lines().collect();
        let masked_lines: Vec<&str> = masked.lines().collect();
        if raw_lines.len() != masked_lines.len() {
            return None;
        }
        let mut scrutinee: Option<String> = None;
        let mut arms: Vec<MatchArm> = Vec::new();
        let mut saw_match_close = false;
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
            if saw_match_close {
                // Only the function's closing brace may follow the
                // match; the match is the tail expression.
                if trimmed != "}" {
                    return None;
                }
                continue;
            }
            if scrutinee.is_none() {
                // `match <scrutinee> {` — the structure check uses the
                // masked line, the scrutinee text the raw line
                // (masking erases string literals, and a literal or
                // family scrutinee is supported through #3295).
                if !trimmed.starts_with("match ") || !trimmed.ends_with('{') {
                    return None;
                }
                let rest = raw.strip_prefix("match ")?.strip_suffix('{')?;
                scrutinee = Some(rest.trim().to_string());
                continue;
            }
            if trimmed == "}" {
                saw_match_close = true;
                continue;
            }
            if let Some(arm) = parse_arm(raw) {
                arms.push(arm);
                continue;
            }
            // Any other line inside the match is an unsupported edge.
            return None;
        }
        if !saw_match_close || arms.is_empty() {
            return None;
        }
        Some(MatchShape {
            scrutinee: scrutinee?,
            arms,
        })
    }
}

/// `"literal" => "value",` — the pattern is a plain string literal or
/// `_`; the value is a plain string literal or one nested direct call
/// (`label_of("a")`) to a unique helper with literal or bound-parameter
/// arguments. Guards, alternatives, bindings, paths, and escapes all
/// refuse the arm (and with it the whole helper): an unresolved
/// pattern could match anything, and any other value shape is not an
/// exact return the shared authorities can evaluate.
fn parse_arm(line: &str) -> Option<MatchArm> {
    let trimmed = line.trim().trim_end_matches(',').trim();
    let (pattern, value) = trimmed.split_once(" => ")?;
    let pattern = if pattern == "_" {
        None
    } else {
        Some(literal_string(pattern)?)
    };
    let value = if let Some(literal) = literal_string(value) {
        MatchValue::Literal(literal)
    } else {
        direct_call(value)?;
        MatchValue::Call(value.to_string())
    };
    Some(MatchArm { pattern, value })
}

/// A single direct-call shape: `name(args)` with an identifier callee
/// and a balanced argument list. Full resolution (uniqueness,
/// splittability, binding) happens in the shared nested-call
/// authority, which refuses anything unsupported.
fn direct_call(text: &str) -> Option<()> {
    let trimmed = text.trim();
    let name = trimmed.split('(').next()?;
    if name.is_empty()
        || !name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        || !trimmed.contains('(')
        || !trimmed.ends_with(')')
    {
        return None;
    }
    Some(())
}

/// A plain string literal: non-empty, no quote, backslash, apostrophe,
/// or newline inside — the same conservative literal rule the scanner
/// states use.
fn literal_string(text: &str) -> Option<String> {
    let inner = text.strip_prefix('"')?.strip_suffix('"')?;
    if !inner.is_empty() && !inner.contains(['"', '\\', '\'', '\n']) {
        Some(inner.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::facts::FunctionSourceRole;
    use crate::analysis::facts::{FunctionSummary, RustIndex};
    use crate::domain::SymbolId;
    use std::path::PathBuf;

    fn match_fn(name: &str, body: &str) -> FunctionSummary {
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

    fn inputs(pairs: &[(&str, &str)]) -> ExactInputs {
        let mut inputs = ExactInputs::new();
        for (parameter, literal) in pairs {
            inputs.insert(parameter.to_string(), literal.to_string());
        }
        inputs
    }

    fn root_eval(index: &RustIndex) -> HelperEval<'_> {
        HelperEval::root(index, true)
    }

    fn returned(helper: &FunctionSummary, kind: &str) -> Result<String, String> {
        let index = RustIndex::default();
        let eval = root_eval(&index);
        let bound = inputs(&[("kind", kind)]);
        match match_return_value(helper, &bound, &eval) {
            Some(value) => match value {
                TypedValue::Str(text) => Ok(text),
                other => Err(format!("expected a string value, got {}", other.render())),
            },
            None => Err("match must resolve over an exact input".into()),
        }
    }

    const BODY: &str = "pub fn label(kind: &str) -> &'static str {
    match kind {
        \"word\" => \"alpha\",
        \"text\" => \"beta\",
        _ => \"other\",
    }
}";

    #[test]
    fn match_resolves_each_supported_arm() -> Result<(), String> {
        let helper = match_fn("label", BODY);
        assert_eq!(returned(&helper, "\"word\"")?, "alpha");
        assert_eq!(returned(&helper, "\"text\"")?, "beta");
        // A value no literal arm names reaches the wildcard in order.
        assert_eq!(returned(&helper, "\"other\"")?, "other");
        Ok(())
    }

    #[test]
    fn wildcard_and_duplicate_arms_respect_source_order() -> Result<(), String> {
        // A wildcard placed first wins everything, exactly like the
        // Rust match; a duplicate literal keeps the first arm.
        let reordered = BODY.replace(
            "        \"word\" => \"alpha\",\n        \"text\" => \"beta\",\n        _ => \"other\",",
            "        _ => \"first\",\n        \"word\" => \"alpha\",\n        \"word\" => \"second\",",
        );
        assert_ne!(reordered, BODY, "the reordered-arm substitution must apply");
        let helper = match_fn("label", &reordered);
        assert_eq!(returned(&helper, "\"word\"")?, "first");
        assert_eq!(returned(&helper, "\"text\"")?, "first");
        Ok(())
    }

    #[test]
    fn no_matching_arm_without_wildcard_stops() -> Result<(), String> {
        let no_wildcard = BODY.replace("        _ => \"other\",\n", "");
        assert_ne!(
            no_wildcard, BODY,
            "the wildcard-removal substitution must apply"
        );
        let helper = match_fn("label", &no_wildcard);
        let index = RustIndex::default();
        let eval = root_eval(&index);
        let bound = inputs(&[("kind", "\"other\"")]);
        assert!(
            match_return_value(&helper, &bound, &eval).is_none(),
            "a scrutinee no literal names and no wildcard catches must stop"
        );
        Ok(())
    }

    #[test]
    fn guard_alternative_binding_and_computed_shapes_fail_closed() -> Result<(), String> {
        let cases: [(&str, &str); 5] = [
            (
                "guard arm",
                "        \"word\" if kind.len() > 2 => \"alpha\",",
            ),
            (
                "alternative pattern",
                "        \"word\" | \"text\" => \"alpha\",",
            ),
            ("bare binding pattern", "        word => \"alpha\","),
            (
                "computed argument in a call value",
                "        \"word\" => pick(kind.trim()),",
            ),
            (
                "escaped literal pattern",
                "        \"a\\\"b\" => \"alpha\",",
            ),
        ];
        let index = RustIndex::default();
        let eval = root_eval(&index);
        for (name, arm) in cases {
            let body = BODY.replace("        \"word\" => \"alpha\",", arm);
            assert_ne!(body, BODY, "the {name} substitution must apply");
            let helper = match_fn("label", &body);
            assert!(
                match_return_value(&helper, &inputs(&[("kind", "\"word\"")]), &eval).is_none(),
                "a {name} arm must refuse the match"
            );
        }
        Ok(())
    }

    #[test]
    fn non_string_scrutinee_and_non_tail_bodies_fail_closed() -> Result<(), String> {
        let index = RustIndex::default();
        let eval = root_eval(&index);
        // A char scrutinee is a named limitation, not a guessed match.
        let char_scrutinee = BODY.replace("match kind", "match c");
        let helper = match_fn("label", &char_scrutinee);
        let bound = inputs(&[("c", "'a'")]);
        assert!(
            match_return_value(&helper, &bound, &eval).is_none(),
            "a non-string scrutinee must refuse the match"
        );
        // A statement after the closing brace is not the pinned shape.
        let with_tail = BODY.replace("    }\n}", "    }\n    kind\n}");
        assert_ne!(
            with_tail, BODY,
            "the tail-statement substitution must apply"
        );
        let helper2 = match_fn("label", &with_tail);
        assert!(
            match_return_value(&helper2, &inputs(&[("kind", "\"word\"")]), &eval).is_none(),
            "a body with a statement after the match must refuse"
        );
        // An unresolvable scrutinee never guesses.
        let unknown = BODY.replace("match kind", "match unknown_var");
        let helper3 = match_fn("label", &unknown);
        assert!(
            match_return_value(&helper3, &inputs(&[("kind", "\"word\"")]), &eval).is_none(),
            "an unbound scrutinee must refuse the match"
        );
        Ok(())
    }

    #[test]
    fn nested_call_values_resolve_within_the_hop_bound() -> Result<(), String> {
        // One self-recursive helper: label_of("word") re-enters with
        // "text" — two distinct (helper, inputs) states on one path,
        // inside the hop bound. The real entry (helper_return_value)
        // owns the depth accounting, so the test goes through it.
        let body = BODY.replace(
            "        \"word\" => \"alpha\",",
            "        \"word\" => label_of(\"text\"),",
        );
        assert_ne!(body, BODY, "the nested-call substitution must apply");
        let helper = match_fn("label_of", &body);
        let index = RustIndex {
            functions: vec![helper.clone()],
            ..RustIndex::default()
        };
        let eval = root_eval(&index);
        let bound = inputs(&[("kind", "\"word\"")]);
        match super::super::helper_transfer::helper_return_value(&helper, &bound, &eval) {
            Some(TypedValue::Str(text)) => assert_eq!(text, "beta"),
            other => {
                return Err(format!(
                    "expected the nested call to resolve, got {other:?}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn repeated_states_and_beyond_bound_chains_refuse() -> Result<(), String> {
        // `_ => label_of(kind)` re-enters the same (helper, inputs)
        // state: a true cycle refuses.
        let cyclic = BODY.replace("        _ => \"other\",", "        _ => label_of(kind),");
        assert_ne!(cyclic, BODY, "the cycle substitution must apply");
        let helper = match_fn("label_of", &cyclic);
        let index = RustIndex {
            functions: vec![helper.clone()],
            ..RustIndex::default()
        };
        let eval = root_eval(&index);
        let bound = inputs(&[("kind", "\"zz\"")]);
        assert!(
            super::super::helper_transfer::helper_return_value(&helper, &bound, &eval).is_none(),
            "a repeated (helper, inputs) state must refuse"
        );

        // A self-chain: c -> b -> a is three evaluations (inside the
        // bound); d -> c -> b -> a is four (beyond it).
        let chain = "pub fn label_of(kind: &str) -> &'static str {
    match kind {
        \"a\" => \"alpha\",
        \"b\" => label_of(\"a\"),
        \"c\" => label_of(\"b\"),
        \"d\" => label_of(\"c\"),
    }
}";
        let chained = match_fn("label_of", chain);
        let index = RustIndex {
            functions: vec![chained.clone()],
            ..RustIndex::default()
        };
        let eval = root_eval(&index);
        let inside = inputs(&[("kind", "\"c\"")]);
        match super::super::helper_transfer::helper_return_value(&chained, &inside, &eval) {
            Some(TypedValue::Str(text)) => assert_eq!(text, "alpha"),
            other => {
                return Err(format!(
                    "expected the in-bound chain to resolve, got {other:?}"
                ));
            }
        }
        let beyond = inputs(&[("kind", "\"d\"")]);
        assert!(
            super::super::helper_transfer::helper_return_value(&chained, &beyond, &eval).is_none(),
            "a chain beyond the hop bound must refuse"
        );
        Ok(())
    }
}
