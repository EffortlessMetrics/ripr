//! Static evaluation of a related test's exact-value assertion against the
//! seam owner's body (#6026).
//!
//! A related test that asserts `assert_eq!(owner(literal, ..), literal)` is
//! credited as a strong `exact_value` oracle even when the asserted literal
//! is statically wrong: both the arguments and the expected value are
//! literals, and the owner is frequently one pure integer expression that a
//! constant fold can evaluate. The wrong-value assert is a discriminator
//! pointing the wrong way — it fails at baseline and would pass under the
//! mutation the seam carries — yet static evidence closed the gap on it and
//! reported nothing weak or unknown. Per the repository invariant, a wrong
//! actionable repair signal is worse than a missed advisory finding, so the
//! fold's contradiction must be typed instead of credited.
//!
//! Scope is deliberately narrow and fail-closed toward the current credit:
//! when the owner is not a free function of pure literal arithmetic, when any
//! argument or expected expression does not fold, or when anything about the
//! shape is unexpected, the verdict is [`ExactValueVerdict::NotEvaluable`]
//! and every consumer keeps today's behavior. Only a fold that completes and
//! contradicts the asserted relation downgrades the oracle, states the
//! contradiction in the receipt, and stops closing the gap. The downgrade
//! direction is always weaker, so a fold bug cannot manufacture stronger
//! evidence — it can only withhold it.

use crate::analysis::extract::equality_assertion_arguments;
use crate::analysis::facts::FunctionSummary;
use crate::analysis::syntax::parse_clean_source_file;
use ra_ap_syntax::{AstNode, ast, ast::HasName};
use std::collections::BTreeMap;

/// What static evaluation can say about one related test's equality
/// assertion against the seam owner's body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ExactValueVerdict {
    /// The assertion's operand relation is statically false: the owner folds
    /// to `evaluated` at the asserted literal arguments while the assertion
    /// pins `expected` (equality) or excludes `expected` (inequality). The
    /// test fails at baseline and is not evidence for the seam's behavior.
    Contradicted { expected: String, evaluated: i128 },
    /// The fold completed and agrees with the asserted relation.
    Consistent,
    /// Static evaluation is not possible for this pair. No claim either way;
    /// the current behavior (and its limitation) is retained.
    NotEvaluable,
}

/// Statically evaluate one oracle's equality assertion against `owner`'s
/// body. `owner` is the seam's owning function; `oracle_text` is the
/// assertion fact's text (for example
/// `assert_eq!(discounted_total(5_000, 5_000), 5_000)`).
pub(super) fn exact_value_assertion_verdict(
    owner: Option<&FunctionSummary>,
    oracle_text: &str,
    shadowed: bool,
) -> ExactValueVerdict {
    if shadowed {
        // A `let` binding of the owner's name in the test body shadows the
        // crate function; the bare call then folds against a body the
        // assertion never called.
        return ExactValueVerdict::NotEvaluable;
    }
    let Some(owner) = owner else {
        return ExactValueVerdict::NotEvaluable;
    };
    let Some((asserts_inequality, arguments)) = equality_assertion_shape(oracle_text) else {
        return ExactValueVerdict::NotEvaluable;
    };
    let [actual, expected, ..] = arguments.as_slice() else {
        return ExactValueVerdict::NotEvaluable;
    };
    let Some(call_arguments) = owner_call_arguments(actual, &owner.name) else {
        return ExactValueVerdict::NotEvaluable;
    };
    let empty = BTreeMap::new();
    let mut values = Vec::with_capacity(call_arguments.len());
    for argument in &call_arguments {
        let Some(value) = fold_expression(argument, &empty) else {
            return ExactValueVerdict::NotEvaluable;
        };
        values.push(value);
    }
    let Some(expected_value) = fold_expression(expected, &empty) else {
        return ExactValueVerdict::NotEvaluable;
    };
    let Some(actual_value) = fold_owner_call(owner, &values) else {
        return ExactValueVerdict::NotEvaluable;
    };
    // `assert_eq!` fails at baseline exactly when the folded values differ;
    // `assert_ne!` fails exactly when they are equal.
    let contradicted = if asserts_inequality {
        actual_value == expected_value
    } else {
        actual_value != expected_value
    };
    if contradicted {
        ExactValueVerdict::Contradicted {
            expected: expected.trim().to_string(),
            evaluated: actual_value,
        }
    } else {
        ExactValueVerdict::Consistent
    }
}

/// The one-line disclosure a receipt carries when the oracle is downgraded.
pub(super) fn contradiction_summary(expected: &str, evaluated: i128) -> String {
    format!(
        "assertion expected value contradicts static evaluation (asserts {expected}, owner folds to {evaluated})"
    )
}

/// Whether the test's own body binds `owner_name` with a `let` (a local
/// shadow the bare-name fold cannot see through, #6701 review).
pub(super) fn bare_owner_call_is_shadowed(
    test: &crate::analysis::facts::TestFact,
    owner_name: &str,
) -> bool {
    !owner_name.is_empty()
        && test
            .let_bindings
            .iter()
            .any(|binding| binding.name == owner_name)
}

/// Which equality macro family the assertion uses and its first operands.
/// `Some((true, ..))` is the `assert_ne!` family (the asserted relation is
/// inequality); `None` is not an equality assertion at all.
fn equality_assertion_shape(oracle_text: &str) -> Option<(bool, Vec<String>)> {
    let trimmed = oracle_text.trim_start();
    let starts_with_any = |names: &[&str]| {
        names.iter().any(|name| {
            trimmed.starts_with(name) || trimmed.starts_with(format!("::{name}").as_str())
        })
    };
    let asserts_inequality = starts_with_any(&["assert_ne!", "debug_assert_ne!"]);
    let asserts_equality = starts_with_any(&["assert_eq!", "debug_assert_eq!"]);
    if !asserts_equality && !asserts_inequality {
        return None;
    }
    let arguments = equality_assertion_arguments(oracle_text)?;
    Some((asserts_inequality, arguments))
}

/// The call arguments when `operand` is a bare-name call to `owner_name`
/// (`owner(..)`). See the in-function guard for why receivers and
/// qualified paths stay not evaluable.
fn owner_call_arguments(operand: &str, owner_name: &str) -> Option<Vec<String>> {
    let trimmed = operand.trim();
    let open = super::named_call_open_paren_index(trimmed, owner_name)?;
    let before = trimmed[..open - owner_name.len()].trim_end();
    // A bare name only: a method receiver names a different entity than the
    // free function the seam owns, and a qualified path cannot be resolved
    // to the owner's module from the assertion alone, so both stay not
    // evaluable rather than folding against a body the assertion never
    // called (#6701 review).
    if !before.is_empty() {
        return None;
    }
    let inside = super::delimited_contents_at(trimmed, open)?;
    // The call must consume the whole operand: `owner(1).abs()` or
    // `owner(1) + 1` asserts a value the fold of `owner(1)` never computed,
    // so a trailing expression leaves the pair not evaluable instead of
    // contradicting against a transformed result.
    let bytes = trimmed.as_bytes();
    let mut depth = 0i32;
    let mut close = None;
    for (offset, byte) in bytes[open..].iter().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(open + offset);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    if !trimmed[close + 1..].trim().is_empty() {
        return None;
    }
    Some(super::split_top_level_commas(&inside))
}

/// Evaluate the owner's body as a pure integer function of `arguments`.
/// Free functions only: a method or nested fn is reached through a receiver
/// or closure that the bare-path assertion does not name.
fn fold_owner_call(owner: &FunctionSummary, arguments: &[i128]) -> Option<i128> {
    let parse = parse_clean_source_file(owner.body.as_str())?;
    let function = parse
        .tree()
        .syntax()
        .descendants()
        .find_map(ast::Fn::cast)?;
    for ancestor in function.syntax().ancestors().skip(1) {
        if ast::Impl::can_cast(ancestor.kind())
            || ast::Trait::can_cast(ancestor.kind())
            || ast::Fn::can_cast(ancestor.kind())
        {
            return None;
        }
    }
    let param_list = function.param_list()?;
    if param_list.self_param().is_some() {
        return None;
    }
    let mut names = Vec::new();
    let mut widths = Vec::new();
    for param in param_list.params() {
        let ast::Pat::IdentPat(ident) = param.pat()? else {
            return None;
        };
        // A subpattern (`x @ (a, b)`) does not bind the written name alone.
        if ident.pat().is_some() {
            return None;
        }
        names.push(ident.name()?.text().to_string());
        // A declared-width parameter bounds the inputs the real function
        // accepts; an argument outside it names a call that cannot compile.
        let width = param
            .ty()
            .and_then(|ty| declared_integer_range(&ty.syntax().text().to_string()));
        widths.push(width);
    }
    if names.len() != arguments.len() {
        return None;
    }
    for (width, value) in widths.iter().zip(arguments.iter().copied()) {
        if let Some((low, high)) = width
            && !(*low..=*high).contains(&value)
        {
            return None;
        }
    }
    // The fold uses the return type's declared width when it is a
    // recognized integer type: release builds wrap at that width, so a
    // mathematical result outside it does not describe a value the owner
    // can return and the pair stays not evaluable instead of contradicting
    // a passing wrapping assertion (#6701 review).
    // No recognized return type: the fold keeps exact i128 arithmetic,
    // which can only withhold credit.
    let fold_range = function
        .ret_type()
        .and_then(|ret| declared_integer_range(&ret.syntax().text().to_string()));
    let block = function.body()?;
    let environment: BTreeMap<String, i128> =
        names.into_iter().zip(arguments.iter().copied()).collect();
    // Comments and strings carry no arithmetic; masking them first keeps the
    // tokenizer away from brace-bearing content inside either.
    let masked =
        crate::analysis::extract::mask_comments_and_strings(&block.syntax().text().to_string());
    let tokens = tokenize(&masked)?;
    fold_block(&tokens, &environment, fold_range)
}

/// The inclusive value range a recognized Rust integer type admits, or
/// `None` for anything else: `u128`/`i128` exceed the fold's i128
/// arithmetic, and pointer-sized types depend on the analysis target's
/// width, which no established input names here — a fold at the wrong
/// width could contradict a passing wrapping assertion, so they stay not
/// evaluable (#6701 review).
fn declared_integer_range(ty: &str) -> Option<(i128, i128)> {
    // The parser's return-type node carries its `->` arrow; parameter types
    // are bare.
    let compact = ty.replace(' ', "").trim_start_matches("->").to_string();
    match compact.as_str() {
        "u8" => Some((0, i128::from(u8::MAX))),
        "u16" => Some((0, i128::from(u16::MAX))),
        "u32" => Some((0, i128::from(u32::MAX))),
        "u64" => Some((0, i128::from(u64::MAX))),
        "i8" => Some((i128::from(i8::MIN), i128::from(i8::MAX))),
        "i16" => Some((i128::from(i16::MIN), i128::from(i16::MAX))),
        "i32" => Some((i128::from(i32::MIN), i128::from(i32::MAX))),
        "i64" => Some((i128::from(i64::MIN), i128::from(i64::MAX))),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Int(i128),
    Ident(String),
    Symbol(&'static str),
}

const INTEGER_SUFFIXES: [&str; 12] = [
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
];

/// Tokenize masked Rust expression text. Any byte the fold grammar cannot
/// mean (`?`, `|`, `.`, `:`), a non-suffix identifier glued to a number, or
/// a non-ASCII byte fails the whole fold.
fn tokenize(text: &str) -> Option<Vec<Token>> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if byte.is_ascii_digit() {
            let start = index;
            while index < bytes.len() && (bytes[index].is_ascii_digit() || bytes[index] == b'_') {
                index += 1;
            }
            let digits_end = index;
            if index < bytes.len() && (bytes[index].is_ascii_alphabetic() || bytes[index] == b'_') {
                let suffix_start = index;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
                {
                    index += 1;
                }
                if !INTEGER_SUFFIXES.contains(&&text[suffix_start..index]) {
                    return None;
                }
            }
            let digits = text[start..digits_end].replace('_', "");
            let value = digits.parse::<i128>().ok()?;
            tokens.push(Token::Int(value));
            continue;
        }
        if byte.is_ascii_alphabetic() || byte == b'_' {
            let start = index;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
            tokens.push(Token::Ident(text[start..index].to_string()));
            continue;
        }
        let two_byte_symbol = |pair: &[u8]| match pair {
            [b'=', b'='] => "==",
            [b'!', b'='] => "!=",
            [b'<', b'='] => "<=",
            [b'>', b'='] => ">=",
            [b'&', b'&'] => "&&",
            [b'|', b'|'] => "||",
            _ => "",
        };
        let symbol = if index + 2 <= bytes.len() {
            two_byte_symbol(&bytes[index..index + 2])
        } else {
            ""
        };
        let symbol = if !symbol.is_empty() {
            symbol
        } else if !byte.is_ascii() {
            return None;
        } else {
            match byte {
                b'<' => "<",
                b'>' => ">",
                b'+' => "+",
                b'-' => "-",
                b'*' => "*",
                b'/' => "/",
                b'%' => "%",
                b'!' => "!",
                b'(' => "(",
                b')' => ")",
                b'{' => "{",
                b'}' => "}",
                b',' => ",",
                b';' => ";",
                _ => return None,
            }
        };
        index += symbol.len();
        tokens.push(Token::Symbol(symbol));
    }
    Some(tokens)
}

/// A folded value: the only two types a pure integer fold produces.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Value {
    Int(i128),
    Bool(bool),
}

struct Parser<'a> {
    tokens: &'a [Token],
    position: usize,
    environment: &'a BTreeMap<String, i128>,
    /// The owner return type's declared width, when recognized. Every
    /// integer produced inside the body must fit it: release builds wrap at
    /// the declared width, so a mathematical value outside the range does
    /// not describe a value the owner can return.
    range: Option<(i128, i128)>,
}

impl<'a> Parser<'a> {
    fn check_range(&self, value: i128) -> Option<i128> {
        match self.range {
            Some((low, high)) if !(low..=high).contains(&value) => None,
            _ => Some(value),
        }
    }
}

impl<'a> Parser<'a> {
    fn peek_symbol(&self) -> Option<&'static str> {
        match self.tokens.get(self.position) {
            Some(Token::Symbol(symbol)) => Some(symbol),
            _ => None,
        }
    }

    fn peek_ident(&self) -> Option<&str> {
        match self.tokens.get(self.position) {
            Some(Token::Ident(name)) => Some(name.as_str()),
            _ => None,
        }
    }

    fn eat_symbol(&mut self, symbol: &str) -> bool {
        if self.peek_symbol() == Some(symbol) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn expect_symbol(&mut self, symbol: &str) -> Option<()> {
        self.eat_symbol(symbol).then_some(())
    }

    /// A `{ .. }` block: a sequence of expression statements and one optional
    /// trailing expression. `let` bindings, `return`, and any statement shape
    /// outside the fold leave it unevaluated instead of being guessed at.
    fn parse_block_value(&mut self) -> Option<Value> {
        self.expect_symbol("{")?;
        let mut value = None;
        loop {
            match self.tokens.get(self.position) {
                None => return None,
                Some(Token::Symbol("}")) => {
                    self.position += 1;
                    return value;
                }
                _ => {}
            }
            if matches!(self.peek_ident(), Some("let" | "return")) {
                return None;
            }
            value = Some(self.parse_expression()?);
            match self.peek_symbol() {
                Some(";") => {
                    self.position += 1;
                    value = None;
                }
                Some("}") => {}
                _ => return None,
            }
        }
    }

    fn parse_expression(&mut self) -> Option<Value> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Option<Value> {
        let mut left = self.parse_and()?;
        while self.eat_symbol("||") {
            // Rust short-circuits: a deciding LHS never evaluates the RHS,
            // so an undefined RHS (a division by zero the run never reaches)
            // must not make the fold silent (#6701 review).
            if bool_value(left)? {
                self.skip_expression()?;
                left = Value::Bool(true);
            } else {
                left = Value::Bool(bool_value(self.parse_and()?)?);
            }
        }
        Some(left)
    }

    fn parse_and(&mut self) -> Option<Value> {
        let mut left = self.parse_comparison()?;
        while self.eat_symbol("&&") {
            if !bool_value(left)? {
                self.skip_expression()?;
                left = Value::Bool(false);
            } else {
                left = Value::Bool(bool_value(self.parse_comparison()?)?);
            }
        }
        Some(left)
    }

    /// Consumes one expression's tokens without evaluating it (the untaken
    /// branch of an `if` or the short-circuited side of `&&`/`||`): balanced
    /// delimiters, no semantics. `let`/`return`/calls stay un-parseable so
    /// a skipped region cannot smuggle new grammar in.
    fn skip_expression(&mut self) -> Option<()> {
        self.skip_or()
    }

    fn skip_or(&mut self) -> Option<()> {
        self.skip_and()?;
        while self.eat_symbol("||") {
            self.skip_and()?;
        }
        Some(())
    }

    fn skip_and(&mut self) -> Option<()> {
        self.skip_comparison()?;
        while self.eat_symbol("&&") {
            self.skip_comparison()?;
        }
        Some(())
    }

    fn skip_comparison(&mut self) -> Option<()> {
        self.skip_additive()?;
        if matches!(
            self.peek_symbol(),
            Some("==" | "!=" | "<" | "<=" | ">" | ">=")
        ) {
            self.position += 1;
            self.skip_additive()?;
        }
        Some(())
    }

    fn skip_additive(&mut self) -> Option<()> {
        self.skip_multiplicative()?;
        while matches!(self.peek_symbol(), Some("+" | "-")) {
            self.position += 1;
            self.skip_multiplicative()?;
        }
        Some(())
    }

    fn skip_multiplicative(&mut self) -> Option<()> {
        self.skip_unary()?;
        while matches!(self.peek_symbol(), Some("*" | "/" | "%")) {
            self.position += 1;
            self.skip_unary()?;
        }
        Some(())
    }

    fn skip_unary(&mut self) -> Option<()> {
        if matches!(self.peek_symbol(), Some("-" | "!")) {
            self.position += 1;
            return self.skip_unary();
        }
        self.skip_primary()
    }

    fn skip_primary(&mut self) -> Option<()> {
        match self.tokens.get(self.position) {
            Some(Token::Symbol("(")) => {
                self.position += 1;
                self.skip_or()?;
                self.expect_symbol(")")
            }
            Some(Token::Symbol("{")) => self.skip_balanced_braces(),
            Some(Token::Ident(name)) if name == "if" => {
                self.position += 1;
                self.skip_or()?;
                self.skip_balanced_braces()?;
                if self.peek_ident() == Some("else") {
                    self.position += 1;
                    if self.peek_ident() == Some("if") {
                        self.position += 1;
                        self.skip_or()?;
                        self.skip_balanced_braces()?;
                    } else {
                        self.skip_balanced_braces()?;
                    }
                }
                Some(())
            }
            Some(Token::Int(_) | Token::Ident(_)) => {
                self.position += 1;
                Some(())
            }
            _ => None,
        }
    }

    /// Consumes one balanced `{ .. }` region without evaluating it.
    fn skip_balanced_braces(&mut self) -> Option<()> {
        self.expect_symbol("{")?;
        let mut depth = 1i32;
        while depth > 0 {
            match self.tokens.get(self.position) {
                Some(Token::Symbol("{")) => depth += 1,
                Some(Token::Symbol("}")) => depth -= 1,
                None => return None,
                _ => {}
            }
            self.position += 1;
        }
        Some(())
    }

    fn parse_comparison(&mut self) -> Option<Value> {
        let left = self.parse_additive()?;
        let Some(op) = self.peek_symbol() else {
            return Some(left);
        };
        if !matches!(op, "==" | "!=" | "<" | "<=" | ">" | ">=") {
            return Some(left);
        }
        self.position += 1;
        let right = self.parse_additive()?;
        match (left, right) {
            (Value::Int(left), Value::Int(right)) => {
                let result = match op {
                    "==" => left == right,
                    "!=" => left != right,
                    "<" => left < right,
                    "<=" => left <= right,
                    ">" => left > right,
                    _ => left >= right,
                };
                Some(Value::Bool(result))
            }
            (Value::Bool(left), Value::Bool(right)) if matches!(op, "==" | "!=") => {
                Some(Value::Bool(if op == "==" {
                    left == right
                } else {
                    left != right
                }))
            }
            _ => None,
        }
    }

    fn parse_additive(&mut self) -> Option<Value> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.peek_symbol() {
                Some("+") => "+",
                Some("-") => "-",
                _ => return Some(left),
            };
            self.position += 1;
            let right = self.parse_multiplicative()?;
            let Value::Int(left_value) = left else {
                return None;
            };
            let Value::Int(right_value) = right else {
                return None;
            };
            left = Value::Int(self.check_range(match op {
                "+" => left_value.checked_add(right_value)?,
                _ => left_value.checked_sub(right_value)?,
            })?);
        }
    }

    fn parse_multiplicative(&mut self) -> Option<Value> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek_symbol() {
                Some("*") => "*",
                Some("/") => "/",
                Some("%") => "%",
                _ => return Some(left),
            };
            self.position += 1;
            let right = self.parse_unary()?;
            let Value::Int(left_value) = left else {
                return None;
            };
            let Value::Int(right_value) = right else {
                return None;
            };
            left = Value::Int(self.check_range(match op {
                "*" => left_value.checked_mul(right_value)?,
                "/" => left_value.checked_div(right_value)?,
                _ => left_value.checked_rem(right_value)?,
            })?);
        }
    }

    fn parse_unary(&mut self) -> Option<Value> {
        if self.eat_symbol("-") {
            let Value::Int(value) = self.parse_unary()? else {
                return None;
            };
            return Some(Value::Int(self.check_range(value.checked_neg()?)?));
        }
        if self.eat_symbol("!") {
            let value = self.parse_unary()?;
            return Some(Value::Bool(!bool_value(value)?));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Option<Value> {
        match self.tokens.get(self.position) {
            Some(Token::Symbol("(")) => {
                self.position += 1;
                let value = self.parse_expression()?;
                self.expect_symbol(")")?;
                Some(value)
            }
            Some(Token::Symbol("{")) => self.parse_block_value(),
            // Keywords lex as identifiers: `if` opens the fold's only
            // control form.
            Some(Token::Ident(name)) if name == "if" => {
                self.position += 1;
                self.parse_if_tail()
            }
            Some(Token::Int(value)) => {
                let value = *value;
                self.position += 1;
                Some(Value::Int(value))
            }
            Some(Token::Ident(name)) => {
                let name = name.clone();
                self.position += 1;
                match name.as_str() {
                    "true" => return Some(Value::Bool(true)),
                    "false" => return Some(Value::Bool(false)),
                    _ => {}
                }
                // Calls, method chains, paths and field access are outside
                // the fold's scope; only bound parameters evaluate.
                match self.peek_symbol() {
                    Some("(" | "." | "::") => None,
                    _ => self.environment.get(&name).copied().map(Value::Int),
                }
            }
            _ => None,
        }
    }

    /// The `if` keyword has been consumed; parse the condition, the taken
    /// block, and (when present) the `else` arm. An `if` without `else` is
    /// evaluable only when the condition folds true.
    fn parse_if_tail(&mut self) -> Option<Value> {
        let condition = bool_value(self.parse_expression()?)?;
        if condition {
            let taken = self.parse_block_value()?;
            // The untaken arm is consumed syntactically but never folded: a
            // division by zero or overflow the run would never reach must
            // not silence the fold (#6701 review).
            if self.peek_ident() == Some("else") {
                self.position += 1;
                if self.peek_ident() == Some("if") {
                    self.position += 1;
                    self.skip_if_tail_syntax()?;
                } else {
                    self.skip_balanced_braces()?;
                }
            }
            Some(taken)
        } else {
            // Skip the taken block syntactically, then evaluate the else
            // arm. An `if` without `else` is not evaluable when false.
            self.skip_balanced_braces()?;
            if self.peek_ident() != Some("else") {
                return None;
            }
            self.position += 1;
            let alternative = if self.peek_ident() == Some("if") {
                self.position += 1;
                self.parse_if_tail()?
            } else {
                self.parse_block_value()?
            };
            Some(alternative)
        }
    }

    /// Skips an `else if` chain syntactically (the `if` already consumed).
    fn skip_if_tail_syntax(&mut self) -> Option<()> {
        self.skip_or()?;
        self.skip_balanced_braces()?;
        if self.peek_ident() == Some("else") {
            self.position += 1;
            if self.peek_ident() == Some("if") {
                self.position += 1;
                self.skip_if_tail_syntax()?;
            } else {
                self.skip_balanced_braces()?;
            }
        }
        Some(())
    }
}

fn bool_value(value: Value) -> Option<bool> {
    match value {
        Value::Bool(value) => Some(value),
        Value::Int(_) => None,
    }
}

/// Fold a standalone expression (an assertion operand) with no bound names.
fn fold_expression(text: &str, environment: &BTreeMap<String, i128>) -> Option<i128> {
    let masked = crate::analysis::extract::mask_comments_and_strings(text);
    let tokens = tokenize(&masked)?;
    let mut parser = Parser {
        tokens: &tokens,
        position: 0,
        environment,
        range: None,
    };
    let value = parser.parse_expression()?;
    if parser.position != tokens.len() {
        return None;
    }
    match value {
        Value::Int(value) => Some(value),
        Value::Bool(_) => None,
    }
}

/// Fold a `{ .. }` block with `environment` as the parameter bindings and
/// `range` as the return type's declared width (no width when `None`).
fn fold_block(
    tokens: &[Token],
    environment: &BTreeMap<String, i128>,
    range: Option<(i128, i128)>,
) -> Option<i128> {
    let mut parser = Parser {
        tokens,
        position: 0,
        environment,
        range,
    };
    let value = parser.parse_block_value()?;
    if parser.position != tokens.len() {
        return None;
    }
    match value {
        Value::Int(value) => Some(value),
        Value::Bool(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::rust_index::{RaRustSyntaxAdapter, RustSyntaxAdapter};

    fn verdict_for_owner(owner: &str, assertion: &str) -> Result<ExactValueVerdict, String> {
        let adapter = RaRustSyntaxAdapter;
        let facts = adapter
            .summarize_file(std::path::Path::new("src/lib.rs"), &format!("{owner}\n"))
            .map_err(|error| format!("owner source summarizes: {error}"))?;
        let owner = facts
            .functions
            .iter()
            .find(|function| function.name == "discounted_total")
            .ok_or("owner function is indexed")?;
        Ok(exact_value_assertion_verdict(Some(owner), assertion, false))
    }

    const BOUNDARY_OWNER: &str = r#"
pub fn discounted_total(amount_cents: u64, discount_threshold_cents: u64) -> u64 {
    if amount_cents >= discount_threshold_cents {
        amount_cents * 70 / 100
    } else {
        amount_cents
    }
}
"#;

    #[test]
    fn wrongval_assertion_of_the_mutants_value_is_contradicted() -> Result<(), String> {
        let verdict = verdict_for_owner(
            BOUNDARY_OWNER,
            "assert_eq!(discounted_total(5_000, 5_000), 5_000);",
        );
        assert_eq!(
            verdict?,
            ExactValueVerdict::Contradicted {
                expected: "5_000".to_string(),
                evaluated: 3_500,
            }
        );
        Ok(())
    }

    #[test]
    fn correct_boundary_literal_is_consistent() -> Result<(), String> {
        let verdict = verdict_for_owner(
            BOUNDARY_OWNER,
            "assert_eq!(discounted_total(5_000, 5_000), 3_500);",
        );
        assert_eq!(verdict?, ExactValueVerdict::Consistent);
        Ok(())
    }

    #[test]
    fn non_boundary_literal_is_consistent() -> Result<(), String> {
        let verdict = verdict_for_owner(
            BOUNDARY_OWNER,
            "assert_eq!(discounted_total(1_000, 5_000), 1_000);",
        );
        assert_eq!(verdict?, ExactValueVerdict::Consistent);
        Ok(())
    }

    #[test]
    fn non_literal_expected_value_is_not_evaluable() -> Result<(), String> {
        let verdict = verdict_for_owner(
            BOUNDARY_OWNER,
            "assert_eq!(discounted_total(5_000, 5_000), expected_total());",
        );
        assert_eq!(verdict?, ExactValueVerdict::NotEvaluable);
        Ok(())
    }

    #[test]
    fn non_literal_call_argument_is_not_evaluable() -> Result<(), String> {
        let verdict = verdict_for_owner(
            BOUNDARY_OWNER,
            "assert_eq!(discounted_total(amount, 5_000), 3_500);",
        );
        assert_eq!(verdict?, ExactValueVerdict::NotEvaluable);
        Ok(())
    }

    #[test]
    fn qualified_owner_path_call_is_not_evaluable() -> Result<(), String> {
        // A qualified path may name a same-named function in another module;
        // the assertion alone cannot resolve it to the seam owner, so the
        // fold stays silent instead of contradicting against the wrong body.
        let verdict = verdict_for_owner(
            BOUNDARY_OWNER,
            "assert_eq!(crate::discounted_total(5_000, 5_000), 5_000);",
        )?;
        assert_eq!(verdict, ExactValueVerdict::NotEvaluable);
        Ok(())
    }

    #[test]
    fn owner_that_calls_another_function_is_not_evaluable() -> Result<(), String> {
        let owner = r#"
pub fn discounted_total(amount_cents: u64, discount_threshold_cents: u64) -> u64 {
    if amount_cents >= discount_threshold_cents {
        discount_factor(amount_cents)
    } else {
        amount_cents
    }
}
fn discount_factor(amount_cents: u64) -> u64 { amount_cents * 70 / 100 }
"#;
        let verdict =
            verdict_for_owner(owner, "assert_eq!(discounted_total(5_000, 5_000), 5_000);")?;
        assert_eq!(verdict, ExactValueVerdict::NotEvaluable);
        Ok(())
    }

    #[test]
    fn method_owner_is_not_evaluable() -> Result<(), String> {
        let owner = r#"
pub struct Pricing { pub threshold: u64 }
impl Pricing {
    pub fn discounted_total(&self, amount_cents: u64) -> u64 {
        if amount_cents >= self.threshold {
            amount_cents * 70 / 100
        } else {
            amount_cents
        }
    }
}
"#;
        let verdict =
            verdict_for_owner(owner, "assert_eq!(discounted_total(5_000, 5_000), 5_000);")?;
        assert_eq!(verdict, ExactValueVerdict::NotEvaluable);
        Ok(())
    }

    #[test]
    fn transformed_owner_result_is_not_evaluable() -> Result<(), String> {
        // A method or arithmetic on the call's result asserts a value the
        // owner fold never computed; the fold must stay silent rather than
        // contradict a passing assertion.
        for assertion in [
            "assert_eq!(discounted_total(5_000, 5_000).max(3_500), 3_500);",
            "assert_eq!(discounted_total(5_000, 5_000) - 500, 3_000);",
        ] {
            assert_eq!(
                verdict_for_owner(BOUNDARY_OWNER, assertion)?,
                ExactValueVerdict::NotEvaluable,
                "{assertion}"
            );
        }
        Ok(())
    }

    #[test]
    fn method_call_operand_is_not_evaluable() -> Result<(), String> {
        let verdict = verdict_for_owner(
            BOUNDARY_OWNER,
            "assert_eq!(order.discounted_total(5_000, 5_000), 5_000);",
        );
        assert_eq!(verdict?, ExactValueVerdict::NotEvaluable);
        Ok(())
    }

    #[test]
    fn wrong_assert_ne_is_contradicted_and_correct_ne_is_consistent() -> Result<(), String> {
        let wrong = verdict_for_owner(
            BOUNDARY_OWNER,
            "assert_ne!(discounted_total(5_000, 5_000), 3_500);",
        );
        assert_eq!(
            wrong?,
            ExactValueVerdict::Contradicted {
                expected: "3_500".to_string(),
                evaluated: 3_500,
            }
        );
        let correct = verdict_for_owner(
            BOUNDARY_OWNER,
            "assert_ne!(discounted_total(5_000, 5_000), 5_000);",
        );
        assert_eq!(correct?, ExactValueVerdict::Consistent);
        Ok(())
    }

    #[test]
    fn contradiction_summary_names_both_values() {
        let summary = contradiction_summary("5_000", 3_500);
        assert!(
            summary.contains("5_000") && summary.contains("3500"),
            "{summary}"
        );
        assert!(
            summary.contains("contradicts static evaluation"),
            "{summary}"
        );
    }

    #[test]
    fn else_if_chain_folds() -> Result<(), String> {
        let owner = r#"
pub fn discounted_total(amount_cents: u64, discount_threshold_cents: u64) -> u64 {
    if amount_cents >= discount_threshold_cents {
        amount_cents * 70 / 100
    } else if amount_cents >= discount_threshold_cents / 2 {
        amount_cents * 90 / 100
    } else {
        amount_cents
    }
}
"#;
        // 3_000 < 5_000 takes the else-if arm (3_000 >= 5_000 / 2), which
        // folds to 3_000 * 90 / 100 = 2_700.
        let contradicted =
            verdict_for_owner(owner, "assert_eq!(discounted_total(3_000, 5_000), 3_000)?;");
        assert_eq!(
            contradicted?,
            ExactValueVerdict::Contradicted {
                expected: "3_000".to_string(),
                evaluated: 2_700,
            }
        );
        let consistent =
            verdict_for_owner(owner, "assert_eq!(discounted_total(3_000, 5_000), 2_700)?;");
        assert_eq!(consistent?, ExactValueVerdict::Consistent);
        Ok(())
    }

    #[test]
    fn negative_and_suffixed_literals_fold() -> Result<(), String> {
        let owner = r#"
pub fn discounted_total(amount_cents: i64, discount_threshold_cents: i64) -> i64 {
    if amount_cents >= discount_threshold_cents {
        -amount_cents + 10i64
    } else {
        amount_cents
    }
}
"#;
        // -30 >= -100 takes the discounted arm: -(-30) + 10 = 40.
        let verdict = verdict_for_owner(owner, "assert_eq!(discounted_total(-30, -100), 40)?;");
        assert_eq!(verdict?, ExactValueVerdict::Consistent);
        let verdict = verdict_for_owner(owner, "assert_eq!(discounted_total(-30, -100), -40)?;");
        assert_eq!(
            verdict?,
            ExactValueVerdict::Contradicted {
                expected: "-40".to_string(),
                evaluated: 40,
            }
        );
        Ok(())
    }

    #[test]
    fn division_by_zero_is_not_evaluable() -> Result<(), String> {
        let owner = r#"
pub fn discounted_total(amount_cents: u64, discount_threshold_cents: u64) -> u64 {
    if amount_cents >= discount_threshold_cents {
        amount_cents / discount_threshold_cents
    } else {
        amount_cents
    }
}
"#;
        let verdict = verdict_for_owner(owner, "assert_eq!(discounted_total(10, 0), 0)?;");
        assert_eq!(verdict?, ExactValueVerdict::NotEvaluable);
        Ok(())
    }

    #[test]
    fn debug_equality_macros_folds_like_plain_equality() -> Result<(), String> {
        let wrong = verdict_for_owner(
            BOUNDARY_OWNER,
            "debug_assert_eq!(discounted_total(5_000, 5_000), 5_000);",
        )?;
        assert!(matches!(wrong, ExactValueVerdict::Contradicted { .. }));
        let correct = verdict_for_owner(
            BOUNDARY_OWNER,
            "debug_assert_eq!(discounted_total(5_000, 5_000), 3_500);",
        )?;
        assert_eq!(correct, ExactValueVerdict::Consistent);
        let wrong_ne = verdict_for_owner(
            BOUNDARY_OWNER,
            "debug_assert_ne!(discounted_total(5_000, 5_000), 3_500);",
        )?;
        assert!(matches!(wrong_ne, ExactValueVerdict::Contradicted { .. }));
        Ok(())
    }

    #[test]
    fn declared_width_overflow_is_not_evaluable() -> Result<(), String> {
        // `fn next(x: u8) -> u8 { x + 1 }` at 255 wraps in release builds;
        // the mathematical fold (256) does not describe a value the owner
        // can return, so the pair stays not evaluable instead of
        // contradicting a passing wrapping assertion.
        let owner = "pub fn next(x: u8) -> u8 { x + 1 }";
        let adapter = RaRustSyntaxAdapter;
        let facts = adapter
            .summarize_file(
                std::path::Path::new("src/lib.rs"),
                &format!(
                    "{owner}
"
                ),
            )
            .map_err(|error| format!("owner source summarizes: {error}"))?;
        let owner_fn = facts
            .functions
            .iter()
            .find(|function| function.name == "next")
            .ok_or("owner function is indexed")?;
        let verdict =
            exact_value_assertion_verdict(Some(owner_fn), "assert_eq!(next(255), 0);", false);
        assert_eq!(verdict, ExactValueVerdict::NotEvaluable);
        // A value outside the declared width cannot be returned at all, so
        // an assertion pinning one stays not evaluable too.
        let verdict =
            exact_value_assertion_verdict(Some(owner_fn), "assert_eq!(next(255), 256);", false);
        assert_eq!(verdict, ExactValueVerdict::NotEvaluable);
        // Inside the declared width the same shape folds normally.
        let verdict =
            exact_value_assertion_verdict(Some(owner_fn), "assert_eq!(next(2), 3);", false);
        assert_eq!(verdict, ExactValueVerdict::Consistent);
        let verdict =
            exact_value_assertion_verdict(Some(owner_fn), "assert_eq!(next(2), 4);", false);
        assert!(matches!(verdict, ExactValueVerdict::Contradicted { .. }));
        Ok(())
    }

    #[test]
    fn untaken_branches_do_not_silence_the_fold() -> Result<(), String> {
        // `if x == 0 { 0 } else { 10 / x }` at 0 never reaches the
        // division; the fold evaluates only the selected branch, so the
        // baseline-failing assert is contradicted instead of keeping its
        // strong credit behind an undefined untaken arm (#6701 review).
        let owner = r#"
pub fn discounted_total(amount_cents: i64, discount_threshold_cents: i64) -> i64 {
    if amount_cents == 0 {
        0
    } else {
        10 / amount_cents
    }
}
"#;
        let verdict = verdict_for_owner(owner, "assert_eq!(discounted_total(0, 5), 1);")?;
        assert!(matches!(verdict, ExactValueVerdict::Contradicted { .. }));
        // The guard branch itself is honored: at 5 the division runs (2).
        let verdict = verdict_for_owner(owner, "assert_eq!(discounted_total(5, 5), 3);")?;
        assert!(matches!(verdict, ExactValueVerdict::Contradicted { .. }));
        let verdict = verdict_for_owner(owner, "assert_eq!(discounted_total(5, 5), 2);")?;
        assert_eq!(verdict, ExactValueVerdict::Consistent);
        Ok(())
    }

    #[test]
    fn short_circuit_operands_skip_undefined_sides() -> Result<(), String> {
        // `x != 0 && 10 / x > 1` at 0 short-circuits before the division.
        let owner = r#"
pub fn discounted_total(amount_cents: i64, discount_threshold_cents: i64) -> i64 {
    if amount_cents != 0 && 10 / amount_cents > 1 {
        10 / amount_cents
    } else {
        0
    }
}
"#;
        let verdict = verdict_for_owner(owner, "assert_eq!(discounted_total(0, 5), 1);")?;
        assert!(matches!(verdict, ExactValueVerdict::Contradicted { .. }));
        let verdict = verdict_for_owner(owner, "assert_eq!(discounted_total(0, 5), 0);")?;
        assert_eq!(verdict, ExactValueVerdict::Consistent);
        Ok(())
    }

    #[test]
    fn local_shadow_of_the_owner_name_is_not_evaluable() -> Result<(), String> {
        // A `let` binding of the owner's name shadows the crate function;
        // the bare-name fold must stay silent instead of folding against a
        // body the assertion never called (#6701 review).
        let source = r#"
pub fn discounted_total(amount_cents: u64, discount_threshold_cents: u64) -> u64 {
    if amount_cents >= discount_threshold_cents {
        amount_cents * 70 / 100
    } else {
        amount_cents
    }
}
#[cfg(test)]
mod tests {
    use super::discounted_total;
    #[test]
    fn boundary_uses_a_shadow() {
        let discounted_total = |a: u64, b: u64| a;
        assert_eq!(discounted_total(5_000, 5_000), 5_000);
    }
}
"#;
        let adapter = RaRustSyntaxAdapter;
        let facts = adapter
            .summarize_file(std::path::Path::new("src/lib.rs"), source)
            .map_err(|error| format!("source summarizes: {error}"))?;
        let owner = facts
            .functions
            .iter()
            .find(|function| function.name == "discounted_total")
            .ok_or("owner function is indexed")?;
        let test = facts
            .tests
            .iter()
            .find(|test| test.name == "boundary_uses_a_shadow")
            .ok_or("shadowing test is indexed")?;
        assert!(bare_owner_call_is_shadowed(test, "discounted_total"));
        let assertion = test
            .assertions
            .iter()
            .find(|oracle| oracle.text.contains("assert_eq!"))
            .ok_or("the shadow test carries an equality assertion")?;
        let verdict = exact_value_assertion_verdict(Some(owner), &assertion.text, true);
        assert_eq!(verdict, ExactValueVerdict::NotEvaluable);
        Ok(())
    }

    #[test]
    fn non_equality_assertions_are_not_evaluable() -> Result<(), String> {
        for assertion in [
            "assert!(discounted_total(5_000, 5_000) > 0);",
            "assert!(matches!(discounted_total(5_000, 5_000), 3_500));",
            "assert_eq!(result, Ok(()));",
        ] {
            assert_eq!(
                verdict_for_owner(BOUNDARY_OWNER, assertion)?,
                ExactValueVerdict::NotEvaluable,
                "{assertion}"
            );
        }
        Ok(())
    }
}
